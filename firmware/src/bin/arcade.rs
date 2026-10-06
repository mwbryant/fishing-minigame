//! The arcade on the Waveshare ESP32-S3-Touch-AMOLED-1.75: the boot menu,
//! then whichever game it picked, then the menu again when the game goes
//! HOME. Each app is dropped (freeing its heap) before the next is built.

#![no_std]
#![no_main]

use arcade::{ArcadePlugin, Game, Menu};
use bevy::prelude::*;
use esp_backtrace as _;
use esp_hal::main;
use orbit_firmware::Board;
use orbit_gfx::Rgb565;

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let mut board = Board::new();
    loop {
        let menu = board.run(
            |app| {
                app.add_plugins(ArcadePlugin);
            },
            arcade::BACKGROUND,
            false,
        );
        let picked = menu.world().resource::<Menu>().chosen();
        drop(menu);
        if let Some(game) = picked {
            let (add_game, background) = launch(game);
            drop(board.run(add_game, background, true));
        }
    }
}

fn launch(game: Game) -> (fn(&mut App), Rgb565) {
    match game {
        Game::Angler => (
            |app| {
                app.add_plugins(angler_game::AnglerPlugin);
            },
            angler_game::BACKGROUND,
        ),
        Game::Spider => (
            |app| {
                app.add_plugins(spider_game::SpiderPlugin);
            },
            spider_game::BACKGROUND,
        ),
        Game::Skate => (
            |app| {
                app.add_plugins(skate_game::SkatePlugin);
            },
            skate_game::BACKGROUND,
        ),
        Game::Bees => (
            |app| {
                app.add_plugins(bees_game::BeesPlugin);
            },
            bees_game::BACKGROUND,
        ),
    }
}
