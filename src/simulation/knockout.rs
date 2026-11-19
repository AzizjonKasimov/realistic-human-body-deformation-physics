//! A side-view fighter knocked out by a blow to the head. What puts a person
//! out is how hard a blow turns the head: the brain lags the skull as it
//! turns, and the strain that comes with the turning takes consciousness,
//! where a blow that only throws the head does far less (Ommaya and
//! Gennarelli 1974; Gennarelli et al. 1982). So the engine watches how each
//! fighter's head turns, substep by substep, as a mouthguard's sensors watch
//! a boxer's, and knocks out a fighter whose head a blow turns past either
//! of the two measures the one knockout measured on a person reached: its
//! turning changed by more than `Materials::knockout_turn` within the blow
//! ([`KNOCKOUT_TURN`]), or faster than `Materials::knockout_spin` from one
//! substep to the next ([`KNOCKOUT_SPIN`]). Its legs give way and stop
//! carrying it, every muscle lets go, as an arm's do when a bone in it breaks
//! (`World::let_limb_go`), and it goes down (`Mover::down`), to lie limp for
//! longer the harder the blow and then get up (`rise.rs`).
//!
//! A fighter's punches aim at the other's jaw, as boxers' do (`mover.rs`),
//! and turn the head about as hard as people's: a cross landed well turns
//! the struck head at about 2,800 to 3,100 rad/s² and 17 to 20 rad/s, and
//! jabs and crosses landed from every distance less (`print_fighter_punches`),
//! where boxing turned the heads of 22 West Point cadets sparring at 1,520 ±
//! 960 rad/s² (at most 8,850 in 205 bouts, instrumented mouthguards) and
//! mixed martial arts the heads of 13 fighters at 2,567 ± 1,739 (Tiernan et
//! al. 2020). A dummy's head took Olympic boxers' hardest straight punches to
//! the chin at 6,343 ± 1,789 rad/s² (Walilko, Viano and Bir 2005).
//!
//! Left out for now: the stiff arm two in three knocked-out fighters hold
//! up for a few seconds as they fall (Hosseini and Lifshitz 2009).

use super::rise::Down;
use super::*;

/// How fast a blow must change how fast a fighter's head turns to knock it
/// out, in radians a second squared, from one substep to the next, as
/// `Materials::knockout_spin` is built. No study gives a knockout threshold.
/// Concussion comes at even odds near 5,800 to 6,400 rad/s² and 28 rad/s
/// (King et al. 2003; Rowson et al. 2012), but only about one concussion in
/// ten puts the person out (9.3% of the NFL's, Pellman et al. 2004), and the
/// one knockout measured on a person, a college football player tackled,
/// his head turned mostly sideways, turned it at 12,900 rad/s² and 34 rad/s
/// (Hernandez et al. 2015).
pub(super) const KNOCKOUT_SPIN: f64 = 10_000.0;

/// How much a blow must change how fast a fighter's head turns, within the
/// blow, to knock it out, in radians a second, as `Materials::knockout_turn`
/// is built: as much as the one knockout measured (34 rad/s, above the 28 of
/// concussion at even odds). A punch's blow lasts longer than a tackle's,
/// so it reaches this before [`KNOCKOUT_SPIN`]: over the fighters' 8 ms blow,
/// 10,000 rad/s² would turn the head at 51 rad/s.
pub(super) const KNOCKOUT_TURN: f64 = 34.0;

/// How many substeps a blow's change in the head's turning is measured over
/// (17 ms): the 8 ms of a fist's push and a little more, before the neck
/// pulls the head back. Over 33 ms the head's swing back counted as well,
/// and a cross to the jaw seemed to change its turning by 33 rad/s where the
/// blow itself changed it by 22.
const BLOW_SUBSTEPS: usize = 4;

/// How a side-view fighter's head has turned (`World::watch_heads`), and
/// whether a blow has knocked it out.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeadSpin {
    /// How fast the head turned over each of the last substeps, in radians
    /// a second, oldest first, and how many of them there are yet.
    recent: [f64; BLOW_SUBSTEPS],
    seen: usize,
    /// Since the fighter was built: the fastest the head has turned, in
    /// radians a second; the fastest its turning has changed from one
    /// substep to the next, in radians a second squared; and the most its
    /// turning has changed within a blow's time, in radians a second.
    pub fastest: f64,
    pub hardest: f64,
    pub changed: f64,
    /// Whether a blow has knocked it out.
    pub knocked_out: bool,
}

impl HeadSpin {
    /// The same head in a mirror image, turning the other way.
    pub(super) fn mirror(&mut self) {
        for spin in &mut self.recent {
            *spin = -*spin;
        }
    }

    /// Takes how fast the head turned over the substep `h` long that just
    /// ended, in radians a second: returns how fast its turning changed from
    /// the substep before, in radians a second squared, and how much it has
    /// changed within a blow's time, in radians a second.
    fn watch(&mut self, spin: f64, h: f64) -> (f64, f64) {
        let jolt = if self.seen > 0 {
            (spin - self.recent[BLOW_SUBSTEPS - 1]).abs() / h
        } else {
            0.0
        };
        self.recent.rotate_left(1);
        self.recent[BLOW_SUBSTEPS - 1] = spin;
        self.seen = (self.seen + 1).min(BLOW_SUBSTEPS);
        let kept = &self.recent[BLOW_SUBSTEPS - self.seen..];
        let change = kept.iter().fold(f64::MIN, |most, &spin| most.max(spin))
            - kept.iter().fold(f64::MAX, |least, &spin| least.min(spin));
        self.fastest = self.fastest.max(spin.abs());
        self.hardest = self.hardest.max(jolt);
        self.changed = self.changed.max(change);
        (jolt, change)
    }
}

impl World {
    /// Watches how each side-view fighter's head turns as a substep `h`
    /// long begins, from how its skull turned over the last: a fighter still
    /// on its feet whose head a blow turns past `Materials::knockout_turn` or
    /// `Materials::knockout_spin` is knocked out.
    pub(super) fn watch_heads(&mut self, h: f64) {
        let (turn_limit, spin_limit) = (self.materials.knockout_turn, self.materials.knockout_spin);
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
            let (jolt, change) = self.bodies[body].head.watch(spin, h);
            if self.bodies[body].head.knocked_out || self.fighter_fallen(body) {
                continue;
            }
            if change > turn_limit {
                self.knock_out(body, Measure::Turn, change, turn_limit);
            } else if jolt > spin_limit {
                self.knock_out(body, Measure::Spin, jolt, spin_limit);
            }
        }
    }

    /// Knocks side-view fighter `body` out, its head turned by a blow to
    /// `value` past `limit`, as `measure` measures them: every muscle lets
    /// go, the pull each carried with it, its legs no longer carry it, and it
    /// takes no more moves, lying limp for longer the further past the limit
    /// the blow went, until it comes to and gets up (`rise.rs`).
    fn knock_out(&mut self, body: usize, measure: Measure, value: f64, limit: f64) {
        self.bodies[body].head.knocked_out = true;
        // Out for longer the harder the blow turned the head past the limit
        // (`rise.rs`).
        let past = if limit > 0.0 {
            (value / limit - 1.0).max(0.0)
        } else {
            0.0
        };
        let out_for = self.materials.knockout_seconds + self.materials.knockout_more_seconds * past;
        if let Some(mover) = &mut self.bodies[body].mover {
            mover.go_down(Down::knocked_out(out_for));
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
            measure,
            value,
            limit,
        );
    }

    /// Whether body `body` has been knocked out.
    pub fn knocked_out(&self, body: usize) -> bool {
        self.bodies
            .get(body)
            .is_some_and(|state| state.head.knocked_out)
    }

    /// How hard body `body`'s head has been turned, if it is a side-view
    /// fighter ([`HeadSpin`]): the fastest its turning changed from one
    /// substep to the next, in radians a second squared, the fastest it
    /// turned, and the most its turning changed within a blow's time, in
    /// radians a second.
    pub fn hardest_head_spin(&self, body: usize) -> Option<(f64, f64, f64)> {
        self.bodies
            .get(body)
            .filter(|state| state.fighter.is_some())
            .map(|state| (state.head.hardest, state.head.fastest, state.head.changed))
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

    /// A head whose turning a blow changes by more than a knockout takes,
    /// within the blow, or faster than one takes from one substep to the
    /// next, is knocked out, and one turned a little less is not.
    #[test]
    fn a_head_turned_past_either_limit_is_knocked_out() {
        let h = Materials::default().fixed_dt / Materials::default().substeps as f64;
        for (share, out) in [(0.9, false), (1.1, true)] {
            // Its turning changed by `share` of the limit within the blow,
            // over a substep: far slower than the other limit.
            let mut world = standing();
            world.watch_heads(h);
            let now = world.bodies[1].head.recent[BLOW_SUBSTEPS - 1];
            world.bodies[1].head.recent = [now - share * KNOCKOUT_TURN; BLOW_SUBSTEPS];
            world.watch_heads(h);
            assert_eq!(
                world.knocked_out(1),
                out,
                "a head's turning changed by {share} of the limit within a blow"
            );
            // Its turning changing at `share` of the other limit, from one
            // substep to the next, with no limit on how much.
            let mut world = standing();
            world.materials.knockout_turn = f64::INFINITY;
            world.watch_heads(h);
            let now = world.bodies[1].head.recent[BLOW_SUBSTEPS - 1];
            world.bodies[1].head.recent = [now - share * KNOCKOUT_SPIN * h; BLOW_SUBSTEPS];
            world.watch_heads(h);
            assert_eq!(
                world.knocked_out(1),
                out,
                "a head's turning changed at {share} of the limit from one substep to the next"
            );
            assert!(!world.knocked_out(0), "the other fighter was knocked out");
        }
    }

    /// A cross knocks out a fighter whose head takes less than it gives,
    /// here with the limit lowered under what a cross landed well gives (10
    /// rad/s within the blow): the fighter goes limp, falls, and lies limp
    /// for as long as the blow sets, taking no moves, and nothing tears as it
    /// falls. Then it comes to and gets up (`rise.rs`), seen getting up, and
    /// stands in its stance, nothing torn. The fighter that threw it stands
    /// on, steps back as a referee would send it, and throws no punch at it
    /// until it stands.
    #[test]
    fn a_fighter_knocked_out_lies_limp_then_gets_up() {
        let mut world = create_side_fighters_at(Materials::default(), 0.60);
        world.keep_journal();
        world.materials.knockout_turn = 10.0;
        let dt = world.materials.fixed_dt;
        let rig = world.bodies[1].fighter.clone().unwrap();
        let cm = world.mm_per_px() * 0.1;
        let up = |world: &World, at: Vec2| (world.floor_y() - at.y) * cm;
        let hips = |world: &World, body: usize| {
            let rig = world.bodies[body].fighter.as_ref().unwrap();
            up(world, world.bones[rig.pelvis].b)
        };
        let stood = hips(&world, 0);
        let (mut out_at, mut lying, mut seen_getting_up, mut up_at) = (None, None, false, None);
        for step in 0..900 {
            let mut input = InputState::default();
            if step == 20 {
                input.moves[0] = Some((0, Move::Cross));
            }
            if [80, 125, 170, 215].contains(&step) {
                input.moves[0] = Some((0, Move::StepOut));
            }
            // Told to move while down, it takes no notice; and the other,
            // told to punch it, does not.
            if step > 40 && step % 30 == 0 && up_at.is_none() {
                input.moves[1] = Some((1, Move::StepIn));
                input.moves[2] = Some((0, Move::Cross));
            }
            world.step(dt, &input, SCENARIO_WIDTH);
            if step > 40 && up_at.is_none() {
                assert!(
                    !world.striking_out(0, Limb::RightArm) && !world.striking_out(0, Limb::LeftArm),
                    "a fighter punched at one down on the floor, at step {step}"
                );
            }
            if world.knocked_out(1) && out_at.is_none() {
                out_at = Some(step);
            }
            if step == 200 {
                let skull = world.bones[rig.skull];
                lying = Some((hips(&world, 1), up(&world, lerp(skull.a, skull.b, 0.5))));
            }
            seen_getting_up |= world.fighter_doing(1).is_some_and(|doing| doing.getting_up);
            if out_at.is_some() && up_at.is_none() && !world.fighter_fallen(1) {
                up_at = Some(step);
            }
        }
        assert!(
            out_at.is_some_and(|step| (20..35).contains(&step)),
            "knocked out at step {out_at:?}"
        );
        let (lying_hips, lying_head) = lying.unwrap();
        assert!(
            lying_hips < 35.0 && lying_head < 30.0,
            "three seconds after a knockout the fighter's hips were {lying_hips:.0} cm up and its head {lying_head:.0}"
        );
        assert!(seen_getting_up, "the fighter was never seen getting up");
        assert!(
            up_at.is_some_and(|step| step < 720) && !world.fighter_fallen(1),
            "the knocked-out fighter stood again on step {up_at:?}, and is down now: {}",
            world.fighter_fallen(1)
        );
        assert!(
            (hips(&world, 1) - stood).abs() < 6.0,
            "standing again, its hips are {:.0} cm up, where they stood at {stood:.0}",
            hips(&world, 1)
        );
        assert!(
            (hips(&world, 0) - stood).abs() < 5.0 && !world.knocked_out(0),
            "the fighter that threw the cross had its hips {stood:.0} cm up and now {:.0}",
            hips(&world, 0)
        );
        // The fighter knocked out tears nothing lying and getting up. The
        // cross that knocked it out may split a fibre of the puncher's
        // knuckles: its hand's bone held in by the skin (`skin_hold.rs`),
        // the knuckles' skin is pressed between the bone and the head it
        // lands on, here stretched 69% against the 68% it tears at.
        let knocked_out = world.body_state(1).map_or(0, |state| state.skin_tears);
        let puncher = world.body_state(0).map_or(0, |state| state.skin_tears);
        assert_eq!(
            (
                knocked_out,
                world.stats.broken_muscle,
                world.stats.broken_bone_joints,
                world.stats.bone_joint_subluxations,
                world.stats.fractured_bones
            ),
            (0, 0, 0, 0, 0),
            "the knocked-out fighter's torn skin, muscle, joints; joints slipped; bones broken"
        );
        assert!(
            puncher <= 1,
            "the cross tore {puncher} fibres of the puncher's skin"
        );
    }
}
