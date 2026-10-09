//! Every Fishing Minigame screen at its most crowded: a full tank with the
//! longest names and the tallest hat, every level and purse at its biggest,
//! played at random and put through every trick. The harness checks each frame
//! for crowded text (`orbit_gfx::crowding`).

use angler_game::SPECIES;
use bevy::prelude::*;
use orbit_core::{Rng, SaveBlob, Touch, testing::*};
use pals::{Gesture, HATS, Name, Pals, Pet, TRICKS, Trigger};
use pals_game::{
    contest::{Contest, ContestPlugin},
    home::{Home, HomePlugin},
    shop::ShopPlugin,
    town::TownPlugin,
    walk::WalkPlugin,
};

const NAMES: [&str; 3] = ["MMMMMMMM", "WWWWWWWW", "QQQQ QQQ"];

fn name(text: &str) -> Name {
    let mut name = Name::default();
    text.chars().for_each(|c| assert!(name.push(c)));
    name
}

fn tallest_hat() -> u8 {
    (0..HATS.len())
        .max_by_key(|&hat| HATS[hat].sprite.size().height)
        .unwrap() as u8
}

/// The biggest of `species`, in the tallest hat, walked as far as can be.
fn champion(species: u8, called: &str) -> Pet {
    let kind = &SPECIES[usize::from(species)];
    let mut pet = Pet::new(
        species,
        kind.tier,
        (kind.length_cm.1 * 10.0) as u16,
        100,
        name(called),
        &mut Rng::new(u32::from(species)),
    );
    pet.hat = Some(tallest_hat());
    pet.walk(f32::MAX);
    pet
}

fn saved(pets: Vec<Pet>) -> Option<Vec<u8>> {
    let mut tank = Pals::default();
    tank.earn(999_999);
    for hat in 0..HATS.len() {
        tank.buy_hat(hat);
    }
    tank.earn(999_999);
    tank.mock_walk = true;
    tank.mock_pace = pals::MOCK_PACES.len() - 1;
    pets.into_iter().for_each(|pet| tank.keep(pet).unwrap());
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    blob.take_pending()
}

fn full_tank(first_species: u8) -> Option<Vec<u8>> {
    let pets = NAMES
        .iter()
        .enumerate()
        .map(|(i, called)| champion((first_species + i as u8) % SPECIES.len() as u8, called))
        .collect();
    saved(pets)
}

fn app_with(plugin: impl Plugin, saved: Option<Vec<u8>>) -> App {
    let mut plugin = Some(plugin);
    headless(
        |app| {
            app.add_plugins(plugin.take().unwrap());
        },
        saved,
    )
}

#[test]
fn played_at_random_with_a_full_tank_no_place_crowds_its_text() {
    for (seed, first_species) in [(1, 0), (2, 3), (3, 5)] {
        let tank = || full_tank(first_species);
        monkey(&mut app_with(TownPlugin, tank()), 60.0, seed);
        monkey(&mut app_with(HomePlugin, tank()), 60.0, seed);
        monkey(&mut app_with(WalkPlugin, tank()), 60.0, seed);
        monkey(&mut app_with(ContestPlugin, tank()), 60.0, seed);
        monkey(&mut app_with(ShopPlugin, tank()), 60.0, seed);
    }
}

#[test]
fn with_one_fish_or_none_no_place_crowds_its_text() {
    for tank in [saved(vec![]), saved(vec![champion(6, "MMMMMMMM")])] {
        monkey(&mut app_with(TownPlugin, tank.clone()), 30.0, 7);
        monkey(&mut app_with(HomePlugin, tank.clone()), 30.0, 7);
        monkey(&mut app_with(WalkPlugin, tank.clone()), 30.0, 7);
        monkey(&mut app_with(ContestPlugin, tank.clone()), 30.0, 7);
        monkey(&mut app_with(ShopPlugin, tank), 30.0, 7);
    }
}

/// A champion that knows `trick` by an upward swipe, in its first slot.
fn knows(species: u8, trick: usize) -> Pet {
    let mut pet = champion(species, "MMMMMMMM");
    pet.tricks[0].trick = trick as u8;
    pet.teach(
        0,
        Trigger {
            gesture: Some(Gesture::SwipeUp),
            voice: None,
        },
    );
    pet.tricks[0].learned = 100;
    pet
}

fn swipe_up(app: &mut App, from_y: f32) {
    for i in 0..=10 {
        app.world_mut()
            .resource_mut::<Touch>()
            .set(Some(Vec2::new(233.0, from_y - i as f32 * 12.0)));
        update(app);
    }
    app.world_mut().resource_mut::<Touch>().set(None);
    update(app);
}

#[test]
fn every_trick_on_the_contest_stage_stays_clear_of_the_text() {
    for species in 0..SPECIES.len() as u8 {
        for trick in 0..TRICKS.len() {
            let mut app = app_with(ContestPlugin, saved(vec![knows(species, trick)]));
            click_action(&mut app);
            click_action(&mut app);
            app.world_mut().resource_mut::<Contest>().rig_request(0);
            app.world_mut().resource_mut::<Contest>().rig_next_try(true);
            swipe_up(&mut app, 300.0);
            tick(&mut app, 120);
        }
    }
}

#[test]
fn every_trick_in_the_house_stays_clear_of_the_text() {
    for species in 0..SPECIES.len() as u8 {
        for trick in 0..TRICKS.len() {
            let mut app = app_with(HomePlugin, saved(vec![knows(species, trick)]));
            for _ in 0..3 {
                click_action(&mut app);
            }
            app.world_mut().resource_mut::<Home>().rig_next_try(true);
            swipe_up(&mut app, 330.0);
            tick(&mut app, 120);
        }
    }
}
