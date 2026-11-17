//! A side-view fighter standing on its own legs.
//!
//! A fighter's moves say where its hips should be and how its pelvis should
//! tilt (`mover.rs`, `Carry`), and a hold on the hips puts them there. That
//! hold pushed from nowhere: through a jab, a cross taken, a duck, and a
//! lean back it carried 94-103% of the fighters' weight on average and up to
//! two and a half times it, while their feet were held as if each bore half
//! the body (`print_fighter_support`). Nothing pushed back, so no blow could
//! knock a fighter over, and without the hold both fell flat on their backs
//! within a second.
//!
//! Here the legs make that push: every push the hold gives the hips comes
//! from the feet standing on the floor, each leg pressing its foot down with
//! the weight it bears, the floor taking the leg's push across through the
//! gripping foot, and the turn on the pelvis going back on the thighs, so
//! the floor bears the fighter's weight and its feet grip by the weight
//! really on them (`solve_foot_holds`). A fighter with no foot touching the
//! floor has nothing to push from, and its hips are not held at all. The two
//! feet share a push as legs standing apart do, each pushing along the line
//! from its ankle to the hips (Pratt, Chew, Torres, Dilworth and Pratt
//! 2001's virtual model control, which turns a push on the body into the
//! legs' pushes), turned off that line just enough to give the turn on the
//! pelvis between them as the share of the weight each bears says (Coros,
//! Beaudoin and van de Panne 2010). A step hands a foot's share to the
//! other before it lifts, and the leg carries the lifted foot along its
//! way (`carry_swinging_foot`).
//!
//! The legs push as a whole, between hips and feet: the hold does not yet
//! turn the hips, knees, and ankles that a person's legs push with, whose
//! muscles still only hold the pose the move asks for. Given as torques at
//! those joints, worked out at the start of each substep and given at once,
//! the push overshot: sized for the whole body but landing on the light
//! pieces it turns first, it turned the pelvis back and forth at 200
//! degrees a second and the fighter fell. The hold is solved as the
//! solver's other soft constraints are, which never overshoot.

use super::sides::BonePiece;
use super::soft::{Softness, KG_PER_MASS};
use super::*;
use crate::profile::{Hinge, Side};

/// A balancing fighter's feet standing on the floor, as the substep began:
/// where its legs push its hips from (between its two hip joints), and each
/// planted foot.
#[derive(Clone, Debug)]
pub(super) struct Stance {
    hips: Vec2,
    pub(super) feet: Vec<PlantedFoot>,
    /// The mass the legs hold up at the hips: all of the body but the
    /// planted legs, in the engine's units.
    above: f64,
    /// Where the body's capture point lies, across the window, and where
    /// the planted feet's ends reach, back and front: how far the hips must
    /// shift to keep it over the feet (`World::shift_over_the_feet`).
    capture: f64,
    support: (f64, f64),
    /// Where the body's centre of mass lies across the window, and how fast
    /// it moves across, for the tools that print it.
    centre_x: f64,
    speed_x: f64,
    /// The share of the body's mass that moves with the hips.
    moving: f64,
    /// All the hold has given the hips so far this substep, as a push and a
    /// turn about them, and what each leg has been given back for it: how
    /// hard its foot was pushed at the ankle, and how its thigh was turned.
    given: (Vec2, f64),
    pressed: Vec<(Vec2, f64)>,
    /// Getting up off the floor (`rise.rs`): what else bears the body on the
    /// floor besides its feet, and the share of the hips' push each foot,
    /// then each of those, bears, by where they stand under the hips; a hand
    /// bears none of it, only the chest's (`World::solve_chest_holds`).
    rising: bool,
    pub(super) others: Vec<Support>,
    pub(super) shares: Vec<f64>,
    /// What each of those was pressed into the floor with so far this
    /// substep, and the turn its thigh was given.
    others_pressed: Vec<(f64, f64)>,
    /// All the hold on the chest has given it so far this substep, getting
    /// up (`World::solve_chest_holds`), and all the floor has given each
    /// knee and fist it holds up (`World::hold_supports_up`).
    pub(super) chest_given: Vec2,
    floor_given: Vec<f64>,
}

/// What a support is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Bears {
    /// A knee, of the leg on that side.
    Knee(Side),
    /// A hand, the fist resting on the floor.
    Hand(Side),
}

/// A part of a fighter getting up that bears it on the floor besides its
/// feet: a knee or a hand.
#[derive(Clone, Debug)]
pub(super) struct Support {
    pub(super) bears: Bears,
    /// The bone and its flesh, and where the floor bears it: the knee's end
    /// of the shin, the knuckles' end of the hand.
    pub(super) piece: BonePiece,
    pub(super) at: Vec2,
    /// For a knee, the thigh it pushes through, which the turn the legs give
    /// the pelvis comes back on.
    thigh: Option<BonePiece>,
}

/// How near the floor the bone of a knee and a fist may be, in body
/// heights, for the flesh round it to rest on the floor and bear weight: the
/// shin's flesh round the knee and the fist's are about 6 and 5 cm thick
/// (`profile.rs`).
const KNEE_ON_FLOOR: f64 = 0.045;
const FIST_ON_FLOOR: f64 = 0.040;

/// How far across from the hips a foot or a knee may rest, at the most, in
/// body heights, for its leg to push them up: about a leg's length leaning
/// out under them (the hip joint stands 0.53 of the height up).
const LEG_REACH: f64 = 0.55;

/// How much of its flesh's thickness the floor leaves under the bone of a
/// knee and a fist it bears, in body heights: about half, as a heel's pad
/// keeps (`fighter.rs`, `HEEL_PAD_LOADED`), of the shin's 6 cm round the knee
/// and the fist's 5 cm. And the most the floor holds either up with, in body
/// weights.
const KNEE_PAD: f64 = 0.019;
const FIST_PAD: f64 = 0.015;
const SUPPORT_HOLDS: f64 = 2.0;

/// How far inside the planted feet's ends a fighter keeps its capture
/// point, as a share of how far apart those ends are: a centre of mass
/// coming within reach of a foot's edge tips the body over it.
const CAPTURE_MARGIN: f64 = 0.15;

/// The least share of the weight a leg bears standing still that it keeps
/// bearing while the legs push the hips across.
const LEAST_LOAD: f64 = 0.25;

/// How firmly a step carries a swinging foot along its way, in hertz,
/// critically damped, on the mass a leg turning about its hip has at its
/// foot: about 2.5 kg m² about the hip over a leg 0.9 m long (de Leva 1996's
/// thigh, shank, and foot), 3 kg.
const SWING_HERTZ: f64 = 10.0;
const SWING_MASS_KG: f64 = 3.0;

/// The hardest a leg swings its foot, in newtons: its hip's muscles
/// (`fighter::HIP_STRENGTH`) over the leg's 0.9 m.
const SWING_FORCE: f64 = fighter::HIP_STRENGTH / 0.9;

/// A foot standing on the floor, as the substep began.
#[derive(Clone, Debug)]
pub(super) struct PlantedFoot {
    side: Side,
    foot_hold: usize,
    /// How much of its share the foot bears (`FootHold::load`).
    load: f64,
    /// Where the leg's ankle joint holds the shin, and the foot's piece and
    /// the thigh's.
    ankle: Vec2,
    foot: BonePiece,
    thigh: BonePiece,
}

/// What a balancing fighter's legs pushed with, and its feet bore, over the
/// last substep they stood on the floor, in newtons and newton metres, for
/// the tools that print it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct BalanceReport {
    /// The push the legs gave the hips and the turn they gave the pelvis.
    pub(super) push: Vec2,
    pub(super) turn: f64,
    /// Each foot, the lead's first: whether it stood on the floor, and how
    /// hard the legs pushed it down and across.
    pub(super) planted: [bool; 2],
    pub(super) feet: [Vec2; 2],
    /// Where the body's centre of mass lies over its feet, from the rear
    /// foot's back end (0) to the lead foot's front end (1), the feet
    /// standing on the floor or not: outside 0 to 1, no foot can hold it.
    pub(super) over_feet: f64,
    /// Where the capture point lies over the planted feet, the same way,
    /// and how far the balance shifts the hips to keep it there, in
    /// centimetres.
    pub(super) capture_over_feet: f64,
    pub(super) shift_cm: f64,
    /// How far across the hips stood from where the hold aimed them, and
    /// where the move put them over the feet, in centimetres.
    pub(super) aim_off_cm: f64,
    pub(super) pose_off_cm: f64,
    /// How far below where the move puts them over the feet the hips stood,
    /// in centimetres.
    pub(super) sunk_cm: f64,
    /// Where the body's centre of mass is across the window, in
    /// centimetres, and how fast it moves across, in metres a second.
    pub(super) centre_cm: f64,
    pub(super) centre_speed: f64,
}

impl World {
    /// Whether body `body` is a side-view fighter whose legs carry its hips
    /// (`Materials::legs_carry`), so that its hold on them pushes back on its
    /// feet; and if so, whether it has a foot on the floor to push from.
    pub(super) fn legs_carry(&self, body: usize) -> Option<bool> {
        if !self.materials.legs_carry {
            return None;
        }
        let state = self.bodies.get(body)?;
        state.fighter.as_ref()?;
        // A fighter down on the floor no longer stands up on its legs, until
        // it gets up on what bears it there (`rise.rs`).
        if self.fighter_fallen(body) && !self.standing_up(body) {
            return Some(
                self.rising(body)
                    && self
                        .stances
                        .get(body)
                        .and_then(Option::as_ref)
                        .is_some_and(|stance| stance.shares.iter().any(|&share| share > 0.0)),
            );
        }
        Some(
            self.stances
                .get(body)
                .and_then(Option::as_ref)
                .is_some_and(|stance| !stance.feet.is_empty()),
        )
    }

    /// Measures each side-view fighter's feet standing on the floor as this
    /// substep begins, before its hold on the hips warm starts and pushes
    /// back on them.
    pub(super) fn measure_stances(&mut self) {
        self.stances.clear();
        self.foot_bearing.clear();
        if !self.materials.legs_carry || self.bodies.iter().all(|state| state.fighter.is_none()) {
            return;
        }
        self.foot_bearing = vec![Vec2::default(); self.foot_holds.len()];
        self.balance_reports
            .resize(self.bodies.len(), BalanceReport::default());
        self.stances = (0..self.bodies.len())
            .map(|body| {
                let rig = self.bodies[body].fighter.clone()?;
                Some(self.stance_of(body, &rig))
            })
            .collect();
        for body in 0..self.bodies.len() {
            let planted =
                self.stances
                    .get(body)
                    .and_then(Option::as_ref)
                    .map_or([false; 2], |stance| {
                        let mut planted = [false; 2];
                        for foot in &stance.feet {
                            planted[foot.side.index()] = true;
                        }
                        planted
                    });
            if let Some(rig) = self.bodies[body].fighter.clone() {
                let capture_over_feet =
                    self.stances
                        .get(body)
                        .and_then(Option::as_ref)
                        .map_or(0.5, |stance| {
                            let (back, front) = stance.support;
                            if front > back {
                                (stance.capture - back) / (front - back)
                            } else {
                                0.5
                            }
                        });
                let cm = self.mm_per_px() * 0.1;
                let (centre_cm, centre_speed) = self
                    .stances
                    .get(body)
                    .and_then(Option::as_ref)
                    .map_or((0.0, 0.0), |stance| {
                        (stance.centre_x * cm, stance.speed_x * cm * 0.01)
                    });
                self.balance_reports[body] = BalanceReport {
                    planted,
                    over_feet: self.centre_over_feet(body, &rig),
                    capture_over_feet,
                    shift_cm: self.shift_over_the_feet(body) * cm,
                    centre_cm,
                    centre_speed,
                    ..BalanceReport::default()
                };
            }
        }
    }

    /// Where fighter `body`'s centre of mass lies over its feet, from the
    /// rear foot's back end (0) to the lead foot's front end (1).
    fn centre_over_feet(&self, body: usize, rig: &fighter::Rig) -> f64 {
        let (mut mass, mut moment) = (0.0, 0.0);
        let rear = self.bones[rig.feet[Side::Rear.index()]];
        let lead = self.bones[rig.feet[Side::Lead.index()]];
        // From the rear foot's back end, so that mirror images sum alike.
        let back = if (lead.previous_b.x - rear.previous_a.x).abs()
            > (lead.previous_b.x - rear.previous_b.x).abs()
        {
            rear.previous_a.x
        } else {
            rear.previous_b.x
        };
        let front = if (lead.previous_a.x - back).abs() > (lead.previous_b.x - back).abs() {
            lead.previous_a.x
        } else {
            lead.previous_b.x
        };
        for (index, bone) in self.bones.iter().enumerate() {
            if bone.body as usize != body {
                continue;
            }
            if let Some(piece) = self.bone_pieces.get(index).and_then(Option::as_ref) {
                mass += piece.mass;
                moment += piece.mass * (piece.centre.x - back);
            }
        }
        if mass <= 0.0 || (front - back).abs() < EPSILON {
            return 0.5;
        }
        moment / mass / (front - back)
    }

    /// Fighter `body`'s feet standing on the floor.
    fn stance_of(&self, body: usize, rig: &fighter::Rig) -> Stance {
        let feet: Vec<PlantedFoot> = Side::BOTH
            .iter()
            .filter_map(|&side| self.planted_foot(body, rig, side))
            .collect();
        // The legs push the hips between their two joints there.
        let hip_joints: Vec<Vec2> = Side::BOTH
            .iter()
            .filter_map(|&side| {
                let joint = self.bone_joints.get(rig.hinge(Hinge::Hip(side)))?;
                let pelvis = self.bones.get(joint.a)?;
                Some(joint_seat(
                    joint,
                    pelvis,
                    pelvis.previous_a,
                    pelvis.previous_b,
                ))
            })
            .collect();
        let hips = if hip_joints.is_empty() {
            self.bones
                .get(rig.pelvis)
                .map_or(Vec2::default(), |pelvis| pelvis.previous_b)
        } else {
            scale(
                hip_joints
                    .iter()
                    .fold(Vec2::default(), |sum, &joint| add(sum, joint)),
                1.0 / hip_joints.len() as f64,
            )
        };
        // Getting up, its knees and hands bear it too, where they rest on the
        // floor.
        let rising = self.rising(body);
        let others = if rising {
            self.supports_on_floor(rig)
        } else {
            Vec::new()
        };
        // All of the body but the planted legs hangs on the hips, and as much
        // of a leg as its foot no longer bears: a hip takes up its leg's
        // weight as the foot hands its share over, so that the leg already
        // hangs from it as the foot lifts.
        let leg = |s: usize| [rig.thighs[s], rig.shins[s], rig.feet[s]];
        let planted: Vec<usize> = feet
            .iter()
            .flat_map(|foot| leg(foot.side.index()))
            .collect();
        let mass_of = |index: usize| {
            self.bone_pieces
                .get(index)
                .and_then(Option::as_ref)
                .map_or(0.0, |piece| piece.mass)
        };
        let above: f64 = (0..self.bones.len())
            .filter(|&index| self.bones[index].body as usize == body && !planted.contains(&index))
            .map(mass_of)
            .sum::<f64>()
            + feet
                .iter()
                .map(|foot| {
                    (1.0 - foot.load) * leg(foot.side.index()).into_iter().map(mass_of).sum::<f64>()
                })
                .sum::<f64>();
        // The capture point: where the centre of mass would come to rest
        // over a point it could stop on, as far ahead of it as it moves in
        // the time an upturned pendulum of its height takes to fall a
        // radian (Pratt, Carff, Drakunov and Goswami 2006). Measured from
        // the hips, so that mirror images sum alike.
        let (mut mass, mut moment, mut momentum) = (0.0, Vec2::default(), Vec2::default());
        let h = self.solver_dt.max(EPSILON);
        for point in &self.points {
            if point.body as usize != body || point.pinned || point.is_ghost() {
                continue;
            }
            mass += point.mass;
            moment = add(moment, scale(subtract(point.position, hips), point.mass));
            momentum = add(
                momentum,
                scale(subtract(point.position, point.previous), point.mass / h),
            );
        }
        for bone in &self.bones {
            if bone.body as usize != body {
                continue;
            }
            let middle = lerp(bone.a, bone.b, 0.5);
            let was = lerp(bone.previous_a, bone.previous_b, 0.5);
            mass += bone.mass;
            moment = add(moment, scale(subtract(middle, hips), bone.mass));
            momentum = add(momentum, scale(subtract(middle, was), bone.mass / h));
        }
        let (centre, speed) = if mass > 0.0 {
            (
                add(hips, scale(moment, 1.0 / mass)),
                scale(momentum, 1.0 / mass),
            )
        } else {
            (hips, Vec2::default())
        };
        let height = (self.floor_y - centre.y).max(1.0);
        let capture = centre.x + speed.x * (height / self.gravity_px().max(EPSILON)).sqrt();
        // A foot a step swings counts where it will come down: a step
        // carries the capture point on toward it, as robots step, keeping
        // the pressure on the standing foot and letting the capture point run
        // on to the next foothold (Englsberger, Ott and Albu-Schäffer 2015).
        // Counted by the standing foot alone, a step in read as a fall, and
        // the hold pulled the hips back 23 cm with all the push it had.
        // The floor presses only where a foot touches it, so a raised heel
        // bears nothing; a foot touching it at neither end yet counts both.
        let mut ends: Vec<f64> = feet
            .iter()
            .flat_map(|foot| {
                let bone = self.bones[foot.foot.bone];
                let ends = [bone.previous_a, bone.previous_b];
                let touching = self.ends_on_floor(&bone, ends);
                (0..2)
                    .filter(|&end| touching[end] || !touching.contains(&true))
                    .map(|end| ends[end].x)
                    .collect::<Vec<f64>>()
            })
            .collect();
        if !ends.is_empty() {
            ends.extend(self.step_landing(body));
        }
        let support = (
            ends.iter().copied().fold(f64::INFINITY, f64::min),
            ends.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        );
        let pressed = vec![(Vec2::default(), 0.0); feet.len()];
        // Getting up, the hips are pushed from the feet and knees under
        // them, each bearing a share by where it stands under them; the
        // hands bear the chest. A seat on the floor bears the hips as they
        // rest there, but cannot push them up.
        let shares = if rising {
            let reach = LEG_REACH * rig.height;
            let places: Vec<Option<f64>> = feet
                .iter()
                .map(|foot| {
                    let bone = self.bones[foot.foot.bone];
                    let ends = [bone.previous_a, bone.previous_b];
                    let touching = self.ends_on_floor(&bone, ends);
                    let on: Vec<f64> = (0..2)
                        .filter(|&end| touching[end])
                        .map(|end| ends[end].x)
                        .collect();
                    if on.is_empty() {
                        foot.ankle.x
                    } else {
                        on.iter().sum::<f64>() / on.len() as f64
                    }
                })
                .chain(others.iter().map(|support| match support.bears {
                    Bears::Hand(_) => f64::NAN,
                    _ => support.at.x,
                }))
                .map(|x| (!x.is_nan() && (x - hips.x).abs() < reach).then_some(x))
                .collect();
            let under: Vec<f64> = places.iter().flatten().copied().collect();
            let mut shared = weight_shares(&under, hips.x).into_iter();
            places
                .iter()
                .map(|place| place.map_or(0.0, |_| shared.next().unwrap_or(0.0)))
                .collect()
        } else {
            Vec::new()
        };
        let others_pressed = vec![(0.0, 0.0); others.len()];
        let others_count = others.len();
        Stance {
            hips,
            feet,
            above,
            capture,
            support,
            centre_x: centre.x,
            speed_x: speed.x,
            moving: if mass > 0.0 {
                (above / mass).clamp(0.1, 1.0)
            } else {
                1.0
            },
            given: (Vec2::default(), 0.0),
            pressed,
            rising,
            others,
            shares,
            others_pressed,
            chest_given: Vec2::default(),
            floor_given: vec![0.0; others_count],
        }
    }

    /// How much further across legs that carry fighter `body`'s hips shift
    /// them, to keep its capture point over its feet, inside their ends by
    /// `CAPTURE_MARGIN`, should the weight still run toward an edge.
    pub(super) fn shift_over_the_feet(&self, body: usize) -> f64 {
        let Some(stance) = self.stances.get(body).and_then(Option::as_ref) else {
            return 0.0;
        };
        // Getting up, its moves put its weight where its key poses do.
        if stance.rising {
            return 0.0;
        }
        let (back, front) = stance.support;
        if !(front > back) {
            return 0.0;
        }
        let margin = (front - back) * CAPTURE_MARGIN;
        let (lowest, highest) = (back + margin, front - margin);
        let beyond = if stance.capture > highest {
            stance.capture - highest
        } else if stance.capture < lowest {
            stance.capture - lowest
        } else {
            0.0
        };
        -beyond / stance.moving
    }

    /// Where legs that carry fighter `body`'s hips put the end of them that
    /// its move puts `from_ankles` from each ankle, over the feet standing
    /// where they are now: so that the hold and the legs' joints, aimed by
    /// the same pose, ask for the hips in the same place. Held toward where
    /// the pose stood instead, the hips sat 2 cm behind it once the feet had
    /// settled, and the hold pushed them on with up to 400 N, more than the
    /// feet's grip, until they slid and the fighter fell.
    pub(super) fn hips_over_the_feet(&self, body: usize, from_ankles: [Vec2; 2]) -> Option<Vec2> {
        let stance = self.stances.get(body)?.as_ref()?;
        if stance.feet.is_empty() || stance.rising {
            return None;
        }
        let sum = stance.feet.iter().fold(Vec2::default(), |sum, foot| {
            add(sum, add(foot.ankle, from_ankles[foot.side.index()]))
        });
        Some(scale(sum, 1.0 / stance.feet.len() as f64))
    }

    /// How far fighter `body`'s capture point lies past its feet standing on
    /// the floor, the way it faces (`facing`), in body heights `height`
    /// pixels tall: positive in front of them, negative behind, nothing
    /// within them. Past them, a person's margin of stability is gone, and
    /// only a step, or arms and trunk swung against the fall, can still
    /// catch the body (Hof, Gazendam and Sinke 2005, whose extrapolated
    /// centre of mass the capture point is).
    /// And how far the centre of mass still has to go to the capture point,
    /// the same way: where a step that catches it is measured from, as
    /// Coros, Beaudoin and van de Panne (2010) place a step from the centre
    /// of mass by the inverted pendulum.
    pub(super) fn capture_beyond(&self, body: usize, facing: f64, height: f64) -> (f64, f64) {
        let Some(stance) = self.stances.get(body).and_then(Option::as_ref) else {
            return (0.0, 0.0);
        };
        if stance.feet.is_empty() || height <= 0.0 {
            return (0.0, 0.0);
        }
        let (back, front) = stance.support;
        let past = if stance.capture > front {
            stance.capture - front
        } else if stance.capture < back {
            stance.capture - back
        } else {
            0.0
        };
        (
            past * facing / height,
            (stance.capture - stance.centre_x) * facing / height,
        )
    }

    /// The weight fighter `body`'s legs hold up at its hips standing as it
    /// does, as an impulse over a substep `h` long: all of the body but its
    /// planted legs.
    pub(super) fn weight_on_the_legs(&self, body: usize, h: f64) -> f64 {
        self.stances
            .get(body)
            .and_then(Option::as_ref)
            .map_or(0.0, |stance| stance.above * self.gravity_px() * h)
    }

    /// Fighter `body`'s foot on `side`, if it stands on the floor bearing
    /// weight: its leg not hanging limp, the foot standing, not swung by a
    /// step, and touching the floor at one end at least, and its ankle
    /// holding the shin. Counted as standing while its ball was 2 cm up and
    /// its heel as high as the pose lifts it, a ducking fighter's feet,
    /// lifted by its own folding legs, went on pushing its hips, and it
    /// floated forward half a metre on feet that touched nothing.
    fn planted_foot(&self, body: usize, rig: &fighter::Rig, side: Side) -> Option<PlantedFoot> {
        let s = side.index();
        let limb = match side {
            Side::Lead => Limb::LeftLeg,
            Side::Rear => Limb::RightLeg,
        };
        if self.limp(body, limb) {
            return None;
        }
        let foot_hold = self
            .foot_holds
            .iter()
            .position(|hold| hold.bone == rig.feet[s])?;
        let hold = self.foot_holds[foot_hold];
        if hold.swinging || hold.lifted == [true; 2] {
            return None;
        }
        let bone = self.bones.get(hold.bone)?;
        if !self
            .ends_on_floor(bone, [bone.previous_a, bone.previous_b])
            .contains(&true)
        {
            return None;
        }
        let foot = self.foot_pieces.get(foot_hold)?.clone()?;
        let thigh = self.bone_pieces.get(rig.thighs[s])?.clone()?;
        let ankle = self
            .joint_sides
            .get(rig.hinge(Hinge::Ankle(side)))?
            .as_ref()?
            .anchor();
        Some(PlantedFoot {
            side,
            foot_hold,
            load: hold.load,
            ankle,
            foot,
            thigh,
        })
    }

    /// Carries the foot `foot` that foot hold `index` holds, which a step
    /// swings, along its way over the floor, as the leg's muscles do: a
    /// spring and a damper on the foot, bone and flesh, toward where the step
    /// has its bone's middle (`FootHold::swing_to`) at the step's speed, that
    /// pushes back on the pelvis at the hip and pushes no harder than a leg
    /// swings (`SWING_FORCE`). Virtual model control places a swinging foot
    /// so (Pratt, Chew, Torres, Dilworth and Pratt 2001), and Coros, Beaudoin
    /// and van de Panne (2010) carry it along a path over the ground; it is
    /// solved as the holds are, so it never overshoots. Swung by its joints'
    /// muscles alone, which pull their hardest only a quarter of a radian off
    /// their aims, a lead foot meant to go 23 cm forward in 0.16 s slid back
    /// along the floor instead, its knee folding while its hip lagged 0.12
    /// radians behind.
    pub(super) fn carry_swinging_foot(&mut self, index: usize, foot: &BonePiece, h: f64) {
        let hold = self.foot_holds[index];
        let Some((pelvis, hip)) = self.hip_above_foot(hold.bone) else {
            self.foot_holds[index].swing_impulse = Vec2::default();
            return;
        };
        let bone = self.bones[hold.bone];
        let (at, now) = (
            lerp(bone.previous_a, bone.previous_b, 0.5),
            lerp(bone.a, bone.b, 0.5),
        );
        let speed = subtract(scale(subtract(now, at), 1.0 / h), hold.swing_speed);
        let error = subtract(at, hold.swing_to);
        let lever = subtract(at, foot.centre);
        let masses = (
            1.0 / (1.0 / foot.mass + lever.y * lever.y / foot.inertia),
            1.0 / (1.0 / foot.mass + lever.x * lever.x / foot.inertia),
        );
        let leg = SWING_MASS_KG / KG_PER_MASS;
        let omega = 2.0 * PI * SWING_HERTZ;
        let (stiffness, damping) = (leg * omega * omega, 2.0 * leg * omega);
        let given = hold.swing_impulse;
        let impulse = Vec2 {
            x: Softness::of_stiffness(stiffness, damping, masses.0, h)
                .impulse(masses.0, speed.x, error.x, given.x),
            y: Softness::of_stiffness(stiffness, damping, masses.1, h)
                .impulse(masses.1, speed.y, error.y, given.y),
        };
        let most = SWING_FORCE * self.newtons() * h;
        let total = add(given, impulse);
        let length = hypot(total.x, total.y);
        let total = if length > most {
            scale(total, most / length)
        } else {
            total
        };
        let impulse = subtract(total, given);
        self.foot_holds[index].swing_impulse = total;
        self.push_piece(foot, lever, impulse, h);
        self.push_piece(
            &pelvis,
            subtract(hip, pelvis.centre),
            scale(impulse, -1.0),
            h,
        );
    }

    /// Gives the swinging foot that foot hold `index` holds, and its
    /// pelvis, the push its leg gave it last substep, to begin this one
    /// from (`carry_swinging_foot`).
    pub(super) fn warm_start_swinging_foot(&mut self, index: usize, foot: &BonePiece, h: f64) {
        let hold = self.foot_holds[index];
        if hold.swing_impulse == Vec2::default() {
            return;
        }
        let Some((pelvis, hip)) = self.hip_above_foot(hold.bone) else {
            return;
        };
        let bone = self.bones[hold.bone];
        let at = lerp(bone.previous_a, bone.previous_b, 0.5);
        self.push_piece(foot, subtract(at, foot.centre), hold.swing_impulse, h);
        self.push_piece(
            &pelvis,
            subtract(hip, pelvis.centre),
            scale(hold.swing_impulse, -1.0),
            h,
        );
    }

    /// The pelvis, bone and flesh, of the side-view fighter whose foot's
    /// bone is `foot`, and where its hip joint on that foot's side holds the
    /// thigh, as the substep began.
    fn hip_above_foot(&self, foot: usize) -> Option<(BonePiece, Vec2)> {
        let body = self.bones.get(foot)?.body as usize;
        let rig = self.bodies.get(body)?.fighter.as_ref()?;
        let side = Side::BOTH
            .into_iter()
            .find(|side| rig.feet[side.index()] == foot)?;
        let joint = self.bone_joints.get(rig.hinge(Hinge::Hip(side)))?;
        let pelvis = self.bones.get(joint.a)?;
        let seat = joint_seat(joint, pelvis, pelvis.previous_a, pelvis.previous_b);
        let piece = self.bone_pieces.get(rig.pelvis)?.clone()?;
        Some((piece, seat))
    }

    /// Gives back on fighter `body`'s planted feet the push `push`, an
    /// impulse over the substep `h`, that its hold gave the pelvis at `at`,
    /// and the turn `turn` it gave it, so that the legs, not the window, push
    /// the hips. What the feet are given follows all
    /// the hold has given this substep, not each push as it comes: the hold
    /// gives its push a little at a time, warm started and then pass by pass,
    /// and each little push shared out on its own, as if it were all, put
    /// 64 N across on a lead foot bearing none of the weight, which crept
    /// back 11 cm while the fighter stood still.
    pub(super) fn push_back_on_the_feet(
        &mut self,
        body: usize,
        at: Vec2,
        push: Vec2,
        turn: f64,
        h: f64,
    ) {
        let Some(mut stance) = self.stances.get(body).and_then(Option::as_ref).cloned() else {
            return;
        };
        if stance.feet.is_empty() && stance.others.is_empty() {
            return;
        }
        // All the hold has given, as a push at the hips and a turn about
        // them.
        stance.given.0 = add(stance.given.0, push);
        stance.given.1 += turn + cross(subtract(at, stance.hips), push);
        let (push, turn) = stance.given;
        if stance.rising {
            self.push_back_on_supports(body, stance, h);
            return;
        }
        // The push comes back on the feet along the legs; the turn, which
        // the hips' muscles give the pelvis against the thighs, on the
        // thighs, and the legs' joints pass it on to the floor. Given back
        // on the feet instead, as a shift in where the weight pressed or as
        // pushes across the legs at the ankles, a pelvis held upright with
        // 50 N m moved the pressure 12 cm, put the weight all on one foot,
        // and spun the other off the floor.
        let (pushes, shares) = along_the_legs(&stance, push);
        let newtons = self.newtons() * h;
        let newton_metres = self.newton_metres() * h;
        for (index, foot) in stance.feet.iter().enumerate() {
            // Each leg presses its foot into the floor with the weight it
            // bears, and the floor takes the leg's push across through the
            // foot, which grips by that weight (`solve_foot_holds`,
            // `foot_bearing`), as the floor holds a planted foot still: the
            // push on the hips no harder than all the feet grip
            // (`solve_hip_hold`), it is the floor's push back, given the body
            // as residual forces on its root are (Yuan and Kitani 2020).
            // Given to the foot's bone at the ankle as well, the push across
            // went up the leg through the ankle, knee, and hip joints to the
            // pelvis and undid itself: legs pushing the hips forward with
            // 341 N had the floor push back with 81, and a fighter stepping
            // in stood still. Not pressed down either, the feet were held to
            // the floor by their own legs' weight alone, and a duck folding
            // the knees lifted both off it.
            let back = scale(pushes[index], -1.0);
            let turned = -turn * shares[index];
            // Take back what the leg was given so far, and give it what all
            // the hold has given asks of it.
            let (was_back, was_turned) = stance.pressed[index];
            if back.y != was_back.y {
                let lever = subtract(foot.ankle, foot.foot.centre);
                let pressed = Vec2 {
                    x: 0.0,
                    y: back.y - was_back.y,
                };
                self.push_piece(&foot.foot, lever, pressed, h);
            }
            if turned != was_turned {
                self.turn_piece(&foot.thigh, turned - was_turned, h);
            }
            stance.pressed[index] = (back, turned);
            if let Some(bearing) = self.foot_bearing.get_mut(foot.foot_hold) {
                *bearing = back;
            }
            if let Some(report) = self.balance_reports.get_mut(body) {
                report.feet[foot.side.index()] = scale(back, 1.0 / newtons);
            }
        }
        if let Some(report) = self.balance_reports.get_mut(body) {
            report.push = scale(push, 1.0 / newtons);
            report.turn = turn / newton_metres;
        }
        if let Some(slot) = self.stances.get_mut(body) {
            *slot = Some(stance);
        }
    }
}

impl World {
    /// What bears side-view fighter `rig` on the floor besides its feet, as
    /// the substep began, while it gets up: each knee, each fist, and its
    /// seat resting there, of a limb whose muscles hold.
    fn supports_on_floor(&self, rig: &fighter::Rig) -> Vec<Support> {
        let body = self.bones[rig.pelvis].body as usize;
        let near = |at: Vec2, reach: f64| self.floor_y - at.y < reach * rig.height;
        let piece = |bone: usize| self.bone_pieces.get(bone).cloned().flatten();
        let mut found = Vec::new();
        for side in Side::BOTH {
            let s = side.index();
            let (leg, arm) = match side {
                Side::Lead => (Limb::LeftLeg, Limb::LeftArm),
                Side::Rear => (Limb::RightLeg, Limb::RightArm),
            };
            let knee = self.bones[rig.shins[s]].previous_a;
            if !self.limp(body, leg) && near(knee, KNEE_ON_FLOOR) {
                if let Some(shin) = piece(rig.shins[s]) {
                    found.push(Support {
                        bears: Bears::Knee(side),
                        piece: shin,
                        at: knee,
                        thigh: piece(rig.thighs[s]),
                    });
                }
            }
            let knuckles = self.bones[rig.hands[s]].previous_b;
            if !self.limp(body, arm) && near(knuckles, FIST_ON_FLOOR) {
                if let Some(hand) = piece(rig.hands[s]) {
                    found.push(Support {
                        bears: Bears::Hand(side),
                        piece: hand,
                        at: knuckles,
                        thigh: None,
                    });
                }
            }
        }
        found
    }

    /// The floor holds up each knee and fist a fighter getting up rests on,
    /// bone and flesh together, wherever its bone sinks below the pressed
    /// flesh under it, and never pulls it down, as it holds a foot up on its
    /// sole (`solve_foot_holds`): pressed by the hold on the chest with
    /// nothing under its bone but flesh, a fist was crushed into the floor
    /// and a wound on it opened again.
    pub(super) fn hold_supports_up(&mut self, h: f64) {
        let (stiffness, damping) = self.foot_hold_spring();
        let most =
            SUPPORT_HOLDS * tissue::BODY_MASS_KG * self.materials.gravity * self.newtons() * h;
        for body in 0..self.stances.len() {
            let Some(height) = self
                .bodies
                .get(body)
                .and_then(|state| state.fighter.as_ref())
                .map(|rig| rig.height)
            else {
                continue;
            };
            let Some(stance) = self.stances[body].as_ref().filter(|stance| stance.rising) else {
                continue;
            };
            let supports: Vec<(Bears, BonePiece)> = stance
                .others
                .iter()
                .map(|support| (support.bears, support.piece.clone()))
                .collect();
            for (index, (bears, piece)) in supports.into_iter().enumerate() {
                let bone = self.bones[piece.bone];
                let (place, previous, pad) = match bears {
                    Bears::Knee(_) => (bone.a, bone.previous_a, KNEE_PAD),
                    Bears::Hand(_) => (bone.b, bone.previous_b, FIST_PAD),
                };
                let level = self.floor_y - pad * height;
                let lever = subtract(previous, piece.centre);
                let mass = 1.0 / (1.0 / piece.mass + lever.x * lever.x / piece.inertia);
                let speed = (place.y - previous.y) / h;
                let given = self.stances[body]
                    .as_ref()
                    .map_or(0.0, |stance| stance.floor_given[index]);
                let impulse = Softness::of_stiffness(stiffness, damping, mass, h).impulse(
                    mass,
                    speed,
                    previous.y - level,
                    given,
                );
                let total = (given + impulse).clamp(-most, 0.0);
                if let Some(stance) = self.stances[body].as_mut() {
                    stance.floor_given[index] = total;
                }
                self.push_piece(
                    &piece,
                    lever,
                    Vec2 {
                        x: 0.0,
                        y: total - given,
                    },
                    h,
                );
            }
        }
    }

    /// Gives back on what bears fighter `body` on the floor as it gets up
    /// all its hold has given its hips this substep (`stance.given`): the
    /// push up shared by where each stands under the centre of mass
    /// (`Stance::shares`), each pressed into the floor where it rests, the
    /// feet at their ankles; and the turn on the pelvis on the thighs of the
    /// legs bearing it, by their shares. The floor holds the feet across by
    /// what they bear (`solve_foot_holds`).
    fn push_back_on_supports(&mut self, body: usize, mut stance: Stance, h: f64) {
        let (push, turn) = stance.given;
        let newtons = self.newtons() * h;
        let feet = stance.feet.len();
        let legs: f64 = stance.shares[..feet].iter().sum::<f64>()
            + stance
                .others
                .iter()
                .zip(&stance.shares[feet..])
                .filter(|(support, _)| support.thigh.is_some())
                .map(|(_, share)| share)
                .sum::<f64>();
        let turned_by = |share: f64| {
            if legs > EPSILON {
                -turn * share / legs
            } else {
                0.0
            }
        };
        for (index, foot) in stance.feet.iter().enumerate() {
            let share = stance.shares[index];
            let back = Vec2 {
                x: 0.0,
                y: -push.y * share,
            };
            let turned = turned_by(share);
            let (was_back, was_turned) = stance.pressed[index];
            if back.y != was_back.y {
                let lever = subtract(foot.ankle, foot.foot.centre);
                let pressed = Vec2 {
                    x: 0.0,
                    y: back.y - was_back.y,
                };
                self.push_piece(&foot.foot, lever, pressed, h);
            }
            if turned != was_turned {
                self.turn_piece(&foot.thigh, turned - was_turned, h);
            }
            stance.pressed[index] = (back, turned);
            if let Some(bearing) = self.foot_bearing.get_mut(foot.foot_hold) {
                *bearing = back;
            }
            if let Some(report) = self.balance_reports.get_mut(body) {
                report.feet[foot.side.index()] = scale(back, 1.0 / newtons);
            }
        }
        for (index, support) in stance.others.iter().enumerate() {
            let share = stance.shares[feet + index];
            let back = -push.y * share;
            let turned = if support.thigh.is_some() {
                turned_by(share)
            } else {
                0.0
            };
            let (was_back, was_turned) = stance.others_pressed[index];
            if back != was_back {
                let lever = subtract(support.at, support.piece.centre);
                let pressed = Vec2 {
                    x: 0.0,
                    y: back - was_back,
                };
                self.push_piece(&support.piece, lever, pressed, h);
            }
            if let Some(thigh) = support.thigh.as_ref().filter(|_| turned != was_turned) {
                self.turn_piece(thigh, turned - was_turned, h);
            }
            stance.others_pressed[index] = (back, turned);
        }
        let newton_metres = self.newton_metres() * h;
        if let Some(report) = self.balance_reports.get_mut(body) {
            report.push = scale(push, 1.0 / newtons);
            report.turn = turn / newton_metres;
        }
        if let Some(slot) = self.stances.get_mut(body) {
            *slot = Some(stance);
        }
    }

    /// Whether the turn fighter `body`'s hold gives its pelvis comes back on
    /// a leg bearing it: getting up on its hands or its seat alone, nothing
    /// would take it back, and its pelvis is left to them.
    pub(super) fn legs_take_the_turn(&self, body: usize) -> bool {
        self.stances
            .get(body)
            .and_then(Option::as_ref)
            .is_some_and(|stance| {
                !stance.rising
                    || stance.shares[..stance.feet.len()]
                        .iter()
                        .any(|&share| share > 0.0)
                    || stance
                        .others
                        .iter()
                        .zip(&stance.shares[stance.feet.len()..])
                        .any(|(support, &share)| support.thigh.is_some() && share > 0.0)
            })
    }
}

/// How a body whose centre of mass stands at `centre`, across the floor, is
/// borne by things resting on the floor at `places`, as shares of its
/// weight: as evenly as they can bear it while their moments about the
/// centre balance, the least-squares answer to the two sums, none bearing
/// less than nothing; a centre past the outermost leaves it all on that
/// one, as a body tips over its edge.
pub(super) fn weight_shares(places: &[f64], centre: f64) -> Vec<f64> {
    let mut bearing = vec![true; places.len()];
    loop {
        let on: Vec<usize> = (0..places.len()).filter(|&index| bearing[index]).collect();
        if on.is_empty() {
            return vec![0.0; places.len()];
        }
        let count = on.len() as f64;
        let mean = on.iter().map(|&index| places[index]).sum::<f64>() / count;
        let spread: f64 = on
            .iter()
            .map(|&index| (places[index] - mean) * (places[index] - mean))
            .sum();
        let mut shares = vec![0.0; places.len()];
        for &index in &on {
            shares[index] = 1.0 / count
                + if spread > 1.0e-9 {
                    (places[index] - mean) * (centre - mean) / spread
                } else {
                    0.0
                };
        }
        match on
            .iter()
            .copied()
            .filter(|&index| shares[index] < 0.0)
            .min_by(|&a, &b| shares[a].total_cmp(&shares[b]))
        {
            Some(least) => bearing[least] = false,
            None => return shares,
        }
    }
}

/// How the legs standing on `stance`'s feet share `push` on the hips: each
/// pushes along the line from its ankle to the hips, as legs standing apart
/// do, so that each foot takes its share at the ankle, inside its heel and
/// its ball; and each leg's share of the weight. A leg only pushes on the
/// floor, never pulls it: where the push across would take one to pull, the
/// legs give as much of it as they can with both pushing, and each takes
/// what is left as its share of the weight says. For each leg: the push it
/// gives the hips; and its share.
fn along_the_legs(stance: &Stance, push: Vec2) -> (Vec<Vec2>, Vec<f64>) {
    let hips = stance.hips;
    let mut pushes: Vec<Vec2> = match stance.feet.as_slice() {
        [first, second] => {
            let line = |foot: &PlantedFoot| {
                normalized(subtract(hips, foot.ankle), Vec2 { x: 0.0, y: -1.0 })
            };
            let (first_up, second_up) = (line(first), line(second));
            let apart = cross(first_up, second_up);
            let along = |wanted: Vec2| -> [f64; 2] {
                if apart.abs() > 1.0e-6 {
                    [
                        cross(wanted, second_up) / apart,
                        cross(first_up, wanted) / apart,
                    ]
                } else {
                    [dot(wanted, first_up) * 0.5, dot(wanted, second_up) * 0.5]
                }
            };
            let upright = along(Vec2 { x: 0.0, y: push.y });
            let all = along(push);
            // Each leg keeps at least a share of the weight it bears standing
            // still: a boxer keeps weight on both feet. Pushed on across until
            // the lead foot bore none of it, the foot had no grip, the leg
            // dragged it back, the hips followed it, and the stance shrank by
            // 30 cm while the fighter stood still.
            let least = upright.map(|along| along * LEAST_LOAD);
            let pushing = |along: [f64; 2]| (0..2).all(|k| along[k] >= least[k]);
            if pushing(all) {
                vec![scale(first_up, all[0]), scale(second_up, all[1])]
            } else if upright.iter().all(|&along| along >= 0.0) {
                // As much of the push across as keeps both legs pushing.
                let most = (0..2).fold(1.0_f64, |most, k| {
                    if all[k] < least[k] && upright[k] > all[k] {
                        most.min((upright[k] - least[k]) / (upright[k] - all[k]))
                    } else {
                        most
                    }
                });
                let given = [0, 1].map(|k| upright[k] + (all[k] - upright[k]) * most);
                vec![scale(first_up, given[0]), scale(second_up, given[1])]
            } else {
                // The legs cannot share even the weight both pushing: the
                // one standing nearer under the hips takes it all.
                let mut pushes = vec![Vec2::default(); 2];
                let nearer =
                    usize::from((second.ankle.x - hips.x).abs() < (first.ankle.x - hips.x).abs());
                pushes[nearer] = push;
                pushes
            }
        }
        _ => vec![push; stance.feet.len()],
    };
    // A foot handing its weight to the other before it lifts bears as much
    // of its share as it has left, the other foot the rest, as a robot
    // brings a foot's push on the floor to nothing before letting go of it
    // (Herzog, Righetti, Grimminger, Pastor and Schaal 2014). Let go of
    // bearing its share at once, a lead foot was kicked back 3 cm in a step
    // by its own leg, which the push on the foot had stretched.
    if let [first, second] = stance.feet.as_slice() {
        let keep = first.load.min(second.load).clamp(0.0, 1.0);
        if keep < 1.0 {
            let other = usize::from(first.load <= second.load);
            for (index, leg) in pushes.iter_mut().enumerate() {
                let alone = if index == other {
                    push
                } else {
                    Vec2::default()
                };
                *leg = add(scale(*leg, keep), scale(alone, 1.0 - keep));
            }
        }
    }
    let bearing: Vec<f64> = pushes.iter().map(|push| (-push.y).max(0.0)).collect();
    let borne: f64 = bearing.iter().sum();
    let shares: Vec<f64> = if borne > EPSILON {
        bearing.iter().map(|bears| bears / borne).collect()
    } else {
        vec![1.0 / stance.feet.len().max(1) as f64; stance.feet.len()]
    };
    let left = pushes.iter().fold(push, |left, &leg| subtract(left, leg));
    let pushes = pushes
        .iter()
        .zip(&shares)
        .map(|(&leg, &share)| add(leg, scale(left, share)))
        .collect();
    (pushes, shares)
}
