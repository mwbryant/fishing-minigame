//! The bytes Angler hands the platform to persist:
//! `FLOG | version | species count | total | (caught, length, weight)* | difficulty`,
//! little-endian. Version 1 (before settings existed) had no difficulty byte
//! and still loads.

use alloc::vec::Vec;

use crate::{
    log::{FishLog, SpeciesRecord},
    settings::{Difficulty, Settings},
    species::SPECIES,
};

const MAGIC: &[u8; 4] = b"FLOG";
const VERSION: u8 = 2;
const HEADER_BYTES: usize = 10;
const RECORD_BYTES: usize = 8;

pub fn encode(log: &FishLog, settings: &Settings) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_BYTES + RECORD_BYTES * SPECIES.len() + 1);
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(SPECIES.len() as u8);
    out.extend_from_slice(&log.total.to_le_bytes());
    for entry in &log.species {
        out.extend_from_slice(&entry.caught.to_le_bytes());
        out.extend_from_slice(&entry.best_length_mm.to_le_bytes());
        out.extend_from_slice(&entry.best_weight_g.to_le_bytes());
    }
    out.push(settings.difficulty.index() as u8);
    out
}

/// Saves from builds with more or fewer species load what overlaps.
pub fn decode(bytes: &[u8]) -> Option<(FishLog, Settings)> {
    let (header, rest) = bytes.split_at_checked(HEADER_BYTES)?;
    let version = header[4];
    if &header[..4] != MAGIC || !(1..=VERSION).contains(&version) {
        return None;
    }
    let (records, tail) = rest.split_at_checked(usize::from(header[5]) * RECORD_BYTES)?;
    let mut log = FishLog {
        total: u32::from_le_bytes(header[6..10].try_into().ok()?),
        ..Default::default()
    };
    for (entry, raw) in log
        .species
        .iter_mut()
        .zip(records.chunks_exact(RECORD_BYTES))
    {
        *entry = SpeciesRecord {
            caught: u16::from_le_bytes([raw[0], raw[1]]),
            best_length_mm: u16::from_le_bytes([raw[2], raw[3]]),
            best_weight_g: u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]),
        };
    }
    let settings = match version {
        1 => Settings::default(),
        _ => Settings {
            difficulty: Difficulty::from_index(*tail.first()?).unwrap_or_default(),
        },
    };
    Some((log, settings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species::Catch;

    fn some_log() -> FishLog {
        let mut log = FishLog::default();
        log.record(&Catch {
            species: 4,
            length_mm: 400,
            weight_g: 900,
        });
        log.record(&Catch {
            species: 7,
            length_mm: 655,
            weight_g: 5620,
        });
        log
    }

    #[test]
    fn the_log_and_settings_round_trip_through_bytes() {
        let settings = Settings {
            difficulty: Difficulty::Hard,
        };
        assert_eq!(
            decode(&encode(&some_log(), &settings)),
            Some((some_log(), settings))
        );
    }

    #[test]
    fn saves_from_before_settings_existed_load_at_normal_difficulty() {
        let mut bytes = encode(&some_log(), &Settings::default());
        bytes[4] = 1;
        bytes.pop();
        assert_eq!(decode(&bytes), Some((some_log(), Settings::default())));
    }

    #[test]
    fn foreign_or_truncated_bytes_are_rejected() {
        let bytes = encode(&FishLog::default(), &Settings::default());
        assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
        assert_eq!(decode(b"NOPE and some more bytes"), None);
        assert_eq!(decode(&[]), None);
    }

    #[test]
    fn saves_from_builds_with_fewer_species_still_load() {
        let log = {
            let mut log = FishLog::default();
            log.record(&Catch {
                species: 4,
                length_mm: 400,
                weight_g: 900,
            });
            log
        };
        let bytes = encode(&log, &Settings::default());
        let mut fewer = bytes[..HEADER_BYTES + 5 * RECORD_BYTES].to_vec();
        fewer[5] = 5;
        fewer.push(Difficulty::Normal.index() as u8);
        assert_eq!(decode(&fewer), Some((log, Settings::default())));
    }
}
