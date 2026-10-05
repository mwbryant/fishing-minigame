use alloc::vec::Vec;

use bevy::prelude::*;
use orbit_motion::{ImuSample, MotionTracker};

use crate::Buttons;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<ImuFeed>()
        .init_resource::<Tilt>()
        .init_resource::<Tracker>()
        .add_systems(PreUpdate, track_tilt);
}

/// IMU samples since the last update, in the screen frame (see
/// `orbit_motion`). Platforms push every reading; the core drains them.
#[derive(Resource, Default, Debug)]
pub struct ImuFeed {
    samples: Vec<ImuSample>,
}

impl ImuFeed {
    pub fn push(&mut self, sample: ImuSample) {
        self.samples.push(sample);
    }
}

/// Device pose relative to the last zero, in degrees. `steer_deg` is the
/// steering-wheel turn (positive clockwise as seen by the player); `lean_deg`
/// is the tip forward/back (positive with the top edge away from the player).
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct Tilt {
    pub steer_deg: f32,
    pub lean_deg: f32,
    /// True for the one update in which the zero button re-zeroed the pose.
    pub just_zeroed: bool,
}

#[derive(Resource, Default)]
struct Tracker(MotionTracker);

/// Tilt only changes when samples arrive or the pose is zeroed, so tests (and
/// a platform without an IMU) can set [`Tilt`] directly.
fn track_tilt(
    buttons: Res<Buttons>,
    mut feed: ResMut<ImuFeed>,
    mut tracker: ResMut<Tracker>,
    mut tilt: ResMut<Tilt>,
) {
    let zero = buttons.zero.pressed();
    if feed.samples.is_empty() && !zero && !tilt.just_zeroed {
        return;
    }
    for sample in feed.samples.drain(..) {
        tracker.0.update(&sample);
    }
    if zero {
        tracker.0.zero();
    }
    *tilt = Tilt {
        steer_deg: tracker.0.steer_deg(),
        lean_deg: tracker.0.lean_deg(),
        just_zeroed: zero,
    };
}
