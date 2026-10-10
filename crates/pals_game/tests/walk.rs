use bevy::{app::AppExit, prelude::*};
use orbit_core::{Buttons, Power, SaveBlob, Steps, testing::*};
use pals::{Name, Pals, Pet};
use pals_game::walk::{DOZE_AFTER_SECS, Walk, WalkPhase, WalkPlugin};

fn name(text: &str) -> Name {
    let mut name = Name::default();
    text.chars().for_each(|c| assert!(name.push(c)));
    name
}

fn park(pets: &[(&str, f32)], mock_walk: bool) -> App {
    let mut tank = Pals::default();
    tank.mock_walk = mock_walk;
    for (text, walked) in pets {
        let mut pet = Pet::new(1, 0, 150, 50, name(text), &mut orbit_core::Rng::new(2));
        pet.walk(*walked);
        tank.keep(pet).unwrap();
    }
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    headless(
        |app| {
            app.add_plugins(WalkPlugin);
        },
        blob.take_pending(),
    )
}

fn phase(app: &App) -> WalkPhase {
    (*app.world().resource::<Walk>().phase).clone()
}

fn walked(app: &App, pet: usize) -> u32 {
    app.world().resource::<Pals>().pets[pet].walked_m
}

fn saved_walked(app: &mut App, pet: usize) -> u32 {
    let pending = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    pals::save::load(&SaveBlob::from_storage(pending)).pets[pet].walked_m
}

/// The board's pedometer counting `count` steps, then a frame.
fn take_steps(app: &mut App, count: u32) {
    app.world_mut().resource_mut::<Steps>().add(count);
    update(app);
}

fn dozing(app: &App) -> bool {
    app.world().resource::<Power>().dozing()
}

fn start_walking(app: &mut App) {
    click_action(app);
    assert!(matches!(phase(app), WalkPhase::Walking { pet: 0, .. }));
}

#[test]
fn a_mock_walk_strolls_on_its_own_and_counts_for_the_fish() {
    let mut app = park(&[("GILLY", 0.0)], true);
    assert!(shows(&app, "WHO IS WALKING?"));
    start_walking(&mut app);
    assert!(shows(&app, "MOCK WALK x1"));
    tick(&mut app, 60 * 120);
    let metres = walked(&app, 0);
    assert!((140..=200).contains(&metres), "two minutes made {metres} m");
    assert!(shows(&app, &format!("{metres} M")), "{:?}", labels(&app));
    click_action(&mut app);
    assert!(matches!(phase(&app), WalkPhase::Done { .. }));
    assert!(shows(&app, &format!("{metres} M")));
    assert_eq!(saved_walked(&mut app, 0), metres);
}

#[test]
fn a_real_walk_counts_the_steps_the_board_reports() {
    let mut app = park(&[("GILLY", 0.0)], false);
    start_walking(&mut app);
    for _ in 0..10 {
        take_steps(&mut app, 20);
        tick(&mut app, 30);
    }
    assert_eq!(walked(&app, 0), 150, "200 steps of 0.75 m");
    click_action(&mut app);
    assert!(matches!(phase(&app), WalkPhase::Done { .. }));
    assert!(
        shows(&app, "WALKED 200 STEPS") && shows(&app, "150 M"),
        "{:?}",
        labels(&app)
    );
    assert_eq!(saved_walked(&mut app, 0), 150);
}

#[test]
fn steps_from_before_the_walk_starts_dont_count() {
    let mut app = park(&[("GILLY", 0.0)], false);
    take_steps(&mut app, 500);
    start_walking(&mut app);
    tick(&mut app, 60);
    assert_eq!(walked(&app, 0), 0);
    assert!(shows(&app, "0 STEPS"));
}

#[test]
fn a_few_seconds_in_the_screen_goes_dark_and_steps_still_count() {
    let mut app = park(&[("GILLY", 0.0)], false);
    start_walking(&mut app);
    assert!(!dozing(&app));
    assert!(
        shows(&app, "SCREEN GOES OFF TO SAVE POWER"),
        "{:?}",
        labels(&app)
    );
    tick(&mut app, (DOZE_AFTER_SECS * 60.0) as u32 + 5);
    assert!(dozing(&app));
    assert!(!shows(&app, "STAMINA LV 1"), "the dozing screen is dark");
    take_steps(&mut app, 400);
    click_action(&mut app);
    assert!(matches!(phase(&app), WalkPhase::Done { .. }));
    assert!(!dozing(&app), "the screen comes back for the results");
    assert!(shows(&app, "WALKED 400 STEPS"), "{:?}", labels(&app));
}

#[test]
fn the_boot_button_ends_a_dozing_walk_too() {
    let mut app = park(&[("GILLY", 0.0)], false);
    start_walking(&mut app);
    tick(&mut app, (DOZE_AFTER_SECS * 60.0) as u32 + 5);
    assert!(dozing(&app));
    app.world_mut().resource_mut::<Buttons>().zero.set(true);
    update(&mut app);
    app.world_mut().resource_mut::<Buttons>().zero.set(false);
    update(&mut app);
    assert!(
        matches!(phase(&app), WalkPhase::Done { .. }),
        "{:?}",
        phase(&app)
    );
}

#[test]
fn a_mock_walk_keeps_the_screen_on_for_its_pace_buttons() {
    let mut app = park(&[("GILLY", 0.0)], true);
    start_walking(&mut app);
    tick(&mut app, (DOZE_AFTER_SECS * 60.0) as u32 * 3);
    assert!(!dozing(&app));
    assert!(shows(&app, "MOCK WALK x1"));
}

#[test]
fn walking_far_enough_levels_up_stamina() {
    let mut app = park(&[("GILLY", 90.0)], true);
    start_walking(&mut app);
    tick(&mut app, 60 * 10);
    assert!(shows(&app, "LEVEL UP!"), "{:?}", labels(&app));
    assert!(shows(&app, "STAMINA LV 2"));
}

#[test]
fn turning_picks_which_fish_walks() {
    let mut app = park(&[("GILLY", 0.0), ("BUBBA", 0.0)], true);
    set_tilt(&mut app, 40.0, 0.0);
    update(&mut app);
    click_action(&mut app);
    assert!(matches!(phase(&app), WalkPhase::Walking { pet: 1, .. }));
    tick(&mut app, 60 * 30);
    assert!(walked(&app, 1) > 20 && walked(&app, 0) == 0);
}

#[test]
fn with_no_fish_it_says_so_and_pwr_goes_back_to_town() {
    let mut app = park(&[], true);
    assert!(shows(&app, "NO FISH TO WALK"));
    click_action(&mut app);
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

#[test]
fn done_goes_back_to_choosing_and_back_leaves() {
    let mut app = park(&[("GILLY", 0.0)], true);
    start_walking(&mut app);
    tick(&mut app, 60 * 5);
    click_action(&mut app);
    click_action(&mut app);
    assert_eq!(phase(&app), WalkPhase::Choose);
    set_tilt(&mut app, 60.0, 0.0);
    update(&mut app);
    click_action(&mut app);
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

fn park_at_pace(pace: usize) -> App {
    let mut tank = Pals::default();
    tank.mock_walk = true;
    tank.mock_pace = pace;
    tank.keep(Pet::new(
        1,
        0,
        150,
        50,
        name("GILLY"),
        &mut orbit_core::Rng::new(2),
    ))
    .unwrap();
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    headless(
        |app| {
            app.add_plugins(WalkPlugin);
        },
        blob.take_pending(),
    )
}

#[test]
fn a_faster_mock_walk_covers_more_ground_in_the_same_time() {
    let mut slow = park_at_pace(0);
    start_walking(&mut slow);
    assert!(shows(&slow, "MOCK WALK x1"));
    tick(&mut slow, 60 * 30);
    let mut fast = park_at_pace(3);
    start_walking(&mut fast);
    assert!(shows(&fast, "MOCK WALK x10"));
    tick(&mut fast, 60 * 30);
    let (slow_m, fast_m) = (walked(&slow, 0), walked(&fast, 0));
    assert!((30..=48).contains(&slow_m), "x1 walked {slow_m} m");
    assert!(fast_m > slow_m * 8, "x10 walked {fast_m} m");
}

#[test]
fn faster_and_slower_change_the_pace_mid_walk_and_its_remembered() {
    let mut app = park_at_pace(0);
    start_walking(&mut app);
    tap(&mut app, pals_game::walk::ui::FASTER.center());
    tap(&mut app, pals_game::walk::ui::FASTER.center());
    assert!(shows(&app, "MOCK WALK x5"));
    tap(&mut app, pals_game::walk::ui::SLOWER.center());
    assert!(shows(&app, "MOCK WALK x2"));
    let pending = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    assert_eq!(
        pals::save::load(&SaveBlob::from_storage(pending)).mock_pace,
        1
    );
}

#[test]
fn a_real_walk_has_no_pace_buttons() {
    let mut app = park(&[("GILLY", 0.0)], false);
    start_walking(&mut app);
    assert!(!shows(&app, "+") && !shows(&app, "-"));
}

#[test]
fn tapping_stop_ends_a_walk() {
    let mut app = park(&[("GILLY", 0.0)], true);
    start_walking(&mut app);
    tick(&mut app, 60);
    tap(&mut app, pals_game::walk::ui::STOP.center());
    assert!(
        matches!(phase(&app), WalkPhase::Done { .. }),
        "{:?}",
        phase(&app)
    );
}
