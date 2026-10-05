//! Front-facing human silhouette used to generate the body mesh.
//!
//! The outline is the public-domain reference drawing in
//! `docs/reference/human_body_silhouette.svg`. It is rasterized once into a
//! signed distance field measured in body heights, with the spread fingers and
//! toes merged into mitten shapes that the tissue mesh can resolve.
//!
//! Coordinates: `u` is horizontal from the body midline (positive toward the
//! viewer's right), `v` runs down from the top of the head; both are in body
//! heights, so the figure spans `v` from 0 to 1.

use std::collections::HashMap;
use std::sync::OnceLock;

const REFERENCE_SVG: &str = include_str!("../docs/reference/human_body_silhouette.svg");

/// Field samples per body height; under a pixel at the default 560 px body.
const FIELD_RESOLUTION: f64 = 640.0;
const FIELD_U_RANGE: (f64, f64) = (-0.27, 0.27);
const FIELD_V_RANGE: (f64, f64) = (-0.02, 1.02);
const BEZIER_STEPS: usize = 8;
/// Below this height (the wrists), gaps narrower than twice the closing radius
/// are filled so fingers and toes merge into mittens instead of slivers.
const EXTREMITY_START_V: f64 = 0.525;
const EXTREMITY_CLOSING_RADIUS: f64 = 0.007;

pub(crate) struct SilhouetteField {
    cols: usize,
    rows: usize,
    cell: f64,
    /// Signed distance to the outline in body heights, negative inside.
    sdf: Vec<f64>,
}

impl SilhouetteField {
    /// Signed distance to the outline at (u, v) in body heights; negative inside.
    pub(crate) fn distance(&self, u: f64, v: f64) -> f64 {
        let gx = (u - FIELD_U_RANGE.0) / self.cell;
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

    /// Closed outlines where the signed distance equals `level`, as (u, v) loops.
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
            (
                FIELD_U_RANGE.0 + gx * self.cell,
                FIELD_V_RANGE.0 + gy * self.cell,
            )
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
                loops.push(outline);
            }
        }
        loops
    }
}

/// The reference human silhouette, built on first use.
pub(crate) fn human_silhouette() -> &'static SilhouetteField {
    static FIELD: OnceLock<SilhouetteField> = OnceLock::new();
    FIELD.get_or_init(build_field)
}

fn build_field() -> SilhouetteField {
    let outline = reference_outline();
    let cell = 1.0 / FIELD_RESOLUTION;
    let cols = ((FIELD_U_RANGE.1 - FIELD_U_RANGE.0) / cell).ceil() as usize + 1;
    let rows = ((FIELD_V_RANGE.1 - FIELD_V_RANGE.0) / cell).ceil() as usize + 1;

    let mut inside = rasterize(&outline, cols, rows, cell);
    close_extremities(&mut inside, cols, rows, cell);

    let to_inside = squared_distance_transform(&inside, cols, rows);
    let outside: Vec<bool> = inside.iter().map(|&filled| !filled).collect();
    let to_outside = squared_distance_transform(&outside, cols, rows);
    // The outline lies halfway between neighboring inside and outside samples.
    let raw: Vec<f64> = (0..inside.len())
        .map(|i| {
            if inside[i] {
                -(to_outside[i].sqrt() - 0.5) * cell
            } else {
                (to_inside[i].sqrt() - 0.5) * cell
            }
        })
        .collect();
    SilhouetteField {
        cols,
        rows,
        cell,
        sdf: smooth(&raw, cols, rows),
    }
}

/// Parses the reference SVG path (absolute M/C/L/Z commands) into a closed
/// polygon normalized to body heights.
fn reference_outline() -> Vec<(f64, f64)> {
    let data_start = REFERENCE_SVG
        .find(" d=\"")
        .expect("reference silhouette SVG has a path")
        + 4;
    let data_len = REFERENCE_SVG[data_start..]
        .find('"')
        .expect("reference silhouette path data is closed");
    let data = &REFERENCE_SVG[data_start..data_start + data_len];

    let mut points: Vec<(f64, f64)> = Vec::new();
    let mut numbers: Vec<f64> = Vec::new();
    let mut command = ' ';
    let mut current = (0.0, 0.0);
    let flush = |command: char,
                 numbers: &mut Vec<f64>,
                 points: &mut Vec<(f64, f64)>,
                 current: &mut (f64, f64)| {
        match command {
            'M' | 'L' => {
                for pair in numbers.chunks_exact(2) {
                    *current = (pair[0], pair[1]);
                    points.push(*current);
                }
            }
            'C' => {
                for curve in numbers.chunks_exact(6) {
                    let p0 = *current;
                    let (c1, c2, p3) = (
                        (curve[0], curve[1]),
                        (curve[2], curve[3]),
                        (curve[4], curve[5]),
                    );
                    for step in 1..=BEZIER_STEPS {
                        let t = step as f64 / BEZIER_STEPS as f64;
                        let s = 1.0 - t;
                        let a = s * s * s;
                        let b = 3.0 * s * s * t;
                        let c = 3.0 * s * t * t;
                        let d = t * t * t;
                        points.push((
                            a * p0.0 + b * c1.0 + c * c2.0 + d * p3.0,
                            a * p0.1 + b * c1.1 + c * c2.1 + d * p3.1,
                        ));
                    }
                    *current = p3;
                }
            }
            _ => {}
        }
        numbers.clear();
    };

    let mut token = String::new();
    for ch in data.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == 'e' {
            token.push(ch);
            continue;
        }
        if !token.is_empty() {
            numbers.push(token.parse().expect("reference silhouette number"));
            token.clear();
        }
        if ch.is_ascii_alphabetic() {
            flush(command, &mut numbers, &mut points, &mut current);
            command = ch.to_ascii_uppercase();
        }
    }
    if !token.is_empty() {
        numbers.push(token.parse().expect("reference silhouette number"));
    }
    flush(command, &mut numbers, &mut points, &mut current);

    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, y) in &points {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let height = max_y - min_y;
    let center_x = (min_x + max_x) * 0.5;
    points
        .into_iter()
        .map(|(x, y)| ((x - center_x) / height, (y - min_y) / height))
        .collect()
}

/// Even-odd fill of the polygon, sampled at the field nodes.
fn rasterize(outline: &[(f64, f64)], cols: usize, rows: usize, cell: f64) -> Vec<bool> {
    let mut inside = vec![false; cols * rows];
    let mut crossings = Vec::new();
    for row in 0..rows {
        let v = FIELD_V_RANGE.0 + row as f64 * cell;
        crossings.clear();
        for i in 0..outline.len() {
            let a = outline[i];
            let b = outline[(i + 1) % outline.len()];
            if (a.1 <= v) != (b.1 <= v) {
                let t = (v - a.1) / (b.1 - a.1);
                crossings.push(a.0 + t * (b.0 - a.0));
            }
        }
        crossings.sort_by(|a, b| a.partial_cmp(b).expect("finite crossing"));
        for span in crossings.chunks_exact(2) {
            let first = ((span[0] - FIELD_U_RANGE.0) / cell).ceil().max(0.0) as usize;
            let last = ((span[1] - FIELD_U_RANGE.0) / cell).floor();
            if last < 0.0 {
                continue;
            }
            for col in first..=(last as usize).min(cols - 1) {
                inside[row * cols + col] = true;
            }
        }
    }
    inside
}

/// Morphological closing limited to the hands and feet.
fn close_extremities(inside: &mut [bool], cols: usize, rows: usize, cell: f64) {
    let radius_sq = (EXTREMITY_CLOSING_RADIUS / cell).powi(2);
    let to_inside = squared_distance_transform(inside, cols, rows);
    let dilated_out: Vec<bool> = to_inside.iter().map(|&d| d > radius_sq).collect();
    let to_dilated_out = squared_distance_transform(&dilated_out, cols, rows);
    let first_row = ((EXTREMITY_START_V - FIELD_V_RANGE.0) / cell).ceil() as usize;
    for row in first_row.min(rows)..rows {
        for col in 0..cols {
            let i = row * cols + col;
            if to_dilated_out[i] > radius_sq {
                inside[i] = true;
            }
        }
    }
}

/// One pass of a [1 2 1] blur in each direction, which removes the raster
/// staircase from the outline without moving it.
fn smooth(values: &[f64], cols: usize, rows: usize) -> Vec<f64> {
    let mut horizontal = values.to_vec();
    for row in 0..rows {
        for col in 1..cols - 1 {
            let i = row * cols + col;
            horizontal[i] = (values[i - 1] + 2.0 * values[i] + values[i + 1]) * 0.25;
        }
    }
    let mut result = horizontal.clone();
    for row in 1..rows - 1 {
        for col in 0..cols {
            let i = row * cols + col;
            result[i] = (horizontal[i - cols] + 2.0 * horizontal[i] + horizontal[i + cols]) * 0.25;
        }
    }
    result
}

/// Squared Euclidean distance, in cells, from every node to the nearest node
/// where `feature` is set (Felzenszwalb and Huttenlocher's separable transform).
fn squared_distance_transform(feature: &[bool], cols: usize, rows: usize) -> Vec<f64> {
    const FAR: f64 = 1.0e20;
    let mut grid: Vec<f64> = feature
        .iter()
        .map(|&set| if set { 0.0 } else { FAR })
        .collect();
    let n = cols.max(rows);
    let mut input = vec![0.0; n];
    let mut output = vec![0.0; n];
    let mut hull = vec![0usize; n];
    let mut bounds = vec![0.0; n + 1];
    for col in 0..cols {
        for row in 0..rows {
            input[row] = grid[row * cols + col];
        }
        distance_transform_1d(&input[..rows], &mut output, &mut hull, &mut bounds);
        for row in 0..rows {
            grid[row * cols + col] = output[row];
        }
    }
    for row in 0..rows {
        input[..cols].copy_from_slice(&grid[row * cols..(row + 1) * cols]);
        distance_transform_1d(&input[..cols], &mut output, &mut hull, &mut bounds);
        grid[row * cols..(row + 1) * cols].copy_from_slice(&output[..cols]);
    }
    grid
}

fn distance_transform_1d(f: &[f64], d: &mut [f64], hull: &mut [usize], bounds: &mut [f64]) {
    let n = f.len();
    let intersection = |q: usize, p: usize| {
        ((f[q] + (q * q) as f64) - (f[p] + (p * p) as f64)) / (2.0 * (q as f64 - p as f64))
    };
    let mut k = 0;
    hull[0] = 0;
    bounds[0] = f64::NEG_INFINITY;
    bounds[1] = f64::INFINITY;
    for q in 1..n {
        let mut s = intersection(q, hull[k]);
        while s <= bounds[k] {
            k -= 1;
            s = intersection(q, hull[k]);
        }
        k += 1;
        hull[k] = q;
        bounds[k] = s;
        bounds[k + 1] = f64::INFINITY;
    }
    k = 0;
    for (q, value) in d.iter_mut().enumerate().take(n) {
        while bounds[k + 1] < q as f64 {
            k += 1;
        }
        let p = hull[k];
        *value = (q as f64 - p as f64).powi(2) + f[p];
    }
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
        assert_eq!(spans_at(field, 0.06), 1, "one head");
        assert_eq!(
            spans_at(field, 0.42),
            3,
            "two arms beside the torso at elbow height"
        );
        assert_eq!(spans_at(field, 0.78), 2, "two separated legs");
        assert_eq!(
            spans_at(field, 0.585),
            4,
            "mitten hands beside the thighs, no fingers"
        );
    }

    #[test]
    fn outline_contour_is_one_closed_loop() {
        let field = human_silhouette();
        let loops = field.contours(0.0);
        let longest = loops.iter().map(Vec::len).max().unwrap_or(0);
        assert!(longest > 1000, "main outline should be a long smooth loop");
        let total: usize = loops.iter().map(Vec::len).sum();
        assert!(
            total < longest + 40,
            "stray islands should be negligible next to the main outline"
        );
    }
}
