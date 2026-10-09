//! Angler's pixel art as text: `.` is transparent and every other key is in
//! the palette. Fish face right.

use orbit_gfx::{Rgb565, Sprite};

pub const fn rgb(r: u8, g: u8, b: u8) -> Rgb565 {
    Rgb565::new(r >> 3, g >> 2, b >> 3)
}

const OUTLINE: (u8, Rgb565) = (b'K', rgb(20, 22, 30));
const EYE: (u8, Rgb565) = (b'W', rgb(245, 245, 240));
const BELLY: (u8, Rgb565) = (b'L', rgb(232, 226, 190));

pub static MINNOW: Sprite = Sprite {
    rows: &[
        "K.....KKKK..",
        "KK..KKSSSSK.",
        "KSKKSSSSSWKK",
        "KK..KKLLLLK.",
        "K.....KKKK..",
    ],
    palette: &[OUTLINE, EYE, BELLY, (b'S', rgb(170, 186, 196))],
};

pub static BLUEGILL: Sprite = Sprite {
    rows: &[
        ".......KKKKK........",
        ".....KKBBBBBKK......",
        "K..KKBBBBBBBBBKK....",
        "KKKBBBBBBBBBKBWKK...",
        "KBKBBBBBBBBBBBBBBK..",
        "KKKOOOOOOOOOOOOOK...",
        "K..KKOOOOOOOOOKK....",
        ".....KKOOOOOKK......",
        ".......KKKKK........",
    ],
    palette: &[
        OUTLINE,
        EYE,
        (b'B', rgb(70, 110, 160)),
        (b'O', rgb(230, 140, 60)),
    ],
};

pub static PERCH: Sprite = Sprite {
    rows: &[
        "......RKKKKK........",
        ".....RKYYYYYKK......",
        "K..KKYDYYDYYDYKK....",
        "KKKYYYDYYDYYDYYWKK..",
        "KYKYYYDYYDYYDYYYYYK.",
        "KKKLLLLLLLLLLLLLLK..",
        "K..KKLLLLLLLLLKK....",
        ".....RKKLLLKKR......",
        "......R.KKK.R.......",
    ],
    palette: &[
        OUTLINE,
        EYE,
        BELLY,
        (b'Y', rgb(196, 190, 70)),
        (b'D', rgb(70, 80, 40)),
        (b'R', rgb(220, 90, 40)),
    ],
};

pub static TROUT: Sprite = Sprite {
    rows: &[
        ".........KKKKK......",
        "K....KKKGGDGGGKK....",
        "KK.KKGDGGGGGDGGGKK..",
        "KGKGGGGGDGGGGGGGWGKK",
        "KPKPPPPPPPPPPPPPPPPK",
        "KK.KLLLLLLLLLLLLLKK.",
        "K....KKLLLLLLLLKK...",
        ".........KKKK.......",
    ],
    palette: &[
        OUTLINE,
        EYE,
        BELLY,
        (b'G', rgb(120, 130, 80)),
        (b'D', rgb(50, 50, 40)),
        (b'P', rgb(220, 120, 140)),
    ],
};

pub static BASS: Sprite = Sprite {
    rows: &[
        "........KKKKK.......",
        "......KKGGGGGKK.....",
        "K...KKGGGGGGGGGKK...",
        "KK.KGGGGGGGGGGGGWKK.",
        "KDKDDDDDDDDDDDDGGGGK",
        "KK.KLLLLLLLLLLLLLKK.",
        "K...KKLLLLLLLLLKK...",
        "......KKLLLLLKK.....",
        "........KKKKK.......",
    ],
    palette: &[
        OUTLINE,
        EYE,
        BELLY,
        (b'G', rgb(80, 140, 70)),
        (b'D', rgb(40, 70, 35)),
    ],
};

pub static CATFISH: Sprite = Sprite {
    rows: &[
        ".......KKKK.........",
        "K....KKCCCCKKKKK....",
        "KK.KKCCCCCCCCCCCKK..",
        "KCKCCCCCCCCCCCCCWCKK",
        "KCCCCCCCCCCCCCCCCCKK",
        "KK.KLLLLLLLLLLLLLK.K",
        "K....KKLLLLLLLKKK..K",
        "........KKKK........",
    ],
    palette: &[OUTLINE, EYE, BELLY, (b'C', rgb(110, 100, 80))],
};

pub static PIKE: Sprite = Sprite {
    rows: &[
        "..........KKK...........",
        "K.......KKGGGKKKKKK.....",
        "KK..KKKKGLGGGLGGGGGKKK..",
        "KGKKGGLGGGGLGGGGLGGGWKKK",
        "KGKKGGGGLGGGGGLGGGGGGGGK",
        "KK..KKLLLLLLLLLLLLLLKKK.",
        "K.......KKKKKKKKKKKK....",
    ],
    palette: &[
        OUTLINE,
        EYE,
        (b'G', rgb(90, 120, 60)),
        (b'L', rgb(210, 214, 150)),
    ],
};

pub static GOLDEN_KOI: Sprite = Sprite {
    rows: &[
        "........KKKKKK......",
        "......KKOOWWOOKK....",
        "K...KKOOOWWWOOOOKK..",
        "KK.KOOWWOOOOOOOOWKK.",
        "KOKOOOWWWOOOOOOOOOOK",
        "KK.KYYYYYYYYYYYYYKK.",
        "K...KKYYYYYYYYYKK...",
        "......KKYYYYYKK.....",
        "........KKKKKK......",
    ],
    palette: &[
        OUTLINE,
        (b'W', rgb(250, 248, 240)),
        (b'O', rgb(240, 120, 30)),
        (b'Y', rgb(250, 200, 50)),
    ],
};

/// The anonymous shape on the reel bar; the species stays a surprise.
pub static REEL_FISH: Sprite = Sprite {
    rows: &[
        "K...KKKK..",
        "KK.KFFFFK.",
        "KFKFFFFWFK",
        "KK.KFFFFK.",
        "K...KKKK..",
    ],
    palette: &[OUTLINE, EYE, (b'F', rgb(150, 200, 220))],
};

/// The angler, facing right, hands at `ANGLER_HANDS`. The town uses it as the
/// walking avatar.
pub static ANGLER: Sprite = Sprite {
    rows: &[
        "....DDDDD.....",
        "....SSSSS.....",
        "....SSSKS.....",
        "....SSSSS.....",
        ".....SSS......",
        "...VVVVVVV....",
        "..VVVVVVVVSSS.",
        "..VVVVVVVV.SS.",
        "..VVVVVVV.....",
        "..VVVVVVV.....",
        "...VVVVV......",
        "...PPPPP......",
        "...PP.PP......",
        "...PP.PP......",
        "...PP.PP......",
        "...PP.PP......",
        "..BBB.BBB.....",
    ],
    palette: &[
        (b'D', rgb(90, 60, 35)),
        (b'S', rgb(236, 190, 150)),
        (b'K', rgb(30, 30, 30)),
        (b'V', rgb(60, 110, 70)),
        (b'P', rgb(60, 70, 110)),
        (b'B', rgb(70, 45, 30)),
    ],
};

/// The hands' pixel in `ANGLER`: column, row.
pub const ANGLER_HANDS: (i32, i32) = (11, 6);

/// The waterline sits between rows 2 and 3.
pub static BOBBER: Sprite = Sprite {
    rows: &["..K..", ".RRR.", "RRRRR", "WWWWW", ".WWW.", "..K.."],
    palette: &[
        (b'K', rgb(30, 30, 30)),
        (b'R', rgb(225, 50, 45)),
        (b'W', rgb(245, 245, 245)),
    ],
};

pub const BOBBER_WATERLINE: i32 = 3;
