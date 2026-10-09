//! Cuts that open. A blade does not break the fibers it crosses: it opens the
//! tissue along the mesh edges nearest its path, so the two sides part and
//! the cut gapes, showing what lies beneath, while every triangle stays whole.
//!
//! Rebuilt from two published techniques:
//!
//! - Splitting points, as Rapier's soft bodies cut and tear (rapier2d 0.36,
//!   `tearing_cut.rs` and `tearing_particle_split.rs`, Apache-2.0), after the
//!   tearing in Müller et al., "Position Based Dynamics" (2007): each group of
//!   a point's triangles that no longer shares an edge with the others gets
//!   its own copy of the point, the point's mass is shared by area, and the
//!   springs follow their triangles.
//! - Node snapping, from Nienhuys and van der Stappen, "A surgery simulation
//!   supporting cuts and finite element deformation" (MICCAI 2001): the points
//!   nearest the blade's path move onto it in the rest shape, so a cut runs
//!   straight along the path instead of zigzagging through the mesh as
//!   Rapier's cuts do.
//!
//! Skin and muscle are under tension in a living body and pull back when cut,
//! so each lip's rest shape draws back from the cut, and the cut gapes.

use super::*;

/// The farthest a point's rest place moves onto a blade's path, in point
/// spacings, so a stray crossing cannot drag the mesh.
const MAX_SNAP: f64 = 0.6;

impl World {
    /// The blade crossed spring `index`, `cut_at` of the way from its `a`
    /// end. The fiber is marked cut; the cut opens when the knife's step ends
    /// ([`World::open_blade_cuts`]).
    pub(super) fn cut_spring(&mut self, index: usize, cut_at: f64) {
        let spring = &mut self.springs[index];
        if spring.broken || spring.cut {
            return;
        }
        spring.cut = true;
        spring.cut_at = cut_at.clamp(0.0, 1.0);
        if spring.layer == TissueLayer::Skin {
            self.open_skin.push(index);
        }
        self.pending_cuts.push(index);
    }

    /// Opens the cuts the blade made this step. Each triangle the blade has
    /// passed through, two of its edges cut, adds to the cut the edge between
    /// the points nearest its two crossings, and those points move onto the
    /// blade's path in the rest shape. Every point on the cut is then split,
    /// and each lip draws back from the cut. A cut fiber that no triangle
    /// uses, such as a lone test fiber, breaks instead.
    pub(super) fn open_blade_cuts(&mut self) {
        if self.pending_cuts.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_cuts);
        let mut fresh = vec![false; self.springs.len()];
        for &index in &pending {
            fresh[index] = true;
        }
        // How many triangles use each spring: none for a lone fiber, one on
        // the sheet's outline or a cut's lip.
        let mut sides = vec![0u8; self.springs.len()];
        let mut touched = Vec::new();
        for (index, triangle) in self.triangles.iter().enumerate() {
            let edges = [triangle.edge_ab, triangle.edge_bc, triangle.edge_ca];
            for &edge in &edges {
                if let Some(sides) = sides.get_mut(edge) {
                    *sides = sides.saturating_add(1);
                }
            }
            if edges.iter().any(|&edge| fresh.get(edge) == Some(&true)) {
                touched.push(index);
            }
        }
        for &index in &pending {
            if sides[index] == 0 {
                self.break_spring(index);
            }
        }
        // Points on the sheet's outline, where tissue lies on one side only.
        // A cut's own lips do not count, so a cut keeps running through the
        // points it has opened.
        let mut on_edge = vec![false; self.points.len()];
        for (spring, &count) in self.springs.iter().zip(&sides) {
            if count == 1 && spring.twin == MISSING_SPRING {
                on_edge[spring.a] = true;
                on_edge[spring.b] = true;
            }
        }

        let homes_before: Vec<Vec2> = self.points.iter().map(|point| point.home).collect();
        let mut opened = Vec::new();
        for index in touched {
            let triangle = self.triangles[index];
            let cut: Vec<usize> = [triangle.edge_ab, triangle.edge_bc, triangle.edge_ca]
                .into_iter()
                .filter(|&edge| {
                    self.springs
                        .get(edge)
                        .is_some_and(|spring| spring.cut && !spring.broken)
                })
                .collect();
            for i in 0..cut.len() {
                for j in i + 1..cut.len() {
                    let (a, crossing_i) = self.cut_end(cut[i], &on_edge);
                    let (b, crossing_j) = self.cut_end(cut[j], &on_edge);
                    self.snap_to_cut(a, crossing_i, crossing_j);
                    self.snap_to_cut(b, crossing_i, crossing_j);
                    if a != b {
                        if let Some(edge) = triangle_edge(&triangle, a, b) {
                            opened.push(edge);
                        }
                    }
                }
            }
        }
        if opened.is_empty() {
            self.follow_rest_places(&homes_before, &[]);
            return;
        }
        opened.sort_unstable();
        opened.dedup();
        let mut ends = Vec::with_capacity(opened.len() * 2);
        for &edge in &opened {
            self.springs[edge].parted = true;
            ends.push(self.springs[edge].a);
            ends.push(self.springs[edge].b);
        }
        ends.sort_unstable();
        ends.dedup();

        let first_copy = self.points.len();
        let mut origin = Vec::new();
        let mut families = Vec::new();
        for point in ends {
            if let Some(family) = self.split_point(point) {
                for &(member, _) in &family {
                    if member >= first_copy {
                        origin.push(point);
                    }
                }
                families.push(family);
            }
        }
        if families.is_empty() {
            self.follow_rest_places(&homes_before, &origin);
            return;
        }

        // Each lip draws back into its own side of the cut.
        let spacing = self.materials.point_spacing;
        for family in &families {
            for &(member, toward) in family {
                let retraction = match self.points[member].layer {
                    TissueLayer::Skin => self.materials.skin_cut_retraction,
                    TissueLayer::Muscle => self.materials.muscle_cut_retraction,
                } * spacing;
                let length = hypot(toward.x, toward.y);
                if length > EPSILON {
                    self.move_point_with_rest(member, scale(toward, retraction / length));
                }
            }
        }
        self.follow_rest_places(&homes_before, &origin);
        self.blunt_knock.resize(self.points.len(), 0.0);
        self.flesh.resize(self.points.len(), true);
        self.topology_version += 1;
    }

    /// The end of cut spring `index` the cut runs through, and where the
    /// blade crossed it in the rest shape. That is the end nearest the
    /// crossing, unless it lies on the sheet's outline (`on_edge`) and the
    /// other does not: a cut running just inside a limb's outline then opens along
    /// the first points inside it, cutting off a strip, rather than along the
    /// outline itself, which has tissue on one side only. A pinned end cannot
    /// open, so the other end stands in for it.
    fn cut_end(&self, index: usize, on_edge: &[bool]) -> (usize, Vec2) {
        let spring = self.springs[index];
        let (a, b) = (self.points[spring.a], self.points[spring.b]);
        let (near, far) = if spring.cut_at <= 0.5 {
            (spring.a, spring.b)
        } else {
            (spring.b, spring.a)
        };
        let edge = |point: usize| on_edge.get(point).copied().unwrap_or(false);
        let end = if a.pinned != b.pinned {
            if a.pinned {
                spring.b
            } else {
                spring.a
            }
        } else if edge(near) && !edge(far) {
            far
        } else {
            near
        };
        (end, lerp(a.home, b.home, spring.cut_at))
    }

    /// Moves point `index`'s rest place onto the line through two of a
    /// cut's crossings, unless it is pinned, already on a cut, or too far
    /// off; its cut springs then cross the cut at the point itself.
    fn snap_to_cut(&mut self, index: usize, from: Vec2, to: Vec2) {
        let point = self.points[index];
        if point.pinned || point.on_cut {
            return;
        }
        let line = subtract(to, from);
        let length_sq = dot(line, line);
        let target = if length_sq > EPSILON {
            add(
                from,
                scale(line, dot(subtract(point.home, from), line) / length_sq),
            )
        } else {
            from
        };
        if distance(target, point.home) > self.materials.point_spacing * MAX_SNAP {
            return;
        }
        self.move_point_with_rest(index, subtract(target, point.home));
        self.points[index].on_cut = true;
        // The blade's path now runs through the point, so a cut spring
        // ending there is crossed there, whichever end the blade was nearer.
        for spring in &mut self.springs {
            if !spring.cut {
                continue;
            }
            if spring.a == index {
                spring.cut_at = 0.0;
            } else if spring.b == index {
                spring.cut_at = 1.0;
            }
        }
    }

    /// Moves point `index` and its rest place by `offset`, keeping its
    /// velocity, so the edit leaves the tissue no more strained than it was;
    /// [`World::follow_rest_places`] then carries the rest lengths along.
    fn move_point_with_rest(&mut self, index: usize, offset: Vec2) {
        let point = &mut self.points[index];
        point.home = add(point.home, offset);
        point.position = add(point.position, offset);
        point.previous = add(point.previous, offset);
    }

    /// Splits point `point` along the parted edges through it: its triangles
    /// fall into groups that still share an unparted edge through it, and
    /// every group but the one with the most area gets its own copy of the
    /// point.
    /// Returns each copy (the point itself among them) with the direction, in
    /// the rest shape, from the point into its group's triangles; `None` when
    /// the point's triangles all still hold together.
    fn split_point(&mut self, point: usize) -> Option<Vec<(usize, Vec2)>> {
        let source = self.points[point];
        if source.pinned {
            return None;
        }
        let fan: Vec<usize> = (0..self.triangles.len())
            .filter(|&index| {
                let triangle = &self.triangles[index];
                triangle.layer == source.layer && corners(triangle).contains(&point)
            })
            .collect();
        if fan.len() < 2 {
            return None;
        }
        let count = fan.len();
        let mut parent: Vec<usize> = (0..count).collect();
        for i in 0..count {
            for j in i + 1..count {
                let (first, second) = (self.triangles[fan[i]], self.triangles[fan[j]]);
                let Some(other) = corners(&first)
                    .into_iter()
                    .find(|&corner| corner != point && corners(&second).contains(&corner))
                else {
                    continue;
                };
                let joined = triangle_edge(&first, point, other)
                    .is_some_and(|edge| !self.springs[edge].parted);
                if joined {
                    let (root_i, root_j) = (find_root(&mut parent, i), find_root(&mut parent, j));
                    parent[root_i.max(root_j)] = root_i.min(root_j);
                }
            }
        }
        let mut group_of_root = vec![usize::MAX; count];
        let mut groups = 0;
        let group: Vec<usize> = (0..count)
            .map(|i| {
                let root = find_root(&mut parent, i);
                if group_of_root[root] == usize::MAX {
                    group_of_root[root] = groups;
                    groups += 1;
                }
                group_of_root[root]
            })
            .collect();
        if groups < 2 {
            return None;
        }

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
                lip.lambda = 0.0;
                lip.twin = index;
                self.springs.push(lip);
                let lip_index = self.springs.len() - 1;
                self.springs[index].twin = lip_index;
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
        // cut muscle stay tied to the bone.
        for index in 0..self.attachments.len() {
            let attachment = self.attachments[index];
            let (own, other) = match source.layer {
                TissueLayer::Skin => (attachment.skin_point, attachment.muscle_point),
                TissueLayer::Muscle => (attachment.muscle_point, attachment.skin_point),
            };
            if own != point {
                continue;
            }
            let copy = member[toward_point(self.points[other].home)];
            match source.layer {
                TissueLayer::Skin => self.attachments[index].skin_point = copy,
                TissueLayer::Muscle => self.attachments[index].muscle_point = copy,
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
    /// source was (`origin`, in order of the copies).
    fn follow_rest_places(&mut self, before: &[Vec2], origin: &[usize]) {
        let old: Vec<Vec2> = (0..self.points.len())
            .map(|index| match index.checked_sub(before.len()) {
                None => before[index],
                Some(copy) => before[origin[copy]],
            })
            .collect();
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
        for spring in &mut self.springs {
            if !(moved[spring.a] || moved[spring.b]) {
                continue;
            }
            if let Some(ratio) = ratio(spring.a, spring.b) {
                spring.rest *= ratio;
                spring.rest_reference *= ratio;
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

/// The spring along the edge of `triangle` between corners `a` and `b`.
fn triangle_edge(triangle: &Triangle, a: usize, b: usize) -> Option<usize> {
    let pair = |x: usize, y: usize| (x == a && y == b) || (x == b && y == a);
    let edge = if pair(triangle.a, triangle.b) {
        triangle.edge_ab
    } else if pair(triangle.b, triangle.c) {
        triangle.edge_bc
    } else if pair(triangle.c, triangle.a) {
        triangle.edge_ca
    } else {
        return None;
    };
    (edge != MISSING_SPRING).then_some(edge)
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

    /// A sheet of skin built like the body's: a grid of points joined by a
    /// spring along every triangle edge, with the triangles and their area
    /// constraints, held at its top and bottom rows.
    fn skin_patch(columns: usize, rows: usize) -> World {
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
                    m.skin_structural_stiffness,
                    m.skin_tear_stretch,
                    m.skin_tear_impulse,
                    false,
                );
            }
        }
        for &(here, right, below, across) in &cells {
            for [a, b, c] in [[here, right, below], [right, across, below]] {
                world.add_triangle(a, b, c, TissueLayer::Skin);
                world.add_area(a, b, c, TissueLayer::Skin, m.skin_area_stiffness);
            }
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
            world.step(dt, &input, 640.0, 480.0);
        }
        for _ in 0..60 {
            world.step(dt, &InputState::default(), 640.0, 480.0);
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
        }
        for (index, area) in world.areas.iter().enumerate() {
            check(
                &format!("area {index}"),
                [area.a, area.b, area.c],
                [area.edge_ab, area.edge_bc, area.edge_ca],
            );
        }
        for (index, spring) in world.springs.iter().enumerate() {
            if spring.twin != MISSING_SPRING {
                assert_eq!(
                    world.springs[spring.twin].twin, index,
                    "spring {index}'s twin"
                );
            }
        }
    }

    #[test]
    fn knife_just_inside_the_edge_cuts_off_a_strip() {
        let mut world = skin_patch(12, 10);
        // A free edge, held only by the rest of the patch.
        let top_row: Vec<usize> = (0..12).collect();
        for &point in &top_row {
            world.points[point].pinned = false;
        }
        // The blade runs three pixels inside the patch's top edge, nearer the
        // edge's own points than the next row in.
        slash(&mut world, 83.0);
        assert!(
            world.stats.cut_openings > 0,
            "a cut just inside the edge should open along the first row inside it"
        );
        assert_mesh_consistent(&world);
        // The edge itself is not what opened: only where the knife came in
        // and went out through the patch's sides does the cut reach it.
        let edge_split = top_row
            .iter()
            .filter(|&&point| world.points[point].on_cut)
            .count();
        assert!(
            edge_split < top_row.len() / 2,
            "the cut should run inside the edge, not along it: {edge_split} edge points on it"
        );
    }

    #[test]
    fn knife_cut_opens_a_straight_gap_and_keeps_the_skin_whole() {
        let mut world = skin_patch(12, 10);
        let points_before = world.points.len();
        let mass_before: f64 = world.points.iter().map(|point| point.mass).sum();
        // The blade runs nearer the fifth row than the sixth, so the cut
        // opens along the fifth row, moved onto the blade's path.
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

        let lips: Vec<(usize, usize)> = world
            .springs
            .iter()
            .enumerate()
            .filter(|(index, spring)| spring.twin != MISSING_SPRING && *index < spring.twin)
            .map(|(index, spring)| (index, spring.twin))
            .collect();
        assert!(!lips.is_empty(), "the cut should have lips");
        let mut widest: f64 = 0.0;
        for &(lip, twin) in &lips {
            let (lip, twin) = (world.springs[lip], world.springs[twin]);
            for (here, there) in [(lip.a, twin.a), (lip.b, twin.b)] {
                let (here, there) = (world.points[here], world.points[there]);
                widest = widest.max(distance(here.position, there.position));
                // Both lips' rest places straddle the blade's path.
                let middle = midpoint(here.home, there.home);
                assert!(
                    (middle.y - cut_y).abs() < 2.5,
                    "the cut should run along the blade's path, not {:.1} px off it",
                    middle.y - cut_y
                );
            }
        }
        let spacing = world.materials.point_spacing;
        assert!(
            widest > world.materials.skin_cut_retraction * spacing,
            "the cut should gape once the knife has gone: widest {widest:.2} px"
        );
    }
}
