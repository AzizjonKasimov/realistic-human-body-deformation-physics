//! How the world is shown on a screen. Every body is simulated at the
//! scenarios' size and place, whatever the window (`world_frame`), because
//! the simulation works in pixels: its point spacing, its tools, and many of
//! its limits. A body built larger for a bigger screen was a finer mesh hit
//! by smaller tools, and was hurt differently. A [`View`] only draws the
//! world larger or smaller, and turns the pointer back into world positions.

use crate::scenarios::{SCENARIO_HEIGHT, SCENARIO_WIDTH};
use crate::{body_frame, BodyFrame, Vec2};

/// Where every body is simulated: the scenarios' place, 561.6 px tall, in a
/// world [`SCENARIO_WIDTH`] pixels wide whose sides nothing passes.
pub fn world_frame() -> BodyFrame {
    body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT)
}

/// How far apart the two fighters' midlines stand, in body heights: their
/// guards' elbows, 0.258 body heights out from each midline, just clear of
/// each other.
pub const FIGHTER_SPACING: f64 = 0.52;

/// Where each of two fighters stands in the same world as [`world_frame`],
/// at the same size, the viewer's left one first: side by side about the
/// world's middle, [`FIGHTER_SPACING`] apart.
pub fn fighter_frames() -> [BodyFrame; 2] {
    fighter_frames_at(FIGHTER_SPACING)
}

/// Where two fighters stand `spacing` body heights apart, as
/// [`fighter_frames`] places them.
pub fn fighter_frames_at(spacing: f64) -> [BodyFrame; 2] {
    let frame = world_frame();
    let middle = SCENARIO_WIDTH * 0.5;
    let half = spacing * frame.height * 0.5;
    [middle - half, middle + half].map(|x| BodyFrame {
        origin: Vec2 {
            x,
            y: frame.origin.y,
        },
        height: frame.height,
    })
}

/// How far out from its midline a fighter reaches in the guard, in body
/// heights, with room for a punch's fist.
pub const FIGHTER_REACH: f64 = 0.31;

/// Where to draw the scenarios' body frame ([`world_frame`]) so that two
/// fighters `spacing` body heights apart fit across a window `width` pixels
/// wide, from the left fighter's outer side to the right's with room for a
/// fist, `margin` pixels in from each side: no larger than `frame`, where a
/// lone body would be drawn, and smaller still if they would not fit, the
/// two centred across the window.
pub fn fighters_across(frame: BodyFrame, spacing: f64, width: f64, margin: f64) -> BodyFrame {
    let world = world_frame();
    let [left, right] = fighter_frames_at(spacing);
    let reach = FIGHTER_REACH * world.height;
    let (from, to) = (left.origin.x - reach, right.origin.x + reach);
    let room = (width - 2.0 * margin).max(1.0);
    let scale = (frame.height / world.height).min(room / (to - from));
    BodyFrame {
        origin: Vec2 {
            x: width * 0.5 + (world.origin.x - (from + to) * 0.5) * scale,
            y: frame.origin.y,
        },
        height: world.height * scale,
    }
}

/// The world drawn uniformly larger or smaller and moved: a world position
/// `p` lands on the screen at `p * scale + offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Screen pixels per world pixel.
    pub scale: f64,
    /// Where the world's origin lands on the screen.
    pub offset: Vec2,
}

impl View {
    /// The world drawn as it is, one world pixel to a screen pixel.
    pub const IDENTITY: View = View {
        scale: 1.0,
        offset: Vec2 { x: 0.0, y: 0.0 },
    };

    /// Draws the body simulated at `world` where `screen` places it. The same
    /// frame for both is the identity, exactly.
    pub fn fitting(world: BodyFrame, screen: BodyFrame) -> View {
        let scale = screen.height / world.height;
        View {
            scale,
            offset: Vec2 {
                x: screen.origin.x - world.origin.x * scale,
                y: screen.origin.y - world.origin.y * scale,
            },
        }
    }

    /// Where the world position `at` is drawn.
    pub fn to_screen(self, at: Vec2) -> Vec2 {
        Vec2 {
            x: at.x * self.scale + self.offset.x,
            y: at.y * self.scale + self.offset.y,
        }
    }

    /// The world position drawn at the screen position `at`.
    pub fn to_world(self, at: Vec2) -> Vec2 {
        Vec2 {
            x: (at.x - self.offset.x) / self.scale,
            y: (at.y - self.offset.y) / self.scale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(x: f64, y: f64, height: f64) -> BodyFrame {
        BodyFrame {
            origin: Vec2 { x, y },
            height,
        }
    }

    #[test]
    fn a_view_draws_the_body_where_its_screen_frame_places_it() {
        let world = world_frame();
        let screen = frame(998.4, 97.2, 720.0);
        let view = View::fitting(world, screen);
        for (u, v) in [(0.0, 0.0), (0.0, 1.0), (-0.3, 0.4), (0.25, 0.9)] {
            let drawn = view.to_screen(world.point(u, v));
            let wanted = screen.point(u, v);
            assert!(
                (drawn.x - wanted.x).abs() < 1.0e-9 && (drawn.y - wanted.y).abs() < 1.0e-9,
                "({u}, {v}) drawn at {drawn:?}, not {wanted:?}"
            );
        }
    }

    #[test]
    fn the_pointer_comes_back_to_where_it_was_drawn() {
        let view = View::fitting(world_frame(), frame(202.8, 75.96, 501.3));
        for at in [
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 665.6, y: 300.0 },
            Vec2 {
                x: 1280.0,
                y: 720.0,
            },
            Vec2 {
                x: -40.5,
                y: 912.25,
            },
        ] {
            let back = view.to_world(view.to_screen(at));
            assert!(
                (back.x - at.x).abs() < 1.0e-9 && (back.y - at.y).abs() < 1.0e-9,
                "{at:?} came back as {back:?}"
            );
        }
    }

    /// Two fighters drawn across a window fit inside it with their reach and
    /// stand centred in it, at the lone body's size where that fits.
    #[test]
    fn two_fighters_fit_across_the_window() {
        let world = world_frame();
        let [left, right] = fighter_frames();
        for width in [1280.0, 800.0, 390.0] {
            let lone = crate::body_frame(width, 720.0);
            let frame = fighters_across(lone, FIGHTER_SPACING, width, 12.0);
            assert!(frame.height <= lone.height + 1.0e-9);
            assert_eq!(frame.origin.y, lone.origin.y);
            let view = View::fitting(world, frame);
            let reach = FIGHTER_REACH * world.height;
            let from = view.to_screen(Vec2 {
                x: left.origin.x - reach,
                y: 0.0,
            });
            let to = view.to_screen(Vec2 {
                x: right.origin.x + reach,
                y: 0.0,
            });
            assert!(from.x >= 12.0 - 1.0e-9 && to.x <= width - 12.0 + 1.0e-9);
            assert!(((from.x + to.x) * 0.5 - width * 0.5).abs() < 1.0e-9);
        }
    }

    /// At 1280x720 the app draws the body where it is simulated, so its
    /// pointer must reach the world unchanged, bit for bit, as the
    /// scenarios' does.
    #[test]
    fn the_scenarios_own_frame_is_drawn_as_it_is() {
        let view = View::fitting(world_frame(), world_frame());
        assert_eq!(view, View::IDENTITY);
        let at = Vec2 {
            x: 358.4 + 1.0e-7,
            y: 331.2,
        };
        assert_eq!(view.to_world(at), at);
        assert_eq!(view.to_screen(at), at);
    }
}
