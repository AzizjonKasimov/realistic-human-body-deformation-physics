//! Moved-body tests. The same blows on the body mirrored left to right,
//! moved across the window, or drawn larger should hurt it alike, judged
//! over the sweep's runs: a rule that differs is reading where something is
//! on the screen, or its size in pixels, rather than what is happening to
//! the body. And a body falling or drifting free, nothing touching it,
//! should come to no harm, its flesh staying put on its bones: anything
//! else is a rule judging motion against the screen instead of against the
//! flesh around it, as broken pieces were once damped against the world.

use super::*;

/// A way to move the body and its blows that should change nothing but
/// where things happen.
#[derive(Clone, Copy, PartialEq)]
enum Moved {
    Same,
    Mirrored,
    Across,
    Larger,
}

impl Moved {
    const ALL: [Moved; 4] = [Moved::Same, Moved::Mirrored, Moved::Across, Moved::Larger];

    fn describe(self) -> &'static str {
        match self {
            Moved::Same => "as tuned",
            Moved::Mirrored => "mirrored left to right",
            Moved::Across => "moved 208 px across the window",
            Moved::Larger => "drawn 28% larger, as on a 1080p screen",
        }
    }

    /// The window it plays in: wider moves the body across without
    /// changing its size; taller draws it larger, at the app's largest.
    fn window(self) -> (f64, f64) {
        match self {
            Moved::Same | Moved::Mirrored => (SCENARIO_WIDTH, SCENARIO_HEIGHT),
            Moved::Across => (SCENARIO_WIDTH + 400.0, SCENARIO_HEIGHT),
            Moved::Larger => (1600.0, 923.0),
        }
    }

    fn scenario(self, scenario: &Scenario) -> Scenario {
        match self {
            Moved::Mirrored => scenario.mirrored(),
            _ => *scenario,
        }
    }

    /// How many of this window's pixels make one of the scenarios' window.
    fn px_scale(self) -> f64 {
        let (width, height) = self.window();
        rp::body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT).height
            / rp::body_frame(width, height).height
    }
}

/// A number the comparison judges, how to read it from a run, whether it
/// is in pixels (and so scales with the body drawn larger), and the least
/// difference in its median that counts.
struct Metric {
    name: &'static str,
    value: fn(&ScenarioResult) -> f64,
    px: bool,
    floor: f64,
}

fn metrics() -> [Metric; 9] {
    [
        Metric {
            name: "bones broken",
            value: |r| r.bone_fractures as f64,
            px: false,
            floor: 1.0,
        },
        Metric {
            name: "skin torn",
            value: |r| r.skin_tears as f64,
            px: false,
            floor: 4.0,
        },
        Metric {
            name: "muscle torn",
            value: |r| r.muscle_tears as f64,
            px: false,
            floor: 4.0,
        },
        Metric {
            name: "bruises",
            value: |r| r.contusion_events as f64,
            px: false,
            floor: 15.0,
        },
        Metric {
            name: "arteries cut",
            value: |r| r.vessel_lacerations as f64,
            px: false,
            floor: 1.0,
        },
        Metric {
            name: "blood lost (ml)",
            value: |r| r.blood_loss * rp::scenarios::BLOOD_VOLUME_ML,
            px: false,
            floor: 2.0,
        },
        Metric {
            name: "fastest tissue (px/s)",
            value: |r| r.max_point_speed,
            px: true,
            floor: 250.0,
        },
        Metric {
            name: "fist off its guard (px)",
            value: |r| r.final_fist_offset,
            px: true,
            floor: 6.0,
        },
        Metric {
            name: "head turn (degrees)",
            value: |r| r.max_head_turn,
            px: false,
            floor: 5.0,
        },
    ]
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(|p, q| p.total_cmp(q));
    values.get(values.len() / 2).copied().unwrap_or(0.0)
}

/// Where `moved`'s runs of `scenario` differ from `base`'s by more than the
/// aim and the bones' strength spread them: how often each real-life
/// outcome happened, how often the runs stayed in band, and the medians.
fn differences(
    scenario: &Scenario,
    moved: Moved,
    base: &[(ScenarioResult, bool)],
    other: &[(ScenarioResult, bool)],
) -> Vec<String> {
    let runs = base.len().max(1);
    // A fifth of the runs, at least nine of the sweep's 45.
    let count_limit = (runs / 5).max(2);
    let mut found = Vec::new();
    let in_band = |runs: &[(ScenarioResult, bool)]| runs.iter().filter(|(_, ok)| *ok).count();
    let (base_band, other_band) = (in_band(base), in_band(other));
    if base_band.abs_diff(other_band) >= count_limit {
        found.push(format!(
            "in band {base_band}/{runs} -> {other_band}/{}",
            other.len()
        ));
    }
    for check in scenario.real {
        let hits = |runs: &[(ScenarioResult, bool)]| {
            runs.iter()
                .filter(|(result, _)| check.outcome.happened(result))
                .count()
        };
        let (before, after) = (hits(base), hits(other));
        if before.abs_diff(after) >= count_limit {
            found.push(format!(
                "{}: {before}/{runs} -> {after}/{}",
                check.outcome.describe(),
                other.len()
            ));
        }
    }
    let scale = moved.px_scale();
    for metric in &metrics() {
        let before = median(
            base.iter()
                .map(|(result, _)| (metric.value)(result))
                .collect(),
        );
        let after = median(
            other
                .iter()
                .map(|(result, _)| (metric.value)(result) * if metric.px { scale } else { 1.0 })
                .collect(),
        );
        let limit = (0.3 * before.abs().max(after.abs())).max(metric.floor);
        if (after - before).abs() > limit {
            found.push(format!(
                "median {} {before:.1} -> {after:.1}{}",
                metric.name,
                if metric.px && scale != 1.0 {
                    " (in the tuned window's pixels)"
                } else {
                    ""
                }
            ));
        }
    }
    found
}

/// Plays every selected scenario's sweep as tuned and moved each way, on
/// all cores, and reports where a move changed what the blows did; then
/// lets a body fall and drift free.
pub(crate) fn moved_body_tests(selected: &[Scenario], output_dir: &Path) {
    let started = Instant::now();
    let mut jobs = Vec::new();
    for (index, _) in selected.iter().enumerate() {
        for (way, _) in Moved::ALL.iter().enumerate() {
            for &strength in &SWEEP_STRENGTH {
                for &along in &SWEEP_ALONG {
                    for &across in &SWEEP_ACROSS {
                        jobs.push((index, way, along, across, strength));
                    }
                }
            }
        }
    }
    let next = AtomicUsize::new(0);
    let finished = Mutex::new(Vec::with_capacity(jobs.len()));
    let workers = thread::available_parallelism().map_or(4, |count| count.get());
    thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len()) {
            scope.spawn(|| loop {
                let job = next.fetch_add(1, Ordering::Relaxed);
                let Some(&(index, way, along, across, strength)) = jobs.get(job) else {
                    break;
                };
                let moved = Moved::ALL[way];
                let scenario = moved
                    .scenario(&selected[index])
                    .shifted(along, across)
                    .with_bone_strength(strength);
                let (width, height) = moved.window();
                let body = settled_body(width, height, scenario.materials());
                let (_, result) = run_on(body, &scenario, width, height, |_, _, _| {});
                let ok = scenario.violations(&result).is_empty();
                finished
                    .lock()
                    .expect("moved results")
                    .push((job, (index, way, result, ok)));
            });
        }
    });
    let mut runs = finished.into_inner().expect("moved results");
    runs.sort_by_key(|(job, _)| *job);
    // Each scenario's runs played each way.
    let mut played: Vec<Vec<Vec<(ScenarioResult, bool)>>> =
        vec![vec![Vec::new(); Moved::ALL.len()]; selected.len()];
    for (_, (index, way, result, ok)) in runs {
        played[index][way].push((result, ok));
    }

    let mut report = String::new();
    let mut differing = vec![0; Moved::ALL.len()];
    for (index, scenario) in selected.iter().enumerate() {
        let base = &played[index][0];
        for (way, &moved) in Moved::ALL.iter().enumerate().skip(1) {
            let found = differences(scenario, moved, base, &played[index][way]);
            if found.is_empty() {
                continue;
            }
            differing[way] += 1;
            report.push_str(&format!(
                "  {} {}: {}\n",
                scenario.name,
                moved.describe(),
                found.join("; ")
            ));
        }
    }
    let mut text = String::from("Moved-body tests: the same blows should hurt the body alike\n");
    for (way, &moved) in Moved::ALL.iter().enumerate().skip(1) {
        text.push_str(&format!(
            "  {}: {} of {} scenarios differ\n",
            moved.describe(),
            differing[way],
            selected.len()
        ));
    }
    if !report.is_empty() {
        text.push_str("Where they differ, over the sweep's runs (as tuned -> moved):\n");
        text.push_str(&report);
    }
    text.push_str("A body free of everything, nothing touching it, for a second:\n");
    for line in free_body_checks() {
        text.push_str(&format!("  {line}\n"));
    }
    let path = output_dir.join("strike_moved_report.txt");
    fs::write(&path, &text).expect("write moved-body report");
    print!("{text}");
    println!(
        "wrote {} ({} runs in {:.0} s)",
        path.display(),
        jobs.len(),
        started.elapsed().as_secs_f64()
    );
}

/// What a second free of everything did to a body: falling under gravity,
/// and drifting sideways at about 1 m/s with no gravity.
fn free_body_checks() -> Vec<String> {
    let falling = free_body(
        "falling free",
        rp::scenarios::set_materials(),
        rp::Vec2::default(),
    );
    let drifting = free_body(
        "drifting at 1 m/s with no gravity",
        rp::Materials {
            gravity: 0.0,
            ..rp::scenarios::set_materials()
        },
        rp::Vec2 { x: 320.0, y: 0.0 },
    );
    vec![falling, drifting]
}

/// A body built in the scenarios' window, its hips let go and its floor
/// taken away, set moving at `velocity`, after a second.
fn free_body(name: &str, materials: rp::Materials, velocity: rp::Vec2) -> String {
    let mut world = settled_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, materials);
    world.release_hips();
    world.set_floor_y(f64::INFINITY);
    world.keep_journal();
    world.set_body_velocity(velocity);
    let flesh_offsets = |world: &rp::World| -> Vec<Option<rp::Vec2>> {
        (0..world.points().len())
            .map(|index| {
                let place = world.posture_target(index)?;
                let point = world.points()[index].position;
                Some(rp::Vec2 {
                    x: point.x - place.x,
                    y: point.y - place.y,
                })
            })
            .collect()
    };
    let turns = |world: &rp::World| -> Vec<f64> {
        (0..world.bone_joints().len())
            .map(|joint| world.joint_turn(joint).unwrap_or(0.0))
            .collect()
    };
    let (offsets_before, turns_before) = (flesh_offsets(&world), turns(&world));
    let dt = world.materials().fixed_dt;
    let frames = (1.0 / dt).round() as i32;
    for _ in 0..frames {
        world.step(dt, &rp::InputState::default(), SCENARIO_WIDTH);
    }
    let slid = flesh_offsets(&world)
        .iter()
        .zip(&offsets_before)
        .filter_map(|(after, before)| {
            let (after, before) = ((*after)?, (*before)?);
            Some((after.x - before.x).hypot(after.y - before.y))
        })
        .fold(0.0, f64::max);
    let bent = turns(&world)
        .iter()
        .zip(&turns_before)
        .map(|(after, before)| (after - before).abs())
        .fold(0.0, f64::max)
        .to_degrees();
    let injuries = world.journal().map_or(0, |journal| {
        journal
            .events
            .iter()
            .filter(|event| !event.kind.is_minor())
            .count()
    });
    let speeds = |velocities: Vec<rp::Vec2>| -> f64 {
        let count = velocities.len().max(1) as f64;
        let sum = velocities
            .iter()
            .fold(rp::Vec2::default(), |sum, v| rp::Vec2 {
                x: sum.x + v.x,
                y: sum.y + v.y,
            });
        (sum.x / count).hypot(sum.y / count)
    };
    let flesh_speed = speeds(
        world
            .points()
            .iter()
            .map(|point| rp::Vec2 {
                x: (point.position.x - point.previous.x) / dt,
                y: (point.position.y - point.previous.y) / dt,
            })
            .collect(),
    );
    let bone_speed = speeds(
        world
            .bones()
            .iter()
            .map(|bone| rp::Vec2 {
                x: (bone.a.x + bone.b.x - bone.previous_a.x - bone.previous_b.x) * 0.5 / dt,
                y: (bone.a.y + bone.b.y - bone.previous_a.y - bone.previous_b.y) * 0.5 / dt,
            })
            .collect(),
    );
    format!(
        "{name}: {injuries} injuries, flesh slid up to {slid:.1} px on its bones, joints bent up to {bent:.1} degrees; flesh ends at {flesh_speed:.0} px/s and bone at {bone_speed:.0} px/s"
    )
}
