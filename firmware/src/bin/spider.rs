//! Spider Roulette (twelve eggs, one spider) on the Waveshare ESP32-S3-Touch-AMOLED-1.75, on its own.

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::main;

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    orbit_firmware::run(
        |app| {
            app.add_plugins(spider_game::SpiderPlugin);
        },
        spider_game::BACKGROUND,
    )
}
