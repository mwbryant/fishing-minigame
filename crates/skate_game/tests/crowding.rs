//! Plays Skate at random, street and halfpipe. The harness checks every frame
//! for crowded text (`orbit_gfx::crowding`).

use orbit_core::{Launcher, testing::*};
use skate_game::SkatePlugin;

#[test]
fn played_at_random_no_screen_crowds_its_text() {
    for seed in 1..=3 {
        let mut app = headless(
            |app| {
                app.add_plugins(SkatePlugin).insert_resource(Launcher);
            },
            None,
        );
        monkey(&mut app, 90.0, seed);
    }
}
