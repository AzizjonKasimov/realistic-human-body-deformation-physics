//! Cuts that open. A blade does not break the fibers it crosses: each
//! triangle it passes through splits into two pieces along its path, and the
//! two sides part, so the cut gapes exactly where the blade went and shows
//! what lies beneath, while no flesh is lost.
//!
//! Rebuilt from the virtual node algorithm (Molino, Bao, and Fedkiw, "A
//! virtual node algorithm for changing mesh topology during simulation",
//! SIGGRAPH 2004), as PhysBAM's 2D version builds it
//! (`VIRTUAL_NODE_ALGORITHM.cpp`, BSD), with the cut embedded in each piece
//! as Sifakis, Der, and Fedkiw do ("Arbitrary cutting of deformable
//! tetrahedralized objects", 2007). Each triangle the blade crosses becomes
//! two copies of itself, each holding the flesh on one side of the blade's
//! path through it ([`TriangleCut`]). A corner where a piece holds no flesh
//! gets a ghost, a copy of the point shared with the pieces beside it on the
//! same side, so the two sides come apart while every piece keeps its whole
//! triangle's shape and no piece is too small to stay steady. Points share
//! their mass by the flesh each copy holds, as Koschier, Bender, and Thuerey
//! advise ("Robust eXtended finite elements for complex cutting of
//! deformables", 2017), so ghosts are light.
//!
//! Tears open by splitting points instead, as Rapier's soft bodies tear
//! (rapier2d 0.36, `tearing_crack.rs`, Apache-2.0), after the tearing in
//! Müller et al., "Position Based Dynamics" (2007).
//!
//! Skin rests stretched in a living body (`tissue.rs`), so once a cut parts
//! it, the skin on each side pulls back from the cut and the cut gapes.

use super::*;

/// How fast, in pixels per second, a tool must move for the tissue it tears
/// by stretching to split open into a laceration: half a firm swing, below
/// which a blow should tear nothing much.
pub(super) const TEAR_OPENING_SPEED: f64 = 800.0;

/// The nearest a cut crosses a side to either of its corners, as a share of
/// the side, so no piece is a sliver and no cut runs through a point.
const CROSSING_MARGIN: f64 = 0.1;

/// The least share of a point's mass a ghost of it gets, so a piece's far
/// corners never weigh nothing.
const GHOST_MASS_SHARE: f64 = 0.1;

/// Where a knife's cut crosses a triangle, and which side of it this piece of
/// the triangle holds. The cut runs straight between its crossings of two of
/// the triangle's sides, which meet at the corner they share.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriangleCut {
    /// The sides the cut crosses: 0 runs from corner `a` to `b`, 1 from `b`
    /// to `c`, and 2 from `c` to `a`.
    pub sides: [usize; 2],
    /// How far along each of those sides, from its first corner, the cut
    /// crosses it.
    pub at: [f64; 2],
    /// This piece holds the corner the two sides share, rather than the
    /// other two.
    pub holds_shared_corner: bool,
    /// The triangle's other piece, across the cut.
    pub partner: usize,
}

impl TriangleCut {
    /// The corner (0 for `a`, 1 for `b`, 2 for `c`) the two cut sides share.
    fn shared_corner(&self) -> usize {
        (0..3)
            .find(|&corner| self.sides.iter().all(|&side| side_has(side, corner)))
            .unwrap_or(0)
    }

    /// Whether this piece holds the flesh at its corner `corner`.
    pub fn holds_corner(&self, corner: usize) -> bool {
        (corner == self.shared_corner()) == self.holds_shared_corner
    }

    /// The cut's two ends, in weights on the triangle's corners.
    pub fn ends(&self) -> [[f64; 3]; 2] {
        [0, 1].map(|end| along_side(self.sides[end], self.at[end]))
    }

    /// The stretch of side `side`, from its first corner, this piece holds.
    fn holds_along(&self, side: usize) -> Option<(f64, f64)> {
        let first = self.holds_corner(side);
        match self.sides.iter().position(|&cut| cut == side) {
            Some(end) if first => Some((0.0, self.at[end])),
            Some(end) => Some((self.at[end], 1.0)),
            None => first.then_some((0.0, 1.0)),
        }
    }

    /// Whether this piece holds the flesh at `weights`.
    fn holds(&self, weights: [f64; 3]) -> bool {
        let [from, to] = self.ends();
        let side = |weights: [f64; 3]| {
            (to[1] - from[1]) * (weights[2] - from[2]) - (to[2] - from[2]) * (weights[1] - from[1])
        };
        let mut corner = [0.0; 3];
        corner[self.shared_corner()] = 1.0;
        let here = side(weights) * side(corner);
        if self.holds_shared_corner {
            here >= -1.0e-9
        } else {
            here <= 1.0e-9
        }
    }
}

/// The flesh one triangle holds, as the corners of the polygon it covers, in
/// weights on the triangle's corners `a`, `b`, and `c`: the whole triangle,
/// or, for a piece of a cut one, its side of the cut.
#[derive(Clone, Copy, Debug)]
pub struct Piece {
    corners: [[f64; 3]; 4],
    count: usize,
}

impl Piece {
    pub fn corners(&self) -> &[[f64; 3]] {
        &self.corners[..self.count]
    }

    /// The share of the triangle the piece covers, and its centroid.
    fn share_and_centroid(&self) -> (f64, [f64; 3]) {
        let corners = self.corners();
        let mut doubled = 0.0;
        let (mut cx, mut cy) = (0.0, 0.0);
        for (index, here) in corners.iter().enumerate() {
            let next = corners[(index + 1) % corners.len()];
            let cross = here[1] * next[2] - next[1] * here[2];
            doubled += cross;
            cx += (here[1] + next[1]) * cross;
            cy += (here[2] + next[2]) * cross;
        }
        if doubled.abs() <= 1.0e-12 {
            return (0.0, [1.0 / 3.0; 3]);
        }
        let (cx, cy) = (cx / (3.0 * doubled), cy / (3.0 * doubled));
        (doubled.abs(), [1.0 - cx - cy, cx, cy])
    }
}

impl Triangle {
    /// The flesh this triangle holds (see [`Piece`]).
    pub fn piece(&self) -> Piece {
        let mut corners = [[0.0; 3]; 4];
        let Some(cut) = self.cut else {
            for (corner, weights) in corners.iter_mut().take(3).enumerate() {
                weights[corner] = 1.0;
            }
            return Piece { corners, count: 3 };
        };
        // Round the triangle's corners, with the cut's crossings on the
        // sides it crosses; the piece is the run from one crossing to the
        // other that holds its own corners.
        let ends = cut.ends();
        let mut round = Vec::with_capacity(5);
        for corner in 0..3 {
            let mut weights = [0.0; 3];
            weights[corner] = 1.0;
            round.push((weights, Some(corner)));
            if let Some(end) = cut.sides.iter().position(|&side| side == corner) {
                round.push((ends[end], None));
            }
        }
        let crossings: Vec<usize> = (0..round.len())
            .filter(|&index| round[index].1.is_none())
            .collect();
        let (first, second) = (crossings[0], crossings[1]);
        let inside: Vec<usize> = (first..=second).collect();
        let held_inside = round[first + 1]
            .1
            .is_some_and(|corner| cut.holds_corner(corner));
        let run: Vec<usize> = if held_inside {
            inside
        } else {
            (second..round.len()).chain(0..=first).collect()
        };
        let mut count = 0;
        for index in run.into_iter().take(4) {
            corners[count] = round[index].0;
            count += 1;
        }
        Piece { corners, count }
    }

    /// Whether this triangle holds flesh at `weights` on its corners.
    pub fn holds(&self, weights: [f64; 3]) -> bool {
        self.cut.map_or(true, |cut| cut.holds(weights))
    }
}

/// An opened cut or tear, as its two lips, where each lies now and in the
/// rest shape, their ends in matching order.
#[derive(Clone, Copy, Debug)]
pub struct OpenedCut {
    pub layer: TissueLayer,
    pub lips: [(Vec2, Vec2); 2],
    pub rest_lips: [(Vec2, Vec2); 2],
}

impl World {
    /// The blade crossed spring `index`, `cut_at` of the way from its `a`
    /// end. The fiber is marked cut; the cut opens when the knife's step ends
    /// ([`World::open_blade_cuts`]).
    pub(super) fn cut_spring(&mut self, index: usize, cut_at: f64) {
        let spring = self.springs[index];
        let wear = &mut self.spring_wear[index];
        if spring.broken || wear.cut {
            return;
        }
        wear.cut = true;
        wear.cut_at = cut_at.clamp(0.0, 1.0);
        if spring.layer == TissueLayer::Skin {
            self.open_skin.push(index);
        }
        self.pending_cuts.push(index);
    }

    /// The cuts and tears that have opened, each as its two lips: a knife's
    /// cut by the two pieces of each triangle it split, a tear by the twin
    /// springs along its lips. Pieces or lips torn away since are left out.
    pub fn opened_cuts(&self) -> Vec<OpenedCut> {
        let mut opened = Vec::new();
        for (index, triangle) in self.triangles.iter().enumerate() {
            let Some(cut) = triangle.cut else {
                continue;
            };
            let partner = &self.triangles[cut.partner];
            if cut.partner < index
                || !self.triangle_alive(triangle)
                || !self.triangle_alive(partner)
            {
                continue;
            }
            let [from, to] = cut.ends();
            let lip = |piece: &Triangle, rest: bool| {
                (
                    self.piece_position(piece, from, rest),
                    self.piece_position(piece, to, rest),
                )
            };
            opened.push(OpenedCut {
                layer: triangle.layer,
                lips: [lip(triangle, false), lip(partner, false)],
                rest_lips: [lip(triangle, true), lip(partner, true)],
            });
        }
        for (index, (lip, wear)) in self.springs.iter().zip(&self.spring_wear).enumerate() {
            if wear.twin == MISSING_SPRING || wear.twin < index || lip.broken {
                continue;
            }
            let twin = self.springs[wear.twin];
            let points = &self.points;
            opened.push(OpenedCut {
                layer: lip.layer,
                lips: [
                    (points[lip.a].position, points[lip.b].position),
                    (points[twin.a].position, points[twin.b].position),
                ],
                rest_lips: [
                    (points[lip.a].home, points[lip.b].home),
                    (points[twin.a].home, points[twin.b].home),
                ],
            });
        }
        opened
    }

    /// Where `weights` on triangle `triangle`'s corners lies now, or in the
    /// rest shape.
    fn piece_position(&self, triangle: &Triangle, weights: [f64; 3], rest: bool) -> Vec2 {
        let at = |point: usize| {
            let point = &self.points[point];
            if rest {
                point.home
            } else {
                point.position
            }
        };
        let (a, b, c) = (at(triangle.a), at(triangle.b), at(triangle.c));
        Vec2 {
            x: a.x * weights[0] + b.x * weights[1] + c.x * weights[2],
            y: a.y * weights[0] + b.y * weights[1] + c.y * weights[2],
        }
    }

    /// Opens the cuts the blade made this step. Each triangle the blade has
    /// passed through, two of its sides cut, splits into two pieces, each
    /// holding the flesh on its side of the blade's path; the points around
    /// it split where the flesh on the two sides no longer meets, ghosts
    /// carrying each piece's far corners; and the skin's own tension draws
    /// each side back from the cut. A cut fiber that no triangle uses, such
    /// as a lone test fiber,
    /// breaks instead.
    pub(super) fn open_blade_cuts(&mut self) {
        if self.pending_cuts.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_cuts);
        let mut used = vec![false; self.springs.len()];
        for triangle in &self.triangles {
            for edge in [triangle.edge_ab, triangle.edge_bc, triangle.edge_ca] {
                if let Some(used) = used.get_mut(edge) {
                    *used = true;
                }
            }
        }
        for &index in &pending {
            if !used[index] {
                self.break_spring(index);
            }
        }

        let mut split = Vec::new();
        for index in 0..self.triangles.len() {
            let triangle = self.triangles[index];
            if triangle.cut.is_some() || !self.triangle_alive(&triangle) {
                continue;
            }
            if let Some(crossings) = self.blade_crossings(&triangle) {
                split.push((index, crossings));
            }
        }
        if split.is_empty() {
            return;
        }

        // The triangles around each corner of a split triangle, and the area
        // constraint over each, found before the pieces share corners.
        let mut nodes: Vec<usize> = split
            .iter()
            .flat_map(|&(index, _)| corners(&self.triangles[index]))
            .collect();
        nodes.sort_unstable();
        nodes.dedup();
        let mut fans = vec![Vec::new(); nodes.len()];
        for (index, triangle) in self.triangles.iter().enumerate() {
            for corner in corners(triangle) {
                if let Ok(node) = nodes.binary_search(&corner) {
                    fans[node].push(index);
                }
            }
        }
        let mut around: Vec<usize> = fans.iter().flatten().copied().collect();
        around.sort_unstable();
        around.dedup();
        let mut area_of: HashMap<usize, usize> = around
            .iter()
            .filter_map(|&index| self.area_of_triangle(index).map(|area| (index, area)))
            .collect();

        // Each triangle the blade crossed becomes two pieces.
        let mut pairs = Vec::with_capacity(split.len());
        for &(index, (sides, at)) in &split {
            let partner = self.triangles.len();
            let mut piece = self.triangles[index];
            piece.cut = Some(TriangleCut {
                sides,
                at,
                holds_shared_corner: false,
                partner: index,
            });
            self.triangles[index].cut = Some(TriangleCut {
                sides,
                at,
                holds_shared_corner: true,
                partner,
            });
            self.triangles.push(piece);
            if let Some(&area) = area_of.get(&index) {
                self.areas.push(AreaConstraint {
                    impulse: 0.0,
                    ..self.areas[area]
                });
                area_of.insert(partner, self.areas.len() - 1);
            }
            for fan in fans.iter_mut() {
                if fan.contains(&index) {
                    fan.push(partner);
                }
            }
            around.push(partner);
            pairs.push((index, partner));
        }
        self.rehome_blood_on_split_skin(&pairs);

        // Each corner splits into one point for each group of the pieces
        // around it whose flesh still meets along a side: the group holding
        // the flesh at the corner keeps it, and every other gets a ghost.
        let homes_before: Vec<Vec2> = self.points.iter().map(|point| point.home).collect();
        let before: Vec<[usize; 3]> = around
            .iter()
            .map(|&index| corners(&self.triangles[index]))
            .collect();
        let mut origin = Vec::new();
        let mut moves: Vec<(usize, usize, usize)> = Vec::new();
        for (node, fan) in nodes.iter().zip(&fans) {
            let (groups, group_of) = self.flesh_groups(*node, fan);
            if groups.len() < 2 || self.points[*node].pinned {
                continue;
            }
            let source = self.points[*node];
            let total: f64 = groups.iter().map(|group| group.1).sum::<f64>().max(EPSILON);
            let mut kept = source.mass;
            for (group, &(holds_node, share)) in groups.iter().enumerate() {
                if holds_node {
                    continue;
                }
                let mass = (source.mass * share / total).max(source.mass * GHOST_MASS_SHARE);
                kept -= mass;
                self.points.push(Point {
                    mass,
                    ghost_of: *node,
                    on_cut: false,
                    ..source
                });
                origin.push(*node);
                let ghost = self.points.len() - 1;
                for (&triangle, &member) in fan.iter().zip(&group_of) {
                    if member == Some(group) {
                        moves.push((triangle, *node, ghost));
                    }
                }
            }
            self.points[*node].mass = kept.max(source.mass * GHOST_MASS_SHARE);
            // The blade cut the fat over the rest of the point's flesh too.
            let share = self.points[*node].mass / source.mass.max(EPSILON);
            self.share_fat_ties(*node, share);
        }
        for &(triangle, from, to) in &moves {
            replace_corner(&mut self.triangles[triangle], from, to);
            if let Some(&area) = area_of.get(&triangle) {
                let area = &mut self.areas[area];
                for corner in [&mut area.a, &mut area.b, &mut area.c] {
                    if *corner == from {
                        *corner = to;
                    }
                }
            }
        }
        self.rejoin_piece_springs(&around, &before, &area_of);

        self.follow_rest_places(&homes_before, &origin);
        self.stats.cut_openings += origin.len() as i32;
        for &node in &origin {
            self.note(
                EventKind::CutOpened,
                Subject::Point(node),
                self.points[node].position,
                Some(Actor::Tool),
                Measure::None,
                0.0,
                0.0,
            );
        }
        self.finish_topology_change(&origin);
    }

    /// Where the blade crossed two sides of `triangle`, if it did: the sides
    /// and how far along each, moved in from the corners. Of three crossed
    /// sides, the two crossings farthest apart.
    fn blade_crossings(&self, triangle: &Triangle) -> Option<([usize; 2], [f64; 2])> {
        let corners = corners(triangle);
        let edges = [triangle.edge_ab, triangle.edge_bc, triangle.edge_ca];
        let mut crossed = Vec::with_capacity(3);
        for side in 0..3 {
            let (Some(spring), Some(wear)) = (
                self.springs.get(edges[side]),
                self.spring_wear.get(edges[side]),
            ) else {
                continue;
            };
            if !wear.cut || spring.broken {
                continue;
            }
            let at = if spring.a == corners[side] {
                wear.cut_at
            } else {
                1.0 - wear.cut_at
            };
            crossed.push((side, at.clamp(CROSSING_MARGIN, 1.0 - CROSSING_MARGIN)));
        }
        if crossed.len() < 2 {
            return None;
        }
        let rest = |&(side, at): &(usize, f64)| {
            let weights = along_side(side, at);
            self.piece_position(triangle, weights, true)
        };
        let mut best = (0, 1, -1.0);
        for i in 0..crossed.len() {
            for j in i + 1..crossed.len() {
                let gap = distance(rest(&crossed[i]), rest(&crossed[j]));
                if gap > best.2 {
                    best = (i, j, gap);
                }
            }
        }
        let (first, second) = (crossed[best.0], crossed[best.1]);
        Some(([first.0, second.0], [first.1, second.1]))
    }

    /// Groups the living triangles of `fan` around point `node` by whether
    /// their flesh meets along a side through it, the way the flesh of two
    /// pieces of cut triangles beside each other does on the same side of
    /// the cut and not across it. Returns, for each group, whether it holds
    /// the flesh at the point itself (all that do are one group, which keeps
    /// the point) and its share of the point's mass, and each triangle's
    /// group (none for a dead one, which stays with the point).
    fn flesh_groups(&self, node: usize, fan: &[usize]) -> (Vec<(bool, f64)>, Vec<Option<usize>>) {
        let count = fan.len();
        let alive: Vec<bool> = fan
            .iter()
            .map(|&index| self.triangle_alive(&self.triangles[index]))
            .collect();
        let mut parent: Vec<usize> = (0..count).collect();
        for i in 0..count {
            for j in i + 1..count {
                if !alive[i] || !alive[j] {
                    continue;
                }
                let (first, second) = (&self.triangles[fan[i]], &self.triangles[fan[j]]);
                let meets = corners(first)
                    .into_iter()
                    .filter(|&other| other != node && corners(second).contains(&other))
                    .any(|other| {
                        let held = |triangle: &Triangle| held_between(triangle, node, other);
                        match (held(first), held(second)) {
                            (Some(a), Some(b)) => a.1.min(b.1) - a.0.max(b.0) > 1.0e-6,
                            _ => false,
                        }
                    });
                if meets {
                    let (root_i, root_j) = (find_root(&mut parent, i), find_root(&mut parent, j));
                    parent[root_i.max(root_j)] = root_i.min(root_j);
                }
            }
        }
        // The flesh at the point itself joins all that hold it.
        let holders: Vec<usize> = (0..count)
            .filter(|&i| alive[i] && holds_node(&self.triangles[fan[i]], node))
            .collect();
        for pair in holders.windows(2) {
            let (root_i, root_j) = (
                find_root(&mut parent, pair[0]),
                find_root(&mut parent, pair[1]),
            );
            parent[root_i.max(root_j)] = root_i.min(root_j);
        }
        let mut group_of_root = vec![usize::MAX; count];
        let mut groups: Vec<(bool, f64)> = Vec::new();
        let mut group_of = vec![None; count];
        for i in 0..count {
            if !alive[i] {
                continue;
            }
            let root = find_root(&mut parent, i);
            if group_of_root[root] == usize::MAX {
                group_of_root[root] = groups.len();
                groups.push((false, 0.0));
            }
            let group = group_of_root[root];
            group_of[i] = Some(group);
            let triangle = &self.triangles[fan[i]];
            groups[group].0 |= holds_node(triangle, node);
            groups[group].1 += self.mass_share(triangle, node);
        }
        // With no group holding the point's flesh, the first keeps it.
        if !groups.iter().any(|group| group.0) {
            if let Some(first) = groups.first_mut() {
                first.0 = true;
            }
        }
        (groups, group_of)
    }

    /// How much of the flesh `triangle` holds weighs on its corner `node`:
    /// its area in the rest shape times the share of its flesh nearest that
    /// corner, as a triangle's mass is lumped onto its corners.
    fn mass_share(&self, triangle: &Triangle, node: usize) -> f64 {
        let Some(corner) = corners(triangle).iter().position(|&point| point == node) else {
            return 0.0;
        };
        let [a, b, c] = corners(triangle).map(|point| self.points[point].home);
        let (share, centroid) = triangle.piece().share_and_centroid();
        signed_area(a, b, c).abs() * share * centroid[corner]
    }

    /// Gives every side of the triangles `around` a spring between its
    /// corners as they are now, after a cut split points: a side whose
    /// corners are unchanged keeps its spring, and every other side gets a
    /// copy of the spring it had, which a piece beside it on the same side
    /// of the cut shares. `before` holds each triangle's corners before the
    /// split; `area_of` the area constraint over each.
    fn rejoin_piece_springs(
        &mut self,
        around: &[usize],
        before: &[[usize; 3]],
        area_of: &HashMap<usize, usize>,
    ) {
        // The ends each user of a spring wants, in the spring's own order.
        let mut wants: Vec<(usize, usize, usize, (usize, usize))> = Vec::new();
        for (slot, &index) in around.iter().enumerate() {
            let now = corners(&self.triangles[index]);
            let was = before.get(slot).copied().unwrap_or(now);
            let edges = edge_slots(&self.triangles[index]);
            for side in 0..3 {
                let edge = edges[side];
                let Some(spring) = self.springs.get(edge) else {
                    continue;
                };
                let next = (side + 1) % 3;
                let ends = if spring.a == was[side] && spring.b == was[next] {
                    (now[side], now[next])
                } else if spring.a == was[next] && spring.b == was[side] {
                    (now[next], now[side])
                } else {
                    continue;
                };
                wants.push((edge, index, side, ends));
            }
        }
        wants.sort_unstable_by_key(|&(edge, index, side, _)| (edge, index, side));
        let mut start = 0;
        while start < wants.len() {
            let edge = wants[start].0;
            let mut end = start;
            while end < wants.len() && wants[end].0 == edge {
                end += 1;
            }
            let spring = self.springs[edge];
            let wear = self.spring_wear[edge];
            let mut pairs: Vec<(usize, usize)> = Vec::new();
            for want in &wants[start..end] {
                if !pairs.contains(&want.3) {
                    pairs.push(want.3);
                }
            }
            let keeper = pairs
                .iter()
                .position(|&pair| pair == (spring.a, spring.b))
                .unwrap_or(0);
            let mut index_of = Vec::with_capacity(pairs.len());
            for (number, &(a, b)) in pairs.iter().enumerate() {
                if number == keeper {
                    self.springs[edge].a = a;
                    self.springs[edge].b = b;
                    index_of.push(edge);
                } else {
                    let added = self.push_spring(
                        Spring {
                            a,
                            b,
                            impulse: 0.0,
                            ..spring
                        },
                        SpringWear {
                            twin: MISSING_SPRING,
                            ..wear
                        },
                    );
                    index_of.push(added);
                }
            }
            for want in &wants[start..end] {
                let number = pairs.iter().position(|&pair| pair == want.3).unwrap_or(0);
                set_edge_slot(&mut self.triangles[want.1], want.2, index_of[number]);
            }
            start = end;
        }
        // Each area constraint takes its triangle's sides.
        for &index in around {
            let Some(&area) = area_of.get(&index) else {
                continue;
            };
            let triangle = self.triangles[index];
            let edges = edge_slots(&triangle);
            let corners = corners(&triangle);
            let edge_between = |x: usize, y: usize| {
                (0..3)
                    .find(|&side| {
                        let (p, q) = (corners[side], corners[(side + 1) % 3]);
                        (p == x && q == y) || (p == y && q == x)
                    })
                    .map_or(MISSING_SPRING, |side| edges[side])
            };
            let area = &mut self.areas[area];
            area.edge_ab = edge_between(area.a, area.b);
            area.edge_bc = edge_between(area.b, area.c);
            area.edge_ca = edge_between(area.c, area.a);
        }
    }

    /// Blood on skin a cut split goes with the piece holding the skin it
    /// lies on.
    fn rehome_blood_on_split_skin(&mut self, pairs: &[(usize, usize)]) {
        let triangles = &self.triangles;
        let rehome = |spot: &mut SkinSpot| {
            if let Some(&(first, second)) = pairs.iter().find(|&&(first, _)| first == spot.triangle)
            {
                if !triangles[first].holds(spot.weights) {
                    spot.triangle = second;
                }
            }
        };
        for fluid in &mut self.fluids {
            if let Some(spot) = fluid.on_skin.as_mut() {
                rehome(spot);
            }
            if let Some(spot) = fluid.trail_from.as_mut() {
                rehome(spot);
            }
        }
        for trail in &mut self.blood_trails {
            rehome(&mut trail.from);
            rehome(&mut trail.to);
        }
    }

    /// Opens the tears stretching made this step: each fiber torn by
    /// stretching alone splits one of its ends across itself, so the tissue
    /// parts perpendicular to the pull, as a laceration does, and the fiber
    /// itself stays whole on the far side. A fiber whose ends cannot split
    /// stays torn.
    pub(super) fn open_tears(&mut self) {
        if self.pending_tears.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_tears);
        let homes_before: Vec<Vec2> = self.points.iter().map(|point| point.home).collect();
        let mut origin = Vec::new();
        let mut families = Vec::new();
        for index in pending {
            let spring = self.springs[index];
            if !spring.broken {
                continue;
            }
            // The more loaded end opens first.
            let (first, second) = if self.points[spring.a].load >= self.points[spring.b].load {
                (spring.a, spring.b)
            } else {
                (spring.b, spring.a)
            };
            self.springs[index].broken = false;
            let mut opened = None;
            for (end, other) in [(first, second), (second, first)] {
                let normal = subtract(self.points[other].home, self.points[end].home);
                let before_split = self.points.len();
                if let Some(family) = self.split_point_across(end, normal) {
                    opened = Some((end, before_split, family));
                    break;
                }
            }
            match opened {
                Some((end, before_split, family)) => {
                    for &(member, _) in &family {
                        if member >= before_split {
                            origin.push(end);
                        }
                    }
                    families.push(family);
                }
                None => self.springs[index].broken = true,
            }
        }
        if families.is_empty() {
            return;
        }
        // A tear parts as far as what tore it pulls it.
        self.follow_rest_places(&homes_before, &origin);
        self.finish_topology_change(&origin);
    }

    /// Leaves the fat ties on `point` `share` as stiff as they were, as a
    /// point split by a cut or tear holds only part of the flesh round it,
    /// and the fat over that.
    fn share_fat_ties(&mut self, point: usize, share: f64) {
        let share = share.clamp(0.0, 1.0);
        for attachment in &mut self.attachments {
            if attachment.skin_point == point || attachment.muscle_point == point {
                attachment.stiffness *= share;
            }
        }
    }

    /// Keeps the per-point state in step with points a cut or tear added,
    /// copied from the points in `origin` in order, and tells anything built
    /// from the mesh to rebuild. A ghost holds no flesh from the start. What
    /// the soft constraints on the split points carried no longer fits them,
    /// as their springs now reach copies of a fraction of the mass, so those
    /// start afresh (`soft.rs`).
    fn finish_topology_change(&mut self, origin: &[usize]) {
        let first_copy = self.points.len() - origin.len();
        let mut split = vec![false; self.points.len()];
        for (copy, &source) in origin.iter().enumerate() {
            split[first_copy + copy] = true;
            split[source] = true;
        }
        self.forget_soft_impulses_at(&split);
        self.blunt_knock.resize(self.points.len(), 0.0);
        let known = self.flesh.len();
        self.flesh.resize(self.points.len(), true);
        for index in known..self.points.len() {
            self.flesh[index] = !self.points[index].is_ghost();
        }
        self.topology_version += 1;
        self.outline_wrap_stale = true;
    }

    /// Splits point `point` across the line through its rest place normal to
    /// `normal`: each of its triangles goes to the side its rest centroid is
    /// on, and the triangles of each side that share edges through the point
    /// stay together. A fiber torn by stretching opens this way, across
    /// itself, as Müller et al.'s position-based tearing and Rapier's
    /// `crack_across_edge` open a crack (see `split_fan`). A point among the
    /// pieces of a knife's cut does not split this way.
    fn split_point_across(&mut self, point: usize, normal: Vec2) -> Option<Vec<(usize, Vec2)>> {
        let fan = self.fan_of(point)?;
        if fan.iter().any(|&index| self.triangles[index].cut.is_some()) {
            return None;
        }
        let home = self.points[point].home;
        let side = |triangle: &Triangle| {
            let [a, b, c] = corners(triangle);
            let centroid = scale(
                add(
                    add(self.points[a].home, self.points[b].home),
                    self.points[c].home,
                ),
                1.0 / 3.0,
            );
            dot(subtract(centroid, home), normal) > 0.0
        };
        let (group, groups) =
            self.group_fan(point, &fan, |first, second, _| side(first) == side(second));
        self.split_fan(point, fan, group, groups)
    }

    /// The triangles of point `point`'s sheet around it, unless it is pinned
    /// or a ghost, or has fewer than two.
    fn fan_of(&self, point: usize) -> Option<Vec<usize>> {
        let source = self.points[point];
        if source.pinned || source.is_ghost() {
            return None;
        }
        let fan: Vec<usize> = (0..self.triangles.len())
            .filter(|&index| {
                let triangle = &self.triangles[index];
                triangle.layer == source.layer && corners(triangle).contains(&point)
            })
            .collect();
        (fan.len() >= 2).then_some(fan)
    }

    /// Groups the triangles of `fan`: two that share an edge through `point`
    /// to the corner `other` belong together when `joined(first, second,
    /// other)` holds. Returns each triangle's group, numbered in fan order,
    /// and the number of groups.
    fn group_fan(
        &self,
        point: usize,
        fan: &[usize],
        joined: impl Fn(&Triangle, &Triangle, usize) -> bool,
    ) -> (Vec<usize>, usize) {
        let count = fan.len();
        let mut parent: Vec<usize> = (0..count).collect();
        for i in 0..count {
            for j in i + 1..count {
                let (first, second) = (&self.triangles[fan[i]], &self.triangles[fan[j]]);
                let Some(other) = corners(first)
                    .into_iter()
                    .find(|&corner| corner != point && corners(second).contains(&corner))
                else {
                    continue;
                };
                if joined(first, second, other) {
                    let (root_i, root_j) = (find_root(&mut parent, i), find_root(&mut parent, j));
                    parent[root_i.max(root_j)] = root_i.min(root_j);
                }
            }
        }
        let mut group_of_root = vec![usize::MAX; count];
        let mut groups = 0;
        let group = (0..count)
            .map(|i| {
                let root = find_root(&mut parent, i);
                if group_of_root[root] == usize::MAX {
                    group_of_root[root] = groups;
                    groups += 1;
                }
                group_of_root[root]
            })
            .collect();
        (group, groups)
    }

    /// Splits point `point` by the groups of its triangles `fan` (`group` of
    /// `groups`): every group but the one with the most area gets its own
    /// copy of the point. Returns each copy (the point itself among them)
    /// with the direction, in the rest shape, from the point into its
    /// group's triangles; `None` when the triangles are all one group.
    fn split_fan(
        &mut self,
        point: usize,
        fan: Vec<usize>,
        group: Vec<usize>,
        groups: usize,
    ) -> Option<Vec<(usize, Vec2)>> {
        if groups < 2 {
            return None;
        }
        let source = self.points[point];
        let count = fan.len();

        let fan_corners: Vec<[usize; 3]> = fan
            .iter()
            .map(|&index| corners(&self.triangles[index]))
            .collect();
        let fan_areas: Vec<Option<usize>> = fan
            .iter()
            .map(|&index| self.area_of_triangle(index))
            .collect();
        let mut weight = vec![0.0; groups];
        let mut toward = vec![Vec2::default(); groups];
        for i in 0..count {
            let [a, b, c] = fan_corners[i];
            let (ha, hb, hc) = (
                self.points[a].home,
                self.points[b].home,
                self.points[c].home,
            );
            let area = match fan_areas[i] {
                Some(area) => self.areas[area].rest_area,
                None => signed_area(ha, hb, hc),
            };
            weight[group[i]] += area.abs();
            let centroid = scale(add(add(ha, hb), hc), 1.0 / 3.0);
            toward[group[i]] = add(toward[group[i]], subtract(centroid, source.home));
        }
        let keeper = (0..groups).fold(0, |best, g| if weight[g] > weight[best] { g } else { best });
        let total: f64 = weight.iter().sum();
        let mut member = vec![point; groups];
        for g in 0..groups {
            let share = if total > EPSILON {
                weight[g] / total
            } else {
                1.0 / groups as f64
            };
            let mass = source.mass * share;
            if g == keeper {
                self.points[point].mass = mass;
                self.points[point].on_cut = true;
            } else {
                self.points.push(Point {
                    mass,
                    on_cut: true,
                    ..source
                });
                member[g] = self.points.len() - 1;
            }
        }

        // Triangles and their area constraints follow their group's copy.
        for i in 0..count {
            let copy = member[group[i]];
            if copy == point {
                continue;
            }
            replace_corner(&mut self.triangles[fan[i]], point, copy);
            if let Some(area) = fan_areas[i] {
                let area = &mut self.areas[area];
                for corner in [&mut area.a, &mut area.b, &mut area.c] {
                    if *corner == point {
                        *corner = copy;
                    }
                }
            }
        }

        // The groups of the fan's triangles holding each neighbor.
        let holders = |neighbor: usize| -> Vec<usize> {
            let mut held: Vec<usize> = (0..count)
                .filter(|&i| neighbor != point && fan_corners[i].contains(&neighbor))
                .map(|i| group[i])
                .collect();
            held.sort_unstable();
            held.dedup();
            held
        };
        let toward_point = |target: Vec2| -> usize {
            let offset = subtract(target, source.home);
            let mut best = (f64::MIN, keeper);
            for (g, direction) in toward.iter().enumerate() {
                let score = dot(offset, *direction) / hypot(direction.x, direction.y).max(EPSILON);
                if score > best.0 {
                    best = (score, g);
                }
            }
            best.1
        };

        // Springs follow the group holding their other end. An edge held by
        // triangles of two groups is a lip of the opening: each group gets
        // its own, and the two are twins across it.
        for index in 0..self.springs.len() {
            let spring = self.springs[index];
            if spring.layer != source.layer || (spring.a != point && spring.b != point) {
                continue;
            }
            let other = if spring.a == point {
                spring.b
            } else {
                spring.a
            };
            let held = holders(other);
            let first = held
                .first()
                .copied()
                .unwrap_or_else(|| toward_point(self.points[other].home));
            replace_end(&mut self.springs[index], point, member[first]);
            for &g in held.iter().skip(1) {
                let mut lip = self.springs[index];
                replace_end(&mut lip, member[first], member[g]);
                lip.impulse = 0.0;
                let lip_index = self.push_spring(
                    lip,
                    SpringWear {
                        twin: index,
                        ..self.spring_wear[index]
                    },
                );
                self.spring_wear[index].twin = lip_index;
                for i in (0..count).filter(|&i| group[i] == g) {
                    replace_edge(&mut self.triangles[fan[i]], index, lip_index);
                    if let Some(area) = fan_areas[i] {
                        let area = &mut self.areas[area];
                        for edge in [&mut area.edge_ab, &mut area.edge_bc, &mut area.edge_ca] {
                            if *edge == index {
                                *edge = lip_index;
                            }
                        }
                    }
                }
            }
        }

        // A long fiber passing over the point from one side of the opening
        // to the other is cut through.
        for index in 0..self.springs.len() {
            let spring = self.springs[index];
            if spring.broken
                || spring.layer != source.layer
                || member.contains(&spring.a)
                || member.contains(&spring.b)
            {
                continue;
            }
            let (held_a, held_b) = (holders(spring.a), holders(spring.b));
            if held_a.is_empty()
                || held_b.is_empty()
                || held_a.iter().any(|g| held_b.contains(g))
                || self.share_triangle(spring.a, spring.b, source.layer)
            {
                continue;
            }
            self.springs[index].broken = true;
        }

        // Skin stays tied to the muscle on its own side, and both sides of
        // torn muscle stay tied to the bone. Each copy holds the fat over no
        // more than its share of the flesh, as it holds its share of the mass.
        let mut held_fat = vec![0.0; groups];
        let mut all_fat = 0.0;
        for index in 0..self.attachments.len() {
            let attachment = self.attachments[index];
            let (own, other) = match source.layer {
                TissueLayer::Skin => (attachment.skin_point, attachment.muscle_point),
                TissueLayer::Muscle => (attachment.muscle_point, attachment.skin_point),
            };
            if own != point {
                continue;
            }
            let group = toward_point(self.points[other].home);
            let copy = member[group];
            match source.layer {
                TissueLayer::Skin => self.attachments[index].skin_point = copy,
                TissueLayer::Muscle => self.attachments[index].muscle_point = copy,
            }
            held_fat[group] += attachment.stiffness;
            all_fat += attachment.stiffness;
        }
        for (group, &held) in held_fat.iter().enumerate() {
            let copy = member[group];
            if held > EPSILON {
                let share = self.points[copy].mass / source.mass.max(EPSILON);
                self.share_fat_ties(copy, share * all_fat / held);
            }
        }
        let anchors: Vec<BoneAttachment> = self
            .bone_attachments
            .iter()
            .filter(|attachment| attachment.point == point)
            .copied()
            .collect();
        for &copy in member.iter().filter(|&&copy| copy != point) {
            for &anchor in &anchors {
                self.bone_attachments.push(BoneAttachment {
                    point: copy,
                    ..anchor
                });
            }
        }

        self.follow_split_outline(point, &member);
        self.stats.cut_openings += (groups - 1) as i32;
        self.note(
            EventKind::CutOpened,
            Subject::Point(point),
            self.points[point].position,
            None,
            Measure::Count,
            (groups - 1) as f64,
            0.0,
        );
        Some(member.into_iter().zip(toward).collect())
    }

    /// Whether any triangle of `layer` has both points as corners.
    fn share_triangle(&self, a: usize, b: usize, layer: TissueLayer) -> bool {
        self.triangles.iter().any(|triangle| {
            triangle.layer == layer
                && corners(triangle).contains(&a)
                && corners(triangle).contains(&b)
        })
    }

    /// The area constraint over triangle `index`: the one at the same index
    /// when they were built together, as the body builds them.
    fn area_of_triangle(&self, index: usize) -> Option<usize> {
        let triangle = self.triangles[index];
        let mut want = corners(&triangle);
        want.sort_unstable();
        let matches = |area: &AreaConstraint| {
            let mut have = [area.a, area.b, area.c];
            have.sort_unstable();
            area.layer == triangle.layer && have == want
        };
        if self.areas.get(index).is_some_and(matches) {
            return Some(index);
        }
        self.areas.iter().position(matches)
    }

    /// Keeps the rest lengths, areas, and attachments of everything joined
    /// to a point whose rest place moved in step with it, so the new rest
    /// shape is the one the tissue settles into. `before` holds the rest
    /// places before the move; a copy made since moved from where its
    /// source was (`origin`, in order of the copies), which may itself be a
    /// copy made earlier.
    fn follow_rest_places(&mut self, before: &[Vec2], origin: &[usize]) {
        let mut old: Vec<Vec2> = Vec::with_capacity(self.points.len());
        for index in 0..self.points.len() {
            let was = match index.checked_sub(before.len()) {
                None => before[index],
                Some(copy) => old[origin[copy]],
            };
            old.push(was);
        }
        let homes: Vec<Vec2> = self.points.iter().map(|point| point.home).collect();
        let moved: Vec<bool> = old
            .iter()
            .zip(&homes)
            .map(|(&was, &now)| distance(was, now) > 1.0e-9)
            .collect();
        if !moved.contains(&true) {
            return;
        }
        let ratio = |a: usize, b: usize| {
            let was = distance(old[a], old[b]);
            let now = distance(homes[a], homes[b]);
            (was > EPSILON && now > EPSILON).then(|| now / was)
        };
        for (spring, wear) in self.springs.iter_mut().zip(&mut self.spring_wear) {
            if !(moved[spring.a] || moved[spring.b]) {
                continue;
            }
            if let Some(ratio) = ratio(spring.a, spring.b) {
                spring.rest *= ratio;
                wear.rest_reference *= ratio;
            }
        }
        for area in &mut self.areas {
            if !(moved[area.a] || moved[area.b] || moved[area.c]) {
                continue;
            }
            let was = signed_area(old[area.a], old[area.b], old[area.c]);
            let now = signed_area(homes[area.a], homes[area.b], homes[area.c]);
            // A triangle the move would turn over keeps its old rest area.
            if was.abs() > EPSILON && now / was > 0.05 {
                area.rest_area *= now / was;
            }
        }
        // Attachments are built at their rest distance, with a floor, and do
        // not flow, so they take the new distance outright: scaling a floored
        // rest would multiply the floor.
        let spacing = self.materials.point_spacing;
        for attachment in &mut self.attachments {
            if !(moved[attachment.skin_point] || moved[attachment.muscle_point]) {
                continue;
            }
            attachment.rest =
                distance(homes[attachment.skin_point], homes[attachment.muscle_point])
                    .max(spacing * 0.34);
        }
        for attachment in &mut self.bone_attachments {
            if attachment.point >= moved.len() || !moved[attachment.point] {
                continue;
            }
            attachment.offset = add(
                attachment.offset,
                subtract(homes[attachment.point], old[attachment.point]),
            );
            attachment.rest = hypot(attachment.offset.x, attachment.offset.y).max(spacing * 0.42);
        }
    }
}

fn corners(triangle: &Triangle) -> [usize; 3] {
    [triangle.a, triangle.b, triangle.c]
}

fn edge_slots(triangle: &Triangle) -> [usize; 3] {
    [triangle.edge_ab, triangle.edge_bc, triangle.edge_ca]
}

fn set_edge_slot(triangle: &mut Triangle, side: usize, edge: usize) {
    match side {
        0 => triangle.edge_ab = edge,
        1 => triangle.edge_bc = edge,
        _ => triangle.edge_ca = edge,
    }
}

/// Whether side `side` of a triangle (0 from `a` to `b`, 1 from `b` to `c`,
/// 2 from `c` to `a`) ends at corner `corner`.
fn side_has(side: usize, corner: usize) -> bool {
    corner == side || corner == (side + 1) % 3
}

/// The weights on a triangle's corners of the place `at` of the way along
/// its side `side`.
fn along_side(side: usize, at: f64) -> [f64; 3] {
    let mut weights = [0.0; 3];
    weights[side] = 1.0 - at;
    weights[(side + 1) % 3] = at;
    weights
}

/// Whether `triangle` holds the flesh at its corner `node`.
fn holds_node(triangle: &Triangle, node: usize) -> bool {
    corners(triangle)
        .iter()
        .position(|&corner| corner == node)
        .is_some_and(|corner| triangle.cut.map_or(true, |cut| cut.holds_corner(corner)))
}

/// The stretch of `triangle`'s side from `from` to `to` that its flesh
/// covers, as shares of the way from `from`.
fn held_between(triangle: &Triangle, from: usize, to: usize) -> Option<(f64, f64)> {
    let corners = corners(triangle);
    let side = (0..3).find(|&side| {
        let (p, q) = (corners[side], corners[(side + 1) % 3]);
        (p == from && q == to) || (p == to && q == from)
    })?;
    let (start, end) = match triangle.cut {
        Some(cut) => cut.holds_along(side)?,
        None => (0.0, 1.0),
    };
    if corners[side] == from {
        Some((start, end))
    } else {
        Some((1.0 - end, 1.0 - start))
    }
}

fn replace_corner(triangle: &mut Triangle, from: usize, to: usize) {
    for corner in [&mut triangle.a, &mut triangle.b, &mut triangle.c] {
        if *corner == from {
            *corner = to;
        }
    }
}

fn replace_edge(triangle: &mut Triangle, from: usize, to: usize) {
    for edge in [
        &mut triangle.edge_ab,
        &mut triangle.edge_bc,
        &mut triangle.edge_ca,
    ] {
        if *edge == from {
            *edge = to;
        }
    }
}

fn replace_end(spring: &mut Spring, from: usize, to: usize) {
    if spring.a == from {
        spring.a = to;
    } else if spring.b == from {
        spring.b = to;
    }
}

fn find_root(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH_SPACING: f64 = 10.0;
    const PATCH_PRESTRETCH: f64 = 1.12;

    /// A sheet of skin built like the body's: a grid of points joined by a
    /// spring along every triangle edge, with the triangles and their area
    /// constraints, held at its top and bottom rows and stretched between
    /// them as living skin is round a limb.
    fn skin_patch(columns: usize, rows: usize) -> World {
        stretched_skin_patch(columns, rows, PATCH_PRESTRETCH)
    }

    /// The same, stretched `stretch` times between its held rows.
    fn stretched_skin_patch(columns: usize, rows: usize, stretch: f64) -> World {
        let materials = Materials {
            gravity: 0.0,
            // Tears spreading from a loaded cut are their own rule; these
            // tests look at the cut alone.
            max_tear_propagations_per_step: 0,
            ..Materials::default()
        };
        let mut world = World::new(materials);
        let m = world.materials;
        for row in 0..rows {
            for column in 0..columns {
                let position = Vec2 {
                    x: 200.0 + column as f64 * PATCH_SPACING,
                    y: 80.0 + row as f64 * PATCH_SPACING,
                };
                world.add_point(position, TissueLayer::Skin, row == 0 || row == rows - 1);
            }
        }
        let at = |column: usize, row: usize| row * columns + column;
        let mut cells = Vec::new();
        for row in 0..rows - 1 {
            for column in 0..columns - 1 {
                cells.push((
                    at(column, row),
                    at(column + 1, row),
                    at(column, row + 1),
                    at(column + 1, row + 1),
                ));
            }
        }
        for &(here, right, below, across) in &cells {
            for (a, b) in [
                (here, right),
                (here, below),
                (right, below),
                (right, across),
                (below, across),
            ] {
                world.add_spring(
                    a,
                    b,
                    TissueLayer::Skin,
                    1.0,
                    m.skin_tear_stretch,
                    m.skin_tear_impulse,
                    false,
                );
            }
        }
        for &(here, right, below, across) in &cells {
            for [a, b, c] in [[here, right, below], [right, across, below]] {
                world.add_triangle(a, b, c, TissueLayer::Skin);
                world.add_area(a, b, c, TissueLayer::Skin, 1.0);
            }
        }
        for (spring, wear) in world.springs.iter_mut().zip(&mut world.spring_wear) {
            let (a, b) = (world.points[spring.a].home, world.points[spring.b].home);
            let along = normalized(subtract(b, a), Vec2 { x: 1.0, y: 0.0 });
            let by = 1.0 + (stretch - 1.0) * along.y * along.y;
            spring.rest /= by;
            wear.rest_reference = spring.rest;
            wear.prestretch = by;
        }
        world
    }

    /// Draws the knife across the patch from left to right at height `y`,
    /// then takes it away and lets the patch settle.
    fn slash(world: &mut World, y: f64) {
        let dt = world.materials.fixed_dt;
        let mut hand = 120.0;
        for _ in 0..45 {
            hand += 700.0 * dt;
            let input = InputState {
                active: true,
                down: true,
                x: hand,
                y,
                vx: 0.0,
                vy: 0.0,
                power: swing_power(ToolMode::Sharp),
                tool: ToolMode::Sharp,
            };
            world.step(dt, &input, 640.0);
        }
        for _ in 0..60 {
            world.step(dt, &InputState::default(), 640.0);
        }
    }

    fn assert_mesh_consistent(world: &World) {
        let check = |what: &str, corners: [usize; 3], edges: [usize; 3]| {
            let [a, b, c] = corners;
            for (edge, x, y) in [(edges[0], a, b), (edges[1], b, c), (edges[2], c, a)] {
                if edge == MISSING_SPRING {
                    continue;
                }
                let spring = world.springs[edge];
                assert!(
                    (spring.a == x && spring.b == y) || (spring.a == y && spring.b == x),
                    "{what}: edge {edge} joins {}-{} instead of {x}-{y}",
                    spring.a,
                    spring.b
                );
            }
        };
        for (index, t) in world.triangles.iter().enumerate() {
            check(
                &format!("triangle {index}"),
                [t.a, t.b, t.c],
                [t.edge_ab, t.edge_bc, t.edge_ca],
            );
            if let Some(cut) = t.cut {
                let partner = world.triangles[cut.partner];
                assert_eq!(
                    partner.cut.map(|other| other.partner),
                    Some(index),
                    "triangle {index}'s partner"
                );
            }
        }
        for (index, area) in world.areas.iter().enumerate() {
            check(
                &format!("area {index}"),
                [area.a, area.b, area.c],
                [area.edge_ab, area.edge_bc, area.edge_ca],
            );
        }
        for (index, wear) in world.spring_wear.iter().enumerate() {
            if wear.twin != MISSING_SPRING {
                assert_eq!(
                    world.spring_wear[wear.twin].twin, index,
                    "spring {index}'s twin"
                );
            }
        }
    }

    /// How far each opened cut's lips lie from the line `y` in the rest
    /// shape, taking the middle between the two lips.
    fn farthest_off(world: &World, y: f64) -> f64 {
        world
            .opened_cuts()
            .iter()
            .flat_map(|cut| {
                let [first, second] = cut.rest_lips;
                [midpoint(first.0, second.0).y, midpoint(first.1, second.1).y]
            })
            .map(|lip_y| (lip_y - y).abs())
            .fold(0.0, f64::max)
    }

    #[test]
    fn knife_cut_opens_along_the_blades_path_and_keeps_the_skin_whole() {
        let mut world = skin_patch(12, 10);
        let points_before = world.points.len();
        let mass_before: f64 = world.points.iter().map(|point| point.mass).sum();
        // The blade runs between rows, nearer the fifth than the sixth: the
        // cut runs where it went, not along either row.
        let cut_y = 124.0;
        slash(&mut world, cut_y);

        assert!(world.stats.broken_skin > 0, "the knife should cut the skin");
        assert!(
            world.stats.cut_openings > 0 && world.points.len() > points_before,
            "the cut should split points to open"
        );
        let mass_after: f64 = world.points.iter().map(|point| point.mass).sum();
        assert!(
            (mass_after - mass_before).abs() < 1.0e-9,
            "splitting should keep the mass: {mass_before} became {mass_after}"
        );
        assert!(
            world.triangles.iter().all(|t| world.triangle_alive(t)),
            "a clean cut parts the skin without destroying any of it"
        );
        assert_mesh_consistent(&world);

        let cuts = world.opened_cuts();
        assert!(!cuts.is_empty(), "the cut should have lips");
        let off = farthest_off(&world, cut_y);
        assert!(
            off < PATCH_SPACING * 0.2,
            "the cut should run along the blade's path, not {off:.2} px off it"
        );
        let widest = cuts
            .iter()
            .flat_map(|cut| {
                let [first, second] = cut.lips;
                [distance(first.0, second.0), distance(first.1, second.1)]
            })
            .fold(0.0, f64::max);
        assert!(
            widest > PATCH_SPACING * 0.2,
            "the skin's tension should open the cut once the knife has gone: widest {widest:.2} px"
        );
    }

    #[test]
    fn knife_just_inside_the_edge_opens_there() {
        // A free edge, held only by the rest of the patch: unstretched, as
        // nothing here stands for the skin running on beyond it.
        let mut world = stretched_skin_patch(12, 10, 1.0);
        for point in 0..12 {
            world.points[point].pinned = false;
        }
        // The blade runs three pixels inside the patch's top edge.
        let cut_y = 83.0;
        slash(&mut world, cut_y);
        assert!(
            world.stats.cut_openings > 0,
            "a cut just inside the edge should open"
        );
        assert_mesh_consistent(&world);
        let off = farthest_off(&world, cut_y);
        assert!(
            off < PATCH_SPACING * 0.15,
            "the cut should open where the blade went, not {off:.2} px off it"
        );
    }

    #[test]
    fn ghosts_hold_no_flesh_and_nothing_is_tied_to_them() {
        let mut world = skin_patch(12, 10);
        slash(&mut world, 124.0);
        let ghosts: Vec<usize> = (0..world.points.len())
            .filter(|&index| world.points[index].is_ghost())
            .collect();
        assert!(!ghosts.is_empty(), "the cut should make ghosts");
        let flesh = world.flesh_points();
        for &ghost in &ghosts {
            assert!(!flesh[ghost], "ghost {ghost} should hold no flesh");
            assert!(
                world.points[ghost].mass < world.points[world.points[ghost].ghost_of].mass,
                "ghost {ghost} should weigh less than its point"
            );
            assert!(
                world
                    .attachments
                    .iter()
                    .all(|a| a.skin_point != ghost && a.muscle_point != ghost),
                "nothing should be tied to ghost {ghost}"
            );
        }
    }

    #[test]
    fn a_piece_holds_only_its_side_of_the_cut() {
        let mut triangle = Triangle::default();
        let cut = TriangleCut {
            sides: [0, 2],
            at: [0.5, 0.5],
            holds_shared_corner: true,
            partner: 1,
        };
        triangle.cut = Some(cut);
        // Corner `a` is shared by sides 0 and 2: this piece is the corner's
        // triangle, half the triangle's sides across.
        assert_eq!(triangle.piece().corners().len(), 3);
        assert!(triangle.holds([0.9, 0.05, 0.05]));
        assert!(!triangle.holds([0.1, 0.45, 0.45]));
        let (share, _) = triangle.piece().share_and_centroid();
        assert!((share - 0.25).abs() < 1.0e-12);
        triangle.cut = Some(TriangleCut {
            holds_shared_corner: false,
            ..cut
        });
        assert_eq!(triangle.piece().corners().len(), 4);
        assert!(!triangle.holds([0.9, 0.05, 0.05]));
        assert!(triangle.holds([0.1, 0.45, 0.45]));
        let (share, _) = triangle.piece().share_and_centroid();
        assert!((share - 0.75).abs() < 1.0e-12);
    }
}
