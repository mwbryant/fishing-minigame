//! One browser tab as the board: the town (or one app on its own), the app
//! behind each door, switching off and on, the speaker, mic and save flash.

use std::time::Duration;

use angler_game::AnglerPlugin;
use bees_game::BeesPlugin;
use bevy::prelude::*;
use orbit_audio::Mixer;
use orbit_core::{
    Battery, Buttons, Frame, ImuFeed, Launcher, Mic, Power, Rng, SaveBlob, SoundQueue, Steps,
    Touch, preferences,
};
use orbit_game::OrbitPlugin;
use orbit_gfx::{Presenter, Rgb565};
use orbit_motion::{TURN_DEG_PER_SEC, VirtualDevice};
use orbit_save::{RamFlash, SECTOR, SlotStore};
use pals_game::{
    Arrival, Place,
    contest::ContestPlugin,
    home::HomePlugin,
    shop::ShopPlugin,
    town::{Town, TownPlugin},
    walk::WalkPlugin,
};
use skate_game::SkatePlugin;
use spider_game::SpiderPlugin;

use crate::panel::{RgbaPanel, SIZE};

/// Matches the firmware's 8-row band buffer.
const BAND_BYTES: usize = SIZE * 2 * 8;

/// What the page boots: the town (whose doors lead to the others) or one app
/// on its own, like the firmware's binaries and the desktop simulator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Game {
    Town,
    Home,
    Park,
    Contest,
    Shop,
    Orbit,
    Angler,
    Spider,
    Skate,
    Bees,
}

impl Game {
    const ALL: [Game; 10] = [
        Game::Town,
        Game::Home,
        Game::Park,
        Game::Contest,
        Game::Shop,
        Game::Orbit,
        Game::Angler,
        Game::Spider,
        Game::Skate,
        Game::Bees,
    ];

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|game| game.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Game::Town => "town",
            Game::Home => "home",
            Game::Park => "park",
            Game::Contest => "contest",
            Game::Shop => "shop",
            Game::Orbit => "orbit",
            Game::Angler => "angler",
            Game::Spider => "spider",
            Game::Skate => "skate",
            Game::Bees => "bees",
        }
    }

    fn add_to(self, app: &mut App) {
        match self {
            Game::Town => app.add_plugins(TownPlugin),
            Game::Home => app.add_plugins(HomePlugin),
            Game::Park => app.add_plugins(WalkPlugin),
            Game::Contest => app.add_plugins(ContestPlugin),
            Game::Shop => app.add_plugins(ShopPlugin),
            Game::Orbit => app.add_plugins(OrbitPlugin),
            Game::Angler => app.add_plugins(AnglerPlugin),
            Game::Spider => app.add_plugins(SpiderPlugin),
            Game::Skate => app.add_plugins(SkatePlugin),
            Game::Bees => app.add_plugins(BeesPlugin),
        };
    }

    fn background(self) -> Rgb565 {
        match self {
            Game::Town => pals_game::town::BACKGROUND,
            Game::Home => pals_game::home::BACKGROUND,
            Game::Park => pals_game::walk::BACKGROUND,
            Game::Contest => pals_game::contest::BACKGROUND,
            Game::Shop => pals_game::shop::BACKGROUND,
            Game::Orbit => orbit_game::BACKGROUND,
            Game::Angler => angler_game::BACKGROUND,
            Game::Spider => spider_game::BACKGROUND,
            Game::Skate => skate_game::BACKGROUND,
            Game::Bees => bees_game::BACKGROUND,
        }
    }

    /// The app behind one of the town's doors.
    fn behind(place: Place) -> Option<Self> {
        match place {
            Place::Home => Some(Game::Home),
            Place::Park => Some(Game::Park),
            Place::Contest => Some(Game::Contest),
            Place::Shop => Some(Game::Shop),
            Place::Pond => Some(Game::Angler),
            Place::Den => Some(Game::Spider),
            Place::Arcade => Some(Game::Skate),
            Place::BeeTree => Some(Game::Bees),
            Place::Settings => None,
        }
    }
}

/// The page's keys and side buttons, one bit each (`page/board.js` sends
/// the same bits).
pub mod key {
    /// Space or the upper side button (PWR).
    pub const ACTION: u32 = 1;
    /// Z or the lower side button (BOOT).
    pub const ZERO: u32 = 1 << 1;
    pub const LEFT: u32 = 1 << 2;
    pub const RIGHT: u32 = 1 << 3;
    pub const UP: u32 = 1 << 4;
    pub const DOWN: u32 = 1 << 5;
    pub const STRAIGHTEN: u32 = 1 << 6;
    pub const WALK: u32 = 1 << 7;
}

/// What the page read this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub finger: Option<Vec2>,
    /// [`key`] bits held down.
    pub keys: u32,
    /// The phone's pose, when its orientation sensor reports one.
    pub phone: Option<Quat>,
}

impl Input {
    fn held(&self, bit: u32) -> bool {
        self.keys & bit != 0
    }

    fn axis(&self, plus: u32, minus: u32) -> f32 {
        f32::from(u8::from(self.held(plus))) - f32::from(u8::from(self.held(minus)))
    }
}

/// Switched off: the last frame stays up until BOOT is let go, then the
/// panel is dark until PWR turns the board on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Off {
    Holding,
    Dark,
}

pub struct Board {
    boot: Game,
    playing: Game,
    app: App,
    arrival: Option<Place>,
    off: Option<Off>,
    seed: u32,
    last: Duration,
    presenter: Presenter<Vec<u8>>,
    panel: RgbaPanel,
    drawn: bool,
    device: VirtualDevice,
    flash: SlotStore<RamFlash>,
    saved: bool,
    mixer: Mixer,
    mix: Vec<i16>,
    speaker: Vec<f32>,
    brightness: u8,
    frame_rate: u32,
}

impl Board {
    /// `flash` is a save image from an earlier visit (two sectors, as the
    /// firmware and the desktop simulator store it).
    pub fn boot(game: Game, flash: Option<Vec<u8>>, seed: u32) -> Self {
        let flash = flash
            .filter(|bytes| bytes.len() == 2 * SECTOR as usize)
            .map_or_else(|| RamFlash::new(2), RamFlash::from_bytes);
        let mut board = Self {
            boot: game,
            playing: game,
            app: App::empty(),
            arrival: None,
            off: None,
            seed,
            last: crate::clock::read(),
            presenter: Presenter::new(Vec::new(), game.background()),
            panel: RgbaPanel::default(),
            drawn: false,
            device: VirtualDevice::upright(),
            flash: SlotStore::new(flash, 0),
            saved: false,
            mixer: Mixer::default(),
            mix: Vec::new(),
            speaker: Vec::new(),
            brightness: 100,
            frame_rate: 60,
        };
        board.launch(game);
        board
    }

    /// Builds `game` in place of the running app, like a boot of the board.
    fn launch(&mut self, game: Game) {
        self.seed = self
            .seed
            .wrapping_mul(747_796_405)
            .wrapping_add(2_891_336_453);
        let launched = self.boot == Game::Town && game != self.boot;
        self.app = App::empty();
        let mut app = App::new();
        game.add_to(&mut app);
        if launched {
            app.insert_resource(Launcher);
        }
        if let (Game::Town, Some(place)) = (game, self.arrival) {
            app.insert_resource(Arrival(place));
        }
        app.insert_resource(Rng::new(self.seed))
            .insert_resource(SaveBlob::from_storage(self.flash.load()));
        app.finish();
        app.cleanup();
        if let Some(mut power) = app.world_mut().get_resource_mut::<Power>() {
            power.battery = Some(Battery {
                percent: None,
                external_power: true,
                charging: false,
            });
        }
        self.app = app;
        self.playing = game;
        self.presenter = Presenter::new(vec![0u8; BAND_BYTES], game.background());
        self.device = VirtualDevice::upright();
    }

    /// One frame at `now` (the clock the app reads, already set).
    pub fn frame(&mut self, now: Duration, input: &Input) {
        let dt = now.saturating_sub(self.last).as_secs_f32();
        self.last = now;
        if let Some(off) = self.off {
            self.stay_off(off, input);
            return;
        }
        self.feed(input, dt);
        self.app.update();
        self.collect();
        let world = self.app.world();
        let power = world.get_resource::<Power>().copied().unwrap_or_default();
        if power.sleep_requested() {
            self.off = Some(Off::Holding);
            self.frame_rate = 20;
            return;
        }
        if self.app.should_exit().is_none() {
            return;
        }
        match world.get_resource::<Town>().and_then(Town::destination) {
            Some(place) => {
                self.arrival = Some(place);
                self.launch(Game::behind(place).unwrap_or(self.boot));
            }
            None => self.launch(self.boot),
        }
    }

    fn stay_off(&mut self, off: Off, input: &Input) {
        match off {
            Off::Holding if !input.held(key::ZERO) => {
                self.panel.clear();
                self.drawn = true;
                self.off = Some(Off::Dark);
            }
            Off::Dark if input.held(key::ACTION) => {
                self.off = None;
                self.arrival = None;
                self.frame_rate = 60;
                self.launch(self.boot);
            }
            _ => {}
        }
    }

    /// The finger, buttons, pedometer and IMU, for the apps that use them.
    fn feed(&mut self, input: &Input, dt: f32) {
        let world = self.app.world_mut();
        world.resource_mut::<Touch>().set(input.finger);
        if let Some(mut buttons) = world.get_resource_mut::<Buttons>() {
            buttons.action.set(input.held(key::ACTION));
            buttons.zero.set(input.held(key::ZERO));
        }
        if let Some(mut steps) = world.get_resource_mut::<Steps>() {
            steps.add(self.device.steps(input.held(key::WALK), dt));
        }
        if let Some(mut feed) = world.get_resource_mut::<ImuFeed>() {
            let sample = match input.phone {
                Some(pose) => self.device.held_at(pose, dt),
                None => {
                    if input.held(key::STRAIGHTEN) {
                        self.device.straighten();
                    }
                    let steer = input.axis(key::RIGHT, key::LEFT) * TURN_DEG_PER_SEC;
                    let lean = input.axis(key::UP, key::DOWN);
                    self.device.sample(steer, lean, dt)
                }
            };
            feed.push(sample);
        }
    }

    /// The app's save, sounds, frame, brightness and pace after an update.
    fn collect(&mut self) {
        let world = self.app.world_mut();
        if let Some(bytes) = world.resource_mut::<SaveBlob>().take_pending() {
            self.saved |= self.flash.save(&bytes).is_ok();
        }
        if let Some(mut queue) = world.get_resource_mut::<SoundQueue>() {
            for command in queue.drain() {
                self.mixer.apply(command);
            }
        }
        let sent = self
            .presenter
            .present(&world.resource::<Frame>().shapes, &mut self.panel);
        self.drawn |= sent > 0;
        let power = world.get_resource::<Power>().copied().unwrap_or_default();
        let percent = world
            .get_resource::<SaveBlob>()
            .map_or(100, preferences::brightness);
        self.brightness = if power.idle { percent / 4 } else { percent };
        self.frame_rate = match (power.dozing(), power.idle) {
            (true, _) => 5,
            (false, true) => 20,
            (false, false) => 60,
        };
    }

    pub fn playing(&self) -> Game {
        self.playing
    }

    pub fn is_off(&self) -> bool {
        self.off.is_some()
    }

    pub fn pixels(&self) -> &[u8] {
        self.panel.pixels()
    }

    /// True once after the pixels change.
    pub fn take_drawn(&mut self) -> bool {
        core::mem::take(&mut self.drawn)
    }

    /// The panel's brightness in percent: the setting, a quarter of it while
    /// idle.
    pub fn brightness(&self) -> u8 {
        self.brightness
    }

    /// Frames a second the board runs at now: fewer while idle or dozing.
    pub fn frame_rate(&self) -> u32 {
        self.frame_rate
    }

    /// The running app hears the mic.
    pub fn listening(&self) -> bool {
        self.off.is_none() && self.app.world().contains_resource::<Mic>()
    }

    /// True once after a save changes the flash image.
    pub fn take_saved(&mut self) -> bool {
        core::mem::take(&mut self.saved)
    }

    /// The save flash, both sectors, to keep for the next visit.
    pub fn flash(&self) -> &[u8] {
        self.flash.flash().bytes()
    }

    /// The speaker's next `samples` at `orbit_audio::SAMPLE_RATE`, -1..1.
    pub fn speak(&mut self, samples: usize) -> &[f32] {
        self.mix.resize(samples, 0);
        self.mixer.fill(&mut self.mix);
        self.speaker.clear();
        self.speaker
            .extend(self.mix.iter().map(|&s| f32::from(s) / 32768.0));
        &self.speaker
    }

    /// 16 kHz mic input, for an app that listens; others drop it.
    pub fn hear(&mut self, samples: &[i16]) {
        if let Some(mut mic) = self.app.world_mut().get_resource_mut::<Mic>() {
            mic.push(samples);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock;

    const FRAME: Duration = Duration::from_micros(16_667);

    /// Runs `board` for `secs` at 60 fps with `input` held.
    fn hold(board: &mut Board, input: Input, secs: f32) {
        for _ in 0..(secs * 60.0) as u32 {
            let now = clock::read() + FRAME;
            clock::set(now);
            board.frame(now, &input);
        }
    }

    fn keys(keys: u32) -> Input {
        Input {
            keys,
            ..Input::default()
        }
    }

    fn booted(game: Game, flash: Option<Vec<u8>>) -> Board {
        clock::start();
        let mut board = Board::boot(game, flash, 7);
        hold(&mut board, Input::default(), 0.5);
        board
    }

    #[test]
    fn every_game_boots_by_its_name_and_draws() {
        for game in Game::ALL {
            assert_eq!(Game::named(game.name()), Some(game));
            let mut board = booted(game, None);
            assert!(board.take_drawn(), "{} drew nothing", game.name());
            assert!(
                board
                    .pixels()
                    .chunks_exact(4)
                    .any(|px| px[..3] != [0, 0, 0]),
                "{} is black",
                game.name()
            );
        }
        assert_eq!(Game::named("pals"), None);
    }

    #[test]
    fn holding_boot_switches_the_board_off_and_pwr_turns_it_back_on() {
        let mut board = booted(Game::Town, None);
        hold(&mut board, keys(key::ZERO), 6.0);
        assert!(board.is_off());
        hold(&mut board, Input::default(), 0.1);
        assert!(
            board
                .pixels()
                .chunks_exact(4)
                .all(|px| px[..3] == [0, 0, 0])
        );
        hold(&mut board, Input::default(), 2.0);
        assert!(board.is_off(), "only PWR turns it on");
        hold(&mut board, keys(key::ACTION), 0.1);
        assert!(!board.is_off());
        assert_eq!(board.playing(), Game::Town);
        hold(&mut board, Input::default(), 0.5);
        assert!(
            board
                .pixels()
                .chunks_exact(4)
                .any(|px| px[..3] != [0, 0, 0])
        );
    }

    #[test]
    fn a_save_comes_back_in_the_flash_image_the_next_visit_boots_from() {
        let mut board = booted(Game::Town, None);
        board
            .app
            .world_mut()
            .resource_mut::<SaveBlob>()
            .save_section(*b"TEST", vec![1, 2, 3]);
        hold(&mut board, Input::default(), 0.1);
        assert!(board.take_saved());
        assert!(!board.take_saved());
        let image = board.flash().to_vec();
        let again = booted(Game::Town, Some(image));
        let blob = again.app.world().resource::<SaveBlob>();
        assert_eq!(blob.section(b"TEST"), Some(&[1u8, 2, 3][..]));
    }
}
