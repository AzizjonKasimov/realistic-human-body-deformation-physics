//! The story a run's journal tells: on each frame something happened, what
//! the tool was doing and what each injury rule did, alike events grouped,
//! and the minor ones (bruising, tiring, setting) only counted.

use super::names::{actor_name, measure_text, place_name, subject_name};
use crate::simulation::{Event, EventKind, Journal, Subject, World};

/// Events of one kind at one place by one actor on one frame.
struct Group<'a> {
    kind: EventKind,
    place: String,
    actor: String,
    phase: &'static str,
    events: Vec<&'a Event>,
}

/// Whether events about `subject` are told one by one: bones, joints, limbs,
/// organs, and arteries, of which there are few.
fn told_alone(subject: Subject) -> bool {
    matches!(
        subject,
        Subject::Bone(_)
            | Subject::Joint(_)
            | Subject::Limb(_)
            | Subject::Organ(_)
            | Subject::Vessel(_)
            | Subject::Cavity(_)
    )
}

/// The event of `events` that went furthest past its limit.
fn worst<'a>(events: &[&'a Event]) -> &'a Event {
    events
        .iter()
        .copied()
        .max_by(|p, q| {
            let excess = |event: &Event| {
                if event.limit > 0.0 {
                    event.value / event.limit
                } else {
                    event.value
                }
            };
            excess(p).total_cmp(&excess(q))
        })
        .expect("a group has events")
}

/// One line for a group: what, how many, where, by what, and the worst
/// measure, with the phase of the step that decided it.
fn group_line(world: &World, group: &Group) -> String {
    let worst = worst(&group.events);
    let what = if told_alone(worst.subject) {
        format!(
            "{}: {}",
            group.kind.describe(),
            subject_name(world, worst.subject)
        )
    } else if group.events.len() > 1 {
        format!(
            "{} x{} at {}",
            group.kind.describe(),
            group.events.len(),
            group.place
        )
    } else {
        format!("{} at {}", group.kind.describe(), group.place)
    };
    let measure = measure_text(worst.measure, worst.value, worst.limit);
    let measure = if measure.is_empty() {
        String::new()
    } else if group.events.len() > 1 {
        format!(", worst {measure}")
    } else {
        format!(", {measure}")
    };
    // What the tissue bore of itself goes without saying: the measure says it.
    let by = if group.actor.is_empty() {
        String::new()
    } else {
        format!(", by {}", group.actor)
    };
    format!(
        "{what}{by}{measure} [{}]{}",
        group.phase,
        why_name(worst.subject)
    )
}

/// What `strike_scenarios --why` takes to follow `subject`, as ` (spring:12)`,
/// or nothing for what it cannot follow.
fn why_name(subject: Subject) -> String {
    let named = match subject {
        Subject::Point(index) => format!("point:{index}"),
        Subject::Spring(index) => format!("spring:{index}"),
        Subject::Bone(index) => format!("bone:{index}"),
        Subject::Joint(index) => format!("joint:{index}"),
        Subject::Triangle(index) => format!("triangle:{index}"),
        _ => return String::new(),
    };
    format!(" ({named})")
}

/// Groups `events` by kind, place (or subject, for what is told alone),
/// and actor, in the order they first happened.
fn groups<'a>(world: &World, journal: &Journal, events: &[&'a Event]) -> Vec<Group<'a>> {
    let mut groups: Vec<Group> = Vec::new();
    for &event in events {
        let tool = journal
            .steps
            .get(event.step as usize)
            .map(|note| note.tool)
            .unwrap_or_default();
        let place = if told_alone(event.subject) {
            subject_name(world, event.subject)
        } else {
            place_name(world, event.at)
        };
        let actor = actor_name(world, event.by, tool);
        match groups
            .iter_mut()
            .find(|group| group.kind == event.kind && group.place == place && group.actor == actor)
        {
            Some(group) => group.events.push(event),
            None => groups.push(Group {
                kind: event.kind,
                place,
                actor,
                phase: event.phase,
                events: vec![event],
            }),
        }
    }
    groups
}

/// The story of `journal`, kept on `world` as the run left it: a line for
/// each frame something happened on, then a line for each injury, alike
/// ones grouped.
pub fn story(world: &World, journal: &Journal) -> Vec<String> {
    let mm = world.mm_per_px();
    let mut lines = Vec::new();
    let mut from = 0;
    while from < journal.events.len() {
        let step = journal.events[from].step;
        let to = journal.events[from..]
            .iter()
            .position(|event| event.step != step)
            .map_or(journal.events.len(), |offset| from + offset);
        let events: Vec<&Event> = journal.events[from..to].iter().collect();
        from = to;
        let note = journal
            .steps
            .get(step as usize)
            .copied()
            .unwrap_or_default();
        let touching = if note.tissue_contacts + note.bone_contacts > 0 {
            format!(
                "; {} tissue contacts, {} bone contacts",
                note.tissue_contacts, note.bone_contacts
            )
        } else {
            String::new()
        };
        lines.push(format!(
            "frame {step}: {} {}, the hand at {:.1} m/s{touching}",
            super::names::tool_name(note.tool),
            if note.down { "pressed" } else { "carried" },
            note.hand_speed * mm * 0.001,
        ));
        let (minor, major): (Vec<&Event>, Vec<&Event>) =
            events.into_iter().partition(|event| event.kind.is_minor());
        for group in groups(world, journal, &major) {
            lines.push(format!("  {}", group_line(world, &group)));
        }
        if !minor.is_empty() {
            let counts: Vec<String> = groups(world, journal, &minor)
                .iter()
                .fold(Vec::<(EventKind, usize)>::new(), |mut counts, group| {
                    match counts.iter_mut().find(|(kind, _)| *kind == group.kind) {
                        Some((_, count)) => *count += group.events.len(),
                        None => counts.push((group.kind, group.events.len())),
                    }
                    counts
                })
                .into_iter()
                .map(|(kind, count)| format!("{} {count}", kind.describe()))
                .collect();
            lines.push(format!("  and {}", counts.join(", ")));
        }
    }
    lines
}

/// Each kind of event over the whole run: how many, where most, and by what.
pub fn totals(world: &World, journal: &Journal) -> Vec<String> {
    let events: Vec<&Event> = journal.events.iter().collect();
    let all = groups(world, journal, &events);
    let mut kinds: Vec<EventKind> = all.iter().map(|group| group.kind).collect();
    kinds.sort();
    kinds.dedup();
    kinds
        .into_iter()
        .map(|kind| {
            let mine: Vec<&Group> = all.iter().filter(|group| group.kind == kind).collect();
            let count: usize = mine.iter().map(|group| group.events.len()).sum();
            format!(
                "{} {count}: at {}; by {}",
                kind.describe(),
                tally(&mine, |group| &group.place),
                tally(&mine, |group| &group.actor)
            )
        })
        .collect()
}

/// The three commonest of what `label` names among `groups`, with how many
/// events each holds.
fn tally<'a>(groups: &[&'a Group], label: impl Fn(&'a Group) -> &'a String) -> String {
    let mut tallies: Vec<(String, usize)> = Vec::new();
    for group in groups {
        let name = label(group);
        let name = if name.is_empty() {
            "its own load or stretch"
        } else {
            name
        };
        match tallies.iter_mut().find(|(seen, _)| seen == name) {
            Some((_, n)) => *n += group.events.len(),
            None => tallies.push((name.to_string(), group.events.len())),
        }
    }
    tallies.sort_by(|p, q| q.1.cmp(&p.1));
    tallies
        .iter()
        .take(3)
        .map(|(name, n)| format!("{name} {n}"))
        .collect::<Vec<_>>()
        .join(", ")
}
