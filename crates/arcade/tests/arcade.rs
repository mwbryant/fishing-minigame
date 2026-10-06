use arcade::{ArcadePlugin, Game, Menu, ui};
use bevy::{app::AppExit, prelude::*};
use orbit_core::{SaveBlob, SoundQueue, preferences, testing::*};

fn menu() -> App {
    headless(
        |app| {
            app.add_plugins(ArcadePlugin);
        },
        None,
    )
}

fn chosen(app: &App) -> Option<Game> {
    app.world().resource::<Menu>().chosen()
}

fn turn(app: &mut App, steer_deg: f32) {
    set_tilt(app, steer_deg, 0.0);
    app.update();
}

#[test]
fn holding_level_points_at_angler_and_turning_sweeps_the_dial() {
    let mut app = menu();
    assert!(shows(&app, "ANGLER") && shows(&app, "PRESS PWR"));
    turn(&mut app, 30.0);
    assert!(
        shows(&app, "SPIDER") && shows(&app, "ROULETTE"),
        "{:?}",
        labels(&app)
    );
    turn(&mut app, 60.0);
    assert!(shows(&app, "SKATE"));
    turn(&mut app, -30.0);
    assert!(shows(&app, "SETTINGS"));
    assert_eq!(chosen(&app), None, "turning alone launches nothing");
}

#[test]
fn pwr_launches_the_game_the_pointer_is_on_and_ends_the_menu() {
    let mut app = menu();
    turn(&mut app, 30.0);
    hold_action(&mut app, 1);
    assert_eq!(chosen(&app), Some(Game::Spider));
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

#[test]
fn tapping_a_game_launches_it_too() {
    let mut app = menu();
    app.world_mut()
        .resource_mut::<orbit_core::Touch>()
        .set(Some(ui::SKATE.center()));
    app.update();
    assert_eq!(chosen(&app), Some(Game::Skate));
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

fn open_settings(app: &mut App) {
    turn(app, -30.0);
    click_action(app);
    assert!(shows(app, "SOUND ON"), "{:?}", labels(app));
    assert_eq!(chosen(app), None, "settings is not a game");
}

#[test]
fn settings_mutes_the_sound_for_every_game_and_saves_it() {
    let mut app = menu();
    open_settings(&mut app);
    turn(&mut app, 0.0);
    click_action(&mut app);
    assert!(shows(&app, "SOUND OFF"));
    assert!(app.world().resource::<SoundQueue>().muted());
    let record = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    let saved = SaveBlob::from_storage(record);
    assert!(preferences::muted(&saved));
    let rebooted = headless(
        |app| {
            app.add_plugins(ArcadePlugin);
        },
        saved_record(saved),
    );
    assert!(rebooted.world().resource::<SoundQueue>().muted());
}

fn saved_record(mut blob: SaveBlob) -> Option<Vec<u8>> {
    blob.save_section(*b"TEST", Vec::new());
    blob.take_pending()
}

#[test]
fn tapping_sound_toggles_it_back_on() {
    let mut app = menu();
    open_settings(&mut app);
    tap(&mut app, ui::SOUND.center());
    tap(&mut app, ui::SOUND.center());
    assert!(shows(&app, "SOUND ON"));
    assert!(!app.world().resource::<SoundQueue>().muted());
}

#[test]
fn back_returns_to_the_dial() {
    let mut app = menu();
    open_settings(&mut app);
    turn(&mut app, 60.0);
    click_action(&mut app);
    assert!(shows(&app, "PRESS PWR"), "{:?}", labels(&app));
    open_settings(&mut app);
    tap(&mut app, ui::BACK.center());
    assert!(shows(&app, "PRESS PWR"));
}

#[test]
fn the_menu_shows_the_battery_like_every_game() {
    let mut app = menu();
    app.world_mut().resource_mut::<orbit_core::Power>().battery = Some(orbit_core::Battery {
        percent: Some(64),
        external_power: false,
        charging: false,
    });
    app.update();
    assert!(shows(&app, "64%"));
}
