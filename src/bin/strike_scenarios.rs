//! Plays the tuned strike scenarios from `realistic_physics::scenarios` and
//! checks the injuries against each scenario's bands.
//!
//! ```text
//! strike_scenarios [CSV]              play every scenario once: per-frame CSV,
//!                                     strike_summary.csv, strike_tuning_report.txt
//!   --only NAME[,NAME...]             only these scenarios
//!   --sweep                           replay each scenario with the swing moved a
//!                                     little along and across its path, and report
//!                                     how often it stays in band
//!                                     (strike_sweep.csv, strike_sweep_report.txt)
//!   --strike TOOL:U0,V0:U1,V1[:power=P][:frames=N][:settle=N]
//!                                     play one custom swing (body coordinates) and
//!                                     print its injuries and how steady the tool
//!                                     was; with --sweep, their spread
//!   --gesture TOOL:U,V[:STEP...]      the same for a gesture played the way the
//!                                     app is (steps: U,V/N, down, up, wait=N);
//!                                     writes strike_custom_frames.csv too
//!   --list                            print the scenarios and their plays
//! ```
//!
//! Outputs go next to the CSV path (default `output/strike_scenarios.csv`).

use realistic_physics as rp;
use rp::scenarios::{
    active_fluid_count, fastest_point_speed, free_fragment_count, run, scenarios,
    spinning_fragment_count, tool_axis, tool_lag, tool_name, tool_touching, Gesture, Play,
    Scenario, ScenarioResult, Strike, SCENARIO_HEIGHT, SCENARIO_WIDTH,
};
use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;

/// How far a sweep moves each swing along its path and across it, in body
/// heights: about one point spacing either way, as aim varies from swing to
/// swing.
const SWEEP_ALONG: [f64; 5] = [-0.010, -0.005, 0.0, 0.005, 0.010];
const SWEEP_ACROSS: [f64; 3] = [-0.005, 0.0, 0.005];

struct Options {
    csv_path: PathBuf,
    only: Vec<String>,
    sweep: bool,
    /// A custom swing or gesture to play instead of the scenarios.
    custom: Option<Play>,
    list: bool,
}

fn main() {
    let options = parse_options().unwrap_or_else(|message| {
        eprintln!("{message}");
        eprintln!(
            "usage: strike_scenarios [CSV] [--only NAME,...] [--sweep] [--strike SPEC | --gesture SPEC] [--list]"
        );
        process::exit(2);
    });
    let output_dir = options
        .csv_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    fs::create_dir_all(&output_dir).expect("create output directory");

    if options.list {
        for scenario in scenarios() {
            println!("{:<30} {}", scenario.name, scenario.play);
        }
        return;
    }

    let selected: Vec<Scenario> = match options.custom {
        Some(play) => vec![Scenario::custom(play)],
        None => {
            let all = scenarios();
            for name in &options.only {
                if !all.iter().any(|scenario| scenario.name == name) {
                    eprintln!("unknown scenario `{name}`; --list shows them");
                    process::exit(2);
                }
            }
            all.into_iter()
                .filter(|scenario| {
                    options.only.is_empty() || options.only.iter().any(|name| name == scenario.name)
                })
                .collect()
        }
    };

    if options.sweep {
        sweep(&selected, &output_dir, options.custom.is_some());
    } else if options.custom.is_some() {
        play_custom(&selected[0], &output_dir);
    } else {
        play_scenarios(&selected, &options.csv_path, &output_dir);
    }
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        csv_path: PathBuf::from("output/strike_scenarios.csv"),
        only: Vec::new(),
        sweep: false,
        custom: None,
        list: false,
    };
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sweep" => options.sweep = true,
            "--list" => options.list = true,
            "--only" => {
                let names = args.next().ok_or("--only needs scenario names")?;
                options
                    .only
                    .extend(names.split(',').map(|name| name.trim().to_string()));
            }
            "--strike" => {
                let spec = args.next().ok_or("--strike needs a swing")?;
                options.custom = Some(Play::Swing(Strike::parse(&spec)?, None));
            }
            "--gesture" => {
                let spec = args.next().ok_or("--gesture needs a gesture")?;
                options.custom = Some(Play::Gesture(Gesture::parse(&spec)?));
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option `{flag}`")),
            path => options.csv_path = PathBuf::from(path),
        }
    }
    Ok(options)
}

/// Plays each scenario once with the per-frame CSV, summary, and tuning report.
fn play_scenarios(selected: &[Scenario], csv_path: &Path, output_dir: &Path) {
    let summary_path = output_dir.join("strike_summary.csv");
    let report_path = output_dir.join("strike_tuning_report.txt");
    let mut csv = BufWriter::new(File::create(csv_path).expect("create strike CSV"));
    write_frame_header(&mut csv).expect("write frame header");
    let mut summary = BufWriter::new(File::create(&summary_path).expect("create summary"));
    writeln!(summary, "{SUMMARY_HEADER}").expect("write summary header");
    let mut warnings = Vec::new();

    for scenario in selected {
        let mut write_error = None;
        let mut last_positions = Vec::new();
        let (_, result) = run(
            scenario,
            SCENARIO_WIDTH,
            SCENARIO_HEIGHT,
            |frame, input, world| {
                if write_error.is_none() {
                    write_error =
                        write_frame(&mut csv, scenario, frame, input, world, &mut last_positions)
                            .err();
                }
            },
        );
        if let Some(error) = write_error {
            panic!("write strike CSV: {error}");
        }
        writeln!(summary, "{}", summary_fields(scenario, &result).join(","))
            .expect("write summary row");
        warnings.extend(scenario.violations(&result));
    }

    let mut report = BufWriter::new(File::create(&report_path).expect("create report"));
    if warnings.is_empty() {
        writeln!(report, "All strike scenarios are inside expected bands.").expect("write report");
    } else {
        for warning in &warnings {
            writeln!(report, "{warning}").expect("write report");
        }
    }
    println!("wrote {}", csv_path.display());
    println!("wrote {}", summary_path.display());
    println!("wrote {}", report_path.display());
    if warnings.is_empty() {
        println!("PASS: strike scenarios are inside expected bands");
    } else {
        println!("WARN: {} strike tuning warnings", warnings.len());
    }
}

/// Plays one custom swing or gesture and prints what it did, with its
/// frame-by-frame telemetry in `strike_custom_frames.csv`.
fn play_custom(scenario: &Scenario, output_dir: &Path) {
    let frames_path = output_dir.join("strike_custom_frames.csv");
    let mut frames = BufWriter::new(File::create(&frames_path).expect("create custom frames"));
    write_frame_header(&mut frames).expect("write frame header");
    let mut write_error = None;
    let mut last_positions = Vec::new();
    let (_, result) = run(
        scenario,
        SCENARIO_WIDTH,
        SCENARIO_HEIGHT,
        |frame, input, world| {
            if write_error.is_none() {
                write_error = write_frame(
                    &mut frames,
                    scenario,
                    frame,
                    input,
                    world,
                    &mut last_positions,
                )
                .err();
            }
        },
    );
    if let Some(error) = write_error {
        panic!("write custom frames: {error}");
    }
    println!("{}", scenario.play);
    println!("  {}", headline(&result));
    println!("  {}", steadiness(&result));
    let path = output_dir.join("strike_custom.csv");
    let mut out = BufWriter::new(File::create(&path).expect("create custom CSV"));
    writeln!(out, "{SUMMARY_HEADER}").expect("write custom CSV");
    writeln!(out, "{}", summary_fields(scenario, &result).join(",")).expect("write custom CSV");
    println!("wrote {}", path.display());
    println!("wrote {}", frames_path.display());
}

struct SweepRun {
    scenario: usize,
    along: f64,
    across: f64,
    result: ScenarioResult,
    violations: Vec<String>,
}

/// Replays each scenario with its swing moved by every combination of
/// `SWEEP_ALONG` and `SWEEP_ACROSS`, on all cores, and reports the spread.
fn sweep(selected: &[Scenario], output_dir: &Path, custom: bool) {
    let jobs: Vec<(usize, f64, f64)> = (0..selected.len())
        .flat_map(|index| {
            SWEEP_ALONG.iter().flat_map(move |&along| {
                SWEEP_ACROSS
                    .iter()
                    .map(move |&across| (index, along, across))
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
                let Some(&(index, along, across)) = jobs.get(job) else {
                    break;
                };
                let scenario = selected[index].shifted(along, across);
                let (_, result) = run(&scenario, SCENARIO_WIDTH, SCENARIO_HEIGHT, |_, _, _| {});
                let violations = scenario.violations(&result);
                finished.lock().expect("sweep results").push((
                    job,
                    SweepRun {
                        scenario: index,
                        along,
                        across,
                        result,
                        violations,
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
    writeln!(csv, "along,across,in_band,violations,{SUMMARY_HEADER}").expect("write sweep CSV");
    for run in &runs {
        let scenario = &selected[run.scenario];
        let failed: Vec<&str> = run
            .violations
            .iter()
            .map(|violation| violated_metric(violation))
            .collect();
        writeln!(
            csv,
            "{:.4},{:.4},{},{},{}",
            run.along,
            run.across,
            u8::from(run.violations.is_empty()),
            failed.join(";"),
            summary_fields(scenario, &run.result).join(",")
        )
        .expect("write sweep CSV");
    }

    let mut report = String::new();
    for (index, scenario) in selected.iter().enumerate() {
        let mine: Vec<&SweepRun> = runs.iter().filter(|run| run.scenario == index).collect();
        report.push_str(&sweep_summary(scenario, &mine, custom));
    }
    let report_path = output_dir.join("strike_sweep_report.txt");
    fs::write(&report_path, &report).expect("write sweep report");
    print!("{report}");
    println!("wrote {}", csv_path.display());
    println!("wrote {}", report_path.display());
}

fn sweep_summary(scenario: &Scenario, runs: &[&SweepRun], custom: bool) -> String {
    let mut text = String::new();
    let in_band = runs.iter().filter(|run| run.violations.is_empty()).count();
    let unmoved = runs
        .iter()
        .find(|run| run.along == 0.0 && run.across == 0.0)
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
    ];
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

/// How steady the tool was, in one line.
fn steadiness(result: &ScenarioResult) -> String {
    format!(
        "tool turn max {:.0} deg/step ({:.0} deg in all), snaps {}, contact toggles {}, trails hand by up to {:.0} px in the air and {:.0} px at the end, fastest tissue {:.0} px/s",
        result.max_tool_turn,
        result.tool_turning,
        result.tool_snaps,
        result.contact_toggles,
        result.max_free_lag,
        result.final_lag,
        result.max_point_speed
    )
}

fn headline(result: &ScenarioResult) -> String {
    format!(
        "bones {} (ribs {}), skin {}, muscle {}, bruises {}, vessels {}, organ damage {:.2}, reopens {}, blood loss {:.3}",
        result.bone_fractures,
        result.rib_fractures,
        result.skin_tears,
        result.muscle_tears,
        result.contusion_events,
        result.vessel_lacerations,
        result.max_organ_damage,
        result.wound_reopens,
        result.blood_loss
    )
}

/// Columns of a summary row: the scenario, then its result.
const SUMMARY_HEADER: &str = "scenario,region,intent,tool,tissue_contacts,bone_contacts,skin_tears,muscle_tears,muscle_fiber_tears,contusion_events,tissue_fatigue_events,tissue_plastic_events,tear_propagations,muscle_cut_transfers,muscle_crush_ruptures,cavity_pressure_events,cavity_ruptures,organ_damage_events,organ_penetrations,rib_organ_punctures,organ_ruptures,skin_flap_detachments,vessel_lacerations,fragment_vessel_lacerations,wound_reopens,max_active_contusions,detachments,bone_detachments,bone_joint_breaks,bone_joint_subluxations,joint_ligament_damage_events,bone_fractures,rib_fractures,fracture_marrow_sources,final_bones,fluid_emitted,wound_fluid,blood_loss,final_blood_volume,final_blood_turgor,blood_stain_deposits,max_active_blood_stains,opened_wounds,max_active_wounds,wound_leaks,fragment_hits,fragment_tears,fragment_skin_punctures,fragment_bone_contacts,fragment_bone_damping_events,fragment_bone_resting_contacts,fragment_pair_contacts,fragment_pair_damping_events,fragment_pair_resting_contacts,fragment_floor_contacts,fragment_floor_resting_contacts,post_fracture_joint_corrections,max_impact,max_bone_load,max_point_load,max_depth,max_fragment_depth,max_fragment_impulse,max_fragment_overlap,max_post_fracture_joint_stretch,max_post_fracture_joint_angle,max_bone_joint_subluxation,max_wound_pressure,max_wound_clot,max_cavity_pressure,max_cavity_collapse,max_organ_damage,max_contusion,max_tissue_softening,max_tissue_fatigue,max_tissue_plasticity,max_bone_angular_speed,final_free_fragments,final_spinning_fragments,final_sleeping_fragments,max_active_fragments,max_sleeping_fragments,fragment_sleep_events,fragment_wake_events,fragment_budget_skips,fracture_budget_blocks,fragment_bone_checks,fragment_bone_budget_skips,fragment_pair_checks,fragment_pair_budget_skips,fragment_tissue_checks,fragment_tissue_budget_skips,fluid_budget_replacements,blood_stain_budget_replacements,wound_budget_replacements,max_solver_iterations,max_tool_turn,tool_snaps,tool_turning,contact_toggles,max_point_speed,max_free_lag,internal_bleeding,final_lag";

fn summary_fields(scenario: &Scenario, result: &ScenarioResult) -> Vec<String> {
    vec![
        scenario.name.to_string(),
        scenario.region.to_string(),
        scenario.intent.to_string(),
        tool_name(scenario.play.tool()).to_string(),
        result.tissue_contacts.to_string(),
        result.bone_contacts.to_string(),
        result.skin_tears.to_string(),
        result.muscle_tears.to_string(),
        result.muscle_fiber_tears.to_string(),
        result.contusion_events.to_string(),
        result.tissue_fatigue_events.to_string(),
        result.tissue_plastic_events.to_string(),
        result.tear_propagations.to_string(),
        result.muscle_cut_transfers.to_string(),
        result.muscle_crush_ruptures.to_string(),
        result.cavity_pressure_events.to_string(),
        result.cavity_ruptures.to_string(),
        result.organ_damage_events.to_string(),
        result.organ_penetrations.to_string(),
        result.rib_organ_punctures.to_string(),
        result.organ_ruptures.to_string(),
        result.skin_flap_detachments.to_string(),
        result.vessel_lacerations.to_string(),
        result.fragment_vessel_lacerations.to_string(),
        result.wound_reopens.to_string(),
        result.max_active_contusions.to_string(),
        result.detachments.to_string(),
        result.bone_detachments.to_string(),
        result.bone_joint_breaks.to_string(),
        result.bone_joint_subluxations.to_string(),
        result.joint_ligament_damage_events.to_string(),
        result.bone_fractures.to_string(),
        result.rib_fractures.to_string(),
        result.fracture_marrow_sources.to_string(),
        result.final_bones.to_string(),
        result.fluid_emitted.to_string(),
        result.wound_fluid.to_string(),
        format!("{:.5}", result.blood_loss),
        format!("{:.5}", result.final_blood_volume),
        format!("{:.5}", result.final_blood_turgor),
        result.blood_stain_deposits.to_string(),
        result.max_active_blood_stains.to_string(),
        result.opened_wounds.to_string(),
        result.max_active_wounds.to_string(),
        result.wound_leaks.to_string(),
        result.fragment_hits.to_string(),
        result.fragment_tears.to_string(),
        result.fragment_skin_punctures.to_string(),
        result.fragment_bone_contacts.to_string(),
        result.fragment_bone_damping_events.to_string(),
        result.fragment_bone_resting_contacts.to_string(),
        result.fragment_pair_contacts.to_string(),
        result.fragment_pair_damping_events.to_string(),
        result.fragment_pair_resting_contacts.to_string(),
        result.fragment_floor_contacts.to_string(),
        result.fragment_floor_resting_contacts.to_string(),
        result.post_fracture_joint_corrections.to_string(),
        format!("{:.3}", result.max_impact),
        format!("{:.3}", result.max_bone_load),
        format!("{:.3}", result.max_point_load),
        format!("{:.3}", result.max_depth),
        format!("{:.3}", result.max_fragment_depth),
        format!("{:.3}", result.max_fragment_impulse),
        format!("{:.3}", result.max_fragment_overlap),
        format!("{:.3}", result.max_post_fracture_joint_stretch),
        format!("{:.3}", result.max_post_fracture_joint_angle),
        format!("{:.3}", result.max_bone_joint_subluxation),
        format!("{:.3}", result.max_wound_pressure),
        format!("{:.3}", result.max_wound_clot),
        format!("{:.3}", result.max_cavity_pressure),
        format!("{:.3}", result.max_cavity_collapse),
        format!("{:.3}", result.max_organ_damage),
        format!("{:.3}", result.max_contusion),
        format!("{:.3}", result.max_tissue_softening),
        format!("{:.3}", result.max_tissue_fatigue),
        format!("{:.3}", result.max_tissue_plasticity),
        format!("{:.3}", result.max_bone_angular_speed),
        result.final_free_fragments.to_string(),
        result.final_spinning_fragments.to_string(),
        result.final_sleeping_fragments.to_string(),
        result.max_active_fragments.to_string(),
        result.max_sleeping_fragments.to_string(),
        result.fragment_sleep_events.to_string(),
        result.fragment_wake_events.to_string(),
        result.fragment_budget_skips.to_string(),
        result.fracture_budget_blocks.to_string(),
        result.fragment_bone_checks.to_string(),
        result.fragment_bone_budget_skips.to_string(),
        result.fragment_pair_checks.to_string(),
        result.fragment_pair_budget_skips.to_string(),
        result.fragment_tissue_checks.to_string(),
        result.fragment_tissue_budget_skips.to_string(),
        result.fluid_budget_replacements.to_string(),
        result.blood_stain_budget_replacements.to_string(),
        result.wound_budget_replacements.to_string(),
        result.max_solver_iterations.to_string(),
        format!("{:.3}", result.max_tool_turn),
        result.tool_snaps.to_string(),
        format!("{:.3}", result.tool_turning),
        result.contact_toggles.to_string(),
        format!("{:.3}", result.max_point_speed),
        format!("{:.3}", result.max_free_lag),
        result.internal_bleeding.to_string(),
        format!("{:.3}", result.final_lag),
    ]
}

fn write_frame_header(csv: &mut dyn Write) -> std::io::Result<()> {
    writeln!(csv, "scenario,region,intent,tool,frame,striker_x,striker_y,striker_speed,impact,tissue_contacts,bone_contacts,max_depth,max_point_load,max_bone_load,fractures,skin_tears,muscle_tears,muscle_fiber_tears_frame,total_muscle_fiber_tears,contusion_events_frame,active_contusions,total_contusion_events,max_contusion,max_tissue_softening,tissue_fatigue_events_frame,total_tissue_fatigue_events,max_tissue_fatigue,tissue_plastic_events_frame,total_tissue_plastic_events,max_tissue_plasticity,tear_propagations_frame,total_tear_propagations,muscle_cut_transfers_frame,total_muscle_cut_transfers,muscle_crush_ruptures_frame,total_muscle_crush_ruptures,cavity_pressure_events_frame,total_cavity_pressure_events,cavity_ruptures_frame,total_cavity_ruptures,organ_damage_events_frame,total_organ_damage_events,organ_penetrations_frame,total_organ_penetrations,rib_organ_punctures_frame,total_rib_organ_punctures,organ_ruptures_frame,total_organ_ruptures,skin_flap_detachments_frame,total_skin_flap_detachments,vessel_lacerations_frame,total_vessel_lacerations,fragment_vessel_lacerations_frame,total_fragment_vessel_lacerations,wound_reopens_frame,total_wound_reopens,detachments,bone_detachments,bone_joint_breaks,bone_joint_subluxations_frame,total_bone_joint_subluxations,joint_ligament_damage_frame,total_joint_ligament_damage,max_bone_joint_subluxation,bone_fractures,rib_fractures_frame,total_rib_fractures,fracture_marrow_sources,fluid_emitted_frame,active_fluids,total_fluid,blood_stain_deposits_frame,active_blood_stains,total_blood_stain_deposits,opened_wounds,active_wounds,wound_leaks,wound_fluid,blood_loss,blood_volume,blood_turgor,max_wound_pressure,max_wound_clot,max_cavity_pressure,max_cavity_collapse,max_organ_damage,fragment_contacts,fragment_tears,fragment_skin_punctures_frame,total_fragment_skin_punctures,fragment_bone_contacts,fragment_bone_damping_events,fragment_bone_resting_contacts,fragment_pair_contacts,fragment_pair_damping_events,fragment_pair_resting_contacts,fragment_floor_contacts,fragment_floor_resting_contacts,max_fragment_depth,max_fragment_impulse,max_fragment_overlap,post_fracture_joint_corrections,max_post_fracture_joint_stretch,max_post_fracture_joint_angle,fragment_hits,fragment_tissue_tears,max_bone_angular_speed,free_fragments,spinning_fragments,active_fragments,sleeping_fragments,fragment_sleep_events,fragment_wake_events,fragment_budget_skips,fracture_budget_blocks,fragment_bone_checks,fragment_bone_budget_skips,fragment_pair_checks,fragment_pair_budget_skips,fragment_tissue_checks,fragment_tissue_budget_skips,fluid_budget_replacements,blood_stain_budget_replacements,wound_budget_replacements,solver_iterations,tool_down,tool_touching,tool_axis_deg,tool_x,tool_y,max_point_speed,hand_x,hand_y,tool_lag,reach_flesh,reach_torn_away,reach_bones,reach_spine")
}

fn write_frame(
    csv: &mut dyn Write,
    scenario: &Scenario,
    frame: i32,
    input: &rp::InputState,
    world: &rp::World,
    last_positions: &mut Vec<rp::Vec2>,
) -> std::io::Result<()> {
    let point_speed = fastest_point_speed(world, last_positions);
    last_positions.clear();
    last_positions.extend(world.points().iter().map(|point| point.position));
    let debug = world.debug();
    let stats = world.stats();
    let reach = world.tool_reach();
    let fields = [
        scenario.name.to_string(),
        scenario.region.to_string(),
        scenario.intent.to_string(),
        tool_name(debug.tool).to_string(),
        frame.to_string(),
        format!("{:.3}", debug.striker_position.x),
        format!("{:.3}", debug.striker_position.y),
        format!("{:.3}", debug.striker_speed),
        format!("{:.3}", debug.impact),
        debug.tissue_contacts.to_string(),
        debug.bone_contacts.to_string(),
        format!("{:.3}", debug.max_depth),
        format!("{:.3}", debug.max_point_load),
        format!("{:.3}", debug.max_bone_load),
        debug.fractures.to_string(),
        stats.broken_skin.to_string(),
        stats.broken_muscle.to_string(),
        debug.muscle_fiber_tears.to_string(),
        stats.muscle_fiber_tears.to_string(),
        debug.contusion_events.to_string(),
        debug.active_contusions.to_string(),
        stats.contusion_events.to_string(),
        format!("{:.3}", debug.max_contusion),
        format!("{:.3}", debug.max_tissue_softening),
        debug.tissue_fatigue_events.to_string(),
        stats.tissue_fatigue_events.to_string(),
        format!("{:.3}", debug.max_tissue_fatigue),
        debug.tissue_plastic_events.to_string(),
        stats.tissue_plastic_events.to_string(),
        format!("{:.3}", debug.max_tissue_plasticity),
        debug.tear_propagations.to_string(),
        stats.tear_propagations.to_string(),
        debug.muscle_cut_transfers.to_string(),
        stats.muscle_cut_transfers.to_string(),
        debug.muscle_crush_ruptures.to_string(),
        stats.muscle_crush_ruptures.to_string(),
        debug.cavity_pressure_events.to_string(),
        stats.cavity_pressure_events.to_string(),
        debug.cavity_ruptures.to_string(),
        stats.cavity_ruptures.to_string(),
        debug.organ_damage_events.to_string(),
        stats.organ_damage_events.to_string(),
        debug.organ_penetrations.to_string(),
        stats.organ_penetrations.to_string(),
        debug.rib_organ_punctures.to_string(),
        stats.rib_organ_punctures.to_string(),
        debug.organ_ruptures.to_string(),
        stats.organ_ruptures.to_string(),
        debug.skin_flap_detachments.to_string(),
        stats.skin_flap_detachments.to_string(),
        debug.vessel_lacerations.to_string(),
        stats.vessel_lacerations.to_string(),
        debug.fragment_vessel_lacerations.to_string(),
        stats.fragment_vessel_lacerations.to_string(),
        debug.wound_reopens.to_string(),
        stats.wound_reopens.to_string(),
        stats.broken_attachments.to_string(),
        stats.broken_bone_attachments.to_string(),
        stats.broken_bone_joints.to_string(),
        debug.bone_joint_subluxations.to_string(),
        stats.bone_joint_subluxations.to_string(),
        debug.joint_ligament_damage_events.to_string(),
        stats.joint_ligament_damage_events.to_string(),
        format!("{:.3}", debug.max_bone_joint_subluxation),
        stats.fractured_bones.to_string(),
        debug.rib_fractures.to_string(),
        stats.fractured_ribs.to_string(),
        stats.fracture_marrow_sources.to_string(),
        debug.fluid_emitted.to_string(),
        active_fluid_count(world).to_string(),
        stats.emitted_fluid_particles.to_string(),
        debug.blood_stain_deposits.to_string(),
        debug.active_blood_stains.to_string(),
        stats.blood_stain_deposits.to_string(),
        stats.opened_wounds.to_string(),
        debug.active_wounds.to_string(),
        debug.wound_leaks.to_string(),
        stats.wound_fluid_particles.to_string(),
        format!("{:.5}", stats.blood_loss),
        format!("{:.5}", world.blood_volume_fraction()),
        format!("{:.5}", world.blood_turgor_scale()),
        format!("{:.3}", debug.max_wound_pressure),
        format!("{:.3}", debug.max_wound_clot),
        format!("{:.3}", debug.max_cavity_pressure),
        format!("{:.3}", debug.max_cavity_collapse),
        format!("{:.3}", debug.max_organ_damage),
        debug.fragment_contacts.to_string(),
        debug.fragment_tears.to_string(),
        debug.fragment_skin_punctures.to_string(),
        stats.fragment_skin_punctures.to_string(),
        debug.fragment_bone_contacts.to_string(),
        debug.fragment_bone_damping_events.to_string(),
        debug.fragment_bone_resting_contacts.to_string(),
        debug.fragment_pair_contacts.to_string(),
        debug.fragment_pair_damping_events.to_string(),
        debug.fragment_pair_resting_contacts.to_string(),
        debug.fragment_floor_contacts.to_string(),
        debug.fragment_floor_resting_contacts.to_string(),
        format!("{:.3}", debug.max_fragment_depth),
        format!("{:.3}", debug.max_fragment_impulse),
        format!("{:.3}", debug.max_fragment_overlap),
        debug.post_fracture_joint_corrections.to_string(),
        format!("{:.3}", debug.max_post_fracture_joint_stretch),
        format!("{:.3}", debug.max_post_fracture_joint_angle),
        stats.fragment_tissue_hits.to_string(),
        stats.fragment_tissue_tears.to_string(),
        format!("{:.3}", debug.max_bone_angular_speed),
        free_fragment_count(world).to_string(),
        spinning_fragment_count(world).to_string(),
        debug.active_fragments.to_string(),
        debug.sleeping_fragments.to_string(),
        debug.fragment_sleep_events.to_string(),
        debug.fragment_wake_events.to_string(),
        debug.fragment_budget_skips.to_string(),
        debug.fracture_budget_blocks.to_string(),
        debug.fragment_bone_checks.to_string(),
        debug.fragment_bone_budget_skips.to_string(),
        debug.fragment_pair_checks.to_string(),
        debug.fragment_pair_budget_skips.to_string(),
        debug.fragment_tissue_checks.to_string(),
        debug.fragment_tissue_budget_skips.to_string(),
        debug.fluid_budget_replacements.to_string(),
        debug.blood_stain_budget_replacements.to_string(),
        debug.wound_budget_replacements.to_string(),
        debug.solver_iterations.to_string(),
        u8::from(input.down).to_string(),
        u8::from(tool_touching(world)).to_string(),
        tool_axis(world).map_or(String::new(), |axis| {
            format!("{:.2}", axis.y.atan2(axis.x).to_degrees())
        }),
        format!("{:.3}", world.tool_position().x),
        format!("{:.3}", world.tool_position().y),
        format!("{:.1}", point_speed),
        format!("{:.3}", input.x),
        format!("{:.3}", input.y),
        format!("{:.3}", tool_lag(world, input)),
        reach.flesh.to_string(),
        reach.torn_away.to_string(),
        reach.bones.to_string(),
        reach.spine.to_string(),
    ];
    writeln!(csv, "{}", fields.join(","))
}
