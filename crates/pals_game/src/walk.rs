//! The park: take a fish for a walk to build its stamina. The -G board's
//! satellite receiver counts the metres you really walk (not standing
//! still, and not riding in a car); with MOCK WALK on in the settings, a
//! pretend walker strolls at walking pace instead. Each walk saves as it
//! goes.

use alloc::{boxed::Box, format, vec::Vec};

use bevy::{app::AppExit, prelude::*};
use orbit_core::{
    Buttons, ControlsPlugin, CorePlugin, Frame, Gps, GpsPlugin, Power, SaveBlob, SoundPlugin,
    SoundQueue, Tilt, Touch,
};
use orbit_gfx::{Point, Rgb565, SCREEN_CENTER, Shape};
use orbit_ui::{
    CLICK, DEEP, GOLD, HINT, TEXT, TOAST, battery_level, battery_level_on, button, dial_choice,
    dial_point, disc, label, plated, plated_lines, power_overlay, rect, rgb,
};
use pals::{MOCK_PACES, MockWalk, Pals, Trail};

use crate::fish;

pub const BACKGROUND: Rgb565 = GRASS;
const GRASS: Rgb565 = rgb(78, 150, 84);
const PATH: Rgb565 = rgb(206, 186, 136);
const BOWL: Rgb565 = rgb(190, 225, 245);
const BOWL_WATER: Rgb565 = rgb(70, 140, 205);
/// Progress saves this often, so a flat battery loses little of a walk.
const SAVE_EVERY_M: f32 = 25.0;
/// How long "LEVEL UP!" shows.
const LEVEL_UP_SECS: f32 = 10.0;
/// Without a fix this long, the walk suggests a mock walk.
pub const NO_SKY_SECS: f32 = 60.0;
const NAME_RING: f32 = 160.0;

pub mod ui {
    pub use orbit_ui::Button;

    /// On a mock walk: fast-forward it, or slow it back down.
    pub const SLOWER: Button = Button::new(52, 306, 56, 44, "-");
    pub const FASTER: Button = Button::new(358, 306, 56, 44, "+");
}

#[derive(Clone, Debug, PartialEq)]
pub enum WalkPhase {
    /// Which fish walks; turning points at one, or BACK.
    Choose,
    Walking {
        pet: usize,
        trail: Trail,
        mock: Option<MockWalk>,
        /// Metres walked but not yet whole, and since the last save.
        fraction_m: f32,
        unsaved_m: f32,
        secs: f32,
        levelled_at: Option<f32>,
        from_level: u32,
    },
    Done {
        pet: usize,
        meters: u32,
        from_level: u32,
    },
}

/// The phase is boxed to keep the resource small (Bevy's resource storage
/// costs many times a resource's size in heap).
#[derive(Resource, Debug)]
pub struct Walk {
    pub phase: Box<WalkPhase>,
    clock: f32,
}

impl Default for Walk {
    fn default() -> Self {
        Self {
            phase: Box::new(WalkPhase::Choose),
            clock: 0.0,
        }
    }
}

pub struct WalkPlugin;

impl Plugin for WalkPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<CorePlugin>() {
            app.add_plugins(CorePlugin);
        }
        if !app.is_plugin_added::<ControlsPlugin>() {
            app.add_plugins(ControlsPlugin);
        }
        if !app.is_plugin_added::<SoundPlugin>() {
            app.add_plugins(SoundPlugin);
        }
        app.add_plugins(GpsPlugin)
            .init_resource::<Walk>()
            .init_resource::<Pals>()
            .add_systems(Startup, arrive)
            .add_systems(Update, stroll)
            .add_systems(PostUpdate, draw_frame);
    }
}

fn arrive(blob: Res<SaveBlob>, mut pals: ResMut<Pals>, mut tilt: ResMut<Tilt>) {
    *pals = pals::save::load(&blob);
    tilt.request_zero();
}

#[allow(clippy::too_many_arguments)]
fn stroll(
    time: Res<Time>,
    buttons: Res<Buttons>,
    mut tilt: ResMut<Tilt>,
    touch: Res<Touch>,
    power: Res<Power>,
    mut gps: ResMut<Gps>,
    mut walk: ResMut<Walk>,
    mut pals: ResMut<Pals>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    walk.clock += dt;
    let fix = gps.take();
    if power.saying_good_night() {
        return;
    }
    let pressed = buttons.action.pressed();
    let tap = touch.just_pressed().then(|| touch.position()).flatten();
    let mock_walk = pals.mock_walk;
    let mut next = None;
    let mut save = false;
    match walk.phase.as_mut() {
        WalkPhase::Choose => {
            let count = pals.pets.len();
            let pointed = dial_choice(tilt.steer_deg, count + 1);
            let tapped = tap.and_then(|at| {
                (0..=count).find(|&i| choice_point(i, count + 1).distance(at) < 40.0)
            });
            match tapped.or(pressed.then_some(pointed)) {
                Some(pet) if pet < count => {
                    next = Some(WalkPhase::Walking {
                        pet,
                        trail: Trail::default(),
                        mock: mock_walk.then(|| MockWalk::at_pace(MOCK_PACES[pals.mock_pace])),
                        fraction_m: 0.0,
                        unsaved_m: 0.0,
                        secs: 0.0,
                        levelled_at: None,
                        from_level: pals.pets[pet].stamina_level(),
                    });
                }
                Some(_) => {
                    sound.play(&CLICK);
                    exit.write(AppExit::Success);
                }
                None => {}
            }
        }
        WalkPhase::Walking {
            pet,
            trail,
            mock,
            fraction_m,
            unsaved_m,
            secs,
            levelled_at,
            from_level,
        } => {
            *secs += dt;
            if let Some(mock) = mock.as_mut() {
                let faster = tap.is_some_and(|at| ui::FASTER.hit(at));
                let slower = tap.is_some_and(|at| ui::SLOWER.hit(at));
                let pace = match (faster, slower) {
                    (true, _) => (pals.mock_pace + 1).min(MOCK_PACES.len() - 1),
                    (_, true) => pals.mock_pace.saturating_sub(1),
                    _ => pals.mock_pace,
                };
                if pace != pals.mock_pace {
                    pals.mock_pace = pace;
                    mock.set_pace(MOCK_PACES[pace]);
                    save = true;
                    sound.play(&CLICK);
                }
            }
            let fix = match mock {
                Some(mock) => mock.step(dt),
                None => fix,
            };
            let gained = fix.map_or(0.0, |fix| trail.step(fix));
            let fish = &mut pals.pets[*pet];
            let before = fish.stamina_level();
            *fraction_m += gained;
            let whole = *fraction_m as u32;
            if whole > 0 {
                fish.walk(whole as f32);
                *fraction_m -= whole as f32;
                *unsaved_m += whole as f32;
            }
            if fish.stamina_level() > before {
                *levelled_at = Some(*secs);
                sound.play(&angler_game::audio::CATCH);
            }
            save |= *unsaved_m >= SAVE_EVERY_M || pressed;
            if save {
                *unsaved_m = 0.0;
            }
            if pressed {
                next = Some(WalkPhase::Done {
                    pet: *pet,
                    meters: trail.meters as u32,
                    from_level: *from_level,
                });
            }
        }
        WalkPhase::Done { .. } => {
            if pressed || tap.is_some() {
                next = Some(WalkPhase::Choose);
            }
        }
    }
    if save {
        pals::save::store(&mut blob, &pals);
    }
    if let Some(phase) = next {
        sound.play(&CLICK);
        *walk.phase = phase;
        tilt.request_zero();
    }
}

/// Where choice `index` of `count` sits on the rim.
fn choice_point(index: usize, count: usize) -> Vec2 {
    let at = dial_point(index as f32 * 360.0 / count as f32, NAME_RING);
    Vec2::new(at.x as f32, at.y as f32)
}

fn draw_frame(
    walk: Res<Walk>,
    pals: Res<Pals>,
    gps: Res<Gps>,
    tilt: Res<Tilt>,
    power: Res<Power>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    match walk.phase.as_ref() {
        WalkPhase::Choose => choose(shapes, &pals, tilt.steer_deg),
        WalkPhase::Walking {
            pet,
            trail,
            mock,
            secs,
            levelled_at,
            ..
        } => {
            let fish = &pals.pets[*pet];
            park(shapes);
            bowl(shapes, fish, walk.clock);
            progress(shapes, fish);
            text(shapes, label(66, fish.name.as_str(), 2, TEXT));
            text(
                shapes,
                label(330, &format!("{} M", trail.meters as u32), 3, TEXT),
            );
            let level = format!("STAMINA LV {}", fish.stamina_level());
            let status = match (mock, gps.latest()) {
                (Some(mock), _) => format!("MOCK WALK x{}", mock.pace() as u32),
                (None, Some(_)) => format!("SATELLITES {}", gps.satellites),
                (None, None) if *secs >= NO_SKY_SECS => "NO SKY? TRY MOCK WALK".into(),
                (None, None) => "LOOKING FOR THE SKY".into(),
            };
            let lines = [
                label(368, &level, 1, GOLD),
                label(392, &status, 1, HINT),
                label(420, "PRESS TO STOP", 1, HINT),
            ];
            plated_lines(shapes, lines, GRASS);
            if levelled_at.is_some_and(|at| secs - at < LEVEL_UP_SECS) {
                text(shapes, label(104, "LEVEL UP!", 2, GOLD));
            }
            if mock.is_some() {
                button(shapes, &ui::SLOWER);
                button(shapes, &ui::FASTER);
            }
        }
        WalkPhase::Done {
            pet,
            meters,
            from_level,
        } => {
            let fish = &pals.pets[*pet];
            shapes.push(rect(0, 0, 466, 466, DEEP));
            fish::draw(shapes, fish, Vec2::new(233.0, 150.0), 5, true);
            shapes.push(label(230, &format!("WALKED {meters} M"), 2, TEXT));
            let now = fish.stamina_level();
            let level = if now > *from_level {
                format!("STAMINA LV {from_level} TO {now}!")
            } else {
                format!("STAMINA LV {now}")
            };
            shapes.push(label(270, &level, 1, GOLD));
            let next = match fish.next_level_m() {
                Some(next) => format!("{} M TO THE NEXT LEVEL", next - fish.walked_m),
                None => "THE TOP LEVEL!".into(),
            };
            shapes.push(label(296, &next, 1, HINT));
            shapes.push(label(400, "PRESS TO GO ON", 1, HINT));
        }
    }
    match (power.battery, walk.phase.as_ref()) {
        (Some(battery), WalkPhase::Done { .. }) => battery_level(shapes, &battery),
        (Some(battery), _) => battery_level_on(shapes, &battery, GRASS),
        (None, _) => {}
    }
    power_overlay(shapes, &power);
}

fn choose(shapes: &mut Vec<Shape>, pals: &Pals, steer: f32) {
    park(shapes);
    let count = pals.pets.len() + 1;
    let (title, hint) = if pals.pets.is_empty() {
        ("NO FISH TO WALK", "KEEP ONE FROM THE POND")
    } else {
        ("WHO IS WALKING?", "TURN TO PICK, PRESS")
    };
    let lines = [label(170, title, 2, TEXT), label(200, hint, 1, HINT)];
    plated_lines(shapes, lines, GRASS);
    let spots = (0..count).map(|i| {
        let at = choice_point(i, count);
        Point::new(at.x as i32, at.y as i32)
    });
    fish::dial(
        shapes,
        &pals.pets,
        "BACK",
        spots,
        dial_choice(steer, count),
        GRASS,
    );
}

/// Text on the park's grass, so the path stops short of its letters.
fn text(shapes: &mut Vec<Shape>, label: Shape) {
    plated(shapes, label, GRASS);
}

fn park(shapes: &mut Vec<Shape>) {
    shapes.push(rect(0, 0, 466, 466, GRASS));
    shapes.push(rect(213, 0, 40, 466, PATH));
}

/// The fish in a round bowl on wheels, bobbing along.
fn bowl(shapes: &mut Vec<Shape>, pet: &pals::Pet, clock: f32) {
    let bob = (bevy::math::ops::sin(clock * 6.0) * 3.0) as i32;
    let center = Point::new(233, 210 + bob);
    shapes.push(disc(center.x, center.y, 74, BOWL));
    shapes.push(disc(center.x, center.y + 6, 66, BOWL_WATER));
    fish::draw(
        shapes,
        pet,
        Vec2::new(center.x as f32, center.y as f32 + 8.0),
        3,
        true,
    );
    for dx in [-40, 40] {
        shapes.push(disc(center.x + dx, 292, 10, rgb(60, 60, 70)));
    }
}

/// How far to the next stamina level, around the rim.
fn progress(shapes: &mut Vec<Shape>, pet: &pals::Pet) {
    let level = pet.stamina_level();
    let from = (level - 1) * (level - 1) * 100;
    let done = match pet.next_level_m() {
        Some(to) => pet.walked_m.saturating_sub(from) as f32 / (to - from).max(1) as f32,
        None => 1.0,
    };
    shapes.push(Shape::Arc {
        center: SCREEN_CENTER,
        radius: 224,
        width: 8,
        start_deg: -90.0,
        sweep_deg: 360.0 * done.clamp(0.01, 1.0),
        color: TOAST,
    });
}
