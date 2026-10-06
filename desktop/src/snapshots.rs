//! Scripted, headless runs that save one PNG per screen, rendered through the
//! same presenter and panel model as the live window.

use std::{fs, path::Path};

use angler_game::{
    Inventory, Phase, Session,
    cast::{self, CHARGE_SECS, Flight},
    testing as angler, ui,
};
use arcade::ArcadePlugin;
use bevy::prelude::*;
use orbit_core::testing as core_testing;
use orbit_core::{Battery, Buttons, Frame, Power, SaveBlob};
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
        Game::Arcade => arcade(&mut camera),
        Game::Orbit => orbit(&mut camera),
        Game::Angler => angler(&mut camera),
        Game::Spider => spider(&mut camera),
        Game::Skate => skate(&mut camera),
        Game::Bees => bees(&mut camera),
    }
}

fn arcade(camera: &mut Camera) {
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(ArcadePlugin);
        },
        None,
    );
    battery(&mut app, 76);
    camera.shoot(&app, "arcade_angler");
    core_testing::set_tilt(&mut app, 30.0, 0.0);
    app.update();
    camera.shoot(&app, "arcade_spider");
    core_testing::set_tilt(&mut app, 48.0, 0.0);
    app.update();
    camera.shoot(&app, "arcade_skate");
    core_testing::set_tilt(&mut app, -48.0, 0.0);
    app.update();
    camera.shoot(&app, "arcade_bees");
    core_testing::set_tilt(&mut app, -30.0, 0.0);
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 4.0, 0.0);
    core_testing::click_action(&mut app);
    camera.shoot(&app, "arcade_settings");
}

fn spider(camera: &mut Camera) {
    use spider_game::Table;
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(spider_game::SpiderPlugin);
        },
        None,
    );
    battery(&mut app, 76);
    core_testing::tick(&mut app, 7);
    camera.shoot(&app, "spider_title");
    core_testing::click_action(&mut app);
    app.world_mut().resource_mut::<Table>().hide_spider(6);
    for egg in [0, 3, 8, 10] {
        app.world_mut().resource_mut::<Table>().rig_next_spin(egg);
        core_testing::click_action(&mut app);
        core_testing::tick(&mut app, 60 * 4);
    }
    core_testing::tick(&mut app, 9);
    camera.shoot(&app, "spider_pick");
    app.world_mut().resource_mut::<Table>().rig_next_spin(6);
    core_testing::click_action(&mut app);
    core_testing::tick(&mut app, 40);
    camera.shoot(&app, "spider_spin");
    core_testing::tick(&mut app, 140);
    camera.shoot(&app, "spider_pounce");
    core_testing::tick(&mut app, 120);
    camera.shoot(&app, "spider_bitten");
}

fn skate(camera: &mut Camera) {
    use skate_game::{Kind, POP_DEG, Ride, SKATER_X};
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(skate_game::SkatePlugin);
        },
        None,
    );
    battery(&mut app, 76);
    camera.shoot(&app, "skate_title");
    core_testing::click_action(&mut app);
    core_testing::tick(&mut app, 40);
    app.world_mut()
        .resource_mut::<Ride>()
        .place(Kind::Bench, SKATER_X + 90.0);
    core_testing::set_tilt(&mut app, -(POP_DEG + 8.0), 0.0);
    core_testing::tick(&mut app, 14);
    camera.shoot(&app, "skate_ollie");
    core_testing::set_tilt(&mut app, 0.0, 0.0);
    core_testing::tick(&mut app, 34);
    camera.shoot(&app, "skate_grind");
    core_testing::tick(&mut app, 60);
    app.world_mut()
        .resource_mut::<Ride>()
        .place(Kind::Cone, SKATER_X + 40.0);
    core_testing::tick(&mut app, 70);
    camera.shoot(&app, "skate_bailed");
}

/// Bees??? with the angler wearing the cowboy hat, so the bees do too.
fn bees(camera: &mut Camera) {
    use bees_game::{CONTROL_GAIN, Orbit};
    let mut inventory = angler_game::Inventory::default();
    inventory.earn(1000);
    let cowboy = angler_game::HATS
        .iter()
        .position(|hat| hat.name == "COWBOY HAT")
        .unwrap_or(0);
    inventory.buy(cowboy);
    let mut blob = SaveBlob::from_storage(None);
    blob.save_section(
        angler_game::save::KEY,
        angler_game::save::encode(
            &angler_game::FishLog::default(),
            &angler_game::Settings::default(),
            &inventory,
        ),
    );
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(bees_game::BeesPlugin);
        },
        blob.take_pending(),
    );
    battery(&mut app, 76);
    camera.shoot(&app, "bees_title");
    core_testing::click_action(&mut app);
    for _ in 0..60 * 3 {
        let steer = app.world().resource::<Orbit>().drift_deg / CONTROL_GAIN;
        core_testing::set_tilt(&mut app, steer, 0.0);
        app.update();
    }
    camera.shoot(&app, "bees_orbit");
    core_testing::set_tilt(&mut app, 60.0, 0.0);
    core_testing::tick(&mut app, 50);
    camera.shoot(&app, "bees_spilling");
    core_testing::tick(&mut app, 60 * 12);
    camera.shoot(&app, "bees_empty");
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

/// What the board's power chip would report, so the menus show a level.
fn battery(app: &mut App, percent: u8) {
    app.world_mut().resource_mut::<Power>().battery = Some(Battery {
        percent: Some(percent),
        external_power: false,
        charging: false,
    });
    app.update();
}

fn angler(camera: &mut Camera) {
    let mut app = angler::headless_app();
    battery(&mut app, 76);
    camera.shoot(&app, "angler_title");

    angler::click_action(&mut app);
    angler::set_tilt(&mut app, -12.0, 0.0);
    let dots_end = Flight::launch(cast::elevation_deg(-12.0), 1.0).landing_x();
    app.world_mut().resource_mut::<Session>().spot_x = dots_end;
    angler::hold_action(&mut app, (CHARGE_SECS * 60.0) as u32 + 1);
    camera.shoot(&app, "angler_aim");
    angler::release_action(&mut app);
    angler::tick(&mut app, 20);
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
    battery(&mut app, 76);
    angler::tap(&mut app, ui::LOG.center());
    camera.shoot(&app, "angler_log");

    angler::tap(&mut app, Vec2::splat(233.0));
    angler::tap(&mut app, ui::SETTINGS.center());
    camera.shoot(&app, "angler_settings");
    angler::tap(&mut app, ui::RESET.center());
    camera.shoot(&app, "angler_reset");
    angler::tap(&mut app, ui::KEEP.center());
    angler::tap(&mut app, ui::GUTEN.center());
    angler::tick(&mut app, 8);
    camera.shoot(&app, "angler_guten");

    angler::tick(&mut app, 40);
    angler::tap(&mut app, Vec2::splat(233.0));
    angler::tap(&mut app, ui::BACK.center());
    app.world_mut().resource_mut::<Inventory>().earn(120);
    angler::tap(&mut app, ui::SHOP.center());
    for _ in 0..4 {
        angler::tap(&mut app, ui::NEXT_HAT.center());
    }
    camera.shoot(&app, "angler_shop");
    angler::tap(&mut app, ui::BUY.center());
    angler::tap(&mut app, ui::BACK.center());
    camera.shoot(&app, "angler_title_hat");
    app.world_mut().resource_mut::<Buttons>().zero.set(true);
    angler::tick(&mut app, 50);
    camera.shoot(&app, "angler_hold_to_sleep");
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
            Game::Arcade => arcade::BACKGROUND,
            Game::Orbit => orbit_game::BACKGROUND,
            Game::Angler => angler_game::BACKGROUND,
            Game::Spider => spider_game::BACKGROUND,
            Game::Skate => skate_game::BACKGROUND,
            Game::Bees => bees_game::BACKGROUND,
        }
    }
}
