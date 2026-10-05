//! Software rasterizer shared by the desktop simulator and the ESP32 firmware.
//!
//! The game describes each frame as a list of [`Shape`]s. A [`Canvas`] is a
//! rectangular window of RGB565 pixels stored big-endian (the byte order the
//! CO5300 panel expects over QSPI), so the firmware can DMA a rendered band
//! straight to the display while the simulator blits the same bytes to a window.

#![no_std]

extern crate alloc;

mod canvas;
mod dirty;
mod present;
mod shape;
mod sprite;

pub use canvas::Canvas;
pub use dirty::{FrameDiff, align_even, bands};
pub use embedded_graphics::{
    pixelcolor::{Rgb565, RgbColor},
    prelude::{Point, Size},
    primitives::Rectangle,
};
pub use present::{Panel, Presenter};
pub use shape::{Shape, render};
pub use sprite::{Sprite, SpriteRef};

pub const SCREEN_SIZE: u32 = 466;
pub const SCREEN_CENTER: Point = Point::new(SCREEN_SIZE as i32 / 2, SCREEN_SIZE as i32 / 2);

pub fn screen_rect() -> Rectangle {
    Rectangle::new(Point::zero(), Size::new_equal(SCREEN_SIZE))
}
