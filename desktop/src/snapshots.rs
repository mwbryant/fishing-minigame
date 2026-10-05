//! Scripted, headless runs that save one PNG per screen, rendered through the
//! same presenter and panel model as the live window.

use std::{fs, path::Path};

use angler_game::{
    Phase, Session,
    cast::{self, CHARGE_SECS, Flight},
    testing as angler,
};
use bevy::prelude::*;
use orbit_core::{Frame, SaveBlob};
use orbit_game::{
    Screen,
    arena::SCREEN_CENTER,
    testing::{headless_app, press, tap, tick},
};
use orbit_gfx::{Presenter, Rgb565};

use crate::{BAND_BYTES, Game, panel::RamPanel};

struct Camera<'a> {
    dir: &'a Path,
    panel: RamPanel,
    presenter: Presenter<Vec<u8>>,
}

impl Camera<'_> {
    fn shoot(&mut self, app: &App, name: &str) {
        self.presenter
            .present(&app.world().resource::<Frame>().shapes, &mut self.panel);
        let path = self.dir.join(format!("{name}.png"));
        self.panel.save_png(&path);
        println!("wrote {}", path.display());
    }
}

pub fn run(game: Game, dir: &Path) {
    fs::create_dir_all(dir).expect("create snapshot dir");
    let mut camera = Camera {
        dir,
        panel: RamPanel::new(),
        presenter: Presenter::new(vec![0u8; BAND_BYTES], game.background()),
    };
    match game {
        Game::Orbit => orbit(&mut camera),
        Game::Angler => angler(&mut camera),
    }
}

fn orbit(camera: &mut Camera) {
    let mut app = headless_app();
    camera.shoot(&app, "orbit_title");
    tap(&mut app, SCREEN_CENTER);
    press(&mut app, Some(SCREEN_CENTER + Vec2::new(-60.0, 90.0)));
    tick(&mut app, 70);
    camera.shoot(&app, "orbit_playing");
    press(&mut app, None);
    while *app.world().resource::<State<Screen>>().get() != Screen::GameOver {
        tick(&mut app, 1);
    }
    tick(&mut app, 30);
    camera.shoot(&app, "orbit_game_over");
}

fn angler(camera: &mut Camera) {
    let mut app = angler::headless_app();
    camera.shoot(&app, "angler_title");

    angler::click_action(&mut app);
    angler::set_tilt(&mut app, -12.0, 0.0);
    let power = 0.72;
    let lands = Flight::launch(cast::elevation_deg(-12.0), power).landing_x();
    app.world_mut().resource_mut::<Session>().spot_x = lands + 6.0;
    angler::hold_action(&mut app, (power * CHARGE_SECS * 60.0) as u32 + 1);
    camera.shoot(&app, "angler_aim");
    angler::release_action(&mut app);
    angler::tick(&mut app, 30);
    camera.shoot(&app, "angler_cast");
    angler::run_until(&mut app, 600, |p| matches!(p, Phase::Waiting { .. }));
    angler::tick(&mut app, 12);
    camera.shoot(&app, "angler_waiting");
    angler::run_until(&mut app, 60 * 15, |p| matches!(p, Phase::Bite { .. }));
    angler::tick(&mut app, 5);
    camera.shoot(&app, "angler_bite");
    angler::run_until(&mut app, 120, |p| matches!(p, Phase::Reel { .. }));
    reel(&mut app, 70);
    camera.shoot(&app, "angler_reel");
    reel(&mut app, 60 * 60);
    angler::tick(&mut app, 60);
    camera.shoot(&app, "angler_caught");

    let saved = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    let mut app = angler::headless_app_with_save(saved);
    angler::tap(&mut app, Vec2::splat(233.0));
    camera.shoot(&app, "angler_log");
}

/// Plays the fight with a steady hand for up to `frames` updates.
fn reel(app: &mut App, frames: u32) {
    for _ in 0..frames {
        let Phase::Reel { reel, .. } = angler::phase(app) else {
            return;
        };
        let push =
            (((reel.zone_y - reel.fish_y) * 30.0 + reel.zone_v * 3.6) / 1100.0).clamp(-1.0, 1.0);
        angler::set_tilt(app, 0.0, push.signum() * (2.0 + push.abs() * 20.0));
        app.update();
    }
}

impl Game {
    pub fn background(self) -> Rgb565 {
        match self {
            Game::Orbit => orbit_game::BACKGROUND,
            Game::Angler => angler_game::BACKGROUND,
        }
    }
}
