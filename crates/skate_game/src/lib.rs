//! Skate: an endless street side-scroller where rotation is the board. Turn
//! the device counterclockwise (nose up) to pop an ollie. In the air the board
//! follows the turn, and it has to come down within `LAND_DEG` of level or you
//! bail. Clear cones, clear or grind benches, and the street speeds up.
//!
//! Every `HALFPIPE_EVERY_M` metres the street drops into a halfpipe for a
//! freestyle section: air off each wall and turn the device to spin tricks.
//! The score is metres travelled plus 10 per clean landing, 25 a second of
//! grinding, and the `TRICKS` points. The best persists in the `SKAT` section.

#![no_std]

extern crate alloc;

pub mod audio;
mod draw;
mod save;
mod sprites;
pub mod ui;

use alloc::vec::Vec;

use core::f32::consts::FRAC_PI_2;

use bevy::{app::AppExit, math::ops, prelude::*};
use orbit_core::{
    Buttons, Clip, ControlsPlugin, CorePlugin, Launcher, Power, Rng, SaveBlob, SoundPlugin,
    SoundQueue, Tilt, Touch,
};
use orbit_gfx::Rgb565;
use orbit_ui::CLICK;

/// A counterclockwise turn this far pops an ollie.
pub const POP_DEG: f32 = 15.0;
/// The turn has to ease back under this before the next ollie can pop.
pub const REARM_DEG: f32 = 7.0;
/// Coming down with the board further from level than this bails.
pub const LAND_DEG: f32 = 25.0;
/// Screen x of the board's middle.
pub const SKATER_X: f32 = 130.0;
/// Screen y of the street's top edge.
pub const GROUND_Y: f32 = 316.0;
/// Half the board, for collisions.
pub const BOARD_HALF: f32 = 22.0;
/// Upward speed of an ollie, px/s.
pub const POP_SPEED: f32 = 560.0;
/// Px/s^2, on the street and in the halfpipe.
pub const GRAVITY: f32 = 1500.0;
/// Street speed at the start, px/s.
pub const START_SPEED: f32 = 220.0;
/// Street speed gained per second, px/s.
pub const SPEED_GAIN: f32 = 6.0;
/// Top street speed, px/s.
pub const MAX_SPEED: f32 = 420.0;
const CLEAN_BONUS: f32 = 10.0;
const GRIND_PER_SEC: f32 = 25.0;
const GRIND_TICK_SECS: f32 = 0.12;
/// Obstacles appear here, past the right edge of the glass.
const SPAWN_X: f32 = 480.0;
/// Px of street before the first obstacle, at the start and after a halfpipe.
const FIRST_SPAWN: f32 = 260.0;
/// Results ignore presses this long, so a panicked press can't skip them.
pub const RESULT_GUARD_SECS: f32 = 0.8;
const TOAST_SECS: f32 = 0.8;
/// Screen px per metre of street, for the score and the halfpipe spacing.
pub const PX_PER_METER: f32 = 30.0;
/// Metres of street between halfpipes.
pub const HALFPIPE_EVERY_M: f32 = 150.0;
/// Freestyle lasts at least this long, then the skater rolls out at the
/// bottom of the pipe.
pub const HALFPIPE_SECS: f32 = 7.0;
/// Center of the circle whose bottom half is the halfpipe.
pub const PIPE_CENTER: Vec2 = Vec2::new(233.0, 230.0);
/// Radius of the halfpipe, px.
pub const PIPE_RADIUS: f32 = 175.0;
/// Speed leaving the coping, the same every time, so every wall gives the
/// same air (about 0.7 s).
pub const AIR_SPEED: f32 = 520.0;
/// In the halfpipe's air the skater spins this many times the device's turn,
/// so about 60 degrees of wrist is a 360.
pub const SPIN_GAIN: f32 = 6.0;
/// A spin lands if it's within this of a whole half turn.
pub const LAND_SPIN_DEG: f32 = 50.0;
/// Name and points by half turns spun: none, 180, 360, 540, 720 or more.
pub const TRICKS: [(&str, u32); 5] = [
    ("AIR!", 5),
    ("180!", 15),
    ("360!", 35),
    ("540!", 60),
    ("720!", 90),
];
/// No street obstacles spawn within this many px of the drop into the halfpipe.
const SPAWN_STOP_BEFORE_PIPE: f32 = 600.0;
pub const BACKGROUND: Rgb565 = orbit_ui::rgb(12, 14, 34);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Title,
    Riding,
    Bailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Cone,
    Bench,
}

impl Kind {
    /// Width and height in screen px.
    pub fn size(self) -> (f32, f32) {
        match self {
            Kind::Cone => (18.0, 21.0),
            Kind::Bench => (120.0, 36.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obstacle {
    pub kind: Kind,
    /// Left edge in screen px; the street scrolls it left.
    pub x: f32,
    id: u32,
}

impl Obstacle {
    fn under_board(&self) -> bool {
        let (width, _) = self.kind.size();
        self.x < SKATER_X + BOARD_HALF && self.x + width > SKATER_X - BOARD_HALF
    }
}

/// A freestyle section in the halfpipe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Halfpipe {
    /// Seconds since dropping in.
    pub secs: f32,
    /// Where on the pipe: radians from the bottom, positive toward the right wall.
    pub angle: f32,
    /// Radians per second.
    pub angular_speed: f32,
    pub air: Option<Air>,
}

/// Off the coping: straight up and back down the same wall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Air {
    /// Px above the coping.
    pub height: f32,
    /// Upward speed, px/s.
    pub climb: f32,
    /// +1 off the right wall, -1 off the left.
    pub side: f32,
    /// Degrees spun clockwise: the device's turn since takeoff times
    /// `SPIN_GAIN`.
    pub spin_deg: f32,
    steer_at_takeoff: f32,
}

/// How the last halfpipe landing went, for the toast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trick {
    /// An index into `TRICKS`.
    Landed(usize),
    Slam,
}

/// The ride in play, and the best score.
#[derive(Resource, Debug)]
pub struct Ride {
    pub phase: Phase,
    pub phase_secs: f32,
    /// Street speed, px/s.
    pub speed: f32,
    /// Street travelled this ride, in px.
    pub distance: f32,
    pub score: u32,
    pub best: u32,
    pub new_best: bool,
    /// The board's height above the street, px.
    pub height: f32,
    /// The board's upward speed, px/s.
    pub climb: f32,
    /// The board's angle, degrees clockwise: the device's turn, clamped to
    /// +-80.
    pub board_deg: f32,
    /// Obstacles spawned this ride; each one's id is its count.
    pub spawned: u32,
    /// A message (CLEAN!, GRIND!, DROP IN!) and its seconds left.
    pub toast: Option<(&'static str, f32)>,
    /// The last halfpipe landing and its seconds left on screen.
    pub trick: Option<(Trick, f32)>,
    pub halfpipe: Option<Halfpipe>,
    obstacles: Vec<Obstacle>,
    next_halfpipe: f32,
    airborne: bool,
    grinding: Option<u32>,
    armed: bool,
    bonus: f32,
    next_spawn: f32,
    grind_tick: f32,
}

impl Default for Ride {
    fn default() -> Self {
        Self {
            phase: Phase::Title,
            phase_secs: 0.0,
            speed: START_SPEED,
            distance: 0.0,
            score: 0,
            best: 0,
            new_best: false,
            height: 0.0,
            climb: 0.0,
            board_deg: 0.0,
            spawned: 0,
            toast: None,
            trick: None,
            halfpipe: None,
            obstacles: Vec::new(),
            next_halfpipe: HALFPIPE_EVERY_M * PX_PER_METER,
            airborne: false,
            grinding: None,
            armed: true,
            bonus: 0.0,
            next_spawn: FIRST_SPAWN,
            grind_tick: 0.0,
        }
    }
}

impl Ride {
    pub fn airborne(&self) -> bool {
        self.airborne
    }

    /// The bench being ground, if any.
    pub fn grinding(&self) -> Option<u32> {
        self.grinding
    }

    pub fn obstacles(&self) -> &[Obstacle] {
        &self.obstacles
    }

    pub fn in_halfpipe(&self) -> bool {
        self.halfpipe.is_some()
    }

    /// Up off a halfpipe wall.
    pub fn halfpipe_air(&self) -> bool {
        self.halfpipe.is_some_and(|pipe| pipe.air.is_some())
    }

    /// Where the next drop into a halfpipe is, in metres of street.
    pub fn next_halfpipe_m(&self) -> f32 {
        self.next_halfpipe / PX_PER_METER
    }

    /// Rolls the ride to just before the next halfpipe on an empty street,
    /// for tests.
    pub fn approach_halfpipe(&mut self) {
        self.distance = self.next_halfpipe - 10.0;
        self.clear_street();
    }

    /// Empties the street, for tests and a fresh start.
    pub fn clear_street(&mut self) {
        self.obstacles.clear();
        self.next_spawn = self.distance + 2000.0;
    }

    /// Puts one obstacle on an otherwise empty street, for tests.
    pub fn place(&mut self, kind: Kind, x: f32) {
        self.clear_street();
        self.spawn(kind, x);
    }

    fn spawn(&mut self, kind: Kind, x: f32) {
        self.spawned += 1;
        self.obstacles.push(Obstacle {
            kind,
            x,
            id: self.spawned,
        });
    }

    fn start(&mut self) {
        *self = Self {
            best: self.best,
            phase: Phase::Riding,
            ..Self::default()
        };
    }

    fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_secs = 0.0;
    }

    fn drop_in(&mut self) {
        self.obstacles.clear();
        self.halfpipe = Some(Halfpipe {
            secs: 0.0,
            angle: -FRAC_PI_2,
            angular_speed: AIR_SPEED / PIPE_RADIUS,
            air: None,
        });
        self.toast = Some(("DROP IN!", TOAST_SECS));
    }

    /// One step of the halfpipe: ride the pipe like a pendulum, air off each
    /// wall, spin with the device in the air, judge the landing; after
    /// `HALFPIPE_SECS`, roll out at the bottom back onto the street.
    fn freestyle(&mut self, steer: f32, dt: f32) -> Option<&'static Clip> {
        let mut pipe = self.halfpipe?;
        let mut sfx = None;
        pipe.secs += dt;
        match pipe.air.as_mut() {
            Some(air) => {
                air.climb -= GRAVITY * dt;
                air.height += air.climb * dt;
                air.spin_deg = (steer - air.steer_at_takeoff) * SPIN_GAIN;
                if air.height <= 0.0 {
                    let (side, spin) = (air.side, air.spin_deg);
                    pipe.air = None;
                    pipe.angle = side * FRAC_PI_2;
                    pipe.angular_speed = -side * AIR_SPEED / PIPE_RADIUS;
                    let half_turns = ops::round(spin.abs() / 180.0);
                    if (spin.abs() - half_turns * 180.0).abs() <= LAND_SPIN_DEG {
                        let trick = (half_turns as usize).min(TRICKS.len() - 1);
                        self.bonus += TRICKS[trick].1 as f32;
                        self.trick = Some((Trick::Landed(trick), TOAST_SECS * 1.5));
                        sfx = Some(&audio::LAND);
                    } else {
                        self.trick = Some((Trick::Slam, TOAST_SECS * 1.5));
                        sfx = Some(&audio::BAIL);
                    }
                }
            }
            None => {
                pipe.angular_speed -= GRAVITY / PIPE_RADIUS * ops::sin(pipe.angle) * dt;
                pipe.angle += pipe.angular_speed * dt;
                let side = pipe.angle.signum();
                if pipe.angle.abs() >= FRAC_PI_2 && pipe.angular_speed * side > 0.0 {
                    pipe.angle = side * FRAC_PI_2;
                    pipe.air = Some(Air {
                        height: 0.0,
                        climb: AIR_SPEED,
                        side,
                        spin_deg: 0.0,
                        steer_at_takeoff: steer,
                    });
                    sfx = Some(&audio::POP);
                }
            }
        }
        if pipe.secs >= HALFPIPE_SECS && pipe.air.is_none() && pipe.angle.abs() < 0.35 {
            self.halfpipe = None;
            self.clear_street();
            self.next_spawn = self.distance + FIRST_SPAWN;
            self.next_halfpipe = self.distance + HALFPIPE_EVERY_M * PX_PER_METER;
        } else {
            self.halfpipe = Some(pipe);
        }
        sfx
    }
}

pub struct SkatePlugin;

impl Plugin for SkatePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<CorePlugin>() {
            app.add_plugins(CorePlugin);
        }
        if !app.is_plugin_added::<ControlsPlugin>() {
            app.add_plugins(ControlsPlugin);
        }
        if !app.is_plugin_added::<SoundPlugin>() {
            app.add_plugins(SoundPlugin);
        }
        app.init_resource::<Ride>()
            .add_systems(Startup, load)
            .add_systems(Update, menus)
            .add_systems(FixedUpdate, simulate)
            .add_systems(PostUpdate, draw::draw_frame);
    }
}

fn load(blob: Res<SaveBlob>, mut ride: ResMut<Ride>) {
    if let Some(best) = blob.section(&save::KEY).and_then(save::decode) {
        ride.best = best;
    }
}

/// Starting a ride, and HOME, from the title and the bail screen.
#[allow(clippy::too_many_arguments)]
fn menus(
    time: Res<Time>,
    buttons: Res<Buttons>,
    touch: Res<Touch>,
    power: Res<Power>,
    launcher: Option<Res<Launcher>>,
    mut tilt: ResMut<Tilt>,
    mut ride: ResMut<Ride>,
    mut sound: ResMut<SoundQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    ride.phase_secs += time.delta_secs();
    if power.saying_good_night() || ride.phase == Phase::Riding {
        return;
    }
    let tap = touch.just_pressed().then(|| touch.position()).flatten();
    let open = ride.phase == Phase::Title || ride.phase_secs >= RESULT_GUARD_SECS;
    if launcher.is_some() && tap.is_some_and(|at| ui::HOME.hit(at)) {
        sound.play(&CLICK);
        exit.write(AppExit::Success);
    } else if open && (buttons.action.pressed() || tap.is_some()) {
        tilt.request_zero();
        ride.start();
        sound.play(&CLICK);
    }
}

#[allow(clippy::too_many_arguments)]
fn simulate(
    time: Res<Time>,
    tilt: Res<Tilt>,
    power: Res<Power>,
    mut ride: ResMut<Ride>,
    mut rng: ResMut<Rng>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
) {
    if ride.phase != Phase::Riding || power.saying_good_night() {
        return;
    }
    let dt = time.delta_secs();
    let mut sfx: Option<&'static Clip> = None;
    if let Some((_, left)) = ride.toast.as_mut() {
        *left -= dt;
    }
    ride.toast = ride.toast.filter(|(_, left)| *left > 0.0);
    if let Some((_, left)) = ride.trick.as_mut() {
        *left -= dt;
    }
    ride.trick = ride.trick.filter(|(_, left)| *left > 0.0);

    if ride.halfpipe.is_some() {
        sfx = ride.freestyle(tilt.steer_deg, dt);
        ride.board_deg = 0.0;
        ride.score = (ride.distance / PX_PER_METER) as u32 + ride.bonus as u32;
        if let Some(clip) = sfx {
            sound.play(clip);
        }
        return;
    }

    ride.speed = (ride.speed + SPEED_GAIN * dt).min(MAX_SPEED);
    let travel = ride.speed * dt;
    ride.distance += travel;
    for obstacle in &mut ride.obstacles {
        obstacle.x -= travel;
    }
    ride.obstacles
        .retain(|obstacle| obstacle.x + obstacle.kind.size().0 > -40.0);
    if ride.distance >= ride.next_spawn
        && ride.distance < ride.next_halfpipe - SPAWN_STOP_BEFORE_PIPE
    {
        let kind = if rng.next_f32() < 0.6 {
            Kind::Cone
        } else {
            Kind::Bench
        };
        ride.spawn(kind, SPAWN_X);
        let gap = (300.0 + 300.0 * rng.next_f32()) * ride.speed / START_SPEED;
        ride.next_spawn = ride.distance + gap;
    }
    if !ride.airborne && ride.grinding.is_none() && ride.distance >= ride.next_halfpipe {
        ride.drop_in();
        sound.play(&audio::POP);
        return;
    }

    let steer = tilt.steer_deg;
    ride.board_deg = steer.clamp(-80.0, 80.0);
    if steer > -REARM_DEG {
        ride.armed = true;
    }
    if !ride.airborne && ride.armed && steer <= -POP_DEG {
        ride.airborne = true;
        ride.grinding = None;
        ride.armed = false;
        ride.climb = POP_SPEED;
        sfx = Some(&audio::POP);
    }

    let level = steer.abs() <= LAND_DEG;
    let mut bail = false;
    if ride.airborne {
        let before = ride.height;
        ride.climb -= GRAVITY * dt;
        ride.height += ride.climb * dt;
        for obstacle in ride.obstacles.clone() {
            let (_, top) = obstacle.kind.size();
            if !obstacle.under_board() || ride.height >= top {
                continue;
            }
            if obstacle.kind == Kind::Bench && before >= top && level {
                ride.height = top;
                ride.airborne = false;
                ride.grinding = Some(obstacle.id);
                ride.toast = Some(("GRIND!", TOAST_SECS));
                sfx = Some(&audio::LAND);
            } else {
                bail = true;
            }
            break;
        }
        if ride.airborne && ride.height <= 0.0 {
            ride.height = 0.0;
            ride.airborne = false;
            if level {
                ride.bonus += CLEAN_BONUS;
                ride.toast = Some(("CLEAN!", TOAST_SECS));
                sfx = Some(&audio::LAND);
            } else {
                bail = true;
            }
        }
    } else if let Some(id) = ride.grinding {
        let still_on = ride
            .obstacles
            .iter()
            .any(|obstacle| obstacle.id == id && obstacle.under_board());
        if still_on {
            ride.bonus += GRIND_PER_SEC * dt;
            ride.grind_tick -= dt;
            if ride.grind_tick <= 0.0 {
                ride.grind_tick = GRIND_TICK_SECS;
                sfx = sfx.or(Some(&audio::GRIND));
            }
        } else {
            ride.grinding = None;
            ride.airborne = true;
            ride.climb = 0.0;
        }
    } else {
        bail = ride.obstacles.iter().any(Obstacle::under_board);
    }

    ride.score = (ride.distance / PX_PER_METER) as u32 + ride.bonus as u32;
    if bail {
        ride.new_best = ride.score > ride.best;
        if ride.new_best {
            ride.best = ride.score;
            blob.save_section(save::KEY, save::encode(ride.best));
        }
        ride.enter(Phase::Bailed);
        sfx = Some(&audio::BAIL);
    }
    if let Some(clip) = sfx {
        sound.play(clip);
    }
}
