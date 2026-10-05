use alloc::vec::Vec;

use bevy::prelude::*;
use orbit_audio::{Clip, ClipRef, SoundCommand};

/// What the game wants the speaker to do, in order. The platform drains it
/// into its `orbit_audio::Mixer` after every update.
#[derive(Resource, Default, Debug)]
pub struct SoundQueue {
    commands: Vec<SoundCommand>,
}

impl SoundQueue {
    pub fn play(&mut self, clip: &'static Clip) {
        self.commands.push(SoundCommand::Play(ClipRef(clip)));
    }

    pub fn stop(&mut self) {
        self.commands.push(SoundCommand::Stop);
    }

    pub fn pending(&self) -> &[SoundCommand] {
        &self.commands
    }

    pub fn drain(&mut self) -> impl Iterator<Item = SoundCommand> + '_ {
        self.commands.drain(..)
    }
}
