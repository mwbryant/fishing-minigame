//! Deterministic headless apps for game tests: one 60 Hz step per update.

use alloc::vec::Vec;
use core::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};

use crate::{Buttons, Mic, SaveBlob, Tilt, Touch};

pub const TICK_HZ: f64 = 60.0;

/// Builds an app with `add_game`, as if `saved` were the bytes in storage,
/// and runs its first update.
pub fn headless(add_game: impl FnOnce(&mut App), saved: Option<Vec<u8>>) -> App {
    let mut app = App::new();
    add_game(&mut app);
    app.insert_resource(SaveBlob::from_storage(saved))
        .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / TICK_HZ,
        )));
    app.finish();
    app.cleanup();
    update(&mut app);
    app
}

/// One update. With the `crowding` feature, it then checks that no text in
/// the frame is crowded (`orbit_gfx::crowding`) and panics naming what is.
/// `CROWDING_LOG=<file>` appends the findings to the file instead.
pub fn update(app: &mut App) {
    app.update();
    #[cfg(feature = "crowding")]
    crowding::check(app);
}

pub fn tick(app: &mut App, frames: u32) {
    for _ in 0..frames {
        update(app);
    }
}

pub fn hold_action(app: &mut App, frames: u32) {
    app.world_mut().resource_mut::<Buttons>().action.set(true);
    tick(app, frames);
}

pub fn release_action(app: &mut App) {
    app.world_mut().resource_mut::<Buttons>().action.set(false);
    update(app);
}

/// Press and release on consecutive updates.
pub fn click_action(app: &mut App) {
    hold_action(app, 1);
    release_action(app);
}

/// Finger down for one update, up for the next.
pub fn tap(app: &mut App, at: Vec2) {
    app.world_mut().resource_mut::<Touch>().set(Some(at));
    update(app);
    app.world_mut().resource_mut::<Touch>().set(None);
    update(app);
}

/// Says `word` (mono samples at `orbit_audio::SAMPLE_RATE`) into the mic
/// of a quiet room, a frame's worth of samples per update, then waits long
/// enough for the listener to hear that it ended. It streams, as a mic
/// driver does, so heap measurements see only what the game allocates.
pub fn speak(app: &mut App, word: &[i16]) {
    let per_update = (orbit_audio::SAMPLE_RATE as f64 / TICK_HZ) as usize;
    let quiet = [0i16; 512];
    let quiet_updates = |secs: f64| (secs * TICK_HZ) as usize;
    let push = |app: &mut App, samples: &[i16]| {
        app.world_mut().resource_mut::<Mic>().push(samples);
        update(app);
    };
    for _ in 0..quiet_updates(0.3) {
        push(app, &quiet[..per_update]);
    }
    for chunk in word.chunks(per_update) {
        push(app, chunk);
    }
    for _ in 0..quiet_updates(0.5) {
        push(app, &quiet[..per_update]);
    }
}

/// Poses the device directly, as if the IMU had reported it.
pub fn set_tilt(app: &mut App, steer_deg: f32, lean_deg: f32) {
    let mut tilt = app.world_mut().resource_mut::<Tilt>();
    tilt.steer_deg = steer_deg;
    tilt.lean_deg = lean_deg;
}

/// Plays like someone who doesn't know the game, for `secs` of game time:
/// turns and leans the device anywhere, presses, holds, taps all over the
/// glass and sometimes holds BOOT toward sleep (never long enough to sleep).
/// With the `crowding` feature every frame on the way is checked.
pub fn monkey(app: &mut App, secs: f32, seed: u32) {
    let mut rng = crate::Rng::new(seed);
    let mut left = (secs * TICK_HZ as f32) as u32;
    let frames = |rng: &mut crate::Rng, most: u32| 1 + rng.next_u32() % most;
    while left > 0 {
        let spent = match rng.next_u32() % 7 {
            0 => {
                let steer = rng.next_f32() * 140.0 - 70.0;
                let lean = rng.next_f32() * 40.0 - 20.0;
                set_tilt(app, steer, lean);
                update(app);
                1
            }
            1 => {
                click_action(app);
                2
            }
            2 => {
                let held = frames(&mut rng, 90);
                hold_action(app, held);
                release_action(app);
                held + 1
            }
            3 | 4 => {
                let angle = rng.next_f32() * core::f32::consts::TAU;
                let reach = rng.next_f32() * 225.0;
                let (sin, cos) = bevy::math::ops::sin_cos(angle);
                tap(app, Vec2::new(233.0 + cos * reach, 233.0 + sin * reach));
                2
            }
            5 if rng.next_u32().is_multiple_of(4) => {
                let held = frames(&mut rng, 140);
                app.world_mut().resource_mut::<Buttons>().zero.set(true);
                tick(app, held);
                app.world_mut().resource_mut::<Buttons>().zero.set(false);
                update(app);
                held + 1
            }
            _ => {
                let waited = frames(&mut rng, 40);
                tick(app, waited);
                waited
            }
        };
        left = left.saturating_sub(spent);
    }
}

/// Every label in the current frame.
pub fn labels(app: &App) -> Vec<alloc::string::String> {
    app.world()
        .resource::<crate::Frame>()
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            orbit_gfx::Shape::Label { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

pub fn shows(app: &App, text: &str) -> bool {
    labels(app).iter().any(|label| label == text)
}

#[cfg(feature = "crowding")]
mod crowding {
    use core::cell::RefCell;
    use std::{
        format,
        fs::OpenOptions,
        io::Write,
        string::{String, ToString},
        vec::Vec,
    };

    use bevy::prelude::*;
    use orbit_gfx::{Clash, Shape, ink};

    use crate::Frame;

    fn describe(shape: &Shape) -> String {
        match shape {
            Shape::Label { text, .. } => format!("{text:?} at {:?}", ink(shape)),
            other => {
                let kind = format!("{other:?}");
                let kind = kind.split([' ', '{', '(']).next().unwrap_or("").to_string();
                format!("a {kind} at {:?}", other.bounds())
            }
        }
    }

    std::thread_local! {
        /// The last frame checked, so a screen that doesn't change is checked once.
        static CHECKED: RefCell<Vec<Shape>> = const { RefCell::new(Vec::new()) };
    }

    pub fn check(app: &App) {
        let shapes = &app.world().resource::<Frame>().shapes;
        if CHECKED.with_borrow(|checked| checked == shapes) {
            return;
        }
        CHECKED.with_borrow_mut(|checked| checked.clone_from(shapes));
        let found = orbit_gfx::crowding(shapes);
        if found.is_empty() {
            return;
        }
        let lines: Vec<String> = found
            .iter()
            .map(|crowding| {
                let label = describe(&shapes[crowding.label]);
                match crowding.clash {
                    Clash::Text(other) => format!("{label} overlaps {}", describe(&shapes[other])),
                    Clash::Over(other) => format!("{label} is under {}", describe(&shapes[other])),
                    Clash::Under(other) => {
                        format!("{label} is half on {}", describe(&shapes[other]))
                    }
                    Clash::OffGlass => format!("{label} runs off the glass"),
                }
            })
            .collect();
        let test = std::thread::current().name().unwrap_or("?").to_string();
        if let Ok(path) = std::env::var("CROWDING_LOG") {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            for line in &lines {
                writeln!(file, "{test}\t{line}").unwrap();
            }
            return;
        }
        panic!("crowded text in {test}:\n{}", lines.join("\n"));
    }
}
