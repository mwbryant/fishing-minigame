use core::{f32::consts::FRAC_PI_2, time::Duration};

use bevy::{prelude::*, time::TimeUpdateStrategy};
use orbit_core::{Buttons, CorePlugin, ImuFeed, ImuSample, SaveBlob, Tilt};

const DT: f32 = 1.0 / 60.0;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(CorePlugin)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            DT,
        )));
    app.finish();
    app.cleanup();
    app.update();
    app
}

/// Turns an upright device about its screen normal at `deg_per_sec` for one
/// frame, feeding the matching IMU sample.
fn turn(app: &mut App, truth: &mut Quat, deg_per_sec: f32) {
    let rate = Vec3::Z * deg_per_sec.to_radians();
    *truth = (*truth * Quat::from_scaled_axis(rate * DT)).normalize();
    app.world_mut().resource_mut::<ImuFeed>().push(ImuSample {
        accel: truth.inverse() * Vec3::Z,
        gyro: rate,
        dt: DT,
    });
    app.update();
}

fn tilt(app: &App) -> Tilt {
    *app.world().resource::<Tilt>()
}

fn set_zero_button(app: &mut App, held: bool) {
    app.world_mut().resource_mut::<Buttons>().zero.set(held);
    app.update();
}

/// What game systems saw of the action button, one entry per update.
#[derive(Resource, Default)]
struct Seen(Vec<(bool, bool, bool)>);

fn record_action(buttons: Res<Buttons>, mut seen: ResMut<Seen>) {
    let action = buttons.action;
    seen.0
        .push((action.pressed(), action.held(), action.released()));
}

#[test]
fn systems_see_each_button_edge_for_exactly_one_update() {
    let mut app = app();
    app.init_resource::<Seen>()
        .add_systems(Update, record_action);
    for held in [true, true, false, false] {
        app.world_mut().resource_mut::<Buttons>().action.set(held);
        app.update();
    }
    assert_eq!(
        app.world().resource::<Seen>().0,
        vec![
            (true, true, false),
            (false, true, false),
            (false, false, true),
            (false, false, false),
        ]
    );
}

#[test]
fn imu_samples_drive_the_tilt_resource() {
    let mut app = app();
    let mut truth = Quat::from_rotation_x(-FRAC_PI_2);
    for _ in 0..30 {
        turn(&mut app, &mut truth, 60.0);
    }
    let steer = tilt(&app).steer_deg;
    assert!((steer - 30.0).abs() < 2.0, "steer {steer}");
}

#[test]
fn the_zero_button_rezeroes_from_any_pose() {
    let mut app = app();
    let mut truth = Quat::from_rotation_x(-FRAC_PI_2);
    for _ in 0..30 {
        turn(&mut app, &mut truth, -80.0);
    }
    assert!(tilt(&app).steer_deg < -30.0);
    set_zero_button(&mut app, true);
    assert!(tilt(&app).just_zeroed);
    assert!(tilt(&app).steer_deg.abs() < 0.01);
    set_zero_button(&mut app, false);
    assert!(!tilt(&app).just_zeroed);
    for _ in 0..15 {
        turn(&mut app, &mut truth, 40.0);
    }
    let steer = tilt(&app).steer_deg;
    assert!((steer - 10.0).abs() < 2.0, "steer {steer}");
}

#[test]
fn tilt_set_directly_stays_put_without_samples() {
    let mut app = app();
    app.world_mut().resource_mut::<Tilt>().lean_deg = 12.0;
    app.update();
    assert_eq!(tilt(&app).lean_deg, 12.0);
}

#[test]
fn pending_saves_are_handed_over_once() {
    let mut blob = SaveBlob::from_storage(Some(vec![1, 2]));
    assert_eq!(blob.loaded(), Some(&[1u8, 2][..]));
    blob.request_save(vec![9]);
    assert_eq!(blob.take_pending(), Some(vec![9]));
    assert_eq!(blob.take_pending(), None);
}
