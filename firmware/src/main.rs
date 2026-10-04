//! Orbit on the Waveshare ESP32-S3-Touch-AMOLED-1.75: the same no_std Bevy
//! app as the desktop simulator, fed by the CST9217 touch controller and
//! presented to the CO5300 AMOLED over QSPI.

#![no_std]
#![no_main]

extern crate alloc;

mod co5300;

use alloc::vec;
use core::time::Duration;

use bevy::{platform::time::Instant, prelude::*};
use cst92xx::CST92xx;
use esp_alloc::MemoryCapability;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    dma_tx_buffer,
    gpio::{Level, Output, OutputConfig},
    i2c::master::{Config as I2cConfig, I2c},
    main,
    rng::Rng as HardwareRng,
    spi::{
        Mode,
        master::{Config as SpiConfig, Spi},
    },
    time::{Instant as HalInstant, Rate},
};
use log::{info, warn};
use orbit_game::{BACKGROUND, Frame, OrbitPlugin, Rng, Touch};
use orbit_gfx::{Presenter, SCREEN_SIZE};

use crate::co5300::Co5300;

esp_bootloader_esp_idf::esp_app_desc!();

/// 40 MHz is what every Waveshare demo uses; the CO5300 write clock tops out
/// at 50 MHz.
const DISPLAY_SPI_MHZ: u32 = 40;
/// Rows rendered per band. Small bands leave internal SRAM to Bevy; each
/// band (466 * 8 * 2 = 7,456 bytes) goes out in a single DMA transfer.
const BAND_ROWS: usize = 8;
const BAND_BYTES: usize = SCREEN_SIZE as usize * 2 * BAND_ROWS;
const DMA_BYTES: usize = 7680;
/// Second internal heap region. Bevy needs ~216 KB steady (~270 KB peak while
/// schedules build), measured on a 32-bit target; this plus the 72 KB
/// reclaimed region holds even the peak and leaves ~76 KB of stack.
const INTERNAL_HEAP_BYTES: usize = 212 * 1024;
/// The touch panel reports coordinates rotated 180 degrees from the display.
const TOUCH_MIRROR_X: bool = true;
const TOUCH_MIRROR_Y: bool = true;
const FRAME_BUDGET: Duration = Duration::from_micros(16_667);

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    // Internal SRAM first: on the S3, atomic read-modify-write instructions
    // don't work on PSRAM, and Bevy keeps Arc counts and spinlocks on the
    // heap. PSRAM is registered last as a safety net; in emulation the whole
    // app fits internally and nothing lands there.
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);
    esp_alloc::heap_allocator!(size: INTERNAL_HEAP_BYTES);
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);

    // SAFETY: called once, before anything reads Bevy's clock.
    unsafe { Instant::set_elapsed(since_boot) };

    let mut delay = Delay::new();

    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(DISPLAY_SPI_MHZ))
            .with_mode(Mode::_0),
    )
    .expect("display SPI config")
    .with_sio0(peripherals.GPIO4)
    .with_sio1(peripherals.GPIO5)
    .with_sio2(peripherals.GPIO6)
    .with_sio3(peripherals.GPIO7)
    .with_sck(peripherals.GPIO38)
    .with_dma(peripherals.DMA_CH0);
    let mut display = Co5300::new(
        spi,
        Output::new(peripherals.GPIO12, Level::High, OutputConfig::default()),
        Output::new(peripherals.GPIO39, Level::High, OutputConfig::default()),
        [
            dma_tx_buffer!(DMA_BYTES).expect("DMA buffer A"),
            dma_tx_buffer!(DMA_BYTES).expect("DMA buffer B"),
        ],
    );
    display.init(&mut delay).expect("CO5300 init");
    info!("display ready");

    let i2c = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(400)),
    )
    .expect("I2C config")
    .with_sda(peripherals.GPIO15)
    .with_scl(peripherals.GPIO14);
    let touch_reset = Output::new(peripherals.GPIO40, Level::High, OutputConfig::default());
    let mut touch = CST92xx::new(i2c, delay).with_reset(touch_reset);
    let touch_ready = touch.init().is_ok();
    if !touch_ready {
        warn!("CST9217 touch init failed; running without input");
    }

    let mut app = App::new();
    app.add_plugins(OrbitPlugin)
        .insert_resource(Rng::new(HardwareRng::new().random()));
    app.finish();
    app.cleanup();
    app.update();
    match psram_used() {
        0 => info!("bevy app built entirely in internal SRAM"),
        spilled => warn!(
            "bevy spilled {spilled} B into PSRAM; atomics there are unreliable, \
             grow INTERNAL_HEAP_BYTES if the game hangs"
        ),
    }

    let mut presenter = Presenter::new(vec![0u8; BAND_BYTES], BACKGROUND);
    let mut stats = Stats::new();
    loop {
        let frame_start = HalInstant::now();

        let finger = if touch_ready {
            touch
                .touches()
                .ok()
                .and_then(|points| points[0])
                .map(|p| screen_point(p.x, p.y))
        } else {
            None
        };
        app.world_mut().resource_mut::<Touch>().set(finger);
        app.update();
        let sent = presenter.present(&app.world().resource::<Frame>().shapes, &mut display);

        let busy = frame_start.elapsed();
        stats.record(busy, sent);
        let spare = FRAME_BUDGET.saturating_sub(Duration::from_micros(busy.as_micros()));
        delay.delay_micros(spare.as_micros() as u32);
    }
}

fn psram_used() -> usize {
    esp_alloc::HEAP
        .stats()
        .region_stats
        .iter()
        .flatten()
        .filter(|region| region.capabilities.contains(MemoryCapability::External))
        .map(|region| region.used)
        .sum()
}

fn since_boot() -> Duration {
    Duration::from_micros(HalInstant::now().duration_since_epoch().as_micros())
}

fn screen_point(x: u16, y: u16) -> Vec2 {
    let max = SCREEN_SIZE as f32 - 1.0;
    let (x, y) = (f32::from(x).min(max), f32::from(y).min(max));
    Vec2::new(
        if TOUCH_MIRROR_X { max - x } else { x },
        if TOUCH_MIRROR_Y { max - y } else { y },
    )
}

/// Logs frame time and panel traffic every couple of seconds over USB serial.
struct Stats {
    since: HalInstant,
    frames: u32,
    busy_us: u64,
    worst_us: u64,
    bytes: usize,
}

impl Stats {
    fn new() -> Self {
        Self {
            since: HalInstant::now(),
            frames: 0,
            busy_us: 0,
            worst_us: 0,
            bytes: 0,
        }
    }

    fn record(&mut self, busy: esp_hal::time::Duration, sent: usize) {
        self.frames += 1;
        self.busy_us += busy.as_micros();
        self.worst_us = self.worst_us.max(busy.as_micros());
        self.bytes += sent;
        if self.since.elapsed().as_millis() < 2000 {
            return;
        }
        let frames = u64::from(self.frames);
        info!(
            "{} frames | avg {} us, worst {} us | {} B/frame to panel | heap {}",
            self.frames,
            self.busy_us / frames,
            self.worst_us,
            self.bytes as u64 / frames,
            esp_alloc::HEAP.stats(),
        );
        *self = Self::new();
    }
}
