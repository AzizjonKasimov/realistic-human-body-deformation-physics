//! Physical tissue: what the body's points weigh and how stiff its springs
//! are, from what real skin, fat, muscle, and bone measure
//! (`docs/INJURY_REFERENCE.md`, Physical tissue). The solver in `soft.rs`
//! moves them.
//!
//! The figure is a flat front view, so each sheet stands for a slab of the
//! body seen from the front. The skin sheet is the skin and the fat under it,
//! which slide over the muscle together, front and back and round the sides:
//! a limb's skin is about three times as wide around as across. Its stiffness
//! is the dermis's; its mass, the dermis's and the fat's. The muscle sheet is
//! everything under that, as thick at each point as the body is deep there:
//! a limb is round, and the trunk about three quarters as deep as it is wide
//! (ANSUR II). A sheet of shear modulus `mu` and thickness `t` resists in its
//! plane as a membrane of `mu t` newtons per metre; a triangle mesh carries
//! that on springs of `8/3 mu t` times the area of each side's triangles over
//! its length squared (Van Gelder 1998), with the rest of its resistance to
//! growing or shrinking, as tissue that keeps its volume does, on each
//! triangle's area.

use super::soft::KG_PER_MASS;
use super::*;

/// Density of the dermis, of fat, and of the muscle, organs, and fat inside
/// the body, in kilograms per cubic metre (ICRU 44 via NIST).
const SKIN_DENSITY: f64 = 1090.0;
const FAT_DENSITY: f64 = 950.0;
const SOFT_TISSUE_DENSITY: f64 = 1050.0;
/// How far a limb's skin runs around it for each width the front view shows:
/// pi for a round limb, 2.9 for the belly (94 cm around and 32.6 cm across in
/// ANSUR II).
const SKIN_WRAP: f64 = 3.0;
/// How deep the body is for its width as seen from the front: limbs are about
/// round, the trunk about three quarters as deep as wide at the belly and
/// hips (ANSUR II men: 23.8 cm deep and 32.6 cm wide at the navel), the head
/// deeper than wide.
const LIMB_DEPTH_RATIO: f64 = 1.0;
const TRUNK_DEPTH_RATIO: f64 = 0.73;
const HEAD_DEPTH_RATIO: f64 = 1.25;
/// The person the body stands for weighs this much, in kilograms.
pub(super) const BODY_MASS_KG: f64 = 72.5;

/// What kind of fiber a spring is, which sets its stiffness and how it
/// stiffens as it stretches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpringKind {
    /// A side of a skin triangle.
    #[default]
    Skin,
    /// A side of a muscle triangle along the muscle's fibers, across them,
    /// or between, by its angle to the nearest bone.
    Fiber,
    Cross,
    Shear,
    /// A spring two sides long along the fibers, which keeps muscle from
    /// folding along its length.
    LongFiber,
}

impl World {
    /// Weighs every point, bone, and spring of the body as built in `frame`
    /// from what real tissue measures.
    pub(super) fn give_tissue_physical_properties(&mut self, frame: BodyFrame) {
        let thickness = self.tissue_thickness(frame);
        self.weigh_tissue(&thickness);
        self.stiffen_tissue(&thickness);
        self.weigh_bones();
        self.prestretch_skin(frame);
    }

    /// How thick each point's sheet is, in metres: the skin sheet the dermis
    /// all round, and the muscle sheet as deep as the body is there.
    fn tissue_thickness(&self, frame: BodyFrame) -> Vec<f64> {
        let m_per_px = self.mm_per_px() * 0.001;
        let muscle: Vec<usize> = (0..self.points.len())
            .filter(|&index| self.points[index].layer == TissueLayer::Muscle)
            .collect();
        self.points
            .iter()
            .map(|point| match point.layer {
                TissueLayer::Skin => self.materials.skin_thickness * SKIN_WRAP,
                TissueLayer::Muscle => {
                    // The largest circle inside the body that holds the point
                    // is as wide as the limb or trunk it lies in; from how
                    // far in from the outline the point is, the body is as
                    // deep there as a round limb would be.
                    let depth = point.surface_depth;
                    let mut radius = depth;
                    for &other in &muscle {
                        let held = &self.points[other];
                        if held.surface_depth > radius
                            && distance(held.home, point.home) <= held.surface_depth
                        {
                            radius = held.surface_depth;
                        }
                    }
                    let (u, v) = frame.body_coords(point.home);
                    let ratio = if v < 0.13 {
                        HEAD_DEPTH_RATIO
                    } else if u.abs() < 0.105 && v < 0.53 {
                        TRUNK_DEPTH_RATIO
                    } else {
                        LIMB_DEPTH_RATIO
                    };
                    let half_chord = (2.0 * radius * depth - depth * depth).max(0.0).sqrt();
                    // Under the skin and the fat that move with it.
                    let body = 2.0 * ratio * half_chord.max(radius * 0.35) * m_per_px;
                    (body - 2.0 * self.integument_thickness()).max(body * 0.4)
                }
            })
            .collect()
    }

    /// The skin and the fat under it, which slide over the muscle together.
    fn integument_thickness(&self) -> f64 {
        self.materials.skin_thickness + self.materials.subcutaneous_fat_thickness
    }

    /// Each point weighs what its share of its sheet's triangles holds.
    fn weigh_tissue(&mut self, thickness: &[f64]) {
        let m_per_px = self.mm_per_px() * 0.001;
        let mut area = vec![0.0; self.points.len()];
        for triangle in &self.triangles {
            let corners = [triangle.a, triangle.b, triangle.c];
            let share = signed_area(
                self.points[triangle.a].home,
                self.points[triangle.b].home,
                self.points[triangle.c].home,
            )
            .abs()
                / 3.0;
            for corner in corners {
                area[corner] += share;
            }
        }
        for (index, point) in self.points.iter_mut().enumerate() {
            let square_metres = area[index] * m_per_px * m_per_px;
            let kilograms = match point.layer {
                TissueLayer::Skin => {
                    square_metres
                        * SKIN_WRAP
                        * (self.materials.skin_thickness * SKIN_DENSITY
                            + self.materials.subcutaneous_fat_thickness * FAT_DENSITY)
                }
                TissueLayer::Muscle => square_metres * thickness[index] * SOFT_TISSUE_DENSITY,
            };
            point.mass = (kilograms / KG_PER_MASS).max(0.01);
        }
    }

    /// Sets each spring's and triangle's stiffness from its sheet's modulus
    /// and thickness, and each skin point's hold on the muscle from the fat
    /// under it.
    fn stiffen_tissue(&mut self, thickness: &[f64]) {
        let m_per_px = self.mm_per_px() * 0.001;
        // Van Gelder's sum over each side's triangles of area over the
        // side's length squared, from the rest shape.
        let mut reach = vec![0.0; self.springs.len()];
        for triangle in &self.triangles {
            let (a, b, c) = (
                self.points[triangle.a].home,
                self.points[triangle.b].home,
                self.points[triangle.c].home,
            );
            let area = signed_area(a, b, c).abs();
            for (edge, from, to) in [
                (triangle.edge_ab, a, b),
                (triangle.edge_bc, b, c),
                (triangle.edge_ca, c, a),
            ] {
                if let Some(reach) = reach.get_mut(edge) {
                    *reach += area / distance_sq(from, to).max(EPSILON);
                }
            }
        }
        // A spring two sides long has no triangle of its own: it carries
        // what an inside side of the lattice does.
        let lattice = 3.0_f64.sqrt() / 2.0;
        for (index, spring) in self.springs.iter_mut().enumerate() {
            let modulus = self.materials.shear_modulus(spring.kind);
            let sheet = (thickness[spring.a] + thickness[spring.b]) * 0.5;
            let share = match spring.kind {
                SpringKind::LongFiber => lattice * LONG_FIBER_SHARE,
                _ => reach[index],
            };
            spring.stiffness = 8.0 / 3.0 * modulus * sheet * share;
        }
        // A triangle that keeps its volume resists growing or shrinking in
        // the plane as much again as the springs do (plane stress with
        // Poisson's ratio one half: lambda = 2 mu, of which the springs carry
        // mu).
        for area in &mut self.areas {
            let modulus = self.materials.shear_modulus(match area.layer {
                TissueLayer::Skin => SpringKind::Skin,
                TissueLayer::Muscle => SpringKind::Shear,
            });
            let sheet = (thickness[area.a] + thickness[area.b] + thickness[area.c]) / 3.0;
            area.stiffness = modulus * sheet;
        }
        // Fat shears over the muscle: a layer of shear modulus G and
        // thickness h holds each square metre of skin by G / h newtons per
        // metre it slides.
        let fat = self.materials.fat_shear_modulus
            / self.materials.subcutaneous_fat_thickness.max(1.0e-4);
        let mut held = vec![0usize; self.points.len()];
        for attachment in &self.attachments {
            held[attachment.skin_point] += 1;
        }
        let mut area = vec![0.0; self.points.len()];
        for triangle in &self.triangles {
            if triangle.layer != TissueLayer::Skin {
                continue;
            }
            let share = signed_area(
                self.points[triangle.a].home,
                self.points[triangle.b].home,
                self.points[triangle.c].home,
            )
            .abs()
                / 3.0;
            for corner in [triangle.a, triangle.b, triangle.c] {
                area[corner] += share;
            }
        }
        for attachment in &mut self.attachments {
            let point = attachment.skin_point;
            attachment.stiffness =
                fat * area[point] * m_per_px * m_per_px * SKIN_WRAP / held[point].max(1) as f64;
        }
    }

    /// Each bone weighs its share of its part of the body (Winter's segment
    /// masses; bone is about 15% of a man's mass, Clarys et al. 1984).
    fn weigh_bones(&mut self) {
        for bone in &mut self.bones {
            bone.mass = bone_mass_kg(bone.part) / KG_PER_MASS;
        }
    }

    /// Skin in a living body is stretched: cut out, it shrinks about a tenth
    /// across the lines it is tensed along and a quarter along them (Deroy et
    /// al. 2017), and those lines run round the trunk and limbs. So each skin
    /// spring rests shorter than it was built, by `skin_prestretch_along`
    /// along those lines and `skin_prestretch_across` across them, and the
    /// skin pulls a cut's lips apart. The outline's springs stand for skin
    /// that runs on round the back, which holds the stretch.
    fn prestretch_skin(&mut self, frame: BodyFrame) {
        let along = self.materials.skin_prestretch_along;
        let across = self.materials.skin_prestretch_across;
        if along <= 1.0 && across <= 1.0 {
            return;
        }
        let guides = body::fiber_guides(frame);
        for (spring, wear) in self.springs.iter_mut().zip(&mut self.spring_wear) {
            if spring.kind != SpringKind::Skin {
                continue;
            }
            let (a, b) = (self.points[spring.a].home, self.points[spring.b].home);
            let direction = normalized(subtract(b, a), Vec2 { x: 1.0, y: 0.0 });
            let axis = body::fiber_direction(midpoint(a, b), &guides);
            let cos = dot(direction, axis);
            let stretch = (along * cos * cos + across * (1.0 - cos * cos)).max(1.0);
            spring.rest /= stretch;
            wear.rest_reference = spring.rest;
            wear.prestretch = stretch;
        }
        // Stretched both ways, each triangle of skin rests smaller too.
        for area in &mut self.areas {
            if area.layer == TissueLayer::Skin {
                area.rest_area /= along.max(1.0) * across.max(1.0);
            }
        }
        self.settle_skin();
        self.measure_outline_wrap();
        self.carry_rest_tension();
    }

    /// The flesh round the elbows rests as a living arm's does: a relaxed
    /// elbow rests part way open (Wiegner and Watts 1986), not bent as far as
    /// the guard holds it, and its skin and muscle bear being stretched as
    /// far as the elbow's whole range takes them. A weightless copy of the
    /// body turns each joint in `joints` (its index, the way it opens, how far
    /// it opens to relax, and how far its range takes it that way) from the
    /// guard to where it relaxes, and on to the end of its range; each spring
    /// and triangle near those joints then rests at the length and area it
    /// had relaxed, and tires, sets, and tears only past the longest the
    /// range stretched it. Built at rest in the guard, the fold held a limp
    /// forearm up, and the skin at the elbow's crease, stretched half again
    /// with the arm straight, tore as it tired. The shoulders rest as built:
    /// rested hanging at the side, a struck upper arm broke more often and
    /// its flesh turned inside out. Returns the points whose flesh took new
    /// lengths; `width` is the window the body stands in.
    pub(super) fn rest_flesh_round_joints(
        &mut self,
        joints: &[(usize, f64, f64, f64)],
        width: f64,
    ) -> Vec<bool> {
        let mut touched = vec![false; self.points.len()];
        if joints.is_empty() {
            return touched;
        }
        // Only the flesh round the joints takes new lengths: the sweep moves
        // nothing else on purpose, and whatever else it nudged is not the
        // joints' to set.
        let reach = JOINT_REST_REACH * self.materials.point_spacing;
        let pivots: Vec<Vec2> = joints
            .iter()
            .map(|&(index, ..)| self.bones[self.bone_joints[index].b].home_a)
            .collect();
        let near: Vec<bool> = self
            .points
            .iter()
            .map(|point| {
                pivots
                    .iter()
                    .any(|&pivot| distance(point.home, pivot) < reach)
            })
            .collect();
        let mut swept = self.clone();
        swept.materials.gravity = 0.0;
        let dt = swept.materials.fixed_dt;
        let lengths = |world: &World| -> Vec<f64> {
            world
                .springs
                .iter()
                .map(|spring| {
                    distance(
                        world.points[spring.a].position,
                        world.points[spring.b].position,
                    )
                })
                .collect()
        };
        let built = lengths(self);
        let mut longest = built.clone();
        let mut opened = Vec::new();
        let mut opened_areas = Vec::new();
        // Muscles stiff enough to hold the elbow where they aim it against
        // the flesh: opened to the relaxed angle and held there while the
        // flesh settles, then on to straight.
        for (phase, ramp, hold) in [(0, 24, 24), (1, 36, 12)] {
            for step in 0..ramp + hold {
                let share = ((step + 1) as f64 / ramp as f64).min(1.0);
                for &(index, way, relaxed, full) in joints {
                    let (from, to) = if phase == 0 {
                        (0.0, relaxed)
                    } else {
                        (relaxed, full)
                    };
                    let drive = swept.bone_joints[index].drive;
                    swept.bone_joints[index].drive = JointDrive {
                        stiffness: 300.0,
                        damping: 15.0,
                        max_torque: 200.0,
                        aim: way * (from + (to - from) * share),
                        ..drive
                    };
                }
                swept.step(dt, &InputState::default(), width);
                for (most, now) in longest.iter_mut().zip(lengths(&swept)) {
                    *most = most.max(now);
                }
            }
            if opened.is_empty() {
                opened = lengths(&swept);
                opened_areas = swept
                    .areas
                    .iter()
                    .map(|area| {
                        signed_area(
                            swept.points[area.a].position,
                            swept.points[area.b].position,
                            swept.points[area.c].position,
                        )
                    })
                    .collect();
            }
        }
        for (index, (spring, wear)) in self
            .springs
            .iter_mut()
            .zip(&mut self.spring_wear)
            .enumerate()
        {
            let (was, now, most) = (built[index], opened[index], longest[index]);
            if !(near[spring.a] && near[spring.b])
                || was <= EPSILON
                || ((now / was - 1.0).abs() < 0.01 && most / was - 1.0 < 0.01)
            {
                continue;
            }
            spring.rest *= now / was;
            wear.rest_reference = spring.rest;
            wear.prestretch = most / spring.rest;
            touched[spring.a] = true;
            touched[spring.b] = true;
        }
        for (area, &now) in self.areas.iter_mut().zip(&opened_areas) {
            if !(near[area.a] && near[area.b] && near[area.c]) {
                continue;
            }
            let was = signed_area(
                self.points[area.a].home,
                self.points[area.b].home,
                self.points[area.c].home,
            );
            if was.abs() <= EPSILON || (now / was - 1.0).abs() < 0.01 {
                continue;
            }
            area.rest_area *= now / was;
            for corner in [area.a, area.b, area.c] {
                touched[corner] = true;
            }
        }
        self.carry_rest_tension();
        touched
    }

    /// Once the body has settled, the flesh `touched` takes its hold on the
    /// muscle under it, on its bones, and its place on the skeleton from
    /// where it now lies: rested part way open (`rest_flesh_round_joints`),
    /// the flesh round the elbow settles a little off where it was built,
    /// and held to where it was built, the fat there started sheared, so a
    /// jolt to the other side of the body tore it and let the elbow sag open.
    pub(super) fn hold_flesh_where_it_lies(&mut self, touched: &[bool]) {
        let touches = |point: usize| touched.get(point).copied().unwrap_or(false);
        let spacing = self.materials.point_spacing;
        for attachment in &mut self.attachments {
            if attachment.broken
                || !(touches(attachment.skin_point) || touches(attachment.muscle_point))
            {
                continue;
            }
            attachment.rest = distance(
                self.points[attachment.skin_point].position,
                self.points[attachment.muscle_point].position,
            )
            .max(spacing * 0.34);
            attachment.impulse = 0.0;
        }
        for attachment in &mut self.bone_attachments {
            if attachment.broken || !touches(attachment.point) {
                continue;
            }
            let bone = self.bones[attachment.bone];
            let point = self.points[attachment.point].position;
            attachment.offset = subtract(point, bone_point(bone, attachment.t));
            attachment.rest = distance(point, bone_point(bone, attachment.t)).max(1.0);
            (attachment.along, attachment.across) = bone_tie_frame(&bone, attachment.t, point);
            attachment.impulse = Vec2::default();
        }
        for (index, hold) in self.point_holds.iter_mut().enumerate() {
            if !touches(index) {
                continue;
            }
            let point = self.points[index].position;
            let binding = &mut hold.posture;
            for slot in 0..2 {
                let Some(bone) = self.bones.get(binding.bones[slot]) else {
                    continue;
                };
                (binding.tau[slot], binding.sigma[slot]) =
                    stance::bone_frame(bone.a, bone.b, point);
            }
            hold.posture_impulse = Vec2::default();
        }
    }

    /// Lets the stretched skin settle where it pulls itself even, its outline
    /// held: on an uneven mesh its springs do not quite balance as built, and
    /// what is left would slide it over the muscle. Each point inside moves
    /// a fraction of a pixel.
    fn settle_skin(&mut self) {
        let mut on_outline = vec![false; self.points.len()];
        for outline in &self.outlines {
            for &point in outline.points() {
                on_outline[point] = true;
            }
        }
        let skin_springs: Vec<usize> = (0..self.springs.len())
            .filter(|&index| self.springs[index].kind == SpringKind::Skin)
            .collect();
        let skin_areas: Vec<usize> = (0..self.areas.len())
            .filter(|&index| self.areas[index].layer == TissueLayer::Skin)
            .collect();
        for _ in 0..SKIN_SETTLE_PASSES {
            let mut force = vec![Vec2::default(); self.points.len()];
            let mut stiffness = vec![0.0; self.points.len()];
            for &index in &skin_springs {
                let spring = self.springs[index];
                let (a, b) = (self.points[spring.a].home, self.points[spring.b].home);
                let along = subtract(b, a);
                let length = hypot(along.x, along.y);
                if length < EPSILON {
                    continue;
                }
                let (k, rest) = self.spring_stiffness_at(&spring, length);
                let pull = scale(along, k * (length - rest) / length);
                force[spring.a] = add(force[spring.a], pull);
                force[spring.b] = subtract(force[spring.b], pull);
                stiffness[spring.a] += k;
                stiffness[spring.b] += k;
            }
            for &index in &skin_areas {
                let area = self.areas[index];
                let corners = [area.a, area.b, area.c];
                let [a, b, c] = corners.map(|corner| self.points[corner].home);
                let k = area.stiffness / area.rest_area.abs().max(1.0);
                let squeeze = k * (signed_area(a, b, c) - area.rest_area);
                let gradients = [
                    scale(perpendicular_to(subtract(b, c)), 0.5),
                    scale(perpendicular_to(subtract(c, a)), 0.5),
                    scale(perpendicular_to(subtract(a, b)), 0.5),
                ];
                for (corner, gradient) in corners.into_iter().zip(gradients) {
                    force[corner] = subtract(force[corner], scale(gradient, squeeze));
                    stiffness[corner] += k * dot(gradient, gradient);
                }
            }
            for (index, point) in self.points.iter_mut().enumerate() {
                if point.layer != TissueLayer::Skin
                    || point.pinned
                    || on_outline[index]
                    || stiffness[index] <= EPSILON
                {
                    continue;
                }
                let shift = scale(force[index], 0.5 / stiffness[index]);
                point.home = add(point.home, shift);
                point.position = point.home;
                point.previous = point.home;
            }
        }
        // The fat under the skin rests as the skin now lies.
        let spacing = self.materials.point_spacing;
        for attachment in &mut self.attachments {
            attachment.rest = distance(
                self.points[attachment.skin_point].home,
                self.points[attachment.muscle_point].home,
            )
            .max(spacing * 0.34);
        }
    }

    /// Starts every spring and triangle of skin carrying the tension it holds
    /// at rest, as the warm start would after a few substeps, so the body is
    /// still from its first step: the skin's pull at the outline is held from
    /// the start (`pull_outline_round`), and the light points there would be
    /// flung out before the springs caught up.
    fn carry_rest_tension(&mut self) {
        let h = self.materials.fixed_dt / self.materials.substeps.max(1) as f64;
        for index in 0..self.springs.len() {
            let spring = self.springs[index];
            let length = distance(self.points[spring.a].home, self.points[spring.b].home);
            let (stiffness, rest) = self.spring_stiffness_at(&spring, length);
            self.springs[index].impulse = -h * stiffness / KG_PER_MASS * (length - rest);
        }
        for area in &mut self.areas {
            let rest = signed_area(
                self.points[area.a].home,
                self.points[area.b].home,
                self.points[area.c].home,
            );
            let stiffness = area.stiffness / KG_PER_MASS / area.rest_area.abs().max(1.0);
            area.impulse = -h * stiffness * (rest - area.rest_area);
        }
    }

    /// How stiff `spring` is at length `length`, in newtons per metre, and the
    /// length it pulls toward there: tissue is soft at first and stiffens
    /// sharply once its collagen straightens, as a J-shaped curve.
    pub(super) fn spring_stiffness_at(&self, spring: &Spring, length: f64) -> (f64, f64) {
        let (onset, ratio) = self.materials.stiffening(spring.kind);
        if ratio <= 1.0 || length <= spring.rest * onset {
            return (spring.stiffness, spring.rest);
        }
        // Past the onset the spring is `ratio` times as stiff, and pulls
        // toward the length at which that line would carry no load.
        (
            spring.stiffness * ratio,
            spring.rest * (onset - (onset - 1.0) / ratio),
        )
    }

    /// How fast a spring's motion along it dies away, as a damping ratio.
    pub(super) fn spring_damping(&self, spring: &Spring) -> f64 {
        match spring.layer {
            TissueLayer::Skin => self.materials.skin_damping_ratio,
            TissueLayer::Muscle => self.materials.muscle_damping_ratio,
        }
    }

    pub(super) fn area_damping(&self, area: &AreaConstraint) -> f64 {
        match area.layer {
            TissueLayer::Skin => self.materials.skin_damping_ratio,
            TissueLayer::Muscle => self.materials.muscle_damping_ratio,
        }
    }

    /// Fat stiffens as it shears, several times over by the time it has
    /// slid half its thickness (Alkhouli et al. 2013: 1.6 kPa at first, 11.7
    /// kPa by 30% strain).
    pub(super) fn fat_stiffening(&self, shear: f64) -> f64 {
        1.0 + (FAT_STIFFENING - 1.0) * (shear / FAT_STIFFENING_SHEAR).min(1.0)
    }

    /// How far the skin of `attachment` has slid over the muscle, `length`
    /// apart, in thicknesses of the fat between them.
    pub(super) fn fat_shear(&self, attachment: &Attachment, length: f64) -> f64 {
        let thickness = self.materials.subcutaneous_fat_thickness * 1000.0 / self.mm_per_px();
        (length - attachment.rest).abs() / thickness.max(EPSILON)
    }
}

/// How far round a joint's turning point, in point spacings, the flesh rests
/// as a relaxed arm's does (`rest_flesh_round_joints`): the elbow's crease
/// and its point, where the fold bends the flesh.
const JOINT_REST_REACH: f64 = 4.0;

/// How many passes the stretched skin takes to settle as the body is built.
const SKIN_SETTLE_PASSES: usize = 300;

/// `value` turned a quarter turn: (y, -x), so that half of it for each side
/// of a triangle is the gradient of the triangle's signed area.
fn perpendicular_to(value: Vec2) -> Vec2 {
    Vec2 {
        x: value.y,
        y: -value.x,
    }
}

/// A spring two sides long carries this share of what a side of the lattice
/// does, enough to keep muscle from folding along its fibers.
const LONG_FIBER_SHARE: f64 = 0.3;
/// How many times stiffer fat is once it has slid far, and how far that is,
/// in thicknesses of the fat.
const FAT_STIFFENING: f64 = 6.0;
const FAT_STIFFENING_SHEAR: f64 = 0.5;

impl Materials {
    /// The shear modulus at small strain, in pascals, of the tissue a spring
    /// of `kind` is in.
    pub(super) fn shear_modulus(&self, kind: SpringKind) -> f64 {
        match kind {
            SpringKind::Skin => self.skin_shear_modulus,
            SpringKind::Fiber | SpringKind::LongFiber => self.muscle_fiber_shear_modulus,
            SpringKind::Cross => self.muscle_cross_shear_modulus,
            SpringKind::Shear => {
                (self.muscle_fiber_shear_modulus * self.muscle_cross_shear_modulus).sqrt()
            }
        }
    }

    /// Where a spring of `kind` starts to stiffen, as a stretch of its rest
    /// length, and how many times stiffer it is past there.
    pub(super) fn stiffening(&self, kind: SpringKind) -> (f64, f64) {
        match kind {
            SpringKind::Skin => (self.skin_stiffening_stretch, self.skin_stiffening_ratio),
            SpringKind::Fiber | SpringKind::LongFiber => (
                self.muscle_fiber_stiffening_stretch,
                self.muscle_fiber_stiffening_ratio,
            ),
            SpringKind::Cross => (
                self.muscle_cross_stiffening_stretch,
                self.muscle_cross_stiffening_ratio,
            ),
            SpringKind::Shear => (f64::INFINITY, 1.0),
        }
    }
}

/// What a bone of `part` weighs, in kilograms: its part's share of a 72.5 kg
/// man (Winter 2009) times the share of that which is bone. The shares by
/// part are estimates from typical bone weights; the whole skeleton weighs
/// about 15% of a man (Clarys et al. 1984). The figure's ribs are four a side
/// for a real twelve, and its spine the column from the neck to the pelvis.
/// Its collarbone stands for the shoulder girdle, the collarbone and the
/// shoulder blade behind it, which the front view does not show; the arm
/// hangs on both, and a bone as light as the collarbone alone, between the
/// spine and the arm, would let the joints on it stretch and the shoulder's
/// muscles hold the arm too weakly (Box2D's `docs/loose_ends.md`).
fn bone_mass_kg(part: BonePart) -> f64 {
    let segment = |share_of_body: f64, share_bone: f64| BODY_MASS_KG * share_of_body * share_bone;
    match part {
        BonePart::UpperArm => segment(0.028, 0.14),
        BonePart::Forearm => segment(0.016, 0.18),
        BonePart::Hand => segment(0.006, 0.25),
        BonePart::Thigh => segment(0.100, 0.10),
        BonePart::Shin => segment(0.0465, 0.18),
        BonePart::Foot => segment(0.0145, 0.28),
        BonePart::Collarbone => 0.15,
        BonePart::Rib => 0.15,
        BonePart::Spine => 1.2,
        BonePart::Pelvis => 0.9,
        BonePart::Skull => 0.9,
        BonePart::Other => 0.2,
    }
}
