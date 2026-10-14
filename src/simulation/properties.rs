//! Properties the engine's geometry and solver math must have for any input,
//! checked on many generated cases (proptest), which shrinks a failing case
//! to the simplest one that still fails before reporting it.

use super::*;
use proptest::prelude::*;

/// A point anywhere in a large window.
fn point() -> impl Strategy<Value = Vec2> {
    (-600.0..600.0f64, -600.0..600.0f64).prop_map(|(x, y)| Vec2 { x, y })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// The closest points of two segments lie on them, as far apart as the
    /// distance given, and no pair of endpoints lies closer.
    #[test]
    fn closest_points_lie_on_their_segments(a0 in point(), a1 in point(), b0 in point(), b1 in point()) {
        let closest = closest_segment_points(a0, a1, b0, b1);
        prop_assert!((0.0..=1.0).contains(&closest.t_a), "t_a {}", closest.t_a);
        prop_assert!((0.0..=1.0).contains(&closest.t_b), "t_b {}", closest.t_b);
        prop_assert!(distance(lerp(a0, a1, closest.t_a), closest.point_a) < 1e-6);
        prop_assert!(distance(lerp(b0, b1, closest.t_b), closest.point_b) < 1e-6);
        prop_assert!((distance(closest.point_a, closest.point_b) - closest.distance).abs() < 1e-6);
        for (p, q) in [(a0, b0), (a0, b1), (a1, b0), (a1, b1)] {
            prop_assert!(closest.distance <= distance(p, q) + 1e-6);
        }
    }

    /// An angle wraps into one turn, from -pi to pi, pointing the same way.
    #[test]
    fn angles_wrap_into_one_turn(angle in -60.0..60.0f64) {
        let wrapped = wrap_angle(angle);
        prop_assert!((-PI..=PI).contains(&wrapped), "{angle} wrapped to {wrapped}");
        prop_assert!((libm::sin(wrapped) - libm::sin(angle)).abs() < 1e-9);
        prop_assert!((libm::cos(wrapped) - libm::cos(angle)).abs() < 1e-9);
    }

    /// A soft constraint never pushes harder than a rigid one would, nor
    /// gives back more of what it gave before than it gave, for any
    /// stiffness, damping, and substep (Box2D's `b2MakeSoft`).
    #[test]
    fn a_soft_constraint_is_never_harder_than_a_rigid_one(
        hertz in 0.01..240.0f64,
        zeta in 0.0..10.0f64,
        substep in 1.0e-4..0.05f64,
        mass in 0.01..1000.0f64,
        speed in -5000.0..5000.0f64,
        given in -5000.0..5000.0f64,
    ) {
        let softness = soft::Softness::new(2.0 * PI * hertz, zeta, substep);
        // Moving with no error, it stops at most the whole motion, against it.
        let stop = softness.impulse(mass, speed, 0.0, 0.0);
        prop_assert!(stop.abs() <= mass * speed.abs() * (1.0 + 1e-12));
        prop_assert!(stop * speed <= 0.0);
        // At rest with no error, it lets go of at most what it gave.
        let release = softness.impulse(mass, 0.0, 0.0, given);
        prop_assert!(release.abs() <= given.abs() * (1.0 + 1e-12));
        prop_assert!(release * given <= 0.0);
    }

    /// Where a joint holds its second bone, written along and across the
    /// first bone, leads back to the anchor it was taken from.
    #[test]
    fn a_joint_hold_leads_back_to_its_anchor(a in point(), b in point(), t in 0.0..1.0f64, anchor in point()) {
        prop_assume!(distance(a, b) > 1.0);
        let bone = BoneSegment {
            a,
            b,
            rest_length: distance(a, b),
            ..BoneSegment::default()
        };
        let hold = joint_hold_offset(&bone, t, anchor, 0.0);
        let along = normalized(subtract(b, a), Vec2 { x: 0.0, y: 1.0 });
        let across = Vec2 { x: -along.y, y: along.x };
        let back = add(bone_point(bone, t), add(scale(along, hold.x), scale(across, hold.y)));
        prop_assert!(distance(back, anchor) < 1e-6, "back at {back:?}, anchor {anchor:?}");
    }
}
