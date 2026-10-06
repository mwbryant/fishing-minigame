//! Skate's sounds, synthesized by assets/sfx.py.

use orbit_core::Clip;

pub static POP: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/pop.pcm"),
};
pub static LAND: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/land.pcm"),
};
/// Repeats while grinding a bench.
pub static GRIND: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/grind.pcm"),
};
pub static BAIL: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/bail.pcm"),
};
