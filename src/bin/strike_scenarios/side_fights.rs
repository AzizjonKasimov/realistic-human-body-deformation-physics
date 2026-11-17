//! Random side-view fights (`--side-fights N`): the engine's random fights
//! between two side-view fighters (`rp::play_side_fight`), both making a move
//! about four times a second, played on all cores and written one to a line
//! (`side_fights.csv`), which `--diff` sets beside another build's, fight
//! against fight (`side_fights_diff.rs`). Fight `seed` here is fight `seed`
//! of the engine's `print_random_fights` test, step for step, so any of them
//! can be followed there with `RP_SEED`, `RP_WHY`, and `RP_GESTURE`.

use super::*;
use rp::inspect::joint_name;

/// The file the side-view fights are written to, which `--diff` reads.
pub(crate) const SIDE_FIGHTS_FILE: &str = "side_fights.csv";

/// How long each fight lasts, in seconds: as `print_random_fights` plays
/// them.
pub(crate) const SIDE_FIGHT_SECONDS: f64 = 20.0;

/// The columns of a side-view fights file, after the comment lines that say
/// what the fights were played with.
pub(crate) const SIDE_FIGHTS_HEADER: &str = "fight,seed,seconds,left_joints_torn,right_joints_torn,left_joints_slipped,right_joints_slipped,left_bones_broken,right_bones_broken,left_skin_torn,right_skin_torn,muscle_torn,left_knocked_out,right_knocked_out,left_fell,right_fell,left_head_off_cm,right_head_off_cm,fastest_m_s,left_torn_joints,right_torn_joints,left_slipped_joints,right_slipped_joints,left_downs,right_downs,left_got_up,right_got_up,left_longest_down_s,right_longest_down_s";

/// Plays side-view fights `seed` to `seed + count - 1`, each
/// [`SIDE_FIGHT_SECONDS`] long, on all cores, writes them to
/// `side_fights.csv` in `output_dir`, and prints and writes what they did.
pub(crate) fn side_fights(count: usize, seed: u64, output_dir: &Path) {
    let started = Instant::now();
    let materials = rp::scenarios::set_materials();
    let seeds: Vec<u64> = (0..count as u64).map(|offset| seed + offset).collect();
    let next = AtomicUsize::new(0);
    let finished = Mutex::new(Vec::with_capacity(count));
    let workers = thread::available_parallelism().map_or(4, |workers| workers.get());
    thread::scope(|scope| {
        for _ in 0..workers.min(count) {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(&seed) = seeds.get(index) else {
                    break;
                };
                let fight = rp::play_side_fight(seed, SIDE_FIGHT_SECONDS, materials.clone());
                finished.lock().expect("side fights").push((index, fight));
            });
        }
    });
    let mut fights = finished.into_inner().expect("side fights");
    fights.sort_by_key(|(index, _)| *index);
    let fights: Vec<rp::SideFight> = fights.into_iter().map(|(_, fight)| fight).collect();
    // Joints are named from a world built as the fights' were.
    let world = rp::create_side_fighters(materials);
    let names = |joints: &[usize]| {
        joints
            .iter()
            .map(|&joint| joint_name(&world, joint))
            .collect::<Vec<_>>()
            .join("; ")
    };

    let csv_path = output_dir.join(SIDE_FIGHTS_FILE);
    let mut csv = BufWriter::new(File::create(&csv_path).expect("create side fights CSV"));
    writeln!(
        csv,
        "# settings: {}\n# trying: {}\n# seed: {seed}\n# seconds: {SIDE_FIGHT_SECONDS}\n{SIDE_FIGHTS_HEADER}",
        rp::scenarios::settings_used().join(" "),
        env::var("RP_TRY").unwrap_or_default(),
    )
    .expect("write side fights CSV");
    for (index, fight) in fights.iter().enumerate() {
        let out = |step: Option<usize>| step.map_or(String::new(), |step| step.to_string());
        writeln!(
            csv,
            "{index},{},{SIDE_FIGHT_SECONDS},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.1},{:.1},{:.2},\"{}\",\"{}\",\"{}\",\"{}\",{},{},{},{},{:.2},{:.2}",
            fight.seed,
            fight.torn_joints[0].len(),
            fight.torn_joints[1].len(),
            fight.slipped_joints[0].len(),
            fight.slipped_joints[1].len(),
            fight.bones_broken[0],
            fight.bones_broken[1],
            fight.skin_torn[0],
            fight.skin_torn[1],
            fight.muscle_torn,
            out(fight.knocked_out[0]),
            out(fight.knocked_out[1]),
            u8::from(fight.fell[0]),
            u8::from(fight.fell[1]),
            fight.head_off[0] * 100.0,
            fight.head_off[1] * 100.0,
            fight.fastest,
            names(&fight.torn_joints[0]),
            names(&fight.torn_joints[1]),
            names(&fight.slipped_joints[0]),
            names(&fight.slipped_joints[1]),
            fight.downs[0],
            fight.downs[1],
            fight.got_up[0],
            fight.got_up[1],
            fight.longest_down[0],
            fight.longest_down[1],
        )
        .expect("write side fights CSV");
    }
    drop(csv);

    let mut report = format!(
        "{count} random side-view fights, seeds {seed} to {}, {SIDE_FIGHT_SECONDS} s each: both fighters make a move about four times a second, drawn at random (the engine's print_random_fights; replay one there with RP_SEED=N).\n\n",
        seed + count as u64 - 1
    );
    for fight in &fights {
        let mut line = format!(
            "seed {}: joints torn {}, slipped {}, bones broken {}, skin torn {}, muscle torn {}",
            fight.seed,
            fight.torn_joints[0].len() + fight.torn_joints[1].len(),
            fight.slipped_joints[0].len() + fight.slipped_joints[1].len(),
            fight.bones_broken[0] + fight.bones_broken[1],
            fight.skin_torn[0] + fight.skin_torn[1],
            fight.muscle_torn
        );
        for (body, side) in ["left", "right"].iter().enumerate() {
            if let Some(step) = fight.knocked_out[body] {
                line.push_str(&format!("; the {side} fighter knocked out on step {step}"));
            }
            if fight.fell[body] {
                line.push_str(&format!("; the {side} fighter fell with no knockout"));
            }
            if fight.downs[body] > 0 {
                line.push_str(&format!(
                    "; the {side} fighter went down {} times, got up {}, down {:.1} s at the longest",
                    fight.downs[body], fight.got_up[body], fight.longest_down[body]
                ));
            }
        }
        report.push_str(&line);
        report.push('\n');
    }
    let count_of =
        |test: &dyn Fn(&rp::SideFight) -> bool| fights.iter().filter(|f| test(f)).count();
    report.push_str(&format!(
        "\nIn all: {} fights tore or put out a joint, {} broke a bone, {} tore skin or muscle, {} knocked a fighter out, {} had a fighter fall with no knockout; fighters went down {} times and got up {}; heads got {:.0} cm from their poses at the median.\n",
        count_of(&|f| f.torn_joints.iter().chain(&f.slipped_joints).any(|joints| !joints.is_empty())),
        count_of(&|f| f.bones_broken.iter().sum::<i32>() > 0),
        count_of(&|f| f.skin_torn.iter().sum::<i32>() + f.muscle_torn > 0),
        count_of(&|f| f.knocked_out.iter().any(Option::is_some)),
        count_of(&|f| f.fell.iter().any(|&fell| fell)),
        fights.iter().map(|f| f.downs.iter().sum::<u32>()).sum::<u32>(),
        fights.iter().map(|f| f.got_up.iter().sum::<u32>()).sum::<u32>(),
        diff::median(
            fights
                .iter()
                .flat_map(|fight| fight.head_off.map(|off| off * 100.0))
                .collect()
        ),
    ));
    let report_path = output_dir.join("side_fights_report.txt");
    fs::write(&report_path, &report).expect("write side fights report");
    print!("{report}");
    println!(
        "wrote {} and {} in {:.1} s",
        csv_path.display(),
        report_path.display(),
        started.elapsed().as_secs_f64()
    );
}
