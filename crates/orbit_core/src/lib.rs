//! Platform plumbing every game on the round board shares, as `no_std` Bevy
//! resources. Platforms (desktop simulator, ESP32 firmware) write the inputs
//! each frame before `app.update()`, then rasterize [`Frame`] and persist
//! [`SaveBlob::take_pending`]; games read the inputs and fill the outputs.

#![no_std]

extern crate alloc;

mod buttons;
mod motion;
mod power;
mod rng;
mod save;
mod sound;
mod touch;

use alloc::vec::Vec;

use bevy::{prelude::*, time::TimePlugin};
use orbit_gfx::Shape;

pub use buttons::{ButtonState, Buttons};
pub use motion::{ImuFeed, Tilt};
pub use orbit_audio::{Clip, ClipRef, SoundCommand};
pub use orbit_motion::ImuSample;
pub use power::{Battery, Power};
pub use rng::Rng;
pub use save::SaveBlob;
pub use sound::SoundQueue;
pub use touch::Touch;

/// This frame's display list, back to front.
#[derive(Resource, Default, Debug)]
pub struct Frame {
    pub shapes: Vec<Shape>,
}

/// Frame, touch, rng and save plumbing every game needs.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TimePlugin>() {
            app.add_plugins(TimePlugin);
        }
        app.init_resource::<Frame>()
            .init_resource::<Rng>()
            .init_resource::<SaveBlob>()
            .add_plugins(touch::plugin);
    }
}

/// Sound requests for the platform's speaker. Opt-in like [`ControlsPlugin`].
pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundQueue>();
    }
}

/// The physical buttons, IMU tilt (with the zero button re-levelling it) and
/// [`Power`] (battery, idle, sleep). Opt-in: every resource and system costs
/// device heap, so games that only use touch leave it out, and platforms
/// feed these only when present.
pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Power>()
            .add_plugins((buttons::plugin, motion::plugin));
    }
}
