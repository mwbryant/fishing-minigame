//! Skate on its own, without the town: a street side-scroller ridden by turning the device.

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::main;

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    orbit_firmware::run(
        |app| {
            app.add_plugins(skate_game::SkatePlugin);
        },
        skate_game::BACKGROUND,
    )
}
