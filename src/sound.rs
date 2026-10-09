//! The game's sound, made from what the simulation does, with no recordings.
//!
//! [`Foley`] listens to the world after each step and hears what happened: a
//! thud as loud as the blow was fast when a bat or hammer lands, a crack for
//! each break, a wet tear for flesh torn by a blow, a spatter when fresh blood
//! lands, air rushing past a swung tool, and a slice while the knife cuts.
//! [`SoundBank`] synthesizes those sounds once (`synth.rs`) and [`Voices`]
//! says which clip plays for each and how loud. The app plays them through
//! macroquad; [`Mixdown`] mixes the same thing offline, so tools and tests can
//! listen to a scenario (`strike_scenarios --sound DIR`).
//!
//! Rebuilt from the foley that makes the sound of the project's videos
//! (`video_tools/tools/foley.py`, which reads the same counts from the capture
//! mode's `--events` log), so the game sounds like them.

mod mixdown;
mod synth;

pub use mixdown::Mixdown;

use crate::{ToolMode, World};

/// Samples per second of every clip: the rate quad-snd mixes at natively, so
/// nothing is resampled there.
pub const SAMPLE_RATE: u32 = 44_100;

/// Overall loudness: a full-strength thud peaks at about half of full scale,
/// so a thud, a crack, and a tear landing together reach it, and
/// [`soft_clip`] rounds those peaks off. Measured with `strike_scenarios
/// --sound`, a full sledgehammer blow is about -16 LUFS over its loudest
/// 400 ms, a knife cut -22, and a slow push -37.
pub const MASTER_VOLUME: f32 = 0.6;

/// The looped sounds: air past a bat or hammer (low and high), past the knife
/// (low and high), and the knife cutting.
pub const BED_COUNT: usize = 5;

/// A blow lands as loud as it was fast: silent below this tool speed (px/s)...
const THUD_FROM: f64 = 150.0;
/// ...and full strength this much faster.
const THUD_SPAN: f64 = 2400.0;
/// Air starts rushing past a tool above this speed (px/s)...
const RUSH_FROM: f64 = 250.0;
/// ...and is at its loudest this much faster.
const RUSH_SPAN: f64 = 2600.0;
/// Fibers the knife cuts in one step for a full-strength slice.
const SLICE_FIBERS: f32 = 6.0;
/// Fibers a blow tears in one step for a full-strength tear.
const TEAR_FIBERS: f32 = 30.0;
/// Fresh blood drops landing together within this long make one spatter...
const SPATTER_WINDOW: f64 = 0.12;
/// ...if there are at least this many...
const SPATTER_MIN_DROPS: i32 = 12;
/// ...at full strength with this many.
const SPATTER_FULL_DROPS: f32 = 80.0;
/// Shortest gap between two cues of a kind (thud, crack, tear, spatter), so a
/// crush that breaks bone after bone, or flesh tearing step after step, does
/// not stack a wall of sound. A cue much stronger than the last still sounds.
const COOLDOWN: [f64; 4] = [0.08, 0.06, 0.08, 0.0];

/// A one-off sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CueKind {
    /// A bat or hammer landing on the body.
    Thud,
    /// A bone breaking.
    Crack,
    /// Skin or muscle torn by a blow.
    Tear,
    /// Fresh blood landing.
    Spatter,
}

impl CueKind {
    pub const ALL: [CueKind; 4] = [
        CueKind::Thud,
        CueKind::Crack,
        CueKind::Tear,
        CueKind::Spatter,
    ];

    fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            CueKind::Thud => "thud",
            CueKind::Crack => "crack",
            CueKind::Tear => "tear",
            CueKind::Spatter => "spatter",
        }
    }
}

/// A one-off sound and how strong it is, from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cue {
    pub kind: CueKind,
    pub strength: f32,
}

/// How loud the continuous sounds should be after a step, from 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Beds {
    /// Air rushing past a tool swung with the button down, clear of the body.
    pub rush: f32,
    /// The rushing tool is the knife, which sounds thinner than a bat or hammer.
    pub sharp: bool,
    /// The knife cutting through flesh.
    pub slice: f32,
}

/// What one step sounded like.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Heard {
    pub cues: Vec<Cue>,
    pub beds: Beds,
}

/// The counts and contact the sound is heard from, as of one step.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Tally {
    touching: bool,
    speed: f64,
    fractures: i32,
    torn: i32,
    /// Blood drops released, without those leaking from wounds.
    fresh_blood: i32,
}

impl Tally {
    fn of(world: &World) -> Tally {
        let stats = world.stats();
        Tally {
            touching: crate::scenarios::tool_touching(world),
            speed: world.debug().striker_speed,
            fractures: stats.fractured_bones,
            torn: stats.broken_skin + stats.broken_muscle,
            fresh_blood: stats.emitted_fluid_particles - stats.wound_fluid_particles,
        }
    }

    /// A count went down, so this is a new body.
    fn restarted_from(&self, last: &Tally) -> bool {
        self.fractures < last.fractures
            || self.torn < last.torn
            || self.fresh_blood < last.fresh_blood
    }
}

/// Listens to the world after each step and says what it sounded like.
#[derive(Clone, Debug)]
pub struct Foley {
    last: Option<Tally>,
    /// Seconds since each kind of cue last sounded, and how strong it was.
    since: [f64; 4],
    last_strength: [f32; 4],
    /// Fresh blood drops gathered for the next spatter, and for how long.
    blood: Option<(i32, f64)>,
}

impl Default for Foley {
    fn default() -> Foley {
        Foley::new()
    }
}

impl Foley {
    pub fn new() -> Foley {
        Foley {
            last: None,
            since: [f64::INFINITY; 4],
            last_strength: [0.0; 4],
            blood: None,
        }
    }

    /// What the step the world just took sounded like. A new body (a reset,
    /// or a refit to the window) is heard silently, as the baseline.
    pub fn hear(&mut self, world: &World) -> Heard {
        let dt = world.materials().fixed_dt;
        let now = Tally::of(world);
        let last = match self.last.replace(now) {
            Some(last) if !now.restarted_from(&last) => last,
            _ => {
                self.blood = None;
                return Heard::default();
            }
        };
        for since in &mut self.since {
            *since += dt;
        }

        let debug = world.debug();
        let sharp = debug.tool == ToolMode::Sharp;
        let mut heard = Heard::default();
        heard.beds.sharp = sharp;
        if debug.down && !now.touching {
            heard.beds.rush = ((now.speed - RUSH_FROM) / RUSH_SPAN)
                .clamp(0.0, 1.0)
                .powf(1.3) as f32;
        }

        // On the step a tool lands the solver has already slowed it, so the
        // blow is as fast as the faster of the two steps.
        if now.touching && !last.touching && !sharp {
            let strength = ((now.speed.max(last.speed) - THUD_FROM) / THUD_SPAN).clamp(0.03, 1.0);
            self.cue(&mut heard, CueKind::Thud, strength as f32);
        }
        let fractures = now.fractures - last.fractures;
        if fractures > 0 {
            self.cue(
                &mut heard,
                CueKind::Crack,
                (0.6 + 0.2 * fractures as f32).min(1.0),
            );
        }
        let torn = (now.torn - last.torn) as f32;
        if sharp && now.touching {
            heard.beds.slice = (torn / SLICE_FIBERS).min(1.0);
        } else if torn > 0.0 {
            self.cue(&mut heard, CueKind::Tear, (torn / TEAR_FIBERS).min(1.0));
        }

        // Drops land a moment after they fly, so a spatter sounds when its
        // window closes; a wound's slow leak stays quiet.
        let fresh = (now.fresh_blood - last.fresh_blood).max(0);
        self.blood = match self.blood {
            Some((drops, age)) => Some((drops + fresh, age + dt)),
            None if fresh > 0 => Some((fresh, 0.0)),
            None => None,
        };
        if let Some((drops, age)) = self.blood {
            if age >= SPATTER_WINDOW - 1.0e-9 {
                self.blood = None;
                if drops >= SPATTER_MIN_DROPS {
                    let strength = (drops as f32 / SPATTER_FULL_DROPS).min(1.0);
                    self.cue(&mut heard, CueKind::Spatter, strength);
                }
            }
        }
        heard
    }

    fn cue(&mut self, heard: &mut Heard, kind: CueKind, strength: f32) {
        let i = kind.index();
        if self.since[i] < COOLDOWN[i] && strength < self.last_strength[i] + 0.3 {
            return;
        }
        self.since[i] = 0.0;
        self.last_strength[i] = strength;
        heard.cues.push(Cue { kind, strength });
    }
}

/// A clip of the bank: mono samples at [`SAMPLE_RATE`], loudest at 1.
#[derive(Clone, Debug)]
pub struct Clip {
    pub name: String,
    pub samples: Vec<f32>,
    /// Plays on a loop for as long as the app runs, at the volume [`Voices::beds`] gives.
    pub looped: bool,
}

/// A clip to play and how loud.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Voice {
    pub clip: usize,
    pub volume: f32,
}

/// Thuds are synthesized at these strengths: a harder blow is deeper, rings
/// longer, and slaps louder, so its sound changes with more than its volume.
const THUD_TIERS: [f32; 3] = [0.12, 0.45, 0.9];
/// Takes of each sound, slightly apart in pitch, so a sound heard twice in a
/// row differs.
const THUD_PITCHES: [f64; 3] = [1.0, 0.94, 1.06];
const CRACK_PITCHES: [f64; 3] = [1.0, 0.92, 1.08];
const TEAR_TIERS: [f32; 2] = [0.3, 0.85];
const TEAR_PITCHES: [f64; 2] = [1.0, 0.9];
const SPATTER_TIERS: [f32; 2] = [0.3, 0.9];
const SPATTER_PITCHES: [f64; 2] = [1.0, 1.08];
/// Air at full rush, and the knife at full slice, before the master volume.
const RUSH_VOLUME: f32 = 0.35;
const SLICE_VOLUME: f32 = 0.45;
const CLICK_VOLUME: f32 = 0.2;

/// Which clip of the bank plays for what, and how loud.
#[derive(Clone, Debug)]
pub struct Voices {
    thuds: [[usize; THUD_PITCHES.len()]; THUD_TIERS.len()],
    cracks: [usize; CRACK_PITCHES.len()],
    tears: [[usize; TEAR_PITCHES.len()]; TEAR_TIERS.len()],
    spatters: [[usize; SPATTER_PITCHES.len()]; SPATTER_TIERS.len()],
    click: usize,
    beds: [usize; BED_COUNT],
}

impl Voices {
    /// The clip for `cue` and how loud; `take` picks among the takes.
    pub fn cue(&self, cue: Cue, take: usize) -> Voice {
        let s = cue.strength.clamp(0.0, 1.0);
        // Peak levels as foley.py gives them.
        let (clip, peak) = match cue.kind {
            CueKind::Thud => (
                self.thuds[nearest(&THUD_TIERS, s)][take % THUD_PITCHES.len()],
                0.05 + 0.85 * s,
            ),
            CueKind::Crack => (self.cracks[take % CRACK_PITCHES.len()], 0.55 + 0.4 * s),
            CueKind::Tear => (
                self.tears[nearest(&TEAR_TIERS, s)][take % TEAR_PITCHES.len()],
                0.12 + 0.4 * s,
            ),
            CueKind::Spatter => (
                self.spatters[nearest(&SPATTER_TIERS, s)][take % SPATTER_PITCHES.len()],
                0.08 + 0.22 * s,
            ),
        };
        Voice {
            clip,
            volume: peak * MASTER_VOLUME,
        }
    }

    /// A soft tick for a control button or key.
    pub fn click(&self) -> Voice {
        Voice {
            clip: self.click,
            volume: CLICK_VOLUME * MASTER_VOLUME,
        }
    }

    /// Each looped clip and the volume it should have for these levels. The
    /// air brightens as it gets louder: its high band rises faster.
    pub fn beds(&self, beds: Beds) -> [Voice; BED_COUNT] {
        let rush = beds.rush.clamp(0.0, 1.0);
        let low = rush * (1.0 - 0.6 * rush) * RUSH_VOLUME * MASTER_VOLUME;
        let high = rush * rush * RUSH_VOLUME * MASTER_VOLUME;
        let (blunt, sharp) = if beds.sharp { (0.0, 1.0) } else { (1.0, 0.0) };
        let volumes = [
            low * blunt,
            high * blunt,
            low * sharp,
            high * sharp,
            beds.slice.clamp(0.0, 1.0) * SLICE_VOLUME * MASTER_VOLUME,
        ];
        std::array::from_fn(|i| Voice {
            clip: self.beds[i],
            volume: volumes[i],
        })
    }
}

/// Index of the value in `tiers` closest to `x`.
fn nearest(tiers: &[f32], x: f32) -> usize {
    (0..tiers.len())
        .min_by(|&a, &b| (tiers[a] - x).abs().total_cmp(&(tiers[b] - x).abs()))
        .unwrap_or(0)
}

/// Which take of each kind of sound plays next.
#[derive(Clone, Debug, Default)]
pub struct Takes {
    next: [usize; 4],
}

impl Takes {
    /// The voice for `cue`, a different take from the last of its kind.
    pub fn voice(&mut self, voices: &Voices, cue: Cue) -> Voice {
        let take = &mut self.next[cue.kind.index()];
        let voice = voices.cue(cue, *take);
        *take += 1;
        voice
    }
}

/// Every sound the game makes, synthesized.
#[derive(Clone, Debug)]
pub struct SoundBank {
    pub clips: Vec<Clip>,
    pub voices: Voices,
}

impl SoundBank {
    /// Synthesizes the bank, the same every time: about 20 seconds of audio,
    /// a few hundredths of a second's work in a release build.
    pub fn render() -> SoundBank {
        let mut synth = synth::Synth::new(0x5EED_50D5);
        let mut clips = Vec::new();
        let mut add = |name: String, samples: Vec<f32>, looped: bool| {
            clips.push(Clip {
                name,
                samples,
                looped,
            });
            clips.len() - 1
        };
        let thuds = THUD_TIERS.map(|strength| {
            THUD_PITCHES.map(|pitch| {
                add(
                    format!("thud-{strength}-{pitch}"),
                    synth.thud(strength as f64, pitch),
                    false,
                )
            })
        });
        let cracks =
            CRACK_PITCHES.map(|pitch| add(format!("crack-{pitch}"), synth.crack(pitch), false));
        let tears = TEAR_TIERS.map(|amount| {
            TEAR_PITCHES.map(|pitch| {
                add(
                    format!("tear-{amount}-{pitch}"),
                    synth.tear(amount as f64, pitch),
                    false,
                )
            })
        });
        let spatters = SPATTER_TIERS.map(|amount| {
            SPATTER_PITCHES.map(|pitch| {
                add(
                    format!("spatter-{amount}-{pitch}"),
                    synth.spatter(amount as f64, pitch),
                    false,
                )
            })
        });
        let click = add("click".to_string(), synth.click(), false);
        let beds = [
            add("rush-blunt-low".to_string(), synth.air(180.0, 900.0), true),
            add(
                "rush-blunt-high".to_string(),
                synth.air(900.0, 3200.0),
                true,
            ),
            add("rush-sharp-low".to_string(), synth.air(700.0, 2500.0), true),
            add(
                "rush-sharp-high".to_string(),
                synth.air(2500.0, 8000.0),
                true,
            ),
            add("slice".to_string(), synth.slice(), true),
        ];
        SoundBank {
            clips,
            voices: Voices {
                thuds,
                cracks,
                tears,
                spatters,
                click,
                beds,
            },
        }
    }
}

/// The output's last stage: samples pass unchanged up to 0.8, and louder ones
/// bend toward 1 along a tanh instead of clipping hard, as when a thud, a
/// crack, and a tear land at once. The patched quad-snd applies the same
/// curve to the app's sound, natively and in the browser.
pub fn soft_clip(sample: f32) -> f32 {
    const KNEE: f32 = 0.8;
    let magnitude = sample.abs();
    if magnitude <= KNEE {
        return sample;
    }
    (KNEE + (1.0 - KNEE) * ((magnitude - KNEE) / (1.0 - KNEE)).tanh()).copysign(sample)
}

/// How a looped sound's volume follows its target over `dt` seconds: it
/// rises within about 15 ms and falls over about 70 ms, so a knife that cuts
/// on one step and not the next slices on smoothly instead of stuttering.
pub fn ease_bed(volume: f32, target: f32, dt: f32) -> f32 {
    let tau = if target > volume { 0.015 } else { 0.07 };
    let eased = target + (volume - target) * (-dt / tau).exp();
    if eased.abs() < 1.0e-4 {
        0.0
    } else {
        eased
    }
}

/// `samples` as a 16-bit mono WAV file at [`SAMPLE_RATE`], the form the app
/// hands clips to the audio device in, and tools write for people to hear.
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + samples.len() * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // bytes per second
    wav.extend_from_slice(&2u16.to_le_bytes()); // bytes per sample frame
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for &sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        wav.extend_from_slice(&value.to_le_bytes());
    }
    wav
}
