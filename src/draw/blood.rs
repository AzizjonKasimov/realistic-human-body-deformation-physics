//! Blood: where wounds bleed from, pools on the floor, trails and drops
//! on the skin, and drops in the air.

use super::*;

pub(super) fn draw_wound_sources(ctx: &Scene) {
    for wound in ctx.world.wounds() {
        if !wound.active {
            continue;
        }
        let pressure = (wound.pressure / 6.0).clamp(0.0, 1.0) as f32;
        let clot = wound.clot.clamp(0.0, 1.0) as f32;
        let radius = (wound.radius * (1.5 + wound.depth * 0.38)) as f32;
        let pos = to_mq(wound.position);
        draw_soft_circle(
            pos,
            radius + 3.0 + pressure * 9.0,
            4,
            with_alpha(ctx.palette.blood_dark, 0.14 + pressure * 0.24),
        );
        draw_circle(
            pos.x,
            pos.y,
            radius,
            with_alpha(
                mix(ctx.palette.blood_mid, ctx.palette.blood_fresh, pressure),
                0.84 - clot * 0.28,
            ),
        );
        // Only a spurting, high-pressure bleed gets a streak.
        if pressure > 0.3 {
            let dir = normalized(wound.direction, rp::Vec2 { x: 0.0, y: 1.0 });
            draw_line_vec(
                wound.position,
                add(wound.position, scale(dir, 8.0 + wound.pressure * 3.2)),
                1.4,
                with_alpha(ctx.palette.blood_fresh, 0.30 + pressure * 0.40),
            );
        }
    }
}

/// Blood outside the body: pools on the floor and drops in the air.
pub fn draw_effects(ctx: &Scene) {
    draw_blood_stains(ctx);
    draw_fluids(ctx);
}

/// How long blood outside the body takes to darken as it dries: minutes for
/// a thin film.
pub(super) const BLOOD_DRYING_SECONDS: f64 = 240.0;

/// Pools of blood on the floor, seen from in front: as wide as the floor
/// they cover and flat, lying just in front of the foot of the wall. Fresh
/// blood is red and glossy; it dries dark.
pub(super) fn draw_blood_stains(ctx: &Scene) {
    for stain in ctx.world.blood_stains() {
        if stain.volume <= 0.0 {
            continue;
        }
        let half = (stain.width * 0.5) as f32;
        // A low view squashes the round pool into a flat ellipse.
        let deep = half * 0.22;
        let (x, y) = (
            stain.position.x as f32,
            ctx.world_floor_y + 1.5 + deep * 0.4,
        );
        let dry = (stain.age / BLOOD_DRYING_SECONDS).clamp(0.0, 1.0) as f32;
        let body = mix(rgba(118, 6, 16, 255), ctx.palette.blood_stain, dry);
        draw_ellipse(
            x,
            y,
            half,
            deep,
            0.0,
            with_alpha(ctx.palette.blood_dark, 0.62),
        );
        draw_ellipse(x, y, half * 0.9, deep * 0.85, 0.0, with_alpha(body, 0.88));
        draw_ellipse(
            x - half * 0.25,
            y - deep * 0.35,
            half * 0.35,
            deep * 0.22,
            0.0,
            rgba(255, 190, 190, (36.0 * (1.0 - dry)) as u8),
        );
    }
}

/// Blood on the skin: the trails it left running down, then the drops still
/// on it, all carried by the skin, each as wide as its blood wets the skin.
pub(super) fn draw_skin_blood(ctx: &Scene) {
    let world = ctx.world;
    let mut stretches = Vec::new();
    for trail in world.blood_trails() {
        let Some((from, to)) = world.blood_trail_ends(trail) else {
            continue;
        };
        // Wet blood is bright; it darkens to brown as it dries.
        let dry = (trail.age / BLOOD_DRYING_SECONDS).clamp(0.0, 1.0) as f32;
        let color = mix(rgba(146, 10, 20, 235), rgba(86, 20, 17, 225), dry);
        stretches.push((from, to, trail.width as f32, color));
    }
    let edge = with_alpha(ctx.palette.blood_dark, 0.22);
    for &(from, to, width, _) in &stretches {
        draw_line_vec(from, to, width, edge);
    }
    for &(from, to, width, color) in &stretches {
        draw_line_vec(from, to, width * 0.75, color);
        draw_circle(to.x as f32, to.y as f32, width * 0.375, color);
    }
    for fluid in world.fluids() {
        let Some(spot) = fluid.on_skin else {
            continue;
        };
        if fluid.life <= 0.0 {
            continue;
        }
        let Some(center) = world.skin_spot_position(spot) else {
            continue;
        };
        // The head of a trickle, joined to the trail behind it, with a dark
        // rim and a glint.
        let radius = world.drop_radius(fluid) as f32;
        let body = rgba(156, 12, 22, 245);
        let behind = fluid
            .trail_from
            .and_then(|from| world.skin_spot_position(from));
        if let Some(behind) = behind {
            // The trail is most of the bead's width (see `blood.rs`).
            let width = radius * 1.2;
            draw_line_vec(behind, center, width, edge);
            draw_line_vec(behind, center, width * 0.75, body);
        }
        let (x, y) = (center.x as f32, center.y as f32);
        draw_circle(x, y, radius, with_alpha(ctx.palette.blood_dark, 0.5));
        draw_circle(x, y, radius * 0.82, body);
        draw_circle(
            x - radius * 0.3,
            y - radius * 0.3,
            radius * 0.25,
            rgba(255, 200, 200, 70),
        );
    }
}

/// Drops of blood in the air, each the sphere its blood makes. A fast drop
/// smears along its path within a frame, as it would on camera.
pub(super) fn draw_fluids(ctx: &Scene) {
    let world = ctx.world;
    for fluid in world.fluids() {
        if fluid.life <= 0.0 || fluid.on_skin.is_some() {
            continue;
        }
        let radius = world.drop_radius(fluid) as f32;
        let travel = sub(fluid.position, fluid.previous);
        let smear = (length(travel) / 18.0).clamp(0.0, 1.0) as f32;
        let color = Color::new(0.22 + 0.54 * fluid.intensity as f32, 0.055, 0.085, 0.92);
        if smear > 0.08 {
            draw_line_vec(
                fluid.previous,
                fluid.position,
                radius * 2.0,
                with_alpha(color, 0.25 + smear * 0.35),
            );
        }
        draw_circle(
            fluid.position.x as f32,
            fluid.position.y as f32,
            radius,
            color,
        );
    }
}
