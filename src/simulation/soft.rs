//! The solver that moves the body's tissue and bones, rebuilt from Box2D v3's
//! soft step (Erin Catto, MIT: `b2MakeSoft` in `solver.h`, the stages of
//! `solver.c`, the spring of `distance_joint.c`). Each step splits into
//! substeps. Each substep integrates velocities, warm starts every soft
//! constraint with the impulse it carried last, and solves each against the
//! error it had when the substep began, in passes between which the body's
//! contacts and bones' lengths are still projected by position. Box2D then
//! relaxes its constraints once more to take out the speed their position
//! correction added; here the projections would cancel that speed again
//! every step, and leave it hidden in a body at rest, so there is no relax.
//!
//! A soft constraint is a spring and damper solved implicitly (Catto, "Soft
//! Constraints", GDC 2011): given its stiffness as a frequency and its damping
//! ratio, `Softness` says how much of its error it closes each substep, how
//! much of its effective mass it pushes with, and how much the impulse it has
//! already given softens the next. Tissue (`tissue.rs`) sets each spring's
//! stiffness from what the real tissue measures, so the body is as stiff with
//! any number of substeps, unlike the share of the error each pass closed
//! before.
//!
//! The tissue and bones keep their positions as now and a step ago, as the
//! rest of the engine reads them. While the substeps run, `previous` is where
//! a point was when the substep began, and a point's velocity is how far it
//! moves in a substep over the substep's length, so a velocity change moves
//! `position`.

use super::*;

/// The engine's unit of mass, in kilograms. The tools' inertia was set in it
/// and a muscle point of the old mesh weighed 1.25 of it; tissue weighs what
/// it does in a body (`tissue.rs`), in the same unit.
pub(super) const KG_PER_MASS: f64 = 0.1;

/// Share of its speed past the flesh it hangs from a cut's ghost keeps each
/// substep.
const GHOST_SPEED_KEPT: f64 = 0.5;

/// How a soft constraint closes its error over a substep of length `h`, from
/// its angular frequency `omega` and damping ratio `zeta` (`b2MakeSoft`).
#[derive(Clone, Copy, Debug)]
pub(super) struct Softness {
    bias_rate: f64,
    mass_scale: f64,
    impulse_scale: f64,
}

impl Softness {
    pub(super) fn new(omega: f64, zeta: f64, h: f64) -> Self {
        let a1 = 2.0 * zeta + h * omega;
        let a2 = h * omega * a1;
        let a3 = 1.0 / (1.0 + a2);
        Self {
            bias_rate: omega / a1.max(EPSILON),
            mass_scale: a2 * a3,
            impulse_scale: a3,
        }
    }

    /// The softness of a spring of stiffness `stiffness` (in newtons per
    /// metre) on a constraint of effective mass `mass` (in the engine's unit).
    pub(super) fn of_spring(stiffness: f64, mass: f64, zeta: f64, h: f64) -> Self {
        Self::new(
            (stiffness / (mass * KG_PER_MASS).max(1.0e-12)).sqrt(),
            zeta,
            h,
        )
    }

    /// The softness of a spring of stiffness `stiffness` and damping
    /// `damping`, in the engine's units, on a constraint of effective mass
    /// `mass`: the same implicit spring as [`Softness::new`], set by how stiff
    /// and how damped the spring is rather than by how fast it rings on this
    /// mass, as Jolt's `SpringPart` takes it (Jorrit Rouwe, MIT), so it holds
    /// as the spring it stands for whatever the mass it acts on directly.
    pub(super) fn of_stiffness(stiffness: f64, damping: f64, mass: f64, h: f64) -> Self {
        let spring = damping + h * stiffness;
        if spring <= EPSILON {
            return Self {
                bias_rate: 0.0,
                mass_scale: 0.0,
                impulse_scale: 1.0,
            };
        }
        // Jolt's softness, 1 / (h (c + h k)), times the mass.
        let softness = mass / (h * spring);
        Self {
            bias_rate: stiffness / spring,
            mass_scale: 1.0 / (1.0 + softness),
            impulse_scale: softness / (1.0 + softness),
        }
    }

    /// The impulse to add to a constraint with effective mass `mass`, moving
    /// at `speed` along it with error `error`, that has given `given` already.
    pub(super) fn impulse(&self, mass: f64, speed: f64, error: f64, given: f64) -> f64 {
        -self.mass_scale * mass * (speed + self.bias_rate * error) - self.impulse_scale * given
    }
}

/// Where a point was when the substep began, which a constraint is judged
/// from.
fn place(point: &Point) -> Vec2 {
    point.previous
}

fn velocity(point: &Point, h: f64) -> Vec2 {
    scale(subtract(point.position, point.previous), 1.0 / h)
}

/// Changes a point's velocity by `change`, over a substep `h` long.
fn speed_up(point: &mut Point, change: Vec2, h: f64) {
    point.position = add(point.position, scale(change, h));
}

fn bone_place(bone: &BoneSegment) -> (Vec2, Vec2) {
    (bone.previous_a, bone.previous_b)
}

fn speed_up_bone(bone: &mut BoneSegment, change_a: Vec2, change_b: Vec2, h: f64) {
    bone.a = add(bone.a, scale(change_a, h));
    bone.b = add(bone.b, scale(change_b, h));
}

/// A quarter turn counterclockwise.
fn perpendicular(value: Vec2) -> Vec2 {
    Vec2 {
        x: -value.y,
        y: value.x,
    }
}

impl Point {
    /// One over the point's mass; nothing moves a pinned point.
    pub(super) fn inverse_mass(&self) -> f64 {
        if self.pinned {
            0.0
        } else {
            1.0 / self.mass.max(EPSILON)
        }
    }
}

impl World {
    /// Works out each point's inverse mass for the substep about to be
    /// solved. Nothing a substep does changes what a point weighs or whether
    /// it is pinned: cuts and tears split points only once the step's motion
    /// is solved (`cuts.rs`), and a debug build checks it after each
    /// substep. So the soft constraints take each end's inverse mass from
    /// here, rather than dividing for it at every end on every pass.
    fn refresh_inverse_masses(&mut self) {
        self.inverse_masses.clear();
        self.inverse_masses
            .extend(self.points.iter().map(Point::inverse_mass));
    }
}

impl BoneSegment {
    /// One over the mass at each end of the bone, which carries half of it;
    /// nothing moves a pinned bone.
    pub(super) fn inverse_end_mass(&self) -> f64 {
        if self.pinned {
            0.0
        } else {
            2.0 / self.mass.max(EPSILON)
        }
    }
}

impl World {
    /// Solves this step's motion: the body's soft constraints in substeps, and
    /// its contacts and joints in passes within each. The tissue and bones come
    /// in where the step will carry them, as the step's prediction and the
    /// tool's shoves left them; each moves from where it began the step at
    /// that velocity instead, and ends with `previous` a step behind again.
    pub(super) fn solve_motion(&mut self, dt: f64, width: f64, floor_y: f64) {
        let substeps = self.materials.substeps.max(1);
        let h = dt / substeps as f64;
        self.split_step_into_substeps(substeps);
        for substep in 0..substeps {
            self.solver_dt = h;
            self.solver_progress = (substep + 1) as f64 / substeps as f64;
            self.mark_trace_time(substep as f64 / substeps as f64);
            // Points, the outline's hold on them, the cuts' ghosts, then the
            // bones: each a phase of its own, so a trace names which moved
            // what.
            self.timed("integrate", |world| {
                world.integrate_points(h, substeps, floor_y)
            });
            self.timed("outline hold", |world| world.pull_outline_round(h));
            self.timed("ghosts", Self::carry_ghosts_with_their_flesh);
            self.timed("integrate", |world| world.integrate_bones(h, substeps));
            // What tears this substep tears before the rest are warm started:
            // a tie that let go gives none of the pull it carried, which
            // nothing would take back.
            self.timed("bone attachments", Self::judge_bone_attachments);
            self.timed("attachments", Self::judge_attachments);
            self.timed("bone joints", Self::judge_bone_joints);
            self.refresh_inverse_masses();
            if self.materials.warm_starting {
                self.warm_start_soft(h);
            } else {
                self.forget_soft_impulses();
            }
            for pass in 0..self.materials.contact_passes.max(1) {
                // The tissue's soft constraints, then how the body holds its
                // stance (`stance.rs`) and its joints, each kind a phase of its
                // own. A joint's muscles come before its limit and its hold on
                // the bones, as Box2D orders a joint's rows. The tissue's soft
                // hold on the skeleton, a spring of a hertz or two, is taken
                // on the substep's first pass only: it moves a point by a hair
                // a pass, and the warm start carries what it gave from one
                // substep to the next.
                self.timed("fibers", |world| world.solve_soft_springs(h));
                self.timed("fat", |world| world.solve_soft_attachments(h));
                self.timed("muscle ties", |world| world.solve_soft_bone_ties(h));
                self.timed("areas", |world| world.solve_soft_areas(h));
                if pass == 0 {
                    self.timed("posture", |world| world.solve_posture(h));
                }
                self.timed("hip hold", |world| world.solve_hip_hold(h));
                self.timed("joint muscles", |world| world.solve_joint_drives(h));
                self.timed("joints", |world| world.solve_soft_joints(h));
                self.timed("tool contact", Self::solve_tool_contact);
                self.timed("bones", Self::solve_bones);
                self.timed("fragment-bone", Self::solve_bone_fragment_bone_contacts);
                self.timed("fragment-tissue", Self::solve_bone_fragment_tissue_contacts);
                self.timed("fragment pairs", Self::solve_bone_fragment_repulsion);
                self.timed("outline contact", Self::solve_outline_contacts);
                self.timed("world bounds", |world| {
                    world.constrain_to_world(width, floor_y)
                });
            }
            debug_assert!(
                self.points
                    .iter()
                    .zip(&self.inverse_masses)
                    .all(|(point, &weighed)| point.inverse_mass().to_bits() == weighed.to_bits()),
                "a point's weight changed during a substep"
            );
        }
        self.join_substeps(substeps);
        self.solver_dt = dt;
        self.solver_progress = 1.0;
        self.mark_trace_time(1.0);
    }

    /// Puts each point and bone back where it began the step, moving at the
    /// velocity the step gave it: `previous` one substep behind.
    fn split_step_into_substeps(&mut self, substeps: usize) {
        let share = 1.0 / substeps as f64;
        for point in &mut self.points {
            if point.pinned {
                point.position = point.home;
                point.previous = point.home;
                continue;
            }
            let travel = subtract(point.position, point.previous);
            point.position = point.previous;
            point.previous = subtract(point.position, scale(travel, share));
        }
        for bone in &mut self.bones {
            if bone.pinned || (bone.sleeping && free_bone_fragment(*bone)) {
                bone.previous_a = bone.a;
                bone.previous_b = bone.b;
                continue;
            }
            let (travel_a, travel_b) = (
                subtract(bone.a, bone.previous_a),
                subtract(bone.b, bone.previous_b),
            );
            bone.a = bone.previous_a;
            bone.b = bone.previous_b;
            bone.previous_a = subtract(bone.a, scale(travel_a, share));
            bone.previous_b = subtract(bone.b, scale(travel_b, share));
        }
    }

    /// Sets `previous` back to a whole step behind, at the velocity the last
    /// substep ended with.
    fn join_substeps(&mut self, substeps: usize) {
        let steps = substeps as f64;
        for point in &mut self.points {
            let travel = subtract(point.position, point.previous);
            point.previous = subtract(point.position, scale(travel, steps));
        }
        for bone in &mut self.bones {
            let (travel_a, travel_b) = (
                subtract(bone.a, bone.previous_a),
                subtract(bone.b, bone.previous_b),
            );
            bone.previous_a = subtract(bone.a, scale(travel_a, steps));
            bone.previous_b = subtract(bone.b, scale(travel_b, steps));
        }
    }

    /// How fast things fall here, in pixels per second squared:
    /// `Materials::gravity` at the body's scale.
    pub fn gravity_px(&self) -> f64 {
        self.materials.gravity * 1000.0 / self.mm_per_px()
    }

    /// Moves every point through one substep at its velocity, under gravity
    /// and a little drag, onto the floor at the most.
    fn integrate_points(&mut self, h: f64, substeps: usize, floor_y: f64) {
        let keep = 1.0 / (1.0 + h * self.materials.linear_drag);
        let fall = self.gravity_px() * h * h;
        let floor_keep = libm::pow(self.materials.floor_friction, 1.0 / substeps as f64);
        for point in &mut self.points {
            if point.pinned {
                point.position = point.home;
                point.previous = point.home;
                continue;
            }
            let travel = scale(subtract(point.position, point.previous), keep);
            point.previous = point.position;
            point.position = add(point.position, travel);
            point.position.y += fall;
            if point.position.y > floor_y {
                point.position.y = floor_y;
                point.previous.x =
                    point.position.x + (point.previous.x - point.position.x) * floor_keep;
            }
        }
    }

    /// Moves every bone through one substep at its velocity, under gravity
    /// and a little drag, and spins loose pieces of bone.
    fn integrate_bones(&mut self, h: f64, substeps: usize) {
        let fall = self.gravity_px() * h * h;
        let bone_keep = libm::pow(self.materials.bone_damping, 1.0 / substeps as f64);
        let spin_keep = libm::pow(self.materials.bone_angular_damping, 1.0 / substeps as f64);
        for index in 0..self.bones.len() {
            let mut bone = self.bones[index];
            if bone.pinned {
                bone.a = bone.home_a;
                bone.b = bone.home_b;
                bone.previous_a = bone.a;
                bone.previous_b = bone.b;
                self.bones[index] = bone;
                continue;
            }
            let free_fragment = bone.fractured || bone.splinter;
            if bone.sleeping && free_bone_fragment(bone) {
                bone.angular_velocity = 0.0;
                bone.previous_a = bone.a;
                bone.previous_b = bone.b;
                self.bones[index] = bone;
                continue;
            }
            let travel_a = scale(subtract(bone.a, bone.previous_a), bone_keep);
            let travel_b = scale(subtract(bone.b, bone.previous_b), bone_keep);
            bone.previous_a = bone.a;
            bone.previous_b = bone.b;
            bone.a = add(bone.a, travel_a);
            bone.b = add(bone.b, travel_b);
            bone.a.y += fall;
            bone.b.y += fall;
            bone.angular_velocity *= spin_keep;
            if !free_fragment && bone.angular_velocity.abs() < 0.01 {
                bone.angular_velocity = 0.0;
            }
            if free_fragment {
                self.debug.max_bone_angular_speed = self
                    .debug
                    .max_bone_angular_speed
                    .max(bone.angular_velocity.abs());
                let turn = bone.angular_velocity * h;
                rotate_bone_around_center(&mut bone, turn);
            }
            self.bones[index] = bone;
        }
    }

    /// A cut's ghost carries its piece's shape across the cut but holds no
    /// flesh (`cuts.rs`), so nothing but its piece's springs moves it: a
    /// piece hinged on a single point of flesh could spin there freely. Each
    /// ghost keeps half of how fast it moves past the flesh its springs tie
    /// it to, each substep.
    fn carry_ghosts_with_their_flesh(&mut self) {
        let mut flesh_velocity: Vec<(Vec2, f64)> = Vec::new();
        for spring in &self.springs {
            if spring.broken {
                continue;
            }
            let (a, b) = (&self.points[spring.a], &self.points[spring.b]);
            for (ghost, flesh, index) in [(a, b, spring.a), (b, a, spring.b)] {
                if !ghost.is_ghost() || flesh.is_ghost() {
                    continue;
                }
                if flesh_velocity.is_empty() {
                    flesh_velocity = vec![(Vec2::default(), 0.0); self.points.len()];
                }
                let travel = subtract(flesh.position, flesh.previous);
                let entry = &mut flesh_velocity[index];
                entry.0 = add(entry.0, travel);
                entry.1 += 1.0;
            }
        }
        for (index, &(sum, count)) in flesh_velocity.iter().enumerate() {
            if count <= 0.0 {
                continue;
            }
            let point = &mut self.points[index];
            let travel = subtract(point.position, point.previous);
            let flesh = scale(sum, 1.0 / count);
            let kept = add(flesh, scale(subtract(travel, flesh), GHOST_SPEED_KEPT));
            point.position = add(point.previous, kept);
        }
    }

    /// Gives every soft constraint the impulse it ended the last substep with,
    /// as Box2D's warm start does, so a held load is carried from the start:
    /// each kind a phase of its own, so a trace names which carried what.
    fn warm_start_soft(&mut self, h: f64) {
        self.timed("warm start: fibers", |world| world.warm_start_fibers(h));
        self.timed("warm start: areas", |world| world.warm_start_areas(h));
        self.timed("warm start: fat", |world| world.warm_start_fat(h));
        self.timed("warm start: muscle ties", |world| {
            world.warm_start_muscle_ties(h)
        });
        self.timed("warm start: posture", |world| world.warm_start_posture(h));
        self.timed("warm start: hip hold", |world| world.warm_start_hip_hold(h));
        self.timed("warm start: joint muscles", |world| {
            world.warm_start_joint_drives(h)
        });
        self.timed("warm start: joints", |world| world.warm_start_joints(h));
    }

    fn warm_start_fibers(&mut self, h: f64) {
        for index in 0..self.springs.len() {
            let spring = self.springs[index];
            if spring.broken || spring.impulse == 0.0 {
                continue;
            }
            let (a, b) = (self.points[spring.a], self.points[spring.b]);
            let along = subtract(b.previous, a.previous);
            let length = hypot(along.x, along.y);
            if length < EPSILON {
                continue;
            }
            let push = scale(along, spring.impulse / length);
            let (wa, wb) = (self.inverse_masses[spring.a], self.inverse_masses[spring.b]);
            speed_up(&mut self.points[spring.a], scale(push, -wa), h);
            speed_up(&mut self.points[spring.b], scale(push, wb), h);
        }
    }

    fn warm_start_areas(&mut self, h: f64) {
        for index in 0..self.areas.len() {
            let area = self.areas[index];
            if area.impulse == 0.0 || !self.area_alive(&area) {
                continue;
            }
            let gradients = area_gradients(&self.points, &area);
            for (corner, gradient) in [area.a, area.b, area.c].into_iter().zip(gradients) {
                let w = self.inverse_masses[corner];
                speed_up(
                    &mut self.points[corner],
                    scale(gradient, w * area.impulse),
                    h,
                );
            }
        }
    }

    fn warm_start_fat(&mut self, h: f64) {
        for index in 0..self.attachments.len() {
            let attachment = self.attachments[index];
            if attachment.broken || attachment.impulse == 0.0 {
                continue;
            }
            let (skin, muscle) = (
                self.points[attachment.skin_point],
                self.points[attachment.muscle_point],
            );
            let along = subtract(muscle.previous, skin.previous);
            let length = hypot(along.x, along.y);
            if length < EPSILON {
                continue;
            }
            let push = scale(along, attachment.impulse / length);
            let (ws, wm) = (
                self.inverse_masses[attachment.skin_point],
                self.inverse_masses[attachment.muscle_point],
            );
            speed_up(&mut self.points[attachment.skin_point], scale(push, -ws), h);
            speed_up(
                &mut self.points[attachment.muscle_point],
                scale(push, wm),
                h,
            );
        }
    }

    fn warm_start_muscle_ties(&mut self, h: f64) {
        for index in 0..self.bone_attachments.len() {
            let attachment = self.bone_attachments[index];
            if attachment.broken
                || attachment.point >= self.points.len()
                || attachment.bone >= self.bones.len()
            {
                continue;
            }
            self.apply_bone_tie(index, attachment.impulse, h);
        }
    }

    fn warm_start_joints(&mut self, h: f64) {
        for index in 0..self.bone_joints.len() {
            let joint = self.bone_joints[index];
            let Some(hold) = self.joint_hold(&joint) else {
                continue;
            };
            if joint.impulse != 0.0 {
                let link = JointLink::of(&joint, &self.bones);
                self.apply_joint_link(&joint, &link, joint.impulse, h);
            }
            if joint.hold_impulse != Vec2::default() {
                let pin = JointPin::of(&joint, &self.bones, hold.reach());
                self.apply_joint_pin(&joint, &pin, joint.hold_impulse, h);
            }
            if joint.angular_impulse != 0.0 {
                let turn = JointTurn::of(&joint, &self.bones);
                self.apply_joint_turn(&joint, &turn, joint.angular_impulse, h);
            }
        }
    }

    /// Starts every soft constraint afresh, with no impulse carried over.
    fn forget_soft_impulses(&mut self) {
        for spring in &mut self.springs {
            spring.impulse = 0.0;
        }
        for area in &mut self.areas {
            area.impulse = 0.0;
        }
        for attachment in &mut self.attachments {
            attachment.impulse = 0.0;
        }
        for attachment in &mut self.bone_attachments {
            attachment.impulse = Vec2::default();
        }
        for hold in &mut self.point_holds {
            hold.posture_impulse = Vec2::default();
        }
        for hold in &mut self.hip_holds {
            hold.impulse = Vec2::default();
        }
        for joint in &mut self.bone_joints {
            joint.impulse = 0.0;
            joint.hold_impulse = Vec2::default();
            joint.angular_impulse = 0.0;
            joint.drive_impulse = 0.0;
        }
    }

    /// Starts every soft constraint on a point `marked` afresh, with no
    /// impulse carried over.
    pub(super) fn forget_soft_impulses_at(&mut self, marked: &[bool]) {
        let touches = |point: usize| marked.get(point).copied().unwrap_or(false);
        for spring in &mut self.springs {
            if touches(spring.a) || touches(spring.b) {
                spring.impulse = 0.0;
            }
        }
        for area in &mut self.areas {
            if touches(area.a) || touches(area.b) || touches(area.c) {
                area.impulse = 0.0;
            }
        }
        for attachment in &mut self.attachments {
            if touches(attachment.skin_point) || touches(attachment.muscle_point) {
                attachment.impulse = 0.0;
            }
        }
        for attachment in &mut self.bone_attachments {
            if touches(attachment.point) {
                attachment.impulse = Vec2::default();
            }
        }
        for (index, hold) in self.point_holds.iter_mut().enumerate() {
            if touches(index) {
                hold.posture_impulse = Vec2::default();
            }
        }
    }

    /// How far whole joint `joint` has let its second bone's anchor move
    /// from where it was built on the first bone, in pixels: what the settle
    /// learns to hold at nothing under the body's weight, and a blow can
    /// move. `None` for a joint loosened or torn, which holds only a gap.
    /// (Against where the joint holds the anchor, `BoneJoint::hold`, a joint
    /// at rest would read the give the settle learned out.)
    pub fn joint_slip(&self, joint: usize) -> Option<f64> {
        let joint = self.bone_joints.get(joint)?;
        let hold = self.joint_hold(joint)?;
        if hold.pulls_only {
            return None;
        }
        let (a, b) = (&self.bones[joint.a], &self.bones[joint.b]);
        let built = |bone: &BoneSegment| BoneSegment {
            a: bone.home_a,
            b: bone.home_b,
            ..*bone
        };
        let meant = joint_hold_offset(
            &built(a),
            joint.t_a,
            bone_point(built(b), joint.t_b),
            joint.rest,
        );
        let length = a.rest_length.max(EPSILON);
        let tau = joint.t_a + meant.x * hold.reach() / length;
        let sigma = meant.y * hold.reach() / length;
        let held = add(
            lerp(a.a, a.b, tau),
            scale(perpendicular(subtract(a.b, a.a)), sigma),
        );
        Some(distance(lerp(b.a, b.b, joint.t_b), held))
    }

    /// A whole joint holds its bones together and within their range, as
    /// Box2D's joints do: stiff, but soft enough to stay steady, at most a
    /// quarter of the substep rate, and heavily damped. It holds its second
    /// bone's anchor where it was built on the first bone, as Box2D's
    /// revolute joint holds its two anchors together, so the second bone
    /// turns about it and cannot swing round the first's anchor. A joint
    /// slipped out of place holds only the gap between them, more loosely,
    /// and only against pulling further apart; one a break or an injury has
    /// loosened holds like a slack rope, only once its bones pull past its
    /// slack, and lets them bend further.
    fn solve_soft_joints(&mut self, h: f64) {
        let cap = 0.25 / h;
        let zeta = self.materials.bone_joint_damping_ratio;
        for index in 0..self.bone_joints.len() {
            let joint = self.bone_joints[index];
            let Some(hold) = self.joint_hold(&joint) else {
                continue;
            };
            if !hold.pulls_only {
                self.bone_joints[index].impulse = 0.0;
                let pin = JointPin::of(&joint, &self.bones, hold.reach());
                if pin.inverse_mass > EPSILON {
                    let hertz = (self.materials.bone_joint_hertz * hold.loosened).min(cap);
                    let softness = Softness::new(2.0 * PI * hertz, zeta, h);
                    let mass = 1.0 / pin.inverse_mass;
                    let speed = pin.speed(&self.bones, h);
                    let given = joint.hold_impulse;
                    let impulse = Vec2 {
                        x: softness.impulse(mass, speed.x, pin.error.x, given.x),
                        y: softness.impulse(mass, speed.y, pin.error.y, given.y),
                    };
                    self.bone_joints[index].hold_impulse = add(given, impulse);
                    self.apply_joint_pin(&joint, &pin, impulse, h);
                } else {
                    self.bone_joints[index].hold_impulse = Vec2::default();
                }
                self.solve_joint_limit(index, &hold, cap, h);
                continue;
            }
            self.bone_joints[index].hold_impulse = Vec2::default();
            let link = JointLink::of(&joint, &self.bones);
            let error = link.length - hold.length;
            if hold.loose {
                let stretch = link.length / hold.rest.max(1.0);
                self.debug.max_post_fracture_joint_stretch =
                    self.debug.max_post_fracture_joint_stretch.max(stretch);
            }
            let slack = hold.pulls_only && error < 0.0;
            if link.inverse_mass > EPSILON && !slack {
                let hertz = (self.materials.bone_joint_hertz * hold.loosened).min(cap);
                let softness = Softness::new(2.0 * PI * hertz, zeta, h);
                let speed = link.speed(&self.bones, h);
                let given = joint.impulse;
                let mut total =
                    given + softness.impulse(1.0 / link.inverse_mass, speed, error, given);
                // A slack hold only pulls its bones together.
                if hold.pulls_only {
                    total = total.min(0.0);
                }
                self.bone_joints[index].impulse = total;
                self.apply_joint_link(&joint, &link, total - given, h);
                if hold.loose {
                    self.debug.post_fracture_joint_corrections += 1;
                }
            } else {
                self.bone_joints[index].impulse = 0.0;
            }
            self.solve_joint_limit(index, &hold, cap, h);
        }
    }

    /// Joint `index`'s hold on its bones' turn, `hold` as it holds them now:
    /// it pushes them back once they turn past its range.
    fn solve_joint_limit(&mut self, index: usize, hold: &JointHold, cap: f64, h: f64) {
        let joint = self.bone_joints[index];
        let turn = JointTurn::of(&joint, &self.bones);
        let angle = wrap_angle(turn.angle - hold.rest_angle);
        if hold.loose {
            self.debug.max_post_fracture_joint_angle =
                self.debug.max_post_fracture_joint_angle.max(angle.abs());
        }
        let past = angle - angle.clamp(hold.min_angle, hold.max_angle);
        let given = joint.angular_impulse;
        // A turn's inverse mass is small in pixels, for long, heavy bones
        // such as the thigh and spine, but only nothing moves at zero.
        if past == 0.0 || turn.inverse_mass <= 0.0 {
            self.bone_joints[index].angular_impulse = 0.0;
            return;
        }
        let zeta = self.materials.bone_joint_damping_ratio;
        let hertz = (self.materials.bone_joint_angle_hertz * hold.loosened).min(cap);
        let softness = Softness::new(2.0 * PI * hertz, zeta, h);
        let speed = turn.speed(&self.bones, h);
        let mut impulse = softness.impulse(1.0 / turn.inverse_mass, speed, past, given);
        // A limit only pushes back toward the range.
        let total = if past > 0.0 {
            (given + impulse).min(0.0)
        } else {
            (given + impulse).max(0.0)
        };
        impulse = total - given;
        self.bone_joints[index].angular_impulse = total;
        self.apply_joint_turn(&joint, &turn, impulse, h);
        if hold.loose {
            // A broken piece that turns against what holds it stops
            // spinning on its own.
            for bone in [joint.a, joint.b] {
                self.bones[bone].angular_velocity *= 1.0 - LOOSE_JOINT_SPIN_LOSS;
            }
        }
    }

    /// How joint `joint` holds its bones now, or `None` if it holds nothing:
    /// whole, or loose, as a break or an injury left it.
    fn joint_hold(&self, joint: &BoneJoint) -> Option<JointHold> {
        let (a, b) = (self.bones.get(joint.a)?, self.bones.get(joint.b)?);
        let m = &self.materials;
        if !joint.broken && !joint.post_fracture_limited {
            let slack = m.bone_joint_subluxation_angular * joint.subluxation * 0.55;
            return Some(JointHold {
                rest: joint.rest,
                length: joint.rest + m.bone_joint_subluxation_slack * joint.subluxation,
                rest_angle: joint.rest_angle,
                min_angle: joint.min_angle - slack,
                max_angle: joint.max_angle + slack,
                loosened: if joint.subluxated {
                    m.bone_joint_subluxation_stiffness_scale.sqrt()
                } else {
                    1.0
                },
                pulls_only: joint.subluxated,
                loose: false,
            });
        }
        if a.splinter || b.splinter {
            return None;
        }
        let remapped = joint.post_fracture_rest > 0.0;
        let rest = if remapped {
            joint.post_fracture_rest
        } else {
            joint.rest
        }
        .max(1.0);
        let (slack, angle_slack) = if joint.fracture_link {
            (m.fracture_link_slack, m.fracture_link_angle_slack)
        } else {
            (
                m.post_fracture_joint_slack,
                m.post_fracture_joint_angle_slack,
            )
        };
        Some(JointHold {
            rest,
            length: if joint.fracture_link {
                rest + slack
            } else {
                (rest + slack).max(rest * m.post_fracture_joint_max_stretch)
            },
            rest_angle: if remapped {
                joint.post_fracture_rest_angle
            } else {
                joint.rest_angle
            },
            min_angle: joint.min_angle - angle_slack,
            max_angle: joint.max_angle + angle_slack,
            loosened: 1.0,
            pulls_only: true,
            loose: true,
        })
    }

    /// A joint whose bones the soft solver holds whole: between two bones,
    /// and not loosened by a break or torn apart.
    pub(super) fn joint_holds(&self, joint: &BoneJoint) -> bool {
        !joint.broken
            && !joint.post_fracture_limited
            && joint.a < self.bones.len()
            && joint.b < self.bones.len()
    }

    /// Gives joint `joint`'s bones `impulse` along `link`, pulling their
    /// anchors together.
    fn apply_joint_link(&mut self, joint: &BoneJoint, link: &JointLink, impulse: f64, h: f64) {
        let push = scale(link.normal, impulse);
        for (bone, t, sign) in [(joint.a, joint.t_a, -1.0), (joint.b, joint.t_b, 1.0)] {
            let w = self.bones[bone].inverse_end_mass() * sign;
            speed_up_bone(
                &mut self.bones[bone],
                scale(push, w * (1.0 - t)),
                scale(push, w * t),
                h,
            );
        }
    }

    /// Gives joint `joint`'s second bone `impulse` at its anchor, and its
    /// first bone the same back at the place `pin` holds that anchor.
    fn apply_joint_pin(&mut self, joint: &BoneJoint, pin: &JointPin, impulse: Vec2, h: f64) {
        let w = self.bones[joint.b].inverse_end_mass();
        speed_up_bone(
            &mut self.bones[joint.b],
            scale(impulse, w * (1.0 - pin.t_b)),
            scale(impulse, w * pin.t_b),
            h,
        );
        // The place is (1 - tau) a + tau b + sigma times b - a turned a
        // quarter turn, so the first bone's ends take the impulse back
        // through that, as a muscle tie's bone does.
        let w = self.bones[joint.a].inverse_end_mass();
        let turned = perpendicular(impulse);
        let change_a = scale(
            add(scale(impulse, 1.0 - pin.tau), scale(turned, pin.sigma)),
            -w,
        );
        let change_b = scale(
            subtract(scale(impulse, pin.tau), scale(turned, pin.sigma)),
            -w,
        );
        speed_up_bone(&mut self.bones[joint.a], change_a, change_b, h);
    }

    /// Turns joint `joint`'s bones by angular impulse `impulse`, the second
    /// bone one way and the first the other.
    pub(super) fn apply_joint_turn(
        &mut self,
        joint: &BoneJoint,
        turn: &JointTurn,
        impulse: f64,
        h: f64,
    ) {
        for (bone, gradient, sign) in [
            (joint.a, turn.gradient_a, -1.0),
            (joint.b, turn.gradient_b, 1.0),
        ] {
            let w = self.bones[bone].inverse_end_mass() * sign * impulse;
            speed_up_bone(
                &mut self.bones[bone],
                scale(gradient, -w),
                scale(gradient, w),
                h,
            );
        }
    }

    /// Each fiber pulls and pushes its ends by its stiffness at its stretch
    /// (`tissue.rs`), damped along its length.
    fn solve_soft_springs(&mut self, h: f64) {
        for index in 0..self.springs.len() {
            let spring = self.springs[index];
            if spring.broken {
                continue;
            }
            let (a, b) = (self.points[spring.a], self.points[spring.b]);
            let (wa, wb) = (self.inverse_masses[spring.a], self.inverse_masses[spring.b]);
            let w = wa + wb;
            if w <= EPSILON {
                continue;
            }
            let along = subtract(place(&b), place(&a));
            let length = hypot(along.x, along.y);
            if length < EPSILON {
                continue;
            }
            let normal = scale(along, 1.0 / length);
            let (stiffness, rest) = self.spring_stiffness_at(&spring, length);
            if stiffness <= 0.0 {
                continue;
            }
            let mass = 1.0 / w;
            let softness = Softness::of_spring(stiffness, mass, self.spring_damping(&spring), h);
            let speed = dot(normal, subtract(velocity(&b, h), velocity(&a, h)));
            let impulse = softness.impulse(mass, speed, length - rest, spring.impulse);
            self.springs[index].impulse += impulse;
            let push = scale(normal, impulse);
            speed_up(&mut self.points[spring.a], scale(push, -wa), h);
            speed_up(&mut self.points[spring.b], scale(push, wb), h);
        }
    }

    /// The fat under the skin holds each skin point to the muscle beneath it,
    /// softly.
    fn solve_soft_attachments(&mut self, h: f64) {
        let zeta = self.materials.fat_damping_ratio;
        for index in 0..self.attachments.len() {
            let attachment = self.attachments[index];
            if attachment.broken || attachment.stiffness <= 0.0 {
                continue;
            }
            let (skin, muscle) = (
                self.points[attachment.skin_point],
                self.points[attachment.muscle_point],
            );
            let (ws, wm) = (
                self.inverse_masses[attachment.skin_point],
                self.inverse_masses[attachment.muscle_point],
            );
            let w = ws + wm;
            if w <= EPSILON {
                continue;
            }
            let along = subtract(place(&muscle), place(&skin));
            let length = hypot(along.x, along.y);
            if length < EPSILON {
                continue;
            }
            let normal = scale(along, 1.0 / length);
            let mass = 1.0 / w;
            let shear = self.fat_shear(&attachment, length);
            let stiffness = attachment.stiffness * self.fat_stiffening(shear);
            let softness = Softness::of_spring(stiffness, mass, zeta, h);
            let speed = dot(normal, subtract(velocity(&muscle, h), velocity(&skin, h)));
            let impulse =
                softness.impulse(mass, speed, length - attachment.rest, attachment.impulse);
            self.attachments[index].impulse += impulse;
            let push = scale(normal, impulse);
            speed_up(&mut self.points[attachment.skin_point], scale(push, -ws), h);
            speed_up(
                &mut self.points[attachment.muscle_point],
                scale(push, wm),
                h,
            );
        }
    }

    /// Muscle is tied to the bone in it where it lay when the body was built,
    /// in the bone's own frame, so it turns with the bone; flesh and bone pull
    /// each other by their masses, and a blow to the flesh moves the bone in
    /// it. A piece of broken bone the flesh holds only follows the flesh, at
    /// the tie's own frequency whatever it weighs, and the flesh does not
    /// feel it.
    fn solve_soft_bone_ties(&mut self, h: f64) {
        let zeta = self.materials.bone_tie_damping_ratio;
        for index in 0..self.bone_attachments.len() {
            let attachment = self.bone_attachments[index];
            if attachment.broken
                || attachment.point >= self.points.len()
                || attachment.bone >= self.bones.len()
            {
                continue;
            }
            let point = self.points[attachment.point];
            let bone = self.bones[attachment.bone];
            let tie = BoneTie::of(&attachment, self.inverse_masses[attachment.point], &bone);
            if tie.inverse_mass <= EPSILON {
                continue;
            }
            let hertz = match bone.part {
                BonePart::Foot => self.materials.foot_tie_hertz,
                _ => self.materials.bone_tie_hertz,
            };
            let omega = 2.0 * PI * hertz;
            let (a, b) = bone_place(&bone);
            let error = subtract(place(&point), tie.target(a, b));
            let bone_velocity = tie.anchor(
                scale(subtract(bone.a, bone.previous_a), 1.0 / h),
                scale(subtract(bone.b, bone.previous_b), 1.0 / h),
            );
            let speed = subtract(velocity(&point, h), bone_velocity);
            let mass = 1.0 / tie.inverse_mass;
            let softness = if attachment.holds_piece {
                Softness::new(omega, zeta, h)
            } else {
                // The tie is as stiff as the flesh it holds is heavy, whatever
                // the bone weighs.
                let stiffness = point.mass * KG_PER_MASS * omega * omega;
                Softness::of_spring(stiffness, mass, zeta, h)
            };
            let impulse = Vec2 {
                x: softness.impulse(mass, speed.x, error.x, attachment.impulse.x),
                y: softness.impulse(mass, speed.y, error.y, attachment.impulse.y),
            };
            self.bone_attachments[index].impulse = add(attachment.impulse, impulse);
            self.apply_bone_tie(index, impulse, h);
        }
    }

    /// Gives the flesh and bone of tie `index` the impulse `impulse`, the
    /// flesh one way and the bone the other.
    fn apply_bone_tie(&mut self, index: usize, impulse: Vec2, h: f64) {
        let attachment = self.bone_attachments[index];
        let bone = self.bones[attachment.bone];
        let tie = BoneTie::of(&attachment, self.inverse_masses[attachment.point], &bone);
        speed_up(
            &mut self.points[attachment.point],
            scale(impulse, tie.point_inverse_mass),
            h,
        );
        if tie.end_inverse_mass <= 0.0 {
            return;
        }
        // The anchor is (1 - tau) a + tau b + sigma times b - a turned a
        // quarter turn, so the bone's ends take the impulse back through that.
        let turned = perpendicular(impulse);
        let change_a = scale(
            add(scale(impulse, 1.0 - tie.tau), scale(turned, tie.sigma)),
            -tie.end_inverse_mass,
        );
        let change_b = scale(
            subtract(scale(impulse, tie.tau), scale(turned, tie.sigma)),
            -tie.end_inverse_mass,
        );
        speed_up_bone(&mut self.bones[attachment.bone], change_a, change_b, h);
    }

    /// Each triangle of tissue resists growing or shrinking, as tissue that
    /// keeps its volume bulges rather than squashes.
    fn solve_soft_areas(&mut self, h: f64) {
        let turgor = self.blood_turgor_scale_internal();
        for index in 0..self.areas.len() {
            let area = self.areas[index];
            if area.stiffness <= 0.0 || !self.area_alive(&area) {
                continue;
            }
            let corners = [area.a, area.b, area.c];
            let gradients = area_gradients(&self.points, &area);
            let mut weight = 0.0;
            let mut speed = 0.0;
            for (corner, gradient) in corners.into_iter().zip(gradients) {
                let point = &self.points[corner];
                weight += self.inverse_masses[corner] * dot(gradient, gradient);
                speed += dot(gradient, velocity(point, h));
            }
            if weight <= EPSILON {
                continue;
            }
            let (pa, pb, pc) = (
                place(&self.points[area.a]),
                place(&self.points[area.b]),
                place(&self.points[area.c]),
            );
            let error = signed_area(pa, pb, pc) - area.rest_area;
            let mass = 1.0 / weight;
            // Stiffness per unit area change, from the sheet's modulus over
            // the triangle's rest area (`tissue.rs`).
            let stiffness = area.stiffness * turgor / area.rest_area.abs().max(1.0);
            let softness = Softness::of_spring(stiffness, mass, self.area_damping(&area), h);
            let impulse = softness.impulse(mass, speed, error, area.impulse);
            self.areas[index].impulse += impulse;
            for (corner, gradient) in corners.into_iter().zip(gradients) {
                let w = self.inverse_masses[corner];
                speed_up(&mut self.points[corner], scale(gradient, w * impulse), h);
            }
        }
    }

    /// How far the flesh of `attachment` sits off its place on its bone, in
    /// pixels.
    pub fn bone_tie_slip(&self, attachment: &BoneAttachment) -> f64 {
        let point = &self.points[attachment.point];
        let bone = &self.bones[attachment.bone];
        let tie = BoneTie::of(attachment, point.inverse_mass(), bone);
        distance(point.position, tie.target(bone.a, bone.b))
    }

    /// A triangle's area constraint counts while two of its sides hold.
    pub(super) fn area_alive(&self, area: &AreaConstraint) -> bool {
        self.live_edge_count(area.edge_ab, area.edge_bc, area.edge_ca) >= 2
    }
}

/// A held piece of bone at least this long, in pixels, is held in its own
/// frame (`BoneTie`): about one and a half point spacings, longer than the
/// splinters and wedges a break chips off.
const HELD_IN_FRAME_LENGTH: f64 = 17.0;

/// How a muscle point's tie to its bone reaches the bone: the anchor lies at
/// `(1 - tau) a + tau b + sigma perp(b - a)`, so it turns and slides with the
/// bone, and the masses each end and the flesh weigh in. A piece of broken
/// bone is held instead at its own nearest point to the flesh, the flesh as
/// far off it as when the flesh took it, the way the old position-based ties
/// held it: anchored in its frame, a chip a few pixels long held by flesh
/// many times as far away would be swung round on those long levers.
struct BoneTie {
    tau: f64,
    sigma: f64,
    /// Where the flesh lies off the anchor, for a held piece.
    offset: Vec2,
    point_inverse_mass: f64,
    end_inverse_mass: f64,
    inverse_mass: f64,
}

impl BoneTie {
    /// The tie `attachment` from flesh of inverse mass `point_inverse_mass`
    /// to `bone`.
    fn of(attachment: &BoneAttachment, point_inverse_mass: f64, bone: &BoneSegment) -> Self {
        let length = bone.rest_length.max(EPSILON);
        // A chip is held at its nearest point, the flesh at its world offset;
        // a longer piece is held in its own frame, so it turns with the flesh
        // as a limb hanging from the break swings round.
        let (tau, sigma, offset) = if attachment.holds_piece && length < HELD_IN_FRAME_LENGTH {
            (attachment.t.clamp(0.0, 1.0), 0.0, attachment.offset)
        } else {
            (
                attachment.t + attachment.along / length,
                attachment.across / length,
                Vec2::default(),
            )
        };
        // A piece the flesh holds follows it and does not drag it.
        let point_inverse_mass = if attachment.holds_piece {
            0.0
        } else {
            point_inverse_mass
        };
        let end_inverse_mass = bone.inverse_end_mass();
        let inverse_mass = point_inverse_mass
            + end_inverse_mass * ((1.0 - tau) * (1.0 - tau) + tau * tau + 2.0 * sigma * sigma);
        Self {
            tau,
            sigma,
            offset,
            point_inverse_mass,
            end_inverse_mass,
            inverse_mass,
        }
    }

    /// The anchor on a bone with ends `a` and `b`, or how fast it moves for
    /// ends moving at `a` and `b`.
    fn anchor(&self, a: Vec2, b: Vec2) -> Vec2 {
        add(
            lerp(a, b, self.tau),
            scale(perpendicular(subtract(b, a)), self.sigma),
        )
    }

    /// Where the tie holds the flesh on a bone with ends `a` and `b`.
    fn target(&self, a: Vec2, b: Vec2) -> Vec2 {
        add(self.anchor(a, b), self.offset)
    }
}

/// How much of its spin a broken piece turning against a loose joint's limit
/// loses each pass.
const LOOSE_JOINT_SPIN_LOSS: f64 = 0.08;

/// How a joint holds its bones this substep (`World::joint_hold`): how far
/// apart it keeps their anchors, from the length it rests at and the most it
/// lets them part, and the angle it lets them turn through.
struct JointHold {
    rest: f64,
    length: f64,
    rest_angle: f64,
    min_angle: f64,
    max_angle: f64,
    /// How much of its stiffness it keeps, as a joint slipped out of place
    /// holds more loosely.
    loosened: f64,
    /// It only pulls its bones together, never pushes them apart.
    pulls_only: bool,
    /// A break or an injury has loosened it.
    loose: bool,
}

impl JointHold {
    /// How far it holds its bones apart against how far they rest apart: a
    /// joint slipping out of place holds them further.
    fn reach(&self) -> f64 {
        self.length / self.rest.max(EPSILON)
    }
}

/// Where a whole joint holds its second bone's anchor (`BoneJoint::hold`)
/// when the substep began: how far the anchor is from there, and how hard
/// the two are to move together. The place is `(1 - tau) a + tau b + sigma
/// perp(b - a)` on the first bone, with ends `a` and `b`, so it turns and
/// slides with that bone as a muscle tie's anchor does (`BoneTie`).
struct JointPin {
    error: Vec2,
    a: usize,
    b: usize,
    tau: f64,
    sigma: f64,
    t_b: f64,
    inverse_mass: f64,
}

impl JointPin {
    /// The hold reaching `reach` times as far from the first bone's anchor
    /// as it was built (`JointHold::reach`).
    fn of(joint: &BoneJoint, bones: &[BoneSegment], reach: f64) -> Self {
        let (a, b) = (&bones[joint.a], &bones[joint.b]);
        let length = a.rest_length.max(EPSILON);
        let tau = joint.t_a + joint.hold.x * reach / length;
        let sigma = joint.hold.y * reach / length;
        let (a0, a1) = bone_place(a);
        let (b0, b1) = bone_place(b);
        let held = add(
            lerp(a0, a1, tau),
            scale(perpendicular(subtract(a1, a0)), sigma),
        );
        let share = |t: f64| (1.0 - t) * (1.0 - t) + t * t;
        Self {
            error: subtract(lerp(b0, b1, joint.t_b), held),
            a: joint.a,
            b: joint.b,
            tau,
            sigma,
            t_b: joint.t_b,
            inverse_mass: a.inverse_end_mass() * (share(tau) + 2.0 * sigma * sigma)
                + b.inverse_end_mass() * share(joint.t_b),
        }
    }

    /// How fast the second bone's anchor moves off the place it is held.
    fn speed(&self, bones: &[BoneSegment], h: f64) -> Vec2 {
        let (a, b) = (&bones[self.a], &bones[self.b]);
        let (a0, a1) = (subtract(a.a, a.previous_a), subtract(a.b, a.previous_b));
        let held = add(
            lerp(a0, a1, self.tau),
            scale(perpendicular(subtract(a1, a0)), self.sigma),
        );
        let anchor = lerp(
            subtract(b.a, b.previous_a),
            subtract(b.b, b.previous_b),
            self.t_b,
        );
        scale(subtract(anchor, held), 1.0 / h)
    }
}

/// How a joint's anchors, `t_a` along its first bone and `t_b` along its
/// second, lie when the substep began: how far apart, along which line, and how hard
/// they are to move apart.
struct JointLink {
    length: f64,
    normal: Vec2,
    a: usize,
    b: usize,
    t_a: f64,
    t_b: f64,
    inverse_mass: f64,
}

impl JointLink {
    fn of(joint: &BoneJoint, bones: &[BoneSegment]) -> Self {
        let (a, b) = (&bones[joint.a], &bones[joint.b]);
        let (a0, a1) = bone_place(a);
        let (b0, b1) = bone_place(b);
        let along = subtract(lerp(b0, b1, joint.t_b), lerp(a0, a1, joint.t_a));
        let length = hypot(along.x, along.y);
        let normal = if length > EPSILON {
            scale(along, 1.0 / length)
        } else {
            Vec2 { x: 1.0, y: 0.0 }
        };
        let reach = |t: f64| (1.0 - t) * (1.0 - t) + t * t;
        Self {
            length,
            normal,
            a: joint.a,
            b: joint.b,
            t_a: joint.t_a,
            t_b: joint.t_b,
            inverse_mass: a.inverse_end_mass() * reach(joint.t_a)
                + b.inverse_end_mass() * reach(joint.t_b),
        }
    }

    /// How fast the anchors move apart.
    fn speed(&self, bones: &[BoneSegment], h: f64) -> f64 {
        let anchor_velocity = |bone: &BoneSegment, t: f64| {
            scale(
                lerp(
                    subtract(bone.a, bone.previous_a),
                    subtract(bone.b, bone.previous_b),
                    t,
                ),
                1.0 / h,
            )
        };
        dot(
            self.normal,
            subtract(
                anchor_velocity(&bones[self.b], self.t_b),
                anchor_velocity(&bones[self.a], self.t_a),
            ),
        )
    }
}

/// How a joint's second bone is turned against its first when the substep
/// began, and how each bone's ends move it.
pub(super) struct JointTurn {
    pub(super) angle: f64,
    a: usize,
    b: usize,
    gradient_a: Vec2,
    gradient_b: Vec2,
    pub(super) inverse_mass: f64,
}

impl JointTurn {
    pub(super) fn of(joint: &BoneJoint, bones: &[BoneSegment]) -> Self {
        let (a, b) = (&bones[joint.a], &bones[joint.b]);
        let (a0, a1) = bone_place(a);
        let (b0, b1) = bone_place(b);
        let (da, db) = (subtract(a1, a0), subtract(b1, b0));
        let (la, lb) = (dot(da, da).max(EPSILON), dot(db, db).max(EPSILON));
        // A bone's angle turns by its far end moving across it over its
        // length squared, and its near end the other way.
        let gradient_a = scale(perpendicular(da), 1.0 / la);
        let gradient_b = scale(perpendicular(db), 1.0 / lb);
        Self {
            angle: libm::atan2(db.y, db.x) - libm::atan2(da.y, da.x),
            a: joint.a,
            b: joint.b,
            gradient_a,
            gradient_b,
            inverse_mass: 2.0 * a.inverse_end_mass() * dot(gradient_a, gradient_a)
                + 2.0 * b.inverse_end_mass() * dot(gradient_b, gradient_b),
        }
    }

    /// How fast the second bone turns against the first.
    pub(super) fn speed(&self, bones: &[BoneSegment], h: f64) -> f64 {
        let spread = |bone: &BoneSegment| {
            scale(
                subtract(
                    subtract(bone.b, bone.previous_b),
                    subtract(bone.a, bone.previous_a),
                ),
                1.0 / h,
            )
        };
        dot(self.gradient_b, spread(&bones[self.b])) - dot(self.gradient_a, spread(&bones[self.a]))
    }
}

/// The gradient of a triangle's signed area with respect to each corner,
/// where the corners were when the substep began.
fn area_gradients(points: &[Point], area: &AreaConstraint) -> [Vec2; 3] {
    let (a, b, c) = (
        place(&points[area.a]),
        place(&points[area.b]),
        place(&points[area.c]),
    );
    [
        Vec2 {
            x: (b.y - c.y) * 0.5,
            y: (c.x - b.x) * 0.5,
        },
        Vec2 {
            x: (c.y - a.y) * 0.5,
            y: (a.x - c.x) * 0.5,
        },
        Vec2 {
            x: (a.y - b.y) * 0.5,
            y: (b.x - a.x) * 0.5,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ball on a spring hanging from a pinned point, released stretched.
    fn spring_world(stiffness: f64, zeta: f64, substeps: usize) -> World {
        let mut materials = Materials {
            substeps,
            gravity: 0.0,
            linear_drag: 0.0,
            skin_posture_hertz: 0.0,
            muscle_posture_hertz: 0.0,
            skin_damping_ratio: zeta,
            ..Materials::default()
        };
        materials.skin_stiffening_stretch = 10.0;
        let mut world = World::new(materials);
        let top = world.add_point(Vec2 { x: 100.0, y: 100.0 }, TissueLayer::Skin, true);
        let ball = world.add_point(Vec2 { x: 100.0, y: 200.0 }, TissueLayer::Skin, false);
        world.add_spring(
            top,
            ball,
            TissueLayer::Skin,
            stiffness / STANDARD_SPRING_STIFFNESS,
            10.0,
            1.0e9,
            false,
        );
        world.points[ball].position.y = 210.0;
        world.points[ball].previous.y = 210.0;
        world
    }

    fn ball_height(world: &World) -> f64 {
        world.points[1].position.y
    }

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// A joint's pin moves the place it holds and the anchor it holds
        /// there apart by exactly its impulse over its effective mass, the
        /// same every way, wherever the two bones lie and whatever the
        /// hold: the effective mass the solver divides by is the one the
        /// bones' ends really give.
        #[test]
        fn a_joint_pin_moves_its_anchor_as_its_mass_says(
            ax in -200.0..200.0f64, ay in -200.0..200.0f64, bx in -200.0..200.0f64, by in -200.0..200.0f64,
            cx in -200.0..200.0f64, cy in -200.0..200.0f64, dx in -200.0..200.0f64, dy in -200.0..200.0f64,
            t_a in 0.0..1.0f64, t_b in 0.0..1.0f64,
            along in -20.0..20.0f64, across in -20.0..20.0f64,
            push_x in -50.0..50.0f64, push_y in -50.0..50.0f64,
        ) {
            let (a0, a1) = (Vec2 { x: ax, y: ay }, Vec2 { x: bx, y: by });
            let (b0, b1) = (Vec2 { x: cx, y: cy }, Vec2 { x: dx, y: dy });
            prop_assume!(distance(a0, a1) > 5.0 && distance(b0, b1) > 5.0);
            let mut world = World::new(Materials::default());
            let first = world.add_bone_segment(a0, a1, 4.0, 1.0e9, false);
            let second = world.add_bone_segment(b0, b1, 4.0, 1.0e9, false);
            world.bones[first].mass = 3.0;
            world.bones[second].mass = 7.0;
            let joint = BoneJoint {
                a: first,
                b: second,
                t_a,
                t_b,
                rest: 8.0,
                hold: Vec2 { x: along, y: across },
                ..BoneJoint::default()
            };
            let h = 1.0 / 240.0;
            let pin = JointPin::of(&joint, &world.bones, 1.0);
            let error_now = |world: &World| JointPin::of(&joint, &world.bones, 1.0).error;
            // The pin is judged from where the substep began, so let the
            // bones begin where they are.
            let before = error_now(&world);
            let impulse = Vec2 { x: push_x, y: push_y };
            world.apply_joint_pin(&joint, &pin, impulse, h);
            for bone in [first, second] {
                world.bones[bone].previous_a = world.bones[bone].a;
                world.bones[bone].previous_b = world.bones[bone].b;
            }
            let after = error_now(&world);
            let moved = subtract(after, before);
            let expected = scale(impulse, pin.inverse_mass * h);
            prop_assert!(
                distance(moved, expected) < 1e-6 * (1.0 + hypot(expected.x, expected.y)),
                "moved {moved:?}, expected {expected:?}"
            );
        }
    }

    /// One whole step, with no tool in play.
    fn run(world: &mut World) {
        let dt = world.materials.fixed_dt;
        world.step(dt, &InputState::default(), 1000.0);
    }

    #[test]
    fn a_soft_spring_rings_at_its_own_frequency_with_any_number_of_substeps() {
        // 10 N/m on 0.1 kg rings at sqrt(100) = 10 rad/s, 1.59 Hz: the ball
        // comes back up past its rest after about a third of a second.
        for substeps in [2, 4, 8] {
            let mut world = spring_world(10.0, 0.0, substeps);
            let mut crossed_at = None;
            for frame in 0..60 {
                run(&mut world);
                if ball_height(&world) < 200.0 {
                    crossed_at = Some(frame + 1);
                    break;
                }
            }
            // A quarter of the period is 0.157 s, between steps 9 and 10.
            let crossed = crossed_at.expect("the spring should pull the ball back");
            assert!(
                (9..=11).contains(&crossed),
                "with {substeps} substeps the ball came back after {crossed} steps"
            );
        }
    }

    #[test]
    fn damping_ratio_settles_a_spring() {
        let mut ringing = spring_world(10.0, 0.0, 4);
        let mut damped = spring_world(10.0, 1.0, 4);
        for _ in 0..40 {
            run(&mut ringing);
            run(&mut damped);
        }
        let ring = (ball_height(&ringing) - 200.0).abs();
        let rest = (ball_height(&damped) - 200.0).abs();
        assert!(
            rest < 0.6,
            "a critically damped spring should settle, {rest}"
        );
        assert!(ring > rest * 2.0, "{ring} against {rest}");
    }

    #[test]
    fn a_blow_to_the_flesh_moves_the_bone_in_it() {
        let mut materials = Materials {
            gravity: 0.0,
            muscle_posture_hertz: 0.0,
            ..Materials::default()
        };
        materials.linear_drag = 0.0;
        let mut world = World::new(materials);
        let bone = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 100.0, y: 200.0 },
            5.0,
            1.0e9,
            false,
        );
        world.bones[bone].mass = 1.0;
        for y in [120.0, 150.0, 180.0] {
            let point = world.add_point(Vec2 { x: 115.0, y }, TissueLayer::Muscle, false);
            world.points[point].mass = 3.0;
            world.add_bone_attachment(point, bone, (y - 100.0) / 100.0);
            // Shoved sideways as a tool would.
            world.points[point].previous.x -= 4.0;
        }
        for _ in 0..12 {
            run(&mut world);
        }
        let middle = midpoint(world.bones[bone].a, world.bones[bone].b);
        let flesh = world.points.iter().map(|p| p.position.x).sum::<f64>() / 3.0;
        assert!(
            middle.x > 115.0,
            "the flesh should carry the bone, which moved to {:.1}",
            middle.x
        );
        assert!(
            (flesh - middle.x - 15.0).abs() < 3.0,
            "the flesh should stay where it sits on the bone: {flesh:.1} against {:.1}",
            middle.x
        );
    }
}
