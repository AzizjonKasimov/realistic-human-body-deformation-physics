//! A fingerprint of where everything in a world is, to tell two runs that
//! should have simulated alike apart: the app's capture mode prints one, and
//! `tools\capture.ps1` checks that a play comes out the same in every window
//! size it captures.

use crate::{Vec2, World};

/// 64-bit FNV-1a over the bits of every point's, bone's, and drop's place and
/// motion, the blood pools, the tool's pose, and the injury counts: any
/// difference at all between two worlds changes it, almost surely.
pub fn digest(world: &World) -> u64 {
    let mut hash = Fnv(0xcbf2_9ce4_8422_2325);
    for point in world.points() {
        hash.vec(point.position);
        hash.vec(point.previous);
        hash.f64(point.contusion);
    }
    for bone in world.bones() {
        hash.vec(bone.a);
        hash.vec(bone.b);
        hash.vec(bone.previous_a);
        hash.vec(bone.previous_b);
        hash.u64(u64::from(bone.fractured));
    }
    for fluid in world.fluids() {
        hash.vec(fluid.position);
        hash.vec(fluid.previous);
        hash.f64(fluid.volume);
        hash.f64(fluid.life);
    }
    for stain in world.blood_stains() {
        hash.vec(stain.position);
        hash.f64(stain.width);
        hash.f64(stain.volume);
    }
    if let Some(pose) = world.current_tool_pose() {
        hash.vec(pose.center);
        hash.vec(pose.heading);
    }
    // Every count and the blood lost, floats written exactly.
    for byte in format!("{:?}", world.stats()).bytes() {
        hash.byte(byte);
    }
    hash.0
}

struct Fnv(u64);

impl Fnv {
    fn byte(&mut self, byte: u8) {
        self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }

    fn u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.byte(byte);
        }
    }

    fn f64(&mut self, value: f64) {
        self.u64(value.to_bits());
    }

    fn vec(&mut self, value: Vec2) {
        self.f64(value.x);
        self.f64(value.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::{SCENARIO_HEIGHT, SCENARIO_WIDTH};
    use crate::{create_layered_body, swing_power, InputState, Materials, ToolMode};

    #[test]
    fn a_world_and_its_copy_agree_until_one_moves() {
        let mut world = create_layered_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, Materials::default());
        let copy = world.clone();
        assert_eq!(digest(&world), digest(&copy));
        let dt = world.materials().fixed_dt;
        let hand = InputState {
            active: true,
            x: 300.0,
            y: 300.0,
            power: swing_power(ToolMode::Heavy),
            tool: ToolMode::Heavy,
            ..InputState::default()
        };
        world.step(dt, &hand, SCENARIO_WIDTH);
        assert_ne!(digest(&world), digest(&copy));
    }
}
