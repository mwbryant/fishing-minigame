//! What every game on the round board draws the same way: text and touch
//! buttons, the battery level, the hold-to-sleep hint and goodbye, the dial
//! that turning the device sweeps around the glass, and typing by turning.

#![no_std]

extern crate alloc;

mod dial;
mod keys;

use alloc::{format, string::String, vec::Vec};

use bevy::math::Vec2;
use orbit_core::{Battery, Clip, Power};
use orbit_gfx::{Point, Rectangle, Rgb565, SCREEN_CENTER, Shape, Size, Sprite, SpriteRef};

pub use dial::{
    DIAL_GAIN, LIST_STEP_DEG, dial_angle, dial_choice, dial_point, dial_pointer, list_choice,
};
pub use keys::{KEY_RING, KEYS, Key, KeyRing, key_center, key_ring, key_turn};

/// An 8-bit-per-channel color, truncated to RGB565.
pub const fn rgb(r: u8, g: u8, b: u8) -> Rgb565 {
    Rgb565::new(r >> 3, g >> 2, b >> 3)
}

pub const BLACK: Rgb565 = rgb(0, 0, 0);
pub const TEXT: Rgb565 = rgb(255, 255, 255);
pub const HINT: Rgb565 = rgb(200, 215, 230);
pub const DEEP: Rgb565 = rgb(12, 34, 58);
pub const GOLD: Rgb565 = rgb(255, 215, 70);
pub const TOAST: Rgb565 = rgb(120, 230, 255);
pub const DANGER: Rgb565 = rgb(240, 80, 60);
const BATTERY_OK: Rgb565 = rgb(110, 220, 90);
const BATTERY_LOW: Rgb565 = rgb(250, 210, 60);
/// At or below this, games show the battery during play too (unless charging).
pub const LOW_BATTERY_PERCENT: u8 = 15;

/// How far outside its box a tap still presses a button (about 1.5 mm of
/// glass), so a fingertip that lands just off one still counts.
pub const TAP_SLOP: f32 = 16.0;

/// An on-screen touch button. Its rectangle is shared by drawing and hit
/// testing, so what you see is what you tap, give or take [`TAP_SLOP`].
#[derive(Debug, PartialEq, Eq)]
pub struct Button {
    pub area: Rectangle,
    pub text: &'static str,
}

impl Button {
    pub const fn new(x: i32, y: i32, width: u32, height: u32, text: &'static str) -> Self {
        Self {
            area: Rectangle::new(Point::new(x, y), Size::new(width, height)),
            text,
        }
    }

    pub fn center(&self) -> Vec2 {
        let c = self.area.center();
        Vec2::new(c.x as f32, c.y as f32)
    }

    /// How far `at` is from the button's box: 0 on it.
    pub fn miss(&self, at: Vec2) -> f32 {
        let low = Vec2::new(self.area.top_left.x as f32, self.area.top_left.y as f32);
        let size = Vec2::new(self.area.size.width as f32, self.area.size.height as f32);
        let high = low + size - Vec2::ONE;
        at.clamp(low, high).distance(at)
    }

    /// A tap on the button or within [`TAP_SLOP`] of it. Where buttons sit
    /// closer together than that, pick between them with [`tapped`].
    pub fn hit(&self, at: Vec2) -> bool {
        self.miss(at) <= TAP_SLOP
    }
}

/// Which of `buttons` (the ones on screen together) a tap at `at` meant: the
/// one it lands on, or else the nearest within [`TAP_SLOP`].
pub fn tapped<'a>(at: Vec2, buttons: impl IntoIterator<Item = &'a Button>) -> Option<&'a Button> {
    buttons
        .into_iter()
        .map(|button| (button, button.miss(at)))
        .filter(|(_, miss)| *miss <= TAP_SLOP)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(button, _)| button)
}

/// Which of `spots` (dial choices, keys, fish) a tap at `at` meant: the
/// nearest, if it's within `reach`.
pub fn nearest(at: Vec2, spots: impl IntoIterator<Item = Vec2>, reach: f32) -> Option<usize> {
    spots
        .into_iter()
        .map(|spot| spot.distance(at))
        .enumerate()
        .filter(|(_, distance)| *distance <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

pub fn button(shapes: &mut Vec<Shape>, button: &Button) {
    framed_button(shapes, button, HINT);
}

pub fn framed_button(shapes: &mut Vec<Shape>, button: &Button, frame: Rgb565) {
    labelled_button(shapes, button, button.text, frame);
}

/// A button showing `text` instead of its own, e.g. a toggle's state.
pub fn labelled_button(shapes: &mut Vec<Shape>, button: &Button, text: &str, frame: Rgb565) {
    let area = button.area;
    shapes.push(Shape::Rect { area, color: frame });
    shapes.push(Shape::Rect {
        area: area.offset(-2),
        color: DEEP,
    });
    shapes.push(label_at(area.center(), text, 1, TEXT));
}

/// A tap or press that did something (from Angler's sound set).
pub static CLICK: Clip = Clip {
    pcm: include_bytes!("../assets/click.pcm"),
};
/// A tap or press that can't do anything.
pub static THUD: Clip = Clip {
    pcm: include_bytes!("../assets/thud.pcm"),
};

/// Text centered across the screen at height `y`.
pub fn label(y: i32, text: &str, scale: u32, color: Rgb565) -> Shape {
    label_at(Point::new(SCREEN_CENTER.x, y), text, scale, color)
}

pub fn label_at(center: Point, text: &str, scale: u32, color: Rgb565) -> Shape {
    Shape::Label {
        center,
        text: text.into(),
        scale,
        color,
    }
}

/// How far a plate reaches past the letters it backs.
const PLATE_PAD: i32 = 3;

/// Pushes `label` on a plate of `backing` a little bigger than its letters,
/// so scenery behind it (a path, a web, stars) stops short of the text.
pub fn plated(shapes: &mut Vec<Shape>, label: Shape, backing: Rgb565) {
    if let Some(ink) = orbit_gfx::ink(&label) {
        shapes.push(Shape::Rect {
            area: ink.offset(PLATE_PAD),
            color: backing,
        });
    }
    shapes.push(label);
}

/// Pushes `lines` on one plate of `backing` round them all, so nothing
/// behind shows in the gaps between them either.
pub fn plated_lines(
    shapes: &mut Vec<Shape>,
    lines: impl IntoIterator<Item = Shape>,
    backing: Rgb565,
) {
    let lines: Vec<Shape> = lines.into_iter().collect();
    let inks = lines.iter().filter_map(orbit_gfx::ink);
    let around = |a: Rectangle, b: Rectangle| {
        let (Some(a_end), Some(b_end)) = (a.bottom_right(), b.bottom_right()) else {
            return a;
        };
        Rectangle::with_corners(
            Point::new(
                a.top_left.x.min(b.top_left.x),
                a.top_left.y.min(b.top_left.y),
            ),
            Point::new(a_end.x.max(b_end.x), a_end.y.max(b_end.y)),
        )
    };
    if let Some(area) = inks.reduce(around) {
        shapes.push(Shape::Rect {
            area: area.offset(LINES_PLATE_PAD),
            color: backing,
        });
    }
    shapes.extend(lines);
}

/// Pushes `label` in a box framed in `frame`, e.g. the pointed name on a
/// dial, sized to its letters however long the name is.
pub fn framed_label(shapes: &mut Vec<Shape>, label: Shape, frame: Rgb565, fill: Rgb565) {
    if let Some(ink) = orbit_gfx::ink(&label) {
        shapes.push(Shape::Rect {
            area: ink.offset(FRAME_PAD + 2),
            color: frame,
        });
        shapes.push(Shape::Rect {
            area: ink.offset(FRAME_PAD),
            color: fill,
        });
    }
    shapes.push(label);
}

/// How far a framed label's box reaches past its letters, inside the frame.
const FRAME_PAD: i32 = 5;

/// Drops the decorations (bubbles, stars, windows: whatever `decor` picks
/// out) that come within `margin` of any text in the frame, so scenery never
/// runs through it or peeks out from behind its plate. Call it once the
/// frame's text is all pushed.
pub fn clear_text_of(shapes: &mut Vec<Shape>, margin: i32, decor: impl Fn(&Shape) -> bool) {
    let inks: Vec<Rectangle> = shapes.iter().filter_map(orbit_gfx::ink).collect();
    shapes.retain(|shape| {
        !decor(shape)
            || inks.iter().all(|ink| {
                ink.offset(margin)
                    .intersection(&shape.bounds())
                    .is_zero_sized()
            })
    });
}

/// How far past their letters [`plated_lines`] plates reach.
pub const LINES_PLATE_PAD: i32 = PLATE_PAD * 2;

pub fn rect(x: i32, y: i32, w: u32, h: u32, color: Rgb565) -> Shape {
    Shape::Rect {
        area: Rectangle::new(Point::new(x, y), Size::new(w, h)),
        color,
    }
}

pub fn disc(x: i32, y: i32, radius: u32, color: Rgb565) -> Shape {
    Shape::Disc {
        center: Point::new(x, y),
        radius,
        color,
    }
}

/// A gold coin, shown beside gold amounts.
pub static COIN: Sprite = Sprite {
    rows: &[".KKK.", "KYYOK", "KYOOK", "KYOOK", ".KKK."],
    palette: &[
        (b'K', rgb(120, 80, 10)),
        (b'Y', rgb(255, 230, 120)),
        (b'O', rgb(240, 180, 40)),
    ],
};

/// Gold-colored text with a coin in front.
pub fn gold_label(shapes: &mut Vec<Shape>, center: Point, text: &str, scale: u32) {
    gold_on(shapes, center, text, scale, None);
}

/// A gold amount with its text on a plate of `backing` (see [`plated`]).
pub fn gold_label_on(
    shapes: &mut Vec<Shape>,
    center: Point,
    text: &str,
    scale: u32,
    backing: Rgb565,
) {
    gold_on(shapes, center, text, scale, Some(backing));
}

fn gold_on(
    shapes: &mut Vec<Shape>,
    center: Point,
    text: &str,
    scale: u32,
    backing: Option<Rgb565>,
) {
    let text = label_at(center, text, scale, GOLD);
    let coin_scale = scale * 2;
    let coin = COIN.size() * coin_scale;
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(&COIN),
        top_left: Point::new(
            text.bounds().top_left.x - coin.width as i32 - 4 * scale as i32,
            center.y - coin.height as i32 / 2,
        ),
        scale: coin_scale,
        flip_x: false,
    });
    match backing {
        Some(backing) => plated(shapes, text, backing),
        None => shapes.push(text),
    }
}

/// Charging, beside the battery level.
pub static BOLT: Sprite = Sprite {
    rows: &["...YY", "..YY.", ".YYYY", "YYYY.", ".YY..", "YY..."],
    palette: &[(b'Y', rgb(255, 220, 60))],
};

/// At or under [`LOW_BATTERY_PERCENT`] with nothing charging it.
pub fn running_low(battery: &Battery) -> bool {
    !battery.charging
        && !battery.external_power
        && battery
            .percent
            .is_some_and(|percent| percent <= LOW_BATTERY_PERCENT)
}

/// A battery outline filled to the charge, the percentage (or USB with no
/// cell), and a bolt while charging, at the top of the glass.
pub fn battery_level(shapes: &mut Vec<Shape>, battery: &Battery) {
    battery_on(shapes, battery, None);
}

/// The battery level on a plate of `backing`, for screens with scenery
/// (a path) running under the top of the glass.
pub fn battery_level_on(shapes: &mut Vec<Shape>, battery: &Battery, backing: Rgb565) {
    battery_on(shapes, battery, Some(backing));
}

fn battery_on(shapes: &mut Vec<Shape>, battery: &Battery, backing: Option<Rgb565>) {
    let (x, y) = (196, 17);
    shapes.push(rect(x, y, 24, 14, TEXT));
    shapes.push(rect(x + 2, y + 2, 20, 10, BLACK));
    shapes.push(rect(x + 24, y + 4, 3, 6, TEXT));
    if let Some(percent) = battery.percent {
        let fill = (20 * u32::from(percent.min(100)) / 100).max(1);
        let color = match percent {
            0..=LOW_BATTERY_PERCENT => DANGER,
            16..=40 => BATTERY_LOW,
            _ => BATTERY_OK,
        };
        shapes.push(rect(x + 2, y + 2, fill, 10, color));
    }
    if battery.charging {
        shapes.push(Shape::Sprite {
            sprite: SpriteRef(&BOLT),
            top_left: Point::new(x - 14, y + 1),
            scale: 2,
            flip_x: false,
        });
    }
    let text = match battery.percent {
        Some(percent) => format!("{percent}%"),
        None => String::from("USB"),
    };
    let text = label_at(Point::new(x + 54, y + 7), &text, 1, TEXT);
    match backing {
        Some(backing) => plated(shapes, text, backing),
        None => shapes.push(text),
    }
}

/// A message across the screen at height `y` on its own dark banner, over
/// whatever is there. Any text the banner would half cover gives way.
pub fn notice(shapes: &mut Vec<Shape>, y: i32, text: &str, color: Rgb565) {
    let message = label(y, text, 2, color);
    let Some(ink) = orbit_gfx::ink(&message) else {
        return;
    };
    let banner = Rectangle::with_center(
        Point::new(SCREEN_CENTER.x, y),
        Size::new(ink.size.width.max(NOTICE_WIDTH) + 24, NOTICE_HEIGHT),
    );
    shapes.retain(|shape| {
        orbit_gfx::ink(shape).is_none_or(|ink| ink.offset(2).intersection(&banner).is_zero_sized())
    });
    shapes.push(Shape::Rect {
        area: banner,
        color: DEEP,
    });
    shapes.push(message);
}

const NOTICE_WIDTH: u32 = 256;
const NOTICE_HEIGHT: u32 = 44;

/// Draw last. While the zero button is held toward sleep it adds a hint (text
/// it would half cover gives way) and a filling rim. Once the goodbye starts,
/// it replaces the whole frame.
pub fn power_overlay(shapes: &mut Vec<Shape>, power: &Power) {
    if power.saying_good_night() {
        shapes.clear();
        shapes.push(label(210, "GOOD NIGHT", 2, TEXT));
        shapes.push(label(256, "PRESS TO WAKE", 1, HINT));
        return;
    }
    let Some(progress) = power.sleep_progress() else {
        return;
    };
    notice(shapes, 233, "HOLD TO SLEEP", TOAST);
    shapes.push(Shape::Arc {
        center: SCREEN_CENTER,
        radius: 226,
        width: 6,
        start_deg: -90.0,
        sweep_deg: 360.0 * progress.max(0.01),
        color: TOAST,
    });
}
