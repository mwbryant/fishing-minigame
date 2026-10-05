use alloc::vec::Vec;

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

const MAGIC: &[u8; 4] = b"FLOG";
const VERSION: u8 = 1;
const RECORD_BYTES: usize = 8;

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

    /// `FLOG | version | species count | total | (caught, length, weight)*`,
    /// little-endian.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(10 + RECORD_BYTES * SPECIES.len());
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(SPECIES.len() as u8);
        out.extend_from_slice(&self.total.to_le_bytes());
        for entry in &self.species {
            out.extend_from_slice(&entry.caught.to_le_bytes());
            out.extend_from_slice(&entry.best_length_mm.to_le_bytes());
            out.extend_from_slice(&entry.best_weight_g.to_le_bytes());
        }
        out
    }

    /// Saves from a build with more or fewer species load what overlaps.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let (header, rest) = bytes.split_at_checked(10)?;
        if &header[..4] != MAGIC || header[4] != VERSION {
            return None;
        }
        let stored = usize::from(header[5]);
        if rest.len() < stored * RECORD_BYTES {
            return None;
        }
        let mut log = FishLog {
            total: u32::from_le_bytes(header[6..10].try_into().ok()?),
            ..default()
        };
        for (entry, raw) in log
            .species
            .iter_mut()
            .zip(rest.chunks_exact(RECORD_BYTES).take(stored))
        {
            *entry = SpeciesRecord {
                caught: u16::from_le_bytes([raw[0], raw[1]]),
                best_length_mm: u16::from_le_bytes([raw[2], raw[3]]),
                best_weight_g: u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]),
            };
        }
        Some(log)
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

    #[test]
    fn the_log_round_trips_through_bytes() {
        let mut log = FishLog::default();
        log.record(&bass(400, 900));
        log.record(&Catch {
            species: 7,
            length_mm: 655,
            weight_g: 5620,
        });
        assert_eq!(FishLog::decode(&log.encode()), Some(log));
    }

    #[test]
    fn foreign_or_truncated_bytes_are_rejected() {
        let bytes = FishLog::default().encode();
        assert_eq!(FishLog::decode(&bytes[..bytes.len() - 1]), None);
        assert_eq!(FishLog::decode(b"NOPE and some more bytes"), None);
        assert_eq!(FishLog::decode(&[]), None);
    }

    #[test]
    fn saves_from_builds_with_fewer_species_still_load() {
        let mut log = FishLog::default();
        log.record(&bass(400, 900));
        let mut bytes = log.encode();
        bytes[5] = 5;
        bytes.truncate(10 + 5 * RECORD_BYTES);
        assert_eq!(FishLog::decode(&bytes), Some(log));
    }
}
