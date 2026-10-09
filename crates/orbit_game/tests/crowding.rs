//! Plays Orbit at random. The harness checks every frame for crowded text
//! (`orbit_gfx::crowding`).

use orbit_core::{Buttons, Tilt, testing::*};
use orbit_game::OrbitPlugin;

#[test]
fn played_at_random_no_screen_crowds_its_text() {
    let mut app = headless(
        |app| {
            app.add_plugins(OrbitPlugin)
                .init_resource::<Buttons>()
                .init_resource::<Tilt>();
        },
        None,
    );
    monkey(&mut app, 90.0, 1);
}
