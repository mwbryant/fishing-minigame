use bevy::prelude::*;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<Touch>()
        .add_systems(Last, remember_touch);
}

/// The finger (or mouse) in screen pixels. Platforms call [`Touch::set`]
/// before `app.update()` whenever they poll the touch controller.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Touch {
    current: Option<Vec2>,
    previous: Option<Vec2>,
}

impl Touch {
    pub fn set(&mut self, position: Option<Vec2>) {
        self.current = position;
    }

    pub fn position(&self) -> Option<Vec2> {
        self.current
    }

    /// True for exactly one update after the finger lands.
    pub fn just_pressed(&self) -> bool {
        self.current.is_some() && self.previous.is_none()
    }
}

fn remember_touch(mut touch: ResMut<Touch>) {
    touch.previous = touch.current;
}
