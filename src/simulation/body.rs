//! Procedural layered body: skin and muscle sheets meshed to follow the
//! mannequin figure in `crate::silhouette`, a skeleton built on that figure's
//! limb joints, and organs and vessels placed at landmarks inside it.
//!
//! Body coordinates are fractions of the body height: `u` runs from the midline
//! toward the viewer's right and `v` runs down from the top of the head. "Left"
//! and "right" in landmark names are as seen on screen; organs use anatomical
//! sides, so the figure's right lung sits on the screen's left.

use std::collections::HashMap;

use super::*;
use crate::mesh::{delaunay, hex_lattice, resample_loop, P2};
use crate::silhouette::{
    human_silhouette, Landmark, SilhouetteField, ANKLE, ELBOW, HIP, KNEE, KNUCKLES, SHOULDER, TOES,
    WRIST,
};

/// Outline points are spaced this many point spacings apart.
const BOUNDARY_SPACING_SCALE: f64 = 0.92;
/// Interior lattice edge length, in point spacings.
const LATTICE_EDGE_SCALE: f64 = 1.12;
/// Interior points stay this many lattice edges inside their layer outline.
const LATTICE_CLEARANCE_SCALE: f64 = 0.55;
/// The muscle sheet's outline sits this many point spacings inside the skin outline.
const MUSCLE_INSET_SCALE: f64 = 0.35;
/// Tissue above this height (the crown of the head) is pinned and holds the body up.
const PIN_V: f64 = 0.035;
/// Muscle edges within this angle of the local bone direction act as fibers.
const FIBER_ALIGNMENT: f64 = 0.80;
/// Muscle edges further than this from the bone direction act as cross fibers.
const CROSS_ALIGNMENT: f64 = 0.45;

const SKULL: [Landmark; 2] = [(0.0, 0.030), (0.0, 0.118)];
const SPINE: [Landmark; 2] = [(0.0, 0.150), (0.0, 0.495)];
// The girdle and pelvis stop short of the shoulder and hip joints by about the
// gap a bone joint holds at rest (0.7 point spacings); a joint whose bone ends
// start together shoves them apart as soon as the body moves.
const SHOULDER_GIRDLE: [Landmark; 2] = [(-0.103, 0.203), (0.103, 0.203)];
const PELVIS: [Landmark; 2] = [(-0.072, 0.487), (0.072, 0.487)];
const LEFT_SHOULDER: Landmark = mirrored(SHOULDER);
const LEFT_ELBOW: Landmark = mirrored(ELBOW);
const LEFT_WRIST: Landmark = mirrored(WRIST);
const LEFT_KNUCKLES: Landmark = mirrored(KNUCKLES);
const RIGHT_SHOULDER: Landmark = SHOULDER;
const RIGHT_ELBOW: Landmark = ELBOW;
const RIGHT_WRIST: Landmark = WRIST;
const RIGHT_KNUCKLES: Landmark = KNUCKLES;
const LEFT_HIP: Landmark = mirrored(HIP);
const LEFT_KNEE: Landmark = mirrored(KNEE);
const LEFT_ANKLE: Landmark = mirrored(ANKLE);
const LEFT_TOES: Landmark = mirrored(TOES);
const RIGHT_HIP: Landmark = HIP;
const RIGHT_KNEE: Landmark = KNEE;
const RIGHT_ANKLE: Landmark = ANKLE;
const RIGHT_TOES: Landmark = TOES;
/// (root height, lateral reach, strength scale) for each rib pair.
const RIBS: [(f64, f64, f64); 4] = [
    (0.240, 0.062, 0.62),
    (0.275, 0.072, 0.58),
    (0.310, 0.070, 0.55),
    (0.343, 0.058, 0.52),
];
const RIB_DROP: f64 = 0.022;
/// The aorta sits deep in the torso, so it takes this much more force to cut.
const AORTA_DEPTH_SCALE: f64 = 2.4;
/// Torso muscle inside this box forms the pressurized cavity.
const CAVITY_U: f64 = 0.078;
const CAVITY_V: (f64, f64) = (0.215, 0.505);

/// The landmark on the viewer's left matching one on the right.
const fn mirrored((u, v): Landmark) -> Landmark {
    (-u, v)
}

/// Placement of the body in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyFrame {
    /// World position of the top of the head on the body midline.
    pub origin: Vec2,
    /// Body height in world pixels.
    pub height: f64,
}

impl BodyFrame {
    /// World position of body coordinates (`u`, `v`).
    pub fn point(&self, u: f64, v: f64) -> Vec2 {
        Vec2 {
            x: self.origin.x + u * self.height,
            y: self.origin.y + v * self.height,
        }
    }

    /// Body coordinates (`u`, `v`) of a world position.
    pub fn body_coords(&self, point: Vec2) -> (f64, f64) {
        (
            (point.x - self.origin.x) / self.height,
            (point.y - self.origin.y) / self.height,
        )
    }
}

/// Where `create_layered_body` places the body in a window of this size.
pub fn body_frame(width: f64, height: f64) -> BodyFrame {
    BodyFrame {
        origin: Vec2 {
            x: width * 0.52,
            y: height * 0.09,
        },
        height: (height * 0.78).min(width * 1.45).min(720.0),
    }
}

/// The body with its crown at `top`, as large as fits above `bottom` and in the
/// window's width; the app uses it to keep the body clear of its controls.
pub fn body_frame_between(width: f64, top: f64, bottom: f64) -> BodyFrame {
    BodyFrame {
        origin: Vec2 {
            x: width * 0.52,
            y: top,
        },
        height: (bottom - top).min(width * 1.45).min(720.0).max(1.0),
    }
}

pub fn create_layered_body(width: f64, height: f64, materials: Materials) -> World {
    create_layered_body_in(body_frame(width, height), materials)
}

/// The layered body placed at `frame`.
pub fn create_layered_body_in(frame: BodyFrame, materials: Materials) -> World {
    let field = human_silhouette();
    let mut world = World::new(materials);
    let spacing = materials.point_spacing;
    let at = |landmark: Landmark| frame.point(landmark.0, landmark.1);

    let skin = build_layer_mesh(&mut world, frame, field, TissueLayer::Skin, 0.0);
    let muscle = build_layer_mesh(
        &mut world,
        frame,
        field,
        TissueLayer::Muscle,
        spacing * MUSCLE_INSET_SCALE,
    );
    let fiber_guides: Vec<(Vec2, Vec2)> = fiber_guide_landmarks()
        .iter()
        .map(|&(a, b)| (at(a), at(b)))
        .collect();
    let skin_edges = add_layer_springs(&mut world, &skin, TissueLayer::Skin, &fiber_guides);
    let muscle_edges = add_layer_springs(&mut world, &muscle, TissueLayer::Muscle, &fiber_guides);
    add_long_fiber_springs(&mut world, &muscle, &muscle_edges, &fiber_guides);
    add_layer_triangles(&mut world, &skin, &skin_edges, TissueLayer::Skin);
    add_layer_triangles(&mut world, &muscle, &muscle_edges, TissueLayer::Muscle);
    for outline in skin.outlines.iter().chain(&muscle.outlines) {
        world.add_outline_loop(outline.clone());
    }

    let cavity_areas = world
        .areas
        .iter()
        .enumerate()
        .filter_map(|(index, area)| {
            if area.layer != TissueLayer::Muscle {
                return None;
            }
            let homes = [area.a, area.b, area.c].map(|i| world.points[i].home);
            let centroid = Vec2 {
                x: (homes[0].x + homes[1].x + homes[2].x) / 3.0,
                y: (homes[0].y + homes[1].y + homes[2].y) / 3.0,
            };
            let (u, v) = frame.body_coords(centroid);
            (u.abs() <= CAVITY_U && (CAVITY_V.0..=CAVITY_V.1).contains(&v)).then_some(index)
        })
        .collect::<Vec<_>>();
    world.add_cavity_from_areas(cavity_areas);

    attach_skin_to_muscle(&mut world, &skin.points, &muscle.points);

    let h = frame.height;
    world.add_organ_region(
        OrganKind::RightLung,
        frame.point(-0.040, 0.278),
        Vec2 {
            x: h * 0.034,
            y: h * 0.060,
        },
    );
    world.add_organ_region(
        OrganKind::LeftLung,
        frame.point(0.040, 0.278),
        Vec2 {
            x: h * 0.032,
            y: h * 0.058,
        },
    );
    world.add_organ_region(
        OrganKind::Liver,
        frame.point(-0.030, 0.352),
        Vec2 {
            x: h * 0.054,
            y: h * 0.032,
        },
    );
    world.add_organ_region(
        OrganKind::Spleen,
        frame.point(0.058, 0.352),
        Vec2 {
            x: h * 0.026,
            y: h * 0.024,
        },
    );

    add_skeleton(&mut world, frame, materials);

    for &muscle_point in &muscle.points {
        let position = world.points[muscle_point].position;
        let mut nearest_bone = usize::MAX;
        let mut nearest_distance = spacing * 2.7;
        let mut nearest_t = 0.0;
        for (i, bone) in world.bones.iter().enumerate() {
            let t = segment_t(position, bone.a, bone.b);
            let d = distance(position, bone_point(*bone, t));
            if d < nearest_distance {
                nearest_distance = d;
                nearest_bone = i;
                nearest_t = t;
            }
        }
        if nearest_bone != usize::MAX {
            world.add_bone_attachment(muscle_point, nearest_bone, nearest_t);
        }
    }

    // The aorta runs deep behind the sternum and ribs: a surface slash should not
    // reach it, only a deep stab or crushing trauma.
    let aorta =
        world.add_vessel_segment(frame.point(0.0, 0.155), frame.point(0.0, 0.490), 4.1, 1.65);
    world.vessels[aorta].laceration_impulse *= AORTA_DEPTH_SCALE;
    for side in [-1.0, 1.0] {
        world.add_vessel_segment(
            frame.point(side * 0.030, 0.515),
            frame.point(side * 0.056, 0.860),
            3.0,
            1.22,
        );
        // The brachial artery runs down the inside of the upper arm between the
        // bone and the skin, and on down the inside of the forearm.
        world.add_vessel_segment(
            frame.point(side * 0.088, 0.240),
            frame.point(side * 0.110, 0.380),
            2.5,
            1.05,
        );
        world.add_vessel_segment(
            frame.point(side * 0.110, 0.380),
            frame.point(side * 0.133, 0.480),
            2.0,
            0.95,
        );
        world.add_vessel_segment(
            frame.point(side * 0.016, 0.125),
            frame.point(side * 0.089, 0.228),
            2.6,
            1.10,
        );
    }

    world
}

/// Bone directions that steer muscle fiber orientation (ribs excluded).
fn fiber_guide_landmarks() -> [(Landmark, Landmark); 16] {
    [
        (SKULL[0], SKULL[1]),
        (SPINE[0], SPINE[1]),
        (SHOULDER_GIRDLE[0], SHOULDER_GIRDLE[1]),
        (PELVIS[0], PELVIS[1]),
        (LEFT_SHOULDER, LEFT_ELBOW),
        (LEFT_ELBOW, LEFT_WRIST),
        (LEFT_WRIST, LEFT_KNUCKLES),
        (RIGHT_SHOULDER, RIGHT_ELBOW),
        (RIGHT_ELBOW, RIGHT_WRIST),
        (RIGHT_WRIST, RIGHT_KNUCKLES),
        (LEFT_HIP, LEFT_KNEE),
        (LEFT_KNEE, LEFT_ANKLE),
        (LEFT_ANKLE, LEFT_TOES),
        (RIGHT_HIP, RIGHT_KNEE),
        (RIGHT_KNEE, RIGHT_ANKLE),
        (RIGHT_ANKLE, RIGHT_TOES),
    ]
}

fn add_skeleton(world: &mut World, frame: BodyFrame, materials: Materials) {
    let at = |landmark: Landmark| frame.point(landmark.0, landmark.1);
    let strength = materials.bone_fracture_impulse;
    let bone = |world: &mut World, a: Landmark, b: Landmark, radius: f64, scale: f64| {
        world.add_bone_segment(at(a), at(b), radius, strength * scale, false)
    };

    // The skull is pinned with the crown, so it must stay at index 0 and the spine at 1.
    let head = world.add_bone_segment(at(SKULL[0]), at(SKULL[1]), 8.2, strength * 0.75, true);
    let spine = world.add_bone_segment_with_kind(
        at(SPINE[0]),
        at(SPINE[1]),
        7.2,
        strength,
        false,
        BoneKind::Spine,
    );
    let shoulders = bone(world, SHOULDER_GIRDLE[0], SHOULDER_GIRDLE[1], 6.2, 0.95);
    let pelvis = bone(world, PELVIS[0], PELVIS[1], 6.4, 0.9);
    let spine_t = |v: f64| ((v - SPINE[0].1) / (SPINE[1].1 - SPINE[0].1)).clamp(0.0, 1.0);

    let mut ribs = Vec::new();
    for (root_v, reach, scale) in RIBS {
        for side in [-1.0, 1.0] {
            let rib = world.add_bone_segment_with_kind(
                frame.point(side * 0.008, root_v),
                frame.point(side * reach, root_v + RIB_DROP),
                3.4,
                strength * scale,
                false,
                BoneKind::Rib,
            );
            ribs.push((rib, root_v));
        }
    }
    // Joints never rest closer than 0.7 point spacings, so a bone hanging from an
    // elbow, wrist, knee, or ankle starts that far past the joint. Starting it
    // flush makes the joint shove it away as soon as the body settles and
    // overshoot into a subluxation while the body is still at rest.
    let joint_gap = materials.point_spacing * 0.70;
    let below_joint =
        |world: &mut World, joint: Landmark, tip: Landmark, radius: f64, strength_scale: f64| {
            let start = at(joint);
            let end = at(tip);
            let gap = scale(
                normalized(subtract(end, start), Vec2 { x: 0.0, y: 1.0 }),
                joint_gap,
            );
            world.add_bone_segment(
                add(start, gap),
                end,
                radius,
                strength * strength_scale,
                false,
            )
        };
    let left_upper_arm = bone(world, LEFT_SHOULDER, LEFT_ELBOW, 5.7, 0.82);
    let left_forearm = below_joint(world, LEFT_ELBOW, LEFT_WRIST, 4.8, 0.72);
    let right_upper_arm = bone(world, RIGHT_SHOULDER, RIGHT_ELBOW, 5.7, 0.82);
    let right_forearm = below_joint(world, RIGHT_ELBOW, RIGHT_WRIST, 4.8, 0.72);
    let left_thigh = bone(world, LEFT_HIP, LEFT_KNEE, 6.4, 0.9);
    let left_shin = below_joint(world, LEFT_KNEE, LEFT_ANKLE, 5.3, 0.78);
    let right_thigh = bone(world, RIGHT_HIP, RIGHT_KNEE, 6.4, 0.9);
    let right_shin = below_joint(world, RIGHT_KNEE, RIGHT_ANKLE, 5.3, 0.78);
    let left_hand = below_joint(world, LEFT_WRIST, LEFT_KNUCKLES, 3.6, 0.6);
    let right_hand = below_joint(world, RIGHT_WRIST, RIGHT_KNUCKLES, 3.6, 0.6);
    let left_foot = below_joint(world, LEFT_ANKLE, LEFT_TOES, 4.0, 0.65);
    let right_foot = below_joint(world, RIGHT_ANKLE, RIGHT_TOES, 4.0, 0.65);
    // What each bone is, as injury research names it.
    for (bone, part) in [
        (head, BonePart::Skull),
        (spine, BonePart::Spine),
        (shoulders, BonePart::Collarbone),
        (pelvis, BonePart::Pelvis),
        (left_upper_arm, BonePart::UpperArm),
        (right_upper_arm, BonePart::UpperArm),
        (left_forearm, BonePart::Forearm),
        (right_forearm, BonePart::Forearm),
        (left_hand, BonePart::Hand),
        (right_hand, BonePart::Hand),
        (left_thigh, BonePart::Thigh),
        (right_thigh, BonePart::Thigh),
        (left_shin, BonePart::Shin),
        (right_shin, BonePart::Shin),
        (left_foot, BonePart::Foot),
        (right_foot, BonePart::Foot),
    ] {
        world.bones[bone].part = part;
    }
    for &(rib, _) in &ribs {
        world.bones[rib].part = BonePart::Rib;
    }

    world.add_bone_joint(head, 1.0, spine, 0.0, -0.45, 0.45);
    world.add_bone_joint(
        spine,
        spine_t(SHOULDER_GIRDLE[0].1),
        shoulders,
        0.5,
        -0.55,
        0.55,
    );
    world.add_bone_joint(spine, 1.0, pelvis, 0.5, -0.45, 0.45);
    for (rib, root_v) in ribs {
        world.add_bone_joint(spine, spine_t(root_v), rib, 0.0, -0.40, 0.40);
    }
    let along = |segment: [Landmark; 2], u: f64| {
        ((u - segment[0].0) / (segment[1].0 - segment[0].0)).clamp(0.0, 1.0)
    };
    world.add_bone_joint(
        shoulders,
        along(SHOULDER_GIRDLE, LEFT_SHOULDER.0),
        left_upper_arm,
        0.0,
        -1.25,
        1.05,
    );
    world.add_bone_joint(left_upper_arm, 1.0, left_forearm, 0.0, -1.10, 1.10);
    world.add_bone_joint(left_forearm, 1.0, left_hand, 0.0, -0.90, 0.90);
    world.add_bone_joint(
        shoulders,
        along(SHOULDER_GIRDLE, RIGHT_SHOULDER.0),
        right_upper_arm,
        0.0,
        -1.05,
        1.25,
    );
    world.add_bone_joint(right_upper_arm, 1.0, right_forearm, 0.0, -1.10, 1.10);
    world.add_bone_joint(right_forearm, 1.0, right_hand, 0.0, -0.90, 0.90);
    world.add_bone_joint(
        pelvis,
        along(PELVIS, LEFT_HIP.0),
        left_thigh,
        0.0,
        -0.78,
        0.78,
    );
    world.add_bone_joint(left_thigh, 1.0, left_shin, 0.0, -0.85, 0.85);
    world.add_bone_joint(left_shin, 1.0, left_foot, 0.0, -0.60, 0.60);
    world.add_bone_joint(
        pelvis,
        along(PELVIS, RIGHT_HIP.0),
        right_thigh,
        0.0,
        -0.78,
        0.78,
    );
    world.add_bone_joint(right_thigh, 1.0, right_shin, 0.0, -0.85, 0.85);
    world.add_bone_joint(right_shin, 1.0, right_foot, 0.0, -0.60, 0.60);
}

struct LayerMesh {
    points: Vec<usize>,
    triangles: Vec<[usize; 3]>,
    /// The sheet's outline loops, as world point indices in order.
    outlines: Vec<Vec<usize>>,
}

/// Meshes one tissue sheet: evenly spaced points along its outline plus a
/// hexagonal interior lattice, Delaunay-triangulated and trimmed to the shape.
fn build_layer_mesh(
    world: &mut World,
    frame: BodyFrame,
    field: &SilhouetteField,
    layer: TissueLayer,
    inset: f64,
) -> LayerMesh {
    let spacing = world.materials.point_spacing;
    let lattice_edge = spacing * LATTICE_EDGE_SCALE;
    let clearance = lattice_edge * LATTICE_CLEARANCE_SCALE;
    // Depth below the skin outline in pixels (positive inside the body).
    let depth = |p: P2| -> f64 {
        let (u, v) = frame.body_coords(Vec2 { x: p.0, y: p.1 });
        -field.distance(u, v) * frame.height
    };

    let mut candidates: Vec<P2> = Vec::new();
    // For each outline point, the next point along its outline loop.
    let mut next_on_outline: Vec<usize> = Vec::new();
    let mut outline_ranges = Vec::new();
    for outline in field.contours(-inset / frame.height) {
        let world_outline: Vec<P2> = outline
            .iter()
            .map(|&(u, v)| {
                let p = frame.point(u, v);
                (p.x, p.y)
            })
            .collect();
        let first = candidates.len();
        let resampled = resample_loop(&world_outline, spacing * BOUNDARY_SPACING_SCALE);
        let count = resampled.len();
        candidates.extend(resampled);
        next_on_outline.extend((0..count).map(|k| first + (k + 1) % count));
        outline_ranges.push(first..first + count);
    }
    let follows_outline = |a: usize, b: usize| {
        (a < next_on_outline.len() && next_on_outline[a] == b)
            || (b < next_on_outline.len() && next_on_outline[b] == a)
    };
    let min = frame.point(-0.27, -0.01);
    let max = frame.point(0.27, 1.01);
    candidates.extend(
        hex_lattice((min.x, min.y), (max.x, max.y), lattice_edge)
            .into_iter()
            .filter(|&p| depth(p) > inset + clearance),
    );

    let midpoint = |a: P2, b: P2| ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    let kept: Vec<[usize; 3]> = delaunay(&candidates)
        .into_iter()
        .filter(|t| {
            let (a, b, c) = (candidates[t[0]], candidates[t[1]], candidates[t[2]]);
            let doubled_area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
            let centroid = ((a.0 + b.0 + c.0) / 3.0, (a.1 + b.1 + c.1) / 3.0);
            // A triangle bridging a gap (arm to torso, between the legs) has its
            // centroid or an edge in the gap. An edge between neighboring
            // outline points follows the outline, so it may dip outside where
            // the outline turns inward, as at the crotch; trimming it there
            // would leave a notch in the skin that bares the muscle. Other
            // edges get a little slack for gently curved outline.
            doubled_area > spacing * spacing * 0.02
                && depth(centroid) > inset
                && [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])]
                    .iter()
                    .all(|&(i, j)| {
                        follows_outline(i, j)
                            || depth(midpoint(candidates[i], candidates[j])) > inset - 1.5
                    })
        })
        .collect();

    let mut index_of = vec![usize::MAX; candidates.len()];
    let mut points = Vec::new();
    for triangle in &kept {
        for &candidate in triangle {
            if index_of[candidate] != usize::MAX {
                continue;
            }
            let position = Vec2 {
                x: candidates[candidate].0,
                y: candidates[candidate].1,
            };
            let pinned = frame.body_coords(position).1 < PIN_V;
            let index = world.add_point(position, layer, pinned);
            world.points[index].surface_depth = depth(candidates[candidate]).max(0.0);
            index_of[candidate] = index;
            points.push(index);
        }
    }
    LayerMesh {
        points,
        triangles: kept
            .iter()
            .map(|t| [index_of[t[0]], index_of[t[1]], index_of[t[2]]])
            .collect(),
        outlines: outline_ranges
            .into_iter()
            .map(|range| {
                range
                    .map(|candidate| index_of[candidate])
                    .filter(|&index| index != usize::MAX)
                    .collect()
            })
            .collect(),
    }
}

fn edge_key(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

/// Unit direction of the bone guide nearest to `point`.
fn fiber_direction(point: Vec2, guides: &[(Vec2, Vec2)]) -> Vec2 {
    let mut best = (f64::INFINITY, Vec2 { x: 0.0, y: 1.0 });
    for &(a, b) in guides {
        let d = distance_to_segment(point, a, b);
        if d < best.0 {
            best = (d, normalized(subtract(b, a), Vec2 { x: 0.0, y: 1.0 }));
        }
    }
    best.1
}

fn push_spring(
    world: &mut World,
    a: usize,
    b: usize,
    layer: TissueLayer,
    (stiffness, tear_stretch, tear_impulse, fiber): (f64, f64, f64, bool),
) -> usize {
    let rest = distance(world.points[a].position, world.points[b].position);
    world.springs.push(Spring {
        a,
        b,
        rest,
        rest_reference: rest,
        stiffness,
        tear_stretch,
        tear_impulse,
        layer,
        fiber,
        ..Spring::default()
    });
    world.springs.len() - 1
}

/// One spring per triangle edge. Muscle edges take fiber, cross, or shear
/// properties from their angle to the nearest bone.
fn add_layer_springs(
    world: &mut World,
    mesh: &LayerMesh,
    layer: TissueLayer,
    guides: &[(Vec2, Vec2)],
) -> HashMap<(usize, usize), usize> {
    let m = world.materials;
    let mut edges = HashMap::new();
    for triangle in &mesh.triangles {
        for (a, b) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let key = edge_key(a, b);
            if edges.contains_key(&key) {
                continue;
            }
            let properties = if layer == TissueLayer::Skin {
                (
                    m.skin_structural_stiffness,
                    m.skin_tear_stretch,
                    m.skin_tear_impulse,
                    false,
                )
            } else {
                let pa = world.points[a].position;
                let pb = world.points[b].position;
                let along = normalized(subtract(pb, pa), Vec2 { x: 1.0, y: 0.0 });
                let alignment = dot(along, fiber_direction(midpoint(pa, pb), guides)).abs();
                if alignment >= FIBER_ALIGNMENT {
                    (
                        m.muscle_fiber_stiffness,
                        m.muscle_tear_stretch,
                        m.muscle_tear_impulse,
                        true,
                    )
                } else if alignment <= CROSS_ALIGNMENT {
                    (
                        m.muscle_cross_stiffness,
                        m.muscle_tear_stretch,
                        m.muscle_tear_impulse,
                        false,
                    )
                } else {
                    (
                        m.muscle_shear_stiffness,
                        m.muscle_tear_stretch * 1.12,
                        m.muscle_tear_impulse,
                        false,
                    )
                }
            };
            edges.insert(key, push_spring(world, a, b, layer, properties));
        }
    }
    edges
}

/// Softer two-edge springs along the fiber direction, giving muscle the same
/// lengthwise bending resistance the old grid's skip-one fibers provided.
fn add_long_fiber_springs(
    world: &mut World,
    mesh: &LayerMesh,
    edges: &HashMap<(usize, usize), usize>,
    guides: &[(Vec2, Vec2)],
) {
    let m = world.materials;
    let lattice_edge = m.point_spacing * LATTICE_EDGE_SCALE;
    let mut neighbors: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut sorted_edges: Vec<(usize, usize)> = edges.keys().copied().collect();
    sorted_edges.sort_unstable();
    for &(a, b) in &sorted_edges {
        neighbors.entry(a).or_default().push(b);
        neighbors.entry(b).or_default().push(a);
    }
    let mut added: HashMap<(usize, usize), ()> = HashMap::new();
    for &p in &mesh.points {
        let position = world.points[p].position;
        let fiber = fiber_direction(position, guides);
        let mut best: Option<(f64, usize)> = None;
        for &q in neighbors.get(&p).map(Vec::as_slice).unwrap_or(&[]) {
            for &r in neighbors.get(&q).map(Vec::as_slice).unwrap_or(&[]) {
                if r == p || edges.contains_key(&edge_key(p, r)) {
                    continue;
                }
                let delta = subtract(world.points[r].position, position);
                let length = hypot(delta.x, delta.y);
                if !(1.5 * lattice_edge..=2.6 * lattice_edge).contains(&length) {
                    continue;
                }
                let alignment = dot(delta, fiber) / length;
                if alignment >= 0.9 && best.is_none_or(|(score, _)| alignment > score) {
                    best = Some((alignment, r));
                }
            }
        }
        if let Some((_, r)) = best {
            if added.insert(edge_key(p, r), ()).is_none() {
                push_spring(
                    world,
                    p,
                    r,
                    TissueLayer::Muscle,
                    (
                        m.muscle_fiber_stiffness * 0.42,
                        m.muscle_tear_stretch * 1.05,
                        m.muscle_tear_impulse,
                        true,
                    ),
                );
            }
        }
    }
}

fn add_layer_triangles(
    world: &mut World,
    mesh: &LayerMesh,
    edges: &HashMap<(usize, usize), usize>,
    layer: TissueLayer,
) {
    let stiffness = if layer == TissueLayer::Skin {
        world.materials.skin_area_stiffness
    } else {
        world.materials.muscle_area_stiffness
    };
    for &[a, b, c] in &mesh.triangles {
        let edge_ab = edges[&edge_key(a, b)];
        let edge_bc = edges[&edge_key(b, c)];
        let edge_ca = edges[&edge_key(c, a)];
        world.triangles.push(Triangle {
            a,
            b,
            c,
            edge_ab,
            edge_bc,
            edge_ca,
            layer,
            ..Triangle::default()
        });
        world.areas.push(AreaConstraint {
            a,
            b,
            c,
            edge_ab,
            edge_bc,
            edge_ca,
            rest_area: signed_area(
                world.points[a].position,
                world.points[b].position,
                world.points[c].position,
            ),
            stiffness,
            layer,
            lambda: 0.0,
        });
    }
}

/// Ties each skin point to its nearest muscle points underneath.
fn attach_skin_to_muscle(world: &mut World, skin_points: &[usize], muscle_points: &[usize]) {
    let spacing = world.materials.point_spacing;
    let max_distance = spacing * 3.15;
    for &skin_point in skin_points {
        let skin_position = world.points[skin_point].position;
        let mut nearest = [(usize::MAX, f64::INFINITY); SKIN_ATTACHMENT_CANDIDATES];
        for &muscle_point in muscle_points {
            let d = distance(skin_position, world.points[muscle_point].position);
            if d > max_distance {
                continue;
            }
            if let Some(slot) = nearest.iter().position(|&(_, best)| d < best) {
                nearest[slot..].rotate_right(1);
                nearest[slot] = (muscle_point, d);
            }
        }
        for (muscle_point, d) in nearest {
            if muscle_point != usize::MAX {
                world.attachments.push(Attachment {
                    skin_point,
                    muscle_point,
                    rest: d.max(spacing * 0.34),
                    broken: false,
                    stress: 0.0,
                });
            }
        }
    }
}
