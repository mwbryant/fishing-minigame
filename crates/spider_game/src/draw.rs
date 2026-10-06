use alloc::{format, vec::Vec};

use bevy::{math::ops, prelude::*};
use orbit_core::{Frame, Launcher, Power};
use orbit_gfx::{Point, SCREEN_CENTER, Shape, Sprite, SpriteRef};
use orbit_ui::{
    DANGER, GOLD, HINT, TEXT, battery_level, button, dial_point, label, power_overlay, rect, rgb,
    running_low,
};

use crate::{EGG_DEG, EGGS, Phase, RESULT_GUARD_SECS, Table, sprites, ui};

const WEB: orbit_gfx::Rgb565 = rgb(58, 58, 70);
const FLASH: orbit_gfx::Rgb565 = rgb(170, 10, 10);
const BLOOD: orbit_gfx::Rgb565 = rgb(70, 0, 0);
const DANCER_SCALE: u32 = 5;
const POSE_SECS: f32 = 0.22;

pub(crate) fn draw_frame(
    table: Res<Table>,
    power: Res<Power>,
    launcher: Option<Res<Launcher>>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    let menu = matches!(table.phase, Phase::Title | Phase::Bitten | Phase::Won);
    match table.phase {
        Phase::Title => {
            web(shapes);
            shapes.push(label(100, "SPIDER", 3, TEXT));
            shapes.push(label(160, "ROULETTE", 3, TEXT));
            dancer(shapes, table.clock, Point::new(233, 262));
            shapes.push(label(330, "ONE EGG HIDES A SPIDER", 1, HINT));
            shapes.push(label(356, "PRESS PWR TO SPIN", 1, GOLD));
            shapes.push(label(390, &format!("BEST {}", table.best), 1, TEXT));
        }
        Phase::Ready | Phase::Spinning | Phase::Reveal => {
            web(shapes);
            for egg in 0..EGGS {
                eggs(shapes, &table, egg);
            }
            shapes.push(Shape::Arc {
                center: SCREEN_CENTER,
                radius: 214,
                width: 8,
                start_deg: table.pointer_deg - 90.0 - EGG_DEG / 3.0,
                sweep_deg: EGG_DEG * 2.0 / 3.0,
                color: GOLD,
            });
            dancer(shapes, table.clock, SCREEN_CENTER);
            let status = format!("SAFE {} | 1 IN {}", table.safe, table.whole());
            shapes.push(label(36, &status, 1, TEXT));
            if table.phase == Phase::Ready {
                shapes.push(label(432, "PRESS PWR TO SPIN", 1, GOLD));
            }
        }
        Phase::Pounce => pounce(shapes, table.phase_secs),
        Phase::Bitten => {
            shapes.push(rect(0, 0, 466, 466, BLOOD));
            shapes.push(label(150, "GOT YOU!", 3, TEXT));
            result(shapes, &table);
        }
        Phase::Won => {
            web(shapes);
            shapes.push(label(150, "SPIDER TRAPPED!", 2, GOLD));
            shapes.push(label(190, "EVERY SAFE EGG FOUND", 1, TEXT));
            result(shapes, &table);
        }
    }
    if menu && launcher.is_some() {
        button(shapes, &ui::HOME);
    }
    if let Some(battery) = power.battery
        && (menu || running_low(&battery))
    {
        battery_level(shapes, &battery);
    }
    power_overlay(shapes, &power);
}

/// Three rings and twelve spokes, static after the first frame.
fn web(shapes: &mut Vec<Shape>) {
    for radius in [60, 110, 190] {
        shapes.push(Shape::Ring {
            center: SCREEN_CENTER,
            radius,
            width: 1,
            color: WEB,
        });
    }
    for spoke in 0..EGGS {
        let deg = (spoke as f32 + 0.5) * EGG_DEG;
        shapes.push(Shape::Line {
            from: dial_point(deg, 30.0),
            to: dial_point(deg, 205.0),
            width: 1,
            color: WEB,
        });
    }
}

/// An egg (or its shell), ringed in gold while the pointer is on it.
fn eggs(shapes: &mut Vec<Shape>, table: &Table, egg: usize) {
    let center = ui::egg_center(egg);
    if egg == table.pointed() {
        shapes.push(Shape::Ring {
            center,
            radius: 24,
            width: 3,
            color: if table.cracked(egg) { HINT } else { GOLD },
        });
    }
    let sprite = if table.cracked(egg) {
        &sprites::SHELL
    } else {
        &sprites::EGG
    };
    centered(shapes, sprite, center, 3, false);
}

/// Swaps poses on the beat, bobs, sways, and turns round every few beats.
fn dancer(shapes: &mut Vec<Shape>, clock: f32, at: Point) {
    let beat = (clock / POSE_SECS) as usize;
    let bob = ops::round(ops::sin(clock * 7.0) * 5.0) as i32;
    let sway = ops::round(ops::sin(clock * 3.5) * 8.0) as i32;
    centered(
        shapes,
        &sprites::DANCER[beat % 2],
        at + Point::new(sway, bob),
        DANCER_SCALE,
        beat / 4 % 2 == 1,
    );
}

/// The dancer leaps at the glass in three bounds over a red flash.
fn pounce(shapes: &mut Vec<Shape>, t: f32) {
    shapes.push(rect(0, 0, 466, 466, if t < 0.6 { FLASH } else { BLOOD }));
    let scale = match t {
        t if t < 0.12 => 8,
        t if t < 0.24 => 14,
        _ => 22,
    };
    centered(shapes, &sprites::DANCER[0], SCREEN_CENTER, scale, false);
}

fn result(shapes: &mut Vec<Shape>, table: &Table) {
    shapes.push(label(240, &format!("SAFE PICKS {}", table.safe), 2, TEXT));
    if table.new_best {
        shapes.push(label(284, "NEW BEST!", 2, GOLD));
    } else {
        shapes.push(label(284, &format!("BEST {}", table.best), 1, HINT));
    }
    if table.phase_secs >= RESULT_GUARD_SECS {
        let color = if table.phase == Phase::Won {
            GOLD
        } else {
            DANGER
        };
        shapes.push(label(340, "PRESS PWR", 1, color));
    }
}

fn centered(
    shapes: &mut Vec<Shape>,
    sprite: &'static Sprite,
    center: Point,
    scale: u32,
    flip_x: bool,
) {
    let size = sprite.size() * scale;
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(sprite),
        top_left: center - Point::new(size.width as i32 / 2, size.height as i32 / 2),
        scale,
        flip_x,
    });
}
