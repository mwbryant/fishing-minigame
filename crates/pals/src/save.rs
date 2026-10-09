//! The `PALS` section of the board's save record: the tank, purse, hats and
//! mock-walk setting.
//!
//! Little-endian: `version | gold u32 | owned hats u32 | flags | pet count |
//! pet*`. Flags: bit 0 mock walks, bits 1-3 their pace (an index into
//! `MOCK_PACES`). A pet is `species | tier | length u16 | size | name length |
//! name 8 | hat (0xFF none) | walked u32 | trick*5`, and a trick is
//! `id | learned | gesture (0 none) | has word | word print?`.
//!
//! Version 1 word prints are an older 145-byte kind that can't be compared with
//! the current one, so decoding skips them and those tricks need their words
//! taught again.

use alloc::vec::Vec;

use orbit_core::{SaveBlob, VoicePrint};

use crate::{
    MAX_PETS, Pals,
    gesture::Gesture,
    pet::{MAX_NAME, Name, Pet, TrickSlot, Trigger},
    tricks::{TRICKS, TRICKS_PER_PET},
    walk::MOCK_PACES,
};

pub const KEY: [u8; 4] = *b"PALS";
const VERSION: u8 = 2;
const OLD_PRINT_BYTES: usize = 145;
const MOCK_WALK: u8 = 1;
const PACE_SHIFT: u8 = 1;
const PACE_BITS: u8 = 0b111;
const NO_HAT: u8 = 0xFF;

pub fn encode(pals: &Pals) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(VERSION);
    out.extend_from_slice(&pals.gold.to_le_bytes());
    out.extend_from_slice(&pals.owned_hats_mask().to_le_bytes());
    let mock = if pals.mock_walk { MOCK_WALK } else { 0 };
    out.push(mock | (pals.mock_pace as u8 & PACE_BITS) << PACE_SHIFT);
    out.push(pals.pets.len() as u8);
    for pet in &pals.pets {
        out.extend_from_slice(&[pet.species, pet.tier]);
        out.extend_from_slice(&pet.length_mm.to_le_bytes());
        out.push(pet.size_pct);
        out.push(pet.name.len() as u8);
        out.extend_from_slice(&pet.name.letters());
        out.push(pet.hat.unwrap_or(NO_HAT));
        out.extend_from_slice(&pet.walked_m.to_le_bytes());
        for slot in &pet.tricks {
            let gesture = slot.trigger.gesture.map_or(0, |g| g as u8);
            out.extend_from_slice(&[slot.trick, slot.learned, gesture]);
            match &slot.trigger.voice {
                Some(voice) => {
                    out.push(1);
                    out.extend_from_slice(&voice.to_bytes());
                }
                None => out.push(0),
            }
        }
    }
    out
}

/// `None` unless `bytes` is exactly one valid section.
pub fn decode(bytes: &[u8]) -> Option<Pals> {
    let mut reader = Reader(bytes);
    let version = reader.byte()?;
    if !(1..=VERSION).contains(&version) {
        return None;
    }
    let mut pals = Pals {
        gold: reader.u32()?,
        ..Pals::default()
    };
    pals.set_owned_hats(reader.u32()?);
    let flags = reader.byte()?;
    pals.mock_walk = flags & MOCK_WALK != 0;
    pals.mock_pace = usize::from(flags >> PACE_SHIFT & PACE_BITS).min(MOCK_PACES.len() - 1);
    let count = usize::from(reader.byte()?);
    if count > MAX_PETS {
        return None;
    }
    for _ in 0..count {
        let species = reader.byte()?;
        let tier = reader.byte()?;
        let length_mm = reader.u16()?;
        let size_pct = reader.byte()?;
        let name_length = usize::from(reader.byte()?).min(MAX_NAME);
        let name = Name::from_bytes(&reader.take(MAX_NAME)?[..name_length])?;
        let hat = Some(reader.byte()?).filter(|&hat| hat != NO_HAT);
        let walked_m = reader.u32()?;
        let mut tricks = [TrickSlot {
            trick: 0,
            learned: 0,
            trigger: Trigger::default(),
        }; TRICKS_PER_PET];
        for slot in &mut tricks {
            slot.trick = reader.byte()?;
            if usize::from(slot.trick) >= TRICKS.len() {
                return None;
            }
            slot.learned = reader.byte()?.min(100);
            slot.trigger.gesture = Gesture::from_code(reader.byte()?);
            match (reader.byte()?, version) {
                (0, _) => {}
                (_, 1) => {
                    reader.take(OLD_PRINT_BYTES)?;
                }
                _ => {
                    let print = reader.take(VoicePrint::BYTES)?;
                    slot.trigger.voice = Some(VoicePrint::from_bytes(print)?);
                }
            }
        }
        pals.pets.push(Pet {
            species,
            tier,
            length_mm,
            size_pct,
            name,
            hat,
            walked_m,
            tricks,
        });
    }
    reader.0.is_empty().then_some(pals)
}

/// The tank as saved, or an empty one.
pub fn load(blob: &SaveBlob) -> Pals {
    blob.section(&KEY).and_then(decode).unwrap_or_default()
}

pub fn store(blob: &mut SaveBlob, pals: &Pals) {
    blob.save_section(KEY, encode(pals));
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let (taken, rest) = self.0.split_at_checked(count)?;
        self.0 = rest;
        Some(taken)
    }

    fn byte(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
}
