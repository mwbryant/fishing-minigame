//! Board support for the Waveshare ESP32-S3-Touch-AMOLED-1.75 and the frame
//! loop every game binary shares: bring up the display, touch, IMU, buttons
//! and flash, build the game's `no_std` Bevy app, then each frame feed the
//! inputs, update, persist saves and present the frame over QSPI.

#![no_std]
#![feature(asm_experimental_arch)]

extern crate alloc;

mod axp2101;
mod co5300;
mod idle;
mod power_key;
mod qmi8658;
mod speaker;

use alloc::{boxed::Box, format, string::String, vec};
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
    gpio::{Event, Input, InputConfig, Level, Output, OutputConfig, Pull, WakeupConfig},
    i2c::master::{Config as I2cConfig, I2c},
    i2s::master::{Channels, DataFormat, I2s, TdmConfig},
    rng::Rng as HardwareRng,
    rtc_cntl::{
        SocResetReason,
        sleep::{LowPower, RtcSleepConfig},
    },
    spi::{
        Mode,
        master::{Config as SpiConfig, Spi},
    },
    system::reset_reason,
    time::{Duration as HalDuration, Instant as HalInstant, Rate},
    timer::systimer::SystemTimer,
};
use esp_storage::FlashStorage;
use log::{info, warn};
use orbit_audio::{Mixer, SAMPLE_RATE};
use orbit_core::{
    Battery, Buttons, Frame, ImuFeed, ImuSample, Power, Rng, SaveBlob, SoundQueue, Touch,
};
use orbit_gfx::{Presenter, Rgb565, SCREEN_SIZE};
use orbit_save::SlotStore;

use crate::{
    axp2101::Axp2101,
    co5300::Co5300,
    idle::Idle,
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
/// While the game reports itself idle: 20 Hz and a dimmed panel.
const IDLE_FRAME_BUDGET: Duration = Duration::from_millis(50);
const FULL_BRIGHTNESS: u8 = 0xFF;
const IDLE_BRIGHTNESS: u8 = 0x40;
const BATTERY_POLL_MS: u64 = 5000;
/// A finger counts as lifted only after this many empty touch reads in a
/// row, so one dropped report can't fake a new tap.
const TOUCH_RELEASE_READS: u8 = 2;

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
    if reset_reason() == Some(SocResetReason::CoreDeepSleep) {
        info!("woke from sleep");
    }

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
    let mut pmic = Axp2101::new(RefCellDevice::new(bus))
        .inspect_err(|err| warn!("AXP2101 unavailable: {err:?}; no battery level"))
        .ok();
    let mut pwr = PwrKey::new(RefCellDevice::new(bus))
        .inspect_err(|err| warn!("PWR key unavailable: {err:?}"))
        .ok();
    let mut boot = Input::new(
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
        if let Err(err) = codec.init(SAMPLE_RATE, &mut amp) {
            warn!("ES8311 init failed: {err:?}");
        }
        (audio, mixer, amp)
    });

    let idle = Idle::new(SystemTimer::new(peripherals.SYSTIMER).alarm0);
    let lpwr = peripherals.LPWR;
    let mut presenter = Presenter::new(vec![0u8; BAND_BYTES], background);
    let mut stats = Stats::new();
    let mut last_imu_read = HalInstant::now();
    let mut finger = None;
    let mut empty_touch_reads = 0u8;
    let mut battery: Option<(Battery, u16)> = None;
    let mut battery_read: Option<HalInstant> = None;
    let mut dimmed = false;
    loop {
        let frame_start = HalInstant::now();
        let world = app.world_mut();

        // A failed read says nothing about the finger, so it changes nothing.
        if touch_ready && let Ok(points) = touch.touches() {
            match points[0] {
                Some(p) => {
                    finger = Some(screen_point(p.x, p.y));
                    empty_touch_reads = 0;
                }
                None => {
                    empty_touch_reads = empty_touch_reads.saturating_add(1);
                    if empty_touch_reads >= TOUCH_RELEASE_READS {
                        finger = None;
                    }
                }
            }
        }
        world.resource_mut::<Touch>().set(finger);

        if battery_read.is_none_or(|at| at.elapsed().as_millis() >= BATTERY_POLL_MS)
            && let Some(pmic) = pmic.as_mut()
        {
            battery_read = Some(HalInstant::now());
            match pmic.battery() {
                Ok(reading) => battery = Some(reading),
                Err(err) => warn!("battery read failed: {err:?}"),
            }
            if let (Some((reading, _)), Some(mut power)) =
                (battery, world.get_resource_mut::<Power>())
            {
                power.battery = Some(reading);
            }
        }

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

        if let Some((audio, mixer, _)) = sound.as_mut() {
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

        let power = app.world().get_resource::<Power>().copied();
        if power.is_some_and(|power| power.sleep_requested()) {
            // The game's goodbye frame is on the glass. Wait for BOOT to come
            // up first: it wakes the chip on a low level.
            info!("sleeping; press BOOT to wake");
            while boot.is_low() {
                delay.delay_millis(10);
            }
            delay.delay_millis(50);
            if let Some((_, _, amp)) = sound.as_mut() {
                amp.set_low();
            }
            if touch_ready && let Err(err) = touch.sleep() {
                warn!("touch sleep failed: {err:?}");
            }
            if imu_ready && let Err(err) = imu.power_down() {
                warn!("IMU power-down failed: {err:?}");
            }
            if let Err(err) = display.sleep(&mut delay) {
                warn!("display sleep failed: {err:?}");
            }
            boot.listen(Event::LowLevel);
            boot.apply_wakeup_config(&WakeupConfig::default().with_low_power_path(true))
                .expect("GPIO0 is a low-power pad");
            LowPower::new(lpwr).sleep_deep(RtcSleepConfig::deep());
        }
        let idle_now = power.is_some_and(|power| power.idle);
        if idle_now != dimmed {
            dimmed = idle_now;
            let level = if dimmed {
                IDLE_BRIGHTNESS
            } else {
                FULL_BRIGHTNESS
            };
            if let Err(err) = display.set_brightness(level) {
                warn!("brightness change failed: {err:?}");
            }
        }

        let busy = frame_start.elapsed();
        if let Some(frames) = stats.record(busy, sent) {
            info!(
                "{frames}{}{} | heap {}",
                sound
                    .as_ref()
                    .map_or(String::new(), |(audio, ..)| audio_status(audio)),
                battery_status(battery),
                esp_alloc::HEAP.stats(),
            );
        }
        let budget = if dimmed {
            IDLE_FRAME_BUDGET
        } else {
            FRAME_BUDGET
        };
        let spare = budget.saturating_sub(Duration::from_micros(busy.as_micros()));
        idle.wait(HalDuration::from_micros(spare.as_micros() as u64));
    }
}

fn audio_status(audio: &AudioOut) -> String {
    match (audio.underruns, audio.failed_starts) {
        (0, 0) => String::new(),
        (underruns, 0) => format!(" | audio restarted {underruns}x"),
        (underruns, failed) => format!(" | audio restarted {underruns}x, {failed} failed starts"),
    }
}

fn battery_status(battery: Option<(Battery, u16)>) -> String {
    let Some((battery, millivolts)) = battery else {
        return String::new();
    };
    let level = match battery.percent {
        Some(percent) => format!(
            "{percent}% {}.{:02} V",
            millivolts / 1000,
            millivolts % 1000 / 10
        ),
        None => String::from("no cell"),
    };
    let supply = match (battery.external_power, battery.charging) {
        (true, true) => ", USB, charging",
        (true, false) => ", USB",
        (false, _) => "",
    };
    format!(" | battery {level}{supply}")
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

    /// Counts a frame; every 2 s returns the summary and starts over.
    fn record(&mut self, busy: HalDuration, sent: usize) -> Option<String> {
        self.frames += 1;
        self.busy_us += busy.as_micros();
        self.worst_us = self.worst_us.max(busy.as_micros());
        self.bytes += sent;
        if self.since.elapsed().as_millis() < 2000 {
            return None;
        }
        let frames = u64::from(self.frames);
        let summary = format!(
            "{} frames | avg {} us, worst {} us | {} B/frame to panel",
            self.frames,
            self.busy_us / frames,
            self.worst_us,
            self.bytes as u64 / frames,
        );
        *self = Self::new();
        Some(summary)
    }
}
