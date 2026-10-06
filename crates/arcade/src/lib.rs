//! The boot menu. Turning the device sweeps a dial over the four games and
//! the settings; PWR (or a tap) picks. Picking a game ends this app with
//! `AppExit`, and the platform builds that game in its place.

#![no_std]

extern crate alloc;

mod draw;
mod sprites;
pub mod ui;

use bevy::{app::AppExit, prelude::*};
use orbit_core::{
    Buttons, ControlsPlugin, CorePlugin, Power, SaveBlob, SoundPlugin, SoundQueue, Tilt, Touch,
    preferences,
};
use orbit_gfx::Rgb565;
use orbit_ui::{CLICK, dial_choice};

/// Black is free on an AMOLED: the pixel is off.
pub const BACKGROUND: Rgb565 = orbit_ui::BLACK;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Game {
    Angler,
    Spider,
    Skate,
    Bees,
}

/// What the dial offers, clockwise from 12 o'clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Play(Game),
    Settings,
}

pub const CHOICES: [Choice; 5] = [
    Choice::Play(Game::Angler),
    Choice::Play(Game::Spider),
    Choice::Play(Game::Skate),
    Choice::Play(Game::Bees),
    Choice::Settings,
];

impl Choice {
    /// One or two short lines for the middle of the dial.
    pub fn name(self) -> [&'static str; 2] {
        match self {
            Choice::Play(Game::Angler) => ["ANGLER", ""],
            Choice::Play(Game::Spider) => ["SPIDER", "ROULETTE"],
            Choice::Play(Game::Skate) => ["SKATE", ""],
            Choice::Play(Game::Bees) => ["BEES???", ""],
            Choice::Settings => ["SETTINGS", ""],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Dial,
    Settings,
}

/// Settings' dial, clockwise from 12 o'clock: SOUND on top, BACK below.
const SETTINGS_OPTIONS: usize = 2;

#[derive(Resource, Default, Debug)]
pub struct Menu {
    screen: Screen,
    chosen: Option<Game>,
}

impl Menu {
    /// The game picked, once the menu has ended.
    pub fn chosen(&self) -> Option<Game> {
        self.chosen
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }
}

pub struct ArcadePlugin;

impl Plugin for ArcadePlugin {
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
        app.init_resource::<Menu>()
            .add_systems(Startup, level)
            .add_systems(Update, pick)
            .add_systems(PostUpdate, draw::draw_frame);
    }
}

/// However the device is held when the menu comes up points at the top.
fn level(mut tilt: ResMut<Tilt>) {
    tilt.request_zero();
}

#[allow(clippy::too_many_arguments)]
fn pick(
    buttons: Res<Buttons>,
    tilt: Res<Tilt>,
    touch: Res<Touch>,
    power: Res<Power>,
    mut menu: ResMut<Menu>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    if power.saying_good_night() || menu.chosen.is_some() {
        return;
    }
    let pressed = buttons.action.pressed();
    let tapped = |button: &orbit_ui::Button| {
        touch.just_pressed() && touch.position().is_some_and(|at| button.hit(at))
    };
    match menu.screen {
        Screen::Dial => {
            let pointed = CHOICES[dial_choice(tilt.steer_deg, CHOICES.len())];
            let picked = if pressed {
                Some(pointed)
            } else {
                ui::CHOICE_BUTTONS
                    .iter()
                    .zip(CHOICES)
                    .find(|(button, _)| tapped(button))
                    .map(|(_, choice)| choice)
            };
            match picked {
                Some(Choice::Play(game)) => {
                    sound.play(&CLICK);
                    menu.chosen = Some(game);
                    exit.write(AppExit::Success);
                }
                Some(Choice::Settings) => {
                    sound.play(&CLICK);
                    menu.screen = Screen::Settings;
                }
                None => {}
            }
        }
        Screen::Settings => {
            let pointed = dial_choice(tilt.steer_deg, SETTINGS_OPTIONS);
            if (pressed && pointed == 0) || tapped(&ui::SOUND) {
                let muted = !sound.muted();
                preferences::set_muted(&mut blob, &mut sound, muted);
                sound.play(&CLICK);
            } else if (pressed && pointed == 1) || tapped(&ui::BACK) {
                sound.play(&CLICK);
                menu.screen = Screen::Dial;
            }
        }
    }
}
