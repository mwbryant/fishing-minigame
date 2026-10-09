use bevy::prelude::*;

use crate::{Buttons, SoundQueue, Tilt, Touch};

/// Holding the zero button this long starts the goodbye before sleep.
pub const SLEEP_HOLD_SECS: f32 = 3.0;
/// Holds shorter than this only re-level. Longer ones show the sleep hint.
pub const SLEEP_HINT_SECS: f32 = 0.5;
/// How long the goodbye screen stays up before the platform sleeps.
pub const GOOD_NIGHT_SECS: f32 = 2.5;
/// This long without a touch, a button or a turn of the device is idle.
pub const IDLE_SECS: f32 = 30.0;
/// Turning or leaning the device more than this counts as input.
pub const IDLE_TILT_DEG: f32 = 3.0;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<Power>()
        .add_systems(PreUpdate, track_power);
}

/// The battery as the platform last read it from its power chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    /// Charge 0..=100 from the fuel gauge, or `None` with no cell connected.
    pub percent: Option<u8>,
    /// USB (or another external supply) is powering the board.
    pub external_power: bool,
    pub charging: bool,
}

/// Battery, idleness and hold-to-sleep, shared by every game and the platform.
/// The platform reports the battery and may dim the panel and slow its loop
/// while idle. The core tracks idleness and the zero-button hold. Games draw
/// the hint and goodbye with `orbit_ui::power_overlay` and ignore input while
/// [`Power::saying_good_night`]. Then the platform sleeps until a button wakes
/// it, which restarts the game.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct Power {
    /// `None` until the platform reports a reading, and on platforms without a gauge.
    pub battery: Option<Battery>,
    /// True after [`IDLE_SECS`] without input.
    pub idle: bool,
    zero_held_secs: f32,
    good_night_secs: Option<f32>,
    sleep_requested: bool,
    idle_secs: f32,
    idle_pose: Vec2,
}

impl Power {
    /// 0..=1 while the zero button has been held long enough to show the
    /// sleep hint and the goodbye hasn't started.
    pub fn sleep_progress(&self) -> Option<f32> {
        (self.good_night_secs.is_none() && self.zero_held_secs >= SLEEP_HINT_SECS).then(|| {
            ((self.zero_held_secs - SLEEP_HINT_SECS) / (SLEEP_HOLD_SECS - SLEEP_HINT_SECS)).min(1.0)
        })
    }

    pub fn saying_good_night(&self) -> bool {
        self.good_night_secs.is_some()
    }

    /// Set once the goodbye has been on screen long enough.
    pub fn sleep_requested(&self) -> bool {
        self.sleep_requested
    }
}

fn track_power(
    time: Res<Time>,
    buttons: Res<Buttons>,
    touch: Res<Touch>,
    tilt: Res<Tilt>,
    mut power: ResMut<Power>,
    sound: Option<ResMut<SoundQueue>>,
) {
    let dt = time.delta_secs();
    if let Some(shown) = power.good_night_secs.as_mut() {
        *shown += dt;
        if *shown >= GOOD_NIGHT_SECS {
            power.sleep_requested = true;
        }
        return;
    }
    power.zero_held_secs = if buttons.zero.held() {
        power.zero_held_secs + dt
    } else {
        0.0
    };
    if power.zero_held_secs >= SLEEP_HOLD_SECS {
        power.good_night_secs = Some(0.0);
        if let Some(mut sound) = sound {
            sound.stop();
        }
        return;
    }
    let pose = Vec2::new(tilt.steer_deg, tilt.lean_deg);
    if touch.position().is_some()
        || buttons.action.held()
        || buttons.zero.held()
        || pose.distance(power.idle_pose) > IDLE_TILT_DEG
    {
        power.idle_secs = 0.0;
        power.idle_pose = pose;
    } else {
        power.idle_secs += dt;
    }
    power.idle = power.idle_secs >= IDLE_SECS;
}
