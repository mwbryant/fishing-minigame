//! Desktop stand-in for the ESP32-S3-Touch-AMOLED-1.75: the same Bevy games
//! and dirty-region band presenter, drawing into a RAM model of the CO5300,
//! with a virtual IMU driven from the keyboard.
//!
//! ```text
//! cargo run -p orbit_desktop -- [GAME]   play (default town: Fishing Minigame)
//!   Holding Z (BOOT) sleeps the board like the firmware does; Z again wakes it.
//! cargo run -p orbit_desktop -- GAME snapshot [DIR]   scripted PNGs, headless
//!   (DIR defaults to screenshots)
//! cargo run -p orbit_desktop -- footage [DIR [TAKE...]]   the trailer's takes,
//!   every frame as a PNG plus the speaker's WAV (DIR defaults to
//!   trailer/cache/footage; see trailer/README.md)
//!
//! GAME: town home park contest shop angler spider skate bees orbit
//! On the desktop, walks use MOCK WALK (in the town's settings).
//! ```

mod device;
mod footage;
mod microphone;
mod panel;
mod snapshots;
mod speaker;

use std::{
    env,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use angler_game::AnglerPlugin;
use bees_game::BeesPlugin;
use bevy::prelude::*;
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use orbit_core::{
    Battery, Buttons, Frame, ImuFeed, Launcher, Mic, Power, Rng, SaveBlob, SoundQueue, Touch,
};
use orbit_game::OrbitPlugin;
use orbit_gfx::Presenter;
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

use crate::{
    device::{SaveFile, VirtualDevice},
    microphone::Microphone,
    panel::{RamPanel, SIZE},
    speaker::Speaker,
};

/// Matches the firmware's 8-row band buffer.
pub const BAND_BYTES: usize = SIZE * 2 * 8;
/// What the real link moves: 40 MHz QSPI clock, four data lines.
const QSPI_BYTES_PER_SEC: f64 = 40e6 * 4.0 / 8.0;
/// How fast ←/→ turn the virtual device.
const TURN_DEG_PER_SEC: f32 = 60.0;

/// What the simulator boots: the town (whose doors lead to the others) or
/// one app on its own, like the firmware's binaries.
#[derive(Clone, Copy, PartialEq, Eq)]
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
    fn name(self) -> &'static str {
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

    /// The app behind one of the town's doors, once it exists.
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
            _ => None,
        }
    }
}

/// How one run of an app ended.
enum Ended {
    Closed,
    Slept,
    /// The app sent `AppExit` on its own (a game went back OUT).
    Exited,
    /// The town sent `AppExit` at a door.
    WentTo(Place),
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("footage") {
        let dir = args.get(1).map_or("trailer/cache/footage", String::as_str);
        footage::run(Path::new(dir), args.get(2..).unwrap_or_default());
        return;
    }
    let (game, rest) = match args.first().map(String::as_str) {
        Some("town") => (Game::Town, &args[1..]),
        Some("home") => (Game::Home, &args[1..]),
        Some("park") => (Game::Park, &args[1..]),
        Some("contest") => (Game::Contest, &args[1..]),
        Some("shop") => (Game::Shop, &args[1..]),
        Some("orbit") => (Game::Orbit, &args[1..]),
        Some("angler") => (Game::Angler, &args[1..]),
        Some("spider") => (Game::Spider, &args[1..]),
        Some("skate") => (Game::Skate, &args[1..]),
        Some("bees") => (Game::Bees, &args[1..]),
        _ => (Game::Town, &args[..]),
    };
    match rest.first().map(String::as_str) {
        Some("snapshot") => snapshots::run(
            game,
            Path::new(rest.get(1).map_or("screenshots", String::as_str)),
        ),
        _ => play(game),
    }
}

fn play(boot: Game) {
    println!(
        "{}: mouse = finger | Space = action (PWR) | Z = zero (BOOT; hold to sleep, press to wake) | \
         left/right = turn the device | hold up/down = lean | C = straighten | Esc = quit",
        boot.name()
    );
    let mut window =
        Window::new("Orbit board", SIZE, SIZE, WindowOptions::default()).expect("open window");
    window.set_target_fps(60);
    let mut save = SaveFile::open(boot.name());
    let mut sound = Sound::default();
    let mut pixels = vec![0u32; SIZE * SIZE];
    let mut next = boot;
    let mut arrival = None;
    loop {
        let launched = boot == Game::Town && next != boot;
        match run(
            next,
            launched,
            arrival,
            &mut window,
            &mut save,
            &mut sound,
            &mut pixels,
        ) {
            Ended::Closed => return,
            Ended::Slept => {
                println!("asleep: press Z (BOOT) to wake");
                if !sleep_until_woken(&mut window, &pixels) {
                    return;
                }
                next = boot;
                arrival = None;
            }
            Ended::Exited => next = boot,
            Ended::WentTo(place) => {
                arrival = Some(place);
                next = Game::behind(place).unwrap_or(boot);
            }
        }
    }
}

/// The laptop's speaker and microphone, each opened the first time an app
/// wants it and kept for the next.
#[derive(Default)]
struct Sound {
    speaker: Option<Speaker>,
    microphone: Option<Microphone>,
    microphone_tried: bool,
}

/// One app, like one boot of the board: runs until the window closes, the
/// app asks to sleep, or it exits (the town at a door, or a game going back
/// out). `launched` games offer OUT.
fn run(
    game: Game,
    launched: bool,
    arrival: Option<Place>,
    window: &mut Window,
    save: &mut SaveFile,
    sound: &mut Sound,
    pixels: &mut [u32],
) -> Ended {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| d.subsec_nanos());
    let mut app = App::new();
    game.add_to(&mut app);
    if launched {
        app.insert_resource(Launcher);
    }
    if let (Game::Town, Some(place)) = (game, arrival) {
        app.insert_resource(Arrival(place));
    }
    app.insert_resource(Rng::new(seed))
        .insert_resource(SaveBlob::from_storage(save.load()));
    app.finish();
    app.cleanup();
    if let Some(mut power) = app.world_mut().get_resource_mut::<Power>() {
        power.battery = Some(Battery {
            percent: None,
            external_power: true,
            charging: false,
        });
    }
    if sound.speaker.is_none() && app.world().contains_resource::<SoundQueue>() {
        sound.speaker = Some(Speaker::open());
    }
    if !sound.microphone_tried && app.world().contains_resource::<Mic>() {
        sound.microphone_tried = true;
        sound.microphone = Microphone::open();
    }

    let mut panel = RamPanel::new();
    let mut presenter = Presenter::new(vec![0u8; BAND_BYTES], game.background());
    let mut stats = Stats::default();
    let mut device = VirtualDevice::upright();
    let mut last_frame = Instant::now();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let dt = last_frame.elapsed().as_secs_f32();
        last_frame = Instant::now();
        feed_inputs(window, app.world_mut(), &mut device, dt);
        if let Some(microphone) = sound.microphone.as_mut() {
            let mic = app.world_mut().get_resource_mut::<Mic>();
            microphone.drain(mic.map(Mut::into_inner));
        }
        app.update();
        if let Some(bytes) = app.world_mut().resource_mut::<SaveBlob>().take_pending() {
            save.save(&bytes);
        }
        if let Some(speaker) = sound.speaker.as_mut() {
            for command in app.world_mut().resource_mut::<SoundQueue>().drain() {
                speaker.apply(command);
            }
            speaker.pump();
        }

        let sent = presenter.present(&app.world().resource::<Frame>().shapes, &mut panel);
        if let Some(title) = stats.record(sent) {
            window.set_title(&title);
        }
        panel.to_argb(pixels);
        let power = app
            .world()
            .get_resource::<Power>()
            .copied()
            .unwrap_or_default();
        if power.idle {
            for px in pixels.iter_mut() {
                *px = (*px >> 2) & 0x003F_3F3F;
            }
        }
        window.set_target_fps(if power.idle { 20 } else { 60 });
        window
            .update_with_buffer(pixels, SIZE, SIZE)
            .expect("present window");
        if power.sleep_requested() {
            return Ended::Slept;
        }
        if app.should_exit().is_some() {
            if let Some(place) = app
                .world()
                .get_resource::<Town>()
                .and_then(Town::destination)
            {
                return Ended::WentTo(place);
            }
            return Ended::Exited;
        }
    }
    Ended::Closed
}

/// Keeps the last frame up until Z (BOOT) is released, then a dark panel
/// until it is pressed again; false if the window closes instead.
fn sleep_until_woken(window: &mut Window, last_frame: &[u32]) -> bool {
    let dark = vec![0u32; SIZE * SIZE];
    let mut released = false;
    window.set_target_fps(20);
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let held = window.is_key_down(Key::Z);
        if released && held {
            window.set_target_fps(60);
            return true;
        }
        released |= !held;
        let shown = if released { &dark } else { last_frame };
        window
            .update_with_buffer(shown, SIZE, SIZE)
            .expect("present window");
    }
    false
}

/// Mouse as the finger, keys as the buttons and the device's motion. Button
/// and IMU resources only exist for games that use them.
fn feed_inputs(window: &Window, world: &mut World, device: &mut VirtualDevice, dt: f32) {
    let finger = window
        .get_mouse_down(MouseButton::Left)
        .then(|| window.get_mouse_pos(MouseMode::Discard))
        .flatten()
        .map(|(x, y)| Vec2::new(x, y));
    world.resource_mut::<Touch>().set(finger);

    if let Some(mut buttons) = world.get_resource_mut::<Buttons>() {
        buttons.action.set(window.is_key_down(Key::Space));
        buttons.zero.set(window.is_key_down(Key::Z));
    }
    if let Some(mut feed) = world.get_resource_mut::<ImuFeed>() {
        if window.is_key_down(Key::C) {
            device.straighten();
        }
        let axis = |plus: Key, minus: Key| {
            f32::from(u8::from(window.is_key_down(plus)))
                - f32::from(u8::from(window.is_key_down(minus)))
        };
        let steer = axis(Key::Right, Key::Left) * TURN_DEG_PER_SEC;
        let lean = axis(Key::Up, Key::Down);
        feed.push(device.sample(steer, lean, dt));
    }
}

/// Once a second: frame rate, average bytes pushed per frame, and how long
/// that would occupy the real 40 MHz QSPI link.
struct Stats {
    since: Instant,
    frames: u32,
    bytes: usize,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            since: Instant::now(),
            frames: 0,
            bytes: 0,
        }
    }
}

impl Stats {
    fn record(&mut self, sent: usize) -> Option<String> {
        self.frames += 1;
        self.bytes += sent;
        let secs = self.since.elapsed().as_secs_f64();
        if secs < 1.0 {
            return None;
        }
        let per_frame = self.bytes as f64 / self.frames as f64;
        let title = format!(
            "Orbit board | {:.0} fps | {:.1} KB/frame | ~{:.1} ms/frame on QSPI",
            self.frames as f64 / secs,
            per_frame / 1024.0,
            per_frame / QSPI_BYTES_PER_SEC * 1000.0,
        );
        *self = Self::default();
        Some(title)
    }
}
