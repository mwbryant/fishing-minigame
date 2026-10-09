//! Plays Bees??? at random with the bees in the tallest hat. The harness
//! checks every frame for crowded text (`orbit_gfx::crowding`).

use bees_game::BeesPlugin;
use orbit_core::{Launcher, Rng, SaveBlob, testing::*};
use pals::{HATS, Name, Pals, Pet};

#[test]
fn played_at_random_no_screen_crowds_its_text() {
    let tallest = (0..HATS.len())
        .max_by_key(|&hat| HATS[hat].sprite.size().height)
        .unwrap();
    let mut lead = Pet::new(0, 0, 50, 50, Name::default(), &mut Rng::new(1));
    lead.hat = Some(tallest as u8);
    let mut tank = Pals::default();
    tank.keep(lead).unwrap();
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    let saved = blob.take_pending();
    for seed in 1..=3 {
        let mut app = headless(
            |app| {
                app.add_plugins(BeesPlugin).insert_resource(Launcher);
            },
            saved.clone(),
        );
        monkey(&mut app, 90.0, seed);
    }
}
