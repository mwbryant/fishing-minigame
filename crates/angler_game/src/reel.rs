//! The Stardew-style fight: keep the fish inside a catch zone on a vertical
//! bar. Leaning the device forward pushes the zone up, leaning back pushes it
//! down; the zone has inertia. Progress fills while the fish is inside and
//! drains while it's out.

use bevy::math::ops;
use orbit_core::Rng;

use crate::species::{Species, Temper};

/// Track geometry. Positions on the track are centers, measured down from
/// the top of the track.
pub const TRACK_TOP: f32 = 70.0;
pub const TRACK_HEIGHT: f32 = 326.0;
pub const TRACK_WIDTH: f32 = 44.0;
pub const ZONE_HEIGHT: f32 = 96.0;
pub const FISH_HALF: f32 = 8.0;

pub const START_PROGRESS: f32 = 0.35;
pub const FILL_PER_SEC: f32 = 0.30;
pub const LEAN_DEAD_ZONE_DEG: f32 = 2.0;
/// Lean (degrees past the dead zone) that gives full push.
pub const LEAN_FULL_DEG: f32 = 20.0;
pub const LEAN_ACCEL: f32 = 1100.0;
const ZONE_DAMPING: f32 = 2.5;
const ZONE_MAX_SPEED: f32 = 450.0;
const ZONE_BOUNCE: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReelOutcome {
    Landed,
    Escaped,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reel {
    pub zone_y: f32,
    pub zone_v: f32,
    pub fish_y: f32,
    pub fish_v: f32,
    pub progress: f32,
    target: f32,
    retarget_in: f32,
    temper: Temper,
    drain_per_sec: f32,
}

impl Reel {
    pub fn new(species: &Species, rng: &mut Rng) -> Self {
        let mut reel = Self {
            zone_y: TRACK_HEIGHT - ZONE_HEIGHT / 2.0,
            zone_v: 0.0,
            fish_y: TRACK_HEIGHT / 2.0,
            fish_v: 0.0,
            progress: START_PROGRESS,
            target: TRACK_HEIGHT / 2.0,
            retarget_in: 0.0,
            temper: species.temper,
            drain_per_sec: 0.18 + 0.04 * f32::from(species.tier),
        };
        reel.retarget(rng);
        reel
    }

    pub fn fish_in_zone(&self) -> bool {
        (self.fish_y - self.zone_y).abs() <= ZONE_HEIGHT / 2.0
    }

    pub fn step(&mut self, lean_deg: f32, dt: f32, rng: &mut Rng) -> Option<ReelOutcome> {
        self.move_zone(lean_deg, dt);
        self.move_fish(dt, rng);
        let rate = if self.fish_in_zone() {
            FILL_PER_SEC
        } else {
            -self.drain_per_sec
        };
        self.progress = (self.progress + rate * dt).clamp(0.0, 1.0);
        if self.progress >= 1.0 {
            return Some(ReelOutcome::Landed);
        }
        (self.progress <= 0.0).then_some(ReelOutcome::Escaped)
    }

    fn move_zone(&mut self, lean_deg: f32, dt: f32) {
        let past_dead_zone = (lean_deg.abs() - LEAN_DEAD_ZONE_DEG).max(0.0);
        let push = (past_dead_zone / LEAN_FULL_DEG).min(1.0) * lean_deg.signum();
        self.zone_v += (-push * LEAN_ACCEL - self.zone_v * ZONE_DAMPING) * dt;
        self.zone_v = self.zone_v.clamp(-ZONE_MAX_SPEED, ZONE_MAX_SPEED);
        self.zone_y += self.zone_v * dt;
        let (top, bottom) = (ZONE_HEIGHT / 2.0, TRACK_HEIGHT - ZONE_HEIGHT / 2.0);
        if self.zone_y < top {
            self.zone_y = top;
            self.zone_v = -self.zone_v * ZONE_BOUNCE;
        } else if self.zone_y > bottom {
            self.zone_y = bottom;
            self.zone_v = -self.zone_v * ZONE_BOUNCE;
        }
    }

    fn move_fish(&mut self, dt: f32, rng: &mut Rng) {
        self.retarget_in -= dt;
        if self.retarget_in <= 0.0 {
            self.retarget(rng);
        }
        if rng.next_f32() < self.temper.darts_per_sec * dt {
            let far = 0.75 + 0.2 * rng.next_f32();
            let share = if self.fish_y < TRACK_HEIGHT / 2.0 {
                far
            } else {
                1.0 - far
            };
            self.target = travel(share);
        }
        let k = self.temper.agility;
        let damping = 1.6 * ops::sqrt(k);
        self.fish_v += ((self.target - self.fish_y) * k - self.fish_v * damping) * dt;
        self.fish_v = self
            .fish_v
            .clamp(-self.temper.max_speed, self.temper.max_speed);
        self.fish_y += self.fish_v * dt;
        if !(FISH_HALF..=TRACK_HEIGHT - FISH_HALF).contains(&self.fish_y) {
            self.fish_y = self.fish_y.clamp(FISH_HALF, TRACK_HEIGHT - FISH_HALF);
            self.fish_v = 0.0;
        }
    }

    fn retarget(&mut self, rng: &mut Rng) {
        let share = (rng.next_f32() + 0.35 * self.temper.depth_bias).clamp(0.0, 1.0);
        self.target = travel(share);
        let (min, max) = self.temper.retarget_secs;
        self.retarget_in = min + (max - min) * rng.next_f32();
    }
}

/// Track position `share` of the way down the fish's range.
fn travel(share: f32) -> f32 {
    FISH_HALF + share * (TRACK_HEIGHT - 2.0 * FISH_HALF)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species::SPECIES;

    const DT: f32 = 1.0 / 60.0;

    /// Lean that produces `push` (-1..1, positive = zone up).
    fn lean_for(push: f32) -> f32 {
        let push = push.clamp(-1.0, 1.0);
        if push == 0.0 {
            return 0.0;
        }
        push.signum() * (LEAN_DEAD_ZONE_DEG + push.abs() * LEAN_FULL_DEG)
    }

    /// A player steering the zone toward the fish with a PD controller.
    fn fight(species: usize, seed: u32, gain: f32, idle: bool) -> ReelOutcome {
        let mut rng = Rng::new(seed);
        let mut reel = Reel::new(&SPECIES[species], &mut rng);
        for _ in 0..(60.0 / DT) as usize {
            let error = reel.zone_y - reel.fish_y;
            let push = (gain * error + 0.12 * gain * reel.zone_v) / LEAN_ACCEL;
            let lean = if idle { 0.0 } else { lean_for(push) };
            if let Some(outcome) = reel.step(lean, DT, &mut rng) {
                return outcome;
            }
        }
        panic!("fight never ended");
    }

    fn land_rate(species: usize, gain: f32, idle: bool) -> f32 {
        let landed = (1..=20)
            .filter(|&seed| fight(species, seed, gain, idle) == ReelOutcome::Landed)
            .count();
        landed as f32 / 20.0
    }

    #[test]
    fn leaning_forward_raises_the_zone_and_back_lowers_it() {
        let mut rng = Rng::new(1);
        let mut reel = Reel::new(&SPECIES[0], &mut rng);
        let start = reel.zone_y;
        for _ in 0..20 {
            reel.step(15.0, DT, &mut rng);
        }
        assert!(reel.zone_y < start - 20.0);
        let high = reel.zone_y;
        for _ in 0..60 {
            reel.step(-15.0, DT, &mut rng);
        }
        assert!(reel.zone_y > high);
    }

    #[test]
    fn small_wobbles_inside_the_dead_zone_do_nothing() {
        let mut rng = Rng::new(1);
        let mut reel = Reel::new(&SPECIES[0], &mut rng);
        let start = reel.zone_y;
        for _ in 0..60 {
            reel.step(LEAN_DEAD_ZONE_DEG * 0.9, DT, &mut rng);
        }
        assert_eq!(reel.zone_y, start);
    }

    #[test]
    fn the_zone_never_leaves_the_track() {
        let mut rng = Rng::new(2);
        let mut reel = Reel::new(&SPECIES[0], &mut rng);
        for lean in [40.0, -40.0] {
            for _ in 0..240 {
                reel.step(lean, DT, &mut rng);
                let half = ZONE_HEIGHT / 2.0;
                assert!((half..=TRACK_HEIGHT - half).contains(&reel.zone_y));
            }
        }
    }

    #[test]
    fn a_skilled_player_lands_every_species() {
        for (index, species) in SPECIES.iter().enumerate() {
            let rate = land_rate(index, 30.0, false);
            assert!(rate >= 0.9, "{}: {rate}", species.name);
        }
    }

    #[test]
    fn an_idle_player_almost_never_lands_anything() {
        let total: f32 = (0..SPECIES.len())
            .map(|species| land_rate(species, 0.0, true))
            .sum();
        assert!(total / (SPECIES.len() as f32) < 0.15, "{total}");
    }

    #[test]
    fn a_sluggish_player_finds_pike_harder_than_minnows() {
        let minnow = land_rate(0, 4.0, false);
        let pike = land_rate(6, 4.0, false);
        assert!(minnow > pike, "minnow {minnow} pike {pike}");
    }

    #[test]
    fn progress_fills_inside_the_zone_and_drains_outside() {
        let mut rng = Rng::new(4);
        let mut reel = Reel::new(&SPECIES[0], &mut rng);
        reel.zone_y = reel.fish_y;
        reel.fish_v = 0.0;
        reel.retarget_in = 10.0;
        reel.target = reel.fish_y;
        reel.step(0.0, DT, &mut rng);
        assert!(reel.progress > START_PROGRESS);
        reel.zone_y = TRACK_HEIGHT - ZONE_HEIGHT / 2.0;
        reel.fish_y = FISH_HALF;
        reel.target = FISH_HALF;
        let before = reel.progress;
        reel.step(0.0, DT, &mut rng);
        assert!(reel.progress < before);
    }
}
