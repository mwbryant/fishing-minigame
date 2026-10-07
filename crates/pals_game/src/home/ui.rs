//! The house's touch buttons.

pub use orbit_ui::Button;

pub const OUT: Button = Button::new(183, 404, 100, 32, "OUT");
pub const RENAME: Button = Button::new(75, 356, 104, 36, "RENAME");
pub const TRICKS: Button = Button::new(181, 356, 104, 36, "TRICKS");
pub const BACK: Button = Button::new(287, 356, 104, 36, "BACK");
/// The card's buttons, in their turning order.
pub const CARD_BUTTONS: [Button; 3] = [RENAME, TRICKS, BACK];
/// The tricks list's rows, then BACK.
pub const TRICK_ROWS: [Button; 6] = [
    Button::new(73, 96, 320, 40, ""),
    Button::new(73, 140, 320, 40, ""),
    Button::new(73, 184, 320, 40, ""),
    Button::new(73, 228, 320, 40, ""),
    Button::new(73, 272, 320, 40, ""),
    Button::new(173, 330, 120, 38, "BACK"),
];
/// In practice: teach the trick a different word or stroke.
pub const NEW_WORD: Button = Button::new(163, 412, 140, 30, "NEW COMMAND");
