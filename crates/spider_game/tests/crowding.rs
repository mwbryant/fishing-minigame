//! Plays Spider Roulette at random with purses from empty to a fortune, so the
//! bet dial shows every count of stakes. The harness checks every frame for
//! crowded text (`orbit_gfx::crowding`).

use orbit_core::{Launcher, SaveBlob, testing::*};
use pals::Pals;
use spider_game::SpiderPlugin;

#[test]
fn played_at_random_with_any_purse_no_screen_crowds_its_text() {
    for (seed, gold) in [(1, 0), (2, 7), (3, 12), (4, 30), (5, 60), (6, 999_999)] {
        let mut tank = Pals::default();
        tank.earn(gold);
        let mut blob = SaveBlob::from_storage(None);
        pals::save::store(&mut blob, &tank);
        let mut app = headless(
            |app| {
                app.add_plugins(SpiderPlugin).insert_resource(Launcher);
            },
            blob.take_pending(),
        );
        monkey(&mut app, 60.0, seed);
    }
}
