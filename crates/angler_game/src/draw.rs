use alloc::{format, string::String, vec::Vec};

use bevy::{math::ops, prelude::*};
use orbit_core::{Frame, Tilt};
use orbit_gfx::{Point, Rectangle, Rgb565, SCREEN_CENTER, Shape, Size, SpriteRef};

use crate::{
    cast::{self, Flight, scene::*},
    log::FishLog,
    reel::{FISH_HALF, Reel, TRACK_HEIGHT, TRACK_TOP, TRACK_WIDTH, ZONE_HEIGHT},
    session::{Phase, RESULT_GUARD_SECS, Session},
    species::{Catch, SPECIES},
    sprites::{self, rgb},
};

pub const BACKGROUND: Rgb565 = rgb(0, 0, 0);
const SKY: [Rgb565; 6] = [
    rgb(40, 70, 140),
    rgb(60, 95, 165),
    rgb(85, 125, 190),
    rgb(115, 155, 210),
    rgb(150, 185, 225),
    rgb(185, 210, 235),
];
const SUN: Rgb565 = rgb(255, 230, 150);
const FAR_HILL: Rgb565 = rgb(50, 95, 70);
const NEAR_HILL: Rgb565 = rgb(65, 112, 80);
const WATER: Rgb565 = rgb(35, 90, 140);
const WAVE: Rgb565 = rgb(70, 130, 175);
const RIPPLE: Rgb565 = rgb(160, 210, 235);
const PLANK: Rgb565 = rgb(140, 95, 55);
const PLANK_EDGE: Rgb565 = rgb(90, 60, 35);
const POST: Rgb565 = rgb(100, 70, 40);
const ROD: Rgb565 = rgb(120, 80, 40);
const LINE: Rgb565 = rgb(225, 225, 225);
const TRAIL: Rgb565 = rgb(235, 235, 235);
const DEEP: Rgb565 = rgb(12, 34, 58);
const TRACK: Rgb565 = rgb(24, 26, 36);
const ZONE: Rgb565 = rgb(80, 190, 90);
const ZONE_HOT: Rgb565 = rgb(130, 235, 120);
const GAUGE_BACK: Rgb565 = rgb(40, 40, 52);
const TEXT: Rgb565 = rgb(255, 255, 255);
const HINT: Rgb565 = rgb(200, 215, 230);
const GOLD: Rgb565 = rgb(255, 215, 70);
const TOAST: Rgb565 = rgb(120, 230, 255);

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, draw_frame);
}

fn draw_frame(session: Res<Session>, tilt: Res<Tilt>, log: Res<FishLog>, mut frame: ResMut<Frame>) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    let elevation = cast::elevation_deg(tilt.steer_deg);
    let t = session.phase_secs;
    match &session.phase {
        Phase::Title => {
            lake(shapes, &session, cast::BASE_ELEVATION_DEG, false);
            shapes.push(label(110, "GONE FISHIN", 3, TEXT));
            shapes.push(label(160, "PRESS BUTTON", 1, HINT));
            shapes.push(label(186, "TAP FOR FISH LOG", 1, HINT));
            shapes.push(label(400, &format!("CAUGHT {}", log.total), 1, TEXT));
            level(shapes, &tilt);
        }
        Phase::Log => fish_log(shapes, &log),
        Phase::Aim { charging_secs } => {
            lake(shapes, &session, elevation, true);
            shapes.push(label(52, &format!("ANGLE {}", elevation as i32), 1, TEXT));
            match charging_secs {
                Some(secs) => {
                    let power = cast::charge_power(*secs);
                    power_gauge(shapes, power);
                    preview(shapes, &Flight::launch(elevation, power));
                }
                None => shapes.push(label(76, "HOLD BUTTON TO CAST", 1, HINT)),
            }
        }
        Phase::Flight { flight, trail, .. } => {
            lake(shapes, &session, elevation, true);
            dots(shapes, trail, 2, TRAIL);
            bobber(shapes, flight.pos, 0);
        }
        Phase::Waiting {
            bobber_x, trail, ..
        } => {
            lake(shapes, &session, elevation, true);
            dots(shapes, trail, 2, TRAIL);
            let bob = (ops::sin(session.clock * 3.0) * 1.5) as i32;
            floating(shapes, elevation, *bobber_x, bob);
            if t < 0.5 {
                splash(shapes, *bobber_x, t);
            }
            shapes.push(label(52, "WAIT FOR A BITE", 1, TEXT));
            shapes.push(label(76, "PRESS TO REEL IN", 1, HINT));
        }
        Phase::Bite { bobber_x, .. } => {
            lake(shapes, &session, elevation, true);
            floating(shapes, elevation, *bobber_x, 4);
            shapes.push(Shape::Label {
                center: Point::new(*bobber_x as i32, WATER_Y as i32 - 40),
                text: "!".into(),
                scale: 3,
                color: GOLD,
            });
        }
        Phase::Notice(text) => {
            lake(shapes, &session, elevation, true);
            shapes.push(label(120, text, 2, TEXT));
        }
        Phase::Reel { reel, .. } => reel_bar(shapes, reel),
        Phase::Caught { catch, milestones } => {
            caught(shapes, catch);
            let banner = if milestones.new_species {
                Some("NEW SPECIES!")
            } else if milestones.longest || milestones.heaviest {
                Some("NEW RECORD!")
            } else {
                None
            };
            if let Some(banner) = banner {
                shapes.push(label(92, banner, 2, GOLD));
            }
            if t >= RESULT_GUARD_SECS {
                shapes.push(label(400, "PRESS BUTTON", 1, HINT));
            }
        }
        Phase::Escaped => {
            shapes.push(rect(0, 0, 466, 466, DEEP));
            shapes.push(label(220, "IT GOT AWAY...", 2, TEXT));
        }
    }
    if session.toast_secs > 0.0 {
        shapes.push(label(140, "LEVEL SET", 2, TOAST));
    }
}

/// The side-view scene: sky, hills, lake, dock, angler and rod, plus the
/// rippling fishing spot when `spot` is set.
fn lake(shapes: &mut Vec<Shape>, session: &Session, elevation: f32, spot: bool) {
    let band = WATER_Y as i32 / SKY.len() as i32 + 1;
    for (i, color) in SKY.iter().enumerate() {
        shapes.push(rect(0, i as i32 * band, 466, band as u32, *color));
    }
    shapes.push(disc(382, 165, 18, SUN));
    shapes.push(disc(110, 335, 110, FAR_HILL));
    shapes.push(disc(335, 350, 120, NEAR_HILL));
    shapes.push(rect(0, WATER_Y as i32, 466, 466 - WATER_Y as u32, WATER));
    for (x, y, w) in [
        (150, 290, 60),
        (300, 310, 80),
        (200, 345, 50),
        (360, 372, 40),
        (125, 398, 70),
    ] {
        shapes.push(rect(x, y, w, 2, WAVE));
    }
    if spot {
        ripples(shapes, session.spot_x, session.clock);
    }
    shapes.push(rect(0, DOCK_TOP, SHORE_X as u32, 10, PLANK));
    shapes.push(rect(0, DOCK_TOP + 10, SHORE_X as u32, 3, PLANK_EDGE));
    for x in [18, 58, 98] {
        shapes.push(rect(x, DOCK_TOP + 13, 7, 48, POST));
    }
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(&sprites::ANGLER),
        top_left: Point::new(ANGLER_TOP_LEFT.0, ANGLER_TOP_LEFT.1),
        scale: ANGLER_SCALE as u32,
        flip_x: false,
    });
    shapes.push(Shape::Line {
        from: point(HANDS),
        to: point(cast::rod_tip(elevation)),
        width: 2,
        color: ROD,
    });
}

fn ripples(shapes: &mut Vec<Shape>, spot_x: f32, clock: f32) {
    for i in 0..3 {
        let phase = ops::rem_euclid(clock * 0.6 + i as f32 / 3.0, 1.0);
        let width = 10 + (34.0 * phase) as u32;
        let y = WATER_Y as i32 + 4 + i * 5;
        shapes.push(rect(spot_x as i32 - width as i32 / 2, y, width, 2, RIPPLE));
    }
}

fn floating(shapes: &mut Vec<Shape>, elevation: f32, x: f32, dip: i32) {
    shapes.push(Shape::Line {
        from: point(cast::rod_tip(elevation)),
        to: Point::new(x as i32, WATER_Y as i32 - 5 + dip),
        width: 1,
        color: LINE,
    });
    bobber(shapes, Vec2::new(x, WATER_Y), dip);
}

fn bobber(shapes: &mut Vec<Shape>, at: Vec2, dip: i32) {
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(&sprites::BOBBER),
        top_left: Point::new(
            at.x as i32 - 5,
            at.y as i32 - sprites::BOBBER_WATERLINE * 2 + dip,
        ),
        scale: 2,
        flip_x: false,
    });
}

fn splash(shapes: &mut Vec<Shape>, x: f32, t: f32) {
    for dx in [-1.0, 1.0] {
        for (spread, lift) in [(8.0, 70.0), (16.0, 45.0)] {
            let at = Vec2::new(
                x + dx * (spread + 30.0 * t),
                WATER_Y - lift * t + 160.0 * t * t,
            );
            if at.y < WATER_Y {
                shapes.push(disc(at.x as i32, at.y as i32, 2, RIPPLE));
            }
        }
    }
}

fn dots(shapes: &mut Vec<Shape>, points: &[Vec2], radius: u32, color: Rgb565) {
    for p in points {
        shapes.push(Shape::Disc {
            center: point(*p),
            radius,
            color,
        });
    }
}

/// Angry Birds-style dotted path for the cast being charged.
fn preview(shapes: &mut Vec<Shape>, flight: &Flight) {
    let path: Vec<Vec2> = (1..16)
        .map(|i| flight.at(i as f32 * 0.09))
        .take_while(|p| p.y < WATER_Y)
        .collect();
    dots(shapes, &path, 2, TEXT);
}

/// Fills along the lower-left rim, green to red as power rises.
fn power_gauge(shapes: &mut Vec<Shape>, power: f32) {
    let arc = |sweep: f32, color| Shape::Arc {
        center: SCREEN_CENTER,
        radius: 212,
        width: 12,
        start_deg: 100.0,
        sweep_deg: sweep,
        color,
    };
    shapes.push(arc(70.0, GAUGE_BACK));
    let color = if power < 0.5 {
        rgb(110, 220, 90)
    } else if power < 0.85 {
        rgb(250, 210, 60)
    } else {
        rgb(240, 80, 50)
    };
    shapes.push(arc(70.0 * power.max(0.02), color));
}

/// A bubble level on the title screen: shows the IMU working and where zero is.
fn level(shapes: &mut Vec<Shape>, tilt: &Tilt) {
    let center = Point::new(233, 330);
    shapes.push(Shape::Ring {
        center,
        radius: 24,
        width: 2,
        color: HINT,
    });
    let offset = Vec2::new(tilt.steer_deg, -tilt.lean_deg) * 0.8;
    let offset = offset.clamp_length_max(20.0);
    shapes.push(Shape::Disc {
        center: center + Point::new(offset.x as i32, offset.y as i32),
        radius: 4,
        color: TOAST,
    });
}

fn reel_bar(shapes: &mut Vec<Shape>, reel: &Reel) {
    shapes.push(rect(0, 0, 466, 466, DEEP));
    shapes.push(label(44, "LEAN TO REEL", 1, HINT));
    let left = 233 - TRACK_WIDTH as i32 / 2;
    let top = TRACK_TOP as i32;
    shapes.push(rect(
        left,
        top,
        TRACK_WIDTH as u32,
        TRACK_HEIGHT as u32,
        TRACK,
    ));
    let zone_top = top + (reel.zone_y - ZONE_HEIGHT / 2.0) as i32;
    let zone_color = if reel.fish_in_zone() { ZONE_HOT } else { ZONE };
    shapes.push(rect(
        left,
        zone_top,
        TRACK_WIDTH as u32,
        ZONE_HEIGHT as u32,
        zone_color,
    ));
    let fish = sprites::REEL_FISH.size() * 3;
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(&sprites::REEL_FISH),
        top_left: Point::new(
            233 - fish.width as i32 / 2,
            top + (reel.fish_y - FISH_HALF) as i32,
        ),
        scale: 3,
        flip_x: false,
    });

    let gauge_left = left + TRACK_WIDTH as i32 + 14;
    shapes.push(rect(gauge_left, top, 12, TRACK_HEIGHT as u32, GAUGE_BACK));
    let filled = (TRACK_HEIGHT * reel.progress) as u32;
    let color = if reel.progress < 0.3 {
        rgb(240, 80, 50)
    } else if reel.progress < 0.7 {
        rgb(250, 210, 60)
    } else {
        rgb(110, 220, 90)
    };
    shapes.push(rect(
        gauge_left,
        top + TRACK_HEIGHT as i32 - filled as i32,
        12,
        filled,
        color,
    ));
}

fn caught(shapes: &mut Vec<Shape>, catch: &Catch) {
    shapes.push(rect(0, 0, 466, 466, DEEP));
    let species = catch.species();
    let size = species.sprite.size();
    let scale = (280 / size.width).clamp(1, 12);
    let drawn = size * scale;
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(species.sprite),
        top_left: Point::new(233 - drawn.width as i32 / 2, 175 - drawn.height as i32 / 2),
        scale,
        flip_x: false,
    });
    shapes.push(label(272, species.name, 2, TEXT));
    shapes.push(label(314, &length_text(catch.length_mm), 2, TEXT));
    shapes.push(label(354, &weight_text(catch.weight_g), 2, TEXT));
}

fn fish_log(shapes: &mut Vec<Shape>, log: &FishLog) {
    shapes.push(rect(0, 0, 466, 466, DEEP));
    shapes.push(label(74, "FISH LOG", 2, TEXT));
    for (i, (species, entry)) in SPECIES.iter().zip(&log.species).enumerate() {
        let line = if entry.caught == 0 {
            String::from("???")
        } else {
            format!(
                "{:<10} x{:<3} {:>7} {:>8}",
                species.name,
                entry.caught,
                length_text(entry.best_length_mm),
                weight_text(entry.best_weight_g)
            )
        };
        shapes.push(label(118 + i as i32 * 30, &line, 1, TEXT));
    }
    shapes.push(label(380, &format!("TOTAL {}", log.total), 1, HINT));
}

pub fn length_text(length_mm: u16) -> String {
    format!("{}.{} CM", length_mm / 10, length_mm % 10)
}

pub fn weight_text(weight_g: u32) -> String {
    if weight_g >= 1000 {
        return format!("{}.{:02} KG", weight_g / 1000, weight_g % 1000 / 10);
    }
    format!("{weight_g} G")
}

fn label(y: i32, text: &str, scale: u32, color: Rgb565) -> Shape {
    Shape::Label {
        center: Point::new(233, y),
        text: text.into(),
        scale,
        color,
    }
}

fn rect(x: i32, y: i32, w: u32, h: u32, color: Rgb565) -> Shape {
    Shape::Rect {
        area: Rectangle::new(Point::new(x, y), Size::new(w, h)),
        color,
    }
}

fn disc(x: i32, y: i32, radius: u32, color: Rgb565) -> Shape {
    Shape::Disc {
        center: Point::new(x, y),
        radius,
        color,
    }
}

fn point(v: Vec2) -> Point {
    Point::new(ops::round(v.x) as i32, ops::round(v.y) as i32)
}
