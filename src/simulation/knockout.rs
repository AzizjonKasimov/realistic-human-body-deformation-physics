//! A side-view fighter knocked out by a blow to the head. What puts a person
//! out is how hard a blow turns the head: the brain lags the skull as it
//! turns, and the strain that comes with the turning takes consciousness,
//! where a blow that only throws the head does far less (Ommaya and
//! Gennarelli 1974; Gennarelli et al. 1982). So the engine watches how each
//! fighter's head turns, substep by substep, as a mouthguard's sensors watch
//! a boxer's, and a fighter whose head's turning changed faster than
//! `Materials::knockout_spin` ([`KNOCKOUT_SPIN`]) is knocked out: its legs give way and stop carrying it,
//! every muscle lets go, as an arm's do when a bone in it breaks
//! (`World::let_limb_go`), and it stays down (`Mover::fallen`).
//!
//! The fighters' punches turn the head about as hard as people's: a cross
//! landed well turned the struck head at about 2,300 rad/s², and jabs and
//! crosses landed from every distance at about 1,600 on average
//! (`print_fighter_punches`), where boxing turned the heads of 22 West Point
//! cadets sparring at 1,520 ± 960 rad/s² (at most 8,850 in 205 bouts,
//! instrumented mouthguards) and mixed martial arts the heads of 13 fighters
//! at 2,567 ± 1,739 (Tiernan et al. 2020). A dummy's head takes the same
//! punches harder, 6,343 ± 1,789 for Olympic boxers' straight punches
//! (Walilko, Viano and Bir 2005), and is not built for punches.
//!
//! Left out for now: getting up, how long a knockout lasts, and the stiff
//! arm two in three knocked-out fighters hold up for a few seconds as they
//! fall (Hosseini and Lifshitz 2009).

use super::*;

/// How fast a blow must change how fast a fighter's head turns to knock it
/// out, in radians a second squared, from one substep to the next, as
/// `Materials::knockout_spin` is built. No study
/// gives a knockout threshold. Concussion comes at even odds near 5,800 to
/// 6,400 rad/s² (King et al. 2003; Rowson et al. 2012), but only about one
/// concussion in ten puts the person out (9.3% of the NFL's, Pellman et al.
/// 2004), and the one knockout measured on a person turned the head at
/// 12,900 rad/s² and 34 rad/s (Hernandez et al. 2015). So between the two:
/// a punch must turn the head about four times as hard as a cross landed
/// well.
pub(super) const KNOCKOUT_SPIN: f64 = 10_000.0;

/// How a side-view fighter's head has turned (`World::watch_heads`), and
/// whether a blow has knocked it out.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeadSpin {
    /// How fast the head turned over the last substep, in radians a second;
    /// `None` before the first.
    last: Option<f64>,
    /// The fastest the head has turned, in radians a second, and the
    /// fastest its turning has changed from one substep to the next, in
    /// radians a second squared, since the fighter was built.
    pub fastest: f64,
    pub hardest: f64,
    /// Whether a blow has knocked it out.
    pub knocked_out: bool,
}

impl HeadSpin {
    /// The same head in a mirror image, turning the other way.
    pub(super) fn mirror(&mut self) {
        self.last = self.last.map(|spin| -spin);
    }
}

impl World {
    /// Watches how each side-view fighter's head turns as a substep `h`
    /// long begins, from how its skull turned over the last: a fighter still
    /// on its feet whose head's turning changed faster than
    /// `Materials::knockout_spin` is knocked out.
    pub(super) fn watch_heads(&mut self, h: f64) {
        let limit = self.materials.knockout_spin;
        for body in 0..self.bodies.len() {
            let Some(skull) = self.bodies[body].fighter.as_ref().map(|rig| rig.skull) else {
                continue;
            };
            let Some(&bone) = self.bones.get(skull) else {
                continue;
            };
            let angle = |a: Vec2, b: Vec2| libm::atan2(b.y - a.y, b.x - a.x);
            let spin =
                wrap_angle(angle(bone.a, bone.b) - angle(bone.previous_a, bone.previous_b)) / h;
            let head = &mut self.bodies[body].head;
            let jolt = head.last.map_or(0.0, |last| (spin - last).abs() / h);
            head.last = Some(spin);
            head.fastest = head.fastest.max(spin.abs());
            head.hardest = head.hardest.max(jolt);
            if jolt > limit && !head.knocked_out && !self.fighter_fallen(body) {
                self.knock_out(body, jolt);
            }
        }
    }

    /// Knocks side-view fighter `body` out, its head's turning changed at
    /// `jolt` radians a second squared: every muscle lets go, the pull each
    /// carried with it, its legs no longer carry it, and it takes no more
    /// moves.
    fn knock_out(&mut self, body: usize, jolt: f64) {
        self.bodies[body].head.knocked_out = true;
        if let Some(mover) = &mut self.bodies[body].mover {
            mover.knock_down();
        }
        for index in 0..self.bone_joints.len() {
            if self.joint_body(&self.bone_joints[index]) == body {
                self.bone_joints[index].drive_impulse = 0.0;
            }
        }
        let skull = self.bodies[body]
            .fighter
            .as_ref()
            .map_or(MISSING_BONE, |rig| rig.skull);
        let at = self
            .bones
            .get(skull)
            .map_or(Vec2::default(), |bone| lerp(bone.a, bone.b, 0.5));
        self.note(
            EventKind::KnockedOut,
            Subject::Bone(skull),
            at,
            None,
            Measure::Spin,
            jolt,
            self.materials.knockout_spin,
        );
    }

    /// Whether body `body` has been knocked out.
    pub fn knocked_out(&self, body: usize) -> bool {
        self.bodies
            .get(body)
            .is_some_and(|state| state.head.knocked_out)
    }

    /// The hardest body `body`'s head has been turned, in radians a second
    /// squared ([`HeadSpin::hardest`]), and the fastest, in radians a second.
    pub fn hardest_head_spin(&self, body: usize) -> Option<(f64, f64)> {
        self.bodies
            .get(body)
            .filter(|state| state.fighter.is_some())
            .map(|state| (state.head.hardest, state.head.fastest))
    }

    /// Whether the muscles of body `body`'s `limb` have let go: it hangs
    /// from a broken bone, or the body is knocked out. A knocked-out body's
    /// head and trunk let go too, for which `limb` is `Limb::None`.
    pub(super) fn muscles_let_go(&self, body: usize, limb: Limb) -> bool {
        self.limp(body, limb) || self.knocked_out(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::SCENARIO_WIDTH;

    /// Two fighters that have stood for half a second.
    fn standing() -> World {
        let mut world = create_side_fighters(Materials::default());
        world.keep_journal();
        let dt = world.materials.fixed_dt;
        for _ in 0..30 {
            world.step(dt, &InputState::default(), SCENARIO_WIDTH);
        }
        world
    }

    /// A head whose turning changes faster than a knockout takes, from one
    /// substep to the next, is knocked out, and one a little slower is not.
    #[test]
    fn a_jolt_past_the_limit_knocks_a_fighter_out() {
        let h = Materials::default().fixed_dt / Materials::default().substeps as f64;
        for (share, out) in [(0.9, false), (1.1, true)] {
            let mut world = standing();
            world.watch_heads(h);
            // As though the head had turned slower over the last substep, by
            // `share` of the limit.
            let last = world.bodies[1].head.last.unwrap();
            world.bodies[1].head.last = Some(last - share * KNOCKOUT_SPIN * h);
            world.watch_heads(h);
            assert_eq!(
                world.knocked_out(1),
                out,
                "a head's turning changed at {share} of the knockout limit"
            );
            assert!(!world.knocked_out(0), "the other fighter was knocked out");
        }
    }

    /// A cross knocks out a fighter whose head takes less than it gives,
    /// here with the limit lowered under what a cross landed well gives:
    /// the fighter goes limp, falls, and stays down, taking no moves, and
    /// nothing tears as it falls. The fighter that threw it stands on.
    #[test]
    fn a_fighter_knocked_out_falls_and_stays_down() {
        let mut world = create_side_fighters_at(Materials::default(), 0.60);
        world.keep_journal();
        world.materials.knockout_spin = 1_500.0;
        let dt = world.materials.fixed_dt;
        let rig = world.bodies[1].fighter.clone().unwrap();
        let cm = world.mm_per_px() * 0.1;
        let up = |world: &World, at: Vec2| (world.floor_y() - at.y) * cm;
        let hips = |world: &World, body: usize| {
            let rig = world.bodies[body].fighter.as_ref().unwrap();
            up(world, world.bones[rig.pelvis].b)
        };
        let stood = hips(&world, 0);
        let mut out_at = None;
        for step in 0..200 {
            let mut input = InputState::default();
            if step == 20 {
                input.moves[0] = Some((0, Move::Cross));
            }
            // Told to move once knocked out, it takes no notice.
            if step > 40 && step % 30 == 0 {
                input.moves[1] = Some((1, Move::StepIn));
            }
            world.step(dt, &input, SCENARIO_WIDTH);
            if world.knocked_out(1) && out_at.is_none() {
                out_at = Some(step);
            }
        }
        let skull = world.bones[rig.skull];
        let head = up(&world, lerp(skull.a, skull.b, 0.5));
        assert!(
            out_at.is_some_and(|step| (20..35).contains(&step)) && world.fighter_fallen(1),
            "knocked out at step {out_at:?}, fallen: {}",
            world.fighter_fallen(1)
        );
        assert!(
            hips(&world, 1) < 30.0 && head < 30.0,
            "three seconds after a knockout the fighter's hips are {:.0} cm up and its head {head:.0}",
            hips(&world, 1)
        );
        assert!(
            (hips(&world, 0) - stood).abs() < 5.0 && !world.knocked_out(0),
            "the fighter that threw the cross had its hips {stood:.0} cm up and now {:.0}",
            hips(&world, 0)
        );
        assert_eq!(
            (
                world.stats.broken_skin,
                world.stats.broken_muscle,
                world.stats.broken_bone_joints,
                world.stats.bone_joint_subluxations,
                world.stats.fractured_bones
            ),
            (0, 0, 0, 0, 0),
            "torn skin, muscle, joints; joints slipped; bones broken"
        );
    }
}
