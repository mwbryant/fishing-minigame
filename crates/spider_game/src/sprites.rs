//! Text pixel art (`.` is transparent).

use orbit_gfx::Sprite;
use orbit_ui::rgb;

pub static EGG: Sprite = Sprite {
    rows: &[
        "..WWW..", ".WWWWW.", ".WWWWW.", "WWWWWWW", "WWWWSWW", "WWSWWWW", "WWWWWWW", ".WWWWW.",
        "..WWW..",
    ],
    palette: &[(b'W', rgb(240, 234, 218)), (b'S', rgb(196, 186, 160))],
};

/// What's left of a safe egg.
pub static SHELL: Sprite = Sprite {
    rows: &[
        ".......", ".......", ".......", ".......", "W.W.W.W", "WWWWWWW", "WWWWWWW", ".WWWWW.",
        "..WWW..",
    ],
    palette: &[(b'W', rgb(110, 106, 96))],
};

/// The one that leaps out.
pub static SPIDER: Sprite = Sprite {
    rows: &[
        "K.....K.K.....K",
        ".K....K.K....K.",
        "..K..KKKKK..K..",
        "...KKKKKKKKK...",
        "KK.KKEKKKEKK.KK",
        "..KKKKKKKKKKK..",
        "...KKKWKWKKK...",
        "KK..KKKKKKK..KK",
        "..K.KKKKKKK.K..",
        ".K...KKKKK...K.",
        "K.....K.K.....K",
        "......K.K......",
    ],
    palette: &[
        (b'K', rgb(26, 20, 30)),
        (b'E', rgb(230, 255, 70)),
        (b'W', rgb(250, 250, 250)),
    ],
};
