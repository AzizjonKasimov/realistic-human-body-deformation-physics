//! Muscles that tire as they work and recover as they rest: the
//! three-compartment model of muscle fatigue (Xia and Frey Law 2008), with
//! the rest recovery of its 3CC-r form (Looft, Herkert and Frey-Law 2018),
//! as Rakshit, Barman, Xiang and Yang (2022) write it out. The same model
//! tires physically simulated characters in Cheema et al. (SIGGRAPH Asia
//! 2023), which cap each joint's torque by what its muscles have left, as
//! here.
//!
//! A muscle group's motor units are resting, active, or fatigued, each a
//! share of the whole. A drive calls resting units up to give the torque
//! asked of the muscle and lets active ones rest when less is asked; active
//! units tire at the group's fatigue rate F, and tired units recover at its
//! recovery rate R, many times faster while the muscle rests and its blood
//! flows freely (the rest multiplier r). Only units not fatigued can pull,
//! so a group gives at most its fresh strength times what is left (the
//! residual capacity): a tired arm punches slower and softer, and its block
//! gives way sooner. Nothing scripts that; the muscles just have less to give.
//!
//! What the model asks of a muscle is how much of it must work: the share of
//! its motor units the torque it gives needs. A muscle shortening fast gives
//! far less than its strength held still (Hill 1938: force falls along a
//! hyperbola to none at its fastest shortening), so a punch, whose arm joints
//! turn at 17 to 36 radians a second, near the fastest people turn them, has
//! the muscles throwing it working whole even where they give a fraction of
//! their held strength. Counted by torque alone, a fighter punching flat out
//! for 40 s lost 4% of its arms' strength, its punches as fast as ever.
//!
//! The rates were fitted to muscles holding still. Shortening fast, a muscle
//! turns its energy over several times faster than holding the same share
//! of itself working (the Fenn effect, Fenn 1924), and tires
//! faster: a muscle shortening at its fastest tires `FAST_FATIGUE` times as
//! fast as one holding still (`Materials::muscle_fatigue`), and one
//! shortening slower in proportion. That one number is set from boxing
//! rather than measured muscle by muscle: elite boxers punching all out in
//! eleven five-second bursts with five seconds' rest between punched visibly
//! slower from the sixth burst on (Haralabidis et al. 2020). Played so
//! (`print_tiring` with `RP_BURST=5`), at the rates measured holding still
//! the fighters' punches kept their speed, and at 30 times them a fighter's
//! punches landed at 8.1 m/s in the first burst, 7.8 in the fifth, 7.5 in
//! the sixth, and 6.9 in the eleventh, its arms down to 77% and 70% of
//! their strength. Holding the guard up, still, tires the shoulders at the
//! measured rates: 1% of their strength a minute.
//!
//! The rates are those fitted to measured endurance, joint by joint, for men,
//! per second. The model's rest multiplier applies to a muscle at rest; a
//! guard held up asks a little of the shoulders all the time, so here a
//! muscle pulling under a twentieth of its strength recovers as one at rest,
//! and one pulling a seventh or more as one at work, the multiplier fading
//! out between: a working muscle's blood flow starts to be cut off somewhere
//! between 5% and 15% of its strength (Rowell 1993, as Xia 2018 has recovery
//! follow the blood's flow). The multiplier was fitted to rests of seconds;
//! a muscle's blood comes back over a few seconds once it stops working
//! hard, so the multiplier grows in over `REFLOW_SECONDS` of rest. Given in
//! full at once, it let the muscles of a fighter punching flat out recover
//! in the fifth of a second between its punches nearly as they would resting,
//! and the fighter tired no further than a quarter of its arms' strength
//! however long it kept punching.
//!
//! Only the side-view fighters' arms tire for now: their shoulders, elbows,
//! and wrists, which throw punches and hold the guard. The trunk, neck, and
//! legs keep their strength: no rates were measured for the hips or the
//! neck, a tired neck would change how blows knock a fighter out, and legs
//! that tire would ask more of the fighters' balance.

use super::soft::JointTurn;
use super::World;

/// How fast a muscle group tires and recovers, per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FatigueRates {
    /// F: the share of the active motor units that tire each second.
    pub fatigue: f64,
    /// R: the share of the fatigued units that recover each second while
    /// the muscle works.
    pub recovery: f64,
    /// r: how many times faster they recover while it rests.
    pub rest: f64,
}

/// The shoulder's muscles, the most fatigable of the joints measured
/// (Frey-Law and Avin 2010): F and R fitted to sustained holds (Frey-Law,
/// Looft and Heitsman 2012), and a rest multiplier of 15, which fitted
/// intermittent shoulder work better than 30 (Looft and Frey-Law 2020).
pub const SHOULDER: FatigueRates = FatigueRates {
    fatigue: 0.0182,
    recovery: 0.00168,
    rest: 15.0,
};

/// The elbow's flexors and extensors, and the grip, which holds the wrist
/// and fist firm (Rakshit et al. 2021, men, as tabled in Rakshit et al.
/// 2022).
pub const ELBOW_FLEXORS: FatigueRates = FatigueRates {
    fatigue: 0.01302,
    recovery: 0.00188,
    rest: 8.99,
};
pub const ELBOW_EXTENSORS: FatigueRates = FatigueRates {
    fatigue: 0.01269,
    recovery: 0.00085,
    rest: 30.21,
};
pub const GRIP: FatigueRates = FatigueRates {
    fatigue: 0.01238,
    recovery: 0.00178,
    rest: 8.0,
};

/// The fastest each joint's muscles turn it, in radians a second, past which
/// they give nothing: the elbow's extensors drive it at 1,400 to 2,200
/// degrees a second at most (24 to 38 rad/s), and the wrist reaches 50 rad/s,
/// the fastest of the arm's joints (Jessop and Pain 2016); the shoulder,
/// swinging the whole arm, an estimate below the elbow's.
pub const SHOULDER_FASTEST: f64 = 20.0;
pub const ELBOW_FASTEST: f64 = 30.0;
pub const WRIST_FASTEST: f64 = 50.0;

/// How many times as fast as holding still a muscle shortening at its
/// fastest tires, set so that the fighters tire as elite boxers did punching
/// all out (module docs): `Materials::muscle_fatigue`'s default.
pub const FAST_FATIGUE: f64 = 30.0;

/// How sharply a muscle's strength falls as it shortens faster: Hill's
/// constant a over the strength held still, about a quarter (Hill 1938).
const HILL_CURVATURE: f64 = 0.25;

/// The least share of its held strength a muscle is taken to give however
/// fast it shortens, so that a small pull at a joint turning past its
/// muscles' fastest counts as working them whole, not more.
const LEAST_STRENGTH: f64 = 0.05;

/// How fast the drive calls resting units up, and lets active ones rest,
/// per second of the gap between what is asked and what is active (L_D and
/// L_R): 10 each, as the model's authors set them.
const ACTIVATION_RATE: f64 = 10.0;
const RELAXATION_RATE: f64 = 10.0;

/// The loads, as shares of a muscle's fresh strength, under which it
/// recovers as at rest and over which it recovers as at work.
const RESTING_LOAD: f64 = 0.05;
const WORKING_LOAD: f64 = 0.15;

/// How long a muscle's blood takes to flow back in full once it stops
/// working hard, in seconds: after a contraction blood flow rises within
/// two seconds and peaks within about five heartbeats (rapid-onset
/// vasodilation; Tschakovsky et al. 1996 and 2004, as reviews give them).
const REFLOW_SECONDS: f64 = 3.0;

/// The muscles that turn a joint one way, and how tired they are. Muscles
/// with no rates never tire.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MuscleGroup {
    rates: Option<FatigueRates>,
    /// The fastest they turn the joint, in radians a second.
    fastest: f64,
    /// The shares of its motor units now active and fatigued; the rest rest.
    active: f64,
    fatigued: f64,
    /// How long it has worked under `WORKING_LOAD`, its blood flowing back,
    /// in seconds.
    rested: f64,
}

impl MuscleGroup {
    /// Fresh muscles that tire at `rates` and turn their joint at most
    /// `fastest` radians a second.
    pub const fn tiring(rates: FatigueRates, fastest: f64) -> MuscleGroup {
        MuscleGroup {
            rates: Some(rates),
            fastest,
            active: 0.0,
            fatigued: 0.0,
            rested: 0.0,
        }
    }

    /// The share of their held strength they give shortening at
    /// `shortening` radians a second (lengthening when negative), on Hill's
    /// hyperbola; held still or lengthening, all of it.
    fn strength_at(&self, shortening: f64) -> f64 {
        if shortening <= 0.0 || self.fastest <= 0.0 {
            return 1.0;
        }
        let speed = shortening / self.fastest;
        ((1.0 - speed) / (1.0 + speed / HILL_CURVATURE)).max(LEAST_STRENGTH)
    }

    /// Whether these muscles tire at all.
    pub fn tires(&self) -> bool {
        self.rates.is_some()
    }

    /// The share of their fresh strength they can give now, from 1 fresh
    /// down: what is not fatigued.
    pub fn capacity(&self) -> f64 {
        1.0 - self.fatigued
    }

    /// `dt` seconds of pulling with `load`, a share of the fresh strength
    /// held still, while the muscles shorten at `shortening` radians a
    /// second: what is asked of them is the share of them that must work to
    /// give that at that speed. Shortening at their fastest, active units
    /// tire `fast` times as fast as holding still, and in proportion between
    /// (`Materials::muscle_fatigue`).
    pub fn work(&mut self, load: f64, shortening: f64, dt: f64, fast: f64) {
        let Some(rates) = self.rates else {
            return;
        };
        let speed = if self.fastest > 0.0 {
            (shortening / self.fastest).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let load = (load.max(0.0) / self.strength_at(shortening)).min(1.0);
        let resting = (1.0 - self.active - self.fatigued).max(0.0);
        let drive = if self.active < load {
            ACTIVATION_RATE * (load - self.active).min(resting)
        } else {
            RELAXATION_RATE * (load - self.active)
        };
        // Recovering as at rest the more lightly it pulls, once its blood
        // has had time to flow back.
        self.rested = if load < WORKING_LOAD {
            self.rested + dt
        } else {
            0.0
        };
        let flowing = (self.rested / REFLOW_SECONDS).min(1.0);
        let at_rest =
            ((WORKING_LOAD - load) / (WORKING_LOAD - RESTING_LOAD)).clamp(0.0, 1.0) * flowing;
        let recovery = rates.recovery * (1.0 + (rates.rest - 1.0) * at_rest);
        let tiring = rates.fatigue * (1.0 + (fast - 1.0) * speed) * self.active;
        let recovering = recovery * self.fatigued;
        self.active = (self.active + (drive - tiring) * dt).clamp(0.0, 1.0);
        self.fatigued = (self.fatigued + (tiring - recovering) * dt).clamp(0.0, 1.0 - self.active);
    }
}

impl World {
    /// The muscles across every joint work, over a substep `h` long, as hard
    /// as the torque their drive gave this substep asked of them: the
    /// muscles turning it each way tire by what they pulled, and recover.
    pub(super) fn tire_muscles(&mut self, h: f64) {
        let newton_metres = self.newton_metres();
        let scale = self.materials.muscle_fatigue;
        // `muscle_fatigue=0`: muscles that never tire, for comparisons.
        if scale <= 0.0 {
            return;
        }
        for index in 0..self.bone_joints.len() {
            let joint = &self.bone_joints[index];
            if !joint.drive.muscles.iter().any(MuscleGroup::tires) {
                continue;
            }
            let strongest = joint.drive.max_torque * newton_metres * h;
            if strongest <= 0.0 {
                continue;
            }
            let load = joint.drive_impulse / strongest;
            // How fast the joint turned this substep, the way the first
            // muscles pull it.
            let speed = JointTurn::of(joint, &self.bones).speed(&self.bones, h);
            let muscles = &mut self.bone_joints[index].drive.muscles;
            muscles[0].work(load, speed, h, scale);
            muscles[1].work(-load, -speed, h, scale);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::SCENARIO_WIDTH;
    use crate::simulation::{create_side_fighters_at, InputState, Materials, Move};

    const DT: f64 = 1.0 / 240.0;

    /// A second of a fighter punching again and again.
    #[derive(Clone, Copy, Debug, Default)]
    struct Second {
        /// Both fighters' arms' strength left as it ended, the puncher's
        /// first.
        arms: [f64; 2],
        /// The punches the puncher threw in it, with how long each takes to
        /// go out added up, and those that landed, with their landing speeds
        /// added up.
        thrown: u32,
        out: f64,
        landed: u32,
        speeds: f64,
    }

    impl Second {
        fn out_seconds(&self) -> f64 {
            self.out / f64::from(self.thrown.max(1))
        }

        fn landing_speed(&self) -> f64 {
            self.speeds / f64::from(self.landed.max(1))
        }
    }

    /// What a fighter punching again and again came to, second by second.
    struct Flurry {
        seconds: Vec<Second>,
    }

    /// Two fighters `spacing` body heights apart, the left throwing a jab or
    /// a cross, by turns, `gap` seconds after its last is back, for
    /// `punching` seconds, then standing for `resting`; the right stands in
    /// its guard. With `burst` seconds, the left punches in bursts that long
    /// with as long a rest between.
    fn flurry(
        materials: Materials,
        spacing: f64,
        gap: f64,
        burst: f64,
        punching: f64,
        resting: f64,
    ) -> Flurry {
        let mut world = create_side_fighters_at(materials, spacing);
        let dt = world.materials.fixed_dt;
        let steps_a_second = (1.0 / dt).round() as usize;
        let total = ((punching + resting) / dt).round() as usize;
        let mut seconds = Vec::new();
        let mut second = Second::default();
        let (mut waited, mut cross) = (0.0, false);
        // `RP_LOADS=1`: each tiring joint's load, as a share of its fresh
        // strength each way, summed and at most, while the puncher punches.
        let loads_wanted = std::env::var("RP_LOADS").is_ok();
        let mut loads: Vec<(usize, f64, f64, f64, f64)> = Vec::new();
        // And each tiring joint's fastest turn while the puncher punches, in
        // radians a second, and its speed at the moment it pulled hardest.
        let mut turns: Vec<(usize, f64, f64)> = Vec::new();
        for step in 0..total {
            let mut input = InputState::default();
            let free = world
                .fighter_doing(0)
                .is_some_and(|doing| doing.punch.is_none());
            if free {
                waited += dt;
            }
            let clock = step as f64 * dt;
            let bursting = burst <= 0.0 || clock % (2.0 * burst) < burst;
            let throws = clock < punching && bursting && free && waited >= gap;
            if throws {
                input.moves[0] = Some((0, if cross { Move::Cross } else { Move::Jab }));
                cross = !cross;
                waited = 0.0;
            }
            world.step(dt, &input, SCENARIO_WIDTH);
            if let Some(punch) = world.fighter_doing(0).and_then(|doing| doing.punch) {
                if throws {
                    second.thrown += 1;
                    second.out += punch.out_seconds;
                }
            }
            if loads_wanted && (step as f64) * dt < punching {
                let h = dt / world.materials.substeps as f64;
                let newton_metres = world.newton_metres();
                for (index, joint) in world.bone_joints.iter().enumerate() {
                    if world.joint_body(joint) != 0 || !joint.drive.muscles[0].tires() {
                        continue;
                    }
                    let load = joint.drive_impulse / (joint.drive.max_torque * newton_metres * h);
                    let entry = match loads.iter_mut().find(|entry| entry.0 == index) {
                        Some(entry) => entry,
                        None => {
                            loads.push((index, 0.0, 0.0, 0.0, 0.0));
                            loads.last_mut().unwrap()
                        }
                    };
                    entry.1 += load.max(0.0) * dt;
                    entry.2 = entry.2.max(load);
                    entry.3 += (-load).max(0.0) * dt;
                    entry.4 = entry.4.max(-load);
                    // A step done, each bone's `previous` is a step behind.
                    let speed = crate::simulation::soft::JointTurn::of(joint, &world.bones)
                        .speed(&world.bones, dt)
                        .abs();
                    match turns.iter_mut().find(|turn| turn.0 == index) {
                        Some(turn) => turn.1 = turn.1.max(speed),
                        None => turns.push((index, speed, 0.0)),
                    }
                }
            }
            for punch in world.landed_punches().iter().filter(|punch| punch.by == 0) {
                second.landed += 1;
                second.speeds += punch.speed;
            }
            if (step + 1) % steps_a_second == 0 {
                let both = |body| {
                    world
                        .fighter_stamina(body)
                        .map_or(1.0, |stamina| stamina.both())
                };
                second.arms = [both(0), both(1)];
                seconds.push(std::mem::take(&mut second));
            }
        }
        for (index, plus, most_plus, minus, most_minus) in loads {
            let joint = world.bone_joints[index];
            println!(
                "joint {index:2} ({:?}, {:.0} N m): load + mean {:.3} most {:.2}, - mean {:.3} most {:.2}; left + {:.3} - {:.3}",
                joint.drive.limb,
                joint.drive.max_torque,
                plus / punching,
                most_plus,
                minus / punching,
                most_minus,
                joint.drive.muscles[0].capacity(),
                joint.drive.muscles[1].capacity(),
            );
        }
        for (index, fastest, _) in turns {
            println!("joint {index:2} turned at most {fastest:.1} rad/s");
        }
        Flurry { seconds }
    }

    /// Prints a fighter punching flat out and then resting, second by
    /// second: its arms' strength left, punches thrown and landed, and how
    /// fast they landed (`cargo test --release --lib print_tiring --
    /// --ignored --nocapture`; `RP_PUNCHING` and `RP_RESTING` seconds,
    /// `RP_GAP` seconds between punches, `RP_BURST` seconds of punching and
    /// as many of rest by turns, `RP_SPACING` body heights apart,
    /// `RP_FATIGUE` for `Materials::muscle_fatigue`).
    #[test]
    #[ignore]
    fn print_tiring() {
        let number = |name: &str, default: f64| {
            std::env::var(name)
                .ok()
                .and_then(|text| text.parse().ok())
                .unwrap_or(default)
        };
        let mut materials = Materials::default();
        materials.muscle_fatigue = number("RP_FATIGUE", materials.muscle_fatigue);
        let (punching, resting) = (number("RP_PUNCHING", 30.0), number("RP_RESTING", 20.0));
        let flurry = flurry(
            materials,
            number("RP_SPACING", 0.62),
            number("RP_GAP", 0.0),
            number("RP_BURST", 0.0),
            punching,
            resting,
        );
        println!("second  arms left (puncher, other)  thrown  out s  landed  landing m/s");
        for (index, second) in flurry.seconds.iter().enumerate() {
            println!(
                "{:6}  {:>8.3} {:>8.3}             {:>6}  {:>5.3}  {:>6}  {:>6.2}",
                index + 1,
                second.arms[0],
                second.arms[1],
                second.thrown,
                second.out_seconds(),
                second.landed,
                second.landing_speed(),
            );
        }
    }

    /// Pulls `muscle` at `load` for `seconds`, and says what it can give
    /// then.
    fn hold(muscle: &mut MuscleGroup, load: f64, seconds: f64) -> f64 {
        for _ in 0..(seconds / DT).round() as usize {
            muscle.work(load, 0.0, DT, 1.0);
        }
        muscle.capacity()
    }

    /// How long `rates`' muscles hold `load` before what they can give falls
    /// under it: the model's endurance time, in seconds.
    fn endurance(rates: FatigueRates, load: f64) -> f64 {
        let mut muscle = MuscleGroup::tiring(rates, 0.0);
        let mut seconds = 0.0;
        while muscle.capacity() > load && seconds < 3600.0 {
            muscle.work(load, 0.0, DT, 1.0);
            seconds += DT;
        }
        seconds
    }

    /// Punching flat out, a fighter's arms tire, and its punches go out
    /// slower and land slower; resting, its arms come back. The fighter it
    /// punches, standing in its guard, hardly tires.
    #[test]
    fn a_fighter_punching_flat_out_tires_and_recovers() {
        let flurry = flurry(Materials::default(), 0.62, 0.0, 0.0, 20.0, 15.0);
        let at = |second: usize| flurry.seconds[second - 1];
        let tired = at(20).arms[0];
        assert!(
            (0.6..0.92).contains(&tired),
            "after 20 s its arms have {tired:.3}"
        );
        let (first, last) = (at(1).out_seconds(), at(20).out_seconds());
        assert!(
            last > 1.04 * first,
            "its punches went out in {first:.3} s, then {last:.3} s"
        );
        let rested = at(35).arms[0];
        assert!(
            rested > tired + 0.03,
            "rested 15 s, {tired:.3} came back to {rested:.3}"
        );
        assert!(at(35).arms[1] > 0.95, "the other has {:.3}", at(35).arms[1]);
    }

    /// A fighter standing in its guard for half a minute keeps nearly all
    /// its strength: holding the arms up asks little of the shoulders.
    #[test]
    fn a_fighter_standing_in_its_guard_hardly_tires() {
        let flurry = flurry(Materials::default(), 0.78, 0.0, 0.0, 0.0, 30.0);
        let arms = flurry.seconds[29].arms;
        assert!(arms.iter().all(|&left| left > 0.98), "{arms:?}");
    }

    /// Fresh muscles give their whole strength; muscles with no rates never
    /// tire, however hard they pull.
    #[test]
    fn fresh_muscles_give_everything_and_untiring_ones_never_tire() {
        assert_eq!(
            MuscleGroup::tiring(SHOULDER, SHOULDER_FASTEST).capacity(),
            1.0
        );
        let mut untiring = MuscleGroup::default();
        assert_eq!(hold(&mut untiring, 1.0, 60.0), 1.0);
    }

    /// Held at their whole strength the shoulder's muscles give out within
    /// seconds, and held at a fifth of it they last minutes, as the
    /// endurance curves the rates were fitted to have it; the shoulder
    /// gives out sooner than the elbow, the most fatigable joint measured
    /// (Frey-Law and Avin 2010).
    #[test]
    fn muscles_give_out_sooner_the_harder_they_pull() {
        let all_out = endurance(SHOULDER, 0.95);
        let half = endurance(SHOULDER, 0.5);
        let fifth = endurance(SHOULDER, 0.2);
        assert!(all_out < 10.0, "held at 95% for {all_out:.1} s");
        assert!((30.0..120.0).contains(&half), "held at 50% for {half:.1} s");
        assert!(fifth > 2.0 * 60.0, "held at 20% for {fifth:.1} s");
        let elbow = endurance(ELBOW_FLEXORS, 0.5);
        assert!(
            elbow > half,
            "the elbow held half for {elbow:.1} s, the shoulder {half:.1}"
        );
    }

    /// Worked hard, then let rest, tired muscles come back, faster while
    /// they rest than while they hold a load that stops the blood; never
    /// past fresh.
    #[test]
    fn tired_muscles_recover_faster_at_rest() {
        let mut resting = MuscleGroup::tiring(ELBOW_EXTENSORS, ELBOW_FASTEST);
        let tired = hold(&mut resting, 1.0, 20.0);
        assert!(tired < 0.9, "after 20 s flat out it has {tired:.2}");
        let mut loaded = resting;
        let rested = hold(&mut resting, 0.0, 30.0);
        let held = hold(&mut loaded, 0.5, 30.0);
        assert!(
            rested > tired && rested > held + 0.05,
            "rested {rested:.2}, held {held:.2}"
        );
        assert!(hold(&mut resting, 0.0, 3600.0) <= 1.0);
    }
}
