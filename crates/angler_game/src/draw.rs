use alloc::{format, string::String, vec::Vec};

use bevy::{math::ops, prelude::*};
use orbit_core::{Frame, Launcher, Power, Tilt};
use orbit_gfx::{Point, Rectangle, Rgb565, SCREEN_CENTER, Shape, Size, SpriteRef};
use orbit_ui::{
    battery_level, button, clear_text_of, dial_choice, framed_button, key_ring, labelled_button,
    notice, power_overlay, running_low,
};
use pals::{MAX_PETS, Pals};

use crate::{
    audio::GUTEN_SPIN_SECS,
    cast::{self, Flight, scene::*},
    log::FishLog,
    reel::{FISH_HALF, Reel, TRACK_HEIGHT, TRACK_TOP, TRACK_WIDTH},
    session::{Phase, RELEASE_TURN_DEG, RESULT_GUARD_SECS, SWAP_CHOICES, Session},
    settings::Settings,
    species::{Catch, SPECIES},
    sprites::{self, rgb},
    ui,
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
/// The full-power path drawn while aiming.
pub const PREVIEW_DOT: Rgb565 = rgb(240, 240, 240);
const DEEP: Rgb565 = rgb(12, 34, 58);
const TRACK: Rgb565 = rgb(24, 26, 36);
const ZONE: Rgb565 = rgb(80, 190, 90);
const ZONE_HOT: Rgb565 = rgb(130, 235, 120);
const GAUGE_BACK: Rgb565 = rgb(40, 40, 52);
const GAUGE_FILL: Rgb565 = rgb(150, 210, 240);
const GAUGE_PERFECT_ZONE: Rgb565 = rgb(150, 120, 40);
const TEXT: Rgb565 = rgb(255, 255, 255);
const HINT: Rgb565 = rgb(200, 215, 230);
const GOLD: Rgb565 = rgb(255, 215, 70);
const TOAST: Rgb565 = rgb(120, 230, 255);

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, draw_frame);
}

#[allow(clippy::too_many_arguments)]
fn draw_frame(
    session: Res<Session>,
    tilt: Res<Tilt>,
    log: Res<FishLog>,
    settings: Res<Settings>,
    pals: Res<Pals>,
    power: Res<Power>,
    launcher: Option<Res<Launcher>>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    let elevation = cast::elevation_deg(tilt.steer_deg);
    let t = session.phase_secs;
    match &session.phase {
        Phase::Title => {
            lake(shapes, &session, cast::BASE_ELEVATION_DEG, false);
            shapes.push(label(110, "GONE FISHIN", 3, TEXT));
            shapes.push(label(160, "PRESS BUTTON TO FISH", 1, HINT));
            level(shapes, &tilt);
            if launcher.is_some() {
                button(shapes, &ui::HOME);
            }
            button(shapes, &ui::LOG);
            button(shapes, &ui::SETTINGS);
            let caught = format!("CAUGHT {}", log.total);
            shapes.push(label_at(Point::new(170, 412), &caught, 1, TEXT));
            let tank = format!("TANK {}/{MAX_PETS}", pals.pets.len());
            shapes.push(label_at(Point::new(296, 412), &tank, 1, TEXT));
        }
        Phase::Log => fish_log(shapes, &log),
        Phase::Settings => settings_menu(shapes, &settings),
        Phase::Guten => guten(shapes, t),
        Phase::Aim { charging_secs } => {
            lake(shapes, &session, elevation, true);
            shapes.push(label(52, &format!("ANGLE {}", elevation as i32), 1, TEXT));
            preview(shapes, &Flight::launch(elevation, 1.0));
            match charging_secs {
                Some(secs) => power_gauge(
                    shapes,
                    cast::charge_power(*secs),
                    settings.difficulty.tuning().perfect_power,
                ),
                None => {
                    shapes.push(label(76, "HOLD BUTTON TO CAST", 1, HINT));
                    button(shapes, &ui::MENU);
                }
            }
        }
        Phase::Flight { flight, perfect } => {
            lake(shapes, &session, elevation, true);
            bobber(shapes, flight.pos, 0);
            if *perfect {
                notice(shapes, 380, "PERFECT!", GOLD);
            }
        }
        Phase::Waiting { bobber_x, .. } => {
            lake(shapes, &session, elevation, true);
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
            alarm(shapes, Point::new(*bobber_x as i32, WATER_Y as i32 - 40));
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
                shapes.push(label(70, banner, 2, GOLD));
            }
            if t >= RESULT_GUARD_SECS {
                keep_or_release(shapes, &pals, tilt.steer_deg >= RELEASE_TURN_DEG);
            }
        }
        Phase::Swap { .. } => swap(shapes, &pals, dial_choice(tilt.steer_deg, SWAP_CHOICES)),
        Phase::Naming { ring, name, .. } => {
            shapes.push(rect(0, 0, 466, 466, DEEP));
            key_ring(
                shapes,
                ring,
                "NAME YOUR FISH",
                name.as_str(),
                !name.is_empty(),
            );
        }
        Phase::Escaped => {
            shapes.push(rect(0, 0, 466, 466, DEEP));
            shapes.push(label(220, "IT GOT AWAY...", 2, TEXT));
        }
    }
    let menu = matches!(session.phase, Phase::Title | Phase::Log | Phase::Settings);
    let on_the_lake = matches!(
        session.phase,
        Phase::Aim { .. }
            | Phase::Flight { .. }
            | Phase::Waiting { .. }
            | Phase::Bite { .. }
            | Phase::Notice(_)
    );
    if let Some(battery) = power.battery
        && (menu || (on_the_lake && running_low(&battery)))
    {
        battery_level(shapes, &battery);
    }

    clear_text_of(
        shapes,
        2,
        |shape| matches!(shape, Shape::Disc { color, .. } if *color == PREVIEW_DOT),
    );
    if session.toast_secs > 0.0 && power.sleep_progress().is_none() {
        notice(shapes, 140, "LEVEL SET", TOAST);
    }
    power_overlay(shapes, &power);
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

/// A bite's "!" over the bobber, drawn as a mark rather than text since it
/// stands over the hills.
fn alarm(shapes: &mut Vec<Shape>, at: Point) {
    shapes.push(rect(at.x - 3, at.y - 19, 6, 27, GOLD));
    shapes.push(rect(at.x - 3, at.y + 13, 6, 6, GOLD));
}

/// Angry Birds-style dotted path of a full-power cast, ending exactly where
/// the bobber would land. Charging only decides whether a cast reaches it.
fn preview(shapes: &mut Vec<Shape>, flight: &Flight) {
    let landing = Vec2::new(flight.landing_x(), WATER_Y);
    let path = (1..)
        .map(|i| flight.at(i as f32 * 0.09))
        .take_while(|p| p.y < WATER_Y && p.distance(landing) > 8.0)
        .chain([landing]);
    for p in path {
        shapes.push(Shape::Disc {
            center: point(p),
            radius: 2,
            color: PREVIEW_DOT,
        });
    }
}

/// Fills along the lower-left rim. The gold tip is the window that throws at
/// full power; the fill turns gold while the meter is inside it.
fn power_gauge(shapes: &mut Vec<Shape>, power: f32, perfect_power: f32) {
    let arc = |sweep: f32, color| Shape::Arc {
        center: SCREEN_CENTER,
        radius: 212,
        width: 12,
        start_deg: 100.0,
        sweep_deg: sweep,
        color,
    };
    shapes.push(arc(70.0, GAUGE_BACK));
    shapes.push(Shape::Arc {
        center: SCREEN_CENTER,
        radius: 212,
        width: 12,
        start_deg: 100.0 + 70.0 * perfect_power,
        sweep_deg: 70.0 * (1.0 - perfect_power),
        color: GAUGE_PERFECT_ZONE,
    });
    let color = if power >= perfect_power {
        GOLD
    } else {
        GAUGE_FILL
    };
    shapes.push(arc(70.0 * power.max(0.02), color));
}

/// A bubble level on the title screen: shows the IMU working and where zero is.
fn level(shapes: &mut Vec<Shape>, tilt: &Tilt) {
    let center = Point::new(233, 300);
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
    let zone_top = top + (reel.zone_y - reel.zone_height / 2.0) as i32;
    let zone_color = if reel.fish_in_zone() { ZONE_HOT } else { ZONE };
    shapes.push(rect(
        left,
        zone_top,
        TRACK_WIDTH as u32,
        reel.zone_height as u32,
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
    let scale = (260 / size.width).clamp(1, 12);
    let drawn = size * scale;
    shapes.push(Shape::Sprite {
        sprite: SpriteRef(species.sprite),
        top_left: Point::new(233 - drawn.width as i32 / 2, 150 - drawn.height as i32 / 2),
        scale,
        flip_x: false,
    });
    shapes.push(label(236, species.name, 2, TEXT));
    shapes.push(label(268, &length_text(catch.length_mm), 1, TEXT));
    shapes.push(label(290, &weight_text(catch.weight_g), 1, TEXT));
}

/// KEEP and RELEASE, the pointed one framed in gold, and whether keeping
/// means swapping one out.
fn keep_or_release(shapes: &mut Vec<Shape>, pals: &Pals, release_pointed: bool) {
    let (keep, release) = if release_pointed {
        (HINT, GOLD)
    } else {
        (GOLD, HINT)
    };
    framed_button(shapes, &ui::KEEP_FISH, keep);
    framed_button(shapes, &ui::RELEASE_FISH, release);
    let tank = if pals.is_full() {
        String::from("TANK FULL: KEEP SWAPS ONE OUT")
    } else {
        format!("TANK {}/{MAX_PETS}", pals.pets.len())
    };
    shapes.push(label(392, &tank, 1, HINT));
    shapes.push(label(414, "TURN TO PICK, PRESS", 1, HINT));
}

/// The kept fish round the glass and LET IT GO; the pointed one in gold.
fn swap(shapes: &mut Vec<Shape>, pals: &Pals, pointed: usize) {
    shapes.push(rect(0, 0, 466, 466, DEEP));
    shapes.push(label(196, "TANK FULL", 2, TEXT));
    shapes.push(label(230, "WHO GOES BACK?", 1, HINT));
    shapes.push(label(266, "TURN TO PICK, PRESS", 1, HINT));
    for (slot, area) in ui::SWAP_SLOTS.iter().enumerate() {
        let name = pals
            .pets
            .get(slot)
            .map_or(area.text, |pet| pet.name.as_str());
        let frame = if slot == pointed { GOLD } else { HINT };
        labelled_button(shapes, area, name, frame);
    }
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

fn settings_menu(shapes: &mut Vec<Shape>, settings: &Settings) {
    shapes.push(rect(0, 0, 466, 466, DEEP));
    shapes.push(label(80, "SETTINGS", 2, TEXT));
    shapes.push(label(150, "DIFFICULTY", 1, HINT));
    button(shapes, &ui::EASIER);
    button(shapes, &ui::HARDER);
    let difficulty = settings.difficulty;
    shapes.push(label(
        ui::EASIER.area.center().y,
        difficulty.name(),
        2,
        GOLD,
    ));
    for (line, text) in difficulty.blurb().iter().enumerate() {
        if !text.is_empty() {
            shapes.push(label(244 + line as i32 * 24, text, 1, HINT));
        }
    }
    button(shapes, &ui::GUTEN);
    button(shapes, &ui::BACK);
}

/// The easter egg: logic-fish's spinning rainbow trout, faked in 2D by
/// squashing the sprite's width with the cosine of its turn and mirroring it
/// for the far side.
fn guten(shapes: &mut Vec<Shape>, t: f32) {
    let band = 466 / SKY.len() as i32 + 1;
    for (i, color) in SKY.iter().rev().enumerate() {
        shapes.push(rect(0, i as i32 * band, 466, band as u32, *color));
    }
    shapes.push(disc(233, 400, 70, SUN));
    shapes.push(label(92, "GUTEN MORGEN", 2, TEXT));
    let size = sprites::TROUT.size() * 13;
    let turn = ops::cos(t / GUTEN_SPIN_SECS * core::f32::consts::TAU);
    let width = ((size.width as f32 * turn.abs()) as u32).max(2);
    shapes.push(Shape::SpriteFit {
        sprite: SpriteRef(&sprites::TROUT),
        area: Rectangle::new(
            Point::new(233 - width as i32 / 2, 236 - size.height as i32 / 2),
            Size::new(width, size.height),
        ),
        flip_x: turn < 0.0,
    });
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
    label_at(Point::new(233, y), text, scale, color)
}

fn label_at(center: Point, text: &str, scale: u32, color: Rgb565) -> Shape {
    Shape::Label {
        center,
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
