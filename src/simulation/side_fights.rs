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
    /// The steps on which bone of it showed at least [`BONE_SHOWS_PX`] out
    /// of the flesh drawn over it (`inspect::bare_bones`) through whole
    /// skin, away from the window's sides, and the farthest out it showed
    /// so, in pixels; the steps it showed only in a wound, beside skin torn
    /// or cut ([`bone_in_a_wound`]), as an open injury shows it; and the
    /// steps it showed near the sides, which only random fights reach: the
    /// game's ring stops well short of them. An open break's pieces, which
    /// come out through the skin they cut, are not counted.
    pub bone_shown: [u32; 2],
    pub bone_shown_px: [f64; 2],
    pub bone_in_wounds: [u32; 2],
    pub bone_shown_at_sides: [u32; 2],
    /// Every move made: the step, the fighter, and the move.
    pub moves: Vec<(usize, usize, Move)>,
}

/// How far out of the flesh drawn over it, in pixels (about a centimetre),
/// bone counts as showing in a fight ([`SideFight::bone_shown`]).
pub const BONE_SHOWS_PX: f64 = 3.0;

/// How near the window's sides, in pixels, bone showing counts as at the
/// sides ([`SideFight::bone_shown_at_sides`]).
const SIDES_PX: f64 = 100.0;

/// Whether bone `bone`, bare at `at`, shows in a wound: beside a fibre of
/// its own flesh's skin torn or cut, or a triangle of that skin that has
/// failed, within two point spacings.
pub fn bone_in_a_wound(world: &World, bone: &BoneSegment, at: Vec2) -> bool {
    let reach = 2.0 * world.materials.point_spacing;
    let own = |point: usize| {
        let point = &world.points[point];
        point.body == bone.body && point.sheet == bone.sheet
    };
    let near = |a: usize, b: usize| {
        distance_to_segment(at, world.points[a].position, world.points[b].position) < reach
    };
    world.springs.iter().any(|spring| {
        spring.broken
            && spring.layer == TissueLayer::Skin
            && own(spring.a)
            && near(spring.a, spring.b)
    }) || world.triangles.iter().any(|triangle| {
        triangle.layer == TissueLayer::Skin
            && own(triangle.a)
            && !world.triangle_alive(triangle)
            && (near(triangle.a, triangle.b)
                || near(triangle.b, triangle.c)
                || near(triangle.c, triangle.a))
    })
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
        // Through whole skin, in a wound, and near the sides.
        let mut shown = [[false; 3]; 2];
        for bare in crate::inspect::bare_bones(&world) {
            let bone = &world.bones[bare.bone];
            let body = bone.body as usize;
            if bare.out < BONE_SHOWS_PX || bone.open_break || body > 1 {
                continue;
            }
            let width = crate::scenarios::SCENARIO_WIDTH;
            if bare.farthest.x < SIDES_PX || bare.farthest.x > width - SIDES_PX {
                shown[body][2] = true;
            } else if bone_in_a_wound(&world, bone, bare.farthest) {
                shown[body][1] = true;
            } else {
                shown[body][0] = true;
                fight.bone_shown_px[body] = fight.bone_shown_px[body].max(bare.out);
            }
        }
        for (body, [whole, wound, at_sides]) in shown.into_iter().enumerate() {
            fight.bone_shown[body] += u32::from(whole);
            fight.bone_in_wounds[body] += u32::from(wound && !whole);
            fight.bone_shown_at_sides[body] += u32::from(at_sides && !whole && !wound);
        }
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

#[cfg(test)]
mod bare_tests {
    use super::*;
    use crate::inspect::{bare_bones, bone_name, BareBone};

    /// One stretch of frames a bone showed through its flesh.
    struct Episode {
        from: usize,
        to: usize,
        out: f64,
        bare: f64,
        /// Where the bone lay farthest out.
        at: Vec2,
        /// It showed in a wound at some step (`bone_in_a_wound`).
        wound: bool,
        /// Shown on a step's own frame, not only between two.
        on_step: bool,
        doing: String,
    }

    fn doing_text(world: &World, body: usize) -> String {
        let Some(doing) = world.fighter_doing(body) else {
            return String::new();
        };
        let mut text = Vec::new();
        if let Some(punch) = doing.punch {
            text.push(format!(
                "{:?}{} age {:.3}s reach {:.2}{}",
                punch.kind,
                if punch.to_body { " to body" } else { "" },
                punch.age,
                punch.reach,
                if punch.landed { " landed" } else { "" }
            ));
        }
        if doing.step.is_some() {
            text.push("stepping".into());
        }
        if doing.lean > 0.1 {
            text.push(format!("lean {:.1}", doing.lean));
        }
        if doing.duck > 0.1 {
            text.push(format!("duck {:.1}", doing.duck));
        }
        if doing.block > 0.1 {
            text.push(format!("block {:.1}", doing.block));
        }
        if doing.fallen {
            text.push("down".into());
        }
        if doing.getting_up {
            text.push("getting up".into());
        }
        text.join(", ")
    }

    /// The flesh tied to `bone`, nearest its far end first, at most six
    /// points.
    fn tied_flesh(world: &World, bone: usize) -> Vec<usize> {
        let end = world.bones[bone].b;
        let mut tied: Vec<(f64, usize)> = world
            .bone_attachments()
            .iter()
            .filter(|tie| tie.bone == bone && !tie.broken)
            .map(|tie| (distance(world.points()[tie.point].position, end), tie.point))
            .collect();
        tied.sort_by(|a, b| a.0.total_cmp(&b.0));
        tied.dedup_by_key(|(_, point)| *point);
        tied.into_iter().take(6).map(|(_, point)| point).collect()
    }

    /// What each phase did over the last step to `bone` and, on average, to
    /// the flesh tied to it.
    fn print_traced(world: &World, step: usize, bone: usize, flesh: &[usize]) {
        let segment = world.bones[bone];
        let held: Vec<Vec2> = flesh
            .iter()
            .map(|&point| world.points()[point].position)
            .collect();
        let middle = lerp(segment.a, segment.b, 0.5);
        let flesh_middle = held.iter().fold(Vec2::default(), |sum, &at| add(sum, at));
        let flesh_middle = scale(flesh_middle, 1.0 / held.len().max(1) as f64);
        println!(
            "  step {step}: bone {bone} middle ({:.1}, {:.1}), its flesh's middle ({:.1}, {:.1}), {}",
            middle.x,
            middle.y,
            flesh_middle.x,
            flesh_middle.y,
            doing_text(world, segment.body as usize)
        );
        let mut phases: Vec<(&'static str, Vec2, Vec2, f64)> = Vec::new();
        for stage in world.traced_stages() {
            let slot = match phases.iter().position(|(phase, ..)| *phase == stage.phase) {
                Some(slot) => slot,
                None => {
                    phases.push((stage.phase, Vec2::default(), Vec2::default(), 0.0));
                    phases.len() - 1
                }
            };
            if stage.target == 0 {
                phases[slot].1 = add(phases[slot].1, stage.moved);
                phases[slot].3 += stage.turned;
            } else {
                phases[slot].2 = add(
                    phases[slot].2,
                    scale(stage.moved, 1.0 / flesh.len().max(1) as f64),
                );
            }
        }
        for (phase, bone_moved, flesh_moved, turned) in phases {
            if hypot(bone_moved.x, bone_moved.y) > 0.5
                || hypot(flesh_moved.x, flesh_moved.y) > 0.5
                || turned.abs() > 0.02
            {
                println!(
                    "    {phase:>20}: bone ({:+.1}, {:+.1}) px, turned {:+.3} rad; flesh ({:+.1}, {:+.1}) px",
                    bone_moved.x, bone_moved.y, turned, flesh_moved.x, flesh_moved.y
                );
            }
        }
    }

    /// Prints where bone shows through the flesh in random side-view fights,
    /// on each step's frame and halfway between steps as the game draws it
    /// (`cargo test --release --lib print_bare_bones -- --ignored
    /// --nocapture`; `RP_FIGHTS`, `RP_SEED`, `RP_SECONDS`, `RP_BARE_OUT` the
    /// least distance out of the flesh to print, in pixels).
    #[test]
    #[ignore]
    fn print_bare_bones() {
        let fights: u64 = std::env::var("RP_FIGHTS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(6);
        let only: Option<u64> = std::env::var("RP_SEED")
            .ok()
            .and_then(|value| value.parse().ok());
        let seconds: f64 = std::env::var("RP_SECONDS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(60.0);
        let least: f64 = std::env::var("RP_BARE_OUT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1.5);
        let trace_bone: Option<usize> = std::env::var("RP_BARE_BONE")
            .ok()
            .and_then(|value| value.parse().ok());
        let trace_steps = std::env::var("RP_BARE_TRACE")
            .ok()
            .and_then(|value| {
                let (from, to) = value.split_once('-')?;
                Some(from.parse::<usize>().ok()?..=to.parse::<usize>().ok()?)
            })
            .unwrap_or(1..=0);
        let rest = create_side_fighters(Materials::default());
        for found in bare_bones(&rest) {
            println!(
                "at rest: {} bare {:.1} px, out {:.1} px",
                bone_name(&rest, found.bone),
                found.bare,
                found.out
            );
        }
        for seed in 1..=fights {
            if only.is_some_and(|only| only != seed) {
                continue;
            }
            // `RP_SET=name=value,...` changes settings as `--set` does, from
            // the start or, with `RP_SET_AT=STEP`, from that step on.
            let settings: Vec<(String, String)> = std::env::var("RP_SET")
                .ok()
                .map(|items| crate::parse_settings(&[items]).expect("RP_SET takes name=value"))
                .unwrap_or_default();
            let set_at: usize = std::env::var("RP_SET_AT")
                .ok()
                .and_then(|step| step.parse().ok())
                .unwrap_or(0);
            let mut materials = Materials::default();
            if set_at == 0 {
                crate::apply_settings(&mut materials, &settings).expect("RP_SET's settings");
            }
            let mut world = create_side_fighters(materials);
            let dt = world.materials.fixed_dt;
            let mut moves = RandomMoves::new(seed);
            // `RP_MOVES=0:step=in,40:duck=on,60:jab` plays those moves on
            // those steps instead of random ones, each written as a gesture
            // writes it (`scenarios::fighter_move`).
            let script: Option<Vec<(usize, usize, Move)>> =
                std::env::var("RP_MOVES").ok().map(|script| {
                    script
                        .split(',')
                        .filter_map(|part| {
                            let (step, word) = part.split_once(':')?;
                            match crate::scenarios::fighter_move(word)? {
                                crate::scenarios::GestureStep::Act(body, made) => {
                                    Some((step.parse().ok()?, body, made))
                                }
                                _ => None,
                            }
                        })
                        .collect()
                });
            let mut earlier = MotionSnapshot::default();
            let mut actual = MotionSnapshot::default();
            let mut open: std::collections::BTreeMap<usize, Episode> = Default::default();
            let mut episodes: Vec<(usize, Episode)> = Vec::new();
            let mut breaks: Vec<(usize, usize, String)> = Vec::new();
            let mut fractured = world.stats.fractured_bones;
            let steps = (seconds / dt) as usize;
            for step in 1..=steps {
                if set_at > 0 && step == set_at {
                    for (name, value) in &settings {
                        world.set_setting(name, value).expect("RP_SET's settings");
                    }
                }
                let mut input = InputState::default();
                if let Some(script) = &script {
                    for (slot, (_, body, made)) in
                        script.iter().filter(|made| made.0 == step).enumerate()
                    {
                        input.moves[slot] = Some((*body, *made));
                    }
                } else {
                    for (body, made) in moves.next_step().into_iter().enumerate() {
                        if let Some(made) = made {
                            input.moves[body] = Some((body, made));
                        }
                    }
                }
                world.capture_motion(&mut earlier);
                // `RP_BARE_TRACE=FROM-TO` with `RP_BARE_BONE=N` follows bone N
                // and the flesh tied to it through those steps, phase by
                // phase.
                let traced = trace_bone.filter(|_| trace_steps.contains(&step));
                let flesh: Vec<usize> = traced.map_or(Vec::new(), |bone| tied_flesh(&world, bone));
                if let Some(bone) = traced {
                    let mut targets = vec![Traced::Bone(bone)];
                    targets.extend(flesh.iter().map(|&point| Traced::Point(point)));
                    world.trace(&targets);
                }
                world.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
                if let Some(bone) = traced {
                    print_traced(&world, step, bone, &flesh);
                    world.trace(&[]);
                }
                if world.stats.fractured_bones > fractured {
                    fractured = world.stats.fractured_bones;
                    for (index, bone) in world.bones.iter().enumerate() {
                        if (bone.fractured || bone.splinter)
                            && !breaks.iter().any(|(_, known, _)| *known == index)
                        {
                            breaks.push((step, index, bone_name(&world, index)));
                        }
                    }
                }
                let on_step = bare_bones(&world);
                world.blend_motion(&earlier, 0.5, &mut actual);
                let between = bare_bones(&world);
                world.restore_motion(&actual);
                let mut seen: Vec<(BareBone, bool)> = Vec::new();
                for found in on_step.iter().filter(|found| found.out >= least) {
                    seen.push((*found, true));
                }
                for found in between.iter().filter(|found| found.out >= least) {
                    if let Some(known) = seen.iter_mut().find(|(known, _)| known.bone == found.bone)
                    {
                        known.0.out = known.0.out.max(found.out);
                    } else {
                        seen.push((*found, false));
                    }
                }
                for (found, shown_on_step) in &seen {
                    let body = world.bones[found.bone].body as usize;
                    let episode = open.entry(found.bone).or_insert_with(|| Episode {
                        from: step,
                        to: step,
                        out: 0.0,
                        bare: 0.0,
                        at: found.farthest,
                        wound: false,
                        on_step: false,
                        doing: doing_text(&world, body),
                    });
                    episode.to = step;
                    episode.wound |=
                        bone_in_a_wound(&world, &world.bones[found.bone], found.farthest);
                    if found.out > episode.out {
                        episode.at = found.farthest;
                    }
                    episode.out = episode.out.max(found.out);
                    episode.bare = episode.bare.max(found.bare);
                    episode.on_step |= *shown_on_step;
                }
                let closed: Vec<usize> = open
                    .keys()
                    .copied()
                    .filter(|bone| !seen.iter().any(|(found, _)| found.bone == *bone))
                    .collect();
                for bone in closed {
                    let episode = open.remove(&bone).expect("open");
                    episodes.push((bone, episode));
                }
            }
            for (bone, episode) in open {
                episodes.push((bone, episode));
            }
            episodes.sort_by_key(|(_, episode)| episode.from);
            println!(
                "seed {seed}: {} episodes of bone showing {least} px or more out, bones broken {}, world digest {:016x} after {steps} steps",
                episodes.len(),
                world.stats.fractured_bones,
                crate::inspect::digest(&world)
            );
            for (step, bone, name) in &breaks {
                println!("  step {step}: broke, piece {bone} {name}");
            }
            for (bone, episode) in &episodes {
                let segment = &world.bones[*bone];
                println!(
                    "  steps {}-{} ({} frames){}{}: fighter {} {} [{}{}], out {:.1} px at ({:.0}, {:.0}), bare {:.1} px; {}",
                    episode.from,
                    episode.to,
                    episode.to - episode.from + 1,
                    if episode.on_step { "" } else { " between steps only" },
                    if episode.wound { " in a wound" } else { "" },
                    segment.body,
                    bone_name(&world, *bone),
                    bone,
                    if segment.fractured || segment.splinter {
                        ", broken"
                    } else {
                        ""
                    },
                    episode.out,
                    episode.at.x,
                    episode.at.y,
                    episode.bare,
                    episode.doing
                );
            }
        }
    }
}
