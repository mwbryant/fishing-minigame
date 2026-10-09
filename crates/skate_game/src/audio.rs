//! Skate's sounds, synthesized by assets/sfx.py.

use orbit_core::Clip;

/// Popping an ollie, dropping in, or leaving a halfpipe wall.
pub static POP: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/pop.pcm"),
};
/// Landing clean, onto a bench, or a halfpipe trick.
pub static LAND: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/land.pcm"),
};
/// Repeats while grinding a bench.
pub static GRIND: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/grind.pcm"),
};
/// Bailing, or slamming a halfpipe trick.
pub static BAIL: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/bail.pcm"),
};
