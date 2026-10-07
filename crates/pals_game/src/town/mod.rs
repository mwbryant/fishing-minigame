//! The town, where the board starts: walk around six clearings by turning
//! (left and right) and leaning (up and down) the device, or tap where to
//! go. Stand at a door and press PWR to go in; the town app ends and the
//! platform builds that place's app. The signpost by the house opens the
//! settings: sound, mock walks and resetting.

mod draw;
pub mod map;
pub mod ui;

use bevy::{app::AppExit, prelude::*};
use orbit_core::{
    Buttons, ControlsPlugin, CorePlugin, Power, SaveBlob, SoundPlugin, SoundQueue, Tilt, Touch,
    preferences,
};
use orbit_ui::{CLICK, THUD, dial_choice};
use pals::Pals;

pub use map::Screen;

use crate::{Arrival, Place};
use map::{CENTER, LAKE, Way, door, props};

pub const BACKGROUND: orbit_gfx::Rgb565 = draw::GRASS;
/// Walking pace at a full turn or lean.
pub const WALK_SPEED: f32 = 150.0;
/// Turns and leans smaller than this don't walk; at `FULL_DEG` it's full pace.
const DEAD_DEG: f32 = 4.0;
const FULL_DEG: f32 = 25.0;
/// The clearing's edge, how far down a path you go before the next one, and
/// where you come out in it.
const WALK_RADIUS: f32 = 200.0;
const EXIT_RADIUS: f32 = 206.0;
const ENTRY_RADIUS: f32 = 185.0;
/// How wide a path off the edge is, as an angle.
const PATH_HALF_DEG: f32 = 14.0;
/// Your size, for bumping into things, and how near a door counts.
const BODY: f32 = 8.0;
const DOOR_REACH: f32 = 22.0;
/// Settings' dial, clockwise from 12 o'clock.
pub const SETTINGS_CHOICES: usize = 4;
/// On the reset question, turning right past this points at ERASE.
const ERASE_TURN_DEG: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TownPhase {
    Walking,
    Settings,
    ConfirmReset,
}

#[derive(Resource, Debug)]
pub struct Town {
    pub phase: TownPhase,
    screen: Screen,
    at: Vec2,
    facing_right: bool,
    /// Where a tap said to walk.
    target: Option<Vec2>,
    /// Seconds spent walking, for the step bob.
    stride: f32,
    destination: Option<Place>,
}

impl Default for Town {
    fn default() -> Self {
        let (screen, at) = door(Place::Home);
        Self {
            phase: TownPhase::Walking,
            screen,
            at,
            facing_right: true,
            target: None,
            stride: 0.0,
            destination: None,
        }
    }
}

impl Town {
    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// Where you stand: your feet, in screen pixels.
    pub fn at(&self) -> Vec2 {
        self.at
    }

    pub fn facing_right(&self) -> bool {
        self.facing_right
    }

    pub(crate) fn stepping(&self) -> bool {
        self.stride % 0.3 > 0.15
    }

    /// The door you're standing at.
    pub fn near(&self) -> Option<Place> {
        Place::ALL.into_iter().find(|&place| {
            let (screen, spot) = door(place);
            screen == self.screen && spot.distance(self.at) <= DOOR_REACH
        })
    }

    /// Where PWR at a door sent you, once the town has ended.
    pub fn destination(&self) -> Option<Place> {
        self.destination
    }

    /// Puts you at `place`'s door.
    pub fn stand_at(&mut self, place: Place) {
        (self.screen, self.at) = door(place);
        self.target = None;
    }

    fn blocked(&self, at: Vec2) -> bool {
        let lake = self.screen == Screen::Pond && at.distance(LAKE.0) < LAKE.1 + BODY;
        lake || props(self.screen).iter().filter(|p| p.solid).any(|prop| {
            let area = prop.area();
            let low = Vec2::new(area.top_left.x as f32, area.top_left.y as f32);
            let high = low + Vec2::new(area.size.width as f32, area.size.height as f32);
            at.clamp(low, high).distance(at) < BODY
        })
    }

    /// Walks at `velocity` for `dt`, sliding along whatever's in the way and
    /// round the rim, and down a path into the next clearing.
    fn walk(&mut self, velocity: Vec2, dt: f32) {
        if velocity == Vec2::ZERO {
            return;
        }
        self.stride += dt;
        if velocity.x.abs() > 1.0 {
            self.facing_right = velocity.x > 0.0;
        }
        for step in [
            Vec2::new(velocity.x * dt, 0.0),
            Vec2::new(0.0, velocity.y * dt),
        ] {
            let next = self.at + step;
            if !self.blocked(next) {
                self.at = next;
            }
        }
        let out = self.at - CENTER;
        let path = Way::ALL.into_iter().find(|way| {
            self.screen.toward(*way).is_some()
                && out.angle_to(way.out()).abs().to_degrees() < PATH_HALF_DEG
        });
        match path {
            Some(way) if out.length() > EXIT_RADIUS => {
                self.screen = self.screen.toward(way).unwrap_or(self.screen);
                self.at = CENTER - way.out() * ENTRY_RADIUS;
                self.target = None;
            }
            Some(_) => {}
            None if out.length() > WALK_RADIUS => self.at = CENTER + out.normalize() * WALK_RADIUS,
            None => {}
        }
    }
}

pub struct TownPlugin;

impl Plugin for TownPlugin {
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
        app.init_resource::<Town>()
            .init_resource::<Pals>()
            .add_systems(Startup, arrive)
            .add_systems(Update, roam)
            .add_systems(PostUpdate, draw::draw_frame);
    }
}

/// Back at the door you came out of (or home, at boot), with the tank as
/// saved. A save from before the town keeps the gold and hats Angler's shop
/// held.
fn arrive(
    arrival: Option<Res<Arrival>>,
    mut town: ResMut<Town>,
    mut pals: ResMut<Pals>,
    mut blob: ResMut<SaveBlob>,
    mut tilt: ResMut<Tilt>,
) {
    if let Some(arrival) = arrival {
        town.stand_at(arrival.0);
    }
    *pals = pals::save::load(&blob);
    let purse = blob
        .section(&angler_game::save::KEY)
        .and_then(angler_game::save::legacy_purse);
    if let (None, Some((gold, hats))) = (blob.section(&pals::save::KEY), purse) {
        pals.restore_purse(gold, hats);
        pals::save::store(&mut blob, &pals);
    }
    tilt.request_zero();
}

/// How far a turn or lean of `deg` walks, -1 to 1.
fn pace(deg: f32) -> f32 {
    let over = ((deg.abs() - DEAD_DEG) / (FULL_DEG - DEAD_DEG)).clamp(0.0, 1.0);
    over * deg.signum()
}

#[allow(clippy::too_many_arguments)]
fn roam(
    time: Res<Time>,
    buttons: Res<Buttons>,
    mut tilt: ResMut<Tilt>,
    touch: Res<Touch>,
    power: Res<Power>,
    mut town: ResMut<Town>,
    mut pals: ResMut<Pals>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    if power.saying_good_night() || town.destination.is_some() {
        return;
    }
    let dt = time.delta_secs();
    let pressed = buttons.action.pressed();
    let tap = touch.just_pressed().then(|| touch.position()).flatten();
    let tapped = |button: &orbit_ui::Button| tap.is_some_and(|at| button.hit(at));
    match town.phase {
        TownPhase::Walking => {
            let leaning = Vec2::new(pace(tilt.steer_deg), -pace(tilt.lean_deg));
            if leaning != Vec2::ZERO {
                town.target = None;
            }
            let near = town.near();
            if let Some(place) = near.filter(|_| pressed || tap.is_some_and(|at| ui::ENTER.hit(at)))
            {
                sound.play(&CLICK);
                if place == Place::Settings {
                    town.phase = TownPhase::Settings;
                    tilt.request_zero();
                    return;
                }
                town.destination = Some(place);
                exit.write(AppExit::Success);
                return;
            }
            if let Some(at) = tap {
                town.target = Some(at);
            }
            let velocity = match town.target {
                Some(target) if leaning == Vec2::ZERO => {
                    let to = target - town.at;
                    let reach = WALK_SPEED * dt;
                    if to.length() <= reach || to.length() < 1.0 {
                        town.target = None;
                        to / dt.max(1e-3)
                    } else {
                        to.normalize() * WALK_SPEED
                    }
                }
                _ => leaning * WALK_SPEED,
            };
            let before = town.at;
            town.walk(velocity, dt);
            if town.target.is_some() && town.at == before {
                town.target = None;
            }
        }
        TownPhase::Settings => {
            let pointed = dial_choice(tilt.steer_deg, SETTINGS_CHOICES);
            let choice = ui::SETTINGS_BUTTONS
                .iter()
                .position(&tapped)
                .or(pressed.then_some(pointed));
            match choice {
                Some(0) => {
                    let muted = !sound.muted();
                    preferences::set_muted(&mut blob, &mut sound, muted);
                }
                Some(1) => {
                    pals.mock_walk = !pals.mock_walk;
                    pals::save::store(&mut blob, &pals);
                }
                Some(2) => town.phase = TownPhase::ConfirmReset,
                Some(_) => town.phase = TownPhase::Walking,
                None => {}
            }
            if choice.is_some() {
                sound.play(&CLICK);
            }
        }
        TownPhase::ConfirmReset => {
            let erase_pointed = tilt.steer_deg >= ERASE_TURN_DEG;
            let erase = tapped(&ui::ERASE) || (pressed && erase_pointed);
            let keep = tapped(&ui::KEEP) || (pressed && !erase_pointed);
            if erase {
                blob.retain(|key| *key == preferences::KEY);
                let mock_walk = pals.mock_walk;
                *pals = Pals::default();
                pals.mock_walk = mock_walk;
                pals::save::store(&mut blob, &pals);
            }
            if erase || keep {
                sound.play(if erase { &THUD } else { &CLICK });
                town.phase = TownPhase::Settings;
                tilt.request_zero();
            }
        }
    }
}
