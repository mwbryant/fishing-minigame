use bevy::{app::AppExit, prelude::*};
use orbit_core::{Frame, SaveBlob, testing::*};
use orbit_gfx::{Shape, Sprite, SpriteRef};
use orbit_ui::{Key, key_turn};
use pals::{Gesture, HATS, Name, Pals, Pet, TRICKS, Trigger};
use pals_game::home::{Home, HomePhase, HomePlugin, PracticeStep, TeachStep};

fn name(text: &str) -> Name {
    let mut name = Name::default();
    text.chars().for_each(|c| assert!(name.push(c)));
    name
}

/// A pet whose tricks are SPLASH, SPIN, LOOP, CORKSCREW and RAINBOW LEAP.
fn pet(text: &str, hat: Option<u8>) -> Pet {
    let mut pet = Pet::new(3, 1, 400, 60, name(text), &mut orbit_core::Rng::new(1));
    for (slot, trick) in ["SPLASH", "SPIN", "LOOP", "CORKSCREW", "RAINBOW LEAP"]
        .iter()
        .enumerate()
    {
        pet.tricks[slot].trick = TRICKS.iter().position(|t| t.name == *trick).unwrap() as u8;
    }
    pet.hat = hat;
    pet
}

fn home_with(pets: Vec<Pet>) -> App {
    let mut tank = Pals::default();
    tank.earn(500);
    tank.buy_hat(2);
    pets.into_iter().for_each(|pet| tank.keep(pet).unwrap());
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    headless(
        |app| {
            app.add_plugins(HomePlugin);
        },
        blob.take_pending(),
    )
}

fn phase(app: &App) -> HomePhase {
    (*app.world().resource::<Home>().phase).clone()
}

fn tank(app: &App) -> &Pals {
    app.world().resource::<Pals>()
}

fn saved_tank(app: &mut App) -> Pals {
    let pending = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    pals::save::load(&SaveBlob::from_storage(pending))
}

fn drawn(app: &App, sprite: &'static Sprite) -> usize {
    app.world()
        .resource::<Frame>()
        .shapes
        .iter()
        .filter(|s| matches!(s, Shape::Sprite { sprite: d, .. } if *d == SpriteRef(sprite)))
        .count()
}

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

/// Turns `steer` degrees, presses PWR, and turns back level.
fn choose(app: &mut App, steer: f32) {
    set_tilt(app, steer, 0.0);
    update(app);
    click_action(app);
    set_tilt(app, 0.0, 0.0);
    update(app);
}

fn swipe_up(app: &mut App) {
    for i in 0..=10 {
        app.world_mut()
            .resource_mut::<orbit_core::Touch>()
            .set(Some(Vec2::new(233.0, 330.0 - i as f32 * 12.0)));
        update(app);
    }
    app.world_mut()
        .resource_mut::<orbit_core::Touch>()
        .set(None);
    update(app);
}

fn open_card(app: &mut App) {
    choose(app, 0.0);
    assert_eq!(phase(app), HomePhase::Card { pet: 0 });
}

/// From the card, TRICKS, then the trick in `slot`.
fn open_trick(app: &mut App, slot: usize) {
    open_card(app);
    choose(app, 0.0);
    assert_eq!(phase(app), HomePhase::Tricks { pet: 0 });
    choose(app, slot as f32 * orbit_ui::LIST_STEP_DEG);
}

#[test]
fn an_empty_tank_says_to_go_fishing_and_pwr_goes_back_outside() {
    let mut app = home_with(vec![]);
    assert!(shows(&app, "THE TANK IS EMPTY"));
    click_action(&mut app);
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

#[test]
fn the_fish_swim_in_the_tank_in_their_hats() {
    let mut app = home_with(vec![pet("GILLY", Some(2)), pet("BUBBA", None)]);
    let species = angler_game::SPECIES[3].sprite;
    assert_eq!(drawn(&app, species), 2);
    assert_eq!(drawn(&app, HATS[2].sprite), 1);
    assert!(shows(&app, "GILLY") && shows(&app, "BUBBA") && shows(&app, "EXIT"));
    let before = app.world().resource::<Home>().swimmer(0);
    tick(&mut app, 60);
    assert_ne!(
        app.world().resource::<Home>().swimmer(0),
        before,
        "they swim"
    );
}

#[test]
fn turning_points_at_a_fish_and_its_card_shows_who_it_is_and_what_it_knows() {
    let mut app = home_with(vec![pet("GILLY", None), pet("BUBBA", None)]);
    choose(&mut app, 40.0);
    assert_eq!(phase(&app), HomePhase::Card { pet: 1 });
    for text in [
        "BUBBA",
        "TROUT",
        "STAMINA LV 1",
        "SPLASH",
        "RAINBOW LEAP",
        "LEGENDARY",
    ] {
        assert!(shows(&app, text), "{text}: {:?}", labels(&app));
    }
    choose(&mut app, 20.0);
    assert_eq!(phase(&app), HomePhase::Tank, "BACK on the right");
}

#[test]
fn a_tap_just_off_a_card_button_presses_it_and_between_two_the_nearer_one_wins() {
    let mut app = home_with(vec![pet("GILLY", None)]);
    choose(&mut app, 0.0);
    assert_eq!(phase(&app), HomePhase::Card { pet: 0 });
    let tricks = pals_game::home::ui::TRICKS.area;
    tap(&mut app, Vec2::new(233.0, tricks.top_left.y as f32 - 10.0));
    assert_eq!(
        phase(&app),
        HomePhase::Tricks { pet: 0 },
        "just above TRICKS"
    );

    let mut app = home_with(vec![pet("GILLY", None)]);
    choose(&mut app, 0.0);
    let edge = tricks.top_left.x as f32;
    tap(&mut app, Vec2::new(edge + 1.0, tricks.center().y as f32));
    assert_eq!(
        phase(&app),
        HomePhase::Tricks { pet: 0 },
        "on TRICKS beside RENAME"
    );
}

#[test]
fn turning_to_out_leaves_for_the_street() {
    let mut app = home_with(vec![pet("GILLY", None)]);
    set_tilt(&mut app, 60.0, 0.0);
    update(&mut app);
    click_action(&mut app);
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

#[test]
fn rename_types_a_new_name_by_turning_and_saves_it() {
    let mut app = home_with(vec![pet("GILLY", None)]);
    open_card(&mut app);
    choose(&mut app, -20.0);
    assert!(matches!(phase(&app), HomePhase::Rename { .. }));
    for _ in 0..5 {
        choose(&mut app, key_turn(Key::Delete));
    }
    for key in [Key::Letter('Z'), Key::Letter('O'), Key::Done] {
        choose(&mut app, key_turn(key));
    }
    assert_eq!(phase(&app), HomePhase::Card { pet: 0 });
    assert_eq!(saved_tank(&mut app).pets[0].name.as_str(), "ZO");
}

#[test]
fn a_trick_is_shown_then_taught_a_word_said_twice() {
    let mut app = home_with(vec![pet("GILLY", None)]);
    open_trick(&mut app, 1);
    assert!(matches!(
        phase(&app),
        HomePhase::Teach {
            step: TeachStep::Watch,
            ..
        }
    ));
    assert!(shows(&app, "WATCH GILLY SPIN"));
    tick(&mut app, 60 * 4);
    assert!(shows(&app, "SAY A WORD FOR SPIN"), "{:?}", labels(&app));
    speak(&mut app, &spoken("daniel-spin-170"));
    assert!(shows(&app, "SAY IT AGAIN"));
    speak(&mut app, &spoken("daniel-spin-240"));
    assert!(shows(&app, "GOT THE WORD"));
    click_action(&mut app);
    assert!(matches!(phase(&app), HomePhase::Practice { slot: 1, .. }));
    let saved = saved_tank(&mut app);
    assert!(saved.pets[0].tricks[1].trigger.voice.is_some());
}

#[test]
fn a_stroke_drawn_twice_teaches_a_trick_and_one_already_used_is_refused() {
    let mut app = home_with(vec![{
        let mut pet = pet("GILLY", None);
        pet.teach(
            0,
            Trigger {
                gesture: Some(Gesture::Tap),
                voice: None,
            },
        );
        pet
    }]);
    open_trick(&mut app, 2);
    tick(&mut app, 60 * 4);
    tap(&mut app, Vec2::new(233.0, 300.0));
    assert!(shows(&app, "TAP IS FOR SPLASH"));
    swipe_up(&mut app);
    swipe_up(&mut app);
    assert!(shows(&app, "GOT SWIPE UP"));
    click_action(&mut app);
    assert_eq!(
        tank(&app).pets[0].tricks[2].trigger.gesture,
        Some(Gesture::SwipeUp)
    );
}

/// The house, practicing SPIN with GILLY, who knows it by a swipe up at
/// `learned`.
fn practicing(learned: u8) -> App {
    let mut gilly = pet("GILLY", None);
    gilly.teach(
        1,
        Trigger {
            gesture: Some(Gesture::SwipeUp),
            voice: None,
        },
    );
    gilly.tricks[1].learned = learned;
    let mut app = home_with(vec![gilly]);
    open_trick(&mut app, 1);
    assert!(matches!(phase(&app), HomePhase::Practice { slot: 1, .. }));
    app
}

#[test]
fn the_command_makes_the_fish_try_and_petting_it_after_teaches_twice_as_much() {
    let mut app = practicing(10);
    assert!(shows(&app, "GIVE THE COMMAND"));
    app.world_mut().resource_mut::<Home>().rig_next_try(true);
    swipe_up(&mut app);
    tick(&mut app, 120);
    assert!(shows(&app, "PET GILLY!"), "{:?}", labels(&app));
    let fish = app.world().resource::<Home>().swimmer(0);
    tap(&mut app, fish);
    let praised = tank(&app).pets[0].tricks[1].learned - 10;
    assert!(shows(&app, &format!("GOOD FISH! +{praised}%")));

    let mut app = practicing(10);
    app.world_mut().resource_mut::<Home>().rig_next_try(true);
    swipe_up(&mut app);
    tick(&mut app, 60 * 6);
    let plain = tank(&app).pets[0].tricks[1].learned - 10;
    assert!(plain > 0 && praised > plain, "{praised} vs {plain}");
    assert_eq!(saved_tank(&mut app).pets[0].tricks[1].learned, 10 + plain);
}

#[test]
fn a_flubbed_try_teaches_nothing_and_pwr_ends_practice() {
    let mut app = practicing(10);
    app.world_mut().resource_mut::<Home>().rig_next_try(false);
    swipe_up(&mut app);
    tick(&mut app, 60 * 5);
    assert_eq!(tank(&app).pets[0].tricks[1].learned, 10);
    assert!(shows(&app, "NOT QUITE"), "{:?}", labels(&app));
    click_action(&mut app);
    assert_eq!(phase(&app), HomePhase::Tricks { pet: 0 });
}

/// From the tank, teaches SPIN (slot 1) the word in `takes`, said in turn.
fn teach_spin(app: &mut App, takes: &[&str]) {
    open_trick(app, 1);
    tick(app, 60 * 4);
    for take in takes {
        speak(app, &spoken(take));
    }
}

#[test]
fn a_command_said_a_little_differently_still_gets_the_trick() {
    let mut app = home_with(vec![pet("GILLY", None)]);
    teach_spin(&mut app, &["daniel-spin-170", "daniel-spin-170"]);
    click_action(&mut app);
    assert!(matches!(phase(&app), HomePhase::Practice { slot: 1, .. }));
    tick(&mut app, 70);
    app.world_mut().resource_mut::<Home>().rig_next_try(true);
    speak(&mut app, &spoken("daniel-spin-240"));
    assert!(
        matches!(
            phase(&app),
            HomePhase::Practice {
                step: PracticeStep::Trying { slot: 1, .. },
                ..
            } | HomePhase::Practice {
                step: PracticeStep::Praise,
                ..
            }
        ),
        "{:?}",
        phase(&app)
    );
}

#[test]
fn a_word_it_wasnt_taught_just_puzzles_the_fish() {
    let mut app = home_with(vec![pet("GILLY", None)]);
    teach_spin(&mut app, &["daniel-spin-170", "daniel-spin-240"]);
    click_action(&mut app);
    tick(&mut app, 70);
    speak(&mut app, &spoken("daniel-bubbles-170"));
    assert!(shows(&app, "GILLY LOOKS PUZZLED"), "{:?}", labels(&app));
    assert_eq!(tank(&app).pets[0].tricks[1].learned, 0);
}

#[test]
fn a_second_take_of_a_different_word_isnt_taken() {
    let mut app = home_with(vec![pet("GILLY", None)]);
    teach_spin(&mut app, &["daniel-spin-170", "daniel-bubbles-170"]);
    assert!(shows(&app, "THAT SOUNDED DIFFERENT"));
    assert!(shows(&app, "SAY IT AGAIN"));
}

#[test]
fn tapping_back_ends_practice() {
    let mut app = practicing(10);
    tap(&mut app, pals_game::home::ui::PRACTICE_BACK.center());
    assert_eq!(phase(&app), HomePhase::Tricks { pet: 0 });
}

#[test]
fn tapping_done_finishes_teaching_and_back_leaves_untaught() {
    let mut app = home_with(vec![pet("GILLY", None)]);
    open_trick(&mut app, 2);
    tick(&mut app, 60 * 4);
    assert!(matches!(
        phase(&app),
        HomePhase::Teach {
            step: TeachStep::Listen { .. },
            ..
        }
    ));
    assert!(shows(&app, "BACK"));
    tap(&mut app, pals_game::home::ui::TEACH_DONE.center());
    assert_eq!(phase(&app), HomePhase::Tricks { pet: 0 });

    let mut app = home_with(vec![pet("GILLY", None)]);
    open_trick(&mut app, 2);
    tick(&mut app, 60 * 4);
    swipe_up(&mut app);
    swipe_up(&mut app);
    assert!(shows(&app, "DONE"));
    tap(&mut app, pals_game::home::ui::TEACH_DONE.center());
    assert!(
        matches!(phase(&app), HomePhase::Practice { slot: 2, .. }),
        "{:?}",
        phase(&app)
    );
}
