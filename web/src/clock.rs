//! Bevy's clock. wasm32 has none of its own, so the page hands over its
//! animation-frame time before each frame, like the firmware's timer.

use std::{cell::Cell, time::Duration};

use bevy::platform::time::Instant;

thread_local! {
    static NOW: Cell<Duration> = const { Cell::new(Duration::ZERO) };
}

pub fn start() {
    // SAFETY: `read` is a plain fn that lives for the whole program and only
    // moves forward (`set` never goes back).
    unsafe { Instant::set_elapsed(read) };
}

pub fn set(now: Duration) {
    NOW.set(NOW.get().max(now));
}

pub fn read() -> Duration {
    NOW.get()
}
