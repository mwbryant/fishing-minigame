//! The whole game flow as one phase machine resource (no Bevy states: every
//! state schedule costs heap the ESP32 doesn't have).

use core::cell::Cell;

use bevy::app::AppExit;
use bevy::prelude::*;
use orbit_core::{Buttons, Clip, Launcher, Power, Rng, SaveBlob, SoundQueue, Tilt, Touch};
use orbit_ui::{Key, KeyRing, dial_choice};
use pals::{Name, Pals, Pet};

use crate::{
    audio::{self, GUTEN_MORGEN},
    cast::{self, Flight, scene::*},
    log::{FishLog, Milestones},
    reel::{Reel, ReelOutcome},
    save,
    settings::Settings,
    species::{Catch, roll_catch},
    ui,
};

/// Results ignore presses this long, so a press meant for the fight doesn't
/// skip the celebration.
pub const RESULT_GUARD_SECS: f32 = 0.8;
/// GUTEN ignores taps and presses this long, so a touch that flickers as the
/// finger lifts off the GUTEN button can't stop the song it just started.
pub const GUTEN_GUARD_SECS: f32 = 0.6;
pub const ESCAPED_SECS: f32 = 3.0;
pub const BITE_SHOW_SECS: f32 = 0.7;
pub const NOTHING_BITES_SECS: f32 = 9.0;
pub const NOTICE_SECS: f32 = 1.4;
pub const TOAST_SECS: f32 = 1.2;
/// The reel clicks each time the catch meter rises by this much.
pub const REEL_CLICK_STEP: f32 = 0.03;
/// On the catch screen, turning right past this points at RELEASE.
pub const RELEASE_TURN_DEG: f32 = 10.0;
/// The swap screen's options, clockwise from 12 o'clock: the three kept fish,
/// then letting the new catch go.
pub const SWAP_CHOICES: usize = 4;
pub const LET_IT_GO: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Title,
    Log,
    Settings,
    /// The easter egg: the spinning fish and its song.
    Guten,
    /// Aiming by rotation; `charging_secs` counts while the action button is held.
    Aim {
        charging_secs: Option<f32>,
    },
    /// `perfect` when released inside the window at the top of the meter.
    Flight {
        flight: Flight,
        perfect: bool,
    },
    Waiting {
        bobber_x: f32,
        bite_in: Option<f32>,
        accuracy: f32,
        distance: f32,
    },
    Bite {
        bobber_x: f32,
        catch: Catch,
    },
    Reel {
        reel: Reel,
        catch: Catch,
    },
    /// Landed: KEEP it (held level) or RELEASE it (turned right).
    Caught {
        catch: Catch,
        milestones: Milestones,
    },
    /// The tank is full: which kept fish to let go, or the new catch.
    Swap {
        catch: Catch,
    },
    /// Typing the new fish's name; it takes tank slot `replacing`, or a new one.
    Naming {
        catch: Catch,
        ring: KeyRing,
        name: Name,
        replacing: Option<usize>,
    },
    Escaped,
    /// A short message, then back to aiming.
    Notice(&'static str),
}

#[derive(Resource, Debug)]
pub struct Session {
    pub phase: Phase,
    /// Seconds since `phase` was entered.
    pub phase_secs: f32,
    /// Where the fish are; casts near it get quicker, better bites.
    pub spot_x: f32,
    /// Seconds left showing "LEVEL SET" after the zero button.
    pub toast_secs: f32,
    /// Seconds since boot, for idle animations.
    pub clock: f32,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            phase: Phase::Title,
            phase_secs: 0.0,
            spot_x: (SPOT_RANGE.0 + SPOT_RANGE.1) / 2.0,
            toast_secs: 0.0,
            clock: 0.0,
        }
    }
}

impl Session {
    pub fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_secs = 0.0;
    }

    fn move_spot(&mut self, rng: &mut Rng) {
        self.spot_x = SPOT_RANGE.0 + (SPOT_RANGE.1 - SPOT_RANGE.0) * rng.next_f32();
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<Session>()
        .init_resource::<FishLog>()
        .init_resource::<Settings>()
        .init_resource::<Pals>()
        .add_systems(Startup, (load_save, first_spot))
        .add_systems(Update, play)
        .add_systems(FixedUpdate, simulate);
}

fn load_save(
    blob: Res<SaveBlob>,
    mut log: ResMut<FishLog>,
    mut settings: ResMut<Settings>,
    mut pals: ResMut<Pals>,
) {
    if let Some((saved_log, saved_settings)) = blob.section(&save::KEY).and_then(save::decode) {
        *log = saved_log;
        *settings = saved_settings;
    }
    *pals = pals::save::load(&blob);
}

fn first_spot(mut session: ResMut<Session>, mut rng: ResMut<Rng>) {
    session.move_spot(&mut rng);
}

#[allow(clippy::too_many_arguments)]
fn play(
    time: Res<Time>,
    buttons: Res<Buttons>,
    mut tilt: ResMut<Tilt>,
    touch: Res<Touch>,
    log: Res<FishLog>,
    mut settings: ResMut<Settings>,
    mut pals: ResMut<Pals>,
    mut rng: ResMut<Rng>,
    mut session: ResMut<Session>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
    power: Res<Power>,
    launcher: Option<Res<Launcher>>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    session.clock += dt;
    session.phase_secs += dt;
    session.toast_secs = if tilt.just_zeroed {
        TOAST_SECS
    } else {
        (session.toast_secs - dt).max(0.0)
    };

    if power.saying_good_night() {
        return;
    }
    let action = buttons.action;

    let tap = touch.just_pressed();
    let hit_button = Cell::new(false);
    let tapped = |button: &ui::Button| {
        let hit = tap && touch.position().is_some_and(|at| button.hit(at));
        hit_button.set(hit_button.get() || hit);
        hit
    };
    let elapsed = session.phase_secs;
    let results_open = elapsed >= RESULT_GUARD_SECS;
    let tuning = settings.difficulty.tuning();
    let mut next_fish = false;
    let mut level_now = false;
    let mut difficulty_change = None;
    let mut kept = None;
    let mut sfx: Option<&'static Clip> = None;
    let release_pointed = tilt.steer_deg >= RELEASE_TURN_DEG;
    let full = pals.is_full();
    let next = match &mut session.phase {
        Phase::Title if launcher.is_some() && tapped(&ui::HOME) => {
            exit.write(AppExit::Success);
            None
        }
        Phase::Title if action.pressed() => Some(aim()),
        Phase::Title if tapped(&ui::LOG) => Some(Phase::Log),
        Phase::Title if tapped(&ui::SETTINGS) => Some(Phase::Settings),
        Phase::Log if action.pressed() || tap => Some(Phase::Title),
        Phase::Settings if tapped(&ui::HARDER) => {
            difficulty_change = Some(settings.difficulty.next());
            None
        }
        Phase::Settings if tapped(&ui::EASIER) => {
            difficulty_change = Some(settings.difficulty.previous());
            None
        }
        Phase::Settings if tapped(&ui::GUTEN) => {
            sfx = Some(&GUTEN_MORGEN);
            Some(Phase::Guten)
        }
        Phase::Settings if tapped(&ui::BACK) || action.pressed() => Some(Phase::Title),
        Phase::Guten
            if elapsed >= GUTEN_MORGEN.duration_secs()
                || (elapsed >= GUTEN_GUARD_SECS && (tap || action.pressed())) =>
        {
            sound.stop();
            Some(Phase::Settings)
        }
        Phase::Aim {
            charging_secs: None,
        } if tapped(&ui::MENU) => Some(Phase::Title),
        Phase::Aim { charging_secs } => match *charging_secs {
            None if action.pressed() => {
                *charging_secs = Some(0.0);
                None
            }
            Some(secs) if action.held() && secs + dt > cast::MAX_CHARGE_SECS => {
                *charging_secs = None;
                Some(Phase::Notice("LET GO!"))
            }
            Some(secs) if action.held() => {
                *charging_secs = Some(secs + dt);
                None
            }
            Some(secs) => {
                *charging_secs = None;
                let power = cast::release_power(secs, tuning.perfect_power);
                let perfect = power == 1.0;
                (power >= cast::MIN_CAST_POWER).then(|| {
                    sfx = Some(if perfect {
                        &audio::PERFECT_CAST
                    } else {
                        &audio::CAST
                    });
                    Phase::Flight {
                        flight: Flight::launch(cast::elevation_deg(tilt.steer_deg), power),
                        perfect,
                    }
                })
            }
            None => None,
        },
        Phase::Waiting { .. } if action.pressed() => Some(Phase::Notice("REELED IN")),
        Phase::Waiting {
            bobber_x,
            bite_in,
            accuracy,
            distance,
            ..
        } => match bite_in {
            Some(at) if elapsed >= *at => {
                sfx = Some(&audio::BITE);
                Some(Phase::Bite {
                    bobber_x: *bobber_x,
                    catch: roll_catch(&mut rng, *accuracy, *distance),
                })
            }
            None if elapsed >= NOTHING_BITES_SECS => Some(Phase::Notice("NOTHING BITES")),
            _ => None,
        },
        Phase::Bite { catch, .. } if elapsed >= BITE_SHOW_SECS => Some(Phase::Reel {
            reel: Reel::with_tuning(catch.species(), &tuning, &mut rng),
            catch: *catch,
        }),
        Phase::Caught { catch, .. }
            if results_open
                && (tapped(&ui::KEEP_FISH) || (action.pressed() && !release_pointed)) =>
        {
            level_now = true;
            Some(if full {
                Phase::Swap { catch: *catch }
            } else {
                naming(*catch, None)
            })
        }
        Phase::Caught { .. } if results_open && (tapped(&ui::RELEASE_FISH) || action.pressed()) => {
            next_fish = true;
            Some(aim())
        }
        Phase::Swap { catch } => {
            let picked = if action.pressed() {
                Some(dial_choice(tilt.steer_deg, SWAP_CHOICES))
            } else {
                ui::SWAP_SLOTS.iter().position(&tapped)
            };
            match picked {
                Some(LET_IT_GO) => {
                    next_fish = true;
                    Some(aim())
                }
                Some(slot) => {
                    level_now = true;
                    Some(naming(*catch, Some(slot)))
                }
                None => None,
            }
        }
        Phase::Naming {
            catch,
            ring,
            name,
            replacing,
        } => {
            let pointed = ring.point(tilt.steer_deg);
            let key = match (action.pressed(), tap) {
                (true, _) => Some(pointed),
                (false, true) => touch.position().and_then(KeyRing::key_at),
                (false, false) => None,
            };
            let typed = match key {
                Some(Key::Letter(letter)) => name.push(letter),
                Some(Key::Space) => !name.is_empty() && name.push(' '),
                Some(Key::Delete) => {
                    name.pop();
                    true
                }
                Some(Key::Done) if !name.is_empty() => {
                    kept = Some((*catch, *name, *replacing));
                    true
                }
                Some(Key::Done) | None => false,
            };
            if key.is_some() {
                sfx = Some(if typed { &audio::CLICK } else { &audio::THUD });
            }
            kept.is_some().then_some(Phase::Notice("IN THE TANK!"))
        }
        Phase::Escaped
            if (results_open && (action.pressed() || tap)) || elapsed >= ESCAPED_SECS =>
        {
            next_fish = true;
            Some(aim())
        }
        Phase::Notice(_) if elapsed >= NOTICE_SECS => Some(aim()),
        _ => None,
    };
    if let Some(clip) = sfx.or(hit_button.get().then_some(&audio::CLICK)) {
        sound.play(clip);
    }
    if let Some((catch, name, replacing)) = kept {
        let pet = pet_from(&catch, name, &mut rng);
        match replacing {
            Some(slot) => {
                pals.swap(slot, pet);
            }
            None => {
                pals.keep(pet).ok();
            }
        }
        pals::save::store(&mut blob, &pals);
        next_fish = true;
    }
    if next_fish {
        session.move_spot(&mut rng);
    }
    if next_fish || level_now {
        tilt.request_zero();
    }
    if let Some(difficulty) = difficulty_change {
        settings.difficulty = difficulty;
        blob.save_section(save::KEY, save::encode(&log, &settings));
    }
    if let Some(phase) = next {
        session.enter(phase);
    }
}

#[allow(clippy::too_many_arguments)]
fn simulate(
    time: Res<Time>,
    mut tilt: ResMut<Tilt>,
    settings: Res<Settings>,
    mut rng: ResMut<Rng>,
    mut session: ResMut<Session>,
    mut log: ResMut<FishLog>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
) {
    let dt = time.delta_secs();
    let spot_x = session.spot_x;
    let next = match &mut session.phase {
        Phase::Flight { flight, .. } => flight.step(dt).map(|land_x| {
            if land_x < SHORE_X {
                sound.play(&audio::THUD);
                return Phase::Notice("ON THE DOCK!");
            }
            sound.play(&audio::PLOP);
            let accuracy = cast::accuracy(land_x, spot_x);
            Phase::Waiting {
                bobber_x: land_x,
                bite_in: cast::bite_delay(&mut rng, accuracy),
                accuracy,
                distance: cast::distance(land_x),
            }
        }),
        Phase::Reel { reel, catch } => {
            let before = reel.progress;
            let outcome = reel.step(tilt.lean_deg, dt, &mut rng);
            if (reel.progress / REEL_CLICK_STEP) as u32 > (before / REEL_CLICK_STEP) as u32 {
                sound.play(&audio::REEL);
            }
            outcome.map(|outcome| match outcome {
                ReelOutcome::Landed => {
                    let milestones = log.record(catch);
                    blob.save_section(save::KEY, save::encode(&log, &settings));
                    sound.play(&audio::CATCH);
                    tilt.request_zero();
                    Phase::Caught {
                        catch: *catch,
                        milestones,
                    }
                }
                ReelOutcome::Escaped => {
                    sound.play(&audio::ESCAPE);
                    Phase::Escaped
                }
            })
        }
        _ => None,
    };
    if let Some(phase) = next {
        session.enter(phase);
    }
}

fn aim() -> Phase {
    Phase::Aim {
        charging_secs: None,
    }
}

fn naming(catch: Catch, replacing: Option<usize>) -> Phase {
    Phase::Naming {
        catch,
        ring: KeyRing::default(),
        name: Name::default(),
        replacing,
    }
}

/// A landed fish as a pet: its species, tier and size.
fn pet_from(catch: &Catch, name: Name, rng: &mut Rng) -> Pet {
    Pet::new(
        catch.species as u8,
        catch.species().tier,
        catch.length_mm,
        catch.size_pct(),
        name,
        rng,
    )
}
