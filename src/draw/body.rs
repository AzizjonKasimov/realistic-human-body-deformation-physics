//! The bodies' flesh, layer by layer: crushed flesh, muscle, cuts, and
//! skin with its shading, wounds, and outline.

use super::*;

/// Every body's flesh, bones, and wounds, and the blood on its skin.
pub fn draw_body_layers(ctx: &Scene) {
    let world = ctx.world;

    if ctx.anatomy {
        draw_muscle_layer(ctx);
        draw_crushed_flesh(ctx);
        draw_cut_insides(ctx);
        draw_major_vessels(ctx);
        draw_skin_layer(ctx);
        draw_bone_attachments(ctx);
        draw_bones(ctx, BonePass::Anatomy);
    } else {
        // Inside out: crushed flesh where the sheets are torn away, the
        // bones, the muscle, then the skin. Bone shows only through openings
        // in the flesh, or where a broken end sticks out of the body.
        draw_crushed_flesh(ctx);
        draw_cut_insides(ctx);
        draw_bones(ctx, BonePass::Buried);
        draw_muscle_layer(ctx);
        draw_cut_lips(ctx, rp::TissueLayer::Muscle);
        draw_skin_layer(ctx);
        draw_closed_cut_skin(ctx);
    }
    draw_cut_lips(ctx, rp::TissueLayer::Skin);

    draw_skin_wounds(ctx);
    draw_skin_blood(ctx);
    if !ctx.anatomy {
        draw_major_vessels(ctx);
    }
    // Where each wound bleeds from is the simulation's bookkeeping, not
    // something an onlooker sees: its blood shows only once it is out.
    if ctx.debug {
        draw_wound_sources(ctx);
    }

    // Where the tool presses hardest; a debugging aid, so only with the panel.
    let debug = world.debug();
    if ctx.debug && debug.max_depth > 0.0 {
        draw_soft_circle(
            to_mq(debug.strongest_contact),
            13.0,
            4,
            with_alpha(ctx.palette.tool_accent, 0.18),
        );
        draw_circle_lines(
            debug.strongest_contact.x as f32,
            debug.strongest_contact.y as f32,
            7.0,
            1.4,
            with_alpha(ctx.palette.tool_accent, 0.68),
        );
    }
}

pub(super) fn draw_muscle_layer(ctx: &Scene) {
    if !ctx.anatomy {
        draw_muscle_flesh(ctx);
        return;
    }
    let world = ctx.world;
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Muscle || !world.triangle_alive(triangle) {
            continue;
        }

        let (load, exposure) = triangle_point_metrics(world, triangle);
        let contusion = triangle_point_contusion(world, triangle);
        let visible = ctx.anatomy
            || exposure > 0.035
            || triangle.damage > 0.015
            || load > 140.0
            || contusion > 0.08;
        if !visible {
            continue;
        }

        let heat = ((load / 900.0) + triangle.damage * 0.85 + exposure * 0.35).clamp(0.0, 1.0);
        let mut fill = mix(ctx.palette.muscle_base, ctx.palette.muscle_hot, heat as f32);
        fill = mix(
            fill,
            ctx.palette.muscle_contusion,
            (contusion * 0.48).clamp(0.0, 0.62) as f32,
        );
        fill.a = if ctx.anatomy {
            (0.54 + heat as f32 * 0.28 + exposure as f32 * 0.10).min(0.88)
        } else {
            (0.20
                + exposure as f32 * 0.58
                + triangle.damage as f32 * 0.28
                + contusion as f32 * 0.08)
                .clamp(0.18, 0.86)
        };
        let mut shadow = ctx.palette.muscle_shadow;
        shadow.a = if ctx.anatomy {
            0.16 + heat as f32 * 0.10
        } else {
            (0.08 + exposure as f32 * 0.18).min(0.24)
        };
        fill_piece(world, triangle, shadow);
        fill_piece(world, triangle, fill);
    }
}

/// Muscle as solid flesh in one mesh. The skin covers it, so it shows only
/// through openings in the skin, as red flesh rather than a faint wash.
pub(super) fn draw_muscle_flesh(ctx: &Scene) {
    let world = ctx.world;
    let points = world.points();
    let color_of = |index: usize| {
        let point = shown_point(points, index);
        let heat = (point.load / 900.0 + point.exposure * 0.35).clamp(0.0, 1.0) as f32;
        let mut color = mix(ctx.palette.muscle_base, ctx.palette.muscle_hot, heat);
        color = mix(
            color,
            ctx.palette.muscle_contusion,
            (point.contusion * 0.48).clamp(0.0, 0.62) as f32,
        );
        color.a = 0.94;
        color
    };
    let mut vertex_of = vec![u16::MAX; points.len()];
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: None,
    };
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Muscle || !world.triangle_alive(triangle) {
            continue;
        }
        if triangle.cut.is_some() {
            let colors = [triangle.a, triangle.b, triangle.c].map(color_of);
            push_piece(&mut mesh, world, triangle, colors);
            continue;
        }
        for index in [triangle.a, triangle.b, triangle.c] {
            if vertex_of[index] == u16::MAX {
                let point = &points[index];
                vertex_of[index] = mesh.vertices.len() as u16;
                mesh.vertices.push(Vertex::new(
                    point.position.x as f32,
                    point.position.y as f32,
                    0.0,
                    0.0,
                    0.0,
                    color_of(index),
                ));
            }
            mesh.indices.push(vertex_of[index]);
        }
    }
    draw_mesh(&mesh);
}

/// The inside of a cut through muscle: the gap between its two lips, as deep
/// raw flesh. It lies beneath the bones and muscle, so it shows only where
/// the cut has opened.
pub(super) fn draw_cut_insides(ctx: &Scene) {
    let color = with_alpha(
        mix(ctx.palette.wound_core, ctx.palette.wound_edge, 0.32),
        0.96,
    );
    for cut in ctx.world.opened_cuts() {
        if cut.layer != rp::TissueLayer::Muscle {
            continue;
        }
        // The two lips' ends correspond in order.
        let [(a, b), (twin_a, twin_b)] = cut.lips;
        let corners = [a, b, twin_b, twin_a].map(to_mq);
        draw_triangle(corners[0], corners[1], corners[2], color);
        draw_triangle(corners[0], corners[2], corners[3], color);
    }
}

/// A thin dark line along each lip of the open cuts through `layer`.
pub(super) fn draw_cut_lips(ctx: &Scene, layer: rp::TissueLayer) {
    let (width, color) = match layer {
        rp::TissueLayer::Skin => (
            1.3,
            with_alpha(
                mix(ctx.palette.wound_edge, ctx.palette.wound_shadow, 0.45),
                0.86,
            ),
        ),
        rp::TissueLayer::Muscle => (1.0, with_alpha(ctx.palette.wound_shadow, 0.55)),
    };
    for cut in ctx.world.opened_cuts() {
        if cut.layer != layer {
            continue;
        }
        for (a, b) in cut.lips {
            draw_line_vec(a, b, width, color);
        }
    }
}

/// How far torn-away flesh may be stretched, as a share of its rest length,
/// before its pulp starts to fade, and by when it is gone. A smashed limb's
/// pulp is pulled to about three times its length and must still cover it; a
/// flap torn loose leaves its tissue stretched much further.
pub(super) const PULP_FADE_FROM: f32 = 3.0;
pub(super) const PULP_FADE_TO: f32 = 5.0;

/// Flesh torn away from the sheets, crushed or torn through, as the pulped,
/// bloodied flesh it is. The sheets are the body seen from the front, and a
/// real body keeps its depth under a wound: a crushed arm shows pulped
/// muscle and broken bone, never a hole through to what lies behind it. It
/// lies beneath the bones and the living muscle and skin, so it shows only
/// where they have gone, and it fades where tissue is pulled far apart, as
/// across a flap torn loose, where no flesh is left between.
pub(super) fn draw_crushed_flesh(ctx: &Scene) {
    let world = ctx.world;
    let points = world.points();
    let springs = world.springs();
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: None,
    };
    for triangle in world.triangles() {
        if world.triangle_alive(triangle) || mesh.vertices.len() + 4 > u16::MAX as usize {
            continue;
        }
        let corners = [triangle.a, triangle.b, triangle.c];
        let edges = [triangle.edge_ab, triangle.edge_bc, triangle.edge_ca];
        let mut stretch: f64 = 0.0;
        for (side, &edge) in edges.iter().enumerate() {
            let (a, b) = (corners[side], corners[(side + 1) % 3]);
            let rest = match springs.get(edge) {
                Some(spring) => spring.rest,
                None => length(sub(points[b].home, points[a].home)),
            };
            let now = length(sub(points[b].position, points[a].position));
            stretch = stretch.max(now / rest.max(1.0));
        }
        let opacity = 1.0 - smoothstep(PULP_FADE_FROM, PULP_FADE_TO, stretch as f32);
        if opacity <= 0.02 {
            continue;
        }
        let colors = corners.map(|index| {
            let point = shown_point(points, index);
            // Pulp is uneven, darker where the flesh is most crushed; each
            // point keeps its own shade, so the mottling holds still.
            let pulped = (point.exposure * 0.3 + point.contusion * 0.3).clamp(0.0, 0.6) as f32;
            let mottle = ((index as u32).wrapping_mul(2_654_435_761) >> 7) % 100;
            let mut color = mix(
                ctx.palette.crushed_flesh,
                ctx.palette.crushed_flesh_deep,
                (pulped + mottle as f32 * 0.004).min(1.0),
            );
            color.a *= opacity;
            color
        });
        push_piece(&mut mesh, world, triangle, colors);
    }
    draw_mesh(&mesh);
}

pub(super) fn draw_skin_layer(ctx: &Scene) {
    if !ctx.anatomy {
        draw_shaded_skin(ctx);
        return;
    }
    // Anatomy view: a faint skin veil with its wireframe over the exposed layers.
    let world = ctx.world;
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Skin || !world.triangle_alive(triangle) {
            continue;
        }
        let (load, _) = triangle_point_metrics(world, triangle);
        let contusion = triangle_point_contusion(world, triangle);
        let heat = (load / 1300.0).clamp(0.0, 1.0) as f32;
        let mut veil = mix(ctx.palette.skin_base, ctx.palette.skin_heat, heat * 0.35);
        veil = mix(
            veil,
            ctx.palette.skin_contusion,
            (contusion * 0.42).clamp(0.0, 0.55) as f32,
        );
        veil.a = (0.08 + heat * 0.08).min(0.17);
        fill_piece(world, triangle, veil);
        let wire = mix(ctx.palette.skin_wire, ctx.palette.skin_heat, heat * 0.65);
        outline_piece(world, triangle, wire, 1.0);
    }
}

/// Intact skin as one mesh with per-point colors, so shading blends smoothly
/// across triangles instead of showing the mesh: darker toward the outline for
/// a rounded look, warmer under load, bruised where contused.
pub(super) fn draw_shaded_skin(ctx: &Scene) {
    let world = ctx.world;
    let points = world.points();
    let mut vertex_of = vec![u16::MAX; points.len()];
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: None,
    };
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Skin || !world.triangle_alive(triangle) {
            continue;
        }
        if triangle.cut.is_some() {
            let colors = [triangle.a, triangle.b, triangle.c]
                .map(|index| skin_point_color(ctx, shown_point(points, index)));
            push_piece(&mut mesh, world, triangle, colors);
            continue;
        }
        for index in [triangle.a, triangle.b, triangle.c] {
            if vertex_of[index] == u16::MAX {
                let point = &points[index];
                vertex_of[index] = mesh.vertices.len() as u16;
                mesh.vertices.push(Vertex::new(
                    point.position.x as f32,
                    point.position.y as f32,
                    0.0,
                    0.0,
                    0.0,
                    skin_point_color(ctx, point),
                ));
            }
            mesh.indices.push(vertex_of[index]);
        }
    }
    draw_mesh(&mesh);
    draw_skin_rim(ctx);
}

/// How far a broken skin spring's ends have pulled apart, relative to its rest
/// length. A clean cut starts near 1; tissue torn by stretching starts wide.
pub(super) fn cut_gap(world: &rp::World, spring: &rp::Spring) -> f64 {
    let points = world.points();
    length(sub(points[spring.b].position, points[spring.a].position)) / spring.rest.max(1.0)
}

/// How open a skin triangle is, from 0 (intact, or split by a cut whose edges
/// still meet) to 1 (gone, showing whatever lies beneath).
pub(super) fn skin_opening(world: &rp::World, triangle: &rp::Triangle) -> f32 {
    if world.triangle_alive(triangle) {
        return 0.0;
    }
    if triangle.failed {
        return 1.0;
    }
    let springs = world.springs();
    let widest = [triangle.edge_ab, triangle.edge_bc, triangle.edge_ca]
        .iter()
        .filter_map(|&edge| springs.get(edge))
        .filter(|spring| spring.broken)
        .map(|spring| cut_gap(world, spring))
        .fold(1.0, f64::max);
    smoothstep(1.06, 1.4, widest as f32)
}

/// Skin triangles split by a clean cut that has not pulled apart yet. The mesh
/// cannot split inside a triangle, so without this a thin knife cut would show
/// as a whole row of missing skin; instead the skin stays closed over the cut
/// and fades away only as the edges separate.
pub(super) fn draw_closed_cut_skin(ctx: &Scene) {
    let world = ctx.world;
    let points = world.points();
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: None,
    };
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Skin
            || triangle.failed
            || world.triangle_alive(triangle)
        {
            continue;
        }
        let closed = 1.0 - skin_opening(world, triangle);
        if closed <= 0.02 || mesh.vertices.len() + 4 > u16::MAX as usize {
            continue;
        }
        let colors = [triangle.a, triangle.b, triangle.c].map(|index| {
            let point = shown_point(points, index);
            let mut color = mix(skin_point_color(ctx, point), ctx.palette.blood_mid, 0.18);
            color.a *= closed;
            color
        });
        push_piece(&mut mesh, world, triangle, colors);
    }
    draw_mesh(&mesh);
}

/// Soft shading strips just inside the silhouette plus a thin outline. Kept
/// separate from the mesh colors because thin wrists and ankles are meshed only
/// from outline points, so per-point shading alone would leave them dark.
pub(super) fn draw_skin_rim(ctx: &Scene) {
    let world = ctx.world;
    let points = world.points();
    let springs = world.springs();
    let rim = ctx.rim;
    let mut inward = vec![rp::Vec2 { x: 0.0, y: 0.0 }; points.len()];
    for edge in &rim.edges {
        let spring = springs[edge.spring];
        if spring.broken {
            continue;
        }
        let normal = inward_normal(points, spring, edge.inner_point, |p| p.position);
        inward[spring.a] = add(inward[spring.a], normal);
        inward[spring.b] = add(inward[spring.b], normal);
    }

    // A wide faint strip gives limbs volume; a narrow stronger one defines the edge.
    for (width, alpha, reach_share) in [(24.0_f64, 0.18, 0.5), (9.0, 0.45, 0.42)] {
        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        for edge in &rim.edges {
            let spring = springs[edge.spring];
            if spring.broken {
                continue;
            }
            let base = mesh.vertices.len() as u16;
            for index in [spring.a, spring.b] {
                let position = points[index].position;
                let depth = width.min(rim.reach[index] as f64 * reach_share);
                let direction = normalized(inward[index], rp::Vec2 { x: 0.0, y: 0.0 });
                let inner = add(position, scale(direction, depth));
                mesh.vertices.push(Vertex::new(
                    position.x as f32,
                    position.y as f32,
                    0.0,
                    0.0,
                    0.0,
                    with_alpha(ctx.palette.skin_shade, alpha),
                ));
                mesh.vertices.push(Vertex::new(
                    inner.x as f32,
                    inner.y as f32,
                    0.0,
                    0.0,
                    0.0,
                    with_alpha(ctx.palette.skin_shade, 0.0),
                ));
            }
            mesh.indices.extend_from_slice(&[
                base,
                base + 2,
                base + 1,
                base + 1,
                base + 2,
                base + 3,
            ]);
        }
        draw_mesh(&mesh);
    }

    let outline = with_alpha(ctx.palette.skin_outline, 0.78);
    for edge in &rim.edges {
        let spring = springs[edge.spring];
        if !spring.broken {
            draw_line_vec(
                points[spring.a].position,
                points[spring.b].position,
                1.4,
                outline,
            );
        }
    }
}

/// Skin as it looks: shaded toward the outline, warmer where pressed now,
/// red where a blow landed, and bruised as far as the bruise shows yet (see
/// `bruises.rs`: a bruise darkens over hours, so a fresh blow leaves a red
/// mark).
pub(super) fn skin_point_color(ctx: &Scene, point: &rp::Point) -> Color {
    let depth = smoothstep(0.0, 22.0, point.surface_depth as f32);
    let heat = (point.load / 1300.0).clamp(0.0, 1.0) as f32;
    let mut color = mix(ctx.palette.skin_mid, ctx.palette.skin_light, depth);
    color = mix(color, ctx.palette.skin_heat, heat * 0.85);
    color = mix(
        color,
        ctx.palette.skin_flush,
        (point.flush as f32 * 0.5).clamp(0.0, 0.55),
    );
    color = mix(
        color,
        ctx.palette.skin_contusion,
        (point.bruise_shown as f32 * 0.56).clamp(0.0, 0.72),
    );
    // Whole skin hides what lies under it, however the flesh beneath is hurt;
    // only an opening in it shows the muscle or bone.
    color.a = 0.98;
    color
}

/// Skin wounds drawn from the mesh itself. A cut is a line through where the
/// blade crossed each skin spring, or through the middle of a torn one, joined
/// across each triangle it passes through, so it follows the blade's path. A
/// torn spring's line fades as the tear pulls open; a blade cut that opened
/// gapes between its lips instead (`draw_cut_lips`). Where skin has torn
/// open, a thin dark rim marks the edge of the intact skin around it.
pub(super) fn draw_skin_wounds(ctx: &Scene) {
    let world = ctx.world;
    let points = world.points();
    let springs = world.springs();
    let triangles = world.triangles();
    let opening: Vec<f32> = triangles
        .iter()
        .map(|triangle| {
            if triangle.layer == rp::TissueLayer::Skin {
                skin_opening(world, triangle)
            } else {
                1.0
            }
        })
        .collect();
    let line_color = mix(ctx.palette.wound_shadow, ctx.palette.wound_edge, 0.35);

    for (index, triangle) in triangles.iter().enumerate() {
        // A cut that opened shows its lips instead (`draw_cut_lips`).
        if triangle.layer != rp::TissueLayer::Skin
            || opening[index] >= 0.98
            || triangle.cut.is_some()
        {
            continue;
        }
        let mut cut = [(rp::Vec2 { x: 0.0, y: 0.0 }, 0.0f32); 3];
        let mut count = 0;
        for edge in [triangle.edge_ab, triangle.edge_bc, triangle.edge_ca] {
            let Some((spring, wear)) = springs
                .get(edge)
                .zip(world.spring_wear().get(edge))
                .filter(|(spring, wear)| spring.broken || wear.cut)
            else {
                continue;
            };
            let (a, b) = (points[spring.a].position, points[spring.b].position);
            cut[count] = if spring.broken {
                // A torn spring gapes at once; only a clean cut reads as a line.
                let clean = 1.0 - smoothstep(1.3, 1.9, cut_gap(world, spring) as f32);
                (mid(a, b), clean)
            } else {
                (add(a, scale(sub(b, a), wear.cut_at)), 1.0)
            };
            count += 1;
        }
        let shown = 1.0 - opening[index];
        let centroid = scale(
            add(
                add(points[triangle.a].position, points[triangle.b].position),
                points[triangle.c].position,
            ),
            1.0 / 3.0,
        );
        let stroke = |a: rp::Vec2, b: rp::Vec2, alpha: f32| {
            if alpha > 0.03 {
                draw_line_vec(a, b, 2.0, with_alpha(line_color, 0.85 * alpha));
            }
        };
        match count {
            1 => stroke(cut[0].0, mid(cut[0].0, centroid), shown * cut[0].1),
            2 => stroke(cut[0].0, cut[1].0, shown * cut[0].1.min(cut[1].1)),
            3 => {
                for &(point, clean) in &cut {
                    stroke(point, centroid, shown * clean);
                }
            }
            _ => {}
        }
    }

    for (spring_index, sides) in ctx.rim.edge_triangles.iter().enumerate() {
        if sides[1] == NO_TRIANGLE {
            continue;
        }
        let rim = (opening[sides[0]] - opening[sides[1]]).abs();
        if rim < 0.05 {
            continue;
        }
        let spring = springs[spring_index];
        draw_line_vec(
            points[spring.a].position,
            points[spring.b].position,
            1.6,
            with_alpha(ctx.palette.wound_shadow, 0.72 * rim),
        );
    }
}
