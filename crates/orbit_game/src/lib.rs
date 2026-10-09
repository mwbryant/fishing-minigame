//! Orbit: circular pong for a round touch screen, as a `no_std` Bevy plugin.
//! The finger's angle from the center steers a paddle round the rim, and each
//! return scores a point.
//!
//! Platforms (desktop simulator, ESP32 firmware) own the loop: each frame they
//! write the finger position into [`Touch`], call `app.update()`, then
//! rasterize the [`Frame`] shape list with `orbit_gfx`.

#![no_std]

extern crate alloc;

mod draw;
mod play;
mod screens;
pub mod testing;

use bevy::{prelude::*, state::app::StatesPlugin};
use orbit_core::CorePlugin;

pub use draw::BACKGROUND;
pub use orbit_core::{Frame, Rng, Touch};
pub use play::{Ball, Paddle, Score, arena};
pub use screens::Screen;

pub struct OrbitPlugin;

impl Plugin for OrbitPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<CorePlugin>() {
            app.add_plugins(CorePlugin);
        }
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        app.add_plugins((screens::plugin, play::plugin, draw::plugin));
    }
}
