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
//!                                     app is (steps: U,V/N, down, up, wait=N,
//!                                     punch=, lean=, and with fighters, two
//!                                     fighters, punch2= and lean2=);
//!                                     writes strike_custom_frames.csv too
//!   --sound DIR                       instead, write what each scenario (or the
//!                                     custom swing) sounds like in the app as
//!                                     DIR/NAME.wav, list its sounds and levels
//!                                     in DIR/sound_report.txt, and write each
//!                                     clip of the sound bank to DIR/bank
//!   --variant ALONG,ACROSS,STRENGTH   play the scenarios as one run of the sweep
//!                                     plays them: the swing moved ALONG and ACROSS
//!                                     its path, on bones STRENGTH times as strong
//!                                     (the sweep CSV's first columns), to look
//!                                     into a run the sweep found out of band
//!   --mirrored                        play the selected scenarios mirrored left
//!                                     to right, as the moved-body tests do,
//!                                     to look into a run they found differs
//!   --fastest SPEED                   also print each frame on which some tissue
//!                                     moved faster than SPEED px/s: which point,
//!                                     its layer, weight, and what holds it
//!   --trace point:N[:FROM-TO]         also print what each phase of each step,
//!   --trace bone:N[:FROM-TO]          substep by substep, did to tissue point N
//!                                     or bone N (its middle), on frames FROM to
//!                                     TO: how far it moved it and how it changed
//!                                     its speed, to find what throws it
//!   --stance N                        also print, every N frames, how the body
//!                                     holds its stance: each joint's turn off
//!                                     the guard and its muscles' torque, the
//!                                     hips' and fists' offsets, the flesh's
//!                                     sag off its bones, and the fastest tissue
//!   --journal                         also print what happened, frame by frame:
//!                                     each injury, where, what did it, and the
//!                                     measure that crossed its rule's limit
//!   --why point:N[:FROM-TO]           instead, print what moved point N, bone N
//!   --why bone:N[:FROM-TO]            (its middle and its angle), fiber N (its
//!   --why spring:N[:FROM-TO]          two ends apart), joint N (its bend), or
//!   --why joint:N[:FROM-TO]           triangle N (a corner across its far
//!   --why triangle:N[:FROM-TO]        side) over frames FROM to TO: each
//!                                     phase's share beyond what it does at
//!                                     rest, and the frames that moved it most
//!   --moved                           instead, the moved-body tests: replay each
//!                                     scenario's sweep with the body mirrored and
//!                                     moved across the window, report where the
//!                                     blows hurt it differently, and let a body
//!                                     fall and drift free (strike_moved_report.txt)
//!   --twin mirrored|mirrored-body|across[=PX]
//!                                     instead, play each selected scenario
//!                                     beside a twin of the body mirrored left
//!                                     to right (the same body, or built as
//!                                     its mirror image as a second fighter
//!                                     is), or moved PX px across (208 by
//!                                     default), compare them frame by frame,
//!                                     and print, phase by phase, where they
//!                                     first moved things differently: the
//!                                     rule that is lopsided or reads where
//!                                     the body stands; --twin-frame N explains
//!                                     frame N instead
//!   --random N [--seed S] [--tools T,...]
//!                                     instead, play N random blows: any tool
//!                                     (bat, hammer, and knife, or those --tools
//!                                     names, fist among them), through any point
//!                                     of skin from any side at any speed, or
//!                                     pressed, held, and dragged; report the
//!                                     rules every play follows that they broke
//!                                     and what the watchdogs flagged, each with
//!                                     a play to replay (random_blows_report.txt,
//!                                     random_blows.csv)
//!   --set NAME=VALUE[,NAME=VALUE...]  change any setting of `Materials` from its
//!                                     default for every world played (gravity,
//!                                     bone_joint_hertz, ...), to try a value
//!                                     without rebuilding; repeatable
//!   --quick                           a quick sweep: 9 runs a scenario (3 along,
//!                                     3 bone strengths) instead of 45, to see
//!                                     which way a change goes in a fifth of the
//!                                     time
//!   --diff A B                        set two sweeps side by side (folders that
//!                                     hold their strike_sweep_runs.csv): every
//!                                     band, real-life check, watchdog, and main
//!                                     number that moved, with how likely chance
//!                                     is to move it as far (sweep_diff.txt)
//!   --grid NAME=V1,V2,...             instead, play the scenarios (or the custom
//!                                     play) over every combination of the values
//!                                     given, repeatable: NAME a setting, as
//!                                     --set takes, or a blank {NAME} in the
//!                                     custom play, which also takes sums such as
//!                                     {60-lead}; one run a row, or 9 with
//!                                     --quick and 45 with --sweep; prints a
//!                                     table, each row set against the first by
//!                                     how unlikely chance is to make the
//!                                     difference (strike_grid.txt,
//!                                     strike_grid.csv, each row's runs in
//!                                     grid/rowNN for --diff)
//!   --dump WHAT[:FRAME]               lay the body out in numbers on a frame of
//!                                     the play (0, the start, by default): bones
//!                                     where they lie and how far from where they
//!                                     were built, joints turned and slipped,
//!                                     tendons against their slack, arteries,
//!                                     flesh slid off each bone; WHAT is bones,
//!                                     joints, tendons, vessels, ties, flesh=N
//!                                     (the flesh held to bone N), or all
//!   --mesh U,V[,R][:FRAME]            draw the skin's and the muscle's triangles
//!                                     round (U, V), R body heights either way
//!                                     (0.04), on a frame of the play, slivers
//!                                     and triangles turned over marked
//!                                     (output/mesh_NAME_FRAME.svg)
//!   --record FILE.rrd                 record a lone scenario or custom play for
//!                                     the Rerun viewer: every point, fiber,
//!                                     bone, joint, the tool, and blood on a
//!                                     timeline, the injuries and watchdogs as a
//!                                     log, and the fists, joints, bone loads,
//!                                     and flesh slid off each bone plotted
//!                                     (build with `--features record`;
//!                                     tools\record.ps1 builds, records, opens)
//!   --shrink                          with --random, shrink the first play that
//!                                     broke each rule or tripped each watchdog to
//!                                     the simplest like it that still does:
//!                                     slower, shorter, more level, no drag
//!                                     (proptest's binary search)
//!   --list                            print the scenarios and their plays
//!   --phases                          instead, time each phase of a step over the
//!                                     selected scenarios (or the custom play) and
//!                                     print where the time goes (strike_phases.txt)
//! ```
//!
//! Every run is watched by the watchdogs (`realistic_physics::inspect`): a
//! lone scenario or custom play prints what they flagged and what moved the
//! first thing each flagged, and the sweep counts the runs each tripped in.
//!
//! Outputs go next to the CSV path (default `output/strike_scenarios.csv`).

mod blanks;
mod bodies;
mod diff;
mod dump;
mod grid;
mod inspect;
mod moved;
mod random;
mod record;
mod summary;
mod sweep;
mod twin;
mod watch;

use bodies::*;
use dump::*;
use inspect::*;
use moved::*;
use random::*;
use realistic_physics as rp;
use record::Recorder;
use rp::scenarios::{
    active_fluid_count, fastest_point_speed, free_fragment_count, inside_the_skin, run_on,
    scenarios, spinning_fragment_count, tool_axis, tool_lag, tool_name, tool_touching, Gesture,
    Play, Scenario, ScenarioResult, Strike, SCENARIO_HEIGHT, SCENARIO_WIDTH,
};
use rp::sound::{wav_bytes, CueKind, Foley, Heard, Mixdown, SoundBank};
use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Instant;
use summary::*;
use sweep::*;
use watch::*;

struct Options {
    csv_path: PathBuf,
    only: Vec<String>,
    sweep: bool,
    /// With the sweep, 9 runs a scenario instead of 45.
    quick: bool,
    /// Two sweeps to set side by side instead of playing.
    diff: Option<(PathBuf, PathBuf)>,
    /// Record a lone scenario or custom play for the Rerun viewer.
    record: Option<PathBuf>,
    /// With the random blows, shrink the first play to break each rule.
    shrink: bool,
    /// A custom swing or gesture to play instead of the scenarios.
    custom: Option<Play>,
    /// Where to write what each one sounds like, instead of the CSVs.
    sound: Option<PathBuf>,
    list: bool,
    /// Time each phase of a step instead of checking injuries.
    phases: bool,
    /// One sweep run to play: along, across, and bone strength.
    variant: Option<(f64, f64, f64)>,
    /// Play the scenarios mirrored left to right.
    mirrored: bool,
    /// What to watch while the scenarios play.
    watch: Watch,
    /// What moved a point, bone, fiber, or joint, instead of playing.
    why: Option<rp::inspect::Why>,
    /// The moved-body tests instead of playing.
    moved: bool,
    /// Twin runs instead of playing: each scenario beside a mirrored or
    /// moved copy, compared phase by phase, explaining this frame.
    twin: Option<twin::TwinWay>,
    twin_frame: Option<i32>,
    /// Random blows to play instead, from a seed, with these tools.
    random: Option<usize>,
    seed: u64,
    tools: Vec<rp::ToolMode>,
    /// Settings changed from the defaults for every world, `NAME=VALUE`.
    settings: Vec<String>,
    /// The custom swing or gesture as written, which a grid fills in.
    written: Option<blanks::Written>,
    /// The axes of a grid to play instead: settings, or blanks in the play.
    grid: Vec<grid::Axis>,
}

fn main() {
    let options = parse_options().unwrap_or_else(|message| {
        eprintln!("{message}");
        eprintln!(
            "usage: strike_scenarios [CSV] [--only NAME,...] [--sweep] [--strike SPEC | --gesture SPEC] [--sound DIR] [--list]"
        );
        eprintln!(
            "       strike_scenarios [CSV] [--only NAME,...] [--strike SPEC | --gesture SPEC] --phases"
        );
        eprintln!(
            "       strike_scenarios [CSV] [--only NAME,... | --strike SPEC | --gesture SPEC] --grid NAME=V1,V2,... [--grid ...] [--quick | --sweep]"
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
    let settings = rp::scenarios::settings_used();
    if !settings.is_empty() {
        println!(
            "settings changed from the defaults: {}",
            settings.join(", ")
        );
    }

    if options.list {
        for scenario in scenarios() {
            println!("{:<30} {}", scenario.name, scenario.play);
        }
        return;
    }
    if let Some((a, b)) = &options.diff {
        if let Err(message) = diff::diff_sweeps(a, b, &output_dir) {
            eprintln!("{message}");
            process::exit(2);
        }
        return;
    }
    if let Some(count) = options.random {
        random_blows(
            count,
            options.seed,
            &options.tools,
            &output_dir,
            options.shrink,
        );
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
    if !options.grid.is_empty() {
        let plays = if options.sweep {
            grid::Plays::Sweep(options.quick)
        } else {
            grid::Plays::Once(options.variant.unwrap_or((0.0, 0.0, 1.0)))
        };
        if let Err(message) = grid::grid(
            &options.grid,
            options.written.as_ref(),
            &selected,
            plays,
            &output_dir,
        ) {
            eprintln!("{message}");
            process::exit(2);
        }
        return;
    }
    let selected: Vec<Scenario> = match options.variant {
        Some((along, across, strength)) => selected
            .iter()
            .map(|scenario| scenario.shifted(along, across).with_bone_strength(strength))
            .collect(),
        None => selected,
    };
    // Mirrored after the variant's shift, as the moved-body tests mirror
    // each run of the sweep.
    let selected: Vec<Scenario> = if options.mirrored {
        selected.iter().map(Scenario::mirrored).collect()
    } else {
        selected
    };
    if options.phases {
        time_phases(&selected, &output_dir);
        return;
    }
    if let Some(why) = options.why {
        for scenario in &selected {
            explain(scenario, why);
        }
        return;
    }
    if options.moved {
        moved_body_tests(&selected, &output_dir);
        return;
    }
    if let Some(way) = options.twin {
        twin::twin_runs(&selected, way, options.twin_frame);
        return;
    }

    if let Some(dir) = &options.sound {
        listen(&selected, dir);
    } else if options.sweep {
        sweep(
            &selected,
            &output_dir,
            options.custom.is_some(),
            options.quick,
        );
    } else if options.custom.is_some() {
        play_custom(
            &selected[0],
            &output_dir,
            options.watch,
            options.record.as_deref(),
        );
    } else {
        play_scenarios(
            &selected,
            &options.csv_path,
            &output_dir,
            options.watch,
            options.record.as_deref(),
        );
    }
}

/// Reads `--trace point:N[:FROM-TO]` or `--trace bone:N[:FROM-TO]`.
fn parse_trace(spec: &str) -> Result<(rp::Traced, i32, i32), String> {
    let wrong = || format!("--trace takes point:N[:FROM-TO] or bone:N[:FROM-TO], got `{spec}`");
    let mut parts = spec.split(':');
    let kind = parts.next().ok_or_else(wrong)?;
    let index: usize = parts
        .next()
        .ok_or_else(wrong)?
        .parse()
        .map_err(|_| wrong())?;
    let traced = match kind {
        "point" => rp::Traced::Point(index),
        "bone" => rp::Traced::Bone(index),
        _ => return Err(wrong()),
    };
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
    if parts.next().is_some() {
        return Err(wrong());
    }
    Ok((traced, from, to))
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        csv_path: PathBuf::from("output/strike_scenarios.csv"),
        only: Vec::new(),
        sweep: false,
        custom: None,
        sound: None,
        list: false,
        phases: false,
        variant: None,
        mirrored: false,
        watch: Watch::default(),
        why: None,
        moved: false,
        twin: None,
        twin_frame: None,
        random: None,
        seed: 1,
        tools: vec![
            rp::ToolMode::Blunt,
            rp::ToolMode::Heavy,
            rp::ToolMode::Sharp,
        ],
        settings: Vec::new(),
        quick: false,
        diff: None,
        record: None,
        shrink: false,
        written: None,
        grid: Vec::new(),
    };
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sweep" => options.sweep = true,
            "--quick" => {
                options.sweep = true;
                options.quick = true;
            }
            "--diff" => {
                let a = args.next().ok_or("--diff needs two sweeps' folders")?;
                let b = args.next().ok_or("--diff needs two sweeps' folders")?;
                options.diff = Some((PathBuf::from(a), PathBuf::from(b)));
            }
            "--phases" => options.phases = true,
            "--list" => options.list = true,
            "--sound" => {
                options.sound = Some(PathBuf::from(args.next().ok_or("--sound needs a folder")?));
            }
            "--only" => {
                let names = args.next().ok_or("--only needs scenario names")?;
                options
                    .only
                    .extend(names.split(',').map(|name| name.trim().to_string()));
            }
            "--strike" => {
                let spec = args.next().ok_or("--strike needs a swing")?;
                options.written = Some(blanks::Written::Strike(spec));
            }
            "--gesture" => {
                let spec = args.next().ok_or("--gesture needs a gesture")?;
                options.written = Some(blanks::Written::Gesture(spec));
            }
            "--variant" => {
                let spec = args.next().ok_or("--variant needs ALONG,ACROSS,STRENGTH")?;
                let numbers: Vec<f64> = spec
                    .split(',')
                    .map(|part| part.trim().parse::<f64>())
                    .collect::<Result<_, _>>()
                    .map_err(|_| format!("--variant takes three numbers, got `{spec}`"))?;
                let [along, across, strength] = numbers[..] else {
                    return Err(format!("--variant takes three numbers, got `{spec}`"));
                };
                options.variant = Some((along, across, strength));
            }
            "--fastest" => {
                let speed = args.next().ok_or("--fastest needs a speed in px/s")?;
                options.watch.fastest = Some(
                    speed
                        .parse::<f64>()
                        .map_err(|_| format!("--fastest takes a speed, got `{speed}`"))?,
                );
            }
            "--trace" => {
                let spec = args.next().ok_or("--trace needs point:N or bone:N")?;
                options.watch.trace = Some(parse_trace(&spec)?);
            }
            "--stance" => {
                let every = args.next().ok_or("--stance needs a number of frames")?;
                options.watch.stance =
                    Some(every.parse::<i32>().map_err(|_| {
                        format!("--stance takes a number of frames, got `{every}`")
                    })?);
            }
            "--set" => {
                let item = args.next().ok_or("--set needs NAME=VALUE")?;
                options.settings.push(item);
            }
            "--grid" => {
                let spec = args.next().ok_or("--grid needs NAME=V1,V2,...")?;
                options.grid.push(grid::Axis::parse(&spec)?);
            }
            "--shrink" => options.shrink = true,
            "--record" => {
                let path = args.next().ok_or("--record needs a file to record to")?;
                options.record = Some(PathBuf::from(path));
            }
            "--dump" => {
                let spec = args
                    .next()
                    .ok_or("--dump needs bones, joints, tendons, vessels, ties, or all")?;
                options.watch.dump = Some(Dump::parse(&spec)?);
            }
            "--mesh" => {
                let spec = args.next().ok_or("--mesh needs U,V[,R][:FRAME]")?;
                options.watch.mesh = Some(MeshShot::parse(&spec)?);
            }
            "--journal" => options.watch.journal = true,
            "--moved" => options.moved = true,
            "--mirrored" => options.mirrored = true,
            "--twin" => {
                let way = args.next().ok_or("--twin needs mirrored or across")?;
                options.twin = Some(twin::TwinWay::parse(&way)?);
            }
            "--twin-frame" => {
                let frame = args.next().ok_or("--twin-frame needs a frame")?;
                options.twin_frame = Some(
                    frame
                        .parse()
                        .map_err(|_| format!("--twin-frame takes a frame, got `{frame}`"))?,
                );
            }
            "--random" => {
                let count = args.next().ok_or("--random needs a number of plays")?;
                options.random = Some(
                    count
                        .parse()
                        .map_err(|_| format!("--random takes a number of plays, got `{count}`"))?,
                );
            }
            "--seed" => {
                let seed = args.next().ok_or("--seed needs a number")?;
                options.seed = seed
                    .parse()
                    .map_err(|_| format!("--seed takes a whole number, got `{seed}`"))?;
            }
            "--tools" => {
                let names = args
                    .next()
                    .ok_or("--tools needs bat, hammer, knife, or fist")?;
                options.tools = names
                    .split(',')
                    .map(|name| match name.trim() {
                        "bat" | "blunt" => Ok(rp::ToolMode::Blunt),
                        "hammer" | "heavy" | "sledgehammer" => Ok(rp::ToolMode::Heavy),
                        "knife" | "sharp" => Ok(rp::ToolMode::Sharp),
                        "fist" => Ok(rp::ToolMode::Fist),
                        other => Err(format!(
                            "unknown tool `{other}`; use bat, hammer, knife, or fist"
                        )),
                    })
                    .collect::<Result<_, _>>()?;
            }
            "--why" => {
                let spec = args
                    .next()
                    .ok_or("--why needs point:N, bone:N, spring:N, or joint:N")?;
                options.why = Some(rp::inspect::Why::parse(&spec)?);
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option `{flag}`")),
            path => options.csv_path = PathBuf::from(path),
        }
    }
    rp::scenarios::use_settings(rp::parse_settings(&options.settings)?);
    if options.grid.is_empty() {
        // A play with blanks left in it says to give them with --grid.
        if let Some(written) = &options.written {
            options.custom = Some(written.play(&[])?);
        }
    } else {
        check_grid_options(&options)?;
    }
    Ok(options)
}

/// A grid plays rows of runs for one table, so it takes neither the modes
/// that play something else nor what follows one run frame by frame.
fn check_grid_options(options: &Options) -> Result<(), String> {
    let others = [
        (options.diff.is_some(), "--diff"),
        (options.random.is_some(), "--random"),
        (options.moved, "--moved"),
        (options.twin.is_some(), "--twin"),
        (options.why.is_some(), "--why"),
        (options.phases, "--phases"),
        (options.sound.is_some(), "--sound"),
        (options.list, "--list"),
        (options.record.is_some(), "--record"),
        (options.mirrored, "--mirrored"),
        (
            options.watch.any(),
            "--journal, --fastest, --trace, --stance, --dump, or --mesh",
        ),
    ];
    if let Some((_, flag)) = others.iter().find(|(given, _)| *given) {
        return Err(format!(
            "--grid plays rows of runs for one table; it does not take {flag}"
        ));
    }
    if options.variant.is_some() && options.sweep {
        return Err(
            "--grid plays one run a row (--variant's) or the sweep's (--quick, --sweep), not both"
                .to_string(),
        );
    }
    if options.written.is_some() && !options.only.is_empty() {
        return Err(
            "--grid plays the custom play or the scenarios --only names, not both".to_string(),
        );
    }
    Ok(())
}

/// Plays each scenario once with the per-frame CSV, summary, and tuning report.
fn play_scenarios(
    selected: &[Scenario],
    csv_path: &Path,
    output_dir: &Path,
    watch: Watch,
    record: Option<&Path>,
) {
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
        // A fight's watchdogs watch its two fighters from the start.
        let body = scenario.world_from(watch.body(scenario));
        let mut watchdogs = rp::inspect::Watchdogs::new(&body);
        // Only a lone scenario is recorded: a recording is for looking into one run.
        let mut recorder = record
            .filter(|_| selected.len() == 1)
            .and_then(|path| Recorder::start(path, scenario.name, &body));
        let (world, result) = run_on(
            body,
            scenario,
            SCENARIO_WIDTH,
            SCENARIO_HEIGHT,
            |frame, input, world| {
                watch.report(scenario.name, frame, world, &last_positions);
                watchdogs.check(frame, world);
                if let Some(recorder) = recorder.as_mut() {
                    recorder.frame(frame, world, watchdogs.trips());
                }
                if write_error.is_none() {
                    write_error =
                        write_frame(&mut csv, scenario, frame, input, world, &mut last_positions)
                            .err();
                }
            },
        );
        if let Some(recorder) = recorder {
            recorder.finish();
        }
        if let Some(error) = write_error {
            panic!("write strike CSV: {error}");
        }
        // One scenario on its own, as `--only` or `--variant` plays it, also
        // lists where its broken bone ended up, what the watchdogs flagged
        // and why, and what its journal kept.
        if selected.len() == 1 {
            println!("{}:", scenario.name);
            report_pieces_out(&world);
            report_watchdogs(scenario, &world, watchdogs.trips());
            print_journal(&world);
        } else {
            for line in rp::inspect::watchdog_lines(&world, watchdogs.trips()) {
                println!("{}: watchdog: {line}", scenario.name);
            }
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
fn play_custom(scenario: &Scenario, output_dir: &Path, watch: Watch, record: Option<&Path>) {
    let frames_path = output_dir.join("strike_custom_frames.csv");
    let mut frames = BufWriter::new(File::create(&frames_path).expect("create custom frames"));
    write_frame_header(&mut frames).expect("write frame header");
    let mut write_error = None;
    let mut last_positions = Vec::new();
    let body = scenario.world_from(watch.body(scenario));
    let mut watchdogs = rp::inspect::Watchdogs::new(&body);
    let mut recorder = record.and_then(|path| Recorder::start(path, scenario.name, &body));
    let (world, result) = run_on(
        body,
        scenario,
        SCENARIO_WIDTH,
        SCENARIO_HEIGHT,
        |frame, input, world| {
            watch.report(scenario.name, frame, world, &last_positions);
            watchdogs.check(frame, world);
            if let Some(recorder) = recorder.as_mut() {
                recorder.frame(frame, world, watchdogs.trips());
            }
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
    if let Some(recorder) = recorder {
        recorder.finish();
    }
    println!("{}", scenario.play);
    println!("  {}", headline(&result));
    println!("  {}", skin_openings(&world));
    if result.blade_path_length > 0.0 {
        println!("  {}", cut_quality(&result));
    }
    println!("  {}", steadiness(&result));
    report_pieces_out(&world);
    report_watchdogs(scenario, &world, watchdogs.trips());
    print_journal(&world);
    let path = output_dir.join("strike_custom.csv");
    let mut out = BufWriter::new(File::create(&path).expect("create custom CSV"));
    writeln!(out, "{SUMMARY_HEADER}").expect("write custom CSV");
    writeln!(out, "{}", summary_fields(scenario, &result).join(",")).expect("write custom CSV");
    println!("wrote {}", path.display());
    println!("wrote {}", frames_path.display());
}

/// Plays each scenario on all cores, hearing it the way the app does, and
/// writes the mix as NAME.wav with a line per scenario in sound_report.txt,
/// plus every clip of the sound bank on its own in bank/.
fn listen(selected: &[Scenario], dir: &Path) {
    let started = Instant::now();
    let bank = SoundBank::render();
    let seconds: usize = bank.clips.iter().map(|clip| clip.samples.len()).sum();
    println!(
        "sound bank: {} clips, {:.1} s of audio, rendered in {:.0} ms",
        bank.clips.len(),
        seconds as f64 / rp::sound::SAMPLE_RATE as f64,
        started.elapsed().as_secs_f64() * 1000.0
    );
    let bank_dir = dir.join("bank");
    fs::create_dir_all(&bank_dir).expect("create sound folder");
    for clip in &bank.clips {
        fs::write(
            bank_dir.join(format!("{}.wav", clip.name)),
            wav_bytes(&clip.samples),
        )
        .expect("write bank clip");
    }
    let step_seconds = rp::Materials::default().fixed_dt;
    let next = AtomicUsize::new(0);
    let finished = Mutex::new(Vec::with_capacity(selected.len()));
    let workers = thread::available_parallelism().map_or(4, |count| count.get());
    thread::scope(|scope| {
        for _ in 0..workers.min(selected.len()) {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(scenario) = selected.get(index) else {
                    break;
                };
                let mut foley = Foley::new();
                let mut mix = Mixdown::new(&bank, step_seconds);
                let mut heard_all = Vec::new();
                let body = settled_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, scenario.materials());
                run_on(
                    body,
                    scenario,
                    SCENARIO_WIDTH,
                    SCENARIO_HEIGHT,
                    |_, _, world| {
                        let heard = foley.hear(world);
                        mix.step(&heard);
                        heard_all.push(heard);
                    },
                );
                let samples = mix.finish();
                let path = dir.join(format!("{}.wav", scenario.name));
                fs::write(&path, wav_bytes(&samples)).expect("write sound");
                let line = sound_summary(scenario.name, &heard_all, &samples);
                finished.lock().expect("sound results").push((index, line));
            });
        }
    });
    let mut lines = finished.into_inner().expect("sound results");
    lines.sort_by_key(|(index, _)| *index);
    let report: String = lines.into_iter().map(|(_, line)| line + "\n").collect();
    let report_path = dir.join("sound_report.txt");
    fs::write(&report_path, &report).expect("write sound report");
    print!("{report}");
    println!(
        "wrote {} WAV files and {}",
        selected.len(),
        report_path.display()
    );
}

/// One line on what a scenario sounded like: each kind of cue, how many and
/// the strongest; the loudest the air and the knife got; and the mix's peak,
/// its loudest 50 ms, and its length.
fn sound_summary(name: &str, heard: &[Heard], samples: &[f32]) -> String {
    let cues: Vec<String> = CueKind::ALL
        .iter()
        .map(|&kind| {
            let strengths: Vec<f32> = heard
                .iter()
                .flat_map(|step| step.cues.iter())
                .filter(|cue| cue.kind == kind)
                .map(|cue| cue.strength)
                .collect();
            match strengths.iter().copied().reduce(f32::max) {
                Some(strongest) => format!("{} {} ({strongest:.2})", kind.name(), strengths.len()),
                None => format!("{} 0", kind.name()),
            }
        })
        .collect();
    let loudest = |bed: fn(&Heard) -> f32| heard.iter().map(bed).fold(0.0f32, f32::max);
    let decibels = |level: f32| 20.0 * level.max(1.0e-6).log10();
    let peak = samples
        .iter()
        .fold(0.0f32, |top, sample| top.max(sample.abs()));
    let window = (0.05 * rp::sound::SAMPLE_RATE as f64) as usize;
    let loudest_window = samples
        .chunks(window)
        .map(|chunk| (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt())
        .fold(0.0f32, f32::max);
    format!(
        "{name:<30} {} | rush {:.2} slice {:.2} | peak {:.1} dBFS, loudest 50 ms {:.1} dBFS, {:.2} s",
        cues.join("  "),
        loudest(|step| step.beds.rush),
        loudest(|step| step.beds.slice),
        decibels(peak),
        decibels(loudest_window),
        samples.len() as f64 / rp::sound::SAMPLE_RATE as f64
    )
}

/// Milliseconds since the first call: the clock that times each phase.
fn now_ms() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Plays each scenario on one thread with every phase of its steps timed, and
/// reports where the time goes, slowest phase first.
fn time_phases(selected: &[Scenario], output_dir: &Path) {
    let mut total = rp::PhaseTimes::default();
    let mut report = String::new();
    let mut scenario_lines = String::new();
    for scenario in selected {
        let mut world = settled_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, scenario.materials());
        world.time_phases(Some(now_ms));
        let (world, _) = run_on(
            world,
            scenario,
            SCENARIO_WIDTH,
            SCENARIO_HEIGHT,
            |_, _, _| {},
        );
        let times = world.phase_times();
        scenario_lines.push_str(&format!(
            "  {:<30} {:>7.3} {:>8.3}\n",
            scenario.name,
            times.total_ms / times.steps.max(1) as f64,
            times.slowest_step_ms
        ));
        total.merge(times);
    }

    let steps = total.steps.max(1) as f64;
    report.push_str(&format!(
        "{} steps of {} scenarios: {:.3} ms a step on average, {:.3} ms at most\n\n",
        total.steps,
        selected.len(),
        total.total_ms / steps,
        total.slowest_step_ms
    ));
    report.push_str(&format!(
        "  {:<30} {:>7} {:>6} {:>8}\n",
        "phase", "ms/step", "share", "slowest"
    ));
    let mut phases = total.phases.clone();
    phases.sort_by(|a, b| b.total_ms.total_cmp(&a.total_ms));
    for phase in &phases {
        report.push_str(&format!(
            "  {:<30} {:>7.3} {:>5.1}% {:>8.3}\n",
            phase.name,
            phase.total_ms / steps,
            phase.total_ms / total.total_ms.max(f64::EPSILON) * 100.0,
            phase.slowest_step_ms
        ));
    }
    let between = total.total_ms - phases.iter().map(|phase| phase.total_ms).sum::<f64>();
    report.push_str(&format!(
        "  {:<30} {:>7.3} {:>5.1}%\n\n",
        "(between phases)",
        between / steps,
        between / total.total_ms.max(f64::EPSILON) * 100.0
    ));
    report.push_str(&format!(
        "  {:<30} {:>7} {:>8}\n",
        "scenario", "ms/step", "slowest"
    ));
    report.push_str(&scenario_lines);

    let path = output_dir.join("strike_phases.txt");
    fs::write(&path, &report).expect("write phase times");
    print!("{report}");
    println!("wrote {}", path.display());
}
