use alloc::{format, vec::Vec};

use angler_game::sprites::ANGLER;
use bevy::prelude::*;
use orbit_core::{Frame, Power, SoundQueue, Tilt};
use orbit_gfx::{Point, Rgb565, Shape, SpriteRef};
use orbit_ui::{
    DANGER, DEEP, GOLD, HINT, TEXT, battery_level, button, dial_choice, disc, framed_button,
    gold_label, label, labelled_button, power_overlay, rect, rgb,
};
use pals::Pals;

use super::{
    ERASE_TURN_DEG, SETTINGS_CHOICES, Town, TownPhase,
    map::{LAKE, Screen, Way, props},
    ui,
};

pub const GRASS: Rgb565 = rgb(78, 150, 84);
const PATH: Rgb565 = rgb(206, 186, 136);
const WATER: Rgb565 = rgb(48, 110, 180);
const SHORE: Rgb565 = rgb(196, 180, 120);
const PATH_WIDTH: u32 = 40;
const AVATAR_SCALE: u32 = 2;

pub(crate) fn draw_frame(
    town: Res<Town>,
    pals: Res<Pals>,
    tilt: Res<Tilt>,
    sound: Res<SoundQueue>,
    power: Res<Power>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    match town.phase {
        TownPhase::Walking => {
            clearing(shapes, town.screen());
            avatar(shapes, &town);
            hud(shapes, &town, &pals);
        }
        TownPhase::Settings => settings(
            shapes,
            &pals,
            &sound,
            dial_choice(tilt.steer_deg, SETTINGS_CHOICES),
        ),
        TownPhase::ConfirmReset => confirm_reset(shapes, tilt.steer_deg >= ERASE_TURN_DEG),
    }
    if let Some(battery) = power.battery {
        battery_level(shapes, &battery);
    }
    power_overlay(shapes, &power);
}

/// Grass, the paths off the edge toward the next clearings, the pond's
/// water, and the scenery.
fn clearing(shapes: &mut Vec<Shape>, screen: Screen) {
    shapes.push(rect(0, 0, 466, 466, GRASS));
    let half = PATH_WIDTH as i32 / 2;
    for way in Way::ALL {
        if screen.toward(way).is_none() {
            continue;
        }
        let (x, y, w, h) = match way {
            Way::North => (233 - half, 0, PATH_WIDTH, 233),
            Way::South => (233 - half, 233, PATH_WIDTH, 233),
            Way::West => (0, 233 - half, 233, PATH_WIDTH),
            Way::East => (233, 233 - half, 233, PATH_WIDTH),
        };
        shapes.push(rect(x, y, w, h, PATH));
    }
    shapes.push(disc(233, 233, 34, PATH));
    if screen == Screen::Pond {
        let (center, radius) = LAKE;
        shapes.push(disc(
            center.x as i32,
            center.y as i32,
            radius as u32 + 6,
            SHORE,
        ));
        shapes.push(disc(center.x as i32, center.y as i32, radius as u32, WATER));
    }
    for prop in props(screen) {
        shapes.push(Shape::Sprite {
            sprite: SpriteRef(prop.sprite),
            top_left: prop.top_left,
            scale: prop.scale,
            flip_x: false,
        });
    }
}

/// The angler, feet where you stand, bobbing a pixel as they step.
fn avatar(shapes: &mut Vec<Shape>, town: &Town) {
    let size = ANGLER.size() * AVATAR_SCALE;
    let bob = if town.stepping() { 2 } else { 0 };
    let at = town.at();
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(&ANGLER),
        top_left: Point::new(
            at.x as i32 - size.width as i32 / 2,
            at.y as i32 - size.height as i32 - bob,
        ),
        scale: AVATAR_SCALE,
        flip_x: !town.facing_right(),
    });
}

/// The purse on top, and at a door, its name and how to go in.
fn hud(shapes: &mut Vec<Shape>, town: &Town, pals: &Pals) {
    gold_label(
        shapes,
        Point::new(240, 52),
        &format!("GOLD {}", pals.gold),
        1,
    );
    match town.near() {
        Some(place) => {
            shapes.push(rect(113, 366, 240, 76, DEEP));
            shapes.push(label(384, place.name(), 2, GOLD));
            button(shapes, &ui::ENTER);
        }
        None => {
            shapes.push(label(420, town.screen().name(), 1, rgb(230, 240, 220)));
            shapes.push(label(440, "TAP WHERE TO GO", 1, rgb(200, 220, 190)));
        }
    }
}

fn settings(shapes: &mut Vec<Shape>, pals: &Pals, sound: &SoundQueue, pointed: usize) {
    shapes.push(rect(0, 0, 466, 466, DEEP));
    shapes.push(label(170, "SETTINGS", 2, TEXT));
    shapes.push(label(292, "TURN TO PICK, PWR", 1, HINT));
    let texts = [
        if sound.muted() {
            "SOUND OFF"
        } else {
            "SOUND ON"
        },
        if pals.mock_walk {
            "MOCK WALK ON"
        } else {
            "MOCK WALK OFF"
        },
        "RESET",
        "BACK",
    ];
    for (i, (area, text)) in ui::SETTINGS_BUTTONS.iter().zip(texts).enumerate() {
        let frame = if i == pointed { GOLD } else { HINT };
        labelled_button(shapes, area, text, frame);
    }
    shapes.push(label(400, "MOCK WALKS STROLL ON THEIR OWN", 1, HINT));
}

fn confirm_reset(shapes: &mut Vec<Shape>, erase_pointed: bool) {
    shapes.push(rect(0, 0, 466, 466, DEEP));
    shapes.push(label(140, "RESET PROGRESS?", 2, TEXT));
    shapes.push(label(192, "LETS GO EVERY FISH, ALL GOLD", 1, HINT));
    shapes.push(label(214, "AND HATS, THE FISH LOG", 1, HINT));
    shapes.push(label(236, "AND EVERY BEST SCORE", 1, HINT));
    let (keep, erase) = if erase_pointed {
        (HINT, DANGER)
    } else {
        (GOLD, HINT)
    };
    framed_button(shapes, &ui::KEEP, keep);
    framed_button(shapes, &ui::ERASE, erase);
    shapes.push(label(370, "TURN RIGHT TO ERASE, PWR", 1, HINT));
}
