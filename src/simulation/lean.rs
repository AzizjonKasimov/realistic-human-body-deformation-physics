//! Leaning back, away from the other fighter, so a punch falls short: the
//! lower back's muscles lean the trunk away and the hips shift away for as
//! long as the fighter holds it, then bring the body upright again. Seen
//! from the front, pulling the head back out of a punch's way shows as a
//! lean away from it, the mirror of a punch's lean in.
//!
//! As a punch leans the body in (`punch.rs`), the trunk turns as one piece
//! against the hips and legs, flesh and all, with the head and the far arm
//! braced to it: on their bones alone the head lags and an arm swings out as
//! the trunk moves. The arm nearer the other fighter keeps its guard by its
//! own muscles, so a punch that reaches it meets an arm, not the trunk. A punch thrown takes those joints over and
//! ends the lean; a lean waits for a punching arm to come home, then takes
//! the joints over from the punch.

use super::punch::{smooth, Link, Swing};
use super::soft::{JointTurn, Softness};
use super::*;

/// How fast the lower back's muscles bring the trunk to the lean they aim
/// at, in hertz, critically damped: driven as a punch drives it, at full
/// strength on little damping, the trunk overshot its lean, rocked back, and
/// held it only after a second.
const LEAN_HERTZ: f64 = 4.0;

/// How long a lean let go goes on holding the trunk upright once it aims
/// upright again, in seconds, before the lower back's own muscles take it
/// back: handed back while the trunk still swung upright, it swung on past
/// upright toward the other fighter and rocked for two seconds.
const LEAN_SETTLE_SECONDS: f64 = 1.0;

/// A lean back that a body is holding, or coming back upright from.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Lean {
    /// How far into the lean the body aims now, from 0 upright to 1 leaned
    /// `Materials::lean_back` and shifted `Materials::lean_hip_shift`.
    amount: f64,
    /// How long it has aimed upright since it was let go, in seconds.
    upright: f64,
    /// Which way it leans: 1 toward the right of the window, -1 the left.
    away: f64,
    /// The lower back, the neck, and the far arm's shoulder, which it
    /// drives.
    swings: Vec<Swing>,
    /// The pieces those joints join: the hips and legs, the trunk, the
    /// head, and the far arm beyond its shoulder.
    links: Vec<Link>,
}

impl World {
    /// Body `body` leans back, away from the other bodies, while `held`,
    /// and comes back upright once let go. It leans once its arms are home
    /// from any punch, and a punch it throws ends the lean.
    pub fn hold_lean(&mut self, body: usize, held: bool) {
        if let Some(state) = self.bodies.get_mut(body) {
            state.lean_held = held;
        }
    }

    /// How far body `body` aims to lean back now, from 0 upright to 1
    /// leaned all the way.
    pub fn leaning(&self, body: usize) -> f64 {
        self.bodies
            .get(body)
            .and_then(|state| state.lean.as_ref())
            .map_or(0.0, |lean| smooth(lean.amount))
    }

    /// Whether joint `index` is driven by a lean this step, so its muscles
    /// move the trunk, head, or arm as one piece (`solve_lean_drives`)
    /// rather than only its bones.
    pub(super) fn lean_drives(&self, index: usize) -> bool {
        self.bodies
            .iter()
            .filter_map(|state| state.lean.as_ref())
            .any(|lean| {
                lean.swings
                    .iter()
                    .any(|swing| swing.active && swing.joint == index)
            })
    }

    /// Starts the leans held since the last step, eases each lean toward
    /// leaned while held and back upright once let go, aims the lower back
    /// and the hips by as much, and ends the leans that are upright again.
    /// After `drive_punches`, whose hip shift it adds to.
    pub(super) fn drive_leans(&mut self, dt: f64) {
        let materials = self.materials;
        let px_per_metre = 1000.0 / self.mm_per_px();
        for body in 0..self.bodies.len() {
            let held = self.bodies[body].lean_held;
            if held && self.bodies[body].lean.is_none() && self.arms_home(body) {
                self.start_lean(body);
            }
            let Some(mut lean) = self.bodies[body].lean.take() else {
                continue;
            };
            let seconds = if held {
                materials.lean_back_seconds
            } else {
                materials.lean_recover_seconds
            };
            let step = dt / seconds.max(EPSILON);
            lean.amount = if held {
                (lean.amount + step).min(1.0)
            } else {
                (lean.amount - step).max(0.0)
            };
            lean.upright = if !held && lean.amount <= 0.0 {
                lean.upright + dt
            } else {
                0.0
            };
            if lean.upright >= LEAN_SETTLE_SECONDS {
                self.release_lean(&mut lean);
                continue;
            }
            let leaning = smooth(lean.amount);
            // The lower back joins the spine, above, to the pelvis: leaning
            // the spine's top away turns the joint the other way, as a
            // punch's lean in does (`drive_punches`).
            let turn = -lean.away * materials.lean_back * leaning;
            for (index, swing) in lean.swings.iter().enumerate() {
                if !swing.active {
                    continue;
                }
                if let Some(joint) = self.bone_joints.get_mut(swing.joint) {
                    joint.drive.swing = if index == 0 { turn } else { 0.0 };
                }
            }
            if lean.swings[0].active {
                self.bodies[body].hip_shift.x +=
                    lean.away * materials.lean_hip_shift * px_per_metre * leaning;
            }
            self.bodies[body].lean = Some(lean);
        }
    }

    /// Whether body `body`'s arms are both home from any punch: a lean
    /// waits for a punching arm, whose path is planned in the trunk, and
    /// may take the trunk over once the arm is back in its guard.
    fn arms_home(&self, body: usize) -> bool {
        let materials = self.materials;
        self.bodies[body]
            .punches
            .iter()
            .flatten()
            .all(|punch| punch.arm_home(&materials))
    }

    /// Body `body` begins to lean back: the joints it drives and the pieces
    /// they join, taking over any its last punch still drives as the body
    /// comes back upright, with what their own muscles carried in the guard
    /// before it.
    fn start_lean(&mut self, body: usize) {
        let Some(mut lean) = self.lean_pose(body) else {
            return;
        };
        for swing in &mut lean.swings {
            for punch in self.bodies[body].punches.iter_mut().flatten() {
                for old in &mut punch.swings {
                    if old.active && old.joint == swing.joint {
                        old.active = false;
                        swing.guard_impulses = old.guard_impulses;
                    }
                }
            }
        }
        self.bodies[body].lean = Some(lean);
    }

    /// A lean back by body `body`: away from the other bodies, the lower
    /// back driving the trunk against the hips and legs, the neck holding
    /// the head to the trunk, and the far arm's shoulder holding that arm
    /// in its guard, unless it hangs limp. None if the body has no whole
    /// lower back or neck to lean with.
    fn lean_pose(&self, body: usize) -> Option<Lean> {
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
        let middle = self.midline_of(body)?;
        let toward_others: f64 = (0..self.bodies.len())
            .filter(|&other| other != body)
            .filter_map(|other| self.midline_of(other))
            .map(|x| x - middle)
            .sum();
        // Away from the others; a body alone leans away from its arm that
        // would punch (`inner_arm`).
        let inner = self.inner_arm(body);
        let away = if toward_others != 0.0 {
            -toward_others.signum()
        } else {
            let shoulder = self
                .bones
                .iter()
                .find(|bone| {
                    bone.part == BonePart::UpperArm
                        && bone.limb == inner
                        && bone.body as usize == body
                })
                .map_or(middle + 1.0, |bone| bone.home_a.x);
            -(shoulder - middle).signum()
        };
        use BonePart::{Forearm, Hand, Pelvis, Skull, UpperArm};
        let below_back = |bone: &BoneSegment| {
            bone.part == Pelvis || matches!(bone.limb, Limb::LeftLeg | Limb::RightLeg)
        };
        // The arm away from the others is braced to the trunk, which carries
        // it round as it leans away; the arm nearer them keeps its guard by
        // its own muscles, in neither piece. Braced as one piece with the
        // trunk, a punch that reached the guard's fist tore its wrist.
        let arms: Vec<(Limb, usize)> = [Limb::LeftArm, Limb::RightArm]
            .into_iter()
            .filter(|&limb| limb != inner && !self.limp(body, limb))
            .filter_map(|limb| {
                joint_between(BonePart::Collarbone, UpperArm, limb).map(|joint| (limb, joint))
            })
            .collect();
        let beyond_shoulder = |bone: &BoneSegment| {
            matches!(bone.limb, Limb::LeftArm | Limb::RightArm)
                && matches!(bone.part, UpperArm | Forearm | Hand)
        };
        let mut links = vec![
            self.link_of(body, below_back),
            self.link_of(body, |bone| {
                !below_back(bone) && bone.part != Skull && !beyond_shoulder(bone)
            }),
            self.link_of(body, |bone| bone.part == Skull),
        ];
        for &(limb, _) in &arms {
            links.push(self.link_of(body, |bone| {
                bone.limb == limb && matches!(bone.part, UpperArm | Forearm | Hand)
            }));
        }
        // Every joint is driven, not held round its pieces: its own hold
        // keeps its bones together, as a punch's lower back, neck, and
        // guard arm's shoulder are.
        let swing = |joint: usize, inner: usize, outer: usize| Swing {
            joint,
            body,
            inner,
            outer,
            pinned: false,
            planted: false,
            guard_impulses: (
                self.bone_joints[joint].drive_impulse,
                self.bone_joints[joint].hold_impulse,
            ),
            active: true,
            sides: None,
        };
        // The feet stand on the floor, which takes the lower back's push:
        // only the trunk turns. Pushed back as a punch's lower back once
        // pushed them, the hips and legs swung round their middle, a leg
        // kicked out, and the whole body rocked for a second, so a lean held
        // long swayed back into the punch it was leaning from.
        let mut swings = vec![
            Swing {
                planted: true,
                ..swing(back, 1, 0)
            },
            swing(neck, 2, 1),
        ];
        for (index, &(_, joint)) in arms.iter().enumerate() {
            swings.push(swing(joint, 1, 3 + index));
        }
        Some(Lean {
            amount: 0.0,
            upright: 0.0,
            away,
            swings,
            links,
        })
    }

    /// Gives each joint `lean` drives back to its own muscles, with what
    /// they carried in the guard before the lean, as a punch does once done.
    fn release_lean(&mut self, lean: &mut Lean) {
        for swing in lean.swings.iter_mut().filter(|swing| swing.active) {
            swing.active = false;
            if let Some(joint) = self.bone_joints.get_mut(swing.joint) {
                joint.drive.swing = 0.0;
                joint.drive_impulse = swing.guard_impulses.0;
            }
        }
    }

    /// A punch that body `body` throws ends its lean: the punch takes over
    /// each joint the lean drives, with what its muscles carried in the
    /// guard before the lean. The lean begins again once the punching arm
    /// is home, if it is still held.
    pub(super) fn hand_lean_to_punch(&mut self, body: usize, swings: &mut [Swing]) {
        let Some(mut lean) = self
            .bodies
            .get_mut(body)
            .and_then(|state| state.lean.take())
        else {
            return;
        };
        for swing in swings.iter_mut() {
            for old in lean.swings.iter_mut().filter(|old| old.active) {
                if old.joint == swing.joint {
                    old.active = false;
                    swing.guard_impulses = old.guard_impulses;
                }
            }
        }
        self.release_lean(&mut lean);
    }

    /// How hard each side of each joint a lean drives is to move, from
    /// where everything was when this substep began.
    pub(super) fn measure_leans(&mut self) {
        for body in 0..self.bodies.len() {
            let Some(mut lean) = self.bodies[body].lean.take() else {
                continue;
            };
            for swing in lean.swings.iter_mut().filter(|swing| swing.active) {
                swing.sides = self.sides(swing, &lean.links);
            }
            self.bodies[body].lean = Some(lean);
        }
    }

    /// The muscles across each joint a lean drives, as a punch's
    /// (`solve_punch_drives`): turning the piece beyond the joint toward the
    /// lean and pushing back on the piece before it. The lower back's lean
    /// in the trunk as a critically damped spring; the neck and the far
    /// shoulder hold the head and arm to the trunk as a punch's braces do.
    pub(super) fn solve_lean_drives(&mut self, h: f64) {
        for body in 0..self.bodies.len() {
            let Some(lean) = self.bodies[body].lean.take() else {
                continue;
            };
            for (index, swing) in lean.swings.iter().enumerate() {
                if !swing.active {
                    continue;
                }
                if index == 0 {
                    self.solve_lean_swing(swing, &lean.links, h);
                } else {
                    self.solve_swing(swing, &lean.links, h);
                }
            }
            self.bodies[body].lean = Some(lean);
        }
    }

    /// The lower back's muscles leaning the trunk toward the lean they aim
    /// at, no harder than the joint's strength: a spring of `LEAN_HERTZ` on
    /// the two pieces it turns, damped critically, so the trunk comes to the
    /// lean and stays.
    fn solve_lean_swing(&mut self, swing: &Swing, links: &[Link], h: f64) {
        let joint = self.bone_joints[swing.joint];
        let limit = self.drive_limit(&joint) * h;
        if limit <= 0.0 {
            self.bone_joints[swing.joint].drive_impulse = 0.0;
            return;
        }
        let Some(sides) = swing.sides else {
            return;
        };
        let mass = sides.turning_mass(swing);
        let softness = Softness::new(2.0 * PI * LEAN_HERTZ, 1.0, h);
        let turn = JointTurn::of(&joint, &self.bones);
        let angle = wrap_angle(turn.angle - joint.rest_angle - joint.drive.aim - joint.drive.swing);
        let speed = turn.speed(&self.bones, h);
        let given = joint.drive_impulse.clamp(-limit, limit);
        let total = (given + softness.impulse(mass, speed, angle, given)).clamp(-limit, limit);
        self.bone_joints[swing.joint].drive_impulse = total;
        self.turn_pieces(swing, links, &sides, total - given, h);
    }

    /// Gives each joint a lean drives the impulse it carried last substep.
    pub(super) fn warm_start_lean_drives(&mut self, h: f64) {
        for body in 0..self.bodies.len() {
            let Some(lean) = self.bodies[body].lean.take() else {
                continue;
            };
            for swing in lean.swings.iter().filter(|swing| swing.active) {
                let joint = self.bone_joints[swing.joint];
                let limit = self.drive_limit(&joint) * h;
                let given = joint.drive_impulse.clamp(-limit, limit);
                self.bone_joints[swing.joint].drive_impulse = given;
                if let (true, Some(sides)) = (given != 0.0, swing.sides) {
                    self.turn_pieces(swing, &lean.links, &sides, given, h);
                }
            }
            self.bodies[body].lean = Some(lean);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::SCENARIO_WIDTH;

    /// The middle of body `body`'s skull.
    fn head(world: &World, body: usize) -> Vec2 {
        world
            .bones
            .iter()
            .find(|bone| bone.part == BonePart::Skull && bone.body as usize == body)
            .map(|skull| lerp(skull.a, skull.b, 0.5))
            .unwrap()
    }

    fn steps(world: &mut World, count: usize) {
        let dt = world.materials.fixed_dt;
        for _ in 0..count {
            world.step(dt, &InputState::default(), SCENARIO_WIDTH);
        }
    }

    /// Held, a lean back carries the head well away from the other fighter
    /// and holds it there without rocking; let go, the body comes back
    /// upright, and leaning hurts nothing. Driven as a punch drives the
    /// lower back, the trunk overshot and rocked, and a lean held a second
    /// swayed back into the punch it leaned from.
    #[test]
    fn a_held_lean_leans_away_holds_and_comes_back() {
        let mut world = create_fighters(Materials::default());
        let px = 1000.0 / world.mm_per_px();
        let start = head(&world, 1);
        world.hold_lean(1, true);
        let mut away = Vec::new();
        for _ in 0..90 {
            steps(&mut world, 1);
            away.push(head(&world, 1).x - start.x);
        }
        let held = away[30..].iter().fold(f64::MIN, |a, &b| a.max(b));
        let least = away[30..].iter().fold(f64::MAX, |a, &b| a.min(b));
        assert!(
            least > 0.4 * px,
            "the head leaned only {:.2} m away from the other fighter",
            least / px
        );
        // It eases in a few centimetres as the flesh settles on the leaning
        // bones; driven at full strength, it swayed back 8 cm and more.
        assert!(
            held - least < 0.16 * px,
            "the leaning head swayed {:.1} cm once leaned",
            (held - least) / px * 100.0
        );
        world.hold_lean(1, false);
        let mut swung = 0.0_f64;
        for _ in 0..150 {
            steps(&mut world, 1);
            swung = swung.max(start.x - head(&world, 1).x);
        }
        // Brought back in half a second, it swung 16 cm past.
        assert!(
            swung < 0.08 * px,
            "coming back, the head swung {:.1} cm past where it stood, toward the other fighter",
            swung / px * 100.0
        );
        assert!(
            world.bodies[1].lean.is_none(),
            "the lean is over once the body is upright"
        );
        let back = distance(head(&world, 1), start);
        assert!(
            back < 0.03 * px,
            "the head came back {back:.1} px from where it stood"
        );
        let stats = world.stats;
        assert_eq!(
            (
                stats.fractured_bones,
                stats.broken_skin,
                stats.broken_muscle,
                stats.broken_bone_joints,
                stats.broken_bone_attachments,
                stats.broken_attachments,
            ),
            (0, 0, 0, 0, 0, 0),
            "leaning back hurt the body"
        );
    }

    /// A punch thrown ends the lean, which waits for the punching arm to
    /// come home before it leans again, if still held.
    #[test]
    fn a_punch_ends_a_lean_which_waits_for_the_arm() {
        let mut world = create_fighters(Materials::default());
        let arm = world.inner_arm(0);
        world.hold_lean(0, true);
        steps(&mut world, 10);
        assert!(world.leaning(0) > 0.0, "the held lean began");
        world.throw_punch(0, arm);
        assert_eq!(
            world.leaning(0),
            0.0,
            "the punch took the lean's joints over"
        );
        steps(&mut world, 10);
        assert_eq!(
            world.leaning(0),
            0.0,
            "the lean waited while the arm was out"
        );
        steps(&mut world, 40);
        assert!(
            world.leaning(0) > 0.0,
            "the lean began again once the arm was home"
        );
    }

    /// Two fighters leaning back at once stay each other's mirror images,
    /// as they stand: each leans away from the other alike.
    #[test]
    fn fighters_lean_back_as_mirror_images() {
        let mut world = create_fighters(Materials::default());
        let half = world.points.len() / 2;
        let [left, right] = crate::fighter_frames();
        let middle = (left.origin.x + right.origin.x) * 0.5;
        world.hold_lean(0, true);
        world.hold_lean(1, true);
        steps(&mut world, 45);
        let apart = (0..half)
            .map(|index| {
                let (a, b) = (
                    world.points[index].position,
                    world.points[half + index].position,
                );
                (a.x + b.x - 2.0 * middle).abs().max((a.y - b.y).abs())
            })
            .fold(0.0, f64::max);
        assert!(
            apart < 1.0e-3,
            "leaning, the fighters stood {apart:.2e} px off mirror images"
        );
        assert!(
            head(&world, 0).x < left.origin.x && head(&world, 1).x > right.origin.x,
            "each leaned away from the other"
        );
    }
}
