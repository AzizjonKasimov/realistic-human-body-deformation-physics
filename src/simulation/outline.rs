//! Contact between parts of the body's own outline. The skin and muscle sheets
//! are flat meshes, each bounded by one outline loop that runs down the outside
//! of an arm, round the hand, up its inside, and down the side of the chest, so
//! the inside of the arm and the side of the chest are stretches of the same
//! outline facing each other across a gap. Nothing else keeps them apart: an
//! arm driven inward would slide over the chest and stretch the skin at the
//! armpit until it tore. Here an outline point that passes behind a stretch of
//! outline far from it along the loop is pushed back out, and pushes that
//! stretch in, as an arm pressed against the chest, a hand against a thigh, or
//! one leg against the other do.

use super::*;

/// Outline points this close along the loop are neighbors, which the
/// outline's own springs keep in place.
const OUTLINE_NEIGHBORS: usize = 6;
/// How deep behind a stretch of outline, in point spacings, a point can be
/// pushed back out from: about as far as a limb moves in one step.
const OUTLINE_CONTACT_DEPTH: f64 = 2.5;
/// How far apart, in point spacings, a point and a stretch of outline may be
/// at the start of a step to be checked during it.
const OUTLINE_CONTACT_REACH: f64 = 1.6;
/// Share of an overlap each solver pass removes.
const OUTLINE_CONTACT_STIFFNESS: f64 = 0.8;

/// One closed outline of a tissue sheet, in order along the loop.
#[derive(Clone, Debug)]
pub(super) struct OutlineLoop {
    points: Vec<usize>,
    /// The spring along each outline edge, from each point to the next; a
    /// torn edge is no longer a surface anything can press against.
    edge_springs: Vec<Option<usize>>,
    /// Turns an edge's left-hand perpendicular into its outward normal.
    outward: f64,
}

/// A point that may touch a stretch of outline this step: the edge from loop
/// position `edge` to the next.
#[derive(Clone, Copy, Debug)]
pub(super) struct OutlinePair {
    outline: usize,
    point: usize,
    edge: usize,
}

impl World {
    /// Adds a closed outline of a tissue sheet, given in order along it.
    pub(super) fn add_outline_loop(&mut self, points: Vec<usize>) {
        if points.len() < 3 {
            return;
        }
        let mut doubled_area = 0.0;
        for (index, &a) in points.iter().enumerate() {
            let b = points[(index + 1) % points.len()];
            let (pa, pb) = (self.points[a].home, self.points[b].home);
            doubled_area += pa.x * pb.y - pb.x * pa.y;
        }
        let spring_between: HashMap<(usize, usize), usize> = self
            .springs
            .iter()
            .enumerate()
            .map(|(index, spring)| ((spring.a.min(spring.b), spring.a.max(spring.b)), index))
            .collect();
        let edge_springs = (0..points.len())
            .map(|index| {
                let (a, b) = (points[index], points[(index + 1) % points.len()]);
                spring_between.get(&(a.min(b), a.max(b))).copied()
            })
            .collect();
        self.outlines.push(OutlineLoop {
            points,
            edge_springs,
            outward: if doubled_area > 0.0 { -1.0 } else { 1.0 },
        });
    }

    /// Finds the points and stretches of outline close enough to touch on
    /// this step, so each solver pass checks only those.
    pub(super) fn gather_outline_pairs(&mut self) {
        self.outline_pairs.clear();
        if self.outlines.is_empty() {
            return;
        }
        let spacing = self.materials.point_spacing.max(1.0);
        let reach = spacing * OUTLINE_CONTACT_REACH;
        let cell = spacing * 2.0;
        // A point torn loose from its sheet is a scrap of flesh, not surface.
        let mut attached = vec![false; self.points.len()];
        for spring in &self.springs {
            if !spring.broken {
                attached[spring.a] = true;
                attached[spring.b] = true;
            }
        }
        for (outline_index, outline) in self.outlines.iter().enumerate() {
            let count = outline.points.len();
            let mut grid: HashMap<GridKey, Vec<usize>> = HashMap::new();
            for edge in 0..count {
                let intact =
                    outline.edge_springs[edge].is_some_and(|spring| !self.springs[spring].broken);
                if !intact {
                    continue;
                }
                let a = self.points[outline.points[edge]].position;
                let b = self.points[outline.points[(edge + 1) % count]].position;
                let low = Vec2 {
                    x: a.x.min(b.x) - reach,
                    y: a.y.min(b.y) - reach,
                };
                let high = Vec2 {
                    x: a.x.max(b.x) + reach,
                    y: a.y.max(b.y) + reach,
                };
                for_spatial_cells(
                    Aabb {
                        min: low,
                        max: high,
                    },
                    cell,
                    |key| {
                        grid.entry(key).or_default().push(edge);
                    },
                );
            }
            for (position, &point) in outline.points.iter().enumerate() {
                if !attached[point] {
                    continue;
                }
                let p = self.points[point].position;
                let Some(edges) = grid.get(&spatial_key(p, cell)) else {
                    continue;
                };
                for &edge in edges {
                    let next = (edge + 1) % count;
                    if loop_distance(position, edge, count) <= OUTLINE_NEIGHBORS
                        || loop_distance(position, next, count) <= OUTLINE_NEIGHBORS
                    {
                        continue;
                    }
                    let a = self.points[outline.points[edge]].position;
                    let b = self.points[outline.points[next]].position;
                    // Only surfaces that face each other across a gap in the
                    // body's rest shape can press together; the far side of
                    // the same limb lies behind its outline even at rest.
                    let (home_a, home_b) = (
                        self.points[outline.points[edge]].home,
                        self.points[outline.points[next]].home,
                    );
                    let faces_across_gap = outward_normal(home_a, home_b, outline.outward)
                        .is_some_and(|normal| {
                            dot(subtract(self.points[point].home, home_a), normal) > 0.0
                        });
                    if faces_across_gap && distance_to_segment(p, a, b) <= reach {
                        self.outline_pairs.push(OutlinePair {
                            outline: outline_index,
                            point,
                            edge,
                        });
                    }
                }
            }
        }
    }

    /// Pushes outline points out from behind the stretches of outline they
    /// pressed into, and those stretches back, split by mass.
    pub(super) fn solve_outline_contacts(&mut self) {
        let max_depth = self.materials.point_spacing * OUTLINE_CONTACT_DEPTH;
        for pair_index in 0..self.outline_pairs.len() {
            let pair = self.outline_pairs[pair_index];
            let outline = &self.outlines[pair.outline];
            let count = outline.points.len();
            let ia = outline.points[pair.edge];
            let ib = outline.points[(pair.edge + 1) % count];
            let outward = outline.outward;
            let (p, a, b) = (self.points[pair.point], self.points[ia], self.points[ib]);
            let Some(normal) = outward_normal(a.position, b.position, outward) else {
                continue;
            };
            let along = subtract(b.position, a.position);
            let t = dot(subtract(p.position, a.position), along) / dot(along, along);
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let separation = dot(subtract(p.position, a.position), normal);
            if separation >= 0.0 || separation < -max_depth {
                continue;
            }
            let inverse = |point: &Point| {
                if point.pinned {
                    0.0
                } else {
                    1.0 / point.mass.max(EPSILON)
                }
            };
            let (wp, wa, wb) = (inverse(&p), inverse(&a), inverse(&b));
            let weight = wp + wa * (1.0 - t) * (1.0 - t) + wb * t * t;
            if weight <= EPSILON {
                continue;
            }
            let push = -separation * OUTLINE_CONTACT_STIFFNESS / weight;
            self.points[pair.point].position = add(p.position, scale(normal, push * wp));
            self.points[ia].position = subtract(a.position, scale(normal, push * wa * (1.0 - t)));
            self.points[ib].position = subtract(b.position, scale(normal, push * wb * t));
            self.debug.outline_contacts += 1;
            self.debug.max_outline_overlap = self.debug.max_outline_overlap.max(-separation);
        }
    }
}

/// The outward normal of the outline edge from `a` to `b`.
fn outward_normal(a: Vec2, b: Vec2, outward: f64) -> Option<Vec2> {
    let along = subtract(b, a);
    let length = hypot(along.x, along.y);
    (length > EPSILON).then(|| Vec2 {
        x: -along.y / length * outward,
        y: along.x / length * outward,
    })
}

/// Steps between two positions on a loop of `count`, the short way round.
fn loop_distance(a: usize, b: usize, count: usize) -> usize {
    let forward = a.abs_diff(b);
    forward.min(count - forward)
}
