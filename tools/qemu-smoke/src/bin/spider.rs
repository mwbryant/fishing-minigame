//! Spider Roulette in QEMU: rotation aims at each egg in turn and PWR
//! cracks it until the spider pounces (or is trapped); the best run is
//! saved through `esp-storage` into emulated flash and read back.

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
use spider_game::{EGGS, Phase, SpiderPlugin, Table};

esp_bootloader_esp_idf::esp_app_desc!();

/// Keep in sync with firmware/src/lib.rs.
const SAVE_FLASH_OFFSET: u32 = 0x9000;

#[main]
fn main() -> ! {
    let (mut smoke, flash) = Smoke::boot(
        |app| {
            app.add_plugins(SpiderPlugin);
        },
        spider_game::BACKGROUND,
    );
    core::mem::forget(alloc::vec![0u32; 2048]);
    let mut saves = SlotStore::new(FlashStorage::new(flash), SAVE_FLASH_OFFSET);
    click_action(&mut smoke.app);
    smoke
        .app
        .world_mut()
        .resource_mut::<Table>()
        .hide_spider(EGGS - 2);
    for egg in 0..EGGS {
        let dial = egg as f32 * 360.0 / EGGS as f32;
        let steer = if dial > 180.0 { dial - 360.0 } else { dial } / 3.0;
        set_tilt(&mut smoke.app, steer, 0.0);
        smoke.frame();
        click_action(&mut smoke.app);
        smoke.wait(0.2);
        if smoke.app.world().resource::<Table>().phase != Phase::Pick {
            break;
        }
    }
    smoke.wait(1.5);
    let table = smoke.app.world().resource::<Table>();
    let (phase, safe, best) = (table.phase, table.safe, table.best);
    info!("round ended {phase:?} after {safe} safe eggs, best {best}");
    let pending = smoke
        .app
        .world_mut()
        .resource_mut::<SaveBlob>()
        .take_pending();
    if let Some(bytes) = pending {
        saves.save(&bytes).ok();
    }
    let stored = SaveBlob::from_storage(saves.load());
    let failure = if phase != Phase::Bitten || usize::from(safe) != EGGS - 2 {
        Some("the spider wasn't where it was hidden")
    } else if stored.section(b"SPDR").is_none() {
        Some("the best run didn't reach flash")
    } else {
        None
    };
    smoke.finish(failure)
}
