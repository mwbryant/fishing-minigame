//! A kept fish: who it is, how far it has walked, and the five tricks it
//! can learn, each with the trigger the player taught for it.

use orbit_core::{MATCH_DISTANCE, Rng, VoicePrint, recognize};

use crate::{
    gesture::Gesture,
    hats::HATS,
    tricks::{TRICKS, TRICKS_PER_PET, TrickInfo, roll_tricks},
};

pub const MAX_NAME: usize = 8;
pub const MAX_STAMINA_LEVEL: u32 = 20;
/// Level n takes this many metres times (n - 1) squared: level 2 at 100 m,
/// 3 at 400 m, 4 at 900 m.
const METRES_PER_LEVEL_SQUARED: u32 = 100;
/// Appeal for each species tier (0 common .. 3 trophy).
const TIER_APPEAL: [u32; 4] = [10, 20, 35, 60];
/// How far a confidently heard command pulls the taught word toward how it was
/// just said, so the fish gets used to your voice.
pub const VOICE_ADAPT: f32 = 0.2;
/// Confident means within this fraction of the match distance, so a word that
/// only just passed can't drag the taught one toward itself.
const ADAPT_WITHIN: f32 = 0.7;

/// Up to `MAX_NAME` capital letters and spaces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Name {
    letters: [u8; MAX_NAME],
    length: u8,
}

impl Name {
    /// Adds a letter; false if it isn't A-Z or space, or the name is full.
    pub fn push(&mut self, letter: char) -> bool {
        let allowed = letter.is_ascii_uppercase() || letter == ' ';
        if !allowed || usize::from(self.length) == MAX_NAME {
            return false;
        }
        self.letters[usize::from(self.length)] = letter as u8;
        self.length += 1;
        true
    }

    pub fn pop(&mut self) {
        self.length = self.length.saturating_sub(1);
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.letters[..usize::from(self.length)]).unwrap_or("")
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn len(&self) -> usize {
        usize::from(self.length)
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut name = Name::default();
        for &byte in bytes {
            if !name.push(char::from(byte)) {
                return None;
            }
        }
        Some(name)
    }

    pub(crate) fn letters(&self) -> [u8; MAX_NAME] {
        self.letters
    }
}

/// What makes a fish do a trick: a word the player said and/or a stroke on
/// the glass. Either one works once taught.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Trigger {
    pub gesture: Option<Gesture>,
    pub voice: Option<VoicePrint>,
}

impl Trigger {
    pub fn is_empty(&self) -> bool {
        self.gesture.is_none() && self.voice.is_none()
    }
}

/// One of a fish's tricks, and how well it knows it (0 to 100).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrickSlot {
    pub trick: u8,
    pub learned: u8,
    pub trigger: Trigger,
}

impl TrickSlot {
    pub fn info(&self) -> &'static TrickInfo {
        &TRICKS[usize::from(self.trick) % TRICKS.len()]
    }

    pub fn taught(&self) -> bool {
        !self.trigger.is_empty()
    }

    pub fn mastered(&self) -> bool {
        self.learned >= 100
    }

    /// Counts a successful try; praise right after doubles what it learns.
    pub fn practice(&mut self, praised: bool) {
        let lesson = self.info().rarity.lesson();
        let gain = if praised { lesson } else { lesson.div_ceil(2) };
        self.learned = self.learned.saturating_add(gain).min(100);
    }

    /// The chance it does the trick when asked: 0.25 with nothing learned,
    /// rising to 1 at mastery.
    pub fn success_chance(&self) -> f32 {
        if self.mastered() {
            return 1.0;
        }
        0.25 + 0.75 * f32::from(self.learned) / 100.0
    }
}

/// A kept fish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pet {
    /// An index into the fishing game's species table.
    pub species: u8,
    /// The species' tier, 0 common .. 3 trophy, kept so contests needn't
    /// know the species table.
    pub tier: u8,
    pub length_mm: u16,
    /// Where its length falls in its species' range, 0 to 100.
    pub size_pct: u8,
    pub name: Name,
    /// An index into [`HATS`], if it wears one.
    pub hat: Option<u8>,
    pub walked_m: u32,
    pub tricks: [TrickSlot; TRICKS_PER_PET],
}

impl Pet {
    pub fn new(
        species: u8,
        tier: u8,
        length_mm: u16,
        size_pct: u8,
        name: Name,
        rng: &mut Rng,
    ) -> Self {
        Self {
            species,
            tier,
            length_mm,
            size_pct: size_pct.min(100),
            name,
            hat: None,
            walked_m: 0,
            tricks: roll_tricks(rng, tier).map(|trick| TrickSlot {
                trick,
                learned: 0,
                trigger: Trigger::default(),
            }),
        }
    }

    pub fn walk(&mut self, meters: f32) {
        self.walked_m = self.walked_m.saturating_add(meters.max(0.0) as u32);
    }

    pub fn stamina_level(&self) -> u32 {
        (1 + (self.walked_m / METRES_PER_LEVEL_SQUARED).isqrt()).min(MAX_STAMINA_LEVEL)
    }

    /// The total walked distance (m) that reaches the next stamina level;
    /// `None` at the top level.
    pub fn next_level_m(&self) -> Option<u32> {
        let level = self.stamina_level();
        (level < MAX_STAMINA_LEVEL).then(|| level * level * METRES_PER_LEVEL_SQUARED)
    }

    /// The stamina a contest starts with.
    pub fn stamina_max(&self) -> u32 {
        30 + 10 * self.stamina_level()
    }

    /// How much the judges like the look of it: rarer species, a bigger
    /// fish of its kind, and a pricier hat.
    pub fn appeal(&self) -> u32 {
        let hat = self
            .hat
            .and_then(|hat| HATS.get(usize::from(hat)))
            .map_or(0, |hat| (hat.price / 25).min(20));
        TIER_APPEAL[usize::from(self.tier.min(3))] + u32::from(self.size_pct) / 5 + hat
    }

    pub fn teach(&mut self, slot: usize, trigger: Trigger) {
        if let Some(trick) = self.tricks.get_mut(slot) {
            trick.trigger = trigger;
        }
    }

    /// The trick taught to `gesture`, if any.
    pub fn answer_gesture(&self, gesture: Gesture) -> Option<usize> {
        self.tricks
            .iter()
            .position(|slot| slot.trigger.gesture == Some(gesture))
    }

    /// The trick `heard` asks for, if any; its taught word then moves a
    /// little toward how it was said.
    pub fn hear(&mut self, heard: &VoicePrint) -> Option<usize> {
        let slot = self.answer_voice(heard)?;
        if let Some(word) = self.tricks[slot].trigger.voice.as_mut()
            && word.distance(heard) < MATCH_DISTANCE * ADAPT_WITHIN
        {
            *word = word.blend(heard, VOICE_ADAPT);
        }
        Some(slot)
    }

    /// The trick taught the word nearest `heard`, if it's near enough.
    pub fn answer_voice(&self, heard: &VoicePrint) -> Option<usize> {
        recognize(
            heard,
            self.tricks.iter().enumerate().filter_map(|(slot, trick)| {
                trick.trigger.voice.as_ref().map(|voice| (slot, voice))
            }),
        )
    }
}
