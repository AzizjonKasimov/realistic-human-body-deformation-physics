//! The skin's silhouette edge, which the skin is shaded in from and
//! where a wound meets intact skin.

use super::*;

/// A skin spring on the body's outline, with the third corner of its triangle
/// so the inward side is known as the body deforms.
#[derive(Clone, Copy)]
pub(super) struct OutlineEdge {
    pub(super) spring: usize,
    pub(super) inner_point: usize,
}

/// The skin's silhouette edge, rebuilt with each new body and each time a
/// cut changes the mesh.
#[derive(Default)]
pub struct SkinRim {
    /// The mesh it was built from (`World::topology_version`).
    version: u64,
    pub(super) edges: Vec<OutlineEdge>,
    /// Per point: rest distance across the body along the inward direction,
    /// which caps the shading strip so it never spills past a thin limb.
    pub(super) reach: Vec<f32>,
    /// Per spring: the skin triangles on either side, `NO_TRIANGLE` where there
    /// is none, so wounds can find where intact skin meets an opening.
    pub(super) edge_triangles: Vec<[usize; 2]>,
}

impl SkinRim {
    /// The silhouette edge of `world`'s skin as its mesh is now.
    pub fn of(world: &rp::World) -> SkinRim {
        skin_rim(world)
    }

    /// Builds it again if `world`'s mesh changed since: a cut that opened
    /// rejoined the mesh.
    pub fn refresh(&mut self, world: &rp::World) {
        if self.version != world.topology_version() {
            *self = skin_rim(world);
        }
    }
}

pub(super) const NO_TRIANGLE: usize = usize::MAX;

/// Unit normal of an outline spring pointing into the body.
pub(super) fn inward_normal(
    points: &[rp::Point],
    spring: rp::Spring,
    inner_point: usize,
    position: impl Fn(&rp::Point) -> rp::Vec2,
) -> rp::Vec2 {
    let a = position(&points[spring.a]);
    let b = position(&points[spring.b]);
    let along = normalized(sub(b, a), rp::Vec2 { x: 1.0, y: 0.0 });
    let normal = rp::Vec2 {
        x: -along.y,
        y: along.x,
    };
    let toward_inside = sub(position(&points[inner_point]), a);
    if normal.x * toward_inside.x + normal.y * toward_inside.y < 0.0 {
        scale(normal, -1.0)
    } else {
        normal
    }
}

/// The skin's outline: springs that border only one skin triangle, and how far
/// across the body each outline point can see along its inward direction.
fn skin_rim(world: &rp::World) -> SkinRim {
    let points = world.points();
    let springs = world.springs();
    let mut owners: Vec<(u8, usize)> = vec![(0, 0); springs.len()];
    let mut edge_triangles = vec![[NO_TRIANGLE; 2]; springs.len()];
    for (index, triangle) in world.triangles().iter().enumerate() {
        if triangle.layer != rp::TissueLayer::Skin {
            continue;
        }
        for (edge, opposite) in [
            (triangle.edge_ab, triangle.c),
            (triangle.edge_bc, triangle.a),
            (triangle.edge_ca, triangle.b),
        ] {
            if let Some(owner) = owners.get_mut(edge) {
                *owner = (owner.0.saturating_add(1), opposite);
            }
            if let Some(sides) = edge_triangles.get_mut(edge) {
                let free = usize::from(sides[0] != NO_TRIANGLE);
                sides[free.min(1)] = index;
            }
        }
    }
    // A tear's lips border one triangle too, but they are wound, not
    // outline, and so is a side of a cut's piece that runs to a ghost.
    let edges: Vec<OutlineEdge> = owners
        .iter()
        .enumerate()
        .filter_map(|(spring, &(count, inner_point))| {
            let spring_of = springs[spring];
            (count == 1
                && world.spring_wear()[spring].twin == rp::MISSING_SPRING
                && !points[spring_of.a].is_ghost()
                && !points[spring_of.b].is_ghost())
            .then_some(OutlineEdge {
                spring,
                inner_point,
            })
        })
        .collect();

    let mut reach = vec![0.0f32; points.len()];
    for edge in &edges {
        let spring = springs[edge.spring];
        let normal = inward_normal(points, spring, edge.inner_point, |p| p.home);
        for index in [spring.a, spring.b] {
            let origin = points[index].home;
            // Nearest crossing of the inward ray with another outline edge.
            let mut nearest: f64 = 60.0;
            for other in &edges {
                let other_spring = springs[other.spring];
                if other_spring.a == index || other_spring.b == index {
                    continue;
                }
                if let Some(t) = ray_hits_segment(
                    origin,
                    normal,
                    points[other_spring.a].home,
                    points[other_spring.b].home,
                ) {
                    nearest = nearest.min(t);
                }
            }
            reach[index] = reach[index].max(nearest as f32);
        }
    }
    SkinRim {
        version: world.topology_version(),
        edges,
        reach,
        edge_triangles,
    }
}

/// Distance along the ray to segment `a`-`b`, if they cross.
pub(super) fn ray_hits_segment(
    origin: rp::Vec2,
    direction: rp::Vec2,
    a: rp::Vec2,
    b: rp::Vec2,
) -> Option<f64> {
    let segment = sub(b, a);
    let denominator = direction.x * segment.y - direction.y * segment.x;
    if denominator.abs() < 1.0e-9 {
        return None;
    }
    let offset = sub(a, origin);
    let t = (offset.x * segment.y - offset.y * segment.x) / denominator;
    let s = (offset.x * direction.y - offset.y * direction.x) / denominator;
    (t > 0.5 && (0.0..=1.0).contains(&s)).then_some(t)
}
