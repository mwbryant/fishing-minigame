use alloc::{format, vec::Vec};

use bevy::{math::ops, prelude::*};
use orbit_core::{Frame, Launcher, Power};
use orbit_gfx::{Point, Rgb565, Shape, Sprite, SpriteRef};
use orbit_ui::{
    DANGER, GOLD, HINT, TEXT, battery_level, button, disc, label, power_overlay, rect, rgb,
    running_low,
};

use crate::{
    GROUND_Y, HALFPIPE_SECS, Halfpipe, Kind, PIPE_CENTER, PIPE_RADIUS, Phase, RESULT_GUARD_SECS,
    Ride, SKATER_X, TRICKS, Trick, sprites, ui,
};

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
const NIGHT: Rgb565 = rgb(12, 14, 34);
const CONCRETE: Rgb565 = rgb(92, 94, 106);
const SURFACE: Rgb565 = rgb(170, 172, 184);
const METAL: Rgb565 = rgb(220, 224, 232);
const HELMET: Rgb565 = rgb(60, 170, 230);
const SHIRT: Rgb565 = rgb(230, 70, 60);
const PANTS: Rgb565 = rgb(60, 70, 110);
const SKIN: Rgb565 = rgb(236, 190, 150);
const SKATER_SCALE: u32 = 3;

pub(crate) fn draw_frame(
    ride: Res<Ride>,
    power: Res<Power>,
    launcher: Option<Res<Launcher>>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    if let Some(pipe) = ride.halfpipe {
        halfpipe(shapes, &ride, &pipe);
        power_overlay(shapes, &power);
        return;
    }
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
            shapes.push(label(400, "PRESS TO PLAY", 1, GOLD));
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
                shapes.push(label(400, "PRESS TO PLAY AGAIN", 1, GOLD));
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

/// The freestyle section: a halfpipe carved into concrete, the skater riding
/// it, spinning in the air, and the tricks landed.
fn halfpipe(shapes: &mut Vec<Shape>, ride: &Ride, pipe: &Halfpipe) {
    let (cx, cy) = (PIPE_CENTER.x as i32, PIPE_CENTER.y as i32);
    let radius = PIPE_RADIUS as u32;
    shapes.push(rect(0, 0, 466, 466, NIGHT));
    shapes.push(rect(0, cy, 466, 466 - cy as u32, CONCRETE));
    shapes.push(disc(cx, cy, radius, NIGHT));
    shapes.push(Shape::Arc {
        center: Point::new(cx, cy),
        radius,
        width: 4,
        start_deg: 0.0,
        sweep_deg: 180.0,
        color: SURFACE,
    });
    shapes.push(rect(0, cy - 6, (cx - radius as i32) as u32, 6, SURFACE));
    shapes.push(rect(cx + radius as i32, cy - 6, 466, 6, SURFACE));
    for x in [cx - radius as i32, cx + radius as i32] {
        shapes.push(disc(x, cy - 2, 4, METAL));
    }
    for i in 0..14u32 {
        let x = (i * 37 + 11) % 100 + 60;
        let x = if i % 2 == 0 { x } else { 466 - x };
        let y = (i * 97 + 61) % 150 + 40;
        shapes.push(rect(x as i32, y as i32, 2, 2, HINT));
    }

    let ride_on = PIPE_RADIUS - 6.0;
    let (base, angle) = match pipe.air {
        Some(air) => {
            let angle = -air.side * 90.0 + air.spin_deg;
            let hips = Vec2::new(
                PIPE_CENTER.x + air.side * (ride_on - HIPS),
                PIPE_CENTER.y - air.height,
            );
            (hips - up(angle) * HIPS, angle)
        }
        None => {
            let (sin, cos) = ops::sin_cos(pipe.angle);
            (
                PIPE_CENTER + Vec2::new(sin, cos) * ride_on,
                -pipe.angle.to_degrees(),
            )
        }
    };
    figure(shapes, base, angle);

    shapes.push(label(70, &format!("{}", ride.score), 2, TEXT));
    if let Some((text, _)) = ride.toast {
        shapes.push(label(116, text, 2, GOLD));
    }
    match ride.trick {
        Some((Trick::Landed(trick), _)) => {
            let (name, points) = TRICKS[trick];
            shapes.push(label(150, &format!("{name} +{points}"), 2, GOLD));
        }
        Some((Trick::Slam, _)) => shapes.push(label(150, "SLAM!", 2, DANGER)),
        None => {}
    }
    let left = (HALFPIPE_SECS - pipe.secs).max(0.0);
    shapes.push(label(
        186,
        &format!("FREESTYLE {}", ops::ceil(left) as u32),
        1,
        HINT,
    ));
}

/// How far above the board the figure's hips are; airs spin about them.
const HIPS: f32 = 24.0;

/// The skater's "up" turned `angle_deg` clockwise.
fn up(angle_deg: f32) -> Vec2 {
    let (sin, cos) = ops::sin_cos(angle_deg.to_radians());
    Vec2::new(sin, -cos)
}

/// The skater as lines and discs, so it can spin to any angle: `base` is the
/// middle of the board, `angle_deg` turns the skater's "up" clockwise.
fn figure(shapes: &mut Vec<Shape>, base: Vec2, angle_deg: f32) {
    let up = up(angle_deg);
    let across = Vec2::new(-up.y, up.x);
    let at = |along: f32, height: f32| {
        let p = base + across * along + up * height;
        Point::new(ops::round(p.x) as i32, ops::round(p.y) as i32)
    };
    let line = |from, to, width, color| Shape::Line {
        from,
        to,
        width,
        color,
    };
    shapes.push(line(at(-22.0, 0.0), at(22.0, 0.0), 4, DECK));
    for along in [-14.0, 14.0] {
        let wheel = at(along, -5.0);
        shapes.push(disc(wheel.x, wheel.y, 3, WHEEL));
    }
    for side in [-1.0, 1.0] {
        shapes.push(line(at(side * 8.0, 2.0), at(side * 5.0, 14.0), 4, PANTS));
        shapes.push(line(at(side * 5.0, 14.0), at(0.0, HIPS), 4, PANTS));
    }
    shapes.push(line(at(0.0, HIPS), at(0.0, 40.0), 6, SHIRT));
    shapes.push(line(at(-13.0, 36.0), at(13.0, 36.0), 3, SKIN));
    let head = at(0.0, 48.0);
    shapes.push(disc(head.x, head.y, 6, HELMET));
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
