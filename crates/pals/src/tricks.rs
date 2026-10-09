//! Every trick a fish might know. Each kept fish rolls five; rarer species
//! roll rarer ones, which are worth more in a contest, tire the fish more,
//! and take longer to teach.

use orbit_core::Rng;

pub const TRICKS_PER_PET: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub const ALL: [Rarity; 5] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::Epic,
        Rarity::Legendary,
    ];

    pub fn name(self) -> &'static str {
        ["COMMON", "UNCOMMON", "RARE", "EPIC", "LEGENDARY"][self as usize]
    }

    /// Contest points for performing it.
    pub fn points(self) -> u32 {
        [10, 20, 35, 60, 100][self as usize]
    }

    /// Stamina a performance costs.
    pub fn stamina(self) -> u32 {
        [5, 8, 12, 18, 25][self as usize]
    }

    /// Learning (out of 100) a praised practice adds; unpraised adds half,
    /// rounded up.
    pub fn lesson(self) -> u8 {
        [25, 17, 12, 9, 6][self as usize]
    }

    /// Percent chance of rolling this rarity for the lowest and highest tier;
    /// tiers between blend linearly.
    fn odds(self) -> (f32, f32) {
        [
            (60.0, 25.0),
            (28.0, 30.0),
            (9.0, 24.0),
            (2.5, 14.0),
            (0.5, 7.0),
        ][self as usize]
    }
}

/// How a trick moves the fish, for whoever animates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Splash,
    Bubbles,
    Wiggle,
    Dive,
    Spin,
    Jump,
    Flip,
    FinWave,
    Loop,
    BubbleRing,
    TailWalk,
    Corkscrew,
    Moonwalk,
    RainbowLeap,
    Waterspout,
}

pub struct TrickInfo {
    pub name: &'static str,
    pub rarity: Rarity,
    pub motion: Motion,
}

const fn trick(name: &'static str, rarity: Rarity, motion: Motion) -> TrickInfo {
    TrickInfo {
        name,
        rarity,
        motion,
    }
}

pub const TRICKS: [TrickInfo; 15] = [
    trick("SPLASH", Rarity::Common, Motion::Splash),
    trick("BUBBLES", Rarity::Common, Motion::Bubbles),
    trick("WIGGLE", Rarity::Common, Motion::Wiggle),
    trick("DIVE", Rarity::Common, Motion::Dive),
    trick("SPIN", Rarity::Uncommon, Motion::Spin),
    trick("JUMP", Rarity::Uncommon, Motion::Jump),
    trick("FLIP", Rarity::Uncommon, Motion::Flip),
    trick("FIN WAVE", Rarity::Uncommon, Motion::FinWave),
    trick("LOOP", Rarity::Rare, Motion::Loop),
    trick("BUBBLE RING", Rarity::Rare, Motion::BubbleRing),
    trick("TAIL WALK", Rarity::Rare, Motion::TailWalk),
    trick("CORKSCREW", Rarity::Epic, Motion::Corkscrew),
    trick("MOONWALK", Rarity::Epic, Motion::Moonwalk),
    trick("RAINBOW LEAP", Rarity::Legendary, Motion::RainbowLeap),
    trick("WATERSPOUT", Rarity::Legendary, Motion::Waterspout),
];

/// Five different tricks for a fish of species `tier` (0 common .. 3 trophy).
pub fn roll_tricks(rng: &mut Rng, tier: u8) -> [u8; TRICKS_PER_PET] {
    let blend = f32::from(tier.min(3)) / 3.0;
    let weights = Rarity::ALL.map(|rarity| {
        let (low, high) = rarity.odds();
        low + (high - low) * blend
    });
    let total: f32 = weights.iter().sum();
    let mut chosen = [u8::MAX; TRICKS_PER_PET];
    let mut count = 0;
    while count < TRICKS_PER_PET {
        let mut pick = rng.next_f32() * total;
        let rarity = Rarity::ALL
            .into_iter()
            .zip(weights)
            .find(|(_, weight)| {
                pick -= weight;
                pick < 0.0
            })
            .map_or(Rarity::Common, |(rarity, _)| rarity);
        let open: usize = (0..TRICKS.len())
            .filter(|&t| TRICKS[t].rarity == rarity && !chosen.contains(&(t as u8)))
            .count();
        if open == 0 {
            continue;
        }
        let nth = rng.next_u32() as usize % open;
        let trick = (0..TRICKS.len())
            .filter(|&t| TRICKS[t].rarity == rarity && !chosen.contains(&(t as u8)))
            .nth(nth)
            .unwrap_or(0);
        chosen[count] = trick as u8;
        count += 1;
    }
    chosen
}
