//! Drawing a kept fish, hat and all, at any size and facing either way, how
//! each trick moves it, and the water the wet ones throw about.

use alloc::vec::Vec;

use angler_game::SPECIES;
use bevy::math::{Vec2, ops};
use orbit_gfx::{Point, Rgb565, Shape, Size, SpriteRef};
use orbit_ui::{GOLD, TEXT, label_at, plated, rgb};
use pals::{HATS, Pet, tricks::Motion};

/// Where a hat sits on each species (by index into `SPECIES`): the column
/// its middle goes over, and the top row of the fish there. Fish face right.
const HAT_SEATS: [(f32, i32); 8] = [
    (8.0, 0),
    (11.0, 0),
    (11.0, 0),
    (13.0, 0),
    (12.0, 0),
    (14.0, 1),
    (17.0, 1),
    (13.0, 0),
];
/// The middle of a hat sprite's face, which spans columns 4-8.
const HAT_MIDDLE: f32 = 6.5;
/// The drops and bubbles a trick throws about. Screens clear it away from
/// their text (`orbit_ui::clear_text_of`).
pub const SPRAY: Rgb565 = rgb(215, 240, 255);
const SPLASH_DROPS: usize = 9;
const RING_BUBBLES: usize = 12;
const RISING_BUBBLES: usize = 7;

/// The fish sprite's size at `scale`, body only.
pub fn size(pet: &Pet, scale: u32) -> Size {
    sprite(pet).size() * scale
}

fn sprite(pet: &Pet) -> &'static orbit_gfx::Sprite {
    SPECIES[usize::from(pet.species) % SPECIES.len()].sprite
}

/// The fish centered at `at`, its hat on (a little smaller, to suit a fish).
pub fn draw(shapes: &mut Vec<Shape>, pet: &Pet, at: Vec2, scale: u32, facing_right: bool) {
    let fish = sprite(pet);
    let drawn = fish.size() * scale;
    let top_left = Point::new(
        at.x as i32 - drawn.width as i32 / 2,
        at.y as i32 - drawn.height as i32 / 2,
    );
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(fish),
        top_left,
        scale,
        flip_x: !facing_right,
    });
    let Some(hat) = pet.hat.and_then(|hat| HATS.get(usize::from(hat))) else {
        return;
    };
    let (column, row) = HAT_SEATS[usize::from(pet.species) % HAT_SEATS.len()];
    let width = fish.size().width as f32;
    let column = if facing_right { column } else { width - column };
    let hat_scale = (scale * 3 / 4).max(1);
    let hat_size = hat.sprite.size();
    let middle = if facing_right {
        HAT_MIDDLE
    } else {
        hat_size.width as f32 - HAT_MIDDLE
    };
    let seat_x = top_left.x as f32 + column * scale as f32;
    let seat_y = top_left.y + row * scale as i32;
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(hat.sprite),
        top_left: Point::new(
            (seat_x - middle * hat_scale as f32) as i32,
            seat_y - (hat_size.height as i32 - 1) * hat_scale as i32,
        ),
        scale: hat_scale,
        flip_x: !facing_right,
    });
}

/// A tap this close to a fish on a dial picks it, its name under it included.
pub const DIAL_REACH: f32 = 64.0;
/// How far a picked fish's ring reaches from where it swims.
const PICK_RING: u32 = 30;
/// A picked fish's name sits this far under its middle, clear of the ring.
const NAME_BELOW: i32 = PICK_RING as i32 + 16;

/// The kept fish at `spots`, then `out` at the last spot, for picking one by
/// turning: the pointed one ringed in gold, each name under its fish on a
/// plate of `backing` so the scenery behind stays out of the letters.
pub fn dial(
    shapes: &mut Vec<Shape>,
    pets: &[Pet],
    out: &str,
    spots: impl Iterator<Item = Point>,
    pointed: usize,
    backing: Rgb565,
) {
    for (i, at) in spots.enumerate().take(pets.len() + 1) {
        let color = if i == pointed { GOLD } else { TEXT };
        if i == pointed {
            shapes.push(Shape::Ring {
                center: at,
                radius: PICK_RING,
                width: 2,
                color: GOLD,
            });
        }
        let Some(pet) = pets.get(i) else {
            plated(shapes, label_at(at, out, 1, color), backing);
            continue;
        };
        draw(shapes, pet, Vec2::new(at.x as f32, at.y as f32), 2, true);
        let name = label_at(at + Point::new(0, NAME_BELOW), pet.name.as_str(), 1, color);
        plated(shapes, name, backing);
    }
}

/// Seconds a trick takes to perform.
pub const TRICK_SECS: f32 = 1.6;

/// Where a trick has the fish `t` seconds in, as a pixel offset from where it
/// started (callers scale it to fit their stage), and whether it faces the
/// other way.
pub fn pose(motion: Motion, t: f32) -> (Vec2, bool) {
    let u = (t / TRICK_SECS).clamp(0.0, 1.0);
    let arc = |height: f32| -height * 4.0 * u * (1.0 - u);
    let circle = |radius: f32, turns: f32| {
        let (sin, cos) = ops::sin_cos(u * turns * core::f32::consts::TAU);
        Vec2::new(sin * radius, (cos - 1.0) * radius)
    };
    let toggles = |times: f32| (u * times) as u32 % 2 == 1;
    match motion {
        Motion::Splash => (Vec2::new(0.0, arc(25.0)), false),
        Motion::Bubbles => (Vec2::new(0.0, ops::sin(u * 12.0) * 3.0), false),
        Motion::Wiggle => (Vec2::new(ops::sin(u * 70.0) * 8.0, 0.0), false),
        Motion::Dive => (Vec2::new(0.0, -arc(70.0)), false),
        Motion::Spin => (Vec2::ZERO, toggles(8.0)),
        Motion::Jump => (Vec2::new(0.0, arc(120.0)), false),
        Motion::Flip => (Vec2::new(0.0, arc(70.0)), toggles(2.0)),
        Motion::FinWave => (Vec2::new(0.0, ops::sin(u * 25.0) * 10.0), false),
        Motion::Loop => (circle(45.0, 1.0), false),
        Motion::BubbleRing => (Vec2::ZERO, false),
        Motion::TailWalk => (
            Vec2::new(
                -u * 90.0 + u * u * 90.0,
                arc(60.0) + ops::sin(u * 40.0) * 3.0,
            ),
            false,
        ),
        Motion::Corkscrew => (circle(35.0, 2.0), toggles(6.0)),
        Motion::Moonwalk => (Vec2::new(-arc(-50.0) * 2.0, 0.0), false),
        Motion::RainbowLeap => (Vec2::new((u - 0.5) * 120.0, arc(140.0)), false),
        Motion::Waterspout => (Vec2::new(0.0, arc(110.0)), toggles(4.0)),
    }
}

/// The water a trick throws about `t` seconds in, for a fish that started at
/// `at`. `reach` scales distances the way a stage scales [`pose`]. SPLASH
/// throws drops up and out, BUBBLES rise from the fish's mouth, and BUBBLE
/// RING blows a ring that spreads round it; the other tricks throw nothing.
pub fn effects(
    shapes: &mut Vec<Shape>,
    motion: Motion,
    t: f32,
    at: Vec2,
    facing_right: bool,
    reach: f32,
) {
    if !(0.0..TRICK_SECS).contains(&t) {
        return;
    }
    let u = t / TRICK_SECS;
    let drop = |shapes: &mut Vec<Shape>, center: Vec2, radius: f32| {
        shapes.push(Shape::Disc {
            center: Point::new(center.x as i32, center.y as i32),
            radius: radius.max(2.0) as u32,
            color: SPRAY,
        });
    };
    match motion {
        Motion::Splash => {
            if t < 0.6 {
                shapes.push(Shape::Ring {
                    center: Point::new(at.x as i32, at.y as i32),
                    radius: ((60.0 + 180.0 * t) * reach) as u32,
                    width: 3,
                    color: SPRAY,
                });
            }
            let speed = 440.0 * reach;
            let gravity = 800.0 * reach;
            for i in 0..SPLASH_DROPS {
                let along = i as f32 / (SPLASH_DROPS - 1) as f32;
                let angle = (-155.0 + 130.0 * along).to_radians();
                let (sin, cos) = ops::sin_cos(angle);
                let pace = 1.0 - 0.2 * (i % 3) as f32;
                let start = at + Vec2::new((along - 0.5) * 70.0 * reach, 10.0 * reach);
                let center = start
                    + Vec2::new(cos, sin) * speed * pace * t
                    + Vec2::new(0.0, gravity * t * t / 2.0);
                if center.y <= at.y + 30.0 * reach {
                    drop(shapes, center, 8.0 * reach);
                }
            }
        }
        Motion::Bubbles => {
            let side = if facing_right { 1.0 } else { -1.0 };
            let mouth = at + Vec2::new(side * 40.0 * reach, -10.0 * reach);
            for i in 0..RISING_BUBBLES {
                let age = t - i as f32 * 0.18;
                if !(0.0..1.0).contains(&age) {
                    continue;
                }
                let wobble = ops::sin(age * 9.0 + i as f32) * 6.0 * reach;
                let center = mouth + Vec2::new(wobble, -age * 140.0 * reach);
                drop(shapes, center, (4.0 + (i % 3) as f32) * reach + 1.0);
            }
        }
        Motion::BubbleRing => {
            let radius = (50.0 + 120.0 * u) * reach;
            let size = (7.0 - 4.0 * u) * reach + 1.0;
            for i in 0..RING_BUBBLES {
                let angle = (i as f32 / RING_BUBBLES as f32 + u * 0.15) * core::f32::consts::TAU;
                let (sin, cos) = ops::sin_cos(angle);
                drop(shapes, at + Vec2::new(cos, sin) * radius, size);
            }
        }
        _ => {}
    }
}
