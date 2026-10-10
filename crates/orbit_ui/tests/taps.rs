use bevy::math::Vec2;
use orbit_ui::{Button, TAP_SLOP, nearest, tapped};

const LEFT: Button = Button::new(75, 356, 104, 36, "LEFT");
const RIGHT: Button = Button::new(181, 356, 104, 36, "RIGHT");

#[test]
fn a_fingertip_landing_just_off_a_button_still_presses_it() {
    assert!(LEFT.hit(Vec2::new(120.0, 356.0 - TAP_SLOP + 1.0)));
    assert!(LEFT.hit(Vec2::new(75.0 - TAP_SLOP + 1.0, 370.0)));
    assert!(!LEFT.hit(Vec2::new(120.0, 356.0 - TAP_SLOP - 2.0)));
}

#[test]
fn a_fingertip_two_and_a_half_millimetres_off_a_button_still_presses_it() {
    assert!(LEFT.hit(Vec2::new(120.0, 356.0 - 26.0)));
    assert!(LEFT.hit(Vec2::new(75.0 - 26.0, 370.0)));
}

#[test]
fn between_buttons_close_together_the_nearer_one_is_pressed() {
    let buttons = [LEFT, RIGHT];
    assert_eq!(tapped(Vec2::new(182.0, 370.0), &buttons), Some(&RIGHT));
    assert_eq!(tapped(Vec2::new(178.0, 370.0), &buttons), Some(&LEFT));
    assert_eq!(tapped(Vec2::new(190.0, 350.0), &buttons), Some(&RIGHT));
    assert_eq!(tapped(Vec2::new(233.0, 300.0), &buttons), None);
}

#[test]
fn a_tap_picks_the_nearest_spot_within_reach() {
    let spots = [Vec2::new(100.0, 100.0), Vec2::new(160.0, 100.0)];
    assert_eq!(nearest(Vec2::new(125.0, 110.0), spots, 40.0), Some(0));
    assert_eq!(nearest(Vec2::new(135.0, 90.0), spots, 40.0), Some(1));
    assert_eq!(nearest(Vec2::new(130.0, 160.0), spots, 40.0), None);
}
