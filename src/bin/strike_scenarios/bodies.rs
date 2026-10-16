//! Bodies built once and copied. Every run of a check starts from the same
//! body for its window and settings, built and settled under its weight
//! (`create_layered_body`), which took about a third of each sweep run and
//! half of a random blow. Building is deterministic, so each body is built
//! the first time a run asks for it and every run plays a copy of it, just as
//! if it had built its own: the sweep, the moved-body tests, and the random
//! blows play exactly what they did before, only sooner.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::*;

/// The bodies built so far, by window and settings, each built once even when
/// several threads ask for it at the same time.
type Bodies = Mutex<HashMap<String, Arc<OnceLock<rp::World>>>>;

/// A settled body for a window of `width` by `height` built from
/// `materials`: a copy of the one built for them first.
pub(crate) fn settled_body(width: f64, height: f64, materials: rp::Materials) -> rp::World {
    static BODIES: OnceLock<Bodies> = OnceLock::new();
    let key = format!("{width:?} {height:?} {materials:?}");
    let slot = BODIES
        .get_or_init(Default::default)
        .lock()
        .expect("the bodies built")
        .entry(key)
        .or_default()
        .clone();
    slot.get_or_init(|| rp::create_layered_body(width, height, materials))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A copy plays exactly as a body built afresh: the checks rely on it, and
    /// anything the world holds that does not copy whole would break it.
    #[test]
    fn a_copied_body_plays_as_one_built_afresh() {
        let materials = rp::Materials::default();
        let mut fresh = rp::create_layered_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, materials);
        let _first = settled_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, materials);
        let mut copy = settled_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, materials);
        let frame = rp::body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT);
        let dt = materials.fixed_dt;
        for step in 0..40 {
            // A hammer swung through the guard's elbow into the chest, so the
            // copy is hurt as well as moved.
            let at = frame.point(-0.40 + 0.04 * step as f64, 0.325);
            let input = rp::InputState {
                active: true,
                down: step >= 5,
                x: at.x,
                y: at.y,
                tool: rp::ToolMode::Heavy,
                power: rp::swing_power(rp::ToolMode::Heavy),
                ..rp::InputState::default()
            };
            fresh.step(dt, &input, SCENARIO_WIDTH);
            copy.step(dt, &input, SCENARIO_WIDTH);
        }
        assert_eq!(fresh.points().len(), copy.points().len());
        for (a, b) in fresh.points().iter().zip(copy.points()) {
            assert_eq!(
                (a.position.x.to_bits(), a.position.y.to_bits()),
                (b.position.x.to_bits(), b.position.y.to_bits())
            );
        }
        for (a, b) in fresh.bones().iter().zip(copy.bones()) {
            assert_eq!(
                (
                    a.a.x.to_bits(),
                    a.a.y.to_bits(),
                    a.b.x.to_bits(),
                    a.b.y.to_bits()
                ),
                (
                    b.a.x.to_bits(),
                    b.a.y.to_bits(),
                    b.b.x.to_bits(),
                    b.b.y.to_bits()
                )
            );
        }
        assert_eq!(
            format!("{:?}", fresh.stats()),
            format!("{:?}", copy.stats())
        );
    }
}
