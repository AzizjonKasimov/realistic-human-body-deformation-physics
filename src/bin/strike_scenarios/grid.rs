//! A grid (`--grid NAME=V1,V2,...`, repeatable): the scenarios, or the
//! custom swing or gesture, played over every combination of the values
//! given, each NAME a setting of `Materials` or a blank `{NAME}` in the
//! custom play, and one table with a row for each combination: its runs in
//! band, the real-life checks, the main numbers, and the watchdogs, each set
//! against the first row by how unlikely chance is to make the difference,
//! as `--diff` judges two sweeps. "Which values work?" takes one command
//! instead of a script looping over `--set`.

use super::*;
use blanks::{is_name, Written};
use diff::{fisher, median, number_moved, run_numbers, sign_test, stars, RUNS_FILE};
use rp::inspect::Watchdog;
use rp::scenarios::Often;
use std::collections::BTreeSet;

/// One axis of a grid: a setting or a blank, and the values it takes in the
/// order given. Its first value is in the first row, which every other row
/// is set against.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Axis {
    pub(crate) name: String,
    pub(crate) values: Vec<String>,
}

impl Axis {
    /// Reads `NAME=V1,V2,...`.
    pub(crate) fn parse(spec: &str) -> Result<Axis, String> {
        let wrong = || format!("--grid takes NAME=V1,V2,..., got `{spec}`");
        let (name, values) = spec.split_once('=').ok_or_else(wrong)?;
        let name = name.trim();
        let values: Vec<String> = values
            .split(',')
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
        if !is_name(name) || values.is_empty() {
            return Err(wrong());
        }
        Ok(Axis {
            name: name.to_string(),
            values,
        })
    }
}

/// Which runs each row of a grid plays.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Plays {
    /// One run: the play as tuned, or the run of the sweep that
    /// `--variant` names.
    Once((f64, f64, f64)),
    /// The sweep's 45 runs, or the quick sweep's 9 with `true`.
    Sweep(bool),
}

impl Plays {
    fn variants(self) -> Vec<(f64, f64, f64)> {
        match self {
            Plays::Once(variant) => vec![variant],
            Plays::Sweep(quick) => sweep::sweep_variants(quick),
        }
    }

    fn describe(self) -> String {
        match self {
            Plays::Once((along, across, strength)) => {
                if along == 0.0 && across == 0.0 && strength == 1.0 {
                    "1 run (the play as tuned)".to_string()
                } else {
                    format!("1 run (--variant {along},{across},{strength})")
                }
            }
            Plays::Sweep(true) => "9 runs (the quick sweep: 3 aims, 3 bone strengths)".to_string(),
            Plays::Sweep(false) => "45 runs (the sweep: 15 aims, 3 bone strengths)".to_string(),
        }
    }
}

/// One row of a grid: a value of each axis, and what comes of them.
struct Row {
    values: Vec<String>,
    /// The materials its worlds are made of: the defaults, the settings
    /// given with `--set`, and the row's own.
    base: rp::Materials,
    /// Its settings and filled blanks as `NAME=VALUE` items, for its runs
    /// file.
    label: String,
    /// What it plays: the custom play with its blanks filled, or the
    /// scenarios.
    scenarios: Vec<Scenario>,
}

/// The rows of a grid, the first axis changing slowest, every setting and
/// play checked before anything is played.
fn rows(
    axes: &[Axis],
    written: Option<&Written>,
    selected: &[Scenario],
) -> Result<Vec<Row>, String> {
    let blanks = written.map_or(Ok(BTreeSet::new()), Written::blanks)?;
    for (index, axis) in axes.iter().enumerate() {
        if axes[..index].iter().any(|other| other.name == axis.name) {
            return Err(format!("--grid {} is given twice", axis.name));
        }
        if !is_setting(&axis.name) && !blanks.contains(&axis.name) {
            let unknown = rp::Materials::default()
                .set(&axis.name, &axis.values[0])
                .err()
                .unwrap_or_default();
            return Err(match written {
                Some(_) => format!(
                    "--grid {}: {unknown}, and the play has no blank {{{}}}",
                    axis.name, axis.name
                ),
                None => format!("--grid {}: {unknown}", axis.name),
            });
        }
    }
    let count: usize = axes.iter().map(|axis| axis.values.len()).product();
    let mut rows = Vec::with_capacity(count);
    for index in 0..count {
        let mut values = vec![String::new(); axes.len()];
        let mut rest = index;
        for (slot, axis) in axes.iter().enumerate().rev() {
            values[slot] = axis.values[rest % axis.values.len()].clone();
            rest /= axis.values.len();
        }
        let pairs: Vec<(&str, &str)> = axes
            .iter()
            .zip(&values)
            .map(|(axis, value)| (axis.name.as_str(), value.as_str()))
            .collect();
        let row_name = || {
            pairs
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let mut base = rp::scenarios::set_materials();
        for &(name, value) in pairs.iter().filter(|(name, _)| is_setting(name)) {
            base.set(name, value)
                .map_err(|error| format!("--grid {name}: {error}"))?;
        }
        let scenarios = match written {
            Some(written) => {
                vec![Scenario::custom(written.play(&pairs).map_err(|error| {
                    format!("the play at {}: {error}", row_name())
                })?)]
            }
            None => selected.to_vec(),
        };
        let mut label = rp::scenarios::settings_used();
        label.push(row_name());
        rows.push(Row {
            values,
            base,
            label: label.join(" "),
            scenarios,
        });
    }
    Ok(rows)
}

fn is_setting(name: &str) -> bool {
    rp::Materials::NAMES.contains(&name)
}

/// Plays every row of the grid that `axes` make, of the custom play
/// `written` or else the scenarios `selected`, each row as `plays` says, on
/// all cores; then writes and prints the table, writes each row's runs
/// for `--diff`, and the table as CSV.
pub(crate) fn grid(
    axes: &[Axis],
    written: Option<&Written>,
    selected: &[Scenario],
    plays: Plays,
    output_dir: &Path,
) -> Result<(), String> {
    let rows = rows(axes, written, selected)?;
    let variants = plays.variants();
    let jobs: Vec<(usize, usize, (f64, f64, f64))> = rows
        .iter()
        .enumerate()
        .flat_map(|(row, played)| {
            let variants = &variants;
            (0..played.scenarios.len())
                .flat_map(move |scenario| variants.iter().map(move |&run| (row, scenario, run)))
        })
        .collect();
    let total = jobs.len();
    println!(
        "{} rows of {} scenario{}, each played {}: {total} runs",
        rows.len(),
        rows[0].scenarios.len(),
        if rows[0].scenarios.len() == 1 {
            ""
        } else {
            "s"
        },
        plays.describe()
    );
    let started = Instant::now();
    let done = AtomicUsize::new(0);
    let played = sweep::on_all_cores(&jobs, |&(row, scenario, variant)| {
        let run = SweepRun::play(
            scenario,
            &rows[row].scenarios[scenario],
            variant,
            rows[row].base,
        );
        let count = done.fetch_add(1, Ordering::Relaxed) + 1;
        if count * 10 / total > (count - 1) * 10 / total {
            println!(
                "  played {count} of {total} runs in {:.0} s",
                started.elapsed().as_secs_f64()
            );
        }
        run
    });
    let seconds = started.elapsed().as_secs_f64();
    let mut runs: Vec<Vec<SweepRun>> = rows.iter().map(|_| Vec::new()).collect();
    for (&(row, _, _), run) in jobs.iter().zip(played) {
        runs[row].push(run);
    }

    let grid_dir = output_dir.join("grid");
    let width = (rows.len() - 1).to_string().len().max(2);
    let folder = |row: usize| format!("row{row:0width$}");
    for (index, row) in rows.iter().enumerate() {
        let dir = grid_dir.join(folder(index));
        fs::create_dir_all(&dir)
            .map_err(|error| format!("cannot write {}: {error}", dir.display()))?;
        sweep::write_runs(&row.scenarios, &runs[index], &dir, &row.label);
    }
    clear_old_rows(&grid_dir, rows.len(), &folder);

    let mut report = header(axes, written, selected, &rows, plays, total, seconds);
    let scenario_count = rows[0].scenarios.len();
    if scenario_count > 1 {
        report.push_str("\nAll scenarios together:\n");
        report.push_str(&table(&summary_columns(axes, &rows, &runs, plays)));
    }
    for scenario in 0..scenario_count {
        if scenario_count > 1 {
            report.push_str(&format!("\n{}:\n", rows[0].scenarios[scenario].name));
        } else {
            report.push('\n');
        }
        report.push_str(&table(&scenario_columns(
            axes, &rows, &runs, scenario, plays,
        )));
    }
    report.push_str(&format!(
        "\nEach row's runs are in {}\\{}..{} (--diff sets any two side by side in full).\n",
        grid_dir.display(),
        folder(0),
        folder(rows.len() - 1)
    ));
    let report_path = output_dir.join("strike_grid.txt");
    fs::write(&report_path, &report)
        .map_err(|error| format!("cannot write {}: {error}", report_path.display()))?;
    let csv_path = output_dir.join("strike_grid.csv");
    fs::write(&csv_path, csv(axes, &rows, &runs))
        .map_err(|error| format!("cannot write {}: {error}", csv_path.display()))?;
    print!("{report}");
    println!("wrote {}", report_path.display());
    println!("wrote {}", csv_path.display());
    Ok(())
}

/// Takes away the runs files of rows an earlier, larger grid left in
/// `grid_dir`, and their folders once empty. Only `row` folders' runs files
/// are touched.
fn clear_old_rows(grid_dir: &Path, rows: usize, folder: &dyn Fn(usize) -> String) {
    let current: BTreeSet<String> = (0..rows).map(folder).collect();
    let Ok(entries) = fs::read_dir(grid_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let numbered = name
            .strip_prefix("row")
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()));
        if numbered && !current.contains(&name) {
            let _ = fs::remove_file(entry.path().join(RUNS_FILE));
            let _ = fs::remove_dir(entry.path());
        }
    }
}

/// What the grid played and how its rows are judged.
fn header(
    axes: &[Axis],
    written: Option<&Written>,
    selected: &[Scenario],
    rows: &[Row],
    plays: Plays,
    total: usize,
    seconds: f64,
) -> String {
    let mut text = match written {
        Some(written) => format!("Grid of the play {}\n", written.spec()),
        None => {
            let names: Vec<&str> = selected.iter().map(|scenario| scenario.name).collect();
            format!("Grid of {}\n", names.join(", "))
        }
    };
    let defaults = rp::Materials::default();
    for axis in axes {
        let what = match defaults.get(&axis.name) {
            Some(default) => format!("a setting, {default} by default"),
            None => "a blank in the play".to_string(),
        };
        text.push_str(&format!(
            "  {}: {} ({what})\n",
            axis.name,
            axis.values.join(", ")
        ));
    }
    let settings = rp::scenarios::settings_used();
    if !settings.is_empty() {
        text.push_str(&format!("Settings in every row: {}\n", settings.join(", ")));
    }
    text.push_str(&format!(
        "{} rows, each played {}: {total} runs in {seconds:.0} s.\n",
        rows.len(),
        plays.describe()
    ));
    if matches!(plays, Plays::Sweep(_)) {
        text.push_str(
            "Every row is set against row 0: * a difference chance makes less than 5% of the time, ** less than 1% (Fisher's exact test on the runs; a median by the sign test on the same runs, marked when it moves a tenth). ! misses real life. Numbers are medians of a row's runs.\n",
        );
    } else {
        text.push_str(
            "One run a row is not judged against chance: --quick plays 9 a row and --sweep 45. ! misses real life.\n",
        );
    }
    text
}

/// A column of a grid's table.
struct Column {
    heading: String,
    cells: Vec<Cell>,
    /// Shown even when every row has the same.
    always: bool,
}

/// What a column shows in one row.
#[derive(Clone)]
struct Cell {
    text: String,
    /// How sure its difference from row 0 is (`stars`).
    stars: &'static str,
    /// It misses real life.
    missed: bool,
}

impl Cell {
    fn plain(text: String) -> Cell {
        Cell {
            text,
            stars: "  ",
            missed: false,
        }
    }
}

/// A column of how many runs of each row something happened in, set
/// against row 0 by Fisher's exact test; `missed` says in which rows that
/// misses real life.
fn count_column(heading: String, counts: &[(usize, usize)], missed: &[bool]) -> Column {
    let (first, runs) = counts[0];
    Column {
        heading,
        cells: counts
            .iter()
            .zip(missed)
            .map(|(&(hits, n), &missed)| Cell {
                text: format!("{hits}/{n}{}", if missed { "!" } else { "" }),
                stars: stars(fisher(first, runs, hits, n)),
                missed,
            })
            .collect(),
        always: false,
    }
}

/// A column of a number's median in each row, set against row 0 by the
/// sign test on the same runs (each row's values in the same runs' order).
fn number_column(heading: String, values: &[Vec<f64>]) -> Column {
    let first = &values[0];
    let before = median(first.clone());
    Column {
        heading,
        cells: values
            .iter()
            .map(|row| {
                let (more, less) = first.iter().zip(row).fold((0, 0), |(more, less), (a, b)| {
                    if b > a {
                        (more + 1, less)
                    } else if b < a {
                        (more, less + 1)
                    } else {
                        (more, less)
                    }
                });
                let p = sign_test(more, less);
                let after = median(row.clone());
                Cell {
                    text: shown_number(after),
                    stars: if number_moved(before, after, p) {
                        stars(p)
                    } else {
                        "  "
                    },
                    missed: false,
                }
            })
            .collect(),
        always: false,
    }
}

/// A number for a table: whole when it is whole or large, else to two
/// decimals.
fn shown_number(value: f64) -> String {
    if value == value.trunc() || value.abs() >= 10.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

/// The first columns of every table: the row's number and its values.
fn row_columns(axes: &[Axis], rows: &[Row]) -> Vec<Column> {
    let mut columns = vec![Column {
        heading: "row".to_string(),
        cells: (0..rows.len())
            .map(|row| Cell::plain(row.to_string()))
            .collect(),
        always: true,
    }];
    for (slot, axis) in axes.iter().enumerate() {
        columns.push(Column {
            heading: axis.name.clone(),
            cells: rows
                .iter()
                .map(|row| Cell::plain(row.values[slot].clone()))
                .collect(),
            always: true,
        });
    }
    columns
}

/// The table for scenario number `scenario` of each row: runs in band, runs
/// that touched the body, the real-life checks, the main numbers, and the
/// watchdogs that tripped.
fn scenario_columns(
    axes: &[Axis],
    rows: &[Row],
    runs: &[Vec<SweepRun>],
    scenario: usize,
    plays: Plays,
) -> Vec<Column> {
    let mine: Vec<Vec<&SweepRun>> = runs
        .iter()
        .map(|row| row.iter().filter(|run| run.scenario == scenario).collect())
        .collect();
    let counts = |happened: &dyn Fn(&SweepRun) -> bool| -> Vec<(usize, usize)> {
        mine.iter()
            .map(|row| (row.iter().filter(|run| happened(run)).count(), row.len()))
            .collect()
    };
    let nothing_missed = vec![false; rows.len()];
    let mut columns = row_columns(axes, rows);
    let mut in_band = count_column(
        "in band".to_string(),
        &counts(&|run| run.violations.is_empty()),
        &nothing_missed,
    );
    in_band.always = true;
    columns.push(in_band);
    columns.push(count_column(
        "touched".to_string(),
        &counts(&|run| run.result.tissue_contacts + run.result.bone_contacts > 0),
        &nothing_missed,
    ));
    for check in rows[0].scenarios[scenario].real {
        let hits = counts(&|run| check.outcome.happened(&run.result));
        let missed: Vec<bool> = hits
            .iter()
            .map(|&(hits, n)| misses(plays, check.often, hits, n))
            .collect();
        let gap = if check.gap.is_empty() { "" } else { ", a gap" };
        columns.push(count_column(
            format!("{} ({}{gap})", check.outcome.slug(), check.often.name()),
            &hits,
            &missed,
        ));
    }
    // The numbers `--diff` compares, but bones' loads, which only the CSV
    // keeps: a run without one had it at nought.
    let mut names: Vec<String> = Vec::new();
    for run in mine.iter().flatten() {
        for (name, _) in run_numbers(&run.result) {
            if !name.starts_with("load_") && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    for name in names {
        let values: Vec<Vec<f64>> = mine
            .iter()
            .map(|row| row.iter().map(|run| number_of(run, &name)).collect())
            .collect();
        columns.push(number_column(name, &values));
    }
    for watchdog in Watchdog::ALL {
        let tripped = counts(&|run| run.flagged.iter().any(|(kind, _, _)| *kind == watchdog));
        if tripped.iter().any(|&(hits, _)| hits > 0) {
            columns.push(count_column(
                watchdog.slug().to_string(),
                &tripped,
                &nothing_missed,
            ));
        }
    }
    columns
}

/// Whether something that happened in `hits` of a row's `n` runs misses
/// how often it happens in real life. One run judges only never and
/// always, as a lone run does.
fn misses(plays: Plays, often: Often, hits: usize, n: usize) -> bool {
    match (plays, often) {
        (Plays::Once(_), Often::Never) => hits > 0,
        (Plays::Once(_), Often::Always) => hits < n,
        (Plays::Once(_), _) => false,
        (Plays::Sweep(_), often) => !often.fits(hits, n),
    }
}

/// A run's number called `name` (`run_numbers`), nought when it has none.
fn number_of(run: &SweepRun, name: &str) -> f64 {
    run_numbers(&run.result)
        .into_iter()
        .find(|(given, _)| given == name)
        .map_or(0.0, |(_, value)| value)
}

/// The table over every scenario of each row: runs in band, how many
/// real-life checks miss (known gaps left out), and each watchdog's runs.
fn summary_columns(
    axes: &[Axis],
    rows: &[Row],
    runs: &[Vec<SweepRun>],
    plays: Plays,
) -> Vec<Column> {
    let counts = |happened: &dyn Fn(&SweepRun) -> bool| -> Vec<(usize, usize)> {
        runs.iter()
            .map(|row| (row.iter().filter(|run| happened(run)).count(), row.len()))
            .collect()
    };
    let nothing_missed = vec![false; rows.len()];
    let mut columns = row_columns(axes, rows);
    let mut in_band = count_column(
        "in band".to_string(),
        &counts(&|run| run.violations.is_empty()),
        &nothing_missed,
    );
    in_band.always = true;
    columns.push(in_band);
    let misses: Vec<Cell> = rows
        .iter()
        .zip(runs)
        .map(|(row, runs)| {
            let missed: usize = row
                .scenarios
                .iter()
                .enumerate()
                .map(|(index, scenario)| {
                    let results: Vec<&ScenarioResult> = runs
                        .iter()
                        .filter(|run| run.scenario == index)
                        .map(|run| &run.result)
                        .collect();
                    scenario
                        .real
                        .iter()
                        .filter(|check| check.gap.is_empty())
                        .filter(|check| {
                            let hits = results
                                .iter()
                                .filter(|result| check.outcome.happened(result))
                                .count();
                            misses(plays, check.often, hits, results.len())
                        })
                        .count()
                })
                .sum();
            Cell::plain(missed.to_string())
        })
        .collect();
    columns.push(Column {
        heading: "real-life misses".to_string(),
        cells: misses,
        always: false,
    });
    for watchdog in Watchdog::ALL {
        let tripped = counts(&|run| run.flagged.iter().any(|(kind, _, _)| *kind == watchdog));
        if tripped.iter().any(|&(hits, _)| hits > 0) {
            columns.push(count_column(
                watchdog.slug().to_string(),
                &tripped,
                &nothing_missed,
            ));
        }
    }
    columns
}

/// The columns laid out as a table: those that differ between rows, or
/// miss real life in one, each value with how sure its difference from row
/// 0 is beside it; then a line for the columns every row has the same.
fn table(columns: &[Column]) -> String {
    let shown: Vec<&Column> = columns
        .iter()
        .filter(|column| {
            column.always
                || column.cells.iter().any(|cell| cell.missed)
                || column
                    .cells
                    .iter()
                    .any(|cell| cell.text != column.cells[0].text)
        })
        .collect();
    let widths: Vec<usize> = shown
        .iter()
        .map(|column| {
            column
                .cells
                .iter()
                .map(|cell| cell.text.len())
                .chain([column.heading.len()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut text = String::new();
    let heading: Vec<String> = shown
        .iter()
        .zip(&widths)
        .map(|(column, &width)| format!("{:>width$}  ", column.heading))
        .collect();
    text.push_str(heading.join(" ").trim_end());
    text.push('\n');
    let rows = columns.first().map_or(0, |column| column.cells.len());
    for row in 0..rows {
        let cells: Vec<String> = shown
            .iter()
            .zip(&widths)
            .map(|(column, &width)| {
                let cell = &column.cells[row];
                format!("{:>width$}{}", cell.text, cell.stars)
            })
            .collect();
        text.push_str(cells.join(" ").trim_end());
        text.push('\n');
    }
    let same: Vec<String> = columns
        .iter()
        .filter(|column| !shown.iter().any(|shown| std::ptr::eq(*shown, *column)))
        .map(|column| format!("{} {}", column.heading, column.cells[0].text))
        .collect();
    if !same.is_empty() {
        text.push_str(&format!("The same in every row: {}.\n", same.join(", ")));
    }
    text
}

/// The table as CSV, one line per row, scenario, and measure: how many
/// runs, how many in band and touching, each real-life check's and
/// watchdog's runs, and each number's median, least, and most (bones'
/// loads too).
fn csv(axes: &[Axis], rows: &[Row], runs: &[Vec<SweepRun>]) -> String {
    let names: Vec<&str> = axes.iter().map(|axis| axis.name.as_str()).collect();
    let mut text = format!("row,{},scenario,kind,name,value\n", names.join(","));
    for (index, (row, runs)) in rows.iter().zip(runs).enumerate() {
        for (number, scenario) in row.scenarios.iter().enumerate() {
            let mine: Vec<&SweepRun> = runs.iter().filter(|run| run.scenario == number).collect();
            let key = format!("{index},{},{}", row.values.join(","), scenario.name);
            let count = |happened: &dyn Fn(&SweepRun) -> bool| {
                mine.iter().filter(|run| happened(run)).count()
            };
            let mut lines = vec![
                format!("runs,runs,{}", mine.len()),
                format!("count,in_band,{}", count(&|run| run.violations.is_empty())),
                format!(
                    "count,touched,{}",
                    count(&|run| run.result.tissue_contacts + run.result.bone_contacts > 0)
                ),
            ];
            for check in scenario.real {
                lines.push(format!(
                    "real,{},{}",
                    check.outcome.slug(),
                    count(&|run| check.outcome.happened(&run.result))
                ));
            }
            for watchdog in Watchdog::ALL {
                lines.push(format!(
                    "watchdog,{},{}",
                    watchdog.slug(),
                    count(&|run| run.flagged.iter().any(|(kind, _, _)| *kind == watchdog))
                ));
            }
            let mut numbers: Vec<String> = Vec::new();
            for run in &mine {
                for (name, _) in run_numbers(&run.result) {
                    if !numbers.contains(&name) {
                        numbers.push(name);
                    }
                }
            }
            for name in numbers {
                let mut values: Vec<f64> = mine.iter().map(|run| number_of(run, &name)).collect();
                values.sort_by(f64::total_cmp);
                lines.push(format!("median,{name},{:.4}", median(values.clone())));
                lines.push(format!("min,{name},{:.4}", values[0]));
                lines.push(format!("max,{name},{:.4}", values[values.len() - 1]));
            }
            for line in lines {
                text.push_str(&format!("{key},{line}\n"));
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_axis_reads_its_name_and_values() {
        let axis = Axis::parse("lead=0,4, 8").unwrap();
        assert_eq!(axis.name, "lead");
        assert_eq!(axis.values, ["0", "4", "8"]);
        for wrong in ["lead", "=1", "lead=", "9lives=1", "lean back=1"] {
            assert!(Axis::parse(wrong).is_err(), "{wrong}");
        }
    }

    /// A grid's rows cover every combination, the first axis changing
    /// slowest, each row on its own settings.
    #[test]
    fn rows_cover_every_combination() {
        let axes = [
            Axis::parse("lean_back=0.2,0.3").unwrap(),
            Axis::parse("lead=0,4,8").unwrap(),
        ];
        let written =
            Written::Gesture("fist:-1.3,0.95:wait={lead}:punch=right:fighters".to_string());
        let built = rows(&axes, Some(&written), &[]).unwrap();
        let values: Vec<String> = built.iter().map(|row| row.values.join(" ")).collect();
        assert_eq!(
            values,
            ["0.2 0", "0.2 4", "0.2 8", "0.3 0", "0.3 4", "0.3 8"]
        );
        assert_eq!(built[4].base.lean_back, 0.3);
        assert_eq!(built[4].label, "lean_back=0.3 lead=4");
        let Play::Gesture(gesture) = built[4].scenarios[0].play else {
            panic!("a gesture was written");
        };
        let spec = gesture.to_string();
        assert!(spec.contains(":wait=4:punch=right:"), "{spec}");
        assert!(gesture.fight.is_some());
        // A name that is neither a setting nor a blank, a value a setting
        // cannot take, and a name given twice are refused before playing.
        let refused = [
            vec![Axis::parse("lean_bak=0.2").unwrap()],
            vec![Axis::parse("lean_back=deep").unwrap()],
            vec![
                Axis::parse("lead=1").unwrap(),
                Axis::parse("lead=2").unwrap(),
            ],
        ];
        for axes in refused {
            assert!(rows(&axes, Some(&written), &[]).is_err(), "{axes:?}");
        }
        assert!(rows(&[Axis::parse("lead=1").unwrap()], None, &scenarios()).is_err());
    }

    /// The table shows the columns that differ between rows, or miss real
    /// life, and names the rest once.
    #[test]
    fn the_table_shows_what_differs() {
        let column = |heading: &str, texts: &[&str], missed: bool| Column {
            heading: heading.to_string(),
            cells: texts
                .iter()
                .map(|text| Cell {
                    text: text.to_string(),
                    stars: "  ",
                    missed,
                })
                .collect(),
            always: false,
        };
        let columns = [
            column("bruises", &["0", "15"], false),
            column("skin_torn", &["0", "0"], false),
            column("opens_skin (never)", &["1/1!", "1/1!"], true),
        ];
        let text = table(&columns);
        let lines: Vec<&str> = text.lines().collect();
        // Each value sits under its heading's right end, with room for how
        // sure it is beside it.
        assert_eq!(lines[0], "bruises   opens_skin (never)");
        assert_eq!(lines[2], format!("     15{}1/1!", " ".repeat(17)));
        assert_eq!(lines[3], "The same in every row: skin_torn 0.");
    }
}
