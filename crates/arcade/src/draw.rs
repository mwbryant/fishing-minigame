use alloc::vec::Vec;

use bevy::prelude::*;
use orbit_core::{Frame, Power, SoundQueue, Tilt};
use orbit_gfx::{Point, SCREEN_CENTER, Shape, Sprite, SpriteRef};
use orbit_ui::{
    GOLD, HINT, TEXT, battery_level, dial_angle, dial_choice, framed_button, label,
    labelled_button, power_overlay,
};

use crate::{CHOICES, Choice, Game, Menu, SETTINGS_OPTIONS, Screen, sprites, ui};

pub(crate) fn draw_frame(
    menu: Res<Menu>,
    tilt: Res<Tilt>,
    power: Res<Power>,
    sound: Res<SoundQueue>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    match menu.screen {
        Screen::Dial => dial(shapes, tilt.steer_deg),
        Screen::Settings => settings(shapes, tilt.steer_deg, sound.muted()),
    }
    if let Some(battery) = power.battery {
        battery_level(shapes, &battery);
    }
    power_overlay(shapes, &power);
}

/// Four rings on the compass points, the one the device points at lit, a
/// gold arc on the rim following the turn, and the pick's name in the middle.
fn dial(shapes: &mut Vec<Shape>, steer_deg: f32) {
    let pointed = dial_choice(steer_deg, CHOICES.len());
    for (i, (button, choice)) in ui::CHOICE_BUTTONS.iter().zip(CHOICES).enumerate() {
        let lit = i == pointed;
        let center = button.area.center();
        shapes.push(Shape::Ring {
            center,
            radius: 34,
            width: if lit { 4 } else { 2 },
            color: if lit { GOLD } else { HINT },
        });
        icon(shapes, icon_for(choice), center);
    }
    rim_pointer(shapes, steer_deg);
    let [first, second] = CHOICES[pointed].name();
    if second.is_empty() {
        shapes.push(label(226, first, 2, TEXT));
    } else {
        shapes.push(label(208, first, 2, TEXT));
        shapes.push(label(244, second, 2, TEXT));
    }
    shapes.push(label(290, "PRESS PWR", 1, HINT));
}

fn settings(shapes: &mut Vec<Shape>, steer_deg: f32, muted: bool) {
    let pointed = dial_choice(steer_deg, SETTINGS_OPTIONS);
    let frame = |lit: bool| if lit { GOLD } else { HINT };
    shapes.push(label(80, "SETTINGS", 2, TEXT));
    let sound = if muted { "SOUND OFF" } else { "SOUND ON" };
    labelled_button(shapes, &ui::SOUND, sound, frame(pointed == 0));
    framed_button(shapes, &ui::BACK, frame(pointed == 1));
    shapes.push(label(240, "TURN TO CHOOSE", 1, HINT));
    shapes.push(label(262, "PWR TO PRESS", 1, HINT));
    rim_pointer(shapes, steer_deg);
}

/// A short gold arc on the rim, centered where the dial points.
fn rim_pointer(shapes: &mut Vec<Shape>, steer_deg: f32) {
    shapes.push(Shape::Arc {
        center: SCREEN_CENTER,
        radius: 222,
        width: 8,
        start_deg: dial_angle(steer_deg) - 90.0 - 12.0,
        sweep_deg: 24.0,
        color: GOLD,
    });
}

fn icon_for(choice: Choice) -> &'static Sprite {
    match choice {
        Choice::Play(Game::Angler) => &sprites::FISH,
        Choice::Play(Game::Spider) => &sprites::SPIDER,
        Choice::Play(Game::Skate) => &sprites::SKATEBOARD,
        Choice::Settings => &sprites::GEAR,
    }
}

fn icon(shapes: &mut Vec<Shape>, sprite: &'static Sprite, center: Point) {
    let size = sprite.size() * 4;
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(sprite),
        top_left: center - Point::new(size.width as i32 / 2, size.height as i32 / 2),
        scale: 4,
        flip_x: false,
    });
}
