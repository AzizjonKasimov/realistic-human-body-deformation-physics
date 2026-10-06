//! Drawing between simulation steps. The simulation steps at a fixed 60 Hz,
//! but screens refresh at 120 Hz or more, where a body drawn only as each step
//! left it moves in visible jumps while the cursor glides. The app keeps where
//! everything was before the last step and draws the tissue, bones, and tool
//! that far between the two, which trails the simulation by less than a step.

use super::*;

/// Where the tissue, bones, and tool were at one moment.
#[derive(Clone, Debug, Default)]
pub struct MotionSnapshot {
    points: Vec<Vec2>,
    bones: Vec<(Vec2, Vec2)>,
    /// The tool's position, heading, and side, while it is in play.
    tool: Option<(Vec2, Vec2, Vec2)>,
}

impl World {
    /// Records where the tissue, bones, and tool are now.
    pub fn capture_motion(&self, snapshot: &mut MotionSnapshot) {
        snapshot.points.clear();
        snapshot
            .points
            .extend(self.points.iter().map(|point| point.position));
        snapshot.bones.clear();
        snapshot
            .bones
            .extend(self.bones.iter().map(|bone| (bone.a, bone.b)));
        snapshot.tool = self
            .current_tool_pose()
            .map(|pose| (pose.center, pose.heading, pose.side));
    }

    /// Moves the tissue, bones, and tool back toward where they were in
    /// `earlier`, leaving them `alpha` of the way from there to where they
    /// are, so they can be drawn between two steps. Where they really are goes
    /// into `actual`; [`World::restore_motion`] puts them back before the next
    /// step. Anything new since `earlier`, such as a fresh bone fragment,
    /// stays where it is.
    pub fn blend_motion(
        &mut self,
        earlier: &MotionSnapshot,
        alpha: f64,
        actual: &mut MotionSnapshot,
    ) {
        self.capture_motion(actual);
        let alpha = alpha.clamp(0.0, 1.0);
        for (point, &before) in self.points.iter_mut().zip(&earlier.points) {
            point.position = lerp(before, point.position, alpha);
        }
        for (bone, &(before_a, before_b)) in self.bones.iter_mut().zip(&earlier.bones) {
            bone.a = lerp(before_a, bone.a, alpha);
            bone.b = lerp(before_b, bone.b, alpha);
        }
        if let (Some((position, heading, side)), true) = (earlier.tool, self.tool.present) {
            self.tool.position = lerp(position, self.tool.position, alpha);
            self.tool.heading =
                normalized(lerp(heading, self.tool.heading, alpha), self.tool.heading);
            self.tool.side = normalized(lerp(side, self.tool.side, alpha), self.tool.side);
        }
    }

    /// Puts the tissue, bones, and tool back where [`World::blend_motion`]
    /// found them.
    pub fn restore_motion(&mut self, actual: &MotionSnapshot) {
        for (point, &position) in self.points.iter_mut().zip(&actual.points) {
            point.position = position;
        }
        for (bone, &(a, b)) in self.bones.iter_mut().zip(&actual.bones) {
            bone.a = a;
            bone.b = b;
        }
        if let (Some((position, heading, side)), true) = (actual.tool, self.tool.present) {
            self.tool.position = position;
            self.tool.heading = heading;
            self.tool.side = side;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blending_draws_between_steps_and_restores_the_world() {
        let (width, height) = (1280.0, 720.0);
        let mut world = create_layered_body(width, height, Materials::default());
        let dt = world.materials.fixed_dt;
        let hand = InputState {
            active: true,
            x: 300.0,
            y: 300.0,
            ..InputState::default()
        };
        world.step(dt, &hand, width, height);
        let mut earlier = MotionSnapshot::default();
        world.capture_motion(&mut earlier);
        world.step(dt, &InputState { x: 360.0, ..hand }, width, height);
        let tissue: Vec<Vec2> = world.points.iter().map(|point| point.position).collect();
        let tool = world.tool.position;
        let tool_before = earlier.tool.expect("the tool is in play").0;

        let mut actual = MotionSnapshot::default();
        world.blend_motion(&earlier, 0.5, &mut actual);
        let halfway = world.tool.position;
        assert!((halfway.x - (tool_before.x + tool.x) * 0.5).abs() < 1.0e-9);
        assert!(
            tool.x > tool_before.x,
            "the tool should have moved toward the hand"
        );

        world.restore_motion(&actual);
        assert_eq!(world.tool.position.x, tool.x);
        for (point, position) in world.points.iter().zip(&tissue) {
            assert_eq!(point.position.x, position.x);
            assert_eq!(point.position.y, position.y);
        }
    }
}
