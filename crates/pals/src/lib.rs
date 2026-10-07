//! Fish Pals: the fish you keep. Catch them at the pond, name them, walk
//! them to build stamina, teach them tricks with your voice and the glass,
//! and enter them in contests for the gold that buys their hats.
//!
//! This crate is the shared state and rules (no systems): every place in
//! the town is its own app, and they pass this through the `PALS` save
//! section.

#![no_std]

extern crate alloc;

pub mod gesture;
pub mod hats;
pub mod pet;
pub mod save;
pub mod tricks;
pub mod walk;

use alloc::vec::Vec;

use bevy::prelude::*;

pub use gesture::{Gesture, Strokes};
pub use hats::{HATS, Hat};
pub use pet::{Name, Pet, TrickSlot, Trigger};
pub use tricks::{Rarity, TRICKS, TRICKS_PER_PET, TrickInfo};
pub use walk::{MockWalk, Trail};

/// The tank holds this many.
pub const MAX_PETS: usize = 3;

/// The tank already holds [`MAX_PETS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TankFull;

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct Pals {
    pub pets: Vec<Pet>,
    pub gold: u32,
    owned_hats: u32,
    /// Walks stroll on their own instead of following the receiver.
    pub mock_walk: bool,
}

impl Default for Pals {
    fn default() -> Self {
        Self {
            pets: Vec::new(),
            gold: 0,
            owned_hats: 1,
            mock_walk: false,
        }
    }
}

impl Pals {
    /// Puts a fish in the tank, unless it's full (then [`Pals::swap`] one).
    pub fn keep(&mut self, pet: Pet) -> Result<(), TankFull> {
        if self.is_full() {
            return Err(TankFull);
        }
        self.pets.push(pet);
        Ok(())
    }

    pub fn is_full(&self) -> bool {
        self.pets.len() >= MAX_PETS
    }

    /// Puts `pet` where `index` was; returns the one let go.
    pub fn swap(&mut self, index: usize, pet: Pet) -> Pet {
        core::mem::replace(&mut self.pets[index], pet)
    }

    pub fn release(&mut self, index: usize) {
        if index < self.pets.len() {
            self.pets.remove(index);
        }
    }

    pub fn earn(&mut self, gold: u32) {
        self.gold = self.gold.saturating_add(gold);
    }

    /// Takes up to `gold`; returns how much was there to take.
    pub fn spend_up_to(&mut self, gold: u32) -> u32 {
        let taken = gold.min(self.gold);
        self.gold -= taken;
        taken
    }

    pub fn owns_hat(&self, hat: usize) -> bool {
        hat < HATS.len() && self.owned_hats & (1 << hat) != 0
    }

    pub fn owned_hats_mask(&self) -> u32 {
        self.owned_hats
    }

    pub(crate) fn set_owned_hats(&mut self, mask: u32) {
        self.owned_hats = (mask & hats::ALL_HATS) | 1;
    }

    /// Pays for `hat`; false if it's already owned or too dear.
    pub fn buy_hat(&mut self, hat: usize) -> bool {
        let Some(price) = HATS.get(hat).map(|h| h.price) else {
            return false;
        };
        if self.owns_hat(hat) || self.gold < price {
            return false;
        }
        self.gold -= price;
        self.owned_hats |= 1 << hat;
        true
    }

    /// Puts an owned hat (or none) on fish `pet`; false if it can't.
    pub fn wear(&mut self, pet: usize, hat: Option<usize>) -> bool {
        if hat.is_some_and(|hat| !self.owns_hat(hat)) || pet >= self.pets.len() {
            return false;
        }
        self.pets[pet].hat = hat.map(|hat| hat as u8);
        true
    }

    /// The first fish in the tank, which leads walks and wears its hat for
    /// the bees.
    pub fn lead(&self) -> Option<&Pet> {
        self.pets.first()
    }
}
