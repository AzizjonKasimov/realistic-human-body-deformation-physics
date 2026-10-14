//! The sweep: every scenario replayed with its swing moved a little along
//! and across its path, on bones a little weaker and stronger, judged over
//! all its runs against real life.

use super::*;
use diff::{borderline, run_numbers, RUNS_FILE};
use rp::inspect::Watchdog;

/// How far a sweep moves each swing along its path and across it, in body
/// heights: about one point spacing either way, as aim varies from swing to
/// swing.
pub(crate) const SWEEP_ALONG: [f64; 5] = [-0.010, -0.005, 0.0, 0.005, 0.010];

pub(crate) const SWEEP_ACROSS: [f64; 3] = [-0.005, 0.0, 0.005];

/// Bone strengths a sweep plays, against the tuned strength: people's bones
/// differ by 15-28% (one standard deviation) in the fracture studies in
/// `docs/INJURY_REFERENCE.md`, so a blow near a bone's limit breaks it in
/// some people and not in others.
pub(crate) const SWEEP_STRENGTH: [f64; 3] = [0.8, 1.0, 1.25];

struct SweepRun {
    scenario: usize,
    along: f64,
    across: f64,
    strength: f64,
    result: ScenarioResult,
    violations: Vec<String>,
    /// What the watchdogs flagged: each watchdog with the first frame it
    /// tripped on and the first thing it flagged, as `--why` takes it and by
    /// name.
    flagged: Vec<(Watchdog, i32, String)>,
}

/// The first trip of each watchdog in a run, the thing named while the
/// run's world is still at hand.
pub(crate) fn flagged(
    world: &rp::World,
    trips: &[rp::inspect::Trip],
) -> Vec<(Watchdog, i32, String)> {
    let mut flagged: Vec<(Watchdog, i32, String)> = Vec::new();
    for trip in trips {
        if flagged
            .iter()
            .any(|(watchdog, _, _)| *watchdog == trip.watchdog)
        {
            continue;
        }
        flagged.push((
            trip.watchdog,
            trip.first_frame,
            format!(
                "{}, {}",
                rp::inspect::subject_spec(trip.subject),
                rp::inspect::subject_name(world, trip.subject)
            ),
        ));
    }
    flagged
}

/// The quick sweep's offsets along the path: 9 runs a scenario instead of
/// 45, to see a change's direction in a fifth of the time before the whole
/// sweep judges it.
const QUICK_ALONG: [f64; 3] = [-0.005, 0.0, 0.005];

/// Replays each scenario with its swing moved by every combination of
/// `SWEEP_ALONG` and `SWEEP_ACROSS` (with `quick`, `QUICK_ALONG` alone), on
/// all cores, and reports the spread.
pub(crate) fn sweep(selected: &[Scenario], output_dir: &Path, custom: bool, quick: bool) {
    let (along, across): (&[f64], &[f64]) = if quick {
        (&QUICK_ALONG, &[0.0])
    } else {
        (&SWEEP_ALONG, &SWEEP_ACROSS)
    };
    let jobs: Vec<(usize, f64, f64, f64)> = (0..selected.len())
        .flat_map(|index| {
            SWEEP_STRENGTH.iter().flat_map(move |&strength| {
                along.iter().flat_map(move |&along| {
                    across
                        .iter()
                        .map(move |&across| (index, along, across, strength))
                })
            })
        })
        .collect();
    let next = AtomicUsize::new(0);
    let finished = Mutex::new(Vec::with_capacity(jobs.len()));
    let workers = thread::available_parallelism().map_or(4, |count| count.get());
    thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len()) {
            scope.spawn(|| loop {
                let job = next.fetch_add(1, Ordering::Relaxed);
                let Some(&(index, along, across, strength)) = jobs.get(job) else {
                    break;
                };
                let scenario = selected[index]
                    .shifted(along, across)
                    .with_bone_strength(strength);
                let body =
                    rp::create_layered_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, scenario.materials());
                let mut watchdogs = rp::inspect::Watchdogs::new(&body);
                let (world, result) = run_on(
                    body,
                    &scenario,
                    SCENARIO_WIDTH,
                    SCENARIO_HEIGHT,
                    |frame, _, world| watchdogs.check(frame, world),
                );
                let violations = scenario.violations(&result);
                let flagged = flagged(&world, watchdogs.trips());
                finished.lock().expect("sweep results").push((
                    job,
                    SweepRun {
                        scenario: index,
                        along,
                        across,
                        strength,
                        result,
                        violations,
                        flagged,
                    },
                ));
            });
        }
    });
    let mut runs = finished.into_inner().expect("sweep results");
    runs.sort_by_key(|(job, _)| *job);
    let runs: Vec<SweepRun> = runs.into_iter().map(|(_, run)| run).collect();

    let csv_path = output_dir.join("strike_sweep.csv");
    let mut csv = BufWriter::new(File::create(&csv_path).expect("create sweep CSV"));
    // How close each part of the skeleton came to breaking in each run, as a
    // share of its strength, for tuning how often blows break bones.
    let peak_header: Vec<String> = rp::BonePart::ALL
        .iter()
        .map(|part| format!("peak_load_{}", part.name().replace(' ', "_")))
        .collect();
    writeln!(
        csv,
        "along,across,bone_strength,in_band,violations,{SUMMARY_HEADER},{}",
        peak_header.join(",")
    )
    .expect("write sweep CSV");
    for run in &runs {
        let scenario = &selected[run.scenario];
        let failed: Vec<&str> = run
            .violations
            .iter()
            .map(|violation| violated_metric(violation))
            .collect();
        let peaks: Vec<String> = run
            .result
            .part_peak_load
            .iter()
            .map(|load| format!("{load:.4}"))
            .collect();
        writeln!(
            csv,
            "{:.4},{:.4},{:.2},{},{},{},{}",
            run.along,
            run.across,
            run.strength,
            u8::from(run.violations.is_empty()),
            failed.join(";"),
            summary_fields(scenario, &run.result).join(","),
            peaks.join(",")
        )
        .expect("write sweep CSV");
    }

    write_runs(selected, &runs, output_dir);

    let mut report = String::new();
    let settings = rp::scenarios::settings_used();
    if !settings.is_empty() {
        report.push_str(&format!(
            "Settings changed from the defaults: {}\n",
            settings.join(", ")
        ));
    }
    if quick {
        report.push_str(
            "Quick sweep: 9 runs a scenario (3 along, 3 bone strengths); the full sweep judges a change.\n",
        );
    }
    let mut mismatches = Vec::new();
    let mut gaps = Vec::new();
    let mut borderlines = Vec::new();
    for (index, scenario) in selected.iter().enumerate() {
        let mine: Vec<&SweepRun> = runs.iter().filter(|run| run.scenario == index).collect();
        report.push_str(&sweep_summary(scenario, &mine, custom));
        let results: Vec<&ScenarioResult> = mine.iter().map(|run| &run.result).collect();
        mismatches.extend(scenario.sweep_violations(&results));
        gaps.extend(scenario.sweep_gaps(&results));
        for check in scenario.real {
            let hits = results
                .iter()
                .filter(|result| check.outcome.happened(result))
                .count();
            if let Some(note) = borderline(check.often, hits, results.len()) {
                borderlines.push(format!(
                    "{}: {} ({}), {note}",
                    scenario.name,
                    check.outcome.describe(),
                    check.often.name()
                ));
            }
        }
    }
    if !custom && !gaps.is_empty() {
        report.push_str("Known gaps, still short of real life:\n");
        for gap in &gaps {
            report.push_str(&format!("  {gap}\n"));
        }
    }
    if !custom {
        if mismatches.is_empty() {
            report.push_str("Real life: every scenario breaks, opens, and bruises as often as it does in real life.\n");
        } else {
            report.push_str(
                "Real life: how often things happen across the sweep, against injury research:\n",
            );
            for mismatch in &mismatches {
                report.push_str(&format!("  {mismatch}\n"));
            }
        }
    }
    if !custom && !borderlines.is_empty() {
        report.push_str(
            "Borderline, where a few runs more or fewer could carry a real-life check across its limit:\n",
        );
        for line in &borderlines {
            report.push_str(&format!("  {line}\n"));
        }
    }
    report.push_str(&watchdog_summary(selected, &runs));
    let report_path = output_dir.join("strike_sweep_report.txt");
    fs::write(&report_path, &report).expect("write sweep report");
    print!("{report}");
    println!("wrote {}", csv_path.display());
    println!("wrote {}", report_path.display());
    if !custom && !mismatches.is_empty() {
        println!(
            "WARN: {} real-life mismatches across the sweep",
            mismatches.len()
        );
    }
}

/// Writes every run's outcomes one to a line (`RUNS_FILE`), for `--diff` to
/// set two sweeps side by side: whether it stayed in band and each band it
/// missed, each real-life check, each watchdog, and the numbers that most
/// often move (`run_numbers`).
fn write_runs(selected: &[Scenario], runs: &[SweepRun], output_dir: &Path) {
    let path = output_dir.join(RUNS_FILE);
    let mut out = BufWriter::new(File::create(&path).expect("create sweep runs"));
    let mut lines = vec![
        format!("# settings: {}", rp::scenarios::settings_used().join(" ")),
        "scenario,along,across,strength,kind,name,value".to_string(),
    ];
    for run in runs {
        let scenario = &selected[run.scenario];
        let key = format!(
            "{},{:.4},{:.4},{:.2}",
            scenario.name, run.along, run.across, run.strength
        );
        lines.push(format!(
            "{key},band,in_band,{}",
            u8::from(run.violations.is_empty())
        ));
        for violation in &run.violations {
            lines.push(format!("{key},band,{},1", violated_metric(violation)));
        }
        for check in scenario.real {
            lines.push(format!(
                "{key},real,{},{}",
                check.outcome.slug(),
                u8::from(check.outcome.happened(&run.result))
            ));
        }
        for watchdog in Watchdog::ALL {
            let tripped = run.flagged.iter().any(|(kind, _, _)| *kind == watchdog);
            lines.push(format!(
                "{key},watchdog,{},{}",
                watchdog.slug(),
                u8::from(tripped)
            ));
        }
        for (name, value) in run_numbers(&run.result) {
            lines.push(format!("{key},number,{name},{value:.4}"));
        }
    }
    for line in lines {
        writeln!(out, "{line}").expect("write sweep runs");
    }
    println!("wrote {}", path.display());
}

/// For each scenario, how many runs each watchdog tripped in, with one to
/// replay: the unmoved swing's if it tripped there, else the first.
fn watchdog_summary(selected: &[Scenario], runs: &[SweepRun]) -> String {
    let mut lines = Vec::new();
    for (index, scenario) in selected.iter().enumerate() {
        let mine: Vec<&SweepRun> = runs.iter().filter(|run| run.scenario == index).collect();
        let mut kinds: Vec<Watchdog> = mine
            .iter()
            .flat_map(|run| run.flagged.iter().map(|(watchdog, _, _)| *watchdog))
            .collect();
        kinds.sort();
        kinds.dedup();
        for kind in kinds {
            let tripped: Vec<&&SweepRun> = mine
                .iter()
                .filter(|run| run.flagged.iter().any(|(watchdog, _, _)| *watchdog == kind))
                .collect();
            let example = tripped
                .iter()
                .find(|run| run.along == 0.0 && run.across == 0.0 && run.strength == 1.0)
                .or(tripped.first())
                .expect("a tripped watchdog has runs");
            let (_, frame, what) = example
                .flagged
                .iter()
                .find(|(watchdog, _, _)| *watchdog == kind)
                .expect("the example tripped it");
            lines.push(format!(
                "  {}: {}{} in {}/{} runs; e.g. --variant {:.4},{:.4},{:.2} from frame {frame}: {what}\n",
                scenario.name,
                kind.describe(),
                if kind.known_gap() { " [known gap]" } else { "" },
                tripped.len(),
                mine.len(),
                example.along,
                example.across,
                example.strength,
            ));
        }
    }
    if lines.is_empty() {
        return "Watchdogs: nothing in any run made no physical sense.\n".to_string();
    }
    format!(
        "Watchdogs, what made no physical sense (reported, not judged; --why follows the thing named):\n{}",
        lines.concat()
    )
}

fn sweep_summary(scenario: &Scenario, runs: &[&SweepRun], custom: bool) -> String {
    let mut text = String::new();
    let in_band = runs.iter().filter(|run| run.violations.is_empty()).count();
    // The swing as tuned: unmoved, at the bones' own strength.
    let unmoved = runs
        .iter()
        .find(|run| run.along == 0.0 && run.across == 0.0 && run.strength == 1.0)
        .map_or("missing", |run| {
            if run.violations.is_empty() {
                "in band"
            } else {
                "out of band"
            }
        });
    if custom {
        text.push_str(&format!("{} ({} runs)\n", scenario.play, runs.len()));
    } else {
        text.push_str(&format!(
            "{}: in band {}/{} runs (unmoved swing {})\n",
            scenario.name,
            in_band,
            runs.len(),
            unmoved
        ));
    }
    let spread = |name: &str, value: &dyn Fn(&ScenarioResult) -> f64, decimals: usize| {
        let mut values: Vec<f64> = runs.iter().map(|run| value(&run.result)).collect();
        values.sort_by(|a, b| a.total_cmp(b));
        let median = values[values.len() / 2];
        format!(
            "{name} {:.*}..{:.*} (median {:.*})",
            decimals,
            values[0],
            decimals,
            values[values.len() - 1],
            decimals,
            median
        )
    };
    let lines = [
        spread("bones", &|r| r.bone_fractures as f64, 0),
        spread("ribs", &|r| r.rib_fractures as f64, 0),
        spread("skin", &|r| r.skin_tears as f64, 0),
        spread("muscle", &|r| r.muscle_tears as f64, 0),
        spread("bruises", &|r| r.contusion_events as f64, 0),
        spread("vessels", &|r| r.vessel_lacerations as f64, 0),
        spread("organ damage", &|r| r.max_organ_damage, 2),
        spread("reopens", &|r| r.wound_reopens as f64, 0),
        spread("blood loss", &|r| r.blood_loss, 3),
        spread("tool turn", &|r| r.max_tool_turn, 0),
        spread("snaps", &|r| r.tool_snaps as f64, 0),
        spread("contact toggles", &|r| r.contact_toggles as f64, 0),
        spread("free lag", &|r| r.max_free_lag, 0),
        spread("final lag", &|r| r.final_lag, 0),
        spread("point speed", &|r| r.max_point_speed, 0),
        spread("head turn", &|r| r.max_head_turn, 0),
        spread("fist knocked", &|r| r.max_fist_shift, 0),
        spread("fist off guard", &|r| r.final_fist_offset, 0),
        spread("fist dropped", &|r| r.final_fist_drop, 0),
        spread("cut open", &|r| r.cut_openings as f64, 0),
        spread("blood trail", &|r| r.blood_trail_length, 0),
    ];
    if !scenario.real.is_empty() {
        let tallies: Vec<String> = scenario
            .real
            .iter()
            .map(|check| {
                let hits = runs
                    .iter()
                    .filter(|run| check.outcome.happened(&run.result))
                    .count();
                format!(
                    "{} {hits}/{} (real life: {})",
                    check.outcome.describe(),
                    runs.len(),
                    check.often.name()
                )
            })
            .collect();
        text.push_str(&format!("  {}\n", tallies.join("; ")));
        // How close each checked bone came to breaking, as a share of its
        // strength, for tuning.
        let loads: Vec<String> = scenario
            .real
            .iter()
            .filter_map(|check| match check.outcome {
                rp::scenarios::Outcome::Breaks(part) => Some(part),
                _ => None,
            })
            .map(|part| {
                spread(
                    &format!("{} load", part.name()),
                    &|r: &ScenarioResult| r.part_peak_load[part.index()],
                    2,
                )
            })
            .collect();
        if !loads.is_empty() {
            text.push_str(&format!("  {}\n", loads.join(", ")));
        }
    }
    // Which bones broke, in how many runs.
    let broken: Vec<String> = rp::BonePart::ALL
        .iter()
        .filter_map(|part| {
            let count = runs
                .iter()
                .filter(|run| run.result.part_fractures[part.index()] > 0)
                .count();
            (count > 0).then(|| format!("{} in {count}", part.name()))
        })
        .collect();
    if !broken.is_empty() {
        text.push_str(&format!("  broken: {}\n", broken.join(", ")));
    }
    // A closed break's pieces belong inside the skin; how often one ends up
    // bare beside the limb (`closed_pieces_out`).
    let bare = runs
        .iter()
        .filter(|run| run.result.closed_pieces_out > 0)
        .count();
    if bare > 0 {
        text.push_str(&format!(
            "  a closed break's piece ends outside the skin in {bare}\n"
        ));
    }
    // A whole bone belongs inside its limb too (`whole_bones_out`).
    let stripped = runs
        .iter()
        .filter(|run| run.result.whole_bones_out > 0)
        .count();
    if stripped > 0 {
        text.push_str(&format!(
            "  a whole bone ends outside the skin in {stripped}\n"
        ));
    }
    text.push_str(&format!("  {}\n", lines[..5].join(", ")));
    text.push_str(&format!("  {}\n", lines[5..9].join(", ")));
    text.push_str(&format!("  {}\n", lines[9..].join(", ")));
    let mut misses: Vec<(&str, usize)> = Vec::new();
    for run in runs {
        for violation in &run.violations {
            let metric = violated_metric(violation);
            match misses.iter_mut().find(|(name, _)| *name == metric) {
                Some((_, count)) => *count += 1,
                None => misses.push((metric, 1)),
            }
        }
    }
    if !misses.is_empty() {
        let listed: Vec<String> = misses
            .iter()
            .map(|(metric, count)| format!("{metric} in {count}"))
            .collect();
        text.push_str(&format!("  out of band: {}\n", listed.join(", ")));
    }
    text
}

/// The metric named in a `scenario: metric=value outside a..b` violation.
fn violated_metric(violation: &str) -> &str {
    let after_name = violation
        .split_once(": ")
        .map_or(violation, |(_, rest)| rest);
    after_name.split('=').next().unwrap_or(after_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sweep's unmoved swing is the swing as tuned, unmoved and on bones
    /// of their own strength, not the first unmoved run it played: it was
    /// the run at 0.8 of their strength, so the report could call the tuned
    /// swing out of band when it was in, and the other way.
    #[test]
    fn the_unmoved_swing_is_the_tuned_one() {
        let scenario = scenarios()[0];
        let run = |strength: f64, violations: Vec<String>| SweepRun {
            scenario: 0,
            along: 0.0,
            across: 0.0,
            strength,
            result: ScenarioResult::default(),
            violations,
            flagged: Vec::new(),
        };
        let weaker = run(0.8, vec!["bone_pieces_out=1".to_string()]);
        let tuned = run(1.0, Vec::new());
        let summary = sweep_summary(&scenario, &[&weaker, &tuned], false);
        assert!(summary.contains("(unmoved swing in band)"), "{summary}");
    }
}
