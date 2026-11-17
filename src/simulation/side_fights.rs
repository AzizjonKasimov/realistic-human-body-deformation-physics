//! Random fights between two side-view fighters: both make a move about four
//! times a second, drawn at random from a jab, a cross, a step in or out, and
//! a lean back, a duck, or a block held or let go. The same seed plays the
//! same fight step for step, in the tests (`print_random_fights`) and in the
//! scenario tool, which plays many on all cores and sets two builds' fights
//! side by side, fight against fight (`strike_scenarios --side-fights`,
//! `--diff`).

use super::*;

/// The moves of a random side-view fight, drawn a step at a time from a
/// seed (xorshift64).
#[derive(Clone, Debug)]
pub struct RandomMoves {
    state: u64,
    /// What each fighter holds: a lean back, a duck, and a block.
    held: [[bool; 3]; 2],
}

impl RandomMoves {
    pub fn new(seed: u64) -> RandomMoves {
        RandomMoves {
            state: seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1,
            held: [[false; 3]; 2],
        }
    }

    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    /// The move each fighter makes on the next step, if any, the left's
    /// first: one about four times a second, a punch two times in five.
    pub fn next_step(&mut self) -> [Option<Move>; 2] {
        let mut made = [None; 2];
        for (body, slot) in made.iter_mut().enumerate() {
            if self.next() % 15 != 0 {
                continue;
            }
            let pick = self.next() % 10;
            let held = &mut self.held[body];
            *slot = Some(match pick {
                0 | 1 => Move::Jab,
                2 | 3 => Move::Cross,
                4 => Move::StepIn,
                5 => Move::StepOut,
                6 => {
                    held[0] = !held[0];
                    Move::LeanBack(held[0])
                }
                7 => {
                    held[1] = !held[1];
                    Move::Duck(held[1])
                }
                _ => {
                    held[2] = !held[2];
                    Move::Block(held[2])
                }
            });
        }
        made
    }
}

/// What a random side-view fight did, fighter by fighter, the left's first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SideFight {
    pub seed: u64,
    /// The joints torn apart and put out of place, by their places in
    /// [`World::bone_joints`].
    pub torn_joints: [Vec<usize>; 2],
    pub slipped_joints: [Vec<usize>; 2],
    /// Bones broken, and fibres of skin torn or cut.
    pub bones_broken: [i32; 2],
    pub skin_torn: [i32; 2],
    /// Fibres of muscle torn, both fighters' together: the world counts
    /// them so.
    pub muscle_torn: i32,
    /// The step a blow first knocked it out on, if one did, and whether it
    /// ever fell with no knockout: its weight ran past where any step could
    /// catch it, or its hips sank to the floor.
    pub knocked_out: [Option<usize>; 2],
    pub fell: [bool; 2],
    /// How many times it went down, knocked out or fallen, how many times it
    /// got up again (`rise.rs`), and the longest it was down, in seconds,
    /// counted to the fight's end for one still down.
    pub downs: [u32; 2],
    pub got_up: [u32; 2],
    pub longest_down: [f64; 2],
    /// The farthest its head got from where its moves put it, in metres.
    pub head_off: [f64; 2],
    /// The fastest an end of any bone moved over a step, in metres a second.
    pub fastest: f64,
    /// Every move made: the step, the fighter, and the move.
    pub moves: Vec<(usize, usize, Move)>,
}

/// Plays random fight `seed` for `seconds` between two side-view fighters
/// built on `materials`, and says what it did.
pub fn play_side_fight(seed: u64, seconds: f64, materials: Materials) -> SideFight {
    let mut world = create_side_fighters(materials);
    let dt = world.materials.fixed_dt;
    let mut moves = RandomMoves::new(seed);
    let mut fight = SideFight {
        seed,
        ..SideFight::default()
    };
    let steps = (seconds / dt) as usize;
    let metres = world.mm_per_px() * 0.001;
    let mut down_since: [Option<usize>; 2] = [None; 2];
    for step in 0..steps {
        let mut input = InputState::default();
        for (body, made) in moves.next_step().into_iter().enumerate() {
            if let Some(made) = made {
                input.moves[body] = Some((body, made));
                fight.moves.push((step + 1, body, made));
            }
        }
        world.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
        let fastest = world
            .bones
            .iter()
            .map(|bone| distance(bone.a, bone.previous_a).max(distance(bone.b, bone.previous_b)))
            .fold(0.0, f64::max)
            / dt
            * metres;
        fight.fastest = fight.fastest.max(fastest);
        for body in 0..2 {
            let off = world.head_off_pose(body).unwrap_or(0.0);
            fight.head_off[body] = fight.head_off[body].max(off);
            if fight.knocked_out[body].is_none() && world.knocked_out(body) {
                fight.knocked_out[body] = Some(step + 1);
            }
            match (down_since[body], world.fighter_fallen(body)) {
                (None, true) => {
                    down_since[body] = Some(step);
                    fight.downs[body] += 1;
                    if !world.knocked_out(body) {
                        fight.fell[body] = true;
                    }
                }
                (Some(since), false) => {
                    down_since[body] = None;
                    fight.got_up[body] += 1;
                    fight.longest_down[body] =
                        fight.longest_down[body].max((step - since) as f64 * dt);
                }
                _ => {}
            }
        }
    }
    for body in 0..2 {
        if let Some(since) = down_since[body] {
            fight.longest_down[body] = fight.longest_down[body].max((steps - since) as f64 * dt);
        }
    }
    for (index, joint) in world.bone_joints.iter().enumerate() {
        let Some(bone) = world.bones.get(joint.a) else {
            continue;
        };
        let body = bone.body as usize;
        if body > 1 || joint.fracture_link {
            continue;
        }
        if joint.broken {
            fight.torn_joints[body].push(index);
        } else if joint.subluxated {
            fight.slipped_joints[body].push(index);
        }
    }
    for body in 0..2 {
        if let Some(state) = world.body_state(body) {
            fight.bones_broken[body] = state.fractures.iter().sum();
            fight.skin_torn[body] = state.skin_tears;
        }
    }
    fight.muscle_torn = world.stats.broken_muscle;
    fight
}
