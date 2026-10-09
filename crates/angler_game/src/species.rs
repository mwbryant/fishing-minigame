//! The eight species, how each fights, and rolling what bites.

use bevy::math::ops;
use orbit_core::Rng;
use orbit_gfx::Sprite;

use crate::sprites;

/// How a hooked fish moves on the reel bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Temper {
    /// Spring pull toward its target, per second squared per pixel.
    pub agility: f32,
    /// Px per second.
    pub max_speed: f32,
    /// Seconds between picking new targets.
    pub retarget_secs: (f32, f32),
    /// Chance per second of a sudden dash to a far target.
    pub darts_per_sec: f32,
    /// Where targets cluster: negative toward the top, positive the bottom.
    pub depth_bias: f32,
}

pub struct Species {
    pub name: &'static str,
    /// 0 = common panfish .. 3 = trophy.
    pub tier: u8,
    /// Shortest and longest.
    pub length_cm: (f32, f32),
    /// Weight in grams = `girth` * length_cm^3 (the fisheries length-weight rule).
    pub girth: f32,
    /// Relative odds within its tier.
    pub odds: f32,
    pub temper: Temper,
    pub sprite: &'static Sprite,
}

pub const SPECIES: [Species; 8] = [
    Species {
        name: "MINNOW",
        tier: 0,
        length_cm: (5.0, 11.0),
        girth: 0.010,
        odds: 1.0,
        temper: Temper {
            agility: 6.0,
            max_speed: 120.0,
            retarget_secs: (0.8, 1.6),
            darts_per_sec: 0.0,
            depth_bias: 0.0,
        },
        sprite: &sprites::MINNOW,
    },
    Species {
        name: "BLUEGILL",
        tier: 0,
        length_cm: (10.0, 24.0),
        girth: 0.018,
        odds: 1.0,
        temper: Temper {
            agility: 7.0,
            max_speed: 150.0,
            retarget_secs: (0.7, 1.4),
            darts_per_sec: 0.1,
            depth_bias: 0.0,
        },
        sprite: &sprites::BLUEGILL,
    },
    Species {
        name: "PERCH",
        tier: 1,
        length_cm: (15.0, 35.0),
        girth: 0.012,
        odds: 1.0,
        temper: Temper {
            agility: 9.0,
            max_speed: 200.0,
            retarget_secs: (0.6, 1.2),
            darts_per_sec: 0.3,
            depth_bias: 0.0,
        },
        sprite: &sprites::PERCH,
    },
    Species {
        name: "TROUT",
        tier: 1,
        length_cm: (25.0, 60.0),
        girth: 0.011,
        odds: 0.8,
        temper: Temper {
            agility: 11.0,
            max_speed: 260.0,
            retarget_secs: (0.5, 1.1),
            darts_per_sec: 0.5,
            depth_bias: -0.2,
        },
        sprite: &sprites::TROUT,
    },
    Species {
        name: "BASS",
        tier: 2,
        length_cm: (25.0, 60.0),
        girth: 0.014,
        odds: 1.0,
        temper: Temper {
            agility: 12.0,
            max_speed: 280.0,
            retarget_secs: (0.45, 1.0),
            darts_per_sec: 0.4,
            depth_bias: 0.0,
        },
        sprite: &sprites::BASS,
    },
    Species {
        name: "CATFISH",
        tier: 2,
        length_cm: (40.0, 100.0),
        girth: 0.009,
        odds: 0.8,
        temper: Temper {
            agility: 8.0,
            max_speed: 220.0,
            retarget_secs: (0.9, 1.8),
            darts_per_sec: 0.15,
            depth_bias: 0.5,
        },
        sprite: &sprites::CATFISH,
    },
    Species {
        name: "PIKE",
        tier: 3,
        length_cm: (50.0, 110.0),
        girth: 0.006,
        odds: 1.0,
        temper: Temper {
            agility: 16.0,
            max_speed: 340.0,
            retarget_secs: (0.35, 0.8),
            darts_per_sec: 0.9,
            depth_bias: 0.0,
        },
        sprite: &sprites::PIKE,
    },
    Species {
        name: "GOLDEN KOI",
        tier: 3,
        length_cm: (30.0, 70.0),
        girth: 0.020,
        odds: 0.25,
        temper: Temper {
            agility: 18.0,
            max_speed: 360.0,
            retarget_secs: (0.3, 0.7),
            darts_per_sec: 1.0,
            depth_bias: -0.5,
        },
        sprite: &sprites::GOLDEN_KOI,
    },
];

/// One landed fish.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Catch {
    pub species: usize,
    pub length_mm: u16,
    pub weight_g: u32,
}

impl Catch {
    pub fn species(&self) -> &'static Species {
        &SPECIES[self.species]
    }

    /// Where its length falls in its species' range: 0 the shortest, 100 the
    /// longest.
    pub fn size_pct(&self) -> u8 {
        let (min, max) = self.species().length_cm;
        let fullness = ((f32::from(self.length_mm) / 10.0 - min) / (max - min)).clamp(0.0, 1.0);
        ops::round(fullness * 100.0) as u8
    }
}

pub fn weight_g(species: &Species, length_cm: f32) -> u32 {
    (species.girth * length_cm * length_cm * length_cm) as u32
}

/// Picks what bit. `accuracy` (0..1) is how close the bobber landed to the
/// fishing spot, `distance` (0..1) how far out it was cast; both favor rarer
/// tiers, and accuracy also favors longer fish within the species.
pub fn roll_catch(rng: &mut Rng, accuracy: f32, distance: f32) -> Catch {
    let skill = (0.65 * accuracy + 0.35 * distance).clamp(0.0, 1.0);
    let tier_odds = |tier: u8| match tier {
        0 => 1.0,
        1 => 0.35 + 0.9 * skill,
        2 => 0.05 + 0.9 * skill * skill,
        _ => 0.01 + 0.35 * skill * skill * skill,
    };
    let odds = |s: &Species| s.odds * tier_odds(s.tier);
    let total: f32 = SPECIES.iter().map(odds).sum();
    let mut pick = rng.next_f32() * total;
    let species = SPECIES
        .iter()
        .position(|s| {
            pick -= odds(s);
            pick < 0.0
        })
        .unwrap_or(0);

    let (min, max) = SPECIES[species].length_cm;
    let skew = ops::powf(rng.next_f32(), 1.6 - 1.2 * accuracy.clamp(0.0, 1.0));
    let length_cm = min + (max - min) * skew;
    Catch {
        species,
        length_mm: (length_cm * 10.0) as u16,
        weight_g: weight_g(&SPECIES[species], length_cm),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn average_tier_and_length(accuracy: f32, distance: f32) -> (f32, f32) {
        let mut rng = Rng::new(7);
        let n = 4000;
        let (mut tier, mut fullness) = (0.0, 0.0);
        for _ in 0..n {
            let catch = roll_catch(&mut rng, accuracy, distance);
            let (min, max) = catch.species().length_cm;
            tier += f32::from(catch.species().tier);
            fullness += (f32::from(catch.length_mm) / 10.0 - min) / (max - min);
        }
        (tier / n as f32, fullness / n as f32)
    }

    #[test]
    fn accurate_long_casts_land_rarer_and_bigger_fish() {
        let (sloppy_tier, sloppy_size) = average_tier_and_length(0.0, 0.1);
        let (sharp_tier, sharp_size) = average_tier_and_length(1.0, 1.0);
        assert!(
            sharp_tier > sloppy_tier + 0.6,
            "{sloppy_tier} vs {sharp_tier}"
        );
        assert!(
            sharp_size > sloppy_size + 0.2,
            "{sloppy_size} vs {sharp_size}"
        );
    }

    #[test]
    fn every_species_can_bite_a_perfect_cast() {
        let mut rng = Rng::new(3);
        let mut seen = [false; SPECIES.len()];
        for _ in 0..20_000 {
            seen[roll_catch(&mut rng, 1.0, 1.0).species] = true;
        }
        assert!(seen.iter().all(|&s| s), "{seen:?}");
    }

    #[test]
    fn catches_stay_within_their_species_length_range() {
        let mut rng = Rng::new(11);
        for _ in 0..2000 {
            let catch = roll_catch(&mut rng, 0.5, 0.5);
            let (min, max) = catch.species().length_cm;
            let cm = f32::from(catch.length_mm) / 10.0;
            assert!(
                (min - 0.1..=max).contains(&cm),
                "{cm} for {}",
                catch.species().name
            );
        }
    }

    fn catch_of(species: usize, length_cm: f32) -> Catch {
        Catch {
            species,
            length_mm: (length_cm * 10.0) as u16,
            weight_g: weight_g(&SPECIES[species], length_cm),
        }
    }

    #[test]
    fn size_runs_from_the_shortest_of_a_species_to_its_longest() {
        for (index, species) in SPECIES.iter().enumerate() {
            let (min, max) = species.length_cm;
            assert_eq!(catch_of(index, min).size_pct(), 0);
            assert_eq!(catch_of(index, max).size_pct(), 100);
            let middle = catch_of(index, (min + max) / 2.0).size_pct();
            assert!((48..=52).contains(&middle), "{}: {middle}", species.name);
        }
    }

    #[test]
    fn weight_follows_the_cube_of_length() {
        let bass = &SPECIES[4];
        assert_eq!(weight_g(bass, 40.0), 896);
        assert_eq!(weight_g(bass, 20.0), 112);
    }
}
