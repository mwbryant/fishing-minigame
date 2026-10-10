use bevy::{app::AppExit, prelude::*};
use orbit_core::{Gps, SaveBlob, testing::*};
use pals::{Name, Pals, Pet};
use pals_game::walk::{Walk, WalkPhase, WalkPlugin};

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

/// A sentence with its checksum, as the receiver sends it.
fn nmea(body: &str) -> String {
    let sum = body.bytes().fold(0u8, |sum, byte| sum ^ byte);
    format!("${body}*{sum:02X}\r\n")
}

/// The receiver's one-second report at `north_m` metres up the street.
fn fix_at(north_m: f64) -> String {
    let minutes = north_m / 1852.0;
    let lat = format!("{:02}{:07.4}", 47, 36.0 + minutes);
    nmea(&format!(
        "GNGGA,120000.00,{lat},N,12220.0000,W,1,09,1.0,50.0,M,0.0,M,,"
    )) + &nmea(&format!(
        "GNRMC,120000.00,A,{lat},N,12220.0000,W,2.7,0.0,071026,,,A,V"
    ))
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
    assert!(shows(&app, &format!("WALKED {metres} M")));
    assert_eq!(saved_walked(&mut app, 0), metres);
}

#[test]
fn a_real_walk_counts_the_metres_the_receiver_reports() {
    let mut app = park(&[("GILLY", 0.0)], false);
    start_walking(&mut app);
    assert!(shows(&app, "LOOKING FOR THE SKY"));
    for second in 0..=100 {
        let text = fix_at(second as f64 * 1.4);
        app.world_mut().resource_mut::<Gps>().push(text.as_bytes());
        tick(&mut app, 60);
    }
    let metres = walked(&app, 0);
    assert!((125..=145).contains(&metres), "walked {metres} m");
    assert!(shows(&app, "SATELLITES 9"));
}

#[test]
fn with_no_sky_for_a_minute_it_keeps_looking_and_suggests_a_mock_walk() {
    let mut app = park(&[("GILLY", 0.0)], false);
    start_walking(&mut app);
    tick(&mut app, 60 * 65);
    let (mut looking, mut mock) = (false, false);
    for _ in 0..60 * 7 {
        update(&mut app);
        looking |= shows(&app, "NO SKY YET, STILL LOOKING");
        mock |= shows(&app, "INDOORS? TRY MOCK WALK");
    }
    assert!(looking && mock, "{:?}", labels(&app));
    assert_eq!(walked(&app, 0), 0);
}

#[test]
fn the_sky_found_after_a_long_wait_still_counts() {
    let mut app = park(&[("GILLY", 0.0)], false);
    start_walking(&mut app);
    tick(&mut app, 60 * 70);
    for second in 0..=30 {
        let text = fix_at(second as f64 * 1.4);
        app.world_mut().resource_mut::<Gps>().push(text.as_bytes());
        tick(&mut app, 60);
    }
    assert!(walked(&app, 0) > 30, "walked {} m", walked(&app, 0));
    assert!(shows(&app, "SATELLITES 9"));
}

#[test]
fn losing_the_sky_mid_walk_goes_back_to_looking() {
    let mut app = park(&[("GILLY", 0.0)], false);
    start_walking(&mut app);
    for second in 0..=10 {
        let text = fix_at(second as f64 * 1.4);
        app.world_mut().resource_mut::<Gps>().push(text.as_bytes());
        tick(&mut app, 60);
    }
    assert!(shows(&app, "SATELLITES 9"));
    tick(&mut app, 60 * 8);
    assert!(shows(&app, "LOOKING FOR THE SKY"), "{:?}", labels(&app));
}

#[test]
fn walking_far_enough_levels_up_stamina() {
    let mut app = park(&[("GILLY", 90.0)], true);
    start_walking(&mut app);
    tick(&mut app, 60 * 20);
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
