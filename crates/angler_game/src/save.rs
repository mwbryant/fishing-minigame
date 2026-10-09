//! Angler's save section, little-endian: `FLOG | version | species count |
//! total | (caught, length, weight)* | difficulty`.
//!
//! Version 1 ends after the records. Version 3 also carried gold and hats,
//! which belong to `pals`; [`legacy_purse`] reads them so the town can hand
//! them over once.

use alloc::vec::Vec;

use crate::{
    log::{FishLog, SpeciesRecord},
    settings::{Difficulty, Settings},
    species::SPECIES,
};

use orbit_core::SaveBlob;

/// Angler's section key. It's also the magic the bytes start with, so
/// records from before save sections load as this section.
pub const KEY: [u8; 4] = *MAGIC;
const MAGIC: &[u8; 4] = b"FLOG";
const VERSION: u8 = 4;
const SHOP_VERSION: u8 = 3;
const HEADER_BYTES: usize = 10;
const RECORD_BYTES: usize = 8;
/// Difficulty, then in version 3 gold (4), owned-hat bitmask (4), worn hat (1).
const SHOP_TAIL_BYTES: usize = 10;

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

/// Angler's state from a whole stored record, as the platform writes it.
pub fn decode_stored(record: &[u8]) -> Option<(FishLog, Settings)> {
    SaveBlob::from_storage(Some(record.to_vec()))
        .section(&KEY)
        .and_then(decode)
}

/// Angler's state from its section. Saves from builds with more or fewer
/// species load what overlaps.
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

/// The gold and owned-hat bitmask in a version 3 save, written when Angler
/// ran the hat shop.
pub fn legacy_purse(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.get(..4)? != MAGIC || *bytes.get(4)? != SHOP_VERSION {
        return None;
    }
    let records = usize::from(*bytes.get(5)?) * RECORD_BYTES;
    let shop = bytes.get(HEADER_BYTES + records + 1..HEADER_BYTES + records + SHOP_TAIL_BYTES)?;
    Some((
        u32::from_le_bytes(shop[0..4].try_into().ok()?),
        u32::from_le_bytes(shop[4..8].try_into().ok()?),
    ))
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

    fn hard() -> Settings {
        Settings {
            difficulty: Difficulty::Hard,
        }
    }

    /// A version 3 record: the log, difficulty, then gold, hats and the worn hat.
    fn shop_era(gold: u32, owned: u32) -> Vec<u8> {
        let mut bytes = encode(&some_log(), &hard());
        bytes[4] = SHOP_VERSION;
        bytes.extend_from_slice(&gold.to_le_bytes());
        bytes.extend_from_slice(&owned.to_le_bytes());
        bytes.push(1);
        bytes
    }

    #[test]
    fn the_log_and_settings_round_trip_through_bytes() {
        assert_eq!(
            decode(&encode(&some_log(), &hard())),
            Some((some_log(), hard()))
        );
    }

    #[test]
    fn saves_from_before_settings_existed_load_at_normal_difficulty() {
        let mut bytes = encode(&some_log(), &hard());
        bytes[4] = 1;
        bytes.pop();
        assert_eq!(decode(&bytes), Some((some_log(), Settings::default())));
    }

    #[test]
    fn saves_from_the_shop_era_keep_their_log_and_hand_over_their_purse() {
        let bytes = shop_era(120, 0b1011);
        assert_eq!(decode(&bytes), Some((some_log(), hard())));
        assert_eq!(legacy_purse(&bytes), Some((120, 0b1011)));
        assert_eq!(legacy_purse(&encode(&some_log(), &hard())), None);
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
        let mut log = FishLog::default();
        log.record(&Catch {
            species: 4,
            length_mm: 400,
            weight_g: 900,
        });
        let bytes = encode(&log, &Settings::default());
        let mut fewer = bytes[..HEADER_BYTES + 5 * RECORD_BYTES].to_vec();
        fewer[5] = 5;
        fewer.push(*bytes.last().unwrap());
        assert_eq!(decode(&fewer), Some((log, Settings::default())));
    }
}
