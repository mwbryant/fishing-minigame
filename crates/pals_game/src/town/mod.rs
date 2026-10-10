//! The town, where the board starts: six clearings you walk around by touch.
//! Tap where to go, or hold a finger down and follow it; tap the end of a path
//! to go down it. Tap a place's landmark (its building, sign, or the pond's
//! water and dock) and you walk to its door and go in. PWR at a door goes in
//! too. Going in ends the town app, and the platform builds that
//! place's app. The signpost by the house opens settings instead:
//! sound, mock walks and reset, picked by turning like every menu.

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
use map::{CENTER, LAKE, Way, door, on_landmark, props};

pub const BACKGROUND: orbit_gfx::Rgb565 = draw::GRASS;
/// Walking speed, px/s.
pub const WALK_SPEED: f32 = 150.0;
/// A touch this far from the center, in a path's gap, means go down the path.
const PATH_TOUCH_RADIUS: f32 = 170.0;
/// Distances from the center (px): the clearing's edge, how far down a path
/// you go before the next clearing, and where you come out in it.
const WALK_RADIUS: f32 = 200.0;
const EXIT_RADIUS: f32 = 206.0;
const ENTRY_RADIUS: f32 = 185.0;
/// Half a path's width off the edge, in degrees.
const PATH_HALF_DEG: f32 = 14.0;
/// Your size, for bumping into things, and how near a door counts.
const BODY: f32 = 8.0;
const DOOR_REACH: f32 = 40.0;
/// Spots on the settings dial, clockwise from 12 o'clock: sound, mock walk,
/// reset, back.
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
    /// Where a tap (or the finger still down) says to walk.
    target: Option<Vec2>,
    /// The finger still down walked you down a path or tapped a landmark, so
    /// it won't steer until lifted.
    finger_spent: bool,
    /// A landmark was tapped: walking to its door, then going in.
    heading_in: Option<Place>,
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
            finger_spent: false,
            heading_in: None,
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

    /// The place a door sent you to, once the town has ended.
    pub fn destination(&self) -> Option<Place> {
        self.destination
    }

    /// Puts you at `place`'s door.
    pub fn stand_at(&mut self, place: Place) {
        (self.screen, self.at) = door(place);
        self.target = None;
        self.heading_in = None;
    }

    fn blocked(&self, at: Vec2) -> bool {
        let lake = self.screen == Screen::Pond && at.distance(LAKE.0) < LAKE.1 + BODY;
        lake || props(self.screen)
            .iter()
            .filter(|p| p.solid)
            .any(|prop| map::miss(prop.area(), at) < BODY)
    }

    /// The place whose landmark a tap at `at` is on, unless it's at the end of
    /// a path.
    fn landmark_at(&self, at: Vec2) -> Option<Place> {
        let out = at - CENTER;
        if self.path_toward(out).is_some() && out.length() > PATH_TOUCH_RADIUS {
            return None;
        }
        Place::ALL.into_iter().find(|&place| {
            door(place).0 == self.screen && on_landmark(place, at, orbit_ui::TAP_SLOP)
        })
    }

    /// The path a direction out from the center leads down, if there's one.
    fn path_toward(&self, out: Vec2) -> Option<Way> {
        Way::ALL.into_iter().find(|way| {
            self.screen.toward(*way).is_some()
                && out.angle_to(way.out()).abs().to_degrees() < PATH_HALF_DEG
        })
    }

    /// Where a touch at `at` sends you: there, or past the edge if it's at
    /// the end of a path.
    fn destination_of(&self, at: Vec2) -> Vec2 {
        let out = at - CENTER;
        match self.path_toward(out) {
            Some(way) if out.length() > PATH_TOUCH_RADIUS => {
                CENTER + way.out() * (EXIT_RADIUS + 20.0)
            }
            _ => at,
        }
    }

    /// Walks at `velocity` for `dt`, sliding along obstacles and the rim. True
    /// when it went down a path into the next clearing.
    fn walk(&mut self, velocity: Vec2, dt: f32) -> bool {
        if velocity == Vec2::ZERO {
            return false;
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
        match self.path_toward(out) {
            Some(way) if out.length() > EXIT_RADIUS => {
                self.screen = self.screen.toward(way).unwrap_or(self.screen);
                self.at = CENTER - way.out() * ENTRY_RADIUS;
                self.target = None;
                return true;
            }
            Some(_) => {}
            None if out.length() > WALK_RADIUS => self.at = CENTER + out.normalize() * WALK_RADIUS,
            None => {}
        }
        false
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

/// Stands you at the door you came out of (home at boot) and loads the tank.
/// A save from before the town gets the gold and hats Angler's old shop held.
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
    let on_screen: &[orbit_ui::Button] = match town.phase {
        TownPhase::Walking => &[],
        TownPhase::Settings => &ui::SETTINGS_BUTTONS,
        TownPhase::ConfirmReset => &[ui::KEEP, ui::ERASE],
    };
    let pressed_button = tap.and_then(|at| orbit_ui::tapped(at, on_screen));
    let tapped = |button: &orbit_ui::Button| pressed_button == Some(button);
    match town.phase {
        TownPhase::Walking => {
            if let Some(at) = tap {
                town.heading_in = town.landmark_at(at);
                if let Some(place) = town.heading_in {
                    town.target = Some(door(place).1);
                    town.finger_spent = true;
                }
            }
            let going_in = |place: &Place| pressed || town.heading_in == Some(*place);
            if let Some(place) = town.near().filter(going_in) {
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
            match touch.position() {
                Some(at) if !town.finger_spent => town.target = Some(town.destination_of(at)),
                Some(_) => {}
                None => town.finger_spent = false,
            }
            let velocity = match town.target {
                Some(target) => {
                    let to = target - town.at;
                    if to.length() <= WALK_SPEED * dt {
                        town.target = None;
                        to / dt.max(1e-3)
                    } else {
                        to.normalize() * WALK_SPEED
                    }
                }
                None => Vec2::ZERO,
            };
            let before = town.at;
            if town.walk(velocity, dt) {
                town.finger_spent = touch.position().is_some();
            } else if town.at == before && touch.position().is_none() {
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
                    // Off, then each pace in turn, then off again.
                    match (pals.mock_walk, pals.mock_pace + 1 < pals::MOCK_PACES.len()) {
                        (false, _) => (pals.mock_walk, pals.mock_pace) = (true, 0),
                        (true, true) => pals.mock_pace += 1,
                        (true, false) => (pals.mock_walk, pals.mock_pace) = (false, 0),
                    }
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
