use alloc::format;

use bevy::prelude::*;
use orbit_gfx::{Point, Rectangle, Rgb565, RgbColor, SCREEN_CENTER, Shape, Size};

use crate::{Ball, Frame, Paddle, Score, Screen, arena::*};

/// Pure black: AMOLED pixels are off, so the round panel's edge disappears.
pub const BACKGROUND: Rgb565 = Rgb565::BLACK;
const RIM: Rgb565 = Rgb565::new(3, 7, 9);
const PADDLE: Rgb565 = Rgb565::new(0, 52, 31);
const BALL: Rgb565 = Rgb565::new(31, 42, 6);
const SCORE_BEHIND_PLAY: Rgb565 = Rgb565::new(5, 11, 8);
const TEXT: Rgb565 = Rgb565::WHITE;
const HINT: Rgb565 = Rgb565::new(15, 30, 16);

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, draw_frame);
}

fn draw_frame(
    screen: Res<State<Screen>>,
    paddle: Res<Paddle>,
    score: Res<Score>,
    balls: Query<&Ball>,
    mut frame: ResMut<Frame>,
) {
    let shapes = &mut frame.shapes;
    shapes.clear();
    shapes.push(Shape::Ring {
        center: SCREEN_CENTER,
        radius: PADDLE_RADIUS as u32,
        width: 2,
        color: RIM,
    });
    match screen.get() {
        Screen::Title => {
            shapes.push(label(-36, "ORBIT", 4, TEXT));
            shapes.push(label(40, "TAP TO PLAY", 1, HINT));
        }
        Screen::Playing => {
            let watermark = label(0, &format!("{}", score.current), 6, SCORE_BEHIND_PLAY);
            let ink = orbit_gfx::ink(&watermark);
            let crossed = balls.iter().any(|ball| {
                let at = SCREEN_CENTER + Point::new(ball.pos.x as i32, ball.pos.y as i32);
                let reach = BALL_RADIUS as i32 + 3;
                ink.is_some_and(|ink| {
                    let near = Rectangle::with_center(at, Size::new_equal(reach as u32 * 2 + 1));
                    !ink.intersection(&near).is_zero_sized()
                })
            });
            // The score sits behind play, and steps aside while a ball crosses it.
            if !crossed {
                shapes.push(watermark);
            }
        }
        Screen::GameOver => {
            shapes.push(label(-72, "GAME OVER", 2, TEXT));
            shapes.push(label(-4, &format!("{}", score.current), 4, TEXT));
            shapes.push(label(56, &format!("BEST {}", score.best), 1, HINT));
            shapes.push(label(88, "TAP TO RETRY", 1, HINT));
        }
    }
    shapes.push(Shape::Arc {
        center: SCREEN_CENTER,
        radius: PADDLE_RADIUS as u32,
        width: PADDLE_THICKNESS as u32,
        start_deg: paddle.angle_deg - PADDLE_SPAN_DEG / 2.0,
        sweep_deg: PADDLE_SPAN_DEG,
        color: PADDLE,
    });
    for ball in &balls {
        let pos = ball.pos.round();
        shapes.push(Shape::Disc {
            center: SCREEN_CENTER + Point::new(pos.x as i32, pos.y as i32),
            radius: BALL_RADIUS as u32,
            color: BALL,
        });
    }
}

fn label(dy: i32, text: &str, scale: u32, color: Rgb565) -> Shape {
    Shape::Label {
        center: SCREEN_CENTER + Point::new(0, dy),
        text: text.into(),
        scale,
        color,
    }
}
