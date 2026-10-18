//! Punches, thrown by the arm's own muscles. The muscles across the shoulder
//! and the elbow (`stance.rs`) are aimed at a pose with the arm out to its
//! side, nearly straight, the fist at the height of a head beside it, and
//! pull toward it no harder than a person's measured strength; once the fist
//! has had time to get there they are aimed back at the guard. How fast the
//! fist travels comes from that strength and the arm's weight, as a real
//! punch's does, not from a script.
//!
//! The figure faces the viewer, and a second fighter will stand beside it,
//! so a punch goes out to the side: a flat front view cannot carry a fist
//! across in front of the body. Each arm punches to its own side.

use super::soft::{perpendicular, JointPin, JointTurn, Softness};
use super::*;

/// One joint a punch drives, and what its muscles move. Throwing a punch,
/// the arm's muscles are contracted, and a contracted limb moves as one
/// piece: its muscles turn the whole limb beyond the joint, bone and flesh
/// together, and push back on the rest of the body. Pulling on the light
/// bones alone, they ran the bones ahead of the flesh round them, and
/// stopping at full reach, the fist's flesh pulled the hand's bone off the
/// wrist. Pushing back on the upper arm alone, the elbow's muscles lifted it
/// faster than the shoulder's could hold it, and the fist swung up over the
/// head: a practised arm braces against the turn of its own joints, so the
/// push passes on to the trunk.
#[derive(Clone, Debug, PartialEq)]
struct Swing {
    joint: usize,
    /// Which bones lie beyond the joint, by index as the punch began.
    beyond: Vec<bool>,
    /// How much of each point's place on the skeleton lies on those bones
    /// (`PostureBinding`), by index as the punch began; the rest of each
    /// point belongs to the rest of the body.
    share: Vec<f64>,
    /// How hard each side is to move this substep (`World::measure_punches`).
    sides: Option<Sides>,
}

/// The two sides of a joint a punch drives, where the substep began: the
/// rest of the body and the limb beyond, each moved as one piece.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Sides {
    rest: Piece,
    beyond: Piece,
}

/// A side moved as one piece: its mass, its centre of mass, and how hard it
/// is to turn about that centre.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Piece {
    mass: f64,
    centre: Vec2,
    inertia: f64,
}

/// A punch an arm is throwing.
#[derive(Clone, Debug, PartialEq)]
pub struct Punch {
    /// Seconds since it was thrown.
    pub age: f64,
    /// The shoulder girdle, the shoulder, the elbow, and the wrist, which it
    /// drives.
    swings: [Swing; 4],
    /// The fist's way out, a straight line from where it stood in the guard
    /// to where the punch aims, from the shoulder in the body as built.
    path: ArmPath,
}

/// A two-link arm's fist moving on a straight line, and the shoulder's and
/// elbow's turns that put it there.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ArmPath {
    /// Where the fist starts and ends, from the shoulder.
    from: Vec2,
    to: Vec2,
    /// From the shoulder to the elbow, and from the elbow to the fist.
    upper: f64,
    fore: f64,
    /// Which way the elbow bends: 1 as the viewer's right arm's does, the
    /// forearm turned the way angles shrink down the window, -1 as the left.
    bend: f64,
    /// The upper arm's angle and the elbow's in the guard, as this path's
    /// own solution there, which the turns count from.
    start: (f64, f64),
}

impl ArmPath {
    /// The upper arm's angle and the elbow's (the forearm's angle less the
    /// upper arm's) that put the fist at `fist`, from the shoulder, with the
    /// elbow bent `bend`'s way.
    fn solve(&self, fist: Vec2) -> (f64, f64) {
        let longest = (self.upper + self.fore) * 0.9999;
        let shortest = (self.upper - self.fore).abs() * 1.0001;
        let reach = hypot(fist.x, fist.y).clamp(shortest, longest);
        let cosine = |near: f64, far: f64, across: f64| {
            ((near * near + far * far - across * across) / (2.0 * near * far)).clamp(-1.0, 1.0)
        };
        // The angle at the shoulder between the upper arm and the line to
        // the fist, and the elbow's opening, by the law of cosines.
        let at_shoulder = libm::acos(cosine(self.upper, reach, self.fore));
        let opening = libm::acos(cosine(self.upper, self.fore, reach));
        let line = libm::atan2(fist.y, fist.x);
        (line + self.bend * at_shoulder, -self.bend * (PI - opening))
    }

    /// How far the shoulder and the elbow turn from the guard to put the
    /// fist `reach` of the way along the path.
    fn turns(&self, reach: f64) -> (f64, f64) {
        let fist = lerp(self.from, self.to, reach);
        let (upper, elbow) = self.solve(fist);
        (
            wrap_angle(upper - self.start.0),
            wrap_angle(elbow - self.start.1),
        )
    }
}

impl Punch {
    /// How far along its way to the punch's pose the arm aims at `age`, from
    /// 0 in the guard to 1 at the pose, for a punch timed by `materials`:
    /// out and back along the smoothest path, the one that keeps the change
    /// of acceleration least, as people plan a quick reach (Flash and Hogan
    /// 1985). Its muscles follow the path, so they take up the arm's weight
    /// gradually rather than all at once.
    fn reach_at(age: f64, materials: &Materials) -> f64 {
        let smooth = |t: f64| {
            let t = t.clamp(0.0, 1.0);
            t * t * t * (10.0 + t * (6.0 * t - 15.0))
        };
        let out = materials.punch_out_seconds;
        if age < out {
            smooth(age / out.max(EPSILON))
        } else {
            1.0 - smooth((age - out) / materials.punch_back_seconds.max(EPSILON))
        }
    }

    /// The arm is still on its way out, not yet coming back.
    pub fn going_out(&self, materials: &Materials) -> bool {
        self.age < materials.punch_out_seconds
    }

    /// The punch is over: the arm is back in its guard.
    fn done(&self, materials: &Materials) -> bool {
        self.age >= materials.punch_out_seconds + materials.punch_back_seconds
    }
}

/// Which slot of [`World::punches`] an arm's punch takes, the viewer's left
/// arm first.
fn arm_slot(limb: Limb) -> Option<usize> {
    match limb {
        Limb::LeftArm => Some(0),
        Limb::RightArm => Some(1),
        _ => None,
    }
}

impl World {
    /// The punch `limb`'s arm is throwing, if any.
    pub fn punch(&self, limb: Limb) -> Option<&Punch> {
        arm_slot(limb).and_then(|arm| self.punches[arm].as_ref())
    }

    /// `limb`'s arm throws a punch out to its side, unless it hangs limp,
    /// has no shoulder or elbow left to throw with, or is still going out
    /// with the last one.
    pub fn throw_punch(&mut self, limb: Limb) {
        let Some(arm) = arm_slot(limb) else {
            return;
        };
        let materials = self.materials;
        if self.limp(limb)
            || self.punches[arm]
                .as_ref()
                .is_some_and(|punch| punch.going_out(&materials))
        {
            return;
        }
        if let Some((swings, path)) = self.punch_pose(limb) {
            self.punches[arm] = Some(Punch {
                age: 0.0,
                swings,
                path,
            });
        }
    }

    /// The shoulder, elbow, and wrist joints of `limb`'s arm, and the path
    /// its fist takes: a straight line from where it stands in the guard to
    /// the punch's pose, where the elbow is straight but for
    /// `Materials::punch_elbow_bend` and the fist lies on a line rising
    /// `Materials::punch_rise` from the shoulder, out to the arm's own side. People plan a quick reach as a straight line for the
    /// hand (Flash and Hogan 1985); turned joint by joint toward the pose,
    /// the fist swung up over the head and came down onto it.
    fn punch_pose(&self, limb: Limb) -> Option<([Swing; 4], ArmPath)> {
        let joint_between = |from: BonePart, to: BonePart| {
            self.bone_joints.iter().position(|joint| {
                !joint.broken
                    && joint.drive.limb == limb
                    && self
                        .bones
                        .get(joint.a)
                        .is_some_and(|bone| bone.part == from)
                    && self.bones.get(joint.b).is_some_and(|bone| bone.part == to)
            })
        };
        let girdle = joint_between(BonePart::Spine, BonePart::Collarbone)?;
        let shoulder = joint_between(BonePart::Collarbone, BonePart::UpperArm)?;
        let elbow = joint_between(BonePart::UpperArm, BonePart::Forearm)?;
        let wrist = joint_between(BonePart::Forearm, BonePart::Hand)?;
        let hand = self
            .bones
            .iter()
            .find(|bone| bone.part == BonePart::Hand && bone.limb == limb && !bone.splinter)?;
        let upper = self.bones[self.bone_joints[elbow].a];
        let forearm = self.bones[self.bone_joints[elbow].b];
        let lengths = (
            distance(upper.home_a, forearm.home_a),
            distance(forearm.home_a, hand.home_b),
        );
        let rest = self.bone_joints[elbow].rest_angle;
        // The viewer's right arm reaches right, its elbow bent the way angles
        // shrink down the window; the left arm mirrors it.
        let rightward = upper.home_b.x >= upper.home_a.x;
        let mut path = ArmPath {
            from: subtract(hand.home_b, upper.home_a),
            to: Vec2::default(),
            upper: lengths.0,
            fore: lengths.1,
            bend: -rest.signum(),
            start: (0.0, 0.0),
        };
        path.start = path.solve(path.from);
        let reach = (lengths.0 * lengths.0
            + lengths.1 * lengths.1
            + 2.0 * lengths.0 * lengths.1 * libm::cos(self.materials.punch_elbow_bend))
        .sqrt();
        let rise = self.materials.punch_rise;
        let line = if rightward { -rise } else { PI + rise };
        path.to = Vec2 {
            x: reach * libm::cos(line),
            y: reach * libm::sin(line),
        };
        use BonePart::{Collarbone, Forearm, Hand, UpperArm};
        Some((
            [
                self.swing_of(girdle, limb, &[Collarbone, UpperArm, Forearm, Hand]),
                self.swing_of(shoulder, limb, &[UpperArm, Forearm, Hand]),
                self.swing_of(elbow, limb, &[Forearm, Hand]),
                self.swing_of(wrist, limb, &[Hand]),
            ],
            path,
        ))
    }

    /// Joint `joint` of `limb`'s arm, moving the bones of the `beyond` parts
    /// and their flesh.
    fn swing_of(&self, joint: usize, limb: Limb, beyond: &[BonePart]) -> Swing {
        let beyond: Vec<bool> = self
            .bones
            .iter()
            .map(|bone| bone.limb == limb && beyond.contains(&bone.part))
            .collect();
        let share = self
            .point_holds
            .iter()
            .map(|hold| {
                let binding = hold.posture;
                (0..2)
                    .filter(|&slot| beyond.get(binding.bones[slot]) == Some(&true))
                    .map(|slot| binding.weights[slot])
                    .sum()
            })
            .collect();
        Swing {
            joint,
            beyond,
            share,
            sides: None,
        }
    }

    /// Aims each punching arm's muscles along its punch's path, out to the
    /// pose and back to the guard, and ends punches that are done or whose
    /// arm has gone limp.
    pub(super) fn drive_punches(&mut self, dt: f64) {
        for (arm, limb) in [Limb::LeftArm, Limb::RightArm].into_iter().enumerate() {
            let Some(mut punch) = self.punches[arm].take() else {
                continue;
            };
            punch.age += dt;
            let over = punch.done(&self.materials) || self.limp(limb);
            let (shoulder, elbow) = if over {
                (0.0, 0.0)
            } else {
                let (shoulder, elbow) = punch
                    .path
                    .turns(Punch::reach_at(punch.age, &self.materials));
                // The arm aims its fist in space, as people plan a reach for
                // the hand: the shoulder takes off however far the arm's
                // load has lifted the shoulder girdle, which carried the
                // whole arm up and the fist over its path.
                (shoulder - self.girdle_lift(&punch), elbow)
            };
            for (swing, turn) in punch.swings.iter().zip([0.0, shoulder, elbow, 0.0]) {
                if let Some(joint) = self.bone_joints.get_mut(swing.joint) {
                    joint.drive.swing = turn;
                }
            }
            self.punches[arm] = (!over).then_some(punch);
        }
    }

    /// Where the fist of each arm throwing a punch is, the viewer's left arm
    /// first: the end of its hand's bone.
    pub(super) fn punching_fists(&self) -> [Option<Vec2>; 2] {
        [Limb::LeftArm, Limb::RightArm].map(|limb| {
            self.punch(limb)?;
            self.bones
                .iter()
                .find(|bone| bone.part == BonePart::Hand && bone.limb == limb && !bone.splinter)
                .map(|hand| hand.b)
        })
    }

    /// How fast the fists that stood at `before` a step `dt` long ago have
    /// moved since, the faster of them, in pixels per second.
    pub(super) fn punch_speed(&self, before: [Option<Vec2>; 2], dt: f64) -> f64 {
        let now = self.punching_fists();
        before
            .iter()
            .zip(now)
            .filter_map(|(then, now)| Some(distance((*then)?, now?) / dt.max(EPSILON)))
            .fold(0.0, f64::max)
    }

    /// How far `punch`'s shoulder girdle has turned from where it was built,
    /// in radians: its collarbone against the spine.
    fn girdle_lift(&self, punch: &Punch) -> f64 {
        let Some(joint) = self.bone_joints.get(punch.swings[0].joint) else {
            return 0.0;
        };
        let (Some(spine), Some(collarbone)) = (self.bones.get(joint.a), self.bones.get(joint.b))
        else {
            return 0.0;
        };
        let angle = |a: Vec2, b: Vec2| libm::atan2(b.y - a.y, b.x - a.x);
        wrap_angle(
            angle(collarbone.a, collarbone.b)
                - angle(spine.a, spine.b)
                - (angle(collarbone.home_a, collarbone.home_b) - angle(spine.home_a, spine.home_b)),
        )
    }

    /// Whether joint `index` is driven by a punch this step, so its muscles
    /// move the limb as one piece (`solve_punch_drives`) rather than only its
    /// bones.
    pub(super) fn punch_drives(&self, index: usize) -> bool {
        self.punches
            .iter()
            .flatten()
            .any(|punch| punch.swings.iter().any(|swing| swing.joint == index))
    }

    /// How hard each side of each joint a punch drives is to move, from
    /// where everything was when this substep began, which stays put through
    /// its passes.
    pub(super) fn measure_punches(&mut self) {
        for arm in 0..self.punches.len() {
            let Some(mut punch) = self.punches[arm].take() else {
                continue;
            };
            for swing in &mut punch.swings {
                swing.sides = self.sides(swing);
            }
            self.punches[arm] = Some(punch);
        }
    }

    /// The muscles across each joint a punch drives: a soft spring toward
    /// the punch's path, as `solve_joint_drives` holds a joint, pulling with
    /// all their strength until the arm is nearly there, turning the whole
    /// limb beyond the joint and pushing back on the rest of the body.
    pub(super) fn solve_punch_drives(&mut self, h: f64) {
        for arm in 0..self.punches.len() {
            let Some(punch) = self.punches[arm].take() else {
                continue;
            };
            for swing in &punch.swings {
                self.solve_swing(swing, h);
            }
            self.punches[arm] = Some(punch);
        }
    }

    /// Gives each joint a punch drives the impulse it carried last substep.
    pub(super) fn warm_start_punch_drives(&mut self, h: f64) {
        for arm in 0..self.punches.len() {
            let Some(punch) = self.punches[arm].take() else {
                continue;
            };
            for swing in &punch.swings {
                let joint = self.bone_joints[swing.joint];
                let limit = self.drive_limit(&joint) * h;
                let given = joint.drive_impulse.clamp(-limit, limit);
                self.bone_joints[swing.joint].drive_impulse = given;
                if let (true, Some(sides)) = (given != 0.0, swing.sides) {
                    self.turn_limb(swing, &sides, given, h);
                }
            }
            self.punches[arm] = Some(punch);
        }
    }

    fn solve_swing(&mut self, swing: &Swing, h: f64) {
        let joint = self.bone_joints[swing.joint];
        let limit = self.drive_limit(&joint) * h;
        if limit <= 0.0 {
            self.bone_joints[swing.joint].drive_impulse = 0.0;
            return;
        }
        let Some(sides) = swing.sides else {
            return;
        };
        let mass = 1.0 / (1.0 / sides.rest.inertia + 1.0 / sides.beyond.inertia);
        let newton_metres = self.newton_metres();
        // The muscles of a punch are fully active, not braced as the
        // guard's: they pull with all their strength until the arm is within
        // `punch_full_pull` of the path.
        let full = joint.drive.max_torque / self.materials.punch_full_pull.max(EPSILON);
        let softness = Softness::of_stiffness(
            joint.drive.stiffness.max(full) * newton_metres,
            joint.drive.damping * newton_metres,
            mass,
            h,
        );
        let turn = JointTurn::of(&joint, &self.bones);
        let angle = wrap_angle(turn.angle - joint.rest_angle - joint.drive.aim - joint.drive.swing);
        let speed = turn.speed(&self.bones, h);
        let given = joint.drive_impulse.clamp(-limit, limit);
        let total = (given + softness.impulse(mass, speed, angle, given)).clamp(-limit, limit);
        self.bone_joints[swing.joint].drive_impulse = total;
        self.turn_limb(swing, &sides, total - given, h);
    }

    /// The two sides of `swing`'s joint as pieces, where the substep began:
    /// each bone as two halves at its ends, and each point of flesh split
    /// between the sides by its share.
    fn sides(&self, swing: &Swing) -> Option<Sides> {
        // Each side's mass, its first moment, and its second moment about
        // the window's corner, from which its centre and its turning inertia
        // about the centre follow.
        let mut sums = [(0.0, Vec2::default(), 0.0); 2];
        let mut add_to = |beyond: f64, weight: f64, at: Vec2| {
            for (side, share) in [(0, 1.0 - beyond), (1, beyond)] {
                let mass = weight * share;
                sums[side].0 += mass;
                sums[side].1 = add(sums[side].1, scale(at, mass));
                sums[side].2 += mass * dot(at, at);
            }
        };
        for (index, bone) in self.bones.iter().enumerate() {
            if bone.pinned {
                continue;
            }
            let beyond = if swing.beyond.get(index) == Some(&true) {
                1.0
            } else {
                0.0
            };
            add_to(beyond, bone.mass * 0.5, bone.previous_a);
            add_to(beyond, bone.mass * 0.5, bone.previous_b);
        }
        for (point, &share) in self.points.iter().zip(&swing.share) {
            if point.pinned || point.is_ghost() {
                continue;
            }
            add_to(share, point.mass, point.previous);
        }
        let piece = |(mass, moment, second): (f64, Vec2, f64)| {
            if mass <= EPSILON {
                return None;
            }
            let centre = scale(moment, 1.0 / mass);
            let inertia = second - mass * dot(centre, centre);
            (inertia > EPSILON).then_some(Piece {
                mass,
                centre,
                inertia,
            })
        };
        Some(Sides {
            rest: piece(sums[0])?,
            beyond: piece(sums[1])?,
        })
    }

    /// Turns the limb beyond `swing`'s joint by the angular impulse
    /// `impulse` over a substep `h` long, and the rest of the body the other
    /// way, each about its own centre of mass by its own inertia, as a joint
    /// motor turns two bodies (Box2D's revolute joint); the joint's hold on
    /// its bones keeps them together.
    fn turn_limb(&mut self, swing: &Swing, sides: &Sides, impulse: f64, h: f64) {
        let (rest, beyond) = (sides.rest, sides.beyond);
        let turn = |piece: &Piece, at: Vec2, angle: f64| {
            scale(perpendicular(subtract(at, piece.centre)), angle)
        };
        let (rest_angle, beyond_angle) =
            (-impulse * h / rest.inertia, impulse * h / beyond.inertia);
        for (index, bone) in self.bones.iter_mut().enumerate() {
            if bone.pinned {
                continue;
            }
            let (piece, angle) = if swing.beyond.get(index) == Some(&true) {
                (&beyond, beyond_angle)
            } else {
                (&rest, rest_angle)
            };
            bone.a = add(bone.a, turn(piece, bone.previous_a, angle));
            bone.b = add(bone.b, turn(piece, bone.previous_b, angle));
        }
        for (point, &share) in self.points.iter_mut().zip(&swing.share) {
            if point.pinned || point.is_ghost() {
                continue;
            }
            let at = point.previous;
            let moved = add(
                scale(turn(&beyond, at, beyond_angle), share),
                scale(turn(&rest, at, rest_angle), 1.0 - share),
            );
            point.position = add(point.position, moved);
        }
    }

    /// Each joint a punch drives holds its second bone's anchor where it was
    /// built on its first, as `solve_soft_joints` holds a whole joint, but
    /// pulls the whole limb beyond it and the rest of the body, flesh and
    /// bone.
    /// Swung fast, a fist swings round on a circle, and holding it there took
    /// more than the light bones' own joints could give: the wrist was pulled
    /// apart as the fist's flesh flew outward.
    pub(super) fn solve_punch_pins(&mut self, h: f64) {
        for arm in 0..self.punches.len() {
            let Some(punch) = self.punches[arm].take() else {
                continue;
            };
            for swing in &punch.swings {
                self.solve_swing_pin(swing, h);
            }
            self.punches[arm] = Some(punch);
        }
    }

    /// Gives each joint a punch drives the hold it carried last substep.
    pub(super) fn warm_start_punch_pins(&mut self, h: f64) {
        for arm in 0..self.punches.len() {
            let Some(punch) = self.punches[arm].take() else {
                continue;
            };
            for swing in &punch.swings {
                let joint = self.bone_joints[swing.joint];
                let given = joint.hold_impulse;
                let (Some(sides), Some(hold)) = (swing.sides, self.joint_hold(&joint)) else {
                    continue;
                };
                if given == Vec2::default() || hold.pulls_only {
                    continue;
                }
                let pin = JointPin::of(&joint, &self.bones, hold.reach());
                let (rest_at, beyond_at) = pin.places(&self.bones);
                let levers = (
                    subtract(rest_at, sides.rest.centre),
                    subtract(beyond_at, sides.beyond.centre),
                );
                self.push_limb(swing, &sides, given, levers, h);
            }
            self.punches[arm] = Some(punch);
        }
    }

    fn solve_swing_pin(&mut self, swing: &Swing, h: f64) {
        let joint = self.bone_joints[swing.joint];
        let Some(hold) = self.joint_hold(&joint) else {
            return;
        };
        if hold.pulls_only {
            return;
        }
        let Some(sides) = swing.sides else {
            return;
        };
        let pin = JointPin::of(&joint, &self.bones, hold.reach());
        let (rest_at, beyond_at) = pin.places(&self.bones);
        let (rest, beyond) = (sides.rest, sides.beyond);
        let (r_rest, r_beyond) = (
            subtract(rest_at, rest.centre),
            subtract(beyond_at, beyond.centre),
        );
        // The point constraint of Box2D's revolute joint (Erin Catto, MIT,
        // `revolute_joint.c`): how hard the two anchors are to move apart,
        // through each piece's mass and its turn about its centre.
        let (m_rest, m_beyond) = (1.0 / rest.mass, 1.0 / beyond.mass);
        let (i_rest, i_beyond) = (1.0 / rest.inertia, 1.0 / beyond.inertia);
        let k11 =
            m_rest + m_beyond + r_rest.y * r_rest.y * i_rest + r_beyond.y * r_beyond.y * i_beyond;
        let k12 = -r_rest.y * r_rest.x * i_rest - r_beyond.y * r_beyond.x * i_beyond;
        let k22 =
            m_rest + m_beyond + r_rest.x * r_rest.x * i_rest + r_beyond.x * r_beyond.x * i_beyond;
        let determinant = k11 * k22 - k12 * k12;
        if determinant.abs() <= EPSILON * EPSILON {
            return;
        }
        let hertz = (self.materials.bone_joint_hertz * hold.loosened).min(0.25 / h);
        let softness = Softness::new(2.0 * PI * hertz, self.materials.bone_joint_damping_ratio, h);
        let speed = pin.speed(&self.bones, h);
        // The softness's impulse on a unit mass, then that through the
        // inverse of the anchors' mass matrix.
        let target = Vec2 {
            x: softness.impulse(1.0, speed.x, pin.error.x, 0.0),
            y: softness.impulse(1.0, speed.y, pin.error.y, 0.0),
        };
        let given = joint.hold_impulse;
        let impulse = Vec2 {
            x: (k22 * target.x - k12 * target.y) / determinant
                + softness.impulse(0.0, 0.0, 0.0, given.x),
            y: (k11 * target.y - k12 * target.x) / determinant
                + softness.impulse(0.0, 0.0, 0.0, given.y),
        };
        self.bone_joints[swing.joint].hold_impulse = add(given, impulse);
        self.push_limb(swing, &sides, impulse, (r_rest, r_beyond), h);
    }

    /// Pushes the limb beyond `swing`'s joint by the impulse `impulse` at its
    /// anchor, `levers.1` from its centre of mass, over a substep `h` long,
    /// and the rest of the body the other way at its anchor, `levers.0` from
    /// its centre: each piece moves and turns as one.
    fn push_limb(
        &mut self,
        swing: &Swing,
        sides: &Sides,
        impulse: Vec2,
        levers: (Vec2, Vec2),
        h: f64,
    ) {
        let motion = |piece: &Piece, lever: Vec2, push: Vec2| {
            (
                scale(push, h / piece.mass),
                cross(lever, push) * h / piece.inertia,
            )
        };
        let rest = motion(&sides.rest, levers.0, scale(impulse, -1.0));
        let beyond = motion(&sides.beyond, levers.1, impulse);
        let moved = |piece: &Piece, (shift, turn): (Vec2, f64), at: Vec2| {
            add(
                shift,
                scale(perpendicular(subtract(at, piece.centre)), turn),
            )
        };
        for (index, bone) in self.bones.iter_mut().enumerate() {
            if bone.pinned {
                continue;
            }
            let (piece, push) = if swing.beyond.get(index) == Some(&true) {
                (&sides.beyond, beyond)
            } else {
                (&sides.rest, rest)
            };
            bone.a = add(bone.a, moved(piece, push, bone.previous_a));
            bone.b = add(bone.b, moved(piece, push, bone.previous_b));
        }
        for (point, &share) in self.points.iter_mut().zip(&swing.share) {
            if point.pinned || point.is_ghost() {
                continue;
            }
            let at = point.previous;
            let shift = add(
                scale(moved(&sides.beyond, beyond, at), share),
                scale(moved(&sides.rest, rest, at), 1.0 - share),
            );
            point.position = add(point.position, shift);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where `limb`'s fist is: the end of its hand's bone.
    fn fist(world: &World, limb: Limb) -> Vec2 {
        world
            .bones
            .iter()
            .find(|bone| bone.part == BonePart::Hand && bone.limb == limb && !bone.splinter)
            .map(|hand| hand.b)
            .unwrap()
    }

    /// Throws a punch with `limb` on a fresh body and plays a second after:
    /// the farthest the fist got from where it stood, its fastest, and how
    /// far from there it ended.
    fn throw(limb: Limb) -> (World, f64, f64, f64) {
        let mut world = create_layered_body(1280.0, 720.0, Materials::default());
        let dt = world.materials.fixed_dt;
        let start = fist(&world, limb);
        let (mut farthest, mut fastest, mut last) = (0.0_f64, 0.0_f64, start);
        for step in 0..60 {
            let input = InputState {
                punch: (step == 0).then_some(limb),
                ..InputState::default()
            };
            world.step(dt, &input, 1280.0);
            let now = fist(&world, limb);
            farthest = farthest.max(distance(now, start));
            fastest = fastest.max(distance(now, last) / dt);
            last = now;
        }
        let end = distance(fist(&world, limb), start);
        (world, farthest, fastest, end)
    }

    /// A punch reaches out about a third of a metre at a hard punch's speed
    /// and comes back into the guard, and throwing it hurts nothing: pulled
    /// on its light bones alone, the arm tore its own wrist.
    #[test]
    fn a_punch_reaches_out_and_comes_back_unhurt() {
        let (world, farthest, fastest, end) = throw(Limb::RightArm);
        let px = 1000.0 / world.mm_per_px();
        assert!(
            farthest > 0.25 * px,
            "the fist got only {farthest:.0} px out"
        );
        assert!(
            fastest > 6.0 * px && fastest < 11.1 * px,
            "the fist's fastest was {:.1} m/s",
            fastest / px
        );
        assert!(end < 16.0, "the fist ended {end:.1} px from its guard");
        let stats = world.stats;
        assert_eq!(
            (
                stats.fractured_bones,
                stats.broken_skin,
                stats.broken_muscle,
                stats.broken_bone_joints,
                stats.broken_bone_attachments,
                stats.broken_attachments,
                stats.contusion_events,
            ),
            (0, 0, 0, 0, 0, 0, 0),
            "throwing a punch hurt the arm"
        );
    }

    /// The left arm punches as the right does, out to its own side.
    #[test]
    fn the_left_arm_punches_as_the_right_does() {
        let (_, right_reach, right_speed, _) = throw(Limb::RightArm);
        let (_, left_reach, left_speed, _) = throw(Limb::LeftArm);
        assert!(
            (right_reach - left_reach).abs() < 8.0,
            "reach {right_reach:.1} px right, {left_reach:.1} left"
        );
        assert!(
            (right_speed - left_speed).abs() < 0.1 * right_speed,
            "speed {right_speed:.0} px/s right, {left_speed:.0} left"
        );
    }

    /// An arm hanging limp throws no punch.
    #[test]
    fn a_limp_arm_throws_no_punch() {
        let mut world = create_layered_body(1280.0, 720.0, Materials::default());
        world.let_limb_go(Limb::RightArm);
        world.throw_punch(Limb::RightArm);
        assert!(world.punch(Limb::RightArm).is_none());
        world.throw_punch(Limb::LeftArm);
        assert!(world.punch(Limb::LeftArm).is_some());
    }
}
