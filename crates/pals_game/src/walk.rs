//! The park: walk a fish to build its stamina. The board counts your steps
//! (`orbit_core::Steps`), [`pals::STRIDE_M`] each. A few seconds into a walk
//! the screen goes dark to save power (`Power::dozing`) and steps keep
//! counting; PWR or BOOT ends it. With MOCK WALK on in the settings, a pretend
//! walker steps instead and the screen stays on for its pace buttons. A walk
//! saves as it goes.

use alloc::{boxed::Box, format, vec::Vec};

use bevy::{app::AppExit, prelude::*};
use orbit_core::{
    Buttons, ControlsPlugin, CorePlugin, Frame, Power, SaveBlob, SoundPlugin, SoundQueue, Steps,
    StepsPlugin, Tilt, Touch,
};
use orbit_gfx::{Point, Rgb565, SCREEN_CENTER, Shape};
use orbit_ui::{
    BLACK, CLICK, DEEP, GOLD, HINT, TEXT, TOAST, battery_level, battery_level_on, button,
    dial_choice, dial_point, disc, label, plated, plated_lines, power_overlay, rect, rgb,
};
use pals::{MOCK_PACES, MockWalk, Pals, STRIDE_M};

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
/// A real walk dozes (screen off) this long after it starts or the glass was
/// last touched.
pub const DOZE_AFTER_SECS: f32 = 5.0;
/// The dozing screen's one line, dim on black.
const DOZE_TEXT: Rgb565 = rgb(60, 70, 80);
const NAME_RING: f32 = 160.0;

pub mod ui {
    pub use orbit_ui::Button;

    /// On a mock walk: slow it down, or fast-forward it.
    pub const SLOWER: Button = Button::new(52, 306, 56, 44, "-");
    pub const FASTER: Button = Button::new(358, 306, 56, 44, "+");
    /// Ends the walk, like PWR.
    pub const STOP: Button = Button::new(188, 410, 90, 30, "STOP");
}

#[derive(Clone, Debug, PartialEq)]
pub enum WalkPhase {
    /// Which fish walks; turning points at one, or BACK.
    Choose,
    Walking {
        pet: usize,
        steps: u32,
        mock: Option<MockWalk>,
        /// Metres walked but not yet credited to the fish (under one).
        fraction_m: f32,
        /// Whole metres credited since the last save.
        unsaved_m: f32,
        secs: f32,
        /// Seconds since the walk started or the glass was touched.
        awake_secs: f32,
        levelled_at: Option<f32>,
        from_level: u32,
    },
    Done {
        pet: usize,
        steps: u32,
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
        app.add_plugins(StepsPlugin)
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
    mut power: ResMut<Power>,
    mut counted: ResMut<Steps>,
    mut walk: ResMut<Walk>,
    mut pals: ResMut<Pals>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    walk.clock += dt;
    let counted = counted.take();
    if power.saying_good_night() {
        return;
    }
    let pressed = buttons.action.pressed();
    let any_button = pressed || buttons.zero.pressed();
    let mut dozing = false;
    let tap = touch.just_pressed().then(|| touch.position()).flatten();
    let mock_walk = pals.mock_walk;
    let mut next = None;
    let mut save = false;
    match walk.phase.as_mut() {
        WalkPhase::Choose => {
            let count = pals.pets.len();
            let pointed = dial_choice(tilt.steer_deg, count + 1);
            let tapped = tap.and_then(|at| {
                let spots = (0..=count).map(|i| choice_point(i, count + 1));
                orbit_ui::nearest(at, spots, fish::DIAL_REACH)
            });
            match tapped.or(pressed.then_some(pointed)) {
                Some(pet) if pet < count => {
                    next = Some(WalkPhase::Walking {
                        pet,
                        steps: 0,
                        mock: mock_walk.then(|| MockWalk::at_pace(MOCK_PACES[pals.mock_pace])),
                        fraction_m: 0.0,
                        unsaved_m: 0.0,
                        secs: 0.0,
                        awake_secs: 0.0,
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
            steps,
            mock,
            fraction_m,
            unsaved_m,
            secs,
            awake_secs,
            levelled_at,
            from_level,
        } => {
            *secs += dt;
            *awake_secs = if touch.position().is_some() {
                0.0
            } else {
                *awake_secs + dt
            };
            dozing = mock.is_none() && *awake_secs >= DOZE_AFTER_SECS;
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
            let new = match mock {
                Some(mock) => mock.step(dt),
                None => counted,
            };
            *steps += new;
            let fish = &mut pals.pets[*pet];
            let before = fish.stamina_level();
            *fraction_m += new as f32 * STRIDE_M;
            let whole = *fraction_m as u32;
            if whole > 0 {
                fish.walk(whole as f32);
                *fraction_m -= whole as f32;
                *unsaved_m += whole as f32;
            }
            if fish.stamina_level() > before {
                *levelled_at = Some(*secs);
                if !dozing {
                    sound.play(&angler_game::audio::CATCH);
                }
            }
            let stop = any_button || tap.is_some_and(|at| ui::STOP.hit(at));
            save |= *unsaved_m >= SAVE_EVERY_M || stop;
            if save {
                *unsaved_m = 0.0;
            }
            if stop {
                dozing = false;
                next = Some(WalkPhase::Done {
                    pet: *pet,
                    steps: *steps,
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
    power.set_dozing(dozing);
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
    tilt: Res<Tilt>,
    power: Res<Power>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    match walk.phase.as_ref() {
        WalkPhase::Choose => choose(shapes, &pals, tilt.steer_deg),
        WalkPhase::Walking { .. } if power.dozing() => {
            shapes.push(rect(0, 0, 466, 466, BLACK));
            shapes.push(label(233, "WALKING. PRESS TO STOP", 1, DOZE_TEXT));
            return;
        }
        WalkPhase::Walking {
            pet,
            steps,
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
            text(shapes, label(318, &format!("{steps} STEPS"), 2, TEXT));
            let metres = (*steps as f32 * STRIDE_M) as u32;
            text(shapes, label(344, &format!("{metres} M"), 1, TEXT));
            let level = format!("STAMINA LV {}", fish.stamina_level());
            let status = match mock {
                Some(mock) => format!("MOCK WALK x{}", mock.pace() as u32),
                None => "SCREEN GOES OFF TO SAVE POWER".into(),
            };
            let lines = [label(368, &level, 1, GOLD), label(392, &status, 1, HINT)];
            plated_lines(shapes, lines, GRASS);
            button(shapes, &ui::STOP);
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
            steps,
            from_level,
        } => {
            let fish = &pals.pets[*pet];
            shapes.push(rect(0, 0, 466, 466, DEEP));
            fish::draw(shapes, fish, Vec2::new(233.0, 140.0), 5, true);
            shapes.push(label(220, &format!("WALKED {steps} STEPS"), 2, TEXT));
            let metres = (*steps as f32 * STRIDE_M) as u32;
            shapes.push(label(248, &format!("{metres} M"), 1, TEXT));
            let now = fish.stamina_level();
            let level = if now > *from_level {
                format!("STAMINA LV {from_level} TO {now}!")
            } else {
                format!("STAMINA LV {now}")
            };
            shapes.push(label(274, &level, 1, GOLD));
            let next = match fish.next_level_m() {
                Some(next) => format!("{} M TO THE NEXT LEVEL", next - fish.walked_m),
                None => "THE TOP LEVEL!".into(),
            };
            shapes.push(label(298, &next, 1, HINT));
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
