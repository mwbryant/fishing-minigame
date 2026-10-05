//! Desktop stand-in for the ESP32-S3-Touch-AMOLED-1.75: the same Bevy games,
//! the same dirty-region/band presenter, pixels land in a RAM model of the
//! CO5300, and the IMU is a virtual device driven from the keyboard.
//!
//! ```text
//! cargo run -p orbit_desktop -- [angler|orbit]                 play (default angler)
//! cargo run -p orbit_desktop -- [angler|orbit] snapshot <dir>  scripted PNGs, headless
//! ```

mod device;
mod panel;
mod snapshots;

use std::{
    env,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use angler_game::AnglerPlugin;
use bevy::prelude::*;
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use orbit_core::{Buttons, Frame, ImuFeed, Rng, SaveBlob, Touch};
use orbit_game::OrbitPlugin;
use orbit_gfx::Presenter;

use crate::{
    device::{SaveFile, VirtualDevice},
    panel::{RamPanel, SIZE},
};

/// Matches the firmware's internal-RAM band buffer.
pub const BAND_BYTES: usize = SIZE * 2 * 8;
/// What the real link moves: 40 MHz QSPI clock, four data lines.
const QSPI_BYTES_PER_SEC: f64 = 40e6 * 4.0 / 8.0;
/// How fast ←/→ turn the virtual device.
const TURN_DEG_PER_SEC: f32 = 60.0;

#[derive(Clone, Copy)]
pub enum Game {
    Orbit,
    Angler,
}

impl Game {
    fn name(self) -> &'static str {
        match self {
            Game::Orbit => "orbit",
            Game::Angler => "angler",
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let (game, rest) = match args.first().map(String::as_str) {
        Some("orbit") => (Game::Orbit, &args[1..]),
        Some("angler") => (Game::Angler, &args[1..]),
        _ => (Game::Angler, &args[..]),
    };
    match rest.first().map(String::as_str) {
        Some("snapshot") => snapshots::run(
            game,
            Path::new(rest.get(1).map_or("screenshots", String::as_str)),
        ),
        _ => play(game),
    }
}

fn play(game: Game) {
    println!(
        "{}: mouse = finger | Space = action (PWR) | Z = zero (BOOT) | \
         left/right = turn the device | hold up/down = lean | C = straighten | Esc = quit",
        game.name()
    );
    let mut window =
        Window::new("Orbit board", SIZE, SIZE, WindowOptions::default()).expect("open window");
    window.set_target_fps(60);

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| d.subsec_nanos());
    let mut save = SaveFile::open(game.name());
    let mut app = App::new();
    match game {
        Game::Orbit => app.add_plugins(OrbitPlugin),
        Game::Angler => app.add_plugins(AnglerPlugin),
    };
    app.insert_resource(Rng::new(seed))
        .insert_resource(SaveBlob::from_storage(save.load()));
    app.finish();
    app.cleanup();

    let mut panel = RamPanel::new();
    let mut presenter = Presenter::new(vec![0u8; BAND_BYTES], game.background());
    let mut pixels = vec![0u32; SIZE * SIZE];
    let mut stats = Stats::default();
    let mut device = VirtualDevice::upright();
    let mut last_frame = Instant::now();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let dt = last_frame.elapsed().as_secs_f32();
        last_frame = Instant::now();
        feed_inputs(&window, app.world_mut(), &mut device, dt);
        app.update();
        if let Some(bytes) = app.world_mut().resource_mut::<SaveBlob>().take_pending() {
            save.save(&bytes);
        }

        let sent = presenter.present(&app.world().resource::<Frame>().shapes, &mut panel);
        if let Some(title) = stats.record(sent) {
            window.set_title(&title);
        }
        panel.to_argb(&mut pixels);
        window
            .update_with_buffer(&pixels, SIZE, SIZE)
            .expect("present window");
    }
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
