//! Draws side-view fighters in poses set by hand, from the figure's own
//! shapes (`profile::part_distance`), with nothing simulated: a sketch of how
//! a move is meant to look, to agree it with the user before it is built and
//! to set its key poses by eye. Native tests only; the frames go to
//! `output/sketch/NAME/`, and `print_*` tests below say which sketches there
//! are.

use crate::profile::{Joints, Part, Pose, Side};
use macroquad::prelude::Image;
use std::fs;
use std::path::{Path, PathBuf};

type Rgb = [f64; 3];

// The game's colours (`draw::Palette`).
const BACKGROUND: Rgb = [15.0, 15.0, 17.0];
const FLOOR: Rgb = [35.0, 29.0, 26.0];
const FLOOR_EDGE: Rgb = [92.0, 64.0, 55.0];
const SKIN_LIGHT: Rgb = [201.0, 146.0, 119.0];
const SKIN_MID: Rgb = [184.0, 130.0, 105.0];
const SKIN_BASE: Rgb = [152.0, 101.0, 83.0];
const OUTLINE: Rgb = [69.0, 40.0, 36.0];

/// A fighter in a pose, standing on the canvas.
#[derive(Clone, Copy)]
pub(crate) struct Figure {
    pub(crate) pose: Pose,
    /// Where the fighter's own zero stands across the canvas, in pixels.
    pub(crate) stand: f64,
    /// 1 facing the canvas's right, -1 its left.
    pub(crate) facing: f64,
}

/// A picture being drawn.
pub(crate) struct Canvas {
    width: usize,
    height: usize,
    /// How many pixels a body height is, and how far down the floor is.
    body: f64,
    floor: f64,
    pixels: Vec<Rgb>,
}

impl Canvas {
    /// An empty floor `width` by `height` pixels, a body height `body`
    /// pixels tall standing on the floor `floor` pixels down.
    pub(crate) fn new(width: usize, height: usize, body: f64, floor: f64) -> Self {
        let mut pixels = vec![BACKGROUND; width * height];
        for (row, line) in pixels.chunks_mut(width).enumerate() {
            let y = row as f64 + 0.5;
            let colour = if (y - floor).abs() < 1.5 {
                FLOOR_EDGE
            } else if y > floor {
                FLOOR
            } else {
                BACKGROUND
            };
            line.fill(colour);
        }
        Self {
            width,
            height,
            body,
            floor,
            pixels,
        }
    }

    /// Draws `figure`, its parts far to near, each filled and outlined: the
    /// far limbs darker, as they lie in shadow.
    pub(crate) fn draw(&mut self, figure: &Figure) {
        let joints = Joints::of(&figure.pose);
        let places = [
            joints.hips,
            joints.skull_top,
            joints.neck,
            joints.knuckles[0],
            joints.knuckles[1],
            joints.ball[0],
            joints.ball[1],
            joints.heel[0],
            joints.heel[1],
            joints.knee[0],
            joints.knee[1],
            joints.elbow[0],
            joints.elbow[1],
        ];
        let margin = 0.14;
        let (mut low, mut high) = (
            (f64::INFINITY, f64::INFINITY),
            (f64::NEG_INFINITY, f64::NEG_INFINITY),
        );
        for place in places {
            low = (low.0.min(place.0 - margin), low.1.min(place.1 - margin));
            high = (high.0.max(place.0 + margin), high.1.max(place.1 + margin));
        }
        let column = |x: f64| figure.stand + figure.facing * x * self.body;
        let (left, right) = {
            let (a, b) = (column(low.0), column(high.0));
            (
                a.min(b).floor().max(0.0) as usize,
                (a.max(b).ceil() as usize).min(self.width),
            )
        };
        let top = (self.floor - high.1 * self.body).floor().max(0.0) as usize;
        let bottom = ((self.floor - low.1 * self.body).ceil().max(0.0) as usize).min(self.height);
        for part in Part::FAR_TO_NEAR {
            let colour = match part {
                Part::Trunk => SKIN_MID,
                Part::UpperArm(Side::Lead)
                | Part::Forearm(Side::Lead)
                | Part::Thigh(Side::Lead)
                | Part::Shin(Side::Lead)
                | Part::Foot(Side::Lead) => SKIN_BASE,
                _ => SKIN_LIGHT,
            };
            for row in top..bottom {
                for col in left..right {
                    let own = (
                        (col as f64 + 0.5 - figure.stand) * figure.facing / self.body,
                        (self.floor - (row as f64 + 0.5)) / self.body,
                    );
                    // How far inside the outline, in pixels.
                    let inside = -crate::profile::part_distance(part, &joints, own) * self.body;
                    let cover = (inside + 0.5).clamp(0.0, 1.0);
                    if cover <= 0.0 {
                        continue;
                    }
                    let edge = (2.0 - inside).clamp(0.0, 1.0);
                    let paint = mix(colour, OUTLINE, edge);
                    let pixel = &mut self.pixels[row * self.width + col];
                    *pixel = mix(*pixel, paint, cover);
                }
            }
        }
    }

    /// Saves the picture as a PNG at `path`.
    pub(crate) fn save(&self, path: &Path) {
        // Image rows run bottom up, the way `export_png` expects screen
        // grabs.
        let mut bytes = Vec::with_capacity(self.width * self.height * 4);
        for row in (0..self.height).rev() {
            for pixel in &self.pixels[row * self.width..(row + 1) * self.width] {
                bytes.extend(pixel.map(|c| c.round().clamp(0.0, 255.0) as u8));
                bytes.push(255);
            }
        }
        let image = Image {
            bytes,
            width: self.width as u16,
            height: self.height as u16,
        };
        image.export_png(&path.to_string_lossy());
    }
}

fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t)
}

/// How high off the floor the lowest point of each of `pose`'s parts is, in
/// body heights, far to near: below zero, it sinks into the floor.
pub(crate) fn lowest_points(pose: &Pose) -> Vec<(Part, f64)> {
    let joints = Joints::of(pose);
    Part::FAR_TO_NEAR
        .iter()
        .map(|&part| {
            let mut lowest = f64::INFINITY;
            for i in 0..=400 {
                let x = -1.2 + i as f64 * 0.006;
                for j in 0..=300 {
                    let y = -0.2 + j as f64 * 0.004;
                    if y < lowest && crate::profile::part_distance(part, &joints, (x, y)) < 0.0 {
                        lowest = y;
                        break;
                    }
                }
            }
            (part, lowest)
        })
        .collect()
}

/// The folder a sketch's frames go to, emptied of any frames before.
pub(crate) fn frames_folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("output")
        .join("sketch")
        .join(name);
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(&folder).expect("make the sketch's folder");
    folder
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{self, Foot};

    /// A fighter lying on its back, knees up, as a knockout leaves one: not
    /// a pose the fighters are driven to, only where getting up begins.
    fn lying() -> Pose {
        let mut pose = Pose {
            hips: (0.0, 0.078),
            pelvis: -1.50,
            lumbar: -1.72,
            thorax: -1.62,
            neck: -1.45,
            head: -1.30,
            shoulders: [0.0, 0.0],
            fists: [(0.0, 0.0); 2],
            feet: [
                Foot {
                    ball: (0.40, 0.0),
                    pitch: 0.0,
                },
                Foot {
                    ball: (0.32, 0.0),
                    pitch: 0.0,
                },
            ],
        };
        // The arms lie along the floor beside the body, nearly straight.
        let shoulders = Joints::of(&pose).shoulder;
        pose.fists = shoulders.map(|shoulder| (shoulder.0 + 0.36, shoulder.1 + 0.01));
        pose
    }

    /// The key poses fighters get up through (`profile::sitting_up` and
    /// the rest, `simulation::rise`), by name, from the back and then from
    /// the front.
    fn key_poses() -> Vec<(&'static str, Pose)> {
        vec![
            ("lying", lying()),
            ("sitting up", profile::sitting_up()),
            ("bridging", profile::bridging()),
            ("crouching", profile::crouching()),
            ("on all fours", profile::on_all_fours()),
            ("kneeling up", profile::kneeling_up()),
            ("kneeling", profile::kneeling()),
            ("stance", profile::stance()),
        ]
    }

    /// How low each key pose's parts reach, to set them on the floor, and a
    /// sheet of them side by side (`output/sketch/key_poses`), for setting
    /// key poses by eye (`cargo test --release --lib print_key_poses --
    /// --ignored --nocapture`).
    #[test]
    #[ignore = "prints and draws the key poses; run with --ignored --nocapture"]
    fn print_key_poses() {
        let poses = key_poses();
        let folder = frames_folder("key_poses");
        let (body, gap) = (220.0, 1.05);
        let width = (body * gap * poses.len() as f64 + body * 0.6) as usize;
        let mut canvas = Canvas::new(width, 330, body, 300.0);
        for (index, (name, pose)) in poses.iter().enumerate() {
            let parts: Vec<String> = lowest_points(pose)
                .iter()
                .map(|(part, low)| format!("{part:?} {low:.3}"))
                .collect();
            println!("{name}: {}", parts.join(", "));
            canvas.draw(&Figure {
                pose: *pose,
                stand: body * (0.75 + gap * index as f64),
                facing: 1.0,
            });
        }
        canvas.save(&folder.join("key_poses.png"));
    }
}
