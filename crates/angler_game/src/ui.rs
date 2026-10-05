//! On-screen touch buttons. Their rectangles are shared by drawing and hit
//! testing so what you see is what you tap.

use bevy::math::Vec2;
use orbit_gfx::{Point, Rectangle, Size};

pub struct Button {
    pub area: Rectangle,
    pub text: &'static str,
}

impl Button {
    const fn new(x: i32, y: i32, width: u32, height: u32, text: &'static str) -> Self {
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

pub const LOG: Button = Button::new(73, 352, 100, 36, "LOG");
pub const SHOP: Button = Button::new(183, 352, 100, 36, "SHOP");
pub const SETTINGS: Button = Button::new(293, 352, 100, 36, "SETTINGS");
/// On the aiming screen until the charge starts: back to the title.
pub const MENU: Button = Button::new(283, 366, 90, 34, "MENU");
pub const EASIER: Button = Button::new(70, 182, 56, 44, "<");
pub const HARDER: Button = Button::new(340, 182, 56, 44, ">");
pub const GUTEN: Button = Button::new(103, 296, 120, 36, "GUTEN");
pub const RESET: Button = Button::new(243, 296, 120, 36, "RESET");
pub const ERASE: Button = Button::new(103, 296, 120, 36, "ERASE");
/// RESET's spot, so a double tap on RESET keeps everything.
pub const KEEP: Button = Button {
    text: "KEEP",
    ..RESET
};
pub const BACK: Button = Button::new(173, 352, 120, 36, "BACK");
pub const PREVIOUS_HAT: Button = Button::new(70, 150, 56, 44, "<");
pub const NEXT_HAT: Button = Button::new(340, 150, 56, 44, ">");
pub const BUY: Button = Button::new(173, 302, 120, 36, "BUY");
/// BUY's spot, for a hat already owned.
pub const WEAR: Button = Button {
    text: "WEAR",
    ..BUY
};
