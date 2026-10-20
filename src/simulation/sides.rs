//! A joint holding its two bones with their flesh. The skeleton's joints
//! hold their light bones' anchors together (`soft.rs`), and the flesh, most
//! of a limb's weight, hangs on the bones by soft ties; pressed on bone and
//! flesh at once, as another fighter's arm presses them (`clash.rs`), an
//! arm's joints, held on their light bones alone, came apart: punches thrown
//! fast one after another tore the puncher's wrist and elbow, and the struck
//! guard's. Here each of a joint's two bones moves with the flesh held to
//! it, as one piece, the way Box2D's revolute joint holds two bodies.
//!
//! A point of flesh held seven tenths or more to a bone moves with it
//! wholly, and one held between three and seven tenths blends between its
//! two bones (`share_of`). Shared out by how much each point is held to each
//! bone, a point mid-forearm held seven tenths to the forearm moved only
//! seven tenths as far as the forearm's bone, and pushed again and again,
//! the bone slid out of its flesh; given wholly to the bone it is held to
//! most, the flesh at the elbow split along the line between the two bones'
//! shares, and the skin across it tore. Moved instead with everything on
//! each side of the joint as one piece, as v0.15.1's punch held its arm,
//! the joints beyond were held as if welded, and a limp arm stayed up in
//! its guard.
//!
//! Whatever moves a bone with its flesh moves these same pieces: the
//! joints' muscles and range, a punch's and a lean's muscles, and two
//! bodies' limbs pressing on each other (`clash.rs`). Each moving them by
//! its own reckoning of the flesh, they pushed and pulled the same bone
//! hundreds of pixels each way within a few frames, and it slid out of
//! its skin.
//!
//! Skin that has come away from the muscle under it, as a cut's flap does,
//! is no longer the bone's to carry (`World::loose_skin`): swung with the
//! arm as part of it, a flap a knife had cut loose was flung across the
//! window, and random knife cuts flung flesh three times as often.

use super::soft::{perpendicular, JointHold, JointPin, Softness};
use super::*;

/// How much more stiffly a joint holds its bones with their flesh than
/// Box2D lets a joint hold two bodies: at half the substep rate rather
/// than a quarter. A soft joint gives under a push by the push over its
/// pieces' mass and the square of its rate, and a hand and forearm weigh
/// little against a fist: held at a quarter, a fist landing on a punching
/// hand pushed the wrist out of place.
const SIDE_HOLD_STIFFENING: f64 = 2.0;

/// A bone and its share of the flesh held to it (`share_of`), as one piece
/// where the substep began: its mass, its centre of mass, and how hard it is
/// to turn about it.
#[derive(Clone, Debug)]
pub(super) struct BonePiece {
    pub(super) bone: usize,
    pub(super) mass: f64,
    pub(super) centre: Vec2,
    pub(super) inertia: f64,
}

/// The pieces a joint holds together: its first bone's and its second's.
#[derive(Clone, Debug)]
pub(super) struct JointSides {
    sides: [BonePiece; 2],
}

impl JointSides {
    /// How hard the two pieces are to turn against each other, or, with
    /// the piece beyond the joint `planted` on the floor, the piece before
    /// it alone.
    pub(super) fn turning_mass(&self, planted: bool) -> f64 {
        let [inner, outer] = &self.sides;
        if planted {
            inner.inertia
        } else {
            1.0 / (1.0 / inner.inertia + 1.0 / outer.inertia)
        }
    }
}

/// How much of a bone's motion a point of flesh held to it by `weight`
/// (`PostureBinding`) takes as a joint moves the bone and its flesh: all of
/// it held seven tenths or more, none held three tenths or less, and in
/// between in proportion, so that a point's shares of its two bones still
/// add up to one.
pub(super) fn share_of(weight: f64) -> f64 {
    ((weight - 0.3) / 0.4).clamp(0.0, 1.0)
}

impl World {
    /// Whether joint `index` holds its bones with their flesh this substep:
    /// a joint of an arm, from the collarbone's hinge on the spine out to
    /// the wrist, not torn or loosened by a break, slipped out of place or
    /// not, between whole bones that move. Held on its light bones alone
    /// once it had slipped, a wrist let the hand's flesh fly off and tore
    /// apart.
    fn holds_sides(&self, index: usize) -> bool {
        let joint = &self.bone_joints[index];
        matches!(joint.drive.limb, Limb::LeftArm | Limb::RightArm)
            && self.joint_holds(joint)
            && [joint.a, joint.b].iter().all(|&bone| {
                let bone = &self.bones[bone];
                !bone.pinned && !bone.splinter && !bone.fractured
            })
    }

    /// Measures, as this substep begins, each bone with its share of the
    /// flesh held to it (`share_of`), as one piece: what the joints that hold
    /// their bones with their flesh hold together (`measure_joint_sides`) and
    /// what two bodies' limbs push apart (`clash.rs`), so that both move the
    /// same flesh with each bone. Pushed on a piece of its bone and each
    /// point's whole share of it, against a joint holding it with the share
    /// `share_of` gives, the forearm was pushed one way and pulled the other
    /// 290 px over ten frames, and slid out of its skin.
    pub(super) fn measure_bone_pieces(&mut self) {
        self.side_motion = vec![(Vec2::default(), 0.0); self.bones.len()];
        self.loose_skin = self.loose_skin();
        self.side_flesh.resize_with(self.bones.len(), Vec::new);
        for flesh in &mut self.side_flesh {
            flesh.clear();
        }
        // Each bone with its share of the flesh: its mass, the moment of the
        // mass about the bone's middle, and the second moment. Measured from
        // each bone's own middle rather than the window's origin, two
        // fighters standing as mirror images sum the same small numbers, and
        // stay mirror images: summed from the origin, they parted half a
        // pixel within a punch.
        let middles: Vec<Vec2> = self
            .bones
            .iter()
            .map(|bone| lerp(bone.previous_a, bone.previous_b, 0.5))
            .collect();
        let mut buckets = vec![(0.0, Vec2::default(), 0.0); self.bones.len()];
        let add_to = |bucket: &mut (f64, Vec2, f64), mass: f64, at: Vec2| {
            bucket.0 += mass;
            bucket.1 = add(bucket.1, scale(at, mass));
            bucket.2 += mass * dot(at, at);
        };
        for (index, bone) in self.bones.iter().enumerate() {
            let middle = middles[index];
            add_to(
                &mut buckets[index],
                bone.mass * 0.5,
                subtract(bone.previous_a, middle),
            );
            add_to(
                &mut buckets[index],
                bone.mass * 0.5,
                subtract(bone.previous_b, middle),
            );
        }
        for (index, (point, hold)) in self.points.iter().zip(&self.point_holds).enumerate() {
            if point.pinned || point.is_ghost() || self.loose_skin[index] {
                continue;
            }
            for slot in 0..2 {
                let bone = hold.posture.bones[slot];
                let share = share_of(hold.posture.weights[slot]);
                if let (Some(bucket), true) = (buckets.get_mut(bone), share > 0.0) {
                    add_to(
                        bucket,
                        point.mass * share,
                        subtract(point.previous, middles[bone]),
                    );
                    self.side_flesh[bone].push((index, share));
                }
            }
        }
        self.bone_pieces = buckets
            .into_iter()
            .enumerate()
            .map(|(bone, (mass, moment, second))| {
                if mass <= EPSILON {
                    return None;
                }
                let about = scale(moment, 1.0 / mass);
                let inertia = second - mass * dot(about, about);
                let centre = add(middles[bone], about);
                (inertia > EPSILON).then_some(BonePiece {
                    bone,
                    mass,
                    centre,
                    inertia,
                })
            })
            .collect();
    }

    /// Each point of skin that has come away from the muscle under it, every
    /// fat tie that held it broken, as a cut's flap comes away: no bone
    /// carries it with its flesh.
    pub(super) fn loose_skin(&self) -> Vec<bool> {
        let mut ties = vec![(0u32, 0u32); self.points.len()];
        for attachment in &self.attachments {
            if let Some(count) = ties.get_mut(attachment.skin_point) {
                count.0 += 1;
                if attachment.broken {
                    count.1 += 1;
                }
            }
        }
        ties.iter()
            .map(|&(all, broken)| all > 0 && broken == all)
            .collect()
    }

    /// Measures, as this substep begins, the two pieces each joint that
    /// holds its bones with their flesh holds together (`holds_sides`): each
    /// of its bones with its share of the flesh held to it
    /// (`measure_bone_pieces`).
    pub(super) fn measure_joint_sides(&mut self) {
        self.joint_sides.clear();
        let piece = |bone: usize| self.bone_pieces.get(bone)?.clone();
        let sides: Vec<Option<JointSides>> = (0..self.bone_joints.len())
            .map(|index| {
                if !self.holds_sides(index) {
                    return None;
                }
                let joint = self.bone_joints[index];
                Some(JointSides {
                    sides: [piece(joint.a)?, piece(joint.b)?],
                })
            })
            .collect();
        self.joint_sides = sides;
    }

    /// Gives `piece` the impulse `push` at `lever` from its centre of mass,
    /// over a substep `h` long: its bone moves and turns at once, and its
    /// share of the flesh follows once the joints are done
    /// (`move_flesh_with_sides`).
    pub(super) fn push_piece(&mut self, piece: &BonePiece, lever: Vec2, push: Vec2, h: f64) {
        let shift = scale(push, h / piece.mass);
        let turn = cross(lever, push) * h / piece.inertia;
        self.move_piece(piece, shift, turn);
    }

    /// Turns `piece` about its centre by the angular impulse `impulse` over
    /// a substep `h` long, its flesh following as `push_piece`'s does.
    fn turn_piece(&mut self, piece: &BonePiece, impulse: f64, h: f64) {
        let turn = impulse * h / piece.inertia;
        self.move_piece(piece, Vec2::default(), turn);
    }

    /// Moves `piece`'s bone by `shift` and turns it by `turn` about the
    /// piece's centre, from where the substep began, at once, and keeps the
    /// motion for its flesh (`move_flesh_with_sides`). Each point turns about
    /// the centre rather than the window's origin, so two fighters standing
    /// as mirror images move alike to the last bit: turned about the origin,
    /// a punch's two fighters stood half a pixel off mirror images.
    fn move_piece(&mut self, piece: &BonePiece, shift: Vec2, turn: f64) {
        let motion = &mut self.side_motion[piece.bone];
        motion.0 = add(motion.0, shift);
        motion.1 += turn;
        let moved = |at: Vec2| {
            add(
                shift,
                scale(perpendicular(subtract(at, piece.centre)), turn),
            )
        };
        let bone = &mut self.bones[piece.bone];
        bone.a = add(bone.a, moved(bone.previous_a));
        bone.b = add(bone.b, moved(bone.previous_b));
    }

    /// Turns a joint's two pieces against each other by the angular impulse
    /// `impulse` over a substep `h` long, the piece beyond it one way and the
    /// piece before it the other, each about its centre, as Box2D's
    /// revolute joint turns two bodies: how a joint's muscles and its range
    /// turn its bones with their flesh. Turned on the light bones alone, the
    /// muscles spun them inside the flesh the joint holds with them, and the
    /// wrist was pulled apart.
    pub(super) fn turn_joint_sides(&mut self, sides: &JointSides, impulse: f64, h: f64) {
        self.turn_joint_sides_from(sides, impulse, false, h);
    }

    /// Turns a joint's two pieces against each other as `turn_joint_sides`
    /// does, or, with the piece beyond the joint `planted` on the floor,
    /// which takes the push back through the feet, the piece before it
    /// alone.
    pub(super) fn turn_joint_sides_from(
        &mut self,
        sides: &JointSides,
        impulse: f64,
        planted: bool,
        h: f64,
    ) {
        let [inner, outer] = &sides.sides;
        self.turn_piece(inner, -impulse, h);
        if !planted {
            self.turn_piece(outer, impulse, h);
        }
    }

    /// Gives the piece beyond a joint `push` at its anchor, `anchors.1`, and
    /// the piece before it the same back at its own anchor, `anchors.0`.
    pub(super) fn push_link_sides(
        &mut self,
        sides: &JointSides,
        anchors: (Vec2, Vec2),
        push: Vec2,
        h: f64,
    ) {
        let [inner, outer] = &sides.sides;
        self.push_piece(
            inner,
            subtract(anchors.0, inner.centre),
            scale(push, -1.0),
            h,
        );
        self.push_piece(outer, subtract(anchors.1, outer.centre), push, h);
    }

    /// Gives the piece beyond `pin`'s joint `impulse` at its anchor and the
    /// piece before it the same back at the place it holds that anchor.
    pub(super) fn push_joint_sides(
        &mut self,
        pin: &JointPin,
        sides: &JointSides,
        impulse: Vec2,
        h: f64,
    ) {
        let (inner_at, outer_at) = pin.places(&self.bones);
        let [inner, outer] = &sides.sides;
        self.push_piece(
            inner,
            subtract(inner_at, inner.centre),
            scale(impulse, -1.0),
            h,
        );
        self.push_piece(outer, subtract(outer_at, outer.centre), impulse, h);
    }

    /// Holds joint `index`'s bones' anchors no further apart than `length`,
    /// as a joint slipped out of place holds them (`solve_soft_joints`), but
    /// between its two bones with their flesh, each moved as one piece: the
    /// pull along the line between the anchors, through each piece's mass
    /// and its turn about its centre. Returns the impulse it carries on.
    pub(super) fn solve_side_link(
        &mut self,
        index: usize,
        sides: &JointSides,
        length: f64,
        loosened: f64,
        cap: f64,
        h: f64,
    ) {
        let joint = self.bone_joints[index];
        let (a, b) = (self.bones[joint.a], self.bones[joint.b]);
        let inner_at = lerp(a.previous_a, a.previous_b, joint.t_a);
        let outer_at = lerp(b.previous_a, b.previous_b, joint.t_b);
        let along = subtract(outer_at, inner_at);
        let apart = hypot(along.x, along.y);
        if apart <= EPSILON {
            self.bone_joints[index].impulse = 0.0;
            return;
        }
        let normal = scale(along, 1.0 / apart);
        let [inner, outer] = &sides.sides;
        let (r_inner, r_outer) = (
            subtract(inner_at, inner.centre),
            subtract(outer_at, outer.centre),
        );
        let (arm_inner, arm_outer) = (cross(r_inner, normal), cross(r_outer, normal));
        let inverse = 1.0 / inner.mass
            + 1.0 / outer.mass
            + arm_inner * arm_inner / inner.inertia
            + arm_outer * arm_outer / outer.inertia;
        let error = apart - length;
        let given = joint.impulse;
        if inverse <= EPSILON || (error < 0.0 && given >= 0.0) {
            self.bone_joints[index].impulse = 0.0;
            return;
        }
        let hertz = (self.materials.bone_joint_hertz * SIDE_HOLD_STIFFENING * loosened)
            .min(cap * SIDE_HOLD_STIFFENING);
        let softness = Softness::new(2.0 * PI * hertz, self.materials.bone_joint_damping_ratio, h);
        let velocity = |bone: &BoneSegment, t: f64| {
            lerp(
                scale(subtract(bone.a, bone.previous_a), 1.0 / h),
                scale(subtract(bone.b, bone.previous_b), 1.0 / h),
                t,
            )
        };
        let speed = dot(
            subtract(velocity(&b, joint.t_b), velocity(&a, joint.t_a)),
            normal,
        );
        // A slack hold only pulls its bones together.
        let total = (given + softness.impulse(1.0 / inverse, speed, error, given)).min(0.0);
        self.bone_joints[index].impulse = total;
        let push = scale(normal, total - given);
        self.push_piece(inner, r_inner, scale(push, -1.0), h);
        self.push_piece(outer, r_outer, push, h);
    }

    /// Joint `index` holds its second bone's anchor where it was built on
    /// the first, as `solve_soft_joints` holds a whole joint, but between
    /// its two bones with their flesh, each moved as one piece: the point
    /// constraint of Box2D's revolute joint (Erin Catto, MIT,
    /// `revolute_joint.c`), as soft as the joint's own hold, through each
    /// piece's mass and its turn about its centre.
    pub(super) fn solve_side_pin(
        &mut self,
        index: usize,
        hold: &JointHold,
        sides: &JointSides,
        cap: f64,
        h: f64,
    ) {
        let joint = self.bone_joints[index];
        let pin = JointPin::of(&joint, &self.bones, hold.reach());
        let (inner_at, outer_at) = pin.places(&self.bones);
        let [inner, outer] = &sides.sides;
        let (r_inner, r_outer) = (
            subtract(inner_at, inner.centre),
            subtract(outer_at, outer.centre),
        );
        let (m_inner, m_outer) = (1.0 / inner.mass, 1.0 / outer.mass);
        let (i_inner, i_outer) = (1.0 / inner.inertia, 1.0 / outer.inertia);
        let k11 =
            m_inner + m_outer + r_inner.y * r_inner.y * i_inner + r_outer.y * r_outer.y * i_outer;
        let k12 = -r_inner.y * r_inner.x * i_inner - r_outer.y * r_outer.x * i_outer;
        let k22 =
            m_inner + m_outer + r_inner.x * r_inner.x * i_inner + r_outer.x * r_outer.x * i_outer;
        let determinant = k11 * k22 - k12 * k12;
        if determinant.abs() <= EPSILON * EPSILON {
            return;
        }
        let hertz = (self.materials.bone_joint_hertz * SIDE_HOLD_STIFFENING * hold.loosened)
            .min(cap * SIDE_HOLD_STIFFENING);
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
        self.bone_joints[index].hold_impulse = add(given, impulse);
        self.push_joint_sides(&pin, sides, impulse, h);
    }

    /// Moves the flesh as the joints moved the bones this phase
    /// (`push_piece`): each point by its share of each of its bones' motion,
    /// only the flesh of the bones that moved (`side_flesh`). Every point of
    /// both bodies gone through after each phase of joints and muscles, the
    /// solver took a sixth longer a step.
    pub(super) fn move_flesh_with_sides(&mut self) {
        for bone in 0..self.side_motion.len() {
            let (shift, turn) = self.side_motion[bone];
            if shift == Vec2::default() && turn == 0.0 {
                continue;
            }
            self.side_motion[bone] = (Vec2::default(), 0.0);
            let Some(Some(piece)) = self.bone_pieces.get(bone) else {
                continue;
            };
            let centre = piece.centre;
            let points = &mut self.points;
            for &(index, share) in &self.side_flesh[bone] {
                let point = &mut points[index];
                let at = add(
                    shift,
                    scale(perpendicular(subtract(point.previous, centre)), turn),
                );
                point.position = add(point.position, scale(at, share));
            }
        }
    }
}
