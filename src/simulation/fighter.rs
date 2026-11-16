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
        Hinge::Hip(_) => HIP_STRENGTH,
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

/// How strongly the muscles across one hip turn it at most, in newton metres
/// (`hinge_drive`): both hips' together are what keeps a fighter's pelvis
/// tilted as its moves have it (`mover.rs`, `CARRY_TILT_LIMIT`).
pub(super) const HIP_STRENGTH: f64 = 200.0;

/// How long a fighter's neck muscles take to change how hard they pull by
/// their whole strength, in seconds (`JointDrive::rise`): struck, a head
/// moves off before the neck's muscles answer, whose activity peaked 78 to
/// 114 ms after the head's acceleration did in rear-impact tests
/// (`docs/INJURY_REFERENCE.md`, Punches). Answering at once, they stopped a
/// head a cross had struck within a step: it moved off at 1.2 m/s where the
/// blow's momentum gives it about 3.5.
const NECK_RISE_SECONDS: f64 = 0.08;

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

/// How many sheets of flesh a fighter is made of.
pub(super) const SHEETS: usize = Part::FAR_TO_NEAR.len();

/// The side whose arm sheet `sheet` of a fighter's flesh is part of, if it
/// is part of an arm.
pub(super) fn arm_of_sheet(sheet: u8) -> Option<Side> {
    match Part::FAR_TO_NEAR.get(sheet as usize)? {
        Part::UpperArm(side) | Part::Forearm(side) => Some(*side),
        _ => None,
    }
}

/// Whether sheet `sheet` of a fighter's flesh is part of a leg.
pub(super) fn leg_sheet(sheet: u8) -> bool {
    matches!(
        Part::FAR_TO_NEAR.get(sheet as usize),
        Some(Part::Thigh(_) | Part::Shin(_) | Part::Foot(_))
    )
}

/// The sheet of a fighter's flesh that is the forearm and hand on `side`.
pub(super) fn forearm_sheet(side: Side) -> u8 {
    Part::FAR_TO_NEAR
        .iter()
        .position(|part| *part == Part::Forearm(side))
        .unwrap_or(0) as u8
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
    let mut world = unsettled_side_fighter_in(frame, materials);
    settle_side_fighter(&mut world, frame, |_, _| {});
    world
}

/// A side-view fighter as [`create_side_fighter_in`] builds it, standing on
/// the floor at its soles, before it has settled under its weight.
fn unsettled_side_fighter_in(frame: BodyFrame, materials: Materials) -> World {
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
    // where they stand. Each foot's bone rests on the pressed flesh of its
    // sole: the heel pad keeps about half its thickness under the body's
    // weight (Prichasuk 1994: a compressibility index, loaded over unloaded
    // thickness, of about 0.5-0.6 in healthy heels). Meeting the floor
    // along its own line, the bone sank 3.6 cm through the sole once the
    // legs bore the weight, and the whole fighter with it.
    world.set_floor_y(frame.point(0.0, 1.0).y);
    for foot in rig.feet {
        let bone = world.bones[foot];
        let resting = (world.floor_y() - bone.a.y).min(world.floor_y() - bone.b.y);
        world.bones[foot].sole = resting.max(0.0) * HEEL_PAD_LOADED;
    }
    world.hold_hip(rig.pelvis, false);
    world.hold_hip(rig.pelvis, true);
    for foot in rig.feet {
        world.plant_foot(foot);
    }
    world.carry_fighter_hips(0);
    world
}

/// Settles a fighter [`unsettled_side_fighter_in`] built in `frame` under
/// its weight as its muscles learn to carry it, so that it rests in its
/// stance as built, and gives it what moves it; `watch` sees the world after
/// each step of the settling, for the tools that print it.
fn settle_side_fighter(world: &mut World, frame: BodyFrame, mut watch: impl FnMut(&World, usize)) {
    let dt = world.materials.fixed_dt;
    let width = frame.origin.x * 2.0 + frame.height;
    let gravity = world.materials.gravity;
    for step in 0..SETTLE_STEPS {
        let share = ((step + 1) as f64 / WEIGHT_RAMP_STEPS as f64).min(1.0);
        world.materials.gravity = gravity * share * share * (3.0 - 2.0 * share);
        world.step(dt, &InputState::default(), width);
        world.learn_the_guards_weight();
        watch(world, step);
    }
    world.materials.gravity = gravity;
    world.give_mover(0);
}

/// How much of its thickness the flesh of a sole keeps under the body's
/// weight (`BoneSegment::sole`).
const HEEL_PAD_LOADED: f64 = 0.55;

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
            rise: match hinge {
                Hinge::Neck | Hinge::Head => NECK_RISE_SECONDS,
                _ => 0.0,
            },
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
                    rise: 0.0,
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
pub const SIDE_FIGHTER_SPACING: f64 = 0.78;

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

    /// The settings the tools that print play with: the defaults, changed by
    /// `RP_SET=name=value,...` as `--set` changes them.
    fn materials() -> Materials {
        let mut materials = Materials::default();
        if let Ok(settings) = std::env::var("RP_SET") {
            for setting in settings.split(',').filter(|setting| !setting.is_empty()) {
                let (name, value) = setting.split_once('=').expect("RP_SET takes name=value");
                materials.set(name, value).unwrap();
            }
        }
        materials
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

    /// It settles in its stance as built: standing on its own feet it comes
    /// down as one as far as the flesh of its soles presses, no further, and
    /// from there no bone lies far from where it was built; nothing torn,
    /// and every joint within its range.
    #[test]
    fn a_fighter_rests_in_its_stance() {
        let world = fighter();
        let rig = world.bodies[0].fighter.clone().unwrap();
        let moved = |bone: &BoneSegment| {
            scale(
                add(subtract(bone.a, bone.home_a), subtract(bone.b, bone.home_b)),
                0.5,
            )
        };
        let drop = scale(
            rig.feet.iter().fold(Vec2::default(), |sum, &foot| {
                add(sum, moved(&world.bones[foot]))
            }),
            0.5,
        );
        // Each sole keeps `HEEL_PAD_LOADED` of its thickness, so it presses
        // the rest of it.
        let pressed = rig
            .feet
            .iter()
            .map(|&foot| world.bones[foot].sole * (1.0 / HEEL_PAD_LOADED - 1.0))
            .fold(0.0, f64::max);
        assert!(
            drop.y <= pressed + 2.0,
            "the fighter came down {:.1} px where its soles press {pressed:.1}",
            drop.y
        );
        let worst = world
            .bones
            .iter()
            .map(|bone| {
                distance(subtract(bone.a, drop), bone.home_a)
                    .max(distance(subtract(bone.b, drop), bone.home_b))
            })
            .fold(0.0, f64::max);
        // Its hands, at the end of the trunk and the arms, rest a
        // centimetre and a half off at the most.
        assert!(
            worst < 6.0,
            "a bone rests {worst:.1} px from where it was built, from where its feet came down"
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
    /// --ignored --nocapture`); with `RP_BLOCK=1` it closes its block first
    /// and makes the move out of it.
    #[test]
    #[ignore]
    fn print_fighter_move() {
        let mut world = create_side_fighters(Materials::default());
        let dt = world.materials.fixed_dt;
        // `RP_MOVES=0:duck=on,60:step=out,90:step=out` makes those moves on
        // those steps instead, each written as a gesture writes it
        // (`scenarios.rs`), and prints how the trunk and the legs follow.
        if let Ok(script) = std::env::var("RP_MOVES") {
            let moves: Vec<(usize, usize, Move)> = script
                .split(',')
                .filter_map(|part| {
                    let (step, word) = part.split_once(':')?;
                    match crate::scenarios::fighter_move(word)? {
                        crate::scenarios::GestureStep::Act(body, made) => {
                            Some((step.parse().ok()?, body, made))
                        }
                        _ => None,
                    }
                })
                .collect();
            let steps = std::env::var("RP_STEPS")
                .ok()
                .and_then(|steps| steps.parse().ok())
                .unwrap_or_else(|| moves.iter().map(|made| made.0).max().unwrap_or(0) + 120);
            let rig = world.bodies[0].fighter.clone().unwrap();
            let watched = [
                ("back", Hinge::Back),
                ("mid", Hinge::Mid),
                ("neck", Hinge::Neck),
                ("hip", Hinge::Hip(Side::Lead)),
                ("hip'", Hinge::Hip(Side::Rear)),
                ("knee", Hinge::Knee(Side::Lead)),
                ("knee'", Hinge::Knee(Side::Rear)),
                ("ankle", Hinge::Ankle(Side::Lead)),
                ("ankle'", Hinge::Ankle(Side::Rear)),
            ];
            // `RP_EVERY=1` prints every step instead of every fifth.
            let every: usize = std::env::var("RP_EVERY")
                .ok()
                .and_then(|every| every.parse().ok())
                .unwrap_or(5)
                .max(1);
            for step in 0..steps {
                let mut input = InputState::default();
                for (slot, (_, body, made)) in
                    moves.iter().filter(|made| made.0 == step).enumerate()
                {
                    input.moves[slot] = Some((*body, *made));
                }
                world.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
                if step % every != 0 {
                    continue;
                }
                let pelvis = world.bones[rig.pelvis];
                let mut line = format!(
                    "step {step:3}: head {:3.0} cm off its pose, pelvis at {:+4.0} deg",
                    world.head_off_pose(0).unwrap_or(0.0) * 100.0,
                    libm::atan2(pelvis.b.y - pelvis.a.y, pelvis.b.x - pelvis.a.x).to_degrees(),
                );
                for (name, hinge) in watched {
                    let index = rig.hinge(hinge);
                    let joint = world.bone_joints[index];
                    line.push_str(&format!(
                        "  {name} {:+.2}/{:+.2} ({:+.0}{})",
                        joint.drive.swing,
                        world.joint_turn(index).unwrap_or(0.0),
                        world.joint_torque(index),
                        if joint.subluxated { ", out" } else { "" },
                    ));
                }
                println!("{line}");
            }
            println!(
                "each joint: aim/turn in radians (torque in N m); torn joints {}, slipped {}",
                world.stats.broken_bone_joints, world.stats.bone_joint_subluxations
            );
            return;
        }
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
            ("wrist", rig.hinge(Hinge::Wrist(side))),
            ("back", rig.hinge(Hinge::Back)),
            ("knee", rig.hinge(Hinge::Knee(Side::Lead))),
        ];
        let from_block = std::env::var("RP_BLOCK").is_ok();
        let moves_at = if from_block { 40 } else { 5 };
        // `RP_STEPS=600` plays that many steps and prints every tenth, with
        // where the pose puts the head and where it is.
        let long: Option<usize> = std::env::var("RP_STEPS")
            .ok()
            .and_then(|steps| steps.parse().ok());
        for step in 0..long.unwrap_or(moves_at + 85) {
            let mut input = InputState::default();
            if step == 0 && from_block {
                input.moves[1] = Some((0, Move::Block(true)));
            }
            if step == moves_at {
                input.moves[0] = Some((0, made));
            }
            world.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
            if long.is_some() {
                if step % 10 == 0 {
                    let (skull, pelvis) = (world.bones[rig.skull], world.bones[rig.pelvis]);
                    let head = lerp(skull.a, skull.b, 0.5);
                    println!(
                        "step {step:3}: head ({:.0}, {:.0}), hips ({:.0}, {:.0}); {}",
                        head.x,
                        head.y,
                        pelvis.b.x,
                        pelvis.b.y,
                        world.mover_line(0)
                    );
                }
                continue;
            }
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

    /// Prints what each fighter's legs pushed with over the last substep
    /// (`balance.rs`): the push on the hips and the turn on the pelvis, and
    /// how hard they pushed each foot standing on the floor, down and across.
    fn print_balance_reports(world: &World) {
        for (body, report) in world.balance_reports.iter().enumerate() {
            let mut legs = format!(
                "    [{body}] centre of mass at {:.1} cm moving {:+.2} m/s, {:3.0}% over the feet, capture point {:3.0}% over the planted ones, hips shifted {:+.1} cm, {:+.1} cm off the aim and {:+.1} cm off the move's place, {:+.1} cm below it; legs push the hips ({:+5.0}, {:+5.0}) N and turn the pelvis {:+4.0} N m;",
                report.centre_cm,
                report.centre_speed,
                report.over_feet * 100.0,
                report.capture_over_feet * 100.0,
                report.shift_cm,
                report.aim_off_cm,
                report.pose_off_cm,
                report.sunk_cm,
                report.push.x,
                report.push.y,
                report.turn
            );
            let h = world.materials.fixed_dt / world.materials.substeps as f64;
            let rig = world.bodies[body].fighter.as_ref();
            for (side, name) in ["lead", "rear"].iter().enumerate() {
                if report.planted[side] {
                    legs.push_str(&format!(
                        " {name} foot pressed {:4.0} N down, {:+4.0} N across",
                        report.feet[side].y, report.feet[side].x
                    ));
                    // What the floor gave the foot back, up and across.
                    if let Some(hold) = rig.and_then(|rig| {
                        world
                            .foot_holds
                            .iter()
                            .find(|hold| hold.bone == rig.feet[side])
                    }) {
                        let floor = scale(
                            add(hold.impulse[0], hold.impulse[1]),
                            1.0 / (h * world.newtons()),
                        );
                        legs.push_str(&format!(
                            ", the floor gives ({:+.0}, {:+.0}) N",
                            floor.x, -floor.y
                        ));
                    }
                    legs.push(';');
                } else {
                    legs.push_str(&format!(" {name} foot off the floor;"));
                }
            }
            // Where each foot stands: how high its heel and ball are and
            // how far it has slid from where it was built.
            if let Some(rig) = world.bodies[body].fighter.as_ref() {
                let cm = world.mm_per_px() * 0.1;
                for (name, &foot) in ["lead", "rear"].iter().zip(&rig.feet) {
                    let bone = world.bones[foot];
                    legs.push_str(&format!(
                        " {name} heel {:.1} ball {:.1} cm up, slid {:+.1} cm;",
                        (world.floor_y() - bone.a.y) * cm,
                        (world.floor_y() - bone.b.y) * cm,
                        (bone.b.x - bone.home_b.x) * cm,
                    ));
                }
            }
            println!("{legs}");
        }
    }

    /// Shoves the first side-view fighter's trunk, back toward its rear foot
    /// and then forward over its lead foot, with growing impulses over a
    /// tenth of a second, and prints for each whether it held where it
    /// stood, stepped (how far its feet went), or fell, and what tore
    /// (`cargo test --release --lib print_fighter_shoves -- --ignored
    /// --nocapture`). People pushed in experiments take impulses of 12 to
    /// 56 N s, stepping above a threshold and holding below it (McGreavy et
    /// al. 2020). `RP_SHOVES=20,40` gives the impulses in newton seconds,
    /// `RP_EVERY=10` prints every tenth step of each shove, and `RP_SET`
    /// changes settings.
    #[test]
    #[ignore]
    fn print_fighter_shoves() {
        let base = create_side_fighters(materials());
        let impulses: Vec<f64> = std::env::var("RP_SHOVES")
            .unwrap_or_else(|_| "10,20,30,40,60,80,100,150,200".to_string())
            .split(',')
            .filter_map(|impulse| impulse.parse().ok())
            .collect();
        let every: Option<usize> = std::env::var("RP_EVERY")
            .ok()
            .and_then(|every| every.parse().ok());
        let dt = base.materials.fixed_dt;
        let cm = base.mm_per_px() * 0.1;
        let rig = base.bodies[0].fighter.clone().unwrap();
        let facing = if base.bodies[0].mirrored { -1.0 } else { 1.0 };
        let steps = (2.5 / dt).round() as usize;
        for (name, way) in [("back", -1.0), ("forward", 1.0)] {
            for &impulse in &impulses {
                let mut world = base.clone();
                let feet_home: Vec<f64> =
                    rig.feet.iter().map(|&foot| world.bones[foot].b.x).collect();
                let hips_home = world.bones[rig.pelvis].b.x;
                let (mut lowest_head, mut farthest_foot, mut farthest_hips) =
                    (f64::INFINITY, 0.0_f64, 0.0_f64);
                for step in 0..steps {
                    let mut input = InputState::default();
                    if step == 0 {
                        input.moves[0] = Some((0, Move::Shoved((-way * impulse).round() as i32)));
                    }
                    world.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
                    let skull = world.bones[rig.skull];
                    let head = (world.floor_y() - lerp(skull.a, skull.b, 0.5).y) * cm;
                    lowest_head = lowest_head.min(head);
                    for (side, &foot) in rig.feet.iter().enumerate() {
                        farthest_foot =
                            farthest_foot.max((world.bones[foot].b.x - feet_home[side]).abs() * cm);
                    }
                    let hips = (world.bones[rig.pelvis].b.x - hips_home) * facing * cm;
                    farthest_hips = farthest_hips.max(hips * way);
                    if every.is_some_and(|every| step % every == 0) {
                        println!(
                            "    step {step:3}: hips {hips:+.1} cm, head {head:.0} cm up, lead and rear feet {}; {}",
                            rig.feet
                                .iter()
                                .enumerate()
                                .map(|(side, &foot)| format!(
                                    "{:+.1}",
                                    (world.bones[foot].b.x - feet_home[side]) * facing * cm
                                ))
                                .collect::<Vec<_>>()
                                .join(" and "),
                            world.mover_line(0)
                        );
                    }
                }
                let hips_end = (world.bones[rig.pelvis].b.x - hips_home) * facing * cm;
                let fell = lowest_head < 100.0;
                let outcome = if fell {
                    "fell"
                } else if farthest_foot >= 3.0 {
                    "stepped"
                } else {
                    "held"
                };
                println!(
                    "shoved {name} with {impulse:3.0} N s: {outcome}; hips at most {farthest_hips:.0} cm the way it was shoved, {hips_end:+.0} cm at the end; a foot moved at most {farthest_foot:.0} cm; head as low as {lowest_head:.0} cm; torn joints {}, skin {}, muscle {}, bones broken {}",
                    world.stats.broken_bone_joints,
                    world.stats.broken_skin,
                    world.stats.broken_muscle,
                    world.stats.fractured_bones,
                );
            }
        }
    }

    /// Prints how much of each side-view fighter's weight the hold that
    /// carries its hips bears (`Carry`), how hard it pushes them across and
    /// turns the pelvis, how far the hips move, and how high the head stands,
    /// through a script of moves (`cargo test --release --lib
    /// print_fighter_support -- --ignored --nocapture`). `RP_MOVES` writes
    /// the script as `print_fighter_move` reads it (the default steps the
    /// left fighter in, takes a cross from the right one, jabs, ducks, and
    /// leans back), `RP_EVERY=10` prints every tenth step, and
    /// `RP_SET=carry_strength=0` changes settings as `--set` does: with no
    /// hold, the fighters must stand on their own legs.
    #[test]
    #[ignore]
    fn print_fighter_support() {
        let materials = materials();
        let every: usize = std::env::var("RP_EVERY")
            .ok()
            .and_then(|every| every.parse().ok())
            .unwrap_or(5)
            .max(1);
        // `RP_SETTLE=1` prints how one fighter settles as it is built
        // instead, every `RP_EVERY` steps.
        if std::env::var("RP_SETTLE").is_ok() {
            let frame = crate::world_frame();
            let mut world = unsettled_side_fighter_in(frame, materials);
            let cm = world.mm_per_px() * 0.1;
            settle_side_fighter(&mut world, frame, |world, step| {
                if step % every != 0 {
                    return;
                }
                let Some(rig) = world.bodies[0].fighter.as_ref() else {
                    return;
                };
                let skull = world.bones[rig.skull];
                let pelvis = world.bones[rig.pelvis];
                let mut feet = String::new();
                for (name, &foot) in ["lead", "rear"].iter().zip(&rig.feet) {
                    let bone = world.bones[foot];
                    let Some(hold) = world.foot_holds.iter().find(|hold| hold.bone == foot) else {
                        continue;
                    };
                    feet.push_str(&format!(
                        "; {name} heel {:+.1} ball {:+.1} cm up{}{}, slid {:+.1} cm",
                        (world.floor_y() - bone.a.y) * cm,
                        (world.floor_y() - bone.b.y) * cm,
                        if hold.lifted[0] { " heel off" } else { "" },
                        if hold.lifted[1] { " ball off" } else { "" },
                        (bone.b.x - bone.home_b.x) * cm,
                    ));
                }
                println!(
                    "settle step {step:3}: hips {:+5.1} cm across {:+5.1} cm down, head {:3.0} cm up{feet}",
                    (pelvis.b.x - pelvis.home_b.x) * cm,
                    (pelvis.b.y - pelvis.home_b.y) * cm,
                    (world.floor_y() - lerp(skull.a, skull.b, 0.5).y) * cm,
                );
                print_balance_reports(world);
            });
            return;
        }
        // `RP_SPACING=0.6` stands the fighters that many body heights apart.
        let mut world = std::env::var("RP_SPACING")
            .ok()
            .and_then(|spacing| spacing.parse().ok())
            .map_or_else(
                || create_side_fighters(materials),
                |spacing| create_side_fighters_at(materials, spacing),
            );
        // `RP_SET_AFTER` changes settings once the fighters have settled.
        if let Ok(settings) = std::env::var("RP_SET_AFTER") {
            for setting in settings.split(',').filter(|setting| !setting.is_empty()) {
                let (name, value) = setting
                    .split_once('=')
                    .expect("RP_SET_AFTER takes name=value");
                world.materials.set(name, value).unwrap();
            }
        }
        let dt = world.materials.fixed_dt;
        let h = dt / world.materials.substeps as f64;
        let script = std::env::var("RP_MOVES").unwrap_or_else(|_| {
            "60:step=in,120:cross2,200:jab,260:duck=on,330:duck=off,380:back=on,440:back=off"
                .to_string()
        });
        let moves: Vec<(usize, usize, Move)> = script
            .split(',')
            .filter_map(|part| {
                let (step, word) = part.split_once(':')?;
                match crate::scenarios::fighter_move(word)? {
                    crate::scenarios::GestureStep::Act(body, made) => {
                        Some((step.parse().ok()?, body, made))
                    }
                    _ => None,
                }
            })
            .collect();
        let steps = std::env::var("RP_STEPS")
            .ok()
            .and_then(|steps| steps.parse().ok())
            .unwrap_or_else(|| moves.iter().map(|made| made.0).max().unwrap_or(0) + 120);
        let weight = tissue::BODY_MASS_KG * world.materials.gravity;
        let (newtons, newton_metres, cm) = (
            world.newtons(),
            world.newton_metres(),
            world.mm_per_px() * 0.1,
        );
        let rigs: Vec<Rig> = world
            .bodies
            .iter()
            .filter_map(|state| state.fighter.clone())
            .collect();
        let hips_at_start: Vec<f64> = rigs.iter().map(|rig| world.bones[rig.pelvis].b.x).collect();
        // For each fighter: the shares of its weight held up summed, the
        // most held up, the hardest pushed across, and the hardest turned.
        let mut held = vec![(0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64); rigs.len()];
        // `RP_TRACE=30-34` follows the first fighter's feet and hips through
        // every phase of those steps and prints what each phase did to them,
        // summed over the step's substeps.
        let steps_in = |name: &str| {
            std::env::var(name).ok().and_then(|range| {
                let (from, to) = range.split_once('-')?;
                Some(from.parse::<usize>().ok()?..=to.parse::<usize>().ok()?)
            })
        };
        let traced_steps = steps_in("RP_TRACE");
        // `RP_MOMENTUM=36-40` adds up, for each of those steps, how hard each
        // phase pushed the first fighter's whole body across and up, in
        // newtons: what pushes within the body adds up to nothing, so what
        // is left is the floor's, gravity's, drag's, and the other
        // fighter's.
        let momentum_steps = steps_in("RP_MOMENTUM");
        for step in 0..steps {
            let mut input = InputState::default();
            for (slot, (_, body, made)) in moves.iter().filter(|made| made.0 == step).enumerate() {
                input.moves[slot] = Some((*body, *made));
            }
            if momentum_steps
                .as_ref()
                .is_some_and(|range| range.contains(&step))
            {
                let mut traced = Vec::new();
                let mut masses = Vec::new();
                for (index, point) in world.points.iter().enumerate() {
                    if point.body == 0 && !point.pinned && !point.is_ghost() {
                        traced.push(Traced::Point(index));
                        masses.push(point.mass);
                    }
                }
                for (index, bone) in world.bones.iter().enumerate() {
                    if bone.body == 0 {
                        traced.push(Traced::Bone(index));
                        masses.push(bone.mass);
                    }
                }
                world.trace(&traced);
                world.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
                let mut phases: Vec<(&str, Vec2)> = Vec::new();
                for stage in world.traced_stages() {
                    let change = scale(
                        subtract(stage.velocity_after, stage.velocity_before),
                        masses[stage.target],
                    );
                    match phases.iter_mut().find(|(phase, _)| *phase == stage.phase) {
                        Some(summed) => summed.1 = add(summed.1, change),
                        None => phases.push((stage.phase, change)),
                    }
                }
                // Mass times pixels a second over the step, as newtons.
                let to_newtons =
                    crate::simulation::soft::KG_PER_MASS * world.mm_per_px() * 0.001 / dt;
                let mut line = format!("step {step:3}, pushes on the first fighter's body:");
                for (phase, change) in phases {
                    let (across, up) = (change.x * to_newtons, -change.y * to_newtons);
                    if across.abs() > 5.0 || up.abs() > 5.0 {
                        line.push_str(&format!(" {phase} ({across:+.0}, {up:+.0});"));
                    }
                }
                println!("{line}");
                world.trace(&[]);
                continue;
            }
            let tracing = traced_steps
                .as_ref()
                .is_some_and(|range| range.contains(&step));
            if tracing {
                let rig = &rigs[0];
                world.trace(&[
                    Traced::Bone(rig.feet[0]),
                    Traced::Bone(rig.feet[1]),
                    Traced::Bone(rig.pelvis),
                ]);
            }
            world.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
            if tracing {
                println!("step {step:3}, what each phase did to the first fighter:");
                for (target, name) in ["lead foot", "rear foot", "hips"].iter().enumerate() {
                    let mut phases: Vec<(&str, Vec2, f64)> = Vec::new();
                    for stage in world.traced_stages() {
                        if stage.target != target {
                            continue;
                        }
                        match phases.iter_mut().find(|(phase, ..)| *phase == stage.phase) {
                            Some(summed) => {
                                summed.1 = add(summed.1, stage.moved);
                                summed.2 += stage.turned;
                            }
                            None => phases.push((stage.phase, stage.moved, stage.turned)),
                        }
                    }
                    let mut parts = String::new();
                    for (phase, moved, turned) in phases {
                        if hypot(moved.x, moved.y) * cm > 0.05 || turned.abs() > 0.005 {
                            parts.push_str(&format!(
                                " {phase} ({:+.1}, {:+.1}) cm {:+.2} rad;",
                                moved.x * cm,
                                -moved.y * cm,
                                turned
                            ));
                        }
                    }
                    println!("    {name}:{parts}");
                }
                world.trace(&[]);
            }
            let mut line = format!("step {step:3}:");
            for (body, rig) in rigs.iter().enumerate() {
                let (mut up, mut across, mut tilt) = (0.0, 0.0, 0.0);
                for hold in world
                    .hip_holds
                    .iter()
                    .filter(|hold| hold.bone == rig.pelvis)
                {
                    // The window's y runs down, so holding up is negative.
                    up -= hold.impulse.y / h / newtons;
                    across += hold.impulse.x / h / newtons;
                    if let Some(carry) = hold.carry {
                        tilt += carry.tilt_impulse / h / newton_metres;
                    }
                }
                let share = up / weight;
                let most = &mut held[body];
                most.0 += share;
                most.1 = most.1.max(share);
                most.2 = most.2.max(across.abs());
                most.3 = most.3.max(tilt.abs());
                let skull = world.bones[rig.skull];
                let head_height = (world.floor_y() - lerp(skull.a, skull.b, 0.5).y) * cm;
                line.push_str(&format!(
                    "  [{body}] {} {:3.0}%, across {:+5.0} N, tilt {:+4.0} N m, hips {:+5.1} cm, head {:3.0} cm up {:3.0} cm off pose",
                    if world.materials.legs_carry {
                        "legs push up"
                    } else {
                        "held up from outside"
                    },
                    share * 100.0,
                    across,
                    tilt,
                    (world.bones[rig.pelvis].b.x - hips_at_start[body]) * cm,
                    head_height,
                    world.head_off_pose(body).unwrap_or(0.0) * 100.0,
                ));
            }
            if step % every == 0 {
                println!("{line}");
                print_balance_reports(&world);
            }
        }
        for (body, most) in held.iter().enumerate() {
            // Legs that carry the hips push them from the floor, and every
            // push comes back on the feet: nothing holds them up from
            // outside (`balance.rs`).
            println!(
                "fighter {body}: {} pushed the hips up with {:.0}% of its weight on average and {:.0}% at most, across with up to {:.0} N, and turned the pelvis with up to {:.0} N m (its cap {:.0}); from outside: {}",
                if world.materials.legs_carry {
                    "its legs"
                } else {
                    "the hold on its hips"
                },
                most.0 / steps.max(1) as f64 * 100.0,
                most.1 * 100.0,
                most.2,
                most.3,
                // As `mover.rs` caps it: both hips' strength.
                2.0 * HIP_STRENGTH * world.materials.carry_strength,
                if world.materials.legs_carry {
                    "nothing".to_string()
                } else {
                    format!("{:.0}% of its weight on average", most.0 / steps.max(1) as f64 * 100.0)
                },
            );
        }
        println!(
            "torn joints {}, torn skin {}, bones broken {}",
            world.stats.broken_bone_joints, world.stats.broken_skin, world.stats.fractured_bones
        );
    }

    /// Prints what a jab and a cross do at several distances, into an open
    /// guard and into a block: how fast the fist lands, where it bruises,
    /// how hard it presses the skull, how far it knocks the head and the
    /// hips (`cargo test --release --lib print_fighter_punches -- --ignored
    /// --nocapture`; `RP_STORY=1` adds what the journal kept).
    #[test]
    #[ignore]
    fn print_fighter_punches() {
        let story = std::env::var("RP_STORY").is_ok();
        let one = create_side_fighter_in(crate::world_frame(), materials());
        let mm = one.mm_per_px();
        // What each bone weighs with the flesh held to it (`sides.rs`).
        let rig = one.bodies[0].fighter.clone().unwrap();
        for (name, bone) in [
            ("head", rig.skull),
            ("neck", rig.cervical),
            ("chest", rig.thoracic),
        ] {
            let flesh: f64 = one
                .points
                .iter()
                .zip(&one.point_holds)
                .map(|(point, hold)| {
                    (0..2)
                        .filter(|&slot| hold.posture.bones[slot] == bone)
                        .map(|slot| point.mass * sides::share_of(hold.posture.weights[slot]))
                        .sum::<f64>()
                })
                .sum();
            println!(
                "the {name}'s bone with its flesh weighs {:.1} kg",
                (flesh + one.bones[bone].mass) * KG_PER_MASS
            );
        }
        let detail = std::env::var("RP_DETAIL").is_ok();
        let mut tally: std::collections::BTreeMap<String, [f64; 16]> = Default::default();
        for spacing in (0..46).map(|step| 0.50 + step as f64 * 0.005) {
            let pair = side_fighters_from(&one, spacing);
            for (made, blocked) in [
                (Move::Jab, false),
                (Move::Cross, false),
                (Move::Jab, true),
                (Move::Cross, true),
            ] {
                // `RP_ONLY=cross` or `jab`, and `RP_BLOCK=0` or `1`, play
                // only those.
                let only = std::env::var("RP_ONLY").unwrap_or_default();
                let block = std::env::var("RP_BLOCK").unwrap_or_default();
                if (only == "cross" && made != Move::Cross)
                    || (only == "jab" && made != Move::Jab)
                    || (block == "0" && blocked)
                    || (block == "1" && !blocked)
                {
                    continue;
                }
                let mut world = pair.clone();
                world.keep_journal();
                let dt = world.materials.fixed_dt;
                let rig = world.bodies[1].fighter.clone().unwrap();
                let own = world.bodies[0].fighter.clone().unwrap();
                let side = if made == Move::Jab {
                    Side::Lead
                } else {
                    Side::Rear
                };
                let skull = world.bones[rig.skull];
                let turn =
                    |bone: &BoneSegment| libm::atan2(bone.b.y - bone.a.y, bone.b.x - bone.a.x);
                let (head_then, hips_then) = (turn(&skull), world.bones[rig.pelvis].b.x);
                let (mut fastest, mut pressed, mut turned, mut shoved): (f64, f64, f64, f64) =
                    (0.0, 0.0, 0.0, 0.0);
                let (mut head_speed, mut head_moved): (f64, f64) = (0.0, 0.0);
                let middle = |bone: &BoneSegment| lerp(bone.a, bone.b, 0.5);
                let head_from_hips =
                    |world: &World| middle(&world.bones[rig.skull]).x - world.bones[rig.pelvis].b.x;
                let stood = head_from_hips(&world);
                // How the struck head spins and is thrown, substep by
                // substep, from the skull's bone as it began each: its
                // turning speed in radians a second and its speed in pixels
                // a second. Measured as Walilko, Viano and Bir (2005) measured
                // Olympic boxers' punches on a dummy's head.
                let mut head_motion: Vec<(f64, Vec2)> = Vec::new();
                // `RP_COUNTER=10` has the struck fighter step in that many
                // frames before the punch is thrown, so it lands as a counter.
                let counter: Option<usize> = std::env::var("RP_COUNTER")
                    .ok()
                    .and_then(|frames| frames.parse().ok());
                for step in 0..70 {
                    let mut input = InputState::default();
                    if step == 0 && blocked {
                        input.moves[0] = Some((1, Move::Block(true)));
                    }
                    if counter.is_some_and(|frames| step + frames == 20) {
                        input.moves[0] = Some((1, Move::StepIn));
                    }
                    if step == 20 {
                        input.moves[1] = Some((0, made));
                    }
                    let hand = world.bones[own.hands[side.index()]];
                    let head = middle(&world.bones[rig.skull]);
                    let watching = (20..50).contains(&step);
                    if watching {
                        world.trace(&[Traced::Bone(rig.skull)]);
                    }
                    world.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
                    if watching {
                        head_motion.extend(
                            world
                                .traced_stages()
                                .iter()
                                .filter(|stage| stage.phase == "integrate")
                                .map(|stage| (stage.spin_before, stage.velocity_before)),
                        );
                        world.trace(&[]);
                    }
                    let now = world.bones[own.hands[side.index()]];
                    fastest = fastest.max(distance(hand.b, now.b) / dt * mm * 0.001);
                    if step > 20 {
                        let speed = distance(head, middle(&world.bones[rig.skull])) / dt;
                        head_speed = head_speed.max(speed * mm * 0.001);
                        head_moved = head_moved.max((head_from_hips(&world) - stood).abs());
                    }
                    let skull = world.bones[rig.skull];
                    pressed = pressed.max(skull.break_load / skull.fracture_impulse);
                    turned = turned.max(wrap_angle(turn(&skull) - head_then).abs());
                    shoved = shoved.max((world.bones[rig.pelvis].b.x - hips_then).abs());
                }
                let landed: Vec<f64> = world
                    .journal()
                    .map(|journal| {
                        journal
                            .events
                            .iter()
                            .filter(|event| {
                                event.kind == EventKind::PunchTurnedBack
                                    && event.measure == Measure::Speed
                            })
                            .map(|event| event.value)
                            .collect()
                    })
                    .unwrap_or_default();
                let mut reddened = [0usize; 2];
                for point in world.points.iter().filter(|point| point.body == 1) {
                    if point.flush > 0.01 || point.contusion > 0.01 {
                        reddened[usize::from(arm_of_sheet(point.sheet).is_some())] += 1;
                    }
                }
                let landed_speed = landed.first().copied().unwrap_or(0.0);
                // The head's hardest turning and throwing, substep to substep.
                let h = dt / world.materials.substeps as f64;
                let (mut spin, mut spun, mut thrown) = (0.0_f64, 0.0_f64, 0.0_f64);
                // How much the head's turning changed within a blow's time
                // (four substeps, 17 ms), as the engine measures a knockout
                // (`knockout.rs`).
                let changed = head_motion
                    .windows(4)
                    .map(|window| {
                        let turning = window.iter().map(|(turning, _)| *turning);
                        turning.clone().fold(f64::MIN, f64::max) - turning.fold(f64::MAX, f64::min)
                    })
                    .fold(0.0_f64, f64::max);
                for pair in head_motion.windows(2) {
                    let ((was_turning, was_moving), (turning, moving)) = (pair[0], pair[1]);
                    spin = spin.max(turning.abs());
                    spun = spun.max(((turning - was_turning) / h).abs());
                    let gained = subtract(moving, was_moving);
                    thrown =
                        thrown.max(hypot(gained.x, gained.y) / h * mm * 0.001 / STANDARD_GRAVITY);
                }
                let key = format!("{made:?}{}", if blocked { " into a block" } else { "" });
                let sums = tally.entry(key).or_default();
                sums[0] += 1.0;
                sums[1] += f64::from(!landed.is_empty());
                sums[2] += landed_speed;
                sums[3] += head_speed;
                sums[4] += f64::from(world.stats.broken_skin > 0);
                sums[5] += f64::from(world.stats.broken_bone_joints > 0);
                sums[6] += f64::from(world.stats.bone_joint_subluxations > 0);
                sums[7] += f64::from(world.stats.fractured_bones > 0);
                if !landed.is_empty() {
                    sums[8] += f64::from(reddened[0] > 0);
                    sums[9] += f64::from(reddened[1] > 0);
                    sums[10] += head_moved * mm * 0.1;
                    sums[11] = sums[11].max(pressed);
                    sums[12] += turned.to_degrees();
                    sums[13] += spun;
                    sums[14] += spin;
                    sums[15] += thrown;
                }
                if !detail && !story {
                    continue;
                }
                let (watched, _, _) = world.hardest_head_spin(1).unwrap_or_default();
                println!(
                    "{spacing:.3} apart, {made:?}{}: fist up to {fastest:.1} m/s, landed at {landed:.1?} m/s; reddened points head and trunk {}, arms {}; skull pressed {pressed:.2} of its strength, head up to {head_speed:.1} m/s and {:.1} cm off its hips, turned {:.0} deg, spun at up to {spun:.0} rad/s2 (watched {watched:.0}{}) and {spin:.1} rad/s, its turning changed by {changed:.1} rad/s within the blow, and thrown at {thrown:.0} g, hips shoved {:.1} cm; torn joints {}, skin {}, muscle {}, broken bones {}",
                    if blocked { " into a block" } else { "" },
                    reddened[0],
                    reddened[1],
                    head_moved * mm * 0.1,
                    turned.to_degrees(),
                    if world.knocked_out(1) { ", knocked out" } else { "" },
                    shoved * mm * 0.1,
                    world.stats.broken_bone_joints,
                    world.stats.broken_skin,
                    world.stats.broken_muscle,
                    world.stats.fractured_bones,
                );
                if story {
                    if let Some(journal) = world.journal() {
                        for line in crate::inspect::story(&world, journal) {
                            println!("    {line}");
                        }
                    }
                }
            }
        }
        for (key, sums) in tally {
            let landed = sums[1].max(1.0);
            println!(
                "{key}: landed in {:.0} of {:.0} runs, at {:.1} m/s on average; of those it reddened the head or trunk in {:.0} and the arms in {:.0}, moved the head {:.1} cm off its hips and turned it {:.0} deg on average (up to {:.1} m/s), spun it at up to {:.0} rad/s2 and {:.1} rad/s and threw it at {:.0} g on average, pressed the skull with at most {:.2} of its strength; runs with skin torn {:.0}, a joint torn {:.0}, a joint slipped {:.0}, a bone broken {:.0}",
                sums[1],
                sums[0],
                sums[2] / landed,
                sums[8],
                sums[9],
                sums[10] / landed,
                sums[12] / landed,
                sums[3] / sums[0],
                sums[13] / landed,
                sums[14] / landed,
                sums[15] / landed,
                sums[11],
                sums[4],
                sums[5],
                sums[6],
                sums[7],
            );
        }
    }

    /// Two fighters `spacing` body heights apart, a journal kept.
    fn pair(spacing: f64) -> World {
        let mut world = side_fighters_from(&fighter(), spacing);
        world.keep_journal();
        world
    }

    /// Plays the steps `steps` of `world`, the fighters making the moves
    /// `moves` gives them: each a step, a fighter, and its move. Returns the
    /// fastest an end of any bone moved in a step, in metres a second.
    fn play(
        world: &mut World,
        steps: std::ops::Range<usize>,
        moves: &[(usize, usize, Move)],
    ) -> f64 {
        let dt = world.materials.fixed_dt;
        let mut fastest = 0.0_f64;
        for step in steps {
            let mut input = InputState::default();
            for (slot, &(_, body, made)) in moves.iter().filter(|made| made.0 == step).enumerate() {
                input.moves[slot] = Some((body, made));
            }
            world.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
            fastest = fastest.max(fastest_bone(world, dt));
        }
        fastest
    }

    /// How many points of body `body` a blow has reddened or bruised: of its
    /// head and trunk, and of its arms.
    fn reddened(world: &World, body: u8) -> [usize; 2] {
        let mut reddened = [0; 2];
        for point in world.points.iter().filter(|point| point.body == body) {
            if point.flush > 0.01 || point.contusion > 0.01 {
                reddened[usize::from(arm_of_sheet(point.sheet).is_some())] += 1;
            }
        }
        reddened
    }

    /// How fast each fist that landed met what it hit, in metres a second.
    fn landed(world: &World) -> Vec<f64> {
        world.journal().map_or(Vec::new(), |journal| {
            journal
                .events
                .iter()
                .filter(|event| {
                    event.kind == EventKind::PunchTurnedBack && event.measure == Measure::Speed
                })
                .map(|event| event.value)
                .collect()
        })
    }

    /// Everything a fight has torn, put out of joint, or broken.
    fn hurt(world: &World) -> i32 {
        torn_so_far(world) + world.stats.fractured_bones
    }

    /// A cross goes through the other fighter's open hands and lands on its
    /// head, as fast as a trained boxer's; with the other blocking, it lands
    /// on the forearms, and the head behind them is untouched. Neither tears
    /// anything.
    #[test]
    fn a_punch_goes_through_open_hands_and_a_block_stops_it() {
        let both = pair(0.65);
        let mut open = both.clone();
        play(&mut open, 0..45, &[(5, 0, Move::Cross)]);
        let [head, arms] = reddened(&open, 1);
        assert!(
            head > 0 && arms == 0,
            "a cross into open hands reddened {head} points of the head and trunk and {arms} of the arms"
        );
        let speeds = landed(&open);
        assert!(
            speeds
                .first()
                .is_some_and(|speed| (7.0..11.5).contains(speed)),
            "a cross into open hands landed at {speeds:.1?} m/s"
        );
        assert_eq!(hurt(&open), 0, "a cross into open hands tore something");

        let mut blocked = both;
        play(
            &mut blocked,
            0..60,
            &[(0, 1, Move::Block(true)), (20, 0, Move::Cross)],
        );
        let [head, arms] = reddened(&blocked, 1);
        assert!(
            arms > 0 && head == 0,
            "a blocked cross reddened {head} points of the head and trunk and {arms} of the arms"
        );
        assert_eq!(hurt(&blocked), 0, "a blocked cross tore something");
    }

    /// Fighters step no nearer each other than the nearest two fighters
    /// stand, and two that step in on the same step share the room left
    /// between them, so that neither is favoured: they stay each other's
    /// mirror images.
    #[test]
    fn fighters_step_no_nearer_than_they_stand() {
        let mut world = pair(0.60);
        let height = world.bodies[0].fighter.as_ref().unwrap().height * world.mm_per_px() * 0.001;
        let stood = hips_apart(&world);
        let both = |step: usize| [(step, 0, Move::StepIn), (step, 1, Move::StepIn)];
        play(&mut world, 0..62, &[both(0), both(30)].concat());
        let apart = hips_apart(&world);
        assert!(
            apart < stood - 0.05 * height && (apart - 0.50 * height).abs() < 0.02 * height,
            "stepping in from {stood:.2} m apart, the fighters' hips ended {apart:.2} m apart, where the nearest they stand is {:.2}",
            0.50 * height
        );
        let middle = crate::scenarios::SCENARIO_WIDTH * 0.5;
        let half = world.points.len() / 2;
        let uneven = (0..half)
            .map(|index| {
                let (a, b) = (
                    world.points[index].position,
                    world.points[half + index].position,
                );
                (a.x + b.x - 2.0 * middle).abs().max((a.y - b.y).abs())
            })
            .fold(0.0, f64::max);
        assert!(
            uneven < 1.0e-3,
            "stepping in together, the fighters stood {uneven:.2e} px off mirror images"
        );
    }

    /// A fighter shoved back holds where it stands under a light shove,
    /// steps back to catch itself under a harder one, and falls, and stays
    /// down, under one no step can catch (Hof, Gazendam and Sinke 2005's
    /// margin of stability; people pushed in experiments hold below a
    /// threshold and step above it).
    #[test]
    fn a_shoved_fighter_holds_steps_or_falls() {
        let base = pair(SIDE_FIGHTER_SPACING);
        let rig = base.bodies[0].fighter.clone().unwrap();
        let cm = base.mm_per_px() * 0.1;
        let shoved = |newton_seconds: i32| {
            let mut world = base.clone();
            let home = rig.feet.map(|foot| world.bones[foot].b.x);
            let mut moved = 0.0_f64;
            play(&mut world, 0..1, &[(0, 0, Move::Shoved(newton_seconds))]);
            for step in 1..150 {
                play(&mut world, step..step + 1, &[]);
                for (side, &foot) in rig.feet.iter().enumerate() {
                    moved = moved.max((world.bones[foot].b.x - home[side]).abs() * cm);
                }
            }
            (moved, world.fighter_fallen(0), world)
        };
        let (moved, fallen, _) = shoved(30);
        assert!(
            moved < 3.0 && !fallen,
            "shoved with 30 N s, a foot moved {moved:.1} cm (fallen: {fallen})"
        );
        let (moved, fallen, _) = shoved(90);
        assert!(
            moved > 10.0 && !fallen,
            "shoved with 90 N s, a foot moved {moved:.1} cm (fallen: {fallen})"
        );
        let (_, fallen, world) = shoved(200);
        let pelvis = world.bones[rig.pelvis];
        assert!(
            fallen && (world.floor_y() - pelvis.b.y) * cm < 50.0,
            "shoved with 200 N s, the fighter fell: {fallen}, its hips {:.0} cm up",
            (world.floor_y() - pelvis.b.y) * cm
        );
    }

    /// A fighter that ducks and steps back twice stays in the pose its
    /// moves put it in. Its legs keep its hips tilted as its moves have them
    /// (`Carry`): held by its feet alone, one of them in the air, it tipped
    /// forward about its hips until its head hung a metre from where its
    /// moves put it.
    #[test]
    fn a_ducking_fighter_stepping_back_keeps_its_pose() {
        let mut world = pair(SIDE_FIGHTER_SPACING);
        let moves = [
            (0, 0, Move::Duck(true)),
            (25, 0, Move::StepOut),
            (51, 0, Move::StepOut),
        ];
        let mut farthest = 0.0_f64;
        for step in 0..85 {
            play(&mut world, step..step + 1, &moves);
            if step >= 25 {
                farthest = farthest.max(world.head_off_pose(0).unwrap_or(0.0));
            }
        }
        assert!(
            farthest < 0.20,
            "ducking and stepping back, its head got {:.0} cm from where its moves put it",
            farthest * 100.0
        );
        assert_eq!(hurt(&world), 0, "ducking and stepping back tore something");
    }

    /// Two fighters blocking and ducking toward each other at the nearest
    /// they stand sit back on their hips rather than lean into each other
    /// (`Mover::crowded`), so each still reaches the pose its moves put it
    /// in, and nothing is flung or torn. Leaning in regardless, their blocks
    /// met and held them 20 cm short of their poses, their muscles pressing
    /// arm on arm.
    #[test]
    fn fighters_ducking_toward_each_other_sit_back() {
        let mut world = pair(0.50);
        let stood = hips_apart(&world);
        let both = |made: Move| [(0, 0, made), (0, 1, made)];
        let fastest = play(
            &mut world,
            0..50,
            &[both(Move::Block(true)), both(Move::Duck(true))].concat(),
        );
        let sat_back = hips_apart(&world) - stood;
        assert!(
            sat_back > 0.15,
            "ducking toward each other, the fighters' hips ended {:.0} cm further apart",
            sat_back * 100.0
        );
        for body in 0..2 {
            let off = world.head_off_pose(body).unwrap_or(0.0);
            assert!(
                off < 0.12,
                "ducking toward the other, fighter {body}'s head ended {:.0} cm from where its moves put it",
                off * 100.0
            );
        }
        assert!(
            fastest < 6.0,
            "ducking toward each other, a bone moved at {fastest:.1} m/s"
        );
        assert_eq!(hurt(&world), 0, "ducking toward each other tore something");
    }

    /// Random fights leave the fighters whole and in their poses: six
    /// fights of twenty seconds, both fighters making a move four times a
    /// second (`cargo test --release --lib random_fights_leave -- --ignored`;
    /// `verify.ps1 -Deep` runs it). `print_random_fights` prints what each
    /// did.
    #[test]
    #[ignore]
    fn random_fights_leave_the_fighters_whole() {
        let mut fibres = 0;
        for seed in 1..=6 {
            let seen = random_fight_watched(seed, 20.0, None, None);
            let stats = seen.world.stats;
            assert_eq!(
                (
                    stats.broken_bone_joints,
                    stats.bone_joint_subluxations,
                    stats.fractured_bones
                ),
                (0, 0, 0),
                "fight {seed} tore or put out a joint, or broke a bone"
            );
            assert!(
                seen.fastest < 20.0,
                "fight {seed} moved a bone at {:.1} m/s",
                seen.fastest
            );
            for (body, (farthest, _)) in seen.head_off.iter().enumerate() {
                assert!(
                    *farthest < 0.60,
                    "in fight {seed}, fighter {body}'s head got {:.0} cm from where its moves put it",
                    farthest * 100.0
                );
            }
            fibres += stats.broken_skin + stats.broken_muscle;
        }
        assert!(
            fibres <= 3,
            "six random fights tore {fibres} fibres of skin and muscle"
        );
    }

    /// A random fight played, and what was seen of it.
    struct FightSeen {
        world: World,
        /// How far apart the fighters' hips were, in metres, on each step
        /// that tore something or put a joint out, and how many things it
        /// tore; the first entry is the nearest they came, and tore nothing.
        tears: Vec<(f64, i32)>,
        /// Each move made: the frame, the fighter, and the move.
        moves: Vec<(usize, usize, Move)>,
        /// For each fighter, the farthest its head got from where its pose
        /// put it, in metres, and the first frame it was more than 25 cm
        /// off.
        head_off: [(f64, Option<usize>); 2],
        /// The fastest an end of any bone moved in a step, in metres a
        /// second.
        fastest: f64,
    }

    /// How fast the fastest end of any bone moved over the last step `dt`
    /// long, in metres a second.
    fn fastest_bone(world: &World, dt: f64) -> f64 {
        world
            .bones
            .iter()
            .map(|bone| distance(bone.a, bone.previous_a).max(distance(bone.b, bone.previous_b)))
            .fold(0.0, f64::max)
            / dt
            * world.mm_per_px()
            * 0.001
    }

    /// How far apart two fighters' hips are, in metres.
    fn hips_apart(world: &World) -> f64 {
        let hips = |body: usize| {
            let rig = world.bodies[body].fighter.as_ref().unwrap();
            let pelvis = world.bones[rig.pelvis];
            (pelvis.a.x + pelvis.b.x) * 0.5
        };
        (hips(1) - hips(0)).abs() * world.mm_per_px() * 0.001
    }

    /// Everything a fight has torn or put out of joint so far.
    fn torn_so_far(world: &World) -> i32 {
        world.stats.broken_skin
            + world.stats.broken_muscle
            + world.stats.broken_bone_joints
            + world.stats.bone_joint_subluxations
    }

    /// A random fight as [`random_fight`] plays it, with what was seen of
    /// it. With `flung`, a speed in metres a second, it stops at the first
    /// step that moves a bone's end faster than that, plays that step again
    /// from where it began with that bone traced, and prints what each phase
    /// of the step did to it. With `followed`, some frames and some bones,
    /// it prints what each phase of each of those frames did to each of
    /// those bones, and which limbs pressed on them.
    fn random_fight_watched(
        seed: u64,
        seconds: f64,
        flung: Option<f64>,
        followed: Option<(std::ops::RangeInclusive<usize>, Vec<usize>)>,
    ) -> FightSeen {
        let mut world = create_side_fighters(materials());
        world.keep_journal();
        let dt = world.materials.fixed_dt;
        let mut tears = vec![(hips_apart(&world), 0)];
        let mut moves = Vec::new();
        let mut head_off = [(0.0_f64, None); 2];
        let mut fastest = 0.0_f64;
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut held = [[false; 3]; 2];
        let steps = (seconds / dt) as usize;
        for frame in 0..steps {
            let mut input = InputState::default();
            for body in 0..2 {
                // A move about four times a second.
                if next() % 15 != 0 {
                    continue;
                }
                let made = match next() % 10 {
                    0 | 1 => Move::Jab,
                    2 | 3 => Move::Cross,
                    4 => Move::StepIn,
                    5 => Move::StepOut,
                    6 => {
                        held[body][0] = !held[body][0];
                        Move::LeanBack(held[body][0])
                    }
                    7 => {
                        held[body][1] = !held[body][1];
                        Move::Duck(held[body][1])
                    }
                    _ => {
                        held[body][2] = !held[body][2];
                        Move::Block(held[body][2])
                    }
                };
                input.moves[body] = Some((body, made));
                moves.push((frame + 1, body, made));
            }
            let before = flung.map(|_| world.clone());
            let torn = torn_so_far(&world);
            let follows = followed
                .as_ref()
                .filter(|(frames, _)| frames.contains(&(frame + 1)));
            if let Some((_, bones)) = follows {
                let traced: Vec<Traced> = bones.iter().map(|&bone| Traced::Bone(bone)).collect();
                world.trace(&traced);
            }
            world.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
            if let Some((_, bones)) = follows {
                println!("    frame {}: moves {:?}", frame + 1, &input.moves[..2]);
                // Where each fighter's head and hips are, and what it is
                // doing.
                for body in 0..2 {
                    let rig = world.bodies[body].fighter.as_ref().unwrap();
                    let (skull, pelvis) = (world.bones[rig.skull], world.bones[rig.pelvis]);
                    let head = lerp(skull.a, skull.b, 0.5);
                    println!(
                        "      fighter {body}: head ({:.0}, {:.0}), hips ({:.0}, {:.0}); {}",
                        head.x,
                        head.y,
                        pelvis.b.x,
                        pelvis.b.y,
                        world.mover_line(body),
                    );
                }
                for (target, &bone) in bones.iter().enumerate() {
                    let segment = world.bones[bone];
                    println!(
                        "      bone {bone} ({} of body {}, {:?}), {:?}, from ({:.0}, {:.0}) to ({:.0}, {:.0})",
                        segment.part.name(),
                        segment.body,
                        segment.limb,
                        world.stands(segment.body, segment.sheet),
                        segment.a.x,
                        segment.a.y,
                        segment.b.x,
                        segment.b.y,
                    );
                    for stage in world.traced_stages() {
                        let moved = hypot(stage.moved.x, stage.moved.y);
                        if stage.target == target && (moved > 1.0 || stage.turned.abs() > 0.02) {
                            println!(
                                "        {:>20} at {:.2}: moved it ({:+.1}, {:+.1}) px, turned it {:+.3} rad",
                                stage.phase, stage.time, stage.moved.x, stage.moved.y, stage.turned
                            );
                        }
                    }
                    let named = format!("bone {bone} (");
                    for line in world.traced_clashes() {
                        if line.contains(&named) && !line.ends_with(": 0 N") {
                            println!("        {line}");
                        }
                    }
                }
                world.trace(&[]);
            }
            fastest = fastest.max(fastest_bone(&world, dt));
            for (body, off) in head_off.iter_mut().enumerate() {
                let now = world.head_off_pose(body).unwrap_or(0.0);
                off.0 = off.0.max(now);
                if now > 0.25 && off.1.is_none() {
                    off.1 = Some(frame + 1);
                }
            }
            // `RP_POSTURE=10` prints, every tenth frame, how far each head
            // is from where its pose puts it, and what each fighter is
            // doing.
            let every: Option<usize> = std::env::var("RP_POSTURE")
                .ok()
                .and_then(|frames| frames.parse().ok());
            if every.is_some_and(|every| (frame + 1) % every.max(1) == 0) && followed.is_none() {
                println!(
                    "    frame {}: heads {:.0} and {:.0} cm off their poses
      {}
      {}",
                    frame + 1,
                    world.head_off_pose(0).unwrap_or(0.0) * 100.0,
                    world.head_off_pose(1).unwrap_or(0.0) * 100.0,
                    world.mover_line(0),
                    world.mover_line(1),
                );
            }
            // The first entry keeps the nearest the fighters came.
            let apart = hips_apart(&world);
            tears[0].0 = tears[0].0.min(apart);
            if torn_so_far(&world) > torn {
                tears.push((apart, torn_so_far(&world) - torn));
            }
            let (Some(limit), Some(mut before)) = (flung, before) else {
                continue;
            };
            let metres = world.mm_per_px() * 0.001;
            let fastest = (0..world.bones.len())
                .map(|index| {
                    let (now, then) = (world.bones[index], before.bones[index]);
                    (
                        distance(now.a, then.a).max(distance(now.b, then.b)) / dt * metres,
                        index,
                    )
                })
                .max_by(|p, q| p.0.total_cmp(&q.0))
                .unwrap();
            if fastest.0 > limit {
                let bone = world.bones[fastest.1];
                println!(
                    "frame {}: bone {} ({} of body {}, {:?}) moved at {:.1} m/s; moves this step {:?}",
                    world.journal().map_or(0, |journal| journal.steps.len()),
                    fastest.1,
                    bone.part.name(),
                    bone.body,
                    bone.limb,
                    fastest.0,
                    input.moves
                );
                before.trace(&[Traced::Bone(fastest.1)]);
                before.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
                // Which limbs pressed on each other in each substep.
                for line in before.traced_clashes() {
                    println!("    {line}");
                }
                for stage in before.traced_stages() {
                    let moved = hypot(stage.moved.x, stage.moved.y);
                    if moved > 2.0 {
                        println!(
                            "    {:>24}: moved it ({:+.1}, {:+.1}) px, turned it {:+.2} rad",
                            stage.phase, stage.moved.x, stage.moved.y, stage.turned
                        );
                    }
                }
                break;
            }
        }
        FightSeen {
            world,
            tears,
            moves,
            head_off,
            fastest,
        }
    }

    /// Prints what random fights do to the fighters, and how long a step
    /// takes (`cargo test --release --lib print_random_fights -- --ignored
    /// --nocapture`).
    #[test]
    #[ignore]
    fn print_random_fights() {
        // `RP_SEED=3` plays that fight alone and prints everything its
        // journal kept up to the first joint that tore.
        let only: Option<u64> = std::env::var("RP_SEED")
            .ok()
            .and_then(|seed| seed.parse().ok());
        // `RP_FIGHTS=24` plays that many fights instead of six.
        let fights: u64 = std::env::var("RP_FIGHTS")
            .ok()
            .and_then(|fights| fights.parse().ok())
            .unwrap_or(6);
        for seed in 1..=fights {
            if only.is_some_and(|only| only != seed) {
                continue;
            }
            let started = std::time::Instant::now();
            // `RP_SECONDS=30` plays each fight that long instead of 20 s.
            let seconds = std::env::var("RP_SECONDS")
                .ok()
                .and_then(|seconds| seconds.parse().ok())
                .unwrap_or(20.0);
            // `RP_FLUNG=30` stops at the first bone moved faster than 30 m/s
            // and prints which phase moved it.
            let flung = std::env::var("RP_FLUNG")
                .ok()
                .and_then(|speed| speed.parse().ok());
            let FightSeen {
                world,
                tears,
                moves,
                head_off,
                fastest,
            } = random_fight_watched(seed, seconds, flung, None);
            // `RP_WHY=8` with `RP_SEED` plays the fight again and follows,
            // through the eight frames up to the first thing torn or put
            // out, the bones it happened at: a joint's two bones, or the
            // bones a torn fibre's two ends are held to.
            let why: Option<usize> = std::env::var("RP_WHY")
                .ok()
                .and_then(|frames| frames.parse().ok());
            let first_hurt = world.journal().and_then(|journal| {
                journal.events.iter().find_map(|event| {
                    let bones = match (event.kind, event.subject) {
                        (EventKind::JointSlipped | EventKind::JointTorn, Subject::Joint(joint)) => {
                            let joint = world.bone_joints.get(joint)?;
                            vec![joint.a, joint.b]
                        }
                        (EventKind::SkinTorn | EventKind::MuscleTorn, Subject::Spring(spring)) => {
                            let spring = world.springs.get(spring)?;
                            let mut bones: Vec<usize> = [spring.a, spring.b]
                                .iter()
                                .map(|&point| world.point_holds[point].posture.bones[0])
                                .collect();
                            bones.dedup();
                            bones
                        }
                        _ => return None,
                    };
                    Some((event.step as usize + 1, bones))
                })
            });
            if let (Some(frames), Some((hurt, bones)), true) = (why, first_hurt, only.is_some()) {
                println!("  why the first thing tore, at frame {hurt}:");
                random_fight_watched(
                    seed,
                    hurt as f64 * world.materials.fixed_dt,
                    None,
                    Some((hurt.saturating_sub(frames)..=hurt, bones)),
                );
            }
            let steps = seconds / world.materials.fixed_dt;
            println!(
                "seed {seed}: joints torn {}, slipped {}, bones broken {}, skin torn {}, muscle torn {}, bruises {}, {:.2} ms a step",
                world.stats.broken_bone_joints,
                world.stats.bone_joint_subluxations,
                world.stats.fractured_bones,
                world.stats.broken_skin,
                world.stats.broken_muscle,
                world.stats.contusion_events,
                started.elapsed().as_secs_f64() * 1000.0 / steps,
            );
            println!("    the fastest a bone moved was {fastest:.1} m/s");
            // `RP_GESTURE=1` with `RP_SEED` writes the fight as a gesture the
            // app's capture mode plays (`--side --gesture`), to film it.
            if only.is_some() && std::env::var("RP_GESTURE").is_ok() {
                let on = |held: bool| if held { "on" } else { "off" };
                let mut gesture = String::from("hammer:-1.6,0.95");
                let mut at = 0;
                for &(frame, body, made) in &moves {
                    let frame = frame - 1;
                    if frame > at {
                        gesture += &format!(":wait={}", frame - at);
                        at = frame;
                    }
                    let who = if body == 0 { "" } else { "2" };
                    gesture += &match made {
                        Move::Jab => format!(":jab{who}"),
                        Move::Cross => format!(":cross{who}"),
                        Move::StepIn => format!(":step{who}=in"),
                        Move::StepOut => format!(":step{who}=out"),
                        Move::LeanBack(held) => format!(":back{who}={}", on(held)),
                        Move::Duck(held) => format!(":duck{who}={}", on(held)),
                        Move::Block(held) => format!(":block{who}={}", on(held)),
                        Move::Shoved(newton_seconds) => format!(":shove{who}={newton_seconds}"),
                    };
                }
                gesture += &format!(":wait={}", (steps as usize).saturating_sub(at));
                println!("    gesture: {gesture}");
            }
            // How hard each head was turned, and whether it was knocked out
            // (`knockout.rs`).
            for body in 0..2 {
                let (hardest, spin, changed) = world.hardest_head_spin(body).unwrap_or_default();
                let out = world.journal().and_then(|journal| {
                    journal
                        .events
                        .iter()
                        .find(|event| {
                            event.kind == EventKind::KnockedOut
                                && world.bone_body(match event.subject {
                                    Subject::Bone(bone) => bone,
                                    _ => MISSING_BONE,
                                }) == body
                        })
                        .map(|event| event.step + 1)
                });
                println!(
                    "    fighter {body}'s head was turned at up to {hardest:.0} rad/s2 and {spin:.1} rad/s, its turning changed by up to {changed:.1} rad/s within a blow{}{}",
                    out.map_or(String::new(), |frame| format!(", knocked out at frame {frame}")),
                    if world.fighter_fallen(body) && out.is_none() {
                        ", fallen"
                    } else {
                        ""
                    },
                );
            }
            // How well each body followed its moves.
            for (body, (farthest, first)) in head_off.iter().enumerate() {
                println!(
                    "    fighter {body}'s head got {:.0} cm from where its pose put it{}",
                    farthest * 100.0,
                    first.map_or(String::new(), |frame| format!(
                        ", more than 25 cm from frame {frame}"
                    )),
                );
                // With `RP_SEED`, every move either fighter made up to then.
                if let (Some(first), true) = (first, only.is_some()) {
                    for (frame, mover, made) in &moves {
                        if frame <= first {
                            println!("      frame {frame}: fighter {mover} {made:?}");
                        }
                    }
                }
            }
            // How near the fighters stood when things tore: they begin
            // `SIDE_FIGHTER_SPACING` apart.
            let within = |from: f64, to: f64| -> i32 {
                tears[1..]
                    .iter()
                    .filter(|(apart, _)| (from..to).contains(apart))
                    .map(|(_, torn)| torn)
                    .sum()
            };
            println!(
                "    hips {:.2} m apart to begin with, nearest {:.2} m; torn with the hips under 0.6 m apart {}, to 0.9 m {}, to 1.2 m {}, farther {}",
                hips_apart(&create_side_fighters(Materials::default())),
                tears[0].0,
                within(0.0, 0.6),
                within(0.6, 0.9),
                within(0.9, 1.2),
                within(1.2, f64::INFINITY),
            );
            // What tore, slipped, or broke, from the journal.
            if let Some(journal) = world.journal() {
                let story = crate::inspect::story(&world, journal);
                if only.is_some() {
                    let until = story
                        .iter()
                        .position(|line| {
                            ["joint torn", "skin torn", "muscle torn"]
                                .iter()
                                .any(|word| line.contains(word))
                        })
                        .map_or(story.len(), |at| at + 4);
                    for line in &story[until.saturating_sub(40)..until.min(story.len())] {
                        println!("    {line}");
                    }
                    // The moves made in the second and a half before the
                    // first joint was put out or torn.
                    let hurt = journal
                        .events
                        .iter()
                        .find(|event| {
                            matches!(
                                event.kind,
                                EventKind::JointSlipped
                                    | EventKind::JointTorn
                                    | EventKind::SkinTorn
                                    | EventKind::MuscleTorn
                            )
                        })
                        .map_or(usize::MAX, |event| event.step as usize + 1);
                    let lately = hurt.saturating_sub(90)..=hurt;
                    for (frame, body, made) in &moves {
                        if lately.contains(frame) {
                            println!("    frame {frame}: fighter {body} {made:?}");
                        }
                    }
                    // And the punches that landed or gave way in it.
                    for event in &journal.events {
                        if event.kind == EventKind::PunchTurnedBack
                            && lately.contains(&(event.step as usize + 1))
                        {
                            println!(
                                "    frame {}: punch turned back, {:?} by {:?}, {:?} {:.1} (limit {:.1})",
                                event.step + 1,
                                event.subject,
                                event.by,
                                event.measure,
                                event.value,
                                event.limit
                            );
                        }
                    }
                    continue;
                }
                for line in story {
                    if ["torn", "slipped", "broke", "came loose"]
                        .iter()
                        .any(|word| line.contains(word))
                    {
                        println!("    {}", line.trim());
                    }
                }
            }
        }
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
