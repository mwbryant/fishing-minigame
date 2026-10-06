//! What every game on the round board draws the same way: text and touch
//! buttons, the battery level, the hold-to-sleep hint and goodbye, and the
//! dial that turning the device sweeps around the glass.

#![no_std]

extern crate alloc;

mod dial;

use alloc::{format, string::String, vec::Vec};

use bevy::math::Vec2;
use orbit_core::{Battery, Power};
use orbit_gfx::{Point, Rectangle, Rgb565, SCREEN_CENTER, Shape, Size, Sprite, SpriteRef};

pub use dial::{DIAL_GAIN, dial_angle, dial_choice, dial_point, dial_pointer};

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

/// An on-screen touch button. Its rectangle is shared by drawing and hit
/// testing, so what you see is what you tap.
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

    pub fn hit(&self, at: Vec2) -> bool {
        self.area.contains(Point::new(at.x as i32, at.y as i32))
    }
}

pub fn button(shapes: &mut Vec<Shape>, button: &Button) {
    framed_button(shapes, button, HINT);
}

pub fn framed_button(shapes: &mut Vec<Shape>, button: &Button, frame: Rgb565) {
    let area = button.area;
    shapes.push(Shape::Rect { area, color: frame });
    shapes.push(Shape::Rect {
        area: area.offset(-2),
        color: DEEP,
    });
    shapes.push(label_at(area.center(), button.text, 1, TEXT));
}

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

/// Charging, beside the battery level.
pub static BOLT: Sprite = Sprite {
    rows: &["...YY", "..YY.", ".YYYY", "YYYY.", ".YY..", "YY..."],
    palette: &[(b'Y', rgb(255, 220, 60))],
};

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
    shapes.push(label_at(Point::new(x + 54, y + 7), &text, 1, TEXT));
}

/// Draw last: while the zero button is held toward sleep, a hint and the rim
/// filling; once the goodbye starts, it replaces the whole frame.
pub fn power_overlay(shapes: &mut Vec<Shape>, power: &Power) {
    if power.saying_good_night() {
        shapes.clear();
        shapes.push(label(210, "GOOD NIGHT", 2, TEXT));
        shapes.push(label(256, "PRESS BOOT TO WAKE", 1, HINT));
        return;
    }
    let Some(progress) = power.sleep_progress() else {
        return;
    };
    shapes.push(rect(93, 211, 280, 44, DEEP));
    shapes.push(label(233, "HOLD TO SLEEP", 2, TOAST));
    shapes.push(Shape::Arc {
        center: SCREEN_CENTER,
        radius: 226,
        width: 6,
        start_deg: -90.0,
        sweep_deg: 360.0 * progress.max(0.01),
        color: TOAST,
    });
}
