//! Timing of each phase of a step, to see what a step spends its time on. It
//! runs only when a caller hands the world a clock ([`World::time_phases`]),
//! as `strike_scenarios --phases` does; the app never does, so the browser
//! build needs no clock.
//!
//! The same hook can follow points and bones through every phase
//! ([`World::trace`]), as `strike_scenarios --trace` and `--why` do, to find
//! which phase throws a piece of tissue or bone, and what moved it.

use super::{midpoint, scale, subtract, BoneSegment, Point, Vec2, World};

/// Reads a clock, in milliseconds.
pub type PhaseClock = fn() -> f64;

/// Time spent in each phase of the steps timed so far.
#[derive(Clone, Debug, Default)]
pub struct PhaseTimes {
    /// Steps timed.
    pub steps: usize,
    /// Milliseconds the timed steps took, whole.
    pub total_ms: f64,
    /// The slowest step, whole.
    pub slowest_step_ms: f64,
    /// Each phase, in the order it first ran.
    pub phases: Vec<PhaseTime>,
    /// Where the next phase to record most likely sits in `phases`, since a
    /// step runs its phases in the same order every time.
    cursor: usize,
}

/// One phase of a step. A phase of the solver loop counts all its passes.
#[derive(Clone, Copy, Debug)]
pub struct PhaseTime {
    pub name: &'static str,
    pub total_ms: f64,
    /// The most this phase took in one step.
    pub slowest_step_ms: f64,
    /// Its time so far in the step being timed.
    this_step_ms: f64,
}

impl PhaseTime {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            total_ms: 0.0,
            slowest_step_ms: 0.0,
            this_step_ms: 0.0,
        }
    }
}

impl PhaseTimes {
    fn index_of(&mut self, name: &'static str) -> usize {
        if self
            .phases
            .get(self.cursor)
            .is_some_and(|phase| phase.name == name)
        {
            return self.cursor;
        }
        self.phases
            .iter()
            .position(|phase| phase.name == name)
            .unwrap_or_else(|| {
                self.phases.push(PhaseTime::new(name));
                self.phases.len() - 1
            })
    }

    fn record(&mut self, name: &'static str, ms: f64) {
        let index = self.index_of(name);
        self.phases[index].this_step_ms += ms;
        self.cursor = index + 1;
    }

    fn finish_step(&mut self, ms: f64) {
        self.steps += 1;
        self.total_ms += ms;
        self.slowest_step_ms = self.slowest_step_ms.max(ms);
        for phase in &mut self.phases {
            phase.total_ms += phase.this_step_ms;
            phase.slowest_step_ms = phase.slowest_step_ms.max(phase.this_step_ms);
            phase.this_step_ms = 0.0;
        }
        self.cursor = 0;
    }

    /// Adds the times of another run, such as another scenario's.
    pub fn merge(&mut self, other: &PhaseTimes) {
        self.steps += other.steps;
        self.total_ms += other.total_ms;
        self.slowest_step_ms = self.slowest_step_ms.max(other.slowest_step_ms);
        for phase in &other.phases {
            let index = self.index_of(phase.name);
            let mine = &mut self.phases[index];
            mine.total_ms += phase.total_ms;
            mine.slowest_step_ms = mine.slowest_step_ms.max(phase.slowest_step_ms);
        }
        self.cursor = 0;
    }
}

/// What [`World::trace`] follows through each phase of a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Traced {
    Point(usize),
    /// A bone, by its middle and its angle.
    Bone(usize),
}

/// What one phase of a step did to one traced point or bone: how far it
/// moved it, and its velocity before and after, in pixels per second over
/// the substep (or the whole step) the phase ran in; for a bone, how far it
/// turned it too, and its turning speed before and after, in radians and
/// radians per second.
#[derive(Clone, Copy, Debug)]
pub struct TracedStage {
    /// Which traced thing, by its place in the list [`World::trace`] took.
    pub target: usize,
    pub phase: &'static str,
    /// When in the step the phase ran, as a share of the step: 0 before
    /// the solver, the start of its substep within it, and 1 after it.
    pub time: f64,
    pub moved: Vec2,
    pub velocity_before: Vec2,
    pub velocity_after: Vec2,
    pub turned: f64,
    pub spin_before: f64,
    pub spin_after: f64,
}

/// Where a traced point or bone is, and where it was a substep (or step)
/// ago; a bone's angle now and then.
#[derive(Clone, Copy)]
struct TracedPlace {
    now: Vec2,
    before: Vec2,
    angle: f64,
    angle_before: f64,
}

impl TracedPlace {
    fn of_point(point: &Point) -> Self {
        Self {
            now: point.position,
            before: point.previous,
            angle: 0.0,
            angle_before: 0.0,
        }
    }

    fn of_bone(bone: &BoneSegment) -> Self {
        let angle = |a: Vec2, b: Vec2| libm::atan2(b.y - a.y, b.x - a.x);
        Self {
            now: midpoint(bone.a, bone.b),
            before: midpoint(bone.previous_a, bone.previous_b),
            angle: angle(bone.a, bone.b),
            angle_before: angle(bone.previous_a, bone.previous_b),
        }
    }
}

/// An angle wrapped into -pi to pi.
fn wrapped(angle: f64) -> f64 {
    let turns = libm::floor((angle + std::f64::consts::PI) / std::f64::consts::TAU);
    angle - turns * std::f64::consts::TAU
}

impl World {
    /// Follows each of `traced` through each phase of every later step, for
    /// [`World::traced_stages`]; an empty list stops.
    pub fn trace(&mut self, traced: &[Traced]) {
        self.traced = traced.to_vec();
        self.traced_stages.clear();
    }

    /// What each phase of the last step did to what [`World::trace`]
    /// follows, the phases that left a thing alone left out.
    pub fn traced_stages(&self) -> &[TracedStage] {
        &self.traced_stages
    }

    fn traced_place(&self, traced: Traced) -> Option<TracedPlace> {
        match traced {
            Traced::Point(index) => self.points.get(index).map(TracedPlace::of_point),
            Traced::Bone(index) => self.bones.get(index).map(TracedPlace::of_bone),
        }
    }

    fn traced_places(&self) -> Vec<Option<TracedPlace>> {
        self.traced
            .iter()
            .map(|&traced| self.traced_place(traced))
            .collect()
    }

    fn record_traced_stages(&mut self, phase: &'static str, before: &[Option<TracedPlace>]) {
        let h = self.solver_dt.max(1.0e-9);
        for (target, (&traced, before)) in self.traced.iter().zip(before).enumerate() {
            let (Some(before), Some(after)) = (*before, self.traced_place(traced)) else {
                continue;
            };
            let velocity = |place: TracedPlace| scale(subtract(place.now, place.before), 1.0 / h);
            let spin = |place: TracedPlace| wrapped(place.angle - place.angle_before) / h;
            let stage = TracedStage {
                target,
                phase,
                time: self.trace_time,
                moved: subtract(after.now, before.now),
                velocity_before: velocity(before),
                velocity_after: velocity(after),
                turned: wrapped(after.angle - before.angle),
                spin_before: spin(before),
                spin_after: spin(after),
            };
            let changed = subtract(stage.velocity_after, stage.velocity_before);
            if stage.moved != Vec2::default()
                || changed != Vec2::default()
                || stage.turned != 0.0
                || stage.spin_after != stage.spin_before
            {
                self.traced_stages.push(stage);
            }
        }
    }

    /// Marks how far through the step the phases that follow run, for
    /// [`TracedStage::time`].
    pub(super) fn mark_trace_time(&mut self, time: f64) {
        self.trace_time = time;
    }

    /// Times each phase of every later step with `clock`, from fresh totals;
    /// `None` stops timing.
    pub fn time_phases(&mut self, clock: Option<PhaseClock>) {
        self.phase_clock = clock;
        self.phase_times = PhaseTimes::default();
    }

    /// What [`World::time_phases`] has measured so far.
    pub fn phase_times(&self) -> &PhaseTimes {
        &self.phase_times
    }

    /// Runs one phase of a step, timing it when a clock is set.
    pub(super) fn timed(&mut self, name: &'static str, phase: impl FnOnce(&mut Self)) {
        self.journal_phase(name);
        let traced = (!self.traced.is_empty()).then(|| self.traced_places());
        match self.phase_clock {
            Some(clock) => {
                let started = clock();
                phase(self);
                let ms = clock() - started;
                self.phase_times.record(name, ms);
            }
            None => phase(self),
        }
        if let Some(before) = traced {
            self.record_traced_stages(name, &before);
        }
    }

    /// Runs a whole step, timing it when a clock is set.
    pub(super) fn timed_step(&mut self, step: impl FnOnce(&mut Self)) {
        self.traced_stages.clear();
        self.trace_time = 0.0;
        let Some(clock) = self.phase_clock else {
            return step(self);
        };
        let started = clock();
        step(self);
        let ms = clock() - started;
        self.phase_times.finish_step(ms);
    }
}
