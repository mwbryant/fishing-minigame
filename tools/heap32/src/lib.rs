use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::time::Duration;

use angler_game::{
    Inventory, Phase, Session,
    cast::{self, Flight},
    testing as angler, ui,
};
use bevy::{math::ops, platform::time::Instant, prelude::*};
use orbit_core::testing as core_testing;
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

/// Angler: the log, settings, reset-confirm, GUTEN and hat shop screens (buying a hat),
/// MENU back from aiming, then three full cast, bite, fight and catch cycles.
#[unsafe(no_mangle)]
pub extern "C" fn measure_angler() -> *const u32 {
    start_clock();
    let mut app = angler::headless_app();
    let built = CURRENT.load(Relaxed);
    angler::tap(&mut app, ui::LOG.center());
    angler::tap(&mut app, Vec2::splat(233.0));
    angler::tap(&mut app, ui::SETTINGS.center());
    angler::tap(&mut app, ui::HARDER.center());
    angler::tap(&mut app, ui::RESET.center());
    angler::tap(&mut app, ui::KEEP.center());
    angler::tap(&mut app, ui::GUTEN.center());
    angler::tick(&mut app, 120);
    angler::tap(&mut app, Vec2::splat(233.0));
    angler::tap(&mut app, ui::EASIER.center());
    angler::tap(&mut app, ui::BACK.center());
    app.world_mut().resource_mut::<Inventory>().earn(40);
    angler::tap(&mut app, ui::SHOP.center());
    angler::tap(&mut app, ui::NEXT_HAT.center());
    angler::tap(&mut app, ui::BUY.center());
    angler::tap(&mut app, ui::NEXT_HAT.center());
    angler::tap(&mut app, ui::BACK.center());
    angler::click_action(&mut app);
    angler::tap(&mut app, ui::MENU.center());
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

fn built(add_game: impl FnOnce(&mut App)) -> App {
    core_testing::headless(add_game, None)
}

/// The arcade menu: the dial swept round, settings opened, sound toggled
/// twice, back, then a game picked.
#[unsafe(no_mangle)]
pub extern "C" fn measure_arcade() -> *const u32 {
    start_clock();
    let mut app = built(|app| {
        app.add_plugins(arcade::ArcadePlugin);
    });
    let built = CURRENT.load(Relaxed);
    for frame in 0..600 {
        core_testing::set_tilt(&mut app, ops::sin(frame as f32 * 0.02) * 70.0, 0.0);
        app.update();
    }
    core_testing::set_tilt(&mut app, -24.0, 0.0);
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 0.0, 0.0);
    core_testing::click_action(&mut app);
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 60.0, 0.0);
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 24.0, 0.0);
    core_testing::click_action(&mut app);
    core_testing::tick(&mut app, 60);
    report(app, built)
}

/// Spider Roulette: three rounds of spins until each ends.
#[unsafe(no_mangle)]
pub extern "C" fn measure_spider() -> *const u32 {
    start_clock();
    let mut app = built(|app| {
        app.add_plugins(spider_game::SpiderPlugin);
    });
    let built = CURRENT.load(Relaxed);
    for _ in 0..3 {
        core_testing::click_action(&mut app);
        for _ in 0..spider_game::EGGS {
            core_testing::click_action(&mut app);
            core_testing::tick(&mut app, 60 * 3 + 20);
        }
        core_testing::tick(&mut app, 150);
    }
    report(app, built)
}

/// Skate: a rider popping before each obstacle for 30 s, then a bail.
#[unsafe(no_mangle)]
pub extern "C" fn measure_skate() -> *const u32 {
    use skate_game::{BOARD_HALF, POP_DEG, Phase, Ride, SKATER_X};
    start_clock();
    let mut app = built(|app| {
        app.add_plugins(skate_game::SkatePlugin);
    });
    let built = CURRENT.load(Relaxed);
    core_testing::click_action(&mut app);
    for _ in 0..60 * 30 {
        let ride = app.world().resource::<Ride>();
        if ride.phase != Phase::Riding {
            break;
        }
        let close = ride
            .obstacles()
            .iter()
            .any(|o| (0.0..55.0).contains(&(o.x - SKATER_X - BOARD_HALF)));
        let steer = if close && !ride.airborne() {
            -(POP_DEG + 5.0)
        } else {
            0.0
        };
        core_testing::set_tilt(&mut app, steer, 0.0);
        app.update();
    }
    core_testing::set_tilt(&mut app, 0.0, 0.0);
    core_testing::tick(&mut app, 600);
    report(app, built)
}

/// Bees???: a steady hand for 30 s, then the jar tipped until it's empty.
#[unsafe(no_mangle)]
pub extern "C" fn measure_bees() -> *const u32 {
    use bees_game::{CONTROL_GAIN, Orbit};
    start_clock();
    let mut app = built(|app| {
        app.add_plugins(bees_game::BeesPlugin);
    });
    let built = CURRENT.load(Relaxed);
    core_testing::click_action(&mut app);
    for _ in 0..60 * 30 {
        let steer = app.world().resource::<Orbit>().drift_deg / CONTROL_GAIN;
        core_testing::set_tilt(&mut app, steer, 0.0);
        app.update();
    }
    core_testing::set_tilt(&mut app, 80.0, 0.0);
    core_testing::tick(&mut app, 60 * 20);
    report(app, built)
}
