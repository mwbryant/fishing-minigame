//! Settings shared by every game on the board, kept in their own save
//! section: `version | flags`, bit 0 = muted.

use alloc::vec;

use crate::{SaveBlob, SoundQueue};

pub const KEY: [u8; 4] = *b"PREF";
const VERSION: u8 = 1;
const MUTED: u8 = 1 << 0;

pub fn muted(blob: &SaveBlob) -> bool {
    blob.section(&KEY)
        .and_then(|bytes| bytes.get(1))
        .is_some_and(|flags| flags & MUTED != 0)
}

/// Mutes or unmutes the speaker now and saves the choice for every game.
pub fn set_muted(blob: &mut SaveBlob, sound: &mut SoundQueue, muted: bool) {
    sound.set_muted(muted);
    blob.save_section(KEY, vec![VERSION, if muted { MUTED } else { 0 }]);
}
