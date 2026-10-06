use alloc::{format, vec::Vec};

use bevy::prelude::*;
use orbit_core::{Frame, Launcher, Power, Tilt};
use orbit_gfx::{Point, SCREEN_CENTER, Shape, Sprite, SpriteRef};
use orbit_ui::{
    DANGER, GOLD, HINT, TEXT, battery_level, button, dial_angle, dial_choice, dial_point, label,
    power_overlay, rect, rgb, running_low,
};

use crate::{EGGS, Phase, Table, sprites, ui};

const WEB: orbit_gfx::Rgb565 = rgb(58, 58, 70);
const FLASH: orbit_gfx::Rgb565 = rgb(170, 10, 10);
const BLOOD: orbit_gfx::Rgb565 = rgb(70, 0, 0);

pub(crate) fn draw_frame(
    table: Res<Table>,
    tilt: Res<Tilt>,
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
            shapes.push(label(170, "SPIDER", 3, TEXT));
            shapes.push(label(228, "ROULETTE", 3, TEXT));
            shapes.push(label(282, "ONE EGG HIDES A SPIDER", 1, HINT));
            shapes.push(label(306, "TURN TO AIM, PWR TO CRACK", 1, HINT));
            shapes.push(label(350, "PRESS PWR", 1, GOLD));
            shapes.push(label(390, &format!("BEST {}", table.best), 1, TEXT));
        }
        Phase::Pick => {
            web(shapes);
            let aimed = dial_choice(tilt.steer_deg, EGGS);
            for egg in 0..EGGS {
                eggs(shapes, &table, egg, egg == aimed);
            }
            shapes.push(Shape::Arc {
                center: SCREEN_CENTER,
                radius: 214,
                width: 8,
                start_deg: dial_angle(tilt.steer_deg) - 90.0 - 10.0,
                sweep_deg: 20.0,
                color: GOLD,
            });
            shapes.push(label(222, &format!("1 IN {}", table.whole()), 2, TEXT));
            shapes.push(label(256, &format!("SAFE {}", table.safe), 1, HINT));
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
        let deg = (spoke as f32 + 0.5) * 360.0 / EGGS as f32;
        shapes.push(Shape::Line {
            from: dial_point(deg, 30.0),
            to: dial_point(deg, 205.0),
            width: 1,
            color: WEB,
        });
    }
}

fn eggs(shapes: &mut Vec<Shape>, table: &Table, egg: usize, aimed: bool) {
    let center = ui::egg_center(egg);
    let center = Point::new(center.x as i32, center.y as i32);
    if aimed {
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
    centered(shapes, sprite, center, 3);
}

/// The spider leaps at the glass in three bounds over a red flash.
fn pounce(shapes: &mut Vec<Shape>, t: f32) {
    shapes.push(rect(0, 0, 466, 466, if t < 0.6 { FLASH } else { BLOOD }));
    let scale = match t {
        t if t < 0.12 => 6,
        t if t < 0.24 => 12,
        _ => 20,
    };
    centered(shapes, &sprites::SPIDER, SCREEN_CENTER, scale);
}

fn result(shapes: &mut Vec<Shape>, table: &Table) {
    shapes.push(label(240, &format!("SAFE PICKS {}", table.safe), 2, TEXT));
    if table.new_best {
        shapes.push(label(284, "NEW BEST!", 2, GOLD));
    } else {
        shapes.push(label(284, &format!("BEST {}", table.best), 1, HINT));
    }
    if table.phase_secs >= crate::RESULT_GUARD_SECS {
        shapes.push(label(
            340,
            "PRESS PWR",
            1,
            if table.phase == Phase::Won {
                GOLD
            } else {
                DANGER
            },
        ));
    }
}

fn centered(shapes: &mut Vec<Shape>, sprite: &'static Sprite, center: Point, scale: u32) {
    let size = sprite.size() * scale;
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(sprite),
        top_left: center - Point::new(size.width as i32 / 2, size.height as i32 / 2),
        scale,
        flip_x: false,
    });
}
