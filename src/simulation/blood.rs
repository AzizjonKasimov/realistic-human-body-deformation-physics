//! Blood out of the body: on the skin, in the air, and on the floor. Blood
//! that wells out of a wound clings to the skin and runs down it, leaving a
//! trail, until it runs off the body's edge and drips to the floor, where it
//! spreads into spots that run together into a pool.
//!
//! Every drop holds a volume of blood, and is drawn as large as that blood
//! would be on a person the body's size: a falling drop as a sphere, a bead
//! on the skin by where it wets the skin, a trail by the film it leaves, and a
//! pool by how far that much blood spreads on a floor. So no more blood shows
//! than the wounds let out (`Stats::blood_shed_ml`), and none of it larger.
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

/// How tall the person the body stands for is, in millimetres.
const BODY_HEIGHT_MM: f64 = 1750.0;
/// How tall the body is drawn in the scenarios' 1280x720 window, in pixels;
/// a world built without a body draws its blood to this scale.
pub(super) const STANDARD_BODY_HEIGHT: f64 = 561.6;
/// The shape of a bead of blood on the skin, a spherical cap meeting the skin
/// at 90 degrees (blood meets plastics and card at about 87 to 120 degrees,
/// Attinger 2016; skin has not been measured): its volume is 2 pi a^3 / 3
/// for a wetted radius a, so a^3 = volume times this.
const BEAD_SHAPE: f64 = 3.0 / (2.0 * PI);
/// Beads holding less than this, in millilitres, stay pinned where they are:
/// a drop's weight must beat the surface tension holding its edge before it
/// slides (Furmidge 1962), so one drop of blood beads at a wound and the
/// next that joins it starts it running. No one has measured it for blood on
/// skin; this is set so that a drop beads and two run.
const PIN_ML: f64 = 0.08;
/// A bead holding this much, in millilitres, runs at full speed.
const RUN_ML: f64 = 0.3;
/// How fast, in millimetres a second, a full bead runs down the skin, and a
/// small one runs at 3 cm/s. A film of blood (1062 kg/m^3, about 4.75
/// mPa s) a few tenths of a millimetre thick runs down a wall at
/// rho g h^2 / 3 mu, 3 to 18 cm/s (Nusselt film flow; blood's properties as
/// in Sellier et al. 2010).
const RUN_SPEED_MM: f64 = 112.0;
/// The slowest share of `RUN_SPEED_MM` a running bead goes.
const SLOWEST_RUN: f64 = 0.3;
/// How thick a film, in millimetres, a running bead leaves on the skin
/// behind it: a slow drop's trail is about a tenth of a millimetre (the
/// Landau-Levich film, about 1.34 times the capillary length times the
/// capillary number to the two-thirds, for blood running at a few cm/s).
const TRAIL_FILM_MM: f64 = 0.1;
/// How wide a trail is against the bead that laid it.
const TRAIL_WIDTH_SHARE: f64 = 0.6;
/// How far, in pixels, a drop runs before its trail gets another stretch.
const TRAIL_STEP: f64 = 3.0;
/// How far below the skin's edge, in point spacings, a running drop finds
/// more skin to run onto, as across the narrow mouth of a cut.
const BRIDGE_REACH: f64 = 0.75;
/// How far below a wound, in point spacings, its blood finds skin to well
/// onto, as the lower lip of an open cut.
const WELL_REACH: f64 = 1.5;
/// How long, in seconds, a drop stays on the skin before it has dried into
/// its trail.
const SKIN_BLOOD_LIFE: f64 = 9.0;
/// The most a drop drifts sideways for each pixel it runs down.
const MAX_DRIFT: f64 = 0.08;
/// How much of a fresh wound's leak may spray before it no longer wells onto
/// the skin but flies off it (see `update_wounds`).
pub(super) const WELLING_SPRAY: f64 = 0.15;
/// How deep blood lies in a pool on a smooth floor, in millimetres: pools of
/// up to 31 g poured on floor tile averaged 1.44 mm (Smith, Nicloux, and Brutin
/// 2020), so a pool covers about 7 cm^2 for each millilitre.
const POOL_DEPTH_MM: f64 = 1.44;
/// The smallest cell, in pixels, of the grid that finds beads touching.
const MERGE_CELL: f64 = 4.0;

/// The radius, in millimetres, of `ml` of blood falling free: a sphere.
pub fn falling_drop_radius_mm(ml: f64) -> f64 {
    libm::cbrt(ml.max(0.0) * 1000.0 * 3.0 / (4.0 * PI))
}

/// The radius, in millimetres, of the skin a bead of `ml` of blood wets.
pub fn bead_radius_mm(ml: f64) -> f64 {
    libm::cbrt(ml.max(0.0) * 1000.0 * BEAD_SHAPE)
}

/// The width, in millimetres, of the spot a drop of `ml` of blood leaves
/// where it lands on a smooth floor at `speed_mm_s`.
pub fn landing_spot_mm(ml: f64, speed_mm_s: f64) -> f64 {
    2.0 * falling_drop_radius_mm(ml) * landing_spread(speed_mm_s / 1000.0)
}

/// How many times its own width a drop of blood spreads where it lands on a
/// smooth, hard floor at `speed` metres a second. MacDonell (1971, Fig. 3)
/// measured the spots a 0.05 ml drop, 4.6 mm across, leaves: about 15 mm wide
/// after falling 0.5 m (3.1 m/s), 17 mm after 1 m (4.4 m/s), 17.5 mm after
/// 1.5 m (5.4 m/s), and 18-19 mm from about 2 m (6.3 m/s) on. A drop landing
/// slower, from under half a metre, is given the half-metre spot.
fn landing_spread(speed: f64) -> f64 {
    const MEASURED: [(f64, f64); 4] = [(3.13, 3.26), (4.43, 3.70), (5.42, 3.80), (6.26, 4.02)];
    let (first, last) = (MEASURED[0], MEASURED[MEASURED.len() - 1]);
    if speed <= first.0 {
        return first.1;
    }
    if speed >= last.0 {
        return last.1;
    }
    MEASURED
        .windows(2)
        .find(|pair| speed <= pair[1].0)
        .map_or(last.1, |pair| {
            let share = (speed - pair[0].0) / (pair[1].0 - pair[0].0);
            pair[0].1 + (pair[1].1 - pair[0].1) * share
        })
}

/// The width, in millimetres, of a pool of `ml` of blood on a smooth floor.
pub fn pool_width_mm(ml: f64) -> f64 {
    2.0 * (ml.max(0.0) * 1000.0 / (PI * POOL_DEPTH_MM)).sqrt()
}

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
    /// The blood it holds, in millilitres.
    pub volume: f64,
    /// Seconds since it was laid: blood darkens as it dries.
    pub age: f64,
}

impl World {
    pub fn blood_trails(&self) -> &[BloodTrail] {
        &self.blood_trails
    }

    /// How many millimetres of the person the body stands for one pixel
    /// spans, as the body is drawn.
    pub fn mm_per_px(&self) -> f64 {
        BODY_HEIGHT_MM / self.body_height.max(1.0)
    }

    /// How large `drop` is, as a radius in pixels: a falling drop is the
    /// sphere its blood makes, and a drop on the skin is as wide as the skin
    /// its bead wets.
    pub fn drop_radius(&self, drop: &FluidParticle) -> f64 {
        let mm = if drop.on_skin.is_some() {
            bead_radius_mm(drop.volume)
        } else {
            falling_drop_radius_mm(drop.volume)
        };
        mm / self.mm_per_px()
    }

    /// The blood out of the body still shown, in millilitres: drops in the
    /// air and on the skin, trails, and pools.
    pub fn blood_shown_ml(&self) -> f64 {
        let drops: f64 = self
            .fluids
            .iter()
            .filter(|fluid| fluid.life > 0.0)
            .map(|fluid| fluid.volume)
            .sum();
        let trails: f64 = self.blood_trails.iter().map(|trail| trail.volume).sum();
        let pools: f64 = self.blood_stains.iter().map(|stain| stain.volume).sum();
        drops + trails + pools
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

    /// Blood at `center` that wells out rather than spurting: onto the skin
    /// at the wound, or the lip below it, where it beads and runs down, a
    /// drop (`BLOOD_DROP_ML`) at a time. It comes out along the wound, up to
    /// about `spread` pixels either way. Under unbroken skin it bruises, and
    /// where no skin is near it falls as drops (see `release_blood`). Returns
    /// how many drops welled onto the skin.
    pub(super) fn well_blood(
        &mut self,
        center: Vec2,
        direction: Vec2,
        count: i32,
        speed: f64,
        spread: f64,
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
        let spread = spread.clamp(1.35, 4.8);
        let intensity = intensity.clamp(0.35, 1.35);
        let mut welled = 0;
        for _ in 0..count {
            let jitter = spread * (self.next_fluid_random() - 0.5) * 2.4;
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
                volume: BLOOD_DROP_ML,
                life: SKIN_BLOOD_LIFE,
                max_life: SKIN_BLOOD_LIFE,
                intensity,
                on_skin: Some(spot),
                trail_from: Some(spot),
                drift: (self.next_fluid_random() - 0.5) * 2.0 * MAX_DRIFT,
            };
            self.add_fluid(drop);
            self.stats.skin_blood_drops += 1;
            welled += 1;
        }
        self.grid = grid;
        if welled < count {
            self.release_blood(center, direction, count - welled, speed, spread, intensity);
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
        let mm = self.mm_per_px();
        for index in 0..self.fluids.len() {
            let drop = self.fluids[index];
            let Some(spot) = drop.on_skin else {
                continue;
            };
            let alive = self.triangle_alive(&self.triangles[spot.triangle]);
            if drop.life <= 0.0 || drop.volume <= EPSILON {
                // It has dried where it stopped, the last of its trail.
                if alive && drop.volume > EPSILON {
                    self.lay_trail(BloodTrail {
                        from: spot,
                        to: spot,
                        width: 2.0 * self.drop_radius(&drop),
                        length: 0.0,
                        volume: drop.volume,
                        age: 0.0,
                    });
                } else {
                    self.stats.blood_unshown_ml += drop.volume;
                }
                let fluid = &mut self.fluids[index];
                fluid.life = 0.0;
                fluid.volume = 0.0;
                fluid.on_skin = None;
                fluid.trail_from = None;
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
            if drop.volume >= PIN_ML {
                let pace = ((drop.volume - PIN_ML) / (RUN_ML - PIN_ML)).clamp(SLOWEST_RUN, 1.0);
                let run = RUN_SPEED_MM * pace * dt / mm;
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
            }
            let now = self.spot_position(spot);
            let mut trail = None;
            if let Some(from) = drop.trail_from {
                if self.triangle_alive(&self.triangles[from.triangle]) {
                    let length = distance(self.spot_position(from), now);
                    if length >= TRAIL_STEP {
                        trail = Some(self.trail_behind(&drop, from, spot, length));
                    }
                } else {
                    trail = Some(BloodTrail {
                        from: spot,
                        to: spot,
                        width: 0.0,
                        length: 0.0,
                        volume: 0.0,
                        age: 0.0,
                    });
                }
            }
            let fluid = &mut self.fluids[index];
            fluid.previous = fluid.position;
            fluid.position = now;
            fluid.on_skin = Some(spot);
            if let Some(trail) = trail {
                fluid.trail_from = Some(spot);
                if trail.width > 0.0 {
                    fluid.volume -= trail.volume;
                    self.stats.blood_trail_length += trail.length;
                    self.lay_trail(trail);
                }
            }
        }
        self.grid = grid;
        self.merge_skin_drops();
    }

    /// The stretch of trail `drop` leaves from `from` to `to`, `length` pixels
    /// long: a film of its blood about as wide as its bead, taken from it.
    fn trail_behind(
        &self,
        drop: &FluidParticle,
        from: SkinSpot,
        to: SkinSpot,
        length: f64,
    ) -> BloodTrail {
        let mm = self.mm_per_px();
        let width = 2.0 * self.drop_radius(drop) * TRAIL_WIDTH_SHARE;
        let film = width * mm * length * mm * TRAIL_FILM_MM / 1000.0;
        BloodTrail {
            from,
            to,
            width,
            length,
            volume: film.min(drop.volume),
            age: 0.0,
        }
    }

    /// Merges drops on the skin whose beads touch: the lower takes in the
    /// other, as blood running into a bead joins it there.
    fn merge_skin_drops(&mut self) {
        let widest = self
            .fluids
            .iter()
            .filter(|fluid| fluid.life > 0.0 && fluid.on_skin.is_some())
            .map(|fluid| self.drop_radius(fluid))
            .fold(0.0, f64::max);
        let reach = widest * 2.0;
        let mut grid = std::mem::take(&mut self.grid);
        grid.build_points(
            reach.max(MERGE_CELL),
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
                if distance(a.position, b.position) > self.drop_radius(&a) + self.drop_radius(&b) {
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
        let (kept, mut taken) = (self.fluids[keep], self.fluids[take]);
        if let (Some(from), Some(to)) = (taken.trail_from, kept.on_skin) {
            if self.triangle_alive(&self.triangles[from.triangle]) {
                let start = self.spot_position(from);
                let length = distance(start, kept.position);
                let down = kept.position.y - start.y;
                if length >= 1.0 && down >= (kept.position.x - start.x).abs() {
                    let trail = self.trail_behind(&taken, from, to, length);
                    taken.volume -= trail.volume;
                    self.stats.blood_trail_length += length;
                    self.lay_trail(trail);
                }
            }
        }
        let volume = kept.volume + taken.volume;
        let fluid = &mut self.fluids[keep];
        fluid.volume = volume;
        fluid.intensity =
            (kept.intensity * kept.volume + taken.intensity * taken.volume) / volume.max(EPSILON);
        fluid.life = kept.life.max(taken.life);
        fluid.max_life = kept.max_life.max(taken.max_life);
        let gone = &mut self.fluids[take];
        gone.life = 0.0;
        gone.volume = 0.0;
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
            self.stats.blood_unshown_ml += trail.volume;
            return;
        }
        if self.blood_trails.len() < budget {
            self.blood_trails.push(trail);
        } else {
            let index = self.blood_trail_write_cursor % self.blood_trails.len();
            self.stats.blood_unshown_ml += self.blood_trails[index].volume;
            self.blood_trails[index] = trail;
            self.blood_trail_write_cursor = (index + 1) % self.blood_trails.len();
        }
    }

    /// A drop of `volume` millilitres lands on the floor at `x`, falling at
    /// `speed` pixels a second. It spreads into a spot, which runs together
    /// with the spots and pools it touches, and a pool is as wide as its blood
    /// spreads on a floor.
    pub(super) fn land_blood(&mut self, x: f64, floor_y: f64, volume: f64, speed: f64) {
        if volume <= 0.0 {
            return;
        }
        if self.materials.max_blood_stains == 0 {
            self.stats.blood_unshown_ml += volume;
            return;
        }
        let mm = self.mm_per_px();
        let spot = landing_spot_mm(volume, speed * mm) / mm;
        let (mut left, mut right) = (x - spot * 0.5, x + spot * 0.5);
        let mut volume = volume;
        let mut slot = None;
        // A pool that grows can reach spots beside it, so look again until it
        // reaches no more.
        loop {
            let mut grew = false;
            for index in 0..self.blood_stains.len() {
                let stain = self.blood_stains[index];
                if stain.volume <= 0.0 || slot == Some(index) {
                    continue;
                }
                let half = stain.width * 0.5;
                if stain.position.x - half > right || stain.position.x + half < left {
                    continue;
                }
                left = left.min(stain.position.x - half);
                right = right.max(stain.position.x + half);
                volume += stain.volume;
                if slot.is_none() {
                    slot = Some(index);
                } else {
                    self.blood_stains[index] = BloodStain::default();
                }
                grew = true;
            }
            let width = (right - left).max(pool_width_mm(volume) / mm);
            let middle = (left + right) * 0.5;
            left = middle - width * 0.5;
            right = middle + width * 0.5;
            if !grew {
                break;
            }
        }
        let stain = BloodStain {
            position: Vec2 {
                x: (left + right) * 0.5,
                y: floor_y,
            },
            width: right - left,
            volume,
            age: 0.0,
        };
        let free = slot.or_else(|| {
            self.blood_stains
                .iter()
                .position(|stain| stain.volume <= 0.0)
        });
        if let Some(index) = free {
            self.blood_stains[index] = stain;
        } else if self.blood_stains.len() < self.materials.max_blood_stains {
            self.blood_stains.push(stain);
        } else {
            let index = self.blood_stain_write_cursor % self.blood_stains.len();
            self.stats.blood_unshown_ml += self.blood_stains[index].volume;
            self.blood_stains[index] = stain;
            self.blood_stain_write_cursor = (index + 1) % self.blood_stains.len();
            self.debug.blood_stain_budget_replacements += 1;
        }
        self.stats.blood_stain_deposits += 1;
        self.debug.blood_stain_deposits += 1;
        self.debug.active_blood_stains = self.active_blood_stain_count() as i32;
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
        // A piece of a cut triangle holds skin on its own side of the cut
        // only.
        (wa >= slack && wb >= slack && wc >= slack && triangle.holds([wa, wb, wc]))
            .then_some([wa, wb, wc])
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
        // The floor under the strip, where a 480 px window's lay.
        world.set_floor_y(442.0);
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
                        1.0,
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
            world.step(dt, &InputState::default(), 640.0);
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
    fn one_drop_beads_and_more_blood_starts_it_running() {
        let mut world = skin_strip(4, 12);
        let start = Vec2 { x: 215.0, y: 95.0 };
        world.well_blood(start, Vec2 { x: 0.0, y: 1.0 }, 1, 60.0, 1.35, 1.0);
        let bead = drops_on_skin(&world)[0];
        assert!(bead.volume < PIN_ML, "one drop of blood beads");
        run(&mut world, 30);
        let still = drops_on_skin(&world)[0];
        assert!(
            distance(still.position, bead.position) < 1.0e-9,
            "a bead of one drop should stay pinned"
        );
        for _ in 0..4 {
            world.well_blood(start, Vec2 { x: 0.0, y: 1.0 }, 1, 60.0, 1.35, 1.0);
            run(&mut world, 1);
        }
        run(&mut world, 30);
        let drops = drops_on_skin(&world);
        assert!(
            drops.iter().any(|drop| drop.position.y > start.y + 4.0),
            "beads that run together should grow big enough to run: {drops:?}"
        );
    }

    #[test]
    fn a_running_drop_gives_its_trail_blood() {
        let mut world = skin_strip(4, 12);
        let start = Vec2 { x: 215.0, y: 95.0 };
        world.well_blood(start, Vec2 { x: 0.0, y: 1.0 }, 1, 60.0, 1.35, 1.0);
        let index = world.fluids.iter().position(|f| f.life > 0.0).unwrap();
        world.fluids[index].volume = RUN_ML;
        run(&mut world, 30);
        let drops = drops_on_skin(&world);
        assert_eq!(drops.len(), 1);
        let ran = drops[0].position.y - start.y;
        assert!(ran > 8.0, "the drop should run down the skin: {ran:.1} px");
        assert!(drops[0].volume < RUN_ML, "it gives blood to its trail");
        assert!(!world.blood_trails.is_empty(), "it should leave a trail");
        let trail: f64 = world.blood_trails.iter().map(|t| t.volume).sum();
        assert!(
            (trail + drops[0].volume - RUN_ML).abs() < 1.0e-12,
            "the trail holds what the drop lost: {trail} + {}",
            drops[0].volume
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
            1.35,
            1.0,
        );
        let index = world.fluids.iter().position(|f| f.life > 0.0).unwrap();
        world.fluids[index].volume = RUN_ML;
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

    /// Blood is drawn as large as it would be on a 1.75 m person: a drop of
    /// blood is about 0.05 ml, a sphere about 4.6 mm across.
    #[test]
    fn a_drop_of_blood_is_drawn_as_large_as_one() {
        let world = World::new(Materials::default());
        let drop = FluidParticle {
            volume: BLOOD_DROP_ML,
            ..FluidParticle::default()
        };
        let across_mm = 2.0 * world.drop_radius(&drop) * world.mm_per_px();
        assert!((4.5..4.7).contains(&across_mm), "{across_mm:.2} mm");
        // At the scenarios' scale that is about one and a half pixels.
        let across_px = 2.0 * world.drop_radius(&drop);
        assert!((1.4..1.55).contains(&across_px), "{across_px:.2} px");
    }

    /// A drop of blood landing after a 1 m fall leaves a spot about 17 mm
    /// wide (MacDonell 1971), and 40 ml spreads into a pool about 19 cm wide
    /// on a smooth floor (1.44 mm deep; Smith, Nicloux, and Brutin 2020).
    #[test]
    fn spots_and_pools_are_as_wide_as_real_ones() {
        let one_metre = (2.0_f64 * 9.81 * 1.0).sqrt() * 1000.0;
        let spot = landing_spot_mm(BLOOD_DROP_ML, one_metre);
        assert!((16.5..17.5).contains(&spot), "{spot:.1} mm");
        let pool = pool_width_mm(40.0);
        assert!((180.0..195.0).contains(&pool), "{pool:.0} mm");
    }

    #[test]
    fn drawn_blood_is_the_blood_let_out() {
        let mut world = skin_strip(4, 12);
        world.materials.gravity = 920.0;
        for _ in 0..6 {
            world.well_blood(
                Vec2 { x: 215.0, y: 95.0 },
                Vec2 { x: 0.0, y: 1.0 },
                2,
                60.0,
                1.35,
                1.0,
            );
            world.release_blood(
                Vec2 { x: 400.0, y: 100.0 },
                Vec2 { x: 0.0, y: 1.0 },
                2,
                120.0,
                2.0,
                1.0,
            );
            run(&mut world, 20);
        }
        run(&mut world, 600);
        let shed = world.stats.blood_shed_ml;
        assert!((shed - 24.0 * BLOOD_DROP_ML).abs() < 1.0e-12, "{shed}");
        let accounted = world.blood_shown_ml() + world.stats.blood_unshown_ml;
        assert!(
            (accounted - shed).abs() < 1.0e-9,
            "shown {} and unshown {} should add up to the {shed} ml let out",
            world.blood_shown_ml(),
            world.stats.blood_unshown_ml
        );
        assert!(
            world.blood_stains.iter().any(|stain| stain.volume > 0.0),
            "the blood that fell should lie on the floor"
        );
    }
}
