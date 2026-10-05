use bevy::prelude::*;

use crate::species::{Catch, SPECIES};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpeciesRecord {
    pub caught: u16,
    pub best_length_mm: u16,
    pub best_weight_g: u32,
}

/// Every fish ever landed, persisted through the platform's save storage.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct FishLog {
    pub total: u32,
    pub species: [SpeciesRecord; SPECIES.len()],
}

/// What a catch meant for the log, for the celebration screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Milestones {
    pub new_species: bool,
    pub longest: bool,
    pub heaviest: bool,
}

impl FishLog {
    pub fn record(&mut self, catch: &Catch) -> Milestones {
        self.total += 1;
        let entry = &mut self.species[catch.species];
        let milestones = Milestones {
            new_species: entry.caught == 0,
            longest: entry.caught > 0 && catch.length_mm > entry.best_length_mm,
            heaviest: entry.caught > 0 && catch.weight_g > entry.best_weight_g,
        };
        entry.caught = entry.caught.saturating_add(1);
        entry.best_length_mm = entry.best_length_mm.max(catch.length_mm);
        entry.best_weight_g = entry.best_weight_g.max(catch.weight_g);
        milestones
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bass(length_mm: u16, weight_g: u32) -> Catch {
        Catch {
            species: 4,
            length_mm,
            weight_g,
        }
    }

    #[test]
    fn the_first_of_a_species_is_a_new_species_not_a_record() {
        let mut log = FishLog::default();
        let milestones = log.record(&bass(400, 900));
        assert_eq!(
            milestones,
            Milestones {
                new_species: true,
                longest: false,
                heaviest: false
            }
        );
        assert_eq!(log.total, 1);
        assert_eq!(
            log.species[4],
            SpeciesRecord {
                caught: 1,
                best_length_mm: 400,
                best_weight_g: 900
            }
        );
    }

    #[test]
    fn bigger_catches_beat_the_species_bests() {
        let mut log = FishLog::default();
        log.record(&bass(400, 900));
        assert_eq!(log.record(&bass(380, 800)), Milestones::default());
        let milestones = log.record(&bass(450, 1200));
        assert!(milestones.longest && milestones.heaviest && !milestones.new_species);
        assert_eq!(log.species[4].caught, 3);
        assert_eq!(log.species[4].best_length_mm, 450);
    }
}
