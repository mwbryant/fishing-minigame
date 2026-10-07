//! The hat shop's shelf. Each kept fish wears one (or none); gold from
//! contests and the den buys them, and a hat bought is every fish's to wear.

use orbit_gfx::{Rgb565, Sprite};
use orbit_ui::rgb;

pub struct Hat {
    pub name: &'static str,
    pub price: u32,
    pub sprite: &'static Sprite,
}

/// The shelf, cheapest first. The first hat is free and always owned.
pub const HATS: [Hat; 12] = [
    Hat {
        name: "STRAW HAT",
        price: 0,
        sprite: &STRAW_HAT,
    },
    Hat {
        name: "BEANIE",
        price: 15,
        sprite: &BEANIE,
    },
    Hat {
        name: "BALL CAP",
        price: 30,
        sprite: &BALL_CAP,
    },
    Hat {
        name: "BUCKET HAT",
        price: 50,
        sprite: &BUCKET_HAT,
    },
    Hat {
        name: "BEE HAT",
        price: 70,
        sprite: &BEE_HAT,
    },
    Hat {
        name: "COWBOY HAT",
        price: 90,
        sprite: &COWBOY_HAT,
    },
    Hat {
        name: "CLOWN HAT",
        price: 100,
        sprite: &CLOWN_HAT,
    },
    Hat {
        name: "SPIDER HAT",
        price: 120,
        sprite: &SPIDER_HAT,
    },
    Hat {
        name: "TOP HAT",
        price: 150,
        sprite: &TOP_HAT,
    },
    Hat {
        name: "SKATE HAT",
        price: 200,
        sprite: &SKATE_HAT,
    },
    Hat {
        name: "FISH HAT",
        price: 250,
        sprite: &FISH_HAT,
    },
    Hat {
        name: "CROWN",
        price: 500,
        sprite: &CROWN,
    },
];

const _: () = assert!(HATS.len() <= 32, "owned hats are a u32 bitmask");
pub(crate) const ALL_HATS: u32 = u32::MAX >> (32 - HATS.len() as u32);

const OUTLINE: (u8, Rgb565) = (b'K', rgb(20, 22, 30));
const EYE: (u8, Rgb565) = (b'W', rgb(245, 245, 240));

// Hats are drawn on a 14-column head grid with the face in columns 4-8, the
// bottom row over the top of the head. (They were first drawn for Angler's
// angler; fish seat them by the same columns.)

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

/// A polka-dot cone with a pom-pom, between puffs of orange clown wig.
pub static CLOWN_HAT: Sprite = Sprite {
    rows: &[
        "......R",
        "......W",
        "..O..WBW..O",
        ".OOOWRWWBOOO",
        ".OOOBWWRWOOO",
    ],
    palette: &[
        (b'R', rgb(230, 45, 55)),
        (b'W', rgb(245, 245, 240)),
        (b'B', rgb(50, 110, 230)),
        (b'O', rgb(250, 130, 30)),
    ],
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
