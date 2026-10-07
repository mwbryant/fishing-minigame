use bevy::{app::AppExit, prelude::*};
use orbit_core::{SaveBlob, SoundQueue, preferences, testing::*};
use pals::{Name, Pals, Pet};
use pals_game::{
    Arrival, Place,
    town::{Screen, Town, TownPhase, TownPlugin, ui},
};

fn town(saved: Option<Vec<u8>>) -> App {
    headless(
        |app| {
            app.add_plugins(TownPlugin);
        },
        saved,
    )
}

fn returning_from(place: Place) -> App {
    headless(
        |app| {
            app.add_plugins(TownPlugin).insert_resource(Arrival(place));
        },
        None,
    )
}

fn state(app: &App) -> &Town {
    app.world().resource::<Town>()
}

/// Walks with the device held at `steer` and `lean` for `frames` updates.
fn walk(app: &mut App, steer: f32, lean: f32, frames: u32) {
    set_tilt(app, steer, lean);
    tick(app, frames);
    set_tilt(app, 0.0, 0.0);
    app.update();
}

fn saved(app: &mut App) -> SaveBlob {
    SaveBlob::from_storage(app.world_mut().resource_mut::<SaveBlob>().take_pending())
}

fn a_tank_with_gold(gold: u32) -> Vec<u8> {
    let mut tank = Pals::default();
    tank.earn(gold);
    let mut name = Name::default();
    name.push('B');
    tank.keep(Pet::new(0, 0, 80, 50, name, &mut orbit_core::Rng::new(1)))
        .ok();
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    blob.take_pending().unwrap()
}

#[test]
fn the_town_boots_at_your_front_door_and_pwr_goes_inside() {
    let mut app = town(None);
    assert_eq!(state(&app).screen(), Screen::Home);
    assert_eq!(state(&app).near(), Some(Place::Home));
    assert!(shows(&app, "HOME"));
    click_action(&mut app);
    assert_eq!(state(&app).destination(), Some(Place::Home));
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

#[test]
fn turning_walks_left_and_right_and_leaning_walks_up_and_down() {
    let mut app = town(None);
    let start = state(&app).at();
    walk(&mut app, 25.0, 0.0, 30);
    let east = state(&app).at();
    assert!(east.x > start.x + 40.0 && (east.y - start.y).abs() < 2.0);
    walk(&mut app, 0.0, -25.0, 30);
    assert!(
        state(&app).at().y > east.y + 40.0,
        "leaning back walks down"
    );
    walk(&mut app, 0.0, 25.0, 30);
    walk(&mut app, -25.0, 0.0, 30);
    assert!(state(&app).at().distance(start) < 10.0);
    assert_eq!(state(&app).near(), Some(Place::Home));
}

#[test]
fn the_house_is_solid() {
    let mut app = town(None);
    let door = state(&app).at();
    walk(&mut app, 0.0, 25.0, 120);
    assert!(
        state(&app).at().y > door.y - 12.0,
        "walked into the house to {:?}",
        state(&app).at()
    );
}

#[test]
fn the_paths_lead_to_the_next_clearing() {
    let mut app = town(None);
    walk(&mut app, 25.0, 0.0, 60 * 4);
    assert_eq!(state(&app).screen(), Screen::Park, "east of home");
    walk(&mut app, -25.0, 0.0, 60 * 4);
    assert_eq!(state(&app).screen(), Screen::Home);
    walk(&mut app, -25.0, 0.0, 60 * 4);
    assert_eq!(state(&app).screen(), Screen::Pond, "west of home");
    let mut app = town(None);
    walk(&mut app, 0.0, -25.0, 60 * 4);
    assert_eq!(state(&app).screen(), Screen::MainStreet, "south of home");
}

#[test]
fn every_door_leads_to_its_place() {
    for place in Place::ALL {
        let mut app = town(None);
        app.world_mut().resource_mut::<Town>().stand_at(place);
        app.update();
        assert_eq!(state(&app).near(), Some(place));
        assert!(shows(&app, place.name()), "{place:?}");
        click_action(&mut app);
        if place == Place::Settings {
            assert_eq!(state(&app).phase, TownPhase::Settings);
            continue;
        }
        assert_eq!(state(&app).destination(), Some(place), "{place:?}");
    }
}

#[test]
fn coming_back_out_of_a_place_you_stand_at_its_door() {
    for place in [Place::Den, Place::Arcade, Place::BeeTree, Place::Pond] {
        let app = returning_from(place);
        assert_eq!(state(&app).near(), Some(place), "{place:?}");
        assert_eq!(state(&app).destination(), None);
    }
}

#[test]
fn tapping_a_spot_walks_you_there() {
    let mut app = town(None);
    let target = state(&app).at() + Vec2::new(80.0, 60.0);
    tap(&mut app, target);
    tick(&mut app, 120);
    assert!(
        state(&app).at().distance(target) < 6.0,
        "{:?}",
        state(&app).at()
    );
}

#[test]
fn the_gold_in_the_tank_shows_on_the_street() {
    let app = town(Some(a_tank_with_gold(42)));
    assert!(shows(&app, "GOLD 42"));
}

fn open_settings() -> App {
    let mut app = town(Some(a_tank_with_gold(42)));
    app.world_mut()
        .resource_mut::<Town>()
        .stand_at(Place::Settings);
    app.update();
    click_action(&mut app);
    assert_eq!(state(&app).phase, TownPhase::Settings);
    app
}

#[test]
fn settings_mute_the_board_and_turn_on_mock_walks_and_both_are_saved() {
    let mut app = open_settings();
    assert!(shows(&app, "SOUND ON") && shows(&app, "MOCK WALK OFF"));
    tap(&mut app, ui::SOUND.center());
    assert!(app.world().resource::<SoundQueue>().muted());
    tap(&mut app, ui::MOCK_WALK.center());
    assert!(shows(&app, "SOUND OFF") && shows(&app, "MOCK WALK ON"));
    let blob = saved(&mut app);
    assert!(preferences::muted(&blob));
    assert!(pals::save::load(&blob).mock_walk);
    tap(&mut app, ui::BACK.center());
    assert_eq!(state(&app).phase, TownPhase::Walking);
}

#[test]
fn the_settings_dial_turns_and_pwr_picks() {
    let mut app = open_settings();
    set_tilt(&mut app, 30.0, 0.0);
    app.update();
    click_action(&mut app);
    assert!(
        app.world().resource::<Pals>().mock_walk,
        "3 o'clock is MOCK WALK"
    );
}

#[test]
fn reset_asks_first_then_erases_the_tank_gold_and_scores_but_keeps_the_settings() {
    let mut app = open_settings();
    tap(&mut app, ui::MOCK_WALK.center());
    tap(&mut app, ui::SOUND.center());
    tap(&mut app, ui::RESET.center());
    assert_eq!(state(&app).phase, TownPhase::ConfirmReset);
    assert!(shows(&app, "RESET PROGRESS?"));
    tap(&mut app, ui::KEEP.center());
    assert_eq!(state(&app).phase, TownPhase::Settings);
    assert_eq!(app.world().resource::<Pals>().gold, 42);
    tap(&mut app, ui::RESET.center());
    tap(&mut app, ui::ERASE.center());
    let blob = saved(&mut app);
    let tank = pals::save::load(&blob);
    assert!(tank.pets.is_empty() && tank.gold == 0);
    assert!(tank.mock_walk && preferences::muted(&blob), "settings kept");
    assert!(blob.section(&angler_game::save::KEY).is_none());
    tap(&mut app, ui::BACK.center());
    assert!(shows(&app, "GOLD 0"));
}

#[test]
fn a_save_from_before_the_town_brings_its_gold_and_hats_along() {
    let mut log_save = angler_game::save::encode(
        &angler_game::FishLog::default(),
        &angler_game::Settings::default(),
    );
    log_save[4] = 3;
    log_save.extend_from_slice(&75u32.to_le_bytes());
    log_save.extend_from_slice(&0b101u32.to_le_bytes());
    log_save.push(2);
    let mut app = town(Some(log_save));
    let tank = app.world().resource::<Pals>().clone();
    assert_eq!(tank.gold, 75);
    assert!(tank.owns_hat(2));
    assert_eq!(pals::save::load(&saved(&mut app)), tank, "and it's saved");
}
