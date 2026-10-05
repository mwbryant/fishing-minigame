use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::time::Duration;

use angler_game::{
    Phase, Session,
    cast::{self, Flight},
    testing as angler,
};
use bevy::{math::ops, platform::time::Instant, prelude::*};
use orbit_game::{
    arena::SCREEN_CENTER,
    testing::{headless_app, press, tap, tick},
};

struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let now = CURRENT.fetch_add(layout.size(), Relaxed) + layout.size();
        PEAK.fetch_max(now, Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

static CLOCK_US: AtomicU64 = AtomicU64::new(0);

fn fake_clock() -> Duration {
    Duration::from_micros(CLOCK_US.fetch_add(16_667, Relaxed))
}

static mut REPORT: [u32; 3] = [0; 3];

fn report(app: App, built: usize) -> *const u32 {
    let live = CURRENT.load(Relaxed);
    drop(app);
    // SAFETY: single-threaded wasm; the host reads REPORT after we return.
    unsafe {
        REPORT = [built as u32, live as u32, PEAK.load(Relaxed) as u32];
        (&raw const REPORT).cast()
    }
}

fn start_clock() {
    // SAFETY: called once per run, before anything reads Bevy's clock.
    unsafe { Instant::set_elapsed(fake_clock) };
}

/// Orbit: builds the app, plays 20 s of scripted input, idles 10 s. Returns
/// a pointer to `[bytes after build, bytes live at the end, peak bytes]`.
#[unsafe(no_mangle)]
pub extern "C" fn measure_orbit() -> *const u32 {
    start_clock();
    let mut app = headless_app();
    let built = CURRENT.load(Relaxed);
    tap(&mut app, SCREEN_CENTER);
    for frame in 0..1200 {
        let angle = frame as f32 * 0.05;
        let finger = SCREEN_CENTER + Vec2::new(ops::cos(angle), ops::sin(angle)) * 100.0;
        press(&mut app, Some(finger));
    }
    tick(&mut app, 600);
    report(app, built)
}

/// Angler: three full cast, bite, fight and catch cycles plus the fish log.
#[unsafe(no_mangle)]
pub extern "C" fn measure_angler() -> *const u32 {
    start_clock();
    let mut app = angler::headless_app();
    let built = CURRENT.load(Relaxed);
    angler::tap(&mut app, Vec2::splat(233.0));
    angler::tap(&mut app, Vec2::splat(233.0));
    angler::click_action(&mut app);
    for _ in 0..3 {
        let lands = Flight::launch(cast::elevation_deg(0.0), 0.7).landing_x();
        app.world_mut().resource_mut::<Session>().spot_x = lands;
        angler::hold_action(&mut app, (0.7 * cast::CHARGE_SECS * 60.0) as u32 + 1);
        angler::release_action(&mut app);
        angler::run_until(&mut app, 60 * 15, |p| matches!(p, Phase::Reel { .. }));
        for _ in 0..60 * 60 {
            let Phase::Reel { reel, .. } = angler::phase(&app) else {
                break;
            };
            let push = (((reel.zone_y - reel.fish_y) * 30.0 + reel.zone_v * 3.6) / 1100.0)
                .clamp(-1.0, 1.0);
            angler::set_tilt(&mut app, 0.0, push.signum() * (2.0 + push.abs() * 20.0));
            app.update();
        }
        angler::tick(&mut app, 60);
        angler::click_action(&mut app);
    }
    report(app, built)
}
