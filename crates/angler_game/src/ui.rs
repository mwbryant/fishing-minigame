//! Angler's touch buttons. Drawing and hit testing share these rectangles, so
//! what you see is what you tap.

pub use orbit_ui::Button;

/// On the title when the town launched the game: back out to the street.
pub const HOME: Button = Button::new(193, 40, 80, 30, "OUT");
pub const LOG: Button = Button::new(103, 352, 110, 36, "LOG");
pub const SETTINGS: Button = Button::new(253, 352, 110, 36, "SETTINGS");
/// On the aiming screen until the charge starts: back to the title.
pub const MENU: Button = Button::new(283, 366, 90, 34, "MENU");
pub const EASIER: Button = Button::new(70, 182, 56, 44, "<");
pub const HARDER: Button = Button::new(340, 182, 56, 44, ">");
pub const GUTEN: Button = Button::new(173, 296, 120, 36, "GUTEN");
pub const BACK: Button = Button::new(173, 352, 120, 36, "BACK");
/// On the catch screen, beside RELEASE.
pub const KEEP_FISH: Button = Button::new(88, 330, 140, 40, "KEEP");
pub const RELEASE_FISH: Button = Button::new(238, 330, 140, 40, "RELEASE");
/// On the swap screen, clockwise from 12 o'clock: the kept fish, then
/// letting the new catch go.
pub const SWAP_SLOTS: [Button; 4] = [
    Button::new(173, 60, 120, 50, ""),
    Button::new(330, 208, 116, 50, ""),
    Button::new(173, 356, 120, 50, ""),
    Button::new(20, 208, 116, 50, "LET IT GO"),
];
