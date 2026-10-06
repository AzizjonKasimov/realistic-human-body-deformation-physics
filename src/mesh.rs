//! Small 2D meshing helpers for building the body's tissue layers.

pub(crate) type P2 = (f64, f64);

/// Points spaced evenly (about `spacing` apart) along a closed polyline.
pub(crate) fn resample_loop(outline: &[P2], spacing: f64) -> Vec<P2> {
    let n = outline.len();
    let lengths: Vec<f64> = (0..n)
        .map(|i| distance(outline[i], outline[(i + 1) % n]))
        .collect();
    let perimeter: f64 = lengths.iter().sum();
    if n < 3 || perimeter <= spacing {
        return Vec::new();
    }
    let count = (perimeter / spacing).round().max(3.0) as usize;
    let step = perimeter / count as f64;
    let mut result = Vec::with_capacity(count);
    let mut segment = 0;
    let mut segment_start = 0.0;
    for k in 0..count {
        let target = k as f64 * step;
        while segment + 1 < n && segment_start + lengths[segment] < target {
            segment_start += lengths[segment];
            segment += 1;
        }
        let t = if lengths[segment] > 1.0e-12 {
            ((target - segment_start) / lengths[segment]).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let a = outline[segment];
        let b = outline[(segment + 1) % n];
        result.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
    }
    result
}

/// Hexagonal lattice covering the box, with vertical edges so muscle fibers
/// along limbs and the torso find well-aligned springs. A column runs down the
/// box's center line, so the lattice mirrors across it.
pub(crate) fn hex_lattice(min: P2, max: P2, edge: f64) -> Vec<P2> {
    let column_step = edge * 3.0_f64.sqrt() * 0.5;
    let center = (min.0 + max.0) * 0.5;
    let side_columns = ((max.0 - center) / column_step).floor() as i64;
    let mut points = Vec::new();
    for column in -side_columns..=side_columns {
        let x = center + column as f64 * column_step;
        let mut y = min.1 + if column % 2 != 0 { edge * 0.5 } else { 0.0 };
        while y <= max.1 {
            points.push((x, y));
            y += edge;
        }
    }
    points
}

#[derive(Clone, Copy)]
struct Triangle {
    v: [usize; 3],
    center: P2,
    radius_sq: f64,
}

impl Triangle {
    fn new(v: [usize; 3], points: &[P2]) -> Self {
        let (a, b, c) = (points[v[0]], points[v[1]], points[v[2]]);
        let d = 2.0 * (a.0 * (b.1 - c.1) + b.0 * (c.1 - a.1) + c.0 * (a.1 - b.1));
        let center = if d.abs() < 1.0e-12 {
            ((a.0 + b.0 + c.0) / 3.0, (a.1 + b.1 + c.1) / 3.0)
        } else {
            let a2 = a.0 * a.0 + a.1 * a.1;
            let b2 = b.0 * b.0 + b.1 * b.1;
            let c2 = c.0 * c.0 + c.1 * c.1;
            (
                (a2 * (b.1 - c.1) + b2 * (c.1 - a.1) + c2 * (a.1 - b.1)) / d,
                (a2 * (c.0 - b.0) + b2 * (a.0 - c.0) + c2 * (b.0 - a.0)) / d,
            )
        };
        let radius_sq = if d.abs() < 1.0e-12 {
            f64::INFINITY
        } else {
            distance_sq(center, a)
        };
        Self {
            v,
            center,
            radius_sq,
        }
    }
}

/// Delaunay triangulation (Bowyer-Watson). Triangles are returned with a
/// consistent winding: positive `(b - a) x (c - a)`.
pub(crate) fn delaunay(points: &[P2]) -> Vec<[usize; 3]> {
    let n = points.len();
    if n < 3 {
        return Vec::new();
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in points {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    let span = (max_x - min_x).max(max_y - min_y).max(1.0) * 20.0;
    let mid = ((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
    let mut vertices = points.to_vec();
    vertices.push((mid.0 - span, mid.1 - span));
    vertices.push((mid.0 + span, mid.1 - span));
    vertices.push((mid.0, mid.1 + span));
    let mut triangles = vec![oriented(
        Triangle::new([n, n + 1, n + 2], &vertices),
        &vertices,
    )];

    let mut bad = Vec::new();
    // Sorted vectors instead of a hash map keep the result identical run to run,
    // which the deterministic strike scenarios rely on.
    let mut cavity_edges: Vec<((usize, usize), usize, usize)> = Vec::new();
    for i in 0..n {
        let p = vertices[i];
        bad.clear();
        for (index, triangle) in triangles.iter().enumerate() {
            if distance_sq(p, triangle.center) < triangle.radius_sq * (1.0 - 1.0e-12) {
                bad.push(index);
            }
        }
        cavity_edges.clear();
        for &index in &bad {
            let v = triangles[index].v;
            for (a, b) in [(v[0], v[1]), (v[1], v[2]), (v[2], v[0])] {
                cavity_edges.push(((a.min(b), a.max(b)), a, b));
            }
        }
        cavity_edges.sort_unstable();
        for &index in bad.iter().rev() {
            triangles.swap_remove(index);
        }
        let mut k = 0;
        while k < cavity_edges.len() {
            let shared = k + 1 < cavity_edges.len() && cavity_edges[k + 1].0 == cavity_edges[k].0;
            if shared {
                k += 2;
                continue;
            }
            let (_, a, b) = cavity_edges[k];
            triangles.push(oriented(Triangle::new([a, b, i], &vertices), &vertices));
            k += 1;
        }
    }

    let mut result: Vec<[usize; 3]> = triangles
        .into_iter()
        .filter(|triangle| triangle.v.iter().all(|&v| v < n))
        .map(|triangle| triangle.v)
        .collect();
    // Deterministic order regardless of hash iteration order above.
    for triangle in &mut result {
        let smallest = (0..3).min_by_key(|&k| triangle[k]).unwrap_or(0);
        triangle.rotate_left(smallest);
    }
    result.sort_unstable();
    result
}

fn oriented(mut triangle: Triangle, points: &[P2]) -> Triangle {
    let (a, b, c) = (
        points[triangle.v[0]],
        points[triangle.v[1]],
        points[triangle.v[2]],
    );
    if (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0) < 0.0 {
        triangle.v.swap(1, 2);
    }
    triangle
}

pub(crate) fn distance(a: P2, b: P2) -> f64 {
    distance_sq(a, b).sqrt()
}

fn distance_sq(a: P2, b: P2) -> f64 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delaunay_covers_a_square_lattice_without_overlap() {
        let mut points = Vec::new();
        for y in 0..5 {
            for x in 0..5 {
                // Small deterministic jitter keeps the lattice out of cocircular ties.
                points.push((x as f64 + (y as f64) * 0.013, y as f64 + (x as f64) * 0.007));
            }
        }
        let triangles = delaunay(&points);
        let area: f64 = triangles
            .iter()
            .map(|t| {
                let (a, b, c) = (points[t[0]], points[t[1]], points[t[2]]);
                ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)) * 0.5
            })
            .sum();
        assert_eq!(triangles.len(), 32, "a 5x5 point grid has 32 triangles");
        assert!(
            (area - 16.0).abs() < 0.5,
            "triangles tile the convex hull exactly once (area {area})"
        );
    }

    #[test]
    fn resample_loop_spaces_points_evenly() {
        let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let points = resample_loop(&square, 2.5);
        assert_eq!(points.len(), 16);
        for i in 0..points.len() {
            let gap = distance(points[i], points[(i + 1) % points.len()]);
            assert!((gap - 2.5).abs() < 1.0e-9);
        }
    }
}
