//! Bones, their breaks and attachments, and the major vessels.

use super::*;

pub(super) fn draw_major_vessels(ctx: &Scene) {
    for vessel in ctx.world.vessels() {
        if ctx.anatomy {
            let opacity = if vessel.lacerated { 0.82 } else { 0.34 };
            draw_vessel_run(ctx, vessel.a, vessel.b, vessel.radius, opacity);
            if vessel.lacerated {
                draw_soft_circle(
                    to_mq(point_along(vessel.a, vessel.b, vessel.laceration_t)),
                    (vessel.radius * 3.8 + 7.0) as f32,
                    4,
                    with_alpha(ctx.palette.blood_fresh, 0.18),
                );
            }
        } else if vessel.lacerated
            && ctx
                .world
                .open_to_air(point_along(vessel.a, vessel.b, vessel.laceration_t))
        {
            // A vessel torn under unbroken skin bleeds into the flesh unseen.
            draw_severed_vessel(ctx, vessel);
        }
    }
}

pub(super) fn draw_vessel_run(ctx: &Scene, a: rp::Vec2, b: rp::Vec2, radius: f64, opacity: f32) {
    draw_line_vec(
        a,
        b,
        (radius * 2.5 + 2.0) as f32,
        with_alpha(ctx.palette.major_vessel_shadow, opacity * 0.64),
    );
    draw_line_vec(
        a,
        b,
        (radius * 1.35 + 0.8) as f32,
        with_alpha(ctx.palette.major_vessel, opacity),
    );
}

/// With the skin on, only the cut is visible: two retracted vessel ends in a
/// pool of fresh blood, not the whole artery.
pub(super) fn draw_severed_vessel(ctx: &Scene, vessel: &rp::VesselSegment) {
    let cut = point_along(vessel.a, vessel.b, vessel.laceration_t);
    let along = normalized(sub(vessel.b, vessel.a), rp::Vec2 { x: 0.0, y: 1.0 });
    let gap = vessel.radius * 1.4 + 1.5;
    let stump = vessel.radius * 2.2 + 4.0;
    draw_soft_circle(
        to_mq(cut),
        (vessel.radius * 3.4 + 6.0) as f32,
        4,
        with_alpha(ctx.palette.blood_fresh, 0.22),
    );
    for direction in [1.0, -1.0] {
        let near = add(cut, scale(along, gap * direction));
        let far = add(cut, scale(along, (gap + stump) * direction));
        draw_vessel_run(ctx, near, far, vessel.radius * 0.8, 0.75);
        draw_circle(
            near.x as f32,
            near.y as f32,
            (vessel.radius * 0.75 + 0.6) as f32,
            ctx.palette.wound_core,
        );
    }
}

#[derive(Clone, Copy)]
pub(super) enum BonePass {
    Anatomy,
    /// Beneath the flesh in the normal view.
    Buried,
}

pub(super) fn draw_bones(ctx: &Scene, pass: BonePass) {
    for bone in ctx.world.bones() {
        match pass {
            BonePass::Anatomy => draw_bone(ctx, bone, 1.0, true),
            BonePass::Buried => {
                // The spine runs down the back, behind everything a front
                // wound can open.
                if bone.kind == rp::BoneKind::Spine {
                    continue;
                }
                let broken =
                    bone.fractured || bone.splinter || bone.broken_start || bone.broken_end;
                draw_bone(ctx, bone, 1.0, broken);
            }
        }
    }
}

pub(super) fn draw_bone(ctx: &Scene, bone: &rp::BoneSegment, alpha: f32, details: bool) {
    let width = (bone.radius * if details { 1.85 } else { 1.45 }).max(2.5) as f32;
    let shadow = with_alpha(ctx.palette.bone_shadow, alpha * 0.70);
    draw_line_vec(bone.a, bone.b, width + 4.0, shadow);
    let stroke = if bone.fractured || bone.splinter {
        ctx.palette.bone_fractured
    } else if bone.kind == rp::BoneKind::Rib {
        rgba(232, 207, 156, 255)
    } else {
        ctx.palette.bone
    };
    draw_line_vec(bone.a, bone.b, width, with_alpha(stroke, alpha));
    if details {
        let center = mid(bone.a, bone.b);
        let dir = normalized(sub(bone.b, bone.a), rp::Vec2 { x: 1.0, y: 0.0 });
        let normal = rp::Vec2 {
            x: -dir.y,
            y: dir.x,
        };
        draw_line_vec(
            add(center, scale(normal, -bone.radius * 0.45)),
            add(center, scale(normal, bone.radius * 0.45)),
            1.4,
            with_alpha(rgba(255, 250, 232, 255), alpha * 0.35),
        );
    }
    if details && bone.broken_start {
        draw_fracture_cap(ctx, bone, true);
    }
    if details && bone.broken_end {
        draw_fracture_cap(ctx, bone, false);
    }
}

pub(super) fn draw_fracture_cap(ctx: &Scene, bone: &rp::BoneSegment, at_start: bool) {
    let p = if at_start { bone.a } else { bone.b };
    let dir = normalized(sub(bone.b, bone.a), rp::Vec2 { x: 1.0, y: 0.0 });
    let fallback = rp::Vec2 {
        x: -dir.y,
        y: dir.x,
    };
    let stored = if at_start {
        bone.broken_start_normal
    } else {
        bone.broken_end_normal
    };
    let normal = normalized(stored, fallback);
    let tip_dir = if at_start { scale(dir, -1.0) } else { dir };
    let cap = bone.radius * 1.28;

    draw_soft_circle(
        to_mq(p),
        (bone.radius * 1.15) as f32,
        3,
        with_alpha(ctx.palette.wound_core, 0.32),
    );
    draw_line_vec(
        add(p, scale(normal, -cap)),
        add(
            add(p, scale(normal, -cap * 0.22)),
            scale(tip_dir, bone.radius * 0.55),
        ),
        3.0,
        ctx.palette.bone_fractured,
    );
    draw_line_vec(
        add(
            add(p, scale(normal, -cap * 0.20)),
            scale(tip_dir, bone.radius * 0.48),
        ),
        add(
            add(p, scale(normal, cap * 0.24)),
            scale(tip_dir, -bone.radius * 0.15),
        ),
        3.0,
        ctx.palette.bone_fractured,
    );
    draw_line_vec(
        add(
            add(p, scale(normal, cap * 0.24)),
            scale(tip_dir, -bone.radius * 0.15),
        ),
        add(p, scale(normal, cap)),
        3.0,
        ctx.palette.bone_fractured,
    );
    draw_line_vec(
        add(p, scale(normal, -cap * 0.66)),
        add(p, scale(tip_dir, bone.radius * 0.88)),
        2.4,
        ctx.palette.blood_dark,
    );
    draw_line_vec(
        add(p, scale(normal, 0.12 * cap)),
        add(
            add(p, scale(tip_dir, bone.radius * 0.95)),
            scale(normal, cap * 0.38),
        ),
        2.2,
        ctx.palette.blood_fresh,
    );
    draw_line_vec(
        add(p, scale(normal, cap * 0.64)),
        add(p, scale(tip_dir, bone.radius * 0.55)),
        2.2,
        ctx.palette.blood_dark,
    );
}

pub(super) fn draw_bone_attachments(ctx: &Scene) {
    let world = ctx.world;
    for attachment in world.bone_attachments() {
        if attachment.broken
            || attachment.bone >= world.bones().len()
            || attachment.point >= world.points().len()
        {
            continue;
        }
        let anchor = bone_point(world.bones()[attachment.bone], attachment.t);
        let point = world.points()[attachment.point].position;
        draw_line_vec(anchor, point, 1.0, ctx.palette.attachment);
    }
}
