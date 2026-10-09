//! A uniform grid for finding what lies near a box or a point, rebuilt in
//! place from flat arrays with a counting sort, after Ten Minute Physics
//! tutorial 11 (Matthias Müller, "Spatial Hashing", MIT licence): count the
//! entries of each cell, turn the counts into where each cell's entries
//! start, then write the entries into their cells. The cells are indexed
//! directly over the area the entries cover rather than hashed, since the
//! body stays inside the window, and the arrays are kept from one build to
//! the next so a build allocates nothing.
//!
//! A query lists what its cells hold row by row and, within a cell, in the
//! order the entries were added, each entry once: the order the `HashMap`
//! grids this replaced gave. The contact passes that use it resolve their
//! contacts one after another, so the order is part of the physics.

use super::{Aabb, Vec2};

/// How far from the origin the grid reaches, in cells either way: far past
/// any window. Anything farther out, which only a simulation that blew up
/// could produce, is kept in the outermost cells instead, so the grid stays
/// small and a query stays short.
const GRID_REACH: i32 = 512;

/// The cells a box touches, inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellRange {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl CellRange {
    fn at(x: i32, y: i32) -> Self {
        Self {
            x0: x,
            y0: y,
            x1: x,
            y1: y,
        }
    }

    fn covering(aabb: Aabb, cell_size: f64) -> Self {
        let (ax, ay) = cell_of(aabb.min, cell_size);
        let (bx, by) = cell_of(aabb.max, cell_size);
        Self {
            x0: ax.min(bx),
            y0: ay.min(by),
            x1: ax.max(bx),
            y1: ay.max(by),
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }
}

/// The cell holding `point`.
fn cell_of(point: Vec2, cell_size: f64) -> (i32, i32) {
    let coordinate =
        |value: f64| ((value / cell_size).floor() as i32).clamp(-GRID_REACH, GRID_REACH - 1);
    (coordinate(point.x), coordinate(point.y))
}

#[derive(Clone, Debug, Default)]
pub(super) struct SpatialGrid {
    cell_size: f64,
    /// The grid's first column and row, in cells from the origin, and its
    /// size in cells; no columns means an empty grid.
    left: i32,
    top: i32,
    columns: i32,
    rows: i32,
    /// Where each cell's entries start in `entries`, row by row, with the
    /// total last, so a cell's entries end where the next cell's start.
    cell_start: Vec<u32>,
    entries: Vec<u32>,
    /// The items being sorted in: each id and the cells it touches.
    items: Vec<(u32, CellRange)>,
    /// Some item touches more than one cell, so a query can meet it twice;
    /// `seen` holds the query mark of the last query that listed each id.
    spans_cells: bool,
    seen: Vec<u32>,
    query_mark: u32,
}

impl SpatialGrid {
    /// Sorts points, each with its id, into cells `cell_size` wide.
    pub(super) fn build_points(
        &mut self,
        cell_size: f64,
        points: impl IntoIterator<Item = (usize, Vec2)>,
    ) {
        self.cell_size = cell_size.max(1.0);
        self.items.clear();
        for (id, point) in points {
            let (x, y) = cell_of(point, self.cell_size);
            self.items.push((id as u32, CellRange::at(x, y)));
        }
        self.sort_items();
    }

    /// Sorts boxes, each with its id, into cells `cell_size` wide; a box goes
    /// in every cell it touches.
    pub(super) fn build_boxes(
        &mut self,
        cell_size: f64,
        boxes: impl IntoIterator<Item = (usize, Aabb)>,
    ) {
        self.cell_size = cell_size.max(1.0);
        self.items.clear();
        for (id, aabb) in boxes {
            let range = CellRange::covering(aabb, self.cell_size);
            self.items.push((id as u32, range));
        }
        self.sort_items();
    }

    /// The counting sort: counts what each cell holds, turns the counts into
    /// where each cell's entries end, then writes the items in from the last,
    /// moving each cell's mark back to where its entries start, so each cell
    /// lists its items in the order they were added.
    fn sort_items(&mut self) {
        let Some(bounds) = self
            .items
            .iter()
            .map(|&(_, range)| range)
            .reduce(CellRange::union)
        else {
            self.columns = 0;
            self.rows = 0;
            self.spans_cells = false;
            return;
        };
        let (left, top) = (bounds.x0, bounds.y0);
        let columns = bounds.x1 - bounds.x0 + 1;
        let rows = bounds.y1 - bounds.y0 + 1;
        let cells = (columns * rows) as usize;
        let cell = |x: i32, y: i32| ((y - top) * columns + (x - left)) as usize;

        self.cell_start.clear();
        self.cell_start.resize(cells + 1, 0);
        let mut spans_cells = false;
        let mut id_limit = 0;
        for &(id, range) in &self.items {
            spans_cells |= range.x0 != range.x1 || range.y0 != range.y1;
            id_limit = id_limit.max(id as usize + 1);
            for y in range.y0..=range.y1 {
                for x in range.x0..=range.x1 {
                    self.cell_start[cell(x, y)] += 1;
                }
            }
        }
        let mut end = 0;
        for start in &mut self.cell_start {
            end += *start;
            *start = end;
        }
        self.entries.clear();
        self.entries.resize(end as usize, 0);
        for &(id, range) in self.items.iter().rev() {
            for y in range.y0..=range.y1 {
                for x in range.x0..=range.x1 {
                    let start = &mut self.cell_start[cell(x, y)];
                    *start -= 1;
                    self.entries[*start as usize] = id;
                }
            }
        }

        self.left = left;
        self.top = top;
        self.columns = columns;
        self.rows = rows;
        self.spans_cells = spans_cells;
        if spans_cells && self.seen.len() < id_limit {
            self.seen.resize(id_limit, 0);
        }
    }

    /// Lists in `found` the ids in every cell `aabb` touches: row by row,
    /// each cell's in the order they were added, each id once.
    pub(super) fn query(&mut self, aabb: Aabb, found: &mut Vec<usize>) {
        found.clear();
        if self.columns == 0 {
            return;
        }
        let range = CellRange::covering(aabb, self.cell_size);
        let x0 = range.x0.max(self.left);
        let x1 = range.x1.min(self.left + self.columns - 1);
        let y0 = range.y0.max(self.top);
        let y1 = range.y1.min(self.top + self.rows - 1);
        if x0 > x1 || y0 > y1 {
            return;
        }
        let mark = self.next_query_mark();
        let (first, last) = ((x0 - self.left) as usize, (x1 - self.left) as usize);
        for y in y0..=y1 {
            // A row's cells sit side by side in `entries`, so the cells from
            // x0 to x1 are one run.
            let row = ((y - self.top) * self.columns) as usize;
            let from = self.cell_start[row + first] as usize;
            let to = self.cell_start[row + last + 1] as usize;
            for &id in &self.entries[from..to] {
                if self.spans_cells {
                    let seen = &mut self.seen[id as usize];
                    if *seen == mark {
                        continue;
                    }
                    *seen = mark;
                }
                found.push(id as usize);
            }
        }
    }

    /// The ids in the cell holding `point`, in the order they were added.
    pub(super) fn at(&self, point: Vec2) -> &[u32] {
        let (x, y) = cell_of(point, self.cell_size);
        let (column, row) = (x - self.left, y - self.top);
        if !(0..self.columns).contains(&column) || !(0..self.rows).contains(&row) {
            return &[];
        }
        let cell = (row * self.columns + column) as usize;
        &self.entries[self.cell_start[cell] as usize..self.cell_start[cell + 1] as usize]
    }

    fn next_query_mark(&mut self) -> u32 {
        self.query_mark = self.query_mark.wrapping_add(1);
        if self.query_mark == 0 {
            self.seen.fill(0);
            self.query_mark = 1;
        }
        self.query_mark
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    /// The grid this replaced: a `HashMap` from cell to the ids in it.
    struct HashGrid {
        cell_size: f64,
        cells: HashMap<(i32, i32), Vec<usize>>,
    }

    fn hash_key(point: Vec2, cell_size: f64) -> (i32, i32) {
        let cell_size = cell_size.max(1.0);
        (
            (point.x / cell_size).floor() as i32,
            (point.y / cell_size).floor() as i32,
        )
    }

    fn hash_cells(aabb: Aabb, cell_size: f64, mut visit: impl FnMut((i32, i32))) {
        let (ax, ay) = hash_key(aabb.min, cell_size);
        let (bx, by) = hash_key(aabb.max, cell_size);
        for y in ay.min(by)..=ay.max(by) {
            for x in ax.min(bx)..=ax.max(bx) {
                visit((x, y));
            }
        }
    }

    impl HashGrid {
        fn of_boxes(cell_size: f64, boxes: &[(usize, Aabb)]) -> Self {
            let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
            for &(id, aabb) in boxes {
                hash_cells(aabb, cell_size, |key| {
                    cells.entry(key).or_default().push(id)
                });
            }
            Self { cell_size, cells }
        }

        fn query(&self, aabb: Aabb) -> Vec<usize> {
            let mut seen = HashSet::new();
            let mut found = Vec::new();
            hash_cells(aabb, self.cell_size, |key| {
                for &id in self.cells.get(&key).map_or(&[][..], Vec::as_slice) {
                    if seen.insert(id) {
                        found.push(id);
                    }
                }
            });
            found
        }
    }

    /// A small deterministic random source for the cases below.
    struct Lcg(u64);

    impl Lcg {
        fn unit(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }

        fn between(&mut self, low: f64, high: f64) -> f64 {
            low + (high - low) * self.unit()
        }

        fn point(&mut self) -> Vec2 {
            Vec2 {
                x: self.between(-40.0, 700.0),
                y: self.between(-60.0, 600.0),
            }
        }

        fn aabb(&mut self, largest: f64) -> Aabb {
            let a = self.point();
            Aabb {
                min: a,
                max: Vec2 {
                    x: a.x + self.between(0.0, largest),
                    y: a.y + self.between(0.0, largest),
                },
            }
        }
    }

    #[test]
    fn point_queries_match_the_hash_grid() {
        let mut random = Lcg(7);
        let mut grid = SpatialGrid::default();
        let mut found = Vec::new();
        for round in 0..20 {
            let cell_size = [18.4, 23.0, 8.0, 39.0][round % 4];
            let points: Vec<(usize, Vec2)> = (0..900)
                .filter(|index| index % 7 != round % 7)
                .map(|index| (index, random.point()))
                .collect();
            let boxes: Vec<(usize, Aabb)> = points
                .iter()
                .map(|&(id, point)| {
                    (
                        id,
                        Aabb {
                            min: point,
                            max: point,
                        },
                    )
                })
                .collect();
            let reference = HashGrid::of_boxes(cell_size, &boxes);
            grid.build_points(cell_size, points.iter().copied());
            for _ in 0..200 {
                let aabb = random.aabb(90.0);
                grid.query(aabb, &mut found);
                assert_eq!(found, reference.query(aabb));
            }
            for &(_, point) in &points {
                let cell: Vec<usize> = grid.at(point).iter().map(|&id| id as usize).collect();
                assert_eq!(cell, reference.cells[&hash_key(point, cell_size)]);
            }
        }
    }

    #[test]
    fn box_queries_match_the_hash_grid() {
        let mut random = Lcg(11);
        let mut grid = SpatialGrid::default();
        let mut found = Vec::new();
        for round in 0..20 {
            let cell_size = [42.0, 39.0, 23.0, 8.0][round % 4];
            let boxes: Vec<(usize, Aabb)> = (0..60)
                .filter(|index| index % 5 != round % 5)
                .map(|index| (index * 3, random.aabb(120.0)))
                .collect();
            let reference = HashGrid::of_boxes(cell_size, &boxes);
            grid.build_boxes(cell_size, boxes.iter().copied());
            for _ in 0..200 {
                let aabb = random.aabb(150.0);
                grid.query(aabb, &mut found);
                assert_eq!(found, reference.query(aabb));
            }
            for _ in 0..200 {
                let point = random.point();
                let cell: Vec<usize> = grid.at(point).iter().map(|&id| id as usize).collect();
                let expected = reference
                    .cells
                    .get(&hash_key(point, cell_size))
                    .cloned()
                    .unwrap_or_default();
                assert_eq!(cell, expected);
            }
        }
    }

    #[test]
    fn empty_and_runaway_entries_stay_safe() {
        let mut grid = SpatialGrid::default();
        let mut found = vec![3];
        grid.build_points(18.4, std::iter::empty());
        grid.query(
            Aabb {
                min: Vec2 { x: 0.0, y: 0.0 },
                max: Vec2 { x: 100.0, y: 100.0 },
            },
            &mut found,
        );
        assert!(found.is_empty());
        assert!(grid.at(Vec2 { x: 5.0, y: 5.0 }).is_empty());

        // Positions a blown-up simulation could hold land in the outermost
        // cells instead of stretching the grid without bound.
        let wild = [
            Vec2 {
                x: f64::NAN,
                y: 10.0,
            },
            Vec2 {
                x: f64::INFINITY,
                y: f64::NEG_INFINITY,
            },
            Vec2 {
                x: 1.0e12,
                y: -1.0e12,
            },
            Vec2 { x: 50.0, y: 50.0 },
        ];
        grid.build_points(18.4, wild.iter().copied().enumerate());
        assert!(grid.columns <= 2 * GRID_REACH && grid.rows <= 2 * GRID_REACH);
        grid.build_boxes(
            18.4,
            [(
                0,
                Aabb {
                    min: Vec2 {
                        x: f64::NEG_INFINITY,
                        y: 0.0,
                    },
                    max: Vec2 {
                        x: f64::INFINITY,
                        y: 10.0,
                    },
                },
            )],
        );
        grid.query(
            Aabb {
                min: Vec2 { x: 0.0, y: 0.0 },
                max: Vec2 { x: 1.0, y: 1.0 },
            },
            &mut found,
        );
        assert_eq!(found, vec![0]);
    }
}
