//! Watchdogs: checks, on every frame of a run, that what the body does makes
//! physical sense. Each trip names the frame it first happened on and what
//! tripped it, so `strike_scenarios --why` can follow that thing back to
//! what moved it. They only watch: a run plays the same with them.

use std::collections::HashMap;

use super::blame::BlameTarget;
use super::names::{subject_name, subject_spec};
use crate::scenarios::{inside_the_skin, BLOOD_VOLUME_ML, GUARD_DROP_PX};
use crate::simulation::{BonePart, Limb, Subject, Vec2, World};

/// What a watchdog checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Watchdog {
    /// A point or a bone is not at a finite place.
    NotFinite,
    /// Tissue or bone moving faster than twice the fastest the tool has
    /// gone, and at least 1000 px/s: a perfectly elastic blow throws flesh
    /// at most twice the tool's speed, so faster is the simulation
    /// flinging it, as the scenarios' rule says.
    Flung,
    /// A piece of broken bone more than three point spacings from all
    /// flesh: it has left the body.
    BoneLeftBody,
    /// A piece of a closed break outside the skin, which such a break
    /// leaves whole.
    ClosedPieceOut,
    /// A whole bone outside the skin.
    WholeBoneOut,
    /// A whole joint whose second bone's end has moved more than a point
    /// spacing from where it was built on the first, pulled open or pushed
    /// aside, with no rule having loosened it.
    JointApart,
    /// Living flesh turned inside out: a triangle facing the other way from
    /// how it was built, by at least a quarter of its area. Less is a crease
    /// folding in the flat body's skin, as at the guard's armpit.
    InsideOut,
    /// Nothing has touched the body for a second and a half, and its tissue
    /// is still moving faster than 40 px/s (about 12 cm/s).
    Restless,
    /// Nothing has touched the body for a second and a half, and an arm
    /// with no broken bone or loosened joint holds its fist out of the
    /// guard (`GUARD_DROP_PX`).
    GuardLost,
    /// The blood shown and the blood no longer shown do not add up to what
    /// was let out, or more was let out than lost.
    BloodLedger,
}

impl Watchdog {
    /// Every watchdog, in the order reports list them.
    pub const ALL: [Watchdog; 10] = [
        Watchdog::NotFinite,
        Watchdog::Flung,
        Watchdog::BoneLeftBody,
        Watchdog::ClosedPieceOut,
        Watchdog::WholeBoneOut,
        Watchdog::JointApart,
        Watchdog::InsideOut,
        Watchdog::Restless,
        Watchdog::GuardLost,
        Watchdog::BloodLedger,
    ];

    /// A short name for files, such as `whole_bone_out`.
    pub fn slug(self) -> &'static str {
        match self {
            Watchdog::NotFinite => "not_finite",
            Watchdog::Flung => "flung",
            Watchdog::BoneLeftBody => "bone_left_body",
            Watchdog::ClosedPieceOut => "closed_piece_out",
            Watchdog::WholeBoneOut => "whole_bone_out",
            Watchdog::JointApart => "joint_apart",
            Watchdog::InsideOut => "inside_out",
            Watchdog::Restless => "restless",
            Watchdog::GuardLost => "guard_lost",
            Watchdog::BloodLedger => "blood_ledger",
        }
    }

    /// The watchdog with short name `slug`.
    pub fn from_slug(slug: &str) -> Option<Watchdog> {
        Watchdog::ALL
            .iter()
            .copied()
            .find(|watchdog| watchdog.slug() == slug)
    }

    pub fn describe(self) -> &'static str {
        match self {
            Watchdog::NotFinite => "not at a finite place",
            Watchdog::Flung => "flung faster than twice the tool",
            Watchdog::BoneLeftBody => "broken bone left the body",
            Watchdog::ClosedPieceOut => "a closed break's piece outside the skin",
            Watchdog::WholeBoneOut => "a whole bone outside the skin",
            Watchdog::JointApart => "a whole joint pulled apart",
            Watchdog::InsideOut => "flesh turned inside out",
            Watchdog::Restless => "still moving with nothing touching it",
            Watchdog::GuardLost => "an unhurt arm out of its guard at rest",
            Watchdog::BloodLedger => "blood does not add up",
        }
    }

    /// Whether a trip is a known gap the sweep only reports rather than a
    /// fault: closed breaks' pieces outside the skin
    /// (`scenarios::closed_pieces_out`).
    pub fn known_gap(self) -> bool {
        matches!(self, Watchdog::ClosedPieceOut)
    }

    /// A trip's measure in words, against its limit.
    pub fn measure(self, value: f64, limit: f64) -> String {
        match self {
            Watchdog::NotFinite => "not a number".to_string(),
            Watchdog::Flung | Watchdog::Restless => {
                format!("{value:.0} px/s (limit {limit:.0})")
            }
            Watchdog::BoneLeftBody => format!("{value:.0} px from all flesh (limit {limit:.0})"),
            Watchdog::ClosedPieceOut | Watchdog::WholeBoneOut => "outside".to_string(),
            Watchdog::JointApart => {
                format!("its bone's end {value:.0} px off its place (limit {limit:.0})")
            }
            Watchdog::InsideOut => {
                format!("{:.0}% of its area turned over", value * 100.0)
            }
            Watchdog::GuardLost => format!("fist {value:.0} px off (limit {limit:.0})"),
            Watchdog::BloodLedger => format!("{value:.6} ml off"),
        }
    }
}

/// One thing a watchdog flagged, over every frame it stayed flagged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trip {
    pub watchdog: Watchdog,
    pub subject: Subject,
    pub first_frame: i32,
    pub last_frame: i32,
    /// On how many frames it tripped.
    pub frames: i32,
    /// Where it first tripped.
    pub at: Vec2,
    /// The worst it got, against the limit, and on which frame.
    pub worst: f64,
    pub limit: f64,
    pub worst_frame: i32,
}

impl Trip {
    /// What the why follows for this trip: its point, bone, joint, fiber, or
    /// triangle; nothing for a trip of the whole body.
    pub fn why_target(&self, world: &World) -> Option<BlameTarget> {
        match self.subject {
            Subject::Point(index) => Some(BlameTarget::Point(index)),
            Subject::Bone(index) => Some(BlameTarget::Bone(index)),
            Subject::Joint(index) => Some(BlameTarget::Joint(index)),
            Subject::Spring(index) => Some(BlameTarget::Spring(index)),
            Subject::Triangle(index) => world
                .triangles()
                .get(index)
                .map(|_| BlameTarget::Triangle(index)),
            _ => None,
        }
    }
}

/// One line for each watchdog that tripped in a run: how many things, the
/// first, and the worst.
pub fn watchdog_lines(world: &World, trips: &[Trip]) -> Vec<String> {
    let mut kinds: Vec<Watchdog> = trips.iter().map(|trip| trip.watchdog).collect();
    kinds.sort();
    kinds.dedup();
    kinds
        .into_iter()
        .map(|kind| {
            let mine: Vec<&Trip> = trips.iter().filter(|trip| trip.watchdog == kind).collect();
            let first = mine
                .iter()
                .min_by_key(|trip| trip.first_frame)
                .expect("a tripped watchdog has trips");
            let worst = mine
                .iter()
                .max_by(|p, q| p.worst.total_cmp(&q.worst))
                .expect("a tripped watchdog has trips");
            format!(
                "{}{}: {} thing{} from frame {}, first {} ({}), worst {} on {} at frame {}",
                kind.describe(),
                if kind.known_gap() { " [known gap]" } else { "" },
                mine.len(),
                if mine.len() == 1 { "" } else { "s" },
                first.first_frame,
                subject_spec(first.subject),
                subject_name(world, first.subject),
                kind.measure(worst.worst, worst.limit),
                subject_spec(worst.subject),
                worst.worst_frame,
            )
        })
        .collect()
}

/// How long nothing must touch the body before it should be at rest, in
/// frames.
const QUIET_FRAMES: i32 = 90;
/// The fastest tissue may move at rest, in pixels per second.
const RESTLESS_SPEED: f64 = 40.0;
/// The slowest speed that counts as flung, in pixels per second.
const FLUNG_FLOOR: f64 = 1000.0;
/// How far a triangle must reach, in square pixels, to say which way it
/// faces.
const FACING_AREA: f64 = 1.0;
/// How much of its area a triangle must turn over, as a share of the area
/// it was built with, to count as inside out rather than creased.
const TURNED_OVER: f64 = 0.25;

/// The watchdogs over one run, fed each frame with [`Watchdogs::check`].
#[derive(Clone, Debug, Default)]
pub struct Watchdogs {
    last_points: Vec<Vec2>,
    last_bones: Vec<Vec2>,
    fastest_tool: f64,
    /// Frames since the tool last touched the body.
    quiet: i32,
    trips: Vec<Trip>,
    tripped: HashMap<(Watchdog, Subject), usize>,
}

fn midpoint(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: (a.x + b.x) * 0.5,
        y: (a.y + b.y) * 0.5,
    }
}

fn span(a: Vec2, b: Vec2) -> f64 {
    libm::hypot(a.x - b.x, a.y - b.y)
}

fn finite(at: Vec2) -> bool {
    at.x.is_finite() && at.y.is_finite()
}

fn signed_area(a: Vec2, b: Vec2, c: Vec2) -> f64 {
    ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)) * 0.5
}

impl Watchdogs {
    /// Watchdogs over a run starting from `world` as it is now.
    pub fn new(world: &World) -> Self {
        let mut watchdogs = Self::default();
        watchdogs.remember(world);
        watchdogs
    }

    /// What tripped so far, in the order each first tripped.
    pub fn trips(&self) -> &[Trip] {
        &self.trips
    }

    fn remember(&mut self, world: &World) {
        self.last_points = world.points().iter().map(|point| point.position).collect();
        self.last_bones = world
            .bones()
            .iter()
            .map(|bone| midpoint(bone.a, bone.b))
            .collect();
    }

    #[allow(clippy::too_many_arguments)]
    fn trip(
        &mut self,
        watchdog: Watchdog,
        subject: Subject,
        frame: i32,
        at: Vec2,
        value: f64,
        limit: f64,
    ) {
        match self.tripped.get(&(watchdog, subject)) {
            Some(&index) => {
                let trip = &mut self.trips[index];
                trip.last_frame = frame;
                trip.frames += 1;
                if value > trip.worst {
                    trip.worst = value;
                    trip.worst_frame = frame;
                }
            }
            None => {
                self.tripped.insert((watchdog, subject), self.trips.len());
                self.trips.push(Trip {
                    watchdog,
                    subject,
                    first_frame: frame,
                    last_frame: frame,
                    frames: 1,
                    at,
                    worst: value,
                    limit,
                    worst_frame: frame,
                });
            }
        }
    }

    /// Checks `world` as frame `frame` left it.
    pub fn check(&mut self, frame: i32, world: &World) {
        let debug = world.debug();
        self.fastest_tool = self
            .fastest_tool
            .max(debug.striker_speed)
            .max(debug.punch_speed);
        // A body throwing a punch moves on purpose: it is not left alone
        // to come to rest until the punch is over.
        let punching = (0..world.body_count()).any(|body| {
            [Limb::LeftArm, Limb::RightArm]
                .iter()
                .any(|&arm| world.punch(body, arm).is_some())
        });
        if debug.tissue_contacts + debug.bone_contacts > 0 || punching {
            self.quiet = 0;
        } else {
            self.quiet += 1;
        }
        self.check_motion(frame, world);
        self.check_bones(frame, world);
        self.check_joints(frame, world);
        self.check_flesh(frame, world);
        if self.quiet >= QUIET_FRAMES {
            self.check_rest(frame, world);
        }
        self.check_blood(frame, world);
        self.remember(world);
    }

    /// Nothing out of the numbers, and nothing thrown.
    fn check_motion(&mut self, frame: i32, world: &World) {
        let dt = world.materials().fixed_dt.max(1.0e-9);
        let limit = (2.0 * self.fastest_tool).max(FLUNG_FLOOR);
        for (index, point) in world.points().iter().enumerate() {
            if !finite(point.position) {
                self.trip(
                    Watchdog::NotFinite,
                    Subject::Point(index),
                    frame,
                    Vec2::default(),
                    f64::INFINITY,
                    0.0,
                );
                continue;
            }
            let Some(&last) = self.last_points.get(index) else {
                continue;
            };
            let speed = span(point.position, last) / dt;
            if !point.pinned && speed > limit {
                self.trip(
                    Watchdog::Flung,
                    Subject::Point(index),
                    frame,
                    point.position,
                    speed,
                    limit,
                );
            }
        }
        for (index, bone) in world.bones().iter().enumerate() {
            let middle = midpoint(bone.a, bone.b);
            if !finite(middle) {
                self.trip(
                    Watchdog::NotFinite,
                    Subject::Bone(index),
                    frame,
                    Vec2::default(),
                    f64::INFINITY,
                    0.0,
                );
                continue;
            }
            let Some(&last) = self.last_bones.get(index) else {
                continue;
            };
            let speed = span(middle, last) / dt;
            if !bone.pinned && speed > limit {
                self.trip(
                    Watchdog::Flung,
                    Subject::Bone(index),
                    frame,
                    middle,
                    speed,
                    limit,
                );
            }
        }
    }

    /// Bone stays in the body: broken pieces near flesh, closed breaks'
    /// pieces and whole bones inside the skin.
    fn check_bones(&mut self, frame: i32, world: &World) {
        let reach = world.materials().point_spacing * 3.0;
        for (index, bone) in world.bones().iter().enumerate() {
            let middle = midpoint(bone.a, bone.b);
            if !finite(middle) {
                continue;
            }
            let broken = bone.fractured || bone.splinter;
            if broken {
                let nearest = world
                    .points()
                    .iter()
                    .filter(|point| !point.is_ghost())
                    .map(|point| span(point.position, middle))
                    .fold(f64::INFINITY, f64::min);
                if nearest > reach {
                    self.trip(
                        Watchdog::BoneLeftBody,
                        Subject::Bone(index),
                        frame,
                        middle,
                        nearest,
                        reach,
                    );
                }
            }
            if (broken && !bone.open_break || !broken) && !inside_the_skin(world, middle) {
                self.trip(
                    if broken {
                        Watchdog::ClosedPieceOut
                    } else {
                        Watchdog::WholeBoneOut
                    },
                    Subject::Bone(index),
                    frame,
                    middle,
                    1.0,
                    0.0,
                );
            }
        }
    }

    /// Whole joints hold together.
    fn check_joints(&mut self, frame: i32, world: &World) {
        let spacing = world.materials().point_spacing;
        let bones = world.bones();
        for (index, joint) in world.bone_joints().iter().enumerate() {
            if joint.broken || joint.post_fracture_limited || joint.subluxated {
                continue;
            }
            let (Some(a), Some(b)) = (bones.get(joint.a), bones.get(joint.b)) else {
                continue;
            };
            let along = |bone: &crate::simulation::BoneSegment, t: f64| Vec2 {
                x: bone.a.x + (bone.b.x - bone.a.x) * t,
                y: bone.a.y + (bone.b.y - bone.a.y) * t,
            };
            let (from, to) = (along(a, joint.t_a), along(b, joint.t_b));
            // How far the second bone's end has moved from where it was built
            // on the first, any way: a joint held as a hinge can be pushed
            // off its place sideways as well as pulled open.
            let slip = world.joint_slip(index).unwrap_or(0.0);
            if slip > spacing {
                self.trip(
                    Watchdog::JointApart,
                    Subject::Joint(index),
                    frame,
                    midpoint(from, to),
                    slip,
                    spacing,
                );
            }
        }
    }

    /// Living flesh faces the way it was built.
    fn check_flesh(&mut self, frame: i32, world: &World) {
        let points = world.points();
        for (index, triangle) in world.triangles().iter().enumerate() {
            if !world.triangle_alive(triangle) {
                continue;
            }
            let corners = [triangle.a, triangle.b, triangle.c];
            let [a, b, c] = corners.map(|i| points[i].home);
            let built = signed_area(a, b, c);
            if built.abs() < FACING_AREA {
                continue;
            }
            let [a, b, c] = corners.map(|i| points[i].position);
            let turned = -signed_area(a, b, c) / built;
            if turned >= TURNED_OVER {
                self.trip(
                    Watchdog::InsideOut,
                    Subject::Triangle(index),
                    frame,
                    Vec2 {
                        x: (a.x + b.x + c.x) / 3.0,
                        y: (a.y + b.y + c.y) / 3.0,
                    },
                    turned,
                    TURNED_OVER,
                );
            }
        }
    }

    /// A body nothing has touched for a while is still, and holds its guard.
    fn check_rest(&mut self, frame: i32, world: &World) {
        let dt = world.materials().fixed_dt.max(1.0e-9);
        let fastest = world
            .points()
            .iter()
            .enumerate()
            .filter(|(_, point)| !point.pinned)
            .filter_map(|(index, point)| {
                let last = self.last_points.get(index)?;
                Some((index, span(point.position, *last) / dt))
            })
            .max_by(|p, q| p.1.total_cmp(&q.1));
        if let Some((index, speed)) = fastest {
            if speed > RESTLESS_SPEED {
                self.trip(
                    Watchdog::Restless,
                    Subject::Point(index),
                    frame,
                    world.points()[index].position,
                    speed,
                    RESTLESS_SPEED,
                );
            }
        }
        for (body, arm) in (0..world.body_count())
            .flat_map(|body| [Limb::LeftArm, Limb::RightArm].map(|arm| (body, arm)))
        {
            let of_body = |bone: usize| {
                world
                    .bones()
                    .get(bone)
                    .is_some_and(|bone| bone.body as usize == body)
            };
            let hurt = world.limp(body, arm)
                || world.bone_joints().iter().any(|joint| {
                    joint.drive.limb == arm
                        && of_body(joint.a)
                        && (joint.broken || joint.post_fracture_limited || joint.subluxated)
                });
            if hurt {
                continue;
            }
            let fist = world.bones().iter().enumerate().find(|(_, bone)| {
                bone.part == BonePart::Hand && bone.limb == arm && bone.body as usize == body
            });
            if let Some((index, bone)) = fist {
                let off = span(bone.b, bone.home_b);
                if off > GUARD_DROP_PX {
                    self.trip(
                        Watchdog::GuardLost,
                        Subject::Bone(index),
                        frame,
                        bone.b,
                        off,
                        GUARD_DROP_PX,
                    );
                }
            }
        }
    }

    /// The blood shown and not shown is the blood let out, and no more was
    /// let out than lost.
    fn check_blood(&mut self, frame: i32, world: &World) {
        let stats = world.stats();
        let shed = stats.blood_shed_ml;
        let slack = 1.0e-6 * shed.max(1.0);
        let accounted = world.blood_shown_ml() + stats.blood_unshown_ml;
        let lost = stats.blood_loss * BLOOD_VOLUME_ML;
        let off = (accounted - shed).abs().max(shed - lost);
        if off > slack {
            self.trip(
                Watchdog::BloodLedger,
                Subject::Nothing,
                frame,
                Vec2::default(),
                off,
                slack,
            );
        }
    }
}
