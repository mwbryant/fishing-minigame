use orbit_core::Clip;

/// "Guten Morgen, Sonnenschein" from the spinning-fish meme (see
/// assets/README.md), played by the GUTEN easter egg.
pub static GUTEN_MORGEN: Clip = Clip {
    pcm: include_bytes!("../assets/guten_morgen.pcm"),
};

/// One full turn of the spinning fish, as in logic-fish.
pub const GUTEN_SPIN_SECS: f32 = 2.3;

// Sound effects, synthesized by assets/sfx.py. The mixer has one voice, so
// each replaces whatever was playing; all but ESCAPE are under 0.75 s.

/// Releasing a cast.
pub static CAST: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/cast.pcm"),
};
/// Releasing a cast inside the PERFECT window: the whoosh plus a chime.
pub static PERFECT_CAST: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/perfect.pcm"),
};
/// The bobber landing on the water.
pub static PLOP: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/plop.pcm"),
};
/// The bobber landing on the dock, or a tap that can't do anything.
pub static THUD: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/thud.pcm"),
};
pub static BITE: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/bite.pcm"),
};
/// One click of the reel, each time the catch meter rises another step.
pub static REEL: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/reel.pcm"),
};
pub static CATCH: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/catch.pcm"),
};
pub static ESCAPE: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/escape.pcm"),
};
/// Buying a hat.
pub static COIN: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/coin.pcm"),
};
/// Tapping an on-screen button.
pub static CLICK: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/click.pcm"),
};
