use bevy::prelude::*;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<Buttons>()
        .add_systems(Last, remember_buttons);
}

/// The board's two physical buttons by role. Platforms map the switches
/// (firmware: PWR = action, BOOT = zero; desktop: Space and Z) and call
/// [`ButtonState::set`] with the held state every frame.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Buttons {
    /// Hold-to-charge / confirm.
    pub action: ButtonState,
    /// Re-zeroes the IMU from any screen. A long hold starts sleep.
    pub zero: ButtonState,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonState {
    held: bool,
    was_held: bool,
}

impl ButtonState {
    pub fn set(&mut self, held: bool) {
        self.held = held;
    }

    pub fn held(&self) -> bool {
        self.held
    }

    /// True for exactly one update after the button goes down.
    pub fn pressed(&self) -> bool {
        self.held && !self.was_held
    }

    /// True for exactly one update after the button comes up.
    pub fn released(&self) -> bool {
        !self.held && self.was_held
    }
}

fn remember_buttons(mut buttons: ResMut<Buttons>) {
    let Buttons { action, zero } = &mut *buttons;
    for button in [action, zero] {
        button.was_held = button.held;
    }
}
