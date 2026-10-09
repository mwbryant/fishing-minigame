//! Angler: an Angry Birds-style cast and a Stardew Valley-style fight for the
//! round ESP32-S3 AMOLED board, as a `no_std` Bevy plugin.
//!
//! Turn the device like a steering wheel to aim, hold the action button to
//! charge the cast (release to throw), then lean the device forward and back
//! to keep the fish inside the catch zone. In Fishing Minigame this is the pond: a
//! catch can be kept, named by turning, and put in the tank (`pals`). The
//! log and settings persist through `orbit_core::SaveBlob`. The zero button
//! re-levels the IMU at any time.

#![no_std]

extern crate alloc;

pub mod audio;
pub mod cast;
mod draw;
pub mod log;
pub mod reel;
pub mod save;
pub mod session;
pub mod settings;
pub mod species;
pub mod sprites;
pub mod testing;
pub mod ui;

use bevy::prelude::*;
use orbit_core::{ControlsPlugin, CorePlugin, SoundPlugin};

pub use audio::{GUTEN_MORGEN, GUTEN_SPIN_SECS};
pub use draw::{BACKGROUND, PREVIEW_DOT, length_text, weight_text};
pub use log::{FishLog, Milestones};
pub use session::{Phase, Session};
pub use settings::{Difficulty, Settings};
pub use species::{Catch, SPECIES};

pub struct AnglerPlugin;

impl Plugin for AnglerPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<CorePlugin>() {
            app.add_plugins(CorePlugin);
        }
        if !app.is_plugin_added::<ControlsPlugin>() {
            app.add_plugins(ControlsPlugin);
        }
        if !app.is_plugin_added::<SoundPlugin>() {
            app.add_plugins(SoundPlugin);
        }
        app.add_plugins((session::plugin, draw::plugin));
    }
}
