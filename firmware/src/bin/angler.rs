//! Angler (fishing) on its own, without the town.

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::main;

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    orbit_firmware::run(
        |app| {
            app.add_plugins(angler_game::AnglerPlugin);
        },
        angler_game::BACKGROUND,
    )
}
