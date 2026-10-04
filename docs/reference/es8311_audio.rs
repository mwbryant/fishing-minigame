// REFERENCE ONLY: not compiled as part of the repo.
//
// Speaker driver written during research on 2026-10-04. It compiled and linked
// (clippy clean) as `mod audio` inside a scratch copy of `firmware/` against
// esp-hal 1.2.2, but it has never run on hardware. The wiring, register
// provenance, and caveats are in docs/ESP32_PLATFORM.md section 10.2. To use it:
// copy it to firmware/src/audio.rs, add `embedded-hal-bus = "0.3"`, and share
// the I2C bus between this and touch via `RefCell` + `RefCellDevice`.

//! Speaker output: ES8311 codec (I2C 0x18) -> NS4150B class-D amp (enable on
//! GPIO46, active high) -> MX1.25 speaker connector, fed by I2S0 TX through a
//! streaming DMA buffer that the game loop tops up once per frame.

use embedded_hal::i2c::I2c;
use esp_hal::{
    Blocking,
    dma::{DmaDescriptor, DmaTxStreamBuf, aligned::DmaAlignedMut},
    gpio::Output,
    i2s::master::{I2sTx, I2sTxDmaTransfer},
};

pub const ES8311_ADDR: u8 = 0x18;
/// Stereo 16-bit frames: the ES8311 DAC plays the left slot, the right slot
/// carries the same sample.
pub const BYTES_PER_FRAME: usize = 4;
/// REG32 value for 0 dB; each step is 0.5 dB (0x00 = -95.5 dB, 0xFF = +32 dB).
pub const VOLUME_0DB: u8 = 0xBF;

/// ES8311 register writes for slave mode, MCLK = 256 * fs supplied on the MCLK
/// pin, 16-bit Philips I2S, DAC only. Mirrors esp_codec_dev's es8311_open +
/// es8311_set_fs + es8311_enable as used by Waveshare's BSP
/// (`use_mclk = true`, `master_mode = false`, `ESP_CODEC_DEV_WORK_MODE_DAC`).
fn init_sequence(sample_rate: u32) -> [(u8, u8); 42] {
    // Every MCLK = 256 * fs row of the coefficient table (8k..48k) has
    // pre_div = 1, pre_mult = 1, adc/dac_div = 1, single speed; only the DAC
    // oversampling differs.
    let dac_osr = if sample_rate <= 16_000 { 0x20 } else { 0x10 };
    [
        // es8311_open
        (0x0D, 0xFA),
        (0x44, 0x08),
        (0x44, 0x08),
        (0x01, 0x30),
        (0x02, 0x00),
        (0x03, 0x10),
        (0x16, 0x24),
        (0x04, 0x10),
        (0x05, 0x00),
        (0x0B, 0x00),
        (0x0C, 0x00),
        (0x10, 0x1F),
        (0x11, 0x7F),
        (0x00, 0x80),
        (0x00, 0x80),
        (0x01, 0x3F),
        (0x06, 0x03),
        (0x13, 0x10),
        (0x1B, 0x0A),
        (0x1C, 0x6A),
        (0x44, 0x58),
        // es8311_set_fs: 16-bit I2S on both serial ports, clock dividers
        (0x09, 0x0C),
        (0x0A, 0x0C),
        (0x02, 0x00),
        (0x05, 0x00),
        (0x03, 0x10),
        (0x04, dac_osr),
        (0x07, 0x00),
        (0x08, 0xFF),
        (0x06, 0x03),
        // es8311_start
        (0x00, 0x80),
        (0x01, 0x3F),
        (0x09, 0x0C),
        (0x0A, 0x0C),
        (0x17, 0xBF),
        (0x0E, 0x02),
        (0x12, 0x00),
        (0x14, 0x1A),
        (0x0D, 0x01),
        (0x15, 0x40),
        (0x37, 0x08),
        (0x45, 0x00),
    ]
}

pub struct Es8311<I2C> {
    i2c: I2C,
}

impl<I2C: I2c> Es8311<I2C> {
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    pub fn read(&mut self, reg: u8) -> Result<u8, I2C::Error> {
        let mut value = [0];
        self.i2c.write_read(ES8311_ADDR, &[reg], &mut value)?;
        Ok(value[0])
    }

    pub fn write(&mut self, reg: u8, value: u8) -> Result<(), I2C::Error> {
        self.i2c.write(ES8311_ADDR, &[reg, value])
    }

    /// Chip ID registers 0xFD/0xFE read 0x83/0x11 on an ES8311.
    pub fn chip_id(&mut self) -> Result<(u8, u8), I2C::Error> {
        Ok((self.read(0xFD)?, self.read(0xFE)?))
    }

    /// Start I2S (and therefore MCLK) before calling this.
    pub fn init(&mut self, sample_rate: u32, amp: &mut Output<'_>) -> Result<(), I2C::Error> {
        for (reg, value) in init_sequence(sample_rate) {
            self.write(reg, value)?;
        }
        amp.set_high();
        self.set_mute(false)?;
        self.set_volume(VOLUME_0DB)
    }

    pub fn set_volume(&mut self, reg32: u8) -> Result<(), I2C::Error> {
        self.write(0x32, reg32)
    }

    pub fn set_mute(&mut self, mute: bool) -> Result<(), I2C::Error> {
        let reg31 = self.read(0x31)? & 0x9F;
        self.write(0x31, if mute { reg31 | 0x60 } else { reg31 })
    }
}

/// Anything that produces one mono sample per call.
pub trait SampleSource {
    fn next_sample(&mut self) -> i16;
}

fn fill_frames(bytes: &mut [u8], source: &mut impl SampleSource) -> usize {
    let whole = bytes.len() / BYTES_PER_FRAME * BYTES_PER_FRAME;
    for frame in bytes[..whole].chunks_exact_mut(BYTES_PER_FRAME) {
        let [lo, hi] = source.next_sample().to_le_bytes();
        frame.copy_from_slice(&[lo, hi, lo, hi]);
    }
    whole
}

/// Non-blocking streaming output. `pump` copies as many frames as the DMA has
/// already consumed, so the queued audio (= latency) stays at about the
/// buffer length. If a frame stalls long enough to drain the buffer, the
/// transfer stops and `pump` restarts it.
pub struct AudioOut {
    transfer: Option<I2sTxDmaTransfer<'static, Blocking, DmaTxStreamBuf>>,
    capacity: usize,
    pub underruns: u32,
}

impl AudioOut {
    pub fn start(
        i2s_tx: I2sTx<'static, Blocking>,
        buffer: DmaTxStreamBuf,
        source: &mut impl SampleSource,
    ) -> Self {
        let (descriptors, bytes) = buffer.split();
        let capacity = bytes.len();
        let mut out = Self {
            transfer: None,
            capacity,
            underruns: 0,
        };
        out.launch(i2s_tx, descriptors, bytes, source);
        out
    }

    fn launch(
        &mut self,
        i2s_tx: I2sTx<'static, Blocking>,
        descriptors: DmaAlignedMut<'static, [DmaDescriptor]>,
        bytes: DmaAlignedMut<'static, [u8]>,
        source: &mut impl SampleSource,
    ) {
        let Ok(mut buffer) = DmaTxStreamBuf::new(descriptors, bytes) else {
            return;
        };
        buffer.push_with(|bytes| fill_frames(bytes, source));
        self.transfer = i2s_tx.write(buffer).ok();
    }

    pub fn pump(&mut self, source: &mut impl SampleSource) -> usize {
        let Some(transfer) = self.transfer.as_mut() else {
            return 0;
        };
        if transfer.is_done() && transfer.available_bytes() >= self.capacity {
            self.underruns += 1;
            let Some(transfer) = self.transfer.take() else {
                return 0;
            };
            let (i2s_tx, buffer) = transfer.stop();
            let (descriptors, bytes) = buffer.split();
            self.launch(i2s_tx, descriptors, bytes, source);
            return self.capacity;
        }
        let mut written = 0;
        loop {
            let pushed = transfer.push_with(|bytes| fill_frames(bytes, source));
            if pushed == 0 {
                return written;
            }
            written += pushed;
        }
    }
}

/// Phase-accumulator oscillator, enough for beeps and a test tone.
pub struct Tone {
    phase: u32,
    step: u32,
    amplitude: i16,
    square: bool,
}

impl Tone {
    pub fn new(freq_hz: u32, sample_rate: u32, amplitude: i16, square: bool) -> Self {
        Self {
            phase: 0,
            step: ((u64::from(freq_hz) << 32) / u64::from(sample_rate)) as u32,
            amplitude,
            square,
        }
    }
}

impl SampleSource for Tone {
    fn next_sample(&mut self) -> i16 {
        self.phase = self.phase.wrapping_add(self.step);
        if self.square {
            return if self.phase < 0x8000_0000 {
                self.amplitude
            } else {
                -self.amplitude
            };
        }
        let x = fast_sin(self.phase);
        (x * f32::from(self.amplitude)) as i16
    }
}

/// sin() of a 32-bit phase via a parabola approximation; avoids pulling in
/// libm for a test tone.
fn fast_sin(phase: u32) -> f32 {
    let t = phase as f32 / 4_294_967_296.0;
    let x = if t < 0.5 { t * 2.0 } else { (t - 0.5) * 2.0 };
    let y = 4.0 * x * (1.0 - x);
    let y = y * (0.775 + 0.225 * y);
    if t < 0.5 { y } else { -y }
}
