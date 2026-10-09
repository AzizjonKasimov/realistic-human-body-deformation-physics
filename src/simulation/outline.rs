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

    /// Keeps the outline loops on the copies of `point` after a cut split it
    /// into `copies` (the point among them): each stretch of outline follows
    /// the copy its spring went to, and where the two stretches meeting at
    /// the point went to different copies, the loop crosses the cut's mouth
    /// between them, which is no surface to press against.
    pub(super) fn follow_split_outline(&mut self, point: usize, copies: &[usize]) {
        let springs = &self.springs;
        let holder = |spring: Option<usize>| {
            spring.and_then(|index| {
                let spring = springs[index];
                copies
                    .iter()
                    .copied()
                    .find(|&copy| spring.a == copy || spring.b == copy)
            })
        };
        for outline in &mut self.outlines {
            let count = outline.points.len();
            let Some(position) = outline.points.iter().position(|&p| p == point) else {
                continue;
            };
            let previous = (position + count - 1) % count;
            let before = holder(outline.edge_springs[previous]).unwrap_or(point);
            let after = holder(outline.edge_springs[position]).unwrap_or(point);
            outline.points[position] = before;
            if after != before {
                outline.points.insert(position + 1, after);
                let onward = outline.edge_springs[position];
                outline.edge_springs[position] = None;
                outline.edge_springs.insert(position + 1, onward);
            }
        }
    }

    /// Puts point `p`, which a cut inserted on the outline edge between `a`
    /// and `b`, into the loops running along that edge: spring `from_a` now
    /// joins `a` to `p` and `to_b` joins `p` to `b`.
    pub(super) fn follow_inserted_outline(
        &mut self,
        a: usize,
        b: usize,
        p: usize,
        from_a: usize,
        to_b: usize,
    ) {
        for outline in &mut self.outlines {
            let count = outline.points.len();
            let Some(position) = (0..count).find(|&k| {
                let (x, y) = (outline.points[k], outline.points[(k + 1) % count]);
                (x == a && y == b) || (x == b && y == a)
            }) else {
                continue;
            };
            let (first, second) = if outline.points[position] == a {
                (from_a, to_b)
            } else {
                (to_b, from_a)
            };
            outline.edge_springs[position] = Some(first);
            outline.points.insert(position + 1, p);
            outline.edge_springs.insert(position + 1, Some(second));
        }
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
        let mut grid = std::mem::take(&mut self.grid);
        for (outline_index, outline) in self.outlines.iter().enumerate() {
            let count = outline.points.len();
            // Each edge goes in every cell within reach of it. A torn edge, or
            // one of flesh torn away, is no longer a surface anything can
            // press against.
            grid.build_boxes(
                cell,
                (0..count)
                    .filter(|&edge| {
                        outline.edge_springs[edge].is_some_and(|spring| {
                            !self.springs[spring].broken
                                && self.spring_in_flesh(self.springs[spring])
                        })
                    })
                    .map(|edge| {
                        let a = self.points[outline.points[edge]].position;
                        let b = self.points[outline.points[(edge + 1) % count]].position;
                        (edge, segment_aabb(a, b, reach))
                    }),
            );
            for (position, &point) in outline.points.iter().enumerate() {
                // Flesh torn away is not surface.
                if !is_flesh(&self.flesh, point) {
                    continue;
                }
                let p = self.points[point].position;
                for &edge in grid.at(p) {
                    let edge = edge as usize;
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
        self.grid = grid;
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
