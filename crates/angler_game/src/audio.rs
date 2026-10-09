//! Angler's sounds: the GUTEN song, and effects synthesized by assets/sfx.py.
//! The mixer has one voice, so each clip replaces whatever was playing. Every
//! effect but ESCAPE is under 0.75 s.

use orbit_core::Clip;

/// "Guten Morgen, Sonnenschein" from the spinning-fish meme, played by the GUTEN
/// easter egg. Its PCM is gitignored (see assets/README.md), so a clean clone
/// builds it silent.
pub static GUTEN_MORGEN: Clip = Clip { pcm: GUTEN_PCM };

#[cfg(guten_song)]
const GUTEN_PCM: &[u8] = include_bytes!("../assets/guten_morgen.pcm");
#[cfg(not(guten_song))]
const GUTEN_PCM: &[u8] = &[];

/// How long GUTEN lasts: the song's length, so it ends on cue even when silent.
pub const GUTEN_SECS: f32 = 15.15;

/// Seconds per turn of the spinning fish, as in logic-fish.
pub const GUTEN_SPIN_SECS: f32 = 2.3;

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
/// A fish taking the bait.
pub static BITE: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/bite.pcm"),
};
/// One click of the reel, each time the catch meter rises another step.
pub static REEL: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/reel.pcm"),
};
/// Landing a fish.
pub static CATCH: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/catch.pcm"),
};
/// A fish getting away (1.1 s).
pub static ESCAPE: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/escape.pcm"),
};
/// Buying a hat in the town's shop.
pub static COIN: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/coin.pcm"),
};
/// Tapping an on-screen button.
pub static CLICK: Clip = Clip {
    pcm: include_bytes!("../assets/sfx/click.pcm"),
};
