//! On-screen touch buttons. Their rectangles are shared by drawing and hit
//! testing so what you see is what you tap.

pub use orbit_ui::Button;

/// On the title when the arcade menu launched the game.
pub const HOME: Button = Button::new(193, 40, 80, 30, "HOME");
pub const LOG: Button = Button::new(73, 352, 100, 36, "LOG");
pub const SHOP: Button = Button::new(183, 352, 100, 36, "SHOP");
pub const SETTINGS: Button = Button::new(293, 352, 100, 36, "SETTINGS");
/// On the aiming screen until the charge starts: back to the title.
pub const MENU: Button = Button::new(283, 366, 90, 34, "MENU");
pub const EASIER: Button = Button::new(70, 182, 56, 44, "<");
pub const HARDER: Button = Button::new(340, 182, 56, 44, ">");
pub const GUTEN: Button = Button::new(103, 296, 120, 36, "GUTEN");
pub const RESET: Button = Button::new(243, 296, 120, 36, "RESET");
pub const ERASE: Button = Button::new(103, 296, 120, 36, "ERASE");
/// RESET's spot, so a double tap on RESET keeps everything.
pub const KEEP: Button = Button {
    text: "KEEP",
    ..RESET
};
pub const BACK: Button = Button::new(173, 352, 120, 36, "BACK");
pub const PREVIOUS_HAT: Button = Button::new(70, 150, 56, 44, "<");
pub const NEXT_HAT: Button = Button::new(340, 150, 56, 44, ">");
pub const BUY: Button = Button::new(173, 302, 120, 36, "BUY");
/// BUY's spot, for a hat already owned.
pub const WEAR: Button = Button {
    text: "WEAR",
    ..BUY
};
