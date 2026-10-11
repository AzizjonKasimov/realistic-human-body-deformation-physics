//! How a blow looks on the skin over time. A blow reddens the skin at once,
//! and the bruise the blood let into the skin makes darkens over the next
//! quarter hour: a fast blow's bruise shows almost at once (Scafide et al.
//! 2020) and is fully developed within 15 to 20 minutes (Randeberg et al.
//! 2007), while the redness gives way to it. Then the bruise stays for days
//! (yellow shows only after about 18 hours; Langlois and Gresham 1991), far
//! longer than anyone plays. So a hard blow leaves a red mark that turns into
//! a bruise while you watch, not a purple one the instant it lands
//! (`docs/INJURY_REFERENCE.md`).
//!
//! The simulation's contusion (`Point::contusion`) is the damage to the
//! flesh, which softens it and heals; this only colours the skin as someone
//! looking at it would see it, and changes nothing else. A deep bruise can
//! take days to show or never show at all (Nijs et al. 2019); the skin here
//! shows every bruise as a shallow one would.

use super::*;

/// How long, in seconds, the redness from a blow takes to fade to about a
/// third, giving way to the bruise.
const FLUSH_FADE_SECONDS: f64 = 350.0;
/// How long, in seconds, a bruise takes to show about two thirds of its
/// colour, so that it is fully developed (95%) in about 17 minutes.
const BRUISE_SHOW_SECONDS: f64 = 350.0;

impl World {
    /// Colours the skin by the blows and bleeding under it, as they look this
    /// long after (see the module notes).
    pub(super) fn colour_bruises(&mut self, dt: f64) {
        let fade = (dt / FLUSH_FADE_SECONDS).min(1.0);
        let show = (dt / BRUISE_SHOW_SECONDS).min(1.0);
        for point in &mut self.points {
            if point.layer != TissueLayer::Skin {
                continue;
            }
            point.flush = (point.flush * (1.0 - fade)).max(point.contusion);
            point.bruise = point.bruise.max(point.contusion);
            point.bruise_shown += (point.bruise - point.bruise_shown) * show;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bruised_skin() -> World {
        let mut world = World::new(Materials::default());
        let point = world.add_point(Vec2 { x: 10.0, y: 10.0 }, TissueLayer::Skin, true);
        world.points[point].contusion = 0.8;
        world
    }

    #[test]
    fn a_fresh_blow_reddens_the_skin_before_it_bruises() {
        let mut world = bruised_skin();
        let dt = world.materials.fixed_dt;
        for _ in 0..60 * 60 {
            world.colour_bruises(dt);
        }
        let point = world.points[0];
        assert!(
            point.flush > 0.6,
            "a minute on, the skin is red: {}",
            point.flush
        );
        assert!(
            point.bruise_shown < 0.2,
            "a minute on, the bruise is only starting: {}",
            point.bruise_shown
        );
    }

    #[test]
    fn a_bruise_is_fully_developed_in_a_quarter_hour_and_stays() {
        let mut world = bruised_skin();
        // The flesh heals its softening long before the bruise fades.
        world.points[0].contusion = 0.0;
        world.points[0].bruise = 0.8;
        for _ in 0..18 * 60 {
            world.colour_bruises(1.0);
        }
        let point = world.points[0];
        assert!(
            point.bruise_shown > 0.75,
            "18 minutes on, the bruise has developed: {}",
            point.bruise_shown
        );
        assert!(
            point.flush < 0.05,
            "and the redness is gone: {}",
            point.flush
        );
        for _ in 0..60 {
            world.colour_bruises(60.0);
        }
        assert!(
            world.points[0].bruise_shown > 0.79,
            "an hour later it stays"
        );
    }
}
