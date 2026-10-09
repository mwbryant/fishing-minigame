//! Orbit (circular pong), a separate game from Fishing Minigame.

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
