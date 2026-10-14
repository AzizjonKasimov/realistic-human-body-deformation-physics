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

use super::soft::KG_PER_MASS;
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

impl OutlineLoop {
    /// The loop's points, in order along it.
    pub(super) fn points(&self) -> &[usize] {
        &self.points
    }
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

    /// The skin rests stretched (`tissue.rs`), so it pulls each point of its
    /// outline inward, and in a hollow such as the armpit outward; in a body
    /// the skin runs on round the back there and the flesh inside holds it.
    /// Measures that pull at rest, from the skin that still holds together
    /// there, which `pull_outline_round` then holds against: where the skin
    /// is cut or torn, the pull it carried is gone, and the edge draws back.
    /// Skin cut free of the body's skin has no body behind it, and is held by
    /// nothing.
    pub(super) fn measure_outline_wrap(&mut self) {
        let homes: Vec<Vec2> = self.points.iter().map(|point| point.home).collect();
        let force = self.skin_pull(&homes);
        let joined = self.skin_joined_to_body();
        for point in &mut self.points {
            point.wrap = Vec2::default();
        }
        for outline in &self.outlines {
            for &point in &outline.points {
                // Where a tear or a cut runs through the outline, the skin
                // round the back is open there too, and holds nothing: a
                // light scrap of torn skin held as if whole was thrown.
                let skin = &self.points[point];
                if skin.layer == TissueLayer::Skin && joined[point] && !skin.on_cut {
                    self.points[point].wrap = scale(force[point], -1.0);
                }
            }
        }
        self.outline_wrap_stale = false;
    }

    /// Which skin points the body's skin still holds: the largest piece of
    /// skin its intact springs join together. A flap cut free is a piece of
    /// its own.
    fn skin_joined_to_body(&self) -> Vec<bool> {
        let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); self.points.len()];
        for spring in &self.springs {
            if spring.broken || spring.layer != TissueLayer::Skin {
                continue;
            }
            if self.points[spring.a].is_ghost() || self.points[spring.b].is_ghost() {
                continue;
            }
            neighbours[spring.a].push(spring.b);
            neighbours[spring.b].push(spring.a);
        }
        // Each skin point's piece, numbered in the order the pieces are met.
        let mut piece = vec![usize::MAX; self.points.len()];
        let mut sizes = Vec::new();
        for start in 0..self.points.len() {
            if piece[start] != usize::MAX || self.points[start].layer != TissueLayer::Skin {
                continue;
            }
            let number = sizes.len();
            let mut size = 0usize;
            let mut stack = vec![start];
            piece[start] = number;
            while let Some(index) = stack.pop() {
                size += 1;
                for &next in &neighbours[index] {
                    if piece[next] == usize::MAX {
                        piece[next] = number;
                        stack.push(next);
                    }
                }
            }
            sizes.push(size);
        }
        // The first of the largest pieces, so the choice is the same in
        // every build.
        let body = (0..sizes.len()).fold(None, |best: Option<usize>, number| match best {
            Some(best) if sizes[best] >= sizes[number] => Some(best),
            _ => Some(number),
        });
        piece.iter().map(|&number| Some(number) == body).collect()
    }

    /// The net pull the skin's springs and triangles that still hold real
    /// flesh put on each point lying at `places`. A piece of a cut reaching a
    /// ghost holds no flesh across the cut, and pulls nothing there.
    fn skin_pull(&self, places: &[Vec2]) -> Vec<Vec2> {
        let mut force = vec![Vec2::default(); self.points.len()];
        let real = |index: usize| !self.points[index].is_ghost();
        for spring in &self.springs {
            if spring.broken
                || spring.layer != TissueLayer::Skin
                || !real(spring.a)
                || !real(spring.b)
            {
                continue;
            }
            let (a, b) = (places[spring.a], places[spring.b]);
            let along = subtract(b, a);
            let length = hypot(along.x, along.y);
            if length < EPSILON {
                continue;
            }
            let (stiffness, rest) = self.spring_stiffness_at(spring, length);
            let pull = scale(along, stiffness / KG_PER_MASS * (length - rest) / length);
            force[spring.a] = add(force[spring.a], pull);
            force[spring.b] = subtract(force[spring.b], pull);
        }
        for area in &self.areas {
            if area.layer != TissueLayer::Skin
                || area.rest_area.abs() <= EPSILON
                || !self.area_alive(area)
                || !(real(area.a) && real(area.b) && real(area.c))
            {
                continue;
            }
            let corners = [area.a, area.b, area.c];
            let [a, b, c] = corners.map(|corner| places[corner]);
            let stiffness = area.stiffness / KG_PER_MASS / area.rest_area.abs();
            let squeeze = stiffness * (signed_area(a, b, c) - area.rest_area);
            let gradients = [
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
            ];
            for (corner, gradient) in corners.into_iter().zip(gradients) {
                force[corner] = subtract(force[corner], scale(gradient, squeeze));
            }
        }
        force
    }

    /// Holds each point of the skin's outline against the pull its skin
    /// carried at rest (`measure_outline_wrap`), as a steady load that turns
    /// with the bones the point keeps its place on, so it holds a raised or
    /// lowered limb's skin as it held it in the guard: one that turned with
    /// the outline itself would drive it round in a flutter.
    pub(super) fn pull_outline_round(&mut self, h: f64) {
        if self.outline_wrap_stale {
            self.measure_outline_wrap();
        }
        let turns = self.bone_turns();
        for index in 0..self.points.len() {
            let point = self.points[index];
            if point.wrap == Vec2::default() || point.pinned || !is_flesh(&self.flesh, index) {
                continue;
            }
            let binding = point.posture;
            let mut wrap = Vec2::default();
            let mut held = 0.0;
            for slot in 0..2 {
                let Some(&(cos, sin)) = turns.get(binding.bones[slot]) else {
                    continue;
                };
                let weight = binding.weights[slot];
                wrap.x += weight * (point.wrap.x * cos - point.wrap.y * sin);
                wrap.y += weight * (point.wrap.x * sin + point.wrap.y * cos);
                held += weight;
            }
            if held <= 0.0 {
                wrap = point.wrap;
            }
            let shift = scale(wrap, h * h / point.mass.max(EPSILON));
            self.points[index].position = add(point.position, shift);
        }
    }

    /// How far each bone has turned since the body was built, as the cosine
    /// and sine of the angle.
    fn bone_turns(&self) -> Vec<(f64, f64)> {
        self.bones
            .iter()
            .map(|bone| {
                let now = subtract(bone.b, bone.a);
                let built = subtract(bone.home_b, bone.home_a);
                let lengths = (dot(now, now) * dot(built, built)).sqrt().max(EPSILON);
                (dot(built, now) / lengths, cross(built, now) / lengths)
            })
            .collect()
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
            // A stretch of outline turned over in a crease, as the skin of
            // the armpit folds under an arm pressed to the chest, faces into
            // the body: it is no surface to press against, and a point in
            // front of it would read as deep behind it.
            if dot(along, subtract(b.home, a.home)) <= 0.0 {
                continue;
            }
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
