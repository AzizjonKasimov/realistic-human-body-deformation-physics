//! Two sets of random blows side by side (`--diff A B` on folders that hold
//! `random_blows.csv`, on folders of such folders, one a seed, or on the
//! files): the plays each watchdog tripped in and each rule every play
//! follows was broken in, tool by tool, and the plays that broke bones, tore
//! skin and muscle, and cut arteries, with how likely chance is to make each
//! difference. Random blows count a problem in a few plays of a thousand, so
//! a handful more or less is chance; this says which differences are not.
//! Sides that drew the same blows (the same seeds and counts: every blow's
//! tool, path, speed, and timing come from the dice alike) are set blow
//! against blow, by the sign test on the blows that came out differently,
//! even where a blow is aimed a little differently because the body settled
//! or was built differently, which moves the point of skin it aims through
//! and where it starts. Sides that drew other blows are compared as two
//! samples, by Fisher's exact test on the plays each thing happened in.

use super::*;
use diff::{header_line, stars, write_findings, Change, Finding, Holds, Worse};
use fights_diff::{csv_fields, watchdog_named};
use rp::inspect::Watchdog;
use std::collections::BTreeSet;

/// The file `--random` writes its blows to.
pub(crate) const BLOWS_FILE: &str = "random_blows.csv";

/// What a blow did to the body, as its line's columns name it, and in words.
const INJURIES: [(&str, &str); 4] = [
    ("bones_broken", "bones broken"),
    ("skin_torn", "skin torn"),
    ("muscle_torn", "muscle torn"),
    ("arteries_cut", "arteries cut"),
];

/// The rules every play follows that may drift as the engine changes,
/// reported rather than held: struck flesh faster than twice the tool,
/// which the flung watchdog counts too.
const DRIFTING_RULES: [&str; 1] = ["flesh_speed"];

/// One blow as a blows file holds it.
struct Blow {
    play: String,
    tool: String,
    /// What it did, in the order of `INJURIES`.
    injuries: [u32; 4],
    /// The rules every play follows that it broke, by name.
    broken: BTreeSet<String>,
    /// The watchdogs that tripped in it.
    tripped: BTreeSet<Watchdog>,
}

/// One side's blows, from one blows file or several (a seed each).
struct Blows {
    /// What each file's blows were played with, for the report's heading.
    sources: Vec<String>,
    blows: Vec<Blow>,
}

/// The blows files `path` names: the file itself, the one in its folder,
/// or the one in each of its folders, in the order of their names.
fn blows_files(path: &Path) -> Vec<PathBuf> {
    if !path.is_dir() {
        return vec![path.to_path_buf()];
    }
    if path.join(BLOWS_FILE).is_file() {
        return vec![path.join(BLOWS_FILE)];
    }
    let mut files: Vec<PathBuf> = fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path().join(BLOWS_FILE))
                .filter(|file| file.is_file())
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Whether `path` holds random blows: a blows file under any name, a folder
/// with one, or a folder of such folders.
pub(crate) fn holds_blows(path: &Path) -> bool {
    let files = blows_files(path);
    !files.is_empty()
        && files.iter().all(|file| {
            header_line(file).is_some_and(|header| header.starts_with("index,play,tool,"))
        })
}

impl Blows {
    fn read(path: &Path) -> Result<Blows, String> {
        let mut blows = Blows {
            sources: Vec::new(),
            blows: Vec::new(),
        };
        for file in blows_files(path) {
            let text = fs::read_to_string(&file)
                .map_err(|error| format!("cannot read {}: {error}", file.display()))?;
            let (source, read) =
                parse_blows(&text).map_err(|error| format!("{}: {error}", file.display()))?;
            blows.sources.push(source);
            blows.blows.extend(read);
        }
        if blows.sources.is_empty() {
            return Err(format!("no {BLOWS_FILE} in {}", path.display()));
        }
        Ok(blows)
    }
}

/// A blows file's blows, and what they were played with, in words.
fn parse_blows(text: &str) -> Result<(String, Vec<Blow>), String> {
    let (mut settings, mut trying, mut seed) = (String::new(), String::new(), String::new());
    let mut columns: Vec<String> = Vec::new();
    let mut blows = Vec::new();
    for line in text.lines() {
        if let Some(comment) = line.strip_prefix('#') {
            if let Some((key, value)) = comment.split_once(':') {
                let value = value.trim().to_string();
                match key.trim() {
                    "settings" => settings = value,
                    "trying" => trying = value,
                    "seed" => seed = value,
                    _ => {}
                }
            }
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        let fields = csv_fields(line);
        if columns.is_empty() {
            columns = fields;
            continue;
        }
        let field = |name: &str| {
            columns
                .iter()
                .position(|column| column == name)
                .and_then(|index| fields.get(index))
                .map(String::as_str)
                .ok_or_else(|| format!("no {name} in `{line}`"))
        };
        let list = |name: &str| -> Result<Vec<String>, String> {
            Ok(field(name)?
                .split(';')
                .filter(|entry| !entry.is_empty())
                .map(str::to_string)
                .collect())
        };
        let mut injuries = [0; 4];
        for (index, (column, _)) in INJURIES.iter().enumerate() {
            injuries[index] = field(column)?
                .parse()
                .map_err(|_| format!("unreadable {column} in `{line}`"))?;
        }
        let tripped = list("watchdogs")?
            .iter()
            .map(|entry| {
                let name = entry.split('@').next().unwrap_or(entry);
                watchdog_named(name).ok_or_else(|| format!("unknown watchdog `{name}`"))
            })
            .collect::<Result<_, _>>()?;
        blows.push(Blow {
            play: field("play")?.to_string(),
            tool: field("tool")?.to_string(),
            injuries,
            broken: list("rules_broken")?.into_iter().collect(),
            tripped,
        });
    }
    if columns.is_empty() {
        return Err("no header line".to_string());
    }
    let mut parts = vec![format!("{} plays", blows.len())];
    parts.push(if seed.is_empty() {
        "written before blows files said their seed".to_string()
    } else {
        format!("seed {seed}")
    });
    if !settings.is_empty() {
        parts.push(settings);
    }
    if !trying.is_empty() {
        parts.push(format!("trying {trying}"));
    }
    Ok((parts.join(", "), blows))
}

/// What one line of the report counts in a blow, and how the good-enough
/// rule treats it.
struct Counted<'a> {
    what: String,
    count: Box<dyn Fn(&Blow) -> u32 + 'a>,
    worse: Worse,
    holds: Holds,
}

/// A tool as players know it, for a blows file's `blunt`, `heavy`, and
/// `sharp`.
fn shown_tool(tool: &str) -> &str {
    match tool {
        "blunt" => "bat",
        "heavy" => "hammer",
        "sharp" => "knife",
        other => other,
    }
}

/// Tool by tool, the plays `count` happened in, with how sure each tool's
/// difference is: "bat ** 38 -> 66, hammer 17 -> 20".
fn by_tool(a: &[Blow], b: &[Blow], paired: bool, count: &dyn Fn(&Blow) -> u32) -> String {
    let tools: BTreeSet<&str> = a
        .iter()
        .chain(b)
        .filter(|blow| count(blow) > 0)
        .map(|blow| blow.tool.as_str())
        .collect();
    tools
        .into_iter()
        .map(|tool| {
            // This tool's count in each play, the others' plays counting
            // nought, so paired plays stay set against each other.
            let with = |blows: &[Blow]| -> Vec<u32> {
                blows
                    .iter()
                    .filter(|blow| paired || blow.tool == tool)
                    .map(|blow| if blow.tool == tool { count(blow) } else { 0 })
                    .collect()
            };
            let change = Change::of(&with(a), &with(b), paired, &|count: &u32| *count);
            let sure = match stars(change.p).trim() {
                "" => String::new(),
                marked => format!("{marked} "),
            };
            format!(
                "{} {sure}{} -> {}",
                shown_tool(tool),
                change.plays.0,
                change.plays.1
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A section of the report, a line for each thing counted, each also a
/// finding.
fn section(
    report: &mut String,
    findings: &mut Vec<Finding>,
    (a, b, paired): (&[Blow], &[Blow], bool),
    title: &str,
    lines: &[Counted],
) {
    report.push_str(&format!("\n{title}:\n"));
    if lines.is_empty() {
        report.push_str("  none in either side's plays\n");
    }
    for line in lines {
        let change = Change::of(a, b, paired, &*line.count);
        report.push_str(&format!("  {}\n", change.line(&line.what, paired, "plays")));
        if change.plays != (0, 0) {
            report.push_str(&format!("      {}\n", by_tool(a, b, paired, &*line.count)));
        }
        findings.push(change.finding(title, &line.what, paired, line.worse, line.holds));
    }
}

/// The watchdogs that break the body (`breaks`: all of them, tripped or
/// not, as the good-enough rule holds each), or the others that tripped.
fn watchdog_lines<'a>(breaks: bool, tripped: &BTreeSet<Watchdog>) -> Vec<Counted<'a>> {
    Watchdog::ALL
        .iter()
        .filter(|kind| kind.breaks_the_body() == breaks && (breaks || tripped.contains(kind)))
        .map(|&kind| Counted {
            what: format!(
                "{}{}",
                kind.describe(),
                if kind.known_gap() { " [known gap]" } else { "" }
            ),
            count: Box::new(move |blow: &Blow| u32::from(blow.tripped.contains(&kind))),
            worse: Worse::Up,
            holds: if breaks { Holds::Must } else { Holds::Drift },
        })
        .collect()
}

/// What a side's files were played with, the same words once with how many
/// files say them: "3 files of 2000 plays, written before ...".
fn sources(sources: &[String]) -> String {
    match sources {
        [first, rest @ ..] if !rest.is_empty() && rest.iter().all(|source| source == first) => {
            format!("{} files of {first}", sources.len())
        }
        _ => sources.join("; "),
    }
}

/// How a play is replayed: `--gesture` for one that presses the tool down,
/// `--strike` for a swing.
fn replay(play: &str) -> String {
    let option = if play.contains(":down") {
        "--gesture"
    } else {
        "--strike"
    };
    format!("{option} \"{play}\"")
}

/// The report setting blows `b` beside blows `a`, each headed by `label`,
/// and what it compared, as findings.
fn blows_report(a: &Blows, a_label: &str, b: &Blows, b_label: &str) -> (String, Vec<Finding>) {
    let (x, y) = (a.blows.as_slice(), b.blows.as_slice());
    // The same draws give every blow the same tool, so a side that drew
    // other blows shows in its tools.
    let paired = !x.is_empty()
        && x.len() == y.len()
        && x.iter()
            .zip(y)
            .all(|(first, second)| first.tool == second.tool);
    let aimed_apart = x
        .iter()
        .zip(y)
        .filter(|(first, second)| first.play != second.play)
        .count();
    let sides = (x, y, paired);
    let mut findings = Vec::new();
    let mut report = format!(
        "Blows A: {a_label} ({})\nBlows B: {b_label} ({})\n",
        sources(&a.sources),
        sources(&b.sources)
    );
    if paired {
        report.push_str("Both sides drew the same blows, so each blow is set against itself");
        if aimed_apart > 0 {
            report.push_str(&format!(
                " ({aimed_apart} of {} aimed a little differently, as the body settled or was built differently, which moves the skin they aim through)",
                x.len()
            ));
        }
        report.push_str(". Each count is the plays a thing happened in, A -> B, then tool by tool; * a difference chance gives under 5% of the time, ** under 1% (the sign test on the plays in which B had more or fewer of it than A).\n");
    } else {
        report.push_str("The sides drew different blows (other seeds or counts), so they are compared as two samples. Each count is the plays a thing happened in, A -> B, then tool by tool; * a difference in plays chance gives under 5% of the time, ** under 1% (Fisher's exact test).\n");
    }
    let tripped: BTreeSet<Watchdog> = x
        .iter()
        .chain(y)
        .flat_map(|blow| blow.tripped.iter().copied())
        .collect();
    section(
        &mut report,
        &mut findings,
        sides,
        "Watchdogs for a body that breaks",
        &watchdog_lines(true, &tripped),
    );
    let rules: BTreeSet<String> = x
        .iter()
        .chain(y)
        .flat_map(|blow| blow.broken.iter().cloned())
        .collect();
    let rule_lines: Vec<Counted> = rules
        .into_iter()
        .map(|rule| Counted {
            holds: if DRIFTING_RULES.contains(&rule.as_str()) {
                Holds::Drift
            } else {
                Holds::Must
            },
            what: rule.clone(),
            count: Box::new(move |blow: &Blow| u32::from(blow.broken.contains(&rule))),
            worse: Worse::Up,
        })
        .collect();
    section(
        &mut report,
        &mut findings,
        sides,
        "The rules every play follows",
        &rule_lines,
    );
    section(
        &mut report,
        &mut findings,
        sides,
        "Other watchdogs",
        &watchdog_lines(false, &tripped),
    );
    let injury_lines: Vec<Counted> = INJURIES
        .iter()
        .enumerate()
        .map(|(index, (_, what))| Counted {
            what: what.to_string(),
            count: Box::new(move |blow: &Blow| blow.injuries[index]),
            worse: Worse::Neither,
            holds: Holds::Drift,
        })
        .collect();
    section(&mut report, &mut findings, sides, "Injuries", &injury_lines);
    if paired {
        // The blows only one side tripped a watchdog or broke a rule in, to
        // replay with --strike or --gesture and follow with --journal and
        // --why.
        for (side, a_worse) in [("B", false), ("A", true)] {
            let worse: Vec<String> = x
                .iter()
                .zip(y)
                .filter_map(|(first, second)| {
                    let (worse, better) = if a_worse {
                        (first, second)
                    } else {
                        (second, first)
                    };
                    let mut what: Vec<String> = worse
                        .tripped
                        .difference(&better.tripped)
                        .map(|kind| kind.describe().to_string())
                        .collect();
                    what.extend(worse.broken.difference(&better.broken).cloned());
                    (!what.is_empty())
                        .then(|| format!("  {}: {}", replay(&worse.play), what.join("; ")))
                })
                .collect();
            if worse.is_empty() {
                continue;
            }
            report.push_str(&format!(
                "\nBlows only {side} tripped a watchdog or broke a rule in, {} in all; the first three:\n",
                worse.len()
            ));
            for line in worse.iter().take(3) {
                report.push_str(&format!("{line}\n"));
            }
        }
    }
    (report, findings)
}

/// Sets the blows in `b` beside those in `a`, each a folder holding a
/// blows file, a folder of such folders (a seed each), or the file itself,
/// writes the report to `output` (`random_diff.txt`, and what it compared
/// to `random_diff.csv`), and prints it.
pub(crate) fn diff_blows(a: &Path, b: &Path, output: &Path) -> Result<(), String> {
    let (report, findings) = blows_report(
        &Blows::read(a)?,
        &a.display().to_string(),
        &Blows::read(b)?,
        &b.display().to_string(),
    );
    let path = output.join("random_diff.txt");
    fs::write(&path, &report)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    write_findings(&output.join("random_diff.csv"), &findings)?;
    print!("{report}");
    println!("wrote {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A blows file as `--random` first wrote it, before it said its seed.
    const OLDER: &str = "index,play,tool,bones_broken,skin_torn,muscle_torn,arteries_cut,blood_lost_ml,rules_broken,watchdogs\n";

    /// A blows file of `count` swings with the bat, the first `bones_out`
    /// of them pushing a whole bone out of the skin.
    fn blows(count: usize, bones_out: usize) -> Blows {
        let mut text = String::from("# settings: \n# trying: \n# seed: 1\n");
        text.push_str(OLDER);
        for index in 0..count {
            text.push_str(&format!(
                "{index},\"bat:-0.300,{:.3}:0.100,0.400:power=2.1:frames=12:settle=40\",bat,0,1,0,0,0.00,,{}\n",
                index as f64 / 1000.0,
                if index < bones_out { "WholeBoneOut@20" } else { "" }
            ));
        }
        let (source, blows) = parse_blows(&text).unwrap();
        Blows {
            sources: vec![source],
            blows,
        }
    }

    #[test]
    fn a_blows_file_reads_back_what_it_holds() {
        let text = format!(
            "{OLDER}0,\"hammer:-0.300,0.400:0.100,0.400:power=2.6:frames=12:settle=40\",hammer,1,2,3,4,5.00,bone_pieces_out;flesh_speed,WholeBoneOut@20;JointApart@31\n"
        );
        let (source, blows) = parse_blows(&text).unwrap();
        assert!(source.contains("written before"), "{source}");
        assert_eq!(blows.len(), 1);
        assert_eq!(blows[0].tool, "hammer");
        assert_eq!(blows[0].injuries, [1, 2, 3, 4]);
        assert!(blows[0].broken.contains("bone_pieces_out"));
        assert!(blows[0].tripped.contains(&Watchdog::JointApart));
        assert!(parse_blows("").is_err());
    }

    /// Thirty of the same blows that push a bone out on side B and none on
    /// side A: chance splits thirty blows all one way about once in a
    /// billion, and the finding says the change made it worse.
    #[test]
    fn the_same_blows_are_set_blow_against_blow() {
        let (report, findings) = blows_report(&blows(100, 0), "A", &blows(100, 30), "B");
        assert!(
            report.contains("Both sides drew the same blows"),
            "{report}"
        );
        assert!(!report.contains("aimed a little differently"), "{report}");
        assert!(
            report.contains("** a whole bone outside the skin: in 0 -> 30 plays"),
            "{report}"
        );
        assert!(report.contains("bat ** 0 -> 30"), "{report}");
        assert!(
            report.contains("Blows only B tripped a watchdog"),
            "{report}"
        );
        let bone = findings
            .iter()
            .find(|finding| finding.item == "a whole bone outside the skin")
            .unwrap();
        assert_eq!((bone.change, bone.holds()), ("worse", Holds::Must));
        // A blow either way is chance.
        let (_, findings) = blows_report(&blows(100, 0), "A", &blows(100, 1), "B");
        let bone = findings
            .iter()
            .find(|finding| finding.item == "a whole bone outside the skin")
            .unwrap();
        assert_eq!(bone.change, "same");
    }

    /// A blow aimed a little differently, as when the body settled another
    /// way, is still the same draw and set against its match.
    #[test]
    fn a_blow_aimed_a_little_differently_is_still_paired() {
        let mut moved = blows(100, 30);
        moved.blows[7].play = moved.blows[7].play.replace("0.400", "0.410");
        let (report, findings) = blows_report(&blows(100, 0), "A", &moved, "B");
        assert!(
            report.contains("Both sides drew the same blows, so each blow is set against itself (1 of 100 aimed a little differently"),
            "{report}"
        );
        let bone = findings
            .iter()
            .find(|finding| finding.item == "a whole bone outside the skin")
            .unwrap();
        assert_eq!(bone.change, "worse");
    }

    /// Blows that differ are compared as two samples.
    #[test]
    fn different_blows_are_compared_as_two_samples() {
        let (report, _) = blows_report(&blows(100, 0), "A", &blows(60, 0), "B");
        assert!(report.contains("compared as two samples"), "{report}");
        assert!(!report.contains("Blows only"));
    }

    #[test]
    fn a_folder_of_seeds_holds_blows() {
        let dir = env::temp_dir().join(format!("rp_random_diff_{}", process::id()));
        let (one, two) = (dir.join("seed-1"), dir.join("seed-2"));
        fs::create_dir_all(&one).unwrap();
        fs::create_dir_all(&two).unwrap();
        assert!(!holds_blows(&dir));
        fs::write(one.join(BLOWS_FILE), format!("# seed: 1\n{OLDER}")).unwrap();
        fs::write(two.join(BLOWS_FILE), format!("# seed: 2\n{OLDER}")).unwrap();
        assert!(holds_blows(&dir));
        assert!(holds_blows(&one));
        assert_eq!(
            blows_files(&dir),
            [one.join(BLOWS_FILE), two.join(BLOWS_FILE)]
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
