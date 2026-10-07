//! The hat shop: every hat rings the glass, turning points at one, and the
//! mirror in the middle shows your fish wearing it. PWR buys it (or puts on
//! one you own); NO HAT takes it off; tap the fish to dress the next one.
//! A hat bought is every fish's to wear.

use alloc::{format, vec::Vec};

use bevy::{app::AppExit, prelude::*};
use orbit_core::{
    Buttons, Clip, ControlsPlugin, CorePlugin, Frame, Power, SaveBlob, SoundPlugin, SoundQueue,
    Tilt, Touch,
};
use orbit_gfx::{Point, Rgb565, Shape, SpriteRef};
use orbit_ui::{
    CLICK, DEEP, DIAL_GAIN, GOLD, HINT, TEXT, THUD, battery_level, dial_choice, dial_point, disc,
    gold_label, label, label_at, power_overlay, rect, rgb,
};
use pals::{HATS, Pals};

use crate::fish;

pub const BACKGROUND: Rgb565 = WALL;
const WALL: Rgb565 = rgb(36, 74, 60);
const MIRROR_GLASS: Rgb565 = rgb(150, 200, 220);
const MIRROR_FRAME: Rgb565 = rgb(200, 160, 80);
/// Round the ring, clockwise from 12 o'clock: NO HAT, the shelf, then OUT.
pub const CHOICES: usize = HATS.len() + 2;
pub const OUT: usize = HATS.len() + 1;
const RING: f32 = 186.0;
const MIRROR: Point = Point::new(233, 214);

/// The device turn that points at ring choice `choice`.
pub fn hat_turn(choice: usize) -> f32 {
    let deg = choice as f32 * 360.0 / CHOICES as f32;
    let deg = if deg > 180.0 { deg - 360.0 } else { deg };
    deg / DIAL_GAIN
}

pub mod ui {
    pub use orbit_ui::Button;

    /// The mirror: tap it to dress the next fish.
    pub const MIRROR: Button = Button::new(163, 144, 140, 140, "");
}

#[derive(Resource, Debug, Default)]
pub struct Shop {
    /// Which fish is being dressed.
    pub pet: usize,
}

pub struct ShopPlugin;

impl Plugin for ShopPlugin {
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
        app.init_resource::<Shop>()
            .init_resource::<Pals>()
            .add_systems(Startup, arrive)
            .add_systems(Update, browse)
            .add_systems(PostUpdate, draw_frame);
    }
}

fn arrive(blob: Res<SaveBlob>, mut pals: ResMut<Pals>, mut tilt: ResMut<Tilt>) {
    *pals = pals::save::load(&blob);
    tilt.request_zero();
}

#[allow(clippy::too_many_arguments)]
fn browse(
    buttons: Res<Buttons>,
    tilt: Res<Tilt>,
    touch: Res<Touch>,
    power: Res<Power>,
    mut shop: ResMut<Shop>,
    mut pals: ResMut<Pals>,
    mut blob: ResMut<SaveBlob>,
    mut sound: ResMut<SoundQueue>,
    mut exit: MessageWriter<AppExit>,
) {
    if power.saying_good_night() {
        return;
    }
    let tap = touch.just_pressed().then(|| touch.position()).flatten();
    if tap.is_some_and(|at| ui::MIRROR.hit(at)) && pals.pets.len() > 1 {
        shop.pet = (shop.pet + 1) % pals.pets.len();
        sound.play(&CLICK);
        return;
    }
    let tapped = tap.and_then(|at| {
        (0..CHOICES).find(|&choice| {
            let center = dial_point(choice as f32 * 360.0 / CHOICES as f32, RING);
            Vec2::new(center.x as f32, center.y as f32).distance(at) < 26.0
        })
    });
    let Some(choice) = tapped.or(buttons
        .action
        .pressed()
        .then(|| dial_choice(tilt.steer_deg, CHOICES)))
    else {
        return;
    };
    if choice == OUT {
        sound.play(&CLICK);
        exit.write(AppExit::Success);
        return;
    }
    let pet = shop.pet;
    let worn = pals.pets.get(pet).and_then(|pet| pet.hat).map(usize::from);
    let hat = choice.checked_sub(1);
    let changed: Option<&'static Clip> = match hat {
        Some(hat) if !pals.owns_hat(hat) => pals.buy_hat(hat).then(|| {
            pals.wear(pet, Some(hat));
            &angler_game::audio::COIN
        }),
        _ if hat == worn || pals.pets.is_empty() => None,
        _ => {
            pals.wear(pet, hat);
            Some(&CLICK)
        }
    };
    if changed.is_some() {
        pals::save::store(&mut blob, &pals);
    }
    sound.play(changed.unwrap_or(&THUD));
}

fn draw_frame(
    shop: Res<Shop>,
    pals: Res<Pals>,
    tilt: Res<Tilt>,
    power: Res<Power>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    shapes.push(rect(0, 0, 466, 466, WALL));
    let pointed = dial_choice(tilt.steer_deg, CHOICES);
    ring(shapes, pointed);
    gold_label(
        shapes,
        Point::new(240, 124),
        &format!("GOLD {}", pals.gold),
        1,
    );
    shapes.push(disc(MIRROR.x, MIRROR.y, 72, MIRROR_FRAME));
    shapes.push(disc(MIRROR.x, MIRROR.y, 66, MIRROR_GLASS));
    let hat = pointed.checked_sub(1).filter(|&hat| hat < HATS.len());
    match pals.pets.get(shop.pet) {
        Some(pet) => {
            let mut trying = pet.clone();
            if pointed != OUT {
                trying.hat = hat.map(|hat| hat as u8);
            }
            fish::draw(
                shapes,
                &trying,
                Vec2::new(MIRROR.x as f32, MIRROR.y as f32 + 14.0),
                4,
                true,
            );
            shapes.push(label(102, pet.name.as_str(), 1, TEXT));
        }
        None => {
            if let Some(hat) = hat {
                let size = HATS[hat].sprite.size() * 4;
                shapes.push(Shape::Sprite {
                    sprite: SpriteRef(HATS[hat].sprite),
                    top_left: MIRROR - Point::new(size.width as i32 / 2, size.height as i32 / 2),
                    scale: 4,
                    flip_x: false,
                });
            }
            shapes.push(label(102, "NO FISH TO DRESS", 1, HINT));
        }
    }
    let worn = pals
        .pets
        .get(shop.pet)
        .and_then(|pet| pet.hat)
        .map(usize::from);
    let (name, status, hint): (&str, alloc::string::String, alloc::string::String) = match pointed {
        OUT => ("OUT", "".into(), "PRESS TO LEAVE".into()),
        0 => (
            "NO HAT",
            "".into(),
            if worn.is_some() {
                "PRESS TO TAKE IT OFF".into()
            } else {
                "".into()
            },
        ),
        choice => {
            let hat = choice - 1;
            let shelf = &HATS[hat];
            let owned = pals.owns_hat(hat);
            let status = match (worn == Some(hat), owned) {
                (true, _) => "WEARING".into(),
                (false, true) => "OWNED".into(),
                (false, false) => format!("{} GOLD", shelf.price),
            };
            let hint = match (worn == Some(hat), owned, pals.gold >= shelf.price) {
                (true, _, _) => "".into(),
                (false, true, _) => "PRESS TO WEAR IT".into(),
                (false, false, true) => "PRESS TO BUY".into(),
                (false, false, false) => format!("NEED {} MORE", shelf.price - pals.gold),
            };
            (shelf.name, status, hint)
        }
    };
    shapes.push(label(310, name, 2, TEXT));
    shapes.push(label(336, &status, 1, GOLD));
    shapes.push(label(356, &hint, 1, HINT));
    if pals.pets.len() > 1 {
        shapes.push(label(376, "TAP THE FISH FOR THE NEXT ONE", 1, HINT));
    }
    if let Some(battery) = power.battery {
        battery_level(shapes, &battery);
    }
    power_overlay(shapes, &power);
}

/// The shelf round the glass, the pointed hat ringed in gold.
fn ring(shapes: &mut Vec<Shape>, pointed: usize) {
    for choice in 0..CHOICES {
        let at = dial_point(choice as f32 * 360.0 / CHOICES as f32, RING);
        if choice == pointed {
            shapes.push(disc(at.x, at.y, 24, GOLD));
            shapes.push(disc(at.x, at.y, 21, DEEP));
        }
        match choice.checked_sub(1).and_then(|hat| HATS.get(hat)) {
            Some(hat) => {
                let size = hat.sprite.size() * 2;
                shapes.push(Shape::Sprite {
                    sprite: SpriteRef(hat.sprite),
                    top_left: at - Point::new(size.width as i32 / 2, size.height as i32 / 2),
                    scale: 2,
                    flip_x: false,
                });
            }
            None => {
                let text = if choice == OUT { "OUT" } else { "NONE" };
                shapes.push(label_at(at, text, 1, TEXT));
            }
        }
    }
}
