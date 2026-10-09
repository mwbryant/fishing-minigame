//! The simulator's stand-ins for the board's IMU (a virtual device turned
//! from the keyboard) and its save flash (one file per game).

use std::{fs, path::PathBuf};

use bevy::math::{Quat, Vec3};
use orbit_core::ImuSample;
use orbit_save::{RamFlash, SlotStore};

/// How far holding ↑/↓ tips the device.
pub const LEAN_HELD_DEG: f32 = 15.0;
/// How fast it tips there and back.
pub const LEAN_DEG_PER_SEC: f32 = 120.0;

/// A pretend board held in the player's hands. Keys turn it; it reports what
/// its IMU would read, so the game's real motion filter does the rest.
pub struct VirtualDevice {
    /// Device-to-world rotation, world +Z up, in the screen frame the
    /// firmware maps the QMI8658 into (x right, y down, z into the screen).
    truth: Quat,
    lean_deg: f32,
}

impl VirtualDevice {
    pub fn upright() -> Self {
        Self {
            truth: Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
            lean_deg: 0.0,
        }
    }

    /// Straightens the device without a gyro signal, like setting it down
    /// and picking it up again; the filter re-levels from gravity.
    pub fn straighten(&mut self) {
        *self = Self::upright();
    }

    /// Moves the device on by `dt` seconds and returns what its IMU reads.
    /// `steer` turns it clockwise (as the player sees it) in degrees per
    /// second, and it stays turned. `lean` (-1..1) is how hard ↑/↓ are held:
    /// the top edge tips away (or back) toward `lean * LEAN_HELD_DEG` and
    /// springs back level on release, like a wrist.
    pub fn sample(&mut self, steer: f32, lean: f32, dt: f32) -> ImuSample {
        let max_step = LEAN_DEG_PER_SEC * dt;
        let lean_step = (lean * LEAN_HELD_DEG - self.lean_deg).clamp(-max_step, max_step);
        self.lean_deg += lean_step;
        let lean_rate = if dt > 0.0 { lean_step / dt } else { 0.0 };
        let rate = Vec3::new(-lean_rate, 0.0, steer) * core::f32::consts::PI / 180.0;
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

#[cfg(test)]
mod tests {
    use orbit_motion::MotionTracker;

    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn hold(
        device: &mut VirtualDevice,
        tracker: &mut MotionTracker,
        steer: f32,
        lean: f32,
        secs: f32,
    ) {
        for _ in 0..(secs / DT) as usize {
            tracker.update(&device.sample(steer, lean, DT));
        }
    }

    /// A device at rest, as the simulator starts: the filter's first sample
    /// only finds "down".
    fn resting() -> (VirtualDevice, MotionTracker) {
        let (mut device, mut tracker) = (VirtualDevice::upright(), MotionTracker::default());
        hold(&mut device, &mut tracker, 0.0, 0.0, 0.1);
        (device, tracker)
    }

    #[test]
    fn holding_up_tips_the_top_away_and_releasing_levels_it_again() {
        let (mut device, mut tracker) = resting();
        hold(&mut device, &mut tracker, 0.0, 1.0, 0.5);
        assert!(
            (tracker.lean_deg() - LEAN_HELD_DEG).abs() < 1.0,
            "{}",
            tracker.lean_deg()
        );
        hold(&mut device, &mut tracker, 0.0, 0.0, 0.5);
        assert!(tracker.lean_deg().abs() < 1.0, "{}", tracker.lean_deg());
    }

    #[test]
    fn steering_stays_where_it_is_left() {
        let (mut device, mut tracker) = resting();
        hold(&mut device, &mut tracker, -60.0, 0.0, 0.5);
        hold(&mut device, &mut tracker, 0.0, 0.0, 0.5);
        assert!(
            (tracker.steer_deg() + 30.0).abs() < 1.5,
            "{}",
            tracker.steer_deg()
        );
    }
}
