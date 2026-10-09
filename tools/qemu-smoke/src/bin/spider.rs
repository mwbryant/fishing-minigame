//! Spider Roulette in QEMU: with the spider hidden in the second-to-last egg,
//! PWR spins the pointer onto each egg in turn until it finds the spider. The
//! best run must reach emulated flash through `esp-storage` and read back.

#![no_std]
#![no_main]

extern crate alloc;

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
    // Hold the 8 KB I2S stream buffer the firmware's speaker takes from the heap.
    core::mem::forget(alloc::vec![0u32; 2048]);
    let mut saves = SlotStore::new(FlashStorage::new(flash), SAVE_FLASH_OFFSET);
    click_action(&mut smoke.app);
    smoke
        .app
        .world_mut()
        .resource_mut::<Table>()
        .hide_spider(EGGS - 2);
    let mut spins = 0;
    for egg in 0..EGGS {
        smoke
            .app
            .world_mut()
            .resource_mut::<Table>()
            .rig_next_spin(egg);
        click_action(&mut smoke.app);
        spins += 1;
        smoke.wait(spider_game::SPIN_SECS + spider_game::REVEAL_SECS + 0.3);
        if smoke.app.world().resource::<Table>().phase != Phase::Ready {
            break;
        }
    }
    info!("{spins} spins");
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
