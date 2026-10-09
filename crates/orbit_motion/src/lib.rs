//! Device orientation from a 6-axis IMU, relative to a user-chosen zero pose.
//!
//! Samples are in the screen's frame: x to the screen's right, y to its bottom,
//! z into the screen (away from the viewer). Firmware maps the QMI8658's axes
//! into this frame, and the desktop simulator synthesizes samples. A Mahony
//! filter integrates the gyro and pulls tilt toward the accelerometer's "up".
//! Lean never drifts, and steer only drifts while the screen lies flat.

#![no_std]

use bevy::math::{Quat, Vec3, ops};

/// One IMU reading. `accel` is specific force in g (reads +1 g "up" at rest),
/// `gyro` is body angular rate in rad/s, `dt` the seconds since the last one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImuSample {
    pub accel: Vec3,
    pub gyro: Vec3,
    pub dt: f32,
}

/// Mahony proportional gain: how hard tilt is pulled toward gravity.
const KP: f32 = 2.0;
/// Mahony integral gain, which learns the gyro's bias.
const KI: f32 = 0.05;
/// Accelerometer magnitudes (g) outside this range are mostly hand motion, not
/// gravity, so they don't correct tilt.
const ACCEL_TRUST: core::ops::RangeInclusive<f32> = 0.8..=1.2;

/// Orientation from a stream of [`ImuSample`]s, read as steer and lean from the
/// zero pose.
#[derive(Clone, Debug, Default)]
pub struct MotionTracker {
    /// Device-to-world rotation, world +Z up. `None` until the first sample.
    orientation: Option<Quat>,
    reference: Quat,
    bias: Vec3,
}

impl MotionTracker {
    pub fn update(&mut self, sample: &ImuSample) {
        let Some(q) = self.orientation else {
            if let Some(up) = sample.accel.try_normalize() {
                let q = Quat::from_rotation_arc(up, Vec3::Z);
                self.orientation = Some(q);
                self.reference = q;
            }
            return;
        };
        let mut rate = sample.gyro;
        let g = sample.accel.length();
        if ACCEL_TRUST.contains(&g) {
            let estimated_up = q.inverse() * Vec3::Z;
            let error = (sample.accel / g).cross(estimated_up);
            self.bias += KI * error * sample.dt;
            rate += KP * error + self.bias;
        }
        self.orientation = Some((q * Quat::from_scaled_axis(rate * sample.dt)).normalize());
    }

    /// Makes the current pose the new zero for `steer_deg` and `lean_deg`.
    pub fn zero(&mut self) {
        if let Some(q) = self.orientation {
            self.reference = q;
        }
    }

    /// Rotation about the screen normal since zeroing, degrees, positive
    /// clockwise as the viewer sees it.
    pub fn steer_deg(&self) -> f32 {
        twist_deg(self.relative(), Vec3::Z)
    }

    /// Tip about the screen's horizontal axis since zeroing, degrees, positive
    /// when the top edge moves away from the viewer.
    pub fn lean_deg(&self) -> f32 {
        -twist_deg(self.relative(), Vec3::X)
    }

    fn relative(&self) -> Quat {
        self.orientation
            .map_or(Quat::IDENTITY, |q| self.reference.inverse() * q)
    }
}

/// Angle of the swing-twist decomposition's twist about `axis`.
fn twist_deg(q: Quat, axis: Vec3) -> f32 {
    let q = if q.w < 0.0 { -q } else { q };
    let along = Vec3::new(q.x, q.y, q.z).dot(axis);
    (2.0 * ops::atan2(along, q.w)).to_degrees()
}

#[cfg(test)]
mod tests {
    use core::f32::consts::FRAC_PI_2;

    use super::*;

    const DT: f32 = 0.01;

    /// A simulated device: integrates body rates exactly and reports the
    /// matching gravity reading, like a perfect IMU would.
    struct Device {
        truth: Quat,
        tracker: MotionTracker,
    }

    impl Device {
        /// Held upright facing the viewer: the screen's top points up.
        fn upright() -> Self {
            Self::posed(Quat::from_rotation_x(-FRAC_PI_2))
        }

        /// Lying on a table, screen facing the ceiling.
        fn flat() -> Self {
            Self::posed(Quat::from_rotation_x(core::f32::consts::PI))
        }

        fn posed(truth: Quat) -> Self {
            let mut device = Self {
                truth,
                tracker: MotionTracker::default(),
            };
            device.hold(0.1);
            device
        }

        fn sample(&self, gyro: Vec3) -> ImuSample {
            ImuSample {
                accel: self.truth.inverse() * Vec3::Z,
                gyro,
                dt: DT,
            }
        }

        fn spin(&mut self, body_axis: Vec3, degrees: f32, secs: f32) {
            let steps = (secs / DT) as usize;
            let rate = body_axis * degrees.to_radians() / secs;
            for _ in 0..steps {
                self.truth = (self.truth * Quat::from_scaled_axis(rate * DT)).normalize();
                let sample = self.sample(rate);
                self.tracker.update(&sample);
            }
        }

        fn hold(&mut self, secs: f32) {
            self.spin(Vec3::Z, 0.0, secs);
        }
    }

    fn close(actual: f32, expected: f32, tolerance: f32) -> bool {
        (actual - expected).abs() <= tolerance
    }

    #[test]
    fn a_device_held_still_reads_zero() {
        let mut device = Device::upright();
        device.hold(2.0);
        assert!(close(device.tracker.steer_deg(), 0.0, 0.5));
        assert!(close(device.tracker.lean_deg(), 0.0, 0.5));
    }

    #[test]
    fn turning_like_a_steering_wheel_is_steer_not_lean() {
        let mut device = Device::upright();
        device.spin(Vec3::Z, 30.0, 0.5);
        device.hold(0.5);
        let (steer, lean) = (device.tracker.steer_deg(), device.tracker.lean_deg());
        assert!(close(steer, 30.0, 1.5), "steer {steer}");
        assert!(close(lean, 0.0, 1.5), "lean {lean}");
    }

    #[test]
    fn counterclockwise_turns_read_negative() {
        let mut device = Device::upright();
        device.spin(Vec3::Z, -25.0, 0.5);
        assert!(close(device.tracker.steer_deg(), -25.0, 1.5));
    }

    #[test]
    fn tipping_the_top_away_is_positive_lean_not_steer() {
        let mut device = Device::upright();
        device.spin(Vec3::X, -20.0, 0.4);
        device.hold(0.5);
        let (steer, lean) = (device.tracker.steer_deg(), device.tracker.lean_deg());
        assert!(close(lean, 20.0, 1.5), "lean {lean}");
        assert!(close(steer, 0.0, 1.5), "steer {steer}");
    }

    #[test]
    fn zeroing_makes_the_current_pose_the_new_origin() {
        let mut device = Device::upright();
        device.spin(Vec3::Z, 40.0, 0.5);
        device.spin(Vec3::X, -15.0, 0.5);
        device.tracker.zero();
        assert!(close(device.tracker.steer_deg(), 0.0, 0.01));
        assert!(close(device.tracker.lean_deg(), 0.0, 0.01));
        device.spin(Vec3::Z, 10.0, 0.3);
        assert!(close(device.tracker.steer_deg(), 10.0, 1.5));
    }

    #[test]
    fn gravity_cancels_gyro_bias_on_tilt() {
        let mut device = Device::upright();
        let biased = Vec3::new(0.02, 0.0, 0.03);
        for _ in 0..(30.0 / DT) as usize {
            let sample = device.sample(biased);
            device.tracker.update(&sample);
        }
        let (steer, lean) = (device.tracker.steer_deg(), device.tracker.lean_deg());
        assert!(close(lean, 0.0, 1.0), "lean drifted to {lean}");
        assert!(close(steer, 0.0, 1.0), "steer drifted to {steer}");
    }

    #[test]
    fn hand_shake_spikes_are_not_trusted_as_gravity() {
        let mut device = Device::upright();
        for _ in 0..20 {
            let mut sample = device.sample(Vec3::ZERO);
            sample.accel += Vec3::new(2.5, 0.0, 0.0);
            device.tracker.update(&sample);
        }
        assert!(close(device.tracker.steer_deg(), 0.0, 0.5));
    }

    #[test]
    fn steering_still_tracks_with_the_screen_facing_up() {
        let mut device = Device::flat();
        device.spin(Vec3::Z, 35.0, 0.5);
        assert!(close(device.tracker.steer_deg(), 35.0, 1.5));
    }

    #[test]
    fn nothing_reads_until_a_sample_arrives() {
        let tracker = MotionTracker::default();
        assert_eq!((tracker.steer_deg(), tracker.lean_deg()), (0.0, 0.0));
    }
}
