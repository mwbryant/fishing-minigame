//! Spider Roulette: twelve eggs ring a web and one hides a spider. Turn the
//! device to aim, press PWR (or tap) to crack an egg. Every safe egg
//! shortens the odds and quickens the heartbeat; the spider's egg pounces.
//! Crack all eleven safe ones and the spider is trapped. The best run of safe
//! picks persists in the board's save record.

#![no_std]

extern crate alloc;

pub mod audio;
mod draw;
mod save;
mod sprites;
pub mod ui;

use bevy::{app::AppExit, prelude::*};
use orbit_core::{
    Buttons, Clip, ControlsPlugin, CorePlugin, Launcher, Power, Rng, SaveBlob, SoundPlugin,
    SoundQueue, Tilt, Touch,
};
use orbit_gfx::Rgb565;
use orbit_ui::{CLICK, THUD, dial_choice};

pub const EGGS: usize = 12;
/// How long the spider leaps before the result shows.
pub const POUNCE_SECS: f32 = 1.2;
/// Results ignore presses this long, so a panicked double press can't skip them.
pub const RESULT_GUARD_SECS: f32 = 0.8;
pub const BACKGROUND: Rgb565 = orbit_ui::BLACK;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Title,
    Pick,
    Pounce,
    Bitten,
    Won,
}

/// The round in play, and the best run.
#[derive(Resource, Debug)]
pub struct Table {
    pub phase: Phase,
    pub phase_secs: f32,
    pub safe: u8,
    pub best: u16,
    pub new_best: bool,
    cracked: [bool; EGGS],
    spider: usize,
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
            cracked: [false; EGGS],
            spider: 0,
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

    /// Moves the spider, for tests that need to know where it is.
    pub fn hide_spider(&mut self, egg: usize) {
        self.spider = egg;
    }

    fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_secs = 0.0;
    }

    fn deal(&mut self, rng: &mut Rng) {
        self.cracked = [false; EGGS];
        self.spider = rng.next_u32() as usize % EGGS;
        self.safe = 0;
        self.new_best = false;
        self.heartbeat_in = 0.5;
        self.enter(Phase::Pick);
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
            .add_systems(Startup, load)
            .add_systems(Update, play)
            .add_systems(PostUpdate, draw::draw_frame);
    }
}

fn load(blob: Res<SaveBlob>, mut table: ResMut<Table>) {
    if let Some(best) = blob.section(&save::KEY).and_then(save::decode) {
        table.best = best;
    }
}

#[allow(clippy::too_many_arguments)]
fn play(
    time: Res<Time>,
    buttons: Res<Buttons>,
    touch: Res<Touch>,
    power: Res<Power>,
    launcher: Option<Res<Launcher>>,
    mut tilt: ResMut<Tilt>,
    mut table: ResMut<Table>,
    mut rng: ResMut<Rng>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    table.phase_secs += dt;
    if power.saying_good_night() {
        return;
    }
    let pressed = buttons.action.pressed();
    let tap = touch.just_pressed().then(|| touch.position()).flatten();
    let mut sfx: Option<&'static Clip> = None;
    match table.phase {
        Phase::Title | Phase::Bitten | Phase::Won => {
            let open = table.phase == Phase::Title || table.phase_secs >= RESULT_GUARD_SECS;
            if launcher.is_some() && tap.is_some_and(|at| ui::HOME.hit(at)) {
                sfx = Some(&CLICK);
                exit.write(AppExit::Success);
            } else if open && (pressed || tap.is_some()) {
                tilt.request_zero();
                table.deal(&mut rng);
                sfx = Some(&CLICK);
            }
        }
        Phase::Pick => {
            let aimed = dial_choice(tilt.steer_deg, EGGS);
            let target = if pressed {
                Some(aimed)
            } else {
                tap.and_then(ui::egg_at)
            };
            match target {
                Some(egg) if table.cracked[egg] => sfx = Some(&THUD),
                Some(egg) if egg == table.spider => {
                    table.cracked[egg] = true;
                    table.enter(Phase::Pounce);
                    sfx = Some(&audio::SCREECH);
                }
                Some(egg) => {
                    table.cracked[egg] = true;
                    table.safe += 1;
                    sfx = Some(&audio::CRACK);
                    if table.whole() == 1 {
                        table.record();
                        blob.save_section(save::KEY, save::encode(table.best));
                        table.enter(Phase::Won);
                        sfx = Some(&audio::WIN);
                    }
                }
                None => {}
            }
            table.heartbeat_in -= dt;
            if table.phase == Phase::Pick && table.heartbeat_in <= 0.0 {
                table.heartbeat_in = heartbeat_interval(table.whole());
                sfx = sfx.or(Some(&audio::HEARTBEAT));
            }
        }
        Phase::Pounce if table.phase_secs >= POUNCE_SECS => {
            if table.record() {
                blob.save_section(save::KEY, save::encode(table.best));
            }
            table.enter(Phase::Bitten);
        }
        Phase::Pounce => {}
    }
    if let Some(clip) = sfx {
        sound.play(clip);
    }
}
