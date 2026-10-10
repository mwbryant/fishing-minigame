//! Your house: the tank your fish swim in, each one's card, renaming, and the
//! trick school. Teaching shows the trick, then takes a word (said twice into
//! the mics) and/or a stroke on the glass (drawn twice). In practice you give
//! the command and the fish tries; petting it right after a success doubles
//! what it learns, as in Nintendogs.

mod draw;
pub mod ui;

use alloc::{boxed::Box, format, string::String, vec::Vec};

use bevy::{app::AppExit, prelude::*};
use orbit_core::{
    Buttons, Clip, ControlsPlugin, CorePlugin, Mic, MicPlugin, Power, Rng, SaveBlob, SoundPlugin,
    SoundQueue, Tilt, Touch, VoicePrint,
};
use orbit_ui::{CLICK, Key, KeyRing, THUD, dial_choice, list_choice};
use pals::{Gesture, Name, Pals, Strokes, TRICKS_PER_PET, Trigger, tricks::Motion};

use crate::fish::{TRICK_SECS, pose};

pub use draw::BACKGROUND;

/// Teaching shows the trick twice before listening.
pub const WATCH_SECS: f32 = 2.0 * TRICK_SECS;
/// How long after a success petting still counts as praise.
pub const PRAISE_SECS: f32 = 2.5;
/// The mic hears the speaker: words this soon after a sound are ignored.
const ECHO_SECS: f32 = 1.0;
/// Where fish learn and perform tricks.
pub const STAGE: Vec2 = Vec2::new(233.0, 230.0);
/// A tap this close to a swimming fish picks it.
const FISH_REACH: f32 = 48.0;
/// Tricks move this much of their full size here, so the highest leap stays
/// under the lesson's text and the deepest dive over the mic's.
pub(crate) const STAGE_MOTION: f32 = 0.65;
/// A tap this close to the stage pets the fish.
const PET_REACH: f32 = 60.0;
/// Where tank fish swim: clear of the names round the rim, however long.
const SWIM_LOW: Vec2 = Vec2::new(150.0, 170.0);
const SWIM_HIGH: Vec2 = Vec2::new(316.0, 290.0);
const SWIM_SPEED: f32 = 40.0;
/// The card's buttons, left to right: rename, the tricks, back.
pub const CARD_CHOICES: usize = 3;
/// The tricks list: the five, then BACK.
pub const TRICK_CHOICES: usize = TRICKS_PER_PET + 1;

/// How far a word or stroke being taught has got: heard once, or twice alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heard<T> {
    Nothing,
    Once(T),
    Sure(T),
}

impl<T: Copy> Heard<T> {
    fn sure(&self) -> Option<T> {
        match self {
            Heard::Sure(it) => Some(*it),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeachStep {
    /// The fish shows the trick.
    Watch,
    /// A word and/or a stroke, each given twice to be sure.
    Listen {
        word: Heard<VoicePrint>,
        stroke: Heard<Gesture>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PracticeStep {
    /// Waiting for the command.
    Waiting,
    /// Doing the trick in `slot` (which the command meant), well or not.
    Trying { slot: usize, success: bool },
    /// It did it: pet it now.
    Praise,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HomePhase {
    /// The tank; turning points at a fish or OUT.
    Tank,
    Card {
        pet: usize,
    },
    Rename {
        pet: usize,
        ring: KeyRing,
        name: Name,
    },
    Tricks {
        pet: usize,
    },
    Teach {
        pet: usize,
        slot: usize,
        step: TeachStep,
    },
    Practice {
        pet: usize,
        slot: usize,
        step: PracticeStep,
    },
}

/// A fish swimming about the tank.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Swimmer {
    at: Vec2,
    to: Vec2,
    facing_right: bool,
}

/// The phase is boxed: it can hold two voice prints, and Bevy's resource
/// storage costs many times a resource's size in heap.
#[derive(Resource, Debug)]
pub struct Home {
    pub phase: Box<HomePhase>,
    /// Seconds since the phase (or its step) changed.
    secs: f32,
    /// Seconds since the house opened.
    clock: f32,
    swimmers: Vec<Swimmer>,
    strokes: Strokes,
    /// A press that began on a button, or where commands aren't read,
    /// isn't a stroke.
    pressing_button: bool,
    /// What just happened, shown until the next thing does.
    toast: Option<String>,
    /// The mic is ignored until this `clock` time (the speaker just played).
    deaf_until: f32,
    rigged: Option<bool>,
}

impl Default for Home {
    fn default() -> Self {
        Self {
            phase: Box::new(HomePhase::Tank),
            secs: 0.0,
            clock: 0.0,
            swimmers: Vec::new(),
            strokes: Strokes::default(),
            pressing_button: false,
            toast: None,
            deaf_until: 0.0,
            rigged: None,
        }
    }
}

impl Home {
    /// Where fish `pet` is drawn right now.
    pub fn swimmer(&self, pet: usize) -> Vec2 {
        match self.phase.as_ref() {
            HomePhase::Tank => self.swimmers.get(pet).map_or(STAGE, |s| s.at),
            _ => STAGE,
        }
    }

    pub(crate) fn swimmers(&self) -> impl Iterator<Item = (Vec2, bool)> + '_ {
        self.swimmers.iter().map(|s| (s.at, s.facing_right))
    }

    pub(crate) fn toast(&self) -> Option<&str> {
        self.toast.as_deref()
    }

    /// Makes the next try succeed or flub, for tests.
    pub fn rig_next_try(&mut self, success: bool) {
        self.rigged = Some(success);
    }

    fn enter(&mut self, phase: HomePhase) {
        *self.phase = phase;
        self.secs = 0.0;
        self.toast = None;
    }

    fn step(&mut self, phase: HomePhase) {
        *self.phase = phase;
        self.secs = 0.0;
    }
}

pub struct HomePlugin;

impl Plugin for HomePlugin {
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
        app.add_plugins(MicPlugin)
            .init_resource::<Home>()
            .init_resource::<Pals>()
            .add_systems(Startup, move_in)
            .add_systems(Update, (swim, play).chain())
            .add_systems(PostUpdate, draw::draw_frame);
    }
}

fn move_in(
    blob: Res<SaveBlob>,
    mut pals: ResMut<Pals>,
    mut home: ResMut<Home>,
    mut rng: ResMut<Rng>,
    mut tilt: ResMut<Tilt>,
) {
    *pals = pals::save::load(&blob);
    home.swimmers = (0..pals.pets.len())
        .map(|i| {
            let at = spot(&mut rng);
            Swimmer {
                at,
                to: spot(&mut rng),
                facing_right: i % 2 == 0,
            }
        })
        .collect();
    tilt.request_zero();
}

fn spot(rng: &mut Rng) -> Vec2 {
    SWIM_LOW + (SWIM_HIGH - SWIM_LOW) * Vec2::new(rng.next_f32(), rng.next_f32())
}

fn swim(time: Res<Time>, mut home: ResMut<Home>, mut rng: ResMut<Rng>) {
    let dt = time.delta_secs();
    for swimmer in &mut home.swimmers {
        let to = swimmer.to - swimmer.at;
        if to.length() < 2.0 {
            swimmer.to = spot(&mut rng);
            continue;
        }
        swimmer.at += to.normalize() * (SWIM_SPEED * dt).min(to.length());
        if to.x.abs() > 1.0 {
            swimmer.facing_right = to.x > 0.0;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn play(
    time: Res<Time>,
    buttons: Res<Buttons>,
    mut tilt: ResMut<Tilt>,
    touch: Res<Touch>,
    power: Res<Power>,
    mut mic: ResMut<Mic>,
    mut home: ResMut<Home>,
    mut pals: ResMut<Pals>,
    mut rng: ResMut<Rng>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    home.secs += dt;
    home.clock += dt;
    let heard = mic.take().filter(|_| home.clock >= home.deaf_until);
    if power.saying_good_night() {
        return;
    }
    let pressed = buttons.action.pressed();
    let tap = touch.just_pressed().then(|| touch.position()).flatten();
    let on_screen: &[ui::Button] = match home.phase.as_ref() {
        HomePhase::Tank => &[ui::OUT],
        HomePhase::Card { .. } => &ui::CARD_BUTTONS,
        HomePhase::Tricks { .. } => &ui::TRICK_ROWS,
        HomePhase::Practice { .. } => &[ui::NEW_WORD],
        _ => &[],
    };
    let pressed_button = tap.and_then(|at| orbit_ui::tapped(at, on_screen));
    let tapped = |button: &ui::Button| pressed_button == Some(button);
    let mut sfx: Option<&'static Clip> = None;
    let mut save = false;
    let mut next: Option<HomePhase> = None;
    let mut stroke = None;
    let wants_strokes = matches!(
        *home.phase,
        HomePhase::Teach {
            step: TeachStep::Listen { .. },
            ..
        } | HomePhase::Practice {
            step: PracticeStep::Waiting,
            ..
        }
    );
    let on_button = tap.is_some_and(|at| ui::NEW_WORD.hit(at));
    if touch.just_pressed() {
        home.pressing_button = on_button || !wants_strokes;
    }
    if !home.pressing_button {
        stroke = home.strokes.update(touch.position(), dt);
    }
    if touch.position().is_none() {
        home.pressing_button = false;
    }

    match (*home.phase).clone() {
        HomePhase::Tank => {
            let count = pals.pets.len();
            let pointed = dial_choice(tilt.steer_deg, count + 1);
            let touched = tap.and_then(|at| {
                orbit_ui::nearest(at, (0..count).map(|pet| home.swimmer(pet)), FISH_REACH)
            });
            let picked = touched
                .or(tapped(&ui::OUT).then_some(count))
                .or(pressed.then_some(pointed));
            match picked {
                Some(pet) if pet < count => next = Some(HomePhase::Card { pet }),
                Some(_) => {
                    sound.play(&CLICK);
                    exit.write(AppExit::Success);
                    return;
                }
                None => {}
            }
        }
        HomePhase::Card { pet } => {
            let pointed = list_choice(tilt.steer_deg, CARD_CHOICES, 1);
            let picked = ui::CARD_BUTTONS
                .iter()
                .position(&tapped)
                .or(pressed.then_some(pointed));
            next = match picked {
                Some(0) => Some(HomePhase::Rename {
                    pet,
                    ring: KeyRing::default(),
                    name: pals.pets[pet].name,
                }),
                Some(1) => Some(HomePhase::Tricks { pet }),
                Some(_) => Some(HomePhase::Tank),
                None => None,
            };
        }
        HomePhase::Rename {
            pet,
            mut ring,
            mut name,
        } => {
            let pointed = ring.point(tilt.steer_deg);
            let key = match (pressed, tap) {
                (true, _) => Some(pointed),
                (false, Some(at)) => KeyRing::key_at(at),
                (false, None) => None,
            };
            let typed = match key {
                Some(Key::Letter(letter)) => name.push(letter),
                Some(Key::Space) => !name.is_empty() && name.push(' '),
                Some(Key::Delete) => {
                    name.pop();
                    true
                }
                Some(Key::Done) if !name.is_empty() => {
                    pals.pets[pet].name = name;
                    save = true;
                    next = Some(HomePhase::Card { pet });
                    true
                }
                Some(Key::Done) | None => false,
            };
            if key.is_some() {
                sfx = Some(if typed { &CLICK } else { &THUD });
            }
            if next.is_none() {
                *home.phase = HomePhase::Rename { pet, ring, name };
            }
        }
        HomePhase::Tricks { pet } => {
            let pointed = list_choice(tilt.steer_deg, TRICK_CHOICES, 0);
            let picked = ui::TRICK_ROWS
                .iter()
                .position(&tapped)
                .or(pressed.then_some(pointed));
            next = match picked {
                Some(slot) if slot < TRICKS_PER_PET && pals.pets[pet].tricks[slot].taught() => {
                    Some(HomePhase::Practice {
                        pet,
                        slot,
                        step: PracticeStep::Waiting,
                    })
                }
                Some(slot) if slot < TRICKS_PER_PET => Some(HomePhase::Teach {
                    pet,
                    slot,
                    step: TeachStep::Watch,
                }),
                Some(_) => Some(HomePhase::Card { pet }),
                None => None,
            };
        }
        HomePhase::Teach {
            pet,
            slot,
            step: TeachStep::Watch,
        } => {
            if home.secs >= WATCH_SECS || pressed {
                home.step(HomePhase::Teach {
                    pet,
                    slot,
                    step: listening(),
                });
            }
        }
        HomePhase::Teach {
            pet,
            slot,
            step:
                TeachStep::Listen {
                    mut word,
                    stroke: mut drawn,
                },
        } => {
            let fish = &pals.pets[pet];
            let name_of = |other: usize| fish.tricks[other].info().name;
            if let Some(print) = heard {
                let clash = fish.answer_voice(&print).filter(|&other| other != slot);
                (word, home.toast) = match (clash, word) {
                    (Some(other), _) => (word, Some(format!("SOUNDS LIKE {}", name_of(other)))),
                    (None, Heard::Nothing) => (Heard::Once(print), None),
                    (None, Heard::Once(first)) if first.alike(&print) => {
                        (Heard::Sure(first.blend(&print, 0.5)), None)
                    }
                    (None, Heard::Once(_)) => (
                        Heard::Once(print),
                        Some(String::from("THAT SOUNDED DIFFERENT")),
                    ),
                    (None, Heard::Sure(taught)) if taught.alike(&print) => (
                        Heard::Sure(taught.blend(&print, 1.0 / 3.0)),
                        Some(String::from("EVEN BETTER")),
                    ),
                    (None, Heard::Sure(_)) => {
                        (word, Some(String::from("THAT ONE SOUNDED DIFFERENT")))
                    }
                };
            }
            if let Some(gesture) = stroke {
                let clash = fish.answer_gesture(gesture).filter(|&other| other != slot);
                (drawn, home.toast) = match (clash, drawn) {
                    (Some(other), _) => (
                        drawn,
                        Some(format!("{} IS FOR {}", gesture.name(), name_of(other))),
                    ),
                    (None, Heard::Once(first)) if first == gesture => (Heard::Sure(first), None),
                    (None, _) => (Heard::Once(gesture), None),
                };
            }
            if pressed {
                let (voice, gesture) = (word.sure(), drawn.sure());
                if voice.is_none() && gesture.is_none() {
                    next = Some(HomePhase::Tricks { pet });
                } else {
                    let old = pals.pets[pet].tricks[slot].trigger;
                    pals.pets[pet].teach(
                        slot,
                        Trigger {
                            voice: voice.or(old.voice),
                            gesture: gesture.or(old.gesture),
                        },
                    );
                    save = true;
                    next = Some(HomePhase::Practice {
                        pet,
                        slot,
                        step: PracticeStep::Waiting,
                    });
                }
            } else {
                *home.phase = HomePhase::Teach {
                    pet,
                    slot,
                    step: TeachStep::Listen {
                        word,
                        stroke: drawn,
                    },
                };
            }
        }
        HomePhase::Practice {
            pet,
            slot,
            step: PracticeStep::Waiting,
        } => {
            let command = heard
                .map(|print| pals.pets[pet].hear(&print))
                .or(stroke.map(|gesture| pals.pets[pet].answer_gesture(gesture)));
            save |= heard.is_some() && command.flatten().is_some();
            let fish = &pals.pets[pet];
            if tapped(&ui::NEW_WORD) {
                next = Some(HomePhase::Teach {
                    pet,
                    slot,
                    step: listening(),
                });
            } else if pressed {
                next = Some(HomePhase::Tricks { pet });
            } else if let Some(meant) = command {
                match meant {
                    Some(trick) => {
                        let chance = fish.tricks[trick].success_chance();
                        let success = home.rigged.take().unwrap_or(rng.next_f32() < chance);
                        sfx = Some(if success {
                            &angler_game::audio::PERFECT_CAST
                        } else {
                            &THUD
                        });
                        home.step(HomePhase::Practice {
                            pet,
                            slot,
                            step: PracticeStep::Trying {
                                slot: trick,
                                success,
                            },
                        });
                    }
                    None => {
                        home.toast = Some(format!("{} LOOKS PUZZLED", fish.name.as_str()));
                        sfx = Some(&THUD);
                    }
                }
            }
        }
        HomePhase::Practice {
            pet,
            slot,
            step: PracticeStep::Trying { slot: did, success },
        } => {
            if home.secs >= TRICK_SECS {
                let step = if success && did == slot {
                    PracticeStep::Praise
                } else {
                    home.toast = Some(if success {
                        format!("THAT WAS {}", pals.pets[pet].tricks[did].info().name)
                    } else {
                        String::from("NOT QUITE")
                    });
                    PracticeStep::Waiting
                };
                home.step(HomePhase::Practice { pet, slot, step });
            }
        }
        HomePhase::Practice {
            pet,
            slot,
            step: PracticeStep::Praise,
        } => {
            let petted = tap.is_some_and(|at| at.distance(STAGE) < PET_REACH) || pressed;
            if petted || home.secs >= PRAISE_SECS {
                let trick = &mut pals.pets[pet].tricks[slot];
                let before = trick.learned;
                trick.practice(petted);
                let gain = trick.learned - before;
                home.toast = Some(if petted {
                    sfx = Some(&angler_game::audio::CATCH);
                    format!("GOOD FISH! +{gain}%")
                } else {
                    format!("+{gain}%. PET IT NEXT TIME")
                });
                save = true;
                home.step(HomePhase::Practice {
                    pet,
                    slot,
                    step: PracticeStep::Waiting,
                });
            }
        }
    }

    let pressed_something = next.is_some() && (pressed || tap.is_some());
    if let Some(clip) = sfx.or(pressed_something.then_some(&CLICK)) {
        sound.play(clip);
        home.deaf_until = home.clock + ECHO_SECS;
    }
    if save {
        pals::save::store(&mut blob, &pals);
    }
    if let Some(phase) = next {
        home.enter(phase);
        tilt.request_zero();
    }
}

fn listening() -> TeachStep {
    TeachStep::Listen {
        word: Heard::Nothing,
        stroke: Heard::Nothing,
    }
}

/// The trick the fish is showing or trying, and how far into it.
pub(crate) fn performance(home: &Home, pals: &Pals) -> Option<(Motion, f32)> {
    let (pet, slot) = match *home.phase {
        HomePhase::Teach {
            pet,
            slot,
            step: TeachStep::Watch,
        } => (pet, slot),
        HomePhase::Practice {
            pet,
            step: PracticeStep::Trying { slot, .. },
            ..
        } => (pet, slot),
        _ => return None,
    };
    let motion = pals.pets.get(pet)?.tricks[slot].info().motion;
    Some((motion, home.secs % TRICK_SECS))
}

/// The fish's offset and turn for the trick it's showing or trying.
pub(crate) fn performing(home: &Home, pals: &Pals) -> Option<(Vec2, bool)> {
    let (motion, t) = performance(home, pals)?;
    let (offset, flipped) = pose(motion, t);
    Some((offset * STAGE_MOTION, flipped))
}
