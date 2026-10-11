//! A pretend board for the simulators (desktop and web): keys turn it, or a
//! phone's orientation sensor poses it, and it reports what its IMU would
//! read, so the game's real motion filter does the rest.

use bevy::math::{Quat, Vec3};

use crate::ImuSample;

/// How fast the turn keys turn the device.
pub const TURN_DEG_PER_SEC: f32 = 60.0;
/// How far holding a lean key tips the device.
pub const LEAN_HELD_DEG: f32 = 15.0;
/// How fast it tips there and back.
pub const LEAN_DEG_PER_SEC: f32 = 120.0;
/// Walking pace while the walk key is held, steps a second.
pub const STEPS_PER_SEC: f32 = 2.0;

pub struct VirtualDevice {
    /// Device-to-world rotation, world +Z up, in the screen frame the
    /// firmware maps the QMI8658 into (x right, y down, z into the screen).
    truth: Quat,
    lean_deg: f32,
    /// Part of a step walked (walk key held) but not yet counted.
    stride: f32,
}

impl VirtualDevice {
    pub fn upright() -> Self {
        Self {
            truth: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
            lean_deg: 0.0,
            stride: 0.0,
        }
    }

    /// Straightens the device without a gyro signal, like setting it down
    /// and picking it up again; the filter re-levels from gravity.
    pub fn straighten(&mut self) {
        *self = Self::upright();
    }

    /// The pedometer while `walking` at [`STEPS_PER_SEC`]: whole steps taken
    /// in `dt`.
    pub fn steps(&mut self, walking: bool, dt: f32) -> u32 {
        if !walking {
            return 0;
        }
        self.stride += STEPS_PER_SEC * dt;
        let whole = self.stride as u32;
        self.stride -= whole as f32;
        whole
    }

    /// Moves the device on by `dt` seconds and returns what its IMU reads.
    /// `steer` turns it clockwise (as the player sees it) in degrees per
    /// second, and it stays turned. `lean` (-1..1) is how hard the lean keys
    /// are held: the top edge tips away (or back) toward
    /// `lean * LEAN_HELD_DEG` and springs back level on release, like a wrist.
    pub fn sample(&mut self, steer: f32, lean: f32, dt: f32) -> ImuSample {
        let max_step = LEAN_DEG_PER_SEC * dt;
        let lean_step = (lean * LEAN_HELD_DEG - self.lean_deg).clamp(-max_step, max_step);
        self.lean_deg += lean_step;
        let lean_rate = if dt > 0.0 { lean_step / dt } else { 0.0 };
        let rate = Vec3::new(-lean_rate, 0.0, steer) * core::f32::consts::PI / 180.0;
        self.truth = (self.truth * Quat::from_scaled_axis(rate * dt)).normalize();
        ImuSample {
            accel: self.truth.inverse() * Vec3::Z,
            gyro: rate,
            dt,
        }
    }

    /// Puts the device at `truth` (device-to-world, as in [`Self::upright`]),
    /// as a phone's orientation sensor reports it, and returns what its IMU
    /// reads: gravity at that pose, and the turn since the last one as the
    /// gyro's rate over `dt`.
    pub fn held_at(&mut self, truth: Quat, dt: f32) -> ImuSample {
        let mut turn = self.truth.inverse() * truth;
        if turn.w < 0.0 {
            turn = -turn;
        }
        let gyro = if dt > 0.0 {
            turn.to_scaled_axis() / dt
        } else {
            Vec3::ZERO
        };
        self.truth = truth;
        ImuSample {
            accel: truth.inverse() * Vec3::Z,
            gyro,
            dt,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::MotionTracker;

    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn hold(
        device: &mut VirtualDevice,
        tracker: &mut MotionTracker,
        steer: f32,
        lean: f32,
        secs: f32,
    ) {
        for _ in 0..(secs / DT) as usize {
            tracker.update(&device.sample(steer, lean, DT));
        }
    }

    /// A device at rest, as the simulator starts: the filter's first sample
    /// only finds "down".
    fn resting() -> (VirtualDevice, MotionTracker) {
        let (mut device, mut tracker) = (VirtualDevice::upright(), MotionTracker::default());
        hold(&mut device, &mut tracker, 0.0, 0.0, 0.1);
        (device, tracker)
    }

    #[test]
    fn holding_up_tips_the_top_away_and_releasing_levels_it_again() {
        let (mut device, mut tracker) = resting();
        hold(&mut device, &mut tracker, 0.0, 1.0, 0.5);
        assert!(
            (tracker.lean_deg() - LEAN_HELD_DEG).abs() < 1.0,
            "{}",
            tracker.lean_deg()
        );
        hold(&mut device, &mut tracker, 0.0, 0.0, 0.5);
        assert!(tracker.lean_deg().abs() < 1.0, "{}", tracker.lean_deg());
    }

    #[test]
    fn steering_stays_where_it_is_left() {
        let (mut device, mut tracker) = resting();
        hold(&mut device, &mut tracker, -60.0, 0.0, 0.5);
        hold(&mut device, &mut tracker, 0.0, 0.0, 0.5);
        assert!(
            (tracker.steer_deg() + 30.0).abs() < 1.5,
            "{}",
            tracker.steer_deg()
        );
    }

    #[test]
    fn a_pose_turned_clockwise_from_outside_steers_right_by_the_turn() {
        let (mut device, mut tracker) = resting();
        let upright = Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2);
        for degree in 1..=30 {
            let turned = upright * Quat::from_rotation_z((degree as f32).to_radians());
            tracker.update(&device.held_at(turned, DT));
        }
        let turned = upright * Quat::from_rotation_z(30f32.to_radians());
        for _ in 0..30 {
            tracker.update(&device.held_at(turned, DT));
        }
        assert!(
            (tracker.steer_deg() - 30.0).abs() < 1.0,
            "{}",
            tracker.steer_deg()
        );
        assert!(tracker.lean_deg().abs() < 1.0, "{}", tracker.lean_deg());
    }
}
