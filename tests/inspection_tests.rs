//! The inspection tools only watch: keeping a journal or tracing points and
//! bones must not change what the simulation does, and the journal must tell
//! what a blow did.

use realistic_physics as rp;
use rp::scenarios::{run_on, Play, Scenario, Strike, SCENARIO_HEIGHT, SCENARIO_WIDTH};

/// A hard bat swing across the raised forearm, as `bat_hard_forearm` swings
/// it, cut short.
fn hard_bat_on_the_forearm() -> Scenario {
    let strike =
        Strike::parse("bat:-0.700,0.190:0.200,0.190:power=2.1:frames=6:windup=6:settle=20")
            .expect("a valid swing");
    Scenario::custom(Play::Swing(strike, None))
}

fn play(scenario: &Scenario, watch: impl FnOnce(&mut rp::World)) -> rp::World {
    let mut world = rp::create_layered_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, scenario.materials());
    watch(&mut world);
    run_on(
        world,
        scenario,
        SCENARIO_WIDTH,
        SCENARIO_HEIGHT,
        |_, _, _| {},
    )
    .0
}

/// Every point and bone where it is, bit for bit.
fn places(world: &rp::World) -> Vec<u64> {
    let points = world
        .points()
        .iter()
        .flat_map(|point| [point.position.x, point.position.y]);
    let bones = world
        .bones()
        .iter()
        .flat_map(|bone| [bone.a.x, bone.a.y, bone.b.x, bone.b.y]);
    points.chain(bones).map(f64::to_bits).collect()
}

#[test]
fn keeping_a_journal_or_a_trace_changes_nothing() {
    let scenario = hard_bat_on_the_forearm();
    let plain = places(&play(&scenario, |_| {}));
    let journaled = play(&scenario, rp::World::keep_journal);
    assert!(
        journaled
            .journal()
            .is_some_and(|journal| !journal.events.is_empty()),
        "the blow should leave something in the journal"
    );
    assert!(
        places(&journaled) == plain,
        "keeping a journal moved the body"
    );
    let traced = play(&scenario, |world| {
        world.trace(&[rp::Traced::Point(100), rp::Traced::Bone(5)]);
    });
    assert!(places(&traced) == plain, "tracing moved the body");
}

/// The why splits a motion into each phase's share: they and the motion the
/// thing already had must add up to how far it went, here a point of skin
/// the hard bat throws.
#[test]
fn the_shares_of_a_motion_add_up_to_it() {
    let scenario = hard_bat_on_the_forearm();
    let mut world = rp::create_layered_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, scenario.materials());
    let target = rp::inspect::BlameTarget::Point(82);
    world.trace(&target.traced(&world));
    let start = world.points()[82].position;
    let dt = world.materials().fixed_dt;
    let body = rp::body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT);
    let mut steps = Vec::new();
    for frame in 0..scenario.frames() {
        world.step(dt, &scenario.input(frame, dt, body), SCENARIO_WIDTH);
        steps.push((frame, world.traced_stages().to_vec()));
    }
    let end = world.points()[82].position;
    let total = rp::inspect::blame(&steps, 0, dt).total();
    let (moved_x, moved_y) = (end.x - start.x, end.y - start.y);
    assert!(
        moved_x.hypot(moved_y) > 5.0,
        "the blow should move the point"
    );
    assert!(
        (total.x - moved_x).abs() < 0.5 && (total.y - moved_y).abs() < 0.5,
        "shares add to ({:.2}, {:.2}), but it moved ({moved_x:.2}, {moved_y:.2})",
        total.x,
        total.y
    );
}

/// A body nothing touches does nothing the watchdogs flag.
#[test]
fn a_body_at_rest_trips_no_watchdog() {
    let mut world =
        rp::create_layered_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, rp::Materials::default());
    let mut watchdogs = rp::inspect::Watchdogs::new(&world);
    let dt = world.materials().fixed_dt;
    for frame in 0..150 {
        world.step(dt, &rp::InputState::default(), SCENARIO_WIDTH);
        watchdogs.check(frame, &world);
    }
    assert!(watchdogs.trips().is_empty(), "{:?}", watchdogs.trips());
}

#[test]
fn the_journal_says_what_broke_the_forearm() {
    let world = play(&hard_bat_on_the_forearm(), rp::World::keep_journal);
    let journal = world.journal().expect("a journal");
    let breaks: Vec<&rp::Event> = journal
        .events
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                rp::EventKind::BoneBroken | rp::EventKind::BoneBrokenOpen
            )
        })
        .collect();
    let forearm = breaks.iter().find(|event| match event.subject {
        rp::Subject::Bone(index) => world.bones()[index].part == rp::BonePart::Forearm,
        _ => false,
    });
    let forearm = forearm.expect("the hard bat should break the forearm");
    assert_eq!(forearm.by, rp::Actor::Tool, "the bat broke it");
    assert!(forearm.value >= forearm.limit, "{forearm:?}");
    assert_eq!(
        journal.steps.len(),
        hard_bat_on_the_forearm().frames() as usize
    );
    assert!(
        journal
            .events
            .iter()
            .any(|event| event.kind == rp::EventKind::LimbWentLimp),
        "a broken forearm hangs limp"
    );
}
