use bevy::prelude::*;
use orbit_game::{
    Ball, Frame, Paddle, Score, Screen,
    arena::*,
    testing::{headless_app, press, tap, tick},
};
use orbit_gfx::Shape;

fn screen(app: &App) -> Screen {
    *app.world().resource::<State<Screen>>().get()
}

fn ball(app: &mut App) -> Ball {
    let world = app.world_mut();
    *world.query::<&Ball>().single(world).unwrap()
}

fn place_ball(app: &mut App, pos: Vec2, vel: Vec2) {
    let world = app.world_mut();
    let mut ball = world.query::<&mut Ball>().single_mut(world).unwrap();
    *ball = Ball {
        pos,
        vel,
        serve_in: 0.0,
    };
}

fn set_paddle(app: &mut App, angle_deg: f32) {
    app.world_mut().resource_mut::<Paddle>().angle_deg = angle_deg;
}

fn at_angle(deg: f32, dist: f32) -> Vec2 {
    Vec2::from_angle(deg.to_radians()) * dist
}

/// Tap to start, then park the ball just inside the paddle at `deg`,
/// heading straight out.
fn ball_heading_out_at(app: &mut App, deg: f32) {
    tap(app, SCREEN_CENTER);
    place_ball(app, at_angle(deg, CONTACT - 2.0), at_angle(deg, 200.0));
}

fn lose_round(app: &mut App) {
    ball_heading_out_at(app, 0.0);
    set_paddle(app, 180.0);
    tick(app, 30);
}

#[test]
fn the_game_boots_to_the_title_screen() {
    let app = headless_app();
    assert_eq!(screen(&app), Screen::Title);
}

#[test]
fn tapping_the_title_starts_a_round_with_the_ball_at_the_center() {
    let mut app = headless_app();
    tap(&mut app, SCREEN_CENTER);
    assert_eq!(screen(&app), Screen::Playing);
    assert_eq!(ball(&mut app).pos, Vec2::ZERO);
}

#[test]
fn the_paddle_follows_the_finger_around_the_rim() {
    let mut app = headless_app();
    for (offset, expected) in [
        (Vec2::new(100.0, 0.0), 0.0),
        (Vec2::new(0.0, 100.0), 90.0),
        (Vec2::new(-100.0, 0.0), 180.0),
        (Vec2::new(0.0, -100.0), -90.0),
    ] {
        press(&mut app, Some(SCREEN_CENTER + offset));
        let angle = app.world().resource::<Paddle>().angle_deg;
        assert!((angle - expected).abs() < 0.01, "{offset} -> {angle}");
    }
}

#[test]
fn touches_near_the_center_do_not_steer() {
    let mut app = headless_app();
    press(&mut app, Some(SCREEN_CENTER + Vec2::new(100.0, 0.0)));
    press(&mut app, Some(SCREEN_CENTER + Vec2::new(0.0, -5.0)));
    assert_eq!(app.world().resource::<Paddle>().angle_deg, 0.0);
}

#[test]
fn a_served_ball_waits_at_the_center_then_launches_at_serve_speed() {
    let mut app = headless_app();
    tap(&mut app, SCREEN_CENTER);
    tick(&mut app, 30);
    assert_eq!(ball(&mut app).pos, Vec2::ZERO);
    tick(&mut app, 30);
    let ball = ball(&mut app);
    assert!(ball.pos.length() > 0.0);
    assert!((ball.vel.length() - SERVE_SPEED).abs() < 0.01);
}

#[test]
fn a_ball_meeting_the_paddle_comes_back_faster_and_scores() {
    let mut app = headless_app();
    ball_heading_out_at(&mut app, 30.0);
    set_paddle(&mut app, 30.0);
    tick(&mut app, 5);
    let ball = ball(&mut app);
    assert!(ball.pos.dot(ball.vel) < 0.0, "ball still heading out");
    assert!(ball.vel.length() > 200.0);
    assert_eq!(app.world().resource::<Score>().current, 1);
    assert_eq!(screen(&app), Screen::Playing);
}

#[test]
fn a_ball_that_misses_the_paddle_ends_the_round() {
    let mut app = headless_app();
    lose_round(&mut app);
    assert_eq!(screen(&app), Screen::GameOver);
}

#[test]
fn game_over_records_the_best_score_and_retry_resets_the_current_one() {
    let mut app = headless_app();
    ball_heading_out_at(&mut app, 0.0);
    set_paddle(&mut app, 0.0);
    tick(&mut app, 5);
    place_ball(
        &mut app,
        at_angle(90.0, CONTACT - 2.0),
        at_angle(90.0, 200.0),
    );
    set_paddle(&mut app, -90.0);
    tick(&mut app, 30);
    assert_eq!(screen(&app), Screen::GameOver);
    assert_eq!(
        *app.world().resource::<Score>(),
        Score {
            current: 1,
            best: 1
        }
    );

    tick(&mut app, 60);
    tap(&mut app, SCREEN_CENTER);
    assert_eq!(screen(&app), Screen::Playing);
    assert_eq!(app.world().resource::<Score>().current, 0);
    assert_eq!(app.world().resource::<Score>().best, 1);
}

#[test]
fn taps_right_after_losing_do_not_restart() {
    let mut app = headless_app();
    lose_round(&mut app);
    tap(&mut app, SCREEN_CENTER);
    assert_eq!(screen(&app), Screen::GameOver);
}

#[test]
fn holding_the_finger_down_through_game_over_needs_a_fresh_tap() {
    let mut app = headless_app();
    lose_round(&mut app);
    press(&mut app, Some(SCREEN_CENTER));
    tick(&mut app, 120);
    assert_eq!(screen(&app), Screen::GameOver);
}

#[test]
fn returns_from_anywhere_on_the_paddle_head_back_inside_within_the_return_cone() {
    for offset in [-24.0, -20.0, -10.0, -3.0, 0.0, 3.0, 10.0, 20.0, 24.0] {
        let mut app = headless_app();
        ball_heading_out_at(&mut app, 45.0 + offset);
        set_paddle(&mut app, 45.0);
        tick(&mut app, 5);
        assert_eq!(
            app.world().resource::<Score>().current,
            1,
            "offset {offset} missed"
        );
        let ball = ball(&mut app);
        let off_axis = (-ball.pos).angle_to(ball.vel).to_degrees().abs();
        assert!(
            off_axis <= MAX_RETURN_DEG + 0.01,
            "offset {offset}: {off_axis} deg"
        );
    }
}

#[test]
fn hitting_a_paddle_tip_sends_the_ball_toward_that_side() {
    let mut app = headless_app();
    ball_heading_out_at(&mut app, 90.0 + 15.0);
    set_paddle(&mut app, 90.0);
    tick(&mut app, 5);
    let ball = ball(&mut app);
    assert!(
        ball.vel.x < 0.0,
        "hit left of a bottom paddle, went {}",
        ball.vel
    );
}

#[test]
fn the_frame_draws_the_rim_score_paddle_and_ball_during_play() {
    let mut app = headless_app();
    tap(&mut app, SCREEN_CENTER);
    let shapes = &app.world().resource::<Frame>().shapes;
    assert!(matches!(shapes.first(), Some(Shape::Ring { .. })));
    assert!(
        shapes
            .iter()
            .any(|s| matches!(s, Shape::Label { text, .. } if text == "0"))
    );
    assert!(shapes.iter().any(|s| matches!(s, Shape::Arc { .. })));
    assert!(matches!(shapes.last(), Some(Shape::Disc { .. })));
}

#[test]
fn the_title_frame_invites_a_tap() {
    let app = headless_app();
    let shapes = &app.world().resource::<Frame>().shapes;
    assert!(
        shapes
            .iter()
            .any(|s| matches!(s, Shape::Label { text, .. } if text == "TAP TO PLAY"))
    );
}
