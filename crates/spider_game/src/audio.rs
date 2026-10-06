//! Spider Roulette's sounds, synthesized by assets/sfx.py.

use orbit_core::Clip;

pub static CRACK: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/crack.pcm"),
};
/// Repeats faster as the eggs run out.
pub static HEARTBEAT: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/heartbeat.pcm"),
};
pub static SCREECH: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/screech.pcm"),
};
pub static WIN: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/win.pcm"),
};
