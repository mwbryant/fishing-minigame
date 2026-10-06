//! Speaker output: ES8311 codec (I2C 0x18) -> NS4150B class-D amp (enable on
//! GPIO46, active high) -> MX1.25 speaker connector, fed by I2S0 TX through a
//! streaming DMA buffer that the game loop tops up once per frame from the
//! shared `orbit_audio` mixer. Adapted from docs/reference/es8311_audio.rs.

use alloc::{boxed::Box, vec};

use embedded_hal::i2c::I2c;
use esp_hal::{
    Blocking,
    dma::{DmaDescriptor, DmaTxStreamBuf, aligned::DmaAlignedMut},
    gpio::Output,
    i2s::master::{I2sTx, I2sTxDmaTransfer},
};
use log::warn;
use orbit_audio::Mixer;

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

/// About 1/8 s of stereo 16-bit audio at 16 kHz, in eight 1 KB descriptors.
const STREAM_BYTES: usize = 8 * 1024;
const STREAM_DESCRIPTORS: usize = 8;

/// The I2S stream buffer, taken from the internal heap (so only games that
/// use sound pay for it, and the stack keeps its size).
pub fn stream_buffer() -> DmaTxStreamBuf {
    let words: &'static mut [u32] = Box::leak(vec![0u32; STREAM_BYTES / 4].into_boxed_slice());
    // SAFETY: the same memory viewed as bytes; u8 needs no more alignment than u32.
    let bytes =
        unsafe { core::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), STREAM_BYTES) };
    let descriptors = Box::leak(vec![DmaDescriptor::EMPTY; STREAM_DESCRIPTORS].into_boxed_slice());
    DmaTxStreamBuf::new(
        DmaAlignedMut::new(descriptors).expect("descriptors are word aligned"),
        DmaAlignedMut::new(bytes).expect("stream buffer is word aligned"),
    )
    .expect("stream buffer fits its descriptors")
}

fn fill_frames(bytes: &mut [u8], source: &mut Mixer) -> usize {
    let whole = bytes.len() / BYTES_PER_FRAME * BYTES_PER_FRAME;
    for frame in bytes[..whole].chunks_exact_mut(BYTES_PER_FRAME) {
        let [lo, hi] = source.next_sample().to_le_bytes();
        frame.copy_from_slice(&[lo, hi, lo, hi]);
    }
    whole
}

/// Non-blocking streaming output. `pump` copies as many frames as the DMA has
/// already consumed, so the queued audio (= latency) stays at about the
/// buffer length. If a frame stalls long enough to drain the buffer, the DMA
/// stops for good (esp-hal: "the transfer must then be resumed or
/// restarted"), so `pump` restarts it; a start the driver refuses is kept and
/// retried on the next pump rather than dropped.
pub struct AudioOut {
    stream: Option<Stream>,
    pub underruns: u32,
    pub failed_starts: u32,
}

enum Stream {
    Playing(I2sTxDmaTransfer<'static, Blocking, DmaTxStreamBuf>),
    Stopped(I2sTx<'static, Blocking>, DmaTxStreamBuf),
}

impl AudioOut {
    pub fn start(
        i2s_tx: I2sTx<'static, Blocking>,
        buffer: DmaTxStreamBuf,
        source: &mut Mixer,
    ) -> Self {
        let mut out = Self {
            stream: None,
            underruns: 0,
            failed_starts: 0,
        };
        out.launch(i2s_tx, buffer, source);
        out
    }

    /// Refills the buffer from the start and begins a fresh transfer.
    fn launch(
        &mut self,
        i2s_tx: I2sTx<'static, Blocking>,
        buffer: DmaTxStreamBuf,
        source: &mut Mixer,
    ) {
        let (descriptors, bytes) = buffer.split();
        let mut buffer =
            DmaTxStreamBuf::new(descriptors, bytes).expect("the stream buffer's own parts fit");
        buffer.push_with(|bytes| fill_frames(bytes, source));
        self.stream = Some(match i2s_tx.write(buffer) {
            Ok(transfer) => Stream::Playing(transfer),
            Err((err, i2s_tx, buffer)) => {
                self.failed_starts += 1;
                warn!("audio stream failed to start ({err:?}); retrying next frame");
                Stream::Stopped(i2s_tx, buffer)
            }
        });
    }

    pub fn pump(&mut self, source: &mut Mixer) {
        match self.stream.take() {
            Some(Stream::Playing(transfer)) if transfer.is_done() => {
                self.underruns += 1;
                let (i2s_tx, buffer) = transfer.stop();
                self.launch(i2s_tx, buffer, source);
            }
            Some(Stream::Playing(mut transfer)) => {
                while transfer.push_with(|bytes| fill_frames(bytes, source)) > 0 {}
                self.stream = Some(Stream::Playing(transfer));
            }
            Some(Stream::Stopped(i2s_tx, buffer)) => self.launch(i2s_tx, buffer, source),
            None => {}
        }
    }
}
