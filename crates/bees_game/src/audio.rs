//! Bees???'s sounds, synthesized by assets/sfx.py.

use orbit_core::Clip;

/// Repeats while honey pours out.
pub static DRIP: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/drip.pcm"),
};
/// A bee arriving with a refill.
pub static BUZZ: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/buzz.pcm"),
};
pub static EMPTY: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/empty.pcm"),
};
