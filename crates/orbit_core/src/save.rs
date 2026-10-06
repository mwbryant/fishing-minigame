use alloc::vec::Vec;

use bevy::prelude::*;

const MAGIC: &[u8; 4] = b"ORBC";

/// The one record the platform stores (flash on the device, a file on
/// desktop), holding a section per game plus one for shared preferences, so
/// games built into one firmware don't overwrite each other:
/// `ORBC | count | (key | len u16 | bytes)*`, little-endian. Each section's
/// format belongs to its game; the platform only stores what it's given.
#[derive(Resource, Default, Debug)]
pub struct SaveBlob {
    sections: Vec<([u8; 4], Vec<u8>)>,
    changed: bool,
}

impl SaveBlob {
    /// What the platform found in storage at boot. A record from before
    /// sections existed (one game's bytes) becomes the section named by its
    /// first four bytes, its magic; a damaged record keeps what parses.
    pub fn from_storage(bytes: Option<Vec<u8>>) -> Self {
        let mut blob = Self::default();
        let Some(bytes) = bytes else {
            return blob;
        };
        let Some(body) = bytes.strip_prefix(MAGIC) else {
            if let Some(key) = bytes.first_chunk::<4>() {
                blob.sections.push((*key, bytes.clone()));
            }
            return blob;
        };
        let Some((&count, mut rest)) = body.split_first() else {
            return blob;
        };
        for _ in 0..count {
            let Some((head, tail)) = rest.split_at_checked(6) else {
                break;
            };
            let len = usize::from(u16::from_le_bytes([head[4], head[5]]));
            let Some((payload, tail)) = tail.split_at_checked(len) else {
                break;
            };
            blob.sections
                .push(([head[0], head[1], head[2], head[3]], payload.to_vec()));
            rest = tail;
        }
        blob
    }

    pub fn section(&self, key: &[u8; 4]) -> Option<&[u8]> {
        self.sections
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, bytes)| bytes.as_slice())
    }

    /// Replaces `key`'s section and asks the platform to write the record
    /// after this update.
    pub fn save_section(&mut self, key: [u8; 4], bytes: Vec<u8>) {
        match self.sections.iter_mut().find(|(k, _)| *k == key) {
            Some((_, old)) => *old = bytes,
            None => self.sections.push((key, bytes)),
        }
        self.changed = true;
    }

    /// The whole record, once after any section changed.
    pub fn take_pending(&mut self) -> Option<Vec<u8>> {
        if !self.changed {
            return None;
        }
        self.changed = false;
        let mut out = Vec::from(&MAGIC[..]);
        out.push(self.sections.len() as u8);
        for (key, bytes) in &self.sections {
            out.extend_from_slice(key);
            out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        Some(out)
    }
}
