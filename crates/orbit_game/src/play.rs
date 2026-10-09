//! The paddle, the ball and the score.

use bevy::{math::ops, prelude::*};

use crate::{Rng, Screen, Touch};

/// Arena geometry and tuning. Positions are pixels relative to the screen
/// center; angles are degrees clockwise from 3 o'clock (y points down).
pub mod arena {
    use bevy::math::Vec2;

    pub const SCREEN_CENTER: Vec2 = Vec2::splat(orbit_gfx::SCREEN_SIZE as f32 / 2.0);
    /// Radius of the paddle's centerline.
    pub const PADDLE_RADIUS: f32 = 212.0;
    pub const PADDLE_THICKNESS: f32 = 14.0;
    /// The paddle's arc.
    pub const PADDLE_SPAN_DEG: f32 = 44.0;
    pub const BALL_RADIUS: f32 = 9.0;
    /// Px per second.
    pub const SERVE_SPEED: f32 = 170.0;
    pub const SERVE_DELAY_SECS: f32 = 0.75;
    /// Each return multiplies the ball's speed by this, up to `MAX_SPEED`.
    pub const SPEEDUP: f32 = 1.06;
    /// Px per second.
    pub const MAX_SPEED: f32 = 480.0;
    /// Extra angle given to returns off the paddle tips, Breakout style.
    pub const EDGE_DEFLECT_DEG: f32 = 30.0;
    /// Returns never leave more than this far off straight-back-inward.
    pub const MAX_RETURN_DEG: f32 = 65.0;
    /// Touches this close to the center don't steer; their angle is noise.
    pub const STEER_DEAD_ZONE: f32 = 24.0;
    /// Center distance at which the ball meets the paddle's inner face.
    pub const CONTACT: f32 = PADDLE_RADIUS - PADDLE_THICKNESS / 2.0 - BALL_RADIUS;
    /// Center distance at which the ball has fully left the arena.
    pub const OUT: f32 = PADDLE_RADIUS + PADDLE_THICKNESS / 2.0 + BALL_RADIUS;
}

use arena::*;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Ball {
    pub pos: Vec2,
    pub vel: Vec2,
    /// Seconds the ball waits at the center before launching.
    pub serve_in: f32,
}

/// The paddle's center on the rim; it starts at 6 o'clock.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Paddle {
    pub angle_deg: f32,
}

impl Default for Paddle {
    fn default() -> Self {
        Self { angle_deg: 90.0 }
    }
}

/// Returns this round, and the best since boot (Orbit saves nothing).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    pub current: u32,
    pub best: u32,
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<Paddle>()
        .init_resource::<Score>()
        .add_systems(Update, steer_paddle)
        .add_systems(OnEnter(Screen::Playing), serve)
        .add_systems(OnExit(Screen::Playing), clear_balls)
        .add_systems(OnEnter(Screen::GameOver), record_best)
        .add_systems(FixedUpdate, move_ball.run_if(in_state(Screen::Playing)));
}

fn steer_paddle(touch: Res<Touch>, mut paddle: ResMut<Paddle>) {
    let Some(finger) = touch.position() else {
        return;
    };
    let offset = finger - SCREEN_CENTER;
    if offset.length() >= STEER_DEAD_ZONE {
        paddle.angle_deg = angle_deg(offset);
    }
}

fn serve(mut commands: Commands, mut score: ResMut<Score>, mut rng: ResMut<Rng>) {
    score.current = 0;
    let heading = rng.next_f32() * core::f32::consts::TAU;
    commands.spawn(Ball {
        pos: Vec2::ZERO,
        vel: Vec2::from_angle(heading) * SERVE_SPEED,
        serve_in: SERVE_DELAY_SECS,
    });
}

fn clear_balls(mut commands: Commands, balls: Query<Entity, With<Ball>>) {
    for ball in &balls {
        commands.entity(ball).despawn();
    }
}

fn record_best(mut score: ResMut<Score>) {
    score.best = score.best.max(score.current);
}

fn move_ball(
    time: Res<Time>,
    paddle: Res<Paddle>,
    mut score: ResMut<Score>,
    mut balls: Query<&mut Ball>,
    mut next: ResMut<NextState<Screen>>,
) {
    let dt = time.delta_secs();
    for mut ball in &mut balls {
        if ball.serve_in > 0.0 {
            ball.serve_in -= dt;
            continue;
        }
        let vel = ball.vel;
        ball.pos += vel * dt;
        let dist = ball.pos.length();
        let outward = ball.pos.dot(ball.vel) > 0.0;
        if outward && (CONTACT..CONTACT + BALL_RADIUS).contains(&dist) {
            let offset = angle_diff_deg(angle_deg(ball.pos), paddle.angle_deg);
            if offset.abs() <= paddle_reach_deg() {
                *ball = bounce(*ball, offset);
                score.current += 1;
            }
        }
        if dist >= OUT {
            next.set(Screen::GameOver);
        }
    }
}

/// Reflects off the paddle face, adds tip deflection, speeds up, and parks the
/// ball on the contact circle so it can't register a second hit.
pub(crate) fn bounce(ball: Ball, offset_deg: f32) -> Ball {
    let normal = ball.pos.normalize();
    let inward = -normal;
    let speed = (ball.vel.length() * SPEEDUP).min(MAX_SPEED);
    let reflected = ball.vel - 2.0 * ball.vel.dot(normal) * normal;
    let deflect = -offset_deg / paddle_reach_deg() * EDGE_DEFLECT_DEG;
    let aimed = reflected
        .normalize()
        .rotate(Vec2::from_angle(deflect.to_radians()));
    let off_axis = inward
        .angle_to(aimed)
        .to_degrees()
        .clamp(-MAX_RETURN_DEG, MAX_RETURN_DEG);
    Ball {
        pos: normal * CONTACT,
        vel: inward.rotate(Vec2::from_angle(off_axis.to_radians())) * speed,
        serve_in: 0.0,
    }
}

/// Half the paddle's span plus the ball's own angular radius at contact.
pub(crate) fn paddle_reach_deg() -> f32 {
    PADDLE_SPAN_DEG / 2.0 + (BALL_RADIUS / CONTACT).to_degrees()
}

pub(crate) fn angle_deg(v: Vec2) -> f32 {
    ops::atan2(v.y, v.x).to_degrees()
}

/// Signed shortest rotation from `to` to `from`, in `(-180, 180]`.
pub(crate) fn angle_diff_deg(from: f32, to: f32) -> f32 {
    let d = ops::rem_euclid(from - to, 360.0);
    if d > 180.0 { d - 360.0 } else { d }
}
