//! The simulator's stand-in for the board's save flash (one file per game).

use std::{fs, path::PathBuf};

use orbit_save::{RamFlash, SlotStore};

/// The game's save storage on disk: two flash sectors in the same format the
/// firmware writes, kept in `~/.logic-orbit/<game>.flash`.
pub struct SaveFile {
    path: PathBuf,
    store: SlotStore<RamFlash>,
}

impl SaveFile {
    pub fn open(game: &str) -> Self {
        let dir = PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".logic-orbit");
        let path = dir.join(format!("{game}.flash"));
        let flash = fs::read(&path)
            .ok()
            .filter(|bytes| bytes.len() == 2 * orbit_save::SECTOR as usize)
            .map_or_else(|| RamFlash::new(2), RamFlash::from_bytes);
        Self {
            path,
            store: SlotStore::new(flash, 0),
        }
    }

    pub fn load(&mut self) -> Option<Vec<u8>> {
        self.store.load()
    }

    pub fn save(&mut self, payload: &[u8]) {
        if let Err(err) = self.store.save(payload) {
            eprintln!("save failed: {err:?}");
            return;
        }
        let written = self
            .path
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| fs::write(&self.path, self.store.flash().bytes()));
        if let Err(err) = written {
            eprintln!("writing {}: {err}", self.path.display());
        }
    }
}
