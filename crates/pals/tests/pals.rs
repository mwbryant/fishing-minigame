use bevy::math::Vec2;
use orbit_core::{Fix, Rng, SaveBlob, VoicePrint, testing};
use pals::{
    Gesture, HATS, MAX_PETS, Name, Pals, Pet, Rarity, Strokes, TRICKS, Trail, Trigger, save,
    walk::MockWalk,
};

fn spoken(word: &str) -> Vec<i16> {
    let path = format!(
        "{}/../orbit_audio/tests/voices/{word}.pcm",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(path)
        .expect("voice fixture")
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect()
}

fn print_of(word: &str) -> VoicePrint {
    let mut app = testing::headless(
        |app| {
            app.add_plugins((orbit_core::CorePlugin, orbit_core::MicPlugin));
        },
        None,
    );
    testing::speak(&mut app, &spoken(word));
    app.world_mut()
        .resource_mut::<orbit_core::Mic>()
        .take()
        .expect("heard")
}

fn name(text: &str) -> Name {
    let mut name = Name::default();
    for letter in text.chars() {
        assert!(name.push(letter), "{letter}");
    }
    name
}

fn pet(tier: u8, rng: &mut Rng) -> Pet {
    Pet::new(3, tier, 412, 60, name("GILLY"), rng)
}

#[test]
fn a_kept_fish_knows_five_different_tricks_and_rarer_fish_roll_rarer_ones() {
    let mut rng = Rng::new(5);
    let mean_rarity = |tier: u8, rng: &mut Rng| {
        let mut total = 0usize;
        for _ in 0..400 {
            let pet = pet(tier, rng);
            let mut ids: Vec<u8> = pet.tricks.iter().map(|slot| slot.trick).collect();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), 5, "five different tricks");
            total += pet
                .tricks
                .iter()
                .map(|slot| slot.info().rarity as usize)
                .sum::<usize>();
        }
        total as f32 / 2000.0
    };
    let minnow = mean_rarity(0, &mut rng);
    let trophy = mean_rarity(3, &mut rng);
    assert!(trophy > minnow + 0.5, "trophy {trophy} vs minnow {minnow}");
    assert!(TRICKS.iter().any(|t| t.rarity == Rarity::Legendary));
}

#[test]
fn names_take_capital_letters_and_spaces_up_to_eight() {
    let mut name = name("BUBBA");
    assert_eq!(name.as_str(), "BUBBA");
    assert!(!name.push('b') && !name.push('7') && !name.push('!'));
    assert!(name.push(' ') && name.push('J') && name.push('R'));
    assert!(!name.push('X'), "eight at most");
    assert_eq!(name.as_str(), "BUBBA JR");
    name.pop();
    assert_eq!(name.as_str(), "BUBBA J");
}

#[test]
fn walking_levels_up_stamina_and_each_level_takes_longer() {
    let mut pet = pet(1, &mut Rng::new(1));
    assert_eq!(pet.stamina_level(), 1);
    let start = pet.stamina_max();
    pet.walk(99.0);
    assert_eq!(pet.stamina_level(), 1);
    pet.walk(1.0);
    assert_eq!(pet.stamina_level(), 2);
    assert!(pet.stamina_max() > start);
    let first = pet.next_level_m() - 100;
    pet.walk(first as f32);
    let second = pet.next_level_m() - pet.walked_m;
    assert!(second > first, "{second} after {first}");
}

#[test]
fn a_trick_answers_its_taught_gesture_or_word_once_taught_and_not_before() {
    let mut pet = pet(2, &mut Rng::new(9));
    assert_eq!(pet.answer_gesture(Gesture::SwipeUp), None, "nothing taught");
    pet.teach(
        0,
        Trigger {
            gesture: Some(Gesture::SwipeUp),
            voice: None,
        },
    );
    pet.teach(
        1,
        Trigger {
            gesture: None,
            voice: Some(print_of("daniel-jump-170")),
        },
    );
    pet.teach(
        2,
        Trigger {
            gesture: Some(Gesture::CircleClockwise),
            voice: Some(print_of("daniel-splash-170")),
        },
    );
    assert_eq!(pet.answer_gesture(Gesture::SwipeUp), Some(0));
    assert_eq!(pet.answer_gesture(Gesture::CircleClockwise), Some(2));
    assert_eq!(pet.answer_gesture(Gesture::Tap), None);
    assert_eq!(pet.answer_voice(&print_of("daniel-jump-240")), Some(1));
    assert_eq!(pet.answer_voice(&print_of("daniel-splash-240")), Some(2));
    assert!(pet.tricks[0].taught() && !pet.tricks[3].taught());
}

#[test]
fn practice_teaches_a_trick_faster_with_praise_and_rare_tricks_take_longer() {
    let pet = pet(3, &mut Rng::new(3));
    let common = pet
        .tricks
        .iter()
        .position(|slot| slot.info().rarity == Rarity::Common);
    let rarest = (0..5)
        .max_by_key(|&slot| pet.tricks[slot].info().rarity as u8)
        .unwrap();
    let mut plain = pet.tricks[rarest];
    let mut praised = pet.tricks[rarest];
    plain.practice(false);
    praised.practice(true);
    assert!(praised.learned > plain.learned && plain.learned > 0);
    assert!(praised.success_chance() > pet.tricks[rarest].success_chance());
    if let Some(common) = common {
        let mut easy = pet.tricks[common];
        easy.practice(true);
        assert!(
            easy.learned >= praised.learned,
            "common tricks learn faster"
        );
    }
    for _ in 0..200 {
        praised.practice(true);
    }
    assert_eq!(praised.learned, 100);
    assert!(praised.mastered() && praised.success_chance() == 1.0);
}

#[test]
fn rarer_bigger_fish_in_fancier_hats_have_more_appeal() {
    let mut rng = Rng::new(2);
    let minnow = Pet::new(0, 0, 60, 10, name("TINY"), &mut rng);
    let mut koi = Pet::new(7, 3, 700, 95, name("KOI"), &mut rng);
    assert!(koi.appeal() > minnow.appeal());
    let bare = koi.appeal();
    koi.hat = Some(HATS.len() as u8 - 1);
    assert!(koi.appeal() > bare);
}

#[test]
fn up_to_three_fish_are_kept_and_a_full_tank_swaps_one_out() {
    let mut rng = Rng::new(4);
    let mut pals = Pals::default();
    for n in 0..MAX_PETS {
        assert!(pals.keep(pet(n as u8 % 4, &mut rng)).is_ok());
    }
    let extra = Pet::new(5, 2, 500, 50, name("NEWBIE"), &mut rng);
    assert!(pals.keep(extra.clone()).is_err(), "the tank is full");
    let released = pals.swap(1, extra);
    assert_eq!(released.name.as_str(), "GILLY");
    assert_eq!(pals.pets[1].name.as_str(), "NEWBIE");
    pals.release(0);
    assert_eq!(pals.pets.len(), MAX_PETS - 1);
}

#[test]
fn gold_buys_hats_once_and_each_fish_wears_its_own() {
    let mut rng = Rng::new(6);
    let mut pals = Pals::default();
    pals.keep(pet(1, &mut rng)).unwrap();
    pals.keep(pet(2, &mut rng)).unwrap();
    let beanie = HATS.iter().position(|hat| hat.name == "BEANIE").unwrap();
    assert!(!pals.buy_hat(beanie), "no gold");
    pals.earn(HATS[beanie].price + 3);
    assert!(pals.buy_hat(beanie));
    assert_eq!(pals.gold, 3);
    assert!(!pals.buy_hat(beanie), "already owned");
    assert!(pals.owns_hat(0), "the straw hat is free");
    assert!(pals.wear(0, Some(beanie)));
    assert!(pals.wear(1, Some(beanie)), "hats aren't used up");
    assert!(!pals.wear(1, Some(beanie + 1)), "not owned");
    assert!(pals.wear(1, None));
    assert_eq!(
        (pals.pets[0].hat, pals.pets[1].hat),
        (Some(beanie as u8), None)
    );
}

#[test]
fn the_tank_gold_hats_and_taught_words_survive_the_save_record() {
    let mut rng = Rng::new(8);
    let mut pals = Pals::default();
    pals.earn(77);
    pals.buy_hat(2);
    pals.mock_walk = true;
    pals.mock_pace = 3;
    for word in ["fred-spin-170", "fred-flip-170", "fred-bubbles-170"] {
        let mut pet = pet(3, &mut rng);
        pet.walk(1234.5);
        pet.hat = Some(2);
        for slot in 0..5 {
            pet.teach(
                slot,
                Trigger {
                    gesture: Some(Gesture::SwipeLeft),
                    voice: Some(print_of(word)),
                },
            );
            pet.tricks[slot].practice(true);
        }
        pals.keep(pet).unwrap();
    }
    let bytes = save::encode(&pals);
    // The whole save record is one 4,080-byte flash payload; this leaves
    // the other apps' sections about 880 bytes.
    assert!(bytes.len() < 3_200, "a full tank is {} bytes", bytes.len());
    assert_eq!(save::decode(&bytes), Some(pals.clone()));
    let mut blob = SaveBlob::from_storage(None);
    save::store(&mut blob, &pals);
    let record = blob.take_pending().unwrap();
    assert_eq!(save::load(&SaveBlob::from_storage(Some(record))), pals);
    assert_eq!(save::decode(&bytes[..bytes.len() - 1]), None);
    assert_eq!(save::load(&SaveBlob::from_storage(None)), Pals::default());
}

/// Feeds a finger path, one point per update, then lifts it.
fn stroke(path: &[Vec2]) -> Option<Gesture> {
    let mut strokes = Strokes::default();
    let mut seen = None;
    for &at in path {
        seen = seen.or(strokes.update(Some(at), 1.0 / 60.0));
    }
    seen.or(strokes.update(None, 1.0 / 60.0))
}

fn line(from: Vec2, to: Vec2, steps: usize) -> Vec<Vec2> {
    (0..=steps)
        .map(|i| from.lerp(to, i as f32 / steps as f32))
        .collect()
}

#[test]
fn finger_strokes_read_as_taps_holds_swipes_and_circles() {
    let center = Vec2::splat(233.0);
    assert_eq!(stroke(&[center; 6]), Some(Gesture::Tap));
    assert_eq!(stroke(&[center; 50]), Some(Gesture::Hold));
    let swipe = |to: Vec2| stroke(&line(center, center + to, 12));
    assert_eq!(swipe(Vec2::new(0.0, -120.0)), Some(Gesture::SwipeUp));
    assert_eq!(swipe(Vec2::new(0.0, 120.0)), Some(Gesture::SwipeDown));
    assert_eq!(swipe(Vec2::new(-120.0, 10.0)), Some(Gesture::SwipeLeft));
    assert_eq!(swipe(Vec2::new(120.0, -10.0)), Some(Gesture::SwipeRight));
    let circle = |sign: f32| {
        let path: Vec<Vec2> = (0..=40)
            .map(|i| {
                let a = sign * i as f32 / 40.0 * std::f32::consts::TAU;
                center + Vec2::new(a.cos(), a.sin()) * 70.0
            })
            .collect();
        stroke(&path)
    };
    assert_eq!(
        circle(1.0),
        Some(Gesture::CircleClockwise),
        "y grows downward"
    );
    assert_eq!(circle(-1.0), Some(Gesture::CircleCounterclockwise));
    assert_eq!(
        stroke(&line(center, center + Vec2::new(25.0, 0.0), 8)),
        None,
        "a nudge"
    );
}

fn fix_at(north_m: f64, east_m: f64, hdop: f32, speed_mps: f32) -> Fix {
    let per_deg = 111_195.0;
    Fix {
        lat_deg: 45.0 + north_m / per_deg,
        lon_deg: -122.0 + east_m / (per_deg * std::f64::consts::FRAC_1_SQRT_2),
        hdop,
        satellites: 9,
        speed_mps,
    }
}

#[test]
fn a_walk_counts_metres_walked_but_not_standing_still_driving_or_bad_fixes() {
    let mut rng = Rng::new(11);
    let mut still = Trail::default();
    for _ in 0..300 {
        let jitter = |rng: &mut Rng| (rng.next_f32() as f64 - 0.5) * 6.0;
        still.step(fix_at(jitter(&mut rng), jitter(&mut rng), 1.1, 0.2));
    }
    assert!(
        still.meters < 1.0,
        "jitter in place counted {}",
        still.meters
    );

    let mut walk = Trail::default();
    for second in 0..=100 {
        walk.step(fix_at(second as f64 * 1.4, 0.0, 1.0, 1.4));
    }
    assert!((walk.meters - 140.0).abs() < 12.0, "walked {}", walk.meters);

    let mut drive = Trail::default();
    for second in 0..=60 {
        drive.step(fix_at(second as f64 * 15.0, 0.0, 1.0, 15.0));
    }
    assert!(drive.meters < 1.0, "a car ride counted {}", drive.meters);

    let mut poor = Trail::default();
    for second in 0..=60 {
        poor.step(fix_at(second as f64 * 1.4, 0.0, 12.0, 1.4));
    }
    assert!(poor.meters < 1.0, "poor fixes counted {}", poor.meters);
}

#[test]
fn a_mock_walk_strolls_at_walking_pace() {
    let mut mock = MockWalk::default();
    let mut trail = Trail::default();
    for _ in 0..60 * 120 {
        if let Some(fix) = mock.step(1.0 / 60.0) {
            trail.step(fix);
        }
    }
    assert!(
        (140.0..200.0).contains(&trail.meters),
        "two minutes made {} m",
        trail.meters
    );
}

#[test]
fn a_save_from_before_the_new_voice_prints_keeps_its_fish_and_strokes_but_not_their_words() {
    let mut rng = Rng::new(12);
    let mut pals = Pals::default();
    pals.earn(31);
    let mut pet = pet(2, &mut rng);
    pet.teach(
        0,
        Trigger {
            gesture: Some(Gesture::SwipeUp),
            voice: Some(print_of("daniel-spin-170")),
        },
    );
    pet.tricks[0].learned = 40;
    pals.keep(pet).unwrap();
    // Version 1 stored 145-byte prints: header (11), the pet (19), then the
    // first trick's id, learned, gesture and has-word bytes.
    let mut old = save::encode(&pals);
    old[0] = 1;
    let print_at = 11 + 19 + 4;
    old.drain(print_at + 145..print_at + VoicePrint::BYTES);
    let loaded = save::decode(&old).expect("an old save still loads");
    assert_eq!(loaded.gold, 31);
    let trick = &loaded.pets[0].tricks[0];
    assert_eq!(
        (trick.learned, trick.trigger.gesture),
        (40, Some(Gesture::SwipeUp))
    );
    assert_eq!(
        trick.trigger.voice, None,
        "the old word has to be taught again"
    );
}

#[test]
fn every_word_a_fish_hears_moves_its_taught_word_toward_how_it_was_said() {
    let mut pet = pet(1, &mut Rng::new(13));
    let taught = print_of("daniel-flip-170");
    pet.teach(
        3,
        Trigger {
            gesture: None,
            voice: Some(taught),
        },
    );
    let said = print_of("daniel-flip-240");
    assert_eq!(pet.hear(&said), Some(3));
    let learned = pet.tricks[3].trigger.voice.unwrap();
    assert!(learned.distance(&said) < taught.distance(&said));
    assert_eq!(
        pet.hear(&print_of("daniel-bubbles-170")),
        None,
        "not a taught word"
    );
}
