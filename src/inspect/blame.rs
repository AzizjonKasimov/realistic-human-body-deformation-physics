//! What moved a point or a bone: each phase of each step is blamed for the
//! part of the motion its pushes made. A phase that changed the velocity by
//! dv at time t moved the thing dv times the time left by the window's end,
//! so the phases' shares and the motion the thing already had add up to how
//! far it went; for a bone, the same for how far it turned.

use crate::simulation::{Traced, TracedStage, Vec2, World};

/// What to blame a motion on: a point or a bone; a fiber or a joint, by how
/// its two ends moved apart or bent; or a triangle, by how its corners moved
/// across its sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlameTarget {
    Point(usize),
    Bone(usize),
    Spring(usize),
    Joint(usize),
    Triangle(usize),
}

impl BlameTarget {
    /// Reads `point:N`, `bone:N`, `spring:N`, `joint:N`, or `triangle:N`.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let wrong =
            || format!("expected point:N, bone:N, spring:N, joint:N, or triangle:N, got `{spec}`");
        let (kind, index) = spec.split_once(':').ok_or_else(wrong)?;
        let index: usize = index.parse().map_err(|_| wrong())?;
        match kind {
            "point" => Ok(Self::Point(index)),
            "bone" => Ok(Self::Bone(index)),
            "spring" | "fiber" => Ok(Self::Spring(index)),
            "joint" => Ok(Self::Joint(index)),
            "triangle" => Ok(Self::Triangle(index)),
            _ => Err(wrong()),
        }
    }

    /// The points or bones to follow for it: the one, its two ends, or its
    /// three corners.
    pub fn traced(self, world: &World) -> Vec<Traced> {
        match self {
            Self::Point(index) => vec![Traced::Point(index)],
            Self::Bone(index) => vec![Traced::Bone(index)],
            Self::Spring(index) => world
                .springs()
                .get(index)
                .map(|spring| vec![Traced::Point(spring.a), Traced::Point(spring.b)])
                .unwrap_or_default(),
            Self::Joint(index) => world
                .bone_joints()
                .get(index)
                .map(|joint| vec![Traced::Bone(joint.a), Traced::Bone(joint.b)])
                .unwrap_or_default(),
            Self::Triangle(index) => world
                .triangles()
                .get(index)
                .map(|triangle| {
                    vec![
                        Traced::Point(triangle.a),
                        Traced::Point(triangle.b),
                        Traced::Point(triangle.c),
                    ]
                })
                .unwrap_or_default(),
        }
    }
}

/// One phase's share of a motion: how far its pushes moved the thing by the
/// window's end, and how far they turned it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhasePush {
    pub phase: &'static str,
    pub push: Vec2,
    pub turn: f64,
}

/// What moved a thing over a window of steps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Blame {
    /// How far it would have gone, and turned, at the speed it had when the
    /// window began.
    pub carried: Vec2,
    pub carried_turn: f64,
    /// Each phase's share, in the order the phases first pushed.
    pub phases: Vec<PhasePush>,
}

impl Blame {
    /// How far the pushes and what it carried moved it in all.
    pub fn total(&self) -> Vec2 {
        self.phases.iter().fold(self.carried, |sum, phase| Vec2 {
            x: sum.x + phase.push.x,
            y: sum.y + phase.push.y,
        })
    }

    /// How far they turned it in all.
    pub fn total_turn(&self) -> f64 {
        self.carried_turn + self.phases.iter().map(|phase| phase.turn).sum::<f64>()
    }

    /// Each share times `factor`.
    pub fn times(&self, factor: f64) -> Blame {
        Blame {
            carried: Vec2 {
                x: self.carried.x * factor,
                y: self.carried.y * factor,
            },
            carried_turn: self.carried_turn * factor,
            phases: self
                .phases
                .iter()
                .map(|phase| PhasePush {
                    phase: phase.phase,
                    push: Vec2 {
                        x: phase.push.x * factor,
                        y: phase.push.y * factor,
                    },
                    turn: phase.turn * factor,
                })
                .collect(),
        }
    }

    /// Each share of this and of `other` added, phase by phase.
    pub fn plus(&self, other: &Blame) -> Blame {
        let mut phases: Vec<PhasePush> = self.phases.clone();
        for theirs in &other.phases {
            match phases.iter_mut().find(|phase| phase.phase == theirs.phase) {
                Some(entry) => {
                    entry.push.x += theirs.push.x;
                    entry.push.y += theirs.push.y;
                    entry.turn += theirs.turn;
                }
                None => phases.push(*theirs),
            }
        }
        Blame {
            carried: Vec2 {
                x: self.carried.x + other.carried.x,
                y: self.carried.y + other.carried.y,
            },
            carried_turn: self.carried_turn + other.carried_turn,
            phases,
        }
    }

    /// How `other` moved against this: each share of `other` less this one's.
    pub fn against(&self, other: &Blame) -> Blame {
        self.times(-1.0).plus(other)
    }
}

/// Blames the phases for how the thing traced as `target` moved over the
/// steps `steps` hold, each with its frame and what its phases did, a step
/// lasting `dt` seconds.
pub fn blame(steps: &[(i32, Vec<TracedStage>)], target: usize, dt: f64) -> Blame {
    let Some(last) = steps.last() else {
        return Blame::default();
    };
    blame_until(steps, target, dt, f64::from(last.0 + 1) * dt)
}

/// The same for each step of `steps` on its own: what each step's pushes
/// moved the thing by the end of the last step.
pub fn blame_by_step(steps: &[(i32, Vec<TracedStage>)], target: usize, dt: f64) -> Vec<Blame> {
    let Some(last) = steps.last() else {
        return Vec::new();
    };
    let end = f64::from(last.0 + 1) * dt;
    steps
        .iter()
        .map(|step| {
            let mut blame = blame_until(std::slice::from_ref(step), target, dt, end);
            blame.carried = Vec2::default();
            blame.carried_turn = 0.0;
            blame
        })
        .collect()
}

fn blame_until(steps: &[(i32, Vec<TracedStage>)], target: usize, dt: f64, end: f64) -> Blame {
    let mut result = Blame::default();
    let Some(first) = steps.first() else {
        return result;
    };
    let start = f64::from(first.0) * dt;
    let mut began = false;
    for (frame, stages) in steps {
        for stage in stages.iter().filter(|stage| stage.target == target) {
            let time = (f64::from(*frame) + stage.time) * dt;
            if !began {
                began = true;
                let span = end - start;
                result.carried = Vec2 {
                    x: stage.velocity_before.x * span,
                    y: stage.velocity_before.y * span,
                };
                result.carried_turn = stage.spin_before * span;
            }
            let left = end - time;
            let push = Vec2 {
                x: (stage.velocity_after.x - stage.velocity_before.x) * left,
                y: (stage.velocity_after.y - stage.velocity_before.y) * left,
            };
            let turn = (stage.spin_after - stage.spin_before) * left;
            match result
                .phases
                .iter_mut()
                .find(|phase| phase.phase == stage.phase)
            {
                Some(phase) => {
                    phase.push.x += push.x;
                    phase.push.y += push.y;
                    phase.turn += turn;
                }
                None => result.phases.push(PhasePush {
                    phase: stage.phase,
                    push,
                    turn,
                }),
            }
        }
    }
    result
}
