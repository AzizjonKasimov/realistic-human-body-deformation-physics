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

use super::soft::{perpendicular, JointTurn, Softness};
use super::tools::{FistContact, FistLanding};
use super::*;

/// One joint a punch drives, and the two pieces of the arm's chain it joins
/// (`Link`). Throwing a punch, the arm's muscles are contracted, and a
/// contracted limb moves as pieces: each joint's muscles turn the piece
/// beyond it, bone and flesh together, and push back on the piece before
/// it, and its hold keeps the two together, as Box2D's revolute joint joins
/// two bodies. Pulling on the light bones alone, they ran the bones ahead
/// of the flesh round them, and stopping at full reach, the fist's flesh
/// pulled the hand's bone off the wrist. Pushing back on the whole rest of
/// the body rather than the piece before, each joint out along the arm
/// pushed the arm round the shoulder from outside it: straightening, the
/// elbow held the arm down with nearly the shoulder's whole strength, and
/// the fist never rose above the shoulder.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Swing {
    pub(super) joint: usize,
    /// The body whose arm swings.
    pub(super) body: usize,
    /// The pieces on the body's side of the joint and beyond it, by their
    /// place in [`Punch::links`].
    pub(super) inner: usize,
    pub(super) outer: usize,
    /// Whether the piece beyond the joint stands on the floor, which takes
    /// the joint's push back through the feet (`FootHold`), so that only the
    /// piece before it turns: the lower back turns the trunk against the
    /// hips and legs standing on their feet.
    pub(super) planted: bool,
    /// What the joint's own muscles carried in the guard as the punch
    /// began, which they take up again once it is over.
    pub(super) guard_impulse: f64,
    /// Whether the punch still drives the joint: the arm's joints go back
    /// to their own muscles once the arm is back in its guard, while the
    /// lower back's stay with the punch until the body is upright again.
    pub(super) active: bool,
    /// How hard each side is to move this substep (`World::measure_punches`).
    pub(super) sides: Option<Sides>,
}

/// One piece of a punching arm's chain: the rest of the body, the
/// collarbone, the upper arm, the forearm, or the hand, each with the flesh
/// bound to it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Link {
    /// Which bones it holds, by index as the punch began.
    pub(super) bones: Vec<bool>,
    /// How much of each point's place on the skeleton lies on those bones
    /// (`PostureBinding`), by index as the punch began.
    pub(super) share: Vec<f64>,
}

/// The two sides of a joint a punch drives, where the substep began: the
/// piece before it and the piece beyond, each moved as one piece.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Sides {
    pub(super) inner: Piece,
    pub(super) outer: Piece,
}

impl Sides {
    /// How hard `swing`'s joint is to turn: both pieces against each other,
    /// or, with the piece beyond it planted on the floor, the piece before
    /// it alone.
    pub(super) fn turning_mass(&self, swing: &Swing) -> f64 {
        if swing.planted {
            self.inner.inertia
        } else {
            1.0 / (1.0 / self.inner.inertia + 1.0 / self.outer.inertia)
        }
    }
}

/// A side moved as one piece: its mass, its centre of mass, and how hard it
/// is to turn about that centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Piece {
    mass: f64,
    centre: Vec2,
    pub(super) inertia: f64,
}

/// A punch an arm is throwing.
#[derive(Clone, Debug, PartialEq)]
pub struct Punch {
    /// Seconds since it was thrown.
    pub age: f64,
    /// The lower back, the shoulder girdle, the shoulder, the elbow, the
    /// wrist, the neck, and the other arm's shoulder, which it drives.
    pub(super) swings: [Swing; 7],
    /// The pieces those joints join: the hips and legs, the trunk above
    /// them, the arm's collarbone, upper arm, forearm, and hand, the head,
    /// and the other arm beyond its shoulder.
    links: Vec<Link>,
    /// The fist's way out, a straight line from where it stood in the guard
    /// to where the punch aims, from the shoulder in the trunk as built.
    path: ArmPath,
    /// How far above level the fist's end lies from the shoulder in the
    /// world, in radians (`Materials::punch_rise`).
    rise: f64,
    /// Which way the punch goes: 1 toward the right of the window, -1
    /// toward the left.
    toward: f64,
    /// Where it turned back, if its fist met something on the way out.
    turned: Option<TurnBack>,
}

/// Where a punch whose fist met something turned back: when, and how far
/// out the arm and the body had got (`Punch::reach_at`, `Punch::lean_at`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct TurnBack {
    age: f64,
    reach: f64,
    lean: f64,
}

/// A two-link arm's fist moving on a straight line, and the shoulder's and
/// elbow's turns that put it there.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ArmPath {
    /// Where the fist starts, from the shoulder, and how far from the
    /// shoulder it ends.
    from: Vec2,
    reach: f64,
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
    /// fist `progress` of the way along the path to its end at the angle
    /// `line` from the shoulder.
    fn turns(&self, progress: f64, line: f64) -> (f64, f64) {
        let to = Vec2 {
            x: self.reach * libm::cos(line),
            y: self.reach * libm::sin(line),
        };
        let fist = lerp(self.from, to, progress);
        let (upper, elbow) = self.solve(fist);
        (
            wrap_angle(upper - self.start.0),
            wrap_angle(elbow - self.start.1),
        )
    }
}

/// How far from 0 to 1 the smoothest path, the one that keeps the change of
/// acceleration least, has gone at `t` of its way (Flash and Hogan 1985).
pub(super) fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (10.0 + t * (6.0 * t - 15.0))
}

impl Punch {
    /// When the fist reaches the punch's pose, in seconds after it was
    /// thrown: once the arm has gone out and the trunk has leaned in.
    fn landing(materials: &Materials) -> f64 {
        materials
            .punch_out_seconds
            .max(materials.punch_lean_seconds)
    }

    /// How far along its way to the punch's pose the arm aims at `age`, from
    /// 0 in the guard to 1 at the pose, for a punch timed by `materials`:
    /// out and back along the smoothest path, as people plan a quick reach.
    /// Its muscles follow the path, so they take up the arm's weight
    /// gradually rather than all at once. The arm sets off late enough to
    /// get there as the trunk finishes leaning in, as a punch's trunk moves
    /// before its arm.
    fn reach_at(age: f64, materials: &Materials) -> f64 {
        let landing = Self::landing(materials);
        let out = materials.punch_out_seconds;
        if age < landing {
            smooth((age - (landing - out)) / out.max(EPSILON))
        } else {
            1.0 - smooth((age - landing) / materials.punch_back_seconds.max(EPSILON))
        }
    }

    /// How far the body has gone into the punch at `age`, from 0 in the
    /// guard to 1 leaned `Materials::punch_lean` and shifted
    /// `Materials::punch_hip_shift`: in as the arm goes out, and back over
    /// `Materials::punch_recover_seconds`. Brought back as fast as the arm,
    /// the hips swung the trunk past upright, and it rocked for over a
    /// second.
    fn lean_at(age: f64, materials: &Materials) -> f64 {
        let landing = Self::landing(materials);
        let lean = materials.punch_lean_seconds.max(EPSILON);
        if age < landing {
            smooth((age - (landing - lean)) / lean)
        } else {
            1.0 - smooth((age - landing) / materials.punch_recover_seconds.max(EPSILON))
        }
    }

    /// How far along its way the arm aims now ([`Punch::reach_at`]); once
    /// its fist has met something, coming back from as far as it got.
    fn reach(&self, materials: &Materials) -> f64 {
        match self.turned {
            Some(turn) => {
                let back = materials.punch_back_seconds.max(EPSILON);
                turn.reach * (1.0 - smooth((self.age - turn.age) / back))
            }
            None => Self::reach_at(self.age, materials),
        }
    }

    /// How far the body has gone into the punch now ([`Punch::lean_at`]);
    /// once its fist has met something, coming back from as far as it got.
    fn lean(&self, materials: &Materials) -> f64 {
        match self.turned {
            Some(turn) => {
                let recover = materials.punch_recover_seconds.max(EPSILON);
                turn.lean * (1.0 - smooth((self.age - turn.age) / recover))
            }
            None => Self::lean_at(self.age, materials),
        }
    }

    /// The fist met something on its way out, and the punch turns back from
    /// there, as a punch that lands is pulled back: driven on to the pose it
    /// aimed at, a fist that met a guard pushed it aside and ground into the
    /// neck beyond, tearing its skin.
    fn turn_back(&mut self, materials: &Materials) {
        self.turn_back_from(materials, 1.0);
    }

    /// The punch turns back as [`Punch::turn_back`] does, the body coming
    /// back from no further in than `reached` of the way.
    fn turn_back_from(&mut self, materials: &Materials, reached: f64) {
        if self.going_out(materials) {
            self.turned = Some(TurnBack {
                age: self.age,
                reach: Self::reach_at(self.age, materials),
                lean: Self::lean_at(self.age, materials).min(reached),
            });
        }
    }

    /// When the arm sets off back: as it reaches the punch's pose, or as
    /// its fist meets something.
    fn turning(&self, materials: &Materials) -> f64 {
        self.turned
            .map_or(Self::landing(materials), |turn| turn.age)
    }

    /// The arm is still on its way out, not yet coming back.
    pub fn going_out(&self, materials: &Materials) -> bool {
        self.turned.is_none() && self.age < Self::landing(materials)
    }

    /// The punch still drives the arm, out or back, rather than its own
    /// muscles.
    pub(super) fn drives_arm(&self) -> bool {
        self.swings[1..=4].iter().any(|swing| swing.active)
    }

    /// The arm is back in its guard.
    pub(super) fn arm_home(&self, materials: &Materials) -> bool {
        self.age >= self.turning(materials) + materials.punch_back_seconds
    }

    /// The punch is over: the arm is back in its guard and the body upright.
    fn done(&self, materials: &Materials) -> bool {
        self.age
            >= self.turning(materials)
                + materials
                    .punch_back_seconds
                    .max(materials.punch_recover_seconds)
    }
}

/// How fast a fist must be going, in metres a second, for what it meets to
/// stop it and turn its punch back: a fist barely moving that touches the
/// other fighter's guard beside it, as a punch begins, has not landed.
const LANDING_SPEED: f64 = 2.0;

/// How far ahead of the end of the hand's bone the front of the fist lies,
/// in body heights: the fist's flesh round the knuckles
/// (`silhouette::FIST_TOP`).
const FIST_FRONT: f64 = 0.023;

/// Which slot of [`World::punches`] an arm's punch takes, the viewer's left
/// arm first.
fn arm_slot(limb: Limb) -> Option<usize> {
    match limb {
        Limb::LeftArm => Some(0),
        Limb::RightArm => Some(1),
        _ => None,
    }
}

/// Where a punching fist's knuckles sweep this step (`World::fist_sweep`).
struct FistSweep {
    /// The puncher's hand bone.
    hand: usize,
    from: Vec2,
    to: Vec2,
    heading: Vec2,
}

impl World {
    /// The punch body `body`'s `limb` is throwing, if any.
    pub fn punch(&self, body: usize, limb: Limb) -> Option<&Punch> {
        let arm = arm_slot(limb)?;
        self.bodies.get(body)?.punches[arm].as_ref()
    }

    /// Every punch being thrown, by body and arm, the viewer's left arm
    /// first.
    fn punch_slots(&self) -> Vec<(usize, usize)> {
        (0..self.bodies.len())
            .flat_map(|body| (0..2).map(move |arm| (body, arm)))
            .filter(|&(body, arm)| self.bodies[body].punches[arm].is_some())
            .collect()
    }

    /// Body `body`'s `limb` throws a punch out to its side, unless it hangs
    /// limp, has no shoulder or elbow left to throw with, or is still going
    /// out with the last one.
    pub fn throw_punch(&mut self, body: usize, limb: Limb) {
        let Some(arm) = arm_slot(limb) else {
            return;
        };
        if body >= self.bodies.len() {
            return;
        }
        let materials = self.materials;
        if self.limp(body, limb)
            || self.bodies[body].punches[arm]
                .as_ref()
                .is_some_and(|punch| punch.going_out(&materials))
        {
            return;
        }
        if let Some(mut punch) = self.punch_pose(body, limb) {
            // A joint another punch of the body still drives, the lower back
            // still bringing the body back from the last punch, passes to
            // this one, with what its own muscles carried in the guard
            // before either.
            for swing in &mut punch.swings {
                for earlier in self.bodies[body].punches.iter_mut().flatten() {
                    for old in &mut earlier.swings {
                        if old.active && old.joint == swing.joint {
                            old.active = false;
                            swing.guard_impulse = old.guard_impulse;
                        }
                    }
                }
            }
            // And a lean back ends, its joints passing to the punch.
            self.hand_lean_to_punch(body, &mut punch.swings);
            self.bodies[body].punches[arm] = Some(punch);
        }
    }

    /// A punch by body `body`'s `limb`: the joints it drives, from the lower
    /// back out to the wrist, the pieces they join, and the path its fist
    /// takes, a straight line from where it stands in the guard to the
    /// punch's pose. There the trunk leans `Materials::punch_lean` toward the
    /// arm's side, the elbow is straight but for `Materials::punch_elbow_bend`,
    /// and the fist lies on a line rising `Materials::punch_rise` from the
    /// shoulder. People plan a quick reach as a straight line for the hand
    /// (Flash and Hogan 1985); turned joint by joint toward the pose, the
    /// fist swung up over the head and came down onto it.
    fn punch_pose(&self, body: usize, limb: Limb) -> Option<Punch> {
        let joint_between = |from: BonePart, to: BonePart, limb: Limb| {
            self.bone_joints.iter().position(|joint| {
                !joint.broken
                    && joint.drive.limb == limb
                    && self.joint_body(joint) == body
                    && self
                        .bones
                        .get(joint.a)
                        .is_some_and(|bone| bone.part == from)
                    && self.bones.get(joint.b).is_some_and(|bone| bone.part == to)
            })
        };
        let back = joint_between(BonePart::Spine, BonePart::Pelvis, Limb::None)?;
        let neck = joint_between(BonePart::Skull, BonePart::Spine, Limb::None)?;
        let other = if limb == Limb::RightArm {
            Limb::LeftArm
        } else {
            Limb::RightArm
        };
        let guard = joint_between(BonePart::Collarbone, BonePart::UpperArm, other)
            .filter(|_| !self.limp(body, other));
        let girdle = joint_between(BonePart::Spine, BonePart::Collarbone, limb)?;
        let shoulder = joint_between(BonePart::Collarbone, BonePart::UpperArm, limb)?;
        let elbow = joint_between(BonePart::UpperArm, BonePart::Forearm, limb)?;
        let wrist = joint_between(BonePart::Forearm, BonePart::Hand, limb)?;
        let hand = self.bones.iter().find(|bone| {
            bone.part == BonePart::Hand
                && bone.limb == limb
                && bone.body as usize == body
                && !bone.splinter
        })?;
        let upper = self.bones[self.bone_joints[elbow].a];
        let forearm = self.bones[self.bone_joints[elbow].b];
        let lengths = (
            distance(upper.home_a, forearm.home_a),
            distance(forearm.home_a, hand.home_b),
        );
        let rest = self.bone_joints[elbow].rest_angle;
        let mut path = ArmPath {
            from: subtract(hand.home_b, upper.home_a),
            reach: (lengths.0 * lengths.0
                + lengths.1 * lengths.1
                + 2.0 * lengths.0 * lengths.1 * libm::cos(self.materials.punch_elbow_bend))
            .sqrt(),
            upper: lengths.0,
            fore: lengths.1,
            bend: -rest.signum(),
            start: (0.0, 0.0),
        };
        path.start = path.solve(path.from);
        // The viewer's right arm reaches right, its elbow bent the way angles
        // shrink down the window; the left arm mirrors it.
        let toward = if upper.home_b.x >= upper.home_a.x {
            1.0
        } else {
            -1.0
        };
        use BonePart::{Collarbone, Forearm, Hand, Pelvis, Skull, UpperArm};
        let arm = [Collarbone, UpperArm, Forearm, Hand];
        let in_arm = |bone: &BoneSegment| bone.limb == limb && arm.contains(&bone.part);
        let below_back = |bone: &BoneSegment| {
            bone.part == Pelvis || matches!(bone.limb, Limb::LeftLeg | Limb::RightLeg)
        };
        let guard_arm = |bone: &BoneSegment| {
            guard.is_some() && bone.limb == other && arm[1..].contains(&bone.part)
        };
        let mut links = vec![
            self.link_of(body, below_back),
            self.link_of(body, |bone| {
                !below_back(bone) && !in_arm(bone) && bone.part != Skull && !guard_arm(bone)
            }),
        ];
        links.extend(
            arm.iter()
                .map(|&part| self.link_of(body, |bone| in_arm(bone) && bone.part == part)),
        );
        links.push(self.link_of(body, |bone| bone.part == Skull));
        links.push(self.link_of(body, guard_arm));
        let swing = |joint: usize, inner: usize, outer: usize| Swing {
            joint,
            body,
            inner,
            outer,
            planted: false,
            guard_impulse: self.bone_joints[joint].drive_impulse,
            active: true,
            sides: None,
        };
        Some(Punch {
            age: 0.0,
            swings: [
                // The lower back's muscles turn the whole trunk, flesh and
                // all, against the hips: on its bones alone they were as
                // soft as the spine is light, and the trunk lagged its lean
                // and rocked after it. The joint's own hold keeps its bones
                // together, as it carries the trunk's weight in the guard;
                // held round the pieces, the trunk's flesh was held up twice
                // and flew up as the punch began. The feet stand on the
                // floor, which takes the push back: pushed back on, the hips
                // and legs swung round their middle as the trunk leaned,
                // sliding both feet 40 cm toward the other fighter and
                // lifting the near one, while the trunk itself hardly leaned.
                Swing {
                    planted: true,
                    ..swing(back, 1, 0)
                },
                swing(girdle, 1, 2),
                swing(shoulder, 2, 3),
                swing(elbow, 3, 4),
                swing(wrist, 4, 5),
                // The neck's muscles hold the head to the trunk, as a puncher
                // keeps the head still: on its bones alone they let the head
                // lag 30 degrees as the hips carried the body in.
                swing(neck, 6, 1),
                // And the other arm keeps its guard by the head: on its bones
                // alone it swung up and out as the body went in.
                match guard {
                    Some(joint) => swing(joint, 1, 7),
                    None => Swing {
                        joint: usize::MAX,
                        planted: false,
                        active: false,
                        guard_impulse: 0.0,
                        ..swing(shoulder, 1, 7)
                    },
                },
            ],
            links,
            path,
            rise: self.materials.punch_rise,
            toward,
            turned: None,
        })
    }

    /// The piece of body `body` made of the bones `holds` picks, and the
    /// flesh bound to them.
    pub(super) fn link_of(&self, body: usize, holds: impl Fn(&BoneSegment) -> bool) -> Link {
        let bones: Vec<bool> = self
            .bones
            .iter()
            .map(|bone| bone.body as usize == body && holds(bone))
            .collect();
        // Each point's share of the piece, as the joints share it out
        // between the bones they hold with their flesh (`sides.rs`): shared
        // by how much each point is held to each bone, the punch's muscles
        // and the joints moved the flesh at the wrist by different shares,
        // and a fist landing on the hand pulled the wrist apart.
        // Skin come away from its muscle is no bone's to carry
        // (`World::loose_skin`).
        let loose = self.loose_skin();
        let share = self
            .point_holds
            .iter()
            .zip(loose)
            .map(|(hold, loose)| {
                let binding = hold.posture;
                (0..2)
                    .filter(|&slot| !loose && bones.get(binding.bones[slot]) == Some(&true))
                    .map(|slot| super::sides::share_of(binding.weights[slot]))
                    .sum()
            })
            .collect();
        Link { bones, share }
    }

    /// Aims each punching arm's muscles along its punch's path, out to the
    /// pose and back to the guard, and ends punches that are done or whose
    /// arm has gone limp.
    pub(super) fn drive_punches(&mut self, dt: f64) {
        for state in &mut self.bodies {
            state.hip_shift = Vec2::default();
        }
        let px_per_metre = 1000.0 / self.mm_per_px();
        for (body, arm) in self.punch_slots() {
            let limb = [Limb::LeftArm, Limb::RightArm][arm];
            let Some(mut punch) = self.bodies[body].punches[arm].take() else {
                continue;
            };
            punch.age += dt;
            let over = punch.done(&self.materials) || self.limp(body, limb);
            let (lean, shoulder, elbow) = if over {
                (0.0, 0.0, 0.0)
            } else {
                let leaning = punch.lean(&self.materials);
                // The lower back joins the spine, above, to the pelvis:
                // leaning the spine's top toward the punch turns the joint
                // the other way.
                let lean = -punch.toward * self.materials.punch_lean * leaning;
                if punch.swings[0].active {
                    self.bodies[body].hip_shift.x +=
                        punch.toward * self.materials.punch_hip_shift * px_per_metre * leaning;
                }
                // The arm is planned in the trunk, which leans about the
                // lower back and carries the shoulder and the arm round with
                // it: to end `rise` above level in the world, the arm rises
                // against the trunk by as much again as the trunk has leaned
                // in.
                let leaned = punch.toward * self.trunk_turn(&punch);
                let rise = punch.rise + leaned;
                let line = if punch.toward > 0.0 { -rise } else { PI + rise };
                let (shoulder, elbow) = punch.path.turns(punch.reach(&self.materials), line);
                // The shoulder takes off however far the arm's load has
                // lifted the shoulder girdle, which carried the whole arm up
                // and the fist over its path.
                (lean, shoulder - self.girdle_lift(&punch), elbow)
            };
            for (swing, turn) in punch
                .swings
                .iter()
                .zip([lean, 0.0, shoulder, elbow, 0.0, 0.0, 0.0])
            {
                if !swing.active {
                    continue;
                }
                if let Some(joint) = self.bone_joints.get_mut(swing.joint) {
                    joint.drive.swing = turn;
                }
            }
            // Each joint's own muscles and hold take it back once the punch
            // is done with it, from what they carried in the guard: what the
            // punch's carried turned and held whole pieces, and given to the
            // bones alone it flicked the arm's flesh. Held by the punch until
            // the body was upright again, the arm was held off its guard and
            // swung back into it.
            let arm_home = punch.arm_home(&self.materials);
            for (index, swing) in punch.swings.iter_mut().enumerate() {
                let release = over || ((1..=4).contains(&index) && arm_home);
                if release {
                    self.release_swing(swing);
                }
            }
            self.bodies[body].punches[arm] = (!over).then_some(punch);
        }
    }

    /// The joint `swing` drives goes back to its own muscles and hold, from
    /// what they carried in the guard, unless already back.
    fn release_swing(&mut self, swing: &mut Swing) {
        if !swing.active {
            return;
        }
        swing.active = false;
        if let Some(joint) = self.bone_joints.get_mut(swing.joint) {
            joint.drive.swing = 0.0;
            joint.drive_impulse = swing.guard_impulse;
        }
    }

    /// Each fist on its way through a punch lands on any other body in its
    /// way as the bare fist does (`World::land_fist`), and what it gives
    /// that body the punching arm loses. The force it lands with presses the
    /// puncher's own hand too. Every fist's sweep is worked out before any
    /// lands, and every one lands before any arm takes back what it gave, so
    /// that no fist lands first: landed one after the other, the first
    /// fist's arm, pushed back, moved what the second landed on, and two
    /// fighters punching together parted from mirror images.
    pub(super) fn land_punches(&mut self, dt: f64) {
        self.fist_contacts.clear();
        if self.bodies.len() < 2 {
            return;
        }
        let sweeps: Vec<(usize, usize, FistSweep)> = self
            .punch_slots()
            .into_iter()
            .filter_map(|(body, arm)| {
                let punch = self.bodies[body].punches[arm].as_ref()?;
                if !punch.swings[2].active {
                    return None;
                }
                let limb = [Limb::LeftArm, Limb::RightArm][arm];
                Some((body, arm, self.fist_sweep(body, limb, punch)?))
            })
            .collect();
        let landings: Vec<FistLanding> = sweeps
            .iter()
            .map(|(body, _, sweep)| {
                self.land_fist(*body as u8, sweep.from, sweep.to, sweep.heading, dt)
            })
            .collect();
        for ((body, arm, sweep), landing) in sweeps.into_iter().zip(landings) {
            let Some(mut punch) = self.bodies[body].punches[arm].take() else {
                continue;
            };
            let given = hypot(landing.given.x, landing.given.y) > EPSILON;
            let speed = distance(sweep.from, sweep.to) / dt.max(EPSILON) * self.mm_per_px() * 0.001;
            let landed = given && speed > LANDING_SPEED;
            if landed && punch.going_out(&self.materials) {
                self.note(
                    EventKind::PunchTurnedBack,
                    Subject::Bone(sweep.hand),
                    sweep.to,
                    Some(Actor::Tissue),
                    Measure::Speed,
                    speed,
                    LANDING_SPEED,
                );
            }
            if given {
                self.push_arm_back(&punch, scale(landing.given, -1.0), sweep.to, dt);
            }
            if landing.blow > 0.0 {
                self.load_puncher_hand(sweep.hand, landing.blow, landing.blow_normal);
            }
            if landed {
                punch.turn_back(&self.materials);
            }
            if let Some(contact) = landing.contact {
                self.fist_contacts.push(FistContact {
                    contact,
                    shift: Vec2::default(),
                    body,
                    arm,
                });
            }
            self.bodies[body].punches[arm] = Some(punch);
        }
    }

    /// Body `body`'s punch with `limb`, its arm pressed hard by another
    /// body's limb (`clash.rs`), gives way: the arm goes back to its own
    /// muscles, which bring it home as they do once a punch is done with it,
    /// and a punch still on its way out turns back as a landed fist does,
    /// the body coming back from as far in as its hips got. Driven on at
    /// full strength, two fighters' blocked arms pressed each other's guards
    /// into their own bodies; and aimed on from where they aimed, the hips'
    /// hold drove the two bodies into each other with all its stiffness
    /// through their arms.
    ///
    /// The bone `bone` of the punching arm was met by the bone `by` of the
    /// other body, as `measure`'s `value` crossed its `limit`, which the
    /// journal keeps.
    pub(super) fn block_punch(
        &mut self,
        bone: usize,
        by: usize,
        measure: Measure,
        value: f64,
        limit: f64,
    ) {
        let segment = self.bones[bone];
        let (body, limb) = (segment.body as usize, segment.limb);
        let (Some(arm), Some(toward)) = (
            arm_slot(limb),
            self.punch(body, limb)
                .filter(|punch| punch.drives_arm())
                .map(|punch| punch.toward),
        ) else {
            return;
        };
        let reached = self.hips_gone_in(body, toward);
        let materials = self.materials;
        let Some(mut punch) = self.bodies[body].punches[arm].take() else {
            return;
        };
        punch.turn_back_from(&materials, reached);
        for swing in &mut punch.swings[1..=4] {
            self.release_swing(swing);
        }
        self.bodies[body].punches[arm] = Some(punch);
        self.note(
            EventKind::PunchTurnedBack,
            Subject::Bone(bone),
            lerp(segment.a, segment.b, 0.5),
            Some(Actor::Bone(by)),
            measure,
            value,
            limit,
        );
    }

    /// How far body `body`'s hips have gone in toward `toward` (1 the
    /// window's right, -1 its left), as a share of a punch's hip shift.
    fn hips_gone_in(&self, body: usize, toward: f64) -> f64 {
        let full = self.materials.punch_hip_shift * 1000.0 / self.mm_per_px();
        let offsets: Vec<f64> = self
            .hip_holds
            .iter()
            .filter(|hold| self.bone_body(hold.bone) == body)
            .filter_map(|hold| {
                let bone = self.bones.get(hold.bone)?;
                let at = if hold.end_b { bone.b } else { bone.a };
                Some((at.x - hold.home.x - hold.aim.x) * toward)
            })
            .collect();
        if offsets.is_empty() || full <= EPSILON {
            return 1.0;
        }
        let gone = offsets.iter().sum::<f64>() / offsets.len() as f64;
        (gone / full).clamp(0.0, 1.0)
    }

    /// Each fist that pressed on another body through the substeps is pushed
    /// back as far as that body pushed it, as the tool is
    /// (`finish_tool_step`): its arm loses as much momentum as the fist's
    /// weight moving that far in the step.
    pub(super) fn finish_fist_contacts(&mut self, dt: f64) {
        let contacts = std::mem::take(&mut self.fist_contacts);
        for fist in &contacts {
            if hypot(fist.shift.x, fist.shift.y) <= EPSILON {
                continue;
            }
            let Some(punch) = self.bodies[fist.body].punches[fist.arm].take() else {
                continue;
            };
            let at = lerp(
                fist.contact.shape.axis_start,
                fist.contact.shape.axis_end,
                0.5,
            );
            let impulse = scale(fist.shift, fist.contact.inertia / dt.max(EPSILON));
            self.push_arm_back(&punch, impulse, at, dt);
            self.bodies[fist.body].punches[fist.arm] = Some(punch);
        }
    }

    /// Where body `body`'s fist, punching with `limb` in `punch`, sweeps
    /// this step: its knuckles' face, from where it was to where it is; none
    /// for a fist pushed backward, which lands on nothing.
    fn fist_sweep(&self, body: usize, limb: Limb, punch: &Punch) -> Option<FistSweep> {
        let hand_index = self.bones.iter().position(|bone| {
            bone.part == BonePart::Hand
                && bone.limb == limb
                && bone.body as usize == body
                && !bone.splinter
        })?;
        let hand = self.bones[hand_index];
        // The knuckles' face lies across the hand, its middle a face's
        // radius back from the front of the fist.
        let ahead = FIST_FRONT * self.body_height - tool_geometry(ToolMode::Fist).contact_radius;
        let toward = Vec2 {
            x: punch.toward,
            y: 0.0,
        };
        let heading = normalized(subtract(hand.b, hand.a), toward);
        let heading_then = normalized(subtract(hand.previous_b, hand.previous_a), heading);
        let from = add(hand.previous_b, scale(heading_then, ahead));
        let to = add(hand.b, scale(heading, ahead));
        // The knuckles face the way the fist points: a fist pushed back,
        // as another fighter's arm in its way shoves it (`clash.rs`), lands
        // on nothing. Swept backward, its knuckles shoved the flesh behind
        // them toward the puncher, and that flesh's push back threw the
        // fist forward 40 px in a step.
        (dot(subtract(to, from), heading) > 0.0).then_some(FistSweep {
            hand: hand_index,
            from,
            to,
            heading,
        })
    }

    /// The punching arm, beyond its shoulder, takes the momentum `impulse`
    /// at `at` as one piece over the step `dt`: it moves and turns about its
    /// centre of mass, and the shoulder's hold (`sides.rs`) keeps it
    /// on the body, so it turns about the shoulder as a struck arm does.
    fn push_arm_back(&mut self, punch: &Punch, impulse: Vec2, at: Vec2, dt: f64) {
        let arm = &punch.links[3..=5];
        let in_arm = |index: usize| arm.iter().any(|link| link.bones.get(index) == Some(&true));
        let share = |index: usize| -> f64 {
            arm.iter()
                .map(|link| link.share.get(index).copied().unwrap_or(0.0))
                .sum()
        };
        let mut sums = (0.0, Vec2::default(), 0.0);
        let mut add_to = |mass: f64, at: Vec2| {
            sums.0 += mass;
            sums.1 = add(sums.1, scale(at, mass));
            sums.2 += mass * dot(at, at);
        };
        for (index, bone) in self.bones.iter().enumerate() {
            if !bone.pinned && in_arm(index) {
                add_to(bone.mass * 0.5, bone.a);
                add_to(bone.mass * 0.5, bone.b);
            }
        }
        for (index, point) in self.points.iter().enumerate() {
            let share = share(index);
            if share > 0.0 && !point.pinned && !point.is_ghost() {
                add_to(point.mass * share, point.position);
            }
        }
        let (mass, moment, second) = sums;
        if mass <= EPSILON {
            return;
        }
        let centre = scale(moment, 1.0 / mass);
        let inertia = second - mass * dot(centre, centre);
        if inertia <= EPSILON {
            return;
        }
        let speed = scale(impulse, 1.0 / mass);
        let spin = cross(subtract(at, centre), impulse) / inertia;
        let moved = |at: Vec2| {
            scale(
                add(speed, scale(perpendicular(subtract(at, centre)), spin)),
                dt,
            )
        };
        for index in 0..self.bones.len() {
            let bone = self.bones[index];
            if bone.pinned || !in_arm(index) {
                continue;
            }
            self.bones[index].a = add(bone.a, moved(bone.a));
            self.bones[index].b = add(bone.b, moved(bone.b));
        }
        for index in 0..self.points.len() {
            let share = share(index);
            let point = self.points[index];
            if share <= 0.0 || point.pinned || point.is_ghost() {
                continue;
            }
            self.points[index].position = add(point.position, scale(moved(point.position), share));
        }
    }

    /// A blow of `blow`, in the units of a bone's strength, landed by the
    /// fist whose hand's bone is `hand` the way `toward` presses back on that
    /// hand as hard: it bends the hand's bone as far as it presses across
    /// it, and breaks it past its strength, as a punch on a skull can break
    /// a bare-knuckle fighter's hand.
    fn load_puncher_hand(&mut self, hand: usize, blow: f64, toward: Vec2) {
        let back = scale(toward, -1.0);
        let (share, at) = self.break_share(hand, 0.8, 1.0, back);
        let load = blow * share;
        let mut bone = self.bones[hand];
        bone.bear_break_load(load, at, back, Actor::Tool);
        self.debug.max_bone_load = self.debug.max_bone_load.max(load);
        let breaks = self.can_fracture_bone(bone) && bone.break_load > bone.fracture_impulse;
        self.bones[hand] = bone;
        if breaks {
            self.fracture_bone(hand, bone.break_t, back, load);
        }
    }

    /// Where the fist of each arm throwing a punch is, by body and arm, the
    /// viewer's left arm first: the end of its hand's bone.
    pub(super) fn punching_fists(&self) -> Vec<Option<Vec2>> {
        let mut fists = Vec::with_capacity(self.bodies.len() * 2);
        for body in 0..self.bodies.len() {
            for limb in [Limb::LeftArm, Limb::RightArm] {
                fists.push(self.punch(body, limb).and_then(|_| {
                    self.bones
                        .iter()
                        .find(|bone| {
                            bone.part == BonePart::Hand
                                && bone.limb == limb
                                && bone.body as usize == body
                                && !bone.splinter
                        })
                        .map(|hand| hand.b)
                }));
            }
        }
        fists
    }

    /// How fast the fists that stood at `before` a step `dt` long ago have
    /// moved since, the fastest of them, in pixels per second.
    pub(super) fn punch_speed(&self, before: &[Option<Vec2>], dt: f64) -> f64 {
        let now = self.punching_fists();
        before
            .iter()
            .zip(now)
            .filter_map(|(then, now)| Some(distance((*then)?, now?) / dt.max(EPSILON)))
            .fold(0.0, f64::max)
    }

    /// How far the trunk throwing `punch` has turned in the world from where
    /// it was built, in radians: its spine's turn, the way angles grow down
    /// the window.
    fn trunk_turn(&self, punch: &Punch) -> f64 {
        let Some(spine) = self
            .bone_joints
            .get(punch.swings[0].joint)
            .and_then(|joint| self.bones.get(joint.a))
        else {
            return 0.0;
        };
        let angle = |a: Vec2, b: Vec2| libm::atan2(b.y - a.y, b.x - a.x);
        wrap_angle(angle(spine.a, spine.b) - angle(spine.home_a, spine.home_b))
    }

    /// How far `punch`'s shoulder girdle has turned from where it was built,
    /// in radians: its collarbone against the spine.
    fn girdle_lift(&self, punch: &Punch) -> f64 {
        let Some(joint) = self.bone_joints.get(punch.swings[1].joint) else {
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
        self.bodies
            .iter()
            .flat_map(|state| state.punches.iter().flatten())
            .any(|punch| {
                punch
                    .swings
                    .iter()
                    .any(|swing| swing.active && swing.joint == index)
            })
    }

    /// How hard each side of each joint a punch drives is to move, from
    /// where everything was when this substep began, which stays put through
    /// its passes.
    pub(super) fn measure_punches(&mut self) {
        for (body, arm) in self.punch_slots() {
            let Some(mut punch) = self.bodies[body].punches[arm].take() else {
                continue;
            };
            for swing in punch.swings.iter_mut().filter(|swing| swing.active) {
                swing.sides = self.sides(swing, &punch.links);
            }
            self.bodies[body].punches[arm] = Some(punch);
        }
    }

    /// The muscles across each joint a punch drives: a soft spring toward
    /// the punch's path, as `solve_joint_drives` holds a joint, pulling with
    /// all their strength until the arm is nearly there, turning the piece
    /// beyond the joint and pushing back on the piece before it.
    pub(super) fn solve_punch_drives(&mut self, h: f64) {
        for (body, arm) in self.punch_slots() {
            let Some(punch) = self.bodies[body].punches[arm].take() else {
                continue;
            };
            for swing in punch.swings.iter().filter(|swing| swing.active) {
                self.solve_swing(swing, &punch.links, h);
            }
            self.bodies[body].punches[arm] = Some(punch);
        }
    }

    /// How hard `swing`'s joint is to turn: its two bones with their flesh,
    /// where the joint holds them so (`sides.rs`), else the pieces `sides`
    /// the punch or lean joins there.
    pub(super) fn swing_mass(&self, swing: &Swing, sides: &Sides) -> f64 {
        match self.joint_sides.get(swing.joint).and_then(Option::as_ref) {
            Some(pieces) => pieces.turning_mass(swing.planted),
            None => sides.turning_mass(swing),
        }
    }

    /// Turns `swing`'s joint by the angular impulse `impulse` over a
    /// substep `h` long: its two bones with their flesh, where the joint
    /// holds them so (`sides.rs`), as the joint's own muscles turn them;
    /// else the pieces the punch or lean joins there (`turn_pieces`). Turning
    /// the whole limb beyond against the whole body before while the joints
    /// held each bone with its flesh, the punch's muscles shut the wrist as
    /// the joints' hold pushed it open, 500 px each way, until it tore.
    pub(super) fn turn_swing(
        &mut self,
        swing: &Swing,
        links: &[Link],
        sides: &Sides,
        impulse: f64,
        h: f64,
    ) {
        match self.joint_sides.get(swing.joint).cloned().flatten() {
            Some(pieces) => self.turn_joint_sides_from(&pieces, impulse, swing.planted, h),
            None => self.turn_pieces(swing, links, sides, impulse, h),
        }
    }

    /// Gives each joint a punch drives the impulse it carried last substep.
    pub(super) fn warm_start_punch_drives(&mut self, h: f64) {
        for (body, arm) in self.punch_slots() {
            let Some(punch) = self.bodies[body].punches[arm].take() else {
                continue;
            };
            for swing in punch.swings.iter().filter(|swing| swing.active) {
                let joint = self.bone_joints[swing.joint];
                let limit = self.drive_limit(&joint) * h;
                let given = joint.drive_impulse.clamp(-limit, limit);
                self.bone_joints[swing.joint].drive_impulse = given;
                if let (true, Some(sides)) = (given != 0.0, swing.sides) {
                    self.turn_swing(swing, &punch.links, &sides, given, h);
                }
            }
            self.bodies[body].punches[arm] = Some(punch);
        }
    }

    pub(super) fn solve_swing(&mut self, swing: &Swing, links: &[Link], h: f64) {
        let joint = self.bone_joints[swing.joint];
        let limit = self.drive_limit(&joint) * h;
        if limit <= 0.0 {
            self.bone_joints[swing.joint].drive_impulse = 0.0;
            return;
        }
        let Some(sides) = swing.sides else {
            return;
        };
        let mass = self.swing_mass(swing, &sides);
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
        self.turn_swing(swing, links, &sides, total - given, h);
    }

    /// The two sides of `swing`'s joint as pieces, where the substep began:
    /// each bone as two halves at its ends, and each point of flesh by its
    /// share in each.
    pub(super) fn sides(&self, swing: &Swing, links: &[Link]) -> Option<Sides> {
        let (inner, outer) = (links.get(swing.inner)?, links.get(swing.outer)?);
        // Each side's mass, its first moment, and its second moment about
        // the window's corner, from which its centre and its turning inertia
        // about the centre follow.
        let mut sums = [(0.0, Vec2::default(), 0.0); 2];
        let mut add_to = |side: usize, mass: f64, at: Vec2| {
            sums[side].0 += mass;
            sums[side].1 = add(sums[side].1, scale(at, mass));
            sums[side].2 += mass * dot(at, at);
        };
        for (index, bone) in self.bones.iter().enumerate() {
            if bone.pinned {
                continue;
            }
            for (side, link) in [inner, outer].into_iter().enumerate() {
                if link.bones.get(index) == Some(&true) {
                    add_to(side, bone.mass * 0.5, bone.previous_a);
                    add_to(side, bone.mass * 0.5, bone.previous_b);
                }
            }
        }
        for (index, point) in self.points.iter().enumerate() {
            if point.pinned || point.is_ghost() || point.body as usize != swing.body {
                continue;
            }
            for (side, link) in [inner, outer].into_iter().enumerate() {
                let share = link.share.get(index).copied().unwrap_or(0.0);
                if share > 0.0 {
                    add_to(side, point.mass * share, point.previous);
                }
            }
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
            inner: piece(sums[0])?,
            outer: piece(sums[1])?,
        })
    }

    /// Moves each piece of `swing`'s sides as one, from where the substep
    /// began: `motion` gives a piece's shift and its turn about its centre,
    /// for the piece before the joint and then the piece beyond.
    pub(super) fn move_pieces(
        &mut self,
        swing: &Swing,
        links: &[Link],
        sides: &Sides,
        motion: [(Vec2, f64); 2],
    ) {
        let (Some(inner), Some(outer)) = (links.get(swing.inner), links.get(swing.outer)) else {
            return;
        };
        let pieces = [
            (inner, sides.inner, motion[0]),
            (outer, sides.outer, motion[1]),
        ];
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
            for (link, piece, motion) in &pieces {
                if link.bones.get(index) == Some(&true) {
                    bone.a = add(bone.a, moved(piece, *motion, bone.previous_a));
                    bone.b = add(bone.b, moved(piece, *motion, bone.previous_b));
                }
            }
        }
        for (index, point) in self.points.iter_mut().enumerate() {
            if point.pinned || point.is_ghost() || point.body as usize != swing.body {
                continue;
            }
            let at = point.previous;
            for (link, piece, motion) in &pieces {
                let share = link.share.get(index).copied().unwrap_or(0.0);
                if share > 0.0 {
                    point.position = add(point.position, scale(moved(piece, *motion, at), share));
                }
            }
        }
    }

    /// Turns the piece beyond `swing`'s joint by the angular impulse
    /// `impulse` over a substep `h` long, and the piece before it the other
    /// way, each about its own centre of mass by its own inertia, as a joint
    /// motor turns two bodies (Box2D's revolute joint); the joint's hold on
    /// its bones keeps them together. A piece planted on the floor stays,
    /// the floor taking the push.
    pub(super) fn turn_pieces(
        &mut self,
        swing: &Swing,
        links: &[Link],
        sides: &Sides,
        impulse: f64,
        h: f64,
    ) {
        let outer = if swing.planted {
            0.0
        } else {
            impulse * h / sides.outer.inertia
        };
        let motion = [
            (Vec2::default(), -impulse * h / sides.inner.inertia),
            (Vec2::default(), outer),
        ];
        self.move_pieces(swing, links, sides, motion);
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

    /// Throws a punch with `limb` on a fresh body and plays two seconds
    /// after, long enough for the body to come back upright: the farthest
    /// the fist got from where it stood, its fastest, and how far from there
    /// it ended.
    fn throw(limb: Limb) -> (World, f64, f64, f64) {
        let mut world = create_layered_body(1280.0, 720.0, Materials::default());
        let dt = world.materials.fixed_dt;
        let start = fist(&world, limb);
        let (mut farthest, mut fastest, mut last) = (0.0_f64, 0.0_f64, start);
        for step in 0..120 {
            let input = InputState {
                punches: [(step == 0).then_some((0, limb)), None, None, None],
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
        world.let_limb_go(0, Limb::RightArm);
        world.throw_punch(0, Limb::RightArm);
        assert!(world.punch(0, Limb::RightArm).is_none());
        world.throw_punch(0, Limb::LeftArm);
        assert!(world.punch(0, Limb::LeftArm).is_some());
    }
}
