//! Spider Roulette played through the headless app: spins, the pounce, the
//! trap, the heartbeat, the saved best run, and betting gold.

use bevy::{app::AppExit, prelude::*};
use orbit_core::{ClipRef, Frame, Launcher, SaveBlob, SoundCommand, SoundQueue, testing::*};
use orbit_gfx::{Shape, SpriteRef};
use spider_game::{EGGS, Phase, SpiderPlugin, Table, audio, sprites, ui};

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

fn play(clip: &'static orbit_core::Clip) -> SoundCommand {
    SoundCommand::Play(ClipRef(clip))
}

/// Starts a free round from the title and hides the spider in `egg`.
fn round_with_spider_in(app: &mut App, egg: usize) {
    click_action(app);
    assert_eq!(table(app).phase, Phase::Ready);
    app.world_mut().resource_mut::<Table>().hide_spider(egg);
}

/// Spins (making it land on `egg`) and waits for the egg to crack.
fn spin_onto(app: &mut App, egg: usize) {
    app.world_mut().resource_mut::<Table>().rig_next_spin(egg);
    click_action(app);
    assert_eq!(table(app).phase, Phase::Spinning);
    tick(app, 60 * 4);
}

fn dancer_frames(app: &App) -> Vec<(SpriteRef, i32)> {
    app.world()
        .resource::<Frame>()
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Sprite {
                sprite, top_left, ..
            } if *sprite == SpriteRef(&sprites::DANCER[0])
                || *sprite == SpriteRef(&sprites::DANCER[1]) =>
            {
                Some((*sprite, top_left.y))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn the_title_has_a_spider_dancing_in_the_middle_and_pwr_starts_a_round() {
    let mut app = game(None);
    assert!(shows(&app, "SPIDER") && shows(&app, "ROULETTE"));
    assert!(shows(&app, "PRESS TO SPIN"));
    let mut seen = Vec::new();
    for _ in 0..40 {
        update(&mut app);
        seen.extend(dancer_frames(&app));
    }
    assert!(
        seen.iter()
            .any(|(s, _)| *s == SpriteRef(&sprites::DANCER[0]))
    );
    assert!(
        seen.iter()
            .any(|(s, _)| *s == SpriteRef(&sprites::DANCER[1])),
        "it dances"
    );
    assert!(seen.iter().any(|(_, y)| *y != seen[0].1), "and bobs");
    click_action(&mut app);
    assert_eq!(table(&app).phase, Phase::Ready);
}

#[test]
fn pwr_spins_the_pointer_round_until_it_stops_on_an_egg_that_cracks() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 7);
    app.world_mut().resource_mut::<Table>().rig_next_spin(3);
    click_action(&mut app);
    let start = table(&app).pointer_deg;
    tick(&mut app, 30);
    assert!(table(&app).pointer_deg > start + 360.0, "it whirls round");
    assert!(
        sounds(&app)
            .iter()
            .filter(|s| **s == play(&audio::TICK))
            .count()
            >= 3
    );
    tick(&mut app, 60 * 4);
    assert_eq!(table(&app).phase, Phase::Ready);
    assert!(table(&app).cracked(3), "it stopped on the rigged egg");
    assert_eq!(table(&app).safe, 1);
    assert!(shows(&app, "SAFE 1 | 1 IN 11"), "{:?}", labels(&app));
}

#[test]
fn turning_the_device_does_nothing() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 7);
    let before = table(&app).pointer_deg;
    set_tilt(&mut app, 40.0, 0.0);
    tick(&mut app, 30);
    assert_eq!(table(&app).pointer_deg, before);
    assert_eq!(table(&app).phase, Phase::Ready);
}

#[test]
fn a_spin_in_progress_ignores_more_presses() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 7);
    spin_onto(&mut app, 2);
    app.world_mut().resource_mut::<Table>().rig_next_spin(5);
    click_action(&mut app);
    tick(&mut app, 10);
    click_action(&mut app);
    tick(&mut app, 60 * 4);
    let cracked = (0..EGGS).filter(|&egg| table(&app).cracked(egg)).count();
    assert_eq!(cracked, 2, "one egg per spin");
}

#[test]
fn the_wheel_only_ever_stops_on_a_whole_egg() {
    let mut app = game(None);
    click_action(&mut app);
    let mut whole = EGGS;
    for _ in 0..EGGS {
        if table(&app).phase != Phase::Ready {
            break;
        }
        click_action(&mut app);
        tick(&mut app, 60 * 4);
        let now = (0..EGGS).filter(|&egg| !table(&app).cracked(egg)).count();
        assert_eq!(now, whole - 1, "each spin cracks exactly one new egg");
        whole = now;
    }
    assert!(matches!(
        table(&app).phase,
        Phase::Pounce | Phase::Bitten | Phase::Won
    ));
}

#[test]
fn landing_on_the_spiders_egg_makes_it_pounce() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 4);
    spin_onto(&mut app, 1);
    spin_onto(&mut app, 4);
    assert!(matches!(table(&app).phase, Phase::Pounce | Phase::Bitten));
    assert!(sounds(&app).contains(&play(&audio::SCREECH)));
    tick(&mut app, 60 * 2);
    assert_eq!(table(&app).phase, Phase::Bitten);
    assert!(shows(&app, "GOT YOU!") && shows(&app, "SAFE PICKS 1"));
    tick(&mut app, 60);
    click_action(&mut app);
    assert_eq!(table(&app).phase, Phase::Ready, "a press plays again");
    assert_eq!(table(&app).safe, 0);
}

#[test]
fn eleven_safe_spins_trap_the_spider() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 11);
    for egg in 0..EGGS - 1 {
        spin_onto(&mut app, egg);
    }
    assert_eq!(table(&app).phase, Phase::Won);
    assert!(shows(&app, "SPIDER TRAPPED!"));
    assert!(sounds(&app).contains(&play(&audio::WIN)));
}

#[test]
fn the_best_run_is_saved_and_survives_a_reboot() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 9);
    for egg in 0..4 {
        spin_onto(&mut app, egg);
    }
    spin_onto(&mut app, 9);
    tick(&mut app, 120);
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
            update(app);
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
        spin_onto(&mut app, egg);
    }
    let tense = beats(&mut app);
    assert!(
        tense > calm + 3,
        "{calm} beats at 1 in 12, {tense} at 1 in 3"
    );
}

#[test]
fn a_tap_spins_too() {
    let mut app = game(None);
    round_with_spider_in(&mut app, 7);
    tap(&mut app, Vec2::splat(233.0));
    assert_eq!(table(&app).phase, Phase::Spinning);
}

#[test]
fn launched_from_the_town_the_title_offers_a_way_out() {
    let app = game(None);
    assert!(!shows(&app, "OUT"));
    let mut app = headless(
        |app| {
            app.add_plugins(SpiderPlugin).insert_resource(Launcher);
        },
        None,
    );
    assert!(shows(&app, "OUT"));
    tap(&mut app, ui::HOME.center());
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

fn with_gold(gold: u32) -> App {
    let mut tank = pals::Pals::default();
    tank.earn(gold);
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    game(blob.take_pending())
}

fn purse(app: &App) -> u32 {
    app.world().resource::<pals::Pals>().gold
}

fn saved_purse(app: &mut App) -> u32 {
    let pending = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    pals::save::load(&SaveBlob::from_storage(pending)).gold
}

/// From the title, bets the stake `pointed` steps round the dial.
fn bet(app: &mut App, pointed: usize) {
    click_action(app);
    assert_eq!(table(app).phase, Phase::Bet);
    let count = spider_game::stakes(purse(app)).len();
    let deg = pointed as f32 * 360.0 / count as f32;
    let deg = if deg > 180.0 { deg - 360.0 } else { deg };
    set_tilt(app, deg / orbit_ui::DIAL_GAIN, 0.0);
    update(app);
    click_action(app);
    set_tilt(app, 0.0, 0.0);
    update(app);
    assert_eq!(table(app).phase, Phase::Ready);
}

#[test]
fn with_gold_the_den_takes_a_bet_before_the_first_spin() {
    let mut app = with_gold(60);
    click_action(&mut app);
    assert_eq!(table(&app).phase, Phase::Bet);
    assert!(shows(&app, "YOUR BET?"));
    assert_eq!(
        spider_game::stakes(60),
        [5, 10, 25, 50, 0],
        "only what you can pay, or free"
    );
    let mut app = with_gold(60);
    bet(&mut app, 2);
    assert_eq!(table(&app).stake, 25);
    assert_eq!(purse(&app), 35);
    assert_eq!(saved_purse(&mut app), 35);
}

#[test]
fn each_safe_egg_raises_what_the_bet_is_worth_and_cashing_out_banks_it() {
    let mut app = with_gold(100);
    bet(&mut app, 3);
    app.world_mut().resource_mut::<Table>().hide_spider(11);
    let worth = |safe| spider_game::cash_value(50, safe);
    assert!(worth(1) < 50 && worth(3) > 50 && worth(11) > worth(10));
    assert!(!shows(&app, "CASH OUT"), "nothing to cash yet");
    for egg in 0..3 {
        spin_onto(&mut app, egg);
    }
    let line = format!("BET 50 | WORTH {}", worth(3));
    assert!(shows(&app, &line), "{:?}", labels(&app));
    tap(&mut app, ui::CASH_OUT.center());
    assert_eq!(table(&app).phase, Phase::CashedOut);
    assert!(shows(&app, &format!("+{} GOLD", worth(3))));
    assert_eq!(purse(&app), 50 + worth(3));
    assert_eq!(saved_purse(&mut app), 50 + worth(3));
}

#[test]
fn the_spider_takes_the_stake() {
    let mut app = with_gold(100);
    bet(&mut app, 1);
    app.world_mut().resource_mut::<Table>().hide_spider(4);
    spin_onto(&mut app, 4);
    tick(&mut app, 60 * 2);
    assert_eq!(table(&app).phase, Phase::Bitten);
    assert!(shows(&app, "LOST 10 GOLD"));
    assert_eq!(purse(&app), 90);
}

#[test]
fn trapping_the_spider_pays_the_whole_run() {
    let mut app = with_gold(5);
    bet(&mut app, 0);
    app.world_mut().resource_mut::<Table>().hide_spider(11);
    for egg in 0..11 {
        spin_onto(&mut app, egg);
    }
    assert_eq!(table(&app).phase, Phase::Won);
    let prize = spider_game::cash_value(5, 11);
    assert_eq!(prize, 54);
    assert_eq!(purse(&app), prize);
}

#[test]
fn a_free_game_costs_and_pays_nothing() {
    let mut app = with_gold(30);
    let free = spider_game::stakes(30).len() - 1;
    bet(&mut app, free);
    assert_eq!(table(&app).stake, 0);
    assert_eq!(purse(&app), 30);
    app.world_mut().resource_mut::<Table>().hide_spider(11);
    spin_onto(&mut app, 0);
    assert!(!shows(&app, "CASH OUT"));
}
