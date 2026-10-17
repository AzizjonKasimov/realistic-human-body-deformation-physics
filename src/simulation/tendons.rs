//! The muscles and tendons that cross the elbow, each a cord from a place on
//! the upper arm to a place on the forearm that pulls only once it is
//! stretched past its slack. Slack through the elbow's whole range, they leave
//! the arm's motion alone, so a limp arm still hangs and an arm still
//! straightens; once the elbow tears or a bone is smashed, they keep the arm's
//! bones together, as a real arm's muscles keep a smashed forearm hanging
//! from the elbow. Without them, once the inner elbow was a crease rather than
//! one block of flesh, broken bone left the body in about three times as many
//! sweep runs.
//!
//! Each is MuJoCo's spatial tendon with a length limit (Google DeepMind,
//! Apache-2.0: the limit is a one-sided constraint on the tendon's length),
//! here a straight cord solved as Box2D v3's distance joint solves its upper
//! limit (Erin Catto, MIT, `distance_joint.c`): speculative while slack, so
//! it never overshoots, and a soft spring as stiff as the joints once taut.
//! Where each attaches comes from the arm of Holzbaur, Murray, and Delp 2005
//! (OpenSim's arm26 model, its data under CC BY 3.0); the figures and sources
//! are in `docs/INJURY_REFERENCE.md` (The muscles across the elbow).

use super::soft::{bone_place, perpendicular, speed_up_bone, Softness};
use super::*;

/// A muscle crossing the elbow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Muscle {
    Brachialis,
    Biceps,
    Brachioradialis,
    Triceps,
}

impl Muscle {
    pub fn name(self) -> &'static str {
        match self {
            Muscle::Brachialis => "brachialis",
            Muscle::Biceps => "biceps",
            Muscle::Brachioradialis => "brachioradialis",
            Muscle::Triceps => "triceps",
        }
    }
}

/// Where a tendon attaches: on `bone`, at `(1 - tau) a + tau b + sigma
/// perp(b - a)`, so it turns and slides with the bone as a muscle tie's
/// anchor does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TendonEnd {
    pub bone: usize,
    pub tau: f64,
    pub sigma: f64,
}

impl TendonEnd {
    /// Where it is on a bone with ends `a` and `b`, or how fast it moves for
    /// ends moving at `a` and `b`.
    fn on(&self, a: Vec2, b: Vec2) -> Vec2 {
        add(
            lerp(a, b, self.tau),
            scale(perpendicular(subtract(b, a)), self.sigma),
        )
    }
}

/// A muscle and its tendon across the elbow, from the upper arm (`from`) to
/// the forearm (`to`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tendon {
    pub muscle: Muscle,
    pub limb: Limb,
    pub from: TendonEnd,
    pub to: TendonEnd,
    /// How long it is when it starts to pull, in pixels: the longest it gets
    /// over the elbow's whole range, and `TENDON_SLACK_MM` more.
    pub slack: f64,
    /// The impulse it pulled with last substep, which warm starts the next;
    /// never more than nothing, as it only pulls.
    pub impulse: f64,
}

/// The muscles across each elbow, and where each attaches, in centimetres
/// from the middle of the elbow: on the upper arm how far above it and how
/// far in front, then on the forearm how far below it and how far in front
/// (in front is the side the elbow bends toward, the guard's inner elbow;
/// negative is behind, the point of the elbow). The forearm's flexors and
/// extensors, which run on from the epicondyles to the hand, are left out:
/// pulled by them, the hand's bone came out of the fist's flesh when a
/// smashed forearm hung on them.
const ARM_TENDONS: [(Muscle, (f64, f64), (f64, f64)); 4] = [
    // Holzbaur et al. 2005: the brachialis from 11.7 cm above the elbow to the
    // ulna 2.4 below it, lying on the front of the bones (1 cm in front, an
    // estimate); the biceps's last point on the humerus 11.5 above and 1.7 in
    // front, to the radius 4.8 below and 0.8 in front.
    (Muscle::Brachialis, (11.7, 1.0), (2.4, 0.6)),
    (Muscle::Biceps, (11.5, 1.7), (4.8, 0.8)),
    // The brachioradialis from the ridge above the elbow, 4 to 10 cm up, to
    // the base of the radius's styloid at the wrist (Holzbaur et al. give no
    // distances for it; StatPearls).
    (Muscle::Brachioradialis, (7.0, 1.0), (22.0, 0.6)),
    // The triceps's medial and lateral heads from 15.3 and 16.4 cm above,
    // behind the humerus, round the back of the elbow to the olecranon, 1.0
    // cm above the elbow's middle and 2.2 behind it.
    (Muscle::Triceps, (15.8, -1.5), (-1.0, -2.2)),
];

/// How much longer than its longest over the elbow's range a tendon is when
/// it starts to pull, in millimetres. A relaxed muscle takes centimetres of
/// stretch before it pulls hard (a relaxed biceps about 7 cm to carry its
/// full strength, Thelen 2003), and the joints give a few pixels under load,
/// so a cord as tight as the range would hold a limb at the ends of it.
const TENDON_SLACK_MM: f64 = 10.0;

/// How finely the elbow's range is swept for the longest a tendon gets.
const RANGE_SAMPLES: usize = 48;

/// How a tendon lies when the substep began: how long it is, along which
/// line from its first end to its second, and how hard its ends are to move
/// apart.
struct TendonLine {
    length: f64,
    normal: Vec2,
    inverse_mass: f64,
}

impl TendonLine {
    fn of(tendon: &Tendon, bones: &[BoneSegment]) -> Option<Self> {
        let (from, to) = (bones.get(tendon.from.bone)?, bones.get(tendon.to.bone)?);
        if tendon.from.bone == tendon.to.bone {
            return None;
        }
        let (a0, a1) = bone_place(from);
        let (b0, b1) = bone_place(to);
        let along = subtract(tendon.to.on(b0, b1), tendon.from.on(a0, a1));
        let length = hypot(along.x, along.y);
        if length <= EPSILON {
            return None;
        }
        // A pull along the cord at an end moves the bone's ends by their
        // shares of it, and turns the bone by the end's place across it.
        let reach =
            |end: &TendonEnd| (1.0 - end.tau).powi(2) + end.tau.powi(2) + 2.0 * end.sigma.powi(2);
        Some(Self {
            length,
            normal: scale(along, 1.0 / length),
            inverse_mass: from.inverse_end_mass() * reach(&tendon.from)
                + to.inverse_end_mass() * reach(&tendon.to),
        })
    }

    /// How fast the tendon lengthens.
    fn speed(&self, tendon: &Tendon, bones: &[BoneSegment], h: f64) -> f64 {
        let moving = |end: &TendonEnd| {
            let bone = &bones[end.bone];
            scale(
                end.on(
                    subtract(bone.a, bone.previous_a),
                    subtract(bone.b, bone.previous_b),
                ),
                1.0 / h,
            )
        };
        dot(
            self.normal,
            subtract(moving(&tendon.to), moving(&tendon.from)),
        )
    }
}

impl World {
    /// The muscles and tendons across the joints.
    pub fn tendons(&self) -> &[Tendon] {
        &self.tendons
    }

    /// Where tendon `index`'s two ends are now.
    pub fn tendon_ends(&self, index: usize) -> Option<(Vec2, Vec2)> {
        let tendon = self.tendons.get(index)?;
        let (from, to) = (
            self.bones.get(tendon.from.bone)?,
            self.bones.get(tendon.to.bone)?,
        );
        Some((tendon.from.on(from.a, from.b), tendon.to.on(to.a, to.b)))
    }

    /// How long tendon `index` is now, in pixels.
    pub fn tendon_length(&self, index: usize) -> Option<f64> {
        let (from, to) = self.tendon_ends(index)?;
        Some(distance(from, to))
    }

    /// Adds the muscles across the elbow of `limb`, whose upper arm and
    /// forearm are bones `upper_arm` and `forearm`, as the arm lies when
    /// built, each as slack as the elbow's whole range lets it be.
    pub(super) fn add_arm_tendons(&mut self, limb: Limb, upper_arm: usize, forearm: usize) {
        let (upper, fore) = (self.bones[upper_arm], self.bones[forearm]);
        let Some(elbow) = self
            .bone_joints
            .iter()
            .find(|joint| joint.a == upper_arm && joint.b == forearm)
            .copied()
        else {
            return;
        };
        let mm_per_px = self.mm_per_px();
        let px = |cm: f64| cm * 10.0 / mm_per_px;
        // The middle of the elbow is where the forearm turns, the end of it
        // the joint holds (`soft.rs`).
        let middle = fore.home_a;
        let above = normalized(
            subtract(upper.home_a, upper.home_b),
            Vec2 { x: 0.0, y: -1.0 },
        );
        let below = normalized(subtract(fore.home_b, fore.home_a), Vec2 { x: 0.0, y: -1.0 });
        // In front is the side the elbow bends toward.
        let fold = normalized(add(above, below), Vec2 { x: 1.0, y: 0.0 });
        let front = |along: Vec2| {
            let side = perpendicular(along);
            if dot(side, fold) >= 0.0 {
                side
            } else {
                scale(side, -1.0)
            }
        };
        let end_on = |bone: &BoneSegment, index: usize, along: Vec2, cm: (f64, f64)| {
            let at = add(
                add(middle, scale(along, px(cm.0))),
                scale(front(along), px(cm.1)),
            );
            let (tau, sigma) = bone_frame(bone.home_a, bone.home_b, at);
            TendonEnd {
                bone: index,
                tau,
                sigma,
            }
        };
        for (muscle, origin, insertion) in ARM_TENDONS {
            let mut tendon = Tendon {
                muscle,
                limb,
                from: end_on(&upper, upper_arm, above, origin),
                to: end_on(&fore, forearm, below, insertion),
                slack: 0.0,
                impulse: 0.0,
            };
            tendon.slack = self.longest_over_range(&tendon, &elbow) + TENDON_SLACK_MM / mm_per_px;
            self.tendons.push(tendon);
        }
    }

    /// The longest `tendon` gets as the forearm turns about its end at the
    /// `elbow` through the elbow's whole range, from the arm as built.
    fn longest_over_range(&self, tendon: &Tendon, elbow: &BoneJoint) -> f64 {
        let home = |end: &TendonEnd| {
            let bone = &self.bones[end.bone];
            end.on(bone.home_a, bone.home_b)
        };
        let (from, to) = (home(&tendon.from), home(&tendon.to));
        let pivot = self.bones[elbow.b].home_a;
        let reach = subtract(to, pivot);
        (0..=RANGE_SAMPLES)
            .map(|k| {
                let bend = elbow.min_angle
                    + (elbow.max_angle - elbow.min_angle) * k as f64 / RANGE_SAMPLES as f64;
                let (sin, cos) = (libm::sin(bend), libm::cos(bend));
                let turned = add(
                    pivot,
                    Vec2 {
                        x: reach.x * cos - reach.y * sin,
                        y: reach.x * sin + reach.y * cos,
                    },
                );
                distance(from, turned)
            })
            .fold(0.0, f64::max)
    }

    /// Each tendon pulls its ends together once stretched past its slack:
    /// as stiff as a joint once taut, and while slack, only as hard as keeps
    /// it from stretching past its slack within the substep, as Box2D's
    /// distance joint holds its upper limit.
    pub(super) fn solve_tendons(&mut self, h: f64) {
        let hertz = self.materials.bone_joint_hertz.min(0.25 / h);
        let softness = Softness::new(2.0 * PI * hertz, self.materials.bone_joint_damping_ratio, h);
        for index in 0..self.tendons.len() {
            let tendon = self.tendons[index];
            let Some(line) = TendonLine::of(&tendon, &self.bones) else {
                self.tendons[index].impulse = 0.0;
                continue;
            };
            if line.inverse_mass <= EPSILON {
                continue;
            }
            let mass = 1.0 / line.inverse_mass;
            let error = line.length - tendon.slack;
            let speed = line.speed(&tendon, &self.bones, h);
            let given = tendon.impulse;
            let impulse = if error < 0.0 {
                -mass * (speed + error / h)
            } else {
                softness.impulse(mass, speed, error, given)
            };
            // A tendon only pulls.
            let total = (given + impulse).min(0.0);
            self.tendons[index].impulse = total;
            self.apply_tendon(&tendon, &line, total - given, h);
        }
    }

    pub(super) fn warm_start_tendons(&mut self, h: f64) {
        for index in 0..self.tendons.len() {
            let tendon = self.tendons[index];
            if tendon.impulse == 0.0 {
                continue;
            }
            match TendonLine::of(&tendon, &self.bones) {
                Some(line) => self.apply_tendon(&tendon, &line, tendon.impulse, h),
                None => self.tendons[index].impulse = 0.0,
            }
        }
    }

    /// Gives `tendon`'s second end `impulse` along `line` and its first the
    /// same back, each bone's ends their shares of it at the end's place.
    fn apply_tendon(&mut self, tendon: &Tendon, line: &TendonLine, impulse: f64, h: f64) {
        let push = scale(line.normal, impulse);
        let turned = perpendicular(push);
        for (end, sign) in [(&tendon.to, 1.0), (&tendon.from, -1.0)] {
            let bone = &mut self.bones[end.bone];
            let w = bone.inverse_end_mass() * sign;
            let change_a = scale(add(scale(push, 1.0 - end.tau), scale(turned, end.sigma)), w);
            let change_b = scale(subtract(scale(push, end.tau), scale(turned, end.sigma)), w);
            speed_up_bone(bone, change_a, change_b, h);
        }
    }

    /// Moves the tendons on bone `bone`, broken at `break_t` of the way
    /// along it, onto its two pieces: the first keeps its index and the
    /// stretch before the break, `second` the rest. What each pulled with
    /// was for the whole bone.
    pub(super) fn split_tendons_at_break(&mut self, bone: usize, second: usize, break_t: f64) {
        let first_share = break_t.max(0.05);
        let second_share = (1.0 - break_t).max(0.05);
        for tendon in &mut self.tendons {
            for end in [&mut tendon.from, &mut tendon.to] {
                if end.bone != bone {
                    continue;
                }
                if end.tau < break_t {
                    end.tau /= first_share;
                    end.sigma /= first_share;
                } else {
                    end.bone = second;
                    end.tau = (end.tau - break_t) / second_share;
                    end.sigma /= second_share;
                }
                tendon.impulse = 0.0;
            }
        }
    }
}

/// Where `p` sits on a bone running from `a` to `b`: how far along it, as a
/// share of its length, and how far across it, a quarter turn
/// counterclockwise, as a share of its length.
fn bone_frame(a: Vec2, b: Vec2, p: Vec2) -> (f64, f64) {
    let along = subtract(b, a);
    let length_sq = dot(along, along).max(EPSILON);
    let offset = subtract(p, a);
    (
        dot(offset, along) / length_sq,
        cross(along, offset) / length_sq,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> World {
        create_layered_body(1280.0, 720.0, Materials::default())
    }

    /// Every arm has its muscles across the elbow, and once the body has
    /// settled in the guard none of them pulls: slack through the elbow's
    /// range, they leave the arm's motion alone.
    #[test]
    fn the_arms_tendons_are_slack_in_the_guard() {
        let world = body();
        assert_eq!(world.tendons.len(), 2 * ARM_TENDONS.len());
        for (index, tendon) in world.tendons.iter().enumerate() {
            let length = world.tendon_length(index).unwrap();
            assert!(
                length < tendon.slack - 1.0,
                "the {} of the {:?} is {length:.1} px long, slack to {:.1}",
                tendon.muscle.name(),
                tendon.limb,
                tendon.slack
            );
            assert_eq!(tendon.impulse, 0.0, "the {} pulls", tendon.muscle.name());
        }
    }

    /// The two arms' tendons are each other's mirror images.
    #[test]
    fn the_arms_tendons_mirror_each_other() {
        let world = body();
        let middle = world.bones[1].home_a.x;
        let place = |end: &TendonEnd| {
            let bone = &world.bones[end.bone];
            end.on(bone.home_a, bone.home_b)
        };
        let (left, right): (Vec<&Tendon>, Vec<&Tendon>) = world
            .tendons
            .iter()
            .partition(|tendon| tendon.limb == Limb::LeftArm);
        assert_eq!(left.len(), right.len());
        for (l, r) in left.iter().zip(&right) {
            assert_eq!(l.muscle, r.muscle);
            assert!(
                (l.slack - r.slack).abs() < 1.0e-6,
                "the {}'s slack",
                l.muscle.name()
            );
            for (a, b) in [
                (place(&l.from), place(&r.from)),
                (place(&l.to), place(&r.to)),
            ] {
                assert!(
                    (a.x - middle + (b.x - middle)).abs() < 1.0e-6 && (a.y - b.y).abs() < 1.0e-6,
                    "the {}'s ends mirror: {a:?} {b:?}",
                    l.muscle.name()
                );
            }
        }
    }

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// A tendon's pull changes how fast it lengthens by exactly the
        /// impulse over its effective mass, wherever its two bones lie and
        /// wherever on them, along or across, its ends sit: the effective
        /// mass the solver divides by is the one the bones' ends give.
        #[test]
        fn a_tendon_moves_its_ends_as_its_mass_says(
            ax in -200.0..200.0f64, ay in -200.0..200.0f64, bx in -200.0..200.0f64, by in -200.0..200.0f64,
            cx in -200.0..200.0f64, cy in -200.0..200.0f64, dx in -200.0..200.0f64, dy in -200.0..200.0f64,
            tau_a in -0.3..1.0f64, tau_b in -0.3..1.0f64,
            sigma_a in -0.2..0.2f64, sigma_b in -0.2..0.2f64,
            impulse in -50.0..0.0f64,
        ) {
            let (a0, a1) = (Vec2 { x: ax, y: ay }, Vec2 { x: bx, y: by });
            let (b0, b1) = (Vec2 { x: cx, y: cy }, Vec2 { x: dx, y: dy });
            prop_assume!(distance(a0, a1) > 5.0 && distance(b0, b1) > 5.0);
            let mut world = World::new(Materials::default());
            let first = world.add_bone_segment(a0, a1, 4.0, 1.0e9, false);
            let second = world.add_bone_segment(b0, b1, 4.0, 1.0e9, false);
            world.bones[first].mass = 3.0;
            world.bones[second].mass = 7.0;
            let tendon = Tendon {
                muscle: Muscle::Biceps,
                limb: Limb::LeftArm,
                from: TendonEnd { bone: first, tau: tau_a, sigma: sigma_a },
                to: TendonEnd { bone: second, tau: tau_b, sigma: sigma_b },
                slack: 0.0,
                impulse: 0.0,
            };
            let line = TendonLine::of(&tendon, &world.bones);
            prop_assume!(line.is_some());
            let line = line.unwrap();
            let h = 1.0 / 240.0;
            world.apply_tendon(&tendon, &line, impulse, h);
            let speed = line.speed(&tendon, &world.bones, h);
            let expected = impulse * line.inverse_mass;
            prop_assert!(
                (speed - expected).abs() < 1e-6 * (1.0 + expected.abs()),
                "lengthens at {speed}, expected {expected}"
            );
        }
    }

    /// Two bones, the first held still, the second thrown off it at 600 px/s
    /// (about 2 m/s): a tendon between them, slack while they lie as they
    /// were, pulls nothing then, and once they part stops them at its slack,
    /// stretching less than a pixel past it; without it they fly apart.
    #[test]
    fn a_tendon_holds_its_bones_at_its_slack() {
        let mut world = World::new(Materials {
            gravity: 0.0,
            linear_drag: 0.0,
            ..Materials::default()
        });
        let first = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 200.0, y: 100.0 },
            5.0,
            1.0e9,
            true,
        );
        let second = world.add_bone_segment(
            Vec2 { x: 208.0, y: 100.0 },
            Vec2 { x: 308.0, y: 100.0 },
            5.0,
            1.0e9,
            false,
        );
        world.tendons.push(Tendon {
            muscle: Muscle::Triceps,
            limb: Limb::LeftArm,
            from: TendonEnd {
                bone: first,
                tau: 0.6,
                sigma: 0.05,
            },
            to: TendonEnd {
                bone: second,
                tau: 0.1,
                sigma: 0.05,
            },
            slack: 0.0,
            impulse: 0.0,
        });
        let slack = world.tendon_length(0).unwrap() + 10.0;
        world.tendons[0].slack = slack;
        let dt = world.materials.fixed_dt;
        for _ in 0..10 {
            world.step(dt, &InputState::default(), 1000.0);
        }
        assert_eq!(world.tendons[0].impulse, 0.0, "a slack tendon pulls");
        let mut thrown = world.clone();
        thrown.tendons.clear();
        let throw = 600.0 * dt;
        for world in [&mut world, &mut thrown] {
            world.bones[second].previous_a.x -= throw;
            world.bones[second].previous_b.x -= throw;
        }
        let mut longest: f64 = 0.0;
        for _ in 0..60 {
            world.step(dt, &InputState::default(), 1000.0);
            thrown.step(dt, &InputState::default(), 1000.0);
            longest = longest.max(world.tendon_length(0).unwrap());
        }
        let apart = |world: &World| distance(world.bones[first].b, world.bones[second].a);
        assert!(
            longest < slack + 1.0 && longest > slack - 1.0,
            "the tendon, slack to {slack:.1} px, got to {longest:.1}"
        );
        assert!(
            apart(&thrown) > 100.0 && apart(&world) < 60.0,
            "thrown apart, the bones' ends lie {:.1} px apart on a tendon and {:.1} without",
            apart(&world),
            apart(&thrown)
        );
    }
}
