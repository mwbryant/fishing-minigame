//! The town's pixel art, as text: `.` is see-through.

use orbit_gfx::Sprite;
use orbit_ui::rgb;

/// Your house: a red roof, two windows and a door.
pub static HOUSE: Sprite = Sprite {
    rows: &[
        "......RRRRRR......",
        ".....RRRRRRRR.....",
        "....RRRRRRRRRR....",
        "...RRRRRRRRRRRR...",
        "..RRRRRRRRRRRRRR..",
        ".RRRRRRRRRRRRRRRR.",
        "DDDDDDDDDDDDDDDDDD",
        ".WWWWWWWWWWWWWWWW.",
        ".WBBBWWWWWWWWBBBW.",
        ".WBBBWWWDDWWWBBBW.",
        ".WWWWWWWDDWWWWWWW.",
        ".WWWWWWWDDWWWWWWW.",
        ".WWWWWWWDDWWWWWWW.",
    ],
    palette: &[
        (b'R', rgb(190, 60, 50)),
        (b'D', rgb(120, 40, 35)),
        (b'W', rgb(235, 225, 200)),
        (b'B', rgb(120, 170, 220)),
    ],
};

/// The contest hall, its gold pennant flying.
pub static HALL: Sprite = Sprite {
    rows: &[
        "..........YYY.........",
        "..........YY..........",
        "..........K...........",
        "..PPPPPPPPPPPPPPPPPP..",
        ".PPPPPPPPPPPPPPPPPPPP.",
        "PPPPPPPPPPPPPPPPPPPPPP",
        ".WWWWWWWWWWWWWWWWWWWW.",
        ".WCWWCWWCWWWWWCWWCWWC.",
        ".WCWWCWWCWWDDWCWWCWWC.",
        ".WCWWCWWCWWDDWCWWCWWC.",
        ".WCWWCWWCWWDDWCWWCWWC.",
        "SSSSSSSSSSSSSSSSSSSSSS",
    ],
    palette: &[
        (b'Y', rgb(255, 210, 60)),
        (b'K', rgb(70, 60, 50)),
        (b'P', rgb(120, 70, 170)),
        (b'W', rgb(230, 225, 240)),
        (b'C', rgb(180, 175, 200)),
        (b'D', rgb(90, 60, 40)),
        (b'S', rgb(150, 150, 160)),
    ],
};

/// The hat shop, a top hat on its sign.
pub static SHOP: Sprite = Sprite {
    rows: &[
        "......KKKK......",
        "......KKKK......",
        "....KKKKKKKK....",
        "GGGGGGGGGGGGGGGG",
        "GTGTGTGTGTGTGTGT",
        ".WWWWWWWWWWWWWW.",
        ".WBBBBWWWDDWWWW.",
        ".WBBBBWWWDDWWWW.",
        ".WWWWWWWWDDWWWW.",
        ".WWWWWWWWDDWWWW.",
    ],
    palette: &[
        (b'K', rgb(40, 40, 50)),
        (b'G', rgb(60, 150, 90)),
        (b'T', rgb(240, 240, 230)),
        (b'W', rgb(240, 220, 180)),
        (b'B', rgb(130, 190, 230)),
        (b'D', rgb(120, 80, 50)),
    ],
};

/// The den: a dark door under a web.
pub static DEN: Sprite = Sprite {
    rows: &[
        "N..............N",
        ".N....NNNN....N.",
        "..N..N....N..N..",
        "MMMMMMMMMMMMMMMM",
        "MMMMMMMMMMMMMMMM",
        ".AAAAAAAAAAAAAA.",
        ".AARRAAAAAARRAA.",
        ".AAAAAADDAAAAAA.",
        ".AAAAAADDAAAAAA.",
        ".AAAAAADDAAAAAA.",
    ],
    palette: &[
        (b'N', rgb(200, 200, 210)),
        (b'M', rgb(60, 30, 70)),
        (b'A', rgb(95, 60, 110)),
        (b'R', rgb(230, 60, 60)),
        (b'D', rgb(20, 10, 25)),
    ],
};

/// An arcade cabinet with a skateboard on its screen.
pub static CABINET: Sprite = Sprite {
    rows: &[
        ".KKKKKK.", "KYYYYYYK", "KKKKKKKK", "KSSSSSSK", "KSOOOOSK", "KSSWSWSK", "KKKKKKKK",
        "KRRKKBBK", "KKKKKKKK", "KKKKKKKK", ".KKKKKK.",
    ],
    palette: &[
        (b'K', rgb(40, 40, 60)),
        (b'Y', rgb(255, 210, 60)),
        (b'S', rgb(30, 90, 160)),
        (b'O', rgb(240, 120, 50)),
        (b'W', rgb(240, 240, 240)),
        (b'R', rgb(230, 60, 60)),
        (b'B', rgb(80, 160, 240)),
    ],
};

/// A big old tree with a hollow, humming with bees.
pub static BEE_TREE: Sprite = Sprite {
    rows: &[
        "....LLLLLLLL....",
        "..LLLLLLLLLLLL..",
        ".LLLLLLLLLLLLLL.",
        "LLLLLLLHHLLLLLLL",
        "LLLLLLHHHHLLLLLL",
        "LLLLLLHHHHLLLLLL",
        ".LLLLLLHHLLLLLL.",
        "..LLLLLLLLLLLL..",
        "......TTTT......",
        "......TOOT......",
        "......TOOT......",
        "......TTTT......",
        ".....TTTTTT.....",
    ],
    palette: &[
        (b'L', rgb(50, 120, 60)),
        (b'H', rgb(240, 190, 50)),
        (b'T', rgb(110, 75, 45)),
        (b'O', rgb(30, 20, 15)),
    ],
};

/// A tree.
pub static TREE: Sprite = Sprite {
    rows: &[
        "..LLLL..", ".LLLLLL.", "LLLLLLLL", "LLLLLLLL", ".LLLLLL.", "...TT...", "...TT...",
    ],
    palette: &[(b'L', rgb(60, 140, 70)), (b'T', rgb(110, 75, 45))],
};

/// The signpost by the house: SETTINGS.
pub static SIGN: Sprite = Sprite {
    rows: &[
        "KKKKKKKK", "KWWWWWWK", "KWKKKKWK", "KWWWWWWK", "KKKKKKKK", "...KK...", "...KK...",
        "...KK...",
    ],
    palette: &[(b'K', rgb(110, 75, 45)), (b'W', rgb(235, 220, 180))],
};

/// A park bench, where walks start.
pub static BENCH: Sprite = Sprite {
    rows: &[
        "KKKKKKKKKKKK",
        "WWWWWWWWWWWW",
        "KKKKKKKKKKKK",
        "WWWWWWWWWWWW",
        ".K........K.",
        ".K........K.",
    ],
    palette: &[(b'K', rgb(90, 60, 35)), (b'W', rgb(160, 110, 60))],
};

/// The dock you fish from.
pub static DOCK: Sprite = Sprite {
    rows: &[
        "PPPPPPPPPPPP",
        "EEEEEEEEEEEE",
        ".S...S...S..",
        ".S...S...S..",
    ],
    palette: &[
        (b'P', rgb(140, 95, 55)),
        (b'E', rgb(90, 60, 35)),
        (b'S', rgb(100, 70, 40)),
    ],
};
