//! Following a bone and the flesh tied to it through each phase of a step
//! ([`World::trace`]): how far each phase moved the bone and, on average,
//! its flesh, and how far it slid the flesh along and across the bone, as
//! if the flesh were fixed to the bone and something then moved it off its
//! place: what pulls a bone out of its skin. The engine's `print_bare_bones`
//! (`RP_BARE_BONE` with `RP_BARE_TRACE`) and the game's bout trial print it.

use crate::simulation::{Traced, TracedStage, Vec2, World};

/// How many points of the flesh tied to a bone [`follow_bone`] follows.
const FOLLOWED_FLESH: usize = 6;

/// The flesh tied to `bone` by ties not torn, nearest its far end first, at
/// most six points: what [`follow_bone`] follows beside it.
pub fn tied_flesh(world: &World, bone: usize) -> Vec<usize> {
    let Some(end) = world.bones().get(bone).map(|bone| bone.b) else {
        return Vec::new();
    };
    let points = world.points();
    let mut tied: Vec<(f64, usize)> = world
        .bone_attachments()
        .iter()
        .filter(|tie| tie.bone == bone && !tie.broken)
        .map(|tie| {
            let at = points[tie.point].position;
            let (x, y) = (at.x - end.x, at.y - end.y);
            (x * x + y * y, tie.point)
        })
        .collect();
    tied.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    tied.dedup_by_key(|(_, point)| *point);
    tied.into_iter()
        .take(FOLLOWED_FLESH)
        .map(|(_, point)| point)
        .collect()
}

/// Starts following `bone` and the flesh tied to it through each phase of
/// every later step ([`World::trace`]; `world.trace(&[])` stops), and
/// returns that flesh, for [`followed_phases`].
pub fn follow_bone(world: &mut World, bone: usize) -> Vec<usize> {
    let flesh = tied_flesh(world, bone);
    let mut targets = vec![Traced::Bone(bone)];
    targets.extend(flesh.iter().map(|&point| Traced::Point(point)));
    world.trace(&targets);
    flesh
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

fn length(value: Vec2) -> f64 {
    libm::hypot(value.x, value.y)
}

/// `value` turned by `angle`.
fn turned(value: Vec2, angle: f64) -> Vec2 {
    let (sin, cos) = (libm::sin(angle), libm::cos(angle));
    Vec2 {
        x: value.x * cos - value.y * sin,
        y: value.x * sin + value.y * cos,
    }
}

/// What one run of a phase did: when in the step it ran, how far it moved
/// the bone's middle and turned it, how far it moved the flesh on average,
/// and how far it slid the flesh off where the bone's own motion would
/// have carried it, along the bone (toward its `b` end) and across it.
#[derive(Clone, Copy)]
struct Run {
    phase: &'static str,
    call: usize,
    time: f64,
    bone: Vec2,
    turn: f64,
    flesh: Vec2,
    slid: Vec2,
}

/// What the last step did to the bone [`follow_bone`] follows and its
/// `flesh`: a first line with where the bone and its flesh's middle stand,
/// then a line for each phase that moved either more than `least` pixels
/// (or turned the bone more than `least` / 25 radians, or slid the flesh
/// more than `least`), the phase's runs added up over the step, or, with
/// `each_run`, a line for each run, with when in the step it ran. The
/// flesh's slide is how far the phase moved it off its place on the bone,
/// on average: along the bone toward its `b` end (negative: the bone moved
/// on toward `b` through its flesh) and across it, a quarter turn
/// counterclockwise from `a` to `b`.
pub fn followed_phases(
    world: &World,
    bone: usize,
    flesh: &[usize],
    least: f64,
    each_run: bool,
) -> Vec<String> {
    let Some(segment) = world.bones().get(bone) else {
        return Vec::new();
    };
    let points = world.points();
    let middle = Vec2 {
        x: (segment.a.x + segment.b.x) * 0.5,
        y: (segment.a.y + segment.b.y) * 0.5,
    };
    let share = 1.0 / flesh.len().max(1) as f64;
    let flesh_middle = flesh.iter().fold(Vec2::default(), |sum, &point| Vec2 {
        x: sum.x + points[point].position.x * share,
        y: sum.y + points[point].position.y * share,
    });
    // How far its muscle's ties have slipped off their places on it, in
    // millimetres, against how far they tear at.
    let slips: Vec<f64> = world
        .bone_attachments()
        .iter()
        .filter(|tie| tie.bone == bone && !tie.broken)
        .map(|tie| world.bone_tie_slip(tie) * world.mm_per_px())
        .collect();
    let torn = world
        .bone_attachments()
        .iter()
        .filter(|tie| tie.bone == bone && tie.broken)
        .count();
    let worst = slips.iter().copied().fold(0.0, f64::max);
    let mean = slips.iter().sum::<f64>() / slips.len().max(1) as f64;
    let mut lines = vec![format!(
        "bone {bone} middle ({:.1}, {:.1}), ends ({:.1}, {:.1}) and ({:.1}, {:.1}); its flesh's middle ({:.1}, {:.1}); its {} ties slipped {mean:.0} mm on average, {worst:.0} at most (tear at {:.0}), {torn} torn",
        middle.x,
        middle.y,
        segment.a.x,
        segment.a.y,
        segment.b.x,
        segment.b.y,
        flesh_middle.x,
        flesh_middle.y,
        slips.len(),
        world.materials().bone_tie_tear_mm
    )];
    // The phases' runs in the order they ran, each with what it did to the
    // bone and to each point of flesh.
    let stages: &[TracedStage] = world.traced_stages();
    let mut calls: Vec<usize> = stages.iter().map(|stage| stage.call).collect();
    calls.dedup();
    calls.sort_unstable();
    calls.dedup();
    // Walked back from where everything stands after the step, each run's
    // moves taken off, so each run is judged from where it found things.
    let mut bone_at = middle;
    let mut angle = libm::atan2(segment.b.y - segment.a.y, segment.b.x - segment.a.x);
    let mut flesh_at: Vec<Vec2> = flesh.iter().map(|&point| points[point].position).collect();
    let mut runs: Vec<Run> = Vec::new();
    for &call in calls.iter().rev() {
        let mut run = Run {
            phase: "",
            call,
            time: 0.0,
            bone: Vec2::default(),
            turn: 0.0,
            flesh: Vec2::default(),
            slid: Vec2::default(),
        };
        let mut moved = vec![Vec2::default(); flesh.len()];
        for stage in stages.iter().filter(|stage| stage.call == call) {
            run.phase = stage.phase;
            run.time = stage.time;
            match stage.target {
                0 => {
                    run.bone = stage.moved;
                    run.turn = stage.turned;
                }
                target => {
                    if let Some(slot) = moved.get_mut(target - 1) {
                        *slot = stage.moved;
                    }
                }
            }
        }
        // The step's prediction is taken back, untraced, as the solver
        // splits the step into substeps (`split_step_into_substeps`): what
        // the first substep found is what stood before it.
        let undone = run.phase == "predict";
        let bone_before = if undone {
            bone_at
        } else {
            sub(bone_at, run.bone)
        };
        let angle_before = if undone { angle } else { angle - run.turn };
        let along = Vec2 {
            x: libm::cos(angle_before),
            y: libm::sin(angle_before),
        };
        let across = Vec2 {
            x: -along.y,
            y: along.x,
        };
        for (at, &moved) in flesh_at.iter_mut().zip(&moved) {
            let before = if undone { *at } else { sub(*at, moved) };
            let off = sub(before, bone_before);
            // Where the bone's own motion would have carried the point, as
            // if fixed to it.
            let carried = sub(add(run.bone, turned(off, run.turn)), off);
            let slid = sub(moved, carried);
            run.flesh = add(
                run.flesh,
                Vec2 {
                    x: moved.x * share,
                    y: moved.y * share,
                },
            );
            run.slid = add(
                run.slid,
                Vec2 {
                    x: (slid.x * along.x + slid.y * along.y) * share,
                    y: (slid.x * across.x + slid.y * across.y) * share,
                },
            );
            *at = before;
        }
        bone_at = bone_before;
        angle = angle_before;
        runs.push(run);
    }
    runs.reverse();
    // Added up by phase, unless each run is wanted.
    let mut shown: Vec<Run> = Vec::new();
    for run in runs {
        match shown
            .iter_mut()
            .find(|known| !each_run && known.phase == run.phase)
        {
            Some(known) => {
                known.bone = add(known.bone, run.bone);
                known.turn += run.turn;
                known.flesh = add(known.flesh, run.flesh);
                known.slid = add(known.slid, run.slid);
            }
            None => shown.push(run),
        }
    }
    for run in shown {
        if length(run.bone) > least
            || length(run.flesh) > least
            || length(run.slid) > least
            || run.turn.abs() > 0.04 * least
        {
            let when = if each_run {
                format!(" at {:.2} (run {})", run.time, run.call)
            } else {
                String::new()
            };
            lines.push(format!(
                "{:>20}{when}: bone ({:+.1}, {:+.1}) px, turned {:+.3} rad; flesh ({:+.1}, {:+.1}) px, slid {:+.1} along, {:+.1} across",
                run.phase,
                run.bone.x,
                run.bone.y,
                run.turn,
                run.flesh.x,
                run.flesh.y,
                run.slid.x,
                run.slid.y
            ));
        }
    }
    lines
}
