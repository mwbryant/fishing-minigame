//! Angler in QEMU: scripted button and tilt input casts onto the spot, fights
//! with a steady hand, and saves each catch through `esp-storage` into the
//! emulated flash, then reads the log back.

#![no_std]
#![no_main]

use angler_game::{
    AnglerPlugin, BACKGROUND, FishLog, Phase, Session, cast,
    session::RESULT_GUARD_SECS,
    testing::{click_action, hold_action, phase, release_action, set_tilt},
};
use bevy::prelude::*;
use esp_backtrace as _;
use esp_hal::main;
use esp_storage::FlashStorage;
use log::info;
use orbit_core::SaveBlob;
use orbit_qemu_smoke::Smoke;
use orbit_save::SlotStore;

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
    let mut saves = SlotStore::new(FlashStorage::new(flash), SAVE_FLASH_OFFSET);
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
            let bytes = smoke
                .app
                .world_mut()
                .resource_mut::<SaveBlob>()
                .take_pending();
            if let Some(bytes) = bytes
                && let Err(err) = saves.save(&bytes)
            {
                info!("flash save failed: {err:?}");
            }
        }
        wait(&mut smoke, RESULT_GUARD_SECS + 0.2);
        click_action(&mut smoke.app);
        if landed == CATCHES {
            break;
        }
    }

    let expected = smoke.app.world().resource::<FishLog>().clone();
    let reloaded = saves.load().and_then(|bytes| FishLog::decode(&bytes));
    info!(
        "landed {landed}; log total {}; flash holds total {:?}",
        expected.total,
        reloaded.as_ref().map(|log| log.total)
    );
    let failure = if landed < CATCHES {
        Some("could not land the fish")
    } else if reloaded != Some(expected) {
        Some("fish log did not round-trip through flash")
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
