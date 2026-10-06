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

const LEG: (u8, orbit_gfx::Rgb565) = (b'K', rgb(190, 160, 230));
const BODY: (u8, orbit_gfx::Rgb565) = (b'P', rgb(140, 80, 190));
const EYE: (u8, orbit_gfx::Rgb565) = (b'E', rgb(255, 250, 120));
const GRIN: (u8, orbit_gfx::Rgb565) = (b'W', rgb(250, 250, 250));

/// The spider dancing in the middle of the web: two poses, legs up one way
/// then the other. It's also the one that pounces.
pub static DANCER: [Sprite; 2] = [
    Sprite {
        rows: &[
            "K.............K",
            ".K....PPP....K.",
            "..K..PPPPP..K..",
            "...KPPEPEPPK...",
            "KK..PPPPPPP..KK",
            "..KPPPWPWPPPK..",
            "...PPPPPPPPP...",
            "..K..PPPPP..K..",
            ".K...K...K...K.",
            "K...K.....K...K",
        ],
        palette: &[LEG, BODY, EYE, GRIN],
    },
    Sprite {
        rows: &[
            "...............",
            "KK....PPP....KK",
            "..K..PPPPP..K..",
            "...KPPEPEPPK...",
            "..KPPPPPPPPPK..",
            "KK..PPWPWPP..KK",
            "...PPPPPPPPP...",
            "..K..PPPPP..K..",
            "..K..K...K..K..",
            ".K..K.....K..K.",
        ],
        palette: &[LEG, BODY, EYE, GRIN],
    },
];
