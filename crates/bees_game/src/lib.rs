//! Bees???: the Moon is a hollow jar of honey orbiting the Earth. Something
//! keeps tipping it, and you turn the device to tip it back before honey pours
//! out of its mouth. The fuller the jar, the less tilt it takes to spill.
//!
//! Bees circle the jar and fly refills up from Earth, wearing whatever hat
//! the tank's lead fish wears. The game ends when the jar runs dry. The best
//! number of orbits persists in the `BEES` section.

#![no_std]

extern crate alloc;

pub mod audio;
mod draw;
mod save;
mod sprites;
pub mod ui;

use alloc::vec::Vec;

use bevy::{app::AppExit, math::ops, prelude::*};
use orbit_core::{
    Buttons, Clip, ControlsPlugin, CorePlugin, Launcher, Power, Rng, SaveBlob, SoundPlugin,
    SoundQueue, Tilt, Touch,
};
use orbit_gfx::{Rgb565, SCREEN_CENTER};
use orbit_ui::CLICK;

/// Radius of the jar's orbit around the Earth at the center of the glass, px.
pub const ORBIT_RADIUS: f32 = 150.0;
pub const EARTH_RADIUS: f32 = 50.0;
/// The jar tips back this many degrees per degree the device turns.
pub const CONTROL_GAIN: f32 = 2.0;
/// Orbit speed at the start, degrees per second.
pub const START_SPIN: f32 = 45.0;
/// Orbit speed gained per second, degrees per second.
pub const SPIN_GAIN: f32 = 0.5;
/// Top orbit speed, degrees per second.
pub const MAX_SPIN: f32 = 100.0;
/// Honey lost per second with the tilt 30 or more degrees past the spill
/// limit (1.0 is a full jar). Smaller spills lose less, down to a fifth.
pub const SPILL_PER_SEC: f32 = 0.30;
/// Seconds between refills from Earth.
pub const DELIVERY_SECS: f32 = 3.5;
/// Honey per refill (1.0 is a full jar).
pub const DELIVERY: f32 = 0.05;
/// The bee's drawn flight up from Earth fills the last this-many seconds
/// before each refill.
pub const FLIGHT_SECS: f32 = 1.2;
/// Seconds between drops while spilling.
const DRIP_SECS: f32 = 0.2;
/// How quickly the wobble eases toward its target, per second.
const DRIFT_EASE: f32 = 1.2;
/// Most drops of spilled honey on screen at once.
const MAX_DROPS: usize = 12;
/// Results ignore presses this long.
pub const RESULT_GUARD_SECS: f32 = 0.8;
pub const BACKGROUND: Rgb565 = orbit_ui::BLACK;

/// How far the jar can tip before honey reaches its mouth: 40 degrees full,
/// 100 empty.
pub fn spill_limit_deg(honey: f32) -> f32 {
    100.0 - 60.0 * honey.clamp(0.0, 1.0)
}

/// How far the unseen wobble can tip the jar, growing over the first 90 s.
pub fn wobble_reach_deg(secs: f32) -> f32 {
    35.0 + 45.0 * (secs / 90.0).min(1.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Title,
    Orbiting,
    /// The jar ran dry: the results screen.
    Empty,
}

/// A drop of spilled honey falling to Earth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drop {
    pub at: Vec2,
    velocity: Vec2,
}

/// The game in play, and the best run.
#[derive(Resource, Debug)]
pub struct Orbit {
    pub phase: Phase,
    pub phase_secs: f32,
    /// Where the jar is, degrees clockwise from 12 o'clock, ever increasing.
    pub angle_deg: f32,
    /// Orbit speed, degrees per second.
    pub spin: f32,
    /// The wobble's tilt on the jar, degrees off pointing straight out.
    pub drift_deg: f32,
    /// The jar's tilt once the device's turn counters the wobble, degrees off
    /// pointing straight out.
    pub tilt_deg: f32,
    /// 1.0 is a full jar.
    pub honey: f32,
    pub orbits: u32,
    pub best: u32,
    pub new_best: bool,
    pub clock: f32,
    pub drops: Vec<Drop>,
    /// Seconds until the next refill.
    pub delivery_in: f32,
    drift_target: f32,
    retarget_in: f32,
    drip_in: f32,
    secs: f32,
    spilling: bool,
}

impl Default for Orbit {
    fn default() -> Self {
        Self {
            phase: Phase::Title,
            phase_secs: 0.0,
            angle_deg: 30.0,
            spin: START_SPIN,
            drift_deg: 0.0,
            tilt_deg: 0.0,
            honey: 1.0,
            orbits: 0,
            best: 0,
            new_best: false,
            clock: 0.0,
            drops: Vec::new(),
            delivery_in: DELIVERY_SECS,
            drift_target: 0.0,
            retarget_in: 1.0,
            drip_in: 0.0,
            secs: 0.0,
            spilling: false,
        }
    }
}

impl Orbit {
    pub fn spilling(&self) -> bool {
        self.spilling
    }

    /// The jar's center on the glass.
    pub fn jar(&self) -> Vec2 {
        let (sin, cos) = ops::sin_cos(self.angle_deg.to_radians());
        center() + Vec2::new(sin, -cos) * ORBIT_RADIUS
    }

    /// Which way the jar's mouth points, degrees clockwise from 12 o'clock.
    pub fn mouth_deg(&self) -> f32 {
        self.angle_deg + self.tilt_deg
    }

    fn start(&mut self) {
        *self = Self {
            best: self.best,
            phase: Phase::Orbiting,
            angle_deg: 0.0,
            ..Self::default()
        };
    }

    fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_secs = 0.0;
    }
}

fn center() -> Vec2 {
    Vec2::new(SCREEN_CENTER.x as f32, SCREEN_CENTER.y as f32)
}

pub struct BeesPlugin;

impl Plugin for BeesPlugin {
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
        app.init_resource::<Orbit>()
            .init_resource::<draw::Hat>()
            .add_systems(Startup, load)
            .add_systems(Update, menus)
            .add_systems(FixedUpdate, simulate)
            .add_systems(PostUpdate, draw::draw_frame);
    }
}

/// The best orbits, and the hat the lead fish wears (for the bees).
fn load(blob: Res<SaveBlob>, mut orbit: ResMut<Orbit>, mut hat: ResMut<draw::Hat>) {
    if let Some(best) = blob.section(&save::KEY).and_then(save::decode) {
        orbit.best = best;
    }
    let tank = pals::save::load(&blob);
    hat.0 = tank.lead().and_then(|pet| pet.hat).map(usize::from);
}

#[allow(clippy::too_many_arguments)]
fn menus(
    time: Res<Time>,
    buttons: Res<Buttons>,
    touch: Res<Touch>,
    power: Res<Power>,
    launcher: Option<Res<Launcher>>,
    mut tilt: ResMut<Tilt>,
    mut orbit: ResMut<Orbit>,
    mut sound: ResMut<SoundQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    orbit.phase_secs += dt;
    orbit.clock += dt;
    if power.saying_good_night() || orbit.phase == Phase::Orbiting {
        return;
    }
    let tap = touch.just_pressed().then(|| touch.position()).flatten();
    let open = orbit.phase == Phase::Title || orbit.phase_secs >= RESULT_GUARD_SECS;
    if launcher.is_some() && tap.is_some_and(|at| ui::HOME.hit(at)) {
        sound.play(&CLICK);
        exit.write(AppExit::Success);
    } else if open && (buttons.action.pressed() || tap.is_some()) {
        tilt.request_zero();
        orbit.start();
        sound.play(&CLICK);
    }
}

#[allow(clippy::too_many_arguments)]
fn simulate(
    time: Res<Time>,
    tilt: Res<Tilt>,
    power: Res<Power>,
    mut orbit: ResMut<Orbit>,
    mut rng: ResMut<Rng>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
) {
    if orbit.phase != Phase::Orbiting || power.saying_good_night() {
        return;
    }
    let dt = time.delta_secs();
    let mut sfx: Option<&'static Clip> = None;

    orbit.secs += dt;
    orbit.spin = (START_SPIN + SPIN_GAIN * orbit.secs).min(MAX_SPIN);
    orbit.angle_deg += orbit.spin * dt;
    orbit.orbits = (orbit.angle_deg / 360.0) as u32;

    orbit.retarget_in -= dt;
    if orbit.retarget_in <= 0.0 {
        orbit.drift_target = (rng.next_f32() * 2.0 - 1.0) * wobble_reach_deg(orbit.secs);
        orbit.retarget_in = 1.0 + 1.5 * rng.next_f32();
    }
    let ease = 1.0 - ops::exp(-DRIFT_EASE * dt);
    orbit.drift_deg += (orbit.drift_target - orbit.drift_deg) * ease;
    orbit.tilt_deg = orbit.drift_deg - tilt.steer_deg * CONTROL_GAIN;

    let excess = orbit.tilt_deg.abs() - spill_limit_deg(orbit.honey);
    orbit.spilling = excess > 0.0;
    if orbit.spilling {
        orbit.honey -= SPILL_PER_SEC * (excess / 30.0).clamp(0.2, 1.0) * dt;
        orbit.drip_in -= dt;
        if orbit.drip_in <= 0.0 {
            orbit.drip_in = DRIP_SECS;
            sfx = Some(&audio::DRIP);
            let (sin, cos) = ops::sin_cos(orbit.mouth_deg().to_radians());
            let out = Vec2::new(sin, -cos);
            if orbit.drops.len() < MAX_DROPS {
                let at = orbit.jar() + out * 24.0;
                orbit.drops.push(Drop {
                    at,
                    velocity: out * 60.0,
                });
            }
        }
    }
    for drop in &mut orbit.drops {
        let pull = (center() - drop.at).normalize_or_zero() * 400.0;
        drop.velocity += pull * dt;
        drop.at += drop.velocity * dt;
    }
    orbit
        .drops
        .retain(|drop| drop.at.distance(center()) > EARTH_RADIUS);

    orbit.delivery_in -= dt;
    if orbit.delivery_in <= 0.0 {
        orbit.delivery_in = DELIVERY_SECS;
        orbit.honey = (orbit.honey + DELIVERY).min(1.0);
        sfx = sfx.or(Some(&audio::BUZZ));
    }

    if orbit.honey <= 0.0 {
        orbit.honey = 0.0;
        orbit.spilling = false;
        orbit.new_best = orbit.orbits > orbit.best;
        if orbit.new_best {
            orbit.best = orbit.orbits;
            blob.save_section(save::KEY, save::encode(orbit.best));
        }
        orbit.enter(Phase::Empty);
        sfx = Some(&audio::EMPTY);
    }
    if let Some(clip) = sfx {
        sound.play(clip);
    }
}
