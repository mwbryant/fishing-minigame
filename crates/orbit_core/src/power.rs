use bevy::prelude::*;

/// The battery as the platform last read it from its power chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    /// Charge 0..=100 from the fuel gauge, or `None` with no cell connected.
    pub percent: Option<u8>,
    /// USB (or another external supply) is powering the board.
    pub external_power: bool,
    pub charging: bool,
}

/// Power state shared by the game and the platform. The platform reports the
/// battery; the game says when it is idle (the platform may dim the panel and
/// slow its loop) and when to sleep (the platform powers down until a button
/// wakes it, which restarts the game).
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct Power {
    /// `None` until the platform reports a reading, and on platforms without a gauge.
    pub battery: Option<Battery>,
    pub idle: bool,
    sleep_requested: bool,
}

impl Power {
    pub fn request_sleep(&mut self) {
        self.sleep_requested = true;
    }

    pub fn sleep_requested(&self) -> bool {
        self.sleep_requested
    }
}
