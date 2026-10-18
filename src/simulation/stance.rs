//! How the body holds its stance on its skeleton: its tissue keeps its place
//! on the bones it lies over, the muscles across each joint hold the joint at
//! the angle the guard sets, as stiffly and as strongly as a person's do, and
//! the hips are held up, since the body does not keep its own balance yet.
//!
//! The joints' muscles are rebuilt from Box2D's motor joint (Erin Catto, MIT:
//! `src/motor_joint.c`): a soft spring toward a relative angle whose torque is
//! capped, warm started from the impulse it carried. Its stiffness and damping
//! are given in physical units, as Jolt's spring parts take them (Jorrit
//! Rouwe, MIT: `SpringPart.h`), so a joint is as stiff as the measured one
//! however light its bones are against the flesh they carry. Each joint's
//! stiffness, damping, and strength come from measurements of people
//! (`body.rs`, `docs/INJURY_REFERENCE.md`).

use super::soft::{JointTurn, Softness, KG_PER_MASS};
use super::*;

/// Where a point of tissue keeps its place on the skeleton: on the bones of
/// its part of the body nearest it as the body was built, at most two, each
/// counting by its weight, at the place on each where the point lay then, in
/// that bone's own frame, so the place moves and turns with the bones. Across
/// a joint the bones each side of it share the flesh there, so it bends
/// smoothly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PostureBinding {
    pub bones: [usize; 2],
    pub weights: [f64; 2],
    /// Where the point sits on each bone: `tau` of the way from its `a` end
    /// to its `b` end, and `sigma` of its length across it, a quarter turn
    /// counterclockwise, as a muscle's tie to its bone sits (`soft.rs`).
    pub tau: [f64; 2],
    pub sigma: [f64; 2],
}

impl Default for PostureBinding {
    fn default() -> Self {
        Self {
            bones: [MISSING_BONE; 2],
            weights: [0.0; 2],
            tau: [0.0; 2],
            sigma: [0.0; 2],
        }
    }
}

/// The muscles across a joint, holding it at the angle it was built at: as
/// stiffly as `stiffness`, in newton metres per radian, damped by `damping`,
/// in newton metre seconds per radian, and with at most `max_torque`, in
/// newton metres, a person's strength there; the limb they move, if any.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JointDrive {
    pub stiffness: f64,
    pub damping: f64,
    pub max_torque: f64,
    pub limb: Limb,
    /// How far past the angle it was built at the muscles aim, in radians:
    /// learned as the body settles, so that the weight they carry leaves the
    /// joint at that angle (`learn_the_guards_weight`).
    pub aim: f64,
}

/// How much of its braced damping a joint keeps in a limb hanging limp: a
/// relaxed joint is damped a few times less than a tensed one (Bennett et al.
/// 1992; Gomi and Osu 1998, 0.76 N·m·s/rad at the relaxed elbow against
/// several at a tensed one).
const LIMP_DAMPING_SHARE: f64 = 0.25;

/// A limb, on the viewer's left or right, or none for the head and trunk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Limb {
    #[default]
    None,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

impl Limb {
    /// Its place among the limbs, or `None` for the head and trunk.
    pub fn index(self) -> Option<usize> {
        match self {
            Limb::None => None,
            Limb::LeftArm => Some(0),
            Limb::RightArm => Some(1),
            Limb::LeftLeg => Some(2),
            Limb::RightLeg => Some(3),
        }
    }

    /// Whether a break of a bone of `part` in this limb leaves it hanging:
    /// with a broken collarbone, upper arm, or forearm a person cannot hold
    /// the arm up and holds it against the body (Pecci and Kreher 2008;
    /// AAOS OrthoInfo), and a broken thigh or shin bears no weight. A limb
    /// with a broken hand or foot still holds.
    fn falls_when_broken(self, part: BonePart) -> bool {
        match self {
            Limb::LeftArm | Limb::RightArm => matches!(
                part,
                BonePart::Collarbone | BonePart::UpperArm | BonePart::Forearm
            ),
            Limb::LeftLeg | Limb::RightLeg => matches!(part, BonePart::Thigh | BonePart::Shin),
            Limb::None => false,
        }
    }
}

/// One end of the pelvis, held where it stood as the body was built: the
/// hips are held up, since the body does not keep its own balance yet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HipHold {
    pub bone: usize,
    /// Which end of the bone: its `b` end, or else its `a` end.
    pub end_b: bool,
    pub home: Vec2,
    /// How far from `home` the hold aims: learned as the body settles, so
    /// that the body's weight leaves the hips at `home` rather than sunk
    /// (`learn_the_guards_weight`).
    pub aim: Vec2,
    /// The impulse it gave last substep, which warm starts the next.
    pub impulse: Vec2,
}

/// How much of a joint's turn off its guard, and of the hips' sinking, the
/// muscles take up on each step of a new body's settling: a twelfth, so the
/// body learns its weight within its second of settling, more slowly than
/// its limbs sway.
const LEARN_WEIGHT_RATE: f64 = 0.08;

/// How a point's place on the skeleton lies when the substep began, how fast
/// it moves, and how hard it is to move against the point.
struct PostureHold {
    target: Vec2,
    velocity: Vec2,
    inverse_mass: f64,
    /// For each bone: its index, its weight, its `tau` and `sigma`, and one
    /// over the mass at each of its ends.
    bones: [(usize, f64, f64, f64, f64); 2],
}

impl PostureHold {
    /// How a point of inverse mass `point_inverse_mass` bound to the skeleton
    /// by `binding` is held this substep.
    fn of(
        point_inverse_mass: f64,
        binding: PostureBinding,
        bones: &[BoneSegment],
        h: f64,
    ) -> Option<Self> {
        let mut target = Vec2::default();
        let mut velocity = Vec2::default();
        let mut inverse_mass = point_inverse_mass;
        let mut held = [(MISSING_BONE, 0.0, 0.0, 0.0, 0.0); 2];
        let mut total = 0.0;
        for slot in 0..2 {
            let index = binding.bones[slot];
            let Some(bone) = bones.get(index) else {
                continue;
            };
            let weight = binding.weights[slot];
            let (tau, sigma) = (binding.tau[slot], binding.sigma[slot]);
            let anchor = |a: Vec2, b: Vec2| {
                add(lerp(a, b, tau), scale(perpendicular(subtract(b, a)), sigma))
            };
            target = add(
                target,
                scale(anchor(bone.previous_a, bone.previous_b), weight),
            );
            let moving = anchor(
                scale(subtract(bone.a, bone.previous_a), 1.0 / h),
                scale(subtract(bone.b, bone.previous_b), 1.0 / h),
            );
            velocity = add(velocity, scale(moving, weight));
            // A broken piece asleep on the floor stays put.
            let end = if bone.sleeping && free_bone_fragment(*bone) {
                0.0
            } else {
                bone.inverse_end_mass()
            };
            inverse_mass += weight
                * weight
                * end
                * ((1.0 - tau) * (1.0 - tau) + tau * tau + 2.0 * sigma * sigma);
            held[slot] = (index, weight, tau, sigma, end);
            total += weight;
        }
        (total > 0.5 && inverse_mass > EPSILON).then_some(Self {
            target,
            velocity,
            inverse_mass,
            bones: held,
        })
    }
}

/// A quarter turn counterclockwise.
fn perpendicular(value: Vec2) -> Vec2 {
    Vec2 {
        x: -value.y,
        y: value.x,
    }
}

/// Where `p` sits on a bone running from `a` to `b`: how far along it, as a
/// share of its length, and how far across it, a quarter turn
/// counterclockwise, as a share of its length.
pub(super) fn bone_frame(a: Vec2, b: Vec2, p: Vec2) -> (f64, f64) {
    let along = subtract(b, a);
    let length_sq = dot(along, along).max(EPSILON);
    let offset = subtract(p, a);
    (
        dot(offset, along) / length_sq,
        cross(along, offset) / length_sq,
    )
}

impl World {
    /// One newton metre in the engine's units of torque (`KG_PER_MASS` and
    /// pixels).
    pub(super) fn newton_metres(&self) -> f64 {
        let m_per_px = self.mm_per_px() * 0.001;
        1.0 / (KG_PER_MASS * m_per_px * m_per_px)
    }

    /// One newton in the engine's units of force.
    pub(super) fn newtons(&self) -> f64 {
        1.0 / (KG_PER_MASS * self.mm_per_px() * 0.001)
    }

    /// How far joint `joint` is turned off the angle it was built at, in
    /// radians, or `None` for one that no longer joins two bones.
    pub fn joint_turn(&self, joint: usize) -> Option<f64> {
        let joint = self.bone_joints.get(joint)?;
        let (a, b) = (self.bones.get(joint.a)?, self.bones.get(joint.b)?);
        Some(wrap_angle(
            bone_angle(*b) - bone_angle(*a) - joint.rest_angle,
        ))
    }

    /// Where point `point` keeps its place on the skeleton now
    /// (`PostureBinding`), or `None` for one bound to no bone.
    pub fn posture_target(&self, point: usize) -> Option<Vec2> {
        let hold = self.point_holds.get(point)?;
        let mut target = Vec2::default();
        let mut total = 0.0;
        for slot in 0..2 {
            let binding = hold.posture;
            let Some(bone) = self.bones.get(binding.bones[slot]) else {
                continue;
            };
            let anchor = add(
                lerp(bone.a, bone.b, binding.tau[slot]),
                scale(perpendicular(subtract(bone.b, bone.a)), binding.sigma[slot]),
            );
            target = add(target, scale(anchor, binding.weights[slot]));
            total += binding.weights[slot];
        }
        (total > 0.5).then_some(target)
    }

    /// The torque the muscles across joint `joint` gave on the last substep,
    /// in newton metres.
    pub fn joint_torque(&self, joint: usize) -> f64 {
        let substep = self.materials.fixed_dt / self.materials.substeps.max(1) as f64;
        self.bone_joints.get(joint).map_or(0.0, |joint| {
            joint.drive_impulse / substep / self.newton_metres()
        })
    }

    /// Binds every point of tissue to the bones `candidates` names for it,
    /// the two nearest each counting by how near (see `PostureBinding`), as
    /// the body lies when built.
    pub(super) fn bind_posture(&mut self, candidates: impl Fn(&Point) -> Vec<usize>) {
        let spacing = self.materials.point_spacing.max(1.0);
        for index in 0..self.points.len() {
            let point = self.points[index];
            let mut nearest = [(f64::INFINITY, MISSING_BONE); 2];
            for bone in candidates(&point) {
                let Some(segment) = self.bones.get(bone) else {
                    continue;
                };
                let gap = distance_to_segment(point.home, segment.home_a, segment.home_b);
                if gap < nearest[0].0 {
                    nearest[1] = nearest[0];
                    nearest[0] = (gap, bone);
                } else if gap < nearest[1].0 {
                    nearest[1] = (gap, bone);
                }
            }
            let mut binding = PostureBinding::default();
            let mut total = 0.0;
            for (slot, &(gap, bone)) in nearest.iter().enumerate() {
                if bone == MISSING_BONE {
                    continue;
                }
                let segment = self.bones[bone];
                let (tau, sigma) = bone_frame(segment.home_a, segment.home_b, point.home);
                binding.bones[slot] = bone;
                binding.weights[slot] = 1.0 / (gap * gap + spacing * spacing);
                binding.tau[slot] = tau;
                binding.sigma[slot] = sigma;
                total += binding.weights[slot];
            }
            if total > 0.0 {
                for weight in &mut binding.weights {
                    *weight /= total;
                }
            }
            self.point_holds[index].posture = binding;
        }
    }

    /// The body holds its stance: every point of tissue is drawn back toward
    /// its place on the skeleton, softly, as the tissue holds its shape round
    /// the bones, and the bones take the pull back, so the flesh's weight
    /// hangs on the skeleton; less as the body loses blood.
    pub(super) fn solve_posture(&mut self, h: f64) {
        let zeta = self.materials.posture_damping_ratio;
        let turgor = self.blood_turgor_scale_internal().max(0.0).sqrt();
        for index in 0..self.points.len() {
            let point = self.points[index];
            // A ghost is not flesh of its own and has no place to keep.
            if point.pinned || point.is_ghost() {
                continue;
            }
            let hertz = match point.layer {
                TissueLayer::Skin => self.materials.skin_posture_hertz,
                TissueLayer::Muscle => self.materials.muscle_posture_hertz,
            } * turgor;
            if hertz <= 0.0 {
                continue;
            }
            let point_hold = self.point_holds[index];
            let Some(hold) = PostureHold::of(
                self.inverse_masses[index],
                point_hold.posture,
                &self.bones,
                h,
            ) else {
                continue;
            };
            let omega = 2.0 * PI * hertz;
            // As stiff as the point is heavy, whatever the bones weigh.
            let stiffness = point.mass * KG_PER_MASS * omega * omega;
            let mass = 1.0 / hold.inverse_mass;
            let softness = Softness::of_spring(stiffness, mass, zeta, h);
            let error = subtract(point.previous, hold.target);
            let speed = subtract(
                scale(subtract(point.position, point.previous), 1.0 / h),
                hold.velocity,
            );
            let given = point_hold.posture_impulse;
            let impulse = Vec2 {
                x: softness.impulse(mass, speed.x, error.x, given.x),
                y: softness.impulse(mass, speed.y, error.y, given.y),
            };
            self.point_holds[index].posture_impulse = add(given, impulse);
            self.apply_posture(index, &hold, impulse, h);
        }
    }

    /// Gives point `index` the impulse `impulse` toward its place, and its
    /// bones the same back, each by its weight.
    fn apply_posture(&mut self, index: usize, hold: &PostureHold, impulse: Vec2, h: f64) {
        let w = self.inverse_masses[index];
        let point = &mut self.points[index];
        point.position = add(point.position, scale(impulse, w * h));
        for &(bone, weight, tau, sigma, end) in &hold.bones {
            if bone == MISSING_BONE || end <= 0.0 {
                continue;
            }
            let share = scale(impulse, weight);
            let turned = perpendicular(share);
            let change_a = scale(add(scale(share, 1.0 - tau), scale(turned, sigma)), -end);
            let change_b = scale(subtract(scale(share, tau), scale(turned, sigma)), -end);
            let segment = &mut self.bones[bone];
            segment.a = add(segment.a, scale(change_a, h));
            segment.b = add(segment.b, scale(change_b, h));
        }
    }

    /// Gives every point the impulse that held it in its place last substep.
    pub(super) fn warm_start_posture(&mut self, h: f64) {
        for index in 0..self.points.len() {
            let point = self.points[index];
            let point_hold = self.point_holds[index];
            if point.pinned || point_hold.posture_impulse == Vec2::default() {
                continue;
            }
            let Some(hold) = PostureHold::of(
                self.inverse_masses[index],
                point_hold.posture,
                &self.bones,
                h,
            ) else {
                continue;
            };
            self.apply_posture(index, &hold, point_hold.posture_impulse, h);
        }
    }

    /// The hips are held where they stood, as firmly as legs hold up a body
    /// (`Materials::hip_hold_hertz`): the body does not keep its own balance
    /// yet, so it cannot fall.
    pub(super) fn solve_hip_hold(&mut self, h: f64) {
        let (stiffness, damping) = self.hip_hold_spring();
        for index in 0..self.hip_holds.len() {
            let hold = self.hip_holds[index];
            let Some(&bone) = self.bones.get(hold.bone) else {
                continue;
            };
            let w = bone.inverse_end_mass();
            if w <= 0.0 {
                continue;
            }
            let mass = 1.0 / w;
            let (place, previous) = if hold.end_b {
                (bone.b, bone.previous_b)
            } else {
                (bone.a, bone.previous_a)
            };
            let softness = Softness::of_stiffness(stiffness, damping, mass, h);
            let error = subtract(previous, add(hold.home, hold.aim));
            let speed = scale(subtract(place, previous), 1.0 / h);
            let impulse = Vec2 {
                x: softness.impulse(mass, speed.x, error.x, hold.impulse.x),
                y: softness.impulse(mass, speed.y, error.y, hold.impulse.y),
            };
            self.hip_holds[index].impulse = add(hold.impulse, impulse);
            self.push_hip(hold, scale(impulse, w * h));
        }
    }

    /// Moves the end of the pelvis `hold` holds by `shift`.
    fn push_hip(&mut self, hold: HipHold, shift: Vec2) {
        let bone = &mut self.bones[hold.bone];
        if hold.end_b {
            bone.b = add(bone.b, shift);
        } else {
            bone.a = add(bone.a, shift);
        }
    }

    /// Lets go of the hips: nothing holds the body up any more, as when it
    /// is thrown or falls.
    pub fn release_hips(&mut self) {
        self.hip_holds.clear();
    }

    pub(super) fn warm_start_hip_hold(&mut self, h: f64) {
        for index in 0..self.hip_holds.len() {
            let hold = self.hip_holds[index];
            let w = self
                .bones
                .get(hold.bone)
                .map_or(0.0, BoneSegment::inverse_end_mass);
            if w > 0.0 && hold.impulse != Vec2::default() {
                self.push_hip(hold, scale(hold.impulse, w * h));
            }
        }
    }

    /// The muscles learn the guard's weight as a new body settles: on each
    /// step every held joint aims a share further past the angle it was
    /// built at, the way its weight turns it off, and the hold on the hips a
    /// share higher as the body sinks on it, until the weight leaves the
    /// body in the guard as built. A person sets the muscles holding a limb
    /// for the load it carries, so that it rests where they mean it to, and
    /// the muscles' stiffness resists only a change from there (Feldman 1986,
    /// the equilibrium-point model). Each joint's hold learns it as well: a
    /// soft joint gives a few pixels under the weight it carries, where a
    /// real one gives under a millimetre, so it holds its second bone that
    /// much past where it was built, and the bone rests there.
    ///
    /// The hips' hold learns only how far the pelvis sinks as a whole, both
    /// ends alike, never how it tilts: the trunk stands on the pelvis like
    /// an upturned pendulum, and a hold that also learned to tilt the pelvis
    /// back under it rocked it further each time, until at full gravity the
    /// body fell over within six seconds of settling.
    pub(super) fn learn_the_guards_weight(&mut self) {
        for index in 0..self.bone_joints.len() {
            let joint = self.bone_joints[index];
            if !self.joint_holds(&joint) {
                continue;
            }
            let (a, b) = (self.bones[joint.a], self.bones[joint.b]);
            let built = |bone: BoneSegment| BoneSegment {
                a: bone.home_a,
                b: bone.home_b,
                ..bone
            };
            let off = joint_hold_offset(&a, joint.t_a, bone_point(b, joint.t_b), 0.0);
            let meant = joint_hold_offset(
                &built(a),
                joint.t_a,
                bone_point(built(b), joint.t_b),
                joint.rest,
            );
            self.bone_joints[index].hold =
                subtract(joint.hold, scale(subtract(off, meant), LEARN_WEIGHT_RATE));
            if joint.drive.stiffness <= 0.0 {
                continue;
            }
            if let Some(turn) = self.joint_turn(index) {
                self.bone_joints[index].drive.aim -= LEARN_WEIGHT_RATE * turn;
            }
        }
        let sunk: Vec<Vec2> = self
            .hip_holds
            .iter()
            .filter_map(|hold| {
                let bone = self.bones.get(hold.bone)?;
                let at = if hold.end_b { bone.b } else { bone.a };
                Some(subtract(at, hold.home))
            })
            .collect();
        if sunk.is_empty() {
            return;
        }
        let sunk = scale(
            sunk.iter().fold(Vec2::default(), |sum, &end| add(sum, end)),
            1.0 / sunk.len() as f64,
        );
        for hold in &mut self.hip_holds {
            hold.aim = subtract(hold.aim, scale(sunk, LEARN_WEIGHT_RATE));
        }
    }

    /// The hold's stiffness and damping at each end of the pelvis, in the
    /// engine's units: the whole body springs back on it at
    /// `hip_hold_hertz`, damped by `hip_hold_damping_ratio`.
    fn hip_hold_spring(&self) -> (f64, f64) {
        let body = tissue::BODY_MASS_KG / KG_PER_MASS;
        let omega = 2.0 * PI * self.materials.hip_hold_hertz;
        let ends = self.hip_holds.len().max(1) as f64;
        (
            body * omega * omega / ends,
            2.0 * self.materials.hip_hold_damping_ratio * body * omega / ends,
        )
    }

    /// The muscles across each whole joint hold it at the angle the guard
    /// sets (`JointDrive`), with at most their strength; a joint whose bones
    /// are broken or that has come out of place holds nothing, and a limb it
    /// held drops.
    pub(super) fn solve_joint_drives(&mut self, h: f64) {
        let newton_metres = self.newton_metres();
        // Box2D caps a joint's stiffness at a quarter of the substep rate on
        // its bones' own mass (`joint.c`), past which it rings.
        let cap = 2.0 * PI * 0.25 / h;
        for index in 0..self.bone_joints.len() {
            let joint = self.bone_joints[index];
            let limit = self.drive_limit(&joint) * h;
            if limit <= 0.0 {
                self.bone_joints[index].drive_impulse = 0.0;
                continue;
            }
            let turn = JointTurn::of(&joint, &self.bones);
            if turn.inverse_mass <= 0.0 {
                continue;
            }
            let mass = 1.0 / turn.inverse_mass;
            // A limb hanging from a break holds no pose, but its relaxed
            // muscles still damp how it swings.
            let (stiffness, damping) = if self.limp(joint.drive.limb) {
                (0.0, joint.drive.damping * LIMP_DAMPING_SHARE)
            } else {
                (joint.drive.stiffness, joint.drive.damping)
            };
            let stiffness = (stiffness * newton_metres).min(mass * cap * cap);
            let damping = damping * newton_metres;
            let softness = Softness::of_stiffness(stiffness, damping, mass, h);
            let angle = wrap_angle(turn.angle - joint.rest_angle - joint.drive.aim);
            let speed = turn.speed(&self.bones, h);
            let given = joint.drive_impulse.clamp(-limit, limit);
            let total = (given + softness.impulse(mass, speed, angle, given)).clamp(-limit, limit);
            self.bone_joints[index].drive_impulse = total;
            self.apply_joint_turn(&joint, &turn, total - given, h);
        }
    }

    /// The most torque the muscles across `joint` give, in the engine's
    /// units: none across a broken joint or bone, less the further the joint
    /// has come out of place. A limb hanging from a break only damps.
    fn drive_limit(&self, joint: &BoneJoint) -> f64 {
        if !self.joint_holds(joint) || joint.drive.stiffness <= 0.0 {
            return 0.0;
        }
        joint.drive.max_torque * (1.0 - joint.subluxation).max(0.0) * self.newton_metres()
    }

    /// Whether `limb` hangs from a broken bone, its muscles letting go.
    pub fn limp(&self, limb: Limb) -> bool {
        limb.index().is_some_and(|index| self.limp_limbs[index])
    }

    /// A bone of `part` in `limb` broke: if that leaves the limb hanging,
    /// its muscles let go.
    pub(super) fn let_go_of_broken_limb(&mut self, limb: Limb, part: BonePart) {
        if limb.falls_when_broken(part) {
            self.let_limb_go(limb);
        }
    }

    /// The muscles of `limb` let go, as a broken bone makes them, and it
    /// hangs, only damped: as a person's arm hangs once they are knocked out.
    pub fn let_limb_go(&mut self, limb: Limb) {
        let Some(index) = limb.index() else {
            return;
        };
        if !self.limp_limbs[index] {
            self.note(
                EventKind::LimbWentLimp,
                Subject::Limb(limb),
                Vec2::default(),
                None,
                Measure::None,
                0.0,
                0.0,
            );
        }
        self.limp_limbs[index] = true;
        // The pull the muscles carried goes with them.
        for joint in &mut self.bone_joints {
            if joint.drive.limb == limb {
                joint.drive_impulse = 0.0;
            }
        }
    }

    pub(super) fn warm_start_joint_drives(&mut self, h: f64) {
        for index in 0..self.bone_joints.len() {
            let joint = self.bone_joints[index];
            let limit = self.drive_limit(&joint) * h;
            let given = joint.drive_impulse.clamp(-limit, limit);
            self.bone_joints[index].drive_impulse = given;
            if given != 0.0 {
                let turn = JointTurn::of(&joint, &self.bones);
                self.apply_joint_turn(&joint, &turn, given, h);
            }
        }
    }

    /// Moves the skeleton's places for tissue and the hip hold off bone
    /// `bone`, broken at `break_t` of the way along it, onto its two pieces:
    /// the first keeps its index and the stretch before the break, `second`
    /// the rest.
    pub(super) fn split_stance_at_break(&mut self, bone: usize, second: usize, break_t: f64) {
        let first_share = break_t.max(0.05);
        let second_share = (1.0 - break_t).max(0.05);
        for hold in &mut self.point_holds {
            let binding = &mut hold.posture;
            for slot in 0..2 {
                if binding.bones[slot] != bone {
                    continue;
                }
                if binding.tau[slot] < break_t {
                    binding.tau[slot] /= first_share;
                    binding.sigma[slot] /= first_share;
                } else {
                    binding.bones[slot] = second;
                    binding.tau[slot] = (binding.tau[slot] - break_t) / second_share;
                    binding.sigma[slot] /= second_share;
                }
            }
        }
        for hold in &mut self.hip_holds {
            if hold.bone == bone && hold.end_b {
                hold.bone = second;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// However long a new body goes on learning its weight, it stands in the
    /// guard as built: when the hips' hold learned to tilt the pelvis too,
    /// the body rocked further each time and fell over within six seconds
    /// at full gravity.
    #[test]
    fn learning_the_weight_longer_keeps_the_guard() {
        let mut world = create_layered_body(1280.0, 720.0, Materials::default());
        let dt = world.materials.fixed_dt;
        for _ in 0..270 {
            world.step(dt, &InputState::default(), 1280.0);
            world.learn_the_guards_weight();
        }
        let worst = world
            .bones
            .iter()
            .map(|bone| distance(bone.a, bone.home_a).max(distance(bone.b, bone.home_b)))
            .fold(0.0, f64::max);
        assert!(
            worst < 2.0,
            "a bone stands {worst:.1} px from where it was built"
        );
    }
}
