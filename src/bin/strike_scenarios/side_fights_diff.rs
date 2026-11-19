//! Two builds' random side-view fights set side by side (`--diff A B` on two
//! folders holding `side_fights.csv`, or the files): fight against fight by
//! the sign test when both played the same fights, else as two samples
//! (Fisher's exact test), each difference marked by how sure it is, as the
//! front-view fights are (`fights_diff.rs`). What the findings file says
//! must hold (joints torn apart or put out, bones broken, a fighter falling
//! with no knockout) is what `tools\merge_check.ps1` holds a change to;
//! torn fibres, knockouts, and how far heads stray from their poses drift,
//! reported.

use super::*;
use diff::{write_findings, Change, Finding, Holds, Worse};
use fights_diff::csv_fields;
use side_fights::{SIDE_FIGHTS_FILE, SIDE_FIGHTS_HEADER};

/// How far a head may get from where its moves put it, in centimetres,
/// before a fight counts as one the fighter lost its pose in.
const HEAD_OFF_CM: f64 = 25.0;

/// One fighter's measures in a fight, as `--diff` counts them.
#[derive(Clone, Debug, Default, PartialEq)]
struct Fighter {
    torn: u32,
    slipped: u32,
    bones: u32,
    skin: u32,
    knocked_out: bool,
    fell: bool,
    head_off_cm: f64,
    /// Steps bone showed through whole skin, away from the window's sides.
    bone_shown: u32,
    torn_joints: String,
    slipped_joints: String,
}

/// One fight of a side-view fights file.
#[derive(Clone, Debug, Default, PartialEq)]
struct SideFightLine {
    seed: u64,
    seconds: String,
    fighters: [Fighter; 2],
    muscle: u32,
}

/// A side-view fights file read back, and what its fights were played with.
struct SideFights {
    fights: Vec<SideFightLine>,
    settings: String,
}

/// The side-view fights file in `path`, a folder or the file itself.
fn side_fights_file(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join(SIDE_FIGHTS_FILE)
    } else {
        path.to_path_buf()
    }
}

/// Whether `path` holds side-view fights: a folder with a side-view fights
/// file, or such a file, by its header.
pub(crate) fn holds_side_fights(path: &Path) -> bool {
    diff::header_line(&side_fights_file(path))
        .is_some_and(|header| header.starts_with("fight,seed,seconds,left_joints_torn"))
}

impl SideFights {
    fn read(path: &Path) -> Result<SideFights, String> {
        let file = side_fights_file(path);
        let text = fs::read_to_string(&file)
            .map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        SideFights::parse(&text)
    }

    fn parse(text: &str) -> Result<SideFights, String> {
        let mut settings = Vec::new();
        let mut fights = Vec::new();
        let mut columns: Vec<String> = Vec::new();
        for line in text.lines() {
            if let Some(comment) = line.strip_prefix("# ") {
                if let Some((name, value)) = comment.split_once(": ") {
                    if !value.is_empty() {
                        settings.push(format!("{name} {value}"));
                    }
                }
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            if columns.is_empty() {
                columns = csv_fields(line);
                continue;
            }
            let fields = csv_fields(line);
            let field = |name: &str| -> String {
                columns
                    .iter()
                    .position(|column| column == name)
                    .and_then(|at| fields.get(at))
                    .cloned()
                    .unwrap_or_default()
            };
            let number = |name: &str| field(name).parse::<u32>().unwrap_or(0);
            let fighter = |side: &str| Fighter {
                torn: number(&format!("{side}_joints_torn")),
                slipped: number(&format!("{side}_joints_slipped")),
                bones: number(&format!("{side}_bones_broken")),
                skin: number(&format!("{side}_skin_torn")),
                knocked_out: !field(&format!("{side}_knocked_out")).is_empty(),
                fell: number(&format!("{side}_fell")) > 0,
                head_off_cm: field(&format!("{side}_head_off_cm")).parse().unwrap_or(0.0),
                bone_shown: number(&format!("{side}_bone_shown")),
                torn_joints: field(&format!("{side}_torn_joints")),
                slipped_joints: field(&format!("{side}_slipped_joints")),
            };
            fights.push(SideFightLine {
                seed: field("seed")
                    .parse()
                    .map_err(|_| format!("a side-view fight without a seed: `{line}`"))?,
                seconds: field("seconds"),
                fighters: [fighter("left"), fighter("right")],
                muscle: number("muscle_torn"),
            });
        }
        if columns.is_empty() {
            return Err(format!(
                "no side-view fights: the file has no `{SIDE_FIGHTS_HEADER}` header"
            ));
        }
        Ok(SideFights {
            fights,
            settings: settings.join(", "),
        })
    }

    fn played(&self) -> String {
        let first = self.fights.first().map_or(0, |fight| fight.seed);
        let last = self.fights.last().map_or(0, |fight| fight.seed);
        format!(
            "{} fights, seeds {first} to {last}{}",
            self.fights.len(),
            if self.settings.is_empty() {
                String::new()
            } else {
                format!("; {}", self.settings)
            }
        )
    }
}

/// What is counted of each fighter, how it is named, which way it gets
/// worse, and whether it must hold.
const MEASURES: [(&str, Worse, Holds); 8] = [
    ("joints torn apart", Worse::Up, Holds::Must),
    ("joints put out of place", Worse::Up, Holds::Must),
    ("bones broken", Worse::Up, Holds::Must),
    ("falls with no knockout", Worse::Up, Holds::Must),
    ("skin fibres torn", Worse::Up, Holds::Drift),
    ("knockouts", Worse::Neither, Holds::Drift),
    (
        "heads more than 25 cm off their poses",
        Worse::Up,
        Holds::Drift,
    ),
    (
        "steps bone showed through whole skin",
        Worse::Up,
        Holds::Drift,
    ),
];

/// Measure `index` of [`MEASURES`] in `fighter`.
fn measure(fighter: &Fighter, index: usize) -> u32 {
    match index {
        0 => fighter.torn,
        1 => fighter.slipped,
        2 => fighter.bones,
        3 => u32::from(fighter.fell),
        4 => fighter.skin,
        5 => u32::from(fighter.knocked_out),
        6 => u32::from(fighter.head_off_cm > HEAD_OFF_CM),
        _ => fighter.bone_shown,
    }
}

/// How much worse fight `worse` came out than `better`, in the measures
/// that must hold and torn fibres, and in words.
fn how_much_worse(worse: &SideFightLine, better: &SideFightLine) -> (u32, String) {
    let mut beyond = 0;
    let mut parts = Vec::new();
    for (side, name) in ["left", "right"].iter().enumerate() {
        for (index, (what, _, _)) in MEASURES.iter().enumerate().take(5) {
            let (w, b) = (
                measure(&worse.fighters[side], index),
                measure(&better.fighters[side], index),
            );
            if w > b {
                beyond += w - b;
                parts.push(format!("the {name} fighter's {what} {b} -> {w}"));
            }
        }
    }
    if worse.muscle > better.muscle {
        beyond += worse.muscle - better.muscle;
        parts.push(format!(
            "muscle fibres torn {} -> {}",
            better.muscle, worse.muscle
        ));
    }
    (beyond, parts.join(", "))
}

/// The report setting side-view fights `b` beside `a`, and what it compared.
fn side_fights_report(
    a: &SideFights,
    a_label: &str,
    b: &SideFights,
    b_label: &str,
) -> (String, Vec<Finding>) {
    let mut findings = Vec::new();
    let paired = !a.fights.is_empty()
        && a.fights.len() == b.fights.len()
        && a.fights
            .iter()
            .zip(&b.fights)
            .all(|(x, y)| x.seed == y.seed && x.seconds == y.seconds);
    let mut report = format!(
        "Side-view fights A: {a_label} ({})\nSide-view fights B: {b_label} ({})\n",
        a.played(),
        b.played()
    );
    report.push_str(if paired {
        "Both sides played the same fights, so each fight is set against itself. Each count is the fights a thing happened in, then how many times in all, A -> B; * a difference chance gives under 5% of the time, ** under 1% (the sign test on the fights in which B had more or fewer of it than A). [must] marks what must not get clearly worse before a merge.\n"
    } else {
        "The sides played different fights (another count or seed, or fights of another length), so they are compared as two samples. Each count is the fights a thing happened in, then how many times in all, A -> B; * a difference chance gives under 5% of the time, ** under 1% (Fisher's exact test). [must] marks what must not get clearly worse before a merge.\n"
    });
    for (side, fighter) in ["left", "right"].iter().enumerate() {
        report.push_str(&format!("\nThe {fighter} fighter:\n"));
        for (index, (what, worse, holds)) in MEASURES.iter().enumerate() {
            let change = Change::of(&a.fights, &b.fights, paired, &|fight: &SideFightLine| {
                measure(&fight.fighters[side], index)
            });
            report.push_str(&format!(
                "  {}{}\n",
                change.line(what, paired, "fights"),
                if *holds == Holds::Must { " [must]" } else { "" }
            ));
            findings.push(change.finding(
                "side-view fights",
                &format!("the {fighter} fighter's {what}"),
                paired,
                *worse,
                *holds,
            ));
        }
    }
    let change = Change::of(&a.fights, &b.fights, paired, &|fight: &SideFightLine| {
        fight.muscle
    });
    report.push_str(&format!(
        "\nBoth fighters:\n  {}\n",
        change.line("muscle fibres torn", paired, "fights")
    ));
    findings.push(change.finding(
        "side-view fights",
        "muscle fibres torn",
        paired,
        Worse::Up,
        Holds::Drift,
    ));
    if paired {
        // The fights one side came out of more hurt, the most first, to
        // follow in the engine's print_random_fights with RP_SEED.
        for (worse_side, a_worse) in [("B", false), ("A", true)] {
            let mut worse: Vec<(u32, String)> = a
                .fights
                .iter()
                .zip(&b.fights)
                .filter_map(|(x, y)| {
                    let (worse, better) = if a_worse { (x, y) } else { (y, x) };
                    let (beyond, what) = how_much_worse(worse, better);
                    (beyond > 0).then(|| (beyond, format!("  RP_SEED={}: {what}", worse.seed)))
                })
                .collect();
            if worse.is_empty() {
                continue;
            }
            worse.sort_by(|x, y| y.0.cmp(&x.0));
            report.push_str(&format!(
                "\nFights {worse_side} came out of more hurt, {} in all; the three most (follow one with `cargo test --release --lib print_random_fights -- --ignored --nocapture` and RP_SEED):\n",
                worse.len()
            ));
            for (_, line) in worse.iter().take(3) {
                report.push_str(&format!("{line}\n"));
            }
        }
    }
    (report, findings)
}

/// Sets the side-view fights in `b` beside those in `a`, each a folder
/// holding a side-view fights file or the file itself, writes the report to
/// `output` (`side_fights_diff.txt`, and what it compared to
/// `side_fights_diff.csv`), and prints it.
pub(crate) fn diff_side_fights(a: &Path, b: &Path, output: &Path) -> Result<(), String> {
    let (report, findings) = side_fights_report(
        &SideFights::read(a)?,
        &a.display().to_string(),
        &SideFights::read(b)?,
        &b.display().to_string(),
    );
    let path = output.join("side_fights_diff.txt");
    fs::write(&path, &report)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    write_findings(&output.join("side_fights_diff.csv"), &findings)?;
    print!("{report}");
    println!("wrote {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A side-view fights file of `lines`, each a seed, the left fighter's
    /// torn joints, and whether the right fighter fell.
    fn file(lines: &[(u64, u32, bool)]) -> String {
        let mut text = format!("# settings: \n# seed: 1\n# seconds: 20\n{SIDE_FIGHTS_HEADER}\n");
        for (index, &(seed, torn, fell)) in lines.iter().enumerate() {
            text.push_str(&format!(
                "{index},{seed},20,{torn},0,0,0,0,0,0,1,2,1170,,0,{},12.0,31.5,9.80,\"{}\",\"\",\"\",\"\"\n",
                u8::from(fell),
                if torn > 0 { "left elbow" } else { "" }
            ));
        }
        text
    }

    /// A side-view fights file reads back each fighter's measures, the
    /// joints' names with their commas, and what the fights were played
    /// with.
    #[test]
    fn a_side_fights_file_reads_back_what_it_holds() {
        let fights = SideFights::parse(&file(&[(1, 1, false), (2, 0, true)])).unwrap();
        assert_eq!(fights.fights.len(), 2);
        let first = &fights.fights[0];
        assert_eq!((first.seed, first.fighters[0].torn), (1, 1));
        assert!(first.fighters[0].knocked_out && !first.fighters[1].knocked_out);
        assert_eq!(first.fighters[0].torn_joints, "left elbow");
        assert_eq!(first.muscle, 2);
        assert!(fights.fights[1].fighters[1].fell);
        assert!(fights.settings.contains("seed 1"));
    }

    /// The same fights are set fight against fight, and a must-hold measure
    /// that got worse in many fights is judged worse and marked must.
    #[test]
    fn the_same_fights_are_set_against_each_other() {
        let a = SideFights::parse(&file(
            &(1..=12).map(|seed| (seed, 0, false)).collect::<Vec<_>>(),
        ))
        .unwrap();
        let b = SideFights::parse(&file(
            &(1..=12).map(|seed| (seed, 1, false)).collect::<Vec<_>>(),
        ))
        .unwrap();
        let (report, findings) = side_fights_report(&a, "a", &b, "b");
        assert!(report.contains("each fight is set against itself"));
        let torn = findings
            .iter()
            .find(|finding| finding.item == "the left fighter's joints torn apart")
            .unwrap();
        assert_eq!((torn.change, torn.holds()), ("worse", Holds::Must));
        assert!(report.contains("RP_SEED="));
    }
}
