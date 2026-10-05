use alloc::vec::Vec;

use bevy::prelude::*;

/// The game's persistent bytes, moved between the game and the platform's
/// storage (flash on the device, a file on desktop). The format belongs to the
/// game; the platform only stores what it's given.
#[derive(Resource, Default, Debug)]
pub struct SaveBlob {
    loaded: Option<Vec<u8>>,
    pending: Option<Vec<u8>>,
}

impl SaveBlob {
    /// What the platform found in storage at boot.
    pub fn from_storage(bytes: Option<Vec<u8>>) -> Self {
        Self {
            loaded: bytes,
            pending: None,
        }
    }

    pub fn loaded(&self) -> Option<&[u8]> {
        self.loaded.as_deref()
    }

    /// Asks the platform to write `bytes` after this update.
    pub fn request_save(&mut self, bytes: Vec<u8>) {
        self.pending = Some(bytes);
    }

    pub fn take_pending(&mut self) -> Option<Vec<u8>> {
        self.pending.take()
    }
}
