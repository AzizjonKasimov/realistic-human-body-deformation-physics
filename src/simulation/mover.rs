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
use super::sides::BonePiece;
use super::soft::{perpendicular, KG_PER_MASS};
use super::tools::{FistContact, FistLanding, Spared};
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
    /// Blocking, for as long as it is held: the forearms close in front of
    /// the face and stop a punch, which otherwise goes through the hands
    /// ([`World::stands`]).
    Block(bool),
    /// Not a move the fighter makes but a shove it takes: a push on its
    /// trunk over a tenth of a second from outside, of so many newton
    /// seconds, back toward its rear foot (negative, forward), as a person
    /// is pushed in balance experiments (12 to 56 N s; McGreavy et al. 2020).
    /// For films and tests of how a fighter holds, steps, or falls.
    Shoved(i32),
}

/// How often the stance bounces, in hertz: a boxer on the balls of the feet
/// bounces about twice a second.
const BOUNCE_HERTZ: f64 = 2.1;
/// How far the hips rise and fall with the bounce, and sway forward and
/// back with every other bounce, in body heights.
const BOUNCE_RISE: f64 = 0.004;
const BOUNCE_SWAY: f64 = 0.004;

/// How far a step carries the fighter, in body heights (about 23 cm), and
/// how long its hips take to get there, in seconds: about two thirds of the
/// way by the time the lead foot comes down, 0.37 s on, since the lead leg
/// reaches only about 11 cm past where the stance has it. Moved a fifth of
/// the way by then, the hips left the lead foot out of reach of the floor.
const STEP_LENGTH: f64 = 0.13;
const STEP_SECONDS: f64 = 0.6;
/// How high a stepping foot lifts, in body heights: a boxer's feet skim the
/// floor.
const STEP_LIFT: f64 = 0.020;
/// The nearest two fighters stand to each other, hips to hips, in body
/// heights (89 cm): a step toward the other carries a fighter only as far as
/// that. A straight punch still has its room there, a cross landing at 8 m/s.
/// Boxers do fight nearer, with hooks and uppercuts, which the fighters do
/// not have yet; walked in on each other with only straight punches, their
/// arms and trunks stood in each other's space, with their hips as near as
/// 36 cm, and of the 116 things one random fight tore or put out, 110 went
/// with the hips nearer than 90 cm.
const NEAREST_STAND: f64 = 0.50;
/// The shortest step a fighter takes, in body heights (5 cm): with less room
/// than that left before the other fighter, it stays where it stands.
const SHORTEST_STEP: f64 = 0.03;

/// How far in front of its head a fighter's guard reaches, in body heights:
/// to its lead fist (`profile::guard_fists`). Two fighters' guards do not
/// reach past each other (`Mover::crowded`).
const GUARD_REACH: f64 = 0.168;

/// How near two parts' bones come, as a share of the flesh both carry,
/// before the parts count as lying across each other
/// (`World::note_passing_parts`): three quarters of it, a quarter of their
/// flesh inside each other, where meeting all at once throws them apart.
const PASSING_DEPTH: f64 = 0.75;

/// A punch's timing, in seconds: how long the fist takes out, how long it
/// stays there, and how long it takes back. Trained boxers' fists went from
/// setting off to landing in about 0.10 to 0.22 s (a professional's cross
/// 0.10 s, Atha et al. 1985; crosses within combinations 0.13 s, Whiting et
/// al. 1988; single hardest crosses 0.22 +/- 0.10 s, Piorkowski et al.
/// 2011), and these times bring the fists in at trained boxers' speeds: a
/// cross at about 9 m/s (Olympic boxers 9.1 +/- 2.1, Walilko et al. 2005;
/// 8.2 +/- 1.1, Piorkowski et al. 2011; 8.9 for a professional, Atha et al.
/// 1985) and a jab a little under it (boxers' jabs were about as fast as
/// their crosses, Kimm and Thiel 2015). `docs/INJURY_REFERENCE.md`, Punches.
const JAB_SECONDS: (f64, f64, f64) = (0.11, 0.02, 0.20);
const CROSS_SECONDS: (f64, f64, f64) = (0.15, 0.03, 0.28);
/// How much of the arm's full length a punch reaches at most: the elbow
/// stays a little bent, as a boxer keeps it.
const STRIKE_REACH: f64 = 0.975;
/// How long a fist that has landed goes on driving into what it hit before
/// it is pulled back, in seconds: bare fists stayed in contact with a pad
/// for 27 ms (17-41; Adamec et al. 2021). Pulled back on the step it landed,
/// a cross arriving at 9.6 m/s left the head moving at 0.2 to 1.2 m/s, where
/// 2.9 kg at 9.1 m/s (Walilko et al. 2005) shared with a head of about 4.6 kg
/// leaves it at about 3.5.
const FOLLOW_THROUGH_SECONDS: f64 = 0.03;

/// How long a fist's push on what it struck lasts, in seconds, rising and
/// falling as half a sine wave (`World::land_fist_blows`): a punch's force
/// on a head spikes over about 8 to 10 ms (80 g blows over about 8 ms; 10 ms
/// in models of punch force), though the fist stays against it longer.
/// Passed on evenly over the 30 ms the fist stays in contact, a cross gave
/// the head the speed Walilko, Viano and Bir (2005) measured (2.9 m/s) but
/// spun it at 768 rad/s^2 and threw it at 10 g, where Olympic boxers'
/// straight punches spun a dummy's head at 6,343 and threw it at 58.
const BLOW_SECONDS: f64 = 0.008;

/// How firmly a fighter's hips are carried across and upward, in hertz on
/// the body's weight, and the hardest the hold pushes each end of the pelvis
/// each way, in body weights: half of what feet gripping a dry floor can
/// push sideways with (a friction coefficient near 1 on the whole body's
/// weight), and half of the two and a half body weights legs bear.
const CARRY_HERTZ: Vec2 = Vec2 { x: 2.0, y: 6.0 };
const CARRY_LIMIT: Vec2 = Vec2 { x: 0.5, y: 1.25 };

/// How firmly a fighter's hips are kept tilted as its moves have them, in
/// hertz on the whole body's turning about its hips: an estimate, between
/// how firmly the hips are carried across and upward, firm enough that a
/// deep duck's weight tips the body a degree or two and soft enough that a
/// hard cross on the head rocks it back a few. And the hardest the legs turn
/// the hips, in newton metres: with the strength of both hips' muscles
/// (`fighter.rs`, `hinge_drive`), which are what turns a standing person's
/// pelvis on its legs.
const CARRY_TILT_HERTZ: f64 = 3.0;
const CARRY_TILT_LIMIT: f64 = 2.0 * fighter::HIP_STRENGTH;

/// How deep a fighter's knuckles sink into what they hit, in metres: as far
/// as the flesh over bone gives, an estimate, the flesh of a face being a
/// centimetre or two thick. The bare fist of the sandbox is carried through
/// its whole step, 15 cm at a cross's speed, as a tool in the hand is, and
/// shoves and drags the skin all that way: a cross that happened to reach
/// the face as a step began split its skin (5 of 42 crosses).
const FIST_SINKS_METRES: f64 = 0.02;

/// How quickly the arms close into a block and open again, in hertz: they
/// are most of the way in a tenth of a second.
const BLOCK_HERTZ: f64 = 5.0;
/// How far closed a block is, from 0 to 1, once its arms stand in a punch's
/// way.
const BLOCK_CLOSED: f64 = 0.5;

/// How much of the bare fist's weight is behind a jab (`World::land_fist`).
/// Elite boxers' lead hand lands with 0.59 of their rear hand's force, 2,847
/// against 4,800 N (Smith et al. 2000), at about the same hand speed (Kimm
/// and Thiel 2015); a blow's force goes with its speed and the square root
/// of the weight behind it (`cover.rs`), so a jab at eight ninths of a
/// cross's speed has 0.45 of its weight behind it.
const JAB_WEIGHT: f64 = 0.45;

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
        self.ease_at_most(quickness, f64::INFINITY, dt);
    }

    /// As [`Held::ease`], but speeding `amount` up toward 1 by no more than
    /// `most` a second each second.
    fn ease_at_most(&mut self, quickness: f64, most: f64, dt: f64) {
        let omega = 2.0 * PI * quickness;
        let target = if self.held { 1.0 } else { 0.0 };
        let pull = (omega * omega * (target - self.amount) - 2.0 * omega * self.speed).min(most);
        self.speed += pull * dt;
        self.amount += self.speed * dt;
        if !self.held && self.amount.abs() < 1.0e-4 && self.speed.abs() < 1.0e-3 {
            self.amount = 0.0;
            self.speed = 0.0;
        }
    }
}

/// A step being taken: how far it carries the fighter, in body heights
/// (forward positive), how long ago it began, and when each foot came down
/// on the floor, the one that went first first.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Step {
    /// How far each foot goes: as far as the step is told to go until the
    /// first foot is down, then as far as that foot really went, so that the
    /// other foot keeps the stance's length behind it and the hips go to
    /// where the stance has them over both. Which way it goes stays
    /// `forward`'s.
    by: f64,
    forward: bool,
    /// Taken to catch the body as its weight runs past its feet: the first
    /// foot hands its weight over at once, the body already falling the way
    /// the step goes.
    catching: bool,
    /// Where the hips' way begins, in body heights on from where the stance
    /// has them: where they already are for a step that catches the body.
    /// Begun from where the stance had them, a step catching a fighter shoved
    /// forward pulled its hips back 17 cm against their run, and it rebounded
    /// into a second step 41 cm back.
    hips_from: f64,
    age: f64,
    /// A foot's swing ends as it strikes the floor, as in SIMBICON (Yin,
    /// Loken and van de Panne 2007) and Coros, Beaudoin and van de Panne
    /// (2010), or a moment after its time runs out; the other foot hands its
    /// weight over and lifts only once the first is down, so that a foot
    /// always stands. Lifted together for a tenth of the step, both feet were
    /// off the floor at once, and nothing held the fighter up.
    down: [Option<f64>; 2],
}

/// How long the first foot takes to hand its weight to the other foot
/// before it lifts, while the body begins to fall the way the step goes, in
/// seconds: a person's first step begins so, the pressure under the feet
/// moving back to push the body forward before the stepping foot lifts (the
/// postural phase of gait initiation). Lifted after 0.05 s, the lead foot
/// swung forward while the body stood still, and swinging it pushed the
/// hips back. How long the second foot takes, and how long a foot takes its
/// share back once it is down: its push on the floor is brought to nothing
/// before it lets go (`balance.rs`).
const LEAN_SECONDS: f64 = 0.12;
const UNLOAD_SECONDS: f64 = 0.05;
/// How long the first foot and then the second swing, in seconds, and how
/// much longer a foot still in the air is carried down before it is set
/// down regardless.
const SWING_SECONDS: [f64; 2] = [0.25, 0.20];
const LANDING_GRACE_SECONDS: f64 = 0.08;
/// How near where the step sets it down a swinging foot must be, in body
/// heights (1 cm), to be set down as it touches the floor: a short step of
/// 9 cm, set down 3 cm short, went a third less far.
const LANDING_NEAR: f64 = 0.006;

/// How far past where a fighter's weight is running a step that catches it
/// puts the foot, in body heights (5 cm), and the farthest such a step
/// goes, in body heights (52 cm): about the 0.6 leg lengths Coros, Beaudoin
/// and van de Panne (2010) take as a step's longest, short of the 78% of
/// their height a young adult steps out on purpose.
const CATCH_PAST: f64 = 0.03;
const FARTHEST_STEP: f64 = 0.30;

/// Where along the other fighter's skull a punch aims, from its base (0) to
/// its crown (1): at the height of the jaw, as boxers aim. Strikes to the
/// jaw ended 54% of UFC knockouts (Hutchison et al. 2014), and on a dummy's
/// head a blow off its centre turned it about twice as hard as one through
/// it (McIntosh and Patton 2015). Aimed at the middle of the head (0.45), a
/// cross landed well turned it at about 2,300 rad/s^2, its turning changing
/// by 15 rad/s within the blow; at the jaw, at about 3,100 and by 20. Aimed
/// lower still, at the chin's point, it turned the head no harder and met
/// the hands of the guard more often.
const AIM_UP_THE_SKULL: f64 = 0.0;

/// How far a duck lowers the hips, in body heights (18 cm).
const DUCK_DROP: f64 = 0.100;

/// How fast a duck sets the hips falling, at the most, as a share of how
/// fast things fall: a body drops by its legs giving way under it, the floor
/// pushing back less than its weight, and in a countermovement jump's
/// unweighting the floor's push falls to about a fifth of the body's weight
/// (McMahon, Suchomel, Lake and Comfort 2018). Eased in as a spring that
/// pulls hardest at once, a duck asked the hips to fall at 4 g, and the legs,
/// folding faster than the body could drop, pulled both feet off the floor
/// for a tenth of a second; stepping back so, a fighter fell.
const DUCK_FALL: f64 = 0.8;

/// How long a shove from outside pushes (`Move::Shoved`), in seconds.
const SHOVE_SECONDS: f64 = 0.1;

/// How high above the floor a fighter's hips are, at the least, while its
/// legs still bear it, in body heights (44 cm): a duck takes them down to
/// about 0.40, and below this it has fallen.
const FALLEN_HIPS: f64 = 0.25;

impl Step {
    fn new(by: f64) -> Self {
        Self {
            by,
            forward: by > 0.0,
            catching: false,
            hips_from: 0.0,
            age: 0.0,
            down: [None; 2],
        }
    }

    /// A step `by` body heights long taken to catch the body (`catching`),
    /// its hips `hips_from` body heights on from where the stance has them.
    fn catching(by: f64, hips_from: f64) -> Self {
        Self {
            catching: true,
            hips_from,
            ..Self::new(by)
        }
    }

    /// The foot that goes first, the lead's 0: the one on the side the step
    /// goes to.
    fn first(&self) -> usize {
        if self.forward {
            0
        } else {
            1
        }
    }

    /// The first foot is down, `went` body heights on from where it stood:
    /// the step goes as far, no farther than it was told and never back.
    fn went(&mut self, went: f64) {
        self.by = if self.forward {
            went.clamp(0.0, self.by)
        } else {
            went.clamp(self.by, 0.0)
        };
    }

    /// The side of the foot that goes `order`-th, 0 first.
    fn side(&self, order: usize) -> usize {
        if order == 0 {
            self.first()
        } else {
            1 - self.first()
        }
    }

    /// When the foot that goes `order`-th begins to hand its weight over:
    /// the first at once, the second once the first is down.
    fn starts(&self, order: usize) -> Option<f64> {
        if order == 0 {
            Some(0.0)
        } else {
            self.down[0]
        }
    }

    /// How long the foot that goes `order`-th takes to hand its weight over.
    fn handing(&self, order: usize) -> f64 {
        if order == 0 && !self.catching {
            LEAN_SECONDS
        } else {
            UNLOAD_SECONDS
        }
    }

    /// When the foot that goes `order`-th lifts, if it has begun to hand
    /// its weight over.
    fn lifts(&self, order: usize) -> Option<f64> {
        self.starts(order).map(|start| start + self.handing(order))
    }

    /// Whether the foot that goes `order`-th is in the air: its weight
    /// handed over and not yet down.
    fn swinging(&self, order: usize) -> bool {
        self.down[order].is_none() && self.lifts(order).is_some_and(|lifts| self.age >= lifts)
    }

    /// How far through its swing the foot that goes `order`-th is, from 0
    /// on the floor where it stood to 1 over where it lands.
    fn swung(&self, order: usize) -> f64 {
        match (self.down[order], self.lifts(order)) {
            (Some(_), _) => 1.0,
            (None, Some(lifts)) => smooth((self.age - lifts) / SWING_SECONDS[order]),
            (None, None) => 0.0,
        }
    }

    /// Whether the foot that goes `order`-th, in the air, is to be set down
    /// now: on the floor at its swing's end, `near` where it lands, or its
    /// time and a moment more gone. Set down as soon as it grazed the floor,
    /// still moving at 1.4 m/s, a foot slid on 10 cm past where it was meant
    /// to land; carried on instead, it is slowed there.
    fn lands(&self, order: usize, on_floor: bool, near: bool) -> bool {
        let Some(lifts) = self.lifts(order).filter(|_| self.swinging(order)) else {
            return false;
        };
        let late = self.age >= lifts + SWING_SECONDS[order] + LANDING_GRACE_SECONDS;
        late || (on_floor && near && self.swung(order) >= 0.9)
    }

    /// How far through its swing each foot is, the lead foot's first, and
    /// how far the hips have gone.
    fn progress(&self) -> ([f64; 2], f64) {
        let mut feet = [0.0; 2];
        for order in 0..2 {
            feet[self.side(order)] = self.swung(order);
        }
        (feet, smooth(self.age / STEP_SECONDS))
    }

    /// Which feet are in the air, the lead foot's first.
    fn lifted(&self) -> [bool; 2] {
        let mut lifted = [false; 2];
        for order in 0..2 {
            lifted[self.side(order)] = self.swinging(order);
        }
        lifted
    }

    /// How much of its share each foot bears, the lead's first: all of it
    /// standing, handed to the other foot before it lifts, and taken back
    /// once it is down.
    fn loads(&self) -> [f64; 2] {
        let mut loads = [1.0; 2];
        for order in 0..2 {
            loads[self.side(order)] = match (self.down[order], self.starts(order)) {
                (Some(down), _) => smooth((self.age - down) / UNLOAD_SECONDS),
                (None, Some(start)) => 1.0 - smooth((self.age - start) / self.handing(order)),
                (None, None) => 1.0,
            };
        }
        loads
    }

    fn done(&self) -> bool {
        self.age >= STEP_SECONDS
            && self.down[1].is_some_and(|down| self.age >= down + UNLOAD_SECONDS)
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
    /// When its fist first landed on something, if it has.
    landed: Option<f64>,
    /// When it turned back before reaching its full length, its fist having
    /// landed or its arm having been stopped, how far out the punch aimed
    /// then, and how far out the fist had really got: the body comes back
    /// from where the punch had taken it, and the fist from where it is.
    turned: Option<(f64, f64, f64)>,
}

impl Strike {
    fn seconds(&self) -> (f64, f64, f64) {
        match self.hand {
            Side::Lead => JAB_SECONDS,
            Side::Rear => CROSS_SECONDS,
        }
    }

    /// How far into the punch the body is, from 0 in the stance to 1 with
    /// the fist at the end of its way: the trunk's turn, the shoulders, the
    /// hips. Turned back, it comes back smoothly from as far as it had gone.
    fn reach(&self) -> f64 {
        let (out, stay, back) = self.seconds();
        match self.turned {
            Some((age, reach, _)) => reach * (1.0 - smooth((self.age - age) / back)),
            None => smooth(self.age / out) - smooth((self.age - out - stay) / back),
        }
    }

    /// How far out the fist aims, from 0 in the guard to 1 at the end of its
    /// way: as far as the body is into the punch, or, turned back, coming
    /// back from where the fist had really got. A fist held up by what it
    /// hit comes back from where it is, not from where it would have been;
    /// the body, brought back from there too, was jerked back as far in one
    /// step, the shoulder a cross carries forward with it, and the shoulder
    /// tore (10 of 46 blocked crosses).
    fn fist_reach(&self) -> f64 {
        let (_, _, back) = self.seconds();
        match self.turned {
            Some((age, _, fist)) => fist * (1.0 - smooth((self.age - age) / back)),
            None => self.reach(),
        }
    }

    /// Still on its way out, not yet coming back.
    fn going_out(&self) -> bool {
        self.turned.is_none() && self.age < self.seconds().0
    }

    fn done(&self) -> bool {
        let (out, stay, back) = self.seconds();
        match self.turned {
            Some((age, ..)) => self.age >= age + back,
            None => self.age >= out + stay + back,
        }
    }

    /// The punch turns back from `reached` of its way out, where its fist
    /// has got to, or from as far as it aims if that is nearer: a fist held
    /// up by what it hit comes back from where it is, not from where it
    /// would have been.
    fn turn_back(&mut self, reached: f64) {
        if self.going_out() {
            let aimed = self.reach();
            self.turned = Some((self.age, aimed, reached.clamp(0.0, aimed)));
        }
    }

    /// The fist has landed: it drives on for as long as a fist stays in
    /// contact (`FOLLOW_THROUGH_SECONDS`) and then turns back.
    fn land(&mut self) {
        if self.going_out() && self.landed.is_none() {
            self.landed = Some(self.age);
        }
    }

    /// Whether a fist that landed has driven on for as long as it stays in
    /// contact, and should now come back.
    fn followed_through(&self) -> bool {
        self.going_out()
            && self
                .landed
                .is_some_and(|landed| self.age >= landed + FOLLOW_THROUGH_SECONDS)
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
    /// Its weight ran past where any step could catch it, or a blow knocked
    /// it out (`knockout.rs`): it no longer stands up on its legs or takes
    /// moves, and stays down.
    fallen: bool,
    /// A shove it is taking (`Move::Shoved`): how hard, in newton seconds,
    /// and how long ago it began.
    shoved: Option<(f64, f64)>,
    strike: Option<Strike>,
    lean: Held,
    duck: Held,
    block: Held,
    /// How far it sits back on its hips, in body heights, to keep its guard
    /// from reaching past the other fighter's (`World::drive_movers`): a
    /// fighter that leans or ducks toward one standing close goes down
    /// where it stands instead of into the other. Leaning in regardless,
    /// two fighters ducking at the nearest they stand had their heads 30 cm
    /// apart and their guards a forearm's length into each other, and the
    /// muscles holding those poses ground arm on arm until skin split.
    crowded: f64,
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
            fallen: false,
            shoved: None,
            strike: None,
            lean: Held::default(),
            duck: Held::default(),
            block: Held::default(),
            crowded: 0.0,
            last_turns: None,
            last_hips: None,
        }
    }

    /// Knocked out (`knockout.rs`): it falls, the step and the punch it was
    /// making stop with its muscles, and it stays down.
    pub(super) fn knock_down(&mut self) {
        self.fallen = true;
        self.step = None;
        self.strike = None;
    }

    /// The pose the fighter is asked to take now.
    fn pose(&self) -> Pose {
        self.pose_and_strike_line().0
    }

    /// How far forward of where it was built the front of its guard would
    /// stand, in body heights, by where it stands and how it leans, ducks,
    /// and blocks alone: without the punch it may be throwing, which is
    /// meant to reach the other fighter, and before it sits back for the
    /// other fighter (`crowded`).
    fn guard_front(&self) -> f64 {
        let still = Mover {
            strike: None,
            crowded: 0.0,
            ..self.clone()
        };
        Joints::of(&still.pose()).head().0 + GUARD_REACH
    }

    /// The pose the fighter is asked to take now, and the line its punching
    /// fist is thrown along, if it is throwing a punch: from where the fist
    /// stood in its guard to where the punch ends.
    fn pose_and_strike_line(&self) -> (Pose, Option<(Place, Place)>) {
        let mut pose = profile::stance();
        let turn = 2.0 * PI * BOUNCE_HERTZ * self.clock;
        let (bounce, sway) = (libm::sin(turn), libm::sin(turn * 0.5));
        // The stance, alive: a small bounce and a slower sway; sat back on
        // its hips as far as the other fighter crowds it.
        pose.hips.0 += self.place + BOUNCE_SWAY * sway - self.crowded;
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
            pose.hips.0 += step.hips_from + (step.by - step.hips_from) * hips;
        }
        // Leaning back: the weight goes over the back leg, whose heel comes
        // down to bear it, and the trunk away from the punch, the chin
        // tucked. On the ball of the rear foot alone, its heel still raised
        // as the stance has it, the weight had only that ball's width behind
        // it, and a fighter carried by its own legs tipped over backward.
        let lean = self.lean.amount;
        pose.feet[1].pitch *= 1.0 - lean;
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
        pose.hips.1 -= DUCK_DROP * duck;
        pose.pelvis += 0.16 * duck;
        pose.lumbar += 0.28 * duck;
        pose.thorax += 0.40 * duck;
        pose.neck += 0.26 * duck;
        pose.head += 0.02 * duck;
        // Blocking: the chin tucks behind the forearms and the back rounds.
        let block = self.block.amount;
        pose.hips.1 -= 0.006 * block;
        pose.thorax += 0.05 * block;
        pose.neck += 0.06 * block;
        pose.head += 0.08 * block;
        // The weight stays where the stance and the steps put the hips,
        // however a duck, a lean, or a block bends the trunk; a punch, below,
        // carries it on into the blow, as a cross does.
        profile::keep_the_weight(&mut pose);
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
                    pose.shoulders[1] += 0.135 * reach;
                    pose.shoulders[0] -= 0.055 * reach;
                    pose.lumbar += 0.03 * reach;
                    pose.thorax += 0.08 * reach;
                    pose.hips.0 += 0.045 * reach;
                    pose.hips.1 -= 0.010 * reach;
                    pose.feet[1].pitch += 0.38 * reach;
                }
            }
        }
        // The fists stand by the head, wherever the trunk has taken it, and
        // close in front of the face in a block.
        let joints = Joints::of(&pose);
        let head = joints.head();
        pose.fists = profile::guard_fists(head);
        for (fist, closed) in pose.fists.iter_mut().zip(profile::block_fists(head)) {
            fist.0 += (closed.0 - fist.0) * block;
            fist.1 += (closed.1 - fist.1) * block;
        }
        let mut line_thrown = None;
        if let Some(strike) = &self.strike {
            let side = strike.hand.index();
            // A punch is thrown through what it aims at, to the arm's reach
            // with the elbow a little bent, so the fist meets it on the way,
            // still fast: aimed to stop at the head, it would arrive as it
            // came to rest.
            let shoulder = joints.shoulder[side];
            let line = (strike.target.0 - shoulder.0, strike.target.1 - shoulder.1);
            let far = libm::hypot(line.0, line.1).max(1.0e-9);
            let most = STRIKE_REACH * (profile::UPPER_ARM + profile::FOREARM + profile::HAND);
            let target = (
                shoulder.0 + line.0 * most / far,
                shoulder.1 + line.1 * most / far,
            );
            let guard = pose.fists[side];
            line_thrown = Some((guard, target));
            let out = strike.fist_reach();
            pose.fists[side] = (
                guard.0 + (target.0 - guard.0) * out,
                guard.1 + (target.1 - guard.1) * out,
            );
            if strike.hand == Side::Rear {
                // The lead hand comes back to the chin as the rear goes out.
                pose.fists[0].0 -= 0.050 * reach;
                pose.fists[0].1 -= 0.008 * reach;
            }
        }
        (pose, line_thrown)
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

/// A fist a fighter has landed, sharing its momentum with what it landed on
/// over the time a fist stays in contact (`World::land_fist_blows`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FistBlow {
    /// The fighter that threw it, the side of its punching arm, and that
    /// arm's hand, whose knuckles the push acts at.
    body: usize,
    side: Side,
    hand: usize,
    /// Where its knuckles landed, the way the fist was going, and how fast,
    /// in pixels a second.
    at: Vec2,
    heading: Vec2,
    speed: f64,
    /// The weight behind the fist, in the engine's unit of mass.
    mass: f64,
    /// The momentum its landing already gave what it struck, along its way
    /// (`FistLanding::given`).
    given: f64,
    /// What the fist passes without touching.
    spares: Spared,
    /// Once the pieces it landed on are measured: the bone struck, the
    /// momentum the fist passes on, and how long it has been passing it.
    passing: Option<(usize, f64, f64)>,
}

/// Where the flesh and bones of a blocking arm lay before a fist landed on
/// it (`World::brace_arm`): each point's place, and each bone's two ends.
struct ArmBefore {
    points: Vec<(usize, Vec2)>,
    bones: Vec<(usize, Vec2, Vec2)>,
}

/// How a part of a body stands to another body's punches and limbs. Seen
/// from the side, two fighters' arms cross the same space on the screen
/// without meeting: a fighter's hands in its stance are held one each side
/// of the line a straight punch comes down, so the punch goes between them,
/// through the hands as the side view shows it, and only arms closed in
/// front of the face in a block are in its way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stands {
    /// In the way: a head, a trunk, a leg, or any part of a body that is
    /// not a side-view fighter.
    Body,
    /// A fighter's arm held beside it, out of the other's way.
    Beside,
    /// A fighter's arm throwing a punch, out or back.
    Striking,
    /// A fighter's arm closed in front of it in a block.
    Guarding,
    /// A fighter's leg: in the way of the other fighter's trunk and arms,
    /// but not of its legs, which stand to one side of it. A boxer stands
    /// with its feet about shoulder-width apart across, so two facing each
    /// other, each leading with the same foot, have their lead feet either
    /// side of the line between them; met leg to leg in the side view, two
    /// fighters stepping in to the nearest they stand stubbed their lead
    /// feet on each other's and stood 5 cm short.
    Leg,
}

/// How much of the bare fist's weight is behind a punch thrown with the arm
/// on `side`: all of it behind the rear hand's cross, less behind a jab.
fn punch_weight(side: Side) -> f64 {
    match side {
        Side::Lead => JAB_WEIGHT,
        Side::Rear => 1.0,
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
        // How hard the whole body is to turn about its hips, as it stands.
        let Some(hips) = self.bones.get(rig.pelvis).map(|pelvis| pelvis.b) else {
            return;
        };
        let about_hips = |at: Vec2| {
            let from = subtract(at, hips);
            dot(from, from)
        };
        let flesh: f64 = self
            .points
            .iter()
            .filter(|point| point.body as usize == body && !point.pinned && !point.is_ghost())
            .map(|point| point.mass * about_hips(point.position))
            .sum();
        let bones: f64 = self
            .bones
            .iter()
            .filter(|bone| bone.body as usize == body)
            .map(|bone| bone.mass * 0.5 * (about_hips(bone.a) + about_hips(bone.b)))
            .sum();
        // Where the stance as built puts each end of the hips from the
        // ankles, which legs that carry the hips hold them over as it
        // settles, before any move does.
        let from_ankles = self.fighter_frame(body).map(|frame| {
            let joints = Joints::of(&profile::stance());
            let ends = self.pelvis_ends(&frame, &joints);
            let ankles = joints.ankle.map(|ankle| frame.world(ankle));
            ends.map(|end| ankles.map(|ankle| subtract(end, ankle)))
        });
        for hold in &mut self.hip_holds {
            if hold.bone == rig.pelvis {
                hold.carry = Some(Carry {
                    offset: Vec2::default(),
                    speed: Vec2::default(),
                    from_ankles: from_ankles
                        .map_or([Vec2::default(); 2], |from| from[usize::from(hold.end_b)]),
                    hertz: CARRY_HERTZ,
                    limit: scale(CARRY_LIMIT, tissue::BODY_MASS_KG * STANDARD_GRAVITY),
                    tilt_hertz: CARRY_TILT_HERTZ,
                    tilt_inertia: flesh + bones,
                    tilt_limit: CARRY_TILT_LIMIT,
                    tilt_impulse: 0.0,
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

    /// How sheet `sheet` of body `body`'s flesh stands to another body
    /// ([`Stands`]).
    pub(super) fn stands(&self, body: u8, sheet: u8) -> Stands {
        let Some(state) = self.bodies.get(body as usize) else {
            return Stands::Body;
        };
        let (Some(_), Some(mover)) = (&state.fighter, &state.mover) else {
            return Stands::Body;
        };
        if fighter::leg_sheet(sheet) {
            return Stands::Leg;
        }
        let Some(side) = fighter::arm_of_sheet(sheet) else {
            return Stands::Body;
        };
        if mover.strike.is_some_and(|strike| strike.hand == side) {
            Stands::Striking
        } else if mover.block.amount >= BLOCK_CLOSED {
            Stands::Guarding
        } else {
            Stands::Beside
        }
    }

    /// Whether two parts of two different bodies, each a body and a sheet
    /// of its flesh, would be in each other's way by what each is doing:
    /// neither is an arm held beside its fighter, and they are not two
    /// punching arms, which pass each other as two straight punches thrown
    /// together do.
    fn stand_in_the_way(&self, a: (u8, u8), b: (u8, u8)) -> bool {
        let (a, b) = (self.stands(a.0, a.1), self.stands(b.0, b.1));
        a != Stands::Beside
            && b != Stands::Beside
            && !(a == Stands::Striking && b == Stands::Striking)
            && !(a == Stands::Leg && b == Stands::Leg)
    }

    /// Whether two parts of two different bodies, each a body and a sheet
    /// of its flesh, are in each other's way: they would be by what each is
    /// doing (`stand_in_the_way`), and they are not passing each other
    /// already (`note_passing_parts`).
    pub(super) fn parts_meet(&self, a: (u8, u8), b: (u8, u8)) -> bool {
        self.stand_in_the_way(a, b) && !self.passing(a, b)
    }

    /// Whether the parts `a` and `b` of two bodies are passing each other
    /// (`note_passing_parts`).
    fn passing(&self, a: (u8, u8), b: (u8, u8)) -> bool {
        !self.passing_parts.is_empty()
            && self
                .passing_parts
                .contains(&if a <= b { [a, b] } else { [b, a] })
    }

    /// Notes, as a step begins, which parts of two side-view fighters are
    /// passing each other: an arm that lies across a part of the other
    /// fighter while it is not in that part's way (held beside its fighter,
    /// or one of two punching arms) goes on passing it until the two have
    /// come clear of each other, whatever either does meanwhile. Seen from
    /// the side, arms out of each other's way cross the same space on the
    /// screen; an arm that began to punch, or closed into a block, while it
    /// lay across the other fighter's arm was all at once deep inside what
    /// it now met, and was thrown out of it: two fighters ducking toward
    /// each other at the nearest they stand, one's jab set off with its arm
    /// across the other's block, and the block's forearm was pressed with a
    /// thousand newtons within a step, its flesh pulled 10 cm off the bone,
    /// and its skin split.
    ///
    /// Two parts lie across each other once a bone of one comes nearer a
    /// bone of the other than three quarters of the flesh both carry
    /// (`PASSING_DEPTH`), and are clear of each other again once none is
    /// nearer than all of it. Counted as across each other from where their
    /// skins only touch, a jab thrown from the nearest two fighters stand,
    /// its fist a hand's breadth from the block in front of it, passed the
    /// block and landed on the head behind (4 of 46 blocked jabs).
    pub(super) fn note_passing_parts(&mut self) {
        if !self.bodies.iter().any(|state| state.fighter.is_some()) {
            return;
        }
        // Each fighter's bones by the sheet of flesh they lie in.
        let mut parts: Vec<((u8, u8), Vec<usize>)> = Vec::new();
        for (index, bone) in self.bones.iter().enumerate() {
            let fighter = self
                .bodies
                .get(bone.body as usize)
                .is_some_and(|state| state.fighter.is_some());
            if !fighter || bone.splinter || bone.pinned {
                continue;
            }
            let part = (bone.body, bone.sheet);
            match parts.iter_mut().find(|(known, _)| *known == part) {
                Some((_, bones)) => bones.push(index),
                None => parts.push((part, vec![index])),
            }
        }
        parts.sort_by_key(|(part, _)| *part);
        let mut passing = std::mem::take(&mut self.passing_parts);
        for (slot, (a, a_bones)) in parts.iter().enumerate() {
            for (b, b_bones) in &parts[slot + 1..] {
                let arms =
                    fighter::arm_of_sheet(a.1).is_some() || fighter::arm_of_sheet(b.1).is_some();
                if a.0 == b.0 || !arms {
                    continue;
                }
                // How near their bones come, as a share of the flesh both
                // carry.
                let nearest = a_bones
                    .iter()
                    .flat_map(|&first| b_bones.iter().map(move |&second| (first, second)))
                    .map(|(first, second)| {
                        let (p, q) = (self.bones[first], self.bones[second]);
                        closest_segment_points(p.a, p.b, q.a, q.b).distance
                            / (p.flesh + q.flesh).max(EPSILON)
                    })
                    .fold(f64::INFINITY, f64::min);
                let pair = [*a, *b];
                let known = passing.iter().position(|other| *other == pair);
                match known {
                    Some(at) if nearest >= 1.0 => {
                        passing.remove(at);
                    }
                    None if nearest < PASSING_DEPTH && !self.stand_in_the_way(*a, *b) => {
                        passing.push(pair)
                    }
                    _ => {}
                }
            }
        }
        self.passing_parts = passing;
    }

    /// What a fist fighter `body` throws with the arm on `side` leaves
    /// alone as it lands: its own body, the other bodies' arms that are not
    /// closed in a block, and whatever that arm's forearm and hand are
    /// passing already (`note_passing_parts`).
    fn fist_spares(&self, body: usize, side: Side) -> Spared {
        let mut spared = Spared::body(body as u8);
        let fist = (body as u8, fighter::forearm_sheet(side));
        for other in (0..self.bodies.len()).filter(|&other| other != body) {
            for sheet in 0..fighter::SHEETS as u8 {
                if matches!(
                    self.stands(other as u8, sheet),
                    Stands::Beside | Stands::Striking
                ) || self.passing(fist, (other as u8, sheet))
                {
                    spared = spared.and_sheet(other as u8, sheet);
                }
            }
        }
        spared
    }

    /// What side-view fighter `body` is doing, in a line, for the tools:
    /// where it stands, its punch and where that aims in the world, and how
    /// far into leaning, ducking, and blocking it is.
    #[cfg(test)]
    pub(super) fn mover_line(&self, body: usize) -> String {
        let (Some(frame), Some(mover)) = (
            self.fighter_frame(body),
            self.bodies.get(body).and_then(|state| state.mover.as_ref()),
        ) else {
            return String::from("no fighter");
        };
        let strike = mover.strike.map_or(String::from("no punch"), |strike| {
            let target = frame.world(strike.target);
            format!(
                "{:?} hand's punch {:.3} s old, aimed at ({:.0}, {:.0}), reach {:.2}, landed {:?}, turned {:?}",
                strike.hand,
                strike.age,
                target.x,
                target.y,
                strike.reach(),
                strike.landed,
                strike.turned.map(|turned| turned.0),
            )
        });
        // Where its pose puts its head and its hips.
        let pose = mover.pose();
        let (head, hips) = (
            frame.world(Joints::of(&pose).head()),
            frame.world(pose.hips),
        );
        let step = mover.step.map_or(String::from("none"), |step| {
            format!(
                "{:+.3}{} {:.2} s old, down {:?}",
                step.by,
                if step.catching { " catching" } else { "" },
                step.age,
                step.down,
            )
        });
        // Where its hips really are, and how far past its feet its weight
        // runs (`World::capture_beyond`), in body heights.
        let really = self
            .bodies
            .get(body)
            .and_then(|state| state.fighter.as_ref())
            .and_then(|rig| self.bones.get(rig.pelvis))
            .map_or(Vec2::default(), |pelvis| pelvis.b);
        let (beyond, travel) = self.capture_beyond(body, frame.facing, frame.height);
        // How each foot stands: swung by a step, lifted off the floor at
        // both ends, or touching it, and the share of the weight it bears.
        let feet: Vec<String> = self
            .bodies
            .get(body)
            .and_then(|state| state.fighter.as_ref())
            .map_or(Vec::new(), |rig| {
                rig.feet
                    .iter()
                    .map(|&foot| {
                        self.foot_holds
                            .iter()
                            .find(|hold| hold.bone == foot)
                            .map_or(String::from("no hold"), |hold| {
                                format!(
                                    "{}{}load {:.2}",
                                    if hold.swinging { "swinging, " } else { "" },
                                    if self.foot_on_floor(foot) {
                                        "on the floor, "
                                    } else {
                                        "off the floor, "
                                    },
                                    hold.load
                                )
                            })
                    })
                    .collect()
            });
        format!(
            "pose puts the head at ({:.0}, {:.0}) and the hips at ({:.0}, {:.0}), the hips really at ({:.0}, {:.0}), {:.2} body heights up, the weight {beyond:+.3} past the feet and going {travel:+.3}; feet {feet:?}; place {:+.3}, step {step}{}, lean {:.2}, duck {:.2}, block {:.2}; {strike}",
            head.x,
            head.y,
            hips.x,
            hips.y,
            really.x,
            really.y,
            frame.own(really).1,
            mover.place,
            match (self.knocked_out(body), mover.fallen) {
                (true, _) => ", knocked out",
                (false, true) => ", fallen",
                (false, false) => "",
            },
            mover.lean.amount,
            mover.duck.amount,
            mover.block.amount,
        )
    }

    /// How far side-view fighter `body`'s head is from where its pose puts
    /// it, in metres, for the tools and the tests: a body that follows its
    /// moves keeps its head within a few centimetres of it.
    #[cfg(test)]
    pub(super) fn head_off_pose(&self, body: usize) -> Option<f64> {
        let frame = self.fighter_frame(body)?;
        let state = self.bodies.get(body)?;
        let (rig, mover) = (state.fighter.as_ref()?, state.mover.as_ref()?);
        let skull = self.bones.get(rig.skull)?;
        let aimed = frame.world(Joints::of(&mover.pose()).head());
        // The pose's head is the joint under the skull.
        Some(distance(skull.a, aimed) * self.mm_per_px() * 0.001)
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
            .map(|bone| lerp(bone.a, bone.b, AIM_UP_THE_SKULL))
            .min_by(|p, q| (p.x - own).abs().total_cmp(&(q.x - own).abs()))
    }

    /// How far side-view fighter `body` can still step toward the fighter
    /// in front of it, in body heights, before they stand as near as two
    /// fighters do (`NEAREST_STAND`), counting the steps both are in the
    /// middle of as taken; without limit if no fighter stands in front.
    fn room_ahead(&self, body: usize) -> f64 {
        let (Some(frame), Some(own)) = (self.fighter_frame(body), self.fighter_stands(body)) else {
            return 0.0;
        };
        (0..self.bodies.len())
            .filter(|&other| other != body)
            .filter_map(|other| self.fighter_stands(other))
            .map(|other| frame.facing * (other - own) / frame.height)
            .filter(|&gap| gap > 0.0)
            .map(|gap| gap - NEAREST_STAND)
            .fold(f64::INFINITY, f64::min)
    }

    /// Where side-view fighter `body` stands across the world, in pixels:
    /// where its steps have carried its hips from where it was built, the
    /// step it is in the middle of counted as taken. A lean, a duck, or a
    /// blow moves its hips about that place without moving the place. For a
    /// game's own rules about where fighters may stand, such as a ring's
    /// edge, and for how far apart two fighters stand.
    pub fn fighter_stands(&self, body: usize) -> Option<f64> {
        let frame = self.fighter_frame(body)?;
        let mover = self.bodies.get(body)?.mover.as_ref()?;
        let ahead = mover.place + mover.step.map_or(0.0, |step| step.by);
        Some(frame.stand.x + frame.facing * ahead * frame.height)
    }

    /// Where each foot side-view fighter `body`'s step is swinging will come
    /// down, across the world: its heel and its ball where the step sets it.
    pub(super) fn step_landing(&self, body: usize) -> Vec<f64> {
        let (Some(frame), Some(mover)) = (
            self.fighter_frame(body),
            self.bodies.get(body).and_then(|state| state.mover.as_ref()),
        ) else {
            return Vec::new();
        };
        let Some(step) = mover.step else {
            return Vec::new();
        };
        let lifted = step.lifted();
        let mut pose = mover.pose();
        let mut ends = Vec::new();
        for side in (0..2).filter(|&side| lifted[side]) {
            pose.feet[side].ball = (mover.feet[side] + step.by, 0.0);
            let joints = Joints::of(&pose);
            ends.push(frame.world(joints.heel[side]).x);
            ends.push(frame.world(joints.ball[side]).x);
        }
        ends
    }

    /// Lets a fallen fighter's joints stay as they lie: each joint's muscles
    /// aim it where it is and only slow it, and the shoulders' seats stop
    /// sliding. Aimed at its stance still, a fighter shoved off its feet
    /// pushed itself back up from sitting on the floor to standing, which
    /// getting up, a movement of its own, should do.
    fn let_fallen_joints_be(&mut self, rig: &fighter::Rig) {
        for &index in &rig.hinges {
            let Some(turn) = self.joint_turn(index) else {
                continue;
            };
            if let Some(joint) = self.bone_joints.get_mut(index) {
                joint.drive.swing = turn;
                joint.drive.swing_speed = 0.0;
                joint.seat_speed = Vec2::default();
            }
        }
    }

    /// Pushes side-view fighter `body`'s trunk, its spine, head, and ribs
    /// with the flesh held to them, across by `newton_seconds` (positive to
    /// the window's right) over the coming step `dt` long: as much farther as
    /// it then travels, since a step begins moving each thing as far as the
    /// last moved it (`split_step_into_substeps`).
    fn shove_trunk(&mut self, body: usize, rig: &fighter::Rig, newton_seconds: f64, dt: f64) {
        let spine = [
            rig.pelvis,
            rig.lumbar,
            rig.thoracic,
            rig.cervical,
            rig.skull,
        ];
        let trunk: Vec<usize> = (0..self.bones.len())
            .filter(|&bone| {
                spine.contains(&bone)
                    || (self.bones[bone].body as usize == body
                        && self.bones[bone].part == BonePart::Rib)
            })
            .collect();
        let points: Vec<usize> = (0..self.points.len())
            .filter(|&point| {
                let held = &self.point_holds[point];
                self.points[point].body as usize == body
                    && !self.points[point].pinned
                    && !self.points[point].is_ghost()
                    && trunk.contains(&held.posture.bones[0])
            })
            .collect();
        let kilograms = (trunk.iter().map(|&bone| self.bones[bone].mass).sum::<f64>()
            + points
                .iter()
                .map(|&point| self.points[point].mass)
                .sum::<f64>())
            * KG_PER_MASS;
        if kilograms <= 0.0 {
            return;
        }
        // Metres a second gained, as pixels moved over the step.
        let shift = newton_seconds / kilograms * 1000.0 / self.mm_per_px() * dt;
        for bone in trunk {
            self.bones[bone].a.x += shift;
            self.bones[bone].b.x += shift;
        }
        for point in points {
            self.points[point].position.x += shift;
        }
    }

    /// Whether side-view fighter `body` has fallen, its weight run past where
    /// any step could catch it (`Mover::fallen`).
    pub(super) fn fighter_fallen(&self, body: usize) -> bool {
        self.bodies
            .get(body)
            .and_then(|state| state.mover.as_ref())
            .is_some_and(|mover| mover.fallen)
    }

    /// Whether side-view fighter `body` would take a step if told to: it is
    /// not in the middle of one.
    fn free_to_step(&self, body: usize) -> bool {
        self.bodies
            .get(body)
            .and_then(|state| state.mover.as_ref())
            .is_some_and(|mover| mover.step.is_none())
    }

    /// Tells each fighter the move it makes this step, `moves` as a list of
    /// bodies and their moves, all as one: fighters stepping toward each
    /// other on the same step share the room left between them, so that
    /// neither is told first and takes it all.
    pub(super) fn command_all(&mut self, moves: &[(usize, Move)]) {
        let stepping_in = moves
            .iter()
            .filter(|&&(body, made)| made == Move::StepIn && self.free_to_step(body))
            .count()
            .max(1) as f64;
        let rooms: Vec<f64> = moves
            .iter()
            .map(|&(body, _)| self.room_ahead(body) / stepping_in)
            .collect();
        for (&(body, made), room) in moves.iter().zip(rooms) {
            self.command_with_room(body, made, room);
        }
    }

    /// Tells body `body`, a side-view fighter, to make a move: it throws a
    /// punch unless one is still on its way, the arm hangs limp, or the
    /// other fighter is down, takes a
    /// step unless it is in the middle of one, toward the other fighter only
    /// as far as there is room (`NEAREST_STAND`), and leans back, ducks, or
    /// blocks for as long as told to. Other bodies take no notice.
    pub fn command(&mut self, body: usize, made: Move) {
        let room = self.room_ahead(body);
        self.command_with_room(body, made, room);
    }

    /// [`World::command`], with `room` body heights for a step toward the
    /// other fighter.
    fn command_with_room(&mut self, body: usize, made: Move, room: f64) {
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
        // No punch at a fighter who is down, as boxing's rules have had it
        // since the Marquess of Queensberry's (1867): a man down may not be
        // struck. A punch already on its way goes on.
        let others: Vec<usize> = (0..self.bodies.len())
            .filter(|&other| other != body && self.bodies[other].fighter.is_some())
            .collect();
        let others_down =
            !others.is_empty() && others.iter().all(|&other| self.fighter_fallen(other));
        let lead_free = throws(self, Side::Lead) && !others_down;
        let rear_free = throws(self, Side::Rear) && !others_down;
        let Some(mover) = self.bodies[body]
            .mover
            .as_mut()
            .filter(|mover| !mover.fallen)
        else {
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
                landed: None,
                turned: None,
            }
        };
        match made {
            Move::Jab if lead_free => mover.strike = Some(strike(mover, Side::Lead)),
            Move::Cross if rear_free => mover.strike = Some(strike(mover, Side::Rear)),
            Move::StepIn if mover.step.is_none() => {
                let by = STEP_LENGTH.min(room);
                if by >= SHORTEST_STEP {
                    mover.step = Some(Step::new(by));
                }
            }
            Move::StepOut if mover.step.is_none() => {
                mover.step = Some(Step::new(-STEP_LENGTH));
            }
            Move::LeanBack(held) => mover.lean.held = held,
            Move::Block(held) => mover.block.held = held,
            Move::Duck(held) => mover.duck.held = held,
            Move::Shoved(newton_seconds) => mover.shoved = Some((f64::from(newton_seconds), 0.0)),
            _ => {}
        }
    }

    /// Moves every side-view fighter on by a step `dt` long: its moves go
    /// on, the pose they ask for is worked out, and its joints, hips, and
    /// feet are aimed there.
    pub(super) fn drive_movers(&mut self, dt: f64) {
        // Where the front of each fighter's guard would stand across the
        // window, as all of them stood when the step began, so that two
        // fighters crowding each other sit back alike.
        let fronts: Vec<Option<f64>> = (0..self.bodies.len())
            .map(|body| {
                let frame = self.fighter_frame(body)?;
                let mover = self.bodies[body].mover.as_ref()?;
                Some(frame.stand.x + frame.facing * mover.guard_front() * frame.height)
            })
            .collect();
        // How far each fighter could step toward the other, as all stood.
        let rooms: Vec<f64> = (0..self.bodies.len())
            .map(|body| self.room_ahead(body))
            .collect();
        for body in 0..self.bodies.len() {
            let (Some(frame), Some(rig)) =
                (self.fighter_frame(body), self.bodies[body].fighter.clone())
            else {
                continue;
            };
            let Some(mut mover) = self.bodies[body].mover.take() else {
                continue;
            };
            // Each of two fighters whose guards would reach past each other
            // sits back by half of how far.
            if let Some(front) = fronts[body] {
                mover.crowded = fronts
                    .iter()
                    .enumerate()
                    .filter(|&(other, _)| other != body)
                    .filter_map(|(_, other)| *other)
                    .map(|other| frame.facing * (front - other) / frame.height * 0.5)
                    .fold(0.0, f64::max);
            }
            mover.clock += dt;
            mover.lean.ease(2.4, dt);
            // A duck drops the hips no faster than legs giving way let the
            // body fall (`DUCK_FALL`).
            let metres = frame.height * self.mm_per_px() * 0.001;
            mover.duck.ease_at_most(
                2.4,
                DUCK_FALL * self.materials.gravity / (DUCK_DROP * metres),
                dt,
            );
            mover.block.ease(BLOCK_HERTZ, dt);
            // A shove from outside pushes the trunk, a share each step.
            if let Some((newton_seconds, age)) = mover.shoved {
                let share = newton_seconds * dt.min(SHOVE_SECONDS - age).max(0.0) / SHOVE_SECONDS;
                self.shove_trunk(body, &rig, -frame.facing * share, dt);
                mover.shoved =
                    Some((newton_seconds, age + dt)).filter(|_| age + dt < SHOVE_SECONDS);
            }
            // A foot swings while its part of a step lasts, and is set down
            // as it ends: the first as it strikes the floor on its way down,
            // or as its time runs out.
            let mut swinging = [false; 2];
            let mut loads = [1.0; 2];
            if let Some(step) = &mut mover.step {
                step.age += dt;
                for order in 0..2 {
                    let side = step.side(order);
                    // Near where the step sets it down: within a few
                    // centimetres of the end of its way.
                    let near = self
                        .foot_holds
                        .iter()
                        .find(|hold| hold.bone == rig.feet[side])
                        .zip(self.bones.get(rig.feet[side]))
                        .is_some_and(|(hold, foot)| {
                            let middle = lerp(foot.a, foot.b, 0.5);
                            let off = subtract(middle, hold.swing_to);
                            hypot(off.x, off.y) < LANDING_NEAR * frame.height
                        });
                    if !step.lands(order, self.foot_on_floor(rig.feet[side]), near) {
                        continue;
                    }
                    step.down[order] = Some(step.age);
                    if order == 0 {
                        if let Some(foot) = self.bones.get(rig.feet[side]) {
                            step.went(frame.own(foot.b).0 - mover.feet[side]);
                        }
                    }
                }
                swinging = step.lifted();
                loads = step.loads();
                if step.done() {
                    let by = step.by;
                    mover.place += by;
                    for foot in &mut mover.feet {
                        *foot += by;
                    }
                    mover.step = None;
                    swinging = [false; 2];
                    loads = [1.0; 2];
                }
            }
            // Its hips down near the floor, a fighter has fallen, whether or
            // not a step was still trying to catch it: sat down by a shove
            // in the middle of its step, a fighter not counted as fallen
            // was pushed back up to standing by its legs.
            let hips_up = self
                .bones
                .get(rig.pelvis)
                .map_or(f64::INFINITY, |pelvis| frame.own(pelvis.b).1);
            if !mover.fallen && hips_up < FALLEN_HIPS {
                mover.fallen = true;
                mover.step = None;
            }
            // Its weight run past its feet, a fighter steps to catch itself,
            // toward where its weight is going and past it, no farther than
            // a step goes (Hof, Gazendam and Sinke 2005; Pratt, Carff,
            // Drakunov and Goswami 2006); past that, no step can catch it, and
            // it falls and stays down. Toward the other fighter, it steps no
            // nearer than two fighters stand.
            if mover.step.is_none() && !mover.fallen {
                let (beyond, travel) = self.capture_beyond(body, frame.facing, frame.height);
                if beyond != 0.0 {
                    // The whole stance moves under where the body comes to
                    // rest. Moved only far enough to put the capture point
                    // inside the feet, a step catching a shove of 100 N s went
                    // 11 cm while the hips ran on 28.
                    let needed = travel + travel.signum() * CATCH_PAST;
                    if needed.abs() > FARTHEST_STEP {
                        mover.fallen = true;
                    } else {
                        let by = if needed > 0.0 {
                            needed.min(rooms[body].max(0.0))
                        } else {
                            needed
                        };
                        if by.abs() >= SHORTEST_STEP {
                            let hips_from = self
                                .bones
                                .get(rig.pelvis)
                                .map_or(0.0, |pelvis| frame.own(pelvis.b).0 - mover.pose().hips.0);
                            mover.step = Some(Step::catching(by, hips_from));
                        }
                    }
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
            if mover.strike.is_some_and(|strike| strike.followed_through()) {
                let reached = self.strike_reached(&rig, &frame, &mover);
                if let Some(strike) = &mut mover.strike {
                    strike.turn_back(reached);
                }
            }

            // A swinging foot goes where the step puts it over the floor,
            // reached from where the hips really are, as Coros, Beaudoin and
            // van de Panne (2010) aim a swinging foot along a path over the
            // ground and work out its hip and knee from there. Reached from
            // where the move puts the hips, it went wherever they went: as
            // the hips lagged behind a step in, the lead foot came down 8 cm
            // short of where the step meant it to land.
            let mut planned = mover.pose();
            // A swinging foot stays up until it has got as far along its way
            // as its time says: lowered by the clock while it lagged 2.5 cm
            // behind, a short step's lead foot came down short, and the
            // floor held it there.
            if let Some(step) = mover.step {
                for order in 0..2 {
                    let side = step.side(order);
                    let (Some(foot), true) = (self.bones.get(rig.feet[side]), swinging[side])
                    else {
                        continue;
                    };
                    let along = if step.by.abs() > EPSILON {
                        ((frame.own(foot.b).0 - mover.feet[side]) / step.by).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    let swung = step.swung(order).min(along);
                    planned.feet[side].ball.1 = STEP_LIFT * libm::sin(PI * swung);
                }
            }
            // A foot handing its weight over or taking it back is reached
            // from where the hips really are as far as it has no weight to
            // bear: switched at once as it came down, the leg's aims jumped,
            // the ankle turned half a radian in a step, and it kicked the foot,
            // gripping by almost nothing yet, 7 cm on along the floor.
            let mut pose = planned;
            if let Some(pelvis) = self.bones.get(rig.pelvis).filter(|_| mover.step.is_some()) {
                let hips = frame.own(pelvis.b);
                for side in 0..2 {
                    let free = if swinging[side] {
                        1.0
                    } else {
                        1.0 - loads[side]
                    };
                    pose.feet[side].ball.0 += (pose.hips.0 - hips.0) * free;
                    pose.feet[side].ball.1 += (pose.hips.1 - hips.1) * free;
                }
            }
            let joints = Joints::of(&pose);
            if mover.fallen {
                self.let_fallen_joints_be(&rig);
                self.bodies[body].mover = Some(mover);
                continue;
            }
            self.aim_joints(body, &rig, &frame, &mut mover, &joints, dt);
            self.carry_hips(&rig, &frame, &mut mover, &joints, dt);
            self.place_feet(&rig, &frame, &Joints::of(&planned), swinging, loads, dt);
            self.bodies[body].mover = Some(mover);
        }
    }

    /// How far along its way out the punching fist of the fighter `mover`
    /// moves really is, from 0 where it stood in its guard to 1 where the
    /// punch ends; 1 for a fighter throwing none.
    fn strike_reached(&self, rig: &fighter::Rig, frame: &Frame, mover: &Mover) -> f64 {
        let (Some(strike), (_, Some((from, to)))) = (&mover.strike, mover.pose_and_strike_line())
        else {
            return 1.0;
        };
        let Some(hand) = self.bones.get(rig.hands[strike.hand.index()]) else {
            return 1.0;
        };
        let at = frame.own(hand.b);
        let line = (to.0 - from.0, to.1 - from.1);
        let length_sq = (line.0 * line.0 + line.1 * line.1).max(1.0e-12);
        ((at.0 - from.0) * line.0 + (at.1 - from.1) * line.1) / length_sq
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
        let ends = self.pelvis_ends(frame, joints);
        let ankles = joints.ankle.map(|ankle| frame.world(ankle));
        let was = mover.last_hips.unwrap_or(ends);
        for hold in &mut self.hip_holds {
            if hold.bone != rig.pelvis {
                continue;
            }
            let end = usize::from(hold.end_b);
            if let Some(carry) = &mut hold.carry {
                carry.offset = subtract(ends[end], hold.home);
                carry.speed = scale(subtract(ends[end], was[end]), 1.0 / dt.max(EPSILON));
                carry.from_ankles = ankles.map(|ankle| subtract(ends[end], ankle));
            }
        }
        mover.last_hips = Some(ends);
    }

    /// Where `joints` puts each end of a fighter's pelvis standing in
    /// `frame`, its `a` end first: the pelvis's bone runs from a joint's rest
    /// gap short of the foot of the spine down to the hips (`fighter.rs`).
    fn pelvis_ends(&self, frame: &Frame, joints: &Joints) -> [Vec2; 2] {
        let gap = self.materials.point_spacing * 0.70;
        let (hips, sacrum) = (frame.world(joints.hips), frame.world(joints.sacrum));
        let up = normalized(subtract(sacrum, hips), Vec2 { x: 0.0, y: -1.0 });
        [subtract(sacrum, scale(up, gap)), hips]
    }

    /// Stands each of the fighter's feet where `joints` puts it, bearing
    /// `loads` of its share: a foot a step is swinging is carried along its
    /// way by its leg alone (`World::carry_swinging_foot`), and one on the
    /// floor stands with its ball where it stood and its heel as high as the
    /// pose lifts it.
    fn place_feet(
        &mut self,
        rig: &fighter::Rig,
        frame: &Frame,
        joints: &Joints,
        swinging: [bool; 2],
        loads: [f64; 2],
        dt: f64,
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
                hold.load = loads[side];
                if swinging[side] {
                    let to = lerp(heel, ball, 0.5);
                    hold.swing_speed = if hold.swinging {
                        scale(subtract(to, hold.swing_to), 1.0 / dt.max(EPSILON))
                    } else {
                        Vec2::default()
                    };
                    hold.swing_to = to;
                    hold.swinging = true;
                    continue;
                }
                hold.swing_impulse = Vec2::default();
                if hold.swinging {
                    // Set down where it came down, its heel as high as the
                    // pose lifts it: held toward where the step meant it to
                    // land instead, a foot that came down a little off it
                    // was dragged over the floor.
                    hold.swinging = false;
                    let foot = self.bones[hold.bone];
                    hold.planted = [
                        Vec2 {
                            x: foot.a.x,
                            y: heel.y,
                        },
                        Vec2 {
                            x: foot.b.x,
                            y: ball.y,
                        },
                    ];
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

    /// Whether body `body`'s arm `limb` is throwing a fighter's punch whose
    /// fist is still on its way and has landed on nothing yet: until it
    /// lands, the fist meets the other body only with its knuckles
    /// (`land_strikes`); once it has, it is a limb in the other's way like
    /// any other (`clash.rs`), so it cannot sink into what it hit. Left to
    /// its knuckles alone while it followed through, a fist drove on into
    /// the head it had struck and dragged the skin of the face until it
    /// split.
    pub(super) fn fist_in_flight(&self, body: usize, limb: Limb) -> bool {
        let Some(side) = side_of(limb) else {
            return false;
        };
        self.bodies
            .get(body)
            .and_then(|state| state.mover.as_ref())
            .and_then(|mover| mover.strike.as_ref())
            .is_some_and(|strike| {
                strike.hand == side && strike.going_out() && strike.landed.is_none()
            })
    }

    /// Gives `pieces` the momentum `push` at `at` as one rigid piece, over a
    /// substep `h` long: they move and turn together about their middle.
    pub(super) fn push_together(&mut self, pieces: &[BonePiece], at: Vec2, push: Vec2, h: f64) {
        let Some((mass, middle, inertia)) = Self::pieces_together(pieces) else {
            return;
        };
        let shift = scale(push, h / mass);
        let turn = cross(subtract(at, middle), push) * h / inertia;
        for piece in pieces {
            let swing = scale(perpendicular(subtract(piece.centre, middle)), turn);
            self.move_piece(piece, add(shift, swing), turn);
        }
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
        let (Some(frame), Some(rig)) = (
            self.fighter_frame(body),
            self.bodies
                .get(body)
                .and_then(|state| state.fighter.clone()),
        ) else {
            return;
        };
        let Some(reached) = self.bodies[body]
            .mover
            .as_ref()
            .map(|mover| self.strike_reached(&rig, &frame, mover))
        else {
            return;
        };
        if let Some(strike) = self.bodies[body]
            .mover
            .as_mut()
            .and_then(|mover| mover.strike.as_mut())
            .filter(|strike| strike.hand == side)
        {
            strike.turn_back(reached);
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
                // Its knuckles land once: after that the fist is a limb
                // pressed against what it hit (`fist_in_flight`). Landing
                // again on every step it followed through, the knuckles
                // dragged the skin of the face until it split (15 of 42
                // crosses).
                let strike = mover
                    .strike
                    .as_ref()
                    .filter(|strike| strike.going_out() && strike.landed.is_none())?;
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
        let sink = FIST_SINKS_METRES * 1000.0 / self.mm_per_px();
        // A blocking arm is braced, and takes what a fist gives it as one
        // piece (`brace_arm`).
        let blocking: Vec<ArmBefore> = (0..self.bodies.len())
            .flat_map(|body| Side::BOTH.map(|side| (body, side)))
            .filter(|&(body, side)| self.blocking_arm(body, side))
            .map(|(body, side)| self.arm_before(body, side))
            .collect();
        let landings: Vec<FistLanding> = sweeps
            .iter()
            .map(|&(body, side, _, from, to, heading)| {
                let spares = self.fist_spares(body, side);
                self.land_fist(spares, punch_weight(side), sink, from, to, heading, dt)
            })
            .collect();
        for arm in &blocking {
            self.brace_arm(arm);
        }
        for ((body, side, hand, from, to, heading), landing) in sweeps.into_iter().zip(landings) {
            let given = hypot(landing.given.x, landing.given.y) > EPSILON;
            let speed = distance(from, to) / dt.max(EPSILON) * self.mm_per_px() * 0.001;
            let landed = given && speed > LANDING_SPEED;
            if given {
                self.push_fighter_arm(body, side, scale(landing.given, -1.0), to, dt);
            }
            if landing.blow > 0.0 {
                self.load_puncher_hand(hand, landing.blow, landing.blow_normal);
            }
            let first = self.bodies[body]
                .mover
                .as_ref()
                .and_then(|mover| mover.strike.as_ref())
                .is_some_and(|strike| strike.landed.is_none());
            if landed && first {
                // The fist and what it struck go on together, as far as the
                // weight behind the fist carries them (`land_fist_blows`).
                let way = normalized(subtract(to, from), heading);
                self.fist_blows.push(FistBlow {
                    body,
                    side,
                    hand,
                    passing: None,
                    at: to,
                    heading: way,
                    speed: distance(from, to) / dt.max(EPSILON),
                    mass: tools::tool_mass_kg(ToolMode::Fist) * punch_weight(side) / KG_PER_MASS,
                    given: dot(landing.given, way),
                    spares: self.fist_spares(body, side),
                });
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
                    strike.land();
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

    /// Whether the arm on `side` of body `body` is a fighter's, closed in a
    /// block.
    fn blocking_arm(&self, body: usize, side: Side) -> bool {
        (0..fighter::SHEETS as u8).any(|sheet| {
            fighter::arm_of_sheet(sheet) == Some(side)
                && self.stands(body as u8, sheet) == Stands::Guarding
        })
    }

    /// The bones of the arm on `side` of fighter `body`, from the shoulder
    /// out.
    fn arm_bones(&self, body: usize, side: Side) -> Vec<usize> {
        self.bodies
            .get(body)
            .and_then(|state| state.fighter.as_ref())
            .map_or(Vec::new(), |rig| {
                let s = side.index();
                vec![rig.upper_arms[s], rig.forearms[s], rig.hands[s]]
            })
    }

    /// Where the flesh and bones of the arm on `side` of fighter `body` lie.
    fn arm_before(&self, body: usize, side: Side) -> ArmBefore {
        ArmBefore {
            points: self
                .points
                .iter()
                .enumerate()
                .filter(|(_, point)| {
                    point.body as usize == body
                        && fighter::arm_of_sheet(point.sheet) == Some(side)
                        && !point.pinned
                        && !point.is_ghost()
                })
                .map(|(index, point)| (index, point.position))
                .collect(),
            bones: self
                .arm_bones(body, side)
                .into_iter()
                .filter_map(|index| {
                    let bone = self.bones.get(index).filter(|bone| !bone.pinned)?;
                    Some((index, bone.a, bone.b))
                })
                .collect(),
        }
    }

    /// A blocking arm takes what a fist's landing gave it as one piece. An
    /// arm closed in a block is braced, its wrist and elbow locked by the
    /// muscles both sides of them, and behind the fist a punch lands on
    /// stands the whole arm: from where `before` has it, however the
    /// landing moved its flesh and bones, the arm moves and turns as one
    /// rigid piece carrying the same momentum. Taken on the hand alone, a
    /// piece of half a kilogram, a cross knocked the hand off the forearm
    /// and tore the wrist (4 of 8 blocked punches slipped or tore it).
    fn brace_arm(&mut self, before: &ArmBefore) {
        // The arm's mass, the middle of it, and the momentum the landing
        // gave it, each place counted from the arm's first so two fighters
        // standing as mirror images sum the same small numbers.
        let Some(&(_, origin)) = before.points.first() else {
            return;
        };
        let mut items: Vec<(f64, Vec2, Vec2)> = Vec::new();
        for &(index, was) in &before.points {
            let point = &self.points[index];
            items.push((
                point.mass,
                subtract(was, origin),
                subtract(point.position, was),
            ));
        }
        for &(index, was_a, was_b) in &before.bones {
            let bone = &self.bones[index];
            let half = bone.mass * 0.5;
            items.push((half, subtract(was_a, origin), subtract(bone.a, was_a)));
            items.push((half, subtract(was_b, origin), subtract(bone.b, was_b)));
        }
        let mass: f64 = items.iter().map(|item| item.0).sum();
        let moved = items
            .iter()
            .fold(Vec2::default(), |sum, item| add(sum, scale(item.2, item.0)));
        if mass <= EPSILON || moved == Vec2::default() {
            return;
        }
        let middle = scale(
            items
                .iter()
                .fold(Vec2::default(), |sum, item| add(sum, scale(item.1, item.0))),
            1.0 / mass,
        );
        let (mut turning, mut inertia) = (0.0, 0.0);
        for &(weight, at, by) in &items {
            let from_middle = subtract(at, middle);
            turning += weight * cross(from_middle, by);
            inertia += weight * dot(from_middle, from_middle);
        }
        let shift = scale(moved, 1.0 / mass);
        let turn = if inertia > EPSILON {
            turning / inertia
        } else {
            0.0
        };
        let braced = |was: Vec2| {
            let from_middle = subtract(subtract(was, origin), middle);
            add(was, add(shift, scale(perpendicular(from_middle), turn)))
        };
        for &(index, was) in &before.points {
            self.points[index].position = braced(was);
        }
        for &(index, was_a, was_b) in &before.bones {
            self.bones[index].a = braced(was_a);
            self.bones[index].b = braced(was_b);
        }
    }

    /// The pieces that move with bone `index` when a fist lands on it or
    /// another body's limb presses on it: that bone with its flesh
    /// (`sides.rs`), or, for a bone of an arm closed in a block, that whole
    /// braced arm. A punching arm is not braced so: met as one rigid piece,
    /// a cross stopped by a block stopped all at once, while the trunk
    /// turning into it carried its shoulder on, and the shoulder tore.
    pub(super) fn struck_pieces(&self, index: usize) -> Vec<BonePiece> {
        let Some(bone) = self.bones.get(index) else {
            return Vec::new();
        };
        let bones = match fighter::arm_of_sheet(bone.sheet) {
            Some(side) if self.stands(bone.body, bone.sheet) == Stands::Guarding => {
                self.arm_bones(bone.body as usize, side)
            }
            _ => vec![index],
        };
        bones
            .into_iter()
            .filter_map(|bone| self.bone_pieces.get(bone).cloned().flatten())
            .collect()
    }

    /// The mass of `pieces` together, their middle, and how hard they are
    /// to turn about it as one piece.
    pub(super) fn pieces_together(pieces: &[BonePiece]) -> Option<(f64, Vec2, f64)> {
        let first = pieces.first()?.centre;
        let mass: f64 = pieces.iter().map(|piece| piece.mass).sum();
        if mass <= EPSILON {
            return None;
        }
        let middle = add(
            first,
            scale(
                pieces.iter().fold(Vec2::default(), |sum, piece| {
                    add(sum, scale(subtract(piece.centre, first), piece.mass))
                }),
                1.0 / mass,
            ),
        );
        let inertia: f64 = pieces
            .iter()
            .map(|piece| {
                let off = subtract(piece.centre, middle);
                piece.inertia + piece.mass * dot(off, off)
            })
            .sum();
        (inertia > EPSILON).then_some((mass, middle, inertia))
    }

    /// What the blow `blow` lands on and how much momentum it has to pass
    /// on, as the substep `h` long begins: the bone of another body whose
    /// flesh its knuckles are deepest in, and the momentum that leaves the
    /// fist and the pieces that bone moves (`struck_pieces`) going on
    /// together along the fist's way, less what the landing itself already
    /// gave, and how much of it passes each second.
    fn measure_fist_blow(&self, blow: &FistBlow, h: f64) -> Option<(usize, f64, f64)> {
        let reach = tool_geometry(ToolMode::Fist).contact_radius + self.materials.point_spacing;
        let (gap, index) = (0..self.bones.len())
            .filter(|&index| {
                let bone = &self.bones[index];
                bone.body as usize != blow.body
                    && !blow.spares.holds(bone.body, bone.sheet)
                    && !bone.pinned
                    && !bone.splinter
                    && bone.flesh > 0.0
                    && self.bone_pieces.get(index).is_some_and(Option::is_some)
            })
            .map(|index| {
                let bone = &self.bones[index];
                let gap = distance_to_segment(blow.at, bone.previous_a, bone.previous_b);
                (gap - bone.flesh, index)
            })
            .min_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)))?;
        if gap > reach {
            return None;
        }
        let (mass, middle, inertia) = Self::pieces_together(&self.struck_pieces(index))?;
        // How fast the struck place is already moving the fist's way.
        let bone = self.bones[index];
        let (speed_a, speed_b) = (
            scale(subtract(bone.a, bone.previous_a), 1.0 / h),
            scale(subtract(bone.b, bone.previous_b), 1.0 / h),
        );
        let along = subtract(bone.previous_b, bone.previous_a);
        let spin = cross(along, subtract(speed_b, speed_a)) / dot(along, along).max(EPSILON);
        let place_speed = add(
            speed_a,
            scale(perpendicular(subtract(blow.at, bone.previous_a)), spin),
        );
        let closing = blow.speed - dot(place_speed, blow.heading);
        // How hard the pieces are to move at that place, that way.
        let arm = cross(subtract(blow.at, middle), blow.heading);
        let struck_inverse = 1.0 / mass + arm * arm / inertia;
        let impulse = closing.max(0.0) / (1.0 / blow.mass + struck_inverse) - blow.given;
        (impulse > 0.0).then_some((index, impulse, 0.0))
    }

    /// Each fist a fighter has landed shares its momentum with what it
    /// landed on: the bone of the other body whose flesh its knuckles
    /// struck, with that flesh, as one piece (`sides.rs`). The two go on
    /// together along the fist's way, as two bodies that meet and do not
    /// bounce do, the fist counting as the weight behind a punch, 2.9 kg
    /// for a boxer's rear hand (Walilko et al. 2005, who weighed it this
    /// way, from how fast a struck head moved off). The fist's own landing
    /// (`land_fist`) bruises, splits, and breaks what it strikes and shoves
    /// the flesh under the knuckles, but passes on only what that shove
    /// carries, and a head struck at 9.6 m/s moved off at 1 m/s; this gives
    /// the rest, and takes it from the arm that threw the fist. It passes as
    /// a punch's force does, rising and falling as half a sine wave over
    /// `BLOW_SECONDS`: given all at once, in one substep, it moved the head
    /// off faster than the flesh between head and neck could follow, and
    /// split the skin there; given evenly over the 30 ms the fist stays in
    /// contact, it jolted the head eight times more gently than a punch
    /// does. `h` is the substep it happens in.
    pub(super) fn land_fist_blows(&mut self, h: f64) {
        if self.fist_blows.is_empty() {
            return;
        }
        let mut blows = std::mem::take(&mut self.fist_blows);
        for blow in &mut blows {
            let Some((index, total, age)) =
                blow.passing.or_else(|| self.measure_fist_blow(blow, h))
            else {
                continue;
            };
            // The share of the push that falls in this substep.
            let passed = |t: f64| 0.5 * (1.0 - libm::cos(PI * (t / BLOW_SECONDS).clamp(0.0, 1.0)));
            let impulse = total * (passed(age + h) - passed(age));
            blow.passing = Some((index, total, age + h));
            // Where the knuckles are now.
            let at = self
                .bones
                .get(blow.hand)
                .map_or(blow.at, |hand| hand.previous_b);
            let push = scale(blow.heading, impulse);
            let pieces = self.struck_pieces(index);
            self.push_together(&pieces, at, push, h);
            self.push_fighter_arm(blow.body, blow.side, scale(push, -1.0), at, h);
        }
        blows.retain(|blow| blow.passing.is_some_and(|(_, _, age)| age < BLOW_SECONDS));
        self.fist_blows = blows;
    }

    /// The arm on `side` of fighter `body`, beyond its shoulder, takes the
    /// momentum `impulse` at `at` as one piece over the step or substep `dt`: its
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
