//! QEMU smoke test for the firmware's runtime: same heap layout, Bevy clock,
//! game and presenter as `firmware/src/main.rs`, with a byte-counting panel and
//! scripted touch. Prints `SMOKE PASS` or `SMOKE FAIL: ...`; run via `run.sh`.

#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

extern crate alloc;

use alloc::vec;
use core::time::Duration;

use bevy::{math::ops, platform::time::Instant, prelude::*};
use esp_alloc::MemoryCapability;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock, delay::Delay, main, rng::Rng as HardwareRng, time::Instant as HalInstant,
};
use log::{error, info};
use orbit_game::{BACKGROUND, Frame, OrbitPlugin, Rng, Screen, Touch, arena::SCREEN_CENTER};
use orbit_gfx::{Panel, Presenter, Rectangle, SCREEN_SIZE};

esp_bootloader_esp_idf::esp_app_desc!();

/// Keep in sync with firmware/src/main.rs.
const BAND_BYTES: usize = SCREEN_SIZE as usize * 2 * 8;
/// The firmware's 212 KB plus the ~20 KB of statics this build lacks (DMA
/// buffers, drivers), so both stacks come out the same size. `run.sh` prints
/// the two `.stack` sizes; adjust this when they drift apart.
const INTERNAL_HEAP_BYTES: usize = 232 * 1024;
const FRAMES: u32 = 2000;

struct NullPanel(usize);

impl Panel for NullPanel {
    fn write(&mut self, _: Rectangle, bytes: &[u8]) {
        self.0 += bytes.len();
    }
}

#[main]
fn main() -> ! {
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

    let mut app = App::new();
    app.add_plugins(OrbitPlugin)
        .insert_resource(Rng::new(HardwareRng::new().random()));
    app.finish();
    app.cleanup();
    app.update();
    info!("bevy built; heap {}", esp_alloc::HEAP.stats());

    let mut presenter = Presenter::new(vec![0u8; BAND_BYTES], BACKGROUND);
    let mut panel = NullPanel(0);
    let mut last_screen = Screen::Title;
    let (mut starts, mut losses, mut peak_psram) = (0, 0, psram_used());
    for frame in 0..FRAMES {
        app.world_mut()
            .resource_mut::<Touch>()
            .set(scripted_finger(frame));
        app.update();
        presenter.present(&app.world().resource::<Frame>().shapes, &mut panel);

        let screen = *app.world().resource::<State<Screen>>().get();
        if screen != last_screen {
            info!("frame {frame}: {last_screen:?} -> {screen:?}");
            starts += u32::from(screen == Screen::Playing);
            losses += u32::from(screen == Screen::GameOver);
            last_screen = screen;
        }
        peak_psram = peak_psram.max(psram_used());
    }

    let game_secs = app.world().resource::<Time>().elapsed_secs();
    info!(
        "{FRAMES} frames, {game_secs:.1}s game time, {} KB to panel, {starts} starts, \
         {losses} losses, peak PSRAM {peak_psram} B; heap {}",
        panel.0 / 1024,
        esp_alloc::HEAP.stats()
    );
    if starts < 2 || losses < 1 {
        error!("SMOKE FAIL: game flow stalled ({starts} starts, {losses} losses)");
    } else if peak_psram > 0 {
        error!("SMOKE FAIL: Bevy spilled {peak_psram} B into PSRAM");
    } else {
        info!("SMOKE PASS");
    }
    let delay = Delay::new();
    loop {
        delay.delay_millis(1000);
    }
}

/// Lifts the finger for the first second of every 15, then circles the rim:
/// the press starts (or retries) a round and the paddle sweeps around.
fn scripted_finger(frame: u32) -> Option<Vec2> {
    let phase = frame % 900;
    if phase < 60 {
        return None;
    }
    let angle = phase as f32 * 0.07;
    Some(SCREEN_CENTER + Vec2::new(ops::cos(angle), ops::sin(angle)) * 150.0)
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
