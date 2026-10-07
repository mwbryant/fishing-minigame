use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::time::Duration;

use angler_game::{
    Phase, Session,
    cast::{self, Flight},
    testing as angler, ui,
};
use bevy::{math::ops, platform::time::Instant, prelude::*};
use orbit_core::testing as core_testing;
use orbit_game::{
    arena::SCREEN_CENTER,
    testing::{headless_app, press, tap, tick},
};
use orbit_ui::{Key, key_turn};

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

/// Angler: the log, settings and GUTEN screens, MENU back from aiming, then
/// three full cast, bite, fight and catch cycles: two kept and named by
/// turning, one released.
#[unsafe(no_mangle)]
pub extern "C" fn measure_angler() -> *const u32 {
    start_clock();
    let mut app = angler::headless_app();
    let built = CURRENT.load(Relaxed);
    angler::tap(&mut app, ui::LOG.center());
    angler::tap(&mut app, Vec2::splat(233.0));
    angler::tap(&mut app, ui::SETTINGS.center());
    angler::tap(&mut app, ui::HARDER.center());
    angler::tap(&mut app, ui::GUTEN.center());
    angler::tick(&mut app, 120);
    angler::tap(&mut app, Vec2::splat(233.0));
    angler::tap(&mut app, ui::EASIER.center());
    angler::tap(&mut app, ui::BACK.center());
    angler::click_action(&mut app);
    angler::tap(&mut app, ui::MENU.center());
    angler::click_action(&mut app);
    for catch in 0..3 {
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
        if catch < 2 {
            angler::click_action(&mut app);
            for key in [
                Key::Letter('F'),
                Key::Letter('I'),
                Key::Letter('N'),
                Key::Done,
            ] {
                angler::set_tilt(&mut app, key_turn(key), 0.0);
                angler::tick(&mut app, 1);
                angler::click_action(&mut app);
            }
            angler::tick(&mut app, 120);
        } else {
            angler::set_tilt(&mut app, 25.0, 0.0);
            angler::tick(&mut app, 1);
            angler::click_action(&mut app);
        }
    }
    report(app, built)
}

fn built(add_game: impl FnOnce(&mut App)) -> App {
    core_testing::headless(add_game, None)
}

/// A save whose tank holds three fish, each knowing SPLASH by a swipe up,
/// with `gold` in the purse.
fn tank(gold: u32, mock_walk: bool) -> Vec<u8> {
    let mut pals = pals::Pals::default();
    pals.earn(gold);
    pals.mock_walk = mock_walk;
    for (i, text) in ["GILLY", "BUBBA", "KOI"].into_iter().enumerate() {
        let mut name = pals::Name::default();
        text.chars().for_each(|letter| {
            name.push(letter);
        });
        let mut pet = pals::Pet::new(
            i as u8 * 3,
            i as u8,
            300,
            60,
            name,
            &mut orbit_core::Rng::new(5),
        );
        pet.teach(
            0,
            pals::Trigger {
                gesture: Some(pals::Gesture::SwipeUp),
                voice: None,
            },
        );
        pet.tricks[0].learned = 60;
        pet.walk(1000.0);
        pals.keep(pet).ok();
    }
    let mut blob = orbit_core::SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &pals);
    blob.take_pending().unwrap_or_default()
}

fn with_save(add_game: impl FnOnce(&mut App), saved: Vec<u8>) -> App {
    core_testing::headless(add_game, Some(saved))
}

/// A finger swiping up the middle of the glass.
fn swipe_up(app: &mut App) {
    for i in 0..=10 {
        app.world_mut()
            .resource_mut::<orbit_core::Touch>()
            .set(Some(Vec2::new(233.0, 330.0 - i as f32 * 12.0)));
        app.update();
    }
    app.world_mut()
        .resource_mut::<orbit_core::Touch>()
        .set(None);
    app.update();
}

const SPIN_PCM: &[u8] =
    include_bytes!("../../../crates/orbit_audio/tests/voices/daniel-spin-170.pcm");

/// The fixture as samples, built at compile time so the measurement doesn't
/// count a copy of it.
static SPIN: [i16; SPIN_PCM.len() / 2] = samples(SPIN_PCM);

const fn samples<const N: usize>(bytes: &[u8]) -> [i16; N] {
    let mut out = [0; N];
    let mut i = 0;
    while i < N {
        out[i] = i16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]);
        i += 1;
    }
    out
}

/// The town: ten seconds of walking every way, then the settings with sound
/// and mock walks toggled and the reset question asked and kept.
#[unsafe(no_mangle)]
pub extern "C" fn measure_town() -> *const u32 {
    use pals_game::{
        Place,
        town::{Town, TownPlugin, ui},
    };
    start_clock();
    let mut app = with_save(
        |app| {
            app.add_plugins(TownPlugin);
        },
        tank(120, false),
    );
    let built = CURRENT.load(Relaxed);
    for frame in 0..600 {
        let t = frame as f32 * 0.02;
        core_testing::set_tilt(&mut app, ops::sin(t) * 30.0, ops::cos(t * 0.7) * 30.0);
        app.update();
    }
    app.world_mut()
        .resource_mut::<Town>()
        .stand_at(Place::Settings);
    core_testing::set_tilt(&mut app, 0.0, 0.0);
    core_testing::tick(&mut app, 2);
    core_testing::click_action(&mut app);
    for button in [
        ui::SOUND,
        ui::MOCK_WALK,
        ui::SOUND,
        ui::MOCK_WALK,
        ui::RESET,
        ui::KEEP,
        ui::BACK,
    ] {
        core_testing::tap(&mut app, button.center());
    }
    core_testing::tick(&mut app, 60);
    report(app, built)
}

/// The house with three fish: the tank, a card, the tricks list, a new trick
/// taught a spoken word twice, then practice by stroke and by word.
#[unsafe(no_mangle)]
pub extern "C" fn measure_home() -> *const u32 {
    use pals_game::home::{Home, HomePlugin};
    start_clock();
    let mut app = with_save(
        |app| {
            app.add_plugins(HomePlugin);
        },
        tank(0, false),
    );
    let built = CURRENT.load(Relaxed);
    core_testing::tick(&mut app, 120);
    core_testing::click_action(&mut app);
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, orbit_ui::LIST_STEP_DEG, 0.0);
    core_testing::tick(&mut app, 2);
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 0.0, 0.0);
    core_testing::tick(&mut app, 60 * 4);
    core_testing::speak(&mut app, &SPIN);
    core_testing::speak(&mut app, &SPIN);
    core_testing::click_action(&mut app);
    for _ in 0..3 {
        app.world_mut().resource_mut::<Home>().rig_next_try(true);
        core_testing::speak(&mut app, &SPIN);
        core_testing::tick(&mut app, 120);
        core_testing::tap(&mut app, Vec2::new(233.0, 230.0));
    }
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 0.0, 0.0);
    core_testing::tick(&mut app, 2);
    core_testing::click_action(&mut app);
    for _ in 0..3 {
        swipe_up(&mut app);
        core_testing::tick(&mut app, 60 * 5);
    }
    report(app, built)
}

/// The park: a minute of mock walking, then the summary.
#[unsafe(no_mangle)]
pub extern "C" fn measure_park() -> *const u32 {
    start_clock();
    let mut app = with_save(
        |app| {
            app.add_plugins(pals_game::walk::WalkPlugin);
        },
        tank(0, true),
    );
    let built = CURRENT.load(Relaxed);
    core_testing::click_action(&mut app);
    core_testing::tick(&mut app, 60 * 60);
    core_testing::click_action(&mut app);
    core_testing::tick(&mut app, 30);
    report(app, built)
}

/// The contest hall: a beginner show with commands, then the results.
#[unsafe(no_mangle)]
pub extern "C" fn measure_contest() -> *const u32 {
    start_clock();
    let mut app = with_save(
        |app| {
            app.add_plugins(pals_game::contest::ContestPlugin);
        },
        tank(0, false),
    );
    let built = CURRENT.load(Relaxed);
    core_testing::click_action(&mut app);
    core_testing::click_action(&mut app);
    for _ in 0..6 {
        swipe_up(&mut app);
        core_testing::tick(&mut app, 120);
    }
    core_testing::tick(&mut app, (pals_game::contest::SHOW_SECS * 60.0) as u32);
    report(app, built)
}

/// The hat shop: the ring swept round and three hats bought and worn.
#[unsafe(no_mangle)]
pub extern "C" fn measure_shop() -> *const u32 {
    use pals_game::shop::hat_turn;
    start_clock();
    let mut app = with_save(
        |app| {
            app.add_plugins(pals_game::shop::ShopPlugin);
        },
        tank(400, false),
    );
    let built = CURRENT.load(Relaxed);
    for frame in 0..300 {
        core_testing::set_tilt(&mut app, ops::sin(frame as f32 * 0.03) * 60.0, 0.0);
        app.update();
    }
    for choice in [2, 5, 8] {
        core_testing::set_tilt(&mut app, hat_turn(choice), 0.0);
        app.update();
        core_testing::click_action(&mut app);
        core_testing::tap(&mut app, pals_game::shop::ui::MIRROR.center());
    }
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
