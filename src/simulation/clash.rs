//! Where two bodies' limbs meet, they press on each other's bones through the
//! flesh between them. The skins of two fighters push each other apart
//! (`outline.rs`), but skin moves only skin and the muscle under it: two arms
//! driven into each other by a punch's muscles, as two fighters punching a
//! few steps apart drive their guards elbow into elbow, ran their bones on
//! through each other's flesh until the bones crossed, the skin crumpled, and
//! bone showed.
//!
//! So each bone of a limb, and the skull, is a capsule as thick as the flesh
//! round it, and where two bodies' capsules overlap they are pushed apart as
//! Box2D v3 pushes two bodies' shapes apart (Erin Catto, MIT, `contact_solver.c`:
//! a soft contact at 30 Hz, damped tenfold, pushing out no faster than 3 m/s,
//! speculative before they touch), on the bone and the flesh held to it as one
//! piece, in the same pass as the muscles that drive the limbs, so a driven
//! arm is stopped or turned aside by the other instead of passing through. A
//! punching arm pressed hard gives way (`World::block_punch`).

use super::soft::Softness;
use super::stance::BoundPiece;
use super::*;

/// How stiff the push between two limbs is, in hertz, and how damped: Box2D's
/// contact softness (`b2_contactHertz`, `contactDampingRatio`), capped at a
/// quarter of the substep's rate as it caps it.
const CLASH_HERTZ: f64 = 30.0;
const CLASH_DAMPING_RATIO: f64 = 10.0;

/// The fastest an overlap of two limbs is pushed apart, in metres a second:
/// Box2D's `contactSpeed`, as the skin's own contact (`OUTLINE_PUSH_SPEED`).
const CLASH_PUSH_SPEED: f64 = 3.0;

/// How hard, in newtons, another body may press on an arm its punch drives
/// before the punch gives way: driven on at full strength into an arm held
/// there, a punching arm pressed it into its own body until a forearm's bone
/// came out through its skin, the hips' hold pushing behind both.
const CLASH_BLOCK_FORCE: f64 = 400.0;

/// A bone of one body and a bone of another whose flesh overlaps, or may
/// overlap within this substep, and how they press on each other.
#[derive(Clone, Debug)]
pub(super) struct Clash {
    /// The two bones and the flesh held to each, moved as one piece.
    sides: [ClashSide; 2],
    /// From the first bone toward the second, where they come closest.
    normal: Vec2,
    /// The flesh both carry, axis to axis: how near the bones may come.
    reach: f64,
    /// One over how hard the two pieces are to move apart along `normal`.
    normal_mass: f64,
    /// The impulse it has given this substep, pushing them apart.
    impulse: f64,
}

/// One side of a [`Clash`]: a bone and the flesh held to it as one piece,
/// with the rest of a punching arm beyond it.
#[derive(Clone, Debug)]
struct ClashSide {
    bone: usize,
    /// Where along the bone, from its `a` end, the two bones come closest.
    along: f64,
    /// The bone met, and for an arm throwing a punch, the bones beyond it
    /// out to the hand, with the flesh held to them (`clash_bones`).
    piece: BoundPiece,
    /// From the piece's centre of mass to where the bones come closest.
    lever: Vec2,
}

/// Where along two segments, from `a0` and `b0`, they come closest, and the
/// two closest points: where they cross, the crossing.
fn closest_between(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> (f64, f64, Vec2, Vec2) {
    let pair = closest_segment_points(a0, a1, b0, b1);
    (pair.t_a, pair.t_b, pair.point_a, pair.point_b)
}

impl World {
    /// Measures, once, how thick each bone's flesh is: the middle distance
    /// from the bone to the points of the skin's outline that keep their
    /// place mostly on it, as built.
    pub(super) fn measure_bone_flesh(&mut self) {
        let mut reaches: Vec<Vec<f64>> = vec![Vec::new(); self.bones.len()];
        for outline in &self.outlines {
            for &index in outline.points() {
                let point = self.points[index];
                if point.layer != TissueLayer::Skin || point.is_ghost() {
                    continue;
                }
                let binding = self.point_holds[index].posture;
                for slot in 0..2 {
                    let bone = binding.bones[slot];
                    if binding.weights[slot] < 0.5 {
                        continue;
                    }
                    if let Some(segment) = self.bones.get(bone) {
                        reaches[bone].push(distance_to_segment(
                            point.home,
                            segment.home_a,
                            segment.home_b,
                        ));
                    }
                }
            }
        }
        for (bone, mut reach) in self.bones.iter_mut().zip(reaches) {
            reach.sort_by(f64::total_cmp);
            bone.flesh = reach.get(reach.len() / 2).copied().unwrap_or(0.0);
        }
    }

    /// Whether bone `index` meets other bodies' limbs: a whole bone of an
    /// arm or a leg, or the skull, with flesh round it. The hand of an arm
    /// whose punch is on its way out is left to its knuckles, which land as
    /// the bare fist does (`land_fist`); coming back, left out, it sank
    /// into the arm that had blocked it.
    fn clashes_with_others(&self, index: usize) -> bool {
        let Some(bone) = self.bones.get(index) else {
            return false;
        };
        let limb_part = matches!(
            bone.part,
            BonePart::UpperArm
                | BonePart::Forearm
                | BonePart::Hand
                | BonePart::Thigh
                | BonePart::Shin
                | BonePart::Foot
                | BonePart::Skull
        );
        let punching_fist = bone.part == BonePart::Hand
            && self
                .punch(bone.body as usize, bone.limb)
                .is_some_and(|punch| punch.going_out(&self.materials));
        limb_part
            && bone.flesh > 0.0
            && !bone.fractured
            && !bone.splinter
            && !bone.pinned
            && !punching_fist
    }

    /// Finds, as this substep begins, the bones of different bodies whose
    /// flesh overlaps or may overlap before it ends, and how hard each side
    /// is to move there.
    pub(super) fn measure_clashes(&mut self, h: f64) {
        self.clashes.clear();
        if self.bodies.len() < 2 {
            return;
        }
        let candidates: Vec<usize> = (0..self.bones.len())
            .filter(|&index| self.clashes_with_others(index))
            .collect();
        for (slot, &first) in candidates.iter().enumerate() {
            for &second in &candidates[slot + 1..] {
                let (a, b) = (self.bones[first], self.bones[second]);
                if a.body == b.body {
                    continue;
                }
                let (t_a, t_b, p_a, p_b) =
                    closest_between(a.previous_a, a.previous_b, b.previous_a, b.previous_b);
                let apart = distance(p_a, p_b);
                let reach = a.flesh + b.flesh;
                // How far the two could close within the substep.
                let velocity = |bone: &BoneSegment, t: f64| {
                    lerp(
                        scale(subtract(bone.a, bone.previous_a), 1.0 / h),
                        scale(subtract(bone.b, bone.previous_b), 1.0 / h),
                        t,
                    )
                };
                let closing_velocity = subtract(velocity(&a, t_a), velocity(&b, t_b));
                let margin = hypot(closing_velocity.x, closing_velocity.y) * h;
                if apart > reach + margin {
                    continue;
                }
                // Nearly crossed, the normal is the way the second body
                // stands from the first.
                let normal = if apart > EPSILON {
                    scale(subtract(p_b, p_a), 1.0 / apart)
                } else {
                    let toward = self.midline_of(b.body as usize).unwrap_or(0.0)
                        - self.midline_of(a.body as usize).unwrap_or(0.0);
                    Vec2 {
                        x: if toward >= 0.0 { 1.0 } else { -1.0 },
                        y: 0.0,
                    }
                };
                let (Some(side_a), Some(side_b)) = (
                    self.clash_side(first, t_a, p_a),
                    self.clash_side(second, t_b, p_b),
                ) else {
                    continue;
                };
                let turning = |side: &ClashSide| {
                    let arm = side.lever.x * normal.y - side.lever.y * normal.x;
                    1.0 / side.piece.mass + arm * arm / side.piece.inertia
                };
                let normal_mass = 1.0 / (turning(&side_a) + turning(&side_b));
                self.clashes.push(Clash {
                    sides: [side_a, side_b],
                    normal,
                    reach,
                    normal_mass,
                    impulse: 0.0,
                });
            }
        }
    }

    /// Whether bone `bone` is part of an arm its punch still drives, met by
    /// another body's bone `other`.
    fn punching_arm(&self, bone: usize, other: usize) -> bool {
        let (Some(bone), Some(other)) = (self.bones.get(bone), self.bones.get(other)) else {
            return false;
        };
        bone.body != other.body
            && matches!(
                bone.part,
                BonePart::UpperArm | BonePart::Forearm | BonePart::Hand
            )
            && self
                .punch(bone.body as usize, bone.limb)
                .is_some_and(Punch::drives_arm)
    }

    /// The bones moved with bone `index` when another body's limb presses
    /// on it: the bone, and for an arm throwing a punch, the bones beyond it
    /// out to the hand, which the punch's muscles hold as one with it.
    /// Pressed on its own, a punching forearm was shoved one way while its
    /// muscles drove it the other, and the hand beyond it, left behind,
    /// pulled the wrist apart.
    fn clash_bones(&self, index: usize) -> Vec<usize> {
        let bone = self.bones[index];
        let beyond: &[BonePart] = match bone.part {
            BonePart::UpperArm => &[BonePart::Forearm, BonePart::Hand],
            BonePart::Forearm => &[BonePart::Hand],
            _ => &[],
        };
        let punching = self.punch(bone.body as usize, bone.limb).is_some();
        let mut bones = vec![index];
        if punching {
            bones.extend((0..self.bones.len()).filter(|&other| {
                let piece = self.bones[other];
                piece.body == bone.body
                    && piece.limb == bone.limb
                    && beyond.contains(&piece.part)
                    && !piece.fractured
                    && !piece.splinter
            }));
        }
        bones
    }

    /// Bone `index`, with any it moves with (`clash_bones`), and the flesh
    /// held to them as one piece, as the substep began, pressed at `at`,
    /// `along` the bone.
    fn clash_side(&self, index: usize, along: f64, at: Vec2) -> Option<ClashSide> {
        let piece = self.bound_piece(self.clash_bones(index))?;
        let lever = subtract(at, piece.centre);
        Some(ClashSide {
            bone: index,
            along,
            piece,
            lever,
        })
    }

    /// Pushes apart every pair of limbs whose flesh overlaps, as Box2D's
    /// soft contact does, each pair by how fast and how far it presses from
    /// where things stand as this pass reaches it, all at once (Jacobi), so
    /// that two fighters pressing on each other are solved alike, neither
    /// first. An arm its punch drives, pressed harder than
    /// `CLASH_BLOCK_FORCE`, gives way (`World::block_punch`).
    pub(super) fn solve_clashes(&mut self, h: f64) {
        if self.clashes.is_empty() {
            return;
        }
        let hertz = CLASH_HERTZ.min(0.25 / h);
        let softness = Softness::new(2.0 * PI * hertz, CLASH_DAMPING_RATIO, h);
        let max_push = CLASH_PUSH_SPEED * 1000.0 / self.mm_per_px();
        let mut pushes = Vec::with_capacity(self.clashes.len());
        for (index, clash) in self.clashes.iter_mut().enumerate() {
            let [first, second] = &clash.sides;
            let (a, b) = (self.bones[first.bone], self.bones[second.bone]);
            let gap = dot(
                subtract(lerp(b.a, b.b, second.along), lerp(a.a, a.b, first.along)),
                clash.normal,
            ) - clash.reach;
            let velocity = |bone: &BoneSegment, t: f64| {
                lerp(
                    scale(subtract(bone.a, bone.previous_a), 1.0 / h),
                    scale(subtract(bone.b, bone.previous_b), 1.0 / h),
                    t,
                )
            };
            let speed = dot(
                subtract(velocity(&b, second.along), velocity(&a, first.along)),
                clash.normal,
            );
            let impulse =
                softness.contact_impulse(clash.normal_mass, speed, gap, clash.impulse, max_push, h);
            let total = (clash.impulse + impulse).max(0.0);
            let applied = total - clash.impulse;
            clash.impulse = total;
            if applied != 0.0 {
                pushes.push((index, scale(clash.normal, applied)));
            }
        }
        let clashes = std::mem::take(&mut self.clashes);
        for (index, push) in pushes {
            let [first, second] = &clashes[index].sides;
            self.push_bound_piece(&first.piece, first.lever, scale(push, -1.0), h);
            self.push_bound_piece(&second.piece, second.lever, push, h);
        }
        self.clashes = clashes;
        self.debug.limb_clashes = self.clashes.len() as i32;
        let newtons = self.newtons();
        let mut blocked = Vec::new();
        for clash in &self.clashes {
            let force = clash.impulse / h / newtons;
            self.debug.max_clash_force = self.debug.max_clash_force.max(force);
            if force > CLASH_BLOCK_FORCE {
                let [first, second] = &clash.sides;
                for (bone, other) in [(first.bone, second.bone), (second.bone, first.bone)] {
                    if self.punching_arm(bone, other) {
                        blocked.push((bone, other, force));
                    }
                }
            }
        }
        for (bone, other, force) in blocked {
            self.block_punch(bone, other, Measure::Force, force, CLASH_BLOCK_FORCE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::SCENARIO_WIDTH;

    /// Two fighters each throwing a punch with the arm nearer the other,
    /// the left one `apart` steps after the right, played for a second and
    /// a half: how near the two near arms' bones came, as a share of the
    /// flesh both carry, and the world after.
    fn crossing_punches(apart: usize) -> (f64, World) {
        let mut world = create_fighters(Materials::default());
        let dt = world.materials.fixed_dt;
        let arms = [world.inner_arm(0), world.inner_arm(1)];
        let arm_bones: Vec<usize> = (0..world.bones.len())
            .filter(|&index| {
                let bone = world.bones[index];
                matches!(bone.part, BonePart::UpperArm | BonePart::Forearm)
                    && bone.limb == arms[bone.body as usize]
            })
            .collect();
        let mut nearest = f64::INFINITY;
        for step in 0..90 {
            let mut input = InputState::default();
            if step == 10 {
                input.punches[0] = Some((1, arms[1]));
            }
            if step == 10 + apart {
                input.punches[1] = Some((0, arms[0]));
            }
            world.step(dt, &input, SCENARIO_WIDTH);
            for &a in &arm_bones {
                for &b in &arm_bones {
                    let (first, second) = (world.bones[a], world.bones[b]);
                    if first.body != 0 || second.body != 1 {
                        continue;
                    }
                    let pair = closest_segment_points(first.a, first.b, second.a, second.b);
                    nearest = nearest.min(pair.distance / (first.flesh + second.flesh));
                }
            }
        }
        (nearest, world)
    }

    /// Two fighters punching at nearly the same moment drive their near arms
    /// into each other, guard on guard and then arm across arm: the arms
    /// press on each other's bones through their flesh and give way, so the
    /// bones keep most of their flesh between them and nothing tears. With
    /// only skin pushing on skin, the bones crossed, the skin crumpled, the
    /// muscle came off the bone, and a wrist tore.
    #[test]
    fn crossing_punches_keep_the_arms_whole() {
        for apart in [0, 4, 8] {
            let (nearest, world) = crossing_punches(apart);
            assert!(
                nearest > 0.5,
                "{apart} steps apart, the arms' bones came within {:.0}% of the flesh between them",
                nearest * 100.0
            );
            let stats = world.stats;
            assert_eq!(
                (
                    stats.broken_skin,
                    stats.broken_muscle,
                    stats.broken_bone_joints,
                    stats.fractured_bones
                ),
                (0, 0, 0, 0),
                "{apart} steps apart, the crossing punches hurt an arm"
            );
        }
    }

    /// The two fighters punching on the same step stay each other's mirror
    /// images, their arms meeting alike: neither is solved first.
    #[test]
    fn fighters_punching_together_stay_mirror_images() {
        let mut world = create_fighters(Materials::default());
        let dt = world.materials.fixed_dt;
        let [left, right] = crate::fighter_frames();
        let middle = (left.origin.x + right.origin.x) * 0.5;
        let half = world.points.len() / 2;
        let input = InputState {
            punches: [
                Some((0, world.inner_arm(0))),
                Some((1, world.inner_arm(1))),
                None,
                None,
            ],
            ..InputState::default()
        };
        world.step(dt, &input, SCENARIO_WIDTH);
        // Holding one punch a step, the second pressed on it was lost.
        assert!(
            world.punch(0, world.inner_arm(0)).is_some()
                && world.punch(1, world.inner_arm(1)).is_some(),
            "both fighters' punches pressed on one step throw"
        );
        for _ in 0..45 {
            world.step(dt, &InputState::default(), SCENARIO_WIDTH);
        }
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
            "punching together, the fighters stood {apart:.2e} px off mirror images"
        );
    }
}
