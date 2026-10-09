//! Orbit's title, play and game-over screens as Bevy states, and the taps
//! that move between them.

use bevy::prelude::*;

use crate::Touch;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    Title,
    Playing,
    GameOver,
}

/// Game over ignores taps this long, since a tap right after losing is
/// usually the finger still chasing the ball.
pub const RETRY_DELAY_SECS: f32 = 0.8;

#[derive(Resource, Default)]
struct GameOverAt(f32);

pub(crate) fn plugin(app: &mut App) {
    app.init_state::<Screen>()
        .init_resource::<GameOverAt>()
        .add_systems(OnEnter(Screen::GameOver), stamp_game_over)
        .add_systems(
            Update,
            (
                start_on_tap.run_if(in_state(Screen::Title)),
                retry_on_tap.run_if(in_state(Screen::GameOver)),
            ),
        );
}

fn start_on_tap(touch: Res<Touch>, mut next: ResMut<NextState<Screen>>) {
    if touch.just_pressed() {
        next.set(Screen::Playing);
    }
}

fn stamp_game_over(time: Res<Time>, mut at: ResMut<GameOverAt>) {
    at.0 = time.elapsed_secs();
}

fn retry_on_tap(
    time: Res<Time>,
    at: Res<GameOverAt>,
    touch: Res<Touch>,
    mut next: ResMut<NextState<Screen>>,
) {
    if touch.just_pressed() && time.elapsed_secs() - at.0 >= RETRY_DELAY_SECS {
        next.set(Screen::Playing);
    }
}
