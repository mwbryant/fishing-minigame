use bevy::{app::AppExit, prelude::*};
use orbit_core::{SaveBlob, SoundQueue, Touch, preferences, testing::*};
use orbit_gfx::Point;
use pals::{Name, Pals, Pet};
use pals_game::{
    Arrival, Place,
    town::{Screen, Town, TownPhase, TownPlugin, map::landmark, ui},
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

/// Back out of `place` with the finger that pressed OUT still on the glass
/// at `finger`, or PWR still down.
fn coming_out_of(place: Place, finger: Option<Vec2>, pwr_down: bool) -> App {
    headless(
        |app| {
            app.add_plugins(TownPlugin).insert_resource(Arrival(place));
            app.world_mut().resource_mut::<Touch>().set(finger);
            app.world_mut()
                .resource_mut::<orbit_core::Buttons>()
                .action
                .set(pwr_down);
        },
        None,
    )
}

/// Walks away from the signpost, then taps it, which opens the settings.
fn settings_by_tapping_the_signpost() -> App {
    let mut app = town(None);
    tap_and_walk(&mut app, Vec2::new(330.0, 300.0), 120);
    let sign = landmark(Place::Settings).center();
    tap_and_walk(&mut app, Vec2::new(sign.x as f32, sign.y as f32), 60 * 4);
    assert_eq!(state(&app).phase, TownPhase::Settings);
    app
}

fn state(app: &App) -> &Town {
    app.world().resource::<Town>()
}

/// A finger held at `at` for `frames` updates, then lifted.
fn hold(app: &mut App, at: Vec2, frames: u32) {
    for _ in 0..frames {
        app.world_mut().resource_mut::<Touch>().set(Some(at));
        update(app);
    }
    app.world_mut().resource_mut::<Touch>().set(None);
    update(app);
}

/// A tap at `at`, then `frames` updates to walk there.
fn tap_and_walk(app: &mut App, at: Vec2, frames: u32) {
    tap(app, at);
    tick(app, frames);
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
fn a_finger_held_on_the_glass_walks_you_toward_it() {
    let mut app = town(None);
    let start = state(&app).at();
    let ahead = start + Vec2::new(150.0, 0.0);
    hold(&mut app, ahead, 30);
    let walked = state(&app).at();
    assert!(walked.x > start.x + 40.0 && (walked.y - start.y).abs() < 2.0);
    assert!(walked.x < ahead.x, "half a second gets partway");
    let below = walked + Vec2::new(0.0, 120.0);
    hold(&mut app, below, 30);
    assert!(
        state(&app).at().y > walked.y + 40.0,
        "dragging down walks down"
    );
}

#[test]
fn you_follow_a_finger_dragged_across_the_glass() {
    let mut app = town(None);
    let start = state(&app).at();
    for frame in 0..90 {
        let finger = start + Vec2::new(frame as f32 * 2.0, 40.0);
        app.world_mut().resource_mut::<Touch>().set(Some(finger));
        update(&mut app);
    }
    let finger = start + Vec2::new(178.0, 40.0);
    assert!(
        state(&app).at().distance(finger) < 12.0,
        "kept up with the finger: {:?} vs {finger:?}",
        state(&app).at()
    );
}

#[test]
fn turning_or_leaning_the_device_doesnt_walk() {
    let mut app = town(None);
    let start = state(&app).at();
    set_tilt(&mut app, 30.0, 30.0);
    tick(&mut app, 60);
    assert_eq!(state(&app).at(), start);
}

#[test]
fn the_house_is_solid_when_you_walk_past_it() {
    let mut app = town(None);
    tap_and_walk(&mut app, Vec2::new(110.0, 120.0), 120);
    let house = landmark(Place::Home);
    tap(&mut app, Vec2::new(380.0, 120.0));
    for _ in 0..180 {
        update(&mut app);
        let at = state(&app).at();
        let inside = house.contains(Point::new(at.x as i32, at.y as i32));
        assert!(!inside, "walked into the house to {at:?}");
    }
    assert_eq!(state(&app).destination(), None);
}

#[test]
fn tapping_the_house_from_across_the_clearing_walks_you_in() {
    let mut app = town(None);
    tap_and_walk(&mut app, Vec2::new(330.0, 300.0), 120);
    assert_eq!(state(&app).near(), None);
    tap_and_walk(&mut app, Vec2::new(233.0, 100.0), 60 * 3);
    assert_eq!(state(&app).destination(), Some(Place::Home));
}

#[test]
fn a_tap_just_off_the_house_still_goes_in() {
    let mut app = town(None);
    tap_and_walk(&mut app, Vec2::new(330.0, 300.0), 120);
    let house = landmark(Place::Home);
    let beside = house.top_left.x as f32 + house.size.width as f32 + 20.0;
    tap_and_walk(&mut app, Vec2::new(beside, 100.0), 60 * 3);
    assert_eq!(state(&app).destination(), Some(Place::Home));
}

#[test]
fn tapping_the_water_takes_you_fishing() {
    let mut app = town(None);
    app.world_mut().resource_mut::<Town>().stand_at(Place::Pond);
    tap_and_walk(&mut app, Vec2::new(380.0, 330.0), 120);
    assert_eq!(state(&app).near(), None);
    tap_and_walk(&mut app, Vec2::new(150.0, 150.0), 60 * 3);
    assert_eq!(state(&app).destination(), Some(Place::Pond));
}

#[test]
fn every_place_is_one_tap_on_its_landmark_away() {
    for place in Place::ALL {
        let mut app = town(None);
        app.world_mut().resource_mut::<Town>().stand_at(place);
        let away = if place == Place::Pond {
            Vec2::new(380.0, 330.0)
        } else {
            Vec2::splat(233.0)
        };
        tap_and_walk(&mut app, away, 120);
        assert_ne!(state(&app).near(), Some(place), "{place:?}");
        let center = landmark(place).center();
        tap_and_walk(
            &mut app,
            Vec2::new(center.x as f32, center.y as f32),
            60 * 4,
        );
        if place == Place::Settings {
            assert_eq!(state(&app).phase, TownPhase::Settings);
            continue;
        }
        assert_eq!(state(&app).destination(), Some(place), "{place:?}");
    }
}

#[test]
fn at_a_door_the_plate_names_it_and_there_is_no_go_in_button() {
    for place in Place::ALL {
        let mut app = town(None);
        app.world_mut().resource_mut::<Town>().stand_at(place);
        update(&mut app);
        assert!(!shows(&app, "GO IN"), "{place:?}");
        assert!(shows(&app, place.name()), "{place:?}");
        let hint = if place == Place::Settings {
            "PRESS TO OPEN"
        } else {
            "PRESS TO GO IN"
        };
        assert!(shows(&app, hint), "{place:?}");
    }
}

#[test]
fn where_go_in_was_is_just_the_path_south_now() {
    let mut app = town(None);
    assert_eq!(state(&app).near(), Some(Place::Home));
    tap_and_walk(&mut app, Vec2::new(233.0, 420.0), 60 * 4);
    assert_eq!(state(&app).destination(), None);
    assert_eq!(state(&app).screen(), Screen::MainStreet);
}

#[test]
fn walking_in_from_a_landmark_keeps_the_usual_plate() {
    let mut app = town(None);
    tap_and_walk(&mut app, Vec2::new(330.0, 300.0), 120);
    tap(&mut app, Vec2::new(233.0, 100.0));
    for _ in 0..180 {
        if state(&app).destination().is_some() {
            break;
        }
        assert!(!shows(&app, "PRESS TO GO IN"), "the door plate flashed");
        update(&mut app);
    }
    assert_eq!(state(&app).destination(), Some(Place::Home));
    assert!(!shows(&app, "PRESS TO GO IN"), "the door plate flashed");
}

#[test]
fn the_finger_that_pressed_out_doesnt_walk_you_or_take_you_back_in() {
    let out = Vec2::new(233.0, 55.0);
    let mut app = coming_out_of(Place::Pond, Some(out), false);
    let door = state(&app).at();
    tick(&mut app, 30);
    assert_eq!(state(&app).at(), door, "the finger still down walked you");
    app.world_mut().resource_mut::<Touch>().set(None);
    tick(&mut app, 60 * 3);
    assert_eq!(state(&app).at(), door);
    assert_eq!(state(&app).destination(), None);
}

#[test]
fn pwr_still_down_from_out_doesnt_take_you_back_in() {
    let mut app = coming_out_of(Place::Pond, None, true);
    assert_eq!(state(&app).destination(), None);
    release_action(&mut app);
    tick(&mut app, 30);
    assert_eq!(state(&app).destination(), None);
}

#[test]
fn for_two_seconds_after_coming_out_nothing_goes_in() {
    let mut app = returning_from(Place::Pond);
    tick(&mut app, 60);
    click_action(&mut app);
    assert_eq!(state(&app).destination(), None, "PWR at 1 s");
    tap_and_walk(&mut app, Vec2::new(150.0, 150.0), 30);
    assert_eq!(state(&app).destination(), None, "the lake at 1.5 s");

    let mut app = returning_from(Place::Pond);
    tick(&mut app, 60 * 2 + 5);
    click_action(&mut app);
    assert_eq!(state(&app).destination(), Some(Place::Pond));
}

#[test]
fn backing_out_of_settings_you_opened_by_tapping_doesnt_reopen_them() {
    let mut app = settings_by_tapping_the_signpost();
    tap(&mut app, ui::BACK.center());
    tick(&mut app, 30);
    assert_eq!(state(&app).phase, TownPhase::Walking);
    tick(&mut app, 60 * 2);
    click_action(&mut app);
    assert_eq!(
        state(&app).phase,
        TownPhase::Settings,
        "PWR opens them again later"
    );
}

#[test]
fn the_finger_that_pressed_back_doesnt_walk_you() {
    let mut app = settings_by_tapping_the_signpost();
    let at = state(&app).at();
    hold(&mut app, ui::BACK.center(), 30);
    assert_eq!(state(&app).phase, TownPhase::Walking);
    assert_eq!(state(&app).at(), at);
}

#[test]
fn standing_near_a_door_is_enough_to_go_in() {
    let mut app = town(None);
    let door = state(&app).at();
    tap_and_walk(&mut app, door + Vec2::new(34.0, 12.0), 60);
    assert_eq!(state(&app).near(), Some(Place::Home));
}

#[test]
fn tapping_the_end_of_a_path_walks_into_the_next_clearing() {
    let east = Vec2::new(450.0, 233.0);
    let west = Vec2::new(16.0, 233.0);
    let mut app = town(None);
    tap_and_walk(&mut app, east, 60 * 4);
    assert_eq!(state(&app).screen(), Screen::Park, "east of home");
    tap_and_walk(&mut app, west, 60 * 4);
    assert_eq!(state(&app).screen(), Screen::Home);
    tap_and_walk(&mut app, west, 60 * 4);
    assert_eq!(state(&app).screen(), Screen::Pond, "west of home");
    let mut app = town(None);
    tap_and_walk(&mut app, Vec2::new(233.0, 450.0), 60 * 4);
    assert_eq!(state(&app).screen(), Screen::MainStreet, "south of home");
}

#[test]
fn a_finger_still_down_after_a_path_doesnt_walk_you_back_out() {
    let mut app = town(None);
    hold(&mut app, Vec2::new(450.0, 233.0), 60 * 6);
    assert_eq!(state(&app).screen(), Screen::Park);
    let arrived = state(&app).at();
    assert!(arrived.x < 233.0, "still by the west path: {arrived:?}");
}

#[test]
fn every_door_leads_to_its_place() {
    for place in Place::ALL {
        let mut app = town(None);
        app.world_mut().resource_mut::<Town>().stand_at(place);
        update(&mut app);
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
    tap_and_walk(&mut app, target, 120);
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
    update(&mut app);
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
    assert!(shows(&app, "SOUND OFF") && shows(&app, "MOCK WALK x1"));
    let blob = saved(&mut app);
    assert!(preferences::muted(&blob));
    assert!(pals::save::load(&blob).mock_walk);
    tap(&mut app, ui::BACK.center());
    assert_eq!(state(&app).phase, TownPhase::Walking);
}

#[test]
fn a_tap_just_off_a_settings_button_presses_it() {
    let mut app = open_settings();
    let back = ui::BACK.area;
    let below = back.top_left.y as f32 + back.size.height as f32 + 10.0;
    tap(&mut app, Vec2::new(back.center().x as f32, below));
    assert_eq!(state(&app).phase, TownPhase::Walking);
}

#[test]
fn mock_walk_steps_through_its_paces_then_off() {
    let mut app = open_settings();
    for pace in ["x1", "x2", "x5", "x10", "x25"] {
        tap(&mut app, ui::MOCK_WALK.center());
        assert!(shows(&app, &format!("MOCK WALK {pace}")), "{pace}");
    }
    tap(&mut app, ui::MOCK_WALK.center());
    assert!(shows(&app, "MOCK WALK OFF"));
    assert!(!app.world().resource::<Pals>().mock_walk);
}

#[test]
fn the_settings_dial_turns_and_pwr_picks() {
    let mut app = open_settings();
    set_tilt(&mut app, 30.0, 0.0);
    update(&mut app);
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
