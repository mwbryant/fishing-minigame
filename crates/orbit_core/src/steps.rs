//! Steps the platform's pedometer counts, for walks. Only apps that add
//! [`StepsPlugin`] have [`Steps`], and platforms count steps only while it
//! exists.

use bevy::prelude::*;

/// Steps counted since the game last took them.
#[derive(Resource, Default, Debug)]
pub struct Steps {
    new: u32,
}

impl Steps {
    /// Platforms: `count` more steps were taken.
    pub fn add(&mut self, count: u32) {
        self.new = self.new.saturating_add(count);
    }

    /// The steps taken since the last call.
    pub fn take(&mut self) -> u32 {
        core::mem::take(&mut self.new)
    }
}

pub struct StepsPlugin;

impl Plugin for StepsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Steps>();
    }
}
