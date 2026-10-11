//! The browser's keys, finger and phone tilt, gathered between frames into
//! the board's [`Input`]. Events come faster than frames, so a press or a tap
//! that ends before the next frame still counts for one.

use bevy::math::Vec2;

use crate::{
    board::{Input, key},
    pad, phone,
};

/// The board's bit for a `KeyboardEvent.code`.
pub fn key_bit(code: &str) -> Option<u32> {
    match code {
        "Space" => Some(key::ACTION),
        "KeyZ" => Some(key::ZERO),
        "ArrowLeft" => Some(key::LEFT),
        "ArrowRight" => Some(key::RIGHT),
        "ArrowUp" => Some(key::UP),
        "ArrowDown" => Some(key::DOWN),
        "KeyC" => Some(key::STRAIGHTEN),
        "KeyW" => Some(key::WALK),
        _ => None,
    }
}

/// A `deviceorientation` reading and the page's turn, in degrees.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tilt {
    pub alpha: f32,
    pub beta: f32,
    pub gamma: f32,
    pub screen_angle: f32,
}

#[derive(Clone, Copy)]
struct Finger {
    id: i32,
    at: Vec2,
    seen: bool,
    lifted: bool,
}

#[derive(Default)]
pub struct Controls {
    held: u32,
    pressed: u32,
    /// A turn key or the stick has turned the board, so the phone's tilt no
    /// longer does.
    by_hand: bool,
    finger: Option<Finger>,
    /// The finger on the touch pad and the stick it pushes.
    pad: Option<(i32, Vec2)>,
    tilt: Option<Tilt>,
}

impl Controls {
    pub fn key_down(&mut self, bit: u32) {
        self.held |= bit;
        self.pressed |= bit;
        self.by_hand |= bit & (key::LEFT | key::RIGHT | key::UP | key::DOWN) != 0;
    }

    pub fn key_up(&mut self, bit: u32) {
        self.held &= !bit;
    }

    /// False while another finger is down: the board's touch has one.
    pub fn finger_down(&mut self, id: i32, at: Vec2) -> bool {
        if self.finger.is_some_and(|finger| !finger.lifted) {
            return false;
        }
        self.finger = Some(Finger {
            id,
            at,
            seen: false,
            lifted: false,
        });
        true
    }

    pub fn finger_moved(&mut self, id: i32, at: Vec2) {
        if let Some(finger) = self.finger.as_mut().filter(|f| f.id == id && !f.lifted) {
            finger.at = at;
        }
    }

    pub fn finger_up(&mut self, id: i32) {
        let Some(finger) = self.finger.as_mut().filter(|f| f.id == id) else {
            return;
        };
        if finger.seen {
            self.finger = None;
            return;
        }
        finger.lifted = true;
    }

    /// A finger on the touch pad (pad pixels) is the stick; one at a time.
    pub fn pad_down(&mut self, id: i32, at: Vec2) -> bool {
        if self.pad.is_some() {
            return false;
        }
        self.pad = Some((id, pad::stick(at)));
        true
    }

    pub fn pad_moved(&mut self, id: i32, at: Vec2) {
        if let Some((_, stick)) = self.pad.as_mut().filter(|(held, _)| *held == id) {
            *stick = pad::stick(at);
        }
    }

    pub fn pad_up(&mut self, id: i32) {
        if self.pad.is_some_and(|(held, _)| held == id) {
            self.pad = None;
        }
    }

    pub fn tilt(&mut self, tilt: Tilt) {
        self.tilt = Some(tilt);
    }

    /// Everything lets go, as when the page loses focus.
    pub fn let_go(&mut self) {
        self.held = 0;
        self.finger = None;
        self.pad = None;
    }

    /// This frame's input.
    pub fn frame(&mut self) -> Input {
        let stick = self.pad.map_or(Vec2::ZERO, |(_, stick)| stick);
        self.by_hand |= stick != Vec2::ZERO;
        let input = Input {
            finger: self.finger.map(|finger| finger.at),
            keys: self.held | self.pressed,
            stick,
            phone: self
                .tilt
                .filter(|_| !self.by_hand)
                .map(|t| phone::pose(t.alpha, t.beta, t.gamma, t.screen_angle)),
        };
        self.pressed = 0;
        match self.finger.as_mut() {
            Some(finger) if finger.lifted => self.finger = None,
            Some(finger) => finger.seen = true,
            None => {}
        }
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUSE: Vec2 = Vec2::new(233.0, 110.0);

    fn upright() -> Tilt {
        Tilt {
            beta: 90.0,
            ..Tilt::default()
        }
    }

    #[test]
    fn the_board_keys_are_space_z_the_arrows_c_and_w() {
        assert_eq!(key_bit("Space"), Some(key::ACTION));
        assert_eq!(key_bit("KeyZ"), Some(key::ZERO));
        assert_eq!(key_bit("ArrowLeft"), Some(key::LEFT));
        assert_eq!(key_bit("KeyW"), Some(key::WALK));
        assert_eq!(key_bit("KeyQ"), None);
    }

    #[test]
    fn a_tap_that_ends_between_frames_touches_for_one_frame() {
        let mut controls = Controls::default();
        assert!(controls.finger_down(1, HOUSE));
        controls.finger_up(1);
        assert_eq!(controls.frame().finger, Some(HOUSE));
        assert_eq!(controls.frame().finger, None);
    }

    #[test]
    fn a_held_finger_follows_its_moves_and_lifts_on_release() {
        let mut controls = Controls::default();
        controls.finger_down(1, HOUSE);
        assert_eq!(controls.frame().finger, Some(HOUSE));
        assert!(!controls.finger_down(2, Vec2::ZERO), "a second finger");
        controls.finger_moved(2, Vec2::ZERO);
        controls.finger_moved(1, Vec2::ONE);
        assert_eq!(controls.frame().finger, Some(Vec2::ONE));
        controls.finger_up(1);
        assert_eq!(controls.frame().finger, None);
    }

    #[test]
    fn a_key_tapped_between_frames_is_held_for_one_frame() {
        let mut controls = Controls::default();
        controls.key_down(key::ACTION);
        controls.key_up(key::ACTION);
        assert_eq!(controls.frame().keys, key::ACTION);
        assert_eq!(controls.frame().keys, 0);
        controls.key_down(key::ZERO);
        assert_eq!(controls.frame().keys, key::ZERO);
        assert_eq!(controls.frame().keys, key::ZERO, "still held");
    }

    #[test]
    fn the_phone_steers_until_a_turn_key_is_used() {
        let mut controls = Controls::default();
        controls.tilt(upright());
        assert!(controls.frame().phone.is_some());
        controls.key_down(key::ACTION);
        assert!(controls.frame().phone.is_some(), "PRESS isn't a turn");
        controls.key_down(key::LEFT);
        controls.key_up(key::LEFT);
        controls.tilt(upright());
        assert!(controls.frame().phone.is_none());
    }

    #[test]
    fn the_stick_follows_the_pad_finger_and_springs_back_when_it_lifts() {
        let mut controls = Controls::default();
        let middle = Vec2::splat(233.0);
        assert!(controls.pad_down(4, middle));
        assert_eq!(controls.frame().stick, Vec2::ZERO);
        controls.pad_moved(4, middle + Vec2::new(pad::TRAVEL, 0.0));
        assert!(controls.frame().stick.distance(Vec2::X) < 1e-4);
        controls.pad_up(4);
        assert_eq!(controls.frame().stick, Vec2::ZERO);
    }

    #[test]
    fn a_finger_on_the_pad_and_one_on_the_glass_both_count() {
        let mut controls = Controls::default();
        controls.finger_down(1, HOUSE);
        controls.pad_down(2, Vec2::new(233.0 + pad::TRAVEL, 233.0));
        let input = controls.frame();
        assert_eq!(input.finger, Some(HOUSE));
        assert!(input.stick.x > 0.99);
    }

    #[test]
    fn pushing_the_stick_takes_steering_from_the_phone() {
        let mut controls = Controls::default();
        controls.tilt(upright());
        controls.pad_down(2, Vec2::splat(233.0));
        assert!(controls.frame().phone.is_some(), "a resting thumb");
        controls.pad_moved(2, Vec2::new(233.0, 233.0 - pad::TRAVEL));
        controls.tilt(upright());
        assert!(controls.frame().phone.is_none());
    }

    #[test]
    fn letting_go_releases_keys_and_the_finger() {
        let mut controls = Controls::default();
        controls.key_down(key::ZERO);
        controls.finger_down(1, HOUSE);
        controls.pad_down(2, Vec2::ZERO);
        controls.frame();
        controls.let_go();
        let input = controls.frame();
        assert_eq!(
            (input.keys, input.finger, input.stick),
            (0, None, Vec2::ZERO)
        );
    }
}
