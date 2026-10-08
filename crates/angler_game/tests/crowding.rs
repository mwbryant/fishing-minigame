//! Angler (the pond) played at random with an empty tank and with a full one
//! of long names, so catches go to the naming ring or the swap; the test
//! harness checks every frame for crowded text (`orbit_gfx::crowding`).

use angler_game::AnglerPlugin;
use orbit_core::{Launcher, Rng, SaveBlob, testing::*};
use pals::{Name, Pals, Pet};

fn saved(names: &[&str]) -> Option<Vec<u8>> {
    let mut tank = Pals::default();
    tank.earn(999_999);
    for (i, called) in names.iter().enumerate() {
        let mut name = Name::default();
        called.chars().for_each(|c| assert!(name.push(c)));
        let pet = Pet::new(i as u8, 3, 9_000, 100, name, &mut Rng::new(i as u32));
        tank.keep(pet).unwrap();
    }
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    blob.take_pending()
}

#[test]
fn played_at_random_with_any_tank_no_screen_crowds_its_text() {
    let tanks = [saved(&[]), saved(&["MMMMMMMM", "WWWWWWWW", "QQQQ QQQ"])];
    for (seed, tank) in tanks.into_iter().enumerate() {
        let mut app = headless(
            |app| {
                app.add_plugins(AnglerPlugin).insert_resource(Launcher);
            },
            tank,
        );
        monkey(&mut app, 180.0, seed as u32 + 1);
    }
}
