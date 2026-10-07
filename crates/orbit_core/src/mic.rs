//! The board's microphones as words heard: platforms push the mic's samples,
//! and games take each word as a [`VoicePrint`] to compare with the ones a
//! player taught.

use bevy::prelude::*;
use orbit_audio::voice::{Listener, VoicePrint};

/// What the mic hears. Only apps that add [`MicPlugin`] have one, and
/// platforms power the mics and push samples only while it exists.
#[derive(Resource, Default, Debug)]
pub struct Mic {
    listener: Listener,
}

impl Mic {
    /// Mono samples at `orbit_audio::SAMPLE_RATE`, as they arrive.
    pub fn push(&mut self, samples: &[i16]) {
        self.listener.push(samples);
    }

    /// The last word heard, once.
    pub fn take(&mut self) -> Option<VoicePrint> {
        self.listener.take()
    }

    /// How loud it is over the room's quiet, 0 to 1, for a level meter.
    pub fn level(&self) -> f32 {
        self.listener.level()
    }

    pub fn speaking(&self) -> bool {
        self.listener.speaking()
    }
}

/// Listens on the board's mics. Opt-in like `ControlsPlugin`; it adds no
/// systems, since the listener runs as samples are pushed.
pub struct MicPlugin;

impl Plugin for MicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Mic>();
    }
}
