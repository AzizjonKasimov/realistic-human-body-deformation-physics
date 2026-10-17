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
use crate::mesh::{delaunay, distance as mesh_distance, hex_lattice, resample_loop_refined, P2};
use crate::silhouette::{
    figure_part, human_silhouette, muscle_silhouette, on_elbow_crease, FigurePart, Landmark,
    SilhouetteField, ANKLE, CREASE_POINT_SPACING, ELBOW, FIELD_HALF_WIDTH, HIP, KNEE, KNUCKLES,
    SHOULDER, TOES, WRIST,
};

/// Outline points are spaced this many point spacings apart.
const BOUNDARY_SPACING_SCALE: f64 = 0.92;
/// Interior lattice edge length, in point spacings.
const LATTICE_EDGE_SCALE: f64 = 1.12;
/// Interior points stay this many lattice edges inside their layer outline.
const LATTICE_CLEARANCE_SCALE: f64 = 0.55;
/// The muscle sheet's outline sits this many point spacings inside the skin outline.
const MUSCLE_INSET_SCALE: f64 = 0.35;
/// Muscle edges within this angle of the local bone direction act as fibers.
const FIBER_ALIGNMENT: f64 = 0.80;
/// Muscle edges further than this from the bone direction act as cross fibers.
const CROSS_ALIGNMENT: f64 = 0.45;

const SKULL: [Landmark; 2] = [(0.0, 0.030), (0.0, 0.118)];
const SPINE: [Landmark; 2] = [(0.0, 0.150), (0.0, 0.495)];
// The collarbones and pelvis stop short of the shoulder and hip joints by
// about the gap a bone joint holds at rest (0.7 point spacings); a joint whose
// bone ends start together shoves them apart as soon as the body moves.
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
    (0.240, 0.062, 0.74),
    (0.275, 0.072, 0.70),
    (0.310, 0.070, 0.66),
    (0.343, 0.058, 0.62),
];
const RIB_DROP: f64 = 0.022;
/// The aorta lies deep, against the spine and behind the organs, so it takes
/// this much more force to cut: most slashes stay shallow (Steel et al.
/// 2021), and only a deep stab or crushing trauma reaches it.
const AORTA_DEPTH_SCALE: f64 = 6.0;
/// Torso muscle inside this box forms the pressurized cavity.
const CAVITY_U: f64 = 0.078;
const CAVITY_V: (f64, f64) = (0.215, 0.505);

/// The landmark on the viewer's left matching one on the right.
const fn mirrored((u, v): Landmark) -> Landmark {
    (-u, v)
}

/// The point `t` of the way from `a` to `b`, moved `offset` body heights off
/// that line, to the side a quarter turn from the way it runs: for an arm
/// raised out to the viewer's right, below the upper arm, and outward from
/// a forearm rising from the elbow.
fn off_line(a: Landmark, b: Landmark, t: f64, offset: f64) -> Landmark {
    let (du, dv) = (b.0 - a.0, b.1 - a.1);
    let length = (du * du + dv * dv).sqrt().max(1.0e-9);
    (
        a.0 + du * t - dv / length * offset,
        a.1 + dv * t + du / length * offset,
    )
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
    world.body_height = frame.height;
    let spacing = materials.point_spacing;
    // Flesh is tied only to what it reaches through flesh, not to what lies
    // across a gap such as the inner elbow's crease or the armpit.
    let through_flesh = |from: Vec2, to: Vec2| reaches_through_flesh(field, frame, from, to);

    let skin = build_layer_mesh(&mut world, frame, field, TissueLayer::Skin, 0.0);
    let muscle = build_layer_mesh(
        &mut world,
        frame,
        muscle_silhouette(),
        TissueLayer::Muscle,
        spacing * MUSCLE_INSET_SCALE,
    );
    let fiber_guides = fiber_guides(frame);
    let skin_edges = add_layer_springs(&mut world, &skin, TissueLayer::Skin, &fiber_guides);
    let muscle_edges = add_layer_springs(&mut world, &muscle, TissueLayer::Muscle, &fiber_guides);
    add_long_fiber_springs(
        &mut world,
        &muscle,
        &muscle_edges,
        &fiber_guides,
        &through_flesh,
    );
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

    attach_skin_to_muscle(&mut world, &skin.points, &muscle.points, &through_flesh);

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

    let skeleton = add_skeleton(&mut world, frame, materials);
    for (side, limb) in [Limb::LeftArm, Limb::RightArm].into_iter().enumerate() {
        world.add_arm_tendons(limb, skeleton.upper_arms[side], skeleton.forearms[side]);
    }
    cover::measure_bone_covers(&mut world, frame, field);

    for &muscle_point in &muscle.points {
        let position = world.points[muscle_point].position;
        let mut nearest_bone = usize::MAX;
        let mut nearest_distance = spacing * 2.7;
        let mut nearest_t = 0.0;
        for (i, bone) in world.bones.iter().enumerate() {
            let t = segment_t(position, bone.a, bone.b);
            let d = distance(position, bone_point(*bone, t));
            if d < nearest_distance && through_flesh(position, bone_point(*bone, t)) {
                nearest_distance = d;
                nearest_bone = i;
                nearest_t = t;
            }
        }
        // Flesh past the end of an arm bone at the elbow wraps round the
        // joint, over the point of the elbow, and slides over it as the elbow
        // turns: tied to the end of the upper arm, it was torn off as the
        // elbow straightened, and held a broken forearm bent up in the guard.
        let past_elbow = (skeleton.upper_arms.contains(&nearest_bone) && nearest_t >= 1.0)
            || (skeleton.forearms.contains(&nearest_bone) && nearest_t <= 0.0);
        if nearest_bone != usize::MAX && !past_elbow {
            world.add_bone_attachment(muscle_point, nearest_bone, nearest_t);
        }
    }

    // The aorta runs deep behind the sternum and ribs: a surface slash should not
    // reach it, only a deep stab or crushing trauma.
    let aorta =
        world.add_vessel_segment(frame.point(0.0, 0.155), frame.point(0.0, 0.490), 4.1, 1.65);
    world.vessels[aorta].laceration_impulse *= AORTA_DEPTH_SCALE;
    // The arteries of the limbs, described on the viewer's right and mirrored.
    // The femoral artery runs from the groin down the inside of the thigh to
    // the top of the shin. The subclavian runs from the base of the neck out
    // under the collarbone to the armpit, and the brachial on along the
    // underside of the raised upper arm, between the bone and the skin, to
    // the elbow, then up the inner side of the forearm, toward the head.
    let shin_at = |v: f64| off_line(KNEE, ANKLE, (v - KNEE.1) / (ANKLE.1 - KNEE.1), 0.0);
    let armpit = off_line(SHOULDER, ELBOW, 0.15, 0.017);
    let arteries = [
        ((0.030, 0.515), shin_at(0.860), 3.0, 1.22),
        ((0.016, 0.125), armpit, 2.6, 1.10),
        (armpit, off_line(SHOULDER, ELBOW, 1.0, 0.004), 2.5, 1.05),
        (
            off_line(SHOULDER, ELBOW, 1.0, 0.004),
            off_line(ELBOW, WRIST, 0.75, -0.012),
            2.0,
            0.95,
        ),
    ];
    for side in [-1.0, 1.0] {
        for (from, to, radius, pressure) in arteries {
            world.add_vessel_segment(
                frame.point(side * from.0, from.1),
                frame.point(side * to.0, to.1),
                radius,
                pressure,
            );
        }
    }

    world.give_tissue_physical_properties(frame);
    // Tissue keeps its place on the bones of its own part of the body: the
    // flesh of an arm on the arm's bones, the chest's on the trunk's, and
    // only on bones it reaches through flesh, so the forearm's side of the
    // elbow's crease keeps its place on the forearm alone.
    let bone_homes: Vec<(Vec2, Vec2)> = world
        .bones
        .iter()
        .map(|bone| (bone.home_a, bone.home_b))
        .collect();
    world.bind_posture(|point| {
        let (u, v) = frame.body_coords(point.home);
        let bones = skeleton.bones_of(figure_part(u, v), u);
        let reached: Vec<usize> = bones
            .iter()
            .copied()
            .filter(|&bone| {
                let (a, b) = bone_homes[bone];
                through_flesh(point.home, lerp(a, b, segment_t(point.home, a, b)))
            })
            .collect();
        if reached.is_empty() {
            bones
        } else {
            reached
        }
    });
    // The body stands on the floor at its soles, and its hips are held up.
    world.set_floor_y(frame.point(0.0, 1.0).y);
    world.hold_hip(skeleton.pelvis, false);
    world.hold_hip(skeleton.pelvis, true);
    // It is built in the guard as drawn, without its weight. It settles
    // under that weight before anything happens to it, and its muscles
    // learn the weight on the way, so it comes to rest in the guard as
    // built rather than sinking at the start of play.
    let dt = world.materials.fixed_dt;
    let width = frame.origin.x * 2.0 + frame.height;
    // It takes its weight over the settle's first half second: all at once,
    // the trunk dropped onto the pelvis before the joints and muscles had
    // learned to carry it, stretching the lower back half again past its gap
    // (and on a body drawn larger, past where it tears).
    let gravity = world.materials.gravity;
    for step in 0..SETTLE_STEPS {
        let share = ((step + 1) as f64 / WEIGHT_RAMP_STEPS as f64).min(1.0);
        world.materials.gravity = gravity * share * share * (3.0 - 2.0 * share);
        world.step(dt, &InputState::default(), width);
        world.learn_the_guards_weight();
    }
    world.materials.gravity = gravity;
    world
}

/// How many steps a new body settles under its own weight, a second and a
/// half: the guard's slowest sway dies away in about a second.
const SETTLE_STEPS: usize = 90;

/// Over how many of those steps the body takes its weight, easing in.
const WEIGHT_RAMP_STEPS: usize = 30;

/// The bones the body's parts hang on, the viewer's left one first where
/// there are two.
struct Skeleton {
    skull: usize,
    spine: usize,
    pelvis: usize,
    collarbones: [usize; 2],
    upper_arms: [usize; 2],
    forearms: [usize; 2],
    hands: [usize; 2],
    thighs: [usize; 2],
    shins: [usize; 2],
    feet: [usize; 2],
}

impl Skeleton {
    /// The bones the tissue of `part` keeps its place on, on the side of the
    /// midline `u` lies on. The ribs move with the spine and are left out.
    fn bones_of(&self, part: FigurePart, u: f64) -> Vec<usize> {
        let side = usize::from(u >= 0.0);
        match part {
            FigurePart::Trunk => vec![
                self.skull,
                self.spine,
                self.pelvis,
                self.collarbones[0],
                self.collarbones[1],
            ],
            FigurePart::Arm => vec![
                self.collarbones[side],
                self.upper_arms[side],
                self.forearms[side],
                self.hands[side],
            ],
            FigurePart::Leg => vec![
                self.pelvis,
                self.thighs[side],
                self.shins[side],
                self.feet[side],
            ],
        }
    }
}

/// The bones' directions in `frame`, which steer the muscle's fibers and the
/// lines the skin is stretched along.
pub(super) fn fiber_guides(frame: BodyFrame) -> Vec<(Vec2, Vec2)> {
    fiber_guide_landmarks()
        .iter()
        .map(|&(a, b)| (frame.point(a.0, a.1), frame.point(b.0, b.1)))
        .collect()
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

/// How strong a forearm is, against `Materials::bone_fracture_impulse`.
pub(super) const FOREARM_STRENGTH: f64 = 0.72;

/// How strong each part of the skeleton is, against
/// `Materials::bone_fracture_impulse`. In fracture tests the collarbone takes
/// about half the force that breaks a forearm, and the upper arm, shin, and
/// thigh about 2.1, 2.25, and 2.5 times it (`docs/INJURY_REFERENCE.md`);
/// parts the research does not cover keep their earlier tuned strength. The
/// forearm is set 30% stronger than its share, within the spread of the
/// forearm studies, so a 10 J bat swing rarely breaks it. How hard a blow
/// presses on a bone is in `cover.rs`.
fn bone_strength(part: BonePart) -> f64 {
    const FOREARM: f64 = FOREARM_STRENGTH;
    match part {
        BonePart::Forearm => FOREARM * 1.3,
        BonePart::Collarbone => FOREARM * 0.53,
        BonePart::UpperArm => FOREARM * 2.14,
        BonePart::Shin => FOREARM * 2.25,
        BonePart::Thigh => FOREARM * 2.5,
        // The side of the skull breaks at about 5 kN under a small face
        // (Allsop et al. 1991, 5.0 kN on average for 31 heads struck at the
        // temple) and 5.2 kN under bats and other implements (AAFS 2018),
        // against the forearm's 1.37 kN.
        BonePart::Skull => FOREARM * 3.65,
        BonePart::Spine => FOREARM * 1.39,
        BonePart::Pelvis => FOREARM * 1.25,
        BonePart::Hand => FOREARM * 0.83,
        BonePart::Foot => FOREARM * 0.9,
        // Ribs have their own strengths, in `RIBS`.
        BonePart::Rib | BonePart::Other => 1.0,
    }
}

/// The joints of the frontal plane the figure moves in.
#[derive(Clone, Copy)]
enum Joint {
    Neck,
    Back,
    Collarbone,
    Rib,
    Shoulder,
    Elbow,
    Wrist,
    Hip,
    Knee,
    Ankle,
}

/// How long, in seconds, the muscles' damping is for each newton metre per
/// radian of their stiffness: 0.03-0.09 s at the arm (Gomi and Osu 1998),
/// and taken the same where it was not measured.
const DRIVE_DAMPING_TIME: f64 = 0.05;

/// The muscles across `joint` in the guard, braced for a blow, holding it
/// in the frontal plane: how stiffly, in newton metres per radian, and how
/// strongly at most, in newton metres, for a healthy man
/// (`docs/INJURY_REFERENCE.md`, Holding the guard).
fn joint_drive(joint: Joint) -> JointDrive {
    let (stiffness, max_torque) = match joint {
        // Bending the neck sideways: 36 N·m at most (Vasavada, Li, and Delp
        // 2001); braced, an estimate between the relaxed neck's 1-3 N·m/rad
        // and the tensed neck's 216 bending forward (Portero et al. 2013).
        Joint::Neck => (40.0, 36.0),
        // The trunk bending sideways: 80-172 N·m across postures, and
        // 1300-1800 N·m/rad (Gardner-Morse and Stokes 2001, 14.5-19.8 N/mm at
        // the chest).
        Joint::Back => (1500.0, 130.0),
        // The shoulder girdle lifting and dropping, an estimate: a shrug
        // holds well over the arm's weight.
        Joint::Collarbone => (100.0, 60.0),
        // The rib cage holds each rib to the spine and the breastbone, an
        // estimate.
        Joint::Rib => (30.0, 50.0),
        // Raising and lowering the arm: 61 N·m raising it (Harbo, Brincks, and
        // Andersen 2012), and 10.8 + 3.2 N·m/rad for each N·m held (Gomi and
        // Osu 1998), stiffer braced.
        Joint::Shoulder => (40.0, 61.0),
        // The elbow: 47-51 N·m (Harbo et al. 2012), 8.7 + 6.2 N·m/rad for
        // each N·m held (Gomi and Osu 1998) and 40-60 tensed both ways (Osu
        // and Gomi 1999).
        Joint::Elbow => (30.0, 47.0),
        // Tilting the fist sideways: about 10 N·m (Delp, Grierson, and
        // Buchanan 1996), 5-15 N·m/rad clenched, an estimate.
        Joint::Wrist => (10.0, 9.5),
        // The hip holding the leg out or in: about 120 N·m, and 300-400
        // N·m/rad, enough that the hips' muscles alone hold the body
        // upright sideways (Winter et al. 1998); both estimates.
        Joint::Hip => (350.0, 120.0),
        // The knee does not bend sideways much: its ligaments hold it, an
        // estimate; its extensors give 247 N·m (Harbo et al. 2012).
        Joint::Knee => (300.0, 247.0),
        // Turning the foot in and out: 26-36 N·m (Cybex norms for young men),
        // 40-80 N·m/rad (Lee et al. 2014), an estimate.
        Joint::Ankle => (60.0, 26.0),
    };
    JointDrive {
        stiffness,
        damping: stiffness * DRIVE_DAMPING_TIME,
        max_torque,
        limb: Limb::None,
        aim: 0.0,
    }
}

/// How far `joint` turns either way from the guard, in radians, on the
/// viewer's right side: its range of motion in the frontal plane (AAOS
/// normal ranges, Greene and Heckman 1994). The left side mirrors it.
fn joint_range(joint: Joint) -> (f64, f64) {
    match joint {
        // Bending the neck sideways, about 42 degrees each way (Youdas et
        // al. 1992 found 41-42 in young adults).
        Joint::Neck => (-0.75, 0.75),
        // The trunk bending sideways, about 30 degrees each way.
        Joint::Back => (-0.52, 0.52),
        // A collarbone lifts about 45 degrees at the breastbone but drops only
        // about 10.
        Joint::Collarbone => (-0.55, 0.2),
        Joint::Rib => (-0.40, 0.40),
        // From the guard the arm rises until it is overhead, about 130
        // degrees, and drops to the side and a little past.
        Joint::Shoulder => (-2.2, 1.0),
        // The elbow, bent 134 degrees in the guard, bends about 11 more to
        // its full 145 and straightens all the way.
        Joint::Elbow => (-0.19, 2.40),
        // The wrist tilts about 20 degrees one way and 30 the other.
        Joint::Wrist => (-0.45, 0.45),
        // The leg goes out 45 degrees and in 30 past the midline; the stance
        // already sets it 6 degrees out.
        Joint::Hip => (-0.79, 0.63),
        // The knee gives a few degrees sideways before its ligaments tear.
        Joint::Knee => (-0.15, 0.15),
        // The foot turns in and out, about 30 and 20 degrees.
        Joint::Ankle => (-0.45, 0.45),
    }
}

/// Joins bone `a` at `t_a` along it to bone `b` at `t_b` with the range and
/// the muscles of `joint`, which move `limb`, on the viewer's left side
/// (`left`) or right.
#[allow(clippy::too_many_arguments)]
fn add_joint(
    world: &mut World,
    a: usize,
    t_a: f64,
    b: usize,
    t_b: f64,
    joint: Joint,
    left: bool,
    limb: Limb,
) {
    let (min, max) = joint_range(joint);
    let (min, max) = if left { (-max, -min) } else { (min, max) };
    if let Some(index) = world.add_bone_joint(a, t_a, b, t_b, min, max) {
        world.drive_bone_joint(
            index,
            JointDrive {
                limb,
                ..joint_drive(joint)
            },
        );
    }
}

fn add_skeleton(world: &mut World, frame: BodyFrame, materials: Materials) -> Skeleton {
    let at = |landmark: Landmark| frame.point(landmark.0, landmark.1);
    let strength = materials.bone_fracture_impulse;
    let bone = |world: &mut World, a: Landmark, b: Landmark, radius: f64, part: BonePart| {
        let index =
            world.add_bone_segment(at(a), at(b), radius, strength * bone_strength(part), false);
        world.bones[index].part = part;
        index
    };

    // Joints never rest closer than 0.7 point spacings, so every joined bone
    // starts that far from the joint, or as far as its landmarks put it. A
    // bone built closer is shoved away as soon as the body settles: the whole
    // skeleton shifts before the first blow, and a hanging bone overshoots
    // into a subluxation while the body is still at rest.
    let joint_gap = materials.point_spacing * 0.70;
    // The point a joint's rest gap from `joint`, toward `toward`.
    let gap_from = |joint: Vec2, toward: Vec2, fallback: Vec2| {
        add(
            joint,
            scale(normalized(subtract(toward, joint), fallback), joint_gap),
        )
    };
    let down = Vec2 { x: 0.0, y: 1.0 };

    let head = bone(world, SKULL[0], SKULL[1], 8.2, BonePart::Skull);
    // The spine runs down to the sacrum, a joint's rest gap below the middle
    // of the pelvis.
    let spine_top = at(SPINE[0]);
    let spine_bottom = gap_from(at((0.0, PELVIS[0].1)), at(SPINE[1]), down);
    let spine = world.add_bone_segment_with_kind(
        spine_top,
        spine_bottom,
        7.2,
        strength * bone_strength(BonePart::Spine),
        false,
        BoneKind::Spine,
    );
    world.bones[spine].part = BonePart::Spine;
    // Each collarbone runs from the breastbone, beside the top of the spine,
    // out to its shoulder, so a broken one leaves the other shoulder be. It
    // starts a joint's rest gap off the spine's line and ends one short of
    // the shoulder.
    let collarbone = |world: &mut World, lateral: Landmark, shoulder: Landmark| {
        let medial = at((0.0, lateral.1));
        let end = gap_from(at(shoulder), at(lateral), Vec2 { x: 0.0, y: -1.0 });
        let index = world.add_bone_segment(
            gap_from(medial, end, Vec2 { x: 1.0, y: 0.0 }),
            end,
            6.2,
            strength * bone_strength(BonePart::Collarbone),
            false,
        );
        world.bones[index].part = BonePart::Collarbone;
        index
    };
    let left_collarbone = collarbone(world, SHOULDER_GIRDLE[0], LEFT_SHOULDER);
    let right_collarbone = collarbone(world, SHOULDER_GIRDLE[1], RIGHT_SHOULDER);
    let pelvis = bone(world, PELVIS[0], PELVIS[1], 6.4, BonePart::Pelvis);
    let (top_v, bottom_v) = (
        frame.body_coords(spine_top).1,
        frame.body_coords(spine_bottom).1,
    );
    let spine_t = |v: f64| ((v - top_v) / (bottom_v - top_v)).clamp(0.0, 1.0);

    let mut ribs = Vec::new();
    for (root_v, reach, scale) in RIBS {
        for side in [-1.0, 1.0] {
            // A rib's root sits a joint's rest gap off the spine.
            let rib = world.add_bone_segment_with_kind(
                gap_from(
                    frame.point(0.0, root_v),
                    frame.point(side, root_v),
                    Vec2 { x: side, y: 0.0 },
                ),
                frame.point(side * reach, root_v + RIB_DROP),
                3.4,
                strength * scale,
                false,
                BoneKind::Rib,
            );
            world.bones[rib].part = BonePart::Rib;
            ribs.push((rib, root_v));
        }
    }
    let below_joint =
        |world: &mut World, joint: Landmark, tip: Landmark, radius: f64, part: BonePart| {
            let end = at(tip);
            let index = world.add_bone_segment(
                gap_from(at(joint), end, down),
                end,
                radius,
                strength * bone_strength(part),
                false,
            );
            world.bones[index].part = part;
            index
        };
    let left_upper_arm = bone(world, LEFT_SHOULDER, LEFT_ELBOW, 5.7, BonePart::UpperArm);
    let left_forearm = below_joint(world, LEFT_ELBOW, LEFT_WRIST, 4.8, BonePart::Forearm);
    let right_upper_arm = bone(world, RIGHT_SHOULDER, RIGHT_ELBOW, 5.7, BonePart::UpperArm);
    let right_forearm = below_joint(world, RIGHT_ELBOW, RIGHT_WRIST, 4.8, BonePart::Forearm);
    let left_thigh = bone(world, LEFT_HIP, LEFT_KNEE, 6.4, BonePart::Thigh);
    let left_shin = below_joint(world, LEFT_KNEE, LEFT_ANKLE, 5.3, BonePart::Shin);
    let right_thigh = bone(world, RIGHT_HIP, RIGHT_KNEE, 6.4, BonePart::Thigh);
    let right_shin = below_joint(world, RIGHT_KNEE, RIGHT_ANKLE, 5.3, BonePart::Shin);
    let left_hand = below_joint(world, LEFT_WRIST, LEFT_KNUCKLES, 3.6, BonePart::Hand);
    let right_hand = below_joint(world, RIGHT_WRIST, RIGHT_KNUCKLES, 3.6, BonePart::Hand);
    let left_foot = below_joint(world, LEFT_ANKLE, LEFT_TOES, 4.0, BonePart::Foot);
    let right_foot = below_joint(world, RIGHT_ANKLE, RIGHT_TOES, 4.0, BonePart::Foot);

    // Each limb's bones, the shoulder girdle with its arm.
    let limbs = [
        (
            Limb::LeftArm,
            [left_collarbone, left_upper_arm, left_forearm, left_hand],
        ),
        (
            Limb::RightArm,
            [right_collarbone, right_upper_arm, right_forearm, right_hand],
        ),
        (Limb::LeftLeg, [left_thigh, left_shin, left_foot, left_foot]),
        (
            Limb::RightLeg,
            [right_thigh, right_shin, right_foot, right_foot],
        ),
    ];
    for (limb, bones) in limbs {
        for bone in bones {
            world.bones[bone].limb = limb;
        }
    }
    let none = Limb::None;
    add_joint(world, head, 1.0, spine, 0.0, Joint::Neck, false, none);
    for (collarbone, left, limb) in [
        (left_collarbone, true, Limb::LeftArm),
        (right_collarbone, false, Limb::RightArm),
    ] {
        let t = spine_t(SHOULDER_GIRDLE[0].1);
        add_joint(
            world,
            spine,
            t,
            collarbone,
            0.0,
            Joint::Collarbone,
            left,
            limb,
        );
    }
    add_joint(world, spine, 1.0, pelvis, 0.5, Joint::Back, false, none);
    for (rib, root_v) in ribs {
        add_joint(
            world,
            spine,
            spine_t(root_v),
            rib,
            0.0,
            Joint::Rib,
            false,
            none,
        );
    }
    let along = |segment: [Landmark; 2], u: f64| {
        ((u - segment[0].0) / (segment[1].0 - segment[0].0)).clamp(0.0, 1.0)
    };
    let arms = [
        (
            true,
            Limb::LeftArm,
            left_collarbone,
            left_upper_arm,
            left_forearm,
            left_hand,
        ),
        (
            false,
            Limb::RightArm,
            right_collarbone,
            right_upper_arm,
            right_forearm,
            right_hand,
        ),
    ];
    for (left, limb, collarbone, upper_arm, forearm, hand) in arms {
        add_joint(
            world,
            collarbone,
            1.0,
            upper_arm,
            0.0,
            Joint::Shoulder,
            left,
            limb,
        );
        add_joint(
            world,
            upper_arm,
            1.0,
            forearm,
            0.0,
            Joint::Elbow,
            left,
            limb,
        );
        add_joint(world, forearm, 1.0, hand, 0.0, Joint::Wrist, left, limb);
    }
    let legs = [
        (
            true,
            Limb::LeftLeg,
            LEFT_HIP,
            left_thigh,
            left_shin,
            left_foot,
        ),
        (
            false,
            Limb::RightLeg,
            RIGHT_HIP,
            right_thigh,
            right_shin,
            right_foot,
        ),
    ];
    for (left, limb, hip, thigh, shin, foot) in legs {
        let t = along(PELVIS, hip.0);
        add_joint(world, pelvis, t, thigh, 0.0, Joint::Hip, left, limb);
        add_joint(world, thigh, 1.0, shin, 0.0, Joint::Knee, left, limb);
        add_joint(world, shin, 1.0, foot, 0.0, Joint::Ankle, left, limb);
    }
    Skeleton {
        skull: head,
        spine,
        pelvis,
        collarbones: [left_collarbone, right_collarbone],
        upper_arms: [left_upper_arm, right_upper_arm],
        forearms: [left_forearm, right_forearm],
        hands: [left_hand, right_hand],
        thighs: [left_thigh, right_thigh],
        shins: [left_shin, right_shin],
        feet: [left_foot, right_foot],
    }
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
        // Along the elbow's narrow crease the outline's points lie closer
        // than its width, so the mesh follows the slit.
        let on_crease = |p: P2| {
            let (u, v) = frame.body_coords(Vec2 { x: p.0, y: p.1 });
            on_elbow_crease(u, v, -inset / frame.height)
        };
        let crease_spacing = CREASE_POINT_SPACING * frame.height;
        let resampled =
            resample_loop_refined(&world_outline, spacing * BOUNDARY_SPACING_SCALE, |a, b| {
                if on_crease(a) || on_crease(b) {
                    (mesh_distance(a, b) / crease_spacing).ceil() as usize
                } else {
                    1
                }
            });
        let count = resampled.len();
        candidates.extend(resampled);
        next_on_outline.extend((0..count).map(|k| first + (k + 1) % count));
        outline_ranges.push(first..first + count);
    }
    let follows_outline = |a: usize, b: usize| {
        (a < next_on_outline.len() && next_on_outline[a] == b)
            || (b < next_on_outline.len() && next_on_outline[b] == a)
    };
    let min = frame.point(-FIELD_HALF_WIDTH, -0.01);
    let max = frame.point(FIELD_HALF_WIDTH, 1.01);
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
            let index = world.add_point(position, layer, false);
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
pub(super) fn fiber_direction(point: Vec2, guides: &[(Vec2, Vec2)]) -> Vec2 {
    let mut best = (f64::INFINITY, Vec2 { x: 0.0, y: 1.0 });
    for &(a, b) in guides {
        let d = distance_to_segment(point, a, b);
        if d < best.0 {
            best = (d, normalized(subtract(b, a), Vec2 { x: 0.0, y: 1.0 }));
        }
    }
    best.1
}

/// A spring of `kind` between `a` and `b`; how stiff it is comes from the
/// tissue it is in (`tissue.rs`).
fn push_spring(
    world: &mut World,
    a: usize,
    b: usize,
    layer: TissueLayer,
    (kind, tear_stretch, tear_impulse): (SpringKind, f64, f64),
) -> usize {
    let rest = distance(world.points[a].position, world.points[b].position);
    world.push_spring(
        Spring {
            a,
            b,
            rest,
            kind,
            layer,
            ..Spring::default()
        },
        SpringWear {
            rest_reference: rest,
            tear_stretch,
            tear_impulse,
            fiber: matches!(kind, SpringKind::Fiber | SpringKind::LongFiber),
            ..SpringWear::default()
        },
    )
}

/// One spring per triangle edge. Muscle edges are fibers, cross fibers, or
/// between, by their angle to the nearest bone.
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
                (SpringKind::Skin, m.skin_tear_stretch, m.skin_tear_impulse)
            } else {
                let pa = world.points[a].position;
                let pb = world.points[b].position;
                let along = normalized(subtract(pb, pa), Vec2 { x: 1.0, y: 0.0 });
                let alignment = dot(along, fiber_direction(midpoint(pa, pb), guides)).abs();
                if alignment >= FIBER_ALIGNMENT {
                    (
                        SpringKind::Fiber,
                        m.muscle_tear_stretch,
                        m.muscle_tear_impulse,
                    )
                } else if alignment <= CROSS_ALIGNMENT {
                    (
                        SpringKind::Cross,
                        m.muscle_tear_stretch,
                        m.muscle_tear_impulse,
                    )
                } else {
                    (
                        SpringKind::Shear,
                        m.muscle_tear_stretch * 1.12,
                        m.muscle_tear_impulse,
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
    through_flesh: &dyn Fn(Vec2, Vec2) -> bool,
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
                if alignment >= 0.9
                    && best.is_none_or(|(score, _)| alignment > score)
                    && through_flesh(position, world.points[r].position)
                {
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
                        SpringKind::LongFiber,
                        m.muscle_tear_stretch * 1.05,
                        m.muscle_tear_impulse,
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
            stiffness: 0.0,
            layer,
            impulse: 0.0,
        });
    }
}

/// Ties each skin point to its nearest muscle points underneath.
fn attach_skin_to_muscle(
    world: &mut World,
    skin_points: &[usize],
    muscle_points: &[usize],
    through_flesh: &dyn Fn(Vec2, Vec2) -> bool,
) {
    let spacing = world.materials.point_spacing;
    let max_distance = spacing * 3.15;
    for &skin_point in skin_points {
        let skin_position = world.points[skin_point].position;
        let mut nearest = [(usize::MAX, f64::INFINITY); SKIN_ATTACHMENT_CANDIDATES];
        for &muscle_point in muscle_points {
            let d = distance(skin_position, world.points[muscle_point].position);
            if d > max_distance
                || !through_flesh(skin_position, world.points[muscle_point].position)
            {
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
                    stiffness: 0.0,
                    impulse: 0.0,
                });
            }
        }
    }
}

/// Whether the straight way from `from` to `to` runs through the body's
/// flesh, within a pixel of its outline: flesh is tied to a bone or to other
/// flesh through flesh, not across a gap such as the inner elbow's crease.
fn reaches_through_flesh(field: &SilhouetteField, frame: BodyFrame, from: Vec2, to: Vec2) -> bool {
    let steps = (distance(from, to) / 2.0).ceil().max(1.0) as usize;
    (1..steps).all(|step| {
        let (u, v) = frame.body_coords(lerp(from, to, step as f64 / steps as f64));
        field.distance(u, v) * frame.height < 1.0
    })
}
