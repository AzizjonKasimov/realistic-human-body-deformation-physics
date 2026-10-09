//! Timing of each phase of a step, to see what a step spends its time on. It
//! runs only when a caller hands the world a clock ([`World::time_phases`]),
//! as `strike_scenarios --phases` does; the app never does, so the browser
//! build needs no clock.

use super::World;

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

impl World {
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
        let Some(clock) = self.phase_clock else {
            return phase(self);
        };
        let started = clock();
        phase(self);
        let ms = clock() - started;
        self.phase_times.record(name, ms);
    }

    /// Runs a whole step, timing it when a clock is set.
    pub(super) fn timed_step(&mut self, step: impl FnOnce(&mut Self)) {
        let Some(clock) = self.phase_clock else {
            return step(self);
        };
        let started = clock();
        step(self);
        let ms = clock() - started;
        self.phase_times.finish_step(ms);
    }
}
