//! Deterministic headless app: one 60 Hz fixed step per `app.update()`.

use alloc::vec::Vec;
use core::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};
use orbit_core::{Buttons, SaveBlob, Tilt, Touch};

use crate::{AnglerPlugin, Phase, Session};

pub const TICK_HZ: f64 = 60.0;

/// Boots with `saved` as the bytes found in storage.
pub fn headless_app_with_save(saved: Option<Vec<u8>>) -> App {
    let mut app = App::new();
    app.add_plugins(AnglerPlugin)
        .insert_resource(SaveBlob::from_storage(saved))
        .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / TICK_HZ,
        )));
    app.finish();
    app.cleanup();
    app.update();
    app
}

pub fn headless_app() -> App {
    headless_app_with_save(None)
}

pub fn tick(app: &mut App, frames: u32) {
    for _ in 0..frames {
        app.update();
    }
}

pub fn phase(app: &App) -> Phase {
    app.world().resource::<Session>().phase.clone()
}

/// Holds the action button for `frames` updates.
pub fn hold_action(app: &mut App, frames: u32) {
    app.world_mut().resource_mut::<Buttons>().action.set(true);
    tick(app, frames);
}

pub fn release_action(app: &mut App) {
    app.world_mut().resource_mut::<Buttons>().action.set(false);
    app.update();
}

/// Press and release on consecutive frames.
pub fn click_action(app: &mut App) {
    hold_action(app, 1);
    release_action(app);
}

pub fn press_zero(app: &mut App) {
    app.world_mut().resource_mut::<Buttons>().zero.set(true);
    app.update();
    app.world_mut().resource_mut::<Buttons>().zero.set(false);
    app.update();
}

pub fn tap(app: &mut App, at: Vec2) {
    app.world_mut().resource_mut::<Touch>().set(Some(at));
    app.update();
    app.world_mut().resource_mut::<Touch>().set(None);
    app.update();
}

/// Poses the device directly, as if the IMU had reported it.
pub fn set_tilt(app: &mut App, steer_deg: f32, lean_deg: f32) {
    let mut tilt = app.world_mut().resource_mut::<Tilt>();
    tilt.steer_deg = steer_deg;
    tilt.lean_deg = lean_deg;
}

/// Runs until `done` holds for the current phase, up to `max_frames`.
pub fn run_until(app: &mut App, max_frames: u32, done: impl Fn(&Phase) -> bool) -> bool {
    for _ in 0..max_frames {
        if done(&phase(app)) {
            return true;
        }
        app.update();
    }
    done(&phase(app))
}
