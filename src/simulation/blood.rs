//! Blood on the skin. Blood that wells out of a wound clings to the skin and
//! runs down it, leaving a trail, until it runs off the body's edge and drips
//! to the floor.
//!
//! Rebuilt from Overgrowth's blood (Wolfire Games, Apache-2.0): its
//! `BloodSurface` keeps each drip as a surface walker, a triangle of the
//! character's mesh and a place in it by barycentric weights, so the drip
//! moves as the mesh deforms; each update walks it down the surface from
//! triangle to triangle, paints the path it took, and where it runs off the
//! mesh turns it into a falling drop. Here a drop sits the same way in a
//! triangle of the skin sheet.
//!
//! A drop on a wall stays put until it holds enough liquid for its weight to
//! beat the surface tension pinning its edge, then slides, leaving a film that
//! thins it until it stops again (Furmidge 1962). So blood here beads at a
//! wound until the bead is big enough to run, a running drop gives blood to
//! its trail, and drops that meet merge, so later blood refills the beads
//! earlier blood left and carries them farther.

use super::grid::SpatialGrid;
use super::*;

/// Drops smaller than this, in pixels, stay pinned where they are. Drops
/// are drawn far bigger than real ones, so this stands for the few
/// hundredths of a millilitre a real drop needs before it slides.
const PINNED_RADIUS: f64 = 2.2;
/// A drop this big, in pixels, runs at full speed.
const RUNNING_RADIUS: f64 = 4.0;
/// How fast, in pixels a second, a full drop runs down the skin: 11 cm/s at
/// the 321 px a metre the body is drawn at, and a small one runs at 3 cm/s.
/// A film of blood (1062 kg/m^3, about 4.75 mPa s) a few tenths of a
/// millimetre thick runs down a wall at rho g h^2 / 3 mu, 3 to 18 cm/s
/// (Nusselt film flow; blood's properties as in Sellier et al. 2010).
const RUN_SPEED: f64 = 36.0;
/// The biggest drop the skin holds, in pixels.
const MAX_DROP_RADIUS: f64 = 5.2;
/// Blood a running drop leaves in its trail: its squared radius shrinks by
/// this much for each pixel it runs.
const TRAIL_FILM: f64 = 0.05;
/// How far, in pixels, a drop runs before its trail gets another stretch.
const TRAIL_STEP: f64 = 3.0;
/// How far below the skin's edge, in point spacings, a running drop finds
/// more skin to run onto, as across the narrow mouth of a cut.
const BRIDGE_REACH: f64 = 0.75;
/// How far below a wound, in point spacings, its blood finds skin to well
/// onto, as the lower lip of an open cut.
const WELL_REACH: f64 = 1.5;
/// Drops closer than this share of their radii together merge.
const MERGE_SHARE: f64 = 0.6;
/// How long, in seconds, a drop stays on the skin before it has dried into
/// its trail.
const SKIN_BLOOD_LIFE: f64 = 9.0;
/// The most a drop drifts sideways for each pixel it runs down.
const MAX_DRIFT: f64 = 0.08;
/// How much of a fresh wound's leak may spray before it no longer wells onto
/// the skin but flies off it (see `update_wounds`).
pub(super) const WELLING_SPRAY: f64 = 0.15;

/// A place on the skin: a skin triangle and weights on its corners, so the
/// place moves as the skin does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkinSpot {
    pub triangle: usize,
    pub weights: [f64; 3],
}

/// A stretch of the trail blood leaves running down the skin.
#[derive(Clone, Copy, Debug)]
pub struct BloodTrail {
    pub from: SkinSpot,
    pub to: SkinSpot,
    /// Width in pixels.
    pub width: f64,
    /// How long the stretch was when it was laid, in pixels; one pulled far
    /// longer was torn across by the skin opening under it.
    pub length: f64,
    /// Seconds since it was laid: blood darkens as it dries.
    pub age: f64,
}

impl World {
    pub fn blood_trails(&self) -> &[BloodTrail] {
        &self.blood_trails
    }

    /// Where `spot` is now, unless the skin there is torn away.
    pub fn skin_spot_position(&self, spot: SkinSpot) -> Option<Vec2> {
        let triangle = self.triangles.get(spot.triangle)?;
        (triangle.layer == TissueLayer::Skin && self.triangle_alive(triangle))
            .then(|| self.spot_position(spot))
    }

    /// Where the ends of `trail` are now, unless the skin under either is
    /// torn away or the skin opened under the stretch, pulling it far longer
    /// than it was laid.
    pub fn blood_trail_ends(&self, trail: &BloodTrail) -> Option<(Vec2, Vec2)> {
        let from = self.skin_spot_position(trail.from)?;
        let to = self.skin_spot_position(trail.to)?;
        (distance(from, to) <= trail.length * 2.5 + 2.0).then_some((from, to))
    }

    /// Blood from a fresh cut, up to what fresh injuries may let out this
    /// step, welling onto the skin.
    pub(super) fn well_fresh_blood(
        &mut self,
        center: Vec2,
        direction: Vec2,
        count: i32,
        speed: f64,
        radius: f64,
        intensity: f64,
    ) {
        let count = count.min(self.materials.max_fresh_blood_per_step - self.fresh_blood_used);
        if count <= 0 {
            return;
        }
        self.fresh_blood_used += count;
        let welled = self.well_blood(center, direction, count, speed, radius, intensity);
        self.stats.fresh_blood_welled += welled;
    }

    /// Blood at `center` that wells out rather than spurting: onto the skin
    /// at the wound, or the lip below it, where it beads and runs down. Under
    /// unbroken skin it bruises, and where no skin is near it falls as drops
    /// (see `release_blood`). Returns how many drops welled onto the skin.
    pub(super) fn well_blood(
        &mut self,
        center: Vec2,
        direction: Vec2,
        count: i32,
        speed: f64,
        radius: f64,
        intensity: f64,
    ) -> i32 {
        if self.materials.max_fluid_particles == 0 || count <= 0 {
            return 0;
        }
        if self.sealed_under_skin(center) {
            self.bleed_under_skin(center, count, intensity);
            return 0;
        }
        let mut grid = std::mem::take(&mut self.grid);
        self.build_skin_grid(&mut grid);
        let spacing = self.materials.point_spacing;
        let radius = radius.clamp(1.35, 4.8);
        let intensity = intensity.clamp(0.35, 1.35);
        let mut welled = 0;
        for _ in 0..count {
            // Blood comes out along the wound, not from one point of it.
            let jitter = radius * (self.next_fluid_random() - 0.5) * 2.4;
            let source = Vec2 {
                x: center.x + jitter,
                y: center.y,
            };
            let steps = 6;
            let spot = (0..=steps).find_map(|step| {
                let below = Vec2 {
                    x: source.x,
                    y: source.y + spacing * WELL_REACH * f64::from(step) / f64::from(steps),
                };
                self.skin_spot_at(&grid, below, None)
            });
            let Some(spot) = spot else {
                continue;
            };
            let position = self.spot_position(spot);
            let drop = FluidParticle {
                position,
                previous: position,
                radius: radius * (0.72 + self.next_fluid_random() * 0.58),
                life: SKIN_BLOOD_LIFE,
                max_life: SKIN_BLOOD_LIFE,
                intensity,
                on_skin: Some(spot),
                trail_from: Some(spot),
                drift: (self.next_fluid_random() - 0.5) * 2.0 * MAX_DRIFT,
                ..FluidParticle::default()
            };
            self.add_fluid(drop);
            self.stats.skin_blood_drops += 1;
            welled += 1;
        }
        self.grid = grid;
        if welled < count {
            self.release_blood(center, direction, count - welled, speed, radius, intensity);
        }
        welled
    }

    /// Runs the blood on the skin down it (see the module notes), after the
    /// solver has moved the skin for this step.
    pub(super) fn run_blood_on_skin(&mut self, dt: f64) {
        for trail in &mut self.blood_trails {
            trail.age += dt;
        }
        if !self.fluids.iter().any(|fluid| fluid.on_skin.is_some()) {
            return;
        }
        let mut grid = std::mem::take(&mut self.grid);
        self.build_skin_grid(&mut grid);
        let bridge = self.materials.point_spacing * BRIDGE_REACH;
        for index in 0..self.fluids.len() {
            let drop = self.fluids[index];
            let Some(spot) = drop.on_skin else {
                continue;
            };
            let alive = self.triangle_alive(&self.triangles[spot.triangle]);
            if drop.life <= 0.0 {
                // It has dried where it stopped, the last of its trail.
                if alive {
                    self.lay_trail(BloodTrail {
                        from: spot,
                        to: spot,
                        width: drop.radius,
                        length: 0.0,
                        age: 0.0,
                    });
                }
                self.fluids[index].on_skin = None;
                self.fluids[index].trail_from = None;
                continue;
            }
            if !alive {
                // The skin under it tore away.
                let here = self.spot_position(spot);
                let velocity = self.spot_velocity(spot);
                self.drop_off_skin(index, here, velocity);
                continue;
            }
            let here = self.spot_position(spot);
            let mut spot = spot;
            let mut ran = 0.0;
            if drop.radius >= PINNED_RADIUS {
                let pace = ((drop.radius - PINNED_RADIUS) / (RUNNING_RADIUS - PINNED_RADIUS))
                    .clamp(0.3, 1.0);
                let run = RUN_SPEED * pace * dt;
                let target = Vec2 {
                    x: here.x + drop.drift * run,
                    y: here.y + run,
                };
                let next = self
                    .skin_spot_at(&grid, target, Some(spot.triangle))
                    .or_else(|| {
                        (1..=3).find_map(|step| {
                            let below = Vec2 {
                                x: target.x,
                                y: target.y + bridge * f64::from(step) / 3.0,
                            };
                            self.skin_spot_at(&grid, below, None)
                        })
                    });
                let Some(next) = next else {
                    // It ran off the skin's edge and drips.
                    let velocity = add(
                        self.spot_velocity(spot),
                        Vec2 {
                            x: drop.drift * run,
                            y: run,
                        },
                    );
                    self.drop_off_skin(index, target, velocity);
                    self.stats.blood_drips += 1;
                    continue;
                };
                spot = next;
                ran = run;
            }
            let now = self.spot_position(spot);
            let mut trail = None;
            if let Some(from) = drop.trail_from {
                if self.triangle_alive(&self.triangles[from.triangle]) {
                    let length = distance(self.spot_position(from), now);
                    if length >= TRAIL_STEP {
                        trail = Some(BloodTrail {
                            from,
                            to: spot,
                            width: drop.radius * 0.6,
                            length,
                            age: 0.0,
                        });
                    }
                } else {
                    trail = Some(BloodTrail {
                        from: spot,
                        to: spot,
                        width: 0.0,
                        length: 0.0,
                        age: 0.0,
                    });
                }
            }
            let fluid = &mut self.fluids[index];
            fluid.previous = fluid.position;
            fluid.position = now;
            fluid.on_skin = Some(spot);
            if ran > 0.0 {
                fluid.radius = (fluid.radius * fluid.radius - TRAIL_FILM * ran)
                    .max(0.0)
                    .sqrt();
            }
            if let Some(trail) = trail {
                fluid.trail_from = Some(spot);
                if trail.width > 0.0 {
                    self.stats.blood_trail_length += trail.length;
                    self.lay_trail(trail);
                }
            }
        }
        self.grid = grid;
        self.merge_skin_drops();
    }

    /// Merges drops on the skin that touch: the lower takes in the other, as
    /// blood running into a bead joins it there.
    fn merge_skin_drops(&mut self) {
        let reach = MAX_DROP_RADIUS * 2.0 * MERGE_SHARE;
        let mut grid = std::mem::take(&mut self.grid);
        grid.build_points(
            reach,
            self.fluids
                .iter()
                .enumerate()
                .filter(|(_, fluid)| fluid.life > 0.0 && fluid.on_skin.is_some())
                .map(|(index, fluid)| (index, fluid.position)),
        );
        let mut around = Vec::new();
        for i in 0..self.fluids.len() {
            let drop = self.fluids[i];
            if drop.life <= 0.0 || drop.on_skin.is_none() {
                continue;
            }
            grid.query(
                segment_aabb(drop.position, drop.position, reach),
                &mut around,
            );
            for &j in &around {
                if j <= i {
                    continue;
                }
                let (a, b) = (self.fluids[i], self.fluids[j]);
                if b.life <= 0.0 || b.on_skin.is_none() {
                    continue;
                }
                if distance(a.position, b.position) > (a.radius + b.radius) * MERGE_SHARE {
                    continue;
                }
                let (keep, take) = if b.position.y > a.position.y {
                    (j, i)
                } else {
                    (i, j)
                };
                self.merge_drop(keep, take);
                if take == i {
                    break;
                }
            }
        }
        self.grid = grid;
    }

    /// Drop `take` joins drop `keep`; a drop that ran down into the other
    /// carries its trail on to it.
    fn merge_drop(&mut self, keep: usize, take: usize) {
        let (kept, taken) = (self.fluids[keep], self.fluids[take]);
        let (kept_area, taken_area) = (kept.radius * kept.radius, taken.radius * taken.radius);
        let area = (kept_area + taken_area).max(EPSILON);
        if let (Some(from), Some(to)) = (taken.trail_from, kept.on_skin) {
            if self.triangle_alive(&self.triangles[from.triangle]) {
                let start = self.spot_position(from);
                let length = distance(start, kept.position);
                let down = kept.position.y - start.y;
                if length >= 1.0 && down >= (kept.position.x - start.x).abs() {
                    self.stats.blood_trail_length += length;
                    self.lay_trail(BloodTrail {
                        from,
                        to,
                        width: taken.radius * 0.6,
                        length,
                        age: 0.0,
                    });
                }
            }
        }
        let fluid = &mut self.fluids[keep];
        fluid.radius = area.sqrt().min(MAX_DROP_RADIUS);
        fluid.intensity = (kept.intensity * kept_area + taken.intensity * taken_area) / area;
        fluid.life = kept.life.max(taken.life);
        fluid.max_life = kept.max_life.max(taken.max_life);
        let gone = &mut self.fluids[take];
        gone.life = 0.0;
        gone.on_skin = None;
        gone.trail_from = None;
    }

    /// Drop `index` leaves the skin at `position` with `velocity`, in pixels
    /// a step, and falls.
    fn drop_off_skin(&mut self, index: usize, position: Vec2, velocity: Vec2) {
        let lifetime = self.materials.fluid_lifetime;
        let fluid = &mut self.fluids[index];
        fluid.on_skin = None;
        fluid.trail_from = None;
        fluid.position = position;
        fluid.previous = subtract(position, velocity);
        fluid.life = fluid.life.min(lifetime);
        fluid.max_life = fluid.max_life.min(lifetime).max(fluid.life);
    }

    fn lay_trail(&mut self, trail: BloodTrail) {
        let budget = self.materials.max_blood_trails;
        if budget == 0 {
            return;
        }
        if self.blood_trails.len() < budget {
            self.blood_trails.push(trail);
        } else {
            let index = self.blood_trail_write_cursor % self.blood_trails.len();
            self.blood_trails[index] = trail;
            self.blood_trail_write_cursor = (index + 1) % self.blood_trails.len();
        }
    }

    /// Sorts the skin triangles still in one piece into `grid` by the boxes
    /// they cover now.
    fn build_skin_grid(&self, grid: &mut SpatialGrid) {
        let cell = self.materials.point_spacing.max(1.0) * 2.0;
        grid.build_boxes(
            cell,
            self.triangles
                .iter()
                .enumerate()
                .filter(|(_, triangle)| {
                    triangle.layer == TissueLayer::Skin && self.triangle_alive(triangle)
                })
                .map(|(index, triangle)| {
                    let [a, b, c] = [triangle.a, triangle.b, triangle.c]
                        .map(|point| self.points[point].position);
                    let aabb = Aabb {
                        min: Vec2 {
                            x: a.x.min(b.x).min(c.x),
                            y: a.y.min(b.y).min(c.y),
                        },
                        max: Vec2 {
                            x: a.x.max(b.x).max(c.x),
                            y: a.y.max(b.y).max(c.y),
                        },
                    };
                    (index, aabb)
                }),
        );
    }

    /// The skin at `point`, trying triangle `hint` first, from a grid of the
    /// skin (see `build_skin_grid`).
    fn skin_spot_at(
        &self,
        grid: &SpatialGrid,
        point: Vec2,
        hint: Option<usize>,
    ) -> Option<SkinSpot> {
        if let Some(triangle) = hint {
            if let Some(weights) = self.weights_in(triangle, point) {
                return Some(SkinSpot { triangle, weights });
            }
        }
        grid.at(point).iter().find_map(|&triangle| {
            let triangle = triangle as usize;
            self.weights_in(triangle, point)
                .map(|weights| SkinSpot { triangle, weights })
        })
    }

    /// The weights of `point` on the corners of triangle `triangle` as it is
    /// now, if the point lies in it.
    fn weights_in(&self, triangle: usize, point: Vec2) -> Option<[f64; 3]> {
        let triangle = &self.triangles[triangle];
        let a = self.points[triangle.a].position;
        let ab = subtract(self.points[triangle.b].position, a);
        let ac = subtract(self.points[triangle.c].position, a);
        let ap = subtract(point, a);
        let doubled_area = ab.x * ac.y - ab.y * ac.x;
        if doubled_area.abs() <= EPSILON {
            return None;
        }
        let wb = (ap.x * ac.y - ap.y * ac.x) / doubled_area;
        let wc = (ab.x * ap.y - ab.y * ap.x) / doubled_area;
        let wa = 1.0 - wb - wc;
        let slack = -1.0e-9;
        (wa >= slack && wb >= slack && wc >= slack).then_some([wa, wb, wc])
    }

    fn spot_position(&self, spot: SkinSpot) -> Vec2 {
        let triangle = &self.triangles[spot.triangle];
        let [wa, wb, wc] = spot.weights;
        let (a, b, c) = (
            self.points[triangle.a].position,
            self.points[triangle.b].position,
            self.points[triangle.c].position,
        );
        Vec2 {
            x: a.x * wa + b.x * wb + c.x * wc,
            y: a.y * wa + b.y * wb + c.y * wc,
        }
    }

    /// How far the skin at `spot` moved this step.
    fn spot_velocity(&self, spot: SkinSpot) -> Vec2 {
        let triangle = &self.triangles[spot.triangle];
        let [wa, wb, wc] = spot.weights;
        let moved = |point: usize| {
            let point = &self.points[point];
            subtract(point.position, point.previous)
        };
        let (a, b, c) = (moved(triangle.a), moved(triangle.b), moved(triangle.c));
        Vec2 {
            x: a.x * wa + b.x * wb + c.x * wc,
            y: a.y * wa + b.y * wb + c.y * wc,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STRIP_SPACING: f64 = 10.0;

    /// A strip of skin, held still, with nothing else in the world.
    fn skin_strip(columns: usize, rows: usize) -> World {
        let materials = Materials {
            gravity: 0.0,
            ..Materials::default()
        };
        let mut world = World::new(materials);
        let m = world.materials;
        for row in 0..rows {
            for column in 0..columns {
                let position = Vec2 {
                    x: 200.0 + column as f64 * STRIP_SPACING,
                    y: 80.0 + row as f64 * STRIP_SPACING,
                };
                world.add_point(position, TissueLayer::Skin, true);
            }
        }
        let at = |column: usize, row: usize| row * columns + column;
        for row in 0..rows - 1 {
            for column in 0..columns - 1 {
                let (here, right, below, across) = (
                    at(column, row),
                    at(column + 1, row),
                    at(column, row + 1),
                    at(column + 1, row + 1),
                );
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
                world.add_triangle(here, right, below, TissueLayer::Skin);
                world.add_triangle(right, across, below, TissueLayer::Skin);
            }
        }
        // A slit near the top, where blood gets out.
        let slit = world.find_spring_index(at(1, 1), at(2, 1), TissueLayer::Skin);
        world.break_spring(slit);
        world
    }

    fn run(world: &mut World, steps: usize) {
        let dt = world.materials.fixed_dt;
        for _ in 0..steps {
            world.step(dt, &InputState::default(), 640.0, 480.0);
        }
    }

    fn drops_on_skin(world: &World) -> Vec<FluidParticle> {
        world
            .fluids
            .iter()
            .filter(|fluid| fluid.life > 0.0 && fluid.on_skin.is_some())
            .copied()
            .collect()
    }

    #[test]
    fn welled_blood_runs_down_the_skin_and_leaves_a_trail() {
        let mut world = skin_strip(4, 12);
        let start = Vec2 { x: 215.0, y: 95.0 };
        world.well_blood(start, Vec2 { x: 0.0, y: 1.0 }, 1, 60.0, 4.6, 1.0);
        let drops = drops_on_skin(&world);
        assert_eq!(drops.len(), 1, "the blood should well onto the skin");
        let radius = drops[0].radius;
        assert!(radius >= PINNED_RADIUS, "a full drop runs: {radius:.2}");
        run(&mut world, 30);
        let drops = drops_on_skin(&world);
        assert_eq!(drops.len(), 1);
        let ran = drops[0].position.y - start.y;
        assert!(ran > 8.0, "the drop should run down the skin: {ran:.1} px");
        assert!(drops[0].radius < radius, "it gives blood to its trail");
        assert!(!world.blood_trails.is_empty(), "it should leave a trail");
        assert!(world.stats.blood_trail_length > 8.0);
    }

    #[test]
    fn a_small_drop_stays_pinned_until_more_blood_joins_it() {
        let mut world = skin_strip(4, 12);
        let start = Vec2 { x: 215.0, y: 95.0 };
        world.well_blood(start, Vec2 { x: 0.0, y: 1.0 }, 1, 60.0, 1.35, 1.0);
        let small = drops_on_skin(&world)[0];
        assert!(small.radius < PINNED_RADIUS);
        run(&mut world, 30);
        let still = drops_on_skin(&world)[0];
        assert!(
            distance(still.position, small.position) < 1.0e-9,
            "a small drop should stay pinned"
        );
        for _ in 0..4 {
            world.well_blood(start, Vec2 { x: 0.0, y: 1.0 }, 1, 60.0, 1.35, 1.0);
            run(&mut world, 1);
        }
        run(&mut world, 30);
        let drops = drops_on_skin(&world);
        assert!(
            drops.iter().any(|drop| drop.position.y > start.y + 4.0),
            "merged drops should grow big enough to run: {drops:?}"
        );
    }

    #[test]
    fn blood_running_off_the_skin_drips() {
        let mut world = skin_strip(4, 4);
        world.well_blood(
            Vec2 { x: 215.0, y: 100.0 },
            Vec2 { x: 0.0, y: 1.0 },
            1,
            60.0,
            4.6,
            1.0,
        );
        assert_eq!(drops_on_skin(&world).len(), 1);
        run(&mut world, 120);
        assert!(
            drops_on_skin(&world).is_empty(),
            "the drop should have run off"
        );
        assert_eq!(world.stats.blood_drips, 1);
        let fallen = world
            .fluids
            .iter()
            .find(|fluid| fluid.life > 0.0)
            .expect("the drop falls on");
        assert!(fallen.position.y > 80.0 + 3.0 * STRIP_SPACING);
    }

    #[test]
    fn blood_falls_where_no_skin_is_near() {
        let mut world = skin_strip(4, 4);
        world.well_blood(
            Vec2 { x: 400.0, y: 100.0 },
            Vec2 { x: 0.0, y: 1.0 },
            3,
            60.0,
            2.0,
            1.0,
        );
        assert!(drops_on_skin(&world).is_empty());
        assert_eq!(world.stats.emitted_fluid_particles, 3);
    }

    #[test]
    fn blood_on_skin_follows_the_skin() {
        let mut world = skin_strip(4, 12);
        world.well_blood(
            Vec2 { x: 215.0, y: 95.0 },
            Vec2 { x: 0.0, y: 1.0 },
            1,
            60.0,
            1.35,
            1.0,
        );
        let before = drops_on_skin(&world)[0].position;
        for point in &mut world.points {
            point.position.x += 25.0;
            point.previous.x += 25.0;
            point.home.x += 25.0;
        }
        run(&mut world, 1);
        let after = drops_on_skin(&world)[0].position;
        assert!(
            (after.x - before.x - 25.0).abs() < 1.0e-6,
            "{before:?} -> {after:?}"
        );
    }
}
