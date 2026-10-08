use bevy::{app::AppExit, prelude::*};
use orbit_core::{ClipRef, Launcher, SaveBlob, SoundCommand, SoundQueue, testing::*};
use skate_game::{
    HALFPIPE_SECS, Kind, LAND_DEG, POP_DEG, PX_PER_METER, Phase, Ride, SKATER_X, SPIN_GAIN,
    SkatePlugin, audio, ui,
};

fn game(saved: Option<Vec<u8>>) -> App {
    headless(
        |app| {
            app.add_plugins(SkatePlugin);
        },
        saved,
    )
}

fn ride(app: &App) -> &Ride {
    app.world().resource::<Ride>()
}

fn heard(app: &App, clip: &'static orbit_core::Clip) -> bool {
    app.world()
        .resource::<SoundQueue>()
        .pending()
        .contains(&SoundCommand::Play(ClipRef(clip)))
}

/// Starts rolling on an empty street.
fn roll(app: &mut App) {
    click_action(app);
    assert_eq!(ride(app).phase, Phase::Riding);
    app.world_mut().resource_mut::<Ride>().clear_street();
}

fn pop(app: &mut App) {
    set_tilt(app, -(POP_DEG + 5.0), 0.0);
    tick(app, 3);
    assert!(
        ride(app).airborne(),
        "a counterclockwise turn pops an ollie"
    );
}

#[test]
fn the_title_teaches_the_rotation_controls_and_pwr_starts_rolling() {
    let mut app = game(None);
    assert!(shows(&app, "SKATE"));
    assert!(shows(&app, "TURN LEFT TO OLLIE") && shows(&app, "LEVEL OUT TO LAND"));
    roll(&mut app);
    let start = ride(&app).distance;
    tick(&mut app, 60);
    assert!(ride(&app).distance > start + 150.0);
}

#[test]
fn turning_left_pops_an_ollie_and_levelling_out_lands_it_clean() {
    let mut app = game(None);
    roll(&mut app);
    pop(&mut app);
    assert!(heard(&app, &audio::POP));
    set_tilt(&mut app, 0.0, 0.0);
    tick(&mut app, 60);
    assert!(!ride(&app).airborne());
    assert_eq!(ride(&app).phase, Phase::Riding);
    assert!(heard(&app, &audio::LAND));
    assert!(ride(&app).score >= 10, "a clean landing scores");
}

#[test]
fn landing_with_the_board_crooked_bails() {
    let mut app = game(None);
    roll(&mut app);
    pop(&mut app);
    set_tilt(&mut app, -(LAND_DEG + 15.0), 0.0);
    tick(&mut app, 60);
    assert_eq!(ride(&app).phase, Phase::Bailed);
    assert!(shows(&app, "BAILED!"));
    assert!(heard(&app, &audio::BAIL));
}

#[test]
fn one_ollie_per_turn_the_device_has_to_come_back_first() {
    let mut app = game(None);
    roll(&mut app);
    set_tilt(&mut app, -(POP_DEG + 3.0), 0.0);
    let mut takeoffs = 0;
    let mut was_airborne = false;
    for _ in 0..150 {
        update(&mut app);
        let airborne = ride(&app).airborne();
        takeoffs += usize::from(airborne && !was_airborne);
        was_airborne = airborne;
    }
    assert_eq!(takeoffs, 1);
    assert_eq!(ride(&app).phase, Phase::Riding, "landed within the limit");
}

#[test]
fn rolling_into_a_cone_bails() {
    let mut app = game(None);
    roll(&mut app);
    app.world_mut()
        .resource_mut::<Ride>()
        .place(Kind::Cone, SKATER_X + 60.0);
    tick(&mut app, 60);
    assert_eq!(ride(&app).phase, Phase::Bailed);
}

#[test]
fn an_ollie_clears_a_cone() {
    let mut app = game(None);
    roll(&mut app);
    app.world_mut()
        .resource_mut::<Ride>()
        .place(Kind::Cone, SKATER_X + 110.0);
    pop(&mut app);
    set_tilt(&mut app, 0.0, 0.0);
    tick(&mut app, 90);
    assert_eq!(ride(&app).phase, Phase::Riding);
}

#[test]
fn landing_on_a_bench_grinds_it_for_points_then_drops_off() {
    let mut app = game(None);
    roll(&mut app);
    app.world_mut()
        .resource_mut::<Ride>()
        .place(Kind::Bench, SKATER_X + 90.0);
    pop(&mut app);
    set_tilt(&mut app, 0.0, 0.0);
    assert!(run_until_grinding(&mut app), "the ollie lands on the bench");
    assert!(shows(&app, "GRIND!"));
    let score = ride(&app).score;
    tick(&mut app, 15);
    assert!(ride(&app).score > score, "grinding scores as it goes");
    tick(&mut app, 120);
    assert!(ride(&app).grinding().is_none() && !ride(&app).airborne());
    assert_eq!(ride(&app).phase, Phase::Riding);
}

fn run_until_grinding(app: &mut App) -> bool {
    for _ in 0..90 {
        if ride(app).grinding().is_some() {
            return true;
        }
        update(app);
    }
    false
}

/// Pops when the next obstacle is just ahead of the board, levels otherwise.
fn steady_rider(app: &mut App) {
    let ride = ride(app);
    let front = SKATER_X + skate_game::BOARD_HALF;
    let close = ride
        .obstacles()
        .iter()
        .any(|o| (0.0..55.0).contains(&(o.x - front)));
    let steer = if close && !ride.airborne() {
        -(POP_DEG + 5.0)
    } else {
        0.0
    };
    set_tilt(app, steer, 0.0);
    update(app);
}

#[test]
fn a_rider_who_pops_before_each_obstacle_keeps_going_through_the_street_and_halfpipe() {
    let mut app = game(None);
    click_action(&mut app);
    let slow = ride(&app).speed;
    let mut visited_halfpipe = false;
    for _ in 0..60 * 30 {
        steady_rider(&mut app);
        assert_eq!(
            ride(&app).phase,
            Phase::Riding,
            "bailed at {}",
            ride(&app).distance
        );
        visited_halfpipe |= ride(&app).in_halfpipe();
    }
    assert!(visited_halfpipe, "150 m in, it drops into the halfpipe");
    assert!(ride(&app).spawned >= 8, "{} obstacles", ride(&app).spawned);
    assert!(ride(&app).speed > slow + 100.0);
}

#[test]
fn the_best_score_is_saved_and_survives_a_reboot() {
    let mut app = game(None);
    roll(&mut app);
    tick(&mut app, 90);
    app.world_mut()
        .resource_mut::<Ride>()
        .place(Kind::Cone, SKATER_X + 40.0);
    tick(&mut app, 30);
    assert_eq!(ride(&app).phase, Phase::Bailed);
    let score = ride(&app).score;
    assert!(score > 0 && shows(&app, "NEW BEST!"));
    let record = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    let rebooted = game(record);
    assert_eq!(ride(&rebooted).best, score);
    assert!(shows(&rebooted, &format!("BEST {score}")));
}

#[test]
fn launched_from_the_town_the_title_offers_a_way_out() {
    let app = game(None);
    assert!(!shows(&app, "OUT"));
    let mut app = headless(
        |app| {
            app.add_plugins(SkatePlugin).insert_resource(Launcher);
        },
        None,
    );
    tap(&mut app, ui::HOME.center());
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

fn drop_in(app: &mut App) {
    roll(app);
    app.world_mut().resource_mut::<Ride>().approach_halfpipe();
    tick(app, 5);
    assert!(
        ride(app).in_halfpipe(),
        "every 150 m the street drops into a halfpipe"
    );
}

/// Waits (level) until the skater leaves the coping.
fn until_air(app: &mut App) {
    set_tilt(app, 0.0, 0.0);
    for _ in 0..60 * 3 {
        if ride(app).halfpipe_air() {
            return;
        }
        update(app);
    }
    panic!("never got air in the halfpipe");
}

fn until_landed(app: &mut App) {
    for _ in 0..60 * 2 {
        if !ride(app).halfpipe_air() {
            return;
        }
        update(app);
    }
    panic!("never came back down");
}

#[test]
fn every_150_metres_the_street_drops_into_a_halfpipe() {
    let mut app = game(None);
    roll(&mut app);
    let first = ride(&app).next_halfpipe_m();
    assert!((140.0..=160.0).contains(&first), "{first} m");
    assert!(!ride(&app).in_halfpipe());
    drop_in(&mut app);
    assert!(shows(&app, "DROP IN!"));
    assert!(ride(&app).obstacles().is_empty(), "the street is gone");
}

#[test]
fn the_halfpipe_lasts_about_seven_seconds_then_its_back_to_the_street() {
    let mut app = game(None);
    drop_in(&mut app);
    let at = ride(&app).distance;
    tick(&mut app, (60.0 * (HALFPIPE_SECS - 1.0)) as u32);
    assert!(ride(&app).in_halfpipe());
    tick(&mut app, 60 * 3);
    assert!(!ride(&app).in_halfpipe());
    assert_eq!(ride(&app).phase, Phase::Riding);
    let next = ride(&app).next_halfpipe_m();
    assert!(next >= at / PX_PER_METER + 140.0, "next drop at {next} m");
}

#[test]
fn turning_in_the_air_spins_a_trick_and_landing_it_scores() {
    let mut app = game(None);
    drop_in(&mut app);
    until_air(&mut app);
    let before = ride(&app).score;
    set_tilt(&mut app, 360.0 / SPIN_GAIN, 0.0);
    until_landed(&mut app);
    assert!(shows(&app, "360! +35"), "{:?}", labels(&app));
    assert!(ride(&app).score >= before + 35);
    assert!(heard(&app, &audio::LAND));
}

#[test]
fn a_plain_air_scores_a_little() {
    let mut app = game(None);
    drop_in(&mut app);
    until_air(&mut app);
    until_landed(&mut app);
    assert!(shows(&app, "AIR! +5"), "{:?}", labels(&app));
}

#[test]
fn landing_a_spin_crooked_slams_but_the_run_goes_on() {
    let mut app = game(None);
    drop_in(&mut app);
    until_air(&mut app);
    let before = ride(&app).score;
    set_tilt(&mut app, 90.0 / SPIN_GAIN, 0.0);
    until_landed(&mut app);
    assert!(shows(&app, "SLAM!"));
    assert_eq!(ride(&app).score, before, "no points for a slam");
    assert_eq!(ride(&app).phase, Phase::Riding);
    assert!(ride(&app).in_halfpipe());
}
