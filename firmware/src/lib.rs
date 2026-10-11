//! Board support for the Waveshare ESP32-S3-Touch-AMOLED-1.75 and the frame
//! loop every game binary shares. `Board::new` brings up the drivers once.
//! `Board::run` builds a game's `no_std` Bevy app and, each frame, feeds it
//! input, updates it, persists saves and presents over QSPI, until the app
//! exits or sleeps.

#![no_std]
#![feature(asm_experimental_arch)]

extern crate alloc;

mod axp2101;
mod co5300;
mod idle;
mod lc76g;
mod mic;
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
    gpio::{Event, Input, InputConfig, Level, NoPin, Output, OutputConfig, Pull, WakeupConfig},
    i2c::master::{Config as I2cConfig, I2c},
    i2s::master::{Channels, DataFormat, I2s, I2sRx, TdmConfig},
    peripherals::{DMA_CH1, GPIO8, GPIO9, GPIO10, GPIO42, GPIO45, GPIO46, I2S0},
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
    Battery, Buttons, Frame, Gps, ImuFeed, ImuSample, Launcher, Mic, Power, Rng, SaveBlob,
    SoundQueue, Steps, Touch, preferences,
};
use orbit_gfx::{Presenter, Rgb565, SCREEN_SIZE};
use orbit_motion::StepCounter;
use orbit_save::SlotStore;

use crate::{
    axp2101::Axp2101,
    co5300::Co5300,
    idle::Idle,
    lc76g::Lc76g,
    mic::{Es7210, MicIn},
    power_key::PwrKey,
    qmi8658::Qmi8658,
    speaker::{AudioOut, Es8311},
};

/// 40 MHz is what every Waveshare demo uses; the CO5300 write clock tops out
/// at 50 MHz.
const DISPLAY_SPI_MHZ: u32 = 40;
/// Rows per render band. Small bands leave internal SRAM to Bevy, and a band
/// (466 * 8 * 2 = 7,456 bytes) goes out in one DMA transfer.
const BAND_ROWS: usize = 8;
const BAND_BYTES: usize = SCREEN_SIZE as usize * 2 * BAND_ROWS;
/// Each of the display's two ping-pong DMA buffers holds a whole band.
const DMA_BYTES: usize = 7680;
/// Second internal heap region. Bevy needs ~216 KB steady and ~270 KB peak
/// while schedules build (measured on a 32-bit target). This plus the 72 KB
/// reclaimed region holds the peak, and the DRAM left over, about 67-70 KB, is
/// the stack. `tools/heap32/run.mjs` parses this line, the reclaimed size and
/// `BAND_ROWS`.
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
/// Idle dims the panel to this fraction of the chosen brightness.
const IDLE_DIVISOR: u16 = 4;
const BATTERY_POLL_MS: u64 = 5000;
/// While a game dozes, the board wakes this often to count steps and read
/// the buttons (PWR has no wake line, so it's read on each wake).
const DOZE_PERIOD_MS: u64 = 100;
/// The LC76G reports every second; this long without a byte while an app
/// listens means it has stalled, and it's restarted.
const GPS_SILENT_SECS: u64 = 5;
/// A finger counts as lifted only after this many empty touch reads in a
/// row, so one dropped report can't fake a new tap.
const TOUCH_RELEASE_READS: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Switch {
    Pwr,
    Boot,
}

type Bus = RefCell<I2c<'static, Blocking>>;
type Device = RefCellDevice<'static, I2c<'static, Blocking>>;

/// The speaker's peripherals, kept until a game with sound needs them, so
/// games without sound (Orbit) never pay for the stream buffer.
struct SpeakerPins {
    i2s: I2S0<'static>,
    dma: DMA_CH1<'static>,
    mclk: GPIO42<'static>,
    bclk: GPIO9<'static>,
    ws: GPIO45<'static>,
    dout: GPIO8<'static>,
    /// The ES7210 mics' data, received on the speaker's clocks.
    din: GPIO10<'static>,
    amp: GPIO46<'static>,
}

struct Speaker {
    audio: AudioOut,
    mixer: Mixer,
    amp: Output<'static>,
    mic: MicPort,
}

/// The mics' receiver: built with the speaker, started (ADC and stream)
/// the first time an app listens.
enum MicPort {
    Idle(I2sRx<'static, Blocking>),
    Listening(MicIn),
    Failed,
}

/// The board, brought up once. Apps run on it one after another: Fishing Minigame
/// runs the town, then the place its door led to, then the town again.
pub struct Board {
    delay: Delay,
    display: Co5300<'static>,
    bus: &'static Bus,
    touch: CST92xx<Device, Delay, Output<'static>>,
    touch_ready: bool,
    imu: Qmi8658<Device>,
    imu_ready: bool,
    pmic: Option<Axp2101<Device>>,
    pwr: Option<PwrKey<Device>>,
    boot: Input<'static>,
    saves: SlotStore<FlashStorage<'static>>,
    speaker_pins: Option<SpeakerPins>,
    speaker: Option<Speaker>,
    idle: Idle,
    low_power: LowPower<'static>,
    finger: Option<Vec2>,
    empty_touch_reads: u8,
    battery: Option<(Battery, u16)>,
    battery_read: Option<HalInstant>,
    last_imu_read: HalInstant,
    gps: Option<Lc76g<Device>>,
    gps_errors: u32,
    /// When the receiver was sent to backup mode, until the check that it
    /// went quiet.
    gps_sleep_sent: Option<HalInstant>,
    /// Counts steps from the accelerometer while an app has `Steps`.
    steps: StepCounter,
    /// Steps counted since the app started, for the log.
    steps_counted: u32,
    /// A game asked to doze (`Power::dozing`): screen and touch off.
    dozing: bool,
    /// While an app listens to the receiver: when it last sent anything.
    gps_heard: Option<HalInstant>,
    /// NMEA bytes since the last stats line.
    gps_bytes: usize,
    gps_restarts: u32,
}

impl Board {
    /// Brings up the heap, the clock, and every driver.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
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
        let pmic = Axp2101::new(RefCellDevice::new(bus))
            .inspect_err(|err| warn!("AXP2101 unavailable: {err:?}; no battery level"))
            .ok();
        let pwr = PwrKey::new(RefCellDevice::new(bus))
            .inspect_err(|err| warn!("PWR key unavailable: {err:?}"))
            .ok();
        let boot = Input::new(
            peripherals.GPIO0,
            InputConfig::default().with_pull(Pull::Up),
        );
        // Nothing uses the satellite receiver, and it draws about 36 mA with no
        // power switch, so it goes to backup mode (13 uA) until power is lost.
        let mut gps = Lc76g::new(RefCellDevice::new(bus));
        let gps_sleep_sent = match gps.as_mut().map(|gps| gps.sleep(&mut delay)) {
            Some(Ok(())) => {
                info!("LC76G satellite receiver found; sent to backup mode");
                Some(HalInstant::now())
            }
            Some(Err(err)) => {
                warn!("LC76G backup command failed: {err:?}; it keeps drawing ~36 mA");
                None
            }
            None => {
                info!("no satellite receiver (not a -G board)");
                None
            }
        };

        let mut saves = SlotStore::new(FlashStorage::new(peripherals.FLASH), SAVE_FLASH_OFFSET);
        info!(
            "save storage: {}",
            if saves.load().is_some() {
                "found a save"
            } else {
                "empty"
            }
        );

        Self {
            delay,
            display,
            bus,
            touch,
            touch_ready,
            imu,
            imu_ready,
            pmic,
            pwr,
            boot,
            saves,
            speaker_pins: Some(SpeakerPins {
                i2s: peripherals.I2S0,
                dma: peripherals.DMA_CH1,
                mclk: peripherals.GPIO42,
                bclk: peripherals.GPIO9,
                ws: peripherals.GPIO45,
                dout: peripherals.GPIO8,
                din: peripherals.GPIO10,
                amp: peripherals.GPIO46,
            }),
            speaker: None,
            idle: Idle::new(SystemTimer::new(peripherals.SYSTIMER).alarm0),
            low_power: LowPower::new(peripherals.LPWR),
            finger: None,
            empty_touch_reads: 0,
            battery: None,
            battery_read: None,
            last_imu_read: HalInstant::now(),
            gps,
            gps_errors: 0,
            gps_sleep_sent,
            steps: StepCounter::default(),
            steps_counted: 0,
            dozing: false,
            gps_heard: None,
            gps_bytes: 0,
            gps_restarts: 0,
        }
    }

    /// Builds an app with `add_game` and runs it until it sends `AppExit`,
    /// then hands it back so the caller can see where the town's door led. A
    /// game `launched` from the town offers OUT. When a game asks to sleep
    /// this never returns: the board switches off, and PWR turns it back on as
    /// a fresh boot.
    pub fn run(
        &mut self,
        add_game: impl FnOnce(&mut App),
        background: Rgb565,
        launched: bool,
    ) -> App {
        let mut app = App::new();
        add_game(&mut app);
        if launched {
            app.insert_resource(Launcher);
        }
        app.insert_resource(Rng::new(HardwareRng::new().random()))
            .insert_resource(SaveBlob::from_storage(self.saves.load()));
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
        // stream doesn't underrun at once; only once a game makes sound.
        if self.speaker.is_none()
            && app.world().contains_resource::<SoundQueue>()
            && let Some(pins) = self.speaker_pins.take()
        {
            self.speaker = Some(start_speaker(pins, self.bus));
        }
        if app.world().contains_resource::<Mic>()
            && let Some(speaker) = self.speaker.as_mut()
        {
            start_mic(&mut speaker.mic, self.bus);
        }
        self.steps = StepCounter::default();
        self.steps_counted = 0;
        let mut presenter = Presenter::new(vec![0u8; BAND_BYTES], background);
        let mut stats = Stats::new();
        let mut brightness = None;
        loop {
            let frame_start = HalInstant::now();
            self.feed_inputs(app.world_mut());
            app.update();

            if let Some(speaker) = self.speaker.as_mut() {
                for command in app.world_mut().resource_mut::<SoundQueue>().drain() {
                    speaker.mixer.apply(command);
                }
                speaker.audio.pump(&mut speaker.mixer);
            }
            if let Some(bytes) = app.world_mut().resource_mut::<SaveBlob>().take_pending()
                && let Err(err) = self.saves.save(&bytes)
            {
                warn!("saving failed: {err:?}");
            }
            let power = app.world().get_resource::<Power>().copied();
            let doze = power.is_some_and(|power| power.dozing()) && app.should_exit().is_none();
            if doze != self.dozing {
                if doze {
                    self.start_doze();
                } else {
                    self.end_doze();
                    presenter.invalidate();
                    brightness = None;
                }
            }
            let sent = if self.dozing {
                0
            } else {
                presenter.present(&app.world().resource::<Frame>().shapes, &mut self.display)
            };

            if power.is_some_and(|power| power.sleep_requested()) {
                self.sleep();
            }
            if app.should_exit().is_some() {
                return app;
            }
            let dimmed = power.is_some_and(|power| power.idle);
            let level = panel_brightness(
                preferences::brightness(app.world().resource::<SaveBlob>()),
                dimmed,
            );
            if !self.dozing && brightness != Some(level) {
                brightness = Some(level);
                if let Err(err) = self.display.set_brightness(level) {
                    warn!("brightness change failed: {err:?}");
                }
            }

            let busy = frame_start.elapsed();
            if let Some(frames) = stats.record(busy, sent) {
                let gps = match (self.gps_heard, self.gps_restarts) {
                    (None, _) => String::new(),
                    (Some(_), 0) => format!(" | gps {} B", self.gps_bytes),
                    (Some(_), restarts) => {
                        format!(" | gps {} B, restarted {restarts}x", self.gps_bytes)
                    }
                };
                self.gps_bytes = 0;
                let steps = if app.world().contains_resource::<Steps>() {
                    format!(" | steps {}", self.steps_counted)
                } else {
                    String::new()
                };
                info!(
                    "{frames}{}{}{gps}{steps} | heap {}",
                    self.speaker
                        .as_ref()
                        .map_or(String::new(), |speaker| audio_status(&speaker.audio)),
                    battery_status(self.battery),
                    esp_alloc::HEAP.stats(),
                );
            }
            if self.dozing {
                self.doze_wait();
                continue;
            }
            let budget = if dimmed {
                IDLE_FRAME_BUDGET
            } else {
                FRAME_BUDGET
            };
            let spare = budget.saturating_sub(Duration::from_micros(busy.as_micros()));
            self.idle
                .wait(HalDuration::from_micros(spare.as_micros() as u64));
        }
    }

    /// The game asked to doze: the panel and touch to sleep, the amp and gyro
    /// off, leaving the accelerometer on for steps.
    fn start_doze(&mut self) {
        info!("dozing: screen off until a button");
        self.dozing = true;
        self.finger = None;
        if let Err(err) = self.display.sleep(&mut self.delay) {
            warn!("display sleep failed: {err:?}");
        }
        if self.touch_ready
            && let Err(err) = self.touch.sleep()
        {
            warn!("touch sleep failed: {err:?}");
        }
        if let Some(speaker) = self.speaker.as_mut() {
            speaker.amp.set_low();
        }
        if self.imu_ready
            && let Err(err) = self.imu.gyro(false)
        {
            warn!("gyro off failed: {err:?}");
        }
    }

    /// Back from a doze: the panel and touch start again (only a reset wakes
    /// them) and the amp comes back on.
    fn end_doze(&mut self) {
        info!("awake");
        self.dozing = false;
        if let Err(err) = self.display.init(&mut self.delay) {
            warn!("display wake failed: {err:?}");
        }
        self.touch_ready = self.touch.init().is_ok();
        if !self.touch_ready {
            warn!("CST9217 touch wake failed; running without touch");
        }
        if let Some(speaker) = self.speaker.as_mut() {
            speaker.amp.set_high();
        }
        if self.imu_ready
            && let Err(err) = self.imu.gyro(true)
        {
            warn!("gyro on failed: {err:?}");
        }
    }

    /// Waits out a doze period. On battery the chip light-sleeps (BOOT or the
    /// period's end wakes it); on USB, where power doesn't matter, it only
    /// halts, so the USB serial log keeps working.
    fn doze_wait(&mut self) {
        let period = HalDuration::from_millis(DOZE_PERIOD_MS);
        let on_battery = self
            .battery
            .is_some_and(|(battery, _)| !battery.external_power);
        if !on_battery {
            self.idle.wait(period);
            return;
        }
        self.boot.listen(Event::LowLevel);
        self.low_power
            .set_wakeup_deadline(HalInstant::now() + period);
        self.low_power.sleep_light(RtcSleepConfig::default());
        self.low_power.clear_wakeup_deadline();
        self.boot.unlisten();
    }

    /// Feeds touch, battery, buttons, mics, GPS and IMU into the app's input
    /// resources.
    fn feed_inputs(&mut self, world: &mut World) {
        // A failed read says nothing about the finger, so it changes nothing.
        if self.touch_ready
            && !self.dozing
            && let Ok(points) = self.touch.touches()
        {
            match points[0] {
                Some(p) => {
                    self.finger = Some(screen_point(p.x, p.y));
                    self.empty_touch_reads = 0;
                }
                None => {
                    self.empty_touch_reads = self.empty_touch_reads.saturating_add(1);
                    if self.empty_touch_reads >= TOUCH_RELEASE_READS {
                        self.finger = None;
                    }
                }
            }
        }
        world.resource_mut::<Touch>().set(self.finger);

        if self
            .battery_read
            .is_none_or(|at| at.elapsed().as_millis() >= BATTERY_POLL_MS)
            && let Some(pmic) = self.pmic.as_mut()
        {
            self.battery_read = Some(HalInstant::now());
            match pmic.battery() {
                Ok(reading) => self.battery = Some(reading),
                Err(err) => warn!("battery read failed: {err:?}"),
            }
        }
        if let (Some((reading, _)), Some(mut power)) =
            (self.battery, world.get_resource_mut::<Power>())
            && power.battery != Some(reading)
        {
            power.battery = Some(reading);
        }

        if let Some(mut buttons) = world.get_resource_mut::<Buttons>() {
            let pwr_held = self
                .pwr
                .as_mut()
                .is_some_and(|key| key.is_pressed().unwrap_or(false));
            let boot_held = self.boot.is_low();
            let held = |switch| match switch {
                Switch::Pwr => pwr_held,
                Switch::Boot => boot_held,
            };
            buttons.action.set(held(ACTION_BUTTON));
            buttons.zero.set(held(ZERO_BUTTON));
        }

        if let Some(speaker) = self.speaker.as_mut()
            && let MicPort::Listening(mic_in) = &mut speaker.mic
        {
            match world.get_resource_mut::<Mic>() {
                Some(mut mic) => mic_in.pump(|samples| mic.push(samples)),
                None => mic_in.pump(|_| {}),
            }
        }

        if self
            .gps_sleep_sent
            .is_some_and(|at| at.elapsed().as_millis() >= 1500)
            && let Some(gps) = self.gps.as_mut()
        {
            self.gps_sleep_sent = None;
            match gps.waiting(&mut self.delay) {
                Ok(0) | Err(_) => {
                    info!("LC76G asleep (backup mode)");
                    self.gps = None;
                }
                Ok(bytes) => warn!("LC76G still running after the backup command ({bytes} B)"),
            }
        }

        if let (Some(gps), Some(mut feed)) = (self.gps.as_mut(), world.get_resource_mut::<Gps>()) {
            let now = HalInstant::now();
            let heard = *self.gps_heard.get_or_insert(now);
            let mut text = [0u8; 1024];
            match gps.poll(&mut text) {
                Ok(read) => {
                    if read > 0 {
                        self.gps_heard = Some(now);
                        self.gps_bytes += read;
                    }
                    feed.push(&text[..read]);
                }
                Err(err) => {
                    self.gps_errors += 1;
                    if self.gps_errors.is_power_of_two() {
                        warn!("LC76G read failed ({err:?}), {} so far", self.gps_errors);
                    }
                    gps.reset();
                }
            }
            if (now - heard).as_secs() >= GPS_SILENT_SECS {
                self.gps_restarts += 1;
                warn!(
                    "LC76G silent for {GPS_SILENT_SECS} s; restarting it ({} so far)",
                    self.gps_restarts
                );
                if let Err(err) = gps.restart(&mut self.delay) {
                    warn!("LC76G restart failed: {err:?}");
                }
                self.gps_heard = Some(now);
            }
        } else {
            self.gps_heard = None;
        }

        // Tilt while awake; steps whenever an app counts them, dozing included
        // (one reading a doze period).
        let tilts = !self.dozing && world.contains_resource::<ImuFeed>();
        let counts_steps = world.contains_resource::<Steps>();
        if self.imu_ready
            && (tilts || counts_steps)
            && let Ok((accel, gyro)) = self.imu.read()
        {
            let now = HalInstant::now();
            let dt = (now - self.last_imu_read).as_micros() as f32 / 1e6;
            self.last_imu_read = now;
            let accel = imu_to_screen(accel);
            if let Some(mut steps) = world.get_resource_mut::<Steps>() {
                let new = self.steps.push(accel, dt);
                self.steps_counted += new;
                steps.add(new);
            }
            if tilts && let Some(mut feed) = world.get_resource_mut::<ImuFeed>() {
                feed.push(ImuSample {
                    accel,
                    gyro: imu_to_screen(gyro) * (core::f32::consts::PI / 180.0),
                    dt,
                });
            }
        }
    }

    /// The game's goodbye frame is on the glass: wait for BOOT to come up,
    /// quiet the peripherals, then switch every rail off through the AXP2101,
    /// so nothing draws until PWR turns the board back on. ESP32 deep sleep
    /// alone leaves the codec, mics, display and GPS powered from the same
    /// 3.3 V rail; it is only the fallback here, woken by BOOT.
    fn sleep(&mut self) -> ! {
        info!("switching off; press PWR to turn on");
        // A BOOT still held at the next power-on would start the ROM's
        // download mode, and deep sleep wakes on its low level.
        while self.boot.is_low() {
            self.delay.delay_millis(10);
        }
        self.delay.delay_millis(50);
        if let Some(speaker) = self.speaker.as_mut() {
            speaker.amp.set_low();
        }
        if self.touch_ready
            && let Err(err) = self.touch.sleep()
        {
            warn!("touch sleep failed: {err:?}");
        }
        if self.imu_ready
            && let Err(err) = self.imu.power_down()
        {
            warn!("IMU power-down failed: {err:?}");
        }
        if let Err(err) = self.display.sleep(&mut self.delay) {
            warn!("display sleep failed: {err:?}");
        }
        if let Some(pmic) = self.pmic.as_mut() {
            match pmic.power_off() {
                Ok(()) => {
                    self.delay.delay_millis(1000);
                    warn!("still powered a second after switching off");
                }
                Err(err) => warn!("switching off failed: {err:?}"),
            }
        }
        info!("deep sleep instead; press BOOT to wake");
        self.boot.listen(Event::LowLevel);
        self.boot
            .apply_wakeup_config(&WakeupConfig::default().with_low_power_path(true))
            .expect("GPIO0 is a low-power pad");
        self.low_power.sleep_deep(RtcSleepConfig::deep());
    }
}

/// Brings up the board and runs one game. It gets no `Launcher`, so it offers
/// no OUT; if it exits anyway, it starts again.
pub fn run(add_game: impl Fn(&mut App), background: Rgb565) -> ! {
    let mut board = Board::new();
    loop {
        drop(board.run(&add_game, background, false));
    }
}

fn start_speaker(pins: SpeakerPins, bus: &'static Bus) -> Speaker {
    // Signal loopback: the receiver (the mics) runs on the transmitter's
    // BCLK and WS, so both codecs share one clock.
    let i2s = I2s::new(
        pins.i2s,
        pins.dma,
        TdmConfig::new_tdm_philips()
            .with_sample_rate(Rate::from_hz(SAMPLE_RATE))
            .with_data_format(DataFormat::Data16Channel16)
            .with_channels(Channels::STEREO)
            .with_signal_loopback(true),
    )
    .expect("I2S config")
    .with_mclk(pins.mclk);
    let i2s_rx = i2s
        .i2s_rx
        .with_bclk(NoPin)
        .with_ws(NoPin)
        .with_din(pins.din)
        .build();
    let i2s_tx = i2s
        .i2s_tx
        .with_bclk(pins.bclk)
        .with_ws(pins.ws)
        .with_dout(pins.dout)
        .build();
    let mut mixer = Mixer::default();
    let audio = AudioOut::start(i2s_tx, speaker::stream_buffer(), &mut mixer);
    let mut amp = Output::new(pins.amp, Level::Low, OutputConfig::default());
    let mut codec = Es8311::new(RefCellDevice::new(bus));
    match codec.chip_id() {
        Ok((id1, id2)) => info!("ES8311 id {id1:#04x} {id2:#04x} (expect 0x83 0x11)"),
        Err(err) => warn!("ES8311 not answering: {err:?}"),
    }
    if let Err(err) = codec.init(SAMPLE_RATE, &mut amp) {
        warn!("ES8311 init failed: {err:?}");
    }
    Speaker {
        audio,
        mixer,
        amp,
        mic: MicPort::Idle(i2s_rx),
    }
}

/// Brings up the ES7210 and the receive stream, once; the speaker's stream
/// is already supplying the clocks.
fn start_mic(port: &mut MicPort, bus: &'static Bus) {
    if !matches!(port, MicPort::Idle(_)) {
        return;
    }
    let MicPort::Idle(i2s_rx) = core::mem::replace(port, MicPort::Failed) else {
        return;
    };
    match Es7210::new(RefCellDevice::new(bus)).init() {
        Ok(()) => {
            info!("ES7210 mics listening");
            *port = MicPort::Listening(MicIn::start(i2s_rx, mic::stream_buffer()));
        }
        Err(err) => warn!("ES7210 init failed ({err:?}); no voice commands"),
    }
}

/// The panel's brightness register for the chosen `percent`, a quarter of
/// it while idle (never fully dark).
fn panel_brightness(percent: u8, idle: bool) -> u8 {
    let level = u16::from(FULL_BRIGHTNESS) * u16::from(percent) / 100;
    let level = if idle { level / IDLE_DIVISOR } else { level };
    level.clamp(1, u16::from(FULL_BRIGHTNESS)) as u8
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

/// Frame time and panel traffic, summed for the serial log every 2 s.
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
