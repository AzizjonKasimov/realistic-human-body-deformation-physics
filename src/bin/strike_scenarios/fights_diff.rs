//! Two sets of random fights side by side (`--diff A B` on folders that hold
//! `random_fights.csv`, or on the files): each fighter's torn and slipped
//! joints, each joint that tore or slipped, its broken bones and torn skin,
//! and the fights each watchdog tripped in, with how likely chance is to make
//! each difference. Sides that played the same fights (the same count, seed,
//! and mix, drawn alike) are set fight against fight, by the sign test on the
//! fights that came out differently: a hundred fights so paired say more than
//! counts over a few seeds read by eye. Sides that played different fights
//! are compared as two samples, by Fisher's exact test on the fights each
//! thing happened in.

use super::*;
use diff::{fisher, header_line, sign_test, stars};
use fights::FIGHTS_FILE;
use rp::inspect::Watchdog;
use std::cmp::Ordering as Order;
use std::collections::BTreeSet;

/// Each fighter as a fights file's columns name it.
const FIGHTERS: [&str; 2] = ["left", "right"];

/// What a fight did to each fighter, as the columns after the fighter's name
/// hold it, and in words.
const COUNTS: [(&str, &str); 4] = [
    ("joints_torn", "joints torn apart"),
    ("joints_slipped", "joints slipped out of place"),
    ("bones_broken", "bones broken"),
    ("skin_torn", "skin torn"),
];

/// One fight as a fights file holds it.
struct FightLine {
    play: String,
    /// Each fighter's counts, in the order of `COUNTS`.
    counts: [[u32; 4]; 2],
    /// Each fighter's torn and slipped joints by name, when the file names
    /// them.
    torn: [Vec<String>; 2],
    slipped: [Vec<String>; 2],
    /// The watchdogs that tripped in it.
    tripped: BTreeSet<Watchdog>,
}

/// A fights file: what its fights were played with, and each fight.
struct Fights {
    settings: String,
    /// The experiments it tried (`RP_TRY`).
    trying: String,
    seed: String,
    mix: String,
    /// Whether the file names the joints that tore or slipped; one written
    /// before fights files did only counts them.
    names_joints: bool,
    fights: Vec<FightLine>,
}

/// The fights file `path` names: the file itself, or the one in its folder.
fn fights_file(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join(FIGHTS_FILE)
    } else {
        path.to_path_buf()
    }
}

/// Whether `path` holds random fights: a folder with a fights file, or such
/// a file under any name.
pub(crate) fn holds_fights(path: &Path) -> bool {
    header_line(&fights_file(path)).is_some_and(|header| header.starts_with("index,play,"))
}

/// A CSV line's fields, a quoted field taken whole, commas and all.
fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        let field = fields.last_mut().expect("a field");
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(String::new()),
            _ => field.push(c),
        }
    }
    fields
}

/// The watchdog a fights file names, by the name the code gives it
/// (`JointApart`) or its short name (`joint_apart`).
fn watchdog_named(name: &str) -> Option<Watchdog> {
    Watchdog::ALL
        .iter()
        .copied()
        .find(|watchdog| format!("{watchdog:?}") == name || watchdog.slug() == name)
}

impl Fights {
    fn read(path: &Path) -> Result<Fights, String> {
        let file = fights_file(path);
        let text = fs::read_to_string(&file)
            .map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        Fights::parse(&text).map_err(|error| format!("{}: {error}", file.display()))
    }

    /// The fights a fights file holds, from its text.
    fn parse(text: &str) -> Result<Fights, String> {
        let mut fights = Fights {
            settings: String::new(),
            trying: String::new(),
            seed: String::new(),
            mix: String::new(),
            names_joints: false,
            fights: Vec::new(),
        };
        let mut columns: Vec<String> = Vec::new();
        for line in text.lines() {
            if let Some(comment) = line.strip_prefix('#') {
                if let Some((key, value)) = comment.split_once(':') {
                    let value = value.trim().to_string();
                    match key.trim() {
                        "settings" => fights.settings = value,
                        "trying" => fights.trying = value,
                        "seed" => fights.seed = value,
                        "mix" => fights.mix = value,
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
                fights.names_joints = columns.iter().any(|column| column == "left_torn_joints");
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
            let mut counts = [[0; 4]; 2];
            for (side, fighter) in FIGHTERS.iter().enumerate() {
                for (index, (column, _)) in COUNTS.iter().enumerate() {
                    let name = format!("{fighter}_{column}");
                    counts[side][index] = field(&name)?
                        .parse()
                        .map_err(|_| format!("unreadable {name} in `{line}`"))?;
                }
            }
            let joints = |name: &str| -> Vec<String> {
                field(name)
                    .map(|names| {
                        names
                            .split(';')
                            .filter(|joint| !joint.is_empty())
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let tripped = field("watchdogs")?
                .split(';')
                .filter(|entry| !entry.is_empty())
                .map(|entry| {
                    let name = entry.split('@').next().unwrap_or(entry);
                    watchdog_named(name).ok_or_else(|| format!("unknown watchdog `{name}`"))
                })
                .collect::<Result<_, _>>()?;
            fights.fights.push(FightLine {
                play: field("play")?.to_string(),
                counts,
                torn: [joints("left_torn_joints"), joints("right_torn_joints")],
                slipped: [
                    joints("left_slipped_joints"),
                    joints("right_slipped_joints"),
                ],
                tripped,
            });
        }
        if columns.is_empty() {
            return Err("no header line".to_string());
        }
        Ok(fights)
    }

    /// What the fights were played with, for the report's heading.
    fn played(&self) -> String {
        let mut parts = Vec::new();
        if !self.seed.is_empty() {
            parts.push(format!("seed {}", self.seed));
        }
        if !self.mix.is_empty() {
            parts.push(format!("the {} mix", self.mix));
        }
        if !self.settings.is_empty() {
            parts.push(self.settings.clone());
        }
        if !self.trying.is_empty() {
            parts.push(format!("trying {}", self.trying));
        }
        if self.seed.is_empty() && self.mix.is_empty() {
            parts.push("written before fights files said what they were played with".to_string());
        } else if self.settings.is_empty() && self.trying.is_empty() {
            parts.push("defaults".to_string());
        }
        format!("{} fights, {}", self.fights.len(), parts.join(", "))
    }
}

/// How one thing a fight can do, counted per fight, differs between A's
/// fights and B's.
struct Change {
    /// The fights it happened in on each side, and how many times in all.
    fights: (usize, usize),
    total: (u64, u64),
    /// The fights in which B had more of it than A, and fewer; only paired.
    more: usize,
    fewer: usize,
    /// How likely chance is to make a difference as large.
    p: f64,
}

impl Change {
    fn of(
        a: &[FightLine],
        b: &[FightLine],
        paired: bool,
        count: &dyn Fn(&FightLine) -> u32,
    ) -> Change {
        let happened =
            |fights: &[FightLine]| fights.iter().filter(|fight| count(fight) > 0).count();
        let total = |fights: &[FightLine]| {
            fights
                .iter()
                .map(|fight| u64::from(count(fight)))
                .sum::<u64>()
        };
        let fights = (happened(a), happened(b));
        let (mut more, mut fewer) = (0, 0);
        let p = if paired {
            for (x, y) in a.iter().zip(b) {
                match count(y).cmp(&count(x)) {
                    Order::Greater => more += 1,
                    Order::Less => fewer += 1,
                    Order::Equal => {}
                }
            }
            sign_test(more, fewer)
        } else {
            fisher(fights.0, a.len(), fights.1, b.len())
        };
        Change {
            fights,
            total: (total(a), total(b)),
            more,
            fewer,
            p,
        }
    }

    /// The change as a report's line: how sure, what, and the counts A -> B.
    fn line(&self, what: &str, paired: bool) -> String {
        let mut line = format!(
            "{} {what}: in {} -> {} fights",
            stars(self.p),
            self.fights.0,
            self.fights.1
        );
        if self.total != (self.fights.0 as u64, self.fights.1 as u64) {
            line.push_str(&format!(", {} -> {} in all", self.total.0, self.total.1));
        }
        if paired && self.more + self.fewer > 0 {
            line.push_str(&format!(
                " (B more in {} fights, fewer in {})",
                self.more, self.fewer
            ));
        }
        line
    }
}

/// A joint's name without the fighter it belongs to, which the section it
/// is listed in already says.
fn short_name(name: &str) -> &str {
    ["the left fighter's ", "the right fighter's "]
        .iter()
        .find_map(|fighter| name.strip_prefix(fighter))
        .unwrap_or(name)
}

fn torn(fight: &FightLine, side: usize) -> &[String] {
    &fight.torn[side]
}

fn slipped(fight: &FightLine, side: usize) -> &[String] {
    &fight.slipped[side]
}

/// A line for each joint of fighter `side` that tore (or slipped, as
/// `joints` reads) in either side's fights, the most often first.
fn joint_lines(
    a: &[FightLine],
    b: &[FightLine],
    paired: bool,
    side: usize,
    joints: fn(&FightLine, usize) -> &[String],
) -> Vec<String> {
    let names: BTreeSet<&str> = a
        .iter()
        .chain(b)
        .flat_map(|fight| joints(fight, side).iter().map(String::as_str))
        .collect();
    let mut lines: Vec<(u64, &str, String)> = names
        .into_iter()
        .map(|name| {
            let count = |fight: &FightLine| {
                joints(fight, side)
                    .iter()
                    .filter(|joint| joint.as_str() == name)
                    .count() as u32
            };
            let change = Change::of(a, b, paired, &count);
            (
                change.total.0 + change.total.1,
                name,
                change.line(short_name(name), paired),
            )
        })
        .collect();
    lines.sort_by(|x, y| y.0.cmp(&x.0).then_with(|| x.1.cmp(y.1)));
    lines.into_iter().map(|(_, _, line)| line).collect()
}

/// What fight `worse` did beyond fight `better`, the same fight played by
/// the other side, counts written A -> B (`a_worse` says which is A) and the
/// joints named as the worse side tore or slipped them.
fn what_was_worse(worse: &FightLine, better: &FightLine, a_worse: bool) -> String {
    let mut parts = Vec::new();
    for (side, fighter) in FIGHTERS.iter().enumerate() {
        for (index, (_, what)) in COUNTS.iter().enumerate() {
            let (high, low) = (worse.counts[side][index], better.counts[side][index]);
            if high <= low {
                continue;
            }
            let (from, to) = if a_worse { (high, low) } else { (low, high) };
            let mut part = format!("the {fighter} fighter's {what} {from} -> {to}");
            let names: &[String] = match index {
                0 => &worse.torn[side],
                1 => &worse.slipped[side],
                _ => &[],
            };
            if !names.is_empty() {
                let names: Vec<&str> = names.iter().map(|name| short_name(name)).collect();
                part.push_str(&format!(
                    " ({}{})",
                    if a_worse { "A: " } else { "" },
                    names.join(", ")
                ));
            }
            parts.push(part);
        }
    }
    parts.join("; ")
}

/// The report setting fights `b` beside fights `a`, each headed by `label`.
fn fights_report(a: &Fights, a_label: &str, b: &Fights, b_label: &str) -> String {
    let paired = !a.fights.is_empty()
        && a.fights.len() == b.fights.len()
        && a.fights
            .iter()
            .zip(&b.fights)
            .all(|(x, y)| x.play == y.play);
    let mut report = format!(
        "Fights A: {a_label} ({})\nFights B: {b_label} ({})\n",
        a.played(),
        b.played()
    );
    report.push_str(if paired {
        "Both sides played the same fights, so each fight is set against itself. Each count is the fights a thing happened in, then how many times in all, A -> B; * a difference chance gives under 5% of the time, ** under 1% (the sign test on the fights in which B had more or fewer of it than A).\n"
    } else {
        "The sides played different fights (another count, seed, or mix, or fights drawn another way), so they are compared as two samples. Each count is the fights a thing happened in, then how many times in all, A -> B; * a difference in fights chance gives under 5% of the time, ** under 1% (Fisher's exact test).\n"
    });
    let names_joints = a.names_joints && b.names_joints;
    if !names_joints {
        report.push_str(&format!(
            "Fights {} do not name their joints (written before fights files did), so joints are counted, not named.\n",
            match (a.names_joints, b.names_joints) {
                (false, false) => "A and B",
                (false, true) => "A",
                _ => "B",
            }
        ));
    }
    for (side, fighter) in FIGHTERS.iter().enumerate() {
        report.push_str(&format!("\nThe {fighter} fighter:\n"));
        for (index, (_, what)) in COUNTS.iter().enumerate() {
            let change = Change::of(&a.fights, &b.fights, paired, &|fight| {
                fight.counts[side][index]
            });
            report.push_str(&format!("  {}\n", change.line(what, paired)));
            let joints: Option<fn(&FightLine, usize) -> &[String]> = match index {
                0 => Some(torn),
                1 => Some(slipped),
                _ => None,
            };
            if let (Some(joints), true) = (joints, names_joints) {
                for line in joint_lines(&a.fights, &b.fights, paired, side, joints) {
                    report.push_str(&format!("      {line}\n"));
                }
            }
        }
    }
    let kinds: BTreeSet<Watchdog> = a
        .fights
        .iter()
        .chain(&b.fights)
        .flat_map(|fight| fight.tripped.iter().copied())
        .collect();
    report.push_str("\nWatchdogs, the fights each tripped in:\n");
    if kinds.is_empty() {
        report.push_str("  none tripped in either side's fights\n");
    }
    for kind in kinds {
        let change = Change::of(&a.fights, &b.fights, paired, &|fight| {
            u32::from(fight.tripped.contains(&kind))
        });
        let what = format!(
            "{}{}",
            kind.describe(),
            if kind.known_gap() { " [known gap]" } else { "" }
        );
        report.push_str(&format!("  {}\n", change.line(&what, paired)));
    }
    if paired {
        // The fights one side came out of more hurt, the most first, to
        // replay with --gesture and follow with --journal and --why.
        for (worse_side, a_worse) in [("B", false), ("A", true)] {
            let mut worse: Vec<(u32, String)> = a
                .fights
                .iter()
                .zip(&b.fights)
                .filter_map(|(x, y)| {
                    let (worse, better) = if a_worse { (x, y) } else { (y, x) };
                    let beyond: u32 = (0..2)
                        .flat_map(|side| {
                            (0..COUNTS.len()).map(move |index| {
                                worse.counts[side][index].saturating_sub(better.counts[side][index])
                            })
                        })
                        .sum();
                    (beyond > 0).then(|| {
                        (
                            beyond,
                            format!(
                                "  --gesture \"{}\": {}",
                                worse.play,
                                what_was_worse(worse, better, a_worse)
                            ),
                        )
                    })
                })
                .collect();
            if worse.is_empty() {
                continue;
            }
            worse.sort_by(|x, y| y.0.cmp(&x.0));
            report.push_str(&format!(
                "\nFights {worse_side} came out of more hurt, {} in all; the three most:\n",
                worse.len()
            ));
            for (_, line) in worse.iter().take(3) {
                report.push_str(&format!("{line}\n"));
            }
        }
    }
    report
}

/// Sets the fights in `b` beside those in `a`, each a folder holding a
/// fights file or the file itself, writes the report to `output`
/// (`fights_diff.txt`), and prints it.
pub(crate) fn diff_fights(a: &Path, b: &Path, output: &Path) -> Result<(), String> {
    let report = fights_report(
        &Fights::read(a)?,
        &a.display().to_string(),
        &Fights::read(b)?,
        &b.display().to_string(),
    );
    let path = output.join("fights_diff.txt");
    fs::write(&path, &report)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    print!("{report}");
    println!("wrote {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fights file as `--fights` first wrote it, before it said what its
    /// fights were played with or named the joints.
    const OLDER: &str = "index,play,left_joints_torn,right_joints_torn,left_joints_slipped,right_joints_slipped,left_bones_broken,right_bones_broken,left_skin_torn,right_skin_torn,watchdogs\n\
        0,\"hammer:-1.300,0.950:wait=10:punch=right\",1,0,0,0,0,0,0,0,InsideOut@27\n\
        1,\"hammer:-1.300,0.950:wait=12:punch=right\",0,0,0,1,0,0,2,0,\n";

    /// One line of a fights file as it is written now.
    fn line(index: usize, left_torn: &[&str], right_slipped: &[&str], watchdogs: &str) -> String {
        format!(
            "{index},\"hammer:-1.300,0.950:wait={}:punch=right\",{},0,0,{},0,0,0,0,{watchdogs},\"{}\",\"\",\"\",\"{}\"\n",
            10 + index,
            left_torn.len(),
            right_slipped.len(),
            left_torn.join(";"),
            right_slipped.join(";")
        )
    }

    fn newer(lines: &[String], settings: &str) -> Fights {
        let mut text = format!(
            "# settings: {settings}\n# trying: \n# seed: 1\n# mix: game\nindex,play,left_joints_torn,right_joints_torn,left_joints_slipped,right_joints_slipped,left_bones_broken,right_bones_broken,left_skin_torn,right_skin_torn,watchdogs,left_torn_joints,right_torn_joints,left_slipped_joints,right_slipped_joints\n"
        );
        for line in lines {
            text.push_str(line);
        }
        Fights::parse(&text).unwrap()
    }

    #[test]
    fn a_quoted_field_keeps_its_commas() {
        assert_eq!(
            csv_fields("0,\"hammer:-1.300,0.950\",1,,\"a \"\"b\"\"\""),
            ["0", "hammer:-1.300,0.950", "1", "", "a \"b\""]
        );
    }

    /// A file written before fights files said what they were played with
    /// reads back what it holds, and says it names no joints.
    #[test]
    fn an_older_fights_file_reads_back_what_it_holds() {
        let fights = Fights::parse(OLDER).unwrap();
        assert_eq!(fights.fights.len(), 2);
        assert!(!fights.names_joints);
        assert_eq!(
            fights.fights[0].play,
            "hammer:-1.300,0.950:wait=10:punch=right"
        );
        assert_eq!(fights.fights[0].counts[0], [1, 0, 0, 0]);
        assert_eq!(fights.fights[1].counts, [[0, 0, 0, 2], [0, 1, 0, 0]]);
        assert!(fights.fights[0].tripped.contains(&Watchdog::InsideOut));
        assert!(fights.fights[1].tripped.is_empty());
        assert!(fights.played().contains("written before"));
    }

    #[test]
    fn a_fights_file_reads_its_settings_and_joints() {
        let fights = newer(
            &[line(
                0,
                &["the left fighter's right wrist"],
                &[],
                "joint_apart@40",
            )],
            "gravity=2",
        );
        assert!(fights.names_joints);
        assert_eq!(fights.played(), "1 fights, seed 1, the game mix, gravity=2");
        assert_eq!(fights.fights[0].torn[0], ["the left fighter's right wrist"]);
        assert!(fights.fights[0].tripped.contains(&Watchdog::JointApart));
    }

    /// Ten fights that tear a wrist on side A and none on side B, the same
    /// fights: chance splits ten fights all one way 2 times in 1024.
    #[test]
    fn paired_fights_are_set_fight_against_fight() {
        let wrist = ["the left fighter's right wrist"];
        let a: Vec<String> = (0..20)
            .map(|index| line(index, if index < 10 { &wrist[..] } else { &[] }, &[], ""))
            .collect();
        let b: Vec<String> = (0..20).map(|index| line(index, &[], &[], "")).collect();
        let report = fights_report(&newer(&a, ""), "A", &newer(&b, ""), "B");
        assert!(report.contains("Both sides played the same fights"));
        assert!(
            report.contains(
                "** joints torn apart: in 10 -> 0 fights (B more in 0 fights, fewer in 10)"
            ),
            "{report}"
        );
        assert!(
            report.contains("** right wrist: in 10 -> 0 fights"),
            "{report}"
        );
        assert!(report.contains("Fights A came out of more hurt, 10 in all"));
        assert!(!report.contains("Fights B came out of more hurt"));
        // One fight torn either way is well within chance.
        let c: Vec<String> = (0..20)
            .map(|index| line(index, if index == 3 { &wrist[..] } else { &[] }, &[], ""))
            .collect();
        let report = fights_report(&newer(&c, ""), "A", &newer(&b, ""), "B");
        assert!(
            report.contains("   joints torn apart: in 1 -> 0 fights"),
            "{report}"
        );
    }

    /// Fights that differ are compared as two samples, and a file that
    /// names no joints leaves the joints unnamed on both sides.
    #[test]
    fn different_fights_are_compared_as_two_samples() {
        let b: Vec<String> = (5..7).map(|index| line(index, &[], &[], "")).collect();
        let report = fights_report(&Fights::parse(OLDER).unwrap(), "A", &newer(&b, ""), "B");
        assert!(report.contains("compared as two samples"), "{report}");
        assert!(report.contains("Fights A do not name their joints"));
        assert!(report.contains("flesh turned inside out: in 1 -> 0 fights"));
        assert!(!report.contains("came out of more hurt"));
    }

    #[test]
    fn a_folder_or_a_file_holds_fights_by_its_header() {
        let dir = env::temp_dir().join(format!("rp_fights_diff_{}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(!holds_fights(&dir));
        fs::write(dir.join(FIGHTS_FILE), OLDER).unwrap();
        assert!(holds_fights(&dir));
        let renamed = dir.join("base.csv");
        fs::write(&renamed, format!("# seed: 1\n{OLDER}")).unwrap();
        assert!(holds_fights(&renamed));
        let runs = dir.join("runs.csv");
        fs::write(
            &runs,
            "# settings: \nscenario,along,across,strength,kind,name,value\n",
        )
        .unwrap();
        assert!(!holds_fights(&runs));
        assert!(diff::holds_runs(&runs));
        fs::remove_dir_all(&dir).unwrap();
    }
}
