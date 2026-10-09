//! Fishing Minigame's places: the town (the hub, where the board starts) and
//! the house, park, contest hall and hat shop. Each is its own Bevy app, so only
//! one is in the board's memory at a time. The pond, the den, the arcade and the
//! humming tree are other crates' apps. The kept fish, gold and hats pass
//! between them in the `PALS` save section.

#![no_std]

extern crate alloc;

pub mod contest;
pub mod fish;
pub mod home;
pub mod shop;
mod sprites;
pub mod town;
pub mod walk;

use bevy::prelude::*;

/// Everywhere the town's doors lead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    /// Your house: the tank, the fish's cards and trick training.
    Home,
    /// Angler: catching fish to keep.
    Pond,
    /// Walks, for stamina.
    Park,
    /// Contests, for gold.
    Contest,
    /// Hats for the fish.
    Shop,
    /// Spider Roulette, for a gamble.
    Den,
    /// Skate, on a cabinet.
    Arcade,
    /// Bees???, in the hollow tree.
    BeeTree,
    /// The signpost by the house: sound, mock walks and resetting.
    Settings,
}

impl Place {
    pub const ALL: [Place; 9] = [
        Place::Home,
        Place::Pond,
        Place::Park,
        Place::Contest,
        Place::Shop,
        Place::Den,
        Place::Arcade,
        Place::BeeTree,
        Place::Settings,
    ];

    /// What the door says.
    pub fn name(self) -> &'static str {
        match self {
            Place::Home => "HOME",
            Place::Pond => "THE POND",
            Place::Park => "THE PARK",
            Place::Contest => "CONTEST HALL",
            Place::Shop => "HAT SHOP",
            Place::Den => "THE DEN",
            Place::Arcade => "ARCADE",
            Place::BeeTree => "A HUMMING TREE",
            Place::Settings => "SETTINGS",
        }
    }
}

/// Inserted by the platform when it builds the town again after a place's
/// app ends, so the walk picks up at that place's door.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Arrival(pub Place);
