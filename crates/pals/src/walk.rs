//! Walks: metres walked from satellite fixes, counted only for real
//! walking, and a mock walk for indoors (or a board without the -G module's
//! receiver) that strolls at walking pace through the same counting.

use bevy::math::ops;
use orbit_core::Fix;

/// Fixes less sure than this are ignored.
const MAX_HDOP: f32 = 5.0;
const MIN_SATELLITES: u8 = 4;
/// Distance only counts once the fix has moved this far (or four times its
/// HDOP in metres, if that's more) from the last counted spot, so a receiver
/// wandering in place doesn't count as walking.
const MIN_STEP_M: f32 = 8.0;
const STEP_PER_HDOP_M: f32 = 4.0;
/// Faster than a jog is a bike or a car: it moves the spot but doesn't count.
pub const MAX_WALK_MPS: f32 = 4.5;
/// The mock walker's pace and how often it reports.
pub const MOCK_PACE_MPS: f32 = 1.4;
const MOCK_FIX_SECS: f32 = 1.0;

/// The walk so far.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Trail {
    anchor: Option<Fix>,
    pub meters: f32,
}

impl Trail {
    /// Counts a new fix; returns the metres it added.
    pub fn step(&mut self, fix: Fix) -> f32 {
        if fix.hdop > MAX_HDOP || fix.satellites < MIN_SATELLITES {
            return 0.0;
        }
        let Some(anchor) = self.anchor else {
            self.anchor = Some(fix);
            return 0.0;
        };
        let moved = anchor.meters_to(&fix);
        if moved < MIN_STEP_M.max(fix.hdop * STEP_PER_HDOP_M) {
            return 0.0;
        }
        self.anchor = Some(fix);
        if fix.speed_mps > MAX_WALK_MPS {
            return 0.0;
        }
        self.meters += moved;
        moved
    }
}

/// A pretend walker meandering at [`MOCK_PACE_MPS`], reporting a fix each
/// second like the receiver does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MockWalk {
    north_m: f32,
    east_m: f32,
    heading: f32,
    secs: f32,
    until_fix: f32,
}

impl Default for MockWalk {
    fn default() -> Self {
        Self {
            north_m: 0.0,
            east_m: 0.0,
            heading: 0.6,
            secs: 0.0,
            until_fix: 0.0,
        }
    }
}

impl MockWalk {
    pub fn step(&mut self, dt: f32) -> Option<Fix> {
        self.secs += dt;
        self.heading += 0.15 * ops::sin(self.secs * 0.1) * dt;
        let (sin, cos) = ops::sin_cos(self.heading);
        self.north_m += cos * MOCK_PACE_MPS * dt;
        self.east_m += sin * MOCK_PACE_MPS * dt;
        self.until_fix -= dt;
        if self.until_fix > 0.0 {
            return None;
        }
        self.until_fix += MOCK_FIX_SECS;
        let per_deg = 111_195.0;
        Some(Fix {
            lat_deg: 51.5 + f64::from(self.north_m) / per_deg,
            lon_deg: -0.12 + f64::from(self.east_m) / (per_deg * 0.6225),
            hdop: 0.9,
            satellites: 12,
            speed_mps: MOCK_PACE_MPS,
        })
    }
}
