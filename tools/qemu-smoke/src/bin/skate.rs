//! Skate in QEMU: a steady rider pops (a counterclockwise turn) just before
//! each obstacle for 30 s of game time as the street speeds up, through the
//! halfpipe at 150 m and back out, then rolls into a cone; the best score is
//! saved into emulated flash and read back.

#![no_std]
#![no_main]

extern crate alloc;

use bevy::prelude::*;
use esp_backtrace as _;
use esp_hal::main;
use esp_storage::FlashStorage;
use log::info;
use orbit_core::{SaveBlob, testing::*};
use orbit_qemu_smoke::Smoke;
use orbit_save::SlotStore;
use skate_game::{BOARD_HALF, Kind, POP_DEG, Phase, Ride, SKATER_X, SkatePlugin};

esp_bootloader_esp_idf::esp_app_desc!();

/// Keep in sync with firmware/src/lib.rs.
const SAVE_FLASH_OFFSET: u32 = 0x9000;

#[main]
fn main() -> ! {
    let (mut smoke, flash) = Smoke::boot(
        |app| {
            app.add_plugins(SkatePlugin);
        },
        skate_game::BACKGROUND,
    );
    core::mem::forget(alloc::vec![0u32; 2048]);
    let mut saves = SlotStore::new(FlashStorage::new(flash), SAVE_FLASH_OFFSET);
    click_action(&mut smoke.app);
    let until = smoke.app.world().resource::<Time>().elapsed_secs() + 30.0;
    let mut visited_halfpipe = false;
    loop {
        let ride = smoke.app.world().resource::<Ride>();
        let secs = smoke.app.world().resource::<Time>().elapsed_secs();
        if ride.phase != Phase::Riding || (secs >= until && !ride.in_halfpipe()) {
            break;
        }
        visited_halfpipe |= ride.in_halfpipe();
        let front = SKATER_X + BOARD_HALF;
        let close = ride
            .obstacles()
            .iter()
            .any(|o| (0.0..55.0).contains(&(o.x - front)));
        let steer = if close && !ride.airborne() {
            -(POP_DEG + 5.0)
        } else {
            0.0
        };
        set_tilt(&mut smoke.app, steer, 0.0);
        smoke.frame();
    }
    let survived = smoke.app.world().resource::<Ride>().phase == Phase::Riding;
    set_tilt(&mut smoke.app, 0.0, 0.0);
    smoke
        .app
        .world_mut()
        .resource_mut::<Ride>()
        .place(Kind::Cone, SKATER_X + 40.0);
    smoke.wait(1.0);
    let ride = smoke.app.world().resource::<Ride>();
    let (phase, score, spawned) = (ride.phase, ride.score, ride.spawned);
    info!("rode to {score} over {spawned} obstacles, then {phase:?}");
    let pending = smoke
        .app
        .world_mut()
        .resource_mut::<SaveBlob>()
        .take_pending();
    if let Some(bytes) = pending {
        saves.save(&bytes).ok();
    }
    let stored = SaveBlob::from_storage(saves.load());
    let failure = if !survived {
        Some("the steady rider bailed")
    } else if !visited_halfpipe {
        Some("the street never dropped into the halfpipe")
    } else if phase != Phase::Bailed {
        Some("the cone didn't stop the ride")
    } else if stored.section(b"SKAT").is_none() {
        Some("the best score didn't reach flash")
    } else {
        None
    };
    smoke.finish(failure)
}
