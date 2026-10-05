//! The whole game flow as one phase machine resource (no Bevy states: every
//! state schedule costs heap the ESP32 doesn't have).

use bevy::prelude::*;
use orbit_core::{Buttons, Rng, SaveBlob, SoundQueue, Tilt, Touch};

use crate::{
    audio::GUTEN_MORGEN,
    cast::{self, Flight, scene::*},
    log::{FishLog, Milestones},
    reel::{Reel, ReelOutcome},
    save,
    settings::Settings,
    shop::{HATS, Inventory},
    species::{Catch, roll_catch},
    ui,
};

/// Results ignore presses this long, so a press meant for the fight doesn't
/// skip the celebration.
pub const RESULT_GUARD_SECS: f32 = 0.8;
pub const ESCAPED_SECS: f32 = 3.0;
pub const BITE_SHOW_SECS: f32 = 0.7;
pub const NOTHING_BITES_SECS: f32 = 9.0;
pub const NOTICE_SECS: f32 = 1.4;
pub const TOAST_SECS: f32 = 1.2;

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Title,
    Log,
    Settings,
    /// Asks before erasing the fish log, gold and hats.
    ConfirmReset,
    /// Browsing the hat shelf; `hat` is the one in the mirror.
    Shop {
        hat: usize,
    },
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
    Caught {
        catch: Catch,
        milestones: Milestones,
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
        .init_resource::<Inventory>()
        .add_systems(Startup, (load_save, first_spot))
        .add_systems(Update, play)
        .add_systems(FixedUpdate, simulate);
}

fn load_save(
    blob: Res<SaveBlob>,
    mut log: ResMut<FishLog>,
    mut settings: ResMut<Settings>,
    mut inventory: ResMut<Inventory>,
) {
    if let Some((saved_log, saved_settings, saved_inventory)) = blob.loaded().and_then(save::decode)
    {
        *log = saved_log;
        *settings = saved_settings;
        *inventory = saved_inventory;
    }
}

fn first_spot(mut session: ResMut<Session>, mut rng: ResMut<Rng>) {
    session.move_spot(&mut rng);
}

#[allow(clippy::too_many_arguments)]
fn play(
    time: Res<Time>,
    buttons: Res<Buttons>,
    tilt: Res<Tilt>,
    touch: Res<Touch>,
    mut log: ResMut<FishLog>,
    mut settings: ResMut<Settings>,
    mut inventory: ResMut<Inventory>,
    mut rng: ResMut<Rng>,
    mut session: ResMut<Session>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
) {
    let dt = time.delta_secs();
    session.clock += dt;
    session.phase_secs += dt;
    session.toast_secs = if tilt.just_zeroed {
        TOAST_SECS
    } else {
        (session.toast_secs - dt).max(0.0)
    };

    let action = buttons.action;
    let tap = touch.just_pressed();
    let tapped = |button: &ui::Button| tap && touch.position().is_some_and(|at| button.hit(at));
    let elapsed = session.phase_secs;
    let results_open = elapsed >= RESULT_GUARD_SECS;
    let tuning = settings.difficulty.tuning();
    let mut new_spot = false;
    let mut difficulty_change = None;
    let mut inventory_changed = false;
    let mut erase = false;
    let next = match &mut session.phase {
        Phase::Title if action.pressed() => Some(aim()),
        Phase::Title if tapped(&ui::LOG) => Some(Phase::Log),
        Phase::Title if tapped(&ui::SHOP) => Some(Phase::Shop {
            hat: inventory.worn(),
        }),
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
            sound.play(&GUTEN_MORGEN);
            Some(Phase::Guten)
        }
        Phase::Settings if tapped(&ui::RESET) => Some(Phase::ConfirmReset),
        Phase::Settings if tapped(&ui::BACK) || action.pressed() => Some(Phase::Title),
        Phase::ConfirmReset if tapped(&ui::ERASE) => {
            erase = true;
            Some(Phase::Settings)
        }
        Phase::ConfirmReset if tapped(&ui::KEEP) || action.pressed() => Some(Phase::Settings),
        Phase::Shop { hat } if tapped(&ui::NEXT_HAT) => {
            *hat = (*hat + 1) % HATS.len();
            None
        }
        Phase::Shop { hat } if tapped(&ui::PREVIOUS_HAT) => {
            *hat = (*hat + HATS.len() - 1) % HATS.len();
            None
        }
        // BUY and WEAR share a spot: buying puts the hat on, and an owned
        // hat is just worn.
        Phase::Shop { hat } if tapped(&ui::BUY) => {
            inventory_changed = inventory.buy(*hat) || inventory.wear(*hat);
            None
        }
        Phase::Shop { .. } if tapped(&ui::BACK) || action.pressed() => Some(Phase::Title),
        Phase::Guten if elapsed >= GUTEN_MORGEN.duration_secs() || tap || action.pressed() => {
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
                (power >= cast::MIN_CAST_POWER).then(|| Phase::Flight {
                    flight: Flight::launch(cast::elevation_deg(tilt.steer_deg), power),
                    perfect: power == 1.0,
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
            Some(at) if elapsed >= *at => Some(Phase::Bite {
                bobber_x: *bobber_x,
                catch: roll_catch(&mut rng, *accuracy, *distance),
            }),
            None if elapsed >= NOTHING_BITES_SECS => Some(Phase::Notice("NOTHING BITES")),
            _ => None,
        },
        Phase::Bite { catch, .. } if elapsed >= BITE_SHOW_SECS => Some(Phase::Reel {
            reel: Reel::with_tuning(catch.species(), &tuning, &mut rng),
            catch: *catch,
        }),
        Phase::Caught { .. } if results_open && (action.pressed() || tap) => {
            new_spot = true;
            Some(aim())
        }
        Phase::Escaped
            if (results_open && (action.pressed() || tap)) || elapsed >= ESCAPED_SECS =>
        {
            new_spot = true;
            Some(aim())
        }
        Phase::Notice(_) if elapsed >= NOTICE_SECS => Some(aim()),
        _ => None,
    };
    if new_spot {
        session.move_spot(&mut rng);
    }
    if let Some(difficulty) = difficulty_change {
        settings.difficulty = difficulty;
    }
    if erase {
        *log = FishLog::default();
        *inventory = Inventory::default();
    }
    if difficulty_change.is_some() || inventory_changed || erase {
        blob.request_save(save::encode(&log, &settings, &inventory));
    }
    if let Some(phase) = next {
        session.enter(phase);
    }
}

#[allow(clippy::too_many_arguments)]
fn simulate(
    time: Res<Time>,
    tilt: Res<Tilt>,
    settings: Res<Settings>,
    mut rng: ResMut<Rng>,
    mut session: ResMut<Session>,
    mut log: ResMut<FishLog>,
    mut inventory: ResMut<Inventory>,
    mut blob: ResMut<SaveBlob>,
) {
    let dt = time.delta_secs();
    let spot_x = session.spot_x;
    let next = match &mut session.phase {
        Phase::Flight { flight, .. } => flight.step(dt).map(|land_x| {
            if land_x < SHORE_X {
                return Phase::Notice("ON THE DOCK!");
            }
            let accuracy = cast::accuracy(land_x, spot_x);
            Phase::Waiting {
                bobber_x: land_x,
                bite_in: cast::bite_delay(&mut rng, accuracy),
                accuracy,
                distance: cast::distance(land_x),
            }
        }),
        Phase::Reel { reel, catch } => {
            reel.step(tilt.lean_deg, dt, &mut rng)
                .map(|outcome| match outcome {
                    ReelOutcome::Landed => {
                        let milestones = log.record(catch);
                        inventory.earn(catch.gold());
                        blob.request_save(save::encode(&log, &settings, &inventory));
                        Phase::Caught {
                            catch: *catch,
                            milestones,
                        }
                    }
                    ReelOutcome::Escaped => Phase::Escaped,
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
