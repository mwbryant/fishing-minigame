//! Fish Pals on the Waveshare ESP32-S3-Touch-AMOLED-1.75: the town, then
//! whichever place its door led to, then the town again at that door. Each
//! app is dropped (freeing its heap) before the next is built.

#![no_std]
#![no_main]

use bevy::prelude::*;
use esp_backtrace as _;
use esp_hal::main;
use orbit_firmware::Board;
use orbit_gfx::Rgb565;
use pals_game::{
    Arrival, Place,
    town::{Town, TownPlugin},
};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let mut board = Board::new();
    let mut arrival = None;
    loop {
        let town = board.run(
            move |app| {
                app.add_plugins(TownPlugin);
                if let Some(place) = arrival {
                    app.insert_resource(Arrival(place));
                }
            },
            pals_game::town::BACKGROUND,
            false,
        );
        let went = town.world().resource::<Town>().destination();
        drop(town);
        if let Some(place) = went {
            let (add_place, background) = launch(place);
            drop(board.run(add_place, background, true));
            arrival = Some(place);
        }
    }
}

/// The app behind each door.
fn launch(place: Place) -> (fn(&mut App), Rgb565) {
    match place {
        Place::Home => (
            |app| {
                app.add_plugins(pals_game::home::HomePlugin);
            },
            pals_game::home::BACKGROUND,
        ),
        Place::Park => (
            |app| {
                app.add_plugins(pals_game::walk::WalkPlugin);
            },
            pals_game::walk::BACKGROUND,
        ),
        Place::Contest => (
            |app| {
                app.add_plugins(pals_game::contest::ContestPlugin);
            },
            pals_game::contest::BACKGROUND,
        ),
        Place::Shop => (
            |app| {
                app.add_plugins(pals_game::shop::ShopPlugin);
            },
            pals_game::shop::BACKGROUND,
        ),
        Place::Pond => (
            |app| {
                app.add_plugins(angler_game::AnglerPlugin);
            },
            angler_game::BACKGROUND,
        ),
        Place::Den => (
            |app| {
                app.add_plugins(spider_game::SpiderPlugin);
            },
            spider_game::BACKGROUND,
        ),
        Place::Arcade => (
            |app| {
                app.add_plugins(skate_game::SkatePlugin);
            },
            skate_game::BACKGROUND,
        ),
        Place::BeeTree => (
            |app| {
                app.add_plugins(bees_game::BeesPlugin);
            },
            bees_game::BACKGROUND,
        ),
        // The town opens its own settings; it never leaves for them.
        Place::Settings => (
            |app| {
                app.add_plugins(TownPlugin);
            },
            pals_game::town::BACKGROUND,
        ),
    }
}
