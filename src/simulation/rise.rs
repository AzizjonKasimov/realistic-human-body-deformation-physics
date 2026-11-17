//! A side-view fighter down on the floor, and getting up again.
//!
//! A fighter that falls lies until it comes to rest; one knocked out
//! (`knockout.rs`) lies limp, for longer the harder the blow turned its head
//! past the knockout limit. Knocked-out people lie limp for seconds: 66% of
//! 35 filmed knockouts held a fencing posture for 2 to 16 s (Hosseini and
//! Lifshitz 2009), and monkeys whose heads were turned harder stayed out
//! longer (Gennarelli et al. 1982). No study ties how hard a blow was to how
//! long a person stays down, so how long is set to suit a fight's count: a
//! blow just past the limit leaves a fighter up by about the count of six
//! to eight, a much harder one keeps it down past ten
//! (`Materials::knockout_seconds`, `Materials::knockout_more_seconds`). Its
//! muscles then come back over a moment, as BombSquad's characters' balance
//! does once their knockout timer runs out (Ballistica, MIT: `spaz_node.cc`,
//! the balance climbing back and the stand spring's strength following one
//! less the fourth power of what is left), and it gets up.
//!
//! Getting up is a move like the others (`mover.rs`): a short list of key
//! poses the muscles follow (`profile::sitting_up` and the rest), each set
//! on the floor where the part of the body it stands on already is, and
//! blended into from where the body really lies, so that the muscles are
//! asked first for where the joints already are. From its back a fighter
//! sits up onto its hands, lifts its hips onto its hands and feet, and tips
//! forward over its feet into a crouch; from its front it comes up onto its
//! hands and knees, up onto its knees, and takes a knee, its lead foot
//! forward; out of the crouch or off the knee it rises into its stance as
//! out of a duck (`Mover::stand_up_from`): a humanoid robot got up from its
//! back and its front a hundred times in a hundred through such key poses,
//! each standing still on what touched the floor, with one short tip over
//! its feet (Stückler, Schwenk and Behnke 2006), and people rising from
//! their backs most simply curl up to sit, come to a squat, and stand
//! (VanSant 1988). The hold on the hips carries them through the poses, as
//! it does standing, pushing them only from the feet and knees under them
//! and giving what it pushes back on those (`balance.rs`), so that the floor
//! bears it; the hands hold the chest up. A fighter touching the floor with
//! nothing that could bear it cannot get up at all.

use super::balance::{weight_shares, Bears};
use super::mover::Frame;
use super::sides::BonePiece;
use super::soft::{Softness, KG_PER_MASS};
use super::stance::FOOT_FRICTION;
use super::*;
use crate::profile::{self, BonePlaces, Hinge, Joints, Pose, Side};

/// How long a fighter knocked out by a blow just past the knockout limit
/// lies limp, in seconds, and how much longer for each whole limit past it,
/// as `Materials::knockout_seconds` and `knockout_more_seconds` are built:
/// it then gets up in about four seconds more, up by about the count of six
/// at the limit, by ten a quarter past it, and still down at ten from about
/// a third past it.
pub(super) const KNOCKOUT_SECONDS: f64 = 2.0;
pub(super) const KNOCKOUT_MORE_SECONDS: f64 = 15.0;

/// How long a fighter that fell lies before it gets up, in seconds: about as
/// long as a body takes to come to rest on the floor.
const FALL_SECONDS: f64 = 0.6;

/// How long a knocked-out fighter's muscles take to come back to their full
/// strength, in seconds, once it comes to.
const COME_TO_SECONDS: f64 = 0.6;

/// How much of a stage its joints take to come from where they lay to the
/// stage's poses: the poses move over the whole stage.
const SETTLE_SHARE: f64 = 0.4;

/// How many times its time a stage may take before a fighter getting up
/// gives it up and begins again from how it lies: a stage ends once its time
/// is up and the body has come near its pose (`World::stage_reached`).
const STAGE_PATIENCE: f64 = 2.5;

/// How near a stage's pose a fighter getting up must have come for the
/// stage to end: its hips within this many body heights of where the pose
/// puts them, and its chest's lean within this many radians.
const REACHED_HIPS: f64 = 0.06;
const REACHED_LEAN: f64 = 0.45;
/// How fast its hips may still move, at the most, in metres a second, for a
/// stage to end.
const REACHED_SPEED: f64 = 0.25;

/// How high a stepping foot lifts as getting up swings it, in body heights.
const SWING_LIFT: f64 = 0.025;

/// How high a key pose's fist may be, in body heights, and be meant to rest
/// on the floor.
const ON_THE_FLOOR: f64 = 0.05;

/// How far a fighter's trunk leans from upright, in radians, past which it
/// lies on its back (leaning back) or its front (leaning forward).
const LYING_LEAN: f64 = 1.0;

/// How high a fighter sitting on the floor has its hips, at the most, in
/// body heights: higher, an upright fighter is on its knees.
const SITTING_HIPS: f64 = 0.15;

/// How far behind its hips a fighter's knees are, at the least, in body
/// heights, when it lies on its front with its legs out behind it.
const KNEES_BEHIND: f64 = 0.08;

/// How far apart a fighter's feet must stand, from its lead foot's ball back
/// to its rear foot's, in body heights, for it to stand up out of a crouch
/// or off a knee: about as far as the stance has them (0.31).
const STANDING_FEET: (f64, f64) = (0.18, 0.45);

/// How firmly the legs of a fighter getting up carry its hips through the
/// poses, across and upward, in hertz on the body's weight, critically
/// damped: as a spring, not the weight they bear standing, since the hips
/// go from the floor to standing.
pub(super) const RISE_HERTZ: Vec2 = Vec2 { x: 3.0, y: 4.0 };

/// How firmly a fighter getting up holds its chest where its pose has it on
/// its arms, in hertz on what the chest carries, critically damped; how
/// much of the body that is, the head, the chest, and the arms (de Leva
/// 1996); and the most each arm resting on its hand pushes it up with, in
/// body weights: about what one arm bears at the top of a push-up, where
/// the hands bear about two thirds of the body's weight (Suprak, Bohannon,
/// Morales, Stone and Ebbert 2011).
const CHEST_HERTZ: f64 = 3.0;
const CHEST_SHARE: f64 = 0.40;
const ARM_PUSH: f64 = 0.35;

/// A side-view fighter off its feet (`Mover::down`).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Down {
    /// Seconds since it went down.
    age: f64,
    /// Seconds after going down that it begins to get up: how long a
    /// knockout leaves it limp, or how long a body that fell takes to come
    /// to rest.
    out_for: f64,
    /// Knocked out, its muscles let go until it comes to, rather than
    /// fallen.
    knocked_out: bool,
    /// Getting up off the floor, once it has begun; and once it is up on its
    /// feet in a crouch or on a knee, standing up out of it as it comes up
    /// out of a duck (`Mover::crouch`), still down until it stands.
    rise: Option<Rise>,
    standing: bool,
}

/// A fighter getting up: the way it takes, the stage it is in, and how the
/// body lay as the stage began.
#[derive(Clone, Debug, PartialEq)]
struct Rise {
    route: Route,
    stage: usize,
    /// Seconds since the stage began, and since its muscles began coming
    /// back.
    age: f64,
    awake: f64,
    /// The pose the body was in as the stage began, in its own frame, and
    /// each joint's turn then, in the order of `Hinge::ALL`, as the
    /// mover's poses turn them (`profile::Hinge::ALL`).
    from: Pose,
    from_turns: [f64; 16],
    /// Where the stage's key pose stands: its own zero, in body heights
    /// forward of where the fighter was built.
    origin: f64,
    /// The pose it is asked for now, in its own frame, and where that puts
    /// the foot of its neck, which its arms hold its chest up by, in the
    /// world, and how fast that moves, in pixels a second.
    pose: Pose,
    chest: Option<(Vec2, Vec2)>,
    /// How many times it has begun getting up.
    tries: u32,
}

/// The way up off the floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    /// From lying on its back, or sitting.
    Back,
    /// From lying on its front, or on its knees.
    Front,
}

/// Which part of the body a stage's key pose is set on the floor by: the
/// pose's own zero goes where that part of the pose then lies on the part
/// of the body it stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Anchor {
    Hips,
    LeadBall,
    RearBall,
}

/// A stage of getting up: the key pose the body goes to, over how long,
/// set on the floor by what; which feet stay where they stand; until how far
/// through the stage the hands stay where they are on the floor (0: they go
/// where the pose puts them from the start); and a foot it swings through
/// the air to where the pose puts it, over which part of the stage.
struct Stage {
    /// What it is called, for the tools that print it.
    #[cfg_attr(not(test), allow(dead_code))]
    name: &'static str,
    pose: fn() -> Pose,
    seconds: f64,
    anchor: Anchor,
    feet_stay: [bool; 2],
    hands_until: f64,
    swing: Option<(Side, f64, f64)>,
}

/// Getting up from the back: sat up onto the hands, the feet drawn in as
/// far apart as the stance has them; the hips lifted toward the heels; and
/// tipped forward over the feet into a crouch, to stand up out of. Taken on
/// from the squat by a foot stepping back onto its knee and the other in
/// again as it stood, the stepping foot went far behind, the rear leg stayed
/// straight, and the fighter lunged rather than knelt.
const FROM_THE_BACK: [Stage; 3] = [
    Stage {
        name: "sitting up",
        pose: profile::sitting_up,
        seconds: 1.0,
        anchor: Anchor::Hips,
        feet_stay: [false, false],
        hands_until: 0.0,
        swing: None,
    },
    Stage {
        name: "lifting its hips",
        pose: profile::bridging,
        seconds: 0.8,
        anchor: Anchor::RearBall,
        feet_stay: [true, true],
        hands_until: 1.0,
        swing: None,
    },
    Stage {
        name: "tipping onto its feet",
        pose: profile::crouching,
        seconds: 1.2,
        anchor: Anchor::LeadBall,
        feet_stay: [true, true],
        hands_until: 0.3,
        swing: None,
    },
];

/// Getting up from the front: up onto hands and knees; up onto both knees,
/// the trunk upright; and the lead foot forward onto the floor, down on the
/// rear knee, the feet as far apart as the stance has them, to stand up off.
const FROM_THE_FRONT: [Stage; 3] = [
    Stage {
        name: "onto its hands and knees",
        pose: profile::on_all_fours,
        seconds: 1.0,
        anchor: Anchor::Hips,
        feet_stay: [false, false],
        hands_until: 0.0,
        swing: None,
    },
    Stage {
        name: "up onto its knees",
        pose: profile::kneeling_up,
        seconds: 0.8,
        anchor: Anchor::RearBall,
        feet_stay: [true, true],
        hands_until: 0.4,
        swing: None,
    },
    Stage {
        name: "taking a knee",
        pose: profile::kneeling,
        seconds: 0.9,
        anchor: Anchor::RearBall,
        feet_stay: [false, true],
        hands_until: 0.0,
        swing: Some((Side::Lead, 0.1, 0.7)),
    },
];

impl Route {
    fn stages(self) -> &'static [Stage] {
        match self {
            Route::Back => &FROM_THE_BACK,
            Route::Front => &FROM_THE_FRONT,
        }
    }
}

impl Down {
    /// Knocked out, to lie limp for `out_for` seconds.
    pub(super) fn knocked_out(out_for: f64) -> Self {
        Self {
            age: 0.0,
            out_for,
            knocked_out: true,
            rise: None,
            standing: false,
        }
    }

    /// Fallen, its weight run past its feet or its hips sunk to the floor.
    pub(super) fn fell() -> Self {
        Self {
            age: 0.0,
            out_for: FALL_SECONDS,
            knocked_out: false,
            rise: None,
            standing: false,
        }
    }

    /// Whether it is getting up: off the floor, or standing up.
    pub(super) fn getting_up(&self) -> bool {
        self.rise.is_some() || self.standing
    }

    /// Whether it is up on its feet, standing up out of a crouch or off a
    /// knee.
    pub(super) fn standing_up(&self) -> bool {
        self.standing
    }

    /// Up on its feet: it stands up out of its crouch.
    fn stands(&mut self) {
        self.rise = None;
        self.standing = true;
    }

    /// How strong its muscles are now, from 0, limp, to 1: a knocked-out
    /// fighter's come back as it comes to, as BombSquad's stand spring
    /// follows its balance, one less the fourth power of what is still to
    /// come; a fallen one's hold as they are.
    pub(super) fn strength(&self) -> f64 {
        if self.standing {
            return 1.0;
        }
        match &self.rise {
            None if self.knocked_out => 0.0,
            Some(rise) if self.knocked_out => {
                let back = (rise.awake / COME_TO_SECONDS).clamp(0.0, 1.0);
                let left = (1.0 - back) * (1.0 - back);
                1.0 - left * left
            }
            _ => 1.0,
        }
    }

    /// The pose it is getting up through now, in its own frame, if it is
    /// getting up.
    pub(super) fn rising_pose(&self) -> Option<Pose> {
        self.rise.as_ref().map(|rise| rise.pose)
    }

    /// Where its pose puts the foot of its neck, in the world, and how fast
    /// that moves, while it gets up (`World::solve_chest_holds`).
    fn chest_aim(&self) -> Option<(Vec2, Vec2)> {
        self.rise.as_ref().and_then(|rise| rise.chest)
    }

    /// It has been down `dt` seconds more, standing up.
    pub(super) fn age_by(&mut self, dt: f64) {
        self.age += dt;
    }

    /// What it is doing, in words, for the tools that print it.
    #[cfg(test)]
    pub(super) fn describe(&self) -> String {
        let how = if self.knocked_out {
            "knocked out"
        } else {
            "fallen"
        };
        if self.standing {
            return format!("{how}, standing up {:.2} s after going down", self.age);
        }
        match &self.rise {
            None => format!("{how}, down {:.2} s of {:.2}", self.age, self.out_for),
            Some(rise) => format!(
                "{how}, getting up from its {} ({}, {:.2} s in, try {}, strength {:.2})",
                match rise.route {
                    Route::Back => "back",
                    Route::Front => "front",
                },
                rise.route.stages()[rise.stage].name,
                rise.age,
                rise.tries + 1,
                self.strength(),
            ),
        }
    }
}

impl World {
    /// How strong side-view fighter `body`'s muscles are now, from 0 to 1:
    /// less than 1 only while they come back after a knockout (`Down`).
    pub(super) fn muscle_strength(&self, body: usize) -> f64 {
        self.bodies
            .get(body)
            .and_then(|state| state.mover.as_ref())
            .and_then(|mover| mover.down.as_ref())
            .map_or(1.0, Down::strength)
    }

    /// Whether side-view fighter `body` is getting up off the floor.
    pub(super) fn rising(&self, body: usize) -> bool {
        self.bodies
            .get(body)
            .and_then(|state| state.mover.as_ref())
            .and_then(|mover| mover.down.as_ref())
            .is_some_and(Down::getting_up)
    }

    /// Moves side-view fighter `body`, down on the floor, on by a step `dt`
    /// long: it lies until it comes to or to rest, then gets up through its
    /// stages, and once it stands again, it stands (`Mover::stand_up`).
    pub(super) fn drive_down(
        &mut self,
        body: usize,
        rig: &fighter::Rig,
        frame: &Frame,
        mover: &mut Mover,
        dt: f64,
    ) {
        let Some(mut down) = mover.down.take() else {
            return;
        };
        down.age += dt;
        if down.rise.is_none() {
            if down.age < down.out_for {
                self.let_fallen_joints_be(rig);
                mover.down = Some(down);
                return;
            }
            // It comes to, or to rest, and begins to get up.
            self.bodies[body].head.knocked_out = false;
            let (route, stage) = self.way_up(rig, frame);
            down.rise = Some(self.begin_stage(rig, frame, route, stage, 0.0, 0));
        }
        let Some(rise) = down.rise.as_mut() else {
            mover.down = Some(down);
            return;
        };
        rise.age += dt;
        rise.awake += dt;
        let stages = rise.route.stages();
        let seconds = stages[rise.stage].seconds;
        let reached = self.stage_reached(rig, frame, rise);
        if rise.age >= seconds && !reached && rise.age < seconds * STAGE_PATIENCE {
            // Not there yet: it holds on toward the stage's pose.
        } else if rise.age >= seconds * STAGE_PATIENCE {
            // It did not get there: it begins again from how it lies now.
            let (route, stage) = self.way_up(rig, frame);
            *rise = self.begin_stage(rig, frame, route, stage, rise.awake, rise.tries + 1);
        } else if rise.age >= seconds {
            if rise.stage + 1 < stages.len() {
                *rise = self.begin_stage(
                    rig,
                    frame,
                    rise.route,
                    rise.stage + 1,
                    rise.awake,
                    rise.tries,
                );
            } else {
                // On its feet, or a knee: it stands up out of the crouch the
                // way it comes up out of a duck, its hips carried by its
                // legs from its feet as they stand.
                let now = self.measured_pose(rig, frame);
                let stance = profile::stance();
                let crouch = ((stance.hips.1 - now.hips.1) / profile::CROUCH_DROP).clamp(0.0, 1.0);
                down.stands();
                mover.down = Some(down);
                // Its stance begins where its hips really are over its feet as
                // they stand: set by its lead foot alone, the stance had the
                // hips 10 cm ahead of a crouch that sat back, and the lead leg,
                // aimed from there, kicked its foot up off the floor.
                let feet = now.feet.map(|foot| foot.ball.0);
                mover.stand_up_from(now.hips.0, feet, crouch);
                #[cfg(test)]
                if std::env::var("RP_STAGES").is_ok() {
                    println!(
                        "  stands up: hips ({:.3}, {:.3}), feet {feet:?}, crouch {crouch:.2}",
                        now.hips.0, now.hips.1
                    );
                }
                return;
            }
        }
        let stage = &rise.route.stages()[rise.stage];
        let t = (rise.age / stage.seconds).clamp(0.0, 1.0);
        let mut key = (stage.pose)();
        key.hips.0 += rise.origin;
        for side in 0..2 {
            key.fists[side].0 += rise.origin;
            key.feet[side].ball.0 += rise.origin;
        }
        // Where the holds carry the body: from how it lay as the stage began
        // toward the key pose, set on the floor.
        let aim = profile::blend(&rise.from, &key, ease(t));
        // What its joints are aimed at: the body as it really is at its hips
        // and pelvis, bent from there as `aim` bends it, the feet and hands
        // that stay on the floor reached where they rest, and the others
        // reached for where `aim` puts them, a hand meant for the floor at
        // the floor. Aimed at `aim` itself, the joints of a body that was not
        // yet where `aim` has it fought what rested on the floor, and a
        // fighter sitting up more upright than its pose put its hands up in
        // the air behind it instead of on the floor.
        let now = self.measured_pose(rig, frame);
        let mut shape = aim;
        shape.hips = now.hips;
        shape.pelvis = now.pelvis;
        shape.lumbar = shape.pelvis + (aim.lumbar - aim.pelvis);
        shape.thorax = shape.lumbar + (aim.thorax - aim.lumbar);
        shape.neck = shape.thorax + (aim.neck - aim.thorax);
        shape.head = shape.neck + (aim.head - aim.neck);
        let (shoulders, aimed_shoulders) = (Joints::of(&shape).shoulder, Joints::of(&aim).shoulder);
        for side in 0..2 {
            let fist = aim.fists[side];
            shape.fists[side] = (
                shoulders[side].0 + (fist.0 - aimed_shoulders[side].0),
                if fist.1 < ON_THE_FLOOR {
                    fist.1
                } else {
                    shoulders[side].1 + (fist.1 - aimed_shoulders[side].1)
                },
            );
        }
        // A foot that stays is reached for on the floor where it stood as
        // the stage began: reached for where it was, a rear foot the legs had
        // lifted 6 cm as they lifted the hips stayed in the air and was
        // carried on with them, past the lead foot.
        for side in 0..2 {
            if stage.feet_stay[side] {
                shape.feet[side].ball = (rise.from.feet[side].ball.0, 0.0);
            }
        }
        if stage.hands_until > 0.0 {
            let go = if t < stage.hands_until {
                0.0
            } else {
                ease((t - stage.hands_until) / (1.0 - stage.hands_until).max(EPSILON))
            };
            for side in 0..2 {
                let was = rise.from.fists[side];
                let stays = (was.0, was.1.min(ON_THE_FLOOR * 0.5));
                let goes = shape.fists[side];
                shape.fists[side] = (
                    stays.0 + (goes.0 - stays.0) * go,
                    stays.1 + (goes.1 - stays.1) * go,
                );
            }
        }
        // A foot it swings hands its weight to the other first, swings from
        // where it stood to where the key pose puts it, and takes it back.
        let (mut swinging, mut loads) = ([false; 2], [1.0; 2]);
        if let Some((side, start, end)) = stage.swing {
            let s = side.index();
            if t < start {
                shape.feet[s].ball = (rise.from.feet[s].ball.0, 0.0);
                loads[s] = 1.0 - t / start.max(EPSILON);
            } else if t < end {
                let along = ease((t - start) / (end - start));
                let (from, to) = (rise.from.feet[s].ball, key.feet[s].ball);
                shape.feet[s].ball = (
                    from.0 + (to.0 - from.0) * along,
                    SWING_LIFT * libm::sin(PI * along),
                );
                swinging[s] = true;
                loads[s] = 0.0;
            } else {
                // Down, it is reached for where the key pose puts it: reached
                // for where it was, a lead foot set down short drifted back
                // behind the rear one.
                shape.feet[s].ball = (key.feet[s].ball.0, 0.0);
                loads[s] = ((t - end) / (1.0 - end).max(EPSILON)).min(1.0);
            }
        }
        rise.pose = aim;
        let neck = frame.world(Joints::of(&aim).neck);
        let was = rise.chest.map_or(neck, |(at, _)| at);
        rise.chest = Some((neck, scale(subtract(neck, was), 1.0 / dt.max(EPSILON))));
        let come = ease(t / SETTLE_SHARE);
        let from_turns = rise.from_turns;
        mover.down = Some(down);
        let shaped = Joints::of(&shape);
        self.aim_joints(
            body,
            rig,
            frame,
            mover,
            &shaped,
            Some((&from_turns, come)),
            dt,
        );
        self.carry_hips(rig, frame, mover, &Joints::of(&aim), dt);
        self.place_feet(rig, frame, &shaped, swinging, loads, dt);
    }

    /// Each fighter getting up holds its chest up on its arms where its pose
    /// has it, from the hands resting on the floor (`balance.rs`, `Bears`): a
    /// spring and a damper on the chest, bone and flesh, at the foot of the
    /// neck, toward where the pose puts it, pushing it no harder than the
    /// arms resting there push and never down, and giving what it gives back
    /// on the hands, by where each rests under it, so that the floor bears
    /// the arms' push, as the hips' hold gives its push back on the feet.
    /// Lifted by its trunk's muscles alone, the chest of a fighter sitting up
    /// lifts the legs instead, as a sit-up does with no one holding the
    /// feet.
    pub(super) fn solve_chest_holds(&mut self, h: f64) {
        for body in 0..self.bodies.len() {
            let Some((target, aim_speed)) = self.bodies[body]
                .mover
                .as_ref()
                .and_then(|mover| mover.down.as_ref())
                .and_then(Down::chest_aim)
            else {
                continue;
            };
            let Some(rig) = self.bodies[body].fighter.as_ref() else {
                continue;
            };
            let thoracic = rig.thoracic;
            let Some(stance) = self.stances.get(body).and_then(Option::as_ref) else {
                continue;
            };
            let hands: Vec<(BonePiece, Vec2)> = stance
                .others
                .iter()
                .filter(|support| matches!(support.bears, Bears::Hand(_)))
                .map(|support| (support.piece.clone(), support.at))
                .collect();
            let given = stance.chest_given;
            let Some(chest) = self.bone_pieces.get(thoracic).cloned().flatten() else {
                continue;
            };
            if hands.is_empty() {
                continue;
            }
            let bone = self.bones[thoracic];
            let at = bone.previous_b;
            let lever = subtract(at, chest.centre);
            let masses = (
                1.0 / (1.0 / chest.mass + lever.y * lever.y / chest.inertia),
                1.0 / (1.0 / chest.mass + lever.x * lever.x / chest.inertia),
            );
            let carried = CHEST_SHARE * tissue::BODY_MASS_KG / KG_PER_MASS;
            let omega = 2.0 * PI * CHEST_HERTZ;
            let (stiffness, damping) = (carried * omega * omega, 2.0 * carried * omega);
            let speed = subtract(scale(subtract(bone.b, bone.previous_b), 1.0 / h), aim_speed);
            let error = subtract(at, target);
            let impulse = Vec2 {
                x: Softness::of_stiffness(stiffness, damping, masses.0, h)
                    .impulse(masses.0, speed.x, error.x, given.x),
                y: Softness::of_stiffness(stiffness, damping, masses.1, h)
                    .impulse(masses.1, speed.y, error.y, given.y),
            };
            let weight = tissue::BODY_MASS_KG * self.materials.gravity * self.newtons() * h;
            let up = ARM_PUSH * hands.len() as f64 * weight * self.muscle_strength(body);
            let across = FOOT_FRICTION * up;
            let total = add(given, impulse);
            let held = Vec2 {
                x: total.x.clamp(-across, across),
                y: total.y.clamp(-up, 0.0),
            };
            let change = subtract(held, given);
            if let Some(stance) = self.stances.get_mut(body).and_then(Option::as_mut) {
                stance.chest_given = held;
            }
            self.push_piece(&chest, lever, change, h);
            let places: Vec<f64> = hands.iter().map(|(_, place)| place.x).collect();
            let shares = weight_shares(&places, at.x);
            for ((hand, place), share) in hands.iter().zip(shares) {
                self.push_piece(
                    hand,
                    subtract(*place, hand.centre),
                    scale(change, -share),
                    h,
                );
            }
        }
    }

    /// The pose fighter `rig` stands in `frame` in now, as its bones lie
    /// (`profile::measured`).
    fn measured_pose(&self, rig: &fighter::Rig, frame: &Frame) -> Pose {
        let ends = |bone: usize| {
            let bone = self.bones[bone];
            (frame.own(bone.a), frame.own(bone.b))
        };
        let mut pose = profile::measured(&BonePlaces {
            pelvis: ends(rig.pelvis),
            lumbar: ends(rig.lumbar),
            thoracic: ends(rig.thoracic),
            cervical: ends(rig.cervical),
            skull: ends(rig.skull),
            feet: rig.feet.map(ends),
            knuckles: rig.hands.map(|hand| frame.own(self.bones[hand].b)),
        });
        // Each shoulder as far across the chest as its joint's seat has slid
        // (`World::aim_joints`): measured as square, a knocked-out fighter's
        // shoulders, left where the stance slid them, were moved back 5 cm in
        // a step as its muscles came back, and one tore.
        let stance = profile::stance();
        let slide = rig.shoulder_slide;
        for side in Side::BOTH {
            let s = side.index();
            if let Some(joint) = self.bone_joints.get(rig.hinge(Hinge::Shoulder(side))) {
                let along = dot(subtract(joint.seat, rig.shoulder_seats[s]), slide)
                    / dot(slide, slide).max(EPSILON);
                pose.shoulders[s] = stance.shoulders[s] + along / frame.height;
            }
        }
        pose
    }

    /// Stage `stage` of getting up by `route`, begun from how fighter `rig`
    /// lies now, its muscles `awake` seconds back, on try `tries`.
    fn begin_stage(
        &self,
        rig: &fighter::Rig,
        frame: &Frame,
        route: Route,
        stage: usize,
        awake: f64,
        tries: u32,
    ) -> Rise {
        let from = self.measured_pose(rig, frame);
        let key = (route.stages()[stage].pose)();
        #[cfg(test)]
        if std::env::var("RP_STAGES").is_ok() {
            println!(
                "  begins {:?} stage {stage}: hips ({:.3}, {:.3}), lead ball ({:.3}, {:.3}) pitch {:.2}, rear ball ({:.3}, {:.3}) pitch {:.2}",
                route,
                from.hips.0,
                from.hips.1,
                from.feet[0].ball.0,
                from.feet[0].ball.1,
                from.feet[0].pitch,
                from.feet[1].ball.0,
                from.feet[1].ball.1,
                from.feet[1].pitch
            );
        }
        let origin = match route.stages()[stage].anchor {
            Anchor::Hips => from.hips.0 - key.hips.0,
            Anchor::LeadBall => from.feet[0].ball.0 - key.feet[0].ball.0,
            Anchor::RearBall => from.feet[1].ball.0 - key.feet[1].ball.0,
        };
        // Each joint's turn as the mover's poses turn them: the window's
        // angles run the other way for a fighter facing right, and each
        // joint's muscles aim past what they learned to bear (`aim`).
        let mut from_turns = [0.0; 16];
        for (place, &index) in rig.hinges.iter().enumerate() {
            let aim = self
                .bone_joints
                .get(index)
                .map_or(0.0, |joint| joint.drive.aim);
            from_turns[place] = self
                .joint_turn(index)
                .map_or(0.0, |turn| -frame.facing * (turn - aim));
        }
        Rise {
            route,
            stage,
            age: 0.0,
            awake,
            from,
            from_turns,
            origin,
            pose: from,
            chest: None,
            tries,
        }
    }

    /// Which way fighter `rig` gets up from how it lies, and from which
    /// stage: lying on its back, or sitting on its seat with its feet in
    /// front of it, from the start of getting up from the back; lying on its
    /// front, its chest down or propped up on its arms with its knees behind
    /// its hips, from the start of getting up from the front; up on its
    /// knees, or sitting back on its heels with its feet under it, from
    /// taking a knee. Fallen forward onto its knees and set to sit up with
    /// its feet in front, a fighter sitting on its heels tried again and
    /// again and never got its feet out from under it.
    fn way_up(&self, rig: &fighter::Rig, frame: &Frame) -> (Route, usize) {
        let chest = self.bones[rig.thoracic];
        let (low, high) = (frame.own(chest.a), frame.own(chest.b));
        let lean = libm::atan2(high.0 - low.0, high.1 - low.1);
        let now = self.measured_pose(rig, frame);
        let feet_under = now.feet.iter().all(|foot| foot.ball.0 < now.hips.0);
        let knees_behind = rig
            .shins
            .iter()
            .all(|&shin| frame.own(self.bones[shin].a).0 < now.hips.0 - KNEES_BEHIND);
        if lean < -LYING_LEAN {
            (Route::Back, 0)
        } else if lean > LYING_LEAN || knees_behind {
            (Route::Front, 0)
        } else if now.hips.1 < SITTING_HIPS && !feet_under {
            (Route::Back, 0)
        } else {
            (Route::Front, 1)
        }
    }

    /// Whether fighter `rig`, getting up as `rise`, has come near the pose
    /// its stage ends in: its hips near where the pose puts them, its chest
    /// leaning about as the pose leans it, and come to rest there.
    fn stage_reached(&self, rig: &fighter::Rig, frame: &Frame, rise: &Rise) -> bool {
        let now = self.measured_pose(rig, frame);
        let mut key = (rise.route.stages()[rise.stage].pose)();
        key.hips.0 += rise.origin;
        let off = libm::hypot(now.hips.0 - key.hips.0, now.hips.1 - key.hips.1);
        // Handed on still moving, a fighter that tipped over its feet into
        // a crouch went on over them as it stood.
        let pelvis = self.bones[rig.pelvis];
        let speed = distance(pelvis.b, pelvis.previous_b) / self.materials.fixed_dt
            * self.mm_per_px()
            * 0.001;
        // The last stage hands over to standing up on the feet as they stand,
        // so they must stand about as far apart as the stance has them:
        // handed over with its feet side by side, a fighter stood up and
        // bobbed on one foot and then the other.
        let feet_apart = now.feet[0].ball.0 - now.feet[1].ball.0;
        let last = rise.stage + 1 == rise.route.stages().len();
        let feet_ready = !last || (STANDING_FEET.0..STANDING_FEET.1).contains(&feet_apart);
        off < REACHED_HIPS
            && (now.thorax - key.thorax).abs() < REACHED_LEAN
            && speed < REACHED_SPEED
            && feet_ready
    }

    /// How far fighter `body`, getting up, is from the pose its stage ends
    /// in, for the tools that print it: its hips, in body heights across
    /// and up, and its chest's lean, in radians, past the pose's.
    #[cfg(test)]
    pub(super) fn rise_line(&self, body: usize) -> String {
        let (Some(frame), Some(state)) = (self.fighter_frame(body), self.bodies.get(body)) else {
            return String::new();
        };
        let (Some(rig), Some(rise)) = (
            state.fighter.as_ref(),
            state
                .mover
                .as_ref()
                .and_then(|mover| mover.down.as_ref())
                .and_then(|down| down.rise.as_ref()),
        ) else {
            return String::new();
        };
        let now = self.measured_pose(rig, &frame);
        let mut key = (rise.route.stages()[rise.stage].pose)();
        key.hips.0 += rise.origin;
        let report = self.balance_reports.get(body).copied().unwrap_or_default();
        let chest = self
            .stances
            .get(body)
            .and_then(Option::as_ref)
            .map_or(Vec2::default(), |stance| stance.chest_given);
        let h = self.materials.fixed_dt / self.materials.substeps.max(1) as f64;
        let hips_now = frame.world(now.hips);
        let aimed = frame.world(rise.pose.hips);
        let swung: Vec<String> = rig
            .feet
            .iter()
            .zip(["lead", "rear"])
            .filter_map(|(&foot, name)| {
                let hold = self.foot_holds.iter().find(|hold| hold.bone == foot)?;
                let bone = self.bones[foot];
                let at = frame.own(lerp(bone.a, bone.b, 0.5));
                Some(if hold.swinging {
                    let to = frame.own(hold.swing_to);
                    format!(
                        "{name} foot swung ({:.3}, {:.3}) toward ({:.3}, {:.3})",
                        at.0, at.1, to.0, to.1
                    )
                } else {
                    format!(
                        "{name} foot at ({:.3}, {:.3}) load {:.2}",
                        at.0, at.1, hold.load
                    )
                })
            })
            .collect();
        format!(
            "{}; hips off the stage's pose ({:+.3}, {:+.3}) and off the aim ({:+.3}, {:+.3}), chest lean {:+.2} against {:+.2}, pelvis {:+.2} against {:+.2}; legs push the hips ({:+.0}, {:+.0}) N, arms the chest ({:+.0}, {:+.0}) N",
            swung.join(", "),
            now.hips.0 - key.hips.0,
            now.hips.1 - key.hips.1,
            (hips_now.x - aimed.x) * frame.facing / frame.height,
            (aimed.y - hips_now.y) / frame.height,
            now.thorax,
            key.thorax,
            now.pelvis,
            key.pelvis,
            report.push.x * frame.facing,
            -report.push.y,
            chest.x / (h * self.newtons()) * frame.facing,
            -chest.y / (h * self.newtons()),
        )
    }

    /// Whether side-view fighter `body` is up on its feet, standing up out
    /// of a crouch or off a knee (`Down::standing_up`).
    pub(super) fn standing_up(&self, body: usize) -> bool {
        self.bodies
            .get(body)
            .and_then(|state| state.mover.as_ref())
            .and_then(|mover| mover.down.as_ref())
            .is_some_and(Down::standing_up)
    }
}

/// Smooth in and out, from 0 to 1 as `t` goes from 0 to 1.
fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::SCENARIO_WIDTH;

    /// Two fighters, the second about to be knocked down as `how` says:
    /// knocked out by the first's cross with the knockout limit lowered under
    /// what a cross landed well gives (`ko`, 0.6 body heights apart), or
    /// shoved over backward (`back`), forward onto its knees (`front`), or
    /// flat on its face (`flat`); and the move.
    fn about_to_go_down(how: &str) -> (World, Move) {
        // Shoved over, a fighter needs room: far enough apart that a fall
        // forward does not land at the other's feet.
        let spacing = if how == "ko" { 0.60 } else { 1.40 };
        let mut world = create_side_fighters_at(Materials::default(), spacing);
        world.keep_journal();
        world.materials.knockout_turn = 10.0;
        let made = match how {
            "back" => Move::Shoved(220),
            "front" => Move::Shoved(-260),
            "flat" => Move::Shoved(-420),
            _ => Move::Cross,
        };
        (world, made)
    }

    /// A fighter shoved over backward, or forward onto its knees, gets up
    /// (from its back, and from its front) and stands in its stance again
    /// within a few seconds, on both feet, and nothing tears.
    #[test]
    fn a_fighter_shoved_over_gets_up() {
        for how in ["back", "front"] {
            let (mut world, made) = about_to_go_down(how);
            let dt = world.materials.fixed_dt;
            let rig = world.bodies[1].fighter.clone().unwrap();
            let (mut down_at, mut up_at) = (None, None);
            for step in 0..600 {
                let mut input = InputState::default();
                if step == 20 {
                    input.moves[0] = Some((1, made));
                }
                world.step(dt, &input, SCENARIO_WIDTH);
                match (down_at, world.fighter_fallen(1)) {
                    (None, true) => down_at = Some(step),
                    (Some(_), false) if up_at.is_none() => up_at = Some(step),
                    _ => {}
                }
            }
            let (Some(down), Some(up)) = (down_at, up_at) else {
                panic!(
                    "shoved over {how}, it went down on step {down_at:?} and got up on {up_at:?}"
                );
            };
            assert!(
                (up - down) as f64 * dt < 7.0 && !world.fighter_fallen(1),
                "shoved over {how}, it was down {:.1} s, and is down now: {}",
                (up - down) as f64 * dt,
                world.fighter_fallen(1)
            );
            assert!(
                rig.feet.iter().all(|&foot| world.foot_on_floor(foot)),
                "shoved over {how}, it got up and stands with a foot off the floor"
            );
            assert_eq!(
                (
                    world.stats.broken_skin,
                    world.stats.broken_muscle,
                    world.stats.broken_bone_joints,
                    world.stats.bone_joint_subluxations,
                    world.stats.fractured_bones
                ),
                (0, 0, 0, 0, 0),
                "shoved over {how}: torn skin, muscle, joints; joints slipped; bones broken"
            );
        }
    }

    /// Knocks the second of two fighters down and prints how it lies,
    /// comes to, and gets up, stage by stage (`cargo test --release --lib
    /// print_getting_up -- --ignored --nocapture`): `RP_DOWN=ko` (a
    /// knockout, the default), `back`, or `front` (a shove over that way),
    /// `RP_STEPS` steps (600), every `RP_EVERY`th (10).
    #[test]
    #[ignore]
    fn print_getting_up() {
        let how = std::env::var("RP_DOWN").unwrap_or_else(|_| "ko".to_string());
        let steps: usize = std::env::var("RP_STEPS")
            .ok()
            .and_then(|steps| steps.parse().ok())
            .unwrap_or(600);
        let every: usize = std::env::var("RP_EVERY")
            .ok()
            .and_then(|every| every.parse().ok())
            .unwrap_or(10)
            .max(1);
        let momentum = std::env::var("RP_MOMENTUM").ok().and_then(|range| {
            let (from, to) = range.split_once('-')?;
            Some(from.parse::<usize>().ok()?..=to.parse::<usize>().ok()?)
        });
        let (mut world, made) = about_to_go_down(&how);
        let dt = world.materials.fixed_dt;
        let rig = world.bodies[1].fighter.clone().unwrap();
        let cm = world.mm_per_px() * 0.1;
        for step in 0..steps {
            let mut input = InputState::default();
            let down_at = 20;
            if step == down_at {
                input.moves[0] = match made {
                    Move::Cross => Some((0, Move::Cross)),
                    shove => Some((1, shove)),
                };
            }
            // The fighter still standing steps back, as a referee sends it to
            // a neutral corner.
            if [80, 125, 170, 215].contains(&step) {
                input.moves[0] = Some((0, Move::StepOut));
            }
            // `RP_MOMENTUM=480-482` adds up, on those steps, how hard each
            // phase pushed the fighter's whole body across and up, in
            // newtons: pushes within the body add up to nothing, so what is
            // left is the floor's, gravity's, and the holds' that nothing
            // takes back.
            let traced = momentum.as_ref().is_some_and(|range| range.contains(&step));
            let mut masses = Vec::new();
            if traced {
                let mut followed = Vec::new();
                for (index, point) in world.points.iter().enumerate() {
                    if point.body == 1 && !point.pinned && !point.is_ghost() {
                        followed.push(crate::simulation::Traced::Point(index));
                        masses.push(point.mass);
                    }
                }
                for (index, bone) in world.bones.iter().enumerate() {
                    if bone.body == 1 {
                        followed.push(crate::simulation::Traced::Bone(index));
                        masses.push(bone.mass);
                    }
                }
                world.trace(&followed);
            }
            world.step(dt, &input, SCENARIO_WIDTH);
            if traced {
                let mut phases: Vec<(&str, Vec2)> = Vec::new();
                for stage in world.traced_stages() {
                    let change = scale(
                        subtract(stage.velocity_after, stage.velocity_before),
                        masses[stage.target],
                    );
                    match phases.iter_mut().find(|(phase, _)| *phase == stage.phase) {
                        Some(summed) => summed.1 = add(summed.1, change),
                        None => phases.push((stage.phase, change)),
                    }
                }
                let to_newtons = KG_PER_MASS * world.mm_per_px() * 0.001 / dt;
                let facing = -1.0;
                let mut line = format!("step {step:3}, pushes on the fighter's whole body:");
                for (phase, change) in phases {
                    let (across, up) = (facing * change.x * to_newtons, -change.y * to_newtons);
                    if across.abs() > 5.0 || up.abs() > 5.0 {
                        line.push_str(&format!(" {phase} ({across:+.0}, {up:+.0});"));
                    }
                }
                println!("{line}");
                world.trace(&[]);
            }
            if step % every != 0 {
                continue;
            }
            let up = |at: Vec2| (world.floor_y() - at.y) * cm;
            let (pelvis, skull) = (world.bones[rig.pelvis], world.bones[rig.skull]);
            let supports =
                world
                    .stances
                    .get(1)
                    .and_then(Option::as_ref)
                    .map_or(String::new(), |stance| {
                        format!(
                            "{} feet and {} more bear it, shares {:?}",
                            stance.feet.len(),
                            stance.others.len(),
                            stance
                                .shares
                                .iter()
                                .map(|share| (share * 100.0).round() / 100.0)
                                .collect::<Vec<f64>>()
                        )
                    });
            println!(
                "step {step:3}: hips {:3.0} cm up, head {:3.0}, fallen {}; {}; {}",
                up(pelvis.b),
                up(lerp(skull.a, skull.b, 0.5)),
                world.fighter_fallen(1),
                world.bodies[1]
                    .mover
                    .as_ref()
                    .and_then(|mover| mover.down.as_ref())
                    .map_or(String::from("standing"), Down::describe),
                supports,
            );
            let line = world.rise_line(1);
            if !line.is_empty() {
                println!("          {line}");
            } else if !world.fighter_fallen(1) || world.standing_up(1) {
                println!("          {}", world.mover_line(1));
            }
        }
        // `RP_STORY=1` prints what the journal kept: every injury, where,
        // and what did it.
        if std::env::var("RP_STORY").is_ok() {
            if let Some(journal) = world.journal() {
                for line in crate::inspect::story(&world, journal) {
                    println!("  {line}");
                }
            }
        }
        println!(
            "torn skin {}, muscle {}, joints {} torn and {} slipped, bones broken {}",
            world.stats.broken_skin,
            world.stats.broken_muscle,
            world.stats.broken_bone_joints,
            world.stats.bone_joint_subluxations,
            world.stats.fractured_bones
        );
    }
}

#[cfg(test)]
mod fights {
    use super::*;

    /// Plays random side-view fights `RP_FIGHTS` (24) long, `RP_SECONDS`
    /// (40) each, longer than the scenario tool's 20 so that a fighter going
    /// down late has time to get up, and prints for each how often each
    /// fighter went down and got up, how long it was down at the longest,
    /// whether it was still down at the end, and what tore (`cargo test
    /// --release --lib print_getting_up_in_random_fights -- --ignored
    /// --nocapture`).
    #[test]
    #[ignore]
    fn print_getting_up_in_random_fights() {
        let number = |name: &str, default: f64| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(default)
        };
        let (count, seconds) = (number("RP_FIGHTS", 24.0) as u64, number("RP_SECONDS", 40.0));
        // `RP_SEED=N` plays fight N alone and prints what each fighter was
        // doing each half second it was down, and what the journal kept.
        if let Ok(seed) = std::env::var("RP_SEED").map(|seed| seed.parse::<u64>().unwrap_or(1)) {
            let mut world = create_side_fighters(Materials::default());
            world.keep_journal();
            let dt = world.materials.fixed_dt;
            let mut moves = RandomMoves::new(seed);
            for step in 0..(seconds / dt) as usize {
                let mut input = InputState::default();
                for (body, made) in moves.next_step().into_iter().enumerate() {
                    input.moves[body] = made.map(|made| (body, made));
                }
                world.step(dt, &input, crate::scenarios::SCENARIO_WIDTH);
                for body in 0..2 {
                    if world.fighter_fallen(body) && step % 30 == 0 {
                        println!("step {step}: fighter {body}: {}", world.mover_line(body));
                    }
                }
            }
            if let Some(journal) = world.journal() {
                for line in crate::inspect::story(&world, journal) {
                    if !line.contains("bled under") && !line.contains("bruised") {
                        println!("  {line}");
                    }
                }
            }
            return;
        }
        let (mut downs, mut ups, mut stuck) = (0, 0, 0);
        for seed in 1..=count {
            let fight = play_side_fight(seed, seconds, Materials::default());
            let mut line = format!(
                "seed {seed:2}: torn joints {}, slipped {}, bones {}, skin {}, muscle {}",
                fight.torn_joints[0].len() + fight.torn_joints[1].len(),
                fight.slipped_joints[0].len() + fight.slipped_joints[1].len(),
                fight.bones_broken[0] + fight.bones_broken[1],
                fight.skin_torn[0] + fight.skin_torn[1],
                fight.muscle_torn
            );
            for (body, side) in ["left", "right"].iter().enumerate() {
                if fight.downs[body] == 0 {
                    continue;
                }
                downs += fight.downs[body];
                ups += fight.got_up[body];
                let still = fight.downs[body] > fight.got_up[body];
                if still {
                    stuck += 1;
                }
                line.push_str(&format!(
                    "; {side} down {} (knocked out first on step {:?}, fell {}), up {}, longest {:.1} s{}",
                    fight.downs[body],
                    fight.knocked_out[body],
                    fight.fell[body],
                    fight.got_up[body],
                    fight.longest_down[body],
                    if still { ", still down" } else { "" }
                ));
            }
            println!("{line}");
        }
        println!("went down {downs} times, got up {ups}; {stuck} still down at the end");
    }
}
