//! The whole game flow as one phase machine resource (no Bevy states: every
//! state schedule costs heap the ESP32 doesn't have).

use core::cell::Cell;

use bevy::prelude::*;
use orbit_core::{Buttons, Clip, Power, Rng, SaveBlob, SoundQueue, Tilt, Touch};

use crate::{
    audio::{self, GUTEN_MORGEN},
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
/// Holding the zero button this long puts the board to sleep.
pub const SLEEP_HOLD_SECS: f32 = 1.5;
/// The "HOLD TO SLEEP" hint appears once the zero button is held this long.
pub const SLEEP_HINT_SECS: f32 = 0.35;
/// This long without a touch, a button or a turn of the device, and the game
/// tells the platform it is idle (dim the panel, slow the loop).
pub const IDLE_SECS: f32 = 30.0;
/// Turning or leaning the device more than this counts as input.
pub const IDLE_TILT_DEG: f32 = 3.0;

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
    /// Asked the platform to sleep; only waking it (a restart) leaves.
    Asleep,
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
    /// How long the zero button has been held; holding it long sleeps.
    pub zero_held_secs: f32,
    /// Seconds since the last input, and the pose at that input.
    pub idle_secs: f32,
    idle_pose: Vec2,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            phase: Phase::Title,
            phase_secs: 0.0,
            spot_x: (SPOT_RANGE.0 + SPOT_RANGE.1) / 2.0,
            toast_secs: 0.0,
            clock: 0.0,
            zero_held_secs: 0.0,
            idle_secs: 0.0,
            idle_pose: Vec2::ZERO,
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
    if let Some((saved_log, saved_settings, saved_inventory)) =
        blob.section(&save::KEY).and_then(save::decode)
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
    mut tilt: ResMut<Tilt>,
    touch: Res<Touch>,
    mut log: ResMut<FishLog>,
    mut settings: ResMut<Settings>,
    mut inventory: ResMut<Inventory>,
    mut rng: ResMut<Rng>,
    mut session: ResMut<Session>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
    mut power: ResMut<Power>,
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
    session.zero_held_secs = if buttons.zero.held() {
        session.zero_held_secs + dt
    } else {
        0.0
    };
    let pose = Vec2::new(tilt.steer_deg, tilt.lean_deg);
    if touch.position().is_some()
        || action.held()
        || buttons.zero.held()
        || pose.distance(session.idle_pose) > IDLE_TILT_DEG
    {
        session.idle_secs = 0.0;
        session.idle_pose = pose;
    } else {
        session.idle_secs += dt;
    }
    power.idle = session.idle_secs >= IDLE_SECS;
    let falling_asleep =
        session.zero_held_secs >= SLEEP_HOLD_SECS && session.phase != Phase::Asleep;

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
    let mut difficulty_change = None;
    let mut inventory_changed = false;
    let mut erase = false;
    let mut sfx: Option<&'static Clip> = None;
    let next = match &mut session.phase {
        _ if falling_asleep => {
            power.request_sleep();
            sound.stop();
            Some(Phase::Asleep)
        }
        Phase::Asleep => None,
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
            sfx = Some(&GUTEN_MORGEN);
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
            let bought = inventory.buy(*hat);
            inventory_changed = bought || inventory.wear(*hat);
            sfx = Some(match (bought, inventory_changed) {
                (true, _) => &audio::COIN,
                (false, true) => &audio::CLICK,
                (false, false) => &audio::THUD,
            });
            None
        }
        Phase::Shop { .. } if tapped(&ui::BACK) || action.pressed() => Some(Phase::Title),
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
        Phase::Caught { .. } if results_open && (action.pressed() || tap) => {
            next_fish = true;
            Some(aim())
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
    if next_fish {
        session.move_spot(&mut rng);
        tilt.request_zero();
    }
    if let Some(difficulty) = difficulty_change {
        settings.difficulty = difficulty;
    }
    if erase {
        *log = FishLog::default();
        *inventory = Inventory::default();
    }
    if difficulty_change.is_some() || inventory_changed || erase {
        blob.save_section(save::KEY, save::encode(&log, &settings, &inventory));
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
                    inventory.earn(catch.gold());
                    blob.save_section(save::KEY, save::encode(&log, &settings, &inventory));
                    sound.play(&audio::CATCH);
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
