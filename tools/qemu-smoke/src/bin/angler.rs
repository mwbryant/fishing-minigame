//! Angler in QEMU: scripted touch visits settings (saving a difficulty
//! change) and the GUTEN easter egg, then button and tilt input casts onto the
//! spot and fights with a steady hand. The first catch is kept and named by
//! turning to each letter, the second released. Every save goes through
//! `esp-storage` into the emulated flash, and the final log, settings and
//! tank are read back.

#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;

use angler_game::{
    AnglerPlugin, BACKGROUND, Difficulty, FishLog, Phase, Session, Settings, cast, save,
    session::RESULT_GUARD_SECS,
    testing::{click_action, hold_action, phase, release_action, set_tilt, tap},
    ui,
};
use bevy::prelude::*;
use esp_backtrace as _;
use esp_hal::main;
use esp_storage::FlashStorage;
use log::info;
use orbit_core::SaveBlob;
use orbit_qemu_smoke::Smoke;
use orbit_save::SlotStore;
use orbit_ui::{Key, key_turn};
use pals::Pals;

esp_bootloader_esp_idf::esp_app_desc!();

const CATCHES: u32 = 2;
/// Keep in sync with firmware/src/lib.rs.
const SAVE_FLASH_OFFSET: u32 = 0x9000;

#[main]
fn main() -> ! {
    let (mut smoke, flash) = Smoke::boot(
        |app| {
            app.add_plugins(AnglerPlugin);
        },
        BACKGROUND,
    );
    // Hold the 8 KB I2S stream buffer the firmware's speaker takes from the heap.
    core::mem::forget(vec![0u32; 2048]);
    let mut saves = SlotStore::new(FlashStorage::new(flash), SAVE_FLASH_OFFSET);

    tap(&mut smoke.app, ui::SETTINGS.center());
    tap(&mut smoke.app, ui::HARDER.center());
    persist(&mut smoke, &mut saves);
    tap(&mut smoke.app, ui::GUTEN.center());
    wait(&mut smoke, 2.0);
    info!("guten: {:?} after 2 s", phase(&smoke.app));
    tap(&mut smoke.app, Vec2::splat(233.0));
    tap(&mut smoke.app, ui::EASIER.center());
    persist(&mut smoke, &mut saves);
    tap(&mut smoke.app, ui::BACK.center());
    click_action(&mut smoke.app);

    let mut landed = 0;
    for attempt in 1..=6 {
        charge(&mut smoke, 0.7);
        release_action(&mut smoke.app);
        // Emulated frames don't take real-time durations, so the charge isn't
        // exactly 0.7: put the fish where this cast will actually land.
        if let Phase::Flight { flight, .. } = phase(&smoke.app) {
            smoke.app.world_mut().resource_mut::<Session>().spot_x = flight.landing_x();
        }
        if !run(&mut smoke, 15.0, |p| matches!(p, Phase::Reel { .. })) {
            info!("cast {attempt}: no bite");
            continue;
        }
        fight(&mut smoke);
        let outcome = phase(&smoke.app);
        match &outcome {
            Phase::Caught { catch, .. } => info!(
                "cast {attempt}: landed a {} ({} mm, {} g)",
                catch.species().name,
                catch.length_mm,
                catch.weight_g
            ),
            other => info!("cast {attempt}: fight ended in {other:?}"),
        }
        if let Phase::Caught { .. } = outcome {
            landed += 1;
            persist(&mut smoke, &mut saves);
        }
        wait(&mut smoke, RESULT_GUARD_SECS + 0.2);
        if landed == 1 && matches!(phase(&smoke.app), Phase::Caught { .. }) {
            click_action(&mut smoke.app);
            for key in [
                Key::Letter('Q'),
                Key::Letter('E'),
                Key::Letter('M'),
                Key::Done,
            ] {
                set_tilt(&mut smoke.app, key_turn(key), 0.0);
                smoke.frame();
                click_action(&mut smoke.app);
            }
            persist(&mut smoke, &mut saves);
            wait(&mut smoke, 2.0);
        } else {
            set_tilt(&mut smoke.app, 25.0, 0.0);
            smoke.frame();
            click_action(&mut smoke.app);
            set_tilt(&mut smoke.app, 0.0, 0.0);
        }
        if landed == CATCHES {
            break;
        }
    }

    let expected = (
        smoke.app.world().resource::<FishLog>().clone(),
        *smoke.app.world().resource::<Settings>(),
    );
    let tank = smoke.app.world().resource::<Pals>().clone();
    let stored = saves.load();
    let reloaded = stored.as_deref().and_then(save::decode_stored);
    let reloaded_tank = pals::save::load(&SaveBlob::from_storage(stored));
    info!(
        "landed {landed}; log total {}; tank {:?}; flash holds {:?}, tank of {}",
        expected.0.total,
        tank.pets
            .iter()
            .map(|pet| pet.name.as_str())
            .collect::<alloc::vec::Vec<_>>(),
        reloaded
            .as_ref()
            .map(|(log, settings)| (log.total, settings.difficulty)),
        reloaded_tank.pets.len()
    );
    let failure = if landed < CATCHES {
        Some("could not land the fish")
    } else if expected.1.difficulty != Difficulty::Normal {
        Some("settings taps did not land on the buttons")
    } else if tank.pets.len() != 1 || tank.pets[0].name.as_str() != "QEM" {
        Some("the kept fish wasn't named QEM")
    } else if reloaded != Some(expected) || reloaded_tank != tank {
        Some("save did not round-trip through flash")
    } else {
        None
    };
    smoke.finish(failure)
}

/// Steps frames until `done` holds for the phase, up to `secs` of game time.
fn run(smoke: &mut Smoke, secs: f32, done: impl Fn(&Phase) -> bool) -> bool {
    let until = smoke.app.world().resource::<Time>().elapsed_secs() + secs;
    while smoke.app.world().resource::<Time>().elapsed_secs() < until {
        if done(&phase(&smoke.app)) {
            return true;
        }
        smoke.frame();
    }
    done(&phase(&smoke.app))
}

/// Holds the action button until the power meter reaches `power`.
fn charge(smoke: &mut Smoke, power: f32) {
    hold_action(&mut smoke.app, 1);
    while let Phase::Aim {
        charging_secs: Some(secs),
    } = phase(&smoke.app)
    {
        if cast::charge_power(secs) >= power {
            return;
        }
        smoke.frame();
    }
}

/// Writes any save the game asked for, as the firmware does after an update.
fn persist(smoke: &mut Smoke, saves: &mut SlotStore<FlashStorage<'static>>) {
    let pending = smoke
        .app
        .world_mut()
        .resource_mut::<SaveBlob>()
        .take_pending();
    if let Some(bytes) = pending
        && let Err(err) = saves.save(&bytes)
    {
        info!("flash save failed: {err:?}");
    }
}

/// Steps frames for `secs` of game time.
fn wait(smoke: &mut Smoke, secs: f32) {
    let until = smoke.app.world().resource::<Time>().elapsed_secs() + secs;
    while smoke.app.world().resource::<Time>().elapsed_secs() < until {
        smoke.frame();
    }
}

/// Leans like a steady player until the fight ends (or a minute passes).
fn fight(smoke: &mut Smoke) {
    let until = smoke.app.world().resource::<Time>().elapsed_secs() + 60.0;
    while smoke.app.world().resource::<Time>().elapsed_secs() < until {
        let Phase::Reel { reel, .. } = phase(&smoke.app) else {
            return;
        };
        let push =
            (((reel.zone_y - reel.fish_y) * 30.0 + reel.zone_v * 3.6) / 1100.0).clamp(-1.0, 1.0);
        set_tilt(
            &mut smoke.app,
            0.0,
            push.signum() * (2.0 + push.abs() * 20.0),
        );
        smoke.frame();
    }
}
