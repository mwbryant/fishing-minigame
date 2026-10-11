//! Steps from the accelerometer, counted the easy way: every time the
//! acceleration's size swings below its running average and back up through
//! it, by a hair more than sensor noise, is a step. Turning the board changes
//! nothing (gravity's size stays 1 g), but any bounce counts, so it
//! overcounts on purpose: a walk should never come up empty.

use bevy::math::Vec3;

/// How far past the average (each way) a swing must go, in g. The QMI8658
/// at rest wobbles a few mg and a hand holding it about 20; a walk in the
/// hand swings 100 mg or more. 15 counted a little too much on the board.
pub const STEP_SWING_G: f32 = 0.025;
/// Steps come no closer than this, in seconds (3.3 a second at most, above a
/// jog), so a step's two bumps (the heel landing, the push-off) count once.
pub const STEP_GAP_SECS: f32 = 0.3;
/// The running average forgets with this time constant, in seconds.
const AVERAGE_SECS: f32 = 1.0;

#[derive(Clone, Debug, Default)]
pub struct StepCounter {
    average: Option<f32>,
    /// The swing has gone under the average since the last step.
    dipped: bool,
    since_step: f32,
}

impl StepCounter {
    /// Takes one accelerometer reading (in g, any frame) `dt` seconds after
    /// the last and returns the steps it completes (0 or 1).
    pub fn push(&mut self, accel: Vec3, dt: f32) -> u32 {
        let size = accel.length();
        let average = self.average.get_or_insert(size);
        *average += (size - *average) * (dt / AVERAGE_SECS).min(1.0);
        let swing = size - *average;
        self.since_step += dt;
        if swing < -STEP_SWING_G {
            self.dipped = true;
            return 0;
        }
        if !self.dipped || swing <= STEP_SWING_G || self.since_step < STEP_GAP_SECS {
            return 0;
        }
        self.dipped = false;
        self.since_step = 0.0;
        1
    }
}

#[cfg(test)]
mod tests {
    use core::f32::consts::TAU;

    use bevy::math::{Quat, ops};

    use super::*;

    /// Readings at `rate` Hz for `secs`: gravity (turned by `turn`) times a
    /// bounce of `swing` g at `hz`, plus a few mg of noise.
    fn count(rate: f32, secs: f32, hz: f32, swing: f32, turn: impl Fn(f32) -> Quat) -> u32 {
        count_bounce(rate, secs, turn, |t| swing * ops::sin(TAU * hz * t))
    }

    /// Readings at `rate` Hz for `secs`: gravity (turned by `turn`) times
    /// `1 + bounce(t)`, plus a few mg of noise.
    fn count_bounce(
        rate: f32,
        secs: f32,
        turn: impl Fn(f32) -> Quat,
        bounce: impl Fn(f32) -> f32,
    ) -> u32 {
        let mut counter = StepCounter::default();
        let mut seed = 12345u32;
        let dt = 1.0 / rate;
        (0..(secs * rate) as u32)
            .map(|i| {
                let t = i as f32 * dt;
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = ((seed >> 8) as f32 / (1 << 24) as f32 - 0.5) * 0.006;
                let size = 1.0 + bounce(t) + noise;
                counter.push(turn(t) * Vec3::new(0.0, -size, 0.0), dt)
            })
            .sum()
    }

    fn still(_: f32) -> Quat {
        Quat::IDENTITY
    }

    #[test]
    fn a_board_at_rest_counts_nothing() {
        assert_eq!(count(60.0, 30.0, 2.0, 0.0, still), 0);
    }

    #[test]
    fn turning_the_board_without_walking_counts_nothing() {
        let wobble = |t: f32| Quat::from_rotation_z(ops::sin(TAU * 0.7 * t) * 1.2);
        assert_eq!(count(60.0, 20.0, 2.0, 0.0, wobble), 0);
    }

    #[test]
    fn a_walk_counts_at_least_a_step_for_every_step() {
        let steps = count(60.0, 10.0, 2.0, 0.15, still);
        assert!(steps >= 19, "{steps} for 20 steps");
    }

    #[test]
    fn a_gentle_stroll_with_the_board_held_still_counts() {
        let steps = count(60.0, 10.0, 1.6, 0.045, still);
        assert!(steps >= 15, "{steps} for 16 steps");
    }

    #[test]
    fn a_hand_wobbling_20_mg_counts_nothing() {
        assert_eq!(count(60.0, 20.0, 1.5, 0.02, still), 0);
    }

    #[test]
    fn a_step_that_lands_and_pushes_off_counts_once() {
        // Each step bounces twice: the heel lands, then the foot pushes off.
        let walk = |t: f32| 0.15 * ops::sin(TAU * 2.0 * t) + 0.12 * ops::sin(TAU * 4.0 * t + 1.0);
        let steps = count_bounce(60.0, 10.0, still, walk);
        assert!((19..=21).contains(&steps), "{steps} for 20 steps");
    }

    #[test]
    fn a_walk_read_ten_times_a_second_while_dozing_still_counts() {
        let steps = count(10.0, 10.0, 2.2, 0.15, still);
        assert!(steps >= 20, "{steps} for 22 steps");
    }

    #[test]
    fn a_shake_counts_no_more_than_one_step_a_gap() {
        let steps = count(60.0, 10.0, 9.0, 0.5, still);
        let most = (10.0 / STEP_GAP_SECS) as u32;
        assert!(
            (most - 6..=most).contains(&steps),
            "{steps}, at most {most}"
        );
    }
}
