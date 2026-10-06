//! Deterministic headless apps for game tests: one 60 Hz step per update.

use alloc::vec::Vec;
use core::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};

use crate::{Buttons, SaveBlob, Tilt, Touch};

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
    app.update();
    app
}

pub fn tick(app: &mut App, frames: u32) {
    for _ in 0..frames {
        app.update();
    }
}

pub fn hold_action(app: &mut App, frames: u32) {
    app.world_mut().resource_mut::<Buttons>().action.set(true);
    tick(app, frames);
}

pub fn release_action(app: &mut App) {
    app.world_mut().resource_mut::<Buttons>().action.set(false);
    app.update();
}

/// Press and release on consecutive updates.
pub fn click_action(app: &mut App) {
    hold_action(app, 1);
    release_action(app);
}

/// Finger down for one update, up for the next.
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
