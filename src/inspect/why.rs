//! The why: what moved a point, a bone, a fiber, a joint, or a triangle over
//! some frames of a play, told in lines a person reads, as each phase's
//! share beyond what the same phase does to the body at rest.

use super::blame::{blame, blame_by_step, Blame, BlameTarget, PhasePush};
use super::names::{place_name, subject_name};
use crate::scenarios::{Scenario, SCENARIO_HEIGHT, SCENARIO_WIDTH};
use crate::simulation::{
    body_frame, create_layered_body, InputState, Subject, TracedStage, Vec2, World,
};

/// A question: what moved `target` over frames `from` to `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Why {
    pub target: BlameTarget,
    pub from: i32,
    pub to: i32,
}

impl Why {
    /// Reads `point:N`, `bone:N`, `spring:N`, `joint:N`, or `triangle:N`,
    /// then optionally `:FROM-TO` frames.
    pub fn parse(spec: &str) -> Result<Why, String> {
        let wrong = || {
            format!(
                "expected point:N, bone:N, spring:N, joint:N, or triangle:N, then :FROM-TO, got `{spec}`"
            )
        };
        let mut parts = spec.splitn(3, ':');
        let kind = parts.next().ok_or_else(wrong)?;
        let index = parts.next().ok_or_else(wrong)?;
        let target = BlameTarget::parse(&format!("{kind}:{index}"))?;
        let (from, to) = match parts.next() {
            Some(range) => {
                let (from, to) = range.split_once('-').ok_or_else(wrong)?;
                (
                    from.parse().map_err(|_| wrong())?,
                    to.parse().map_err(|_| wrong())?,
                )
            }
            None => (0, i32::MAX),
        };
        Ok(Why { target, from, to })
    }
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

fn len(v: Vec2) -> f64 {
    libm::hypot(v.x, v.y)
}

fn unit(v: Vec2) -> Vec2 {
    let length = len(v);
    if length > 1.0e-9 {
        Vec2 {
            x: v.x / length,
            y: v.y / length,
        }
    } else {
        Vec2 { x: 0.0, y: 1.0 }
    }
}

fn dot(a: Vec2, b: Vec2) -> f64 {
    a.x * b.x + a.y * b.y
}

/// Where the thing `target` names is now, as one place or several: a
/// point's or a bone's middle, a fiber's or a joint's two ends, a
/// triangle's three corners.
fn places(world: &World, target: BlameTarget) -> Option<Vec<Vec2>> {
    let bone_middle = |index: usize| {
        world.bones().get(index).map(|bone| Vec2 {
            x: (bone.a.x + bone.b.x) * 0.5,
            y: (bone.a.y + bone.b.y) * 0.5,
        })
    };
    match target {
        BlameTarget::Point(index) => world.points().get(index).map(|point| vec![point.position]),
        BlameTarget::Bone(index) => bone_middle(index).map(|middle| vec![middle]),
        BlameTarget::Spring(index) => world.springs().get(index).map(|spring| {
            vec![
                world.points()[spring.a].position,
                world.points()[spring.b].position,
            ]
        }),
        BlameTarget::Joint(index) => world
            .bone_joints()
            .get(index)
            .and_then(|joint| Some(vec![bone_middle(joint.a)?, bone_middle(joint.b)?])),
        BlameTarget::Triangle(index) => world.triangles().get(index).map(|triangle| {
            [triangle.a, triangle.b, triangle.c]
                .map(|corner| world.points()[corner].position)
                .to_vec()
        }),
    }
}

/// The thing `target` names, by number and by name.
pub fn target_name(world: &World, target: BlameTarget) -> String {
    let (what, index, subject) = match target {
        BlameTarget::Point(index) => ("point", index, Subject::Point(index)),
        BlameTarget::Bone(index) => ("bone", index, Subject::Bone(index)),
        BlameTarget::Spring(index) => ("fiber", index, Subject::Spring(index)),
        BlameTarget::Joint(index) => ("joint", index, Subject::Joint(index)),
        BlameTarget::Triangle(index) => ("triangle", index, Subject::Triangle(index)),
    };
    format!("{what} {index} ({})", subject_name(world, subject))
}

/// What following a thing through a play found: the frame it was first
/// followed on, what each phase of each step did to it from then, where it
/// was then (and a joint's turn), and the world the play ended with.
struct Followed {
    from: i32,
    steps: Vec<(i32, Vec<TracedStage>)>,
    at_start: Vec<Vec2>,
    turn_start: Option<f64>,
    world: World,
}

/// Plays `scenario` in its window up to `why`'s last frame, following what
/// it names from its first frame, or from the frame it first exists on, as
/// a piece a break makes; `idle`, with the tool kept out of play, as the
/// body would be at rest.
fn follow(scenario: &Scenario, why: Why, idle: bool) -> Option<Followed> {
    let mut world = create_layered_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, scenario.materials());
    let dt = world.materials().fixed_dt;
    let body = body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT);
    let last = why.to.min(scenario.frames() - 1);
    let mut steps = Vec::new();
    let mut started: Option<(i32, Vec<Vec2>, Option<f64>)> = None;
    for frame in 0..=last {
        if started.is_none() && frame >= why.from {
            let traced = why.target.traced(&world);
            if let (false, Some(at)) = (traced.is_empty(), places(&world, why.target)) {
                world.trace(&traced);
                let turn = match why.target {
                    BlameTarget::Joint(index) => world.joint_turn(index),
                    _ => None,
                };
                started = Some((frame, at, turn));
            }
        }
        let input = if idle {
            InputState::default()
        } else {
            scenario.input(frame, dt, body)
        };
        world.step(dt, &input, SCENARIO_WIDTH);
        if started.is_some() {
            steps.push((frame, world.traced_stages().to_vec()));
        }
    }
    let (from, at_start, turn_start) = started?;
    Some(Followed {
        from,
        steps,
        at_start,
        turn_start,
        world,
    })
}

/// "12.0 left, 30.1 up": a motion on screen in words.
fn direction_words(v: Vec2) -> String {
    let across = if v.x < 0.0 { "left" } else { "right" };
    let down = if v.y < 0.0 { "up" } else { "down" };
    format!("{:.1} {across}, {:.1} {down}", v.x.abs(), v.y.abs())
}

/// What a phase does, for the phases whose names say little.
fn phase_note(phase: &str) -> &'static str {
    match phase {
        "integrate" => " (gravity, drag)",
        "outline hold" => " (the skin outline's hold, which balances its pull at rest)",
        "ghosts" => " (a cut's ghost points)",
        "predict" => " (its own momentum)",
        "fibers" => " (the tissue's springs)",
        "fat" => " (skin held to muscle)",
        "muscle ties" => " (muscle held to its bone)",
        "areas" => " (triangles keeping their area)",
        "posture" => " (flesh held to its place on the bones)",
        "hip hold" => " (the hips held where they stood, as legs would)",
        "joint muscles" => " (the muscles across the joints)",
        "joints" => " (joints holding their bones)",
        "tendons" => " (the muscles and tendons across the joints)",
        "warm start: fibers" => " (the pull the springs carried in)",
        "warm start: areas" => " (the push the triangles carried in)",
        "warm start: fat" => " (the pull skin and muscle carried in)",
        "warm start: muscle ties" => " (the pull muscle and bone carried in)",
        "warm start: posture" => " (the pull to its place carried in)",
        "warm start: hip hold" => " (the hold on the hips carried in)",
        "warm start: joint muscles" => " (the pull the joint muscles carried in)",
        "warm start: joints" => " (the pull the joints carried in)",
        "warm start: tendons" => " (the pull the tendons carried in)",
        "bones" => " (bone lengths, breaks)",
        "world bounds" => " (floor and window)",
        _ => "",
    }
}

/// Plays `scenario` following what `why` names over its frames, and tells
/// what moved it: each phase's share beyond what it does to the body at
/// rest, and the frames that moved it most.
pub fn explain(scenario: &Scenario, why: Why) -> Vec<String> {
    let dt = scenario.materials().fixed_dt;
    let Some(run) = follow(scenario, why, false) else {
        return vec![format!(
            "{}: nothing to follow; {:?} never existed in frames {}-{}",
            scenario.name, why.target, why.from, why.to
        )];
    };
    // The same frames with nothing touching the body, when the thing was
    // there from the start: forces that only hold the body at rest, such as
    // the skin outline's hold against the skin's pull, cancel out of the
    // shares, leaving what the play changed.
    let rest = follow(scenario, why, true).filter(|rest| rest.from == run.from);
    let blame_of = |target: usize| {
        let played = blame(&run.steps, target, dt);
        match &rest {
            Some(rest) => blame(&rest.steps, target, dt).against(&played),
            None => played,
        }
    };
    let world = &run.world;
    let last = run.steps.last().map_or(run.from, |(frame, _)| *frame);
    let Some(at_end) = places(world, why.target) else {
        return Vec::new();
    };
    let at_start = &run.at_start;
    let mut lines = vec![format!(
        "{}: why {} moved over frames {}-{last}{}:",
        scenario.name,
        target_name(world, why.target),
        run.from,
        if rest.is_some() {
            ", beyond what holds it at rest"
        } else {
            ""
        }
    )];
    match why.target {
        BlameTarget::Point(_) | BlameTarget::Bone(_) => {
            let moved = sub(at_end[0], at_start[0]);
            let blame = blame_of(0);
            lines.push(format!(
                "  it moved {:.1} px ({}) and turned {:+.0} degrees, from {}",
                len(moved),
                direction_words(moved),
                blame.total_turn().to_degrees(),
                place_name(world, at_start[0])
            ));
            lines.extend(shares(
                &blame,
                moved,
                matches!(why.target, BlameTarget::Bone(_)),
            ));
            let rest_steps = rest.as_ref().map(|rest| rest.steps.as_slice());
            lines.extend(busiest_steps(&run.steps, rest_steps, 0, dt, moved));
        }
        BlameTarget::Spring(_) => {
            let apart = blame_of(0).against(&blame_of(1));
            let before = len(sub(at_start[1], at_start[0]));
            let after = len(sub(at_end[1], at_end[0]));
            lines.push(format!(
                "  its ends went from {before:.1} to {after:.1} px apart ({:+.1} px); shares of the stretch:",
                after - before
            ));
            lines.extend(shares_along(&apart, unit(sub(at_end[1], at_end[0]))));
        }
        BlameTarget::Joint(index) => {
            let bent = blame_of(0).against(&blame_of(1));
            lines.push(format!(
                "  it bent from {:+.1} to {:+.1} degrees off its guard; shares of the bend:",
                run.turn_start.unwrap_or(0.0).to_degrees(),
                world.joint_turn(index).unwrap_or(0.0).to_degrees()
            ));
            let mut turns: Vec<(&str, f64)> = bent
                .phases
                .iter()
                .map(|phase| (phase.phase, phase.turn))
                .collect();
            turns.push(("(the turn it already had)", bent.carried_turn));
            turns.sort_by(|p, q| q.1.abs().total_cmp(&p.1.abs()));
            for (phase, turn) in turns.iter().filter(|(_, turn)| turn.abs() >= 0.002) {
                lines.push(format!(
                    "    {phase:<22} {:+7.1} degrees",
                    turn.to_degrees()
                ));
            }
        }
        BlameTarget::Triangle(_) => {
            // The corner whose height over the side across from it fell
            // most: the one that crossed it, if the triangle turned over.
            let height = |corners: &[Vec2], corner: usize| {
                let (p, q) = (corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
                let side = unit(sub(q, p));
                let inward = Vec2 {
                    x: -side.y,
                    y: side.x,
                };
                (dot(sub(corners[corner], p), inward), inward)
            };
            let corner = (0..3)
                .min_by(|&p, &q| {
                    let fall = |c: usize| height(&at_end, c).0 - height(at_start, c).0;
                    fall(p).total_cmp(&fall(q))
                })
                .unwrap_or(0);
            let (before, _) = height(at_start, corner);
            let (after, inward) = height(&at_end, corner);
            lines.push(format!(
                "  its corner {corner} went from {before:+.1} to {after:+.1} px over the side across from it; shares of that:"
            ));
            // The corner against the middle of the side across from it.
            let blames = [blame_of(0), blame_of(1), blame_of(2)];
            let middle = blames[(corner + 1) % 3]
                .plus(&blames[(corner + 2) % 3])
                .times(0.5);
            lines.extend(shares_along(&middle.against(&blames[corner]), inward));
        }
    }
    lines
}

/// Each phase's share of a motion `moved`, the largest first: how far it
/// pushed along the way the thing went and across it.
fn shares(blame: &Blame, moved: Vec2, turns: bool) -> Vec<String> {
    let along = unit(moved);
    let across = Vec2 {
        x: -along.y,
        y: along.x,
    };
    let mut shares: Vec<PhasePush> = blame.phases.clone();
    shares.push(PhasePush {
        phase: "(what it carried in)",
        push: blame.carried,
        turn: blame.carried_turn,
    });
    shares.sort_by(|p, q| len(q.push).total_cmp(&len(p.push)));
    let mut lines =
        vec!["  shares, largest first (px along the way it went / across it):".to_string()];
    for share in shares
        .iter()
        .filter(|share| len(share.push) >= 0.05 || share.turn.abs() >= 0.005)
    {
        let turn = if turns {
            format!("  turned {:+.0} degrees", share.turn.to_degrees())
        } else {
            String::new()
        };
        lines.push(format!(
            "    {:<22} {:+7.1} / {:+6.1}{turn}{}",
            share.phase,
            dot(share.push, along),
            dot(share.push, across),
            phase_note(share.phase)
        ));
    }
    lines.push(format!(
        "  together {:.1} px of the {:.1} it moved",
        dot(blame.total(), along),
        len(moved)
    ));
    let pushed: f64 = shares.iter().map(|share| len(share.push)).sum();
    if pushed > 5.0 * len(moved).max(1.0) {
        lines.push(
            "  most of these cancel: forces that hold it in balance, such as the outline's hold against the skin's springs"
                .to_string(),
        );
    }
    lines
}

/// Each phase's share of how far something moved along `axis`.
fn shares_along(blame: &Blame, axis: Vec2) -> Vec<String> {
    let mut shares: Vec<(&str, f64)> = blame
        .phases
        .iter()
        .map(|phase| (phase.phase, dot(phase.push, axis)))
        .collect();
    shares.push(("(what it carried in)", dot(blame.carried, axis)));
    shares.sort_by(|p, q| q.1.abs().total_cmp(&p.1.abs()));
    shares
        .iter()
        .filter(|(_, share)| share.abs() >= 0.05)
        .map(|(phase, share)| format!("    {phase:<22} {share:+7.1} px{}", phase_note(phase)))
        .collect()
}

/// The frames whose pushes moved the thing most along `moved`, and the
/// phase that pushed most on each, beyond the same frames at rest when
/// `rest` holds them.
fn busiest_steps(
    steps: &[(i32, Vec<TracedStage>)],
    rest: Option<&[(i32, Vec<TracedStage>)]>,
    target: usize,
    dt: f64,
    moved: Vec2,
) -> Option<String> {
    let along = unit(moved);
    let at_rest = rest.map(|rest| blame_by_step(rest, target, dt));
    let mut busiest: Vec<(i32, f64, &str)> = blame_by_step(steps, target, dt)
        .into_iter()
        .enumerate()
        .zip(steps)
        .filter_map(|((index, blame), (frame, _))| {
            let blame = match at_rest.as_ref().and_then(|rest| rest.get(index)) {
                Some(rest) => rest.against(&blame),
                None => blame,
            };
            let main = blame
                .phases
                .iter()
                .max_by(|p, q| {
                    dot(p.push, along)
                        .abs()
                        .total_cmp(&dot(q.push, along).abs())
                })?
                .phase;
            Some((*frame, dot(blame.total(), along), main))
        })
        .collect();
    busiest.sort_by(|p, q| q.1.abs().total_cmp(&p.1.abs()));
    let shown: Vec<String> = busiest
        .iter()
        .take(6)
        .filter(|(_, share, _)| share.abs() >= 0.5)
        .map(|(frame, share, phase)| format!("frame {frame} {share:+.1} ({phase})"))
        .collect();
    (!shown.is_empty()).then(|| format!("  the frames that moved it most: {}", shown.join(", ")))
}
