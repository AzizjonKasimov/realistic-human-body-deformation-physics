//! Twin runs (`--twin mirrored` or `--twin across[=PX]`): one play on two
//! copies of the body, one as tuned and one mirrored left to right or moved
//! across the world, stepped side by side and compared on every frame. A rule
//! that is not the same on the body's two sides, or that reads where
//! something is in the world rather than what is happening to the body,
//! makes the twins part on the step it acts: the report names that frame
//! and, phase by phase, the part of the step where they parted and what it
//! moved apart. The moved-body tests (`moved.rs`) say whether a blow hurts
//! alike over the sweep; this says why not.
//!
//! The twins are compared by how far each point and bone has moved since
//! the play began, mirrored for the mirrored twin, since the body at rest is
//! itself a little uneven (the settle takes one side's joints before the
//! other's). A third copy with the hand nudged a millionth of a pixel shows
//! how far two runs drift apart on rounding alone.

use super::*;
use rp::{BodyFrame, Traced, TracedStage, Vec2, World};

/// How far the moved-body tests move the body across: a window 400 px
/// wider puts it this much further right.
pub(crate) const MOVED_ACROSS_PX: f64 = 400.0 * 0.52;

/// The way the twin is changed from the body as tuned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TwinWay {
    Mirrored,
    /// Moved this many pixels across the world.
    Across(f64),
}

impl TwinWay {
    /// Reads `mirrored`, `across`, or `across=PX`.
    pub(crate) fn parse(text: &str) -> Result<TwinWay, String> {
        match text.split_once('=') {
            None if text == "mirrored" => Ok(TwinWay::Mirrored),
            None if text == "across" => Ok(TwinWay::Across(MOVED_ACROSS_PX)),
            Some(("across", px)) => px
                .trim()
                .parse()
                .map(TwinWay::Across)
                .map_err(|_| format!("--twin across=PX takes a number of pixels, got `{px}`")),
            _ => Err(format!(
                "--twin takes mirrored, across, or across=PX, got `{text}`"
            )),
        }
    }

    fn describe(self) -> String {
        match self {
            TwinWay::Mirrored => "mirrored left to right".to_string(),
            TwinWay::Across(px) => format!("moved {px:.0} px across"),
        }
    }
}

/// How the twin's places and motions are turned back into the tuned body's.
#[derive(Clone, Copy)]
struct Turn {
    way: TwinWay,
    /// The tuned body's midline, which the mirror turns about.
    middle: f64,
}

impl Turn {
    /// The twin's place `at`, where the tuned body has it.
    fn place(self, at: Vec2) -> Vec2 {
        match self.way {
            TwinWay::Mirrored => Vec2 {
                x: 2.0 * self.middle - at.x,
                y: at.y,
            },
            TwinWay::Across(px) => Vec2 {
                x: at.x - px,
                y: at.y,
            },
        }
    }

    /// The twin's motion or direction `by`, as the tuned body has it.
    fn motion(self, by: Vec2) -> Vec2 {
        match self.way {
            TwinWay::Mirrored => Vec2 { x: -by.x, y: by.y },
            TwinWay::Across(_) => by,
        }
    }
}

/// Which of the twin's points and bones stands for each of the tuned
/// body's, found by where each was built; a bone's ends may swap in the
/// mirror.
struct Matches {
    points: Vec<Option<usize>>,
    bones: Vec<Option<(usize, bool)>>,
}

fn near(a: Vec2, b: Vec2) -> bool {
    (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01
}

fn matches(tuned: &World, twin: &World, turn: Turn) -> Matches {
    let points = tuned
        .points()
        .iter()
        .map(|point| {
            twin.points().iter().position(|other| {
                other.layer == point.layer && near(turn.place(other.home), point.home)
            })
        })
        .collect();
    let bones = tuned
        .bones()
        .iter()
        .map(|bone| {
            twin.bones().iter().enumerate().find_map(|(index, other)| {
                let (a, b) = (turn.place(other.home_a), turn.place(other.home_b));
                if near(a, bone.home_a) && near(b, bone.home_b) {
                    Some((index, false))
                } else if near(a, bone.home_b) && near(b, bone.home_a) {
                    Some((index, true))
                } else {
                    None
                }
            })
        })
        .collect();
    Matches { points, bones }
}

/// Where every point and bone end stood when the play began.
struct Start {
    points: Vec<Vec2>,
    bones: Vec<(Vec2, Vec2)>,
}

impl Start {
    fn of(world: &World) -> Start {
        Start {
            points: world.points().iter().map(|point| point.position).collect(),
            bones: world.bones().iter().map(|bone| (bone.a, bone.b)).collect(),
        }
    }
}

fn length(v: Vec2) -> f64 {
    libm::hypot(v.x, v.y)
}

fn minus(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

/// How far apart two runs have come: the worst point and bone, by how far
/// each moved since the play began, and the tool.
#[derive(Clone, Copy, Default)]
struct Parting {
    point: f64,
    worst_point: usize,
    bone: f64,
    worst_bone: usize,
    tool: f64,
    heading_degrees: f64,
}

impl Parting {
    fn largest(&self) -> f64 {
        self.point.max(self.bone)
    }
}

fn parting(
    tuned: &World,
    tuned_start: &Start,
    twin: &World,
    twin_start: &Start,
    found: &Matches,
    turn: Turn,
) -> Parting {
    let mut parting = Parting::default();
    for (index, matched) in found.points.iter().enumerate() {
        let Some(other) = *matched else { continue };
        let (Some(point), Some(twin_point)) = (tuned.points().get(index), twin.points().get(other))
        else {
            continue;
        };
        let moved = minus(point.position, tuned_start.points[index]);
        let twin_moved = turn.motion(minus(twin_point.position, twin_start.points[other]));
        let apart = length(minus(moved, twin_moved));
        if apart > parting.point {
            parting.point = apart;
            parting.worst_point = index;
        }
    }
    for (index, matched) in found.bones.iter().enumerate() {
        let Some((other, swapped)) = *matched else {
            continue;
        };
        let (Some(bone), Some(twin_bone)) = (tuned.bones().get(index), twin.bones().get(other))
        else {
            continue;
        };
        let (start_a, start_b) = tuned_start.bones[index];
        let (twin_a, twin_b) = twin_start.bones[other];
        let ends = if swapped {
            [(twin_bone.b, twin_b), (twin_bone.a, twin_a)]
        } else {
            [(twin_bone.a, twin_a), (twin_bone.b, twin_b)]
        };
        for ((now, start), (twin_now, twin_start)) in
            [(bone.a, start_a), (bone.b, start_b)].into_iter().zip(ends)
        {
            let apart = length(minus(
                minus(now, start),
                turn.motion(minus(twin_now, twin_start)),
            ));
            if apart > parting.bone {
                parting.bone = apart;
                parting.worst_bone = index;
            }
        }
    }
    if let (Some(pose), Some(twin_pose)) = (tuned.current_tool_pose(), twin.current_tool_pose()) {
        parting.tool = length(minus(pose.center, turn.place(twin_pose.center)));
        // The tool's heading and its handle side, whichever turned apart
        // further.
        parting.heading_degrees =
            rp::scenarios::turn_degrees(pose.heading, turn.motion(twin_pose.heading)).max(
                rp::scenarios::turn_degrees(pose.side, turn.motion(twin_pose.side)),
            );
    }
    parting
}

/// Plays each scenario beside its twin and prints where they part, and
/// phase by phase what parted them on frame `explain` or, without one, on
/// the first frame they part beyond rounding.
pub(crate) fn twin_runs(selected: &[Scenario], way: TwinWay, explain: Option<i32>) {
    for scenario in selected {
        twin_run(scenario, way, explain);
    }
}

fn twin_run(scenario: &Scenario, way: TwinWay, explain: Option<i32>) {
    let materials = scenario.materials();
    let dt = materials.fixed_dt;
    let tuned_frame = rp::body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT);
    let (twin_frame, twin_width, twin_scenario) = match way {
        TwinWay::Mirrored => (tuned_frame, SCENARIO_WIDTH, scenario.mirrored()),
        TwinWay::Across(px) => (
            BodyFrame {
                origin: Vec2 {
                    x: tuned_frame.origin.x + px,
                    y: tuned_frame.origin.y,
                },
                ..tuned_frame
            },
            SCENARIO_WIDTH + px.max(0.0) / 0.52,
            *scenario,
        ),
    };
    let turn = Turn {
        way,
        middle: tuned_frame.origin.x,
    };
    // A body standing elsewhere is the body built in the scenarios' place
    // and moved there (`World::move_across`), as a second fighter is.
    let mut tuned = settled_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, materials);
    let mut twin = tuned.clone();
    if let TwinWay::Across(px) = way {
        twin.move_across(px);
    }
    let mut nudged = tuned.clone();
    let found = matches(&tuned, &twin, turn);
    let unmatched_points = found.points.iter().filter(|m| m.is_none()).count();
    let unmatched_bones = found.bones.iter().filter(|m| m.is_none()).count();
    println!(
        "{} beside its twin {}: {} frames",
        scenario.name,
        way.describe(),
        scenario.frames()
    );
    if unmatched_points + unmatched_bones > 0 {
        println!(
            "  {unmatched_points} points and {unmatched_bones} bones have no twin; they are left out"
        );
    }
    // How differently the twins stand before anything touches them.
    let (uneven, worst) = found
        .points
        .iter()
        .enumerate()
        .filter_map(|(index, matched)| {
            let other = (*matched)?;
            Some((
                length(minus(
                    tuned.points()[index].position,
                    turn.place(twin.points()[other].position),
                )),
                index,
            ))
        })
        .fold(
            (0.0, 0),
            |most, next| if next.0 > most.0 { next } else { most },
        );
    let (bone_uneven, worst_bone) = found
        .bones
        .iter()
        .enumerate()
        .filter_map(|(index, matched)| {
            let (other, swapped) = (*matched)?;
            let (bone, twin_bone) = (tuned.bones()[index], twin.bones()[other]);
            let (a, b) = if swapped {
                (twin_bone.b, twin_bone.a)
            } else {
                (twin_bone.a, twin_bone.b)
            };
            Some((
                length(minus(bone.a, turn.place(a))).max(length(minus(bone.b, turn.place(b)))),
                index,
            ))
        })
        .fold(
            (0.0, 0),
            |most, next| if next.0 > most.0 { next } else { most },
        );
    println!(
        "  at rest the twins stand up to {uneven:.3} px apart, point for point (point {worst}, {}), and bones up to {bone_uneven:.3} px ({})",
        rp::inspect::place_name(&tuned, tuned.points()[worst].position),
        rp::inspect::bone_name(&tuned, worst_bone)
    );
    if uneven > 0.5 {
        let other = found.points[worst].expect("a matched point");
        let (ours, theirs) = (tuned.points()[worst], twin.points()[other]);
        println!(
            "    point {worst} was built at ({:.2}, {:.2}) and rests at ({:.2}, {:.2}); its twin, point {other}, rests at ({:.2}, {:.2}) turned back",
            ours.home.x,
            ours.home.y,
            ours.position.x,
            ours.position.y,
            turn.place(theirs.position).x,
            turn.place(theirs.position).y
        );
    }
    // How far apart the twins' points were built, point for matching point:
    // the figure's two halves mirror each other only as closely as its mesh.
    let built_apart = found
        .points
        .iter()
        .enumerate()
        .filter_map(|(index, matched)| {
            let other = (*matched)?;
            Some(length(minus(
                tuned.points()[index].home,
                turn.place(twin.points()[other].home),
            )))
        })
        .fold(0.0, f64::max);
    println!("  the twins were built up to {built_apart:.2e} px apart, point for point");
    for line in built_differently(&tuned, &twin, &found, turn) {
        println!("  built differently: {line}");
    }

    let (tuned_start, twin_start, nudged_start) =
        (Start::of(&tuned), Start::of(&twin), Start::of(&nudged));
    let same = Matches {
        points: (0..tuned.points().len()).map(Some).collect(),
        bones: (0..tuned.bones().len())
            .map(|index| Some((index, false)))
            .collect(),
    };
    let unturned = Turn {
        way: TwinWay::Across(0.0),
        middle: tuned_frame.origin.x,
    };
    let mut tuned_result = ScenarioResult::default();
    let mut twin_result = ScenarioResult::default();
    let mut explained = false;
    let mut parted_before = 0.0;
    let mut last_printed = Parting::default();
    println!(
        "  frame  tool px  turned deg    points px (worst)                    bones px (worst)                 rounding px"
    );
    for frame in 0..scenario.frames() {
        let before = (!explained && explain.is_none_or(|chosen| chosen == frame))
            .then(|| (tuned.clone(), twin.clone()));
        let input = scenario.input(frame, dt, tuned_frame);
        let twin_input = twin_scenario.input(frame, dt, twin_frame);
        let nudged_input = rp::InputState {
            x: input.x + 1.0e-6,
            ..input
        };
        tuned.step(dt, &input, SCENARIO_WIDTH);
        twin.step(dt, &twin_input, twin_width);
        nudged.step(dt, &nudged_input, SCENARIO_WIDTH);
        tuned_result.accumulate(&tuned, &input);
        twin_result.accumulate(&twin, &twin_input);
        let apart = parting(&tuned, &tuned_start, &twin, &twin_start, &found, turn);
        let drift = parting(
            &tuned,
            &tuned_start,
            &nudged,
            &nudged_start,
            &same,
            unturned,
        );
        let grew = apart.largest() > 1.5 * last_printed.largest() + 0.01
            || apart.heading_degrees > last_printed.heading_degrees + 1.0;
        if grew || frame % 20 == 0 || frame + 1 == scenario.frames() {
            println!(
                "  {frame:>5}  {:>7.3}  {:>11.2}   {:>9.3} {:<26} {:>9.3} {:<22} {:>9.2e}",
                apart.tool,
                apart.heading_degrees,
                apart.point,
                short(&rp::inspect::place_name(
                    &tuned,
                    tuned.points()[apart.worst_point].position
                )),
                apart.bone,
                short(&rp::inspect::bone_name(&tuned, apart.worst_bone)),
                drift.largest(),
            );
            last_printed = apart;
        }
        // Parted beyond rounding, and by a jump rather than the slow drift
        // of a body a little uneven at rest; or the frame asked for.
        let parted = apart.largest() > 0.05
            && apart.largest() > 20.0 * drift.largest()
            && apart.largest() > 3.0 * parted_before;
        let tool_parted = apart.tool > 0.05 || apart.heading_degrees > 0.5;
        parted_before = apart.largest();
        let chosen = explain == Some(frame) || (explain.is_none() && (parted || tool_parted));
        if let (true, Some((tuned_before, twin_before))) = (chosen, before) {
            explained = true;
            explain_frame(
                tuned_before,
                twin_before,
                &found,
                turn,
                (input, twin_input),
                (SCENARIO_WIDTH, twin_width),
                frame,
            );
        }
    }
    tuned_result.finish(&tuned);
    twin_result.finish(&twin);
    let counts = |r: &ScenarioResult| {
        format!(
            "bones broken {}, skin torn {}, muscle torn {}, bruises {}, arteries cut {}, blood lost {:.1} ml",
            r.bone_fractures,
            r.skin_tears,
            r.muscle_tears,
            r.contusion_events,
            r.vessel_lacerations,
            r.blood_loss * rp::scenarios::BLOOD_VOLUME_ML
        )
    };
    println!("  as tuned: {}", counts(&tuned_result));
    println!("  twin:     {}", counts(&twin_result));
    if !explained {
        println!("  the twins never parted beyond rounding");
    }
}

/// Where the twins' bodies were built with different fibers, fat, or ties,
/// point for matching point: each kind's count, and the first of the tuned
/// body's that the twin lacks.
fn built_differently(tuned: &World, twin: &World, found: &Matches, turn: Turn) -> Vec<String> {
    let point = |index: usize| found.points.get(index).copied().flatten();
    let pair = |a: usize, b: usize| -> Option<(usize, usize)> {
        let (a, b) = (point(a)?, point(b)?);
        Some((a.min(b), a.max(b)))
    };
    let mut lines = Vec::new();
    let mut compare =
        |kind: &str, ours: Vec<Option<(usize, usize)>>, theirs: Vec<(usize, usize)>| {
            let theirs: std::collections::HashSet<(usize, usize)> = theirs.into_iter().collect();
            let missing: Vec<usize> = ours
                .iter()
                .enumerate()
                .filter(|(_, mapped)| mapped.map_or(true, |key| !theirs.contains(&key)))
                .map(|(index, _)| index)
                .collect();
            if ours.len() != theirs.len() || !missing.is_empty() {
                lines.push(format!(
                    "{kind}: {} as tuned, {} in the twin; {} of the tuned body's have no twin{}",
                    ours.len(),
                    theirs.len(),
                    missing.len(),
                    missing
                        .first()
                        .map(|first| format!(" (first {kind} {first})"))
                        .unwrap_or_default()
                ));
            }
        };
    compare(
        "fiber",
        tuned
            .springs()
            .iter()
            .map(|spring| pair(spring.a, spring.b))
            .collect(),
        twin.springs()
            .iter()
            .map(|spring| (spring.a.min(spring.b), spring.a.max(spring.b)))
            .collect(),
    );
    compare(
        "fat tie",
        tuned
            .attachments()
            .iter()
            .map(|tie| pair(tie.skin_point, tie.muscle_point))
            .collect(),
        twin.attachments()
            .iter()
            .map(|tie| {
                (
                    tie.skin_point.min(tie.muscle_point),
                    tie.skin_point.max(tie.muscle_point),
                )
            })
            .collect(),
    );
    let bone = |index: usize| found.bones.get(index).copied().flatten().map(|(b, _)| b);
    compare(
        "muscle tie",
        tuned
            .bone_attachments()
            .iter()
            .map(|tie| Some((point(tie.point)?, bone(tie.bone)?)))
            .collect(),
        twin.bone_attachments()
            .iter()
            .map(|tie| (tie.point, tie.bone))
            .collect(),
    );
    // How differently the matching fibers rest, and how differently the
    // outline holds its matching points.
    let twin_spring: std::collections::HashMap<(usize, usize), usize> = twin
        .springs()
        .iter()
        .enumerate()
        .map(|(index, spring)| ((spring.a.min(spring.b), spring.a.max(spring.b)), index))
        .collect();
    let (mut rest_apart, mut stretch_apart, mut worst_rest) = (0.0f64, 0.0f64, 0);
    for (index, spring) in tuned.springs().iter().enumerate() {
        let Some(&other) = pair(spring.a, spring.b).and_then(|key| twin_spring.get(&key)) else {
            continue;
        };
        let apart = (spring.rest - twin.springs()[other].rest).abs();
        if apart > rest_apart {
            (rest_apart, worst_rest) = (apart, index);
        }
        stretch_apart = stretch_apart.max(
            (tuned.spring_wear()[index].prestretch - twin.spring_wear()[other].prestretch).abs(),
        );
    }
    if rest_apart > 1.0e-6 || stretch_apart > 1.0e-6 {
        let at = tuned.points()[tuned.springs()[worst_rest].a].position;
        lines.push(format!(
            "matching fibers rest up to {rest_apart:.2e} px apart (fiber {worst_rest}, {}) and stretch up to {stretch_apart:.2e} apart at rest",
            rp::inspect::place_name(tuned, at).trim_start_matches("the ")
        ));
    }
    let hold_apart = found
        .points
        .iter()
        .enumerate()
        .filter_map(|(index, matched)| {
            let other = (*matched)?;
            // A hold is a push, turned as any motion is.
            let (ours, theirs) = (
                tuned.point_holds()[index].wrap,
                turn.motion(twin.point_holds()[other].wrap),
            );
            Some(length(minus(ours, theirs)))
        })
        .fold(0.0, f64::max);
    if hold_apart > 1.0e-6 {
        lines.push(format!(
            "the outline holds matching points up to {hold_apart:.2e} apart"
        ));
    }
    // Which kinds of fiber, and where, have no twin.
    let theirs: std::collections::HashSet<(usize, usize)> = twin
        .springs()
        .iter()
        .map(|spring| (spring.a.min(spring.b), spring.a.max(spring.b)))
        .collect();
    let mut kinds: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for (index, spring) in tuned.springs().iter().enumerate() {
        if pair(spring.a, spring.b).is_some_and(|key| theirs.contains(&key)) {
            continue;
        }
        let at = tuned.points()[spring.a].position;
        kinds
            .entry(format!("{:?}", spring.kind))
            .or_default()
            .push(format!(
                "{index} ({})",
                rp::inspect::place_name(tuned, at).trim_start_matches("the ")
            ));
    }
    for (kind, list) in kinds {
        let shown: Vec<&String> = list.iter().take(6).collect();
        lines.push(format!(
            "  {} {kind} fibers without a twin: {}{}",
            list.len(),
            shown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            if list.len() > 6 { ", ..." } else { "" }
        ));
    }
    lines
}

/// A name cut to fit its column.
fn short(name: &str) -> String {
    name.trim_start_matches("the ").chars().take(26).collect()
}

/// Steps both twins through `frame` again, following every point and bone
/// through each phase, and prints the phases that moved them apart.
fn explain_frame(
    mut tuned: World,
    mut twin: World,
    found: &Matches,
    turn: Turn,
    (input, twin_input): (rp::InputState, rp::InputState),
    (width, twin_width): (f64, f64),
    frame: i32,
) {
    let mut tuned_traced = Vec::new();
    let mut twin_traced = Vec::new();
    // What each traced pair is: a point or bone of the tuned body.
    let mut pairs = Vec::new();
    for (index, matched) in found.points.iter().enumerate() {
        if let Some(other) = matched {
            tuned_traced.push(Traced::Point(index));
            twin_traced.push(Traced::Point(*other));
            pairs.push(Traced::Point(index));
        }
    }
    for (index, matched) in found.bones.iter().enumerate() {
        if let Some((other, _)) = matched {
            tuned_traced.push(Traced::Bone(index));
            twin_traced.push(Traced::Bone(*other));
            pairs.push(Traced::Bone(index));
        }
    }
    // Each twin also steps with nothing in play, so what holds its body at
    // rest, large pushes that cancel and need not be even on both sides,
    // can be taken out of what each phase did, as the why does.
    let (mut tuned_rest, mut twin_rest) = (tuned.clone(), twin.clone());
    tuned.trace(&tuned_traced);
    twin.trace(&twin_traced);
    tuned_rest.trace(&tuned_traced);
    twin_rest.trace(&twin_traced);
    tuned.step(tuned.materials().fixed_dt, &input, width);
    twin.step(twin.materials().fixed_dt, &twin_input, twin_width);
    let still = rp::InputState::default();
    tuned_rest.step(tuned_rest.materials().fixed_dt, &still, width);
    twin_rest.step(twin_rest.materials().fixed_dt, &still, twin_width);
    let by_call = |stages: &[TracedStage]| {
        let mut calls: Vec<(usize, &'static str, f64)> = Vec::new();
        let mut moved: std::collections::HashMap<(usize, usize), Vec2> = Default::default();
        for stage in stages {
            if !calls.iter().any(|(call, _, _)| *call == stage.call) {
                calls.push((stage.call, stage.phase, stage.time));
            }
            moved.insert((stage.call, stage.target), stage.moved);
        }
        (calls, moved)
    };
    let (mut calls, tuned_moved) = by_call(tuned.traced_stages());
    let (twin_calls, twin_moved) = by_call(twin.traced_stages());
    let (_, tuned_rest_moved) = by_call(tuned_rest.traced_stages());
    let (_, twin_rest_moved) = by_call(twin_rest.traced_stages());
    for call in twin_calls {
        if !calls.iter().any(|(c, _, _)| *c == call.0) {
            calls.push(call);
        }
    }
    calls.sort_by_key(|(call, _, _)| *call);
    println!(
        "    frame {frame}, phase by phase, where the twins moved things differently, beyond what holds each at rest:"
    );
    let mut shown = 0;
    for (call, phase, time) in calls {
        let mut worst = (0.0, 0, Vec2::default(), Vec2::default());
        for target in 0..pairs.len() {
            let share =
                |moved: &std::collections::HashMap<(usize, usize), Vec2>,
                 rest: &std::collections::HashMap<(usize, usize), Vec2>| {
                    minus(
                        moved.get(&(call, target)).copied().unwrap_or_default(),
                        rest.get(&(call, target)).copied().unwrap_or_default(),
                    )
                };
            let moved = share(&tuned_moved, &tuned_rest_moved);
            let twin_moved = turn.motion(share(&twin_moved, &twin_rest_moved));
            let apart = length(minus(moved, twin_moved));
            if apart > worst.0 {
                worst = (apart, target, moved, twin_moved);
            }
        }
        if worst.0 < 1.0e-3 {
            continue;
        }
        let what = match pairs[worst.1] {
            Traced::Point(index) => format!(
                "point {index} ({})",
                rp::inspect::place_name(&tuned, tuned.points()[index].position)
            ),
            Traced::Bone(index) => {
                format!("bone {index} ({})", rp::inspect::bone_name(&tuned, index))
            }
        };
        println!(
            "      {:<18} at {:.2} of the step: {:.3} px apart at {what}: moved ({:.3}, {:.3}) as tuned, ({:.3}, {:.3}) in the twin",
            phase, time, worst.0, worst.2.x, worst.2.y, worst.3.x, worst.3.y
        );
        shown += 1;
        if shown >= 12 {
            println!("      ...");
            break;
        }
    }
    if shown == 0 {
        println!("      (no phase moved a point or bone differently; the tool parted first)");
    }
}
