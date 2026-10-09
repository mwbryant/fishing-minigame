//! Skate's pixel art as text (`.` is transparent). The skater faces right.

use orbit_gfx::Sprite;
use orbit_ui::rgb;

pub static SKATER: Sprite = Sprite {
    rows: &[
        "....HHH.....",
        "...HHHHH....",
        "...SSSKS....",
        "....SSS.....",
        "...RRRRR....",
        "..RRRRRRR...",
        "..S.RRR.S...",
        "....RRR.....",
        "....PPP.....",
        "...PP.PP....",
        "...PP..PP...",
        "..PP....PP..",
        "..BB....BB..",
    ],
    palette: &[
        (b'H', rgb(60, 170, 230)),
        (b'S', rgb(236, 190, 150)),
        (b'K', rgb(30, 30, 30)),
        (b'R', rgb(230, 70, 60)),
        (b'P', rgb(60, 70, 110)),
        (b'B', rgb(240, 240, 240)),
    ],
};

/// Down on the street after a bail.
pub static FALLEN: Sprite = Sprite {
    rows: &[
        "..........HHH",
        "BB.PPPRRRRHHHH",
        "BBPPPPRRRRSSKS",
        "...PP.S.RR.SS.",
    ],
    palette: &[
        (b'H', rgb(60, 170, 230)),
        (b'S', rgb(236, 190, 150)),
        (b'K', rgb(30, 30, 30)),
        (b'R', rgb(230, 70, 60)),
        (b'P', rgb(60, 70, 110)),
        (b'B', rgb(240, 240, 240)),
    ],
};

pub static CONE: Sprite = Sprite {
    rows: &[
        "..OO..", "..OO..", ".OWWO.", ".OOOO.", "OWWWWO", "OOOOOO", "KKKKKK",
    ],
    palette: &[
        (b'O', rgb(250, 120, 30)),
        (b'W', rgb(245, 245, 245)),
        (b'K', rgb(60, 60, 60)),
    ],
};

pub static BENCH: Sprite = Sprite {
    rows: &[
        "GGGGGGGGGGGGGGGGGGGG",
        "DDDDDDDDDDDDDDDDDDDD",
        ".L................L.",
        ".L................L.",
        ".L................L.",
        ".L................L.",
    ],
    palette: &[
        (b'G', rgb(170, 120, 70)),
        (b'D', rgb(110, 75, 45)),
        (b'L', rgb(90, 90, 100)),
    ],
};
