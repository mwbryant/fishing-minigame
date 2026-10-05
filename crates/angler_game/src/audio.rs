use orbit_core::Clip;

/// "Guten Morgen, Sonnenschein" from the spinning-fish meme (see
/// assets/README.md), played by the GUTEN easter egg.
pub static GUTEN_MORGEN: Clip = Clip {
    pcm: include_bytes!("../assets/guten_morgen.pcm"),
};

/// One full turn of the spinning fish, as in logic-fish.
pub const GUTEN_SPIN_SECS: f32 = 2.3;
