//! Platform plumbing every game on the round board shares, as `no_std` Bevy
//! resources. Each frame the platform (desktop simulator or ESP32 firmware)
//! writes the inputs and runs `app.update()`. Games read the inputs and fill
//! the outputs, then the platform draws [`Frame`] and stores
//! [`SaveBlob::take_pending`].

#![no_std]

#[cfg(feature = "crowding")]
extern crate std;

extern crate alloc;

mod buttons;
mod gps;
mod mic;
mod motion;
mod power;
pub mod preferences;
mod rng;
mod save;
mod sound;
mod steps;
pub mod testing;
mod touch;

use alloc::vec::Vec;

use bevy::{prelude::*, time::TimePlugin};
use orbit_gfx::Shape;

pub use buttons::{ButtonState, Buttons};
pub use gps::{Fix, Gps, GpsPlugin};
pub use mic::{Mic, MicPlugin};
pub use motion::{ImuFeed, Tilt};
pub use orbit_audio::{
    Clip, ClipRef, SoundCommand,
    voice::{MATCH_DISTANCE, VoicePrint, recognize},
};
pub use orbit_motion::ImuSample;
pub use power::{
    Battery, GOOD_NIGHT_SECS, IDLE_SECS, IDLE_TILT_DEG, Power, SLEEP_HINT_SECS, SLEEP_HOLD_SECS,
};
pub use rng::Rng;
pub use save::SaveBlob;
pub use sound::SoundQueue;
pub use steps::{Steps, StepsPlugin};
pub use touch::Touch;

/// Present when a menu launched the game. Games then offer an OUT button that
/// sends `AppExit`, and the platform returns to the menu.
#[derive(Resource, Default, Debug)]
pub struct Launcher;

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

/// Sound requests for the platform's speaker, muted at startup if the
/// shared preferences say so. Opt-in like [`ControlsPlugin`].
pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundQueue>()
            .add_systems(Startup, apply_preferences);
    }
}

fn apply_preferences(blob: Res<SaveBlob>, mut sound: ResMut<SoundQueue>) {
    sound.set_muted(preferences::muted(&blob));
}

/// The physical buttons, IMU tilt (re-levelled by the zero button) and
/// [`Power`] (battery, idle, sleep). Opt-in, because every resource and system
/// costs device heap. Platforms feed these only when present.
pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((buttons::plugin, motion::plugin, power::plugin));
    }
}
