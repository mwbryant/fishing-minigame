//! Power-fail-safe storage for one small save record on NOR flash.
//!
//! Two erase sectors take turns. Each save erases the older one and writes
//! `ORBS | seq u32 | len u32 | crc32 | payload` (little-endian, the CRC over
//! seq, len and payload). Loading picks the valid record with the highest
//! sequence number, so a save torn by power loss leaves the previous one
//! readable. Works on any `embedded-storage` NOR flash: the ESP32's
//! (`esp-storage`) or [`RamFlash`] (tests, desktop file saves).

#![no_std]

extern crate alloc;

use alloc::{vec, vec::Vec};

use embedded_storage::nor_flash::{
    ErrorType, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};

pub const SECTOR: u32 = 4096;
const MAGIC: [u8; 4] = *b"ORBS";
const HEADER: usize = 16;
pub const MAX_PAYLOAD: usize = SECTOR as usize - HEADER;

#[derive(Debug, PartialEq, Eq)]
pub enum SaveError<E> {
    TooLarge,
    Flash(E),
}

/// Two sectors starting at `base` (which must be sector aligned).
pub struct SlotStore<F> {
    flash: F,
    base: u32,
}

impl<F: NorFlash> SlotStore<F> {
    pub fn new(flash: F, base: u32) -> Self {
        Self { flash, base }
    }

    pub fn into_inner(self) -> F {
        self.flash
    }

    pub fn flash(&self) -> &F {
        &self.flash
    }

    /// The newest intact record, if any.
    pub fn load(&mut self) -> Option<Vec<u8>> {
        let (slot, _) = self.newest()?;
        self.read_record(slot).map(|(_, payload)| payload)
    }

    /// Writes over the older slot. Payloads over [`MAX_PAYLOAD`] are refused.
    pub fn save(&mut self, payload: &[u8]) -> Result<(), SaveError<F::Error>> {
        if payload.len() > MAX_PAYLOAD {
            return Err(SaveError::TooLarge);
        }
        let (target, seq) = match self.newest() {
            Some((slot, seq)) => (1 - slot, seq.wrapping_add(1)),
            None => (0, 1),
        };
        let mut record = vec![0u8; padded(HEADER + payload.len(), F::WRITE_SIZE)];
        record[..4].copy_from_slice(&MAGIC);
        record[4..8].copy_from_slice(&seq.to_le_bytes());
        record[8..12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        record[12..16].copy_from_slice(&checksum(seq, payload).to_le_bytes());
        record[HEADER..HEADER + payload.len()].copy_from_slice(payload);

        let start = self.slot_offset(target);
        self.flash
            .erase(start, start + SECTOR)
            .map_err(SaveError::Flash)?;
        self.flash.write(start, &record).map_err(SaveError::Flash)
    }

    /// Slot and sequence number of the newest valid record. Sequence numbers
    /// compare across wraparound.
    fn newest(&mut self) -> Option<(usize, u32)> {
        let a = self.read_record(0).map(|(seq, _)| seq);
        let b = self.read_record(1).map(|(seq, _)| seq);
        match (a, b) {
            (Some(a), Some(b)) if b.wrapping_sub(a) as i32 > 0 => Some((1, b)),
            (Some(a), _) => Some((0, a)),
            (None, Some(b)) => Some((1, b)),
            (None, None) => None,
        }
    }

    fn read_record(&mut self, slot: usize) -> Option<(u32, Vec<u8>)> {
        let start = self.slot_offset(slot);
        let mut header = [0u8; HEADER];
        self.flash.read(start, &mut header).ok()?;
        if header[..4] != MAGIC {
            return None;
        }
        let word =
            |i: usize| u32::from_le_bytes([header[i], header[i + 1], header[i + 2], header[i + 3]]);
        let (seq, len, crc) = (word(4), word(8) as usize, word(12));
        if len > MAX_PAYLOAD {
            return None;
        }
        let mut body = vec![0u8; padded(len, F::READ_SIZE)];
        self.flash.read(start + HEADER as u32, &mut body).ok()?;
        body.truncate(len);
        if checksum(seq, &body) != crc {
            return None;
        }
        Some((seq, body))
    }

    fn slot_offset(&self, slot: usize) -> u32 {
        self.base + slot as u32 * SECTOR
    }
}

fn padded(len: usize, unit: usize) -> usize {
    len.div_ceil(unit.max(1)) * unit.max(1)
}

fn checksum(seq: u32, payload: &[u8]) -> u32 {
    let mut crc = !0u32;
    let len = (payload.len() as u32).to_le_bytes();
    for &byte in seq.to_le_bytes().iter().chain(&len).chain(payload) {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// NOR flash in RAM with real NOR rules: erase sets bytes to 0xFF, writes can
/// only clear bits, and offsets/lengths must be 4-byte aligned like the ESP32's.
/// The desktop simulator persists its bytes to a file.
pub struct RamFlash {
    bytes: Vec<u8>,
    /// Test hook: writes stop after this many more bytes, like a power cut.
    pub cut_power_after: Option<usize>,
}

impl RamFlash {
    pub fn new(sectors: u32) -> Self {
        Self::from_bytes(vec![0xFF; (sectors * SECTOR) as usize])
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            cut_power_after: None,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn check(&self, offset: u32, len: usize, unit: usize) -> Result<usize, RamFlashError> {
        let offset = offset as usize;
        if !offset.is_multiple_of(unit) || !len.is_multiple_of(unit) {
            return Err(RamFlashError(NorFlashErrorKind::NotAligned));
        }
        if offset + len > self.bytes.len() {
            return Err(RamFlashError(NorFlashErrorKind::OutOfBounds));
        }
        Ok(offset)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RamFlashError(NorFlashErrorKind);

impl NorFlashError for RamFlashError {
    fn kind(&self) -> NorFlashErrorKind {
        self.0
    }
}

impl ErrorType for RamFlash {
    type Error = RamFlashError;
}

impl ReadNorFlash for RamFlash {
    const READ_SIZE: usize = 4;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let start = self.check(offset, bytes.len(), Self::READ_SIZE)?;
        bytes.copy_from_slice(&self.bytes[start..start + bytes.len()]);
        Ok(())
    }

    fn capacity(&self) -> usize {
        self.bytes.len()
    }
}

impl NorFlash for RamFlash {
    const WRITE_SIZE: usize = 4;
    const ERASE_SIZE: usize = SECTOR as usize;

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        let start = self.check(from, (to - from) as usize, Self::ERASE_SIZE)?;
        self.bytes[start..to as usize].fill(0xFF);
        Ok(())
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let start = self.check(offset, bytes.len(), Self::WRITE_SIZE)?;
        let len = self
            .cut_power_after
            .map_or(bytes.len(), |left| left.min(bytes.len()));
        for (dst, src) in self.bytes[start..start + len].iter_mut().zip(bytes) {
            *dst &= src;
        }
        if let Some(left) = self.cut_power_after.as_mut() {
            *left -= len;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> SlotStore<RamFlash> {
        SlotStore::new(RamFlash::new(3), SECTOR)
    }

    #[test]
    fn blank_flash_has_no_save() {
        assert_eq!(store().load(), None);
    }

    #[test]
    fn a_saved_record_loads_back() {
        let mut store = store();
        store.save(b"bass 41cm").unwrap();
        assert_eq!(store.load().as_deref(), Some(&b"bass 41cm"[..]));
    }

    #[test]
    fn the_newest_save_wins_as_saves_alternate_sectors() {
        let mut store = store();
        for n in 0u8..5 {
            store.save(&[n; 7]).unwrap();
            assert_eq!(store.load(), Some(vec![n; 7]));
        }
        let flash = store.into_inner();
        let magic_at = |offset: usize| flash.bytes()[offset..offset + 4] == MAGIC;
        assert!(magic_at(SECTOR as usize) && magic_at(2 * SECTOR as usize));
    }

    #[test]
    fn a_save_cut_short_by_power_loss_keeps_the_previous_record() {
        let mut store = store();
        store.save(b"first catch").unwrap();
        let mut flash = store.into_inner();
        flash.cut_power_after = Some(10);
        let mut store = SlotStore::new(flash, SECTOR);
        store.save(b"second catch, much longer").unwrap();
        assert_eq!(store.load().as_deref(), Some(&b"first catch"[..]));
    }

    #[test]
    fn flipped_bits_invalidate_a_record() {
        let mut store = store();
        store.save(b"perch").unwrap();
        let mut bytes = store.into_inner().bytes().to_vec();
        bytes[SECTOR as usize + HEADER + 2] ^= 0x10;
        let mut store = SlotStore::new(RamFlash::from_bytes(bytes), SECTOR);
        assert_eq!(store.load(), None);
    }

    #[test]
    fn records_survive_a_reboot() {
        let mut store = store();
        store.save(b"koi").unwrap();
        let bytes = store.into_inner().bytes().to_vec();
        let mut rebooted = SlotStore::new(RamFlash::from_bytes(bytes), SECTOR);
        assert_eq!(rebooted.load().as_deref(), Some(&b"koi"[..]));
    }

    #[test]
    fn oversized_payloads_are_refused() {
        let mut store = store();
        assert_eq!(store.save(&[0; MAX_PAYLOAD + 1]), Err(SaveError::TooLarge));
        assert!(store.save(&[7; MAX_PAYLOAD]).is_ok());
    }

    #[test]
    fn ram_flash_enforces_esp32_alignment() {
        let mut flash = RamFlash::new(1);
        assert!(flash.write(2, &[0; 4]).is_err());
        assert!(flash.write(0, &[0; 3]).is_err());
        assert!(flash.write(0, &[0; 4]).is_ok());
    }
}
