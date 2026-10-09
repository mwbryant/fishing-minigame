//! Bees??? on its own, without the town: keep the Moon's honey jar from spilling.

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::main;

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    orbit_firmware::run(
        |app| {
            app.add_plugins(bees_game::BeesPlugin);
        },
        bees_game::BACKGROUND,
    )
}
