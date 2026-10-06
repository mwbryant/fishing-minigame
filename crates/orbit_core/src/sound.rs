use alloc::vec::Vec;

use bevy::prelude::*;
use orbit_audio::{Clip, ClipRef, SoundCommand};

/// Commands kept when nothing drains the queue (a harness without a
/// speaker); the one-voice mixer only acts on the newest anyway.
const MAX_PENDING: usize = 16;

/// What the game wants the speaker to do, in order. The platform drains it
/// into its `orbit_audio::Mixer` after every update.
#[derive(Resource, Default, Debug)]
pub struct SoundQueue {
    commands: Vec<SoundCommand>,
}

impl SoundQueue {
    pub fn play(&mut self, clip: &'static Clip) {
        self.push(SoundCommand::Play(ClipRef(clip)));
    }

    pub fn stop(&mut self) {
        self.push(SoundCommand::Stop);
    }

    fn push(&mut self, command: SoundCommand) {
        if self.commands.len() == MAX_PENDING {
            self.commands.remove(0);
        }
        self.commands.push(command);
    }

    pub fn pending(&self) -> &[SoundCommand] {
        &self.commands
    }

    pub fn drain(&mut self) -> impl Iterator<Item = SoundCommand> + '_ {
        self.commands.drain(..)
    }
}
