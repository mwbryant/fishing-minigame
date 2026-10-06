use alloc::{format, vec::Vec};

use bevy::{math::ops, prelude::*};
use orbit_core::{Frame, Launcher, Power};
use orbit_gfx::{Point, Rgb565, Shape, Sprite, SpriteRef};
use orbit_ui::{
    DANGER, GOLD, HINT, TEXT, battery_level, button, disc, label, power_overlay, rect, rgb,
    running_low,
};

use crate::{GROUND_Y, Kind, Phase, RESULT_GUARD_SECS, Ride, SKATER_X, sprites, ui};

const SKY: [Rgb565; 4] = [
    rgb(12, 14, 34),
    rgb(20, 20, 48),
    rgb(32, 26, 62),
    rgb(48, 32, 74),
];
const CITY: Rgb565 = rgb(24, 22, 42);
const WINDOW: Rgb565 = rgb(240, 200, 90);
const MOON: Rgb565 = rgb(232, 230, 210);
const ASPHALT: Rgb565 = rgb(38, 38, 44);
const CURB: Rgb565 = rgb(96, 96, 108);
const DASH: Rgb565 = rgb(210, 180, 60);
const DECK: Rgb565 = rgb(220, 120, 60);
const WHEEL: Rgb565 = rgb(240, 240, 240);
const DASH_EVERY: f32 = 110.0;
const SKATER_SCALE: u32 = 3;

pub(crate) fn draw_frame(
    ride: Res<Ride>,
    power: Res<Power>,
    launcher: Option<Res<Launcher>>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    street(shapes, ride.distance);
    for obstacle in ride.obstacles() {
        let sprite = match obstacle.kind {
            Kind::Cone => &sprites::CONE,
            Kind::Bench => &sprites::BENCH,
        };
        let (width, height) = obstacle.kind.size();
        let scale = (width / sprite.size().width as f32) as u32;
        shapes.push(Shape::Sprite {
            sprite: SpriteRef(sprite),
            top_left: Point::new(obstacle.x as i32, (GROUND_Y - height) as i32),
            scale,
            flip_x: false,
        });
    }
    let menu = ride.phase != Phase::Riding;
    match ride.phase {
        Phase::Title => {
            skater(shapes, &ride);
            shapes.push(label(130, "SKATE", 4, TEXT));
            shapes.push(label(186, "TURN LEFT TO OLLIE", 1, HINT));
            shapes.push(label(208, "LEVEL OUT TO LAND", 1, HINT));
            shapes.push(label(400, "PRESS PWR", 1, GOLD));
            shapes.push(label(424, &format!("BEST {}", ride.best), 1, TEXT));
        }
        Phase::Riding => {
            skater(shapes, &ride);
            shapes.push(label(70, &format!("{}", ride.score), 2, TEXT));
            if let Some((text, _)) = ride.toast {
                shapes.push(label(116, text, 2, GOLD));
            }
        }
        Phase::Bailed => {
            centered(
                shapes,
                &sprites::FALLEN,
                Point::new(SKATER_X as i32 + 10, GROUND_Y as i32 - 7),
            );
            shapes.push(label(110, "BAILED!", 3, DANGER));
            shapes.push(label(168, &format!("SCORE {}", ride.score), 2, TEXT));
            if ride.new_best {
                shapes.push(label(208, "NEW BEST!", 2, GOLD));
            } else {
                shapes.push(label(208, &format!("BEST {}", ride.best), 1, HINT));
            }
            if ride.phase_secs >= RESULT_GUARD_SECS {
                shapes.push(label(400, "PRESS PWR", 1, GOLD));
            }
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

/// A dusk sky, a city skyline and the moon (static after the first frame),
/// then the street with lane dashes that scroll with the ride.
fn street(shapes: &mut Vec<Shape>, distance: f32) {
    let band = GROUND_Y as i32 / SKY.len() as i32 + 1;
    for (i, color) in SKY.iter().enumerate() {
        shapes.push(rect(0, i as i32 * band, 466, band as u32, *color));
    }
    shapes.push(disc(390, 120, 22, MOON));
    for (x, width, height) in [
        (10, 60, 120),
        (74, 44, 80),
        (122, 70, 150),
        (196, 50, 100),
        (250, 80, 170),
        (334, 46, 90),
        (384, 72, 130),
    ] {
        let top = GROUND_Y as i32 - height;
        shapes.push(rect(x, top, width as u32, height as u32, CITY));
        for row in 0..height / 30 {
            shapes.push(rect(x + 10, top + 12 + row * 30, 6, 8, WINDOW));
        }
    }
    shapes.push(rect(
        0,
        GROUND_Y as i32,
        466,
        466 - GROUND_Y as u32,
        ASPHALT,
    ));
    shapes.push(rect(0, GROUND_Y as i32, 466, 5, CURB));
    let offset = ops::rem_euclid(distance, DASH_EVERY);
    let mut x = -offset;
    while x < 466.0 {
        shapes.push(rect(x as i32, GROUND_Y as i32 + 48, 30, 4, DASH));
        x += DASH_EVERY;
    }
}

/// The rider stays upright; the board under the feet turns with the device.
fn skater(shapes: &mut Vec<Shape>, ride: &Ride) {
    let deck_y = GROUND_Y - ride.height - 5.0;
    let size = sprites::SKATER.size() * SKATER_SCALE;
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(&sprites::SKATER),
        top_left: Point::new(
            SKATER_X as i32 - size.width as i32 / 2,
            deck_y as i32 - size.height as i32 - 1,
        ),
        scale: SKATER_SCALE,
        flip_x: false,
    });
    let (sin, cos) = ops::sin_cos(ride.board_deg.to_radians());
    let along = |d: f32, down: f32| {
        Point::new(
            ops::round(SKATER_X + cos * d - sin * down) as i32,
            ops::round(deck_y + sin * d + cos * down) as i32,
        )
    };
    shapes.push(Shape::Line {
        from: along(-24.0, 0.0),
        to: along(24.0, 0.0),
        width: 4,
        color: DECK,
    });
    for d in [-15.0, 15.0] {
        let wheel = along(d, 5.0);
        shapes.push(disc(wheel.x, wheel.y, 3, WHEEL));
    }
}

fn centered(shapes: &mut Vec<Shape>, sprite: &'static Sprite, center: Point) {
    let size = sprite.size() * SKATER_SCALE;
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(sprite),
        top_left: center - Point::new(size.width as i32 / 2, size.height as i32 / 2),
        scale: SKATER_SCALE,
        flip_x: false,
    });
}
