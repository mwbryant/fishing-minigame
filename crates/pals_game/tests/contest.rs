use bevy::{app::AppExit, prelude::*};
use orbit_core::{SaveBlob, testing::*};
use pals::{Gesture, Name, Pals, Pet, TRICKS, Trigger};
use pals_game::contest::{Contest, ContestPhase, ContestPlugin, SHOW_SECS, TIERS};

fn name(text: &str) -> Name {
    let mut name = Name::default();
    text.chars().for_each(|c| assert!(name.push(c)));
    name
}

/// A pet that knows SPLASH (swipe up) and RAINBOW LEAP (swipe down) at
/// `learned`, having walked `walked` metres.
fn performer(walked: f32, learned: u8) -> Pet {
    let mut pet = Pet::new(7, 3, 600, 80, name("STAR"), &mut orbit_core::Rng::new(3));
    for (slot, (trick, gesture)) in [
        ("SPLASH", Gesture::SwipeUp),
        ("RAINBOW LEAP", Gesture::SwipeDown),
    ]
    .into_iter()
    .enumerate()
    {
        pet.tricks[slot].trick = TRICKS.iter().position(|t| t.name == trick).unwrap() as u8;
        pet.teach(
            slot,
            Trigger {
                gesture: Some(gesture),
                voice: None,
            },
        );
        pet.tricks[slot].learned = learned;
    }
    pet.walk(walked);
    pet
}

fn hall(pets: Vec<Pet>, gold: u32) -> App {
    let mut tank = Pals::default();
    tank.earn(gold);
    pets.into_iter().for_each(|pet| tank.keep(pet).unwrap());
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    headless(
        |app| {
            app.add_plugins(ContestPlugin);
        },
        blob.take_pending(),
    )
}

fn phase(app: &App) -> ContestPhase {
    (*app.world().resource::<Contest>().phase).clone()
}

fn contest(app: &mut App) -> Mut<'_, Contest> {
    app.world_mut().resource_mut::<Contest>()
}

fn swipe(app: &mut App, down: bool) {
    let sign = if down { 1.0 } else { -1.0 };
    for i in 0..=10 {
        app.world_mut()
            .resource_mut::<orbit_core::Touch>()
            .set(Some(Vec2::new(233.0, 230.0 + sign * i as f32 * 12.0)));
        update(app);
    }
    app.world_mut()
        .resource_mut::<orbit_core::Touch>()
        .set(None);
    update(app);
}

/// Picks the first fish and the tier `tier` steps down the list.
fn enter(app: &mut App, tier: usize) {
    click_action(app);
    assert_eq!(phase(app), ContestPhase::ChooseTier { pet: 0 });
    set_tilt(app, tier as f32 * orbit_ui::LIST_STEP_DEG, 0.0);
    update(app);
    click_action(app);
    set_tilt(app, 0.0, 0.0);
    update(app);
}

fn score(app: &App) -> u32 {
    match phase(app) {
        ContestPhase::Show { show, .. } => show.score,
        other => panic!("not in a show: {other:?}"),
    }
}

fn stamina(app: &App) -> u32 {
    match phase(app) {
        ContestPhase::Show { show, .. } => show.stamina,
        other => panic!("not in a show: {other:?}"),
    }
}

#[test]
fn every_fish_can_enter_beginner_but_higher_tiers_need_stamina() {
    let mut app = hall(vec![performer(0.0, 50)], 0);
    click_action(&mut app);
    assert!(shows(&app, "BEGINNER") && shows(&app, "NEEDS LV 3"));
    set_tilt(&mut app, orbit_ui::LIST_STEP_DEG, 0.0);
    update(&mut app);
    click_action(&mut app);
    assert_eq!(
        phase(&app),
        ContestPhase::ChooseTier { pet: 0 },
        "OPEN is locked"
    );
    let mut app = hall(vec![performer(400.0, 50)], 0);
    enter(&mut app, 1);
    assert!(matches!(phase(&app), ContestPhase::Show { tier: 1, .. }));
}

#[test]
fn a_command_in_the_show_makes_the_fish_perform_for_points_and_stamina() {
    let mut app = hall(vec![performer(0.0, 50)], 0);
    enter(&mut app, 0);
    assert!(shows(&app, "GO!"));
    let full = stamina(&app);
    contest(&mut app).rig_next_try(true);
    swipe(&mut app, false);
    tick(&mut app, 120);
    assert!(score(&app) > 0, "SPLASH scored");
    assert!(stamina(&app) < full);
}

#[test]
fn doing_the_trick_a_judge_calls_for_scores_double() {
    let plain = {
        let mut app = hall(vec![performer(0.0, 50)], 0);
        enter(&mut app, 0);
        contest(&mut app).rig_next_try(true);
        swipe(&mut app, false);
        tick(&mut app, 120);
        score(&app)
    };
    let mut app = hall(vec![performer(0.0, 50)], 0);
    enter(&mut app, 0);
    contest(&mut app).rig_request(0);
    update(&mut app);
    assert!(shows(&app, "SPLASH!"), "{:?}", labels(&app));
    contest(&mut app).rig_next_try(true);
    swipe(&mut app, false);
    tick(&mut app, 120);
    assert_eq!(score(&app), plain * 2);
}

#[test]
fn a_flubbed_trick_scores_nothing() {
    let mut app = hall(vec![performer(0.0, 50)], 0);
    enter(&mut app, 0);
    contest(&mut app).rig_next_try(false);
    swipe(&mut app, false);
    tick(&mut app, 120);
    assert_eq!(score(&app), 0);
}

#[test]
fn a_tired_fish_ends_the_show_early() {
    let mut app = hall(vec![performer(0.0, 100)], 0);
    enter(&mut app, 0);
    for _ in 0..2 {
        swipe(&mut app, true);
        tick(&mut app, 120);
    }
    assert!(
        matches!(phase(&app), ContestPhase::Results { .. }),
        "two RAINBOW LEAPs and STAR is spent: {:?}",
        phase(&app)
    );
    assert!(shows(&app, "TOO TIRED TO GO ON"));
}

#[test]
fn the_show_lasts_thirty_seconds_then_the_judges_rank_you_against_the_rivals() {
    let mut app = hall(vec![performer(0.0, 50)], 0);
    enter(&mut app, 0);
    tick(&mut app, (SHOW_SECS * 60.0) as u32 - 30);
    assert!(matches!(phase(&app), ContestPhase::Show { .. }));
    tick(&mut app, 60);
    assert!(matches!(phase(&app), ContestPhase::Results { .. }));
    assert!(shows(&app, "STAR"), "your score is listed");
}

#[test]
fn winning_pays_the_prize_into_the_saved_purse_and_losing_pays_nothing() {
    let mut app = hall(vec![performer(0.0, 100)], 7);
    enter(&mut app, 0);
    contest(&mut app).rig_rivals([0, 0, 0]);
    swipe(&mut app, false);
    tick(&mut app, (SHOW_SECS * 60.0) as u32);
    assert!(shows(&app, "1ST PLACE!"), "{:?}", labels(&app));
    let prize = TIERS[0].prizes[0];
    assert!(shows(&app, &format!("+{prize} GOLD")));
    assert_eq!(app.world().resource::<Pals>().gold, 7 + prize);
    let pending = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    assert_eq!(
        pals::save::load(&SaveBlob::from_storage(pending)).gold,
        7 + prize
    );

    let mut app = hall(vec![performer(0.0, 100)], 7);
    enter(&mut app, 0);
    contest(&mut app).rig_rivals([9_000, 9_000, 9_000]);
    tick(&mut app, (SHOW_SECS * 60.0) as u32 + 10);
    assert!(shows(&app, "4TH PLACE"));
    assert_eq!(app.world().resource::<Pals>().gold, 7);
}

#[test]
fn a_rare_good_looking_fish_scores_its_appeal_even_without_tricks() {
    let mut app = hall(vec![performer(0.0, 50)], 0);
    enter(&mut app, 0);
    tick(&mut app, (SHOW_SECS * 60.0) as u32 + 10);
    let ContestPhase::Results { score, .. } = phase(&app) else {
        panic!("{:?}", phase(&app));
    };
    let appeal = app.world().resource::<Pals>().pets[0].appeal();
    assert_eq!(score, appeal);
}

#[test]
fn back_from_choosing_a_fish_leaves_the_hall() {
    let mut app = hall(vec![performer(0.0, 50)], 0);
    set_tilt(&mut app, 60.0, 0.0);
    update(&mut app);
    click_action(&mut app);
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}
