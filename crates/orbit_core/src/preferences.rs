//! Settings shared by every game on the board, kept in their own save
//! section: `version | flags | brightness`, flags bit 0 = muted, brightness
//! in percent. Version 1 had no brightness byte and reads as full.

use alloc::vec;

use crate::{SaveBlob, SoundQueue};

pub const KEY: [u8; 4] = *b"PREF";
const VERSION: u8 = 2;
const MUTED: u8 = 1 << 0;
/// The brightnesses the settings step through, in percent.
pub const BRIGHTNESS_LEVELS: [u8; 4] = [25, 50, 75, 100];
const FULL: u8 = 100;

pub fn muted(blob: &SaveBlob) -> bool {
    flags(blob) & MUTED != 0
}

/// The panel brightness every game shows at, in percent.
pub fn brightness(blob: &SaveBlob) -> u8 {
    blob.section(&KEY)
        .and_then(|bytes| bytes.get(2).copied())
        .filter(|percent| (1..=FULL).contains(percent))
        .unwrap_or(FULL)
}

/// Mutes or unmutes the speaker now and saves the choice for every game.
pub fn set_muted(blob: &mut SaveBlob, sound: &mut SoundQueue, muted: bool) {
    sound.set_muted(muted);
    let flags = if muted { MUTED } else { 0 };
    store(blob, flags, brightness(blob));
}

/// Saves the panel brightness for every game; the platform applies it.
pub fn set_brightness(blob: &mut SaveBlob, percent: u8) {
    store(blob, flags(blob), percent.clamp(1, FULL));
}

fn flags(blob: &SaveBlob) -> u8 {
    blob.section(&KEY)
        .and_then(|bytes| bytes.get(1).copied())
        .unwrap_or(0)
}

fn store(blob: &mut SaveBlob, flags: u8, brightness: u8) {
    blob.save_section(KEY, vec![VERSION, flags, brightness]);
}
