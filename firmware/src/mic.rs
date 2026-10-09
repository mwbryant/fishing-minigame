//! The board's two microphones: the ES7210 ADC (I2C `0x40`) sends both on
//! I2S0's receive line (GPIO10). The receiver runs on the speaker's clocks
//! (`TdmConfig::with_signal_loopback`), so it hears only while the speaker
//! stream runs, which it does from the first app with sound. The register
//! sequence follows esp-adf's `es7210.c` in slave mode with MIC1 and MIC2;
//! see `docs/reference/gnss_and_mics.md`.

use alloc::{boxed::Box, vec};

use embedded_hal::i2c::I2c;
use esp_hal::{
    Blocking,
    dma::{DmaDescriptor, DmaRxStreamBuf, aligned::DmaAlignedMut},
    i2s::master::{I2sRx, I2sRxDmaTransfer},
};
use log::warn;

const ES7210_ADDR: u8 = 0x40;
/// Mic gain: 30 dB, the driver's default (codes are 3 dB steps from 0).
const GAIN_30_DB: u8 = 0x0A;
/// 4 KB holds about 60 ms of 16 kHz stereo; the frame loop drains it every
/// 17 ms. A stall longer than that (an app being built) stops the DMA, and
/// `pump` restarts it.
const STREAM_BYTES: usize = 4 * 1024;
const STREAM_DESCRIPTORS: usize = 8;
/// Left (MIC1) then right (MIC2), 16 bits each.
const BYTES_PER_FRAME: usize = 4;

pub struct Es7210<I2C> {
    i2c: I2C,
}

impl<I2C: I2c> Es7210<I2C> {
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    fn write(&mut self, reg: u8, value: u8) -> Result<(), I2C::Error> {
        self.i2c.write(ES7210_ADDR, &[reg, value])
    }

    fn read(&mut self, reg: u8) -> Result<u8, I2C::Error> {
        let mut value = [0];
        self.i2c.write_read(ES7210_ADDR, &[reg], &mut value)?;
        Ok(value[0])
    }

    /// Sets the bits in `mask` of `reg` to those of `value`.
    fn update(&mut self, reg: u8, mask: u8, value: u8) -> Result<(), I2C::Error> {
        let old = self.read(reg)?;
        self.write(reg, (old & !mask) | (value & mask))
    }

    /// MIC1 and MIC2 on, in plain two-channel I2S.
    fn select_mics(&mut self) -> Result<(), I2C::Error> {
        for reg in 0x43..=0x46 {
            self.update(reg, 0x10, 0x00)?;
        }
        self.write(0x4B, 0xFF)?;
        self.write(0x4C, 0xFF)?;
        self.update(0x01, 0x0B, 0x00)?;
        self.write(0x4B, 0x00)?;
        self.update(0x43, 0x10, 0x10)?;
        self.update(0x44, 0x10, 0x10)?;
        self.write(0x12, 0x00)
    }

    /// Brings the ADC up as an I2S slave on the speaker's MCLK (256 x fs)
    /// with both mics at 30 dB. I2S must already be running.
    pub fn init(&mut self) -> Result<(), I2C::Error> {
        for (reg, value) in [
            (0x00, 0xFF),
            (0x00, 0x41),
            (0x01, 0x3F),
            (0x09, 0x30),
            (0x0A, 0x30),
            (0x23, 0x2A),
            (0x22, 0x0A),
            (0x20, 0x0A),
            (0x21, 0x2A),
        ] {
            self.write(reg, value)?;
        }
        self.update(0x08, 0x01, 0x00)?;
        for (reg, value) in [
            (0x40, 0x43),
            (0x41, 0x70),
            (0x42, 0x70),
            (0x07, 0x20),
            (0x02, 0xC1),
        ] {
            self.write(reg, value)?;
        }
        self.select_mics()?;
        self.update(0x11, 0xE3, 0x60)?;
        for (reg, value) in [(0x01, 0x34), (0x06, 0x00), (0x40, 0x43)] {
            self.write(reg, value)?;
        }
        for reg in 0x47..=0x4A {
            self.write(reg, 0x08)?;
        }
        self.select_mics()?;
        for (reg, value) in [(0x40, 0x43), (0x00, 0x71), (0x00, 0x41)] {
            self.write(reg, value)?;
        }
        self.update(0x43, 0x0F, GAIN_30_DB)?;
        self.update(0x44, 0x0F, GAIN_30_DB)?;
        self.update(0x14, 0x03, 0x00)?;
        self.update(0x15, 0x03, 0x00)
    }
}

/// The receive stream buffer, from the internal heap the first time a game
/// listens.
pub fn stream_buffer() -> DmaRxStreamBuf {
    let words: &'static mut [u32] = Box::leak(vec![0u32; STREAM_BYTES / 4].into_boxed_slice());
    // SAFETY: the same memory viewed as bytes; u8 needs no more alignment than u32.
    let bytes =
        unsafe { core::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), STREAM_BYTES) };
    let descriptors = Box::leak(vec![DmaDescriptor::EMPTY; STREAM_DESCRIPTORS].into_boxed_slice());
    DmaRxStreamBuf::new(
        DmaAlignedMut::new(descriptors).expect("descriptors are word aligned"),
        DmaAlignedMut::new(bytes).expect("stream buffer is word aligned"),
    )
    .expect("stream buffer fits its descriptors")
}

enum Stream {
    Running(I2sRxDmaTransfer<'static, Blocking, DmaRxStreamBuf>),
    Stopped(I2sRx<'static, Blocking>, DmaRxStreamBuf),
}

/// The mics' samples as they arrive, both mics averaged into one.
pub struct MicIn {
    stream: Option<Stream>,
    pub restarts: u32,
}

impl MicIn {
    pub fn start(i2s_rx: I2sRx<'static, Blocking>, buffer: DmaRxStreamBuf) -> Self {
        let mut mic = Self {
            stream: None,
            restarts: 0,
        };
        mic.launch(i2s_rx, buffer);
        mic
    }

    fn launch(&mut self, i2s_rx: I2sRx<'static, Blocking>, buffer: DmaRxStreamBuf) {
        let (descriptors, bytes) = buffer.split();
        let buffer =
            DmaRxStreamBuf::new(descriptors, bytes).expect("the stream buffer's own parts fit");
        self.stream = Some(match i2s_rx.read(buffer) {
            Ok(transfer) => Stream::Running(transfer),
            Err((err, i2s_rx, buffer)) => {
                warn!("mic stream failed to start ({err:?}); retrying next frame");
                Stream::Stopped(i2s_rx, buffer)
            }
        });
    }

    /// Hands every sample heard since the last call to `hear`, a chunk at a
    /// time; restarts the stream if it filled up and stopped.
    pub fn pump(&mut self, mut hear: impl FnMut(&[i16])) {
        match self.stream.take() {
            Some(Stream::Running(transfer)) if transfer.is_done() => {
                self.restarts += 1;
                let (i2s_rx, buffer) = transfer.stop();
                self.launch(i2s_rx, buffer);
            }
            Some(Stream::Running(mut transfer)) => {
                let mut bytes = [0u8; 512];
                let mut samples = [0i16; 512 / BYTES_PER_FRAME];
                loop {
                    let read = transfer.pop(&mut bytes) / BYTES_PER_FRAME * BYTES_PER_FRAME;
                    if read == 0 {
                        break;
                    }
                    let frames = read / BYTES_PER_FRAME;
                    for (sample, frame) in samples.iter_mut().zip(bytes[..read].chunks_exact(4)) {
                        let left = i32::from(i16::from_le_bytes([frame[0], frame[1]]));
                        let right = i32::from(i16::from_le_bytes([frame[2], frame[3]]));
                        *sample = ((left + right) / 2) as i16;
                    }
                    hear(&samples[..frames]);
                }
                self.stream = Some(Stream::Running(transfer));
            }
            Some(Stream::Stopped(i2s_rx, buffer)) => self.launch(i2s_rx, buffer),
            None => {}
        }
    }
}
