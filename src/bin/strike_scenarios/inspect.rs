//! The inspection reports, printed: the story a run's journal tells
//! (`--journal`), what moved a point, bone, fiber, joint, or triangle
//! (`--why`), and what the watchdogs flagged, with what moved each first
//! thing they flagged (`realistic_physics::inspect` works them out).

use super::*;
use rp::inspect::{story, totals, watchdog_lines, Trip, Why};

/// Prints the story of the run `world`'s journal kept, then its totals.
pub(crate) fn print_journal(world: &rp::World) {
    let Some(journal) = world.journal() else {
        return;
    };
    println!("What happened, frame by frame (left and right as seen on screen):");
    for line in story(world, journal) {
        println!("  {line}");
    }
    println!("In all:");
    for line in totals(world, journal) {
        println!("  {line}");
    }
}

/// Plays `scenario` following what `why` names and prints what moved it.
pub(crate) fn explain(scenario: &Scenario, why: Why) {
    for line in rp::inspect::explain(scenario, why) {
        println!("{line}");
    }
}

/// Prints what the watchdogs flagged in a lone run and, for the first thing
/// each flagged, what moved it over the twenty frames before.
pub(crate) fn report_watchdogs(scenario: &Scenario, world: &rp::World, trips: &[Trip]) {
    if trips.is_empty() {
        println!("watchdogs: nothing flagged");
        return;
    }
    println!("watchdogs:");
    for line in watchdog_lines(world, trips) {
        println!("  {line}");
    }
    let mut seen = Vec::new();
    for trip in trips {
        if seen.contains(&trip.watchdog) || trip.watchdog.known_gap() {
            continue;
        }
        seen.push(trip.watchdog);
        let Some(target) = trip.why_target(world) else {
            continue;
        };
        println!("  why, for the first {}:", trip.watchdog.describe());
        explain(
            scenario,
            Why {
                target,
                from: (trip.first_frame - 20).max(0),
                to: trip.first_frame,
            },
        );
    }
}
