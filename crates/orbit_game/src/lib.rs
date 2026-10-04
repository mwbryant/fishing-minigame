//! Orbit: circular pong for a round touch screen, as a `no_std` Bevy plugin.
//!
//! Platforms (desktop simulator, ESP32 firmware) own the loop: each frame they
//! write the finger position into [`Touch`], call `app.update()`, then
//! rasterize the [`Frame`] shape list with `orbit_gfx`.

#![no_std]

extern crate alloc;

mod draw;
mod input;
mod play;
mod rng;
mod screens;
pub mod testing;

use bevy::{prelude::*, state::app::StatesPlugin, time::TimePlugin};

pub use draw::{BACKGROUND, Frame};
pub use input::Touch;
pub use play::{Ball, Paddle, Score, arena};
pub use rng::Rng;
pub use screens::Screen;

pub struct OrbitPlugin;

impl Plugin for OrbitPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TimePlugin>() {
            app.add_plugins(TimePlugin);
        }
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        app.add_plugins((input::plugin, screens::plugin, play::plugin, draw::plugin));
    }
}
