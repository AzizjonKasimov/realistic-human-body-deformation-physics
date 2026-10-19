//! A tissue triangle drawn as the piece of it that holds flesh, a cut's
//! piece included, and the colours its corners carry.

use super::*;

/// The point whose flesh a mesh point shows: a cut's ghost carries a piece's
/// shape but holds no flesh, so it shows the flesh of the point it copies.
pub(super) fn shown_point(points: &[rp::Point], index: usize) -> &rp::Point {
    let point = &points[index];
    if point.is_ghost() {
        &points[point.ghost_of]
    } else {
        point
    }
}

/// The corners of the flesh `triangle` holds, where they are now, with their
/// weights on the triangle's corners: the whole triangle, or a piece's side
/// of a knife's cut through it.
pub(super) fn piece_corners(
    world: &rp::World,
    triangle: &rp::Triangle,
) -> Vec<(rp::Vec2, [f64; 3])> {
    let points = world.points();
    let [a, b, c] = [triangle.a, triangle.b, triangle.c].map(|index| points[index].position);
    triangle
        .piece()
        .corners()
        .iter()
        .map(|&weights| {
            let at = rp::Vec2 {
                x: a.x * weights[0] + b.x * weights[1] + c.x * weights[2],
                y: a.y * weights[0] + b.y * weights[1] + c.y * weights[2],
            };
            (at, weights)
        })
        .collect()
}

/// Adds the flesh `triangle` holds to `mesh`, its color blended from
/// `colors` at the triangle's corners.
pub(super) fn push_piece(
    mesh: &mut Mesh,
    world: &rp::World,
    triangle: &rp::Triangle,
    colors: [Color; 3],
) {
    let corners = piece_corners(world, triangle);
    let base = mesh.vertices.len() as u16;
    for &(at, weights) in &corners {
        let blend = |channel: fn(&Color) -> f32| {
            (0..3)
                .map(|corner| channel(&colors[corner]) * weights[corner] as f32)
                .sum::<f32>()
        };
        let color = Color::new(
            blend(|color| color.r),
            blend(|color| color.g),
            blend(|color| color.b),
            blend(|color| color.a),
        );
        mesh.vertices
            .push(Vertex::new(at.x as f32, at.y as f32, 0.0, 0.0, 0.0, color));
    }
    for corner in 1..corners.len().saturating_sub(1) as u16 {
        mesh.indices
            .extend_from_slice(&[base, base + corner, base + corner + 1]);
    }
}

/// Fills the flesh `triangle` holds with one color.
pub(super) fn fill_piece(world: &rp::World, triangle: &rp::Triangle, color: Color) {
    let corners = piece_corners(world, triangle);
    for corner in 1..corners.len().saturating_sub(1) {
        draw_triangle(
            to_mq(corners[0].0),
            to_mq(corners[corner].0),
            to_mq(corners[corner + 1].0),
            color,
        );
    }
}

/// Outlines the flesh `triangle` holds.
pub(super) fn outline_piece(world: &rp::World, triangle: &rp::Triangle, color: Color, width: f32) {
    let corners = piece_corners(world, triangle);
    for (index, &(at, _)) in corners.iter().enumerate() {
        draw_line_vec(at, corners[(index + 1) % corners.len()].0, width, color);
    }
}

pub(super) fn triangle_point_metrics(world: &rp::World, triangle: &rp::Triangle) -> (f64, f64) {
    let a = world.points()[triangle.a];
    let b = world.points()[triangle.b];
    let c = world.points()[triangle.c];
    (
        (a.load + b.load + c.load) / 3.0,
        (a.exposure + b.exposure + c.exposure) / 3.0,
    )
}

pub(super) fn triangle_point_contusion(world: &rp::World, triangle: &rp::Triangle) -> f64 {
    let a = world.points()[triangle.a];
    let b = world.points()[triangle.b];
    let c = world.points()[triangle.c];
    (a.contusion + b.contusion + c.contusion) / 3.0
}
