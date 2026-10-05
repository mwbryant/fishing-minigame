//! Board support for the Waveshare ESP32-S3-Touch-AMOLED-1.75 and the frame
//! loop every game binary shares: bring up the display, touch, IMU, buttons
//! and flash, build the game's `no_std` Bevy app, then each frame feed the
//! inputs, update, persist saves and present the frame over QSPI.

#![no_std]

extern crate alloc;

mod co5300;
mod power_key;
mod qmi8658;
mod speaker;

use alloc::{boxed::Box, vec};
use core::{cell::RefCell, time::Duration};

use bevy::{platform::time::Instant, prelude::*};
use cst92xx::CST92xx;
use embedded_hal_bus::i2c::RefCellDevice;
use esp_alloc::MemoryCapability;
use esp_hal::{
    Blocking,
    clock::CpuClock,
    delay::Delay,
    dma_tx_buffer,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    i2c::master::{Config as I2cConfig, I2c},
    i2s::master::{Channels, DataFormat, I2s, TdmConfig},
    rng::Rng as HardwareRng,
    spi::{
        Mode,
        master::{Config as SpiConfig, Spi},
    },
    time::{Instant as HalInstant, Rate},
};
use esp_storage::FlashStorage;
use log::{info, warn};
use orbit_audio::{Mixer, SAMPLE_RATE};
use orbit_core::{Buttons, Frame, ImuFeed, ImuSample, Rng, SaveBlob, SoundQueue, Touch};
use orbit_gfx::{Presenter, Rgb565, SCREEN_SIZE};
use orbit_save::SlotStore;

use crate::{
    co5300::Co5300,
    power_key::PwrKey,
    qmi8658::Qmi8658,
    speaker::{AudioOut, Es8311},
};

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
/// With the UI upright both buttons sit on the right edge: PWR at about
/// 2 o'clock, BOOT at about 4 o'clock. Swap these to swap their roles.
const ACTION_BUTTON: Switch = Switch::Pwr;
const ZERO_BUTTON: Switch = Switch::Boot;
/// Two flash sectors in the default partition table's `nvs` region, which
/// `espflash flash` never rewrites, so saves survive reflashing.
const SAVE_FLASH_OFFSET: u32 = 0x9000;
const FRAME_BUDGET: Duration = Duration::from_micros(16_667);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Switch {
    Pwr,
    Boot,
}

type Bus = RefCell<I2c<'static, Blocking>>;

/// Brings up the board, builds the app with `add_game`, and runs it forever.
pub fn run(add_game: impl FnOnce(&mut App), background: Rgb565) -> ! {
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
    let bus: &'static Bus = Box::leak(Box::new(RefCell::new(i2c)));

    let touch_reset = Output::new(peripherals.GPIO40, Level::High, OutputConfig::default());
    let mut touch = CST92xx::new(RefCellDevice::new(bus), delay).with_reset(touch_reset);
    let touch_ready = touch.init().is_ok();
    if !touch_ready {
        warn!("CST9217 touch init failed; running without touch");
    }

    let mut imu = Qmi8658::new(RefCellDevice::new(bus));
    let imu_ready = match imu.init(&mut delay) {
        Ok(()) => true,
        Err(qmi8658::Error::WrongChip(id)) => {
            warn!("IMU answered WHO_AM_I {id:#04x}, expected 0x05; running without tilt");
            false
        }
        Err(err) => {
            warn!("QMI8658 init failed ({err:?}); running without tilt");
            false
        }
    };
    let mut pwr = PwrKey::new(RefCellDevice::new(bus))
        .inspect_err(|err| warn!("PWR key unavailable: {err:?}"))
        .ok();
    let boot = Input::new(
        peripherals.GPIO0,
        InputConfig::default().with_pull(Pull::Up),
    );

    let mut saves = SlotStore::new(FlashStorage::new(peripherals.FLASH), SAVE_FLASH_OFFSET);
    let saved = saves.load();
    info!(
        "save storage: {}",
        if saved.is_some() {
            "found a save"
        } else {
            "empty"
        }
    );

    let mut app = App::new();
    add_game(&mut app);
    app.insert_resource(Rng::new(HardwareRng::new().random()))
        .insert_resource(SaveBlob::from_storage(saved));
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

    // Started after the first update (Bevy's slow schedule build) so the
    // stream doesn't underrun at once; only for games that make sound.
    let mut sound = app.world().contains_resource::<SoundQueue>().then(|| {
        let i2s = I2s::new(
            peripherals.I2S0,
            peripherals.DMA_CH1,
            TdmConfig::new_tdm_philips()
                .with_sample_rate(Rate::from_hz(SAMPLE_RATE))
                .with_data_format(DataFormat::Data16Channel16)
                .with_channels(Channels::STEREO),
        )
        .expect("I2S config")
        .with_mclk(peripherals.GPIO42);
        let i2s_tx = i2s
            .i2s_tx
            .with_bclk(peripherals.GPIO9)
            .with_ws(peripherals.GPIO45)
            .with_dout(peripherals.GPIO8)
            .build();
        let mut mixer = Mixer::default();
        let audio = AudioOut::start(i2s_tx, speaker::stream_buffer(), &mut mixer);
        let mut amp = Output::new(peripherals.GPIO46, Level::Low, OutputConfig::default());
        let mut codec = Es8311::new(RefCellDevice::new(bus));
        match codec.chip_id() {
            Ok((id1, id2)) => info!("ES8311 id {id1:#04x} {id2:#04x} (expect 0x83 0x11)"),
            Err(err) => warn!("ES8311 not answering: {err:?}"),
        }
        // `Output` has no Drop, so the amplifier stays enabled after `amp`
        // goes out of scope.
        if let Err(err) = codec.init(SAMPLE_RATE, &mut amp) {
            warn!("ES8311 init failed: {err:?}");
        }
        (audio, mixer)
    });

    let mut presenter = Presenter::new(vec![0u8; BAND_BYTES], background);
    let mut stats = Stats::new();
    let mut last_imu_read = HalInstant::now();
    loop {
        let frame_start = HalInstant::now();
        let world = app.world_mut();

        let finger = if touch_ready {
            touch
                .touches()
                .ok()
                .and_then(|points| points[0])
                .map(|p| screen_point(p.x, p.y))
        } else {
            None
        };
        world.resource_mut::<Touch>().set(finger);

        if let Some(mut buttons) = world.get_resource_mut::<Buttons>() {
            let pwr_held = pwr
                .as_mut()
                .is_some_and(|key| key.is_pressed().unwrap_or(false));
            let held = |switch| match switch {
                Switch::Pwr => pwr_held,
                Switch::Boot => boot.is_low(),
            };
            buttons.action.set(held(ACTION_BUTTON));
            buttons.zero.set(held(ZERO_BUTTON));
        }

        if imu_ready
            && let Some(mut feed) = world.get_resource_mut::<ImuFeed>()
            && let Ok((accel, gyro)) = imu.read()
        {
            let now = HalInstant::now();
            let dt = (now - last_imu_read).as_micros() as f32 / 1e6;
            last_imu_read = now;
            feed.push(ImuSample {
                accel: imu_to_screen(accel),
                gyro: imu_to_screen(gyro) * (core::f32::consts::PI / 180.0),
                dt,
            });
        }

        app.update();

        if let Some((audio, mixer)) = sound.as_mut() {
            for command in app.world_mut().resource_mut::<SoundQueue>().drain() {
                mixer.apply(command);
            }
            audio.pump(mixer);
        }

        if let Some(bytes) = app.world_mut().resource_mut::<SaveBlob>().take_pending()
            && let Err(err) = saves.save(&bytes)
        {
            warn!("saving failed: {err:?}");
        }
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

/// QMI8658 axes to the screen frame (x right, y down, z into the screen):
/// per the PCB's axis marking and Waveshare's tilt demos, screen-right is
/// +Y, screen-down is -X, and +Z points out of the back.
fn imu_to_screen(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[1], -v[0], v[2])
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
