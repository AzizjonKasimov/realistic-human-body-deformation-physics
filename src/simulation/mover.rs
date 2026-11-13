//! How a side-view fighter moves. A move is not code in the solver: it is a
//! pose the body is asked to take, a handful of numbers that change from step
//! to step (`profile::Pose`: where the hips are, how the trunk leans, where
//! the feet and the fists are). From the pose, [`World::drive_movers`] works
//! out where every joint should be turned (`profile::Joints`), and the body's
//! own muscles turn them there (`JointDrive`), as strongly as a person's and
//! no more; the hold on the hips carries them where the pose puts them
//! (`Carry`), and a stepping foot is swung through the air by its leg and set
//! down. So a new move is a few lines that change the pose, and what the
//! body does on the way, how fast a fist gets, what a blow in the middle of
//! a move does to it, is still the simulation's.
//!
//! The poses here are set by hand from how boxers move: the stance bounces
//! and sways; a step moves the leading foot first and the other after it; a
//! jab drives the lead fist straight at where the other fighter's head is as
//! it sets off, the lead shoulder going forward with it; a cross turns the
//! hips and shoulders through, the rear shoulder coming forward and the rear
//! heel off the floor; leaning back takes the head out of reach over the
//! back leg; a duck bends the knees and folds the trunk under a punch. The
//! hand's way out and back is the smoothest there is (Flash and Hogan 1985),
//! as `punch.rs`'s is.

use super::punch::{smooth, FIST_FRONT, LANDING_SPEED};
use super::tools::{FistContact, FistLanding};
use super::*;
use crate::profile::{self, Hinge, Joints, Place, Pose, Side};

/// A move a side-view fighter can be told to make ([`World::command`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    /// A straight punch with the lead hand, the one nearer the other
    /// fighter.
    Jab,
    /// A straight punch with the rear hand, the hips and shoulders turning
    /// through.
    Cross,
    /// A step toward the other fighter, and one away.
    StepIn,
    StepOut,
    /// Leaning back out of reach, for as long as it is held (`true`) until
    /// let go (`false`).
    LeanBack(bool),
    /// Ducking under a punch, for as long as it is held.
    Duck(bool),
}

/// How often the stance bounces, in hertz: a boxer on the balls of the feet
/// bounces about twice a second.
const BOUNCE_HERTZ: f64 = 2.1;
/// How far the hips rise and fall with the bounce, and sway forward and
/// back with every other bounce, in body heights.
const BOUNCE_RISE: f64 = 0.004;
const BOUNCE_SWAY: f64 = 0.004;

/// How far a step carries the fighter, in body heights (about 23 cm), and
/// how long it takes, in seconds: as fast as feet gripping the floor can
/// push a body off and stop it again.
const STEP_LENGTH: f64 = 0.13;
const STEP_SECONDS: f64 = 0.42;
/// How high a stepping foot lifts, in body heights: a boxer's feet skim the
/// floor.
const STEP_LIFT: f64 = 0.020;

/// A punch's timing, in seconds: how long the fist takes out, how long it
/// stays there, and how long it takes back. A jab is out in about a tenth
/// of a second and a cross a little longer (the hand of an untrained man's
/// straight punch takes 0.08-0.15 s, `docs/INJURY_REFERENCE.md`, Punches).
const JAB_SECONDS: (f64, f64, f64) = (0.10, 0.03, 0.22);
const CROSS_SECONDS: (f64, f64, f64) = (0.13, 0.04, 0.30);
/// How much of the arm's full length a punch reaches at most: the elbow
/// stays a little bent, as a boxer keeps it.
const STRIKE_REACH: f64 = 0.975;

/// How firmly a fighter's hips are carried across and upward, in hertz on
/// the body's weight, and the hardest the hold pushes each end of the pelvis
/// each way, in body weights: half of what feet gripping a dry floor can
/// push sideways with (a friction coefficient near 1 on the whole body's
/// weight), and half of the two and a half body weights legs bear.
const CARRY_HERTZ: Vec2 = Vec2 { x: 2.0, y: 6.0 };
const CARRY_LIMIT: Vec2 = Vec2 { x: 0.5, y: 1.25 };

/// A move held or let go, and how far into it the body is, eased in and
/// out: from 0 in the stance to 1 in the move's pose.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Held {
    held: bool,
    amount: f64,
    speed: f64,
}

impl Held {
    /// Moves `amount` toward 1 while held and 0 once let go, as a critically
    /// damped spring settles, `quickness` hertz fast.
    fn ease(&mut self, quickness: f64, dt: f64) {
        let omega = 2.0 * PI * quickness;
        let target = if self.held { 1.0 } else { 0.0 };
        let pull = omega * omega * (target - self.amount) - 2.0 * omega * self.speed;
        self.speed += pull * dt;
        self.amount += self.speed * dt;
        if !self.held && self.amount.abs() < 1.0e-4 && self.speed.abs() < 1.0e-3 {
            self.amount = 0.0;
            self.speed = 0.0;
        }
    }
}

/// A step being taken: how far it carries the fighter, in body heights
/// (forward positive), and how long ago it began.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Step {
    by: f64,
    age: f64,
}

impl Step {
    /// How far through its swing each foot is, from 0 on the floor where it
    /// stood to 1 on the floor where it lands, the lead foot's first, and
    /// how far the hips have gone. The foot on the side the step goes to
    /// moves first, and the other follows it.
    fn progress(&self) -> ([f64; 2], f64) {
        let t = self.age / STEP_SECONDS;
        let first = smooth(t / 0.55);
        let second = smooth((t - 0.45) / 0.55);
        let hips = smooth((t - 0.10) / 0.85);
        if self.by > 0.0 {
            ([first, second], hips)
        } else {
            ([second, first], hips)
        }
    }

    fn done(&self) -> bool {
        self.age >= STEP_SECONDS
    }
}

/// A punch being thrown.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Strike {
    hand: Side,
    age: f64,
    /// Where the knuckles aim, in the fighter's own frame: where the other
    /// fighter's head was as the punch set off.
    target: Place,
    /// When its fist met something on the way out, and how far out it had
    /// got: it comes back from there.
    turned: Option<(f64, f64)>,
}

impl Strike {
    fn seconds(&self) -> (f64, f64, f64) {
        match self.hand {
            Side::Lead => JAB_SECONDS,
            Side::Rear => CROSS_SECONDS,
        }
    }

    /// How far out the fist aims, from 0 in the guard to 1 at its target.
    fn reach(&self) -> f64 {
        let (out, stay, back) = self.seconds();
        match self.turned {
            Some((age, reach)) => reach * (1.0 - smooth((self.age - age) / back)),
            None => smooth(self.age / out) - smooth((self.age - out - stay) / back),
        }
    }

    /// Still on its way out, not yet coming back.
    fn going_out(&self) -> bool {
        self.turned.is_none() && self.age < self.seconds().0
    }

    fn done(&self) -> bool {
        let (out, stay, back) = self.seconds();
        match self.turned {
            Some((age, _)) => self.age >= age + back,
            None => self.age >= out + stay + back,
        }
    }

    /// The fist met something: the punch turns back from where it got.
    fn turn_back(&mut self) {
        if self.going_out() {
            self.turned = Some((self.age, self.reach()));
        }
    }
}

/// What moves a side-view fighter: where it stands, what it is doing, and
/// what its joints were last asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Mover {
    /// Seconds since it was built, which its bounce keeps time by.
    clock: f64,
    /// How far forward of where it was built it stands, in body heights.
    place: f64,
    /// Where the ball of each foot stands, forward of where the fighter was
    /// built, in body heights, the lead foot's first.
    feet: [f64; 2],
    step: Option<Step>,
    strike: Option<Strike>,
    lean: Held,
    duck: Held,
    /// How far each joint was turned from the stance on the last step, in
    /// the order of [`Hinge::ALL`], and where each end of the hips was put,
    /// from which the aims' speeds follow.
    last_turns: Option<[f64; 16]>,
    last_hips: Option<[Vec2; 2]>,
}

impl Mover {
    pub(super) fn new() -> Self {
        let stance = profile::stance();
        Self {
            clock: 0.0,
            place: 0.0,
            feet: [stance.feet[0].ball.0, stance.feet[1].ball.0],
            step: None,
            strike: None,
            lean: Held::default(),
            duck: Held::default(),
            last_turns: None,
            last_hips: None,
        }
    }

    /// The pose the fighter is asked to take now.
    fn pose(&self) -> Pose {
        let mut pose = profile::stance();
        let turn = 2.0 * PI * BOUNCE_HERTZ * self.clock;
        let (bounce, sway) = (libm::sin(turn), libm::sin(turn * 0.5));
        // The stance, alive: a small bounce and a slower sway.
        pose.hips.0 += self.place + BOUNCE_SWAY * sway;
        pose.hips.1 += BOUNCE_RISE * bounce;
        pose.lumbar += 0.004 * sway;
        pose.thorax += 0.008 * sway;
        pose.feet[1].pitch += 0.04 * bounce;
        for side in 0..2 {
            pose.feet[side].ball.0 = self.feet[side];
        }
        // A step: each foot swings to where it lands, and the hips go with
        // them.
        if let Some(step) = &self.step {
            let (feet, hips) = step.progress();
            for side in 0..2 {
                pose.feet[side].ball.0 += step.by * feet[side];
                pose.feet[side].ball.1 += STEP_LIFT * libm::sin(PI * feet[side]);
            }
            pose.hips.0 += step.by * hips;
        }
        // Leaning back: the weight goes over the back leg and the trunk
        // away from the punch, the chin tucked.
        let lean = self.lean.amount;
        pose.hips.0 -= 0.070 * lean;
        pose.hips.1 -= 0.010 * lean;
        pose.pelvis -= 0.10 * lean;
        pose.lumbar -= 0.20 * lean;
        pose.thorax -= 0.30 * lean;
        pose.neck -= 0.26 * lean;
        pose.head -= 0.10 * lean;
        // Ducking: the knees bend and the trunk folds forward under the
        // punch, the eyes still on the other fighter.
        let duck = self.duck.amount;
        pose.hips.0 += 0.010 * duck;
        pose.hips.1 -= 0.100 * duck;
        pose.pelvis += 0.16 * duck;
        pose.lumbar += 0.28 * duck;
        pose.thorax += 0.40 * duck;
        pose.neck += 0.26 * duck;
        pose.head += 0.02 * duck;
        // A punch turns the trunk into it before the fists are placed, so
        // the guard rides on the trunk as it turns.
        let reach = self.strike.map_or(0.0, |strike| strike.reach());
        if let Some(strike) = &self.strike {
            match strike.hand {
                Side::Lead => {
                    pose.shoulders[0] += 0.045 * reach;
                    pose.shoulders[1] -= 0.020 * reach;
                    pose.thorax += 0.05 * reach;
                    pose.hips.0 += 0.015 * reach;
                }
                Side::Rear => {
                    pose.shoulders[1] += 0.115 * reach;
                    pose.shoulders[0] -= 0.055 * reach;
                    pose.lumbar += 0.03 * reach;
                    pose.thorax += 0.08 * reach;
                    pose.hips.0 += 0.045 * reach;
                    pose.hips.1 -= 0.010 * reach;
                    pose.feet[1].pitch += 0.38 * reach;
                }
            }
        }
        // The fists stand by the head, wherever the trunk has taken it.
        let joints = Joints::of(&pose);
        pose.fists = profile::guard_fists(joints.head());
        if let Some(strike) = &self.strike {
            let side = strike.hand.index();
            // No farther than the arm reaches with the elbow a little bent.
            let shoulder = joints.shoulder[side];
            let line = (strike.target.0 - shoulder.0, strike.target.1 - shoulder.1);
            let far = libm::hypot(line.0, line.1).max(1.0e-9);
            let most = STRIKE_REACH * (profile::UPPER_ARM + profile::FOREARM + profile::HAND);
            let short = (most / far).min(1.0);
            let target = (shoulder.0 + line.0 * short, shoulder.1 + line.1 * short);
            let guard = pose.fists[side];
            pose.fists[side] = (
                guard.0 + (target.0 - guard.0) * reach,
                guard.1 + (target.1 - guard.1) * reach,
            );
            if strike.hand == Side::Rear {
                // The lead hand comes back to the chin as the rear goes out.
                pose.fists[0].0 -= 0.050 * reach;
                pose.fists[0].1 -= 0.008 * reach;
            }
        }
        pose
    }
}

/// A fighter's own frame in the world: where on the floor it was built to
/// stand, how tall it is, and which way it faces (1 the window's right, -1
/// its left).
#[derive(Clone, Copy)]
struct Frame {
    stand: Vec2,
    height: f64,
    facing: f64,
}

impl Frame {
    fn world(&self, place: Place) -> Vec2 {
        Vec2 {
            x: self.stand.x + self.facing * place.0 * self.height,
            y: self.stand.y - place.1 * self.height,
        }
    }

    fn own(&self, at: Vec2) -> Place {
        (
            (at.x - self.stand.x) * self.facing / self.height,
            (self.stand.y - at.y) / self.height,
        )
    }
}

/// The side whose arm `limb` is, for a fighter's arms.
fn side_of(limb: Limb) -> Option<Side> {
    match limb {
        Limb::LeftArm => Some(Side::Lead),
        Limb::RightArm => Some(Side::Rear),
        _ => None,
    }
}

impl World {
    /// Has the hold on side-view fighter `body`'s hips carry them as its
    /// moves will (`Carry`), from where they stand: set before the body
    /// settles, so that it learns its weight on the hold that will carry
    /// it.
    pub(super) fn carry_fighter_hips(&mut self, body: usize) {
        let Some(rig) = self
            .bodies
            .get(body)
            .and_then(|state| state.fighter.clone())
        else {
            return;
        };
        for hold in &mut self.hip_holds {
            if hold.bone == rig.pelvis {
                hold.carry = Some(Carry {
                    offset: Vec2::default(),
                    speed: Vec2::default(),
                    hertz: CARRY_HERTZ,
                    limit: scale(CARRY_LIMIT, tissue::BODY_MASS_KG * STANDARD_GRAVITY),
                });
            }
        }
    }

    /// Gives body `body` what moves a side-view fighter, standing in its
    /// stance.
    pub(super) fn give_mover(&mut self, body: usize) {
        if self
            .bodies
            .get(body)
            .is_some_and(|state| state.fighter.is_some())
        {
            self.bodies[body].mover = Some(Mover::new());
        }
    }

    /// Whether body `body` is a side-view fighter that moves.
    pub fn is_fighter(&self, body: usize) -> bool {
        self.bodies
            .get(body)
            .is_some_and(|state| state.fighter.is_some() && state.mover.is_some())
    }

    /// Body `body`'s own frame, if it is a side-view fighter.
    fn fighter_frame(&self, body: usize) -> Option<Frame> {
        let state = self.bodies.get(body)?;
        let rig = state.fighter.as_ref()?;
        Some(Frame {
            stand: rig.stand,
            height: rig.height,
            facing: if state.mirrored { -1.0 } else { 1.0 },
        })
    }

    /// Where the middle of the head of the body nearest body `body` is.
    fn opponents_head(&self, body: usize) -> Option<Vec2> {
        let own = self.midline_of(body)?;
        let skull = |bone: &BoneSegment| {
            bone.part == BonePart::Skull && bone.body as usize != body && !bone.splinter
        };
        self.bones
            .iter()
            .filter(|bone| skull(bone))
            .map(|bone| lerp(bone.a, bone.b, 0.45))
            .min_by(|p, q| (p.x - own).abs().total_cmp(&(q.x - own).abs()))
    }

    /// Tells body `body`, a side-view fighter, to make a move: it throws a
    /// punch unless one is still on its way or the arm hangs limp, takes a
    /// step unless it is in the middle of one, and leans back or ducks for
    /// as long as told to. Other bodies take no notice.
    pub fn command(&mut self, body: usize, made: Move) {
        let Some(frame) = self.fighter_frame(body) else {
            return;
        };
        let head = self.opponents_head(body).map(|head| frame.own(head));
        let limp = |world: &World, side: Side| {
            world.limp(
                body,
                match side {
                    Side::Lead => Limb::LeftArm,
                    Side::Rear => Limb::RightArm,
                },
            )
        };
        let throws = |world: &World, side: Side| {
            !limp(world, side)
                && world.bodies[body]
                    .mover
                    .as_ref()
                    .is_some_and(|mover| mover.strike.is_none())
        };
        let lead_free = throws(self, Side::Lead);
        let rear_free = throws(self, Side::Rear);
        let Some(mover) = self.bodies[body].mover.as_mut() else {
            return;
        };
        let strike = |mover: &Mover, hand: Side| {
            // At the other fighter's head, or straight ahead at the height
            // of its own.
            let target = head.unwrap_or_else(|| {
                let own = Joints::of(&mover.pose()).head();
                (own.0 + 0.45, own.1 - 0.02)
            });
            Strike {
                hand,
                age: 0.0,
                target,
                turned: None,
            }
        };
        match made {
            Move::Jab if lead_free => mover.strike = Some(strike(mover, Side::Lead)),
            Move::Cross if rear_free => mover.strike = Some(strike(mover, Side::Rear)),
            Move::StepIn | Move::StepOut if mover.step.is_none() => {
                mover.step = Some(Step {
                    by: if made == Move::StepIn {
                        STEP_LENGTH
                    } else {
                        -STEP_LENGTH
                    },
                    age: 0.0,
                });
            }
            Move::LeanBack(held) => mover.lean.held = held,
            Move::Duck(held) => mover.duck.held = held,
            _ => {}
        }
    }

    /// Moves every side-view fighter on by a step `dt` long: its moves go
    /// on, the pose they ask for is worked out, and its joints, hips, and
    /// feet are aimed there.
    pub(super) fn drive_movers(&mut self, dt: f64) {
        for body in 0..self.bodies.len() {
            let (Some(frame), Some(rig)) =
                (self.fighter_frame(body), self.bodies[body].fighter.clone())
            else {
                continue;
            };
            let Some(mut mover) = self.bodies[body].mover.take() else {
                continue;
            };
            mover.clock += dt;
            mover.lean.ease(2.4, dt);
            mover.duck.ease(2.4, dt);
            // A foot swings while its part of a step lasts, and is set down
            // as it ends.
            let mut swinging = [false; 2];
            if let Some(step) = &mut mover.step {
                step.age += dt;
                let (feet, _) = step.progress();
                swinging = feet.map(|progress| progress > 0.0 && progress < 1.0);
                if step.done() {
                    let by = step.by;
                    mover.place += by;
                    for foot in &mut mover.feet {
                        *foot += by;
                    }
                    mover.step = None;
                    swinging = [false; 2];
                }
            }
            if let Some(strike) = &mut mover.strike {
                strike.age += dt;
                let arm = match strike.hand {
                    Side::Lead => Limb::LeftArm,
                    Side::Rear => Limb::RightArm,
                };
                if strike.done() || self.limp(body, arm) {
                    mover.strike = None;
                }
            }

            let joints = Joints::of(&mover.pose());
            self.aim_joints(body, &rig, &frame, &mut mover, &joints, dt);
            self.carry_hips(&rig, &frame, &mut mover, &joints, dt);
            self.place_feet(&rig, &frame, &joints, swinging);
            self.bodies[body].mover = Some(mover);
        }
    }

    /// Aims each of the fighter's joints where `joints` has it turned, and
    /// slides its shoulders across its chest.
    fn aim_joints(
        &mut self,
        body: usize,
        rig: &fighter::Rig,
        frame: &Frame,
        mover: &mut Mover,
        joints: &Joints,
        dt: f64,
    ) {
        let asked = joints.directions();
        let built = Joints::of(&profile::stance());
        let stood = built.directions();
        let mut turns = [0.0; 16];
        for (place, &hinge) in Hinge::ALL.iter().enumerate() {
            // The window's angles run the other way from the figure's for
            // a fighter facing right, and the same way for its mirror image.
            let turn = wrap_angle(asked.turn(hinge) - stood.turn(hinge));
            turns[place] = turn;
            let was = mover.last_turns.map_or(turn, |last| last[place]);
            let limb = match hinge {
                Hinge::Shoulder(side) | Hinge::Elbow(side) | Hinge::Wrist(side) => match side {
                    Side::Lead => Limb::LeftArm,
                    Side::Rear => Limb::RightArm,
                },
                Hinge::Hip(side) | Hinge::Knee(side) | Hinge::Ankle(side) => match side {
                    Side::Lead => Limb::LeftLeg,
                    Side::Rear => Limb::RightLeg,
                },
                _ => Limb::None,
            };
            if self.limp(body, limb) {
                continue;
            }
            if let Some(joint) = self.bone_joints.get_mut(rig.hinges[place]) {
                joint.drive.swing = -frame.facing * turn;
                joint.drive.swing_speed = -frame.facing * wrap_angle(turn - was) / dt.max(EPSILON);
            }
        }
        mover.last_turns = Some(turns);
        // A shoulder slides across the chest from where it was built.
        let stance = profile::stance();
        for side in Side::BOTH {
            let s = side.index();
            let slid = (joints.shoulder[s].0 - joints.square_shoulder().0)
                * joints.chest_forward().0
                + (joints.shoulder[s].1 - joints.square_shoulder().1) * joints.chest_forward().1;
            let by = (slid - stance.shoulders[s]) * frame.height;
            if let Some(joint) = self.bone_joints.get_mut(rig.hinge(Hinge::Shoulder(side))) {
                // It slides there over the step, substep by substep.
                let seat = add(rig.shoulder_seats[s], scale(rig.shoulder_slide, by));
                joint.seat_speed = scale(subtract(seat, joint.seat), 1.0 / dt.max(EPSILON));
            }
        }
    }

    /// Has the hold on the fighter's hips carry each end of its pelvis to
    /// where `joints` puts it.
    fn carry_hips(
        &mut self,
        rig: &fighter::Rig,
        frame: &Frame,
        mover: &mut Mover,
        joints: &Joints,
        dt: f64,
    ) {
        let gap = self.materials.point_spacing * 0.70;
        let (hips, sacrum) = (frame.world(joints.hips), frame.world(joints.sacrum));
        // The pelvis's bone runs from a joint's rest gap short of the foot
        // of the spine down to the hips (`fighter.rs`).
        let up = normalized(subtract(sacrum, hips), Vec2 { x: 0.0, y: -1.0 });
        let ends = [subtract(sacrum, scale(up, gap)), hips];
        let was = mover.last_hips.unwrap_or(ends);
        for hold in &mut self.hip_holds {
            if hold.bone != rig.pelvis {
                continue;
            }
            let end = usize::from(hold.end_b);
            if let Some(carry) = &mut hold.carry {
                carry.offset = subtract(ends[end], hold.home);
                carry.speed = scale(subtract(ends[end], was[end]), 1.0 / dt.max(EPSILON));
            }
        }
        mover.last_hips = Some(ends);
    }

    /// Stands each of the fighter's feet where `joints` puts it: a foot a
    /// step is swinging is held by nothing, and one on the floor stands
    /// with its ball where it stood and its heel as high as the pose lifts
    /// it.
    fn place_feet(
        &mut self,
        rig: &fighter::Rig,
        frame: &Frame,
        joints: &Joints,
        swinging: [bool; 2],
    ) {
        for side in 0..2 {
            let (heel, ball) = (
                frame.world(joints.heel[side]),
                frame.world(joints.ball[side]),
            );
            for hold in &mut self.foot_holds {
                if hold.bone != rig.feet[side] {
                    continue;
                }
                if swinging[side] {
                    hold.swinging = true;
                    continue;
                }
                if hold.swinging {
                    // Set down where the step ends.
                    hold.swinging = false;
                    hold.planted = [heel, ball];
                    hold.lifted = [false; 2];
                    continue;
                }
                // The ball stays where the floor holds it; the heel rises
                // and falls about it as the pose pitches the foot.
                hold.planted[1].y = ball.y;
                hold.planted[0] = add(hold.planted[1], subtract(heel, ball));
            }
        }
    }

    /// Whether body `body`'s arm `limb` is throwing a fighter's punch still
    /// on its way out.
    pub(super) fn striking_out(&self, body: usize, limb: Limb) -> bool {
        let Some(side) = side_of(limb) else {
            return false;
        };
        self.bodies
            .get(body)
            .and_then(|state| state.mover.as_ref())
            .and_then(|mover| mover.strike.as_ref())
            .is_some_and(|strike| strike.hand == side && strike.going_out())
    }

    /// Whether body `body`'s arm `limb` is throwing a fighter's punch, out
    /// or back.
    pub(super) fn striking(&self, body: usize, limb: Limb) -> bool {
        let Some(side) = side_of(limb) else {
            return false;
        };
        self.bodies
            .get(body)
            .and_then(|state| state.mover.as_ref())
            .and_then(|mover| mover.strike.as_ref())
            .is_some_and(|strike| strike.hand == side)
    }

    /// Body `body`'s punch with `limb`, a fighter's, turns back from where
    /// it got: its fist landed, or its arm was pressed too hard to go on
    /// (`clash.rs`).
    pub(super) fn turn_strike_back(&mut self, body: usize, limb: Limb) {
        let Some(side) = side_of(limb) else {
            return;
        };
        if let Some(strike) = self
            .bodies
            .get_mut(body)
            .and_then(|state| state.mover.as_mut())
            .and_then(|mover| mover.strike.as_mut())
            .filter(|strike| strike.hand == side)
        {
            strike.turn_back();
        }
    }

    /// The body whose punch is nearest landing, if a fighter is throwing
    /// one: its fist is drawn over the body it reaches for.
    pub fn striking_body(&self) -> Option<usize> {
        (0..self.bodies.len())
            .filter_map(|body| {
                let strike = self.bodies[body].mover.as_ref()?.strike.as_ref()?;
                Some((strike.age, body))
            })
            .min_by(|p, q| p.0.total_cmp(&q.0))
            .map(|(_, body)| body)
    }

    /// Each fighter's fist on its way out lands on any other body in its
    /// way as the bare fist does (`World::land_fist`), as a front-view
    /// body's punch does (`World::land_punches`): what it gives that body
    /// its arm loses, the force it lands with presses its own hand, and it
    /// turns back. Every fist's sweep is worked out before any lands, so
    /// that no fist lands first.
    pub(super) fn land_strikes(&mut self, dt: f64) {
        if self.bodies.len() < 2 {
            return;
        }
        let ahead = FIST_FRONT * self.body_height - tool_geometry(ToolMode::Fist).contact_radius;
        let sweeps: Vec<(usize, Side, usize, Vec2, Vec2, Vec2)> = (0..self.bodies.len())
            .filter_map(|body| {
                let state = &self.bodies[body];
                let (rig, mover) = (state.fighter.as_ref()?, state.mover.as_ref()?);
                let strike = mover.strike.as_ref().filter(|strike| strike.going_out())?;
                let index = rig.hands[strike.hand.index()];
                let hand = self.bones.get(index).filter(|hand| !hand.splinter)?;
                let facing = Vec2 {
                    x: if state.mirrored { -1.0 } else { 1.0 },
                    y: 0.0,
                };
                let heading = normalized(subtract(hand.b, hand.a), facing);
                let heading_then = normalized(subtract(hand.previous_b, hand.previous_a), heading);
                let from = add(hand.previous_b, scale(heading_then, ahead));
                let to = add(hand.b, scale(heading, ahead));
                // A fist pushed backward lands on nothing (`fist_sweep`).
                (dot(subtract(to, from), heading) > 0.0).then_some((
                    body,
                    strike.hand,
                    index,
                    from,
                    to,
                    heading,
                ))
            })
            .collect();
        let landings: Vec<FistLanding> = sweeps
            .iter()
            .map(|&(body, _, _, from, to, heading)| {
                self.land_fist(body as u8, from, to, heading, dt)
            })
            .collect();
        for ((body, side, hand, from, to, _), landing) in sweeps.into_iter().zip(landings) {
            let given = hypot(landing.given.x, landing.given.y) > EPSILON;
            let speed = distance(from, to) / dt.max(EPSILON) * self.mm_per_px() * 0.001;
            let landed = given && speed > LANDING_SPEED;
            if given {
                self.push_fighter_arm(body, side, scale(landing.given, -1.0), to, dt);
            }
            if landing.blow > 0.0 {
                self.load_puncher_hand(hand, landing.blow, landing.blow_normal);
            }
            if landed {
                self.note(
                    EventKind::PunchTurnedBack,
                    Subject::Bone(hand),
                    to,
                    Some(Actor::Tissue),
                    Measure::Speed,
                    speed,
                    LANDING_SPEED,
                );
                if let Some(strike) = self.bodies[body]
                    .mover
                    .as_mut()
                    .and_then(|mover| mover.strike.as_mut())
                {
                    strike.turn_back();
                }
            }
            if let Some(contact) = landing.contact {
                self.fist_contacts.push(FistContact {
                    contact,
                    shift: Vec2::default(),
                    body,
                    arm: side.index(),
                });
            }
        }
    }

    /// The arm on `side` of fighter `body`, beyond its shoulder, takes the
    /// momentum `impulse` at `at` as one piece over the step `dt`: its
    /// bones and the flesh of its sheets move and turn about their centre
    /// of mass, and the shoulder's hold keeps the arm on the body, as a
    /// front-view body's punching arm takes it (`push_arm_back`).
    pub(super) fn push_fighter_arm(
        &mut self,
        body: usize,
        side: Side,
        impulse: Vec2,
        at: Vec2,
        dt: f64,
    ) {
        let Some(rig) = self
            .bodies
            .get(body)
            .and_then(|state| state.fighter.as_ref())
        else {
            return;
        };
        let s = side.index();
        let arm = [rig.upper_arms[s], rig.forearms[s], rig.hands[s]];
        let sheets: Vec<u8> = arm
            .iter()
            .filter_map(|&bone| self.bones.get(bone).map(|bone| bone.sheet))
            .collect();
        let in_arm = |point: &Point| {
            point.body as usize == body
                && sheets.contains(&point.sheet)
                && !point.pinned
                && !point.is_ghost()
        };
        let mut sums = (0.0, Vec2::default(), 0.0);
        let mut add_to = |mass: f64, at: Vec2| {
            sums.0 += mass;
            sums.1 = add(sums.1, scale(at, mass));
            sums.2 += mass * dot(at, at);
        };
        // Measured from where the push lands rather than the window's
        // origin, so two fighters standing as mirror images sum the same
        // small numbers.
        for &index in &arm {
            if let Some(bone) = self.bones.get(index).filter(|bone| !bone.pinned) {
                add_to(bone.mass * 0.5, subtract(bone.a, at));
                add_to(bone.mass * 0.5, subtract(bone.b, at));
            }
        }
        for point in self.points.iter().filter(|point| in_arm(point)) {
            add_to(point.mass, subtract(point.position, at));
        }
        let (mass, moment, second) = sums;
        if mass <= EPSILON {
            return;
        }
        let centre = scale(moment, 1.0 / mass);
        let inertia = second - mass * dot(centre, centre);
        if inertia <= EPSILON {
            return;
        }
        let speed = scale(impulse, 1.0 / mass);
        // The push lands at the origin these are measured from.
        let spin = cross(scale(centre, -1.0), impulse) / inertia;
        let moved = |place: Vec2| {
            let from_centre = subtract(subtract(place, at), centre);
            scale(
                add(
                    speed,
                    scale(
                        Vec2 {
                            x: -from_centre.y,
                            y: from_centre.x,
                        },
                        spin,
                    ),
                ),
                dt,
            )
        };
        for &index in &arm {
            let Some(bone) = self.bones.get(index).copied().filter(|bone| !bone.pinned) else {
                continue;
            };
            self.bones[index].a = add(bone.a, moved(bone.a));
            self.bones[index].b = add(bone.b, moved(bone.b));
        }
        for index in 0..self.points.len() {
            let point = self.points[index];
            if in_arm(&point) {
                self.points[index].position = add(point.position, moved(point.position));
            }
        }
    }
}
