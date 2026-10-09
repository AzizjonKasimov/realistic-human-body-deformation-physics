//! Blood that holds together as a liquid. Drops near each other pull together
//! and push apart toward a rest spacing, and their motion toward each other
//! damps, so blood runs from a wound in streams and gathers into drops instead
//! of flying as separate specks.
//!
//! Rebuilt from the double-density relaxation and viscosity of Clavet,
//! Beaudoin, and Poulin, "Particle-based Viscoelastic Fluid Simulation"
//! (SCA 2005), as LiquidFun's particle system also builds on them: each drop's
//! density and near density from its neighbors set a pressure that pushes
//! pairs apart when packed and pulls them together when sparse, and the near
//! pressure keeps them from collapsing into one point.

use super::*;

/// How far, in pixels, drops reach each other.
const COHESION_RADIUS: f64 = 9.0;
/// The density a drop settles at among its neighbors: about a neighbor or
/// two at a third of the reach.
const REST_DENSITY: f64 = 1.0;
/// How far, in pixels, one unit of pressure moves a pair of drops in a step.
const PRESSURE_STEP: f64 = 0.3;
/// How much stronger the near pressure, which keeps drops apart, is.
const NEAR_PRESSURE_SHARE: f64 = 2.0;
/// Linear and quadratic damping of the speed at which two drops close, per
/// step: blood is thick.
const VISCOSITY_LINEAR: f64 = 0.20;
const VISCOSITY_QUADRATIC: f64 = 0.02;

impl World {
    /// Lets the moving blood drops pull together into streams and drops (see
    /// the module notes). Settled drops have stopped and stay out of it.
    pub(super) fn cohere_blood(&mut self) {
        let moving = |fluid: &FluidParticle| fluid.life > 0.0 && !fluid.settled;
        let reach = COHESION_RADIUS;
        let mut grid = std::mem::take(&mut self.grid);
        grid.build_points(
            reach,
            self.fluids
                .iter()
                .enumerate()
                .filter(|(_, fluid)| moving(fluid))
                .map(|(index, fluid)| (index, fluid.position)),
        );
        let mut around = Vec::new();

        // Viscosity: the speed at which a pair closes damps, each drop taking
        // half the impulse.
        for i in 0..self.fluids.len() {
            if !moving(&self.fluids[i]) {
                continue;
            }
            let position = self.fluids[i].position;
            grid.query(segment_aabb(position, position, reach), &mut around);
            for &j in &around {
                if j <= i {
                    continue;
                }
                let (fi, fj) = (self.fluids[i], self.fluids[j]);
                let offset = subtract(fj.position, fi.position);
                let gap = hypot(offset.x, offset.y);
                if gap >= reach || gap < EPSILON {
                    continue;
                }
                let toward = scale(offset, 1.0 / gap);
                let q = gap / reach;
                let vi = subtract(fi.position, fi.previous);
                let vj = subtract(fj.position, fj.previous);
                let closing = dot(subtract(vi, vj), toward);
                if closing <= 0.0 {
                    continue;
                }
                let impulse = (1.0 - q)
                    * (VISCOSITY_LINEAR * closing + VISCOSITY_QUADRATIC * closing * closing);
                let half = scale(toward, impulse * 0.5);
                // A drop's step from where it was is its velocity: less of it
                // toward the other is less closing.
                self.fluids[i].previous = add(self.fluids[i].previous, half);
                self.fluids[j].previous = subtract(self.fluids[j].previous, half);
            }
        }

        // Double-density relaxation.
        let mut close = Vec::new();
        for i in 0..self.fluids.len() {
            if !moving(&self.fluids[i]) {
                continue;
            }
            let position = self.fluids[i].position;
            grid.query(segment_aabb(position, position, reach), &mut around);
            let mut density = 0.0;
            let mut near_density = 0.0;
            close.clear();
            for &j in &around {
                if j == i {
                    continue;
                }
                let offset = subtract(self.fluids[j].position, position);
                let gap = hypot(offset.x, offset.y);
                if gap >= reach || gap < EPSILON {
                    continue;
                }
                let q = 1.0 - gap / reach;
                density += q * q;
                near_density += q * q * q;
                close.push((j, scale(offset, 1.0 / gap), q));
            }
            if close.is_empty() {
                continue;
            }
            let pressure = density - REST_DENSITY;
            let near_pressure = near_density * NEAR_PRESSURE_SHARE;
            let mut shift = Vec2::default();
            for &(j, toward, q) in &close {
                let push = PRESSURE_STEP * (pressure * q + near_pressure * q * q);
                let half = scale(toward, push * 0.5);
                self.fluids[j].position = add(self.fluids[j].position, half);
                shift = subtract(shift, half);
            }
            self.fluids[i].position = add(self.fluids[i].position, shift);
        }
        self.grid = grid;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drop_at(x: f64, y: f64) -> FluidParticle {
        FluidParticle {
            position: Vec2 { x, y },
            previous: Vec2 { x, y },
            radius: 2.0,
            life: 5.0,
            max_life: 5.0,
            ..FluidParticle::default()
        }
    }

    #[test]
    fn nearby_drops_pull_together_and_keep_apart() {
        let mut world = World::new(Materials::default());
        world.fluids = vec![drop_at(100.0, 100.0), drop_at(106.0, 100.0)];
        for _ in 0..30 {
            world.cohere_blood();
        }
        let gap = distance(world.fluids[0].position, world.fluids[1].position);
        assert!(
            gap < 6.0,
            "two drops a little apart should draw together: {gap:.2} px"
        );
        assert!(
            gap > 1.0,
            "drops should keep apart rather than merge into one point: {gap:.2} px"
        );
    }

    #[test]
    fn drops_out_of_reach_ignore_each_other() {
        let mut world = World::new(Materials::default());
        world.fluids = vec![
            drop_at(100.0, 100.0),
            drop_at(100.0 + COHESION_RADIUS * 2.0, 100.0),
        ];
        world.cohere_blood();
        assert_eq!(world.fluids[0].position.x, 100.0);
        assert_eq!(world.fluids[1].position.x, 100.0 + COHESION_RADIUS * 2.0);
    }
}
