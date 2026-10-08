//! Deterministic headless app for tests: the clock advances exactly one
//! 60 Hz fixed step per `app.update()`, so `tick(app, 60)` is one second.

use core::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};

use crate::{OrbitPlugin, Touch};

pub const TICK_HZ: f64 = 60.0;

pub fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(OrbitPlugin)
        .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / TICK_HZ,
        )));
    app.finish();
    app.cleanup();
    orbit_core::testing::update(&mut app);
    app
}

pub fn tick(app: &mut App, frames: u32) {
    for _ in 0..frames {
        orbit_core::testing::update(app);
    }
}

/// Puts the finger down at `at` (screen pixels) for one frame, then lifts it.
pub fn tap(app: &mut App, at: Vec2) {
    press(app, Some(at));
    press(app, None);
}

/// Sets the finger position and runs one frame.
pub fn press(app: &mut App, at: Option<Vec2>) {
    app.world_mut().resource_mut::<Touch>().set(at);
    orbit_core::testing::update(app);
}
