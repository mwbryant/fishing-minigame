use alloc::{format, vec::Vec};

use angler_game::{SPECIES, length_text};
use bevy::prelude::*;
use orbit_core::{Frame, Mic, Power, Tilt};
use orbit_gfx::{Point, Rgb565, SCREEN_CENTER, Shape};
use orbit_ui::{
    DEEP, GOLD, HINT, TEXT, TOAST, battery_level, dial_choice, dial_point, disc, framed_button,
    key_ring, label, label_at, labelled_button, list_choice, power_overlay, rect, rgb,
};
use pals::{Pals, Pet, Rarity, TrickSlot};

use super::{
    CARD_CHOICES, Heard, Home, HomePhase, PracticeStep, STAGE, TRICK_CHOICES, TeachStep,
    performing, ui,
};
use crate::fish;

pub const BACKGROUND: Rgb565 = rgb(24, 70, 130);
const GRAVEL: Rgb565 = rgb(176, 150, 110);
const WEED: Rgb565 = rgb(60, 150, 80);
const BUBBLE: Rgb565 = rgb(170, 220, 245);
const TANK_SCALE: u32 = 3;
const STAGE_SCALE: u32 = 6;
const NAME_RING: f32 = 176.0;

pub fn rarity_color(rarity: Rarity) -> Rgb565 {
    match rarity {
        Rarity::Common => rgb(210, 210, 210),
        Rarity::Uncommon => rgb(110, 220, 110),
        Rarity::Rare => rgb(90, 160, 255),
        Rarity::Epic => rgb(200, 110, 255),
        Rarity::Legendary => rgb(255, 190, 50),
    }
}

pub(crate) fn draw_frame(
    home: Res<Home>,
    pals: Res<Pals>,
    tilt: Res<Tilt>,
    mic: Res<Mic>,
    power: Res<Power>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    match home.phase.as_ref() {
        HomePhase::Tank => tank(shapes, &home, &pals, tilt.steer_deg),
        HomePhase::Card { pet } => card(
            shapes,
            &pals.pets[*pet],
            list_choice(tilt.steer_deg, CARD_CHOICES, 1),
        ),
        HomePhase::Rename { ring, name, .. } => {
            shapes.push(rect(0, 0, 466, 466, DEEP));
            key_ring(shapes, ring, "A NEW NAME", name.as_str(), !name.is_empty());
        }
        HomePhase::Tricks { pet } => tricks(
            shapes,
            &pals.pets[*pet],
            list_choice(tilt.steer_deg, TRICK_CHOICES, 0),
        ),
        HomePhase::Teach { pet, slot, step } => {
            let fish = &pals.pets[*pet];
            stage(shapes, &home, &pals, *pet);
            let trick = fish.tricks[*slot].info().name;
            match step {
                TeachStep::Watch => {
                    shapes.push(label(
                        80,
                        &format!("WATCH {} {trick}", fish.name.as_str()),
                        1,
                        TEXT,
                    ));
                }
                TeachStep::Listen { word, stroke } => {
                    let word_text = match word {
                        Heard::Nothing => format!("SAY A WORD FOR {trick}"),
                        Heard::Once(_) => "SAY IT AGAIN".into(),
                        Heard::Sure(_) => "GOT THE WORD".into(),
                    };
                    let stroke_text = match stroke {
                        Heard::Nothing => "OR DRAW A STROKE".into(),
                        Heard::Once(_) => "DRAW IT AGAIN".into(),
                        Heard::Sure(gesture) => format!("GOT {}", gesture.name()),
                    };
                    shapes.push(label(70, &word_text, 1, TEXT));
                    shapes.push(label(92, &stroke_text, 1, TEXT));
                    let done = matches!(word, Heard::Sure(_)) || matches!(stroke, Heard::Sure(_));
                    let hint = if done {
                        "PRESS WHEN DONE"
                    } else {
                        "PRESS TO GO BACK"
                    };
                    shapes.push(label(404, hint, 1, HINT));
                    listening(shapes, &mic);
                }
            }
            toast(shapes, &home);
        }
        HomePhase::Practice { pet, slot, step } => {
            let fish = &pals.pets[*pet];
            stage(shapes, &home, &pals, *pet);
            let trick = &fish.tricks[*slot];
            shapes.push(label(
                60,
                &format!("PRACTICE {}", trick.info().name),
                1,
                TEXT,
            ));
            shapes.push(label(80, &learned_text(trick), 1, HINT));
            match step {
                PracticeStep::Praise => {
                    shapes.push(label(120, &format!("PET {}!", fish.name.as_str()), 2, GOLD));
                }
                PracticeStep::Waiting if home.toast().is_none() => {
                    shapes.push(label(372, "GIVE THE COMMAND", 1, TEXT));
                }
                _ => {}
            }
            toast(shapes, &home);
            shapes.push(label(394, &command_text(trick), 1, HINT));
            labelled_button(shapes, &ui::NEW_WORD, ui::NEW_WORD.text, HINT);
            listening(shapes, &mic);
        }
    }
    if let Some(battery) = power.battery {
        battery_level(shapes, &battery);
    }
    power_overlay(shapes, &power);
}

/// Water, gravel, weed and a few rising bubbles.
fn water(shapes: &mut Vec<Shape>, clock: f32) {
    let bands = 6;
    for i in 0..bands {
        let mix = |deep: u16, top: u16| (deep + (top - deep) * (bands - 1 - i) / (bands - 1)) as u8;
        let color = rgb(mix(24, 60), mix(70, 140), mix(130, 200));
        shapes.push(rect(0, i as i32 * 62, 466, 62, color));
    }
    shapes.push(rect(0, 330, 466, 136, GRAVEL));
    for (x, height) in [(110, 70), (130, 50), (330, 80), (352, 55)] {
        shapes.push(Shape::Line {
            from: Point::new(x, 330),
            to: Point::new(
                x + (bevy::math::ops::sin(clock + x as f32) * 6.0) as i32,
                330 - height,
            ),
            width: 5,
            color: WEED,
        });
    }
    for i in 0..4 {
        let rise = (clock * 30.0 + i as f32 * 80.0) % 260.0;
        shapes.push(disc(150 + i * 55, 330 - rise as i32, 3, BUBBLE));
    }
}

fn tank(shapes: &mut Vec<Shape>, home: &Home, pals: &Pals, steer: f32) {
    water(shapes, home.clock);
    for ((at, facing_right), pet) in home.swimmers().zip(&pals.pets) {
        fish::draw(shapes, pet, at, TANK_SCALE, facing_right);
    }
    if pals.pets.is_empty() {
        shapes.push(label(200, "THE TANK IS EMPTY", 2, TEXT));
        shapes.push(label(236, "CATCH A FISH AT THE POND", 1, HINT));
        shapes.push(label(256, "AND KEEP IT", 1, HINT));
    }
    let count = pals.pets.len() + 1;
    let pointed = dial_choice(steer, count);
    let step = 360.0 / count as f32;
    for i in 0..count {
        let text = pals.pets.get(i).map_or("OUT", |pet| pet.name.as_str());
        let at = dial_point(i as f32 * step, NAME_RING);
        let color = if i == pointed { GOLD } else { TEXT };
        if i == pointed {
            shapes.push(Shape::Ring {
                center: at,
                radius: 26,
                width: 2,
                color: GOLD,
            });
        }
        shapes.push(label_at(at, text, 1, color));
    }
}

/// Where the fish learns and performs, posed for what it's doing.
fn stage(shapes: &mut Vec<Shape>, home: &Home, pals: &Pals, pet: usize) {
    water(shapes, home.clock);
    let (offset, flipped) = performing(home, pals).unwrap_or((Vec2::ZERO, false));
    fish::draw(
        shapes,
        &pals.pets[pet],
        STAGE + offset,
        STAGE_SCALE,
        !flipped,
    );
}

fn toast(shapes: &mut Vec<Shape>, home: &Home) {
    if let Some(text) = home.toast() {
        shapes.push(label(372, text, 1, TOAST));
    }
}

/// While a word is coming in, say so, and how loud, so it's clear whether
/// the mic heard you at all.
pub(crate) fn listening(shapes: &mut Vec<Shape>, mic: &Mic) {
    if mic.speaking() {
        shapes.push(label(330, "LISTENING...", 1, TOAST));
    }
    level_meter(shapes, mic.level());
}

/// How loud the mic hears you, along the bottom of the rim.
pub(crate) fn level_meter(shapes: &mut Vec<Shape>, level: f32) {
    if level < 0.05 {
        return;
    }
    shapes.push(Shape::Arc {
        center: SCREEN_CENTER,
        radius: 224,
        width: 6,
        start_deg: 135.0,
        sweep_deg: 90.0 * level,
        color: TOAST,
    });
}

fn learned_text(trick: &TrickSlot) -> alloc::string::String {
    if trick.mastered() {
        return "MASTERED".into();
    }
    format!("LEARNED {}%", trick.learned)
}

/// What the trick answers to.
fn command_text(trick: &TrickSlot) -> alloc::string::String {
    match (trick.trigger.voice.is_some(), trick.trigger.gesture) {
        (true, Some(gesture)) => format!("YOUR WORD OR {}", gesture.name()),
        (true, None) => "SAY YOUR WORD".into(),
        (false, Some(gesture)) => gesture.name().into(),
        (false, None) => "NOT TAUGHT YET".into(),
    }
}

fn card(shapes: &mut Vec<Shape>, pet: &Pet, pointed: usize) {
    shapes.push(rect(0, 0, 466, 466, DEEP));
    fish::draw(shapes, pet, Vec2::new(233.0, 100.0), 5, true);
    shapes.push(label(160, pet.name.as_str(), 2, TEXT));
    let species = SPECIES[usize::from(pet.species) % SPECIES.len()].name;
    shapes.push(label_at(Point::new(190, 186), species, 1, HINT));
    shapes.push(label_at(
        Point::new(286, 186),
        &length_text(pet.length_mm),
        1,
        HINT,
    ));
    shapes.push(label_at(
        Point::new(176, 206),
        &format!("STAMINA LV {}", pet.stamina_level()),
        1,
        GOLD,
    ));
    shapes.push(label_at(
        Point::new(306, 206),
        &format!("APPEAL {}", pet.appeal()),
        1,
        HINT,
    ));
    for (row, trick) in pet.tricks.iter().enumerate() {
        trick_row(shapes, trick, 236 + row as i32 * 22);
    }
    for (i, button) in ui::CARD_BUTTONS.iter().enumerate() {
        let frame = if i == pointed { GOLD } else { HINT };
        framed_button(shapes, button, frame);
    }
}

/// A trick's name, rarity and how well it's known, across one row.
fn trick_row(shapes: &mut Vec<Shape>, trick: &TrickSlot, y: i32) {
    let info = trick.info();
    shapes.push(label_at(Point::new(150, y), info.name, 1, TEXT));
    shapes.push(label_at(
        Point::new(282, y),
        info.rarity.name(),
        1,
        rarity_color(info.rarity),
    ));
    let known = if trick.taught() {
        format!("{}%", trick.learned)
    } else {
        "NEW".into()
    };
    shapes.push(label_at(Point::new(366, y), &known, 1, HINT));
}

fn tricks(shapes: &mut Vec<Shape>, pet: &Pet, pointed: usize) {
    shapes.push(rect(0, 0, 466, 466, DEEP));
    shapes.push(label(
        66,
        &format!("{}'S TRICKS", pet.name.as_str()),
        1,
        TEXT,
    ));
    for (row, (trick, area)) in pet.tricks.iter().zip(&ui::TRICK_ROWS).enumerate() {
        let frame = if row == pointed { GOLD } else { DEEP };
        shapes.push(Shape::Rect {
            area: area.area,
            color: frame,
        });
        shapes.push(Shape::Rect {
            area: area.area.offset(-2),
            color: DEEP,
        });
        trick_row(shapes, trick, area.area.center().y);
    }
    let back = &ui::TRICK_ROWS[TRICK_CHOICES - 1];
    let frame = if pointed == TRICK_CHOICES - 1 {
        GOLD
    } else {
        HINT
    };
    framed_button(shapes, back, frame);
    shapes.push(label(396, "TURN TO PICK, PRESS", 1, HINT));
}
