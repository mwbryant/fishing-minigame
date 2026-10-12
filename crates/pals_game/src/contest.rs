//! The contest hall, where the big prizes are. Pick a fish and a tier (higher
//! tiers need a higher stamina level), then put on a 30 s show: give taught
//! commands by word or stroke, and the fish performs.
//!
//! A trick scores by rarity and how well it's known, more for changing it up,
//! and double when a judge just called for it. Each try costs stamina, and a
//! spent fish ends its show. The fish's looks (rarity, size, hat) add appeal,
//! and three rivals' scores decide the prize.

use alloc::{boxed::Box, format, string::String, vec::Vec};

use bevy::{app::AppExit, prelude::*};
use orbit_core::{
    Buttons, Clip, ControlsPlugin, CorePlugin, Frame, Mic, MicPlugin, Power, Rng, SaveBlob,
    SoundPlugin, SoundQueue, Tilt, Touch,
};
use orbit_gfx::{Point, Rgb565, SCREEN_CENTER, Shape};
use orbit_ui::{
    CLICK, DANGER, DEEP, GOLD, HINT, TEXT, THUD, TOAST, battery_level_on, clear_text_of,
    dial_choice, dial_point, gold_label_on, label, label_at, list_choice, plated, power_overlay,
    rect, rgb,
};
use pals::{Gesture, Pals, Pet, Strokes, TRICKS_PER_PET};

use crate::fish::{self, TRICK_SECS, pose};

pub const BACKGROUND: Rgb565 = CURTAIN;
const CURTAIN: Rgb565 = rgb(110, 30, 50);
const STAGE_WATER: Rgb565 = rgb(40, 100, 170);
const SPOT: Rgb565 = rgb(70, 140, 210);
const BAR_BACK: Rgb565 = rgb(40, 40, 52);
pub const SHOW_SECS: f32 = 30.0;
/// "GO!" shows this long at the start.
const GO_SECS: f32 = 1.5;
/// Seconds from one judge's call ending to the next (random in this range),
/// and how long a call stands.
const REQUEST_GAP_SECS: (f32, f32) = (4.0, 7.0);
const REQUEST_SECS: f32 = 4.0;
/// Each change of trick in a row adds this percent to a trick's score, up to
/// `MAX_COMBO` changes.
const COMBO_PCT: u32 = 10;
const MAX_COMBO: u32 = 5;
/// The mic hears the speaker: words this soon after a sound are ignored.
const ECHO_SECS: f32 = 1.0;
/// Where the fish performs, low enough that its highest leap stays under the
/// judge's banner and its deepest dive over the line of news under the water.
const STAGE: Vec2 = Vec2::new(233.0, 262.0);
/// Tricks move this much of their full size here, to fit the stage.
const STAGE_MOTION: f32 = 0.7;
/// The banner a judge's request (or the start) goes up on, wide enough for
/// the longest trick's name.
const BANNER: (i32, i32, u32, u32) = (88, 76, 290, 58);
const NAME_RING: f32 = 160.0;

pub struct Tier {
    pub name: &'static str,
    pub min_level: u32,
    /// Gold for first, second and third.
    pub prizes: [u32; 3],
    /// The range the rivals' scores fall in.
    pub rivals: (u32, u32),
    /// The judges weigh looks this many times over.
    pub appeal_times: u32,
}

pub const TIERS: [Tier; 4] = [
    Tier {
        name: "BEGINNER",
        min_level: 1,
        prizes: [40, 20, 10],
        rivals: (40, 110),
        appeal_times: 1,
    },
    Tier {
        name: "OPEN",
        min_level: 3,
        prizes: [100, 50, 20],
        rivals: (130, 280),
        appeal_times: 1,
    },
    Tier {
        name: "EXPERT",
        min_level: 6,
        prizes: [220, 110, 45],
        rivals: (420, 800),
        appeal_times: 2,
    },
    Tier {
        name: "MASTER",
        min_level: 10,
        prizes: [500, 250, 100],
        rivals: (900, 1500),
        appeal_times: 3,
    },
];

pub const RIVALS: [&str; 3] = ["SPLASHY", "FINLEY", "OLD BOOTS"];
/// The tier list: the four, then BACK.
const TIER_CHOICES: usize = TIERS.len() + 1;

pub mod ui {
    pub use orbit_ui::Button;

    /// The tier list's rows (drawn by `choose_tier`), then BACK under them.
    pub const TIER_BUTTONS: [Button; 5] = [
        Button::new(83, 106, 300, 44, ""),
        Button::new(83, 158, 300, 44, ""),
        Button::new(83, 210, 300, 44, ""),
        Button::new(83, 262, 300, 44, ""),
        Button::new(183, 330, 100, 32, ""),
    ];
}

/// A show in progress.
#[derive(Clone, Debug, PartialEq)]
pub struct Show {
    pub score: u32,
    pub stamina: u32,
    secs: f32,
    combo: u32,
    last_trick: Option<usize>,
    /// The trick being performed, whether it'll land, and since when.
    trying: Option<(usize, bool, f32)>,
    /// The trick a judge wants, until when.
    request: Option<(usize, f32)>,
    next_request: f32,
    rivals: [u32; 3],
    toast: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ContestPhase {
    ChoosePet,
    ChooseTier {
        pet: usize,
    },
    Show {
        pet: usize,
        tier: usize,
        show: Show,
    },
    Results {
        pet: usize,
        tier: usize,
        score: u32,
        rivals: [u32; 3],
        /// 0 first .. 3 out of the prizes.
        place: usize,
        tired: bool,
    },
}

/// The phase is boxed to keep the resource small (Bevy's resource storage
/// costs many times a resource's size in heap).
#[derive(Resource, Debug)]
pub struct Contest {
    pub phase: Box<ContestPhase>,
    clock: f32,
    strokes: Strokes,
    deaf_until: f32,
    rigged_try: Option<bool>,
    rigged_rivals: Option<[u32; 3]>,
}

impl Default for Contest {
    fn default() -> Self {
        Self {
            phase: Box::new(ContestPhase::ChoosePet),
            clock: 0.0,
            strokes: Strokes::default(),
            deaf_until: 0.0,
            rigged_try: None,
            rigged_rivals: None,
        }
    }
}

impl Contest {
    /// Makes the next try succeed or flub, for tests.
    pub fn rig_next_try(&mut self, success: bool) {
        self.rigged_try = Some(success);
    }

    /// Sets the rivals' scores for this show, for tests.
    pub fn rig_rivals(&mut self, scores: [u32; 3]) {
        if let ContestPhase::Show { show, .. } = self.phase.as_mut() {
            show.rivals = scores;
        }
        self.rigged_rivals = Some(scores);
    }

    /// Has a judge call for the trick in `slot` now, for tests.
    pub fn rig_request(&mut self, slot: usize) {
        if let ContestPhase::Show { show, .. } = self.phase.as_mut() {
            show.request = Some((slot, show.secs + REQUEST_SECS));
        }
    }
}

pub struct ContestPlugin;

impl Plugin for ContestPlugin {
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
        app.add_plugins(MicPlugin)
            .init_resource::<Contest>()
            .init_resource::<Pals>()
            .add_systems(Startup, arrive)
            .add_systems(Update, perform)
            .add_systems(PostUpdate, draw_frame);
    }
}

fn arrive(blob: Res<SaveBlob>, mut pals: ResMut<Pals>, mut tilt: ResMut<Tilt>) {
    *pals = pals::save::load(&blob);
    tilt.request_zero();
}

fn in_range(rng: &mut Rng, (low, high): (u32, u32)) -> u32 {
    low + (rng.next_f32() * (high - low) as f32) as u32
}

fn request_gap(rng: &mut Rng) -> f32 {
    let (low, high) = REQUEST_GAP_SECS;
    low + rng.next_f32() * (high - low)
}

/// What a performance of the trick in `slot` scores.
fn points(pet: &Pet, slot: usize, requested: bool, combo: u32) -> u32 {
    let trick = &pet.tricks[slot];
    let base = trick.info().rarity.points() * (100 + u32::from(trick.learned)) / 100;
    let base = base * (100 + COMBO_PCT * combo) / 100;
    if requested { base * 2 } else { base }
}

#[allow(clippy::too_many_arguments)]
fn perform(
    time: Res<Time>,
    buttons: Res<Buttons>,
    mut tilt: ResMut<Tilt>,
    touch: Res<Touch>,
    power: Res<Power>,
    mut mic: ResMut<Mic>,
    mut contest: ResMut<Contest>,
    mut pals: ResMut<Pals>,
    mut rng: ResMut<Rng>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    let contest: &mut Contest = &mut contest;
    let dt = time.delta_secs();
    contest.clock += dt;
    let heard = mic.take().filter(|_| contest.clock >= contest.deaf_until);
    let in_show = matches!(*contest.phase, ContestPhase::Show { .. });
    let stroke = if in_show {
        contest.strokes.update(touch.position(), dt)
    } else {
        None
    };
    if power.saying_good_night() {
        return;
    }
    let pressed = buttons.action.pressed();
    let tap = touch.just_pressed().then(|| touch.position()).flatten();
    let mut sfx: Option<&'static Clip> = None;
    let mut next = None;
    match contest.phase.as_mut() {
        ContestPhase::ChoosePet => {
            let count = pals.pets.len();
            let pointed = dial_choice(tilt.steer_deg, count + 1);
            let tapped = tap.and_then(|at| {
                let spots = (0..=count).map(|i| choice_point(i, count + 1));
                orbit_ui::nearest(at, spots, fish::DIAL_REACH)
            });
            match tapped.or(pressed.then_some(pointed)) {
                Some(pet) if pet < count => next = Some(ContestPhase::ChooseTier { pet }),
                Some(_) => {
                    sound.play(&CLICK);
                    exit.write(AppExit::Success);
                    return;
                }
                None => {}
            }
        }
        ContestPhase::ChooseTier { pet } => {
            let pet = *pet;
            let tapped = tap.and_then(|at| {
                let row = orbit_ui::tapped(at, &ui::TIER_BUTTONS)?;
                ui::TIER_BUTTONS.iter().position(|button| button == row)
            });
            let picked = tapped.or(pressed.then(|| list_choice(tilt.steer_deg, TIER_CHOICES, 0)));
            if let Some(pointed) = picked {
                let level = pals.pets[pet].stamina_level();
                next = match TIERS.get(pointed) {
                    Some(tier) if level >= tier.min_level => {
                        let rivals = contest.rigged_rivals.unwrap_or([
                            in_range(&mut rng, tier.rivals),
                            in_range(&mut rng, tier.rivals),
                            in_range(&mut rng, tier.rivals),
                        ]);
                        Some(ContestPhase::Show {
                            pet,
                            tier: pointed,
                            show: Show {
                                score: 0,
                                stamina: pals.pets[pet].stamina_max(),
                                secs: 0.0,
                                combo: 0,
                                last_trick: None,
                                trying: None,
                                request: None,
                                next_request: request_gap(&mut rng),
                                rivals,
                                toast: None,
                            },
                        })
                    }
                    Some(_) => {
                        sfx = Some(&THUD);
                        None
                    }
                    None => Some(ContestPhase::ChoosePet),
                };
            }
        }
        ContestPhase::Show { pet, tier, show } => {
            let command = match show.trying {
                Some(_) => None,
                None => heard
                    .map(|print| pals.pets[*pet].hear(&print))
                    .or(stroke.map(|gesture: Gesture| pals.pets[*pet].answer_gesture(gesture))),
            };
            let fish = &pals.pets[*pet];
            show.secs += dt;
            let now = show.secs;
            if show.request.is_some_and(|(_, until)| now >= until) {
                show.request = None;
            }
            if show.request.is_none() && now >= show.next_request {
                let taught: Vec<usize> = (0..TRICKS_PER_PET)
                    .filter(|&slot| fish.tricks[slot].taught())
                    .collect();
                if !taught.is_empty() {
                    let slot = taught[rng.next_u32() as usize % taught.len()];
                    show.request = Some((slot, now + REQUEST_SECS));
                }
                show.next_request = now + REQUEST_SECS + request_gap(&mut rng);
            }
            match show.trying {
                Some((slot, success, since)) => {
                    if now >= since + TRICK_SECS {
                        show.trying = None;
                        if success {
                            let requested = show.request.is_some_and(|(want, _)| want == slot);
                            if show.last_trick.is_some_and(|last| last != slot) {
                                show.combo = (show.combo + 1).min(MAX_COMBO);
                            } else {
                                show.combo = 0;
                            }
                            let gained = points(fish, slot, requested, show.combo);
                            show.score += gained;
                            show.last_trick = Some(slot);
                            show.toast = Some(if requested {
                                show.request = None;
                                format!("THE JUDGES LOVE IT! +{gained}")
                            } else {
                                format!("+{gained}")
                            });
                            sfx = Some(&angler_game::audio::CATCH);
                        } else {
                            show.toast = Some(String::from("FLUBBED IT"));
                            show.combo = 0;
                        }
                    }
                }
                None => match command {
                    Some(Some(slot)) => {
                        let chance = fish.tricks[slot].success_chance();
                        let success = contest.rigged_try.take().unwrap_or(rng.next_f32() < chance);
                        let cost = fish.tricks[slot].info().rarity.stamina();
                        show.stamina = show.stamina.saturating_sub(cost);
                        show.trying = Some((slot, success, now));
                        sfx = Some(&angler_game::audio::PERFECT_CAST);
                    }
                    Some(None) => show.toast = Some(String::from("?")),
                    None => {}
                },
            }
            let tired = show.stamina == 0 && show.trying.is_none();
            let over = show.secs >= SHOW_SECS && show.trying.is_none();
            if tired || over {
                let tier_info = &TIERS[*tier];
                let score = show.score + fish.appeal() * tier_info.appeal_times;
                let place = show.rivals.iter().filter(|&&rival| rival > score).count();
                if let Some(prize) = tier_info.prizes.get(place) {
                    pals.earn(*prize);
                }
                pals::save::store(&mut blob, &pals);
                sfx = Some(if place == 0 {
                    &angler_game::audio::CATCH
                } else {
                    &angler_game::audio::ESCAPE
                });
                next = Some(ContestPhase::Results {
                    pet: *pet,
                    tier: *tier,
                    score,
                    rivals: show.rivals,
                    place,
                    tired,
                });
            }
        }
        ContestPhase::Results { .. } => {
            if pressed || tap.is_some() {
                next = Some(ContestPhase::ChoosePet);
            }
        }
    }
    let clicked = next.is_some() && (pressed || tap.is_some());
    if let Some(clip) = sfx.or(clicked.then_some(&CLICK)) {
        sound.play(clip);
        contest.deaf_until = contest.clock + ECHO_SECS;
    }
    if let Some(phase) = next {
        *contest.phase = phase;
        contest.strokes = Strokes::default();
        tilt.request_zero();
    }
}

fn choice_point(index: usize, count: usize) -> Vec2 {
    let at = dial_point(index as f32 * 360.0 / count as f32, NAME_RING);
    Vec2::new(at.x as f32, at.y as f32)
}

fn ordinal(place: usize) -> &'static str {
    ["1ST", "2ND", "3RD", "4TH"][place.min(3)]
}

fn draw_frame(
    contest: Res<Contest>,
    pals: Res<Pals>,
    tilt: Res<Tilt>,
    mic: Res<Mic>,
    power: Res<Power>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    match contest.phase.as_ref() {
        ContestPhase::ChoosePet => choose_pet(shapes, &pals, tilt.steer_deg),
        ContestPhase::ChooseTier { pet } => choose_tier(
            shapes,
            &pals,
            &pals.pets[*pet],
            list_choice(tilt.steer_deg, TIER_CHOICES, 0),
        ),
        ContestPhase::Show { pet, show, .. } => {
            stage(shapes, &pals.pets[*pet], show, mic.speaking());
        }
        ContestPhase::Results {
            pet,
            tier,
            score,
            rivals,
            place,
            tired,
        } => results(
            shapes,
            &pals.pets[*pet],
            *tier,
            *score,
            rivals,
            *place,
            *tired,
        ),
    }
    if let Some(battery) = power.battery {
        battery_level_on(shapes, &battery, CURTAIN);
    }
    clear_text_of(
        shapes,
        2,
        |shape| matches!(shape, Shape::Disc { color, .. } if *color == fish::SPRAY),
    );
    power_overlay(shapes, &power);
}

fn curtains(shapes: &mut Vec<Shape>) {
    shapes.push(rect(0, 0, 466, 466, CURTAIN));
    for x in (20..466).step_by(48) {
        shapes.push(rect(x, 0, 6, 466, rgb(90, 22, 40)));
    }
}

fn choose_pet(shapes: &mut Vec<Shape>, pals: &Pals, steer: f32) {
    curtains(shapes);
    let count = pals.pets.len() + 1;
    if pals.pets.is_empty() {
        text(shapes, label(170, "NO FISH TO ENTER", 2, TEXT));
        text(shapes, label(200, "KEEP ONE FROM THE POND", 1, HINT));
    } else {
        text(shapes, label(170, "CONTEST HALL", 2, GOLD));
        text(shapes, label(200, "WHO WILL PERFORM?", 1, TEXT));
    }
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
        CURTAIN,
    );
    gold_label_on(
        shapes,
        Point::new(240, 300),
        &format!("GOLD {}", pals.gold),
        1,
        CURTAIN,
    );
}

/// Text on the curtains, so their folds stop short of its letters.
fn text(shapes: &mut Vec<Shape>, label: Shape) {
    plated(shapes, label, CURTAIN);
}

fn choose_tier(shapes: &mut Vec<Shape>, pals: &Pals, pet: &Pet, pointed: usize) {
    curtains(shapes);
    gold_label_on(
        shapes,
        Point::new(240, 52),
        &format!("GOLD {}", pals.gold),
        1,
        CURTAIN,
    );
    text(
        shapes,
        label(
            84,
            &format!("{} IS LV {}", pet.name.as_str(), pet.stamina_level()),
            1,
            TEXT,
        ),
    );
    let level = pet.stamina_level();
    for (i, tier) in TIERS.iter().enumerate() {
        let row = ui::TIER_BUTTONS[i].area;
        let (left, top) = (row.top_left.x, row.top_left.y);
        let y = top + row.size.height as i32 / 2;
        let open = level >= tier.min_level;
        let frame = if i == pointed { GOLD } else { CURTAIN };
        shapes.push(rect(left, top, row.size.width, row.size.height, frame));
        shapes.push(rect(
            left + 2,
            top + 2,
            row.size.width - 4,
            row.size.height - 4,
            DEEP,
        ));
        shapes.push(label_at(
            Point::new(170, y),
            tier.name,
            1,
            if open { TEXT } else { HINT },
        ));
        let note = if open {
            format!("1ST WINS {}", tier.prizes[0])
        } else {
            format!("NEEDS LV {}", tier.min_level)
        };
        shapes.push(label_at(
            Point::new(300, y),
            &note,
            1,
            if open { GOLD } else { DANGER },
        ));
    }
    let back_color = if pointed == TIERS.len() { GOLD } else { TEXT };
    text(shapes, label(346, "BACK", 1, back_color));
    text(shapes, label(400, "TURN TO PICK, PRESS", 1, HINT));
}

fn stage(shapes: &mut Vec<Shape>, pet: &Pet, show: &Show, listening: bool) {
    curtains(shapes);
    shapes.push(Shape::Disc {
        center: Point::new(STAGE.x as i32, STAGE.y as i32),
        radius: 128,
        color: STAGE_WATER,
    });
    shapes.push(Shape::Disc {
        center: Point::new(STAGE.x as i32, STAGE.y as i32),
        radius: 80,
        color: SPOT,
    });
    let shown = show.trying.map(|(slot, success, since)| {
        let motion = if success {
            pet.tricks[slot].info().motion
        } else {
            pals::tricks::Motion::Wiggle
        };
        (motion, show.secs - since)
    });
    let (offset, flipped) = shown.map_or((Vec2::ZERO, false), |(motion, t)| pose(motion, t));
    fish::draw(shapes, pet, STAGE + offset * STAGE_MOTION, 5, !flipped);
    if let Some((motion, t)) = shown {
        fish::effects(shapes, motion, t, STAGE, !flipped, STAGE_MOTION);
    }
    text(shapes, label(54, &format!("SCORE {}", show.score), 2, GOLD));
    let (x, y, width, height) = BANNER;
    if let Some((slot, _)) = show.request {
        let want = format!("{}!", pet.tricks[slot].info().name);
        shapes.push(rect(x, y, width, height, GOLD));
        shapes.push(label(y + 16, "A JUDGE WANTS", 1, DEEP));
        shapes.push(label(y + 40, &want, 2, DEEP));
    } else if show.secs < GO_SECS {
        shapes.push(rect(x, y, width, height, GOLD));
        shapes.push(label(y + height as i32 / 2, "GO!", 3, DEEP));
    }
    let news = match (listening, &show.toast) {
        (true, _) => Some(("LISTENING...", TOAST)),
        (false, Some(toast)) => Some((toast.as_str(), TOAST)),
        (false, None) => None,
    };
    if let Some((news, color)) = news {
        text(shapes, label(404, news, 1, color));
    }
    let left = (1.0 - show.secs / SHOW_SECS).clamp(0.0, 1.0);
    shapes.push(Shape::Arc {
        center: SCREEN_CENTER,
        radius: 224,
        width: 6,
        start_deg: -90.0,
        sweep_deg: 360.0 * left.max(0.01),
        color: GOLD,
    });
    let full = pet.stamina_max().max(1);
    let width = 160 * show.stamina / full;
    shapes.push(rect(153, 418, 160, 10, BAR_BACK));
    shapes.push(rect(153, 418, width.max(1), 10, rgb(110, 220, 90)));
    text(shapes, label(440, "STAMINA", 1, HINT));
}

#[allow(clippy::too_many_arguments)]
fn results(
    shapes: &mut Vec<Shape>,
    pet: &Pet,
    tier: usize,
    score: u32,
    rivals: &[u32; 3],
    place: usize,
    tired: bool,
) {
    curtains(shapes);
    let title = if place == 0 {
        String::from("1ST PLACE!")
    } else {
        format!("{} PLACE", ordinal(place))
    };
    text(shapes, label(80, &title, 3, GOLD));
    if tired {
        text(shapes, label(118, "TOO TIRED TO GO ON", 1, HINT));
    }
    let prize = TIERS[tier].prizes.get(place).copied().unwrap_or(0);
    if prize > 0 {
        gold_label_on(
            shapes,
            Point::new(240, 146),
            &format!("+{prize} GOLD"),
            2,
            CURTAIN,
        );
    }
    let mut table: Vec<(&str, u32, bool)> = RIVALS
        .iter()
        .zip(rivals)
        .map(|(name, score)| (*name, *score, false))
        .collect();
    table.push((pet.name.as_str(), score, true));
    table.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)));
    for (row, (name, points, yours)) in table.iter().enumerate() {
        let y = 196 + row as i32 * 34;
        let color = if *yours { GOLD } else { TEXT };
        text(shapes, label_at(Point::new(170, y), name, 1, color));
        text(
            shapes,
            label_at(Point::new(310, y), &format!("{points}"), 1, color),
        );
    }
    text(shapes, label(400, "PRESS TO GO ON", 1, HINT));
}
