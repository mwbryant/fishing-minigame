//! What a trick looks like: every one moves the fish or throws water about,
//! and the water tricks throw the water you'd expect.

use bevy::prelude::*;
use orbit_core::{Frame, SaveBlob, testing::*};
use orbit_gfx::Shape;
use pals::{Name, Pals, Pet, TRICKS, tricks::Motion};
use pals_game::{
    fish::{SPRAY, TRICK_SECS, effects, pose},
    home::{HomePhase, HomePlugin, TeachStep},
};

/// How far the house and the contest spread a trick: their `STAGE_MOTION`.
const STAGE_REACH: f32 = 0.65;
const AT: Vec2 = Vec2::new(233.0, 230.0);

fn thrown(motion: Motion, t: f32) -> Vec<Shape> {
    let mut shapes = Vec::new();
    effects(&mut shapes, motion, t, AT, true, STAGE_REACH);
    shapes
}

fn centers(shapes: &[Shape]) -> Vec<Vec2> {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Disc { center, color, .. } | Shape::Ring { center, color, .. }
                if *color == SPRAY =>
            {
                Some(Vec2::new(center.x as f32, center.y as f32))
            }
            _ => None,
        })
        .collect()
}

fn moments() -> impl Iterator<Item = f32> {
    (1..32).map(|i| i as f32 * TRICK_SECS / 32.0)
}

#[test]
fn every_trick_moves_the_fish_flips_it_or_throws_water() {
    for trick in TRICKS {
        let moved = moments()
            .map(|t| pose(trick.motion, t).0.length() * STAGE_REACH)
            .fold(0.0, f32::max);
        let flips = moments().any(|t| pose(trick.motion, t).1);
        let splashes = moments().any(|t| !thrown(trick.motion, t).is_empty());
        assert!(
            moved >= 5.0 || flips || splashes,
            "{} barely shows: it moves {moved:.1} px",
            trick.name
        );
    }
}

#[test]
fn a_bubble_ring_spreads_a_ring_of_bubbles_out_from_the_fish() {
    let ring_at = |t: f32| {
        let bubbles = centers(&thrown(Motion::BubbleRing, t));
        assert!(bubbles.len() >= 8, "{} bubbles at {t} s", bubbles.len());
        let radii: Vec<f32> = bubbles.iter().map(|b| b.distance(AT)).collect();
        let (low, high) = radii
            .iter()
            .fold((f32::MAX, 0.0f32), |(lo, hi), r| (lo.min(*r), hi.max(*r)));
        assert!(high - low < 10.0, "not a ring at {t} s: {radii:?}");
        let around = bubbles.iter().any(|b| b.x < AT.x - 10.0)
            && bubbles.iter().any(|b| b.x > AT.x + 10.0)
            && bubbles.iter().any(|b| b.y < AT.y - 10.0)
            && bubbles.iter().any(|b| b.y > AT.y + 10.0);
        assert!(around, "the ring doesn't go round the fish at {t} s");
        low
    };
    assert!(ring_at(1.2) > ring_at(0.3) + 25.0, "the ring grows");
}

#[test]
fn a_splash_throws_drops_up_and_to_both_sides() {
    let drops = centers(&thrown(Motion::Splash, 0.25));
    assert!(drops.len() >= 6, "{} drops", drops.len());
    assert!(drops.iter().any(|d| d.x < AT.x - 15.0), "none to the left");
    assert!(drops.iter().any(|d| d.x > AT.x + 15.0), "none to the right");
    assert!(drops.iter().any(|d| d.y < AT.y - 20.0), "none thrown up");
}

#[test]
fn bubbles_rise_from_the_fish() {
    let highest = |t: f32| {
        centers(&thrown(Motion::Bubbles, t))
            .iter()
            .map(|b| b.y)
            .fold(f32::MAX, f32::min)
    };
    assert!(highest(0.5) < AT.y);
    assert!(highest(1.0) < highest(0.5), "they rise");
}

#[test]
fn the_house_shows_the_bubble_ring_it_teaches() {
    let mut name = Name::default();
    "GILLY".chars().for_each(|c| assert!(name.push(c)));
    let mut gilly = Pet::new(3, 1, 400, 60, name, &mut orbit_core::Rng::new(1));
    let ring = TRICKS.iter().position(|t| t.name == "BUBBLE RING").unwrap();
    gilly.tricks[0].trick = ring as u8;
    let mut tank = Pals::default();
    tank.keep(gilly).unwrap();
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    let mut app = headless(
        |app| {
            app.add_plugins(HomePlugin);
        },
        blob.take_pending(),
    );
    for _ in 0..3 {
        click_action(&mut app);
    }
    let phase = (*app.world().resource::<pals_game::home::Home>().phase).clone();
    assert!(
        matches!(
            phase,
            HomePhase::Teach {
                step: TeachStep::Watch,
                ..
            }
        ),
        "{phase:?}"
    );
    tick(&mut app, 30);
    let shapes = &app.world().resource::<Frame>().shapes;
    let bubbles = centers(shapes);
    assert!(bubbles.len() >= 8, "{} bubbles in the house", bubbles.len());
}
