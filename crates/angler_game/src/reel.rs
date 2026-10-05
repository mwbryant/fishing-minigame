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

pub const START_PROGRESS: f32 = 0.3;
pub const FILL_PER_SEC: f32 = 0.30;
/// The meter drains at this plus `DRAIN_PER_TIER` per species tier while the
/// fish is outside the zone.
pub const DRAIN_PER_SEC: f32 = 0.15;
pub const DRAIN_PER_TIER: f32 = 0.05;
/// Seconds a freshly hooked fish holds still inside the zone, so the player
/// has time to get a feel for the lean before it starts running.
pub const HOOK_SETTLE_SECS: f32 = 1.0;
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
    settle_secs: f32,
    temper: Temper,
    fill_per_sec: f32,
    drain_per_sec: f32,
}

impl Reel {
    /// Like Stardew, the fish and the zone start together near the bottom.
    pub fn new(species: &Species, rng: &mut Rng) -> Self {
        let start = TRACK_HEIGHT - ZONE_HEIGHT / 2.0;
        let (min, max) = species.temper.retarget_secs;
        Self {
            zone_y: start,
            zone_v: 0.0,
            fish_y: start,
            fish_v: 0.0,
            progress: START_PROGRESS,
            target: start,
            retarget_in: min + (max - min) * rng.next_f32(),
            settle_secs: HOOK_SETTLE_SECS,
            temper: species.temper,
            fill_per_sec: FILL_PER_SEC,
            drain_per_sec: DRAIN_PER_SEC + DRAIN_PER_TIER * f32::from(species.tier),
        }
    }

    pub fn fish_in_zone(&self) -> bool {
        (self.fish_y - self.zone_y).abs() <= ZONE_HEIGHT / 2.0
    }

    pub fn step(&mut self, lean_deg: f32, dt: f32, rng: &mut Rng) -> Option<ReelOutcome> {
        self.move_zone(lean_deg, dt);
        self.move_fish(dt, rng);
        let rate = if self.fish_in_zone() {
            self.fill_per_sec
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
        if self.settle_secs > 0.0 {
            self.settle_secs -= dt;
            if self.settle_secs <= 0.0 {
                self.run_for_it(rng);
            }
            return;
        }
        self.retarget_in -= dt;
        if self.retarget_in <= 0.0 {
            self.retarget(rng);
        }
        if rng.next_f32() < self.temper.darts_per_sec * dt {
            self.run_for_it(rng);
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

    /// Dashes for the far end of the bar: the run right after the hook
    /// settles, and every dart after that.
    fn run_for_it(&mut self, rng: &mut Rng) {
        let far = 0.75 + 0.2 * rng.next_f32();
        let share = if self.fish_y < TRACK_HEIGHT / 2.0 {
            far
        } else {
            1.0 - far
        };
        self.target = travel(share);
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

    /// A player reacting `lag_secs` late with a soft hand: the realistic case
    /// of tilting a device by feel.
    fn hesitant_fight(species: usize, seed: u32, lag_secs: f32) -> ReelOutcome {
        let mut rng = Rng::new(seed);
        let mut reel = Reel::new(&SPECIES[species], &mut rng);
        let lag = (lag_secs / DT) as usize;
        let mut seen = alloc::collections::VecDeque::from(alloc::vec![
            (reel.zone_y, reel.zone_v, reel.fish_y);
            lag + 1
        ]);
        for _ in 0..(60.0 / DT) as usize {
            seen.push_back((reel.zone_y, reel.zone_v, reel.fish_y));
            let (zone_y, zone_v, fish_y) = seen.pop_front().unwrap();
            let push = (12.0 * (zone_y - fish_y) + 1.5 * zone_v) / LEAN_ACCEL;
            if let Some(outcome) = reel.step(lean_for(push), DT, &mut rng) {
                return outcome;
            }
        }
        panic!("fight never ended");
    }

    #[test]
    fn a_hooked_fish_starts_inside_the_catch_zone() {
        for species in &SPECIES {
            for seed in 1..=20 {
                let reel = Reel::new(species, &mut Rng::new(seed));
                assert!(reel.fish_in_zone(), "{} seed {seed}", species.name);
            }
        }
    }

    #[test]
    fn no_fish_gets_away_in_the_first_three_seconds_even_untouched() {
        for (index, species) in SPECIES.iter().enumerate() {
            for seed in 1..=20 {
                let mut rng = Rng::new(seed);
                let mut reel = Reel::new(&SPECIES[index], &mut rng);
                for step in 0..(3.0 / DT) as usize {
                    let outcome = reel.step(0.0, DT, &mut rng);
                    assert_ne!(
                        outcome,
                        Some(ReelOutcome::Escaped),
                        "{} seed {seed} escaped after {:.2}s",
                        species.name,
                        step as f32 * DT
                    );
                }
            }
        }
    }

    #[test]
    fn a_hesitant_player_still_lands_most_common_fish() {
        for (index, species) in SPECIES.iter().enumerate().filter(|(_, s)| s.tier <= 1) {
            let landed = (1..=20)
                .filter(|&seed| hesitant_fight(index, seed, 0.25) == ReelOutcome::Landed)
                .count();
            assert!(landed >= 15, "{}: {landed}/20", species.name);
        }
    }

    #[test]
    fn a_hesitant_player_lands_trophies_sometimes_but_not_always() {
        let landed = (1..=20)
            .filter(|&seed| hesitant_fight(6, seed, 0.25) == ReelOutcome::Landed)
            .count();
        assert!((2..=17).contains(&landed), "pike: {landed}/20");
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
