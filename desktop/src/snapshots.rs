//! Scripted, headless runs that save one PNG per screen, rendered through the
//! same presenter and panel model as the live window.

use std::{fs, path::Path};

use angler_game::{
    Phase, Session,
    cast::{self, CHARGE_SECS, Flight},
    testing as angler, ui,
};
use bevy::prelude::*;
use orbit_core::testing as core_testing;
use orbit_core::{Battery, Buttons, Frame, Launcher, Power, SaveBlob};
use orbit_game::{
    Screen,
    arena::SCREEN_CENTER,
    testing::{headless_app, press, tap, tick},
};
use orbit_gfx::{Presenter, Rgb565};

use crate::{BAND_BYTES, Game, panel::RamPanel};

/// Draws the app's current frame through the presenter and saves it as a PNG.
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

/// Plays `game`'s script, saving a PNG of each screen in `dir`.
pub fn run(game: Game, dir: &Path) {
    fs::create_dir_all(dir).expect("create snapshot dir");
    let mut camera = Camera {
        dir,
        panel: RamPanel::new(),
        presenter: Presenter::new(vec![0u8; BAND_BYTES], game.background()),
    };
    match game {
        Game::Town => town(&mut camera),
        Game::Home => home(&mut camera),
        Game::Park => park(&mut camera),
        Game::Contest => contest(&mut camera),
        Game::Shop => shop(&mut camera),
        Game::Orbit => orbit(&mut camera),
        Game::Angler => angler(&mut camera),
        Game::Spider => spider(&mut camera),
        Game::Skate => skate(&mut camera),
        Game::Bees => bees(&mut camera),
    }
}

/// The town: the clearing at several of its doors, a walk, then the settings
/// and the reset question.
fn town(camera: &mut Camera) {
    use pals_game::{
        Place,
        town::{Town, TownPlugin, ui},
    };
    let mut tank = pals::Pals::default();
    tank.earn(135);
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(TownPlugin);
        },
        blob.take_pending(),
    );
    battery(&mut app, 76);
    camera.shoot(&app, "town_home");
    for (place, name) in [
        (Place::Pond, "town_pond"),
        (Place::Park, "town_park"),
        (Place::Contest, "town_hall"),
        (Place::Shop, "town_main_street"),
        (Place::Arcade, "town_grove"),
    ] {
        app.world_mut().resource_mut::<Town>().stand_at(place);
        core_testing::tick(&mut app, 2);
        camera.shoot(&app, name);
    }
    app.world_mut().resource_mut::<Town>().stand_at(Place::Home);
    core_testing::tap(&mut app, Vec2::new(330.0, 260.0));
    core_testing::tick(&mut app, 30);
    camera.shoot(&app, "town_walking");
    app.world_mut()
        .resource_mut::<Town>()
        .stand_at(Place::Settings);
    core_testing::tick(&mut app, 1);
    core_testing::click_action(&mut app);
    camera.shoot(&app, "town_settings");
    core_testing::tap(&mut app, ui::RESET.center());
    camera.shoot(&app, "town_reset");
}

/// The house: the tank, a card, the tricks list, teaching and practice.
fn home(camera: &mut Camera) {
    use pals_game::home::{Home, HomePlugin};
    let mut tank = pals::Pals::default();
    tank.earn(500);
    for hat in [2, 6] {
        tank.buy_hat(hat);
    }
    for (name, species, hat, walked) in [
        ("GILLY", 3, Some(6), 520.0),
        ("BUBBA", 1, None, 80.0),
        ("KOI", 7, Some(2), 1500.0),
    ] {
        let mut fish = pet(name, species);
        fish.hat = hat;
        fish.walk(walked);
        fish.teach(
            0,
            pals::Trigger {
                gesture: Some(pals::Gesture::SwipeUp),
                voice: None,
            },
        );
        fish.tricks[0].learned = 45;
        tank.keep(fish).ok();
    }
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(HomePlugin);
        },
        blob.take_pending(),
    );
    battery(&mut app, 76);
    core_testing::tick(&mut app, 40);
    camera.shoot(&app, "home_tank");
    core_testing::click_action(&mut app);
    camera.shoot(&app, "home_card");
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 12.0, 0.0);
    core_testing::tick(&mut app, 2);
    camera.shoot(&app, "home_tricks");
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 0.0, 0.0);
    core_testing::tick(&mut app, 50);
    camera.shoot(&app, "home_watch");
    core_testing::tick(&mut app, 200);
    camera.shoot(&app, "home_listen");
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 0.0, 0.0);
    core_testing::tick(&mut app, 2);
    core_testing::click_action(&mut app);
    camera.shoot(&app, "home_practice");
    app.world_mut().resource_mut::<Home>().rig_next_try(true);
    for i in 0..=10 {
        app.world_mut()
            .resource_mut::<orbit_core::Touch>()
            .set(Some(Vec2::new(233.0, 330.0 - i as f32 * 12.0)));
        app.update();
    }
    app.world_mut()
        .resource_mut::<orbit_core::Touch>()
        .set(None);
    core_testing::tick(&mut app, 50);
    camera.shoot(&app, "home_trying");
    core_testing::tick(&mut app, 60);
    camera.shoot(&app, "home_praise");
    core_testing::tap(&mut app, Vec2::new(233.0, 230.0));
    camera.shoot(&app, "home_good_fish");

    for (trick, at_secs, name) in [
        ("BUBBLE RING", 0.7, "home_bubble_ring"),
        ("SPLASH", 0.3, "home_splash"),
    ] {
        let mut fish = pet("GILLY", 3);
        fish.tricks[0].trick = pals::TRICKS.iter().position(|t| t.name == trick).unwrap() as u8;
        let mut tank = pals::Pals::default();
        tank.keep(fish).ok();
        let mut blob = SaveBlob::from_storage(None);
        pals::save::store(&mut blob, &tank);
        let mut app = core_testing::headless(
            |app| {
                app.add_plugins(HomePlugin);
            },
            blob.take_pending(),
        );
        battery(&mut app, 76);
        for _ in 0..3 {
            core_testing::click_action(&mut app);
        }
        core_testing::tick(&mut app, (at_secs * 60.0) as u32);
        camera.shoot(&app, name);
    }
}

/// The park: picking a walker, a mock walk, levelling up, the summary.
fn park(camera: &mut Camera) {
    use pals_game::walk::WalkPlugin;
    let mut tank = pals::Pals::default();
    tank.mock_walk = true;
    tank.keep({
        let mut fish = pet("GILLY", 3);
        fish.walk(380.0);
        fish
    })
    .ok();
    tank.keep(pet("BUBBA", 1)).ok();
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(WalkPlugin);
        },
        blob.take_pending(),
    );
    battery(&mut app, 76);
    camera.shoot(&app, "park_choose");
    core_testing::click_action(&mut app);
    core_testing::tick(&mut app, 60 * 8);
    camera.shoot(&app, "park_walking");
    core_testing::tick(&mut app, 60 * 12);
    camera.shoot(&app, "park_level_up");
    for _ in 0..2 {
        core_testing::tap(&mut app, pals_game::walk::ui::FASTER.center());
    }
    core_testing::tick(&mut app, 60 * 2);
    camera.shoot(&app, "park_fast_forward");
    core_testing::click_action(&mut app);
    camera.shoot(&app, "park_done");

    let mut tank = pals::Pals::default();
    tank.keep(pet("GILLY", 3)).ok();
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(WalkPlugin);
        },
        blob.take_pending(),
    );
    battery(&mut app, 76);
    core_testing::click_action(&mut app);
    for _ in 0..3 {
        app.world_mut().resource_mut::<orbit_core::Steps>().add(48);
        core_testing::tick(&mut app, 30);
    }
    camera.shoot(&app, "park_stepping");
    core_testing::tick(&mut app, 60 * 5);
    camera.shoot(&app, "park_dozing");
}

/// The contest hall: the tiers, a show with a judge's call, the results.
fn contest(camera: &mut Camera) {
    use pals_game::contest::{Contest, ContestPlugin, SHOW_SECS};
    let mut star = pet("STAR", 7);
    star.walk(450.0);
    star.hat = Some(11);
    for (slot, gesture) in [(0, pals::Gesture::SwipeUp), (1, pals::Gesture::SwipeDown)] {
        star.teach(
            slot,
            pals::Trigger {
                gesture: Some(gesture),
                voice: None,
            },
        );
        star.tricks[slot].learned = 80;
    }
    let mut tank = pals::Pals::default();
    tank.earn(1000);
    tank.buy_hat(11);
    tank.keep(star).ok();
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(ContestPlugin);
        },
        blob.take_pending(),
    );
    battery(&mut app, 76);
    camera.shoot(&app, "contest_choose");
    core_testing::click_action(&mut app);
    camera.shoot(&app, "contest_tiers");
    core_testing::click_action(&mut app);
    camera.shoot(&app, "contest_go");
    app.world_mut()
        .resource_mut::<Contest>()
        .rig_rivals([60, 45, 30]);
    app.world_mut().resource_mut::<Contest>().rig_request(0);
    app.world_mut().resource_mut::<Contest>().rig_next_try(true);
    for i in 0..=10 {
        app.world_mut()
            .resource_mut::<orbit_core::Touch>()
            .set(Some(Vec2::new(233.0, 330.0 - i as f32 * 12.0)));
        app.update();
    }
    app.world_mut()
        .resource_mut::<orbit_core::Touch>()
        .set(None);
    core_testing::tick(&mut app, 40);
    camera.shoot(&app, "contest_trick");
    core_testing::tick(&mut app, 70);
    camera.shoot(&app, "contest_judges_love_it");
    core_testing::tick(&mut app, (SHOW_SECS * 60.0) as u32);
    camera.shoot(&app, "contest_results");
}

/// The hat shop: the ring of hats and the fish trying them on.
fn shop(camera: &mut Camera) {
    use pals_game::shop::{ShopPlugin, hat_turn};
    let mut tank = pals::Pals::default();
    tank.earn(260);
    tank.keep(pet("GILLY", 3)).ok();
    tank.keep(pet("KOI", 7)).ok();
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(ShopPlugin);
        },
        blob.take_pending(),
    );
    battery(&mut app, 76);
    core_testing::set_tilt(&mut app, hat_turn(7), 0.0);
    core_testing::tick(&mut app, 2);
    camera.shoot(&app, "shop_clown");
    core_testing::click_action(&mut app);
    camera.shoot(&app, "shop_bought");
    core_testing::set_tilt(&mut app, hat_turn(12), 0.0);
    core_testing::tick(&mut app, 2);
    camera.shoot(&app, "shop_crown");
}

/// Spider Roulette: a round up to the spider's bite, then a bet cashed out.
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

    let mut tank = pals::Pals::default();
    tank.earn(120);
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(spider_game::SpiderPlugin);
        },
        blob.take_pending(),
    );
    battery(&mut app, 76);
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 2.0 * 360.0 / 6.0 / orbit_ui::DIAL_GAIN, 0.0);
    core_testing::tick(&mut app, 2);
    camera.shoot(&app, "spider_bet");
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 0.0, 0.0);
    app.world_mut().resource_mut::<Table>().hide_spider(9);
    for egg in [1, 4, 7] {
        app.world_mut().resource_mut::<Table>().rig_next_spin(egg);
        core_testing::click_action(&mut app);
        core_testing::tick(&mut app, 60 * 4);
    }
    camera.shoot(&app, "spider_worth");
    core_testing::tap(&mut app, spider_game::ui::CASH_OUT.center());
    camera.shoot(&app, "spider_cashed_out");

    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(spider_game::SpiderPlugin)
                .insert_resource(Launcher);
        },
        blob.take_pending(),
    );
    battery(&mut app, 76);
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, -360.0 / 7.0 / orbit_ui::DIAL_GAIN, 0.0);
    core_testing::tick(&mut app, 2);
    camera.shoot(&app, "spider_bet_from_town");
}

/// Skate: an ollie, a grind and a bail, then the halfpipe and a 360.
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

    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(skate_game::SkatePlugin);
        },
        None,
    );
    core_testing::click_action(&mut app);
    app.world_mut().resource_mut::<Ride>().approach_halfpipe();
    core_testing::tick(&mut app, 20);
    camera.shoot(&app, "skate_halfpipe");
    while !app.world().resource::<Ride>().halfpipe_air() {
        app.update();
    }
    core_testing::set_tilt(&mut app, 30.0, 0.0);
    core_testing::tick(&mut app, 18);
    camera.shoot(&app, "skate_halfpipe_air");
    core_testing::set_tilt(&mut app, 60.0, 0.0);
    while app.world().resource::<Ride>().halfpipe_air() {
        app.update();
    }
    core_testing::tick(&mut app, 6);
    camera.shoot(&app, "skate_halfpipe_360");
}

/// Bees??? with the lead fish in the cowboy hat, so the bees wear it too.
fn bees(camera: &mut Camera) {
    use bees_game::{CONTROL_GAIN, Orbit};
    let mut tank = pals::Pals::default();
    tank.earn(1000);
    let cowboy = pals::HATS
        .iter()
        .position(|hat| hat.name == "COWBOY HAT")
        .unwrap_or(0);
    tank.buy_hat(cowboy);
    tank.keep(pet("COWBOY", 4)).ok();
    tank.wear(0, Some(cowboy));
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
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

/// Orbit: the title, a rally and the game over.
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
pub(crate) fn battery(app: &mut App, percent: u8) {
    app.world_mut().resource_mut::<Power>().battery = Some(Battery {
        percent: Some(percent),
        external_power: false,
        charging: false,
    });
    app.update();
}

/// A pet named `name` of species `species`, for scripted saves.
pub(crate) fn pet(name: &str, species: u8) -> pals::Pet {
    let mut typed = pals::Name::default();
    name.chars().for_each(|letter| {
        typed.push(letter);
    });
    let tier = angler_game::SPECIES[usize::from(species)].tier;
    pals::Pet::new(species, tier, 300, 60, typed, &mut orbit_core::Rng::new(7))
}

/// Casts onto the fishing spot and plays the fight to a catch, shooting each
/// step when `camera` is given.
fn land_a_fish(app: &mut App, mut camera: Option<&mut Camera>) {
    let mut shoot = |app: &App, name: &str| {
        if let Some(camera) = camera.as_deref_mut() {
            camera.shoot(app, name);
        }
    };
    angler::click_action(app);
    angler::set_tilt(app, -12.0, 0.0);
    let dots_end = Flight::launch(cast::elevation_deg(-12.0), 1.0).landing_x();
    app.world_mut().resource_mut::<Session>().spot_x = dots_end;
    angler::hold_action(app, (CHARGE_SECS * 60.0) as u32 + 1);
    shoot(app, "angler_aim");
    angler::release_action(app);
    angler::tick(app, 20);
    shoot(app, "angler_cast");
    angler::run_until(app, 600, |p| matches!(p, Phase::Waiting { .. }));
    angler::tick(app, 12);
    shoot(app, "angler_waiting");
    angler::run_until(app, 60 * 15, |p| matches!(p, Phase::Bite { .. }));
    angler::tick(app, 5);
    shoot(app, "angler_bite");
    angler::run_until(app, 120, |p| matches!(p, Phase::Reel { .. }));
    reel(app, 70);
    shoot(app, "angler_reel");
    reel(app, 60 * 60);
    angler::tick(app, 60);
}

/// Angler: a catch from aim to tank, the log, settings, GUTEN, hold to sleep,
/// and a catch into a full tank.
fn angler(camera: &mut Camera) {
    use orbit_ui::{Key, key_turn};
    let mut app = angler::headless_app();
    battery(&mut app, 76);
    camera.shoot(&app, "angler_title");
    land_a_fish(&mut app, Some(camera));
    camera.shoot(&app, "angler_caught");
    angler::click_action(&mut app);
    for letter in "BUBBA".chars() {
        angler::set_tilt(&mut app, key_turn(Key::Letter(letter)), 0.0);
        angler::tick(&mut app, 1);
        angler::click_action(&mut app);
    }
    angler::set_tilt(&mut app, key_turn(Key::Done), 0.0);
    angler::tick(&mut app, 2);
    camera.shoot(&app, "angler_naming");
    angler::click_action(&mut app);
    camera.shoot(&app, "angler_in_the_tank");

    let saved = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    let mut app = angler::headless_app_with_save(saved);
    battery(&mut app, 76);
    angler::tap(&mut app, ui::LOG.center());
    camera.shoot(&app, "angler_log");

    angler::tap(&mut app, Vec2::splat(233.0));
    angler::tap(&mut app, ui::SETTINGS.center());
    camera.shoot(&app, "angler_settings");
    angler::tap(&mut app, ui::GUTEN.center());
    angler::tick(&mut app, 8);
    camera.shoot(&app, "angler_guten");
    angler::tick(&mut app, 40);
    angler::tap(&mut app, Vec2::splat(233.0));
    angler::tap(&mut app, ui::BACK.center());
    app.world_mut().resource_mut::<Buttons>().zero.set(true);
    angler::tick(&mut app, 50);
    camera.shoot(&app, "angler_hold_to_sleep");
    angler::tick(&mut app, 140);
    camera.shoot(&app, "angler_good_night");

    let mut full = pals::Pals::default();
    for (name, species) in [("BUBBA", 1), ("GILLY", 3), ("KOI", 7)] {
        full.keep(pet(name, species)).ok();
    }
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &full);
    let mut app = angler::headless_app_with_save(blob.take_pending());
    land_a_fish(&mut app, None);
    angler::click_action(&mut app);
    angler::set_tilt(&mut app, 30.0, 0.0);
    angler::tick(&mut app, 2);
    camera.shoot(&app, "angler_swap");
}

/// Plays the fight with a steady hand for up to `frames` updates.
pub(crate) fn reel(app: &mut App, frames: u32) {
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
            Game::Town => pals_game::town::BACKGROUND,
            Game::Home => pals_game::home::BACKGROUND,
            Game::Park => pals_game::walk::BACKGROUND,
            Game::Contest => pals_game::contest::BACKGROUND,
            Game::Shop => pals_game::shop::BACKGROUND,
            Game::Orbit => orbit_game::BACKGROUND,
            Game::Angler => angler_game::BACKGROUND,
            Game::Spider => spider_game::BACKGROUND,
            Game::Skate => skate_game::BACKGROUND,
            Game::Bees => bees_game::BACKGROUND,
        }
    }
}
