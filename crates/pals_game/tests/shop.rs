use bevy::{app::AppExit, prelude::*};
use orbit_core::{Frame, SaveBlob, testing::*};
use orbit_gfx::{Shape, Sprite, SpriteRef};
use pals::{HATS, Name, Pals, Pet};
use pals_game::{
    contest::TIERS,
    shop::{OUT, ShopPlugin, hat_turn, ui},
};

fn name(text: &str) -> Name {
    let mut name = Name::default();
    text.chars().for_each(|c| assert!(name.push(c)));
    name
}

fn shop(gold: u32, names: &[&str]) -> App {
    let mut tank = Pals::default();
    tank.earn(gold);
    for text in names {
        tank.keep(Pet::new(
            3,
            1,
            300,
            50,
            name(text),
            &mut orbit_core::Rng::new(1),
        ))
        .unwrap();
    }
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, &tank);
    headless(
        |app| {
            app.add_plugins(ShopPlugin);
        },
        blob.take_pending(),
    )
}

fn tank(app: &App) -> &Pals {
    app.world().resource::<Pals>()
}

fn drawn(app: &App, sprite: &'static Sprite) -> usize {
    app.world()
        .resource::<Frame>()
        .shapes
        .iter()
        .filter(|s| matches!(s, Shape::Sprite { sprite: d, .. } if *d == SpriteRef(sprite)))
        .count()
}

fn point_at(app: &mut App, choice: usize) {
    set_tilt(app, hat_turn(choice), 0.0);
    update(app);
}

fn hat(named: &str) -> usize {
    HATS.iter().position(|hat| hat.name == named).unwrap()
}

/// The ring choice for a hat on the shelf (0 is NO HAT).
fn choice(hat: usize) -> usize {
    hat + 1
}

#[test]
fn the_hats_ring_the_glass_and_the_mirror_tries_the_pointed_one_on() {
    let mut app = shop(0, &["GILLY"]);
    for hat in &HATS {
        assert!(drawn(&app, hat.sprite) >= 1, "{} on the ring", hat.name);
    }
    point_at(&mut app, choice(hat("CROWN")));
    assert!(shows(&app, "CROWN") && shows(&app, "500 GOLD"));
    assert_eq!(drawn(&app, HATS[hat("CROWN")].sprite), 2, "ring and mirror");
}

#[test]
fn pwr_buys_the_pointed_hat_and_puts_it_on_the_fish() {
    let mut app = shop(100, &["GILLY"]);
    let beanie = hat("BEANIE");
    point_at(&mut app, choice(beanie));
    click_action(&mut app);
    assert_eq!(tank(&app).gold, 100 - HATS[beanie].price);
    assert!(tank(&app).owns_hat(beanie));
    assert_eq!(tank(&app).pets[0].hat, Some(beanie as u8));
    assert!(shows(&app, "WEARING"));
    let pending = app.world_mut().resource_mut::<SaveBlob>().take_pending();
    let saved = pals::save::load(&SaveBlob::from_storage(pending));
    assert_eq!(saved.pets[0].hat, Some(beanie as u8));
}

#[test]
fn a_hat_too_dear_says_how_much_more_it_needs() {
    let mut app = shop(10, &["GILLY"]);
    point_at(&mut app, choice(hat("BALL CAP")));
    assert!(shows(&app, "NEED 20 MORE"));
    click_action(&mut app);
    assert_eq!(tank(&app).gold, 10);
    assert_eq!(tank(&app).pets[0].hat, None);
}

#[test]
fn owned_hats_go_on_for_free_and_no_hat_takes_it_off() {
    let mut app = shop(100, &["GILLY"]);
    point_at(&mut app, choice(hat("STRAW HAT")));
    assert!(shows(&app, "OWNED"));
    click_action(&mut app);
    assert_eq!(tank(&app).pets[0].hat, Some(0));
    assert_eq!(tank(&app).gold, 100);
    point_at(&mut app, 0);
    assert!(shows(&app, "NO HAT"));
    click_action(&mut app);
    assert_eq!(tank(&app).pets[0].hat, None);
}

#[test]
fn tapping_the_fish_dresses_the_next_one() {
    let mut app = shop(100, &["GILLY", "BUBBA"]);
    assert!(shows(&app, "GILLY"));
    tap(&mut app, ui::MIRROR.center());
    assert!(shows(&app, "BUBBA"));
    point_at(&mut app, choice(0));
    click_action(&mut app);
    assert_eq!(
        (tank(&app).pets[0].hat, tank(&app).pets[1].hat),
        (None, Some(0))
    );
}

#[test]
fn with_no_fish_hats_can_still_be_bought() {
    let mut app = shop(100, &[]);
    assert!(shows(&app, "NO FISH TO DRESS"));
    let beanie = hat("BEANIE");
    point_at(&mut app, choice(beanie));
    click_action(&mut app);
    assert!(tank(&app).owns_hat(beanie));
}

#[test]
fn out_leaves_the_shop() {
    let mut app = shop(0, &["GILLY"]);
    point_at(&mut app, OUT);
    click_action(&mut app);
    assert_eq!(app.should_exit(), Some(AppExit::Success));
}

#[test]
fn the_first_hat_is_one_beginner_win_away_and_the_crown_takes_a_master_title() {
    let beginner_win = TIERS[0].prizes[0];
    assert!(HATS[1].price <= beginner_win);
    let crown = HATS[HATS.len() - 1].price;
    assert!(crown > TIERS[2].prizes[0], "more than an expert win");
    assert!(crown <= TIERS[3].prizes[0], "a master win buys it");
}
