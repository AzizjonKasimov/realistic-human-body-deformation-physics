//! Front-facing mannequin figure that the body mesh is built from.
//!
//! The figure is defined in code rather than traced from a drawing, so it is
//! exactly mirror-symmetric, faces straight ahead, and stays a neutral
//! mannequin without anatomical detail: an egg-shaped head, a smooth neck and
//! torso outline, and limbs shaped as tapered capsules around the skeleton's
//! joints, ending in mitten hands and simple feet. The parts are blended into
//! one signed distance field. `simulation::body` builds the limb bones on the
//! same joints, so every bone runs down the middle of its limb.
//!
//! Coordinates: `u` is horizontal from the body midline (positive toward the
//! viewer's right), `v` runs down from the top of the head; both are in body
//! heights, so the figure spans `v` from 0 to 1. Every part is described for
//! the viewer's right side and mirrored onto the left.

use std::collections::HashMap;
use std::sync::OnceLock;

/// A position in body coordinates (`u`, `v`).
pub(crate) type Landmark = (f64, f64);

/// A circle in body coordinates: center and radius.
type Circle = (Landmark, f64);

// Joints of the viewer's right limbs; the left limbs mirror them.
pub(crate) const SHOULDER: Landmark = (0.106, 0.215);
pub(crate) const ELBOW: Landmark = (0.126, 0.378);
pub(crate) const WRIST: Landmark = (0.152, 0.508);
/// End of the hand bone, most of the way from the wrist to the fingertips.
pub(crate) const KNUCKLES: Landmark = lerp(WRIST, FINGERTIPS, 0.6);
pub(crate) const HIP: Landmark = (0.050, 0.505);
pub(crate) const KNEE: Landmark = (0.055, 0.718);
pub(crate) const ANKLE: Landmark = (0.058, 0.942);
/// End of the foot bone. Seen from the front the foot points at the viewer,
/// so it shows as a short block below the ankle, turned out slightly.
pub(crate) const TOES: Landmark = (0.062, 0.984);

const FINGERTIPS: Landmark = (0.166, 0.612);

/// Limb radii at the joint each limb starts from and at the joint it ends at.
const UPPER_ARM_RADII: (f64, f64) = (0.030, 0.023);
const FOREARM_RADII: (f64, f64) = (0.024, 0.016);
const THIGH_RADII: (f64, f64) = (0.042, 0.030);
const SHIN_RADII: (f64, f64) = (0.031, 0.018);
/// The mitten hand widens from the wrist to the palm, then tapers to the fingertips.
const PALM: Circle = (lerp(WRIST, FINGERTIPS, 0.42), 0.019);
const FINGERTIP_RADIUS: f64 = 0.015;
/// The foot: a block centered under `TOES` from just above the ankle down to
/// the sole, a little wider at the sole, with rounded corners.
const FOOT_TOP: f64 = 0.928;
const FOOT_HALF_WIDTHS: (f64, f64) = (0.019, 0.027);
const FOOT_ROUNDING: f64 = 0.010;
/// The egg-shaped head: a crown circle tapering to a smaller chin circle.
const CROWN: Circle = ((0.0, 0.047), 0.047);
const CHIN: Circle = ((0.0, 0.094), 0.034);

/// Right half of the neck and torso outline, from under the chin down the
/// side of the body to the crotch. It is smoothed by a Catmull-Rom spline and
/// mirrored; under the upper arm it runs inside the arm.
const TORSO_OUTLINE: [Landmark; 22] = [
    (0.000, 0.100), // under the chin, inside the head
    (0.031, 0.106),
    (0.032, 0.130),
    (0.035, 0.150), // base of the neck
    (0.052, 0.165),
    (0.075, 0.175),
    (0.095, 0.184),
    (0.106, 0.195), // top of the shoulder
    (0.108, 0.215),
    (0.100, 0.250),
    (0.094, 0.285), // armpit
    (0.088, 0.325),
    (0.084, 0.365),
    (0.083, 0.400), // waist
    (0.085, 0.430),
    (0.089, 0.462),
    (0.093, 0.492),
    (0.094, 0.522), // hip
    (0.088, 0.548),
    (0.045, 0.551),
    (0.015, 0.542),
    (0.000, 0.539), // crotch
];
const SPLINE_STEPS: usize = 4;
/// Beyond this distance from the torso outline's bounding box the distance to
/// the box stands in for the exact one; it is wider than any blend.
const TORSO_EXACT_MARGIN: f64 = 0.05;

/// Widths of the smooth blends between parts. The neck and the joints get
/// wide, rounded blends; the armpits and the crotch narrow ones, so the limbs
/// separate cleanly from the torso.
const NECK_BLEND: f64 = 0.030;
const JOINT_BLEND: f64 = 0.012;
const PALM_BLEND: f64 = 0.006;
const ANKLE_BLEND: f64 = 0.020;
const ARM_BLEND: f64 = 0.010;
const LEG_BLEND: f64 = 0.012;

/// Field samples per body height; under a pixel at the default 560 px body.
const FIELD_RESOLUTION: f64 = 640.0;
/// The field reaches at least this far either side of the midline.
const FIELD_HALF_WIDTH: f64 = 0.27;
const FIELD_V_RANGE: (f64, f64) = (-0.02, 1.02);

pub(crate) struct SilhouetteField {
    cols: usize,
    rows: usize,
    cell: f64,
    /// `u` of the first column. One column lies on the midline, so the samples
    /// mirror exactly.
    left: f64,
    /// Signed distance to the outline in body heights, negative inside.
    sdf: Vec<f64>,
}

impl SilhouetteField {
    /// Signed distance to the outline at (u, v) in body heights; negative inside.
    pub(crate) fn distance(&self, u: f64, v: f64) -> f64 {
        let gx = (u - self.left) / self.cell;
        let gy = (v - FIELD_V_RANGE.0) / self.cell;
        let max_x = (self.cols - 1) as f64;
        let max_y = (self.rows - 1) as f64;
        let cx = gx.clamp(0.0, max_x);
        let cy = gy.clamp(0.0, max_y);
        // Outside the raster everything is empty space, so add the gap to the edge value.
        let beyond = ((gx - cx).powi(2) + (gy - cy).powi(2)).sqrt() * self.cell;
        self.sample(cx, cy) + beyond
    }

    fn sample(&self, gx: f64, gy: f64) -> f64 {
        let x0 = (gx.floor() as usize).min(self.cols - 2);
        let y0 = (gy.floor() as usize).min(self.rows - 2);
        let fx = gx - x0 as f64;
        let fy = gy - y0 as f64;
        let at = |x: usize, y: usize| self.sdf[y * self.cols + x];
        let top = at(x0, y0) * (1.0 - fx) + at(x0 + 1, y0) * fx;
        let bottom = at(x0, y0 + 1) * (1.0 - fx) + at(x0 + 1, y0 + 1) * fx;
        top * (1.0 - fy) + bottom * fy
    }

    /// Closed outlines where the signed distance equals `level`, as (u, v)
    /// loops. Each loop starts at its topmost point, which on the figure is the
    /// crown of the head on the midline, so points spaced evenly along a loop
    /// from its start mirror from one side to the other.
    pub(crate) fn contours(&self, level: f64) -> Vec<Vec<(f64, f64)>> {
        let cols = self.cols;
        let value = |x: usize, y: usize| self.sdf[y * cols + x] - level;
        // Crossing points are keyed by the grid edge they lie on, so the segments
        // from neighboring cells chain into loops.
        let horizontal = |x: usize, y: usize| 2 * (y * cols + x);
        let vertical = |x: usize, y: usize| 2 * (y * cols + x) + 1;
        let mut links: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut link = |a: usize, b: usize| {
            links.entry(a).or_default().push(b);
            links.entry(b).or_default().push(a);
        };

        for y in 0..self.rows - 1 {
            for x in 0..cols - 1 {
                let v00 = value(x, y);
                let v10 = value(x + 1, y);
                let v11 = value(x + 1, y + 1);
                let v01 = value(x, y + 1);
                let case = usize::from(v00 < 0.0)
                    | usize::from(v10 < 0.0) << 1
                    | usize::from(v11 < 0.0) << 2
                    | usize::from(v01 < 0.0) << 3;
                let top = horizontal(x, y);
                let bottom = horizontal(x, y + 1);
                let left = vertical(x, y);
                let right = vertical(x + 1, y);
                let center_inside = (v00 + v10 + v11 + v01) < 0.0;
                match case {
                    1 | 14 => link(left, top),
                    2 | 13 => link(top, right),
                    3 | 12 => link(left, right),
                    4 | 11 => link(right, bottom),
                    6 | 9 => link(top, bottom),
                    7 | 8 => link(left, bottom),
                    // Saddles: the cell center decides which diagonal is connected.
                    5 if center_inside => {
                        link(top, right);
                        link(left, bottom);
                    }
                    5 => {
                        link(left, top);
                        link(right, bottom);
                    }
                    10 if center_inside => {
                        link(left, top);
                        link(right, bottom);
                    }
                    10 => {
                        link(top, right);
                        link(left, bottom);
                    }
                    _ => {}
                }
            }
        }

        let position = |key: usize| -> (f64, f64) {
            let node = key / 2;
            let (x, y) = (node % cols, node / cols);
            let (x1, y1) = if key.is_multiple_of(2) {
                (x + 1, y)
            } else {
                (x, y + 1)
            };
            let a = value(x, y);
            let b = value(x1, y1);
            let t = if (a - b).abs() > 1.0e-12 {
                (a / (a - b)).clamp(0.0, 1.0)
            } else {
                0.5
            };
            let gx = x as f64 + (x1 as f64 - x as f64) * t;
            let gy = y as f64 + (y1 as f64 - y as f64) * t;
            (self.left + gx * self.cell, FIELD_V_RANGE.0 + gy * self.cell)
        };

        let mut keys: Vec<usize> = links.keys().copied().collect();
        keys.sort_unstable();
        let mut visited = HashMap::new();
        let mut loops = Vec::new();
        for start in keys {
            if visited.contains_key(&start) {
                continue;
            }
            let mut outline = Vec::new();
            let mut previous = usize::MAX;
            let mut current = start;
            loop {
                visited.insert(current, ());
                outline.push(position(current));
                let next = links[&current]
                    .iter()
                    .copied()
                    .find(|&candidate| candidate != previous && !visited.contains_key(&candidate));
                match next {
                    Some(next) => {
                        previous = current;
                        current = next;
                    }
                    None => break,
                }
            }
            if outline.len() >= 3 {
                let top = (0..outline.len())
                    .min_by(|&a, &b| outline[a].1.total_cmp(&outline[b].1))
                    .unwrap_or(0);
                outline.rotate_left(top);
                loops.push(outline);
            }
        }
        loops
    }
}

/// The mannequin figure, built on first use.
pub(crate) fn human_silhouette() -> &'static SilhouetteField {
    static FIELD: OnceLock<SilhouetteField> = OnceLock::new();
    FIELD.get_or_init(build_field)
}

fn build_field() -> SilhouetteField {
    let cell = 1.0 / FIELD_RESOLUTION;
    let half_cols = (FIELD_HALF_WIDTH / cell).ceil() as usize;
    let cols = 2 * half_cols + 1;
    let rows = ((FIELD_V_RANGE.1 - FIELD_V_RANGE.0) / cell).ceil() as usize + 1;
    let torso = TorsoOutline::new();
    let mut sdf = Vec::with_capacity(cols * rows);
    let mut right_half = vec![0.0; half_cols + 1];
    for row in 0..rows {
        let v = FIELD_V_RANGE.0 + row as f64 * cell;
        for (k, value) in right_half.iter_mut().enumerate() {
            *value = figure_distance((k as f64 * cell, v), &torso);
        }
        sdf.extend((0..cols).map(|col| right_half[col.abs_diff(half_cols)]));
    }
    SilhouetteField {
        cols,
        rows,
        cell,
        left: -(half_cols as f64) * cell,
        sdf,
    }
}

/// Signed distance from a point on the right half (`u` >= 0) to the figure's
/// outline; negative inside.
fn figure_distance(p: Landmark, torso: &TorsoOutline) -> f64 {
    let head = capsule(p, CROWN, CHIN);
    let trunk = smooth_union(head, torso.distance(p), NECK_BLEND);
    let arm = smooth_union(
        smooth_union(
            capsule(p, (SHOULDER, UPPER_ARM_RADII.0), (ELBOW, UPPER_ARM_RADII.1)),
            capsule(p, (ELBOW, FOREARM_RADII.0), (WRIST, FOREARM_RADII.1)),
            JOINT_BLEND,
        ),
        hand(p),
        JOINT_BLEND,
    );
    let leg = smooth_union(
        smooth_union(
            capsule(p, (HIP, THIGH_RADII.0), (KNEE, THIGH_RADII.1)),
            capsule(p, (KNEE, SHIN_RADII.0), (ANKLE, SHIN_RADII.1)),
            JOINT_BLEND,
        ),
        foot(p),
        ANKLE_BLEND,
    );
    smooth_union(smooth_union(trunk, arm, ARM_BLEND), leg, LEG_BLEND)
}

fn hand(p: Landmark) -> f64 {
    let (along_u, along_v) = (FINGERTIPS.0 - WRIST.0, FINGERTIPS.1 - WRIST.1);
    let length = along_u.hypot(along_v);
    let tip = (
        FINGERTIPS.0 - along_u / length * FINGERTIP_RADIUS,
        FINGERTIPS.1 - along_v / length * FINGERTIP_RADIUS,
    );
    smooth_union(
        capsule(p, (WRIST, FOREARM_RADII.1), PALM),
        capsule(p, PALM, (tip, FINGERTIP_RADIUS)),
        PALM_BLEND,
    )
}

/// Inigo Quilez's isosceles trapezoid distance, grown by the rounding radius.
fn foot(p: Landmark) -> f64 {
    let half_height = (1.0 - FOOT_TOP) * 0.5 - FOOT_ROUNDING;
    let top = FOOT_HALF_WIDTHS.0 - FOOT_ROUNDING;
    let bottom = FOOT_HALF_WIDTHS.1 - FOOT_ROUNDING;
    // Centered on the foot with y pointing up.
    let x = (p.0 - TOES.0).abs();
    let y = (FOOT_TOP + 1.0) * 0.5 - p.1;
    let ca = (
        x - x.min(if y < 0.0 { bottom } else { top }),
        y.abs() - half_height,
    );
    let side = (top - bottom, 2.0 * half_height);
    let t = (((top - x) * side.0 + (half_height - y) * side.1)
        / (side.0 * side.0 + side.1 * side.1))
        .clamp(0.0, 1.0);
    let cb = (x - top + side.0 * t, y - half_height + side.1 * t);
    let sign = if cb.0 < 0.0 && ca.1 < 0.0 { -1.0 } else { 1.0 };
    sign * (ca.0 * ca.0 + ca.1 * ca.1)
        .min(cb.0 * cb.0 + cb.1 * cb.1)
        .sqrt()
        - FOOT_ROUNDING
}

/// Distance to the tapered capsule wrapped around two circles (Inigo Quilez's
/// uneven capsule).
fn capsule(p: Landmark, (a, radius_a): Circle, (b, radius_b): Circle) -> f64 {
    let (px, py) = (p.0 - a.0, p.1 - a.1);
    let (bx, by) = (b.0 - a.0, b.1 - a.1);
    let h = bx * bx + by * by;
    // Across and along the axis, in units of the squared axis length.
    let across = ((px * by - py * bx) / h).abs();
    let along = (px * bx + py * by) / h;
    let taper = radius_a - radius_b;
    let side = (h - taper * taper).sqrt();
    let k = side * along - taper * across;
    if k < 0.0 {
        (h * (across * across + along * along)).sqrt() - radius_a
    } else if k > side {
        (h * (across * across + (along - 1.0) * (along - 1.0))).sqrt() - radius_b
    } else {
        side * across + taper * along - radius_a
    }
}

/// Union of two distances that rounds the crease where they meet over about `width`.
fn smooth_union(a: f64, b: f64, width: f64) -> f64 {
    let h = (width - (a - b).abs()).max(0.0) / width;
    a.min(b) - h * h * width * 0.25
}

const fn lerp(a: Landmark, b: Landmark, t: f64) -> Landmark {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// The smoothed right half of the torso outline, from the midline under the
/// chin to the midline at the crotch.
struct TorsoOutline {
    points: Vec<Landmark>,
    max_u: f64,
    v_range: (f64, f64),
}

impl TorsoOutline {
    fn new() -> Self {
        let points = catmull_rom(&TORSO_OUTLINE, SPLINE_STEPS);
        let max_u = points.iter().map(|p| p.0).fold(f64::MIN, f64::max);
        let v_range = points.iter().fold((f64::MAX, f64::MIN), |(low, high), p| {
            (low.min(p.1), high.max(p.1))
        });
        Self {
            points,
            max_u,
            v_range,
        }
    }

    /// Signed distance for a point on the right half; the midline is a mirror
    /// line, not an edge.
    fn distance(&self, p: Landmark) -> f64 {
        let past_u = (p.0 - self.max_u).max(0.0);
        let past_v = (self.v_range.0 - p.1).max(p.1 - self.v_range.1).max(0.0);
        let box_distance = past_u.hypot(past_v);
        if box_distance > TORSO_EXACT_MARGIN {
            return box_distance;
        }
        let mut nearest_sq = f64::INFINITY;
        let mut inside = false;
        for pair in self.points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (eu, ev) = (b.0 - a.0, b.1 - a.1);
            let (wu, wv) = (p.0 - a.0, p.1 - a.1);
            let t = ((wu * eu + wv * ev) / (eu * eu + ev * ev)).clamp(0.0, 1.0);
            nearest_sq = nearest_sq.min((wu - eu * t).powi(2) + (wv - ev * t).powi(2));
            // Even-odd crossings of a ray toward +u. The half outline closes
            // along the midline, which never lies to the right of the point.
            if (a.1 <= p.1) != (b.1 <= p.1) && p.0 < a.0 + (p.1 - a.1) / ev * eu {
                inside = !inside;
            }
        }
        let distance = nearest_sq.sqrt();
        if inside {
            -distance
        } else {
            distance
        }
    }
}

/// Points on the Catmull-Rom spline through `controls`, `steps` per span.
fn catmull_rom(controls: &[Landmark], steps: usize) -> Vec<Landmark> {
    let last = controls.len() - 1;
    let mut points = Vec::with_capacity(last * steps + 1);
    for i in 0..last {
        let (p0, p1) = (controls[i.saturating_sub(1)], controls[i]);
        let (p2, p3) = (controls[i + 1], controls[(i + 2).min(last)]);
        for step in 0..steps {
            let t = step as f64 / steps as f64;
            let blend = |a: f64, b: f64, c: f64, d: f64| {
                0.5 * (2.0 * b
                    + (c - a) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
                    + (3.0 * b - a - 3.0 * c + d) * t * t * t)
            };
            points.push((blend(p0.0, p1.0, p2.0, p3.0), blend(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    points.push(controls[last]);
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans_at(field: &SilhouetteField, v: f64) -> usize {
        let mut spans = 0;
        let mut inside = false;
        let mut u = -0.26;
        while u < 0.26 {
            let now = field.distance(u, v) < 0.0;
            if now && !inside {
                spans += 1;
            }
            inside = now;
            u += 0.001;
        }
        spans
    }

    #[test]
    fn silhouette_has_human_landmarks() {
        let field = human_silhouette();
        assert!(field.distance(0.0, 0.06) < 0.0, "head center is inside");
        assert!(field.distance(0.0, 0.35) < 0.0, "torso center is inside");
        assert!(
            field.distance(0.0, 0.75) > 0.0,
            "gap between the legs is outside"
        );
        assert!(
            field.distance(0.0, -0.01) > 0.0,
            "space above the head is outside"
        );
        assert!(field.distance(TOES.0, 1.01) > 0.0, "feet end at the sole");
        assert_eq!(spans_at(field, 0.06), 1, "one head");
        assert_eq!(
            spans_at(field, 0.42),
            3,
            "two arms beside the torso at elbow height"
        );
        assert_eq!(spans_at(field, 0.78), 2, "two separated legs");
        assert_eq!(spans_at(field, 0.585), 4, "mitten hands beside the thighs");
    }

    #[test]
    fn silhouette_is_mirror_symmetric() {
        let field = human_silhouette();
        for row in 0..=100 {
            let v = row as f64 * 0.01;
            for col in 0..=50 {
                let u = col as f64 * 0.0051;
                let (right, left) = (field.distance(u, v), field.distance(-u, v));
                assert!(
                    (right - left).abs() < 1.0e-9,
                    "outline differs between sides at u={u:.3} v={v:.2}: {right} vs {left}"
                );
            }
        }
    }

    #[test]
    fn limb_bones_run_inside_their_limbs() {
        let field = human_silhouette();
        let limbs = [
            (SHOULDER, ELBOW),
            (ELBOW, WRIST),
            (WRIST, KNUCKLES),
            (HIP, KNEE),
            (KNEE, ANKLE),
            (ANKLE, TOES),
        ];
        for (a, b) in limbs {
            for step in 0..=8 {
                let (u, v) = lerp(a, b, step as f64 / 8.0);
                assert!(
                    field.distance(u, v) < -0.012,
                    "bone at ({u:.3}, {v:.3}) should be well inside its limb"
                );
            }
        }
    }

    #[test]
    fn outline_contour_is_one_closed_loop() {
        let field = human_silhouette();
        let loops = field.contours(0.0);
        assert_eq!(loops.len(), 1, "the figure is one piece without islands");
        assert!(
            loops[0].len() > 1000,
            "outline should be a long smooth loop"
        );
    }
}
