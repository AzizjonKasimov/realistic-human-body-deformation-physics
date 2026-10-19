//! Vector arithmetic on the world's positions and the drawing primitives
//! built on it.

use super::*;

pub(super) fn point_along(a: rp::Vec2, b: rp::Vec2, t: f64) -> rp::Vec2 {
    add(a, scale(sub(b, a), t.clamp(0.0, 1.0)))
}

/// Perpendicular to `dir`, rotated a quarter turn.
pub(super) fn perpendicular(dir: rp::Vec2) -> rp::Vec2 {
    rp::Vec2 {
        x: -dir.y,
        y: dir.x,
    }
}

pub(super) fn draw_polygon(points: &[rp::Vec2], color: Color) {
    for i in 1..points.len().saturating_sub(1) {
        draw_triangle(
            to_mq(points[0]),
            to_mq(points[i]),
            to_mq(points[i + 1]),
            color,
        );
    }
}

pub(super) fn draw_quad(points: [rp::Vec2; 4], color: Color) {
    draw_triangle(to_mq(points[0]), to_mq(points[1]), to_mq(points[2]), color);
    draw_triangle(to_mq(points[0]), to_mq(points[2]), to_mq(points[3]), color);
}

pub(super) fn draw_polyline_closed(points: &[rp::Vec2], width: f32, color: Color) {
    if points.len() < 2 {
        return;
    }
    for i in 0..points.len() {
        draw_line_vec(points[i], points[(i + 1) % points.len()], width, color);
    }
}

pub(super) fn draw_soft_circle(center: Vec2, radius: f32, rings: usize, color: Color) {
    for i in (0..rings).rev() {
        let t = (i + 1) as f32 / rings as f32;
        let mut ring = color;
        ring.a *= (1.0 - t * 0.72).max(0.08);
        draw_circle(center.x, center.y, radius * t, ring);
    }
}

pub fn draw_line_vec(a: rp::Vec2, b: rp::Vec2, width: f32, color: Color) {
    draw_line(a.x as f32, a.y as f32, b.x as f32, b.y as f32, width, color);
}

pub fn bone_point(bone: rp::BoneSegment, t: f64) -> rp::Vec2 {
    rp::Vec2 {
        x: bone.a.x + (bone.b.x - bone.a.x) * t,
        y: bone.a.y + (bone.b.y - bone.a.y) * t,
    }
}

pub fn normalized(value: rp::Vec2, fallback: rp::Vec2) -> rp::Vec2 {
    let len = length(value);
    if len <= 0.0001 {
        fallback
    } else {
        scale(value, 1.0 / len)
    }
}

pub fn length(value: rp::Vec2) -> f64 {
    (value.x * value.x + value.y * value.y).sqrt()
}

pub fn add(a: rp::Vec2, b: rp::Vec2) -> rp::Vec2 {
    rp::Vec2 {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}

pub fn sub(a: rp::Vec2, b: rp::Vec2) -> rp::Vec2 {
    rp::Vec2 {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

pub fn scale(value: rp::Vec2, amount: f64) -> rp::Vec2 {
    rp::Vec2 {
        x: value.x * amount,
        y: value.y * amount,
    }
}

pub fn mid(a: rp::Vec2, b: rp::Vec2) -> rp::Vec2 {
    scale(add(a, b), 0.5)
}

pub fn to_mq(value: rp::Vec2) -> Vec2 {
    vec2(value.x as f32, value.y as f32)
}
