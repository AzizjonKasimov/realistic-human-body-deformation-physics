//! Random testing: thousands of random blows with every tool at every part of
//! the body, swung from every side at every speed and pressed, held, and
//! dragged, each watched by the watchdogs and judged by the rules every play
//! follows. The scenarios try what someone thought to try; this tries what
//! no one did. Every play is written out in the syntax `--strike` and
//! `--gesture` read, rounded as written, so any of them replays exactly.

use super::*;
use rp::inspect::{Watchdog, Watchdogs};
use rp::scenarios::GestureStep;
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

/// A small generator that throws the same plays for the same seed
/// (SplitMix64, Steele, Lea, and Flood 2014).
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number from `low` up to `high`.
    fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * ((self.next() >> 11) as f64 / (1u64 << 53) as f64)
    }

    /// A whole number from `low` to `high`, both included.
    fn whole(&mut self, low: i32, high: i32) -> i32 {
        low + (self.next() % (high - low + 1) as u64) as i32
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[(self.next() % items.len() as u64) as usize]
    }
}

/// A number as the plays are written, to three decimals, so a written play
/// replays the very run it came from.
fn written(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// Where blows can land and start from: every point of skin on the body as
/// built, in body coordinates, and the body to tell which places lie
/// inside it.
struct Targets {
    skin: Vec<(f64, f64)>,
    body: rp::World,
    frame: rp::BodyFrame,
}

impl Targets {
    fn new() -> Self {
        let body = settled_body(
            SCENARIO_WIDTH,
            SCENARIO_HEIGHT,
            rp::scenarios::set_materials(),
        );
        let frame = rp::body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT);
        let skin = body
            .points()
            .iter()
            .filter(|point| point.layer == rp::TissueLayer::Skin && !point.is_ghost())
            .map(|point| frame.body_coords(point.home))
            .collect();
        Self { skin, body, frame }
    }

    fn inside(&self, at: (f64, f64)) -> bool {
        inside_the_skin(&self.body, self.frame.point(at.0, at.1))
    }

    /// A place outside the body `reach` body heights back from `target`
    /// along `-direction`, further back until it is outside.
    fn approach(&self, target: (f64, f64), direction: (f64, f64), reach: f64) -> (f64, f64) {
        let mut reach = reach;
        loop {
            let at = (
                written(target.0 - direction.0 * reach),
                written(target.1 - direction.1 * reach),
            );
            if !self.inside(at) || reach > 1.0 {
                return at;
            }
            reach += 0.05;
        }
    }
}

/// A random play as the numbers it is drawn from, each written so that 0 is
/// the simplest: shrinking one toward 0 (`shrink`) makes the play slower,
/// shorter, more level, or plainer while it still breaks what it broke. The
/// tool, the point of skin it is aimed through, and swing or press stay.
#[derive(Clone, Debug)]
struct Draw {
    tool: rp::ToolMode,
    target: (f64, f64),
    /// Tilted down from level, or up.
    downward: bool,
    /// How far the tool's path tilts from level, from 0 (from the left) to
    /// pi (from the right).
    tilt: f64,
    /// How far back it starts, past 0.30 body heights (up to 0.15).
    reach: f64,
    /// A swing, or the tool pressed in.
    swing: bool,
    /// How far past the target the swing follows through, past 0.05 body
    /// heights (up to 0.20).
    through: f64,
    /// How many frames faster than its slowest the swing or press goes.
    haste: i32,
    /// How many frames past 40 a swing settles (up to 50).
    settle: i32,
    /// How many frames a press holds (up to 30).
    hold: i32,
    /// Where a press drags on to, from the target, if it does.
    drag: Option<(f64, f64)>,
    /// How many frames faster than 40 the drag goes (up to 30).
    drag_haste: i32,
    /// How many frames past 30 it waits once let go (up to 30).
    rest: i32,
}

/// The slowest a swing goes, in frames, and how many frames faster it can go.
fn swing_frames(tool: rp::ToolMode) -> (i32, i32) {
    match tool {
        rp::ToolMode::Sharp => (24, 18),
        _ => (18, 13),
    }
}

/// The numbers for one random play: a swing with any tool through any point
/// of skin from any side at any speed, or the tool pressed into it, held,
/// and dragged. They are drawn in the order the plays were drawn before they
/// could shrink, so a seed throws the same plays.
fn random_draw(dice: &mut Dice, targets: &Targets, tools: &[rp::ToolMode]) -> Draw {
    let tool = dice.pick(tools);
    let target = dice.pick(&targets.skin);
    let mut angle = dice.range(0.0, TAU);
    if angle > PI {
        angle -= TAU;
    }
    let mut draw = Draw {
        tool,
        target,
        downward: angle > 0.0,
        tilt: angle.abs(),
        reach: dice.range(0.30, 0.45) - 0.30,
        swing: dice.range(0.0, 1.0) < 0.7,
        through: 0.0,
        haste: 0,
        settle: 0,
        hold: 0,
        drag: None,
        drag_haste: 0,
        rest: 0,
    };
    if draw.swing {
        let (slowest, faster) = swing_frames(tool);
        draw.through = dice.range(0.05, 0.25) - 0.05;
        draw.haste = slowest - dice.whole(slowest - faster, slowest);
        draw.settle = dice.whole(40, 90) - 40;
        return draw;
    }
    draw.haste = 40 - dice.whole(6, 40);
    draw.hold = dice.whole(0, 30);
    if dice.range(0.0, 1.0) < 0.5 {
        draw.drag = Some((dice.range(-0.12, 0.12), dice.range(-0.12, 0.12)));
        draw.drag_haste = 40 - dice.whole(10, 40);
    }
    draw.rest = dice.whole(30, 60) - 30;
    draw
}

impl Draw {
    /// The play these numbers make.
    fn play(&self, targets: &Targets) -> Play {
        let angle = if self.downward { self.tilt } else { -self.tilt };
        let direction = (angle.cos(), angle.sin());
        let target = self.target;
        let start = targets.approach(target, direction, 0.30 + self.reach);
        if self.swing {
            let (slowest, faster) = swing_frames(self.tool);
            let through = 0.05 + self.through;
            return Play::Swing(
                Strike {
                    tool: self.tool,
                    start,
                    end: (
                        written(target.0 + direction.0 * through),
                        written(target.1 + direction.1 * through),
                    ),
                    windup_frames: 6,
                    strike_frames: slowest - self.haste.clamp(0, faster),
                    settle_frames: 40 + self.settle,
                    power: rp::swing_power(self.tool),
                },
                None,
            );
        }
        // Pressed in, held a while, maybe dragged on, and let go.
        let mut steps = vec![
            GestureStep::Move {
                to: start,
                frames: 0,
            },
            GestureStep::Wait(10),
            GestureStep::Press(true),
            GestureStep::Move {
                to: (written(target.0), written(target.1)),
                frames: 40 - self.haste.clamp(0, 34),
            },
            GestureStep::Wait(self.hold),
        ];
        if let Some((du, dv)) = self.drag {
            steps.push(GestureStep::Move {
                to: (written(target.0 + du), written(target.1 + dv)),
                frames: 40 - self.drag_haste.clamp(0, 30),
            });
        }
        steps.push(GestureStep::Press(false));
        steps.push(GestureStep::Wait(30 + self.rest));
        Play::Gesture(Gesture {
            fight: None,
            tool: self.tool,
            power: rp::swing_power(self.tool),
            steps: Vec::leak(steps),
        })
    }
}

/// The most plays a shrink may play: enough to halve each of the play's
/// numbers several times.
const SHRINK_BUDGET: usize = 40;

/// What a shrunk play must still break: a rule every play follows, by name,
/// or a watchdog.
#[derive(Clone, Copy)]
enum Breaks<'a> {
    Rule(&'a str),
    Watchdog(Watchdog),
}

/// Plays `play` and says whether it broke `what`.
fn still_breaks(play: Play, what: Breaks) -> bool {
    let scenario = Scenario::custom(play);
    let body = settled_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, scenario.materials());
    let mut watchdogs = Watchdogs::new(&body);
    let (_, result) = run_on(
        body,
        &scenario,
        SCENARIO_WIDTH,
        SCENARIO_HEIGHT,
        |frame, _, world| watchdogs.check(frame, world),
    );
    match what {
        Breaks::Rule(rule) => scenario
            .real_world_problems(&result)
            .iter()
            .any(|line| rule_name(line) == rule),
        Breaks::Watchdog(kind) => watchdogs.trips().iter().any(|trip| trip.watchdog == kind),
    }
}

/// Shrinks one number of `best` with proptest's binary search `tree`,
/// writing each value back with `set` and keeping every one that still
/// breaks it, while plays are left in `budget`.
fn shrink_number<T: proptest::strategy::ValueTree>(
    mut tree: T,
    set: impl Fn(&mut Draw, T::Value),
    best: &mut Draw,
    tries: &mut usize,
    budget: usize,
    breaks: &dyn Fn(&Draw) -> bool,
) {
    if !tree.simplify() {
        return;
    }
    while *tries < budget {
        *tries += 1;
        let mut candidate = best.clone();
        set(&mut candidate, tree.current());
        if breaks(&candidate) {
            *best = candidate;
            if !tree.simplify() {
                return;
            }
        } else if !tree.complicate() {
            return;
        }
    }
}

/// The simplest play like `draw` that still breaks `what`, and how many
/// plays it took to find: each number in turn is shrunk toward 0 by
/// proptest's binary search (`BinarySearch`), keeping every value that
/// still breaks it, within `budget` plays in all.
fn shrink_draw(draw: &Draw, targets: &Targets, what: Breaks, budget: usize) -> (Draw, usize) {
    use proptest::num::{f64 as float, i32 as whole};

    let breaks = |candidate: &Draw| still_breaks(candidate.play(targets), what);
    let mut best = draw.clone();
    let mut tries = 0;
    let tilt = float::BinarySearch::new(best.tilt);
    shrink_number(
        tilt,
        |d, v| d.tilt = v,
        &mut best,
        &mut tries,
        budget,
        &breaks,
    );
    let haste = whole::BinarySearch::new(best.haste);
    shrink_number(
        haste,
        |d, v| d.haste = v,
        &mut best,
        &mut tries,
        budget,
        &breaks,
    );
    let through = float::BinarySearch::new(best.through);
    shrink_number(
        through,
        |d, v| d.through = v,
        &mut best,
        &mut tries,
        budget,
        &breaks,
    );
    let reach = float::BinarySearch::new(best.reach);
    shrink_number(
        reach,
        |d, v| d.reach = v,
        &mut best,
        &mut tries,
        budget,
        &breaks,
    );
    let settle = whole::BinarySearch::new(best.settle);
    shrink_number(
        settle,
        |d, v| d.settle = v,
        &mut best,
        &mut tries,
        budget,
        &breaks,
    );
    let hold = whole::BinarySearch::new(best.hold);
    shrink_number(
        hold,
        |d, v| d.hold = v,
        &mut best,
        &mut tries,
        budget,
        &breaks,
    );
    if best.drag.is_some() && tries < budget {
        // A press that still breaks it without its drag needs none.
        tries += 1;
        let mut plain = best.clone();
        plain.drag = None;
        if breaks(&plain) {
            best = plain;
        }
    }
    if let Some((du, dv)) = best.drag {
        let along = float::BinarySearch::new(du);
        shrink_number(
            along,
            |d, v| d.drag = d.drag.map(|(_, dv)| (v, dv)),
            &mut best,
            &mut tries,
            budget,
            &breaks,
        );
        let down = float::BinarySearch::new(dv);
        shrink_number(
            down,
            |d, v| d.drag = d.drag.map(|(du, _)| (du, v)),
            &mut best,
            &mut tries,
            budget,
            &breaks,
        );
        let drag_haste = whole::BinarySearch::new(best.drag_haste);
        shrink_number(
            drag_haste,
            |d, v| d.drag_haste = v,
            &mut best,
            &mut tries,
            budget,
            &breaks,
        );
    }
    let rest = whole::BinarySearch::new(best.rest);
    shrink_number(
        rest,
        |d, v| d.rest = v,
        &mut best,
        &mut tries,
        budget,
        &breaks,
    );
    (best, tries)
}

/// What one random play did and what it broke of the rules.
struct Played {
    draw: Draw,
    play: Play,
    result: ScenarioResult,
    /// The rules every play follows that it broke.
    broken: Vec<String>,
    /// The first trip of each watchdog: the frame and the thing, as
    /// `--why` takes it and by name.
    flagged: Vec<(Watchdog, i32, String)>,
}

/// The rule a line from `Scenario::real_world_problems` names.
fn rule_name(problem: &str) -> &str {
    problem.split('=').next().unwrap_or(problem)
}

/// Plays `count` random blows from `seed` with `tools`, on all cores, and
/// reports what broke the rules and what the watchdogs flagged, with plays
/// to replay; with `shrink`, also the simplest play like the first that
/// broke each, which still breaks it.
pub(crate) fn random_blows(
    count: usize,
    seed: u64,
    tools: &[rp::ToolMode],
    output_dir: &Path,
    shrink: bool,
) {
    let started = Instant::now();
    let targets = Targets::new();
    let mut dice = Dice(seed);
    let draws: Vec<Draw> = (0..count)
        .map(|_| random_draw(&mut dice, &targets, tools))
        .collect();
    let plays: Vec<Play> = draws.iter().map(|draw| draw.play(&targets)).collect();
    let next = AtomicUsize::new(0);
    let finished = Mutex::new(Vec::with_capacity(plays.len()));
    let workers = thread::available_parallelism().map_or(4, |count| count.get());
    thread::scope(|scope| {
        for _ in 0..workers.min(plays.len()) {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(&play) = plays.get(index) else {
                    break;
                };
                let draw = draws[index].clone();
                let scenario = Scenario::custom(play);
                let body = settled_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, scenario.materials());
                let mut watchdogs = Watchdogs::new(&body);
                let (world, result) = run_on(
                    body,
                    &scenario,
                    SCENARIO_WIDTH,
                    SCENARIO_HEIGHT,
                    |frame, _, world| watchdogs.check(frame, world),
                );
                let broken = scenario.real_world_problems(&result);
                let flagged = flagged(&world, watchdogs.trips());
                finished.lock().expect("random results").push((
                    index,
                    Played {
                        draw,
                        play,
                        result,
                        broken,
                        flagged,
                    },
                ));
            });
        }
    });
    let mut played = finished.into_inner().expect("random results");
    played.sort_by_key(|(index, _)| *index);
    let played: Vec<Played> = played.into_iter().map(|(_, played)| played).collect();

    let csv_path = output_dir.join("random_blows.csv");
    let mut csv = BufWriter::new(File::create(&csv_path).expect("create random CSV"));
    writeln!(
        csv,
        "index,play,tool,bones_broken,skin_torn,muscle_torn,arteries_cut,blood_lost_ml,rules_broken,watchdogs"
    )
    .expect("write random CSV");
    for (index, run) in played.iter().enumerate() {
        let rules: Vec<&str> = run.broken.iter().map(|line| rule_name(line)).collect();
        let watchdogs: Vec<String> = run
            .flagged
            .iter()
            .map(|(watchdog, frame, _)| format!("{watchdog:?}@{frame}"))
            .collect();
        writeln!(
            csv,
            "{index},\"{}\",{},{},{},{},{},{:.2},{},{}",
            run.play,
            tool_name(run.play.tool()),
            run.result.bone_fractures,
            run.result.skin_tears,
            run.result.muscle_tears,
            run.result.vessel_lacerations,
            run.result.blood_loss * rp::scenarios::BLOOD_VOLUME_ML,
            rules.join(";"),
            watchdogs.join(";")
        )
        .expect("write random CSV");
    }

    let option = |play: &Play| match play {
        Play::Swing(..) => format!("--strike \"{play}\""),
        Play::Gesture(_) => format!("--gesture \"{play}\""),
    };
    let mut report = format!(
        "Random blows: {} plays from seed {seed} ({}), in {:.0} s\n",
        played.len(),
        tools
            .iter()
            .map(|&tool| {
                let n = played.iter().filter(|run| run.play.tool() == tool).count();
                format!("{} {n}", tool_name(tool))
            })
            .collect::<Vec<_>>()
            .join(", "),
        started.elapsed().as_secs_f64()
    );
    let mut rules: Vec<&str> = played
        .iter()
        .flat_map(|run| run.broken.iter().map(|line| rule_name(line)))
        .collect();
    rules.sort();
    rules.dedup();
    let mut kinds: Vec<Watchdog> = played
        .iter()
        .flat_map(|run| run.flagged.iter().map(|(watchdog, _, _)| *watchdog))
        .collect();
    kinds.sort();
    kinds.dedup();
    // With `shrink`, the first play to break each rule and to trip each
    // watchdog is shrunk to the simplest play like it that still does, each
    // on its own core.
    let shrunk: HashMap<String, (Draw, usize)> = if shrink {
        let mut jobs: Vec<(String, Breaks, Draw)> = Vec::new();
        for &rule in &rules {
            if let Some(run) = played
                .iter()
                .find(|run| run.broken.iter().any(|line| rule_name(line) == rule))
            {
                jobs.push((format!("rule {rule}"), Breaks::Rule(rule), run.draw.clone()));
            }
        }
        for &kind in &kinds {
            if let Some(run) = played
                .iter()
                .find(|run| run.flagged.iter().any(|(watchdog, _, _)| *watchdog == kind))
            {
                jobs.push((
                    format!("watchdog {}", kind.slug()),
                    Breaks::Watchdog(kind),
                    run.draw.clone(),
                ));
            }
        }
        let results = Mutex::new(HashMap::new());
        thread::scope(|scope| {
            for (label, what, draw) in &jobs {
                let (targets, results) = (&targets, &results);
                scope.spawn(move || {
                    let found = shrink_draw(draw, targets, *what, SHRINK_BUDGET);
                    results
                        .lock()
                        .expect("shrunk plays")
                        .insert(label.clone(), found);
                });
            }
        });
        results.into_inner().expect("shrunk plays")
    } else {
        HashMap::new()
    };
    let simplest = |label: String| {
        shrunk.get(&label).map(|(draw, tries)| {
            format!(
                "    simplest still breaking it, found in {tries} plays: {}\n",
                option(&draw.play(&targets))
            )
        })
    };
    if rules.is_empty() {
        report.push_str("Every play kept the rules every play follows.\n");
    } else {
        report.push_str("The rules every play follows, broken:\n");
        for rule in rules {
            let breaking: Vec<&Played> = played
                .iter()
                .filter(|run| run.broken.iter().any(|line| rule_name(line) == rule))
                .collect();
            report.push_str(&format!(
                "  {rule} in {} plays; e.g. {}: {}\n",
                breaking.len(),
                option(&breaking[0].play),
                breaking[0]
                    .broken
                    .iter()
                    .find(|line| rule_name(line) == rule)
                    .map_or("", |line| line.as_str())
            ));
            if let Some(line) = simplest(format!("rule {rule}")) {
                report.push_str(&line);
            }
        }
    }
    if kinds.is_empty() {
        report.push_str("The watchdogs flagged nothing.\n");
    } else {
        report.push_str("What the watchdogs flagged:\n");
        for kind in kinds {
            let tripped: Vec<&Played> = played
                .iter()
                .filter(|run| run.flagged.iter().any(|(watchdog, _, _)| *watchdog == kind))
                .collect();
            let by_tool: Vec<String> = tools
                .iter()
                .map(|&tool| {
                    format!(
                        "{} {}",
                        tool_name(tool),
                        tripped.iter().filter(|run| run.play.tool() == tool).count()
                    )
                })
                .collect();
            report.push_str(&format!(
                "  {}{} in {} plays ({})\n",
                kind.describe(),
                if kind.known_gap() { " [known gap]" } else { "" },
                tripped.len(),
                by_tool.join(", ")
            ));
            if let Some(line) = simplest(format!("watchdog {}", kind.slug())) {
                report.push_str(&line);
            }
            for run in tripped.iter().take(3) {
                let (_, frame, what) = run
                    .flagged
                    .iter()
                    .find(|(watchdog, _, _)| *watchdog == kind)
                    .expect("it tripped");
                report.push_str(&format!(
                    "    {} from frame {frame}: {what}\n",
                    option(&run.play)
                ));
            }
        }
    }
    let report_path = output_dir.join("random_blows_report.txt");
    fs::write(&report_path, &report).expect("write random report");
    print!("{report}");
    println!("wrote {}", csv_path.display());
    println!("wrote {}", report_path.display());
}
