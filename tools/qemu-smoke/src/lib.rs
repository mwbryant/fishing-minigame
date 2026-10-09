//! QEMU smoke harness shared by the per-game binaries: the firmware's heap
//! layout and Bevy clock, a byte-counting panel, and a PASS/FAIL verdict.

#![no_std]
#![feature(asm_experimental_arch)]

extern crate alloc;

use alloc::{vec, vec::Vec};
use core::time::Duration;

use bevy::{platform::time::Instant, prelude::*};
use esp_alloc::MemoryCapability;
use esp_hal::{
    clock::CpuClock, delay::Delay, peripherals::FLASH, rng::Rng as HardwareRng,
    time::Instant as HalInstant,
};
use log::{error, info};
use orbit_core::{Frame, Launcher, Rng, SaveBlob};
use orbit_gfx::{Panel, Presenter, Rectangle, Rgb565, SCREEN_SIZE};

/// Keep in sync with firmware/src/lib.rs.
const BAND_BYTES: usize = SCREEN_SIZE as usize * 2 * 8;
/// The firmware's 212 KB plus the ~20 KB of statics this build lacks (DMA
/// buffers, drivers), so both stacks come out the same size. `run.sh` prints
/// the two `.stack` sizes; adjust this when they drift apart.
const INTERNAL_HEAP_BYTES: usize = 238 * 1024;

pub struct Smoke {
    pub app: App,
    presenter: Presenter<alloc::vec::Vec<u8>>,
    panel: NullPanel,
    peak_psram: usize,
}

struct NullPanel(usize);

impl Panel for NullPanel {
    fn write(&mut self, _: Rectangle, bytes: &[u8]) {
        self.0 += bytes.len();
    }
}

impl Smoke {
    /// Boots like the firmware does and builds the app with `add_game`.
    /// Returns the flash peripheral for games that persist saves.
    pub fn boot(add_game: impl FnOnce(&mut App), background: Rgb565) -> (Self, FLASH<'static>) {
        esp_println::logger::init_logger_from_env();
        // SAFETY: QEMU boots the S3 with the FPU disabled (espressif/qemu#154);
        // real silicon doesn't need this.
        unsafe { core::arch::asm!("movi {0}, 1", "wsr {0}, cpenable", "rsync", out(reg) _) };
        let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
        esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);
        esp_alloc::heap_allocator!(size: INTERNAL_HEAP_BYTES);
        esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);
        // SAFETY: called once, before anything reads Bevy's clock.
        unsafe { Instant::set_elapsed(since_boot) };

        let app = build(add_game, None, false);
        info!("bevy built; heap {}", esp_alloc::HEAP.stats());
        let smoke = Self {
            app,
            presenter: Presenter::new(vec![0u8; BAND_BYTES], background),
            panel: NullPanel(0),
            peak_psram: psram_used(),
        };
        (smoke, peripherals.FLASH)
    }

    /// Drops the running app and builds the next one, as the Fishing Minigame
    /// firmware does, with `saved` as the record in storage.
    pub fn relaunch(
        &mut self,
        add_game: impl FnOnce(&mut App),
        background: Rgb565,
        saved: Option<Vec<u8>>,
        launched: bool,
    ) {
        let old = core::mem::replace(&mut self.app, App::empty());
        drop(old);
        self.app = build(add_game, saved, launched);
        self.presenter = Presenter::new(vec![0u8; BAND_BYTES], background);
        self.peak_psram = self.peak_psram.max(psram_used());
    }

    /// Steps frames for `secs` of game time (emulated frames are short, so
    /// scripts go by game time, never frame counts).
    pub fn wait(&mut self, secs: f32) {
        let until = self.app.world().resource::<Time>().elapsed_secs() + secs;
        while self.app.world().resource::<Time>().elapsed_secs() < until {
            self.frame();
        }
    }

    /// Updates once and presents the frame, tracking PSRAM use.
    pub fn frame(&mut self) {
        self.app.update();
        self.presenter.present(
            &self.app.world().resource::<Frame>().shapes,
            &mut self.panel,
        );
        self.peak_psram = self.peak_psram.max(psram_used());
    }

    /// Logs the verdict (any `failure`, or PSRAM use, fails) and idles.
    pub fn finish(self, failure: Option<&str>) -> ! {
        let game_secs = self.app.world().resource::<Time>().elapsed_secs();
        info!(
            "{game_secs:.1}s game time, {} KB to panel, peak PSRAM {} B; heap {}",
            self.panel.0 / 1024,
            self.peak_psram,
            esp_alloc::HEAP.stats()
        );
        match failure {
            Some(reason) => error!("SMOKE FAIL: {reason}"),
            None if self.peak_psram > 0 => {
                error!("SMOKE FAIL: Bevy spilled {} B into PSRAM", self.peak_psram)
            }
            None => info!("SMOKE PASS"),
        }
        let delay = Delay::new();
        loop {
            delay.delay_millis(1000);
        }
    }
}

fn build(add_game: impl FnOnce(&mut App), saved: Option<Vec<u8>>, launched: bool) -> App {
    let mut app = App::new();
    add_game(&mut app);
    if launched {
        app.insert_resource(Launcher);
    }
    app.insert_resource(Rng::new(HardwareRng::new().random()))
        .insert_resource(SaveBlob::from_storage(saved));
    app.finish();
    app.cleanup();
    app.update();
    app
}

/// Internal-SRAM heap in use, for leak checks across relaunches.
pub fn internal_heap_used() -> usize {
    esp_alloc::HEAP
        .stats()
        .region_stats
        .iter()
        .flatten()
        .filter(|region| !region.capabilities.contains(MemoryCapability::External))
        .map(|region| region.used)
        .sum()
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
