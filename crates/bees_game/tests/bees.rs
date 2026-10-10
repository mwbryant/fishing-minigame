//! Bees??? played through the headless app: spilling, steadying the jar,
//! refills, the bees' hats and the saved best orbit count.

use bees_game::{BeesPlugin, CONTROL_GAIN, Orbit, Phase, audio, spill_limit_deg, ui};
use bevy::{app::AppExit, prelude::*};
use orbit_core::{ClipRef, Frame, Launcher, SaveBlob, SoundCommand, SoundQueue, testing::*};
use orbit_gfx::{Shape, SpriteRef};
use pals::{HATS, Name, Pals, Pet};

fn game(saved: Option<Vec<u8>>) -> App {
    headless(
        |app| {
            app.add_plugins(BeesPlugin);
        },
        saved,
    )
}

fn orbit(app: &App) -> &Orbit {
    app.world().resource::<Orbit>()
}

/// Turns the device to cancel the jar's wobble exactly.
fn steady_hand(app: &mut App, frames: u32) {
    for _ in 0..frames {
        let steer = orbit(app).drift_deg / CONTROL_GAIN;
        set_tilt(app, steer, 0.0);
        update(app);
    }
}

#[test]
fn the_title_explains_the_jar_and_pwr_starts_the_orbit() {
    let mut app = game(None);
    assert!(shows(&app, "BEES???"));
    assert!(shows(&app, "THE MOON IS A JAR OF HONEY"));
    assert!(shows(&app, "TURN TO KEEP IT FROM SPILLING"));
    click_action(&mut app);
    assert_eq!(orbit(&app).phase, Phase::Orbiting);
    let start = orbit(&app).angle_deg;
    tick(&mut app, 60);
    assert!(orbit(&app).angle_deg > start + 20.0, "the moon goes round");
}

#[test]
fn a_full_jar_spills_at_a_smaller_tilt_than_a_low_one() {
    assert!(spill_limit_deg(1.0) < spill_limit_deg(0.5));
    assert!(spill_limit_deg(0.5) < spill_limit_deg(0.1));
    assert!((35.0..=45.0).contains(&spill_limit_deg(1.0)));
}

#[test]
fn turning_against_the_wobble_keeps_the_honey_in() {
    let mut app = game(None);
    click_action(&mut app);
    steady_hand(&mut app, 60 * 40);
    assert_eq!(orbit(&app).phase, Phase::Orbiting);
    assert!(orbit(&app).honey > 0.95, "honey {}", orbit(&app).honey);
    assert!(orbit(&app).orbits >= 4, "{} orbits", orbit(&app).orbits);
    assert!(shows(&app, &format!("ORBIT {}", orbit(&app).orbits)));
}

#[test]
fn tipping_the_jar_spills_honey_until_it_runs_dry() {
    let mut app = game(None);
    click_action(&mut app);
    set_tilt(&mut app, 70.0, 0.0);
    tick(&mut app, 30);
    assert!(orbit(&app).honey < 1.0);
    assert!(orbit(&app).spilling());
    assert!(shows(&app, "SPILLING!"));
    assert!(
        app.world()
            .resource::<SoundQueue>()
            .pending()
            .contains(&SoundCommand::Play(ClipRef(&audio::DRIP)))
    );
    tick(&mut app, 60 * 15);
    assert_eq!(orbit(&app).phase, Phase::Empty);
    assert!(shows(&app, "THE JAR IS EMPTY"));
}

#[test]
fn bees_fly_refills_up_from_earth() {
    let mut app = game(None);
    click_action(&mut app);
    set_tilt(&mut app, 70.0, 0.0);
    tick(&mut app, 25);
    let low = orbit(&app).honey;
    steady_hand(&mut app, 60 * 10);
    assert!(
        orbit(&app).honey > low + 0.05,
        "{low} -> {}",
        orbit(&app).honey
    );
}

/// A save whose tank leads with a fish in `hat`, then one in no hat.
fn wearing(hat: usize) -> Option<Vec<u8>> {
    let mut tank = Pals::default();
    tank.earn(10_000);
    tank.buy_hat(hat);
    let mut rng = orbit_core::Rng::new(1);
    for _ in 0..2 {
        assert!(
            tank.keep(Pet::new(0, 0, 80, 50, Name::default(), &mut rng))
                .is_ok()
        );
    }
    tank.wear(0, Some(hat));
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    blob.take_pending()
}

fn draws(app: &App, sprite: &'static orbit_gfx::Sprite) -> usize {
    app.world()
        .resource::<Frame>()
        .shapes
        .iter()
        .filter(|s| matches!(s, Shape::Sprite { sprite: drawn, .. } if *drawn == SpriteRef(sprite)))
        .count()
}

#[test]
fn the_bees_wear_the_hat_the_lead_fish_wears_and_none_without_one() {
    let crown = HATS.iter().position(|hat| hat.name == "CROWN").unwrap();
    let mut app = game(wearing(crown));
    click_action(&mut app);
    tick(&mut app, 5);
    assert!(
        draws(&app, HATS[crown].sprite) >= 3,
        "every bee has the crown"
    );
    let plain = game(None);
    assert!(
        HATS.iter().all(|hat| draws(&plain, hat.sprite) == 0),
        "no fish, no hats"
    );
}

#[test]
fn the_best_orbit_count_is_saved_and_survives_a_reboot() {
    let mut app = game(None);
    click_action(&mut app);
    steady_hand(&mut app, 60 * 12);
    let orbits = orbit(&app).orbits;
    assert!(orbits >= 1);
    set_tilt(&mut app, 80.0, 0.0);
    tick(&mut app, 60 * 20);
    assert_eq!(orbit(&app).phase, Phase::Empty);
    assert!(shows(&app, "NEW BEST!"));
    let record = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    let rebooted = game(record);
    assert!(orbit(&rebooted).best >= orbits);
    assert!(shows(&rebooted, &format!("BEST {}", orbit(&rebooted).best)));
}

#[test]
fn launched_from_the_town_the_title_offers_a_way_out() {
    let app = game(None);
    assert!(!shows(&app, "EXIT"));
    let mut app = headless(
        |app| {
            app.add_plugins(BeesPlugin).insert_resource(Launcher);
        },
        None,
    );
    tap(&mut app, ui::HOME.center());
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}
