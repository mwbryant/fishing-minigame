use bevy::prelude::*;

use crate::{cast, reel};

/// Player preferences, saved alongside the fish log.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub difficulty: Difficulty,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

/// What a difficulty changes. Normal is the tuning the reel tests pin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// Height of the catch zone on the reel bar.
    pub zone_height: f32,
    /// Multiplies how fast the catch meter drains while the fish is out.
    pub drain_scale: f32,
    /// Multiplies how hard and fast fish swim.
    pub fish_speed_scale: f32,
    /// Releasing with the meter at or above this throws exactly to the dots.
    pub perfect_power: f32,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "EASY",
            Difficulty::Normal => "NORMAL",
            Difficulty::Hard => "HARD",
        }
    }

    /// Two short lines for the settings screen.
    pub fn blurb(self) -> [&'static str; 2] {
        match self {
            Difficulty::Easy => ["BIG CATCH ZONE, LAZY FISH", "WIDE PERFECT WINDOW"],
            Difficulty::Normal => ["THE INTENDED CHALLENGE", ""],
            Difficulty::Hard => ["SMALL CATCH ZONE, FAST FISH", "TIGHT PERFECT WINDOW"],
        }
    }

    pub fn tuning(self) -> Tuning {
        match self {
            Difficulty::Easy => Tuning {
                zone_height: 124.0,
                drain_scale: 0.7,
                fish_speed_scale: 0.85,
                perfect_power: 0.90,
            },
            Difficulty::Normal => Tuning {
                zone_height: reel::ZONE_HEIGHT,
                drain_scale: 1.0,
                fish_speed_scale: 1.0,
                perfect_power: cast::PERFECT_POWER,
            },
            Difficulty::Hard => Tuning {
                zone_height: 76.0,
                drain_scale: 1.3,
                fish_speed_scale: 1.15,
                perfect_power: 0.98,
            },
        }
    }

    /// Cycles EASY -> NORMAL -> HARD -> EASY.
    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|d| *d == self).unwrap_or(1)
    }

    pub fn from_index(index: u8) -> Option<Self> {
        Self::ALL.get(usize::from(index)).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulties_cycle_both_ways() {
        assert_eq!(Difficulty::Normal.next(), Difficulty::Hard);
        assert_eq!(Difficulty::Hard.next(), Difficulty::Easy);
        assert_eq!(Difficulty::Easy.previous(), Difficulty::Hard);
        assert_eq!(Difficulty::Normal.previous(), Difficulty::Easy);
    }

    #[test]
    fn harder_means_a_smaller_zone_faster_drain_faster_fish_and_a_tighter_window() {
        let [easy, normal, hard] = Difficulty::ALL.map(Difficulty::tuning);
        for (looser, tighter) in [(easy, normal), (normal, hard)] {
            assert!(looser.zone_height > tighter.zone_height);
            assert!(looser.drain_scale < tighter.drain_scale);
            assert!(looser.fish_speed_scale < tighter.fish_speed_scale);
            assert!(looser.perfect_power < tighter.perfect_power);
        }
    }

    #[test]
    fn indexes_round_trip_for_saving() {
        for difficulty in Difficulty::ALL {
            assert_eq!(
                Difficulty::from_index(difficulty.index() as u8),
                Some(difficulty)
            );
        }
        assert_eq!(Difficulty::from_index(9), None);
    }
}
