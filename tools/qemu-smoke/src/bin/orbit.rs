//! Orbit in QEMU: scripted touch plays several rounds.

#![no_std]
#![no_main]

use bevy::{math::ops, prelude::*};
use esp_backtrace as _;
use esp_hal::main;
use log::info;
use orbit_core::Touch;
use orbit_game::{BACKGROUND, OrbitPlugin, Screen, arena::SCREEN_CENTER};
use orbit_qemu_smoke::Smoke;

esp_bootloader_esp_idf::esp_app_desc!();

/// Game time to play. Emulated frames are much shorter than real ones, so
/// the script runs on game time, never frame counts.
const PLAY_SECS: f32 = 40.0;

#[main]
fn main() -> ! {
    let (mut smoke, _) = Smoke::boot(
        |app| {
            app.add_plugins(OrbitPlugin);
        },
        BACKGROUND,
    );
    let mut last_screen = Screen::Title;
    let (mut starts, mut losses) = (0, 0);
    loop {
        let secs = smoke.app.world().resource::<Time>().elapsed_secs();
        if secs >= PLAY_SECS {
            break;
        }
        smoke
            .app
            .world_mut()
            .resource_mut::<Touch>()
            .set(scripted_finger(secs));
        smoke.frame();
        let screen = *smoke.app.world().resource::<State<Screen>>().get();
        if screen != last_screen {
            info!("{secs:.1} s: {last_screen:?} -> {screen:?}");
            starts += u32::from(screen == Screen::Playing);
            losses += u32::from(screen == Screen::GameOver);
            last_screen = screen;
        }
    }
    let stalled = starts < 2 || losses < 1;
    smoke.finish(stalled.then_some("game flow stalled"))
}

/// Lifts the finger for the first second of every 15, then circles the rim:
/// the press starts (or retries) a round and the paddle sweeps around.
fn scripted_finger(secs: f32) -> Option<Vec2> {
    let phase = ops::rem_euclid(secs, 15.0);
    if phase < 1.0 {
        return None;
    }
    let angle = phase * 4.2;
    Some(SCREEN_CENTER + Vec2::new(ops::cos(angle), ops::sin(angle)) * 150.0)
}
