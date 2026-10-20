//! Random fights: short fights played as the Fight page plays them, the left
//! fighter tapping punch and lean back at random as a player spamming the
//! buttons does, the right one punching now and then as the sparring partner
//! does, each watched by the watchdogs. The fight scenarios try a punch or a
//! few; this tries how players actually press. Every fight is written in the
//! syntax `--gesture` reads, so any of them replays exactly. `--mix` leaves
//! the leans or the partner's punches out, to tell which press does a harm,
//! and `--diff` sets two sets of fights side by side (`fights_diff.rs`).

use super::*;
use random::Dice;
use rp::inspect::{joint_name, Watchdog, Watchdogs};
use rp::scenarios::{Fight, GestureStep};

/// The file the fights are written to, one to a line, which `--diff` reads.
pub(crate) const FIGHTS_FILE: &str = "random_fights.csv";

/// How long the presses go on, in steps: five seconds, after which the
/// fighters have a second to come to rest.
const FIGHT_STEPS: i32 = 300;

/// The waits between a player's presses a fight draws from, in steps: from
/// a fast tap, 30 a second, to a pause of 0.4 s.
const PRESS_WAITS: [i32; 10] = [2, 3, 4, 5, 6, 8, 10, 12, 16, 24];

/// The shortest and the longest wait between the sparring partner's
/// punches, in steps: 1.5 to 3.5 s, as the game's `PUNCH_GAP` waits.
const PARTNER_WAITS: (i32, i32) = (90, 210);

/// Which presses the fights play (`--mix`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mix {
    /// As the Fight page is played: the player taps punch and lean back,
    /// and the sparring partner punches back.
    Game,
    /// The player's punches alone, the other fighter standing in its guard.
    Spam,
    /// The player's punches and leans, the other fighter standing in its
    /// guard.
    SpamLean,
}

impl Mix {
    pub(crate) fn parse(name: &str) -> Result<Mix, String> {
        match name {
            "game" => Ok(Mix::Game),
            "spam" => Ok(Mix::Spam),
            "spam-lean" => Ok(Mix::SpamLean),
            other => Err(format!(
                "unknown mix `{other}`; use game, spam, or spam-lean"
            )),
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Mix::Game => "game",
            Mix::Spam => "spam",
            Mix::SpamLean => "spam-lean",
        }
    }

    /// Whether the player leans back as well as punching.
    fn leans(self) -> bool {
        self != Mix::Spam
    }

    /// Whether the other fighter punches back.
    fn partner_punches(self) -> bool {
        self == Mix::Game
    }

    /// The presses in words, for the report.
    fn describe(self) -> &'static str {
        match self {
            Mix::Game => "the left fighter taps punch and lean back at random, a press every 2 to 24 steps, for 5 s; the right one punches every 1.5 to 3.5 s",
            Mix::Spam => "the left fighter taps punch at random, a press every 2 to 24 steps, for 5 s; the right one stands in its guard",
            Mix::SpamLean => "the left fighter taps punch and lean back at random, a press every 2 to 24 steps, for 5 s; the right one stands in its guard",
        }
    }
}

/// One fight's presses: the left fighter (body 0) punches with the arm
/// nearer the other or holds and lets go of a lean back, each press a random
/// wait after the last, while the right fighter (body 1) punches once each
/// of its own waits. A mix that leaves the leans or the right fighter's
/// punches out draws the dice all the same, so the left fighter's punches
/// land on the same steps in every mix, and the game mix plays the fights
/// `--fights` played before there were mixes.
fn random_fight(dice: &mut Dice, mix: Mix) -> Vec<GestureStep> {
    let mut steps = vec![
        GestureStep::Move {
            to: (-1.3, 0.95),
            frames: 0,
        },
        GestureStep::Wait(10),
    ];
    let (mut frame, mut leaning) = (10, false);
    let mut partner = dice.whole(PARTNER_WAITS.0, PARTNER_WAITS.1);
    while frame < FIGHT_STEPS {
        let roll = dice.range(0.0, 1.0);
        if roll < 0.55 {
            steps.push(GestureStep::Punch(0, rp::Limb::RightArm));
        } else if roll < 0.75 && mix.leans() {
            leaning = !leaning;
            steps.push(GestureStep::Lean(0, leaning));
        }
        let mut wait = dice.pick(&PRESS_WAITS);
        if frame + wait >= partner {
            if mix.partner_punches() {
                let first = partner - frame;
                if first > 0 {
                    steps.push(GestureStep::Wait(first));
                }
                steps.push(GestureStep::Punch(1, rp::Limb::LeftArm));
                wait -= first.max(0);
                frame = partner;
            }
            partner += dice.whole(PARTNER_WAITS.0, PARTNER_WAITS.1);
        }
        if wait > 0 {
            steps.push(GestureStep::Wait(wait));
            frame += wait;
        }
    }
    if leaning {
        steps.push(GestureStep::Lean(0, false));
    }
    steps.push(GestureStep::Wait(60));
    steps
}

/// What one fight did to each fighter, the left (0) and the right (1).
struct Fought {
    play: Play,
    /// The joints torn apart and slipped out of place, by name.
    torn: [Vec<String>; 2],
    slipped: [Vec<String>; 2],
    bones_broken: [i32; 2],
    skin_torn: [i32; 2],
    /// The first trip of each watchdog: the frame and the thing, as `--why`
    /// takes it and by name.
    flagged: Vec<(Watchdog, i32, String)>,
}

/// Each fighter's torn and slipped joints in `world`, by name.
fn joints_hurt(world: &rp::World) -> ([Vec<String>; 2], [Vec<String>; 2]) {
    let (mut torn, mut slipped) = ([Vec::new(), Vec::new()], [Vec::new(), Vec::new()]);
    let bones = world.bones();
    for (index, joint) in world.bone_joints().iter().enumerate() {
        let Some(bone) = bones.get(joint.a) else {
            continue;
        };
        let body = bone.body as usize;
        if body > 1 || joint.fracture_link {
            continue;
        }
        if joint.broken {
            torn[body].push(joint_name(world, index));
        } else if joint.subluxated {
            slipped[body].push(joint_name(world, index));
        }
    }
    (torn, slipped)
}

/// Plays `count` random fights of `mix` from `seed`, on all cores, and
/// reports which fighter's joints tore or slipped, which bones broke and
/// what the watchdogs flagged, with fights to replay.
pub(crate) fn random_fights(count: usize, seed: u64, mix: Mix, output_dir: &Path) {
    let started = Instant::now();
    let mut dice = Dice(seed);
    let plays: Vec<Play> = (0..count)
        .map(|_| {
            Play::Gesture(rp::scenarios::Gesture {
                tool: rp::ToolMode::Heavy,
                power: rp::swing_power(rp::ToolMode::Heavy),
                steps: Vec::leak(random_fight(&mut dice, mix)),
                fight: Some(Fight {
                    spacing: rp::FIGHTER_SPACING,
                    guard_down: false,
                    aim: 0.0,
                }),
            })
        })
        .collect();
    let next = AtomicUsize::new(0);
    let finished = Mutex::new(Vec::with_capacity(plays.len()));
    let workers = thread::available_parallelism().map_or(4, |count| count.get());
    thread::scope(|scope| {
        for _ in 0..workers.min(plays.len()) {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(&play) = plays.get(index) else {
                    break;
                };
                let scenario = Scenario::custom(play);
                let body = scenario.world_from(settled_body(
                    SCENARIO_WIDTH,
                    SCENARIO_HEIGHT,
                    scenario.materials(),
                ));
                let mut watchdogs = Watchdogs::new(&body);
                let (world, _) = run_on(
                    body,
                    &scenario,
                    SCENARIO_WIDTH,
                    SCENARIO_HEIGHT,
                    |frame, _, world| watchdogs.check(frame, world),
                );
                let (torn, slipped) = joints_hurt(&world);
                let state = |body: usize| world.body_state(body);
                let bones_broken =
                    [0, 1].map(|body| state(body).map_or(0, |state| state.fractures.iter().sum()));
                let skin_torn = [0, 1].map(|body| state(body).map_or(0, |state| state.skin_tears));
                let flagged = flagged(&world, watchdogs.trips());
                finished.lock().expect("fight results").push((
                    index,
                    Fought {
                        play,
                        torn,
                        slipped,
                        bones_broken,
                        skin_torn,
                        flagged,
                    },
                ));
            });
        }
    });
    let mut fought = finished.into_inner().expect("fight results");
    fought.sort_by_key(|(index, _)| *index);
    let fought: Vec<Fought> = fought.into_iter().map(|(_, fight)| fight).collect();

    // What the fights were played with comes first, as a sweep's runs file
    // says it, for `--diff` to name each side; the torn and slipped joints'
    // names come last, after the columns fights were first written with.
    let csv_path = output_dir.join(FIGHTS_FILE);
    let mut csv = BufWriter::new(File::create(&csv_path).expect("create fights CSV"));
    writeln!(
        csv,
        "# settings: {}\n# trying: {}\n# seed: {seed}\n# mix: {}",
        rp::scenarios::settings_used().join(" "),
        env::var("RP_TRY").unwrap_or_default(),
        mix.name()
    )
    .expect("write fights CSV");
    writeln!(
        csv,
        "index,play,left_joints_torn,right_joints_torn,left_joints_slipped,right_joints_slipped,left_bones_broken,right_bones_broken,left_skin_torn,right_skin_torn,watchdogs,left_torn_joints,right_torn_joints,left_slipped_joints,right_slipped_joints"
    )
    .expect("write fights CSV");
    for (index, fight) in fought.iter().enumerate() {
        let watchdogs: Vec<String> = fight
            .flagged
            .iter()
            .map(|(watchdog, frame, _)| format!("{watchdog:?}@{frame}"))
            .collect();
        writeln!(
            csv,
            "{index},\"{}\",{},{},{},{},{},{},{},{},{},\"{}\",\"{}\",\"{}\",\"{}\"",
            fight.play,
            fight.torn[0].len(),
            fight.torn[1].len(),
            fight.slipped[0].len(),
            fight.slipped[1].len(),
            fight.bones_broken[0],
            fight.bones_broken[1],
            fight.skin_torn[0],
            fight.skin_torn[1],
            watchdogs.join(";"),
            fight.torn[0].join(";"),
            fight.torn[1].join(";"),
            fight.slipped[0].join(";"),
            fight.slipped[1].join(";")
        )
        .expect("write fights CSV");
    }

    let mut report = format!(
        "Random fights: {} fights from seed {seed}{}, in {:.0} s: {}\n",
        fought.len(),
        if mix == Mix::Game {
            String::new()
        } else {
            format!(", the {} mix", mix.name())
        },
        started.elapsed().as_secs_f64(),
        mix.describe()
    );
    let fighters = ["the left fighter's", "the right fighter's"];
    let tally = |what: &str, count: &dyn Fn(&Fought, usize) -> usize| {
        let parts: Vec<String> = (0..2)
            .map(|side| {
                let fights = fought.iter().filter(|fight| count(fight, side) > 0).count();
                let total: usize = fought.iter().map(|fight| count(fight, side)).sum();
                format!("{} in {fights} fights ({total} in all)", fighters[side])
            })
            .collect();
        format!("  {what}: {}\n", parts.join(", "))
    };
    report.push_str(&tally("joints torn apart", &|fight, side| {
        fight.torn[side].len()
    }));
    // Which joints tore, most often first, to tell a joint that comes apart
    // again and again from one that tore once.
    let mut by_joint: Vec<(&str, usize)> = Vec::new();
    for name in fought.iter().flat_map(|fight| fight.torn.iter().flatten()) {
        match by_joint
            .iter_mut()
            .find(|(joint, _)| *joint == name.as_str())
        {
            Some((_, count)) => *count += 1,
            None => by_joint.push((name, 1)),
        }
    }
    by_joint.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    if !by_joint.is_empty() {
        let parts: Vec<String> = by_joint
            .iter()
            .map(|(joint, count)| format!("{joint} {count}"))
            .collect();
        report.push_str(&format!("    by joint: {}\n", parts.join(", ")));
    }
    report.push_str(&tally("joints slipped out of place", &|fight, side| {
        fight.slipped[side].len()
    }));
    report.push_str(&tally("bones broken", &|fight, side| {
        fight.bones_broken[side] as usize
    }));
    report.push_str(&tally("skin torn", &|fight, side| {
        fight.skin_torn[side] as usize
    }));
    let mut kinds: Vec<Watchdog> = fought
        .iter()
        .flat_map(|fight| fight.flagged.iter().map(|(watchdog, _, _)| *watchdog))
        .collect();
    kinds.sort();
    kinds.dedup();
    if kinds.is_empty() {
        report.push_str("The watchdogs flagged nothing.\n");
    } else {
        report.push_str("What the watchdogs flagged:\n");
        for kind in kinds {
            let tripped: Vec<&Fought> = fought
                .iter()
                .filter(|fight| {
                    fight
                        .flagged
                        .iter()
                        .any(|(watchdog, _, _)| *watchdog == kind)
                })
                .collect();
            report.push_str(&format!(
                "  {}{} in {} fights\n",
                kind.describe(),
                if kind.known_gap() { " [known gap]" } else { "" },
                tripped.len()
            ));
            for fight in tripped.iter().take(3) {
                let (_, frame, what) = fight
                    .flagged
                    .iter()
                    .find(|(watchdog, _, _)| *watchdog == kind)
                    .expect("it tripped");
                report.push_str(&format!(
                    "    --gesture \"{}\" from frame {frame}: {what}\n",
                    fight.play
                ));
            }
        }
    }
    let tore: Vec<&Fought> = fought
        .iter()
        .filter(|fight| fight.torn.iter().any(|joints| !joints.is_empty()))
        .collect();
    if !tore.is_empty() {
        report.push_str("Fights that tore a joint apart, the first five:\n");
        for fight in tore.iter().take(5) {
            let names: Vec<&str> = fight.torn.iter().flatten().map(String::as_str).collect();
            report.push_str(&format!(
                "  --gesture \"{}\": {}\n",
                fight.play,
                names.join(", ")
            ));
        }
    }
    let report_path = output_dir.join("random_fights_report.txt");
    fs::write(&report_path, &report).expect("write fights report");
    print!("{report}");
    println!("wrote {}", csv_path.display());
    println!("wrote {}", report_path.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The steps on which the left fighter punches in a fight, how many
    /// times it leans or lets go, and how many punches the right one throws.
    fn presses(steps: &[GestureStep]) -> (Vec<i32>, usize, usize) {
        let (mut frame, mut punches, mut leans, mut partner) = (0, Vec::new(), 0, 0);
        for step in steps {
            match *step {
                GestureStep::Wait(wait) => frame += wait,
                GestureStep::Punch(0, _) => punches.push(frame),
                GestureStep::Punch(_, _) => partner += 1,
                GestureStep::Lean(..) => leans += 1,
                _ => {}
            }
        }
        (punches, leans, partner)
    }

    /// Every mix draws the same dice, so the left fighter punches on the
    /// same steps in each, and a mix only leaves presses out.
    #[test]
    fn every_mix_punches_on_the_same_steps() {
        let fights = |mix| {
            let mut dice = Dice(7);
            (0..20)
                .map(|_| presses(&random_fight(&mut dice, mix)))
                .collect::<Vec<_>>()
        };
        let (game, spam, spam_lean) = (fights(Mix::Game), fights(Mix::Spam), fights(Mix::SpamLean));
        for ((game, spam), spam_lean) in game.iter().zip(&spam).zip(&spam_lean) {
            assert_eq!(game.0, spam.0);
            assert_eq!(game.0, spam_lean.0);
            assert_eq!((spam.1, spam.2), (0, 0), "spam only punches");
            assert_eq!(spam_lean.1, game.1, "spam-lean leans as the game does");
            assert_eq!(spam_lean.2, 0, "spam-lean's partner stands");
            assert!(game.2 > 0, "the game's partner punches back");
        }
    }

    #[test]
    fn mixes_read_by_name() {
        for mix in [Mix::Game, Mix::Spam, Mix::SpamLean] {
            assert_eq!(Mix::parse(mix.name()), Ok(mix));
        }
        assert!(Mix::parse("lean").is_err());
    }
}
