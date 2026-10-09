//! Spider Roulette on its own, without the town: twelve eggs, one spider.

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
