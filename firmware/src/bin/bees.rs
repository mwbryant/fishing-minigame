//! Bees??? (keep the Moon's honey jar from spilling) on the Waveshare ESP32-S3-Touch-AMOLED-1.75, on its own.

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
