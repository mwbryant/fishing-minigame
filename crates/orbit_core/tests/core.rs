use core::{f32::consts::FRAC_PI_2, time::Duration};

use bevy::{prelude::*, time::TimeUpdateStrategy};
use orbit_core::{Buttons, ControlsPlugin, CorePlugin, ImuFeed, ImuSample, SaveBlob, Tilt};

const DT: f32 = 1.0 / 60.0;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, ControlsPlugin))
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
fn a_game_can_request_a_quiet_rezero_from_any_pose() {
    let mut app = app();
    let mut truth = Quat::from_rotation_x(-FRAC_PI_2);
    for _ in 0..30 {
        turn(&mut app, &mut truth, 70.0);
    }
    assert!(tilt(&app).steer_deg > 30.0);
    app.world_mut().resource_mut::<Tilt>().request_zero();
    app.update();
    assert!(tilt(&app).steer_deg.abs() < 0.01);
    assert!(!tilt(&app).just_zeroed, "only the button shows LEVEL SET");
    for _ in 0..15 {
        turn(&mut app, &mut truth, -40.0);
    }
    let steer = tilt(&app).steer_deg;
    assert!((steer + 10.0).abs() < 2.0, "steer {steer}");
}

#[test]
fn tilt_set_directly_stays_put_without_samples() {
    let mut app = app();
    app.world_mut().resource_mut::<Tilt>().lean_deg = 12.0;
    app.update();
    assert_eq!(tilt(&app).lean_deg, 12.0);
}

#[test]
fn sound_requests_queue_in_order_until_the_platform_drains_them() {
    static BEEP: orbit_core::Clip = orbit_core::Clip { pcm: &[1, 0] };
    let mut queue = orbit_core::SoundQueue::default();
    queue.play(&BEEP);
    queue.stop();
    let drained: Vec<_> = queue.drain().collect();
    assert_eq!(
        drained,
        vec![
            orbit_core::SoundCommand::Play(orbit_core::ClipRef(&BEEP)),
            orbit_core::SoundCommand::Stop
        ]
    );
    assert!(queue.pending().is_empty());
}

#[test]
fn an_undrained_queue_keeps_only_its_newest_commands() {
    static TICK: orbit_core::Clip = orbit_core::Clip { pcm: &[1, 0] };
    let mut queue = orbit_core::SoundQueue::default();
    for _ in 0..100 {
        queue.play(&TICK);
    }
    queue.stop();
    assert_eq!(queue.pending().len(), 16);
    assert_eq!(
        queue.pending().last(),
        Some(&orbit_core::SoundCommand::Stop)
    );
}

#[test]
fn pending_saves_are_handed_over_once() {
    let mut blob = SaveBlob::from_storage(Some(vec![1, 2]));
    assert_eq!(blob.loaded(), Some(&[1u8, 2][..]));
    blob.request_save(vec![9]);
    assert_eq!(blob.take_pending(), Some(vec![9]));
    assert_eq!(blob.take_pending(), None);
}
