//! The arcade in QEMU: the menu picks each game by rotation and PWR, the
//! game is built in the menu's place with HOME, HOME goes back, twice
//! round. Internal heap after each return to the menu must not creep (no
//! leak across apps), and nothing may spill into PSRAM.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;

use arcade::{ArcadePlugin, Game, Menu};
use bevy::prelude::*;
use esp_backtrace as _;
use esp_hal::main;
use log::info;
use orbit_core::testing::*;
use orbit_gfx::Rgb565;
use orbit_qemu_smoke::{Smoke, internal_heap_used};

esp_bootloader_esp_idf::esp_app_desc!();

/// Heap a return to the menu may differ by before it counts as a leak.
const LEAK_SLACK: usize = 2048;

fn menu(app: &mut App) {
    app.add_plugins(ArcadePlugin);
}

#[main]
fn main() -> ! {
    let (mut smoke, _flash) = Smoke::boot(menu, arcade::BACKGROUND);
    core::mem::forget(alloc::vec![0u32; 2048]);
    let mut at_menu: Vec<usize> = Vec::new();
    let mut failure = None;
    for round in 0..2 {
        for (game, steer) in [
            (Game::Angler, 0.0),
            (Game::Spider, 24.0),
            (Game::Skate, 48.0),
            (Game::Bees, -48.0),
        ] {
            at_menu.push(internal_heap_used());
            smoke.wait(0.3);
            set_tilt(&mut smoke.app, steer, 0.0);
            smoke.frame();
            click_action(&mut smoke.app);
            let picked = smoke.app.world().resource::<Menu>().chosen();
            if picked != Some(game) || smoke.app.should_exit().is_none() {
                failure = Some("the menu didn't launch the game it pointed at");
                break;
            }
            let (add_game, background) = launch(game);
            smoke.relaunch(add_game, background, None, true);
            smoke.wait(1.0);
            let home = match game {
                Game::Angler => angler_game::ui::HOME.center(),
                Game::Spider => spider_game::ui::HOME.center(),
                Game::Skate => skate_game::ui::HOME.center(),
                Game::Bees => bees_game::ui::HOME.center(),
            };
            tap(&mut smoke.app, home);
            if smoke.app.should_exit().is_none() {
                failure = Some("HOME didn't end the game");
                break;
            }
            info!("round {round}: {game:?} launched and went home");
            smoke.relaunch(menu, arcade::BACKGROUND, None, false);
        }
    }
    let first = at_menu.first().copied().unwrap_or(0);
    let worst = at_menu.iter().copied().max().unwrap_or(0);
    info!(
        "internal heap back at the menu: {} KB first, {} KB worst over {} returns",
        first / 1024,
        worst / 1024,
        at_menu.len()
    );
    if failure.is_none() && worst > first + LEAK_SLACK {
        failure = Some("heap grew across app switches");
    }
    smoke.finish(failure)
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
