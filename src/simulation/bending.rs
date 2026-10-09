//! Where and when a load breaks a bone: by how hard it bends the bone, not by
//! its force alone. A bone fails where the bending moment across it is
//! worst, and a load bends a bone by its lever on the supports that hold the
//! bone: a load F that lands a and b from the two supports around it bends
//! the bone by F a b / (a + b) under the load (three-point bending), so a
//! blow to the middle of a bone held at both ends bends it most and the same
//! blow near a joint far less, and a load pressed along a stretch of a bone,
//! as a bat lying along a forearm presses it, bends it less than the same
//! load at one point: half as hard if spread over the whole span. Thigh
//! bones struck in the middle this way broke under the blow in all 45 tests
//! of Kennedy et al. (2004), and fracture risk follows the moment
//! (`docs/INJURY_REFERENCE.md`). This is NVIDIA Blast's
//! stress solver (BSD-3) reduced to a single bone: Blast breaks a bond by the
//! stress that the impulses on it and the inertia around it put across it,
//! where here each bone is a beam on its joints, and a part of it past its
//! last joint is held only by its own inertia.
//!
//! A bending break far past a bone's strength can break out a wedge as well,
//! a butterfly fragment, classically with its broad side where the blow
//! landed (Messerer's wedge); experiments find wedges in a minority of
//! bending breaks (Isa et al. 2021).

use super::*;

/// How far past its strength a bending break must go to break out a
/// butterfly wedge too, as a share of the strength.
const BUTTERFLY_OVERLOAD: f64 = 1.0;
/// A butterfly wedge's length and thickness, in bone radii.
const BUTTERFLY_LENGTH: f64 = 2.6;
const BUTTERFLY_RADIUS: f64 = 0.5;

impl BoneSegment {
    /// Bears a `load` that bends the bone, at `t` along it, toward breaking
    /// it; the break comes where the bone was bent hardest.
    pub(super) fn bear_break_load(&mut self, load: f64, t: f64) {
        if load > self.break_load {
            self.break_load = load;
            self.break_t = t.clamp(0.0, 1.0);
        }
    }
}

impl World {
    /// How hard a load at `t` along bone `index` bends it (see `bending`).
    pub(super) fn bending_share(&self, index: usize, t: f64) -> f64 {
        self.bending(index, t, t).0
    }

    /// How hard a load spread evenly from `from` to `to` along bone `index`
    /// bends it, against the same load at one point in the middle of a span
    /// held at both ends, and where along the bone it bends it most (see the
    /// module notes). Between two supports the bone is a beam on them, and a
    /// load spread along it bends it less than the same load at one point;
    /// past its last support toward a free end, as a hand's fingers, only
    /// the bone's inertia holds it, so it bends less; a bone held nowhere
    /// bends less still.
    pub(super) fn bending(&self, index: usize, from: f64, to: f64) -> (f64, f64) {
        let bone = &self.bones[index];
        let (from, to) = (from.min(to).clamp(0.0, 1.0), from.max(to).clamp(0.0, 1.0));
        let t = (from + to) * 0.5;
        let mut below: Option<f64> = None;
        let mut above: Option<f64> = None;
        let mut support = |at: f64| {
            if at <= t {
                below = Some(below.map_or(at, |nearest| nearest.max(at)));
            }
            if at >= t {
                above = Some(above.map_or(at, |nearest| nearest.min(at)));
            }
        };
        if bone.pinned {
            support(0.0);
            support(1.0);
        }
        // A rib runs from the spine round the side of the chest to its
        // cartilage on the breastbone. Seen from the front, the figure's rib
        // runs from the spine out to the side, the middle of the real one,
        // so the rib's other support lies as far again past its outer end.
        if bone.kind == BoneKind::Rib && !bone.broken_end {
            support(2.0);
        }
        for joint in &self.bone_joints {
            if joint.broken {
                continue;
            }
            if joint.a == index {
                support(joint.t_a);
            }
            if joint.b == index {
                support(joint.t_b);
            }
        }
        match (below, above) {
            (Some(low), Some(high)) => {
                let span = high - low;
                if span <= EPSILON {
                    return (0.0, t);
                }
                // The load F spread evenly over [u1, u2] of a span from 0 to
                // 1, its middle at m, bends the span most at u1 + (1 - m) c,
                // by F (1 - m) (u1 + (1 - m) c / 2), where c = u2 - u1: the
                // three-point F m (1 - m) when the load is at one point.
                let u1 = ((from - low) / span).clamp(0.0, 1.0);
                let u2 = ((to - low) / span).clamp(0.0, 1.0);
                let (spread, middle) = (u2 - u1, (u1 + u2) * 0.5);
                let share = 4.0 * (1.0 - middle) * (u1 + (1.0 - middle) * spread * 0.5);
                (share, low + (u1 + (1.0 - middle) * spread) * span)
            }
            (Some(low), None) => (overhang_share((t - low) / (1.0 - low).max(EPSILON)), t),
            (None, Some(high)) => (overhang_share((high - t) / high.max(EPSILON)), t),
            (None, None) => (8.0 * t * t * (1.0 - t) * (1.0 - t), t),
        }
    }

    /// Breaks a wedge out of the struck side of a bending break at `crack`
    /// along bone direction `dir`, the blow driving the bone along `normal`.
    pub(super) fn chip_butterfly(
        &mut self,
        old: &BoneSegment,
        crack: Vec2,
        previous_crack: Vec2,
        home_crack: Vec2,
        dir: Vec2,
        normal: Vec2,
    ) {
        let half = old.radius * BUTTERFLY_LENGTH * 0.5;
        // The wedge's broad side is the struck surface, so it sits on the side
        // the blow came from, against the bone's line.
        let side = scale(normal, -old.radius * 0.45);
        let along = scale(dir, half);
        let center = add(crack, side);
        let previous_center = add(previous_crack, side);
        let home_center = add(home_crack, side);
        let mut wedge = BoneSegment {
            kind: old.kind,
            part: old.part,
            a: subtract(center, along),
            b: add(center, along),
            previous_a: subtract(previous_center, along),
            previous_b: add(previous_center, along),
            home_a: subtract(home_center, along),
            home_b: add(home_center, along),
            radius: (old.radius * BUTTERFLY_RADIUS).max(2.0),
            fracture_impulse: old.fracture_impulse,
            load: old.load * 0.4,
            fractured: true,
            broken_start: true,
            broken_end: true,
            broken_start_normal: butterfly_cap(normal, dir, 1.0),
            broken_end_normal: butterfly_cap(normal, dir, -1.0),
            fracture_generation: self.materials.max_bone_fracture_depth,
            splinter: true,
            open_break: old.open_break,
            ..BoneSegment::default()
        };
        wedge.rest_length = distance(wedge.a, wedge.b).max(EPSILON);
        self.bones.push(wedge);
        self.stats.butterfly_fragments += 1;
    }

    /// Whether a bending break of `old` loaded `overload` past its strength,
    /// driven by a blow along `normal`, breaks out a butterfly wedge too.
    pub(super) fn breaks_out_butterfly(
        &self,
        old: &BoneSegment,
        overload: f64,
        normal: Vec2,
    ) -> bool {
        let length = distance(old.a, old.b);
        hypot(normal.x, normal.y) > EPSILON
            && overload >= BUTTERFLY_OVERLOAD
            && old.part.is_long_bone()
            && !old.splinter
            && length >= old.radius * 6.0
            && self.free_fragment_count() < self.materials.max_active_bone_fragments
    }
}

/// The stretch of `bone`, as a span of `t` from `a` to `b`, that a tool whose
/// axis runs from `axis_start` to `axis_end` presses, reaching `reach` from
/// its axis: a tool crossing a bone presses about one point of it, `t`, and
/// one lying along it presses all it overlaps.
pub(super) fn pressed_stretch(
    axis_start: Vec2,
    axis_end: Vec2,
    reach: f64,
    bone: &BoneSegment,
    t: f64,
) -> (f64, f64) {
    const SAMPLES: usize = 16;
    let mut stretch: Option<(f64, f64)> = None;
    for sample in 0..=SAMPLES {
        let at = sample as f64 / SAMPLES as f64;
        let point = lerp(bone.a, bone.b, at);
        if distance_to_segment(point, axis_start, axis_end) <= reach {
            stretch = Some(stretch.map_or((at, at), |(from, to)| (from.min(at), to.max(at))));
        }
    }
    match stretch {
        // A stretch a sample or two long is the tool crossing the bone.
        Some((from, to)) if to - from > 2.0 / SAMPLES as f64 => (from, to),
        _ => (t, t),
    }
}

/// The line of a butterfly break's crack on the side `side` (1 toward the
/// bone's start, -1 toward its end) of the wedge: from the crack's point on
/// the far side of the bone back to the wedge's corner on the struck side,
/// which lies half the wedge's length along the bone.
pub(super) fn butterfly_cap(normal: Vec2, dir: Vec2, side: f64) -> Vec2 {
    normalized(
        add(normal, scale(dir, side * BUTTERFLY_LENGTH * 0.25)),
        normal,
    )
}

/// How hard a load `u` of the way from a bone's last support toward its free
/// end bends it, against the same load at the middle of a span held at both
/// ends. The overhang turns about the support, and only its inertia resists
/// the blow: the moment under the load is F L u (1 - 1.5 u + 0.5 u^3) for an
/// overhang of length L with its mass spread evenly.
fn overhang_share(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    4.0 * u * (1.0 - 1.5 * u + 0.5 * u * u * u)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An upper arm held at the shoulder by a pinned girdle and at the elbow
    /// by a forearm, which ends in a hand with a free end.
    fn arm() -> (World, usize, usize, usize) {
        let mut world = World::new(Materials {
            gravity: 0.0,
            ..Materials::default()
        });
        let girdle = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 300.0, y: 100.0 },
            6.0,
            2000.0,
            true,
        );
        let upper = world.add_bone_segment(
            Vec2 { x: 300.0, y: 100.0 },
            Vec2 { x: 300.0, y: 220.0 },
            5.7,
            2000.0,
            false,
        );
        world.bones[upper].part = BonePart::UpperArm;
        let forearm = world.add_bone_segment(
            Vec2 { x: 300.0, y: 228.0 },
            Vec2 { x: 300.0, y: 330.0 },
            4.8,
            2000.0,
            false,
        );
        world.bones[forearm].part = BonePart::Forearm;
        let hand = world.add_bone_segment(
            Vec2 { x: 300.0, y: 338.0 },
            Vec2 { x: 300.0, y: 380.0 },
            3.6,
            2000.0,
            false,
        );
        world.add_bone_joint(girdle, 1.0, upper, 0.0, -1.0, 1.0);
        world.add_bone_joint(upper, 1.0, forearm, 0.0, -1.0, 1.0);
        world.add_bone_joint(forearm, 1.0, hand, 0.0, -1.0, 1.0);
        (world, upper, forearm, hand)
    }

    #[test]
    fn a_blow_bends_a_bone_most_midway_between_its_joints() {
        let (world, upper, _, _) = arm();
        let middle = world.bending_share(upper, 0.5);
        assert!((middle - 1.0).abs() < 1.0e-12, "{middle}");
        let quarter = world.bending_share(upper, 0.25);
        assert!((quarter - 0.75).abs() < 1.0e-12, "{quarter}");
        let near_joint = world.bending_share(upper, 0.05);
        assert!(
            near_joint < 0.2,
            "a blow beside a joint bends little: {near_joint}"
        );
        assert_eq!(world.bending_share(upper, 0.0), 0.0);
    }

    #[test]
    fn a_load_spread_along_a_bone_bends_it_less() {
        let (world, upper, _, _) = arm();
        let (whole, worst) = world.bending(upper, 0.0, 1.0);
        assert!(
            (whole - 0.5).abs() < 1.0e-12,
            "w L^2 / 8 against F L / 4: {whole}"
        );
        assert!((worst - 0.5).abs() < 1.0e-12, "{worst}");
        let (part, worst) = world.bending(upper, 0.1, 0.5);
        assert!(part < world.bending_share(upper, 0.3), "{part}");
        assert!(worst > 0.1 && worst < 0.5, "{worst}");
    }

    #[test]
    fn a_tool_along_a_bone_presses_all_of_it_and_one_across_it_one_point() {
        let (world, upper, _, _) = arm();
        let bone = world.bones[upper];
        // The arm runs down from (300, 100) to (300, 220).
        let along = pressed_stretch(
            Vec2 { x: 285.0, y: 90.0 },
            Vec2 { x: 285.0, y: 230.0 },
            20.0,
            &bone,
            1.0,
        );
        assert!(along.0 < 0.1 && along.1 > 0.9, "{along:?}");
        let across = pressed_stretch(
            Vec2 { x: 260.0, y: 160.0 },
            Vec2 { x: 340.0, y: 160.0 },
            12.0,
            &bone,
            0.5,
        );
        assert_eq!(across, (0.5, 0.5));
    }

    #[test]
    fn a_free_end_bends_less_than_a_held_span() {
        let (world, _, _, hand) = arm();
        let middle = world.bending_share(hand, 0.5);
        assert!(middle < 0.7 && middle > 0.5, "{middle}");
        let tip = world.bending_share(hand, 1.0);
        assert!(
            tip.abs() < 1.0e-12,
            "a blow on the free tip only turns the bone: {tip}"
        );
    }

    #[test]
    fn a_rib_bends_most_at_the_side_of_the_chest() {
        let mut world = World::new(Materials::default());
        let spine = world.add_bone_segment(
            Vec2 { x: 300.0, y: 100.0 },
            Vec2 { x: 300.0, y: 300.0 },
            7.0,
            2000.0,
            true,
        );
        let rib = world.add_bone_segment_with_kind(
            Vec2 { x: 300.0, y: 150.0 },
            Vec2 { x: 380.0, y: 170.0 },
            3.4,
            1200.0,
            false,
            BoneKind::Rib,
        );
        world.add_bone_joint(spine, 0.25, rib, 0.0, -0.4, 0.4);
        // The figure's rib is the half from the spine to the side of the
        // chest, so a blow on its outer end lands mid-rib.
        let side = world.bending_share(rib, 1.0);
        assert!((side - 1.0).abs() < 1.0e-12, "{side}");
        let middle = world.bending_share(rib, 0.5);
        assert!((middle - 0.75).abs() < 1.0e-12, "{middle}");
    }

    #[test]
    fn the_hardest_bending_load_sets_where_the_bone_breaks() {
        let (mut world, upper, _, _) = arm();
        let mut bone = world.bones[upper];
        bone.bear_break_load(900.0, 0.3);
        bone.bear_break_load(700.0, 0.6);
        assert_eq!(bone.break_t, 0.3);
        bone.bear_break_load(2500.0, 0.7);
        assert_eq!(bone.break_load, 2500.0);
        assert_eq!(bone.break_t, 0.7);
        world.bones[upper] = bone;
        let length = distance(bone.a, bone.b);
        world.solve_bones();
        let first = world.bones[upper];
        assert!(first.fractured, "the load is past the bone's strength");
        let broke_at = distance(first.a, first.b) / length;
        assert!(
            (broke_at - 0.7).abs() < 0.08,
            "the bone should break where it was bent hardest: {broke_at:.2}"
        );
    }

    #[test]
    fn a_hard_bending_break_chips_a_butterfly_on_the_struck_side() {
        let (mut world, upper, _, _) = arm();
        let bone = world.bones[upper];
        let crack = lerp(bone.a, bone.b, 0.5);
        // A blow from the left drives the bone right.
        let push = Vec2 { x: 1.0, y: 0.0 };
        world.bones[upper].break_load = bone.fracture_impulse * 2.2;
        world.fracture_bone(upper, 0.5, push, bone.fracture_impulse * 2.2);
        assert_eq!(world.stats.butterfly_fragments, 1);
        let wedge = world
            .bones
            .iter()
            .find(|piece| piece.splinter && piece.part == BonePart::UpperArm)
            .expect("a wedge breaks out");
        let center = lerp(wedge.a, wedge.b, 0.5);
        assert!(
            center.x < crack.x,
            "the wedge lies on the struck side: {center:?} against the crack {crack:?}"
        );
        assert!(wedge.radius < bone.radius);
    }

    #[test]
    fn a_break_just_past_strength_leaves_no_butterfly() {
        let (mut world, upper, _, _) = arm();
        let bone = world.bones[upper];
        world.bones[upper].break_load = bone.fracture_impulse * 1.3;
        world.fracture_bone(
            upper,
            0.5,
            Vec2 { x: 1.0, y: 0.0 },
            bone.fracture_impulse * 1.3,
        );
        assert_eq!(world.stats.butterfly_fragments, 0);
        assert!(world.bones[upper].fractured);
    }
}
