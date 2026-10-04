//! Desktop stand-in for the ESP32-S3-Touch-AMOLED-1.75: same Bevy app, same
//! dirty-region/band presenter, pixels land in a RAM model of the CO5300.
//!
//! `cargo run -p orbit_desktop` plays it (hold the left mouse button = finger).
//! `cargo run -p orbit_desktop -- snapshot <dir>` writes scripted PNGs headlessly.

use std::{
    env,
    fs::{self, File},
    io::BufWriter,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use bevy::prelude::*;
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use orbit_game::{
    BACKGROUND, Frame, OrbitPlugin, Rng, Screen, Touch,
    arena::SCREEN_CENTER,
    testing::{headless_app, press, tap, tick},
};
use orbit_gfx::{Panel, Presenter, Rectangle, SCREEN_SIZE};

const SIZE: usize = SCREEN_SIZE as usize;
/// Matches the firmware's internal-RAM band buffer.
const BAND_BYTES: usize = SIZE * 2 * 32;
/// What the real link moves: 40 MHz QSPI clock, four data lines.
const QSPI_BYTES_PER_SEC: f64 = 40e6 * 4.0 / 8.0;
/// Shown outside the round glass so the simulator reads as a watch face.
const BEZEL: u32 = 0x1a1a1a;

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("snapshot") => snapshot(Path::new(args.get(2).map_or("screenshots", String::as_str))),
        _ => play(),
    }
}

fn play() {
    let mut window =
        Window::new("Orbit", SIZE, SIZE, WindowOptions::default()).expect("open window");
    window.set_target_fps(60);

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| d.subsec_nanos());
    let mut app = App::new();
    app.add_plugins(OrbitPlugin).insert_resource(Rng::new(seed));
    app.finish();
    app.cleanup();

    let mut panel = RamPanel::new();
    let mut presenter = Presenter::new(vec![0u8; BAND_BYTES], BACKGROUND);
    let mut pixels = vec![0u32; SIZE * SIZE];
    let mut stats = Stats::default();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let finger = window
            .get_mouse_down(MouseButton::Left)
            .then(|| window.get_mouse_pos(MouseMode::Discard))
            .flatten()
            .map(|(x, y)| Vec2::new(x, y));
        app.world_mut().resource_mut::<Touch>().set(finger);
        app.update();

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

/// Plays a scripted round headlessly and saves one PNG per screen.
fn snapshot(dir: &Path) {
    fs::create_dir_all(dir).expect("create snapshot dir");
    let mut app = headless_app();
    let mut panel = RamPanel::new();
    let mut presenter = Presenter::new(vec![0u8; BAND_BYTES], BACKGROUND);
    let mut shoot = |app: &mut App, name: &str| {
        presenter.present(&app.world().resource::<Frame>().shapes, &mut panel);
        let path = dir.join(format!("{name}.png"));
        panel.save_png(&path);
        println!("wrote {}", path.display());
    };

    shoot(&mut app, "title");
    tap(&mut app, SCREEN_CENTER);
    press(&mut app, Some(SCREEN_CENTER + Vec2::new(-60.0, 90.0)));
    tick(&mut app, 70);
    shoot(&mut app, "playing");
    press(&mut app, None);
    while *app.world().resource::<State<Screen>>().get() != Screen::GameOver {
        tick(&mut app, 1);
    }
    tick(&mut app, 30);
    shoot(&mut app, "game_over");
}

/// RAM model of the CO5300's frame memory.
struct RamPanel {
    ram: Vec<u8>,
}

impl RamPanel {
    fn new() -> Self {
        Self {
            ram: vec![0; SIZE * SIZE * 2],
        }
    }

    fn rgb565(&self, x: usize, y: usize) -> u16 {
        let i = (y * SIZE + x) * 2;
        u16::from_be_bytes([self.ram[i], self.ram[i + 1]])
    }

    fn to_argb(&self, out: &mut [u32]) {
        for (i, px) in out.iter_mut().enumerate() {
            let (x, y) = (i % SIZE, i / SIZE);
            *px = if on_glass(x, y) {
                expand(self.rgb565(x, y))
            } else {
                BEZEL
            };
        }
    }

    fn save_png(&self, path: &Path) {
        let mut argb = vec![0u32; SIZE * SIZE];
        self.to_argb(&mut argb);
        let rgb: Vec<u8> = argb
            .iter()
            .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
            .collect();
        let file = BufWriter::new(File::create(path).expect("create png"));
        let mut encoder = png::Encoder::new(file, SCREEN_SIZE, SCREEN_SIZE);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("png header");
        writer.write_image_data(&rgb).expect("png data");
    }
}

impl Panel for RamPanel {
    fn write(&mut self, area: Rectangle, bytes: &[u8]) {
        let row_bytes = area.size.width as usize * 2;
        for (row, src) in bytes.chunks_exact(row_bytes).enumerate() {
            let y = area.top_left.y as usize + row;
            let start = (y * SIZE + area.top_left.x as usize) * 2;
            self.ram[start..start + row_bytes].copy_from_slice(src);
        }
    }
}

fn on_glass(x: usize, y: usize) -> bool {
    let r = SIZE as f32 / 2.0;
    let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
    dx * dx + dy * dy <= r * r
}

fn expand(c: u16) -> u32 {
    let r = ((c >> 11) & 0x1f) as u32;
    let g = ((c >> 5) & 0x3f) as u32;
    let b = (c & 0x1f) as u32;
    (r << 3 | r >> 2) << 16 | (g << 2 | g >> 4) << 8 | (b << 3 | b >> 2)
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
            "Orbit | {:.0} fps | {:.1} KB/frame | ~{:.1} ms/frame on QSPI",
            self.frames as f64 / secs,
            per_frame / 1024.0,
            per_frame / QSPI_BYTES_PER_SEC * 1000.0,
        );
        *self = Self::default();
        Some(title)
    }
}
