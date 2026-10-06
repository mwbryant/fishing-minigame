//! The dial's icons, as text pixel art (`.` is transparent).

use orbit_gfx::Sprite;
use orbit_ui::rgb;

pub static FISH: Sprite = Sprite {
    rows: &[
        "..OOOO...O",
        ".OOOOOO.OO",
        "OOWKOOOOOO",
        "OOOOOOOOOO",
        ".OYYYYO.OO",
        "..OOOO...O",
    ],
    palette: &[
        (b'O', rgb(240, 130, 40)),
        (b'Y', rgb(250, 200, 80)),
        (b'W', rgb(250, 250, 250)),
        (b'K', rgb(20, 20, 20)),
    ],
};

pub static SPIDER: Sprite = Sprite {
    rows: &[
        "G..G...G..G",
        ".G.G...G.G.",
        "..GGGGGGG..",
        "GG.GRGRG.GG",
        "..GGGGGGG..",
        ".G.GGGGG.G.",
        "G..G...G..G",
    ],
    palette: &[(b'G', rgb(170, 170, 185)), (b'R', rgb(230, 40, 40))],
};

pub static SKATEBOARD: Sprite = Sprite {
    rows: &[
        "B...........B",
        "BBBBBBBBBBBBB",
        ".DDDDDDDDDDD.",
        "..W.......W..",
        ".WWW.....WWW.",
    ],
    palette: &[
        (b'B', rgb(220, 120, 60)),
        (b'D', rgb(110, 60, 30)),
        (b'W', rgb(240, 240, 240)),
    ],
};

pub static GEAR: Sprite = Sprite {
    rows: &[
        "...SSS...",
        ".S.SSS.S.",
        "..SSSSS..",
        "SSSS.SSSS",
        "SSS...SSS",
        "SSSS.SSSS",
        "..SSSSS..",
        ".S.SSS.S.",
        "...SSS...",
    ],
    palette: &[(b'S', rgb(190, 195, 205))],
};

pub static BEE: Sprite = Sprite {
    rows: &[
        "...WW.....",
        "..WWWW....",
        ".YKYYKYKH.",
        "SYKYYKYKHH",
        ".YKYYKYKH.",
    ],
    palette: &[
        (b'W', rgb(215, 232, 255)),
        (b'Y', rgb(250, 205, 40)),
        (b'K', rgb(30, 30, 30)),
        (b'H', rgb(90, 80, 70)),
        (b'S', rgb(120, 120, 120)),
    ],
};
