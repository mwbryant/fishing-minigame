use bevy::{app::AppExit, prelude::*};
use orbit_core::{ClipRef, Launcher, SaveBlob, SoundCommand, SoundQueue, testing::*};
use spider_game::{EGGS, Phase, SpiderPlugin, Table, audio, ui};

fn game(saved: Option<Vec<u8>>) -> App {
    headless(
        |app| {
            app.add_plugins(SpiderPlugin);
        },
        saved,
    )
}

fn table(app: &App) -> &Table {
    app.world().resource::<Table>()
}

fn sounds(app: &App) -> Vec<SoundCommand> {
    app.world().resource::<SoundQueue>().pending().to_vec()
}

fn last_sound(app: &App) -> Option<SoundCommand> {
    sounds(app).last().copied()
}

fn play(clip: &'static orbit_core::Clip) -> SoundCommand {
    SoundCommand::Play(ClipRef(clip))
}

/// Starts a round with the spider in `egg`.
fn round_with_spider_in(app: &mut App, egg: usize) {
    click_action(app);
    assert_eq!(table(app).phase, Phase::Pick);
    app.world_mut().resource_mut::<Table>().hide_spider(egg);
}

/// Turns the device so the dial points at `egg` (30 degrees of dial each,
/// a third of that on the device), then presses PWR.
fn crack(app: &mut App, egg: usize) {
    let dial_deg = egg as f32 * 360.0 / EGGS as f32;
    let steer = if dial_deg > 180.0 {
        (dial_deg - 360.0) / 3.0
    } else {
        dial_deg / 3.0
    };
    set_tilt(app, steer, 0.0);
    click_action(app);
}

#[test]
fn the_title_explains_the_game_and_pwr_starts_a_round_at_one_in_twelve() {
    let mut app = game(None);
    assert!(shows(&app, "SPIDER") && shows(&app, "ROULETTE"));
    assert!(shows(&app, "TURN TO AIM, PWR TO CRACK"));
    click_action(&mut app);
    assert_eq!(table(&app).phase, Phase::Pick);
    assert!(shows(&app, "1 IN 12"), "{:?}", labels(&app));
}

#[test]
fn cracking_a_safe_egg_counts_it_and_shortens_the_odds() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 7);
    crack(&mut app, 0);
    assert!(table(&app).cracked(0));
    assert_eq!(table(&app).safe, 1);
    assert!(shows(&app, "1 IN 11") && shows(&app, "SAFE 1"));
    assert_eq!(last_sound(&app), Some(play(&audio::CRACK)));
    crack(&mut app, 2);
    assert!(table(&app).cracked(2) && !table(&app).cracked(1));
    assert!(shows(&app, "1 IN 10"));
}

#[test]
fn an_egg_already_cracked_only_thuds() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 7);
    crack(&mut app, 4);
    crack(&mut app, 4);
    assert_eq!(table(&app).safe, 1);
    assert_eq!(last_sound(&app), Some(play(&orbit_ui::THUD)));
}

#[test]
fn tapping_an_egg_cracks_it_too() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 7);
    tap(&mut app, ui::egg_center(5));
    assert!(table(&app).cracked(5));
}

#[test]
fn the_spider_egg_pounces_then_shows_how_far_you_got() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 3);
    crack(&mut app, 0);
    crack(&mut app, 3);
    assert_eq!(table(&app).phase, Phase::Pounce);
    assert_eq!(last_sound(&app), Some(play(&audio::SCREECH)));
    tick(&mut app, 80);
    assert_eq!(table(&app).phase, Phase::Bitten);
    assert!(shows(&app, "GOT YOU!") && shows(&app, "SAFE PICKS 1"));
    tick(&mut app, 60);
    click_action(&mut app);
    assert_eq!(table(&app).phase, Phase::Pick, "PWR plays again");
    assert_eq!(table(&app).safe, 0);
}

#[test]
fn cracking_every_safe_egg_traps_the_spider() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 11);
    for egg in 0..EGGS - 1 {
        crack(&mut app, egg);
    }
    assert_eq!(table(&app).phase, Phase::Won);
    assert!(shows(&app, "SPIDER TRAPPED!"));
    assert_eq!(last_sound(&app), Some(play(&audio::WIN)));
}

#[test]
fn the_best_run_is_saved_and_survives_a_reboot() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 9);
    for egg in 0..4 {
        crack(&mut app, egg);
    }
    crack(&mut app, 9);
    tick(&mut app, 80);
    assert!(shows(&app, "NEW BEST!"));
    let record = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    let rebooted = game(record);
    assert_eq!(table(&rebooted).best, 4);
    assert!(shows(&rebooted, "BEST 4"));
}

#[test]
fn the_heartbeat_quickens_as_the_eggs_run_out() {
    let beats = |app: &mut App| {
        app.world_mut()
            .resource_mut::<SoundQueue>()
            .drain()
            .for_each(drop);
        let mut count = 0;
        for _ in 0..60 * 4 {
            app.update();
            count += app
                .world_mut()
                .resource_mut::<SoundQueue>()
                .drain()
                .filter(|s| *s == play(&audio::HEARTBEAT))
                .count();
        }
        count
    };
    let mut app = game(None);
    round_with_spider_in(&mut app, 11);
    let calm = beats(&mut app);
    for egg in 0..9 {
        crack(&mut app, egg);
    }
    let tense = beats(&mut app);
    assert!(
        tense > calm + 3,
        "{calm} beats at 1 in 12, {tense} at 1 in 3"
    );
}

#[test]
fn launched_from_the_menu_the_title_offers_home() {
    let app = game(None);
    assert!(!shows(&app, "HOME"));
    let mut app = headless(
        |app| {
            app.add_plugins(SpiderPlugin).insert_resource(Launcher);
        },
        None,
    );
    assert!(shows(&app, "HOME"));
    tap(&mut app, ui::HOME.center());
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}
