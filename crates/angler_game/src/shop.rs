//! Gold from catches buys hats for the angler on the dock.

use bevy::prelude::*;
use orbit_gfx::Sprite;

use crate::sprites;

pub struct Hat {
    pub name: &'static str,
    pub price: u32,
    pub sprite: &'static Sprite,
}

/// The shop's shelf, cheapest first. The first hat is free and always owned.
pub const HATS: [Hat; 12] = [
    Hat {
        name: "STRAW HAT",
        price: 0,
        sprite: &sprites::STRAW_HAT,
    },
    Hat {
        name: "BEANIE",
        price: 15,
        sprite: &sprites::BEANIE,
    },
    Hat {
        name: "BALL CAP",
        price: 30,
        sprite: &sprites::BALL_CAP,
    },
    Hat {
        name: "BUCKET HAT",
        price: 50,
        sprite: &sprites::BUCKET_HAT,
    },
    Hat {
        name: "BEE HAT",
        price: 70,
        sprite: &sprites::BEE_HAT,
    },
    Hat {
        name: "COWBOY HAT",
        price: 90,
        sprite: &sprites::COWBOY_HAT,
    },
    Hat {
        name: "CLOWN HAT",
        price: 100,
        sprite: &sprites::CLOWN_HAT,
    },
    Hat {
        name: "SPIDER HAT",
        price: 120,
        sprite: &sprites::SPIDER_HAT,
    },
    Hat {
        name: "TOP HAT",
        price: 150,
        sprite: &sprites::TOP_HAT,
    },
    Hat {
        name: "SKATE HAT",
        price: 200,
        sprite: &sprites::SKATE_HAT,
    },
    Hat {
        name: "FISH HAT",
        price: 250,
        sprite: &sprites::FISH_HAT,
    },
    Hat {
        name: "CROWN",
        price: 500,
        sprite: &sprites::CROWN,
    },
];

const _: () = assert!(HATS.len() <= 32, "owned hats are a u32 bitmask");
const ALL_HATS: u32 = u32::MAX >> (32 - HATS.len() as u32);

/// The player's gold and hats, saved with the fish log. The worn hat is
/// always an owned one.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inventory {
    pub gold: u32,
    owned: u32,
    worn: usize,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            gold: 0,
            owned: 1,
            worn: 0,
        }
    }
}

impl Inventory {
    /// Rebuilds a saved inventory, dropping hats this build doesn't have.
    pub fn restore(gold: u32, owned_mask: u32, worn: u8) -> Self {
        let mut inventory = Self {
            gold,
            owned: (owned_mask & ALL_HATS) | 1,
            worn: 0,
        };
        inventory.wear(usize::from(worn));
        inventory
    }

    pub fn owned_mask(&self) -> u32 {
        self.owned
    }

    pub fn owns(&self, hat: usize) -> bool {
        hat < HATS.len() && self.owned & (1 << hat) != 0
    }

    pub fn worn(&self) -> usize {
        self.worn
    }

    pub fn hat(&self) -> &'static Hat {
        &HATS[self.worn]
    }

    pub fn earn(&mut self, gold: u32) {
        self.gold = self.gold.saturating_add(gold);
    }

    /// Pays for `hat` and puts it on; false if it's already owned or too dear.
    pub fn buy(&mut self, hat: usize) -> bool {
        let Some(price) = HATS.get(hat).map(|h| h.price) else {
            return false;
        };
        if self.owns(hat) || self.gold < price {
            return false;
        }
        self.gold -= price;
        self.owned |= 1 << hat;
        self.worn = hat;
        true
    }

    /// Puts on an owned hat; false if it isn't owned or is already on.
    pub fn wear(&mut self, hat: usize) -> bool {
        if !self.owns(hat) || self.worn == hat {
            return false;
        }
        self.worn = hat;
        true
    }
}

#[cfg(test)]
mod tests {
    use orbit_core::Rng;

    use super::*;
    use crate::species::roll_catch;

    const BEANIE: usize = 1;
    const BALL_CAP: usize = 2;

    fn with_gold(gold: u32) -> Inventory {
        let mut inventory = Inventory::default();
        inventory.earn(gold);
        inventory
    }

    #[test]
    fn a_new_player_has_no_gold_and_wears_the_free_straw_hat() {
        let inventory = Inventory::default();
        assert_eq!(inventory.gold, 0);
        assert_eq!(inventory.worn(), 0);
        assert_eq!(HATS[0].price, 0);
        assert!(inventory.owns(0));
        assert!((1..HATS.len()).all(|hat| !inventory.owns(hat)));
    }

    #[test]
    fn buying_a_hat_spends_its_price_and_puts_it_on() {
        let mut inventory = with_gold(HATS[BEANIE].price + 4);
        assert!(inventory.buy(BEANIE));
        assert_eq!(inventory.gold, 4);
        assert!(inventory.owns(BEANIE));
        assert_eq!(inventory.worn(), BEANIE);
    }

    #[test]
    fn a_hat_you_cannot_afford_stays_on_the_shelf() {
        let mut inventory = with_gold(HATS[BALL_CAP].price - 1);
        assert!(!inventory.buy(BALL_CAP));
        assert_eq!(inventory.gold, HATS[BALL_CAP].price - 1);
        assert!(!inventory.owns(BALL_CAP));
        assert_eq!(inventory.worn(), 0);
    }

    #[test]
    fn an_owned_hat_is_never_paid_for_twice() {
        let mut inventory = with_gold(HATS[BEANIE].price * 3);
        assert!(inventory.buy(BEANIE));
        let gold = inventory.gold;
        assert!(!inventory.buy(BEANIE));
        assert_eq!(inventory.gold, gold);
    }

    #[test]
    fn only_owned_hats_can_be_worn_and_swapping_back_is_free() {
        let mut inventory = with_gold(HATS[BEANIE].price);
        assert!(!inventory.wear(BALL_CAP));
        assert!(inventory.buy(BEANIE));
        assert!(inventory.wear(0));
        assert_eq!(inventory.worn(), 0);
        assert!(!inventory.wear(0), "already worn");
        assert!(inventory.wear(BEANIE));
        assert_eq!(inventory.gold, 0);
    }

    #[test]
    fn the_shelf_has_hats_for_the_other_arcade_games() {
        for name in ["SPIDER HAT", "SKATE HAT", "BEE HAT"] {
            assert!(HATS.iter().any(|hat| hat.name == name), "{name}");
        }
    }

    #[test]
    fn hats_get_pricier_along_the_shelf() {
        for pair in HATS.windows(2) {
            assert!(pair[1].price > pair[0].price, "{}", pair[1].name);
        }
    }

    #[test]
    fn restoring_drops_hats_this_build_lacks_and_keeps_the_straw_hat() {
        let restored = Inventory::restore(55, u32::MAX << 1, 31);
        assert_eq!(restored.gold, 55);
        assert!(restored.owns(0), "the free hat is always owned");
        assert!((0..HATS.len()).all(|hat| restored.owns(hat)));
        assert_eq!(restored.worn(), 0, "a hat index past the shelf");
        assert_eq!(Inventory::restore(0, 1, BEANIE as u8).worn(), 0, "unowned");
        let round_trip = Inventory::restore(9, 0b101, 2);
        assert_eq!(
            Inventory::restore(round_trip.gold, round_trip.owned_mask(), 2),
            round_trip
        );
        assert_eq!(round_trip.worn(), 2);
    }

    #[test]
    fn the_first_hat_takes_a_couple_of_catches_and_the_crown_a_long_campaign() {
        let mut rng = Rng::new(21);
        let n = 4000;
        let gold: u32 = (0..n).map(|_| roll_catch(&mut rng, 0.7, 0.6).gold()).sum();
        let per_catch = gold as f32 / n as f32;
        let catches = |hat: &Hat| hat.price as f32 / per_catch;
        assert!(catches(&HATS[1]) <= 3.0, "{per_catch} gold per catch");
        let crown = &HATS[HATS.len() - 1];
        assert!(catches(crown) >= 40.0, "{per_catch} gold per catch");
        let everything: u32 = HATS.iter().map(|hat| hat.price).sum();
        assert!(everything as f32 / per_catch <= 150.0, "{per_catch}");
    }
}
