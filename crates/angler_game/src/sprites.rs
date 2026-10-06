//! Pixel art as text. `.` is transparent; every other key is in the palette.
//! Fish face right (head on the right, tail on the left).

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

/// Faces right, bareheaded: row 0 is hair, which the worn hat covers. The
/// hands are at column 11, row 6.
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

pub const ANGLER_HANDS: (i32, i32) = (11, 6);

// Hats share the angler's columns (the face is columns 4-8) and are drawn
// with their bottom row over the angler's hair row.

pub static STRAW_HAT: Sprite = Sprite {
    rows: &["....HHHHH", "...HHHHHHH", "..HHHHHHHHHH"],
    palette: &[(b'H', rgb(200, 170, 90))],
};

pub static BEANIE: Sprite = Sprite {
    rows: &["......W", "....RRRRR", "...RRRRRRR", "...CCCCCCC"],
    palette: &[
        (b'W', rgb(240, 240, 240)),
        (b'R', rgb(205, 55, 55)),
        (b'C', rgb(150, 35, 45)),
    ],
};

/// The bill points the way the angler faces.
pub static BALL_CAP: Sprite = Sprite {
    rows: &[".....BBBB", "....BBBBBB", "...BBBBBBBVVV"],
    palette: &[(b'B', rgb(55, 95, 200)), (b'V', rgb(35, 60, 140))],
};

pub static BUCKET_HAT: Sprite = Sprite {
    rows: &["....GGGGG", "....GGGGG", "...DDDDDDD", "..GGGGGGGGG"],
    palette: &[(b'G', rgb(125, 140, 80)), (b'D', rgb(75, 85, 45))],
};

pub static COWBOY_HAT: Sprite = Sprite {
    rows: &["....TT.TT", "....TTTTT", "T...DDDDD...T", "TTTTTTTTTTTTT"],
    palette: &[(b'T', rgb(165, 110, 55)), (b'D', rgb(95, 60, 30))],
};

/// A bee costume: a striped dome, antennae, and little wings.
pub static BEE_HAT: Sprite = Sprite {
    rows: &[
        "...K.....K",
        "....K...K",
        "....YYYYY",
        "..WKKKKKKKW",
        "...YYYYYYY",
    ],
    palette: &[
        (b'Y', rgb(250, 205, 40)),
        (b'K', rgb(30, 30, 30)),
        (b'W', rgb(220, 235, 255)),
    ],
};

/// A spider sitting on the head, legs draped down, eyes to the front.
pub static SPIDER_HAT: Sprite = Sprite {
    rows: &[
        "..K.......K",
        "...K.KKK.K",
        "....KKKKK",
        "..KKKKKRKKK",
        ".K..K...K..K",
    ],
    palette: &[(b'K', rgb(44, 38, 50)), (b'R', rgb(230, 40, 40))],
};

/// A skateboard balanced wheels-up.
pub static SKATE_HAT: Sprite = Sprite {
    rows: &["..WW....WW", "...T....T", ".BBBBBBBBBB"],
    palette: &[
        (b'W', rgb(245, 245, 245)),
        (b'T', rgb(150, 150, 165)),
        (b'B', rgb(220, 120, 60)),
    ],
};

pub static TOP_HAT: Sprite = Sprite {
    rows: &[
        "....KSKKK",
        "....KSKKK",
        "....KSKKK",
        "....KKKKK",
        "....RRRRR",
        "..KKKKKKKKK",
    ],
    palette: &[
        (b'K', rgb(38, 38, 46)),
        (b'S', rgb(90, 90, 105)),
        (b'R', rgb(190, 40, 50)),
    ],
};

/// A koi asleep on the angler's head, nose to the front.
pub static FISH_HAT: Sprite = Sprite {
    rows: &[
        "K.....KKKK",
        "KK..KKOOOOK",
        "KOKKOOOOOWKK",
        "KK..KKYYYYK",
        "K.....KKKK",
    ],
    palette: &[
        OUTLINE,
        EYE,
        (b'O', rgb(240, 120, 30)),
        (b'Y', rgb(250, 200, 50)),
    ],
};

pub static CROWN: Sprite = Sprite {
    rows: &["....Y.Y.Y", "....YYYYY", "....YRYBY", "....YYYYY"],
    palette: &[
        (b'Y', rgb(250, 200, 50)),
        (b'R', rgb(220, 40, 60)),
        (b'B', rgb(60, 120, 230)),
    ],
};

/// A gold coin, shown beside gold amounts.
pub static COIN: Sprite = Sprite {
    rows: &[".KKK.", "KYYOK", "KYOOK", "KYOOK", ".KKK."],
    palette: &[
        (b'K', rgb(120, 80, 10)),
        (b'Y', rgb(255, 230, 120)),
        (b'O', rgb(240, 180, 40)),
    ],
};

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
