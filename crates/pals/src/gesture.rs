//! Strokes on the glass as trick triggers: a tap, a hold, a swipe four ways,
//! or a circle either way round.

use bevy::math::{Vec2, ops};

/// A finger that moves less than this is still.
const STILL_PX: f32 = 15.0;
/// A swipe goes at least this far from where it started.
const SWIPE_PX: f32 = 60.0;
/// A circle turns at least this far (radians) and is at least this long.
const CIRCLE_TURN: f32 = 1.5 * core::f32::consts::PI;
const CIRCLE_PX: f32 = 150.0;
/// A still finger down this long is a hold.
pub const HOLD_SECS: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Gesture {
    Tap = 1,
    Hold,
    SwipeUp,
    SwipeDown,
    SwipeLeft,
    SwipeRight,
    CircleClockwise,
    CircleCounterclockwise,
}

impl Gesture {
    pub const ALL: [Gesture; 8] = [
        Gesture::Tap,
        Gesture::Hold,
        Gesture::SwipeUp,
        Gesture::SwipeDown,
        Gesture::SwipeLeft,
        Gesture::SwipeRight,
        Gesture::CircleClockwise,
        Gesture::CircleCounterclockwise,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Gesture::Tap => "TAP",
            Gesture::Hold => "HOLD",
            Gesture::SwipeUp => "SWIPE UP",
            Gesture::SwipeDown => "SWIPE DOWN",
            Gesture::SwipeLeft => "SWIPE LEFT",
            Gesture::SwipeRight => "SWIPE RIGHT",
            Gesture::CircleClockwise => "CIRCLE",
            Gesture::CircleCounterclockwise => "CIRCLE BACK",
        }
    }

    pub(crate) fn from_code(code: u8) -> Option<Gesture> {
        Gesture::ALL.into_iter().find(|g| *g as u8 == code)
    }
}

/// The finger's current stroke, read into a [`Gesture`] as it goes.
#[derive(Clone, Copy, Debug, Default)]
pub struct Strokes {
    stroke: Option<Stroke>,
}

#[derive(Clone, Copy, Debug)]
struct Stroke {
    start: Vec2,
    last: Vec2,
    heading: Option<Vec2>,
    length: f32,
    /// Radians turned, positive clockwise on screen (y grows downward).
    turned: f32,
    secs: f32,
    /// A hold is told as it happens, so its release says nothing more.
    told: bool,
}

impl Strokes {
    /// The finger this update (`None` when lifted); a gesture when one ends
    /// or, for a hold, once it has been held long enough.
    pub fn update(&mut self, finger: Option<Vec2>, dt: f32) -> Option<Gesture> {
        match (finger, self.stroke.as_mut()) {
            (Some(at), None) => {
                self.stroke = Some(Stroke {
                    start: at,
                    last: at,
                    heading: None,
                    length: 0.0,
                    turned: 0.0,
                    secs: 0.0,
                    told: false,
                });
                None
            }
            (Some(at), Some(stroke)) => stroke.extend(at, dt),
            (None, Some(_)) => self.stroke.take().and_then(|stroke| stroke.read()),
            (None, None) => None,
        }
    }

    pub fn touching(&self) -> bool {
        self.stroke.is_some()
    }
}

impl Stroke {
    fn extend(&mut self, at: Vec2, dt: f32) -> Option<Gesture> {
        self.secs += dt;
        let step = at - self.last;
        if step.length() >= 2.0 {
            let heading = step.normalize();
            if let Some(previous) = self.heading {
                self.turned += ops::atan2(previous.perp_dot(heading), previous.dot(heading));
            }
            self.heading = Some(heading);
            self.length += step.length();
            self.last = at;
        }
        let held = !self.told && self.secs >= HOLD_SECS && self.length < STILL_PX;
        if held {
            self.told = true;
            return Some(Gesture::Hold);
        }
        None
    }

    fn read(self) -> Option<Gesture> {
        if self.told {
            return None;
        }
        if self.turned.abs() >= CIRCLE_TURN && self.length >= CIRCLE_PX {
            return Some(if self.turned > 0.0 {
                Gesture::CircleClockwise
            } else {
                Gesture::CircleCounterclockwise
            });
        }
        let moved = self.last - self.start;
        if moved.length() >= SWIPE_PX {
            return Some(
                match (moved.x.abs() > moved.y.abs(), moved.x > 0.0, moved.y > 0.0) {
                    (true, true, _) => Gesture::SwipeRight,
                    (true, false, _) => Gesture::SwipeLeft,
                    (false, _, true) => Gesture::SwipeDown,
                    (false, _, false) => Gesture::SwipeUp,
                },
            );
        }
        (self.length < STILL_PX).then_some(Gesture::Tap)
    }
}
