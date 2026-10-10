//! The town: six round clearings on a 3 x 2 grid, joined by paths at the
//! compass points. Each has its scenery, what's solid, its doors, and the
//! landmark you tap to go in.

use bevy::math::Vec2;
use orbit_gfx::{Point, Rectangle, Size, Sprite};

use crate::{Place, sprites};

pub const CENTER: Vec2 = Vec2::splat(233.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Screen {
    Pond,
    Home,
    Park,
    Hall,
    MainStreet,
    Grove,
}

impl Screen {
    pub fn name(self) -> &'static str {
        match self {
            Screen::Pond => "THE POND",
            Screen::Home => "HOME",
            Screen::Park => "THE PARK",
            Screen::Hall => "CONTEST HALL",
            Screen::MainStreet => "MAIN STREET",
            Screen::Grove => "THE GROVE",
        }
    }

    fn cell(self) -> (i8, i8) {
        match self {
            Screen::Pond => (0, 0),
            Screen::Home => (1, 0),
            Screen::Park => (2, 0),
            Screen::Hall => (0, 1),
            Screen::MainStreet => (1, 1),
            Screen::Grove => (2, 1),
        }
    }

    /// The clearing a path from this one leads to, going `way`.
    pub fn toward(self, way: Way) -> Option<Screen> {
        let (col, row) = self.cell();
        let (dx, dy) = way.step();
        match (col + dx, row + dy) {
            (0, 0) => Some(Screen::Pond),
            (1, 0) => Some(Screen::Home),
            (2, 0) => Some(Screen::Park),
            (0, 1) => Some(Screen::Hall),
            (1, 1) => Some(Screen::MainStreet),
            (2, 1) => Some(Screen::Grove),
            _ => None,
        }
    }
}

/// A path's direction off the edge of a clearing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    North,
    East,
    South,
    West,
}

impl Way {
    pub const ALL: [Way; 4] = [Way::North, Way::East, Way::South, Way::West];

    fn step(self) -> (i8, i8) {
        match self {
            Way::North => (0, -1),
            Way::East => (1, 0),
            Way::South => (0, 1),
            Way::West => (-1, 0),
        }
    }

    /// Outward, in screen coordinates (y grows downward).
    pub fn out(self) -> Vec2 {
        let (dx, dy) = self.step();
        Vec2::new(f32::from(dx), f32::from(dy))
    }
}

/// Something drawn in a clearing, and whether you can walk through it.
pub struct Prop {
    pub sprite: &'static Sprite,
    pub top_left: Point,
    pub scale: u32,
    pub solid: bool,
}

impl Prop {
    pub fn area(&self) -> Rectangle {
        let size: Size = self.sprite.size() * self.scale;
        Rectangle::new(self.top_left, size)
    }
}

const fn prop(sprite: &'static Sprite, x: i32, y: i32, scale: u32) -> Prop {
    Prop {
        sprite,
        top_left: Point::new(x, y),
        scale,
        solid: true,
    }
}

/// Each place's door: the clearing it's in and the spot to stand on.
pub fn door(place: Place) -> (Screen, Vec2) {
    match place {
        Place::Home => (Screen::Home, Vec2::new(233.0, 162.0)),
        Place::Settings => (Screen::Home, Vec2::new(126.0, 314.0)),
        Place::Pond => (Screen::Pond, Vec2::new(300.0, 262.0)),
        Place::Park => (Screen::Park, Vec2::new(233.0, 166.0)),
        Place::Contest => (Screen::Hall, Vec2::new(106.0, 226.0)),
        Place::Shop => (Screen::MainStreet, Vec2::new(120.0, 174.0)),
        Place::Den => (Screen::MainStreet, Vec2::new(356.0, 174.0)),
        Place::BeeTree => (Screen::Grove, Vec2::new(128.0, 182.0)),
        Place::Arcade => (Screen::Grove, Vec2::new(330.0, 189.0)),
    }
}

/// The pond's water, which you can't walk on: center and radius.
pub const LAKE: (Vec2, f32) = (Vec2::new(170.0, 170.0), 110.0);

/// The building, sign or dock you tap to go into `place`.
pub fn landmark(place: Place) -> Rectangle {
    let (screen, index) = match place {
        Place::Home => (Screen::Home, 0),
        Place::Settings => (Screen::Home, 1),
        Place::Pond => (Screen::Pond, 0),
        Place::Park => (Screen::Park, 0),
        Place::Contest => (Screen::Hall, 0),
        Place::Shop => (Screen::MainStreet, 0),
        Place::Den => (Screen::MainStreet, 1),
        Place::BeeTree => (Screen::Grove, 0),
        Place::Arcade => (Screen::Grove, 1),
    };
    props(screen)[index].area()
}

/// A tap at `at` is on `place`'s landmark, or within `slop` of it. The pond's
/// water counts as its landmark too.
pub fn on_landmark(place: Place, at: Vec2, slop: f32) -> bool {
    let in_lake = place == Place::Pond && at.distance(LAKE.0) <= LAKE.1 + slop;
    in_lake || miss(landmark(place), at) <= slop
}

/// How far `at` is from `area`: 0 inside it.
pub fn miss(area: Rectangle, at: Vec2) -> f32 {
    let low = Vec2::new(area.top_left.x as f32, area.top_left.y as f32);
    let high = low + Vec2::new(area.size.width as f32, area.size.height as f32);
    at.clamp(low, high).distance(at)
}

pub fn props(screen: Screen) -> &'static [Prop] {
    match screen {
        Screen::Home => &HOME,
        Screen::Pond => &POND,
        Screen::Park => &PARK,
        Screen::Hall => &HALL,
        Screen::MainStreet => &MAIN_STREET,
        Screen::Grove => &GROVE,
    }
}

static HOME: [Prop; 5] = [
    prop(&sprites::HOUSE, 179, 70, 6),
    prop(&sprites::SIGN, 110, 268, 4),
    prop(&sprites::TREE, 56, 140, 4),
    prop(&sprites::TREE, 352, 316, 4),
    prop(&sprites::TREE, 92, 330, 4),
];

static POND: [Prop; 3] = [
    Prop {
        solid: false,
        ..prop(&sprites::DOCK, 220, 244, 5)
    },
    prop(&sprites::TREE, 320, 100, 4),
    prop(&sprites::TREE, 96, 330, 4),
];

static PARK: [Prop; 5] = [
    prop(&sprites::BENCH, 203, 120, 5),
    prop(&sprites::TREE, 100, 110, 5),
    prop(&sprites::TREE, 320, 96, 5),
    prop(&sprites::TREE, 330, 300, 5),
    prop(&sprites::TREE, 90, 290, 4),
];

static HALL: [Prop; 3] = [
    prop(&sprites::HALL, 40, 140, 6),
    prop(&sprites::TREE, 70, 300, 4),
    prop(&sprites::TREE, 330, 320, 4),
];

static MAIN_STREET: [Prop; 4] = [
    prop(&sprites::SHOP, 70, 110, 5),
    prop(&sprites::DEN, 316, 110, 5),
    prop(&sprites::TREE, 96, 320, 4),
    prop(&sprites::TREE, 336, 320, 4),
];

static GROVE: [Prop; 5] = [
    prop(&sprites::BEE_TREE, 80, 90, 6),
    prop(&sprites::CABINET, 310, 120, 5),
    prop(&sprites::TREE, 250, 320, 5),
    prop(&sprites::TREE, 100, 300, 4),
    prop(&sprites::TREE, 350, 260, 4),
];
