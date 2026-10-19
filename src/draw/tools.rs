//! The tool in hand, drawn from the pose and geometry the simulation
//! collides with.

use super::*;

/// The tool in hand, if any, unless the scene leaves it out.
pub fn draw_tool(ctx: &Scene) {
    // The same pose and geometry the simulation collides with.
    if let Some(pose) = ctx.world.current_tool_pose().filter(|_| ctx.show_tool) {
        let geometry = rp::tool_geometry(pose.tool);
        match pose.tool {
            rp::ToolMode::Sharp => draw_knife(ctx, &pose, &geometry),
            rp::ToolMode::Heavy => draw_sledgehammer(ctx, &pose, &geometry),
            rp::ToolMode::Blunt => draw_bat(ctx, &pose, &geometry),
            rp::ToolMode::Fist => draw_fist(ctx, &pose, &geometry),
        }
    }
}

/// A fist in the figure's own colours, the row of its knuckles leading, and
/// the forearm behind it, as another fighter's would reach in. Its knuckles
/// lie along the front of the same shape the simulation strikes with.
pub(super) fn draw_fist(ctx: &Scene, pose: &rp::ToolPose, geometry: &rp::ToolGeometry) {
    let heading = pose.heading;
    let across = perpendicular(heading);
    let palette = &ctx.palette;
    let half = geometry.body_half_width;
    let radius = pose.contact_radius;
    // The knuckles' face, the back of the fist where the wrist joins it, and
    // the forearm behind, narrowing a little toward the hand.
    let face = add(pose.center, scale(heading, radius));
    let wrist = sub(pose.center, scale(heading, radius + 8.0));
    let elbow = sub(wrist, scale(heading, geometry.handle_length));
    let forearm = [
        add(wrist, scale(across, geometry.handle_half_width)),
        add(elbow, scale(across, geometry.handle_half_width * 1.25)),
        sub(elbow, scale(across, geometry.handle_half_width * 1.25)),
        sub(wrist, scale(across, geometry.handle_half_width)),
    ];
    draw_polygon(&forearm, palette.skin_base);
    draw_polyline_closed(&forearm, 1.2, palette.skin_outline);
    let rounding = radius * 0.6;
    let front = sub(face, scale(heading, rounding));
    let fist = [
        add(wrist, scale(across, half * 0.85)),
        add(front, scale(across, half)),
        add(face, scale(across, half - rounding)),
        sub(face, scale(across, half - rounding)),
        sub(front, scale(across, half)),
        sub(wrist, scale(across, half * 0.85)),
    ];
    draw_polygon(&fist, palette.skin_mid);
    draw_polyline_closed(&fist, 1.2, palette.skin_outline);
    // The four knuckles along the face, and the line where the fingers fold
    // under.
    for share in [-0.6, -0.2, 0.2, 0.6] {
        let knuckle = add(sub(face, scale(heading, 2.5)), scale(across, half * share));
        draw_circle(knuckle.x as f32, knuckle.y as f32, 2.4, palette.skin_light);
    }
    let fold = sub(face, scale(heading, radius * 0.9));
    draw_line_vec(
        add(fold, scale(across, half * 0.8)),
        sub(fold, scale(across, half * 0.8)),
        1.2,
        palette.skin_outline,
    );
    if ctx.tool_blood > 0.02 {
        draw_polygon(
            &[
                add(face, scale(across, half - rounding)),
                sub(face, scale(across, half - rounding)),
                sub(front, scale(across, half)),
                add(front, scale(across, half)),
            ],
            with_alpha(palette.blood_mid, ctx.tool_blood * 0.6),
        );
    }
}

/// Double-edged blade with a center ridge, crossguard, wrapped grip, and pommel.
pub(super) fn draw_knife(ctx: &Scene, pose: &rp::ToolPose, geometry: &rp::ToolGeometry) {
    let heading = pose.heading;
    let across = perpendicular(heading);
    let guard = pose.contact_start;
    let tip = pose.contact_end;
    let length = length(sub(tip, guard));
    let width = geometry.body_half_width;
    let shoulder = add(guard, scale(heading, length * 0.58));

    let blade = [
        add(guard, scale(across, width * 0.85)),
        add(shoulder, scale(across, width)),
        tip,
        sub(shoulder, scale(across, width)),
        sub(guard, scale(across, width * 0.85)),
    ];
    let steel = if ctx.pressing {
        rgba(214, 222, 226, 255)
    } else {
        rgba(186, 196, 201, 255)
    };
    draw_polygon(&blade, steel);
    // The far bevel catches less light than the near one.
    draw_polygon(
        &[
            guard,
            shoulder,
            tip,
            sub(shoulder, scale(across, width)),
            sub(guard, scale(across, width * 0.85)),
        ],
        rgba(146, 157, 164, 255),
    );
    if ctx.tool_blood > 0.02 {
        let bloodied = add(guard, scale(heading, length * 0.35));
        draw_polygon(
            &[
                add(bloodied, scale(across, width * 0.92)),
                add(shoulder, scale(across, width)),
                tip,
                sub(shoulder, scale(across, width)),
                sub(bloodied, scale(across, width * 0.92)),
            ],
            with_alpha(ctx.palette.blood_mid, ctx.tool_blood * 0.8),
        );
    }
    draw_line_vec(guard, tip, 1.2, rgba(236, 242, 244, 220));
    draw_polyline_closed(&blade, 1.2, rgba(52, 58, 62, 255));

    let guard_half = width + 5.0;
    draw_line_vec(
        add(guard, scale(across, guard_half)),
        sub(guard, scale(across, guard_half)),
        5.0,
        rgba(64, 66, 70, 255),
    );
    draw_line_vec(
        add(guard, scale(across, guard_half - 1.0)),
        sub(guard, scale(across, guard_half - 1.0)),
        1.4,
        rgba(150, 154, 158, 255),
    );

    let grip_start = sub(guard, scale(heading, 2.5));
    let grip_end = sub(guard, scale(heading, 2.5 + geometry.handle_length));
    draw_line_vec(
        grip_start,
        grip_end,
        (geometry.handle_half_width * 2.0) as f32,
        rgba(44, 32, 26, 255),
    );
    let handle_half = geometry.handle_half_width;
    for i in 1..6 {
        let at = sub(
            grip_start,
            scale(heading, geometry.handle_length * i as f64 / 6.0),
        );
        draw_line_vec(
            add(at, scale(across, handle_half)),
            sub(at, scale(across, handle_half)),
            1.0,
            rgba(92, 70, 54, 255),
        );
    }
    draw_circle(
        grip_end.x as f32,
        grip_end.y as f32,
        (handle_half + 1.5) as f32,
        rgba(118, 122, 126, 255),
    );
}

/// Steel head with polished striking faces on a long wooden handle.
pub(super) fn draw_sledgehammer(ctx: &Scene, pose: &rp::ToolPose, geometry: &rp::ToolGeometry) {
    let heading = pose.heading;
    let side = pose.side;
    let center = pose.center;
    let half_length = length(sub(pose.contact_end, pose.contact_start)) * 0.5 + pose.contact_radius;
    let half_width = geometry.body_half_width;

    // Handle first, so the head covers the joint.
    let handle_start = add(center, scale(side, half_width * 0.6));
    let handle_end = add(center, scale(side, half_width + geometry.handle_length));
    let handle_width = (geometry.handle_half_width * 2.0) as f32;
    draw_line_vec(
        handle_start,
        handle_end,
        handle_width + 2.0,
        rgba(58, 40, 26, 255),
    );
    draw_line_vec(
        handle_start,
        handle_end,
        handle_width,
        rgba(156, 112, 64, 255),
    );
    draw_line_vec(
        add(handle_start, scale(heading, -1.5)),
        add(handle_end, scale(heading, -1.5)),
        1.2,
        rgba(196, 150, 96, 200),
    );
    let wrap_start = add(
        center,
        scale(side, half_width + geometry.handle_length * 0.72),
    );
    draw_line_vec(
        wrap_start,
        handle_end,
        handle_width + 1.0,
        rgba(44, 36, 30, 255),
    );

    let corner = |along: f64, out: f64| add(center, add(scale(heading, along), scale(side, out)));
    let head = [
        corner(half_length, -half_width),
        corner(half_length, half_width),
        corner(-half_length, half_width),
        corner(-half_length, -half_width),
    ];
    draw_quad(head, rgba(74, 78, 83, 255));
    // Light falls on the side away from the handle.
    draw_quad(
        [
            corner(half_length - 2.0, -half_width + 2.0),
            corner(half_length - 2.0, -half_width * 0.15),
            corner(-half_length + 2.0, -half_width * 0.15),
            corner(-half_length + 2.0, -half_width + 2.0),
        ],
        rgba(118, 124, 130, 255),
    );
    for face in [half_length, -half_length] {
        let inset = if face > 0.0 { -4.0 } else { 4.0 };
        draw_quad(
            [
                corner(face, -half_width),
                corner(face, half_width),
                corner(face + inset, half_width),
                corner(face + inset, -half_width),
            ],
            rgba(168, 174, 178, 255),
        );
    }
    if ctx.tool_blood > 0.02 {
        // Blood collects on the striking face that leads.
        let face = half_length;
        draw_quad(
            [
                corner(face, -half_width),
                corner(face, half_width),
                corner(face - 12.0, half_width),
                corner(face - 12.0, -half_width),
            ],
            with_alpha(ctx.palette.blood_mid, ctx.tool_blood * 0.75),
        );
    }
    draw_polyline_closed(&head, 2.0, rgba(22, 23, 25, 255));
}

/// Ash bat: full barrel where it hits, tapering to a taped handle and knob.
pub(super) fn draw_bat(ctx: &Scene, pose: &rp::ToolPose, geometry: &rp::ToolGeometry) {
    let side = pose.side;
    let center = pose.center;
    let radius = geometry.body_half_width;
    let along = |distance: f64| add(center, scale(side, distance));
    // Positions along the bat from the driven point toward the hand.
    let end = -(geometry.contact_front + radius * 0.5);
    let barrel_end = geometry.contact_back + 2.0;
    let taper_end = barrel_end + 62.0;
    let knob = barrel_end + geometry.handle_length - 30.0;
    let handle_radius = geometry.handle_half_width;

    let wood = rgba(212, 176, 122, 255);
    let wood_dark = rgba(96, 70, 44, 255);
    let across = perpendicular(side);
    let outline = [
        add(along(end), scale(across, radius)),
        add(along(barrel_end), scale(across, radius)),
        add(along(taper_end), scale(across, handle_radius)),
        add(along(knob), scale(across, handle_radius)),
        sub(along(knob), scale(across, handle_radius)),
        sub(along(taper_end), scale(across, handle_radius)),
        sub(along(barrel_end), scale(across, radius)),
        sub(along(end), scale(across, radius)),
    ];
    draw_polygon(&outline, wood);
    let cap = along(end);
    draw_circle(cap.x as f32, cap.y as f32, radius as f32, wood);
    draw_polyline_closed(&outline[..], 1.4, wood_dark);
    draw_circle_lines(cap.x as f32, cap.y as f32, radius as f32, 1.4, wood_dark);
    draw_line_vec(
        add(along(end), scale(across, radius * 0.45)),
        add(along(barrel_end), scale(across, radius * 0.45)),
        2.0,
        rgba(236, 210, 164, 200),
    );
    draw_line_vec(
        sub(along(end + 6.0), scale(across, radius * 0.4)),
        sub(along(barrel_end - 8.0), scale(across, radius * 0.4)),
        1.0,
        rgba(170, 132, 84, 200),
    );
    if ctx.tool_blood > 0.02 {
        draw_polygon(
            &[
                add(along(end), scale(across, radius)),
                add(along(end + 52.0), scale(across, radius)),
                sub(along(end + 52.0), scale(across, radius)),
                sub(along(end), scale(across, radius)),
            ],
            with_alpha(ctx.palette.blood_mid, ctx.tool_blood * 0.7),
        );
    }

    let tape_start = taper_end + 4.0;
    draw_line_vec(
        along(tape_start),
        along(knob),
        (handle_radius * 2.0 + 1.0) as f32,
        rgba(38, 38, 42, 255),
    );
    let mut wrap = tape_start + 5.0;
    while wrap < knob - 2.0 {
        draw_line_vec(
            add(along(wrap), scale(across, handle_radius)),
            sub(along(wrap + 3.0), scale(across, handle_radius)),
            1.0,
            rgba(84, 84, 90, 255),
        );
        wrap += 6.0;
    }
    let knob_center = along(knob + 2.0);
    draw_circle(
        knob_center.x as f32,
        knob_center.y as f32,
        (handle_radius + 3.0) as f32,
        wood,
    );
    draw_circle_lines(
        knob_center.x as f32,
        knob_center.y as f32,
        (handle_radius + 3.0) as f32,
        1.4,
        wood_dark,
    );
}
