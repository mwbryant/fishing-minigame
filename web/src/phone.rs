//! A phone's orientation sensor as the board's pose, so turning the phone
//! turns the board.

use core::f32::consts::PI;

use bevy::math::Quat;

/// The board's device-to-world pose (as `VirtualDevice::held_at` takes it)
/// from a `deviceorientation` event's `alpha`, `beta` and `gamma` and
/// `screen.orientation.angle` (how far the page is turned counterclockwise
/// from the phone's natural portrait), all in degrees.
pub fn pose(alpha: f32, beta: f32, gamma: f32, screen_angle: f32) -> Quat {
    let phone = Quat::from_rotation_z(alpha.to_radians())
        * Quat::from_rotation_x(beta.to_radians())
        * Quat::from_rotation_y(gamma.to_radians());
    let page = phone * Quat::from_rotation_z(-screen_angle.to_radians());
    page * Quat::from_rotation_x(PI)
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec3;
    use orbit_motion::{MotionTracker, VirtualDevice};

    use super::*;

    const DT: f32 = 1.0 / 60.0;

    /// Feeds `poses` (alpha, beta, gamma) to a fresh tracker, then holds the
    /// last one for half a second.
    fn track(poses: &[(f32, f32, f32)]) -> MotionTracker {
        let (mut device, mut tracker) = (VirtualDevice::upright(), MotionTracker::default());
        let last = poses[poses.len() - 1];
        for &(alpha, beta, gamma) in poses.iter().chain([last].iter().cycle().take(30)) {
            tracker.update(&device.held_at(pose(alpha, beta, gamma, 0.0), DT));
        }
        tracker
    }

    fn up(pose: Quat) -> Vec3 {
        pose.inverse() * Vec3::Z
    }

    #[test]
    fn a_phone_held_upright_is_the_board_held_upright() {
        assert!(up(pose(0.0, 90.0, 0.0, 0.0)).distance(Vec3::NEG_Y) < 1e-5);
        assert!(up(pose(140.0, 90.0, 0.0, 0.0)).distance(Vec3::NEG_Y) < 1e-5);
    }

    #[test]
    fn turning_an_upright_phone_clockwise_like_a_wheel_steers_right() {
        // An upright phone turned clockwise by t degrees reads as alpha -90,
        // beta 90 - t, gamma 90 (the spec's Z-X'-Y'' angles of that pose).
        let poses: Vec<_> = (0..=30).map(|t| (-90.0, 90.0 - t as f32, 90.0)).collect();
        let tracker = track(&poses);
        assert!(
            (tracker.steer_deg() - 30.0).abs() < 1.0,
            "{}",
            tracker.steer_deg()
        );
        assert!(tracker.lean_deg().abs() < 1.0, "{}", tracker.lean_deg());
    }

    #[test]
    fn tipping_the_top_of_the_phone_away_leans_forward() {
        let poses: Vec<_> = (0..=15).map(|t| (0.0, 90.0 - t as f32, 0.0)).collect();
        let tracker = track(&poses);
        assert!(
            (tracker.lean_deg() - 15.0).abs() < 1.0,
            "{}",
            tracker.lean_deg()
        );
        assert!(tracker.steer_deg().abs() < 1.0, "{}", tracker.steer_deg());
    }

    #[test]
    fn a_phone_on_its_side_with_the_page_turned_to_match_is_upright() {
        // Upright, then a quarter turn counterclockwise: alpha -90, beta 180,
        // gamma 90. The page turns with it (angle 90).
        assert!(up(pose(-90.0, 180.0, 90.0, 90.0)).distance(Vec3::NEG_Y) < 1e-5);
    }
}
