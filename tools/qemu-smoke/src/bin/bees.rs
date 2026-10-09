//! Bees??? in QEMU: a steady hand turns against the jar's wobble for 20 s of
//! game time, then tips it until the honey runs out. The best orbit count must
//! reach emulated flash and read back.

#![no_std]
#![no_main]

extern crate alloc;

use bees_game::{BeesPlugin, CONTROL_GAIN, Orbit, Phase};
use bevy::prelude::*;
use esp_backtrace as _;
use esp_hal::main;
use esp_storage::FlashStorage;
use log::info;
use orbit_core::{SaveBlob, testing::*};
use orbit_qemu_smoke::Smoke;
use orbit_save::SlotStore;

esp_bootloader_esp_idf::esp_app_desc!();

/// Keep in sync with firmware/src/lib.rs.
const SAVE_FLASH_OFFSET: u32 = 0x9000;

#[main]
fn main() -> ! {
    let (mut smoke, flash) = Smoke::boot(
        |app| {
            app.add_plugins(BeesPlugin);
        },
        bees_game::BACKGROUND,
    );
    // Hold the 8 KB I2S stream buffer the firmware's speaker takes from the heap.
    core::mem::forget(alloc::vec![0u32; 2048]);
    let mut saves = SlotStore::new(FlashStorage::new(flash), SAVE_FLASH_OFFSET);
    click_action(&mut smoke.app);
    let until = smoke.app.world().resource::<Time>().elapsed_secs() + 20.0;
    while smoke.app.world().resource::<Time>().elapsed_secs() < until {
        let steer = smoke.app.world().resource::<Orbit>().drift_deg / CONTROL_GAIN;
        set_tilt(&mut smoke.app, steer, 0.0);
        smoke.frame();
    }
    let orbit = smoke.app.world().resource::<Orbit>();
    let (held, honey, orbits) = (orbit.phase == Phase::Orbiting, orbit.honey, orbit.orbits);
    set_tilt(&mut smoke.app, 80.0, 0.0);
    smoke.wait(20.0);
    let phase = smoke.app.world().resource::<Orbit>().phase;
    info!("steady for {orbits} orbits with {honey:.2} honey, then {phase:?}");
    let pending = smoke
        .app
        .world_mut()
        .resource_mut::<SaveBlob>()
        .take_pending();
    if let Some(bytes) = pending {
        saves.save(&bytes).ok();
    }
    let stored = SaveBlob::from_storage(saves.load());
    let failure = if !held || honey < 0.9 {
        Some("a steady hand spilled the honey")
    } else if phase != Phase::Empty {
        Some("tipping the jar didn't empty it")
    } else if stored.section(b"BEES").is_none() {
        Some("the best orbits didn't reach flash")
    } else {
        None
    };
    smoke.finish(failure)
}
