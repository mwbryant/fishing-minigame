use bevy::math::{Vec2, ops};
use orbit_core::Rng;

/// Side-view lake layout, screen pixels (y down).
pub mod scene {
    use bevy::math::Vec2;

    use crate::sprites::ANGLER_HANDS;

    pub const WATER_Y: f32 = 262.0;
    /// The dock ends here; a bobber landing short of it is on the planks.
    pub const SHORE_X: f32 = 128.0;
    pub const DOCK_TOP: i32 = 240;
    pub const ANGLER_SCALE: i32 = 2;
    pub const ANGLER_TOP_LEFT: (i32, i32) = (62, 202);
    pub const HANDS: Vec2 = Vec2::new(
        (ANGLER_TOP_LEFT.0 + ANGLER_HANDS.0 * ANGLER_SCALE + 1) as f32,
        (ANGLER_TOP_LEFT.1 + ANGLER_HANDS.1 * ANGLER_SCALE + 1) as f32,
    );
    pub const ROD_LENGTH: f32 = 52.0;
    pub const GRAVITY: f32 = 420.0;
    pub const MAX_CAST_SPEED: f32 = 310.0;
    /// The farthest a cast can reach; used to normalize cast distance.
    pub const FAR_X: f32 = 430.0;
    pub const SPOT_RANGE: (f32, f32) = (180.0, 410.0);
    /// Landing farther than this from the spot gets no help from it.
    pub const SPOT_REACH: f32 = 110.0;
}

use scene::*;

pub const BASE_ELEVATION_DEG: f32 = 40.0;
pub const ELEVATION_RANGE_DEG: (f32, f32) = (10.0, 80.0);
/// Seconds for the power meter to sweep empty to full (then back down).
pub const CHARGE_SECS: f32 = 1.1;
/// Releasing below this is treated as a slip, not a cast.
pub const MIN_CAST_POWER: f32 = 0.08;
/// Releasing with the meter at or above this throws at full power, landing
/// exactly where the dotted path ends: about 0.1 s around the meter's peak.
pub const PERFECT_POWER: f32 = 0.95;
/// Charging gives up after two full sweeps. On the device the action button
/// is PWR, and the power chip switches the board off after a long hold (the
/// firmware stretches that to its 10 s maximum), so the game never rewards one.
pub const MAX_CHARGE_SECS: f32 = 4.0 * CHARGE_SECS;

/// Turning the device counterclockwise (negative steer) raises the rod, like
/// lifting the front of a cannon aimed at the lake on the right.
pub fn elevation_deg(steer_deg: f32) -> f32 {
    (BASE_ELEVATION_DEG - steer_deg).clamp(ELEVATION_RANGE_DEG.0, ELEVATION_RANGE_DEG.1)
}

pub fn rod_tip(elevation_deg: f32) -> Vec2 {
    HANDS + direction(elevation_deg) * ROD_LENGTH
}

/// Ping-pongs 0 -> 1 -> 0 while the button is held, so power is a timing skill.
pub fn charge_power(held_secs: f32) -> f32 {
    let phase = ops::rem_euclid(held_secs / CHARGE_SECS, 2.0);
    if phase <= 1.0 { phase } else { 2.0 - phase }
}

/// What a release after `held_secs` throws with: the meter's reading, or
/// exactly full power inside the window at the top.
pub fn release_power(held_secs: f32) -> f32 {
    let power = charge_power(held_secs);
    if power >= PERFECT_POWER { 1.0 } else { power }
}

fn direction(elevation_deg: f32) -> Vec2 {
    let (sin, cos) = ops::sin_cos(elevation_deg.to_radians());
    Vec2::new(cos, -sin)
}

/// The bobber in the air.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    pub pos: Vec2,
    pub vel: Vec2,
}

impl Flight {
    pub fn launch(elevation_deg: f32, power: f32) -> Self {
        Self {
            pos: rod_tip(elevation_deg),
            vel: direction(elevation_deg) * MAX_CAST_SPEED * power,
        }
    }

    /// Advances one step along the exact arc (so the dotted preview never
    /// lies); returns the landing x once the bobber reaches water.
    pub fn step(&mut self, dt: f32) -> Option<f32> {
        let before = self.pos;
        self.pos = self.at(dt);
        self.vel.y += GRAVITY * dt;
        if self.pos.y < WATER_Y {
            return None;
        }
        let t = (WATER_Y - before.y) / (self.pos.y - before.y);
        Some(before.x + (self.pos.x - before.x) * t)
    }

    /// Closed-form position `t` seconds after launch.
    pub fn at(&self, t: f32) -> Vec2 {
        self.pos + self.vel * t + Vec2::new(0.0, 0.5 * GRAVITY * t * t)
    }

    pub fn landing_x(&self) -> f32 {
        let (vy, drop) = (self.vel.y, WATER_Y - self.pos.y);
        let t = (-vy + ops::sqrt(vy * vy + 2.0 * GRAVITY * drop)) / GRAVITY;
        self.at(t).x
    }
}

/// 1 when the bobber lands on the spot, falling to 0 at `SPOT_REACH` away.
pub fn accuracy(land_x: f32, spot_x: f32) -> f32 {
    (1.0 - (land_x - spot_x).abs() / SPOT_REACH).clamp(0.0, 1.0)
}

/// 0 at the end of the dock, 1 at the longest possible cast.
pub fn distance(land_x: f32) -> f32 {
    ((land_x - SHORE_X) / (FAR_X - SHORE_X)).clamp(0.0, 1.0)
}

/// Seconds until something bites, or `None` if nothing will. Casts on the
/// spot get bites fast; casts that miss it entirely rarely get any.
pub fn bite_delay(rng: &mut Rng, accuracy: f32) -> Option<f32> {
    if accuracy <= 0.0 {
        return (rng.next_f32() < 0.35).then(|| 4.0 + 4.0 * rng.next_f32());
    }
    Some(1.2 + 5.0 * (1.0 - accuracy) + 1.5 * rng.next_f32())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lands(elevation: f32, power: f32) -> f32 {
        Flight::launch(elevation, power).landing_x()
    }

    #[test]
    fn steering_counterclockwise_raises_the_rod_within_limits() {
        assert_eq!(elevation_deg(0.0), BASE_ELEVATION_DEG);
        assert_eq!(elevation_deg(-30.0), 70.0);
        assert_eq!(elevation_deg(25.0), 15.0);
        assert_eq!(elevation_deg(-90.0), 80.0);
        assert_eq!(elevation_deg(90.0), 10.0);
    }

    #[test]
    fn power_sweeps_up_then_back_down_while_held() {
        assert_eq!(charge_power(0.0), 0.0);
        assert!((charge_power(CHARGE_SECS) - 1.0).abs() < 1e-5);
        assert!((charge_power(CHARGE_SECS * 1.5) - 0.5).abs() < 1e-5);
        assert!(charge_power(CHARGE_SECS * 2.0) < 1e-5);
    }

    #[test]
    fn releasing_near_the_top_of_the_meter_counts_as_full_power() {
        assert_eq!(release_power(CHARGE_SECS), 1.0);
        assert_eq!(release_power(CHARGE_SECS * 0.97), 1.0);
        assert_eq!(release_power(CHARGE_SECS * 1.03), 1.0);
        assert!((release_power(CHARGE_SECS * 0.5) - 0.5).abs() < 1e-5);
        assert!(release_power(CHARGE_SECS * (PERFECT_POWER - 0.01)) < 1.0);
    }

    #[test]
    fn a_full_power_cast_reaches_far_water_but_stays_on_screen() {
        let best = (20..=60)
            .map(|deg| lands(deg as f32, 1.0))
            .fold(0.0, f32::max);
        assert!((400.0..=FAR_X + 5.0).contains(&best), "best cast {best}");
    }

    #[test]
    fn a_feeble_steep_cast_lands_on_the_dock() {
        assert!(lands(80.0, 0.1) < SHORE_X);
    }

    #[test]
    fn a_feeble_flat_cast_plops_just_past_the_dock() {
        let x = lands(40.0, 0.15);
        assert!((SHORE_X..SHORE_X + 40.0).contains(&x), "{x}");
    }

    #[test]
    fn stepping_the_flight_lands_where_the_closed_form_says() {
        let mut flight = Flight::launch(55.0, 0.8);
        let expected = flight.landing_x();
        let landed = (0..600).find_map(|_| flight.step(1.0 / 60.0)).unwrap();
        assert!((landed - expected).abs() < 1.0, "{landed} vs {expected}");
    }

    #[test]
    fn every_full_power_arc_stays_inside_the_round_glass() {
        let center = Vec2::splat(233.0);
        for deg in (10..=80).step_by(5) {
            let flight = Flight::launch(deg as f32, 1.0);
            let mut t = 0.0;
            while flight.at(t).y <= WATER_Y {
                let r = flight.at(t).distance(center);
                assert!(r < 226.0, "{deg} deg arc leaves the glass at t={t}: r={r}");
                t += 0.02;
            }
        }
    }

    #[test]
    fn closer_casts_get_quicker_bites() {
        let mut rng = Rng::new(5);
        let on_spot: f32 = (0..200).map(|_| bite_delay(&mut rng, 1.0).unwrap()).sum();
        let near: f32 = (0..200).map(|_| bite_delay(&mut rng, 0.3).unwrap()).sum();
        assert!(on_spot < near);
    }

    #[test]
    fn missing_the_spot_entirely_often_gets_nothing() {
        let mut rng = Rng::new(9);
        let nothing = (0..1000)
            .filter(|_| bite_delay(&mut rng, 0.0).is_none())
            .count();
        assert!((550..=750).contains(&nothing), "{nothing}");
    }

    #[test]
    fn accuracy_and_distance_are_normalized() {
        assert_eq!(accuracy(300.0, 300.0), 1.0);
        assert_eq!(accuracy(300.0, 300.0 + SPOT_REACH), 0.0);
        assert_eq!(distance(SHORE_X), 0.0);
        assert_eq!(distance(FAR_X + 50.0), 1.0);
    }
}
