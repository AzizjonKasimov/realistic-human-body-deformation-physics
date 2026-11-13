//! Side-view mannequin figure that a fighter's body is built from.
//!
//! The sandbox's figure faces the viewer (`crate::silhouette`), which suits
//! a body that stands to be struck. Two fighters face each other, so a
//! fighter is seen from the side: what a fight is made of, stepping in and
//! out, punching, leaning away, and ducking, all happens in that plane. Seen
//! from the side an arm lies in front of the chest and a leg in front of the
//! other, so the figure is not one outline but parts lying over one another:
//! the far arm and leg, the trunk with the head, then the near leg and arm.
//! Each part of a limb is a shape of its own, rounded at both ends and
//! overlapping the next at the joint, as an artist's wooden mannequin is
//! jointed, so an elbow or a knee turns through its whole range without
//! stretching or folding any flesh.
//!
//! A place in the figure is given in the fighter's own frame: how far forward
//! of where it stands, toward its opponent, and how high above the floor,
//! both in body heights. A [`Pose`] says how the figure holds itself with a
//! handful of numbers, and [`Joints`] works out where every joint then lies:
//! the trunk from each of its bones' lean, the legs and arms by reaching for
//! their feet and fists. `simulation::fighter` builds the body in the stance
//! on these joints, and `simulation::mover` moves it through other poses.
//!
//! Proportions follow Winter (2009) and de Leva (1996) for a 1.75 m man:
//! hip joint 0.530 of the height above the floor, knee 0.285, shoulder 0.818,
//! upper arm 0.186, forearm 0.146.

use crate::silhouette::{capsule, smooth_union, Circle, Landmark, SilhouetteField};

/// A place in the fighter's own frame: forward of where it stands, and above
/// the floor, in body heights.
pub(crate) type Place = (f64, f64);

/// The limbs' lengths between the points their joints turn about.
pub(crate) const THIGH: f64 = 0.245;
pub(crate) const SHIN: f64 = 0.254;
pub(crate) const UPPER_ARM: f64 = 0.186;
pub(crate) const FOREARM: f64 = 0.146;
/// From the wrist to the end of the hand's bone, inside the fist.
pub(crate) const HAND: f64 = 0.045;

/// How high the hip joints stand above the floor with the legs straight.
pub(crate) const HIP_HEIGHT: f64 = 0.530;
/// How high the ankle turns above the sole.
const ANKLE_HEIGHT: f64 = HIP_HEIGHT - THIGH - SHIN;

/// The trunk's bones, each from the joint below it: where its upper joint
/// lies from its lower one when it stands upright. The spine runs up the
/// back of the trunk, behind the middle of the chest and belly.
const PELVIS: Place = (-0.022, 0.058);
const LUMBAR: Place = (-0.012, 0.102);
const THORACIC: Place = (0.010, 0.148);
const CERVICAL: Place = (0.018, 0.060);
const SKULL: Place = (0.008, 0.074);
/// Where a shoulder sits on the chest, from the thoracic spine's lower end,
/// with the trunk square to the viewer: in the middle of the chest's depth.
const SHOULDER_SEAT: Place = (0.034, 0.128);

/// The foot, from the point its ankle turns about: the ends of its bone, at
/// the heel and under the ball of the foot, and where the ball touches the
/// floor.
const FOOT_HEEL_END: Place = (-0.020, 0.00233);
const FOOT_BALL_END: Place = (0.084, -0.00977);
const BALL_ON_FLOOR: Place = (0.085, -ANKLE_HEIGHT);

/// Where the flesh of the trunk and head is centred on each bone, from the
/// bone's lower joint with the bone upright, and how far it reaches round
/// there. The buttocks and the back of the hips, the waist, the chest, the
/// top of the chest under the shoulders, the neck, and the head's crown and
/// jaw.
const HIPS_FLESH: Circle = ((-0.012, 0.028), 0.068);
const WAIST_FLESH: Circle = ((0.034, 0.052), 0.056);
const CHEST_FLESH: Circle = ((0.048, 0.070), 0.070);
const SHOULDERS_FLESH: Circle = ((0.036, 0.122), 0.045);
const NECK_LOW: Circle = ((0.020, 0.008), 0.031);
const NECK_HIGH: Circle = ((0.030, 0.054), 0.030);
const CROWN: Circle = ((0.016, 0.042), 0.056);
const JAW: Circle = ((0.036, -0.005), 0.036);

/// Limb radii at the joint each part starts from and at the joint it ends at.
const UPPER_ARM_RADII: (f64, f64) = (0.034, 0.027);
const FOREARM_RADII: (f64, f64) = (0.027, 0.020);
const THIGH_RADII: (f64, f64) = (0.052, 0.036);
const SHIN_RADII: (f64, f64) = (0.035, 0.022);
/// The fist: a short thick capsule round the hand's bone, about 9.5 cm
/// across.
const FIST_RADIUS: f64 = 0.027;
const FIST_FROM: f64 = 0.012;
const FIST_TO: f64 = 0.040;
/// The foot's flesh, from the point its ankle turns about: round the heel,
/// round the toes, and round the ankle.
const HEEL_FLESH: Circle = ((-0.022, 0.020 - ANKLE_HEIGHT), 0.020);
const TOES_FLESH: Circle = ((0.090, 0.0155 - ANKLE_HEIGHT), 0.0155);
const ANKLE_FLESH: Circle = ((-0.002, 0.004), 0.026);

/// Widths of the smooth blends between the trunk's parts and within a foot
/// and a forearm.
const TRUNK_BLEND: f64 = 0.030;
const NECK_BLEND: f64 = 0.024;
const FIST_BLEND: f64 = 0.010;
const FOOT_BLEND: f64 = 0.016;

/// How far past a part's shapes its field is sampled.
const FIELD_MARGIN: f64 = 0.03;

/// The two sides of the body: the lead side, farther from the viewer, whose
/// arm and leg stand toward the opponent, and the rear side, nearer the
/// viewer. A fighter facing right stands as an orthodox boxer does, its left
/// side leading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Lead,
    Rear,
}

impl Side {
    pub(crate) const BOTH: [Side; 2] = [Side::Lead, Side::Rear];

    pub(crate) fn index(self) -> usize {
        match self {
            Side::Lead => 0,
            Side::Rear => 1,
        }
    }
}

/// A foot: where the ball of it is, on the floor or lifted, and how far its
/// heel is raised, in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Foot {
    pub(crate) ball: Place,
    pub(crate) pitch: f64,
}

/// How the figure holds itself: a handful of numbers a move sets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pose {
    /// Where the hip joints are.
    pub(crate) hips: Place,
    /// How far each of the trunk's bones leans forward from upright, in
    /// radians: the pelvis, the lower back, the chest, the neck, and the
    /// head.
    pub(crate) pelvis: f64,
    pub(crate) lumbar: f64,
    pub(crate) thorax: f64,
    pub(crate) neck: f64,
    pub(crate) head: f64,
    /// How far each shoulder has slid forward across the chest as the trunk
    /// turns, the lead side's first.
    pub(crate) shoulders: [f64; 2],
    /// Where each fist's knuckles are, the lead side's first.
    pub(crate) fists: [Place; 2],
    /// Each foot, the lead side's first.
    pub(crate) feet: [Foot; 2],
}

/// `v` turned `lean` radians toward the front: what stands upright leans
/// forward.
pub(crate) fn leaned(v: Place, lean: f64) -> Place {
    let (sin, cos) = (libm::sin(lean), libm::cos(lean));
    (v.0 * cos + v.1 * sin, -v.0 * sin + v.1 * cos)
}

fn plus(a: Place, b: Place) -> Place {
    (a.0 + b.0, a.1 + b.1)
}

fn minus(a: Place, b: Place) -> Place {
    (a.0 - b.0, a.1 - b.1)
}

fn times(a: Place, k: f64) -> Place {
    (a.0 * k, a.1 * k)
}

fn length(a: Place) -> f64 {
    libm::hypot(a.0, a.1)
}

/// The joint between two bones `first` and `second` long that reach from
/// `root` to `end`, bent to the side `bend` (1 counterclockwise of the line
/// from root to end, seen with the front to the right and up upward, -1
/// clockwise), and where the far bone ends: `end`, or as near it as the two
/// bones reach.
pub(crate) fn reach(root: Place, end: Place, first: f64, second: f64, bend: f64) -> (Place, Place) {
    let line = minus(end, root);
    let far = length(line);
    let span = far.clamp((first - second).abs() + 1.0e-6, first + second - 1.0e-6);
    let cosine =
        ((first * first + span * span - second * second) / (2.0 * first * span)).clamp(-1.0, 1.0);
    let angle = libm::atan2(line.1, line.0) + bend * libm::acos(cosine);
    let joint = plus(root, (first * libm::cos(angle), first * libm::sin(angle)));
    let reached = if far > 1.0e-9 {
        plus(root, times(line, span / far))
    } else {
        plus(root, (span, 0.0))
    };
    (joint, reached)
}

/// Where every joint of the figure lies in a pose, and the ends of the bones
/// that do not end at a joint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Joints {
    /// The hip joints' middle, the joint at the foot of the spine, the joint
    /// between the lower back and the chest, the foot of the neck, the base
    /// of the skull, and the top of the skull's bone.
    pub(crate) hips: Place,
    pub(crate) sacrum: Place,
    pub(crate) waist: Place,
    pub(crate) neck: Place,
    pub(crate) skull_base: Place,
    pub(crate) skull_top: Place,
    /// Each side's joints, the lead side's first.
    pub(crate) hip: [Place; 2],
    pub(crate) knee: [Place; 2],
    pub(crate) ankle: [Place; 2],
    pub(crate) heel: [Place; 2],
    pub(crate) ball: [Place; 2],
    pub(crate) shoulder: [Place; 2],
    pub(crate) elbow: [Place; 2],
    pub(crate) wrist: [Place; 2],
    pub(crate) knuckles: [Place; 2],
    /// How far each of the trunk's bones leans, as the pose gave them, and
    /// each foot's pitch.
    pub(crate) leans: [f64; 5],
    pub(crate) pitch: [f64; 2],
}

/// How far each hip joint lies forward of their middle along the pelvis, the
/// lead side's first: the pelvis stands turned a little, the lead hip
/// forward.
const HIP_TURN: [f64; 2] = [0.010, -0.010];

impl Joints {
    pub(crate) fn of(pose: &Pose) -> Self {
        let hips = pose.hips;
        let sacrum = plus(hips, leaned(PELVIS, pose.pelvis));
        let waist = plus(sacrum, leaned(LUMBAR, pose.lumbar));
        let neck = plus(waist, leaned(THORACIC, pose.thorax));
        let skull_base = plus(neck, leaned(CERVICAL, pose.neck));
        let skull_top = plus(skull_base, leaned(SKULL, pose.head));
        let mut joints = Joints {
            hips,
            sacrum,
            waist,
            neck,
            skull_base,
            skull_top,
            hip: [hips; 2],
            knee: [hips; 2],
            ankle: [hips; 2],
            heel: [hips; 2],
            ball: [hips; 2],
            shoulder: [hips; 2],
            elbow: [hips; 2],
            wrist: [hips; 2],
            knuckles: [hips; 2],
            leans: [pose.pelvis, pose.lumbar, pose.thorax, pose.neck, pose.head],
            pitch: [pose.feet[0].pitch, pose.feet[1].pitch],
        };
        for side in 0..2 {
            // The leg reaches from its hip to its ankle, the knee forward.
            let hip = plus(hips, leaned((HIP_TURN[side], 0.0), pose.pelvis));
            let foot = pose.feet[side];
            let ankle = minus(foot.ball, leaned(BALL_ON_FLOOR, foot.pitch));
            let (knee, ankle) = reach(hip, ankle, THIGH, SHIN, 1.0);
            joints.hip[side] = hip;
            joints.knee[side] = knee;
            joints.ankle[side] = ankle;
            joints.heel[side] = plus(ankle, leaned(FOOT_HEEL_END, foot.pitch));
            joints.ball[side] = plus(ankle, leaned(FOOT_BALL_END, foot.pitch));
            // The arm reaches from its shoulder to its knuckles, the wrist
            // straight and the elbow below the line between them.
            let shoulder = plus(
                waist,
                leaned(
                    (SHOULDER_SEAT.0 + pose.shoulders[side], SHOULDER_SEAT.1),
                    pose.thorax,
                ),
            );
            let (elbow, knuckles) =
                reach(shoulder, pose.fists[side], UPPER_ARM, FOREARM + HAND, -1.0);
            let along = minus(knuckles, elbow);
            joints.shoulder[side] = shoulder;
            joints.elbow[side] = elbow;
            joints.wrist[side] = plus(elbow, times(along, FOREARM / (FOREARM + HAND)));
            joints.knuckles[side] = knuckles;
        }
        joints
    }

    /// Where the middle of the head is: between the crown and the jaw.
    pub(crate) fn head(&self) -> Place {
        let lean = self.leans[4];
        let crown = plus(self.skull_base, leaned(CROWN.0, lean));
        let jaw = plus(self.skull_base, leaned(JAW.0, lean));
        times(plus(crown, jaw), 0.5)
    }

    /// Where a shoulder would sit with the trunk square to the viewer, the
    /// place its slide across the chest is measured from.
    pub(crate) fn square_shoulder(&self) -> Place {
        plus(self.waist, leaned(SHOULDER_SEAT, self.leans[2]))
    }

    /// The way the chest faces: forward along it, as a shoulder slides.
    pub(crate) fn chest_forward(&self) -> Place {
        leaned((1.0, 0.0), self.leans[2])
    }
}

/// A joint of the figure, between two of its bones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hinge {
    /// The lower back, between the pelvis and the lumbar spine.
    Back,
    /// Between the lumbar spine and the chest's.
    Mid,
    /// The foot of the neck, and the head on the neck.
    Neck,
    Head,
    Shoulder(Side),
    Elbow(Side),
    Wrist(Side),
    Hip(Side),
    Knee(Side),
    Ankle(Side),
}

impl Hinge {
    /// Every joint of the figure that a pose turns.
    pub(crate) const ALL: [Hinge; 16] = [
        Hinge::Back,
        Hinge::Mid,
        Hinge::Neck,
        Hinge::Head,
        Hinge::Shoulder(Side::Lead),
        Hinge::Elbow(Side::Lead),
        Hinge::Wrist(Side::Lead),
        Hinge::Hip(Side::Lead),
        Hinge::Knee(Side::Lead),
        Hinge::Ankle(Side::Lead),
        Hinge::Shoulder(Side::Rear),
        Hinge::Elbow(Side::Rear),
        Hinge::Wrist(Side::Rear),
        Hinge::Hip(Side::Rear),
        Hinge::Knee(Side::Rear),
        Hinge::Ankle(Side::Rear),
    ];
}

/// Which way each bone of the figure runs in a pose: the angle of the line
/// from its first joint to its second, counterclockwise from forward as seen
/// with the front to the right, in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Directions {
    /// The pelvis from the foot of the spine to the hips, then the trunk's
    /// bones upward.
    pub(crate) pelvis: f64,
    pub(crate) lumbar: f64,
    pub(crate) thoracic: f64,
    pub(crate) cervical: f64,
    pub(crate) skull: f64,
    /// Each side's bones, the lead side's first; the foot's from its heel to
    /// its ball.
    pub(crate) upper_arm: [f64; 2],
    pub(crate) forearm: [f64; 2],
    pub(crate) hand: [f64; 2],
    pub(crate) thigh: [f64; 2],
    pub(crate) shin: [f64; 2],
    pub(crate) foot: [f64; 2],
}

impl Directions {
    /// How far `hinge`'s second bone is turned from its first, in radians,
    /// counterclockwise.
    pub(crate) fn turn(&self, hinge: Hinge) -> f64 {
        match hinge {
            Hinge::Back => self.lumbar - self.pelvis,
            Hinge::Mid => self.thoracic - self.lumbar,
            Hinge::Neck => self.cervical - self.thoracic,
            Hinge::Head => self.skull - self.cervical,
            Hinge::Shoulder(side) => self.upper_arm[side.index()] - self.thoracic,
            Hinge::Elbow(side) => self.forearm[side.index()] - self.upper_arm[side.index()],
            Hinge::Wrist(side) => self.hand[side.index()] - self.forearm[side.index()],
            Hinge::Hip(side) => self.thigh[side.index()] - self.pelvis,
            Hinge::Knee(side) => self.shin[side.index()] - self.thigh[side.index()],
            Hinge::Ankle(side) => self.foot[side.index()] - self.shin[side.index()],
        }
    }
}

impl Joints {
    pub(crate) fn directions(&self) -> Directions {
        let way = |from: Place, to: Place| libm::atan2(to.1 - from.1, to.0 - from.0);
        let sides = |from: &[Place; 2], to: &[Place; 2]| [way(from[0], to[0]), way(from[1], to[1])];
        Directions {
            pelvis: way(self.sacrum, self.hips),
            lumbar: way(self.sacrum, self.waist),
            thoracic: way(self.waist, self.neck),
            cervical: way(self.neck, self.skull_base),
            skull: way(self.skull_base, self.skull_top),
            upper_arm: sides(&self.shoulder, &self.elbow),
            forearm: sides(&self.elbow, &self.wrist),
            hand: sides(&self.wrist, &self.knuckles),
            thigh: sides(&self.hip, &self.knee),
            shin: sides(&self.knee, &self.ankle),
            foot: sides(&self.heel, &self.ball),
        }
    }
}

/// The figure standing upright, its arms hanging and its feet flat, each
/// joint at the middle its range is measured from.
pub(crate) fn upright() -> Pose {
    let hang = |shoulder: Place| (shoulder.0 + 0.004, shoulder.1 - UPPER_ARM - FOREARM - HAND);
    let mut pose = Pose {
        hips: (0.0, HIP_HEIGHT),
        pelvis: 0.0,
        lumbar: 0.0,
        thorax: 0.0,
        neck: 0.0,
        head: 0.0,
        shoulders: [0.0; 2],
        fists: [(0.0, 0.0); 2],
        feet: [Foot {
            ball: (BALL_ON_FLOOR.0, 0.0),
            pitch: 0.0,
        }; 2],
    };
    let shoulder = Joints::of(&pose).square_shoulder();
    pose.fists = [hang(shoulder); 2];
    pose
}

/// The fighting stance the body is built in, standing where its hips are
/// over the floor's zero: an orthodox boxer's, the lead foot and shoulder
/// toward the opponent, the knees a little bent, the trunk leaning in with
/// the chin down, the rear heel off the floor, the lead fist out in front of
/// the face and the rear fist by the chin, the elbows down in front of the
/// ribs.
pub(crate) fn stance() -> Pose {
    let mut pose = Pose {
        hips: (0.0, 0.495),
        pelvis: 0.05,
        lumbar: 0.10,
        thorax: 0.20,
        neck: 0.28,
        head: 0.14,
        shoulders: [0.050, -0.035],
        fists: [(0.0, 0.0); 2],
        feet: [
            Foot {
                ball: (0.235, 0.0),
                pitch: 0.0,
            },
            Foot {
                ball: (-0.075, 0.0),
                pitch: 0.32,
            },
        ],
    };
    // The fists stand by the head, wherever the trunk's lean puts it.
    let head = Joints::of(&pose).head();
    pose.fists = guard_fists(head);
    pose
}

/// Where the stance holds the fists' knuckles for a head at `head`: the lead
/// fist out in front at the height of the chin, the rear fist under the
/// chin, both below the face and, in a real stance, one each side of the
/// line a straight punch comes down.
pub(crate) fn guard_fists(head: Place) -> [Place; 2] {
    [plus(head, (0.168, -0.060)), plus(head, (0.090, -0.080))]
}

/// Where a block holds the fists' knuckles for a head at `head`: both at
/// the brow, each straight above its elbow, so the forearms stand upright
/// between the face and a punch. Held closer in, the forearms leaned back
/// from elbows out in front, and a fist that met them slid up and over
/// them.
pub(crate) fn block_fists(head: Place) -> [Place; 2] {
    [plus(head, (0.155, 0.010)), plus(head, (0.095, 0.004))]
}

/// A part of the figure with flesh of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    UpperArm(Side),
    Forearm(Side),
    Thigh(Side),
    Shin(Side),
    Foot(Side),
    Trunk,
}

impl Part {
    /// The figure's parts from the farthest from the viewer to the nearest:
    /// the order they lie over one another in, and are drawn in.
    pub(crate) const FAR_TO_NEAR: [Part; 11] = [
        Part::UpperArm(Side::Lead),
        Part::Forearm(Side::Lead),
        Part::Thigh(Side::Lead),
        Part::Shin(Side::Lead),
        Part::Foot(Side::Lead),
        Part::Trunk,
        Part::Thigh(Side::Rear),
        Part::Shin(Side::Rear),
        Part::Foot(Side::Rear),
        Part::UpperArm(Side::Rear),
        Part::Forearm(Side::Rear),
    ];
}

/// A place as the fields take it: `u` forward, `v` down from one body
/// height above the floor.
fn field_place(place: Place) -> Landmark {
    (place.0, 1.0 - place.1)
}

/// The signed distance, in body heights, from `p` to the outline of `part`
/// with the figure's joints at `joints`; negative inside.
pub(crate) fn part_distance(part: Part, joints: &Joints, p: Place) -> f64 {
    let p = field_place(p);
    let round = |at: Place, radius: f64| -> Circle { (field_place(at), radius) };
    let limb = |from: Place, to: Place, radii: (f64, f64)| {
        capsule(p, round(from, radii.0), round(to, radii.1))
    };
    match part {
        Part::UpperArm(side) => {
            let side = side.index();
            limb(joints.shoulder[side], joints.elbow[side], UPPER_ARM_RADII)
        }
        Part::Forearm(side) => {
            let side = side.index();
            let (elbow, wrist) = (joints.elbow[side], joints.wrist[side]);
            let along = times(minus(joints.knuckles[side], wrist), 1.0 / HAND);
            let fist = limb(
                plus(wrist, times(along, FIST_FROM)),
                plus(wrist, times(along, FIST_TO)),
                (FIST_RADIUS, FIST_RADIUS),
            );
            smooth_union(limb(elbow, wrist, FOREARM_RADII), fist, FIST_BLEND)
        }
        Part::Thigh(side) => {
            let side = side.index();
            limb(joints.hip[side], joints.knee[side], THIGH_RADII)
        }
        Part::Shin(side) => {
            let side = side.index();
            limb(joints.knee[side], joints.ankle[side], SHIN_RADII)
        }
        Part::Foot(side) => {
            let side = side.index();
            let (ankle, pitch) = (joints.ankle[side], joints.pitch[side]);
            let at = |shape: Circle| round(plus(ankle, leaned(shape.0, pitch)), shape.1);
            let (round_ankle, radius) = at(ANKLE_FLESH);
            smooth_union(
                capsule(p, at(HEEL_FLESH), at(TOES_FLESH)),
                length(minus(p, round_ankle)) - radius,
                FOOT_BLEND,
            )
        }
        Part::Trunk => {
            let on = |joint: Place, lean: f64, shape: Circle| {
                round(plus(joint, leaned(shape.0, lean)), shape.1)
            };
            let [pelvis, lumbar, thorax, neck, head] = joints.leans;
            let hips = on(joints.hips, pelvis, HIPS_FLESH);
            let waist = on(joints.sacrum, lumbar, WAIST_FLESH);
            let chest = on(joints.waist, thorax, CHEST_FLESH);
            let shoulders = on(joints.waist, thorax, SHOULDERS_FLESH);
            let neck_low = on(joints.neck, neck, NECK_LOW);
            let neck_high = on(joints.neck, neck, NECK_HIGH);
            let crown = on(joints.skull_base, head, CROWN);
            let jaw = on(joints.skull_base, head, JAW);
            let mut trunk = capsule(p, hips, waist);
            for (from, to) in [(waist, chest), (chest, shoulders)] {
                trunk = smooth_union(trunk, capsule(p, from, to), TRUNK_BLEND);
            }
            let head = capsule(p, crown, jaw);
            let neck = capsule(p, neck_low, neck_high);
            smooth_union(smooth_union(trunk, neck, NECK_BLEND), head, NECK_BLEND)
        }
    }
}

/// The field of `part` with the figure's joints at `joints`, sampled over the
/// box that holds it.
pub(crate) fn part_field(part: Part, joints: &Joints) -> SilhouetteField {
    let mut low = (f64::INFINITY, f64::INFINITY);
    let mut high = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut hold = |at: Place, radius: f64| {
        low = (low.0.min(at.0 - radius), low.1.min(at.1 - radius));
        high = (high.0.max(at.0 + radius), high.1.max(at.1 + radius));
    };
    match part {
        Part::UpperArm(side) => {
            let side = side.index();
            hold(joints.shoulder[side], UPPER_ARM_RADII.0);
            hold(joints.elbow[side], UPPER_ARM_RADII.0);
        }
        Part::Forearm(side) => {
            let side = side.index();
            hold(joints.elbow[side], FOREARM_RADII.0);
            hold(joints.knuckles[side], FIST_RADIUS + 0.01);
        }
        Part::Thigh(side) => {
            let side = side.index();
            hold(joints.hip[side], THIGH_RADII.0);
            hold(joints.knee[side], THIGH_RADII.0);
        }
        Part::Shin(side) => {
            let side = side.index();
            hold(joints.knee[side], SHIN_RADII.0);
            hold(joints.ankle[side], SHIN_RADII.0);
        }
        Part::Foot(side) => {
            let side = side.index();
            hold(joints.ankle[side], 0.14);
        }
        Part::Trunk => {
            hold(joints.hips, 0.12);
            hold(joints.waist, 0.14);
            hold(joints.neck, 0.12);
            hold(joints.skull_top, 0.12);
        }
    }
    let (low, high) = (
        (low.0 - FIELD_MARGIN, low.1 - FIELD_MARGIN),
        (high.0 + FIELD_MARGIN, high.1 + FIELD_MARGIN),
    );
    // `v` runs down from one body height above the floor.
    SilhouetteField::sampled((low.0, high.0), (1.0 - high.1, 1.0 - low.1), |(u, v)| {
        part_distance(part, joints, (u, 1.0 - v))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Standing upright with the arms hanging, the figure is as tall as a
    /// body height and its joints stand where a man's do.
    #[test]
    fn the_figure_stands_a_body_height_tall() {
        let pose = Pose {
            hips: (0.0, HIP_HEIGHT),
            pelvis: 0.0,
            lumbar: 0.0,
            thorax: 0.0,
            neck: 0.0,
            head: 0.0,
            shoulders: [0.0; 2],
            fists: [(0.0, 0.3); 2],
            feet: [Foot {
                ball: (BALL_ON_FLOOR.0, 0.0),
                pitch: 0.0,
            }; 2],
        };
        let joints = Joints::of(&pose);
        let top = (0..400)
            .map(|step| 1.2 - step as f64 * 0.001)
            .find(|&y| part_distance(Part::Trunk, &joints, (0.01, y)) < 0.0)
            .unwrap();
        assert!((top - 1.0).abs() < 0.01, "the crown stands at {top}");
        assert!((joints.shoulder[0].1 - 0.818).abs() < 0.002);
        assert!((joints.knee[0].1 - 0.285).abs() < 0.002);
        // The sole lies on the floor.
        let sole = part_distance(Part::Foot(Side::Lead), &joints, (0.02, 0.0));
        assert!(sole.abs() < 0.004, "the sole lies {sole} off the floor");
    }

    /// In the stance every bone's line runs inside its own part's flesh.
    #[test]
    fn the_stances_bones_run_inside_their_flesh() {
        let joints = Joints::of(&stance());
        let inside = |part: Part, from: Place, to: Place, margin: f64| {
            for step in 0..=10 {
                let t = step as f64 / 10.0;
                let at = plus(times(from, 1.0 - t), times(to, t));
                let distance = part_distance(part, &joints, at);
                assert!(
                    distance < -margin,
                    "{part:?} is {distance:.4} from its outline at {at:?}"
                );
            }
        };
        for side in Side::BOTH {
            let s = side.index();
            inside(
                Part::UpperArm(side),
                joints.shoulder[s],
                joints.elbow[s],
                0.015,
            );
            inside(
                Part::Forearm(side),
                joints.elbow[s],
                joints.knuckles[s],
                0.012,
            );
            inside(Part::Thigh(side), joints.hip[s], joints.knee[s], 0.020);
            inside(Part::Shin(side), joints.knee[s], joints.ankle[s], 0.012);
            inside(Part::Foot(side), joints.heel[s], joints.ball[s], 0.006);
        }
        inside(Part::Trunk, joints.hips, joints.sacrum, 0.02);
        inside(Part::Trunk, joints.sacrum, joints.waist, 0.012);
        inside(Part::Trunk, joints.waist, joints.neck, 0.012);
        inside(Part::Trunk, joints.neck, joints.skull_base, 0.008);
        inside(Part::Trunk, joints.skull_base, joints.skull_top, 0.012);
    }

    /// A limb reaches its end with both bones at their lengths, and bends to
    /// the side asked.
    #[test]
    fn a_limb_reaches_for_its_end() {
        let (root, end) = ((0.0, 0.5), (0.1, 0.1));
        let (knee, reached) = reach(root, end, THIGH, SHIN, 1.0);
        assert!((length(minus(knee, root)) - THIGH).abs() < 1.0e-9);
        assert!((length(minus(reached, knee)) - SHIN).abs() < 1.0e-9);
        assert!(length(minus(reached, end)) < 1.0e-9);
        // The knee lies forward of the line from hip to ankle.
        let line = minus(end, root);
        let off = minus(knee, root);
        assert!(line.0 * off.1 - line.1 * off.0 > 0.0);
        // Out of reach, the limb straightens toward its end.
        let (_, short) = reach(root, (0.0, -0.2), THIGH, SHIN, 1.0);
        assert!((length(minus(short, root)) - (THIGH + SHIN)).abs() < 1.0e-5);
    }
}
