//! Draws every Spider Roulette screen into the frame's shape list.

use alloc::{format, vec::Vec};

use bevy::{math::ops, prelude::*};
use orbit_core::{Frame, Launcher, Power, Tilt};
use orbit_gfx::{Point, Rgb565, SCREEN_CENTER, Shape, Sprite, SpriteRef};
use orbit_ui::{
    BLACK, DANGER, GOLD, HINT, TEXT, battery_level_on, button, dial_choice, dial_point,
    framed_label, gold_label_on, label, label_at, plated, plated_lines, power_overlay, rect, rgb,
    running_low,
};
use pals::Pals;

use crate::{
    EGG_DEG, EGGS, Phase, RESULT_GUARD_SECS, Table, bet_spots, cash_value, sprites, stakes, ui,
};

const WEB: Rgb565 = rgb(58, 58, 70);
const FLASH: Rgb565 = rgb(170, 10, 10);
const BLOOD: Rgb565 = rgb(70, 0, 0);
const DANCER_SCALE: u32 = 5;
const POSE_SECS: f32 = 0.22;

pub(crate) fn draw_frame(
    table: Res<Table>,
    pals: Res<Pals>,
    tilt: Res<Tilt>,
    power: Res<Power>,
    launcher: Option<Res<Launcher>>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    let menu = matches!(
        table.phase,
        Phase::Title | Phase::Bitten | Phase::Won | Phase::CashedOut
    );
    match table.phase {
        Phase::Title => {
            web(shapes);
            dancer(shapes, table.clock, Point::new(233, 262));
            text(shapes, label(100, "SPIDER", 3, TEXT));
            text(shapes, label(160, "ROULETTE", 3, TEXT));
            text(shapes, label(330, "ONE EGG HIDES A SPIDER", 1, HINT));
            text(shapes, label(356, "PRESS TO SPIN", 1, GOLD));
            text(shapes, label(390, &format!("BEST {}", table.best), 1, TEXT));
            if pals.gold > 0 {
                gold_label_on(
                    shapes,
                    Point::new(240, 416),
                    &format!("GOLD {}", pals.gold),
                    1,
                    BLACK,
                );
            }
        }
        Phase::Bet => bet(shapes, &pals, tilt.steer_deg, launcher.is_some()),
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
            let status = label(140, &status, 1, TEXT);
            if table.stake > 0 {
                let worth = cash_value(table.stake, table.safe);
                let line = format!("BET {} | WORTH {worth}", table.stake);
                text(shapes, label(284, &line, 1, GOLD));
            }
            if table.phase == Phase::Ready {
                if table.stake > 0 && table.safe > 0 {
                    button(shapes, &ui::CASH_OUT);
                }
                plated_lines(
                    shapes,
                    [status, label(166, "PRESS TO SPIN", 1, GOLD)],
                    BLACK,
                );
            } else {
                text(shapes, status);
            }
        }
        Phase::CashedOut => {
            web(shapes);
            text(shapes, label(150, "CASHED OUT", 2, GOLD));
            gold_label_on(
                shapes,
                Point::new(240, 196),
                &format!("+{} GOLD", table.payout),
                2,
                BLACK,
            );
            result(shapes, &table, BLACK);
        }
        Phase::Pounce => pounce(shapes, table.phase_secs),
        Phase::Bitten => {
            shapes.push(rect(0, 0, 466, 466, BLOOD));
            shapes.push(label(150, "GOT YOU!", 3, TEXT));
            if table.stake > 0 {
                shapes.push(label(196, &format!("LOST {} GOLD", table.stake), 1, TEXT));
            }
            result(shapes, &table, BLOOD);
        }
        Phase::Won => {
            web(shapes);
            text(shapes, label(150, "SPIDER TRAPPED!", 2, GOLD));
            text(shapes, label(190, "EVERY SAFE EGG FOUND", 1, TEXT));
            if table.payout > 0 {
                gold_label_on(
                    shapes,
                    Point::new(240, 214),
                    &format!("+{} GOLD", table.payout),
                    1,
                    BLACK,
                );
            }
            result(shapes, &table, BLACK);
        }
    }
    if menu && launcher.is_some() {
        button(shapes, &ui::HOME);
    }
    if let Some(battery) = power.battery
        && (menu || running_low(&battery))
    {
        let backing = if table.phase == Phase::Bitten {
            BLOOD
        } else {
            BLACK
        };
        battery_level_on(shapes, &battery, backing);
    }
    power_overlay(shapes, &power);
}

/// The stakes round a dial (and OUT, when the town launched the game), the
/// pointed one ringed in gold.
fn bet(shapes: &mut Vec<Shape>, pals: &Pals, steer: f32, launched: bool) {
    web(shapes);
    let offered = stakes(pals.gold);
    let spots = bet_spots(&offered, launched);
    let pointed = dial_choice(steer, spots);
    text(shapes, label(196, "YOUR BET?", 2, TEXT));
    gold_label_on(
        shapes,
        Point::new(240, 232),
        &format!("GOLD {}", pals.gold),
        1,
        BLACK,
    );
    let note = match offered.get(pointed) {
        None => "BACK TO TOWN".into(),
        Some(0) => "JUST FOR FUN".into(),
        Some(&stake) => format!("TRAP IT FOR {}", cash_value(stake, (EGGS - 1) as u8)),
    };
    text(shapes, label(262, &note, 1, HINT));
    text(shapes, label(420, "TURN TO PICK, PRESS", 1, HINT));
    for i in 0..spots {
        let at = ui::stake_center(i, spots);
        let at = Point::new(at.x as i32, at.y as i32);
        let (amount, scale) = match offered.get(i) {
            None => ("OUT".into(), 1),
            Some(0) => ("FREE".into(), 1),
            Some(stake) => (format!("{stake}"), 2),
        };
        if i == pointed {
            framed_label(shapes, label_at(at, &amount, scale, GOLD), GOLD, BLACK);
        } else {
            text(shapes, label_at(at, &amount, scale, TEXT));
        }
    }
}

/// The web: three rings and twelve spokes, static after the first frame.
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

/// Text on the web, so its threads stop short of the letters.
fn text(shapes: &mut Vec<Shape>, label: Shape) {
    plated(shapes, label, BLACK);
}

fn result(shapes: &mut Vec<Shape>, table: &Table, backing: Rgb565) {
    let safe = label(240, &format!("SAFE PICKS {}", table.safe), 2, TEXT);
    plated(shapes, safe, backing);
    let best = if table.new_best {
        label(284, "NEW BEST!", 2, GOLD)
    } else {
        label(284, &format!("BEST {}", table.best), 1, HINT)
    };
    plated(shapes, best, backing);
    if table.phase_secs >= RESULT_GUARD_SECS {
        let color = if table.phase == Phase::Won {
            GOLD
        } else {
            DANGER
        };
        plated(shapes, label(340, "PRESS TO PLAY AGAIN", 1, color), backing);
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
