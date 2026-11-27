//! What can be seen of side-view fighters, for a game's computer opponent and
//! its rules: what each fighter is doing (a punch on its way, a step, a lean,
//! a duck, a block), as a person watching the screen would read it, what
//! each punch that landed this step struck, the head, the body, or an arm,
//! and how much strength each fighter's arms have left as they tire
//! (`fatigue.rs`). None of it feeds the simulation: it is read after a step,
//! and reading it changes nothing.

use super::tools::Spared;
use super::*;

/// A punch: the lead hand's straight jab or the rear hand's cross.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PunchKind {
    Jab,
    Cross,
}

/// A punch a fighter is throwing ([`Doing::punch`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Punching {
    pub kind: PunchKind,
    /// Thrown at the body, from a duck, rather than at the head.
    pub to_body: bool,
    /// Seconds since it set off.
    pub age: f64,
    /// Seconds its fist takes from the guard to the end of its way, unless
    /// it lands or is stopped first.
    pub out_seconds: f64,
    /// Still on its way out: it has neither landed, been stopped, nor begun
    /// to come back.
    pub going_out: bool,
    /// Its fist has landed on something: a head, a body, or an arm.
    pub landed: bool,
    /// How far out its fist is, from 0 in the guard to 1 at the end of its
    /// way.
    pub reach: f64,
}

/// A step a fighter is taking ([`Doing::step`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stepping {
    /// Toward the other fighter, or away from it.
    pub forward: bool,
    /// How far through the step it is, from 0 as it sets off to 1.
    pub progress: f64,
    /// Taken to catch the fighter as its weight ran past its feet, after a
    /// shove or a blow, rather than told.
    pub catching: bool,
}

/// What a side-view fighter is doing ([`World::fighter_doing`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Doing {
    pub punch: Option<Punching>,
    pub step: Option<Stepping>,
    /// How far into a lean back, a duck, and a block its body is, from 0 in
    /// its stance to 1 in the move's pose.
    pub lean: f64,
    pub duck: f64,
    pub block: f64,
    /// Whether it holds each now: told to, and not let go of yet.
    pub leaning: bool,
    pub ducking: bool,
    pub blocking: bool,
    /// Its forearms are closed far enough to stop a punch; until then a
    /// punch goes between its hands. Closed while it ducks, they cover its
    /// body instead of its face (`guard_low`).
    pub guarded: bool,
    pub guard_low: bool,
    /// It is down, fallen or knocked out ([`World::fighter_fallen`]), from
    /// the moment it goes down until it stands on its feet again.
    pub fallen: bool,
    /// It is getting up: its muscles have come back and it is on its way
    /// from the floor to its stance (`rise.rs`).
    pub getting_up: bool,
}

/// What a landed punch struck ([`LandedPunch::struck`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Struck {
    /// The head or the neck.
    Head,
    /// The chest, the belly, or the hips.
    Body,
    /// An arm: a forearm closed in a block, or a hand of the guard.
    Arm,
    Leg,
}

/// A punch whose fist landed on another body this step
/// ([`World::landed_punches`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandedPunch {
    /// The fighter that threw it, and the punch.
    pub by: usize,
    pub kind: PunchKind,
    /// The body it landed on, and what of it the knuckles met first.
    pub on: usize,
    pub struck: Struck,
    /// How fast the fist was going as it landed, in metres a second.
    pub speed: f64,
}

/// How much strength a side-view fighter's arms have left
/// ([`World::fighter_stamina`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stamina {
    /// Each arm's, the viewer's left one first, from 1 fresh down toward 0:
    /// the share of its muscles' fresh strength they can give now, each
    /// joint's muscles counting by their strength (`fatigue.rs`).
    pub arms: [f64; 2],
}

impl Stamina {
    /// Both arms' together.
    pub fn both(&self) -> f64 {
        0.5 * (self.arms[0] + self.arms[1])
    }
}

impl World {
    /// How much strength side-view fighter `body`'s arms have left as their
    /// muscles tire and recover; `None` for a body that is not a side-view
    /// fighter.
    pub fn fighter_stamina(&self, body: usize) -> Option<Stamina> {
        self.bodies.get(body)?.mover.as_ref()?;
        // Each arm's muscles' strength left, and fresh.
        let mut sums = [(0.0, 0.0); 2];
        for joint in &self.bone_joints {
            let arm = match joint.drive.limb {
                Limb::LeftArm => 0,
                Limb::RightArm => 1,
                _ => continue,
            };
            if self.joint_body(joint) != body {
                continue;
            }
            for muscles in joint.drive.muscles.iter().filter(|muscles| muscles.tires()) {
                sums[arm].0 += joint.drive.max_torque * muscles.capacity();
                sums[arm].1 += joint.drive.max_torque;
            }
        }
        Some(Stamina {
            arms: sums.map(|(left, fresh)| if fresh > 0.0 { left / fresh } else { 1.0 }),
        })
    }

    /// What side-view fighter `body` is doing now, as someone watching it
    /// would see it; `None` for a body that is not a side-view fighter.
    pub fn fighter_doing(&self, body: usize) -> Option<Doing> {
        let mover = self.bodies.get(body)?.mover.as_ref()?;
        Some(mover.doing())
    }

    /// The punches whose fists landed on another body during the last step:
    /// each lands once, on the step its knuckles first meet something
    /// (`land_strikes`), whether it then drives on or is stopped.
    pub fn landed_punches(&self) -> &[LandedPunch] {
        &self.landed_punches
    }

    /// How far down the window side-view fighter `body`'s lowest part
    /// reaches, its shins and feet left out, since they stand on the floor:
    /// the lowest end of its trunk, head, thighs, and arms, in pixels, each
    /// bone's radius below it. The sound hears a falling fighter's body
    /// reach the floor by it (`sound.rs`); `None` for a body that is not a
    /// side-view fighter.
    pub fn fighter_low_point(&self, body: usize) -> Option<f64> {
        let rig = self.bodies.get(body)?.fighter.as_ref()?;
        [
            rig.pelvis,
            rig.lumbar,
            rig.thoracic,
            rig.cervical,
            rig.skull,
        ]
        .into_iter()
        .chain(rig.thighs)
        .chain(rig.upper_arms)
        .chain(rig.forearms)
        .chain(rig.hands)
        .filter_map(|bone| self.bones.get(bone))
        .map(|bone| bone.a.y.max(bone.b.y) + bone.radius)
        .reduce(f64::max)
    }

    /// What the knuckles of fighter `puncher`'s fist, their face's middle at
    /// `at`, met first: the body nearest them other than its own and the
    /// part of it, by the flesh nearest the knuckles and the bone that
    /// flesh is held to, or by the nearest bone where no flesh lies near,
    /// leaving out what the fist passes (`spares`, as it lands): a body
    /// punch's knuckles in a belly under a high block's elbows struck the
    /// belly, though the elbows' flesh lay nearer.
    pub(super) fn struck_by_fist(
        &self,
        puncher: usize,
        at: Vec2,
        spares: Spared,
    ) -> Option<(usize, Struck)> {
        let reach =
            tool_geometry(ToolMode::Fist).contact_radius + 2.0 * self.materials.point_spacing;
        let flesh = (0..self.points.len())
            .filter(|&point| {
                let held = &self.points[point];
                held.body as usize != puncher
                    && !spares.holds(held.body, held.sheet)
                    && !held.pinned
                    && !held.is_ghost()
                    && is_flesh(&self.flesh, point)
            })
            .map(|point| (distance(self.points[point].position, at), point))
            .filter(|&(far, _)| far <= reach)
            .min_by(|p, q| p.0.total_cmp(&q.0))
            .and_then(|(_, point)| {
                let bone = self.point_holds[point].posture.bones[0];
                (bone != MISSING_BONE).then_some(bone)
            });
        let bone = flesh.or_else(|| {
            (0..self.bones.len())
                .filter(|&bone| {
                    let segment = &self.bones[bone];
                    segment.body as usize != puncher && !spares.holds(segment.body, segment.sheet)
                })
                .map(|bone| {
                    let segment = &self.bones[bone];
                    (
                        distance_to_segment(at, segment.a, segment.b) - segment.radius,
                        bone,
                    )
                })
                .min_by(|p, q| p.0.total_cmp(&q.0))
                .map(|(_, bone)| bone)
        })?;
        let body = self.bones[bone].body as usize;
        Some((body, self.part_of_fighter(body, bone)))
    }

    /// What part of side-view fighter `body` bone `bone` is: its skull and
    /// neck the head, its arms' bones an arm, its legs' a leg, and the rest
    /// (the spine, the ribs, the hips) the body. A body that is not a
    /// fighter is all body.
    fn part_of_fighter(&self, body: usize, bone: usize) -> Struck {
        let Some(rig) = self
            .bodies
            .get(body)
            .and_then(|state| state.fighter.as_ref())
        else {
            return Struck::Body;
        };
        let in_pairs = |pairs: &[[usize; 2]]| pairs.iter().any(|pair| pair.contains(&bone));
        if bone == rig.skull || bone == rig.cervical {
            Struck::Head
        } else if in_pairs(&[rig.upper_arms, rig.forearms, rig.hands]) {
            Struck::Arm
        } else if in_pairs(&[rig.thighs, rig.shins, rig.feet]) {
            Struck::Leg
        } else {
            Struck::Body
        }
    }
}
