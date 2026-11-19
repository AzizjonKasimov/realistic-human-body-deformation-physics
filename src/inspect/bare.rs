//! Bone the drawing leaves bare: where a bone, drawn as the normal view
//! draws it beneath the flesh, lies outside every piece of flesh drawn over
//! it. The normal view draws each sheet of flesh whole over its own bones
//! before the next nearer sheet (`draw::passes`), so a bone shows wherever
//! neither its own sheet's muscle and skin nor any sheet drawn after it
//! covers it; both are drawn nearly opaque, so what this finds is what a
//! player sees. Read before a step or between two (`World::blend_motion`),
//! it finds bone poking out of a limb for a frame or two, which the
//! watchdogs, judging a bone's middle against any skin, miss.

use crate::draw::sheet_passes;
use crate::*;

/// A bone with part of its axis bare.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BareBone {
    /// The bone, by its place in [`World::bones`].
    pub bone: usize,
    /// How much of its axis lies bare, in pixels.
    pub bare: f64,
    /// The bare place farthest from its own sheet's flesh, and how far from
    /// it, in pixels.
    pub farthest: Vec2,
    pub out: f64,
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}

fn scale(value: Vec2, amount: f64) -> Vec2 {
    Vec2 {
        x: value.x * amount,
        y: value.y * amount,
    }
}

fn dot(a: Vec2, b: Vec2) -> f64 {
    a.x * b.x + a.y * b.y
}

fn lerp(a: Vec2, b: Vec2, t: f64) -> Vec2 {
    add(a, scale(sub(b, a), t))
}

/// The spacing of the places tested along a bone's axis, in pixels.
const SAMPLE_SPACING: f64 = 1.0;
/// The side of the grid's cells the flesh is filed in, in pixels.
const CELL: f64 = 12.0;

/// A piece of flesh drawn over bone: a triangle, or a cut's piece of one,
/// and the pass that draws it.
struct Cover {
    corners: [Vec2; 4],
    count: usize,
    pass: usize,
}

impl Cover {
    fn holds(&self, at: Vec2) -> bool {
        let corners = &self.corners[..self.count];
        let mut sign = 0.0;
        for (index, &from) in corners.iter().enumerate() {
            let to = corners[(index + 1) % corners.len()];
            let side = (to.x - from.x) * (at.y - from.y) - (to.y - from.y) * (at.x - from.x);
            if side == 0.0 {
                continue;
            }
            if sign == 0.0 {
                sign = side;
            } else if side * sign < 0.0 {
                return false;
            }
        }
        true
    }

    fn distance(&self, at: Vec2) -> f64 {
        let corners = &self.corners[..self.count];
        (0..corners.len())
            .map(|index| {
                let (from, to) = (corners[index], corners[(index + 1) % corners.len()]);
                let along = sub(to, from);
                let length = dot(along, along);
                let t = if length > 0.0 {
                    (dot(sub(at, from), along) / length).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                distance(at, add(from, scale(along, t)))
            })
            .fold(f64::INFINITY, f64::min)
    }
}

/// Every bone the normal view draws that shows, in part or whole, through
/// the flesh drawn over it, with how much of it and how far out.
pub fn bare_bones(world: &World) -> Vec<BareBone> {
    let points = world.points();
    let mut sheets: Vec<(u8, u8)> = points
        .iter()
        .map(|point| (point.body, point.sheet))
        .collect();
    sheets.sort_unstable();
    sheets.dedup();
    let passes = sheet_passes(world, &sheets);
    let pass_of = |body: u8, sheet: u8| {
        passes
            .iter()
            .position(|only| only.holds(body, sheet))
            .unwrap_or(0)
    };

    let mut covers: Vec<Cover> = Vec::new();
    for triangle in world.triangles() {
        if !matches!(triangle.layer, TissueLayer::Skin | TissueLayer::Muscle)
            || !world.triangle_alive(triangle)
        {
            continue;
        }
        let corner = [triangle.a, triangle.b, triangle.c].map(|index| points[index].position);
        let piece = triangle.piece();
        let mut cover = Cover {
            corners: [Vec2::default(); 4],
            count: 0,
            pass: pass_of(points[triangle.a].body, points[triangle.a].sheet),
        };
        for weights in piece.corners() {
            cover.corners[cover.count] = Vec2 {
                x: corner[0].x * weights[0] + corner[1].x * weights[1] + corner[2].x * weights[2],
                y: corner[0].y * weights[0] + corner[1].y * weights[1] + corner[2].y * weights[2],
            };
            cover.count += 1;
        }
        if cover.corners[..cover.count]
            .iter()
            .all(|corner| corner.x.is_finite() && corner.y.is_finite())
        {
            covers.push(cover);
        }
    }
    let grid = Grid::of(&covers);

    let mut bare = Vec::new();
    for (index, bone) in world.bones().iter().enumerate() {
        // The normal view leaves the spine undrawn, behind the trunk.
        if bone.kind == BoneKind::Spine
            || !(bone.a.x.is_finite() && bone.a.y.is_finite())
            || !(bone.b.x.is_finite() && bone.b.y.is_finite())
        {
            continue;
        }
        let pass = pass_of(bone.body, bone.sheet);
        let length = distance(bone.a, bone.b);
        let samples = (length / SAMPLE_SPACING).ceil().max(1.0) as usize;
        let mut found = BareBone {
            bone: index,
            bare: 0.0,
            farthest: bone.a,
            out: 0.0,
        };
        for sample in 0..=samples {
            let at = lerp(bone.a, bone.b, sample as f64 / samples as f64);
            let covered = grid
                .near(at)
                .any(|&cover| covers[cover].pass >= pass && covers[cover].holds(at));
            if covered {
                continue;
            }
            found.bare += length / samples as f64;
            let out = covers
                .iter()
                .filter(|cover| cover.pass == pass)
                .map(|cover| cover.distance(at))
                .fold(f64::INFINITY, f64::min);
            if out > found.out {
                found.out = out;
                found.farthest = at;
            }
        }
        if found.bare > 0.0 {
            bare.push(found);
        }
    }
    bare
}

/// The covers filed by the grid cells their bounds touch.
struct Grid {
    origin: Vec2,
    columns: usize,
    rows: usize,
    cells: Vec<Vec<usize>>,
}

impl Grid {
    fn of(covers: &[Cover]) -> Grid {
        let mut low = Vec2 {
            x: f64::INFINITY,
            y: f64::INFINITY,
        };
        let mut high = Vec2 {
            x: f64::NEG_INFINITY,
            y: f64::NEG_INFINITY,
        };
        for cover in covers {
            for corner in &cover.corners[..cover.count] {
                low = Vec2 {
                    x: low.x.min(corner.x),
                    y: low.y.min(corner.y),
                };
                high = Vec2 {
                    x: high.x.max(corner.x),
                    y: high.y.max(corner.y),
                };
            }
        }
        if covers.is_empty() {
            return Grid {
                origin: Vec2::default(),
                columns: 0,
                rows: 0,
                cells: Vec::new(),
            };
        }
        let columns = ((high.x - low.x) / CELL).floor() as usize + 1;
        let rows = ((high.y - low.y) / CELL).floor() as usize + 1;
        let mut grid = Grid {
            origin: low,
            columns,
            rows,
            cells: vec![Vec::new(); columns * rows],
        };
        for (index, cover) in covers.iter().enumerate() {
            let corners = &cover.corners[..cover.count];
            let (mut min, mut max) = (corners[0], corners[0]);
            for corner in corners {
                min = Vec2 {
                    x: min.x.min(corner.x),
                    y: min.y.min(corner.y),
                };
                max = Vec2 {
                    x: max.x.max(corner.x),
                    y: max.y.max(corner.y),
                };
            }
            let (column_from, row_from) = grid.cell(min);
            let (column_to, row_to) = grid.cell(max);
            for row in row_from..=row_to {
                for column in column_from..=column_to {
                    grid.cells[row * columns + column].push(index);
                }
            }
        }
        grid
    }

    fn cell(&self, at: Vec2) -> (usize, usize) {
        let column = ((at.x - self.origin.x) / CELL).floor().max(0.0) as usize;
        let row = ((at.y - self.origin.y) / CELL).floor().max(0.0) as usize;
        (
            column.min(self.columns.saturating_sub(1)),
            row.min(self.rows.saturating_sub(1)),
        )
    }

    /// The covers filed where `at` lies; none outside the grid.
    fn near(&self, at: Vec2) -> impl Iterator<Item = &usize> {
        let inside = self.columns > 0
            && at.x >= self.origin.x
            && at.y >= self.origin.y
            && at.x < self.origin.x + self.columns as f64 * CELL
            && at.y < self.origin.y + self.rows as f64 * CELL;
        let cell = if inside {
            let (column, row) = self.cell(at);
            &self.cells[row * self.columns + column][..]
        } else {
            &[][..]
        };
        cell.iter()
    }
}
