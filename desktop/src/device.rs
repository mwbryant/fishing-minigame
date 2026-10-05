use std::{fs, path::PathBuf};

use bevy::math::{Quat, Vec3};
use orbit_core::ImuSample;
use orbit_save::{RamFlash, SlotStore};

/// A pretend board held in the player's hands. Keys turn it; it reports what
/// its IMU would read, so the game's real motion filter does the rest.
pub struct VirtualDevice {
    /// Device-to-world rotation, world +Z up, in the screen frame the
    /// firmware maps the QMI8658 into (x right, y down, z into the screen).
    truth: Quat,
}

impl VirtualDevice {
    pub fn upright() -> Self {
        Self {
            truth: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
        }
    }

    /// Straightens the device without a gyro signal, like setting it down
    /// and picking it up again; the filter re-levels from gravity.
    pub fn straighten(&mut self) {
        *self = Self::upright();
    }

    /// `steer` turns clockwise as the player sees it; `lean` tips the top
    /// edge away. Both in degrees per second.
    pub fn sample(&mut self, steer: f32, lean: f32, dt: f32) -> ImuSample {
        let rate = Vec3::new(-lean, 0.0, steer) * core::f32::consts::PI / 180.0;
        self.truth = (self.truth * Quat::from_scaled_axis(rate * dt)).normalize();
        ImuSample {
            accel: self.truth.inverse() * Vec3::Z,
            gyro: rate,
            dt,
        }
    }
}

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
