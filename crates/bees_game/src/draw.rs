use alloc::{format, vec::Vec};

use bevy::{math::ops, prelude::*};
use orbit_core::{Frame, Launcher, Power};
use orbit_gfx::{Point, Rgb565, SCREEN_CENTER, Shape, Sprite, SpriteRef};
use orbit_ui::{
    BLACK, DANGER, GOLD, HINT, TEXT, battery_level_on, button, disc, label, plated, power_overlay,
    rect, rgb, running_low,
};
use pals::HATS;

use crate::{
    DELIVERY_SECS, EARTH_RADIUS, FLIGHT_SECS, Orbit, Phase, RESULT_GUARD_SECS, sprites, ui,
};

/// The hat the lead fish in the tank wears, which the bees wear too: an
/// index into `HATS`, if it wears one.
#[derive(Resource, Default, Debug)]
pub struct Hat(pub Option<usize>);

const OCEAN: Rgb565 = rgb(40, 100, 210);
const LAND: Rgb565 = rgb(70, 170, 90);
const GLASS: Rgb565 = rgb(200, 220, 236);
const HONEY: Rgb565 = rgb(242, 172, 32);
const STAR: Rgb565 = rgb(150, 150, 170);
const GAUGE: Rgb565 = rgb(40, 34, 20);
const BEE_SCALE: u32 = 3;
const HAT_SCALE: u32 = 2;
const JAR_RADIUS: u32 = 24;
const SWARM: usize = 3;

pub(crate) fn draw_frame(
    orbit: Res<Orbit>,
    hat: Res<Hat>,
    power: Res<Power>,
    launcher: Option<Res<Launcher>>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    let hat = hat.0.and_then(|hat| HATS.get(hat)).map(|hat| hat.sprite);
    space(shapes);
    match orbit.phase {
        Phase::Title => {
            jar(shapes, &orbit);
            swarm(shapes, &orbit, hat);
            text(shapes, label(320, "BEES???", 3, GOLD));
            text(shapes, label(364, "THE MOON IS A JAR OF HONEY", 1, TEXT));
            text(shapes, label(386, "TURN TO KEEP IT FROM SPILLING", 1, HINT));
            text(shapes, label(410, "PRESS TO PLAY", 1, GOLD));
            text(shapes, label(430, &format!("BEST {}", orbit.best), 1, TEXT));
        }
        Phase::Orbiting => {
            for drop in &orbit.drops {
                shapes.push(disc(drop.at.x as i32, drop.at.y as i32, 3, HONEY));
            }
            jar(shapes, &orbit);
            swarm(shapes, &orbit, hat);
            delivery(shapes, &orbit, hat);
            gauge(shapes, orbit.honey);
            text(
                shapes,
                label(318, &format!("ORBIT {}", orbit.orbits), 1, TEXT),
            );
            if orbit.spilling() {
                text(shapes, label(165, "SPILLING!", 2, DANGER));
            }
        }
        Phase::Empty => {
            text(shapes, label(150, "THE JAR IS EMPTY", 2, HONEY));
            text(
                shapes,
                label(300, &format!("ORBITS {}", orbit.orbits), 2, TEXT),
            );
            if orbit.new_best {
                text(shapes, label(340, "NEW BEST!", 2, GOLD));
            } else {
                text(shapes, label(340, &format!("BEST {}", orbit.best), 1, HINT));
            }
            if orbit.phase_secs >= RESULT_GUARD_SECS {
                text(shapes, label(390, "PRESS TO PLAY AGAIN", 1, GOLD));
            }
        }
    }
    let menu = orbit.phase != Phase::Orbiting;
    if menu && launcher.is_some() {
        button(shapes, &ui::HOME);
    }
    if let Some(battery) = power.battery
        && (menu || running_low(&battery))
    {
        battery_level_on(shapes, &battery, BLACK);
    }
    power_overlay(shapes, &power);
}

/// Text in space, so stars and passing bees stay out of its letters.
fn text(shapes: &mut Vec<Shape>, label: Shape) {
    plated(shapes, label, BLACK);
}

/// Stars and the Earth, static after the first frame.
fn space(shapes: &mut Vec<Shape>) {
    for i in 0..24u32 {
        let x = (i * 151 + 37) % 420 + 23;
        let y = (i * 97 + 61) % 420 + 23;
        shapes.push(rect(x as i32, y as i32, 2, 2, STAR));
    }
    let (cx, cy) = (SCREEN_CENTER.x, SCREEN_CENTER.y);
    shapes.push(disc(cx, cy, EARTH_RADIUS as u32, OCEAN));
    for (dx, dy, r) in [(-18, -14, 16), (14, 10, 20), (-8, 26, 10), (24, -22, 8)] {
        shapes.push(disc(cx + dx, cy + dy, r, LAND));
    }
}

/// A glass jar whose mouth (a gap in the glass) points where the tilt says,
/// with the honey pooled on the Earth side.
fn jar(shapes: &mut Vec<Shape>, orbit: &Orbit) {
    let at = orbit.jar();
    let center = Point::new(at.x as i32, at.y as i32);
    let inward = (super::center() - at).normalize_or_zero();
    let fill = 5.0 + 11.0 * ops::sqrt(orbit.honey.max(0.0));
    let pool = at + inward * (JAR_RADIUS as f32 - 4.0 - fill) * 0.8;
    shapes.push(disc(pool.x as i32, pool.y as i32, fill as u32, HONEY));
    shapes.push(Shape::Ring {
        center,
        radius: JAR_RADIUS,
        width: 3,
        color: GLASS,
    });
    shapes.push(Shape::Arc {
        center,
        radius: JAR_RADIUS,
        width: 7,
        start_deg: orbit.mouth_deg() - 90.0 - 35.0,
        sweep_deg: 70.0,
        color: BLACK,
    });
}

/// The bees circling the jar.
fn swarm(shapes: &mut Vec<Shape>, orbit: &Orbit, hat: Option<&'static Sprite>) {
    let jar = orbit.jar();
    for i in 0..SWARM {
        let a = orbit.clock * 2.5 + i as f32 * 2.09;
        let (sin, cos) = ops::sin_cos(a);
        bee(shapes, jar + Vec2::new(cos, sin) * 42.0, -sin > 0.0, hat);
    }
}

/// During the last stretch before a refill, one bee flies up from Earth.
fn delivery(shapes: &mut Vec<Shape>, orbit: &Orbit, hat: Option<&'static Sprite>) {
    if orbit.delivery_in > FLIGHT_SECS || orbit.delivery_in > DELIVERY_SECS {
        return;
    }
    let progress = 1.0 - orbit.delivery_in / FLIGHT_SECS;
    let jar = orbit.jar();
    let out = (jar - super::center()).normalize_or_zero();
    let start = super::center() + out * (EARTH_RADIUS + 8.0);
    let at = start.lerp(jar - out * 30.0, progress);
    shapes.push(disc(at.x as i32 + 6, at.y as i32 + 14, 3, HONEY));
    bee(shapes, at, out.x > 0.0, hat);
}

/// A bee centered at `at`, wearing `hat` on its head if there is one.
fn bee(shapes: &mut Vec<Shape>, at: Vec2, facing_right: bool, hat: Option<&'static Sprite>) {
    let size = sprites::BEE.size() * BEE_SCALE;
    let top_left = Point::new(
        at.x as i32 - size.width as i32 / 2,
        at.y as i32 - size.height as i32 / 2,
    );
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(&sprites::BEE),
        top_left,
        scale: BEE_SCALE,
        flip_x: !facing_right,
    });
    let Some(hat) = hat else {
        return;
    };
    let (head_column, head_top) = sprites::BEE_HEAD;
    let head_x = if facing_right {
        head_column
    } else {
        sprites::BEE.size().width as f32 - head_column
    };
    let head_x = top_left.x as f32 + head_x * BEE_SCALE as f32;
    let head_y = top_left.y + head_top * BEE_SCALE as i32;
    // Hats are drawn on a head grid with the face in columns 4-8 (center 6.5).
    let hat_size = hat.size();
    let hat_center = if facing_right {
        6.5
    } else {
        hat_size.width as f32 - 6.5
    };
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(hat),
        top_left: Point::new(
            (head_x - hat_center * HAT_SCALE as f32) as i32,
            head_y - (hat_size.height as i32 - 1) * HAT_SCALE as i32,
        ),
        scale: HAT_SCALE,
        flip_x: !facing_right,
    });
}

/// The honey left, along the bottom of the rim.
fn gauge(shapes: &mut Vec<Shape>, honey: f32) {
    let arc = |sweep: f32, color| Shape::Arc {
        center: SCREEN_CENTER,
        radius: 224,
        width: 8,
        start_deg: 45.0,
        sweep_deg: sweep,
        color,
    };
    shapes.push(arc(90.0, GAUGE));
    shapes.push(arc(90.0 * honey.max(0.01), HONEY));
}
