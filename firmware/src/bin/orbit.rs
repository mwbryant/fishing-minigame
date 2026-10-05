//! Orbit (circular pong) on the Waveshare ESP32-S3-Touch-AMOLED-1.75.

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::main;

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    orbit_firmware::run(
        |app| {
            app.add_plugins(orbit_game::OrbitPlugin);
        },
        orbit_game::BACKGROUND,
    )
}
