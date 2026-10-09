//! Angler played through the headless app: aiming and casting, the fight,
//! keeping and naming fish, the log, settings, GUTEN, sounds, power and saves.

use angler_game::{
    Catch, Difficulty, FishLog, GUTEN_MORGEN, GUTEN_SECS, Phase, Session, Settings, audio,
    cast::{self, CHARGE_SECS, Flight, scene::WATER_Y},
    reel::{LEAN_DEAD_ZONE_DEG, LEAN_FULL_DEG},
    save,
    session::{ESCAPED_SECS, NOTICE_SECS, RESULT_GUARD_SECS},
    sprites,
    testing::*,
    ui,
};
use bevy::prelude::*;
use orbit_core::{Buttons, ClipRef, Frame, SaveBlob, SoundCommand, SoundQueue, testing::update};
use orbit_gfx::{Point, Shape, SpriteRef};
use orbit_ui::{Key, key_center, key_turn};
use pals::{Name, Pals, Pet};

const SCREEN_CENTER: Vec2 = Vec2::splat(233.0);

fn labels(app: &App) -> Vec<String> {
    app.world()
        .resource::<Frame>()
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Label { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn shows(app: &App, text: &str) -> bool {
    labels(app).iter().any(|l| l == text)
}

fn session(app: &mut App) -> Mut<'_, Session> {
    app.world_mut().resource_mut::<Session>()
}

/// Frames to hold the button for a cast at `power` (the press frame starts
/// the meter at zero).
fn frames_for(power: f32) -> u32 {
    (power * CHARGE_SECS * 60.0).round() as u32 + 1
}

fn start_aiming(app: &mut App) {
    click_action(app);
    assert!(matches!(phase(app), Phase::Aim { .. }));
}

/// Aims, puts the fishing spot exactly where this cast will land, and casts.
fn cast_onto_spot(app: &mut App, steer: f32, power: f32) {
    set_tilt(app, steer, 0.0);
    let lands = Flight::launch(cast::elevation_deg(steer), power).landing_x();
    session(app).spot_x = lands;
    hold_action(app, frames_for(power));
    release_action(app);
}

fn reel_lean(app: &App) -> f32 {
    let Phase::Reel { reel, .. } = phase(app) else {
        return 0.0;
    };
    let push = ((reel.zone_y - reel.fish_y) * 30.0 + reel.zone_v * 3.6) / 1100.0;
    let push = push.clamp(-1.0, 1.0);
    push.signum() * (LEAN_DEAD_ZONE_DEG + push.abs() * LEAN_FULL_DEG)
}

/// Plays the fight with a steady hand until it ends.
fn reel_skillfully(app: &mut App) {
    for _ in 0..60 * 60 {
        if !matches!(phase(app), Phase::Reel { .. }) {
            return;
        }
        let lean = reel_lean(app);
        set_tilt(app, 0.0, lean);
        update(app);
    }
    panic!("fight never ended");
}

fn hook_a_fish(app: &mut App) {
    start_aiming(app);
    cast_onto_spot(app, 0.0, 0.7);
    assert!(run_until(app, 60 * 15, |p| matches!(p, Phase::Reel { .. })));
}

#[test]
fn the_game_boots_to_the_title_and_the_button_starts_aiming() {
    let mut app = headless_app();
    assert_eq!(phase(&app), Phase::Title);
    assert!(shows(&app, "GONE FISHIN"));
    start_aiming(&mut app);
    assert!(shows(&app, "HOLD BUTTON TO CAST"));
}

#[test]
fn tapping_the_title_log_button_opens_the_fish_log_and_tapping_again_closes_it() {
    let mut app = headless_app();
    tap(&mut app, ui::LOG.center());
    assert_eq!(phase(&app), Phase::Log);
    assert!(shows(&app, "FISH LOG"));
    tap(&mut app, SCREEN_CENTER);
    assert_eq!(phase(&app), Phase::Title);
}

#[test]
fn turning_the_device_counterclockwise_raises_the_aim() {
    let mut app = headless_app();
    start_aiming(&mut app);
    set_tilt(&mut app, -30.0, 0.0);
    update(&mut app);
    assert!(shows(&app, "ANGLE 70"));
    set_tilt(&mut app, 20.0, 0.0);
    update(&mut app);
    assert!(shows(&app, "ANGLE 20"));
}

#[test]
fn holding_charges_power_and_releasing_casts_at_the_aimed_angle() {
    let mut app = headless_app();
    start_aiming(&mut app);
    set_tilt(&mut app, -15.0, 0.0);
    hold_action(&mut app, frames_for(0.5));
    assert!(matches!(
        phase(&app),
        Phase::Aim {
            charging_secs: Some(_)
        }
    ));
    release_action(&mut app);
    let Phase::Flight { flight, .. } = phase(&app) else {
        panic!("expected a cast, got {:?}", phase(&app));
    };
    let expected = Flight::launch(55.0, 0.5).vel;
    assert!(
        flight.vel.distance(expected) < 25.0,
        "{:?} vs {expected:?}",
        flight.vel
    );
    assert!(flight.vel.y < 0.0);
}

fn preview_dots(app: &App) -> Vec<Point> {
    app.world()
        .resource::<Frame>()
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Disc { center, color, .. } if *color == angler_game::PREVIEW_DOT => {
                Some(*center)
            }
            _ => None,
        })
        .collect()
}

fn full_power_landing(steer: f32) -> f32 {
    Flight::launch(cast::elevation_deg(steer), 1.0).landing_x()
}

#[test]
fn the_full_power_path_is_dotted_while_aiming_before_any_press() {
    let mut app = headless_app();
    start_aiming(&mut app);
    set_tilt(&mut app, -10.0, 0.0);
    update(&mut app);
    let dots = preview_dots(&app);
    assert!(dots.len() >= 8, "{} dots", dots.len());
    let end = *dots.last().unwrap();
    assert!(
        (end.x as f32 - full_power_landing(-10.0)).abs() <= 1.0,
        "{end:?}"
    );
    assert!((end.y as f32 - WATER_Y).abs() <= 1.0, "{end:?}");
}

#[test]
fn the_dotted_path_stays_put_while_power_charges() {
    let mut app = headless_app();
    start_aiming(&mut app);
    let before = preview_dots(&app);
    hold_action(&mut app, frames_for(0.4));
    assert_eq!(preview_dots(&app), before);
    let gauge = app
        .world()
        .resource::<Frame>()
        .shapes
        .iter()
        .any(|s| matches!(s, Shape::Arc { .. }));
    assert!(gauge);
}

#[test]
fn releasing_at_the_top_of_the_meter_lands_exactly_where_the_dots_end() {
    let mut app = headless_app();
    start_aiming(&mut app);
    let end = *preview_dots(&app).last().unwrap();
    hold_action(&mut app, frames_for(1.0));
    release_action(&mut app);
    let Phase::Flight { flight, perfect } = phase(&app) else {
        panic!("expected a cast, got {:?}", phase(&app));
    };
    assert!(perfect);
    assert!((flight.landing_x() - end.x as f32).abs() <= 1.0);
    assert!(shows(&app, "PERFECT!"));
}

#[test]
fn releasing_early_comes_up_short_of_the_dots() {
    let mut app = headless_app();
    start_aiming(&mut app);
    let end = *preview_dots(&app).last().unwrap();
    hold_action(&mut app, frames_for(0.6));
    release_action(&mut app);
    let Phase::Flight { flight, perfect } = phase(&app) else {
        panic!("expected a cast, got {:?}", phase(&app));
    };
    assert!(!perfect);
    assert!(flight.landing_x() < end.x as f32 - 40.0);
    assert!(!shows(&app, "PERFECT!"));
}

#[test]
fn no_dots_remain_once_the_bobber_is_cast() {
    let mut app = headless_app();
    start_aiming(&mut app);
    hold_action(&mut app, frames_for(1.0));
    release_action(&mut app);
    tick(&mut app, 20);
    assert!(preview_dots(&app).is_empty());
    assert!(run_until(&mut app, 600, |p| matches!(
        p,
        Phase::Waiting { .. }
    )));
    tick(&mut app, 40);
    let any_dots = app
        .world()
        .resource::<Frame>()
        .shapes
        .iter()
        .any(|s| matches!(s, Shape::Disc { radius: 2, .. }));
    assert!(!any_dots, "dots left on screen while waiting");
}

#[test]
fn holding_far_too_long_cancels_the_cast_before_the_power_key_can_shut_down() {
    let mut app = headless_app();
    start_aiming(&mut app);
    hold_action(&mut app, (cast::MAX_CHARGE_SECS * 60.0) as u32 + 5);
    assert_eq!(phase(&app), Phase::Notice("LET GO!"));
    tick(&mut app, (NOTICE_SECS * 60.0) as u32 + 2);
    release_action(&mut app);
    assert!(matches!(
        phase(&app),
        Phase::Aim {
            charging_secs: None
        }
    ));
}

#[test]
fn a_brief_press_is_a_slip_not_a_cast() {
    let mut app = headless_app();
    start_aiming(&mut app);
    click_action(&mut app);
    assert!(matches!(
        phase(&app),
        Phase::Aim {
            charging_secs: None
        }
    ));
}

#[test]
fn a_cast_onto_the_spot_gets_a_bite_and_starts_the_fight() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    assert!(shows(&app, "LEAN TO REEL"));
}

#[test]
fn reeling_the_fish_in_shows_it_and_saves_the_log() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    reel_skillfully(&mut app);
    let Phase::Caught { catch, milestones } = phase(&app) else {
        panic!("expected a catch, got {:?}", phase(&app));
    };
    assert!(milestones.new_species);
    assert!(shows(&app, catch.species().name));
    assert!(shows(&app, &angler_game::length_text(catch.length_mm)));
    let fish_drawn = app.world().resource::<Frame>().shapes.iter().any(|s| {
        matches!(s, Shape::Sprite { sprite, .. } if *sprite == SpriteRef(catch.species().sprite))
    });
    assert!(fish_drawn);

    let log = app.world().resource::<FishLog>().clone();
    assert_eq!(log.total, 1);
    let saved = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    assert_eq!(
        saved
            .and_then(|b| save::decode_stored(&b))
            .map(|(log, ..)| log),
        Some(log)
    );
}

#[test]
fn presses_right_after_a_catch_do_not_skip_the_result() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    reel_skillfully(&mut app);
    click_action(&mut app);
    assert!(matches!(phase(&app), Phase::Caught { .. }));
    tick(&mut app, (RESULT_GUARD_SECS * 60.0) as u32);
    click_action(&mut app);
    assert!(matches!(phase(&app), Phase::Naming { .. }));
}

#[test]
fn ignoring_the_fight_lets_the_fish_escape_and_play_resumes() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    set_tilt(&mut app, 0.0, 0.0);
    assert!(run_until(&mut app, 60 * 30, |p| *p == Phase::Escaped));
    assert!(shows(&app, "IT GOT AWAY..."));
    tick(&mut app, (ESCAPED_SECS * 60.0) as u32 + 2);
    assert!(matches!(phase(&app), Phase::Aim { .. }));
}

#[test]
fn pressing_while_waiting_reels_the_line_back_in() {
    let mut app = headless_app();
    start_aiming(&mut app);
    cast_onto_spot(&mut app, 0.0, 0.7);
    assert!(run_until(&mut app, 600, |p| matches!(
        p,
        Phase::Waiting { .. }
    )));
    click_action(&mut app);
    assert_eq!(phase(&app), Phase::Notice("REELED IN"));
    tick(&mut app, (NOTICE_SECS * 60.0) as u32 + 2);
    assert!(matches!(phase(&app), Phase::Aim { .. }));
}

#[test]
fn a_steep_weak_cast_lands_on_the_dock() {
    let mut app = headless_app();
    start_aiming(&mut app);
    set_tilt(&mut app, -45.0, 0.0);
    hold_action(&mut app, frames_for(0.1));
    release_action(&mut app);
    assert!(run_until(&mut app, 600, |p| *p == Phase::Notice("ON THE DOCK!")));
}

#[test]
fn the_fish_log_survives_a_reboot() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    reel_skillfully(&mut app);
    let saved = app
        .world_mut()
        .resource_mut::<SaveBlob>()
        .take_pending()
        .unwrap();
    let rebooted = headless_app_with_save(Some(saved));
    assert_eq!(rebooted.world().resource::<FishLog>().total, 1);
    assert!(shows(&rebooted, "CAUGHT 1"));
}

#[test]
fn garbage_in_storage_starts_a_fresh_log() {
    let app = headless_app_with_save(Some(vec![1, 2, 3]));
    assert_eq!(*app.world().resource::<FishLog>(), FishLog::default());
}

#[test]
fn the_zero_button_confirms_from_any_screen() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    press_zero(&mut app);
    assert!(shows(&app, "LEVEL SET"));
    tick(&mut app, 90);
    assert!(!shows(&app, "LEVEL SET"));
}

fn open_settings(app: &mut App) {
    tap(app, ui::SETTINGS.center());
    assert_eq!(phase(app), Phase::Settings);
}

fn difficulty(app: &App) -> Difficulty {
    app.world().resource::<Settings>().difficulty
}

fn sounds(app: &App) -> Vec<SoundCommand> {
    app.world().resource::<SoundQueue>().pending().to_vec()
}

#[test]
fn the_title_has_log_and_settings_buttons_and_stray_taps_do_nothing() {
    let mut app = headless_app();
    assert!(shows(&app, "LOG") && shows(&app, "SETTINGS"));
    tap(&mut app, Vec2::new(233.0, 300.0));
    assert_eq!(phase(&app), Phase::Title);
    open_settings(&mut app);
    assert!(shows(&app, "DIFFICULTY") && shows(&app, "NORMAL"));
}

#[test]
fn the_arrows_step_the_difficulty_and_each_change_is_saved() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::HARDER.center());
    assert_eq!(difficulty(&app), Difficulty::Hard);
    assert!(shows(&app, "HARD"));
    let saved = app
        .world_mut()
        .resource_mut::<SaveBlob>()
        .take_pending()
        .and_then(|b| save::decode_stored(&b));
    assert_eq!(
        saved.map(|(_, settings)| settings.difficulty),
        Some(Difficulty::Hard)
    );
    tap(&mut app, ui::HARDER.center());
    assert_eq!(difficulty(&app), Difficulty::Easy);
    tap(&mut app, ui::EASIER.center());
    tap(&mut app, ui::EASIER.center());
    assert_eq!(difficulty(&app), Difficulty::Normal);
}

#[test]
fn the_chosen_difficulty_survives_a_reboot() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::EASIER.center());
    let saved = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    let rebooted = headless_app_with_save(saved);
    assert_eq!(difficulty(&rebooted), Difficulty::Easy);
}

#[test]
fn back_or_the_action_button_leaves_settings() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::BACK.center());
    assert_eq!(phase(&app), Phase::Title);
    open_settings(&mut app);
    click_action(&mut app);
    assert_eq!(phase(&app), Phase::Title);
}

#[test]
fn easy_widens_the_perfect_window_so_a_slightly_early_release_still_hits_the_dots() {
    let release_at_92 = |app: &mut App| {
        start_aiming(app);
        hold_action(app, frames_for(0.92));
        release_action(app);
        matches!(phase(app), Phase::Flight { perfect: true, .. })
    };
    let mut normal = headless_app();
    assert!(!release_at_92(&mut normal));
    let mut easy = headless_app();
    open_settings(&mut easy);
    tap(&mut easy, ui::EASIER.center());
    tap(&mut easy, ui::BACK.center());
    assert!(release_at_92(&mut easy));
}

#[test]
fn the_fight_uses_the_difficultys_catch_zone() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::HARDER.center());
    tap(&mut app, ui::BACK.center());
    hook_a_fish(&mut app);
    let Phase::Reel { reel, .. } = phase(&app) else {
        panic!("expected a fight");
    };
    assert_eq!(reel.zone_height, Difficulty::Hard.tuning().zone_height);
}

#[test]
fn guten_plays_the_song_and_spins_the_trout() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::GUTEN.center());
    assert_eq!(phase(&app), Phase::Guten);
    assert_eq!(
        sounds(&app).last(),
        Some(&SoundCommand::Play(ClipRef(&GUTEN_MORGEN)))
    );
    assert!(shows(&app, "GUTEN MORGEN"));
    let trout_width = |app: &App| {
        app.world()
            .resource::<Frame>()
            .shapes
            .iter()
            .find_map(|s| match s {
                Shape::SpriteFit { sprite, area, .. } if *sprite == SpriteRef(&sprites::TROUT) => {
                    Some(area.size.width)
                }
                _ => None,
            })
            .expect("the trout is on screen")
    };
    let facing = trout_width(&app);
    tick(&mut app, 34);
    assert!(trout_width(&app) < facing / 2, "the trout turns side-on");
}

#[test]
fn guten_lasts_as_long_as_the_song_then_returns_to_settings() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::GUTEN.center());
    tick(&mut app, (GUTEN_SECS * 60.0) as u32 - 10);
    assert_eq!(phase(&app), Phase::Guten);
    tick(&mut app, 12);
    assert_eq!(phase(&app), Phase::Settings);
    assert_eq!(sounds(&app).last(), Some(&SoundCommand::Stop));
}

#[cfg(guten_song)]
#[test]
fn guten_secs_is_the_songs_length() {
    assert!((GUTEN_MORGEN.duration_secs() - GUTEN_SECS).abs() < 0.01);
}

#[test]
fn a_tap_stops_guten_early() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::GUTEN.center());
    tick(&mut app, 60);
    tap(&mut app, SCREEN_CENTER);
    assert_eq!(phase(&app), Phase::Settings);
    assert_eq!(sounds(&app).last(), Some(&SoundCommand::Stop));
}

fn sprite_drawn(app: &App, sprite: &'static orbit_gfx::Sprite) -> bool {
    app.world().resource::<Frame>().shapes.iter().any(
        |shape| matches!(shape, Shape::Sprite { sprite: drawn, .. } if *drawn == SpriteRef(sprite)),
    )
}

fn tank(app: &App) -> &Pals {
    app.world().resource::<Pals>()
}

/// Lands a fish with a steady hand and waits out the result guard.
fn land_a_fish(app: &mut App) -> Catch {
    hook_a_fish(app);
    reel_skillfully(app);
    let Phase::Caught { catch, .. } = phase(app) else {
        panic!("expected a catch, got {:?}", phase(app));
    };
    tick(app, (RESULT_GUARD_SECS * 60.0) as u32);
    catch
}

/// Turns to `key` and presses PWR, as the player types.
fn press_key(app: &mut App, key: Key) {
    set_tilt(app, key_turn(key), 0.0);
    tick(app, 1);
    click_action(app);
}

fn type_name(app: &mut App, name: &str) {
    for letter in name.chars() {
        let key = if letter == ' ' {
            Key::Space
        } else {
            Key::Letter(letter)
        };
        press_key(app, key);
    }
    press_key(app, Key::Done);
}

fn saved_tank(app: &mut App) -> Option<Pals> {
    let pending = app.world_mut().resource_mut::<SaveBlob>().take_pending()?;
    Some(pals::save::load(&SaveBlob::from_storage(Some(pending))))
}

/// A save record whose tank holds the first `count` of ONE, TWO and THREE.
fn tank_of(count: usize) -> Vec<u8> {
    let mut tank = Pals::default();
    let mut rng = orbit_core::Rng::new(3);
    for text in ["ONE", "TWO", "THREE"].into_iter().take(count) {
        let mut name = Name::default();
        text.chars().for_each(|c| assert!(name.push(c)));
        assert!(tank.keep(Pet::new(0, 0, 80, 50, name, &mut rng)).is_ok());
    }
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    blob.take_pending().expect("a record")
}

fn names(app: &App) -> Vec<String> {
    tank(app)
        .pets
        .iter()
        .map(|pet| pet.name.as_str().into())
        .collect()
}

#[test]
fn a_landed_fish_can_be_kept_named_by_turning_and_joins_the_tank() {
    let mut app = headless_app();
    let catch = land_a_fish(&mut app);
    assert!(shows(&app, "KEEP") && shows(&app, "RELEASE"));
    assert!(
        !labels(&app).iter().any(|l| l.contains("GOLD")),
        "catches pay no gold"
    );
    click_action(&mut app);
    assert!(matches!(phase(&app), Phase::Naming { .. }));
    assert!(shows(&app, "NAME YOUR FISH"));
    type_name(&mut app, "GIL");
    assert!(shows(&app, "IN THE TANK!"));
    assert_eq!(names(&app), ["GIL"]);
    let pet = &tank(&app).pets[0];
    assert_eq!(
        (usize::from(pet.species), pet.length_mm),
        (catch.species, catch.length_mm)
    );
    assert_eq!(pet.tier, catch.species().tier);
    assert_eq!(
        saved_tank(&mut app).map(|saved| saved.pets.len()),
        Some(1),
        "the tank is saved"
    );
    tick(&mut app, (NOTICE_SECS * 60.0) as u32 + 1);
    assert!(matches!(phase(&app), Phase::Aim { .. }));
}

#[test]
fn ok_waits_for_a_name_and_del_takes_a_letter_back() {
    let mut app = headless_app();
    land_a_fish(&mut app);
    click_action(&mut app);
    press_key(&mut app, Key::Done);
    assert!(matches!(phase(&app), Phase::Naming { .. }), "no name yet");
    press_key(&mut app, Key::Letter('A'));
    press_key(&mut app, Key::Letter('B'));
    assert!(shows(&app, "AB_"));
    press_key(&mut app, Key::Delete);
    assert!(shows(&app, "A_"));
    press_key(&mut app, Key::Done);
    assert_eq!(names(&app), ["A"]);
}

#[test]
fn turning_right_releases_the_catch_and_the_tank_is_unchanged() {
    let mut app = headless_app_with_save(Some(tank_of(1)));
    land_a_fish(&mut app);
    set_tilt(&mut app, 25.0, 0.0);
    tick(&mut app, 1);
    click_action(&mut app);
    assert!(matches!(phase(&app), Phase::Aim { .. }));
    assert_eq!(names(&app), ["ONE"]);
}

#[test]
fn keep_release_and_letters_can_all_be_tapped() {
    let mut app = headless_app();
    land_a_fish(&mut app);
    tap(&mut app, ui::KEEP_FISH.center());
    assert!(matches!(phase(&app), Phase::Naming { .. }));
    tap(&mut app, key_center(Key::Letter('Z')));
    tap(&mut app, key_center(Key::Done));
    assert_eq!(names(&app), ["Z"]);

    tick(&mut app, (NOTICE_SECS * 60.0) as u32 + 1);
    hook_a_fish(&mut app);
    reel_skillfully(&mut app);
    tick(&mut app, (RESULT_GUARD_SECS * 60.0) as u32);
    tap(&mut app, ui::RELEASE_FISH.center());
    assert!(matches!(phase(&app), Phase::Aim { .. }));
    assert_eq!(names(&app), ["Z"]);
}

#[test]
fn a_full_tank_asks_which_fish_to_let_go_and_the_new_one_takes_its_place() {
    let mut app = headless_app_with_save(Some(tank_of(3)));
    land_a_fish(&mut app);
    assert!(shows(&app, "TANK FULL: KEEP SWAPS ONE OUT"));
    click_action(&mut app);
    assert!(matches!(phase(&app), Phase::Swap { .. }));
    for name in ["ONE", "TWO", "THREE", "LET IT GO"] {
        assert!(shows(&app, name), "{name}");
    }
    set_tilt(&mut app, 30.0, 0.0);
    tick(&mut app, 1);
    click_action(&mut app);
    assert!(matches!(phase(&app), Phase::Naming { .. }));
    type_name(&mut app, "NEW");
    assert_eq!(names(&app), ["ONE", "NEW", "THREE"]);
}

#[test]
fn from_a_full_tank_the_new_catch_can_still_be_let_go() {
    let mut app = headless_app_with_save(Some(tank_of(3)));
    land_a_fish(&mut app);
    click_action(&mut app);
    set_tilt(&mut app, -30.0, 0.0);
    tick(&mut app, 1);
    click_action(&mut app);
    assert!(matches!(phase(&app), Phase::Aim { .. }));
    assert_eq!(names(&app), ["ONE", "TWO", "THREE"]);
}

#[test]
fn the_menu_button_while_aiming_goes_back_to_the_title() {
    let mut app = headless_app();
    start_aiming(&mut app);
    assert!(shows(&app, "MENU"));
    tap(&mut app, ui::MENU.center());
    assert_eq!(phase(&app), Phase::Title);
}

#[test]
fn the_menu_button_hides_while_charging_and_a_tap_there_keeps_the_cast() {
    let mut app = headless_app();
    start_aiming(&mut app);
    hold_action(&mut app, frames_for(0.3));
    assert!(!shows(&app, "MENU"));
    app.world_mut()
        .resource_mut::<orbit_core::Touch>()
        .set(Some(ui::MENU.center()));
    update(&mut app);
    assert!(matches!(
        phase(&app),
        Phase::Aim {
            charging_secs: Some(_)
        }
    ));
}

fn pose(app: &App) -> (f32, f32) {
    let tilt = app.world().resource::<orbit_core::Tilt>();
    (tilt.steer_deg, tilt.lean_deg)
}

#[test]
fn releasing_a_catch_re_levels_the_device_for_the_next_cast() {
    let mut app = headless_app();
    land_a_fish(&mut app);
    set_tilt(&mut app, 25.0, 6.0);
    tick(&mut app, 1);
    click_action(&mut app);
    assert!(matches!(phase(&app), Phase::Aim { .. }));
    assert_eq!(pose(&app), (0.0, 0.0));
    assert!(shows(&app, "ANGLE 40"));
    assert!(!shows(&app, "LEVEL SET"));
}

#[test]
fn a_fish_that_got_away_re_levels_the_device_too() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    set_tilt(&mut app, 0.0, 0.0);
    assert!(run_until(&mut app, 60 * 30, |p| *p == Phase::Escaped));
    set_tilt(&mut app, -20.0, 4.0);
    tick(&mut app, (ESCAPED_SECS * 60.0) as u32 + 2);
    assert!(matches!(phase(&app), Phase::Aim { .. }));
    assert_eq!(pose(&app), (0.0, 0.0));
}

fn clear_sounds(app: &mut App) {
    app.world_mut()
        .resource_mut::<SoundQueue>()
        .drain()
        .for_each(drop);
}

fn play(clip: &'static orbit_core::Clip) -> SoundCommand {
    SoundCommand::Play(ClipRef(clip))
}

fn heard(app: &App, clip: &'static orbit_core::Clip) -> usize {
    sounds(app).iter().filter(|s| **s == play(clip)).count()
}

fn last_sound(app: &App) -> Option<SoundCommand> {
    sounds(app).last().copied()
}

#[test]
fn a_cast_whooshes_and_a_perfect_one_chimes() {
    for (power, clip) in [(0.6, &audio::CAST), (1.0, &audio::PERFECT_CAST)] {
        let mut app = headless_app();
        start_aiming(&mut app);
        clear_sounds(&mut app);
        hold_action(&mut app, frames_for(power));
        release_action(&mut app);
        assert_eq!(last_sound(&app), Some(play(clip)), "power {power}");
    }
}

#[test]
fn the_bobber_plops_on_the_water_and_thuds_on_the_dock() {
    let mut app = headless_app();
    start_aiming(&mut app);
    cast_onto_spot(&mut app, 0.0, 0.7);
    assert!(run_until(&mut app, 600, |p| matches!(
        p,
        Phase::Waiting { .. }
    )));
    assert_eq!(last_sound(&app), Some(play(&audio::PLOP)));

    let mut app = headless_app();
    start_aiming(&mut app);
    set_tilt(&mut app, -45.0, 0.0);
    hold_action(&mut app, frames_for(0.1));
    release_action(&mut app);
    assert!(run_until(&mut app, 600, |p| *p == Phase::Notice("ON THE DOCK!")));
    assert_eq!(last_sound(&app), Some(play(&audio::THUD)));
}

#[test]
fn a_bite_sounds_the_alarm() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    assert_eq!(heard(&app, &audio::BITE), 1);
}

fn progress(app: &App) -> f32 {
    let Phase::Reel { reel, .. } = phase(app) else {
        panic!("expected a fight, got {:?}", phase(app));
    };
    reel.progress
}

#[test]
fn the_reel_clicks_while_the_catch_meter_rises_and_goes_quiet_while_it_drains() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    set_tilt(&mut app, 0.0, 0.0);
    clear_sounds(&mut app);
    tick(&mut app, 30);
    assert!(
        heard(&app, &audio::REEL) >= 3,
        "{} ticks",
        heard(&app, &audio::REEL)
    );

    let mut app = headless_app();
    hook_a_fish(&mut app);
    set_tilt(&mut app, 0.0, LEAN_DEAD_ZONE_DEG + LEAN_FULL_DEG);
    tick(&mut app, 30);
    clear_sounds(&mut app);
    let before = progress(&app);
    tick(&mut app, 20);
    assert!(progress(&app) < before, "the meter should be draining");
    assert_eq!(heard(&app, &audio::REEL), 0);
}

#[test]
fn landing_a_fish_plays_the_jingle_and_losing_one_the_sad_trombone() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    reel_skillfully(&mut app);
    assert!(matches!(phase(&app), Phase::Caught { .. }));
    assert_eq!(last_sound(&app), Some(play(&audio::CATCH)));

    let mut app = headless_app();
    hook_a_fish(&mut app);
    set_tilt(&mut app, 0.0, 0.0);
    assert!(run_until(&mut app, 60 * 30, |p| *p == Phase::Escaped));
    assert_eq!(last_sound(&app), Some(play(&audio::ESCAPE)));
}

#[test]
fn menu_buttons_click_and_taps_off_them_are_silent() {
    let mut app = headless_app();
    tap(&mut app, ui::LOG.center());
    assert_eq!(last_sound(&app), Some(play(&audio::CLICK)));
    tap(&mut app, SCREEN_CENTER);
    clear_sounds(&mut app);
    tap(&mut app, Vec2::new(233.0, 300.0));
    assert_eq!(
        heard(&app, &audio::CLICK),
        0,
        "taps off the buttons are silent"
    );
}

#[test]
fn a_stray_tap_right_after_guten_starts_does_not_cut_the_song() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::GUTEN.center());
    tick(&mut app, 3);
    tap(&mut app, SCREEN_CENTER);
    click_action(&mut app);
    assert_eq!(phase(&app), Phase::Guten);
    assert_eq!(last_sound(&app), Some(play(&GUTEN_MORGEN)));
}

fn report_battery(app: &mut App, percent: Option<u8>, external_power: bool, charging: bool) {
    app.world_mut().resource_mut::<orbit_core::Power>().battery = Some(orbit_core::Battery {
        percent,
        external_power,
        charging,
    });
    update(app);
}

fn power(app: &App) -> orbit_core::Power {
    *app.world().resource::<orbit_core::Power>()
}

#[test]
fn the_menus_show_the_battery_level_with_a_bolt_while_charging() {
    let mut app = headless_app();
    assert!(
        !labels(&app).iter().any(|l| l.ends_with('%')),
        "no reading yet"
    );
    report_battery(&mut app, Some(76), false, false);
    assert!(shows(&app, "76%"));
    assert!(!sprite_drawn(&app, &orbit_ui::BOLT));
    report_battery(&mut app, Some(77), true, true);
    assert!(shows(&app, "77%") && sprite_drawn(&app, &orbit_ui::BOLT));
    open_settings(&mut app);
    assert!(shows(&app, "77%"));
}

#[test]
fn on_usb_without_a_battery_the_menus_say_usb() {
    let mut app = headless_app();
    report_battery(&mut app, None, true, false);
    assert!(shows(&app, "USB"));
}

#[test]
fn a_low_battery_warns_during_play_but_a_healthy_one_stays_out_of_the_way() {
    let mut app = headless_app();
    report_battery(&mut app, Some(60), false, false);
    start_aiming(&mut app);
    assert!(!shows(&app, "60%"));
    report_battery(&mut app, Some(12), false, false);
    assert!(shows(&app, "12%"));
    report_battery(&mut app, Some(12), true, true);
    assert!(!shows(&app, "12%"), "charging is not a warning");
}

fn hold_zero(app: &mut App, frames: u32) {
    app.world_mut().resource_mut::<Buttons>().zero.set(true);
    tick(app, frames);
}

#[test]
fn holding_the_zero_button_says_good_night_then_sleeps() {
    let mut app = headless_app();
    hold_zero(&mut app, 45);
    assert!(shows(&app, "HOLD TO SLEEP"));
    hold_zero(&mut app, 100);
    assert!(!shows(&app, "GOOD NIGHT"), "3 s, not 1.5 s");
    hold_zero(&mut app, 45);
    assert!(shows(&app, "GOOD NIGHT") && shows(&app, "PRESS BOOT TO WAKE"));
    assert!(!power(&app).sleep_requested());
    app.world_mut().resource_mut::<Buttons>().zero.set(false);
    click_action(&mut app);
    tap(&mut app, ui::LOG.center());
    assert_eq!(phase(&app), Phase::Title, "the goodbye ignores input");
    tick(&mut app, 120);
    assert!(shows(&app, "GOOD NIGHT"), "it stays up for a while");
    tick(&mut app, 40);
    assert!(power(&app).sleep_requested());
}

#[test]
fn a_short_zero_press_still_just_re_levels() {
    let mut app = headless_app();
    press_zero(&mut app);
    tick(&mut app, 30);
    assert!(!shows(&app, "HOLD TO SLEEP"));
    assert!(!power(&app).sleep_requested());
    assert_eq!(phase(&app), Phase::Title);
}

#[test]
fn falling_asleep_silences_the_speaker() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::GUTEN.center());
    hold_zero(&mut app, 190);
    assert_eq!(last_sound(&app), Some(SoundCommand::Stop));
}

#[test]
fn launched_from_the_town_the_title_offers_a_way_out() {
    let app = headless_app();
    assert!(!shows(&app, "OUT"));
    let mut app = orbit_core::testing::headless(
        |app| {
            app.add_plugins(angler_game::AnglerPlugin)
                .insert_resource(orbit_core::Launcher);
        },
        None,
    );
    assert!(shows(&app, "OUT"));
    tap(&mut app, ui::HOME.center());
    assert_eq!(app.should_exit(), Some(bevy::app::AppExit::Success));
}
