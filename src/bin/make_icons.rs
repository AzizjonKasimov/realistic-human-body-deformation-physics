//! Draws the browser version's app icons into `web/icons`: the mannequin the
//! body is built from, with the sledgehammer about to strike its arm, in the
//! game's own colors on its dark background. Browsers put them on the home
//! screen, in the app list, and on the tab when the page is installed as an
//! app (`web/manifest.webmanifest`).
//!
//! ```text
//! make_icons [OUT_DIR]        (default web/icons)
//! ```
//!
//! Each icon is drawn from distance fields at its own size, so its edges stay
//! sharp from the 32 px tab icon to the 512 px one. Native only.

use macroquad::prelude::Image;
use realistic_physics as rp;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

type Rgb = [f64; 3];
type Vec2 = (f64, f64);

// The game's colors, from `render_palette` and `draw_sledgehammer` in
// `src/bin/realistic_physics/main.rs`, and the start screen's glow.
const BACKGROUND: Rgb = [15.0, 15.0, 17.0];
const BACKGROUND_LOW: Rgb = [26.0, 22.0, 22.0];
const GLOW: Rgb = [33.0, 29.0, 30.0];
const SKIN_LIGHT: Rgb = [201.0, 146.0, 119.0];
const SKIN_BASE: Rgb = [152.0, 101.0, 83.0];
const HANDLE_EDGE: Rgb = [58.0, 40.0, 26.0];
const HANDLE: Rgb = [156.0, 112.0, 64.0];
const HANDLE_LIGHT: Rgb = [196.0, 150.0, 96.0];
const GRIP: Rgb = [44.0, 36.0, 30.0];
const HEAD: Rgb = [74.0, 78.0, 83.0];
const HEAD_LIGHT: Rgb = [118.0, 124.0, 130.0];
const HEAD_FACE: Rgb = [168.0, 174.0, 178.0];
const HEAD_EDGE: Rgb = [22.0, 23.0, 25.0];

/// The game's figure height in pixels, which the sledgehammer's sizes are
/// measured against (`tool_geometry` in `src/simulation/tools.rs`).
const BODY_PX: f64 = 561.6;
/// The sledgehammer is drawn larger than in the game, so it still reads on a
/// small icon.
const HAMMER_SCALE: f64 = 2.0;
/// It swings in at the arm, its head turned this far down from level.
const HAMMER_TILT: f64 = 0.35;
/// Space left between the head's face and the arm, in body heights.
const HAMMER_GAP: f64 = 0.012;
/// Where the head aims: the outside of the viewer's left upper arm.
const TARGET: Vec2 = (-0.140, 0.270);
/// Body height below which the figure may leave a maskable icon's safe
/// circle: the legs run on out of the frame.
const LEGS: f64 = 0.55;

struct Icon {
    file: &'static str,
    size: usize,
    /// Corner radius of the background, as a share of the size; 0 fills the
    /// square, for icons the system crops to its own shape.
    corner: f64,
    /// One body height as a share of the size. Above about 0.9 the frame
    /// cuts the figure off at the legs, which brings the figure and hammer
    /// closer.
    zoom: f64,
    /// Where the top of the head sits, as a share of the size from the top.
    crown: f64,
    /// Android crops maskable icons to its own shape and keeps only the circle
    /// 80% across for certain, so the hammer, the head, and the body down to
    /// `LEGS` must stay inside it.
    maskable: bool,
}

const ICONS: [Icon; 6] = [
    Icon {
        file: "icon-192.png",
        size: 192,
        corner: 0.2,
        zoom: 1.1,
        crown: 0.09,
        maskable: false,
    },
    Icon {
        file: "icon-512.png",
        size: 512,
        corner: 0.2,
        zoom: 1.1,
        crown: 0.09,
        maskable: false,
    },
    Icon {
        file: "icon-maskable-192.png",
        size: 192,
        corner: 0.0,
        zoom: 0.86,
        crown: 0.15,
        maskable: true,
    },
    Icon {
        file: "icon-maskable-512.png",
        size: 512,
        corner: 0.0,
        zoom: 0.86,
        crown: 0.15,
        maskable: true,
    },
    // iOS rounds the corners itself and fills any transparency with black.
    Icon {
        file: "apple-touch-icon.png",
        size: 180,
        corner: 0.0,
        zoom: 1.1,
        crown: 0.09,
        maskable: false,
    },
    Icon {
        file: "favicon-32.png",
        size: 32,
        corner: 0.2,
        zoom: 1.2,
        crown: 0.06,
        maskable: false,
    },
];

fn main() {
    let out = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("web/icons"));
    fs::create_dir_all(&out).unwrap_or_else(|error| fail(&format!("{}: {error}", out.display())));
    let hammer = Hammer::new();
    for icon in &ICONS {
        let (pixels, reach) = draw(icon, &hammer);
        if icon.maskable && reach > 0.4 {
            fail(&format!(
                "{} draws {reach:.3} of its size from the center, outside the maskable safe \
                 circle (0.4); lower its zoom or move its crown",
                icon.file
            ));
        }
        // Image rows run bottom up, the way export_png expects screen grabs.
        let size = icon.size;
        let mut bytes = Vec::with_capacity(size * size * 4);
        for row in (0..size).rev() {
            bytes.extend_from_slice(&pixels[row * size * 4..(row + 1) * size * 4]);
        }
        let image = Image {
            bytes,
            width: size as u16,
            height: size as u16,
        };
        let path = out.join(icon.file);
        image.export_png(&path.to_string_lossy());
        println!(
            "wrote {} ({size}x{size}, drawn to {reach:.3} of its size from the center)",
            path.display()
        );
    }
}

/// The sledgehammer as the game draws it, in body heights from the top of
/// the figure's head: `heading` runs along the head toward its striking face
/// and `side` down the handle.
struct Hammer {
    center: Vec2,
    heading: Vec2,
    side: Vec2,
    half_length: f64,
    half_width: f64,
    handle_length: f64,
    handle_half_width: f64,
    /// One of the game's screen pixels at the hammer's size.
    px: f64,
}

impl Hammer {
    fn new() -> Self {
        let geometry = rp::tool_geometry(rp::ToolMode::Heavy);
        let unit = HAMMER_SCALE / BODY_PX;
        let half_length = (geometry.contact_back + geometry.contact_front) * 0.5 * unit
            + geometry.contact_radius * unit;
        let heading = (HAMMER_TILT.cos(), HAMMER_TILT.sin());
        Self {
            center: sub(TARGET, scale(heading, half_length + HAMMER_GAP)),
            heading,
            side: (-heading.1, heading.0),
            half_length,
            half_width: geometry.body_half_width * unit,
            handle_length: geometry.handle_length * unit,
            handle_half_width: geometry.handle_half_width * unit,
            px: unit,
        }
    }

    /// The corners of the head and the handle's end, which bound it.
    fn corners(&self) -> Vec<Vec2> {
        let mut corners = Vec::new();
        for (along, across) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            corners.push(self.at(along * self.half_length, across * self.half_width));
        }
        let end = self.half_width + self.handle_length;
        let edge = self.handle_half_width + self.px;
        corners.push(self.at(edge, end));
        corners.push(self.at(-edge, end));
        corners
    }

    fn at(&self, along: f64, across: f64) -> Vec2 {
        add(
            self.center,
            add(scale(self.heading, along), scale(self.side, across)),
        )
    }

    /// Paints the hammer over `color` at `p`, in the game's drawing order:
    /// the handle first, so the head covers the joint. `pixel` is one icon
    /// pixel in body heights.
    fn paint(&self, color: &mut Rgb, p: Vec2, pixel: f64) {
        let px = self.px;
        let (hl, hw) = (self.half_length, self.half_width);
        let handle_start = hw * 0.6;
        let handle_end = hw + self.handle_length;
        // A stretch of the handle from `from` to `to` down it, moved `shift`
        // along the head.
        let handle = |from: f64, to: f64, half_width: f64, shift: f64| {
            let center = self.at(shift, (from + to) * 0.5);
            box_distance(p, center, self.side, ((to - from) * 0.5, half_width))
        };
        let thickness = self.handle_half_width;
        paint(
            color,
            HANDLE_EDGE,
            handle(handle_start, handle_end, thickness + px, 0.0),
            pixel,
        );
        paint(
            color,
            HANDLE,
            handle(handle_start, handle_end, thickness, 0.0),
            pixel,
        );
        paint(
            color,
            HANDLE_LIGHT,
            handle(handle_start, handle_end, 0.6 * px, -1.5 * px),
            pixel,
        );
        let wrap_start = hw + self.handle_length * 0.72;
        paint(
            color,
            GRIP,
            handle(wrap_start, handle_end, thickness + 0.5 * px, 0.0),
            pixel,
        );

        // A box on the head, from `along.0` to `along.1` toward the striking
        // face and `across.0` to `across.1` toward the handle.
        let head = |along: (f64, f64), across: (f64, f64)| {
            let center = self.at((along.0 + along.1) * 0.5, (across.0 + across.1) * 0.5);
            box_distance(
                p,
                center,
                self.heading,
                ((along.1 - along.0) * 0.5, (across.1 - across.0) * 0.5),
            )
        };
        let whole = head((-hl, hl), (-hw, hw));
        paint(color, HEAD, whole, pixel);
        // Light falls on the side away from the handle.
        paint(
            color,
            HEAD_LIGHT,
            head(
                (-hl + 2.0 * px, hl - 2.0 * px),
                (-hw + 2.0 * px, -hw * 0.15),
            ),
            pixel,
        );
        paint(
            color,
            HEAD_FACE,
            head((hl - 4.0 * px, hl), (-hw, hw)),
            pixel,
        );
        paint(
            color,
            HEAD_FACE,
            head((-hl, -hl + 4.0 * px), (-hw, hw)),
            pixel,
        );
        paint(color, HEAD_EDGE, whole.abs() - px, pixel);
    }
}

/// How far left and right, in body heights from the midline, the figure
/// down to `bottom` and the hammer reach.
fn art_span(hammer: &Hammer, bottom: f64) -> (f64, f64) {
    let (mut left, mut right) = (f64::MAX, f64::MIN);
    let step = 0.002;
    let mut v = -0.03;
    while v <= bottom.min(1.05) {
        let mut u = -0.3;
        while u <= 0.3 {
            if rp::silhouette_distance(u, v) < 0.0 {
                left = left.min(u);
                right = right.max(u);
            }
            u += step;
        }
        v += step;
    }
    for corner in hammer.corners() {
        left = left.min(corner.0);
        right = right.max(corner.0);
    }
    (left, right)
}

/// The icon's pixels, top row first, and how far from its center, as a
/// share of its size, the hammer and the figure above `LEGS` were drawn.
fn draw(icon: &Icon, hammer: &Hammer) -> (Vec<u8>, f64) {
    let size = icon.size as f64;
    // Icon coordinates run 0 to 1 across. Body heights map onto them at
    // `zoom` from the crown down, with what shows of the figure and the
    // hammer centered across.
    let bottom = (1.0 - icon.crown) / icon.zoom;
    let (left, right) = art_span(hammer, bottom);
    let origin = ((left + right) * 0.5, -icon.crown / icon.zoom);
    let pixel = 1.0 / (size * icon.zoom);
    let mut pixels = Vec::with_capacity(icon.size * icon.size * 4);
    let mut reach: f64 = 0.0;
    for row in 0..icon.size {
        for col in 0..icon.size {
            let x = (col as f64 + 0.5) / size;
            let y = (row as f64 + 0.5) / size;
            let p = (origin.0 + (x - 0.5) / icon.zoom, origin.1 + y / icon.zoom);

            // The game's backdrop: dark above, warmer below, lit softly
            // behind the figure as the start screen is.
            let mut color = mix(BACKGROUND, BACKGROUND_LOW, smoothstep(0.45, 0.85, y));
            let glow = (1.0 - libm::hypot(x - 0.5, (y - 0.5) * 0.8) / 0.55).clamp(0.0, 1.0);
            color = mix(color, GLOW, glow * glow);

            let backdrop = color;
            let figure = rp::silhouette_distance(p.0, p.1);
            // Skin darkens toward the outline, as the game shades it.
            let skin = mix(SKIN_BASE, SKIN_LIGHT, smoothstep(0.0, 0.02, -figure));
            paint(&mut color, skin, figure, pixel);
            let above_legs = color != backdrop && p.1 <= LEGS;
            let under_hammer = color;
            hammer.paint(&mut color, p, pixel);
            if above_legs || color != under_hammer {
                reach = reach.max(libm::hypot(x - 0.5, y - 0.5));
            }

            let alpha = if icon.corner > 0.0 {
                coverage(rounded_square_distance((x, y), icon.corner) * size)
            } else {
                1.0
            };
            pixels.extend(
                color
                    .iter()
                    .map(|channel| channel.round().clamp(0.0, 255.0) as u8),
            );
            pixels.push((alpha * 255.0).round() as u8);
        }
    }
    (pixels, reach)
}

/// Lays `layer` over `color` where the shape at `distance` (body heights,
/// negative inside) covers the pixel.
fn paint(color: &mut Rgb, layer: Rgb, distance: f64, pixel: f64) {
    let cover = coverage(distance / pixel);
    if cover > 0.0 {
        *color = mix(*color, layer, cover);
    }
}

/// How much of a pixel a shape covers, from the distance of the pixel's
/// center to its edge in pixels.
fn coverage(distance: f64) -> f64 {
    (0.5 - distance).clamp(0.0, 1.0)
}

/// Signed distance to a box centered at `center`, `half.0` along `axis` and
/// `half.1` across it.
fn box_distance(p: Vec2, center: Vec2, axis: Vec2, half: Vec2) -> f64 {
    let d = sub(p, center);
    let along = d.0 * axis.0 + d.1 * axis.1;
    let across = d.1 * axis.0 - d.0 * axis.1;
    let q = (along.abs() - half.0, across.abs() - half.1);
    libm::hypot(q.0.max(0.0), q.1.max(0.0)) + q.0.max(q.1).min(0.0)
}

/// Signed distance to the unit square with rounded corners, in icon sizes.
fn rounded_square_distance(p: Vec2, radius: f64) -> f64 {
    let q = (
        (p.0 - 0.5).abs() - (0.5 - radius),
        (p.1 - 0.5).abs() - (0.5 - radius),
    );
    libm::hypot(q.0.max(0.0), q.1.max(0.0)) + q.0.max(q.1).min(0.0) - radius
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    (a.0 + b.0, a.1 + b.1)
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    (a.0 - b.0, a.1 - b.1)
}

fn scale(a: Vec2, s: f64) -> Vec2 {
    (a.0 * s, a.1 * s)
}

fn fail(message: &str) -> ! {
    eprintln!("make_icons: {message}");
    process::exit(1);
}
