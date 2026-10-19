//! Several bodies in one world. A fight puts two fighters in the same world,
//! each standing in its own place at the same size (`crate::fighter_frames`):
//! their flesh and bones lie in the world's one set of lists, so the solver,
//! the tools, and the blows treat them alike, and each point and bone is
//! marked with the body it belongs to. What is a body's own beyond its flesh
//! and bones (its blood, its limbs that hang, the punches its arms throw, and
//! the bones it has broken) lies in its [`BodyState`].
//!
//! A second fighter is the first's exact mirror image ([`World::mirrored`]),
//! point for point and bone for bone, so the solver takes the two in the
//! same order, mirrored: a blow from either fighter on the other is solved
//! as the mirror image of the same blow the other way, and the fight is fair
//! however a body solved one piece after another leans at rest.

use super::*;

/// What the world keeps of each body besides its flesh and bones.
#[derive(Clone, Debug)]
pub struct BodyState {
    /// The blood it has left, in the units of `Materials::blood_volume_capacity`.
    pub(super) blood_volume: f64,
    /// Which limbs hang from a broken bone, their muscles let go, in the
    /// order of [`Limb::index`].
    pub(super) limp_limbs: [bool; 4],
    /// The punch each arm is throwing, the viewer's left arm first
    /// (`punch.rs`).
    pub(super) punches: [Option<Punch>; 2],
    /// How far its punches carry its hips toward what they hit, in pixels,
    /// from where they are held (`stance.rs`).
    pub(super) hip_shift: Vec2,
    /// Whether it was built as another body's mirror image
    /// ([`World::mirrored`]), so what was taken on a bone's left as the
    /// body was tuned is taken on its right.
    pub(super) mirrored: bool,
    /// How many of its bones of each part have broken, by
    /// [`BonePart::index`]; `Stats` counts every body's together.
    pub fractures: [i32; BONE_PARTS],
    /// How many of its skin's fibers have torn or been cut; `Stats` counts
    /// every body's together.
    pub skin_tears: i32,
}

impl BodyState {
    pub(super) fn new(materials: &Materials) -> Self {
        Self {
            blood_volume: materials.blood_volume_capacity.max(0.0),
            limp_limbs: [false; 4],
            punches: [None, None],
            hip_shift: Vec2::default(),
            mirrored: false,
            fractures: [0; BONE_PARTS],
            skin_tears: 0,
        }
    }
}

/// An index into one of the world's lists, moved past the `offset` entries
/// already there, unless it is the list's "none" sentinel.
fn moved(index: usize, offset: usize, missing: usize) -> usize {
    if index == missing {
        index
    } else {
        index + offset
    }
}

impl World {
    /// How many bodies the world holds.
    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// What body `body` has broken, bled, and let hang.
    pub fn body_state(&self, body: usize) -> Option<&BodyState> {
        self.bodies.get(body)
    }

    /// The body point `index` belongs to.
    pub(super) fn point_body(&self, index: usize) -> usize {
        self.points
            .get(index)
            .map_or(0, |point| point.body as usize)
    }

    /// The body bone `index` belongs to.
    pub(super) fn bone_body(&self, index: usize) -> usize {
        self.bones.get(index).map_or(0, |bone| bone.body as usize)
    }

    /// Body `body`'s arm on the side of the world the other bodies stand on,
    /// the one a fighter punches its opponent with: for the left fighter its
    /// right arm, and for its mirror image beside it, the arm of the same
    /// name, which the mirror puts on the left.
    pub fn inner_arm(&self, body: usize) -> Limb {
        let middle = self.midline_of(body).unwrap_or(0.0);
        let others: Vec<f64> = (0..self.bodies.len())
            .filter(|&other| other != body)
            .filter_map(|other| self.midline_of(other))
            .collect();
        let toward = others.iter().map(|x| x - middle).sum::<f64>();
        let right_shoulder = self
            .bones
            .iter()
            .find(|bone| {
                bone.part == BonePart::UpperArm
                    && bone.limb == Limb::RightArm
                    && bone.body as usize == body
            })
            .map_or(middle + 1.0, |bone| bone.home_a.x);
        if (right_shoulder - middle) * toward >= 0.0 {
            Limb::RightArm
        } else {
            Limb::LeftArm
        }
    }

    /// Where body `body`'s midline lies across the world: where its spine
    /// was built.
    pub(super) fn midline_of(&self, body: usize) -> Option<f64> {
        self.bones
            .iter()
            .find(|bone| bone.kind == BoneKind::Spine && bone.body as usize == body)
            .map(|spine| (spine.home_a.x + spine.home_b.x) * 0.5)
    }

    /// The body whose flesh lies nearest `at`.
    pub(super) fn body_near(&self, at: Vec2) -> usize {
        if self.bodies.len() < 2 {
            return 0;
        }
        self.points
            .iter()
            .map(|point| (distance_sq(point.position, at), point.body as usize))
            .min_by(|p, q| p.0.total_cmp(&q.0))
            .map_or(0, |(_, body)| body)
    }

    /// The body joint `joint` belongs to: its bones'.
    pub(super) fn joint_body(&self, joint: &BoneJoint) -> usize {
        self.bone_body(joint.a)
    }

    /// The bodies of `other`, a world of their own, added to this one, each
    /// after the bodies already here: their flesh, bones, and everything
    /// joining them, moved to follow what is already in the world's lists.
    /// Returns the index the first of them takes. Both must be at rest, as
    /// built: blood out of the body, open wounds, and a tool in play are not
    /// carried over.
    pub fn add_body(&mut self, other: World) -> usize {
        debug_assert!(
            other.wounds.is_empty() && other.fluids.is_empty() && other.blood_trails.is_empty(),
            "only a body at rest can be added to a world"
        );
        let first = self.bodies.len();
        let points = self.points.len();
        let springs = self.springs.len();
        let areas = self.areas.len();
        let triangles = self.triangles.len();
        let bones = self.bones.len();
        let covers = self.bone_covers.len();
        let point = |index: usize| moved(index, points, MISSING_POINT);
        let spring = |index: usize| moved(index, springs, MISSING_SPRING);
        let bone = |index: usize| moved(index, bones, MISSING_BONE);
        let anchor = |index: usize| moved(index, points, MISSING_ANCHOR);
        for mut added in other.points {
            added.ghost_of = point(added.ghost_of);
            added.body += first as u8;
            self.points.push(added);
        }
        for mut hold in other.point_holds {
            hold.posture.bones = hold.posture.bones.map(bone);
            self.point_holds.push(hold);
        }
        for mut added in other.springs {
            (added.a, added.b) = (point(added.a), point(added.b));
            self.springs.push(added);
        }
        for mut wear in other.spring_wear {
            wear.twin = spring(wear.twin);
            self.spring_wear.push(wear);
        }
        for mut area in other.areas {
            (area.a, area.b, area.c) = (point(area.a), point(area.b), point(area.c));
            (area.edge_ab, area.edge_bc, area.edge_ca) = (
                spring(area.edge_ab),
                spring(area.edge_bc),
                spring(area.edge_ca),
            );
            self.areas.push(area);
        }
        for mut tie in other.attachments {
            (tie.skin_point, tie.muscle_point) = (point(tie.skin_point), point(tie.muscle_point));
            self.attachments.push(tie);
        }
        for mut triangle in other.triangles {
            (triangle.a, triangle.b, triangle.c) =
                (point(triangle.a), point(triangle.b), point(triangle.c));
            (triangle.edge_ab, triangle.edge_bc, triangle.edge_ca) = (
                spring(triangle.edge_ab),
                spring(triangle.edge_bc),
                spring(triangle.edge_ca),
            );
            if let Some(cut) = &mut triangle.cut {
                cut.partner += triangles;
            }
            self.triangles.push(triangle);
        }
        for mut added in other.bones {
            added.cover = moved(added.cover, covers, MISSING_COVER);
            added.body += first as u8;
            self.bones.push(added);
        }
        self.bone_covers.extend(other.bone_covers);
        for mut tie in other.bone_attachments {
            (tie.point, tie.bone) = (point(tie.point), bone(tie.bone));
            self.bone_attachments.push(tie);
        }
        for mut joint in other.bone_joints {
            (joint.a, joint.b) = (bone(joint.a), bone(joint.b));
            self.bone_joints.push(joint);
        }
        for mut tendon in other.tendons {
            (tendon.from.bone, tendon.to.bone) = (bone(tendon.from.bone), bone(tendon.to.bone));
            self.tendons.push(tendon);
        }
        for mut vessel in other.vessels {
            (vessel.anchor_a, vessel.anchor_b) = (anchor(vessel.anchor_a), anchor(vessel.anchor_b));
            self.vessels.push(vessel);
        }
        for mut cavity in other.cavities {
            for index in &mut cavity.area_indices {
                *index += areas;
            }
            self.cavities.push(cavity);
        }
        for mut organ in other.organs {
            organ.anchor_point = anchor(organ.anchor_point);
            self.organs.push(organ);
        }
        for mut outline in other.outlines {
            outline.move_indices(points, springs);
            self.outlines.push(outline);
        }
        for mut hold in other.hip_holds {
            hold.bone = bone(hold.bone);
            self.hip_holds.push(hold);
        }
        self.open_skin
            .extend(other.open_skin.into_iter().map(spring));
        self.flesh.extend(other.flesh);
        self.bodies.extend(other.bodies);
        self.topology_version += 1;
        first
    }

    /// This world mirrored left to right about `middle`, across the world:
    /// every point and bone where its mirror image lies, moving as its
    /// mirror image moves, and everything that holds the body together
    /// turned to match, in the same order. Played the mirror image of any
    /// play, it plays the mirror image of what this world does, up to how
    /// differently the same numbers round on the other side (`strike_scenarios
    /// --twin mirrored-body`).
    pub fn mirrored(&self, middle: f64) -> World {
        let mut world = self.clone();
        let place = |at: Vec2| Vec2 {
            x: 2.0 * middle - at.x,
            y: at.y,
        };
        let motion = |by: Vec2| Vec2 { x: -by.x, y: by.y };
        for point in &mut world.points {
            point.position = place(point.position);
            point.previous = place(point.previous);
            point.home = place(point.home);
        }
        for state in &mut world.bodies {
            state.mirrored = !state.mirrored;
        }
        // What is measured across a bone, a quarter turn counterclockwise
        // from it, turns the other way in the mirror; so do angles and turns.
        for hold in &mut world.point_holds {
            hold.posture.sigma = hold.posture.sigma.map(|sigma| -sigma);
            hold.posture_impulse = motion(hold.posture_impulse);
            hold.wrap = motion(hold.wrap);
        }
        for area in &mut world.areas {
            area.rest_area = -area.rest_area;
            area.impulse = -area.impulse;
        }
        for bone in &mut world.bones {
            for at in [
                &mut bone.a,
                &mut bone.b,
                &mut bone.previous_a,
                &mut bone.previous_b,
                &mut bone.home_a,
                &mut bone.home_b,
            ] {
                *at = place(*at);
            }
            bone.break_normal = motion(bone.break_normal);
            bone.broken_start_normal = motion(bone.broken_start_normal);
            bone.broken_end_normal = motion(bone.broken_end_normal);
            bone.angular_velocity = -bone.angular_velocity;
        }
        for cover in &mut world.bone_covers {
            cover.mirror();
        }
        for tie in &mut world.bone_attachments {
            tie.offset = motion(tie.offset);
            tie.across = -tie.across;
            tie.impulse = motion(tie.impulse);
        }
        for joint in &mut world.bone_joints {
            joint.hold.y = -joint.hold.y;
            joint.rest_angle = -joint.rest_angle;
            (joint.min_angle, joint.max_angle) = (-joint.max_angle, -joint.min_angle);
            joint.post_fracture_rest_angle = -joint.post_fracture_rest_angle;
            joint.hold_impulse = motion(joint.hold_impulse);
            joint.angular_impulse = -joint.angular_impulse;
            joint.drive.aim = -joint.drive.aim;
            joint.drive.swing = -joint.drive.swing;
            joint.drive_impulse = -joint.drive_impulse;
        }
        for tendon in &mut world.tendons {
            tendon.from.sigma = -tendon.from.sigma;
            tendon.to.sigma = -tendon.to.sigma;
        }
        for vessel in &mut world.vessels {
            (vessel.a, vessel.b) = (place(vessel.a), place(vessel.b));
            (vessel.offset_a, vessel.offset_b) = (motion(vessel.offset_a), motion(vessel.offset_b));
        }
        for cavity in &mut world.cavities {
            cavity.centroid = place(cavity.centroid);
        }
        for organ in &mut world.organs {
            organ.center = place(organ.center);
            organ.anchor_offset = motion(organ.anchor_offset);
        }
        for outline in &mut world.outlines {
            outline.mirror();
        }
        for hold in &mut world.hip_holds {
            hold.home = place(hold.home);
            hold.aim = motion(hold.aim);
            hold.impulse = motion(hold.impulse);
        }
        world
    }
}

/// Two fighters side by side in one world, the viewer's left one first,
/// each where [`crate::fighter_frames`] puts it: the scenarios' body built
/// once and moved there (`World::move_across`), and its mirror image beside
/// it, so the two face each other as mirror images and are solved alike.
pub fn create_fighters(materials: Materials) -> World {
    create_fighters_at(materials, crate::FIGHTER_SPACING)
}

/// Two fighters as [`create_fighters`] builds them, `spacing` body heights
/// apart.
pub fn create_fighters_at(materials: Materials, spacing: f64) -> World {
    fighters_from(
        &create_layered_body_in(crate::world_frame(), materials),
        spacing,
    )
}

/// Two fighters made of `body`, one body built where the scenarios stand it
/// ([`crate::world_frame`]): it moves to where the first fighter stands and
/// its mirror image to where the second does, `spacing` body heights apart.
pub fn fighters_from(body: &World, spacing: f64) -> World {
    let built = crate::world_frame();
    let [left, right] = crate::fighter_frames_at(spacing);
    let mut twin = body.mirrored(built.origin.x);
    twin.move_across(right.origin.x - built.origin.x);
    let mut world = body.clone();
    world.move_across(left.origin.x - built.origin.x);
    world.add_body(twin);
    world
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::SCENARIO_WIDTH;

    /// The two fighters stand side by side as mirror images, and stay so at
    /// rest: the right one is the left one's mirror image, point for point,
    /// solved in the same order.
    #[test]
    fn two_fighters_stand_as_mirror_images() {
        let mut world = create_fighters(Materials::default());
        assert_eq!(world.body_count(), 2);
        let half = world.points().len() / 2;
        let bones = world.bones().len() / 2;
        let [left, right] = crate::fighter_frames();
        let middle = (left.origin.x + right.origin.x) * 0.5;
        let apart = |world: &World| {
            let points = (0..half).map(|index| {
                let (a, b) = (
                    world.points()[index].position,
                    world.points()[half + index].position,
                );
                (a.x + b.x - 2.0 * middle).abs().max((a.y - b.y).abs())
            });
            let bone_ends = (0..bones).map(|index| {
                let (a, b) = (world.bones()[index], world.bones()[bones + index]);
                (a.a.x + b.a.x - 2.0 * middle)
                    .abs()
                    .max((a.a.y - b.a.y).abs())
                    .max((a.b.x + b.b.x - 2.0 * middle).abs())
            });
            points.chain(bone_ends).fold(0.0, f64::max)
        };
        assert!(
            world.points()[..half].iter().all(|point| point.body == 0)
                && world.points()[half..].iter().all(|point| point.body == 1),
            "each fighter's flesh is marked as its own"
        );
        assert!(
            apart(&world) < 1.0e-6,
            "built {:.2e} px off mirror images",
            apart(&world)
        );
        let dt = world.materials().fixed_dt;
        for _ in 0..120 {
            world.step(dt, &InputState::default(), SCENARIO_WIDTH);
        }
        assert!(
            apart(&world) < 1.0e-3,
            "after two seconds at rest the fighters stand {:.2e} px off mirror images",
            apart(&world)
        );
        let still = world
            .points()
            .iter()
            .map(|point| distance(point.position, point.previous) / dt)
            .fold(0.0, f64::max);
        assert!(still < 5.0, "a fighter at rest still moves {still:.1} px/s");
    }
}
