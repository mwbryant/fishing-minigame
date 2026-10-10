//! Walks counted in steps: the board's IMU counts them, and [`MockWalk`]
//! fakes them for the simulator (or to watch a walk without walking). Stamina
//! is measured in metres, at [`STRIDE_M`] a step.

/// A step's length, for the metres stamina is measured in.
pub const STRIDE_M: f32 = 0.75;
/// The mock walker's cadence, steps a second: about 1.4 m/s.
pub const MOCK_STEPS_PER_SEC: f32 = 1.9;
/// How many times walking pace a mock walk can go: a fast-forward.
pub const MOCK_PACES: [f32; 5] = [1.0, 2.0, 5.0, 10.0, 25.0];

/// A pretend walker stepping at [`MOCK_STEPS_PER_SEC`] times its pace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MockWalk {
    times: f32,
    /// Part of a step taken but not yet counted.
    owed: f32,
}

impl Default for MockWalk {
    fn default() -> Self {
        Self::at_pace(1.0)
    }
}

impl MockWalk {
    /// A mock walk going `times` walking pace.
    pub fn at_pace(times: f32) -> Self {
        Self { times, owed: 0.0 }
    }

    pub fn set_pace(&mut self, times: f32) {
        self.times = times;
    }

    pub fn pace(&self) -> f32 {
        self.times
    }

    /// Walks on for `dt` seconds; returns the whole steps taken.
    pub fn step(&mut self, dt: f32) -> u32 {
        self.owed += MOCK_STEPS_PER_SEC * self.times * dt;
        let whole = self.owed as u32;
        self.owed -= whole as f32;
        whole
    }
}
