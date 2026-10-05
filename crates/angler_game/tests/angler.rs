use angler_game::{
    FishLog, Phase, Session,
    cast::{self, CHARGE_SECS, Flight},
    reel::{LEAN_DEAD_ZONE_DEG, LEAN_FULL_DEG},
    session::{ESCAPED_SECS, NOTICE_SECS, RESULT_GUARD_SECS},
    testing::*,
};
use bevy::prelude::*;
use orbit_core::{Frame, SaveBlob};
use orbit_gfx::{Shape, SpriteRef};

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
        app.update();
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
fn tapping_the_title_opens_the_fish_log_and_tapping_again_closes_it() {
    let mut app = headless_app();
    tap(&mut app, SCREEN_CENTER);
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
    app.update();
    assert!(shows(&app, "ANGLE 70"));
    set_tilt(&mut app, 20.0, 0.0);
    app.update();
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

#[test]
fn charging_shows_the_power_gauge_and_dotted_path() {
    let mut app = headless_app();
    start_aiming(&mut app);
    hold_action(&mut app, frames_for(0.6));
    let shapes = &app.world().resource::<Frame>().shapes;
    assert!(shapes.iter().any(|s| matches!(s, Shape::Arc { .. })));
    let dots = shapes
        .iter()
        .filter(|s| matches!(s, Shape::Disc { radius: 2, .. }))
        .count();
    assert!(dots >= 5, "{dots} preview dots");
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
    assert_eq!(saved.and_then(|b| FishLog::decode(&b)), Some(log));
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
    assert!(matches!(phase(&app), Phase::Aim { .. }));
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
