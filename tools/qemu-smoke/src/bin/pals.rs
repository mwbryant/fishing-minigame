//! Fishing Minigame in QEMU: the town walks to every door in turn, PWR goes in, the
//! place's app is built in the town's place, its way out (OUT, or PWR from
//! an empty tank, park or hall) ends it, and the town comes back at that
//! door, twice round. Internal heap after each return to the town must not
//! creep (no leak across apps), and nothing may spill into PSRAM.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;

use bevy::prelude::*;
use esp_backtrace as _;
use esp_hal::main;
use log::info;
use orbit_core::testing::*;
use orbit_gfx::Rgb565;
use orbit_qemu_smoke::{Smoke, internal_heap_used};
use pals_game::{
    Arrival, Place,
    town::{Town, TownPlugin},
};

esp_bootloader_esp_idf::esp_app_desc!();

/// Heap a return to the town may differ by before it counts as a leak.
const LEAK_SLACK: usize = 2048;
const DOORS: [Place; 8] = [
    Place::Home,
    Place::Pond,
    Place::Park,
    Place::Contest,
    Place::Shop,
    Place::Den,
    Place::Arcade,
    Place::BeeTree,
];

#[main]
fn main() -> ! {
    let (mut smoke, _flash) = Smoke::boot(
        |app| {
            app.add_plugins(TownPlugin);
        },
        pals_game::town::BACKGROUND,
    );
    core::mem::forget(alloc::vec![0u32; 2048]);
    let mut at_town: Vec<usize> = Vec::new();
    let mut failure = None;
    'tour: for round in 0..2 {
        for place in DOORS {
            at_town.push(internal_heap_used());
            smoke.wait(0.3);
            smoke.app.world_mut().resource_mut::<Town>().stand_at(place);
            smoke.frame();
            click_action(&mut smoke.app);
            let went = smoke.app.world().resource::<Town>().destination();
            if went != Some(place) || smoke.app.should_exit().is_none() {
                failure = Some("the door didn't lead to its place");
                break 'tour;
            }
            let (add_place, background) = launch(place);
            smoke.relaunch(add_place, background, None, true);
            smoke.wait(1.0);
            leave(&mut smoke, place);
            if smoke.app.should_exit().is_none() {
                failure = Some("couldn't get back out");
                break 'tour;
            }
            info!("round {round}: in and out of {place:?}");
            smoke.relaunch(
                move |app| {
                    app.add_plugins(TownPlugin).insert_resource(Arrival(place));
                },
                pals_game::town::BACKGROUND,
                None,
                false,
            );
            if smoke.app.world().resource::<Town>().near() != Some(place) {
                failure = Some("the town didn't pick up at the door");
                break 'tour;
            }
        }
    }
    let first = at_town.first().copied().unwrap_or(0);
    let worst = at_town.iter().copied().max().unwrap_or(0);
    info!(
        "internal heap back in town: {} KB first, {} KB worst over {} returns",
        first / 1024,
        worst / 1024,
        at_town.len()
    );
    if failure.is_none() && worst > first + LEAK_SLACK {
        failure = Some("heap grew across app switches");
    }
    smoke.finish(failure)
}

/// Each place's way back out to the street, with nobody in the tank.
fn leave(smoke: &mut Smoke, place: Place) {
    match place {
        Place::Home | Place::Park | Place::Contest => click_action(&mut smoke.app),
        Place::Shop => {
            set_tilt(
                &mut smoke.app,
                pals_game::shop::hat_turn(pals_game::shop::OUT),
                0.0,
            );
            smoke.frame();
            click_action(&mut smoke.app);
        }
        Place::Pond => tap(&mut smoke.app, angler_game::ui::HOME.center()),
        Place::Den => tap(&mut smoke.app, spider_game::ui::HOME.center()),
        Place::Arcade => tap(&mut smoke.app, skate_game::ui::HOME.center()),
        Place::BeeTree | Place::Settings => tap(&mut smoke.app, bees_game::ui::HOME.center()),
    }
}

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
        Place::BeeTree | Place::Settings => (
            |app| {
                app.add_plugins(bees_game::BeesPlugin);
            },
            bees_game::BACKGROUND,
        ),
    }
}
