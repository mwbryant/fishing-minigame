//! CO5300 AMOLED controller over QSPI (Waveshare ESP32-S3-Touch-AMOLED-1.75).
//!
//! Register writes are opcode `0x02` + `0x00RR00` with parameters on one line;
//! pixels are opcode `0x32` + `0x003C00` (memory-write-continue) with data on
//! all four lines, after a RAMWR register write opens the window. Two DMA
//! buffers ping-pong so the next chunk is copied while the last one transmits,
//! and a write returns while its final chunk is still on the wire, letting the
//! caller render the next band in parallel.

use embedded_hal::delay::DelayNs;
use esp_hal::{
    Blocking,
    dma::DmaTxBuf,
    gpio::Output,
    spi::{
        Error,
        master::{Address, Command, DataMode, SpiDma, SpiDmaTransfer},
    },
};
use log::error;
use orbit_gfx::{Panel, Rectangle};

/// The 466-column image sits at columns 6..=471 of the controller's RAM.
const COLUMN_OFFSET: u16 = 6;

const OPCODE_WRITE_REGISTER: u16 = 0x02;
const OPCODE_WRITE_PIXELS: u16 = 0x32;
const MEMORY_WRITE_CONTINUE: u32 = 0x00_3c_00;

const SLEEP_IN: u8 = 0x10;
const SLEEP_OUT: u8 = 0x11;
const DISPLAY_OFF: u8 = 0x28;
const DISPLAY_ON: u8 = 0x29;
const COLUMN_ADDRESS: u8 = 0x2a;
const ROW_ADDRESS: u8 = 0x2b;
const MEMORY_WRITE: u8 = 0x2c;
const TEARING_EFFECT_ON: u8 = 0x35;
const MEMORY_ACCESS_CONTROL: u8 = 0x36;
const PIXEL_FORMAT: u8 = 0x3a;
const BRIGHTNESS: u8 = 0x51;
const CONTROL_DISPLAY: u8 = 0x53;
const HIGH_BRIGHTNESS_LIMIT: u8 = 0x63;
const SPI_MODE_CONTROL: u8 = 0xc4;
const COMMAND_PAGE: u8 = 0xfe;

pub struct Co5300<'d> {
    spi: Option<SpiDma<'d, Blocking>>,
    pending: Option<SpiDmaTransfer<'d, Blocking, DmaTxBuf>>,
    idle: [Option<DmaTxBuf>; 2],
    chunk_len: usize,
    cs: Output<'d>,
    reset: Output<'d>,
}

impl<'d> Co5300<'d> {
    /// Each pixel write is split into chunks no larger than the smaller buffer.
    pub fn new(
        spi: SpiDma<'d, Blocking>,
        cs: Output<'d>,
        reset: Output<'d>,
        buffers: [DmaTxBuf; 2],
    ) -> Self {
        let [a, b] = buffers;
        Self {
            spi: Some(spi),
            pending: None,
            chunk_len: a.capacity().min(b.capacity()),
            idle: [Some(a), Some(b)],
            cs,
            reset,
        }
    }

    /// Hardware reset plus Waveshare's ESP-IDF BSP init list for this board.
    pub fn init(&mut self, delay: &mut impl DelayNs) -> Result<(), Error> {
        self.cs.set_high();
        self.reset.set_low();
        delay.delay_ms(10);
        self.reset.set_high();
        delay.delay_ms(150);

        self.register(MEMORY_ACCESS_CONTROL, &[0x00])?;
        self.register(PIXEL_FORMAT, &[0x55])?;
        // Vendor page 0x20 tweaks from the BSP; their meaning is undocumented.
        self.register(COMMAND_PAGE, &[0x20])?;
        self.register(0x19, &[0x10])?;
        self.register(0x1c, &[0xa0])?;
        self.register(COMMAND_PAGE, &[0x00])?;
        self.register(SPI_MODE_CONTROL, &[0x80])?;
        self.register(PIXEL_FORMAT, &[0x55])?;
        self.register(TEARING_EFFECT_ON, &[0x00])?;
        self.register(CONTROL_DISPLAY, &[0x20])?;
        self.register(BRIGHTNESS, &[0xff])?;
        self.register(HIGH_BRIGHTNESS_LIMIT, &[0xff])?;
        self.register(COLUMN_ADDRESS, &[0x00, 0x06, 0x01, 0xd7])?;
        self.register(ROW_ADDRESS, &[0x00, 0x00, 0x01, 0xd1])?;
        self.register(SLEEP_OUT, &[])?;
        self.wait();
        delay.delay_ms(120);
        self.register(DISPLAY_ON, &[])?;
        self.wait();
        Ok(())
    }

    /// Panel brightness, 0 (dark) to 255. AMOLED draw scales with it.
    pub fn set_brightness(&mut self, level: u8) -> Result<(), Error> {
        self.register(BRIGHTNESS, &[level])?;
        self.wait();
        Ok(())
    }

    /// Darkens the panel and puts the controller to sleep, its lowest-power
    /// state short of cutting its supply. Only a reset (`init`) wakes it.
    pub fn sleep(&mut self, delay: &mut impl DelayNs) -> Result<(), Error> {
        self.set_brightness(0)?;
        self.register(DISPLAY_OFF, &[])?;
        self.register(SLEEP_IN, &[])?;
        self.wait();
        delay.delay_ms(120);
        Ok(())
    }

    /// Writes big-endian RGB565 rows into the window `area` (screen coords).
    pub fn write_pixels(&mut self, area: Rectangle, bytes: &[u8]) -> Result<(), Error> {
        let x0 = area.top_left.x as u16 + COLUMN_OFFSET;
        let y0 = area.top_left.y as u16;
        let x1 = x0 + area.size.width as u16 - 1;
        let y1 = y0 + area.size.height as u16 - 1;
        self.register(COLUMN_ADDRESS, &range_bytes(x0, x1))?;
        self.register(ROW_ADDRESS, &range_bytes(y0, y1))?;
        self.register(MEMORY_WRITE, &[])?;
        for chunk in bytes.chunks(self.chunk_len) {
            self.start(
                DataMode::Quad,
                OPCODE_WRITE_PIXELS,
                MEMORY_WRITE_CONTINUE,
                chunk,
            )?;
        }
        Ok(())
    }

    fn register(&mut self, register: u8, params: &[u8]) -> Result<(), Error> {
        self.start(
            DataMode::Single,
            OPCODE_WRITE_REGISTER,
            u32::from(register) << 8,
            params,
        )
    }

    /// Fills an idle buffer (overlapping any transfer in flight), waits for
    /// that transfer, then starts this one without waiting for it.
    fn start(
        &mut self,
        mode: DataMode,
        opcode: u16,
        address: u32,
        data: &[u8],
    ) -> Result<(), Error> {
        let mut buffer = self
            .idle
            .iter_mut()
            .find_map(Option::take)
            .expect("at most one DMA buffer is ever in flight");
        buffer.fill(data);
        self.wait();
        let spi = self.spi.take().expect("SPI is idle after wait");
        self.cs.set_low();
        match spi.half_duplex_write_buffer(
            mode,
            Command::_8Bit(opcode, DataMode::Single),
            Address::_24Bit(address, DataMode::Single),
            0,
            data.len(),
            buffer,
        ) {
            Ok(transfer) => {
                self.pending = Some(transfer);
                Ok(())
            }
            Err((err, spi, buffer)) => {
                self.cs.set_high();
                self.spi = Some(spi);
                self.park(buffer);
                Err(err)
            }
        }
    }

    fn wait(&mut self) {
        let Some(transfer) = self.pending.take() else {
            return;
        };
        let (spi, buffer) = transfer.wait();
        self.cs.set_high();
        self.spi = Some(spi);
        self.park(buffer);
    }

    fn park(&mut self, buffer: DmaTxBuf) {
        let slot = self
            .idle
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("a parked buffer always has a free slot");
        *slot = Some(buffer);
    }
}

impl Panel for Co5300<'_> {
    fn write(&mut self, area: Rectangle, bytes: &[u8]) {
        if let Err(err) = self.write_pixels(area, bytes) {
            error!("CO5300 write to {area:?} failed: {err:?}");
        }
    }
}

fn range_bytes(start: u16, end: u16) -> [u8; 4] {
    let [s1, s0] = start.to_be_bytes();
    let [e1, e0] = end.to_be_bytes();
    [s1, s0, e1, e0]
}
