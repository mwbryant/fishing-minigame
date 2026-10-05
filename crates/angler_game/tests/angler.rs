use angler_game::{
    Difficulty, FishLog, GUTEN_MORGEN, HATS, Inventory, Phase, Session, Settings,
    cast::{self, CHARGE_SECS, Flight, scene::WATER_Y},
    reel::{LEAN_DEAD_ZONE_DEG, LEAN_FULL_DEG},
    save,
    session::{ESCAPED_SECS, NOTICE_SECS, RESULT_GUARD_SECS},
    sprites,
    testing::*,
    ui,
};
use bevy::prelude::*;
use orbit_core::{ClipRef, Frame, SaveBlob, SoundCommand, SoundQueue};
use orbit_gfx::{Point, Shape, Sprite, SpriteRef};

const SCREEN_CENTER: Vec2 = Vec2::splat(233.0);

fn labels(app: &App) -> Vec<String> {
    app.world()
        .resource::<Frame>()
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Label { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn shows(app: &App, text: &str) -> bool {
    labels(app).iter().any(|l| l == text)
}

fn session(app: &mut App) -> Mut<'_, Session> {
    app.world_mut().resource_mut::<Session>()
}

/// Frames to hold the button for a cast at `power` (the press frame starts
/// the meter at zero).
fn frames_for(power: f32) -> u32 {
    (power * CHARGE_SECS * 60.0).round() as u32 + 1
}

fn start_aiming(app: &mut App) {
    click_action(app);
    assert!(matches!(phase(app), Phase::Aim { .. }));
}

/// Aims, puts the fishing spot exactly where this cast will land, and casts.
fn cast_onto_spot(app: &mut App, steer: f32, power: f32) {
    set_tilt(app, steer, 0.0);
    let lands = Flight::launch(cast::elevation_deg(steer), power).landing_x();
    session(app).spot_x = lands;
    hold_action(app, frames_for(power));
    release_action(app);
}

fn reel_lean(app: &App) -> f32 {
    let Phase::Reel { reel, .. } = phase(app) else {
        return 0.0;
    };
    let push = ((reel.zone_y - reel.fish_y) * 30.0 + reel.zone_v * 3.6) / 1100.0;
    let push = push.clamp(-1.0, 1.0);
    push.signum() * (LEAN_DEAD_ZONE_DEG + push.abs() * LEAN_FULL_DEG)
}

/// Plays the fight with a steady hand until it ends.
fn reel_skillfully(app: &mut App) {
    for _ in 0..60 * 60 {
        if !matches!(phase(app), Phase::Reel { .. }) {
            return;
        }
        let lean = reel_lean(app);
        set_tilt(app, 0.0, lean);
        app.update();
    }
    panic!("fight never ended");
}

fn hook_a_fish(app: &mut App) {
    start_aiming(app);
    cast_onto_spot(app, 0.0, 0.7);
    assert!(run_until(app, 60 * 15, |p| matches!(p, Phase::Reel { .. })));
}

#[test]
fn the_game_boots_to_the_title_and_the_button_starts_aiming() {
    let mut app = headless_app();
    assert_eq!(phase(&app), Phase::Title);
    assert!(shows(&app, "GONE FISHIN"));
    start_aiming(&mut app);
    assert!(shows(&app, "HOLD BUTTON TO CAST"));
}

#[test]
fn tapping_the_title_log_button_opens_the_fish_log_and_tapping_again_closes_it() {
    let mut app = headless_app();
    tap(&mut app, ui::LOG.center());
    assert_eq!(phase(&app), Phase::Log);
    assert!(shows(&app, "FISH LOG"));
    tap(&mut app, SCREEN_CENTER);
    assert_eq!(phase(&app), Phase::Title);
}

#[test]
fn turning_the_device_counterclockwise_raises_the_aim() {
    let mut app = headless_app();
    start_aiming(&mut app);
    set_tilt(&mut app, -30.0, 0.0);
    app.update();
    assert!(shows(&app, "ANGLE 70"));
    set_tilt(&mut app, 20.0, 0.0);
    app.update();
    assert!(shows(&app, "ANGLE 20"));
}

#[test]
fn holding_charges_power_and_releasing_casts_at_the_aimed_angle() {
    let mut app = headless_app();
    start_aiming(&mut app);
    set_tilt(&mut app, -15.0, 0.0);
    hold_action(&mut app, frames_for(0.5));
    assert!(matches!(
        phase(&app),
        Phase::Aim {
            charging_secs: Some(_)
        }
    ));
    release_action(&mut app);
    let Phase::Flight { flight, .. } = phase(&app) else {
        panic!("expected a cast, got {:?}", phase(&app));
    };
    let expected = Flight::launch(55.0, 0.5).vel;
    assert!(
        flight.vel.distance(expected) < 25.0,
        "{:?} vs {expected:?}",
        flight.vel
    );
    assert!(flight.vel.y < 0.0);
}

fn preview_dots(app: &App) -> Vec<Point> {
    app.world()
        .resource::<Frame>()
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Disc { center, color, .. } if *color == angler_game::PREVIEW_DOT => {
                Some(*center)
            }
            _ => None,
        })
        .collect()
}

fn full_power_landing(steer: f32) -> f32 {
    Flight::launch(cast::elevation_deg(steer), 1.0).landing_x()
}

#[test]
fn the_full_power_path_is_dotted_while_aiming_before_any_press() {
    let mut app = headless_app();
    start_aiming(&mut app);
    set_tilt(&mut app, -10.0, 0.0);
    app.update();
    let dots = preview_dots(&app);
    assert!(dots.len() >= 8, "{} dots", dots.len());
    let end = *dots.last().unwrap();
    assert!(
        (end.x as f32 - full_power_landing(-10.0)).abs() <= 1.0,
        "{end:?}"
    );
    assert!((end.y as f32 - WATER_Y).abs() <= 1.0, "{end:?}");
}

#[test]
fn the_dotted_path_stays_put_while_power_charges() {
    let mut app = headless_app();
    start_aiming(&mut app);
    let before = preview_dots(&app);
    hold_action(&mut app, frames_for(0.4));
    assert_eq!(preview_dots(&app), before);
    let gauge = app
        .world()
        .resource::<Frame>()
        .shapes
        .iter()
        .any(|s| matches!(s, Shape::Arc { .. }));
    assert!(gauge);
}

#[test]
fn releasing_at_the_top_of_the_meter_lands_exactly_where_the_dots_end() {
    let mut app = headless_app();
    start_aiming(&mut app);
    let end = *preview_dots(&app).last().unwrap();
    hold_action(&mut app, frames_for(1.0));
    release_action(&mut app);
    let Phase::Flight { flight, perfect } = phase(&app) else {
        panic!("expected a cast, got {:?}", phase(&app));
    };
    assert!(perfect);
    assert!((flight.landing_x() - end.x as f32).abs() <= 1.0);
    assert!(shows(&app, "PERFECT!"));
}

#[test]
fn releasing_early_comes_up_short_of_the_dots() {
    let mut app = headless_app();
    start_aiming(&mut app);
    let end = *preview_dots(&app).last().unwrap();
    hold_action(&mut app, frames_for(0.6));
    release_action(&mut app);
    let Phase::Flight { flight, perfect } = phase(&app) else {
        panic!("expected a cast, got {:?}", phase(&app));
    };
    assert!(!perfect);
    assert!(flight.landing_x() < end.x as f32 - 40.0);
    assert!(!shows(&app, "PERFECT!"));
}

#[test]
fn no_dots_remain_once_the_bobber_is_cast() {
    let mut app = headless_app();
    start_aiming(&mut app);
    hold_action(&mut app, frames_for(1.0));
    release_action(&mut app);
    tick(&mut app, 20);
    assert!(preview_dots(&app).is_empty());
    assert!(run_until(&mut app, 600, |p| matches!(
        p,
        Phase::Waiting { .. }
    )));
    tick(&mut app, 40);
    let any_dots = app
        .world()
        .resource::<Frame>()
        .shapes
        .iter()
        .any(|s| matches!(s, Shape::Disc { radius: 2, .. }));
    assert!(!any_dots, "dots left on screen while waiting");
}

#[test]
fn holding_far_too_long_cancels_the_cast_before_the_power_key_can_shut_down() {
    let mut app = headless_app();
    start_aiming(&mut app);
    hold_action(&mut app, (cast::MAX_CHARGE_SECS * 60.0) as u32 + 5);
    assert_eq!(phase(&app), Phase::Notice("LET GO!"));
    tick(&mut app, (NOTICE_SECS * 60.0) as u32 + 2);
    release_action(&mut app);
    assert!(matches!(
        phase(&app),
        Phase::Aim {
            charging_secs: None
        }
    ));
}

#[test]
fn a_brief_press_is_a_slip_not_a_cast() {
    let mut app = headless_app();
    start_aiming(&mut app);
    click_action(&mut app);
    assert!(matches!(
        phase(&app),
        Phase::Aim {
            charging_secs: None
        }
    ));
}

#[test]
fn a_cast_onto_the_spot_gets_a_bite_and_starts_the_fight() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    assert!(shows(&app, "LEAN TO REEL"));
}

#[test]
fn reeling_the_fish_in_shows_it_and_saves_the_log() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    reel_skillfully(&mut app);
    let Phase::Caught { catch, milestones } = phase(&app) else {
        panic!("expected a catch, got {:?}", phase(&app));
    };
    assert!(milestones.new_species);
    assert!(shows(&app, catch.species().name));
    assert!(shows(&app, &angler_game::length_text(catch.length_mm)));
    let fish_drawn = app.world().resource::<Frame>().shapes.iter().any(|s| {
        matches!(s, Shape::Sprite { sprite, .. } if *sprite == SpriteRef(catch.species().sprite))
    });
    assert!(fish_drawn);

    let log = app.world().resource::<FishLog>().clone();
    assert_eq!(log.total, 1);
    let saved = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    assert_eq!(
        saved.and_then(|b| save::decode(&b)).map(|(log, ..)| log),
        Some(log)
    );
}

#[test]
fn presses_right_after_a_catch_do_not_skip_the_result() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    reel_skillfully(&mut app);
    click_action(&mut app);
    assert!(matches!(phase(&app), Phase::Caught { .. }));
    tick(&mut app, (RESULT_GUARD_SECS * 60.0) as u32);
    click_action(&mut app);
    assert!(matches!(phase(&app), Phase::Aim { .. }));
}

#[test]
fn ignoring_the_fight_lets_the_fish_escape_and_play_resumes() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    set_tilt(&mut app, 0.0, 0.0);
    assert!(run_until(&mut app, 60 * 30, |p| *p == Phase::Escaped));
    assert!(shows(&app, "IT GOT AWAY..."));
    tick(&mut app, (ESCAPED_SECS * 60.0) as u32 + 2);
    assert!(matches!(phase(&app), Phase::Aim { .. }));
}

#[test]
fn pressing_while_waiting_reels_the_line_back_in() {
    let mut app = headless_app();
    start_aiming(&mut app);
    cast_onto_spot(&mut app, 0.0, 0.7);
    assert!(run_until(&mut app, 600, |p| matches!(
        p,
        Phase::Waiting { .. }
    )));
    click_action(&mut app);
    assert_eq!(phase(&app), Phase::Notice("REELED IN"));
    tick(&mut app, (NOTICE_SECS * 60.0) as u32 + 2);
    assert!(matches!(phase(&app), Phase::Aim { .. }));
}

#[test]
fn a_steep_weak_cast_lands_on_the_dock() {
    let mut app = headless_app();
    start_aiming(&mut app);
    set_tilt(&mut app, -45.0, 0.0);
    hold_action(&mut app, frames_for(0.1));
    release_action(&mut app);
    assert!(run_until(&mut app, 600, |p| *p == Phase::Notice("ON THE DOCK!")));
}

#[test]
fn the_fish_log_survives_a_reboot() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    reel_skillfully(&mut app);
    let saved = app
        .world_mut()
        .resource_mut::<SaveBlob>()
        .take_pending()
        .unwrap();
    let rebooted = headless_app_with_save(Some(saved));
    assert_eq!(rebooted.world().resource::<FishLog>().total, 1);
    assert!(shows(&rebooted, "CAUGHT 1"));
}

#[test]
fn garbage_in_storage_starts_a_fresh_log() {
    let app = headless_app_with_save(Some(vec![1, 2, 3]));
    assert_eq!(*app.world().resource::<FishLog>(), FishLog::default());
}

#[test]
fn the_zero_button_confirms_from_any_screen() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    press_zero(&mut app);
    assert!(shows(&app, "LEVEL SET"));
    tick(&mut app, 90);
    assert!(!shows(&app, "LEVEL SET"));
}

fn open_settings(app: &mut App) {
    tap(app, ui::SETTINGS.center());
    assert_eq!(phase(app), Phase::Settings);
}

fn difficulty(app: &App) -> Difficulty {
    app.world().resource::<Settings>().difficulty
}

fn sounds(app: &App) -> Vec<SoundCommand> {
    app.world().resource::<SoundQueue>().pending().to_vec()
}

#[test]
fn the_title_has_log_and_settings_buttons_and_stray_taps_do_nothing() {
    let mut app = headless_app();
    assert!(shows(&app, "LOG") && shows(&app, "SETTINGS"));
    tap(&mut app, Vec2::new(233.0, 300.0));
    assert_eq!(phase(&app), Phase::Title);
    open_settings(&mut app);
    assert!(shows(&app, "DIFFICULTY") && shows(&app, "NORMAL"));
}

#[test]
fn the_arrows_step_the_difficulty_and_each_change_is_saved() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::HARDER.center());
    assert_eq!(difficulty(&app), Difficulty::Hard);
    assert!(shows(&app, "HARD"));
    let saved = app
        .world_mut()
        .resource_mut::<SaveBlob>()
        .take_pending()
        .and_then(|b| save::decode(&b));
    assert_eq!(
        saved.map(|(_, settings, _)| settings.difficulty),
        Some(Difficulty::Hard)
    );
    tap(&mut app, ui::HARDER.center());
    assert_eq!(difficulty(&app), Difficulty::Easy);
    tap(&mut app, ui::EASIER.center());
    tap(&mut app, ui::EASIER.center());
    assert_eq!(difficulty(&app), Difficulty::Normal);
}

#[test]
fn the_chosen_difficulty_survives_a_reboot() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::EASIER.center());
    let saved = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    let rebooted = headless_app_with_save(saved);
    assert_eq!(difficulty(&rebooted), Difficulty::Easy);
}

#[test]
fn back_or_the_action_button_leaves_settings() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::BACK.center());
    assert_eq!(phase(&app), Phase::Title);
    open_settings(&mut app);
    click_action(&mut app);
    assert_eq!(phase(&app), Phase::Title);
}

#[test]
fn easy_widens_the_perfect_window_so_a_slightly_early_release_still_hits_the_dots() {
    let release_at_92 = |app: &mut App| {
        start_aiming(app);
        hold_action(app, frames_for(0.92));
        release_action(app);
        matches!(phase(app), Phase::Flight { perfect: true, .. })
    };
    let mut normal = headless_app();
    assert!(!release_at_92(&mut normal));
    let mut easy = headless_app();
    open_settings(&mut easy);
    tap(&mut easy, ui::EASIER.center());
    tap(&mut easy, ui::BACK.center());
    assert!(release_at_92(&mut easy));
}

#[test]
fn the_fight_uses_the_difficultys_catch_zone() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::HARDER.center());
    tap(&mut app, ui::BACK.center());
    hook_a_fish(&mut app);
    let Phase::Reel { reel, .. } = phase(&app) else {
        panic!("expected a fight");
    };
    assert_eq!(reel.zone_height, Difficulty::Hard.tuning().zone_height);
}

#[test]
fn guten_plays_the_song_and_spins_the_trout() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::GUTEN.center());
    assert_eq!(phase(&app), Phase::Guten);
    assert_eq!(
        sounds(&app),
        vec![SoundCommand::Play(ClipRef(&GUTEN_MORGEN))]
    );
    assert!(shows(&app, "GUTEN MORGEN"));
    let trout_width = |app: &App| {
        app.world()
            .resource::<Frame>()
            .shapes
            .iter()
            .find_map(|s| match s {
                Shape::SpriteFit { sprite, area, .. } if *sprite == SpriteRef(&sprites::TROUT) => {
                    Some(area.size.width)
                }
                _ => None,
            })
            .expect("the trout is on screen")
    };
    let facing = trout_width(&app);
    tick(&mut app, 34);
    assert!(trout_width(&app) < facing / 2, "the trout turns side-on");
}

#[test]
fn guten_lasts_as_long_as_the_song_then_returns_to_settings() {
    assert!((GUTEN_MORGEN.duration_secs() - 15.15).abs() < 0.05);
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::GUTEN.center());
    tick(&mut app, (GUTEN_MORGEN.duration_secs() * 60.0) as u32 - 10);
    assert_eq!(phase(&app), Phase::Guten);
    tick(&mut app, 12);
    assert_eq!(phase(&app), Phase::Settings);
    assert_eq!(sounds(&app).last(), Some(&SoundCommand::Stop));
}

#[test]
fn a_tap_stops_guten_early() {
    let mut app = headless_app();
    open_settings(&mut app);
    tap(&mut app, ui::GUTEN.center());
    tick(&mut app, 60);
    tap(&mut app, SCREEN_CENTER);
    assert_eq!(phase(&app), Phase::Settings);
    assert_eq!(sounds(&app).last(), Some(&SoundCommand::Stop));
}

fn inventory(app: &App) -> Inventory {
    *app.world().resource::<Inventory>()
}

fn give_gold(app: &mut App, gold: u32) {
    app.world_mut().resource_mut::<Inventory>().earn(gold);
}

fn saved_inventory(app: &mut App) -> Option<Inventory> {
    let pending = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    pending
        .and_then(|bytes| save::decode(&bytes))
        .map(|(.., inventory)| inventory)
}

fn sprite_drawn(app: &App, sprite: &'static Sprite) -> bool {
    app.world().resource::<Frame>().shapes.iter().any(
        |shape| matches!(shape, Shape::Sprite { sprite: drawn, .. } if *drawn == SpriteRef(sprite)),
    )
}

fn open_shop(app: &mut App) {
    tap(app, ui::SHOP.center());
    assert!(matches!(phase(app), Phase::Shop { .. }), "{:?}", phase(app));
}

fn browse_to(app: &mut App, hat: usize) {
    for _ in 0..HATS.len() {
        if phase(app) == (Phase::Shop { hat }) {
            return;
        }
        tap(app, ui::NEXT_HAT.center());
    }
    panic!("never reached hat {hat}");
}

const BEANIE: usize = 1;
const BALL_CAP: usize = 2;

#[test]
fn landing_a_fish_pays_its_gold_and_saves_it() {
    let mut app = headless_app();
    hook_a_fish(&mut app);
    reel_skillfully(&mut app);
    let Phase::Caught { catch, .. } = phase(&app) else {
        panic!("expected a catch, got {:?}", phase(&app));
    };
    assert!(catch.gold() > 0);
    assert_eq!(inventory(&app).gold, catch.gold());
    assert!(shows(&app, &format!("+{} GOLD", catch.gold())));
    let expected = inventory(&app);
    assert_eq!(saved_inventory(&mut app), Some(expected));
}

#[test]
fn the_title_shows_the_gold_and_its_shop_button_opens_on_the_worn_hat() {
    let mut purse = Inventory::default();
    purse.earn(42);
    let saved = save::encode(&FishLog::default(), &Settings::default(), &purse);
    let mut app = headless_app_with_save(Some(saved));
    assert!(shows(&app, "GOLD 42") && shows(&app, "SHOP"));
    open_shop(&mut app);
    assert_eq!(phase(&app), Phase::Shop { hat: 0 });
    assert!(shows(&app, "HAT SHOP") && shows(&app, "GOLD 42"));
    assert!(shows(&app, "STRAW HAT") && shows(&app, "WEARING"));
}

#[test]
fn the_arrows_browse_the_shelf_and_the_mirror_tries_each_hat_on() {
    let mut app = headless_app();
    open_shop(&mut app);
    tap(&mut app, ui::NEXT_HAT.center());
    assert_eq!(phase(&app), Phase::Shop { hat: BEANIE });
    assert!(shows(&app, HATS[BEANIE].name));
    assert!(shows(&app, &format!("{} GOLD", HATS[BEANIE].price)));
    assert!(sprite_drawn(&app, HATS[BEANIE].sprite));
    assert!(sprite_drawn(&app, &sprites::ANGLER));
    tap(&mut app, ui::PREVIOUS_HAT.center());
    tap(&mut app, ui::PREVIOUS_HAT.center());
    assert_eq!(
        phase(&app),
        Phase::Shop {
            hat: HATS.len() - 1
        }
    );
    assert_eq!(inventory(&app), Inventory::default(), "browsing is free");
}

#[test]
fn buying_a_hat_spends_the_gold_puts_it_on_and_saves() {
    let mut app = headless_app();
    give_gold(&mut app, HATS[BEANIE].price + 5);
    open_shop(&mut app);
    browse_to(&mut app, BEANIE);
    assert!(shows(&app, "BUY"));
    tap(&mut app, ui::BUY.center());
    assert_eq!(inventory(&app).gold, 5);
    assert_eq!(inventory(&app).worn(), BEANIE);
    assert!(shows(&app, "WEARING") && !shows(&app, "BUY"));
    let expected = inventory(&app);
    assert_eq!(saved_inventory(&mut app), Some(expected));
}

#[test]
fn the_angler_on_the_dock_wears_the_chosen_hat() {
    let mut app = headless_app();
    assert!(sprite_drawn(&app, HATS[0].sprite));
    give_gold(&mut app, HATS[BEANIE].price);
    open_shop(&mut app);
    browse_to(&mut app, BEANIE);
    tap(&mut app, ui::BUY.center());
    tap(&mut app, ui::BACK.center());
    assert_eq!(phase(&app), Phase::Title);
    assert!(sprite_drawn(&app, HATS[BEANIE].sprite));
    assert!(!sprite_drawn(&app, HATS[0].sprite));
    start_aiming(&mut app);
    assert!(sprite_drawn(&app, HATS[BEANIE].sprite));
}

#[test]
fn a_hat_you_cannot_afford_says_how_much_more_gold_it_needs() {
    let mut app = headless_app();
    give_gold(&mut app, 10);
    open_shop(&mut app);
    browse_to(&mut app, BALL_CAP);
    assert!(shows(
        &app,
        &format!("NEED {} MORE", HATS[BALL_CAP].price - 10)
    ));
    assert!(!shows(&app, "BUY"));
    tap(&mut app, ui::BUY.center());
    assert_eq!(inventory(&app).gold, 10);
    assert!(!inventory(&app).owns(BALL_CAP));
    assert_eq!(saved_inventory(&mut app), None, "nothing to save");
}

#[test]
fn owned_hats_can_be_put_back_on_for_free() {
    let mut app = headless_app();
    give_gold(&mut app, HATS[BEANIE].price);
    open_shop(&mut app);
    browse_to(&mut app, BEANIE);
    tap(&mut app, ui::BUY.center());
    tap(&mut app, ui::PREVIOUS_HAT.center());
    assert!(shows(&app, "OWNED") && shows(&app, "WEAR"));
    tap(&mut app, ui::WEAR.center());
    assert_eq!(inventory(&app).worn(), 0);
    assert_eq!(inventory(&app).gold, 0);
    assert!(inventory(&app).owns(BEANIE));
    assert!(saved_inventory(&mut app).is_some_and(|saved| saved.worn() == 0));
}

#[test]
fn gold_and_hats_survive_a_reboot() {
    let mut app = headless_app();
    give_gold(&mut app, 100);
    open_shop(&mut app);
    browse_to(&mut app, BEANIE);
    tap(&mut app, ui::BUY.center());
    let saved = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    let rebooted = headless_app_with_save(saved);
    assert_eq!(inventory(&rebooted), inventory(&app));
    assert!(shows(
        &rebooted,
        &format!("GOLD {}", 100 - HATS[BEANIE].price)
    ));
    assert!(sprite_drawn(&rebooted, HATS[BEANIE].sprite));
}

#[test]
fn back_or_the_action_button_leaves_the_shop() {
    let mut app = headless_app();
    open_shop(&mut app);
    tap(&mut app, ui::BACK.center());
    assert_eq!(phase(&app), Phase::Title);
    open_shop(&mut app);
    click_action(&mut app);
    assert_eq!(phase(&app), Phase::Title);
}

#[test]
fn the_menu_button_while_aiming_goes_home_so_gold_can_be_spent() {
    let mut app = headless_app();
    start_aiming(&mut app);
    assert!(shows(&app, "MENU"));
    tap(&mut app, ui::MENU.center());
    assert_eq!(phase(&app), Phase::Title);
    open_shop(&mut app);
}

#[test]
fn the_menu_button_hides_while_charging_and_a_tap_there_keeps_the_cast() {
    let mut app = headless_app();
    start_aiming(&mut app);
    hold_action(&mut app, frames_for(0.3));
    assert!(!shows(&app, "MENU"));
    app.world_mut()
        .resource_mut::<orbit_core::Touch>()
        .set(Some(ui::MENU.center()));
    app.update();
    assert!(matches!(
        phase(&app),
        Phase::Aim {
            charging_secs: Some(_)
        }
    ));
}
