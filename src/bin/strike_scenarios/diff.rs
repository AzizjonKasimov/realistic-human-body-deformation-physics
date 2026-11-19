//! Two sweeps side by side (`--diff A B`): every run's outcomes, which the
//! sweep writes one to a line (`RUNS_FILE`), compared scenario by scenario,
//! with how likely each difference is to be chance. Forty-five runs are few:
//! a real-life check at 20 of 45 against 24 of 45 may be the same engine
//! aimed a little differently, and only a test says how sure a difference is.

use super::*;
use rp::inspect::Watchdog;
use rp::scenarios::Often;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The file a sweep writes its runs to, next to its other outputs.
pub(crate) const RUNS_FILE: &str = "strike_sweep_runs.csv";

/// The numbers a run's line keeps besides its outcomes: the ones that most
/// often move when the engine changes, and how much the play touched the
/// body (points and bones touched, summed over its frames).
pub(crate) fn run_numbers(result: &ScenarioResult) -> Vec<(String, f64)> {
    let mut numbers = vec![
        (
            "contacts".to_string(),
            f64::from(result.tissue_contacts + result.bone_contacts),
        ),
        ("bones_broken".to_string(), f64::from(result.bone_fractures)),
        ("skin_torn".to_string(), f64::from(result.skin_tears)),
        ("muscle_torn".to_string(), f64::from(result.muscle_tears)),
        ("bruises".to_string(), f64::from(result.contusion_events)),
        (
            "arteries_cut".to_string(),
            f64::from(result.vessel_lacerations + result.fragment_vessel_lacerations),
        ),
        (
            "blood_lost_ml".to_string(),
            result.blood_loss * rp::scenarios::BLOOD_VOLUME_ML,
        ),
        ("fastest_tissue".to_string(), result.max_point_speed),
        ("fist_knocked".to_string(), result.max_fist_shift),
        ("fist_off_guard".to_string(), result.final_fist_offset),
        ("head_turn".to_string(), result.max_head_turn),
        ("pieces_out".to_string(), f64::from(result.bone_pieces_out)),
        (
            "closed_pieces_out".to_string(),
            f64::from(result.closed_pieces_out),
        ),
        (
            "whole_bones_out".to_string(),
            f64::from(result.whole_bones_out),
        ),
    ];
    for part in rp::BonePart::ALL {
        let load = result.part_peak_load[part.index()];
        if load > 0.0 {
            numbers.push((format!("load_{}", part.name().replace(' ', "_")), load));
        }
    }
    numbers
}

/// How sure the sweep is of a real-life check's frequency: `None` when the
/// limit of `often` lies outside the 95% interval of `hits` in `runs`
/// (Wilson's), else a note that a few runs more or less could carry it
/// across. Never and Always turn on any single run, so they are never
/// borderline, and Sometimes is when it hits all but two or at most two.
pub(crate) fn borderline(often: Often, hits: usize, runs: usize) -> Option<String> {
    if runs == 0 {
        return None;
    }
    let (low, high) = wilson(hits, runs);
    let limit = match often {
        Often::Rarely => 1.0 / 3.0,
        Often::Usually => 0.5,
        Often::Sometimes => {
            return (hits <= 2 || hits + 2 >= runs).then(|| {
                format!(
                    "borderline: {hits}/{runs} is close to {}",
                    if hits <= 2 { "never" } else { "always" }
                )
            });
        }
        Often::Never | Often::Always => return None,
    };
    (low <= limit && limit <= high).then(|| {
        format!(
            "borderline: {hits}/{runs}, 95% between {:.0}% and {:.0}%, spans the {:.0}% limit",
            low * 100.0,
            high * 100.0,
            limit * 100.0
        )
    })
}

/// Wilson's 95% interval for a frequency of `hits` in `runs`.
fn wilson(hits: usize, runs: usize) -> (f64, f64) {
    let n = runs as f64;
    let p = hits as f64 / n;
    let z = 1.96f64;
    let centre = (p + z * z / (2.0 * n)) / (1.0 + z * z / n);
    let half = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / (1.0 + z * z / n);
    ((centre - half).max(0.0), (centre + half).min(1.0))
}

/// Natural logarithms of 0! to `n`!.
fn log_factorials(n: usize) -> Vec<f64> {
    let mut table = vec![0.0; n + 1];
    for i in 1..=n {
        table[i] = table[i - 1] + (i as f64).ln();
    }
    table
}

/// Fisher's exact test, two-sided: how likely a split at least as uneven
/// as `a` of `n1` against `b` of `n2` is if both came from one engine.
pub(crate) fn fisher(a: usize, n1: usize, b: usize, n2: usize) -> f64 {
    let n = n1 + n2;
    let hits = a + b;
    if n == 0 || hits == 0 || hits == n {
        return 1.0;
    }
    let lf = log_factorials(n);
    let probability = |x: usize| {
        (lf[hits] + lf[n - hits] + lf[n1] + lf[n2]
            - lf[n]
            - lf[x]
            - lf[n1 - x]
            - lf[hits - x]
            - lf[n2 + x - hits])
            .exp()
    };
    let observed = probability(a);
    let low = hits.saturating_sub(n2);
    let high = hits.min(n1);
    (low..=high)
        .map(probability)
        .filter(|&p| p <= observed * (1.0 + 1e-9))
        .sum::<f64>()
        .min(1.0)
}

/// The sign test on paired runs: how likely `more` runs going up and `less`
/// going down is, or a split more uneven, if the change did nothing.
pub(crate) fn sign_test(more: usize, less: usize) -> f64 {
    let n = more + less;
    if n == 0 {
        return 1.0;
    }
    let k = more.min(less);
    let lf = log_factorials(n);
    let tail: f64 = (0..=k)
        .map(|i| (lf[n] - lf[i] - lf[n - i] - n as f64 * std::f64::consts::LN_2).exp())
        .sum();
    (2.0 * tail).min(1.0)
}

/// Whether a number's median moved enough to list: chance makes a shift as
/// sure as `p` less than 5% of the time, and it moved a tenth and more than
/// a hundredth. A shift every run makes alike can be sure and still too
/// small to matter.
pub(crate) fn number_moved(before: f64, after: f64, p: f64) -> bool {
    let moved = (after - before).abs();
    p < 0.05 && moved >= 0.1 * before.abs().max(after.abs()) && moved > 0.01
}

/// How sure a difference is, for a report: `**` past 1% chance, `*` past 5%.
pub(crate) fn stars(p: f64) -> &'static str {
    if p < 0.01 {
        "**"
    } else if p < SURE {
        "* "
    } else {
        "  "
    }
}

/// How unlikely chance must be to make a difference for it to count as
/// more than chance: under 5%, which the reports mark `*` (and `**` under
/// 1%).
pub(crate) const SURE: f64 = 0.05;

/// How one thing that can happen in a play (a fight, a blow), counted per
/// play, differs between side A's plays and side B's.
pub(crate) struct Change {
    /// The plays it happened in on each side, and how many times in all.
    pub(crate) plays: (usize, usize),
    pub(crate) total: (u64, u64),
    /// How many plays each side played.
    pub(crate) of: (usize, usize),
    /// The plays in which B had more of it than A, and fewer; only paired.
    pub(crate) more: usize,
    pub(crate) fewer: usize,
    /// How likely chance is to make a difference as large.
    pub(crate) p: f64,
}

impl Change {
    /// Sides that played the same plays (`paired`) are set play against
    /// play, by the sign test on the plays in which B had more or fewer of it
    /// than A; other sides as two samples, by Fisher's exact test on the
    /// plays it happened in.
    pub(crate) fn of<T>(a: &[T], b: &[T], paired: bool, count: &dyn Fn(&T) -> u32) -> Change {
        let happened = |plays: &[T]| plays.iter().filter(|play| count(play) > 0).count();
        let total = |plays: &[T]| plays.iter().map(|play| u64::from(count(play))).sum::<u64>();
        let plays = (happened(a), happened(b));
        let (mut more, mut fewer) = (0, 0);
        let p = if paired {
            for (x, y) in a.iter().zip(b) {
                match count(y).cmp(&count(x)) {
                    std::cmp::Ordering::Greater => more += 1,
                    std::cmp::Ordering::Less => fewer += 1,
                    std::cmp::Ordering::Equal => {}
                }
            }
            sign_test(more, fewer)
        } else {
            fisher(plays.0, a.len(), plays.1, b.len())
        };
        Change {
            plays,
            total: (total(a), total(b)),
            of: (a.len(), b.len()),
            more,
            fewer,
            p,
        }
    }

    /// Whether B has more of it than A, and whether less: play against
    /// play when paired, else by the share of plays it happened in.
    fn direction(&self, paired: bool) -> (bool, bool) {
        if paired {
            (self.more > self.fewer, self.fewer > self.more)
        } else {
            let a = self.plays.0 as u64 * self.of.1 as u64;
            let b = self.plays.1 as u64 * self.of.0 as u64;
            (b > a, b < a)
        }
    }

    /// The change as a report's line: how sure, what, and the counts A ->
    /// B, in `plays` (fights, plays).
    pub(crate) fn line(&self, what: &str, paired: bool, plays: &str) -> String {
        let mut line = format!(
            "{} {what}: in {} -> {} {plays}",
            stars(self.p),
            self.plays.0,
            self.plays.1
        );
        if self.total != (self.plays.0 as u64, self.plays.1 as u64) {
            line.push_str(&format!(", {} -> {} in all", self.total.0, self.total.1));
        }
        if paired && self.more + self.fewer > 0 {
            line.push_str(&format!(
                " (B more in {} {plays}, fewer in {})",
                self.more, self.fewer
            ));
        }
        line
    }

    /// The change as a finding, the plays it happened in out of each side's:
    /// worse or better beyond chance by `worse`, else the same.
    pub(crate) fn finding(
        &self,
        section: &str,
        item: &str,
        paired: bool,
        worse: Worse,
        holds: Holds,
    ) -> Finding {
        let (rose, fell) = self.direction(paired);
        Finding::new(
            section,
            item,
            (self.plays.0 as u64, self.of.0 as u64),
            (self.plays.1 as u64, self.of.1 as u64),
            self.p,
            judged(self.p, rose, fell, worse),
        )
        .holding(holds)
    }
}

/// Which way a finding gets worse: as it grows (a watchdog's trips), as it
/// shrinks (runs in band), or neither (an injury count, which a change may
/// mean to move).
#[derive(Clone, Copy)]
pub(crate) enum Worse {
    Up,
    Down,
    Neither,
}

/// What a difference means: worse or better beyond chance, `moved` beyond
/// chance when neither way is worse, or the same.
pub(crate) fn judged(p: f64, rose: bool, fell: bool, worse: Worse) -> &'static str {
    if p >= SURE || (!rose && !fell) {
        return "same";
    }
    match worse {
        Worse::Up if rose => "worse",
        Worse::Down if fell => "worse",
        Worse::Neither => "moved",
        _ => "better",
    }
}

/// Whether a change must not make a finding clearly worse before it merges
/// (`Must`: the tests, the bands and real-life checks across the sweep, the
/// rules every play follows, and the watchdogs for a body that breaks), may
/// let it drift, reported (`Drift`), or whether the finding details another
/// and is listed only (`Detail`): README, "Check A Change Before Merging".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Holds {
    Must,
    Drift,
    Detail,
}

impl Holds {
    fn name(self) -> &'static str {
        match self {
            Holds::Must => "must",
            Holds::Drift => "drift",
            Holds::Detail => "detail",
        }
    }
}

/// One thing a diff compared, as its findings file holds it for
/// `tools\merge_check.ps1`: how often it happened in A and in B out of how
/// many, how likely chance is to make the difference, and what that means.
#[derive(Clone, Debug)]
pub(crate) struct Finding {
    pub(crate) section: String,
    pub(crate) item: String,
    pub(crate) a: (u64, u64),
    pub(crate) b: (u64, u64),
    pub(crate) p: f64,
    /// worse, better, moved, borderline (a real-life check a few runs could
    /// flip, reported), or same.
    pub(crate) change: &'static str,
    holds: Option<Holds>,
}

impl Finding {
    pub(crate) fn new(
        section: &str,
        item: &str,
        a: (u64, u64),
        b: (u64, u64),
        p: f64,
        change: &'static str,
    ) -> Finding {
        Finding {
            section: section.to_string(),
            item: item.to_string(),
            a,
            b,
            p,
            change,
            holds: None,
        }
    }

    pub(crate) fn holding(mut self, holds: Holds) -> Finding {
        self.holds = Some(holds);
        self
    }

    pub(crate) fn holds(&self) -> Holds {
        self.holds.unwrap_or(Holds::Detail)
    }
}

/// The first line of a findings file, which `tools\merge_check.ps1` reads.
const FINDINGS_HEADER: &str = "section,item,a,a_of,b,b_of,p,change,holds";

/// A field for a CSV line, quoted when it holds a comma or a quote.
fn csv_field(text: &str) -> String {
    if text.contains([',', '"']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// Writes `findings` to `path`, a findings file (`*_diff.csv`) beside a
/// diff's report.
pub(crate) fn write_findings(path: &Path, findings: &[Finding]) -> Result<(), String> {
    let mut text = format!("{FINDINGS_HEADER}\n");
    for finding in findings {
        text.push_str(&format!(
            "{},{},{},{},{},{},{:.6},{},{}\n",
            csv_field(&finding.section),
            csv_field(&finding.item),
            finding.a.0,
            finding.a.1,
            finding.b.0,
            finding.b.1,
            finding.p,
            finding.change,
            finding.holds().name()
        ));
    }
    fs::write(path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// The first line of the file at `path` that is not a comment: a CSV's
/// header, which says what the file holds.
pub(crate) fn header_line(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    std::io::BufRead::lines(std::io::BufReader::new(file))
        .map_while(Result::ok)
        .find(|line| !line.starts_with('#'))
}

/// Whether `path` holds a sweep's runs: a folder with a `RUNS_FILE`, or such
/// a file under any name.
pub(crate) fn holds_runs(path: &Path) -> bool {
    let file = if path.is_dir() {
        path.join(RUNS_FILE)
    } else {
        path.to_path_buf()
    };
    header_line(&file).is_some_and(|header| header.starts_with("scenario,along,"))
}

/// One sweep's runs as its file holds them.
struct Runs {
    settings: String,
    /// The experiments it tried (`RP_TRY`).
    trying: String,
    /// For each scenario, its runs by their variant (along, across, strength).
    variants: BTreeMap<String, BTreeSet<String>>,
    /// For each scenario, kind, and name, each run's value by its variant.
    values: HashMap<(String, String, String), HashMap<String, f64>>,
    /// Every number the sweep wrote, in any scenario.
    numbers: BTreeSet<String>,
    /// Every watchdog the sweep wrote, in any scenario: each run writes
    /// every watchdog its build has, tripped or not.
    watchdogs: BTreeSet<String>,
}

impl Runs {
    fn read(path: &Path) -> Result<Runs, String> {
        let file = if path.is_dir() {
            path.join(RUNS_FILE)
        } else {
            path.to_path_buf()
        };
        let text = fs::read_to_string(&file)
            .map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        Runs::parse(&text).map_err(|error| format!("{}: {error}", file.display()))
    }

    /// The runs a `RUNS_FILE` holds, from its text.
    fn parse(text: &str) -> Result<Runs, String> {
        let mut runs = Runs {
            settings: String::new(),
            trying: String::new(),
            variants: BTreeMap::new(),
            values: HashMap::new(),
            numbers: BTreeSet::new(),
            watchdogs: BTreeSet::new(),
        };
        for line in text.lines() {
            if let Some(settings) = line.strip_prefix("# settings:") {
                runs.settings = settings.trim().to_string();
                continue;
            }
            if let Some(tried) = line.strip_prefix("# trying:") {
                runs.trying = tried.trim().to_string();
                continue;
            }
            if line.starts_with('#') || line.starts_with("scenario,") || line.is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.split(',').collect();
            let [scenario, along, across, strength, kind, name, value] = fields[..] else {
                return Err(format!("unreadable line `{line}`"));
            };
            let variant = format!("{along},{across},{strength}");
            let value: f64 = value
                .parse()
                .map_err(|_| format!("unreadable value in `{line}`"))?;
            runs.variants
                .entry(scenario.to_string())
                .or_default()
                .insert(variant.clone());
            if kind == "number" {
                runs.numbers.insert(name.to_string());
            }
            if kind == "watchdog" {
                runs.watchdogs.insert(name.to_string());
            }
            runs.values
                .entry((scenario.to_string(), kind.to_string(), name.to_string()))
                .or_default()
                .insert(variant, value);
        }
        Ok(runs)
    }

    /// How many of `scenario`'s runs `kind` `name` happened in (absent, 0).
    fn hits(&self, scenario: &str, kind: &str, name: &str) -> usize {
        self.values
            .get(&(scenario.to_string(), kind.to_string(), name.to_string()))
            .map_or(0, |values| {
                values.values().filter(|&&value| value > 0.5).count()
            })
    }

    fn runs(&self, scenario: &str) -> usize {
        self.variants.get(scenario).map_or(0, BTreeSet::len)
    }

    /// The names `scenario` has of `kind`.
    fn names(&self, scenario: &str, kind: &str) -> BTreeSet<String> {
        self.values
            .keys()
            .filter(|(s, k, _)| s == scenario && k == kind)
            .map(|(_, _, name)| name.clone())
            .collect()
    }

    /// Whether the sweep keeps the number `name` at all, so that a run
    /// without it had it at nought. A sweep made before a number was added
    /// keeps none of it, and setting it against nought would list a change
    /// in every scenario. A bone's load is written only when it was loaded.
    fn keeps(&self, name: &str) -> bool {
        name.starts_with("load_") || self.numbers.contains(name)
    }

    /// Whether the sweep's build watched for `watchdog` at all: a sweep
    /// made before a watchdog was added never tripped it, and setting it
    /// against nought would read as the newer build worse.
    fn watches(&self, watchdog: Watchdog) -> bool {
        self.watchdogs.contains(watchdog.slug())
    }

    fn value(&self, scenario: &str, name: &str, variant: &str) -> Option<f64> {
        self.values
            .get(&(scenario.to_string(), "number".to_string(), name.to_string()))
            .and_then(|values| values.get(variant))
            .copied()
    }
}

pub(crate) fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        (values[middle - 1] + values[middle]) / 2.0
    }
}

/// Sets sweep `b` beside sweep `a`, each a folder holding a sweep's
/// `RUNS_FILE` or the file itself, writes the report to `output`, and prints
/// it. Every difference is listed with how sure it is; `--all` would list
/// the numbers that did not move too.
pub(crate) fn diff_sweeps(a: &Path, b: &Path, output: &Path) -> Result<(), String> {
    let (left, right) = (Runs::read(a)?, Runs::read(b)?);
    // What each side changed: its settings, and the experiments it tried.
    let changed = |runs: &Runs| {
        let mut parts = Vec::new();
        if !runs.settings.is_empty() {
            parts.push(runs.settings.clone());
        }
        if !runs.trying.is_empty() {
            parts.push(format!("trying {}", runs.trying));
        }
        if parts.is_empty() {
            "defaults".to_string()
        } else {
            parts.join("; ")
        }
    };
    let mut report = String::new();
    report.push_str(&format!(
        "Sweep A: {} ({})\nSweep B: {} ({})\n",
        a.display(),
        changed(&left),
        b.display(),
        changed(&right),
    ));
    report.push_str(
        "Counts are runs out of each sweep's runs; * a difference chance gives under 5% of the time, ** under 1% (Fisher's exact test; numbers by the sign test on the same runs).\n\n",
    );
    let all = scenarios();
    let checks: HashMap<&str, &Scenario> = all
        .iter()
        .map(|scenario| (scenario.name, scenario))
        .collect();
    let names: BTreeSet<&String> = left.variants.keys().chain(right.variants.keys()).collect();
    let mut totals: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    // Numbers only one sweep keeps, such as one added after the other was
    // made: not compared, and named once at the end.
    let mut one_sided: BTreeSet<String> = BTreeSet::new();
    let (mut runs_left, mut runs_right) = (0, 0);
    // Everything compared, for tools\merge_check.ps1 (sweep_diff.csv).
    let mut findings = Vec::new();
    // How often `x` of `n1` runs against `y` of `n2` happened, as a finding.
    let counted = |section: &str,
                   item: &str,
                   (x, n1): (usize, usize),
                   (y, n2): (usize, usize),
                   worse: Worse| {
        let p = fisher(x, n1, y, n2);
        let (rose, fell) = (y * n1 > x * n2, y * n1 < x * n2);
        Finding::new(
            section,
            item,
            (x as u64, n1 as u64),
            (y as u64, n2 as u64),
            p,
            judged(p, rose, fell, worse),
        )
    };
    for scenario in names {
        let (n1, n2) = (left.runs(scenario), right.runs(scenario));
        if n1 == 0 || n2 == 0 {
            report.push_str(&format!(
                "{scenario}: only in sweep {}\n",
                if n1 == 0 { "B" } else { "A" }
            ));
            continue;
        }
        runs_left += n1;
        runs_right += n2;
        let mut lines = Vec::new();
        let in_band = (
            left.hits(scenario, "band", "in_band"),
            right.hits(scenario, "band", "in_band"),
        );
        findings.push(
            counted(
                "in band",
                scenario,
                (in_band.0, n1),
                (in_band.1, n2),
                Worse::Down,
            )
            .holding(Holds::Must),
        );
        if in_band.0 != in_band.1 {
            let p = fisher(in_band.0, n1, in_band.1, n2);
            lines.push(format!(
                "{} in band {}/{n1} -> {}/{n2}",
                stars(p),
                in_band.0,
                in_band.1
            ));
        }
        let bands: BTreeSet<String> = left
            .names(scenario, "band")
            .union(&right.names(scenario, "band"))
            .filter(|name| *name != "in_band")
            .cloned()
            .collect();
        for band in bands {
            let (x, y) = (
                left.hits(scenario, "band", &band),
                right.hits(scenario, "band", &band),
            );
            if x != y {
                let p = fisher(x, n1, y, n2);
                lines.push(format!(
                    "{} out of band on {band} {x}/{n1} -> {y}/{n2}",
                    stars(p)
                ));
                findings.push(
                    counted(
                        "out of band",
                        &format!("{scenario}: {band}"),
                        (x, n1),
                        (y, n2),
                        Worse::Up,
                    )
                    .holding(Holds::Must),
                );
            }
        }
        if let Some(tuned) = checks.get(scenario.as_str()) {
            for check in tuned.real {
                let name = check.outcome.slug();
                let (x, y) = (
                    left.hits(scenario, "real", &name),
                    right.hits(scenario, "real", &name),
                );
                let fits = |hits: usize, runs: usize| {
                    if check.often.fits(hits, runs) {
                        "fits"
                    } else {
                        "MISSES"
                    }
                };
                let edge = borderline(check.often, y, n2)
                    .map(|note| format!("; B {note}"))
                    .unwrap_or_default();
                // Worse is a check that fit and now misses, beyond chance
                // and not borderline; better the other way round.
                let p = fisher(x, n1, y, n2);
                let (fit_a, fit_b) = (check.often.fits(x, n1), check.often.fits(y, n2));
                let change = if !edge.is_empty() {
                    "borderline"
                } else if p >= SURE || fit_a == fit_b {
                    if x * n2 != y * n1 && p < SURE {
                        "moved"
                    } else {
                        "same"
                    }
                } else if fit_a {
                    "worse"
                } else {
                    "better"
                };
                findings.push(
                    Finding::new(
                        "real-life check",
                        &format!("{scenario}: {}", check.outcome.describe()),
                        (x as u64, n1 as u64),
                        (y as u64, n2 as u64),
                        p,
                        change,
                    )
                    .holding(Holds::Must),
                );
                if x != y || !edge.is_empty() {
                    lines.push(format!(
                        "{} {} {x}/{n1} ({}) -> {y}/{n2} ({}), real life: {}{edge}",
                        stars(p),
                        check.outcome.describe(),
                        fits(x, n1),
                        fits(y, n2),
                        check.often.name(),
                    ));
                }
            }
        }
        for watchdog in Watchdog::ALL {
            if !left.watches(watchdog) || !right.watches(watchdog) {
                continue;
            }
            let (x, y) = (
                left.hits(scenario, "watchdog", watchdog.slug()),
                right.hits(scenario, "watchdog", watchdog.slug()),
            );
            let total = totals.entry(watchdog.slug().to_string()).or_insert((0, 0));
            total.0 += x;
            total.1 += y;
            if x != y {
                let p = fisher(x, n1, y, n2);
                lines.push(format!(
                    "{} watchdog: {} {x}/{n1} -> {y}/{n2}",
                    stars(p),
                    watchdog.describe()
                ));
                findings.push(
                    counted(
                        "watchdog in a scenario",
                        &format!("{scenario}: {}", watchdog.describe()),
                        (x, n1),
                        (y, n2),
                        Worse::Up,
                    )
                    .holding(Holds::Detail),
                );
            }
        }
        let numbers: BTreeSet<String> = left
            .names(scenario, "number")
            .union(&right.names(scenario, "number"))
            .cloned()
            .collect();
        let variants: Vec<&String> = left.variants[scenario.as_str()]
            .intersection(&right.variants[scenario.as_str()])
            .collect();
        for number in numbers {
            if !left.keeps(&number) || !right.keeps(&number) {
                one_sided.insert(number);
                continue;
            }
            let pairs: Vec<(f64, f64)> = variants
                .iter()
                .map(|variant| {
                    (
                        left.value(scenario, &number, variant).unwrap_or(0.0),
                        right.value(scenario, &number, variant).unwrap_or(0.0),
                    )
                })
                .collect();
            let (more, less) = pairs.iter().fold((0, 0), |(more, less), (x, y)| {
                if y > x {
                    (more + 1, less)
                } else if y < x {
                    (more, less + 1)
                } else {
                    (more, less)
                }
            });
            let p = sign_test(more, less);
            let (m1, m2) = (
                median(pairs.iter().map(|pair| pair.0).collect()),
                median(pairs.iter().map(|pair| pair.1).collect()),
            );
            if number_moved(m1, m2, p) {
                lines.push(format!(
                    "{} median {number} {} -> {} (higher in {more} runs, lower in {less})",
                    stars(p),
                    shown(m1),
                    shown(m2)
                ));
                findings.push(
                    Finding::new(
                        "median number",
                        &format!("{scenario}: {number} {} -> {}", shown(m1), shown(m2)),
                        (more as u64, pairs.len() as u64),
                        (less as u64, pairs.len() as u64),
                        p,
                        "moved",
                    )
                    .holding(Holds::Drift),
                );
            }
        }
        if !lines.is_empty() {
            report.push_str(&format!("{scenario}\n"));
            for line in lines {
                report.push_str(&format!("  {line}\n"));
            }
        }
    }
    report.push_str(&format!(
        "\nWatchdogs over all {runs_left} -> {runs_right} runs:\n"
    ));
    // Watchdogs only one side's build had, such as one added after the
    // other was made: not compared, and named after the totals.
    let mut unwatched = Vec::new();
    for watchdog in Watchdog::ALL {
        if !left.watches(watchdog) || !right.watches(watchdog) {
            unwatched.push(format!(
                "{} (only {})",
                watchdog.describe(),
                if left.watches(watchdog) { "A" } else { "B" }
            ));
            continue;
        }
        let (x, y) = totals.get(watchdog.slug()).copied().unwrap_or((0, 0));
        findings.push(
            counted(
                "watchdog",
                watchdog.describe(),
                (x, runs_left),
                (y, runs_right),
                Worse::Up,
            )
            .holding(if watchdog.breaks_the_body() {
                Holds::Must
            } else {
                Holds::Drift
            }),
        );
        if x == 0 && y == 0 {
            continue;
        }
        let p = fisher(x, runs_left, y, runs_right);
        report.push_str(&format!(
            "  {} {}: {x} -> {y}\n",
            stars(p),
            watchdog.describe()
        ));
    }
    if !unwatched.is_empty() {
        report.push_str(&format!(
            "  Watched by only one sweep's build, not compared: {}\n",
            unwatched.join(", ")
        ));
    }
    if !one_sided.is_empty() {
        let named: Vec<String> = one_sided
            .iter()
            .map(|number| {
                format!(
                    "{number} (only {})",
                    if left.keeps(number) { "A" } else { "B" }
                )
            })
            .collect();
        report.push_str(&format!(
            "\nNumbers only one sweep keeps, not compared: {}\n",
            named.join(", ")
        ));
    }
    let path = output.join("sweep_diff.txt");
    fs::write(&path, &report)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    write_findings(&output.join("sweep_diff.csv"), &findings)?;
    print!("{report}");
    println!("wrote {}", path.display());
    Ok(())
}

/// A number for a report: whole when large, two decimals when small.
fn shown(value: f64) -> String {
    if value.abs() >= 10.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fisher_matches_known_tables() {
        // A table every textbook gives: 1 of 10 against 11 of 14, p = 0.0028.
        assert!((fisher(1, 10, 11, 14) - 0.002_759).abs() < 1e-5);
        assert_eq!(fisher(5, 45, 5, 45), 1.0);
        // 24 of 45 against 20 of 45 is well within chance.
        assert!(fisher(24, 45, 20, 45) > 0.4);
        // 5 of 1305 against 33 of 1305 is not.
        assert!(fisher(5, 1305, 33, 1305) < 0.001);
    }

    #[test]
    fn sign_test_matches_the_binomial() {
        // 9 up and 1 down: 2 * 11 / 1024.
        assert!((sign_test(9, 1) - 0.021_484).abs() < 1e-5);
        assert_eq!(sign_test(0, 0), 1.0);
        assert_eq!(sign_test(5, 5), 1.0);
    }

    /// A runs file reads back what it says: each run once, an outcome as
    /// often as it is 1, and an outcome not written as never.
    #[test]
    fn a_runs_file_reads_back_what_it_holds() {
        let text = "# settings: gravity=2\n\
            scenario,along,across,strength,kind,name,value\n\
            tap,0.0000,0.0000,1.00,band,in_band,1\n\
            tap,0.0000,0.0000,1.00,real,breaks_forearm,0\n\
            tap,0.0000,0.0000,1.00,number,fist_knocked,13.0000\n\
            tap,0.0050,0.0000,1.00,band,in_band,0\n\
            tap,0.0050,0.0000,1.00,band,max_fist_shift,1\n\
            tap,0.0050,0.0000,1.00,real,breaks_forearm,1\n\
            tap,0.0050,0.0000,1.00,number,fist_knocked,9.0000\n";
        let runs = Runs::parse(text).unwrap();
        assert_eq!(runs.settings, "gravity=2");
        assert_eq!(runs.runs("tap"), 2);
        assert_eq!(runs.hits("tap", "band", "in_band"), 1);
        assert_eq!(runs.hits("tap", "band", "max_fist_shift"), 1);
        assert_eq!(runs.hits("tap", "real", "breaks_forearm"), 1);
        assert_eq!(runs.hits("tap", "real", "opens_skin"), 0);
        assert_eq!(
            runs.value("tap", "fist_knocked", "0.0050,0.0000,1.00"),
            Some(9.0)
        );
        assert!(Runs::parse("tap,1,2\n").is_err());
    }

    /// A number a sweep never wrote, as one made before the number was
    /// added, is not set against nought; a bone's load, written only when
    /// the bone was loaded, is.
    #[test]
    fn a_number_one_sweep_never_kept_is_not_compared() {
        let older = Runs::parse(
            "scenario,along,across,strength,kind,name,value\n\
            tap,0.0000,0.0000,1.00,number,bruises,3.0000\n",
        )
        .unwrap();
        assert!(older.keeps("bruises"));
        assert!(!older.keeps("contacts"));
        assert!(older.keeps("load_skull"));
    }

    /// A watchdog a sweep's build never had, which its runs never wrote, is
    /// not set against nought either.
    #[test]
    fn a_watchdog_one_sweep_never_watched_is_not_compared() {
        let older = Runs::parse(
            "scenario,along,across,strength,kind,name,value\n\
            tap,0.0000,0.0000,1.00,watchdog,whole_bone_out,0\n",
        )
        .unwrap();
        assert!(older.watches(Watchdog::WholeBoneOut));
        assert!(!older.watches(Watchdog::BoneShows));
    }

    #[test]
    fn borderline_checks_are_the_ones_a_few_runs_could_flip() {
        // 20 of 45 is short of half, but not surely.
        assert!(borderline(Often::Usually, 20, 45).is_some());
        assert!(borderline(Often::Usually, 24, 45).is_some());
        assert!(borderline(Often::Usually, 40, 45).is_none());
        assert!(borderline(Often::Rarely, 3, 45).is_none());
        assert!(borderline(Often::Sometimes, 44, 45).is_some());
        assert!(borderline(Often::Sometimes, 20, 45).is_none());
        assert!(borderline(Often::Always, 44, 45).is_none());
    }
}
