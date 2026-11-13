//! A fighter's body, seen from the side and built to move: the figure of
//! `crate::profile` in the same skin, fat, muscle, and bone as the sandbox's
//! front-view body (`body.rs`), under the same injury rules.
//!
//! The front-view body is one sheet of flesh, which suits a body that stands
//! to be struck but not one that fights: nothing can pass in front of
//! anything else, and a joint that turns far stretches or bunches the flesh
//! across it. A fighter is made of sheets lying over one another instead,
//! the farthest from the viewer first (`Point::sheet`): each part of a limb
//! has flesh of its own on its own bone, rounded at both ends and
//! overlapping the next part at the joint, and the trunk with the head is
//! one sheet. The sheets of one body pass over each other freely; another
//! body's skin and limbs press on every one of them (`outline.rs`,
//! `clash.rs`).
//!
//! Its skeleton turns in the plane a fight moves in. Every joint's range and
//! its muscles' stiffness and strength are those of bending forward and
//! back, not sideways as the front-view body's are. The shoulders and hips
//! sit on the trunk beside the spine's line (`BoneJoint::seat`), and a
//! shoulder slides forward and back across the chest as the trunk turns
//! toward or away from the viewer, which a side view shows only so.

use super::body::{
    add_layer_springs, add_layer_triangles, add_long_fiber_springs, attach_skin_to_muscle,
    bone_strength, build_sheet_mesh, reaches_through_flesh, DRIVE_DAMPING_TIME, MUSCLE_INSET_SCALE,
};
use super::soft::KG_PER_MASS;
use super::*;
use crate::profile::{self, Hinge, Joints, Part, Place, Side};

/// How far each rib's root lies along the chest's spine, from its lower end,
/// how far the rib reaches forward of it, in body heights, and how strong it
/// is against `Materials::bone_fracture_impulse` (as the front-view body's
/// four ribs a side).
const RIBS: [(f64, f64, f64); 4] = [
    (0.22, 0.086, 0.62),
    (0.40, 0.092, 0.66),
    (0.58, 0.094, 0.70),
    (0.76, 0.084, 0.74),
];
/// How far a rib's tip lies below its root, in body heights.
const RIB_DROP: f64 = 0.034;

/// What each sheet of flesh weighs with its bones, as a share of the body
/// (de Leva 1996, men): the head and trunk, the upper arm, the forearm with
/// the hand, the thigh, the shin, and the foot.
fn part_mass_share(part: Part) -> f64 {
    match part {
        Part::Trunk => 0.0694 + 0.4346,
        Part::UpperArm(_) => 0.0271,
        Part::Forearm(_) => 0.0162 + 0.0061,
        Part::Thigh(_) => 0.1416,
        Part::Shin(_) => 0.0433,
        Part::Foot(_) => 0.0137,
    }
}

/// How far each joint turns either way from standing upright with the arms
/// hanging, in radians, counterclockwise as seen with the front to the right
/// (bending a knee turns the shin clockwise; raising an arm forward turns it
/// counterclockwise): its range of motion forward and back (AAOS normal
/// ranges, Greene and Heckman 1994; the spine's shared between its joints).
fn hinge_range(hinge: Hinge) -> (f64, f64) {
    match hinge {
        // The lower back bends forward about 60 degrees and back 25, the
        // lumbar spine's and the chest's together.
        Hinge::Back => (-0.60, 0.25),
        Hinge::Mid => (-0.50, 0.20),
        // The neck bends forward about 50 degrees and back 60, shared
        // between its foot and the head's nod.
        Hinge::Neck => (-0.50, 0.60),
        Hinge::Head => (-0.45, 0.60),
        // The arm swings forward and up overhead, about 175 degrees, and
        // back about 55.
        Hinge::Shoulder(_) => (-0.95, 3.05),
        // The elbow bends 145 degrees and straightens.
        Hinge::Elbow(_) => (-0.05, 2.53),
        Hinge::Wrist(_) => (-0.80, 0.80),
        // The thigh comes forward 120 degrees and goes back 25.
        Hinge::Hip(_) => (-0.45, 2.10),
        // The knee bends 140 degrees and straightens.
        Hinge::Knee(_) => (-2.44, 0.03),
        // The foot points down 50 degrees and pulls up about 23.
        Hinge::Ankle(_) => (-0.87, 0.40),
    }
}

/// The muscles across `hinge`, bending it forward and back: how stiffly they
/// hold it braced, in newton metres per radian, and how strongly at most, in
/// newton metres, for a healthy man. Estimates within what isokinetic and
/// isometric studies report (Harbo, Brincks, and Andersen 2012 for the limbs'
/// joints; Vasavada, Li, and Delp 2001 for the neck; the trunk's extensors
/// give about 250 N·m and its flexors about 150); `docs/INJURY_REFERENCE.md`
/// lists them with the front-view body's.
fn hinge_drive(hinge: Hinge) -> (f64, f64) {
    let strength = match hinge {
        Hinge::Back | Hinge::Mid => 300.0,
        Hinge::Neck | Hinge::Head => 50.0,
        Hinge::Shoulder(_) => 90.0,
        Hinge::Elbow(_) => 65.0,
        Hinge::Wrist(_) => 14.0,
        Hinge::Hip(_) => 200.0,
        Hinge::Knee(_) => 200.0,
        Hinge::Ankle(_) => 110.0,
    };
    // A joint's stiffness grows with the torque its muscles hold (at the
    // elbow 8.7 N·m/rad and 6.2 more for each N·m held, Gomi and Osu 1998):
    // a fighter's joints are tensed to move fast, as stiff as its muscles
    // pulling their hardest over a quarter of a radian, which is what the
    // elbow's figures give at full strength. The trunk is stiffer still
    // (Gardner-Morse and Stokes 2001).
    let stiffness = match hinge {
        Hinge::Back | Hinge::Mid => 1500.0,
        _ => strength / FULL_PULL,
    };
    (stiffness, strength)
}

/// How far from its aim a fighter's joint is when its muscles pull their
/// hardest, in radians (`hinge_drive`; `Materials::punch_full_pull` is the
/// front-view body's).
const FULL_PULL: f64 = 0.25;

/// The bones and joints of a side-view fighter, by their places in the
/// world's lists, and where it was built: what the moves that drive it need
/// to find (`mover.rs`). Sides come the lead side's first.
#[derive(Clone, Debug, PartialEq)]
pub struct Rig {
    pub(super) pelvis: usize,
    pub(super) lumbar: usize,
    pub(super) thoracic: usize,
    pub(super) cervical: usize,
    pub(super) skull: usize,
    pub(super) upper_arms: [usize; 2],
    pub(super) forearms: [usize; 2],
    pub(super) hands: [usize; 2],
    pub(super) thighs: [usize; 2],
    pub(super) shins: [usize; 2],
    pub(super) feet: [usize; 2],
    /// Its joints, in the order of [`Hinge::ALL`].
    pub(super) hinges: [usize; 16],
    /// Where each shoulder's joint sat on the chest as built
    /// (`BoneJoint::seat`), and how far that seat moves, in pixels along the
    /// chest's spine and across it, for each pixel the shoulder slides
    /// forward across the chest.
    pub(super) shoulder_seats: [Vec2; 2],
    pub(super) shoulder_slide: Vec2,
    /// Where on the floor it was built to stand, under its hips, and how
    /// tall it is, in pixels.
    pub(super) stand: Vec2,
    pub(super) height: f64,
}

impl Rig {
    /// The joint `hinge` names.
    pub(super) fn hinge(&self, hinge: Hinge) -> usize {
        let place = Hinge::ALL
            .iter()
            .position(|&other| other == hinge)
            .unwrap_or(0);
        self.hinges[place]
    }

    /// The rig moved to follow `bones` bones and `joints` joints already in
    /// a world it joins (`World::add_body`).
    pub(super) fn move_indices(&mut self, bones: usize, joints: usize) {
        for bone in [
            &mut self.pelvis,
            &mut self.lumbar,
            &mut self.thoracic,
            &mut self.cervical,
            &mut self.skull,
        ] {
            *bone += bones;
        }
        for pair in [
            &mut self.upper_arms,
            &mut self.forearms,
            &mut self.hands,
            &mut self.thighs,
            &mut self.shins,
            &mut self.feet,
        ] {
            for bone in pair.iter_mut() {
                *bone += bones;
            }
        }
        for joint in &mut self.hinges {
            *joint += joints;
        }
    }

    /// The rig of the body mirrored left to right about `middle`
    /// (`World::mirrored`): what is measured across a bone turns the other
    /// way.
    pub(super) fn mirror(&mut self, middle: f64) {
        self.stand.x = 2.0 * middle - self.stand.x;
        for seat in &mut self.shoulder_seats {
            seat.y = -seat.y;
        }
        self.shoulder_slide.y = -self.shoulder_slide.y;
    }
}

/// The limb a side's arm or leg is: the lead side's are the body's left.
fn arm_of(side: Side) -> Limb {
    match side {
        Side::Lead => Limb::LeftArm,
        Side::Rear => Limb::RightArm,
    }
}

fn leg_of(side: Side) -> Limb {
    match side {
        Side::Lead => Limb::LeftLeg,
        Side::Rear => Limb::RightLeg,
    }
}

/// The lines of the bones of `part` with the figure's joints at `joints`,
/// which steer its muscle's fibers and the lines its skin is stretched
/// along.
fn guide_lines(part: Part, joints: &Joints) -> Vec<(Place, Place)> {
    match part {
        Part::UpperArm(side) => {
            let s = side.index();
            vec![(joints.shoulder[s], joints.elbow[s])]
        }
        Part::Forearm(side) => {
            let s = side.index();
            vec![
                (joints.elbow[s], joints.wrist[s]),
                (joints.wrist[s], joints.knuckles[s]),
            ]
        }
        Part::Thigh(side) => {
            let s = side.index();
            vec![(joints.hip[s], joints.knee[s])]
        }
        Part::Shin(side) => {
            let s = side.index();
            vec![(joints.knee[s], joints.ankle[s])]
        }
        Part::Foot(side) => {
            let s = side.index();
            vec![(joints.heel[s], joints.ball[s])]
        }
        Part::Trunk => vec![
            (joints.sacrum, joints.hips),
            (joints.sacrum, joints.waist),
            (joints.waist, joints.neck),
            (joints.neck, joints.skull_base),
            (joints.skull_base, joints.skull_top),
        ],
    }
}

/// A side-view fighter facing the right of the window, standing in its
/// stance where `frame` puts it: `frame.origin.x` under its hips, the floor
/// one body height below `frame.origin.y`.
pub fn create_side_fighter_in(frame: BodyFrame, materials: Materials) -> World {
    let joints = Joints::of(&profile::stance());
    let at = |place: Place| frame.point(place.0, 1.0 - place.1);
    let spacing = materials.point_spacing;
    let mut world = World::new(materials);
    world.body_height = frame.height;

    // The flesh, sheet by sheet from the farthest to the nearest.
    let mut fields = Vec::new();
    let mut guides: Vec<Vec<(Vec2, Vec2)>> = Vec::new();
    let mut muscle_points: Vec<Vec<usize>> = Vec::new();
    for (sheet, &part) in Part::FAR_TO_NEAR.iter().enumerate() {
        let field = profile::part_field(part, &joints);
        let lattice = field.bounds();
        let lines: Vec<(Vec2, Vec2)> = guide_lines(part, &joints)
            .into_iter()
            .map(|(from, to)| (at(from), at(to)))
            .collect();
        let first = world.points.len();
        let skin = build_sheet_mesh(
            &mut world,
            frame,
            &field,
            TissueLayer::Skin,
            0.0,
            lattice,
            false,
        );
        let muscle = build_sheet_mesh(
            &mut world,
            frame,
            &field,
            TissueLayer::Muscle,
            spacing * MUSCLE_INSET_SCALE,
            lattice,
            false,
        );
        for point in &mut world.points[first..] {
            point.sheet = sheet as u8;
        }
        let through_flesh = |from: Vec2, to: Vec2| reaches_through_flesh(&field, frame, from, to);
        let skin_edges = add_layer_springs(&mut world, &skin, TissueLayer::Skin, &lines);
        let muscle_edges = add_layer_springs(&mut world, &muscle, TissueLayer::Muscle, &lines);
        // No midline to mirror the long fibers about: every guide lies to
        // one side of a line far to the left.
        add_long_fiber_springs(
            &mut world,
            &muscle,
            &muscle_edges,
            &lines,
            -1.0e9,
            &through_flesh,
        );
        add_layer_triangles(&mut world, &skin, &skin_edges, TissueLayer::Skin);
        add_layer_triangles(&mut world, &muscle, &muscle_edges, TissueLayer::Muscle);
        for outline in skin.outlines.iter().chain(&muscle.outlines) {
            world.add_outline_loop(outline.clone());
        }
        attach_skin_to_muscle(&mut world, &skin.points, &muscle.points, &through_flesh);
        muscle_points.push(muscle.points);
        guides.push(lines);
        fields.push(field);
    }

    let (rig, sheet_bones, sheet_of_bone) = add_side_skeleton(&mut world, frame, &joints);

    // Each point of muscle is tied to the nearest bone of its own sheet that
    // it reaches through flesh.
    for (sheet, points) in muscle_points.iter().enumerate() {
        for &point in points {
            let position = world.points[point].position;
            let mut nearest = (spacing * 2.7, usize::MAX, 0.0);
            for (bone, &of) in sheet_of_bone.iter().enumerate() {
                if of != sheet {
                    continue;
                }
                let segment = world.bones[bone];
                let t = segment_t(position, segment.a, segment.b);
                let on_bone = bone_point(segment, t);
                let gap = distance(position, on_bone);
                if gap < nearest.0
                    && reaches_through_flesh(&fields[sheet], frame, position, on_bone)
                {
                    nearest = (gap, bone, t);
                }
            }
            if nearest.1 != usize::MAX {
                world.add_bone_attachment(point, nearest.1, nearest.2);
            }
        }
    }
    // What a blow meets over each bone.
    cover::measure_side_bone_covers(&mut world, frame, &sheet_of_bone, &fields);

    // Seen from the side a limb is as deep as it is wide, the trunk is wider
    // from side to side than it is deep (32.6 cm against 23.8 at the navel,
    // ANSUR II), and the head narrower than it is long.
    let trunk_sheet = Part::FAR_TO_NEAR
        .iter()
        .position(|&part| part == Part::Trunk)
        .unwrap_or(0);
    let trunk_lines = guides[trunk_sheet].clone();
    let depth_ratio = move |point: &Point| {
        if point.sheet as usize != trunk_sheet {
            return 1.0;
        }
        let nearest = (0..trunk_lines.len())
            .min_by(|&p, &q| {
                let gap = |index: usize| {
                    distance_to_segment(point.home, trunk_lines[index].0, trunk_lines[index].1)
                };
                gap(p).total_cmp(&gap(q))
            })
            .unwrap_or(0);
        match nearest {
            4 => 0.80,
            3 => 1.0,
            _ => 1.37,
        }
    };
    world.give_tissue_properties(&guides, &depth_ratio);
    weigh_parts(&mut world, &rig, &sheet_of_bone);

    // Tissue keeps its place on the bones of its own sheet.
    world.bind_posture(|point| {
        sheet_bones
            .get(point.sheet as usize)
            .cloned()
            .unwrap_or_default()
    });
    world.measure_bone_flesh();
    world.bodies[0].fighter = Some(rig.clone());

    // It stands on the floor at its soles, its hips held up and its feet
    // where they stand, and settles under its weight as its muscles learn
    // to carry it, so that it rests in its stance as built.
    world.set_floor_y(frame.point(0.0, 1.0).y);
    world.hold_hip(rig.pelvis, false);
    world.hold_hip(rig.pelvis, true);
    for foot in rig.feet {
        world.plant_foot(foot);
    }
    world.carry_fighter_hips(0);
    let dt = world.materials.fixed_dt;
    let width = frame.origin.x * 2.0 + frame.height;
    let gravity = world.materials.gravity;
    for step in 0..SETTLE_STEPS {
        let share = ((step + 1) as f64 / WEIGHT_RAMP_STEPS as f64).min(1.0);
        world.materials.gravity = gravity * share * share * (3.0 - 2.0 * share);
        world.step(dt, &InputState::default(), width);
        world.learn_the_guards_weight();
    }
    world.materials.gravity = gravity;
    world.give_mover(0);
    world
}

/// How many steps a new fighter settles under its own weight, its muscles
/// learning to carry it all the while, and over how many of those it takes
/// the weight, as the front-view body does (`body.rs`). It has no mirror
/// image of itself to be kept even with, so it need not rest unlearning
/// afterwards.
const SETTLE_STEPS: usize = 180;
const WEIGHT_RAMP_STEPS: usize = 30;

/// Builds the fighter's bones and joints on the figure's joints in `frame`,
/// and returns its rig, the bones each sheet's flesh keeps its place on, and
/// each bone's sheet.
///
/// Every bone of a chain runs from the point its own joint turns about to a
/// joint's rest gap short of the next such point, on its own line: the next
/// bone then starts in line with it however the joint is turned. The
/// front-view body's forearm starts a gap down its own line from the elbow
/// instead, which is inside the bend of its guard, so with the arm straight
/// the forearm lies beside the upper arm's line.
fn add_side_skeleton(
    world: &mut World,
    frame: BodyFrame,
    joints: &Joints,
) -> (Rig, Vec<Vec<usize>>, Vec<usize>) {
    let at = |place: Place| frame.point(place.0, 1.0 - place.1);
    let materials = world.materials;
    let gap = materials.point_spacing * 0.70;
    let strength = materials.bone_fracture_impulse;
    let up = Vec2 { x: 0.0, y: -1.0 };
    let toward = |from: Vec2, to: Vec2| normalized(subtract(to, from), up);
    // A joint's rest gap short of `to`, coming from `from`.
    let short_of = |from: Vec2, to: Vec2| subtract(to, scale(toward(from, to), gap));
    let sheet_index = |part: Part| {
        Part::FAR_TO_NEAR
            .iter()
            .position(|&other| other == part)
            .unwrap_or(0)
    };
    let mut sheet_of_bone: Vec<usize> = Vec::new();
    let mut add_bone = |world: &mut World,
                        (a, b): (Vec2, Vec2),
                        radius: f64,
                        part: BonePart,
                        strong: f64,
                        limb: Limb,
                        sheet: Part| {
        let index = world.add_bone_segment(a, b, radius, strength * strong, false);
        world.bones[index].part = part;
        world.bones[index].limb = limb;
        world.bones[index].sheet = sheet_index(sheet) as u8;
        sheet_of_bone.push(sheet_index(sheet));
        index
    };

    let (hips, sacrum, waist, neck, skull_base, skull_top) = (
        at(joints.hips),
        at(joints.sacrum),
        at(joints.waist),
        at(joints.neck),
        at(joints.skull_base),
        at(joints.skull_top),
    );
    let trunk = Part::Trunk;
    let none = Limb::None;
    let spine = bone_strength(BonePart::Spine);
    let pelvis = add_bone(
        world,
        (short_of(hips, sacrum), hips),
        6.4,
        BonePart::Pelvis,
        bone_strength(BonePart::Pelvis),
        none,
        trunk,
    );
    let lumbar = add_bone(
        world,
        (sacrum, short_of(sacrum, waist)),
        7.2,
        BonePart::Spine,
        spine,
        none,
        trunk,
    );
    let thoracic = add_bone(
        world,
        (waist, short_of(waist, neck)),
        7.2,
        BonePart::Spine,
        spine,
        none,
        trunk,
    );
    let cervical = add_bone(
        world,
        (neck, short_of(neck, skull_base)),
        5.5,
        BonePart::Spine,
        spine,
        none,
        trunk,
    );
    let skull = add_bone(
        world,
        (skull_base, skull_top),
        8.2,
        BonePart::Skull,
        bone_strength(BonePart::Skull),
        none,
        trunk,
    );
    let mut ribs = Vec::new();
    for (root_t, reach, strong) in RIBS {
        let chest = world.bones[thoracic];
        let root = lerp(chest.a, chest.b, root_t);
        let along = profile::leaned((reach, -RIB_DROP), joints.leans[2]);
        let along = Vec2 {
            x: along.0 * frame.height,
            y: -along.1 * frame.height,
        };
        let tip = add(root, along);
        let rib = add_bone(
            world,
            (add(root, scale(toward(root, tip), gap)), tip),
            3.4,
            BonePart::Rib,
            strong,
            none,
            trunk,
        );
        world.bones[rib].kind = BoneKind::Rib;
        ribs.push((rib, root_t));
    }

    let mut upper_arms = [0; 2];
    let mut forearms = [0; 2];
    let mut hands = [0; 2];
    let mut thighs = [0; 2];
    let mut shins = [0; 2];
    let mut feet = [0; 2];
    for side in Side::BOTH {
        let s = side.index();
        let (shoulder, elbow, wrist, knuckles) = (
            at(joints.shoulder[s]),
            at(joints.elbow[s]),
            at(joints.wrist[s]),
            at(joints.knuckles[s]),
        );
        let arm = arm_of(side);
        upper_arms[s] = add_bone(
            world,
            (shoulder, short_of(shoulder, elbow)),
            5.7,
            BonePart::UpperArm,
            bone_strength(BonePart::UpperArm),
            arm,
            Part::UpperArm(side),
        );
        forearms[s] = add_bone(
            world,
            (elbow, short_of(elbow, wrist)),
            4.8,
            BonePart::Forearm,
            bone_strength(BonePart::Forearm),
            arm,
            Part::Forearm(side),
        );
        hands[s] = add_bone(
            world,
            (wrist, knuckles),
            3.6,
            BonePart::Hand,
            bone_strength(BonePart::Hand),
            arm,
            Part::Forearm(side),
        );
        let (hip, knee, ankle) = (at(joints.hip[s]), at(joints.knee[s]), at(joints.ankle[s]));
        let leg = leg_of(side);
        thighs[s] = add_bone(
            world,
            (hip, short_of(hip, knee)),
            6.4,
            BonePart::Thigh,
            bone_strength(BonePart::Thigh),
            leg,
            Part::Thigh(side),
        );
        shins[s] = add_bone(
            world,
            (knee, short_of(knee, ankle)),
            5.3,
            BonePart::Shin,
            bone_strength(BonePart::Shin),
            leg,
            Part::Shin(side),
        );
        feet[s] = add_bone(
            world,
            (at(joints.heel[s]), at(joints.ball[s])),
            4.0,
            BonePart::Foot,
            bone_strength(BonePart::Foot),
            leg,
            Part::Foot(side),
        );
    }

    // The joints. Each turns within its range from upright, less how far
    // the stance already turns it; the window's angles run clockwise where
    // the figure's run counterclockwise, the fighter facing right.
    let built = joints.directions();
    let straight = Joints::of(&profile::upright()).directions();
    let range_of = |hinge: Hinge| {
        let turned = wrap_angle(built.turn(hinge) - straight.turn(hinge));
        let (low, high) = hinge_range(hinge);
        (-(high - turned), -(low - turned))
    };
    let drive_of = |hinge: Hinge, limb: Limb| {
        let (stiffness, max_torque) = hinge_drive(hinge);
        JointDrive {
            stiffness,
            damping: stiffness * DRIVE_DAMPING_TIME,
            max_torque,
            limb,
            aim: 0.0,
            swing: 0.0,
            swing_speed: 0.0,
        }
    };
    let join = |world: &mut World,
                (a, t_a): (usize, f64),
                (b, t_b): (usize, f64),
                hinge: Hinge,
                limb: Limb| {
        let (min, max) = range_of(hinge);
        let index = world
            .add_bone_joint(a, t_a, b, t_b, min, max)
            .expect("the fighter's bones can be joined");
        world.drive_bone_joint(index, drive_of(hinge, limb));
        index
    };
    // A joint seated off its first bone's line, a rest gap before the point
    // `pivot` it turns about, along that bone.
    let seat = |world: &mut World, index: usize, pivot: Vec2| {
        let joint = world.bone_joints[index];
        let first = world.bones[joint.a];
        let along = toward(first.a, first.b);
        let across = Vec2 {
            x: -along.y,
            y: along.x,
        };
        let from = subtract(
            subtract(pivot, scale(along, gap)),
            lerp(first.a, first.b, joint.t_a),
        );
        let joint = &mut world.bone_joints[index];
        joint.seat = Vec2 {
            x: dot(from, along),
            y: dot(from, across),
        };
        joint.rest = gap;
        joint.hold = Vec2 { x: gap, y: 0.0 };
    };

    let mut hinges = [0; 16];
    let mut put = |hinge: Hinge, index: usize| {
        if let Some(place) = Hinge::ALL.iter().position(|&other| other == hinge) {
            hinges[place] = index;
        }
    };
    put(
        Hinge::Back,
        join(world, (pelvis, 0.0), (lumbar, 0.0), Hinge::Back, none),
    );
    put(
        Hinge::Mid,
        join(world, (lumbar, 1.0), (thoracic, 0.0), Hinge::Mid, none),
    );
    put(
        Hinge::Neck,
        join(world, (thoracic, 1.0), (cervical, 0.0), Hinge::Neck, none),
    );
    put(
        Hinge::Head,
        join(world, (cervical, 1.0), (skull, 0.0), Hinge::Head, none),
    );
    for (rib, root_t) in ribs {
        if let Some(index) = world.add_bone_joint(thoracic, root_t, rib, 0.0, -0.40, 0.40) {
            world.drive_bone_joint(
                index,
                JointDrive {
                    stiffness: 30.0,
                    damping: 30.0 * DRIVE_DAMPING_TIME,
                    max_torque: 50.0,
                    limb: none,
                    aim: 0.0,
                    swing: 0.0,
                    swing_speed: 0.0,
                },
            );
        }
    }
    // Where a shoulder sits along the chest's spine with the trunk square to
    // the viewer, and how its seat moves as it slides across the chest.
    let chest = world.bones[thoracic];
    let chest_along = toward(chest.a, chest.b);
    let chest_across = Vec2 {
        x: -chest_along.y,
        y: chest_along.x,
    };
    let shoulder_t = segment_t(at(joints.square_shoulder()), chest.a, chest.b);
    let forward = joints.chest_forward();
    let forward = Vec2 {
        x: forward.0,
        y: -forward.1,
    };
    let shoulder_slide = Vec2 {
        x: dot(forward, chest_along),
        y: dot(forward, chest_across),
    };
    let mut shoulder_seats = [Vec2::default(); 2];
    for side in Side::BOTH {
        let s = side.index();
        let (arm, leg) = (arm_of(side), leg_of(side));
        let shoulder = join(
            world,
            (thoracic, shoulder_t),
            (upper_arms[s], 0.0),
            Hinge::Shoulder(side),
            arm,
        );
        seat(world, shoulder, at(joints.shoulder[s]));
        shoulder_seats[s] = world.bone_joints[shoulder].seat;
        put(Hinge::Shoulder(side), shoulder);
        put(
            Hinge::Elbow(side),
            join(
                world,
                (upper_arms[s], 1.0),
                (forearms[s], 0.0),
                Hinge::Elbow(side),
                arm,
            ),
        );
        put(
            Hinge::Wrist(side),
            join(
                world,
                (forearms[s], 1.0),
                (hands[s], 0.0),
                Hinge::Wrist(side),
                arm,
            ),
        );
        let hip = join(
            world,
            (pelvis, 1.0),
            (thighs[s], 0.0),
            Hinge::Hip(side),
            leg,
        );
        seat(world, hip, at(joints.hip[s]));
        put(Hinge::Hip(side), hip);
        put(
            Hinge::Knee(side),
            join(
                world,
                (thighs[s], 1.0),
                (shins[s], 0.0),
                Hinge::Knee(side),
                leg,
            ),
        );
        // The ankle turns about a point on the foot's bone, between its
        // heel and its ball.
        let foot = world.bones[feet[s]];
        let ankle_t = segment_t(at(joints.ankle[s]), foot.a, foot.b);
        put(
            Hinge::Ankle(side),
            join(
                world,
                (shins[s], 1.0),
                (feet[s], ankle_t),
                Hinge::Ankle(side),
                leg,
            ),
        );
    }

    // The bones each sheet's flesh keeps its place on: the ribs move with
    // the chest's spine and are left out, as in the front-view body.
    let sheet_bones: Vec<Vec<usize>> = Part::FAR_TO_NEAR
        .iter()
        .map(|&part| match part {
            Part::UpperArm(side) => vec![upper_arms[side.index()]],
            Part::Forearm(side) => vec![forearms[side.index()], hands[side.index()]],
            Part::Thigh(side) => vec![thighs[side.index()]],
            Part::Shin(side) => vec![shins[side.index()]],
            Part::Foot(side) => vec![feet[side.index()]],
            Part::Trunk => vec![pelvis, lumbar, thoracic, cervical, skull],
        })
        .collect();
    let rig = Rig {
        pelvis,
        lumbar,
        thoracic,
        cervical,
        skull,
        upper_arms,
        forearms,
        hands,
        thighs,
        shins,
        feet,
        hinges,
        shoulder_seats,
        shoulder_slide,
        stand: frame.point(0.0, 1.0),
        height: frame.height,
    };
    (rig, sheet_bones, sheet_of_bone)
}

/// Weighs each sheet of flesh so that with its bones it weighs its part's
/// share of the body (`part_mass_share`): the tissue's own weights, worked
/// out from how thick the figure is (`tissue.rs`), keep their spread within
/// a part, and the part as a whole weighs what a man's does. How a limb
/// swings and what a blow moves depend on it.
fn weigh_parts(world: &mut World, rig: &Rig, sheet_of_bone: &[usize]) {
    // The spine is three bones here and one in the front-view body, and the
    // ribs stand for a whole chest's.
    for (bone, kilograms) in [(rig.lumbar, 0.5), (rig.thoracic, 0.6), (rig.cervical, 0.15)] {
        world.bones[bone].mass = kilograms / KG_PER_MASS;
    }
    for (sheet, &part) in Part::FAR_TO_NEAR.iter().enumerate() {
        let bones: f64 = world
            .bones
            .iter()
            .zip(sheet_of_bone)
            .filter(|&(_, &of)| of == sheet)
            .map(|(bone, _)| bone.mass)
            .sum();
        let flesh: f64 = world
            .points
            .iter()
            .filter(|point| point.sheet as usize == sheet)
            .map(|point| point.mass)
            .sum();
        let wanted = tissue::BODY_MASS_KG * part_mass_share(part) / KG_PER_MASS - bones;
        if flesh <= EPSILON || wanted <= 0.0 {
            continue;
        }
        let scale = wanted / flesh;
        for point in &mut world.points {
            if point.sheet as usize == sheet {
                point.mass *= scale;
            }
        }
    }
}

/// Where each of two side-view fighters stands, the viewer's left one
/// first: `spacing` body heights between their hips, about the world's
/// middle, at the scenarios' size, on the scenarios' floor
/// (`crate::world_frame`).
pub fn side_fighter_frames_at(spacing: f64) -> [BodyFrame; 2] {
    crate::fighter_frames_at(spacing)
}

/// How far apart two side-view fighters' hips stand at the start, in body
/// heights: out of reach of each other's fists, a step away from landing
/// one.
pub const SIDE_FIGHTER_SPACING: f64 = 0.88;

/// Two side-view fighters facing each other, the viewer's left one first:
/// one fighter built where the scenarios stand a body and moved to its
/// place, and its mirror image opposite it, so the two are solved alike.
pub fn create_side_fighters(materials: Materials) -> World {
    create_side_fighters_at(materials, SIDE_FIGHTER_SPACING)
}

/// Two side-view fighters as [`create_side_fighters`] builds them, `spacing`
/// body heights apart.
pub fn create_side_fighters_at(materials: Materials, spacing: f64) -> World {
    side_fighters_from(
        &create_side_fighter_in(crate::world_frame(), materials),
        spacing,
    )
}

/// Two side-view fighters made of `body`, one fighter built at
/// [`crate::world_frame`]: it moves to where the first stands and its mirror
/// image to where the second does, `spacing` body heights apart.
pub fn side_fighters_from(body: &World, spacing: f64) -> World {
    let built = crate::world_frame();
    let [left, right] = side_fighter_frames_at(spacing);
    let mut twin = body.mirrored(built.origin.x);
    twin.move_across(right.origin.x - built.origin.x);
    let mut world = body.clone();
    world.move_across(left.origin.x - built.origin.x);
    world.add_body(twin);
    world
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fighter() -> World {
        create_side_fighter_in(crate::world_frame(), Materials::default())
    }

    /// The fighter weighs what a man does, part by part.
    #[test]
    fn a_fighter_weighs_what_a_man_does() {
        let world = fighter();
        let flesh: f64 = world.points.iter().map(|point| point.mass).sum();
        let bones: f64 = world.bones.iter().map(|bone| bone.mass).sum();
        let kilograms = (flesh + bones) * KG_PER_MASS;
        assert!(
            (kilograms - tissue::BODY_MASS_KG).abs() < 1.0,
            "the fighter weighs {kilograms:.1} kg"
        );
    }

    /// It settles in its stance as built: no bone far from where it was
    /// built, nothing torn, and every joint within its range.
    #[test]
    fn a_fighter_rests_in_its_stance() {
        let world = fighter();
        let worst = world
            .bones
            .iter()
            .map(|bone| distance(bone.a, bone.home_a).max(distance(bone.b, bone.home_b)))
            .fold(0.0, f64::max);
        // Its hands, at the end of the trunk and the arms, rest a
        // centimetre and a half off at the most.
        assert!(
            worst < 6.0,
            "a bone rests {worst:.1} px from where it was built"
        );
        assert_eq!(world.stats.broken_bone_joints, 0, "a joint tore");
        assert!(
            world.springs.iter().all(|spring| !spring.broken),
            "flesh tore as the fighter settled"
        );
        for (index, joint) in world.bone_joints.iter().enumerate() {
            let turn = world.joint_turn(index).unwrap_or(0.0);
            assert!(
                turn > joint.min_angle - 0.02 && turn < joint.max_angle + 0.02,
                "joint {index} rests at {turn:.2}, outside {:.2}..{:.2}",
                joint.min_angle,
                joint.max_angle
            );
        }
    }

    /// Prints how a new fighter rests: how far each bone lies from where it
    /// was built, and each joint's turn and torque
    /// (`cargo test --release --lib print_fighter_rest -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn print_fighter_rest() {
        let world = fighter();
        let px = |v: Vec2| format!("({:+.1}, {:+.1})", v.x, v.y);
        for (index, bone) in world.bones.iter().enumerate() {
            println!(
                "bone {index:2} {:10} {:?}: a {} b {}  mass {:.2} kg flesh {:.1}",
                bone.part.name(),
                bone.limb,
                px(subtract(bone.a, bone.home_a)),
                px(subtract(bone.b, bone.home_b)),
                bone.mass * KG_PER_MASS,
                bone.flesh,
            );
        }
        for (index, joint) in world.bone_joints.iter().enumerate() {
            println!(
                "joint {index:2} {}-{}: turn {:+.3} in {:+.2}..{:+.2}, aim {:+.3}, torque {:+.1} N m, slip {:.1} px",
                world.bones[joint.a].part.name(),
                world.bones[joint.b].part.name(),
                world.joint_turn(index).unwrap_or(0.0),
                joint.min_angle,
                joint.max_angle,
                joint.drive.aim,
                world.joint_torque(index),
                world.joint_slip(index).unwrap_or(-1.0),
            );
        }
        let mut sheets = vec![(0usize, 0.0f64); Part::FAR_TO_NEAR.len()];
        for point in &world.points {
            let sheet = &mut sheets[point.sheet as usize];
            sheet.0 += 1;
            sheet.1 += point.mass * KG_PER_MASS;
        }
        for (part, (points, kilograms)) in Part::FAR_TO_NEAR.iter().zip(sheets) {
            println!("{part:?}: {points} points, {kilograms:.2} kg of flesh");
        }
        println!(
            "{} points, {} springs, {} triangles, {} bones, {} joints",
            world.points.len(),
            world.springs.len(),
            world.triangles.len(),
            world.bones.len(),
            world.bone_joints.len()
        );
        for hold in &world.hip_holds {
            let bone = world.bones[hold.bone];
            let at = if hold.end_b { bone.b } else { bone.a };
            println!(
                "hip hold: off {} aim {}",
                px(subtract(at, hold.home)),
                px(hold.aim)
            );
        }
    }

    /// Prints how the joints of a fighter's arm follow their aims through a
    /// move (`RP_MOVE=cross cargo test --release --lib print_fighter_move --
    /// --ignored --nocapture`).
    #[test]
    #[ignore]
    fn print_fighter_move() {
        let mut world = create_side_fighters(Materials::default());
        let dt = world.materials.fixed_dt;
        let made = match std::env::var("RP_MOVE").as_deref() {
            Ok("jab") => Move::Jab,
            Ok("step") => Move::StepIn,
            Ok("duck") => Move::Duck(true),
            Ok("back") => Move::LeanBack(true),
            _ => Move::Cross,
        };
        let rig = world.bodies[0].fighter.clone().unwrap();
        let side = if made == Move::Jab {
            Side::Lead
        } else {
            Side::Rear
        };
        let watched = [
            ("shoulder", rig.hinge(Hinge::Shoulder(side))),
            ("elbow", rig.hinge(Hinge::Elbow(side))),
            ("back", rig.hinge(Hinge::Back)),
            ("knee", rig.hinge(Hinge::Knee(Side::Lead))),
        ];
        for step in 0..90 {
            let mut input = InputState::default();
            if step == 5 {
                input.moves[0] = Some((0, made));
            }
            world.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
            let mut line = format!("step {step:3}:");
            for (name, index) in watched {
                let joint = world.bone_joints[index];
                line.push_str(&format!(
                    "  {name} aim {:+.2} at {:+.2} ({:+.0} N m{})",
                    joint.drive.swing,
                    world.joint_turn(index).unwrap_or(0.0),
                    world.joint_torque(index),
                    if joint.subluxated { ", slipped" } else { "" },
                ));
            }
            let hand = world.bones[rig.hands[side.index()]];
            let pelvis = world.bones[rig.pelvis];
            line.push_str(&format!(
                "  fist ({:.0}, {:.0}) hips ({:.0}, {:.0})",
                hand.b.x, hand.b.y, pelvis.b.x, pelvis.b.y
            ));
            println!("{line}");
        }
        println!(
            "torn joints {}, torn skin {}, bruises {}",
            world.stats.broken_bone_joints, world.stats.broken_skin, world.stats.contusion_events
        );
    }

    /// Two fighters stand as mirror images of each other.
    #[test]
    fn two_fighters_face_each_other_as_mirror_images() {
        let world = create_side_fighters(Materials::default());
        assert_eq!(world.body_count(), 2);
        let middle = crate::scenarios::SCENARIO_WIDTH * 0.5;
        let half = world.points.len() / 2;
        for index in 0..half {
            let (a, b) = (world.points[index], world.points[index + half]);
            assert!(
                (a.position.x + b.position.x - 2.0 * middle).abs() < 1.0e-6
                    && (a.position.y - b.position.y).abs() < 1.0e-6,
                "point {index} and its twin are not mirror images"
            );
        }
    }
}
