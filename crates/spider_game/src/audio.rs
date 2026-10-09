//! Spider Roulette's sounds, synthesized by assets/sfx.py.

use orbit_core::Clip;

/// A safe egg cracking.
pub static CRACK: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/crack.pcm"),
};
/// Repeats faster as the eggs run out.
pub static HEARTBEAT: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/heartbeat.pcm"),
};
/// The spider's egg cracking.
pub static SCREECH: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/screech.pcm"),
};
/// Trapping the spider or cashing out.
pub static WIN: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/win.pcm"),
};
/// The pointer passing an egg.
pub static TICK: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/tick.pcm"),
};
