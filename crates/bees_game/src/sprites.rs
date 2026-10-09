//! Bees???'s pixel art as text (`.` is transparent). The bee faces right.

use orbit_gfx::Sprite;
use orbit_ui::rgb;

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
        (b'H', rgb(60, 50, 40)),
        (b'S', rgb(90, 90, 90)),
    ],
};

/// Where a hat sits on the bee: the head's center x in sprite pixels (it
/// spans columns 8-9) and its top row.
pub const BEE_HEAD: (f32, i32) = (9.0, 2);
