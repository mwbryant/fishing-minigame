//! Spider Roulette: twelve eggs ring a web and one hides a spider. Press PWR
//! (or tap) to spin: a pointer whirls round the ring and slows to a stop on
//! a whole egg, which cracks. Every safe egg shortens the odds and quickens
//! the heartbeat; the spider's egg makes the spider dancing in the middle
//! pounce. Crack all eleven safe ones and the spider is trapped. The best run
//! of safe eggs persists in the board's save record.
//!
//! In Fishing Minigame it's the den's gamble: with gold in the purse (`pals`), a
//! round starts with a bet. Each safe egg raises what the bet is worth (fair
//! odds, less the house's tenth); CASH OUT banks it, the spider takes it,
//! and trapping the spider pays the whole run.

#![no_std]

extern crate alloc;

pub mod audio;
mod draw;
mod save;
pub mod sprites;
pub mod ui;

use bevy::{app::AppExit, math::ops, prelude::*};
use orbit_core::{
    Buttons, Clip, ControlsPlugin, CorePlugin, Launcher, Power, Rng, SaveBlob, SoundPlugin,
    SoundQueue, Tilt, Touch,
};
use orbit_gfx::Rgb565;
use orbit_ui::{CLICK, dial_choice};
use pals::Pals;

pub const EGGS: usize = 12;
/// One egg's slice of the ring.
pub const EGG_DEG: f32 = 360.0 / EGGS as f32;
/// How long a spin takes to slow to a stop, and how many whole turns it makes.
pub const SPIN_SECS: f32 = 2.4;
pub const SPIN_TURNS: f32 = 3.0;
/// The pause on the chosen egg before it cracks.
pub const REVEAL_SECS: f32 = 0.5;
/// How long the spider leaps before the result shows.
pub const POUNCE_SECS: f32 = 1.2;
/// Results ignore presses this long, so a panicked double press can't skip them.
pub const RESULT_GUARD_SECS: f32 = 0.8;
pub const BACKGROUND: Rgb565 = orbit_ui::BLACK;
/// What the den takes bets of.
pub const STAKES: [u32; 5] = [5, 10, 25, 50, 100];
/// The house keeps a tenth of fair odds.
const PAYS_PCT: u32 = 90;

/// The stakes a purse of `gold` can place, then a free game (0).
pub fn stakes(gold: u32) -> alloc::vec::Vec<u32> {
    let mut offered: alloc::vec::Vec<u32> = STAKES
        .iter()
        .copied()
        .filter(|&stake| stake <= gold)
        .collect();
    offered.push(0);
    offered
}

/// What a bet of `stake` is worth after `safe` safe eggs: the odds of having
/// come that far are (eggs - safe) in eggs, so fair pay is their inverse.
pub fn cash_value(stake: u32, safe: u8) -> u32 {
    let left = (EGGS as u32).saturating_sub(u32::from(safe)).max(1);
    stake * EGGS as u32 * PAYS_PCT / (100 * left)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Title,
    /// Choosing a stake, with gold in the purse.
    Bet,
    /// Waiting for PWR to spin.
    Ready,
    Spinning,
    /// Stopped on an egg, about to crack it.
    Reveal,
    Pounce,
    Bitten,
    Won,
    /// Banked the bet's worth.
    CashedOut,
}

/// The round in play, and the best run.
#[derive(Resource, Debug)]
pub struct Table {
    pub phase: Phase,
    pub phase_secs: f32,
    pub safe: u8,
    pub best: u16,
    pub new_best: bool,
    /// This round's bet, and what the round paid out.
    pub stake: u32,
    pub payout: u32,
    /// Where the pointer is, degrees clockwise from 12 o'clock, ever increasing.
    pub pointer_deg: f32,
    pub clock: f32,
    cracked: [bool; EGGS],
    spider: usize,
    spin_from: f32,
    spin_travel: f32,
    target: usize,
    rigged: Option<usize>,
    heartbeat_in: f32,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            phase: Phase::Title,
            phase_secs: 0.0,
            safe: 0,
            best: 0,
            new_best: false,
            stake: 0,
            payout: 0,
            pointer_deg: 0.0,
            clock: 0.0,
            cracked: [false; EGGS],
            spider: 0,
            spin_from: 0.0,
            spin_travel: 0.0,
            target: 0,
            rigged: None,
            heartbeat_in: 0.0,
        }
    }
}

impl Table {
    pub fn cracked(&self, egg: usize) -> bool {
        self.cracked[egg]
    }

    /// Eggs still whole, the spider's among them: the odds are one in this.
    pub fn whole(&self) -> usize {
        self.cracked.iter().filter(|cracked| !**cracked).count()
    }

    /// The egg under the pointer.
    pub fn pointed(&self) -> usize {
        (ops::round(self.pointer_deg / EGG_DEG) as usize) % EGGS
    }

    /// Moves the spider, for tests that need to know where it is.
    pub fn hide_spider(&mut self, egg: usize) {
        self.spider = egg;
    }

    /// Makes the next spin stop on `egg` (if it's whole), for tests.
    pub fn rig_next_spin(&mut self, egg: usize) {
        self.rigged = Some(egg);
    }

    fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_secs = 0.0;
    }

    fn deal(&mut self, rng: &mut Rng, stake: u32) {
        self.stake = stake;
        self.payout = 0;
        self.cracked = [false; EGGS];
        self.spider = rng.next_u32() as usize % EGGS;
        self.safe = 0;
        self.new_best = false;
        self.heartbeat_in = 0.5;
        self.enter(Phase::Ready);
    }

    /// Picks a whole egg, every one as likely, and sets the pointer to wind
    /// down onto it.
    fn spin(&mut self, rng: &mut Rng) {
        let whole: alloc::vec::Vec<usize> = (0..EGGS).filter(|&egg| !self.cracked[egg]).collect();
        self.target = match self.rigged.take() {
            Some(egg) if !self.cracked[egg] => egg,
            _ => whole[rng.next_u32() as usize % whole.len()],
        };
        self.spin_from = self.pointer_deg;
        let offset = ops::rem_euclid(self.target as f32 * EGG_DEG - self.pointer_deg, 360.0);
        self.spin_travel = SPIN_TURNS * 360.0 + offset;
        self.enter(Phase::Spinning);
    }

    fn record(&mut self) -> bool {
        self.new_best = u16::from(self.safe) > self.best;
        self.best = self.best.max(u16::from(self.safe));
        self.new_best
    }
}

/// Seconds between heartbeats with `whole` eggs left: slow at the start,
/// racing when only a couple remain.
fn heartbeat_interval(whole: usize) -> f32 {
    0.35 + 0.9 * (whole.saturating_sub(2) as f32 / (EGGS - 2) as f32)
}

pub struct SpiderPlugin;

impl Plugin for SpiderPlugin {
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
        app.init_resource::<Table>()
            .init_resource::<Pals>()
            .add_systems(Startup, load)
            .add_systems(Update, play)
            .add_systems(PostUpdate, draw::draw_frame);
    }
}

fn load(blob: Res<SaveBlob>, mut table: ResMut<Table>, mut pals: ResMut<Pals>) {
    if let Some(best) = blob.section(&save::KEY).and_then(save::decode) {
        table.best = best;
    }
    *pals = pals::save::load(&blob);
}

#[allow(clippy::too_many_arguments)]
fn play(
    time: Res<Time>,
    buttons: Res<Buttons>,
    mut tilt: ResMut<Tilt>,
    touch: Res<Touch>,
    power: Res<Power>,
    launcher: Option<Res<Launcher>>,
    mut table: ResMut<Table>,
    mut pals: ResMut<Pals>,
    mut rng: ResMut<Rng>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    table.phase_secs += dt;
    table.clock += dt;
    if power.saying_good_night() {
        return;
    }
    let tap = touch.just_pressed().then(|| touch.position()).flatten();
    let go = buttons.action.pressed() || tap.is_some();
    let mut sfx: Option<&'static Clip> = None;
    let staked = table.stake > 0;
    match table.phase {
        Phase::Title | Phase::Bitten | Phase::Won | Phase::CashedOut => {
            let open = table.phase == Phase::Title || table.phase_secs >= RESULT_GUARD_SECS;
            if launcher.is_some() && tap.is_some_and(|at| ui::HOME.hit(at)) {
                sfx = Some(&CLICK);
                exit.write(AppExit::Success);
            } else if open && go && pals.gold >= STAKES[0] {
                table.enter(Phase::Bet);
                tilt.request_zero();
                sfx = Some(&CLICK);
            } else if open && go {
                table.deal(&mut rng, 0);
                sfx = Some(&CLICK);
            }
        }
        Phase::Bet => {
            let offered = stakes(pals.gold);
            let tapped = tap.and_then(|at| {
                let spots = (0..offered.len()).map(|i| ui::stake_center(i, offered.len()));
                orbit_ui::nearest(at, spots, ui::STAKE_REACH)
            });
            let picked = tapped.or(buttons
                .action
                .pressed()
                .then(|| dial_choice(tilt.steer_deg, offered.len())));
            if let Some(stake) = picked.map(|i| offered[i]) {
                pals.spend_up_to(stake);
                pals::save::store(&mut blob, &pals);
                table.deal(&mut rng, stake);
                sfx = Some(&CLICK);
            }
        }
        Phase::Ready if staked && table.safe > 0 && tap.is_some_and(|at| ui::CASH_OUT.hit(at)) => {
            table.payout = cash_value(table.stake, table.safe);
            pals.earn(table.payout);
            pals::save::store(&mut blob, &pals);
            if table.record() {
                blob.save_section(save::KEY, save::encode(table.best));
            }
            table.enter(Phase::CashedOut);
            sfx = Some(&audio::WIN);
        }
        Phase::Ready if go => table.spin(&mut rng),
        Phase::Ready => {}
        Phase::Spinning => {
            let before = table.pointed();
            let progress = (table.phase_secs / SPIN_SECS).min(1.0);
            let eased = 1.0 - (1.0 - progress) * (1.0 - progress) * (1.0 - progress);
            table.pointer_deg = table.spin_from + table.spin_travel * eased;
            if table.pointed() != before {
                sfx = Some(&audio::TICK);
            }
            if progress >= 1.0 {
                table.enter(Phase::Reveal);
            }
        }
        Phase::Reveal if table.phase_secs >= REVEAL_SECS => {
            let egg = table.target;
            table.cracked[egg] = true;
            if egg == table.spider {
                table.enter(Phase::Pounce);
                sfx = Some(&audio::SCREECH);
            } else {
                table.safe += 1;
                sfx = Some(&audio::CRACK);
                if table.whole() == 1 {
                    table.record();
                    blob.save_section(save::KEY, save::encode(table.best));
                    if staked {
                        table.payout = cash_value(table.stake, table.safe);
                        pals.earn(table.payout);
                        pals::save::store(&mut blob, &pals);
                    }
                    table.enter(Phase::Won);
                    sfx = Some(&audio::WIN);
                } else {
                    table.enter(Phase::Ready);
                }
            }
        }
        Phase::Reveal => {}
        Phase::Pounce if table.phase_secs >= POUNCE_SECS => {
            if table.record() {
                blob.save_section(save::KEY, save::encode(table.best));
            }
            table.enter(Phase::Bitten);
        }
        Phase::Pounce => {}
    }
    if matches!(table.phase, Phase::Ready | Phase::Spinning | Phase::Reveal) {
        table.heartbeat_in -= dt;
        if table.heartbeat_in <= 0.0 {
            table.heartbeat_in = heartbeat_interval(table.whole());
            sfx = sfx.or(Some(&audio::HEARTBEAT));
        }
    }
    if let Some(clip) = sfx {
        sound.play(clip);
    }
}
