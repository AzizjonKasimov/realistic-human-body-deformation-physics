//! Scripted strikes and gestures: hand paths in body coordinates and the
//! pointer input that plays them, plus the tuned scenarios with the injury and
//! steadiness bands each one should land in. The strike scenario runner, the
//! visual damage diagnostic, and the app's capture and self-test modes all
//! play them from here, so a scenario plays the same everywhere, in the
//! browser build too.

use std::fmt;

use crate::{
    body_frame, create_layered_body, swing_power, BodyFrame, BonePart, BoneSegment, InputState,
    Materials, TissueLayer, ToolMode, Vec2, World, BONE_PARTS, MISSING_SPRING,
};

/// Window size the tuned scenarios are played in.
pub const SCENARIO_WIDTH: f64 = 1280.0;
pub const SCENARIO_HEIGHT: f64 = 720.0;

/// Frames the hand holds at the end of a swing with the button down, since the
/// tool trails the hand.
pub const FOLLOW_THROUGH_FRAMES: i32 = 20;

/// A scripted swing. After `windup_frames` the hand moves in a straight line
/// from `start` to `end` (body coordinates) over `strike_frames`, holds there
/// with the button down for the follow-through, then lets go while the body
/// settles until `settle_frames` after the swing. `power` is the tool's
/// [`swing_power`] unless an experiment overrides it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    pub tool: ToolMode,
    pub start: (f64, f64),
    pub end: (f64, f64),
    pub windup_frames: i32,
    pub strike_frames: i32,
    pub settle_frames: i32,
    pub power: f64,
}

impl Strike {
    pub fn frames(&self) -> i32 {
        self.windup_frames + self.strike_frames + self.settle_frames
    }

    /// Pointer input on `frame` of this strike against a body placed at `body`.
    pub fn input(&self, frame: i32, dt: f64, body: BodyFrame) -> InputState {
        let held_until = self.windup_frames + self.strike_frames + FOLLOW_THROUGH_FRAMES;
        if frame >= held_until {
            return InputState::default();
        }
        let start = body.point(self.start.0, self.start.1);
        let end = body.point(self.end.0, self.end.1);
        let duration = (self.strike_frames - 1).max(1) as f64;
        let t0 = (frame - self.windup_frames).max(0) as f64 / duration;
        let t = t0.clamp(0.0, 1.0);
        let moving = t0 < 1.0;
        // The swing's velocity only matters on the step the tool appears, so a
        // scripted strike can start mid-swing.
        let down = frame >= self.windup_frames;
        InputState {
            active: down,
            down,
            x: start.x + (end.x - start.x) * t,
            y: start.y + (end.y - start.y) * t,
            vx: if down && moving {
                (end.x - start.x) / (duration * dt)
            } else {
                0.0
            },
            vy: if down && moving {
                (end.y - start.y) / (duration * dt)
            } else {
                0.0
            },
            power: self.power,
            tool: self.tool,
        }
    }

    /// The same swing moved `along` its own direction and `across` it, in body
    /// heights. Small moves show how much an outcome hangs on exact aim.
    pub fn shifted(&self, along: f64, across: f64) -> Strike {
        let (du, dv) = (self.end.0 - self.start.0, self.end.1 - self.start.1);
        let length = libm::hypot(du, dv).max(1.0e-9);
        let (au, av) = (du / length, dv / length);
        let shift = (au * along - av * across, av * along + au * across);
        Strike {
            start: (self.start.0 + shift.0, self.start.1 + shift.1),
            end: (self.end.0 + shift.0, self.end.1 + shift.1),
            ..*self
        }
    }

    /// Parses `tool:u0,v0:u1,v1`, optionally followed by `:frames=16` (strike
    /// frames), `:windup=6`, `:settle=60`, and `:power=P` to try a strength
    /// other than the tool's swing power. Tools are `bat`, `knife`, or `hammer`
    /// (or `blunt`, `sharp`, `heavy`).
    pub fn parse(spec: &str) -> Result<Strike, String> {
        let mut parts = spec.split(':');
        let tool = parse_tool(parts.next().unwrap_or(""))?;
        let point = |text: Option<&str>| -> Result<(f64, f64), String> {
            let text = text.ok_or("expected tool:u0,v0:u1,v1")?;
            let (u, v) = text
                .split_once(',')
                .ok_or_else(|| format!("`{text}` should be u,v"))?;
            let number = |n: &str| {
                n.trim()
                    .parse::<f64>()
                    .map_err(|_| format!("`{n}` is not a number"))
            };
            Ok((number(u)?, number(v)?))
        };
        let mut strike = Strike {
            tool,
            start: point(parts.next())?,
            end: point(parts.next())?,
            windup_frames: 6,
            strike_frames: 16,
            settle_frames: 60,
            power: swing_power(tool),
        };
        for option in parts {
            let (key, value) = option
                .split_once('=')
                .ok_or_else(|| format!("`{option}` should be key=value"))?;
            let number = value
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("`{value}` is not a number"))?;
            match key.trim() {
                "power" | "p" => strike.power = number,
                "frames" | "f" => strike.strike_frames = number as i32,
                "windup" | "w" => strike.windup_frames = number as i32,
                "settle" | "s" => strike.settle_frames = number as i32,
                other => return Err(format!("unknown option `{other}`")),
            }
        }
        Ok(strike)
    }
}

/// Writes the swing in the syntax [`Strike::parse`] reads.
impl fmt::Display for Strike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{:.3},{:.3}:{:.3},{:.3}:power={}:frames={}:windup={}:settle={}",
            tool_spec_name(self.tool),
            self.start.0,
            self.start.1,
            self.end.0,
            self.end.1,
            self.power,
            self.strike_frames,
            self.windup_frames,
            self.settle_frames
        )
    }
}

/// One part of a [`Gesture`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureStep {
    /// The hand moves in a straight line to `to` (body coordinates) over
    /// `frames` steps, or jumps there when `frames` is zero.
    Move { to: (f64, f64), frames: i32 },
    /// The button goes down (`true`) or comes up.
    Press(bool),
    /// The hand stays still for this many steps.
    Wait(i32),
}

/// A tool used the way a person plays the app: the tool is in hand from the
/// first step and follows the pointer, and the button goes down and up
/// between moves of the hand. Unlike a [`Strike`], the tool starts at rest
/// where the hand first is, so the hand has to get it moving, and it hovers
/// over the body while the button is up, as in the app.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gesture {
    pub tool: ToolMode,
    pub power: f64,
    pub steps: &'static [GestureStep],
}

impl Gesture {
    pub fn frames(&self) -> i32 {
        self.steps
            .iter()
            .map(|step| match *step {
                GestureStep::Move { frames, .. } | GestureStep::Wait(frames) => frames.max(0),
                GestureStep::Press(_) => 0,
            })
            .sum()
    }

    /// Where the hand is (body coordinates) and whether the button is down on
    /// `frame`.
    pub fn hand(&self, frame: i32) -> ((f64, f64), bool) {
        let mut position = match self.steps.first() {
            Some(GestureStep::Move { to, .. }) => *to,
            _ => (0.0, 0.0),
        };
        let mut down = false;
        let mut elapsed = 0;
        for step in self.steps {
            match *step {
                GestureStep::Press(state) => down = state,
                GestureStep::Wait(frames) => {
                    if frame < elapsed + frames {
                        return (position, down);
                    }
                    elapsed += frames.max(0);
                }
                GestureStep::Move { to, frames } => {
                    if frames > 0 && frame < elapsed + frames {
                        let t = (frame - elapsed + 1) as f64 / frames as f64;
                        let along = (
                            position.0 + (to.0 - position.0) * t,
                            position.1 + (to.1 - position.1) * t,
                        );
                        return (along, down);
                    }
                    elapsed += frames.max(0);
                    position = to;
                }
            }
        }
        (position, down)
    }

    /// Pointer input on `frame` against a body placed at `body`. The tool is
    /// in play on every frame, as it is in the app.
    pub fn input(&self, frame: i32, body: BodyFrame) -> InputState {
        let ((u, v), down) = self.hand(frame);
        let hand = body.point(u, v);
        InputState {
            active: true,
            down,
            x: hand.x,
            y: hand.y,
            vx: 0.0,
            vy: 0.0,
            power: self.power,
            tool: self.tool,
        }
    }

    /// The same gesture with every point moved `du` across and `dv` down the
    /// body, in body heights. Its steps are leaked to keep `Gesture` `Copy`,
    /// which suits the short-lived programs that shift and parse gestures.
    pub fn shifted(&self, du: f64, dv: f64) -> Gesture {
        let steps: Vec<GestureStep> = self
            .steps
            .iter()
            .map(|step| match *step {
                GestureStep::Move { to, frames } => GestureStep::Move {
                    to: (to.0 + du, to.1 + dv),
                    frames,
                },
                other => other,
            })
            .collect();
        Gesture {
            steps: Vec::leak(steps),
            ..*self
        }
    }

    /// Parses `tool:` followed by steps separated by `:`: `u,v` puts the hand
    /// there at once, `u,v/N` moves it there over N steps, `down` and `up`
    /// press and release the button, and `wait=N` holds still for N steps.
    /// `power=P` tries a strength other than the tool's swing power. For
    /// example `bat:-0.3,0.4:wait=10:down:0.05,0.4/12:wait=40:up:wait=30`
    /// hovers, presses, swings into the chest, leans there, and lets go.
    pub fn parse(spec: &str) -> Result<Gesture, String> {
        let mut parts = spec.split(':');
        let tool = parse_tool(parts.next().unwrap_or(""))?;
        let mut power = swing_power(tool);
        let mut steps = Vec::new();
        for part in parts {
            let part = part.trim();
            let number = |text: &str| {
                text.trim()
                    .parse::<f64>()
                    .map_err(|_| format!("`{text}` is not a number"))
            };
            match part {
                "down" => steps.push(GestureStep::Press(true)),
                "up" => steps.push(GestureStep::Press(false)),
                _ => {
                    if let Some(frames) = part.strip_prefix("wait=") {
                        steps.push(GestureStep::Wait(number(frames)? as i32));
                    } else if let Some(value) = part.strip_prefix("power=") {
                        power = number(value)?;
                    } else {
                        let (point, frames) = match part.split_once('/') {
                            Some((point, frames)) => (point, number(frames)? as i32),
                            None => (part, 0),
                        };
                        let (u, v) = point
                            .split_once(',')
                            .ok_or_else(|| format!("`{part}` should be u,v or u,v/N"))?;
                        steps.push(GestureStep::Move {
                            to: (number(u)?, number(v)?),
                            frames,
                        });
                    }
                }
            }
        }
        if !matches!(steps.first(), Some(GestureStep::Move { .. })) {
            return Err("a gesture starts with the point where the hand is".to_string());
        }
        Ok(Gesture {
            tool,
            power,
            steps: Vec::leak(steps),
        })
    }
}

/// Writes the gesture in the syntax [`Gesture::parse`] reads.
impl fmt::Display for Gesture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", tool_spec_name(self.tool))?;
        for step in self.steps {
            match *step {
                GestureStep::Move { to, frames } if frames > 0 => {
                    write!(f, ":{:.3},{:.3}/{frames}", to.0, to.1)?
                }
                GestureStep::Move { to, .. } => write!(f, ":{:.3},{:.3}", to.0, to.1)?,
                GestureStep::Press(true) => write!(f, ":down")?,
                GestureStep::Press(false) => write!(f, ":up")?,
                GestureStep::Wait(frames) => write!(f, ":wait={frames}")?,
            }
        }
        write!(f, ":power={}", self.power)
    }
}

/// What a scenario plays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Play {
    /// A scripted swing, then maybe a second one after the first one's settle
    /// frames.
    Swing(Strike, Option<Strike>),
    /// A tool used the way a person plays the app.
    Gesture(Gesture),
}

impl Play {
    /// Whether every tool the play uses is the knife.
    pub fn knife_only(&self) -> bool {
        match self {
            Play::Swing(strike, followup) => {
                strike.tool == ToolMode::Sharp
                    && followup.map_or(true, |next| next.tool == ToolMode::Sharp)
            }
            Play::Gesture(gesture) => gesture.tool == ToolMode::Sharp,
        }
    }

    /// The tool the play starts with.
    pub fn tool(&self) -> ToolMode {
        match self {
            Play::Swing(strike, _) => strike.tool,
            Play::Gesture(gesture) => gesture.tool,
        }
    }

    pub fn frames(&self) -> i32 {
        match self {
            Play::Swing(strike, followup) => {
                strike.frames() + followup.map_or(0, |followup| followup.frames())
            }
            Play::Gesture(gesture) => gesture.frames(),
        }
    }

    /// Pointer input on `frame` against a body placed at `body`.
    pub fn input(&self, frame: i32, dt: f64, body: BodyFrame) -> InputState {
        match self {
            Play::Swing(strike, followup) => {
                let first = strike.frames();
                match followup {
                    Some(followup) if frame >= first => followup.input(frame - first, dt, body),
                    _ => strike.input(frame, dt, body),
                }
            }
            Play::Gesture(gesture) => gesture.input(frame, body),
        }
    }

    /// The play aimed a little differently: swings move `along` and `across`
    /// their own paths ([`Strike::shifted`]), and gestures move `along` across
    /// the body and `across` down it, all in body heights.
    pub fn shifted(&self, along: f64, across: f64) -> Play {
        match self {
            Play::Swing(strike, followup) => Play::Swing(
                strike.shifted(along, across),
                followup.map(|followup| followup.shifted(along, across)),
            ),
            Play::Gesture(gesture) => Play::Gesture(gesture.shifted(along, across)),
        }
    }
}

impl fmt::Display for Play {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Play::Swing(strike, None) => write!(f, "{strike}"),
            Play::Swing(strike, Some(followup)) => write!(f, "{strike} then {followup}"),
            Play::Gesture(gesture) => write!(f, "{gesture}"),
        }
    }
}

fn parse_tool(name: &str) -> Result<ToolMode, String> {
    match name.trim() {
        "bat" | "blunt" => Ok(ToolMode::Blunt),
        "knife" | "sharp" => Ok(ToolMode::Sharp),
        "hammer" | "heavy" | "sledgehammer" => Ok(ToolMode::Heavy),
        other => Err(format!("unknown tool `{other}`; use bat, knife, or hammer")),
    }
}

fn tool_spec_name(tool: ToolMode) -> &'static str {
    match tool {
        ToolMode::Blunt => "bat",
        ToolMode::Sharp => "knife",
        ToolMode::Heavy => "hammer",
    }
}

#[derive(Clone, Copy, Debug)]
pub struct IntBand {
    pub min: i32,
    pub max: i32,
}

impl IntBand {
    pub const fn range(min: i32, max: i32) -> Self {
        Self { min, max }
    }

    pub const fn at_least(min: i32) -> Self {
        Self { min, max: i32::MAX }
    }

    pub fn contains(&self, value: i32) -> bool {
        (self.min..=self.max).contains(&value)
    }
}

impl Default for IntBand {
    fn default() -> Self {
        Self::at_least(0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DoubleBand {
    pub min: f64,
    pub max: f64,
}

impl DoubleBand {
    pub const fn range(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    pub fn contains(&self, value: f64) -> bool {
        value >= self.min && value <= self.max
    }
}

impl Default for DoubleBand {
    fn default() -> Self {
        Self::range(0.0, f64::INFINITY)
    }
}

/// Injury bands a scenario should land in; anything left at its default is
/// unchecked.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScenarioExpectations {
    pub contacts: IntBand,
    pub bone_fractures: IntBand,
    pub rib_fractures: IntBand,
    pub skin_tears: IntBand,
    pub muscle_tears: IntBand,
    pub contusion_events: IntBand,
    pub tissue_fatigue_events: IntBand,
    pub tissue_plastic_events: IntBand,
    pub tear_propagations: IntBand,
    pub muscle_fiber_tears: IntBand,
    pub muscle_crush_ruptures: IntBand,
    pub cavity_pressure_events: IntBand,
    pub cavity_ruptures: IntBand,
    pub organ_damage_events: IntBand,
    pub organ_penetrations: IntBand,
    pub rib_organ_punctures: IntBand,
    pub organ_ruptures: IntBand,
    pub skin_flap_detachments: IntBand,
    /// Tissue points split to open cuts.
    pub cut_openings: IntBand,
    pub vessel_lacerations: IntBand,
    pub fragment_vessel_lacerations: IntBand,
    pub wound_reopens: IntBand,
    pub fluid_emitted: IntBand,
    pub wound_fluid: IntBand,
    pub blood_stain_deposits: IntBand,
    /// How far blood ran down the skin, in pixels of trail.
    pub blood_trail_length: DoubleBand,
    pub fracture_marrow_sources: IntBand,
    pub opened_wounds: IntBand,
    pub final_free_fragments: IntBand,
    pub fragment_bone_contacts: IntBand,
    pub fragment_bone_damping_events: IntBand,
    pub fragment_bone_resting_contacts: IntBand,
    pub sleeping_fragments: IntBand,
    pub sleep_events: IntBand,
    pub final_sleeping_fragments: IntBand,
    pub fragment_pair_damping_events: IntBand,
    pub fragment_pair_resting_contacts: IntBand,
    pub fragment_floor_contacts: IntBand,
    pub fragment_floor_resting_contacts: IntBand,
    pub fragment_pair_contacts: IntBand,
    pub fragment_skin_punctures: IntBand,
    pub bone_joint_subluxations: IntBand,
    pub joint_ligament_damage_events: IntBand,
    pub joint_corrections: IntBand,
    pub fragment_overlap: DoubleBand,
    pub bone_spin: DoubleBand,
    pub bone_joint_subluxation: DoubleBand,
    pub tissue_softening: DoubleBand,
    pub tissue_fatigue: DoubleBand,
    pub tissue_plasticity: DoubleBand,
    pub cavity_pressure: DoubleBand,
    pub cavity_collapse: DoubleBand,
    pub organ_damage: DoubleBand,
    pub blood_loss: DoubleBand,
    pub final_blood_volume: DoubleBand,
    pub final_blood_turgor: DoubleBand,
    /// How steady the tool is: the most its long axis may turn in one step
    /// (degrees), how many steps it may snap round by more than
    /// [`TOOL_SNAP_DEGREES`], and how often a held tool may lose touch with
    /// the body and find it again.
    pub tool_turn: DoubleBand,
    pub tool_snaps: IntBand,
    /// All the tool's turning over the run, in degrees.
    pub tool_turning: DoubleBand,
    pub contact_toggles: IntBand,
    /// Fastest a tissue point may move, in pixels per second.
    pub point_speed: DoubleBand,
    /// Farthest the tool may trail the hand while it touches nothing, in
    /// pixels.
    pub free_lag: DoubleBand,
    /// How far the tool may be from the hand at the end, in pixels.
    pub final_lag: DoubleBand,
}

/// A tool whose axis turns more than this in one step has snapped round
/// rather than turned.
pub const TOOL_SNAP_DEGREES: f64 = 45.0;

/// The figure stands for an adult 1.75 m tall, weighing about 70 kg; the
/// injury checks compare it with injury research in real units
/// (`docs/INJURY_REFERENCE.md`).
pub const BODY_HEIGHT_M: f64 = 1.75;

/// Blood in that adult: about 70 ml per kg of body weight (Gutierrez et al.
/// 2004, "Clinical review: Hemorrhagic shock").
pub const BLOOD_VOLUME_ML: f64 = 4900.0;

/// Metres per pixel for a body `body_height` pixels tall.
pub fn metres_per_px(body_height: f64) -> f64 {
    BODY_HEIGHT_M / body_height.max(1.0)
}

/// Metres per pixel in the scenarios' window, where the body is 561.6 px
/// tall: about 3.1 mm.
pub fn scenario_metres_per_px() -> f64 {
    metres_per_px(body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT).height)
}

/// How often something happens in real life. The sweep judges it over its
/// replays of a scenario, aimed a little differently and on bones a little
/// weaker or stronger, as people's are; a single run can only break `Never`
/// and `Always`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Often {
    /// In no run.
    Never,
    /// In at most a third of the runs.
    Rarely,
    /// In some runs but not all.
    Sometimes,
    /// In at least half of the runs.
    Usually,
    /// In every run.
    Always,
}

impl Often {
    pub fn name(self) -> &'static str {
        match self {
            Often::Never => "never",
            Often::Rarely => "rarely",
            Often::Sometimes => "sometimes",
            Often::Usually => "usually",
            Often::Always => "always",
        }
    }

    /// Whether happening in `hits` of `runs` runs fits.
    pub fn fits(self, hits: usize, runs: usize) -> bool {
        match self {
            Often::Never => hits == 0,
            Often::Rarely => hits * 3 <= runs,
            Often::Sometimes => hits > 0 && hits < runs,
            Often::Usually => hits * 2 >= runs,
            Often::Always => hits == runs,
        }
    }
}

/// Something a blow does to the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A bone of this part breaks.
    Breaks(BonePart),
    /// Any bone breaks.
    BreaksAnyBone,
    /// The skin splits or is cut open.
    OpensSkin,
    /// A major blood vessel is cut or torn.
    CutsArtery,
    /// The flesh is bruised.
    Bruises,
    /// A bone breaks and the skin opens over it.
    OpenFracture,
    /// The skin splits though no bone breaks.
    SplitsSkinWithoutBreak,
}

impl Outcome {
    /// Whether it happened in `result`.
    pub fn happened(self, result: &ScenarioResult) -> bool {
        match self {
            Outcome::Breaks(part) => result.part_fractures[part.index()] > 0,
            Outcome::BreaksAnyBone => result.bone_fractures > 0,
            Outcome::OpensSkin => result.skin_tears > 0 || result.cut_openings > 0,
            Outcome::CutsArtery => {
                result.vessel_lacerations + result.fragment_vessel_lacerations > 0
            }
            Outcome::Bruises => result.contusion_events > 0,
            Outcome::OpenFracture => result.open_fractures > 0,
            Outcome::SplitsSkinWithoutBreak => {
                result.bone_fractures == 0 && Outcome::OpensSkin.happened(result)
            }
        }
    }

    pub fn describe(self) -> String {
        match self {
            Outcome::Breaks(part) => format!("the {} breaks", part.name()),
            Outcome::BreaksAnyBone => "a bone breaks".to_string(),
            Outcome::OpensSkin => "the skin opens".to_string(),
            Outcome::CutsArtery => "an artery is cut".to_string(),
            Outcome::Bruises => "the flesh bruises".to_string(),
            Outcome::OpenFracture => "a bone breaks open through the skin".to_string(),
            Outcome::SplitsSkinWithoutBreak => "the skin splits over unbroken bone".to_string(),
        }
    }

    /// A short name for reports.
    pub fn slug(self) -> String {
        match self {
            Outcome::Breaks(part) => format!("breaks_{}", part.name().replace(' ', "_")),
            Outcome::BreaksAnyBone => "breaks_any_bone".to_string(),
            Outcome::OpensSkin => "opens_skin".to_string(),
            Outcome::CutsArtery => "cuts_artery".to_string(),
            Outcome::Bruises => "bruises".to_string(),
            Outcome::OpenFracture => "open_fracture".to_string(),
            Outcome::SplitsSkinWithoutBreak => "splits_skin_without_break".to_string(),
        }
    }
}

/// Something a scenario must do as it happens in real life, how often, and
/// the research it rests on (`docs/INJURY_REFERENCE.md` has the details).
#[derive(Clone, Copy, Debug)]
pub struct RealCheck {
    pub outcome: Outcome,
    pub often: Often,
    pub source: &'static str,
    /// Why the engine cannot meet this yet, or empty. A known gap is still
    /// judged and reported, but apart from the checks that must pass.
    pub gap: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct Scenario {
    pub name: &'static str,
    pub region: &'static str,
    pub intent: &'static str,
    pub play: Play,
    /// Checks of the engine and of how the tools handle.
    pub expectations: ScenarioExpectations,
    /// What the play does in real life, from injury research.
    pub real: &'static [RealCheck],
    /// How strong the bones are, against the tuned strength: the sweep plays
    /// weaker and stronger bones too, as people's differ.
    pub bone_strength: f64,
}

impl Scenario {
    /// One custom swing or gesture, with no bands to check.
    pub fn custom(play: Play) -> Scenario {
        Scenario {
            name: "custom",
            region: "custom",
            intent: "custom",
            play,
            expectations: ScenarioExpectations::default(),
            real: &[],
            bone_strength: 1.0,
        }
    }

    pub fn frames(&self) -> i32 {
        self.play.frames()
    }

    /// Pointer input on `frame` of the scenario against a body placed at `body`.
    pub fn input(&self, frame: i32, dt: f64, body: BodyFrame) -> InputState {
        self.play.input(frame, dt, body)
    }

    /// The scenario aimed a little differently ([`Play::shifted`]).
    pub fn shifted(&self, along: f64, across: f64) -> Scenario {
        Scenario {
            play: self.play.shifted(along, across),
            ..*self
        }
    }

    /// The scenario on a body whose bones are `strength` times as strong as
    /// tuned.
    pub fn with_bone_strength(&self, strength: f64) -> Scenario {
        Scenario {
            bone_strength: strength,
            ..*self
        }
    }

    /// The materials the scenario plays on: the defaults, on bones as strong
    /// as its `bone_strength`.
    pub fn materials(&self) -> Materials {
        let mut materials = Materials::default();
        materials.bone_fracture_impulse *= self.bone_strength;
        materials
    }

    /// The real-world checks over a sweep's runs of this scenario: how often
    /// each outcome happened against how often it does in real life. Known
    /// gaps are left to [`Scenario::sweep_gaps`].
    pub fn sweep_violations(&self, results: &[&ScenarioResult]) -> Vec<String> {
        self.sweep_misses(results, false)
    }

    /// The known gaps over a sweep's runs: how far each is from real life, or
    /// that it now matches and its note can go.
    pub fn sweep_gaps(&self, results: &[&ScenarioResult]) -> Vec<String> {
        let runs = results.len();
        self.real
            .iter()
            .filter(|check| !check.gap.is_empty())
            .map(|check| {
                let hits = results
                    .iter()
                    .filter(|result| check.outcome.happened(result))
                    .count();
                if check.often.fits(hits, runs) {
                    format!(
                        "{}: {}={hits}/{runs} runs now matches real life ({}); the gap is closed",
                        self.name,
                        check.outcome.slug(),
                        check.often.name()
                    )
                } else {
                    format!(
                        "{}: {}={hits}/{runs} runs, but {} in real life: {}",
                        self.name,
                        check.outcome.slug(),
                        check.often.name(),
                        check.gap
                    )
                }
            })
            .collect()
    }

    fn sweep_misses(&self, results: &[&ScenarioResult], gaps: bool) -> Vec<String> {
        let runs = results.len();
        self.real
            .iter()
            .filter(|check| check.gap.is_empty() != gaps)
            .filter_map(|check| {
                let hits = results
                    .iter()
                    .filter(|result| check.outcome.happened(result))
                    .count();
                (!check.often.fits(hits, runs)).then(|| {
                    format!(
                        "{}: {}={hits}/{runs} runs, but {} in real life ({})",
                        self.name,
                        check.outcome.slug(),
                        check.often.name(),
                        check.source
                    )
                })
            })
            .collect()
    }

    /// What a run did against real life: the scenario's own checks that one
    /// run can judge (never and always), and rules for every play.
    fn real_world_problems(&self, result: &ScenarioResult) -> Vec<String> {
        let r = result;
        let mut problems = Vec::new();
        for check in self.real.iter().filter(|check| check.gap.is_empty()) {
            let happened = check.outcome.happened(r);
            let wrong = match check.often {
                Often::Never => happened,
                Often::Always => !happened,
                _ => false,
            };
            if wrong {
                problems.push(format!(
                    "{}={}, but {} in real life ({})",
                    check.outcome.slug(),
                    u8::from(happened),
                    check.often.name(),
                    check.source
                ));
            }
        }
        // A knife notches and cuts into long bones but does not break them;
        // breaking one takes a heavy chopping blade (Lynn and Fairgrieve 2009;
        // Gentile et al. 2019).
        if self.play.knife_only() {
            let long_bones: i32 = BonePart::ALL
                .iter()
                .filter(|part| part.is_long_bone())
                .map(|part| r.part_fractures[part.index()])
                .sum();
            if long_bones > 0 {
                problems.push(format!(
                    "knife_breaks_long_bone={long_bones}, but a knife only notches long bones (Lynn and Fairgrieve 2009)"
                ));
            }
        }
        // Bleeding follows what was cut (Lewis et al. 2017, the surgical
        // bleeding scale, which grades each bleeding site): a wound that
        // misses arteries oozes at most 10 ml a minute, an open fracture's
        // marrow up to 50, and a cut artery spurts at least 10.
        if let Some(total) = r.bleeding_ml_per_min() {
            let per_site = total / f64::from(r.max_active_wounds.max(1));
            let rate = if Outcome::CutsArtery.happened(r) {
                total
            } else {
                per_site
            };
            if Outcome::CutsArtery.happened(r) {
                if rate < 10.0 {
                    problems.push(format!(
                        "bleeding={rate:.1} ml/min, but a cut artery spurts at least 10 (Lewis et al. 2017)"
                    ));
                }
            } else {
                let limit = if r.fracture_marrow_sources > 0 {
                    50.0
                } else {
                    10.0
                };
                if rate > limit {
                    problems.push(format!(
                        "bleeding={rate:.1} ml/min a wound, but wounds that miss arteries bleed at most {limit:.0} (Lewis et al. 2017)"
                    ));
                }
            }
        }
        // Struck flesh moves at most twice as fast as the tool that hit it,
        // the limit of a perfectly elastic collision; faster flesh is the
        // simulation flinging it.
        let flesh_limit = (2.0 * r.max_tool_speed).max(1000.0);
        if r.max_point_speed > flesh_limit {
            problems.push(format!(
                "flesh_speed={:.0} px/s, above {flesh_limit:.0}: twice the tool's speed",
                r.max_point_speed
            ));
        }
        problems
    }

    /// Band violations of `result`, one line each.
    pub fn violations(&self, result: &ScenarioResult) -> Vec<String> {
        let e = &self.expectations;
        let r = result;
        let checks = [
            int("contacts", r.tissue_contacts + r.bone_contacts, e.contacts),
            int("bone_fractures", r.bone_fractures, e.bone_fractures),
            int("rib_fractures", r.rib_fractures, e.rib_fractures),
            int("skin_tears", r.skin_tears, e.skin_tears),
            int("muscle_tears", r.muscle_tears, e.muscle_tears),
            int("contusion_events", r.contusion_events, e.contusion_events),
            int(
                "tissue_fatigue_events",
                r.tissue_fatigue_events,
                e.tissue_fatigue_events,
            ),
            int(
                "tissue_plastic_events",
                r.tissue_plastic_events,
                e.tissue_plastic_events,
            ),
            int(
                "tear_propagations",
                r.tear_propagations,
                e.tear_propagations,
            ),
            int(
                "muscle_fiber_tears",
                r.muscle_fiber_tears,
                e.muscle_fiber_tears,
            ),
            int(
                "muscle_crush_ruptures",
                r.muscle_crush_ruptures,
                e.muscle_crush_ruptures,
            ),
            int(
                "cavity_pressure_events",
                r.cavity_pressure_events,
                e.cavity_pressure_events,
            ),
            int("cavity_ruptures", r.cavity_ruptures, e.cavity_ruptures),
            int(
                "organ_damage_events",
                r.organ_damage_events,
                e.organ_damage_events,
            ),
            int(
                "organ_penetrations",
                r.organ_penetrations,
                e.organ_penetrations,
            ),
            int(
                "rib_organ_punctures",
                r.rib_organ_punctures,
                e.rib_organ_punctures,
            ),
            int("organ_ruptures", r.organ_ruptures, e.organ_ruptures),
            int(
                "skin_flap_detachments",
                r.skin_flap_detachments,
                e.skin_flap_detachments,
            ),
            int("cut_openings", r.cut_openings, e.cut_openings),
            int(
                "vessel_lacerations",
                r.vessel_lacerations,
                e.vessel_lacerations,
            ),
            int(
                "fragment_vessel_lacerations",
                r.fragment_vessel_lacerations,
                e.fragment_vessel_lacerations,
            ),
            int("wound_reopens", r.wound_reopens, e.wound_reopens),
            int("fluid_emitted", r.fluid_emitted, e.fluid_emitted),
            int("wound_fluid", r.wound_fluid, e.wound_fluid),
            int(
                "blood_stain_deposits",
                r.blood_stain_deposits,
                e.blood_stain_deposits,
            ),
            real(
                "blood_trail_length",
                r.blood_trail_length,
                e.blood_trail_length,
            ),
            real("blood_loss", r.blood_loss, e.blood_loss),
            real(
                "final_blood_volume",
                r.final_blood_volume,
                e.final_blood_volume,
            ),
            real(
                "final_blood_turgor",
                r.final_blood_turgor,
                e.final_blood_turgor,
            ),
            real(
                "max_cavity_pressure",
                r.max_cavity_pressure,
                e.cavity_pressure,
            ),
            real(
                "max_cavity_collapse",
                r.max_cavity_collapse,
                e.cavity_collapse,
            ),
            real("max_organ_damage", r.max_organ_damage, e.organ_damage),
            int(
                "fracture_marrow_sources",
                r.fracture_marrow_sources,
                e.fracture_marrow_sources,
            ),
            int("opened_wounds", r.opened_wounds, e.opened_wounds),
            int(
                "final_free_fragments",
                r.final_free_fragments,
                e.final_free_fragments,
            ),
            int(
                "fragment_bone_contacts",
                r.fragment_bone_contacts,
                e.fragment_bone_contacts,
            ),
            int(
                "fragment_bone_damping_events",
                r.fragment_bone_damping_events,
                e.fragment_bone_damping_events,
            ),
            int(
                "fragment_bone_resting_contacts",
                r.fragment_bone_resting_contacts,
                e.fragment_bone_resting_contacts,
            ),
            int(
                "sleeping_fragments",
                r.max_sleeping_fragments,
                e.sleeping_fragments,
            ),
            int("sleep_events", r.fragment_sleep_events, e.sleep_events),
            int(
                "final_sleeping_fragments",
                r.final_sleeping_fragments,
                e.final_sleeping_fragments,
            ),
            int(
                "fragment_pair_damping_events",
                r.fragment_pair_damping_events,
                e.fragment_pair_damping_events,
            ),
            int(
                "fragment_pair_resting_contacts",
                r.fragment_pair_resting_contacts,
                e.fragment_pair_resting_contacts,
            ),
            int(
                "fragment_floor_contacts",
                r.fragment_floor_contacts,
                e.fragment_floor_contacts,
            ),
            int(
                "fragment_floor_resting_contacts",
                r.fragment_floor_resting_contacts,
                e.fragment_floor_resting_contacts,
            ),
            int(
                "fragment_pair_contacts",
                r.fragment_pair_contacts,
                e.fragment_pair_contacts,
            ),
            int(
                "fragment_skin_punctures",
                r.fragment_skin_punctures,
                e.fragment_skin_punctures,
            ),
            int(
                "bone_joint_subluxations",
                r.bone_joint_subluxations,
                e.bone_joint_subluxations,
            ),
            int(
                "joint_ligament_damage_events",
                r.joint_ligament_damage_events,
                e.joint_ligament_damage_events,
            ),
            int(
                "joint_corrections",
                r.post_fracture_joint_corrections,
                e.joint_corrections,
            ),
            real(
                "fragment_overlap",
                r.max_fragment_overlap,
                e.fragment_overlap,
            ),
            real("bone_spin", r.max_bone_angular_speed, e.bone_spin),
            real(
                "bone_joint_subluxation",
                r.max_bone_joint_subluxation,
                e.bone_joint_subluxation,
            ),
            real(
                "tissue_softening",
                r.max_tissue_softening,
                e.tissue_softening,
            ),
            real("tissue_fatigue", r.max_tissue_fatigue, e.tissue_fatigue),
            real(
                "tissue_plasticity",
                r.max_tissue_plasticity,
                e.tissue_plasticity,
            ),
            real("max_tool_turn", r.max_tool_turn, e.tool_turn),
            int("tool_snaps", r.tool_snaps, e.tool_snaps),
            real("tool_turning", r.tool_turning, e.tool_turning),
            int("contact_toggles", r.contact_toggles, e.contact_toggles),
            real("max_point_speed", r.max_point_speed, e.point_speed),
            real("max_free_lag", r.max_free_lag, e.free_lag),
            real("final_lag", r.final_lag, e.final_lag),
        ];
        let mut problems: Vec<String> = checks.into_iter().flatten().collect();
        problems.extend(self.real_world_problems(r));
        problems
            .into_iter()
            .map(|problem| format!("{}: {problem}", self.name))
            .collect()
    }
}

fn int(name: &str, value: i32, band: IntBand) -> Option<String> {
    (!band.contains(value)).then(|| format!("{name}={value} outside {}..{}", band.min, band.max))
}

fn real(name: &str, value: f64, band: DoubleBand) -> Option<String> {
    (!band.contains(value))
        .then(|| format!("{name}={value:.3} outside {:.3}..{:.3}", band.min, band.max))
}

/// What happened over a scenario: event counts summed over its frames, peaks,
/// and the final state.
#[derive(Clone, Debug, Default)]
pub struct ScenarioResult {
    pub tissue_contacts: i32,
    pub bone_contacts: i32,
    pub fractures: i32,
    pub skin_tears: i32,
    pub muscle_tears: i32,
    pub muscle_fiber_tears: i32,
    pub contusion_events: i32,
    pub tissue_fatigue_events: i32,
    pub tissue_plastic_events: i32,
    pub tear_propagations: i32,
    pub muscle_crush_ruptures: i32,
    pub cavity_pressure_events: i32,
    pub cavity_ruptures: i32,
    pub organ_damage_events: i32,
    pub organ_penetrations: i32,
    pub rib_organ_punctures: i32,
    pub organ_ruptures: i32,
    pub skin_flap_detachments: i32,
    pub cut_openings: i32,
    pub vessel_lacerations: i32,
    pub fragment_vessel_lacerations: i32,
    pub wound_reopens: i32,
    pub max_active_contusions: i32,
    pub detachments: i32,
    pub bone_detachments: i32,
    pub bone_joint_breaks: i32,
    pub bone_joint_subluxations: i32,
    pub joint_ligament_damage_events: i32,
    pub bone_fractures: i32,
    pub rib_fractures: i32,
    pub final_bones: i32,
    pub fluid_emitted: i32,
    pub wound_fluid: i32,
    /// Blood that stayed under unbroken skin, in particles.
    pub internal_bleeding: i32,
    pub blood_loss: f64,
    pub final_blood_volume: f64,
    pub final_blood_turgor: f64,
    pub blood_stain_deposits: i32,
    /// Blood that welled onto the skin, in particles.
    pub skin_blood_drops: i32,
    /// Drops of it that ran off the body's edge and dripped.
    pub blood_drips: i32,
    /// How far it ran down the skin, in pixels of trail.
    pub blood_trail_length: f64,
    pub fracture_marrow_sources: i32,
    /// Bending breaks that broke a butterfly wedge out of the struck side.
    pub butterfly_fragments: i32,
    pub opened_wounds: i32,
    pub max_active_wounds: i32,
    pub wound_leaks: i32,
    pub max_active_fluids: i32,
    pub max_active_blood_stains: i32,
    pub fragment_hits: i32,
    pub fragment_tears: i32,
    pub fragment_skin_punctures: i32,
    pub fragment_bone_contacts: i32,
    pub fragment_bone_damping_events: i32,
    pub fragment_bone_resting_contacts: i32,
    pub fragment_pair_contacts: i32,
    pub fragment_pair_damping_events: i32,
    pub fragment_pair_resting_contacts: i32,
    pub fragment_floor_contacts: i32,
    pub fragment_floor_resting_contacts: i32,
    pub post_fracture_joint_corrections: i32,
    pub max_impact: f64,
    pub max_bone_load: f64,
    pub max_point_load: f64,
    pub max_depth: f64,
    pub max_fragment_depth: f64,
    pub max_fragment_impulse: f64,
    pub max_fragment_overlap: f64,
    pub max_post_fracture_joint_stretch: f64,
    pub max_post_fracture_joint_angle: f64,
    pub max_bone_joint_subluxation: f64,
    pub max_wound_pressure: f64,
    pub max_wound_clot: f64,
    pub max_cavity_pressure: f64,
    pub max_cavity_collapse: f64,
    pub max_organ_damage: f64,
    pub max_contusion: f64,
    pub max_tissue_softening: f64,
    pub max_tissue_fatigue: f64,
    pub max_tissue_plasticity: f64,
    pub max_bone_angular_speed: f64,
    pub final_free_fragments: i32,
    pub final_spinning_fragments: i32,
    pub final_sleeping_fragments: i32,
    pub max_active_fragments: i32,
    pub max_sleeping_fragments: i32,
    pub fragment_sleep_events: i32,
    pub fragment_wake_events: i32,
    pub fragment_budget_skips: i32,
    pub fracture_budget_blocks: i32,
    pub fragment_bone_checks: i32,
    pub fragment_bone_budget_skips: i32,
    pub fragment_pair_checks: i32,
    pub fragment_pair_budget_skips: i32,
    pub fragment_tissue_checks: i32,
    pub fragment_tissue_budget_skips: i32,
    pub fluid_budget_replacements: i32,
    pub blood_stain_budget_replacements: i32,
    pub wound_budget_replacements: i32,
    pub max_solver_iterations: i32,
    /// Largest turn of the tool's long axis from one step to the next, in
    /// degrees; the steps it snapped round by more than [`TOOL_SNAP_DEGREES`];
    /// and all of its turning over the run.
    pub max_tool_turn: f64,
    pub tool_snaps: i32,
    pub tool_turning: f64,
    /// Times a held tool lost touch with the body and found it again.
    pub contact_toggles: i32,
    /// Fastest any tissue point moved, in pixels per second.
    pub max_point_speed: f64,
    /// Farthest the tool trailed the hand while it touched nothing, in
    /// pixels: how tightly it follows the pointer.
    pub max_free_lag: f64,
    /// How far the tool was from the hand on the last step, in pixels: one
    /// held still where nothing is drawn should be in hand.
    pub final_lag: f64,
    /// How well a knife cut follows its blade (`measure_cut`): the length of
    /// the blade's path on the steps it cut fibers, the opened skin slit's
    /// length in the rest shape, and how far, on average, the slit runs from
    /// the path, in pixels.
    pub blade_path_length: f64,
    pub slit_length: f64,
    pub slit_off_path: f64,
    /// How wide an opened cut gapes at the end, in pixels: the skin at the
    /// middle of the blade's path and at its widest, and the muscle at its
    /// widest.
    pub skin_gap_middle: f64,
    pub skin_gap_max: f64,
    pub muscle_gap_max: f64,
    /// Bones broken by part, indexed by [`BonePart::index`].
    pub part_fractures: [i32; BONE_PARTS],
    /// Breaks whose ends came out through the skin.
    pub open_fractures: i32,
    /// The most load each part of the skeleton took, as a share of what
    /// breaks it: a bone that reached 1 broke.
    pub part_peak_load: [f64; BONE_PARTS],
    /// Fastest the tool moved, in pixels per second.
    pub max_tool_speed: f64,
    /// Steps since the body first lost blood.
    pub bleed_frames: i32,
    last_positions: Vec<Vec2>,
    last_tool_axis: Option<Vec2>,
    last_touching: bool,
    touched_this_press: bool,
    /// The knife tip's moves on the steps it cut fibers.
    blade_path: Vec<(Vec2, Vec2)>,
    last_tip: Option<Vec2>,
    last_cuts: i32,
}

impl ScenarioResult {
    /// How fast the body bled on average since it first lost blood, in ml a
    /// minute for the 1.75 m adult the figure stands for, or `None` when it
    /// bled for under half a second.
    pub fn bleeding_ml_per_min(&self) -> Option<f64> {
        (self.bleed_frames >= 30)
            .then(|| self.blood_loss * BLOOD_VOLUME_ML / (f64::from(self.bleed_frames) / 3600.0))
    }

    /// Adds one step's events and peaks; `input` is what the step was played
    /// with.
    pub fn accumulate(&mut self, world: &World, input: &InputState) {
        self.accumulate_steadiness(world, input);
        let tip = world.current_tool_pose().map(|pose| pose.contact_end);
        let cuts = world.stats().broken_skin + world.stats().broken_muscle;
        if let (Some(from), Some(to)) = (self.last_tip, tip) {
            if input.tool == ToolMode::Sharp && cuts > self.last_cuts {
                self.blade_path.push((from, to));
            }
        }
        (self.last_tip, self.last_cuts) = (tip, cuts);
        let debug = world.debug();
        self.tissue_contacts += debug.tissue_contacts;
        self.bone_contacts += debug.bone_contacts;
        self.fractures += debug.fractures;
        for (count, fresh) in self.part_fractures.iter_mut().zip(debug.part_fractures) {
            *count += fresh;
        }
        self.max_tool_speed = self.max_tool_speed.max(debug.striker_speed);
        for bone in world.bones() {
            let share = bone.break_load / bone.fracture_impulse.max(1.0);
            let peak = &mut self.part_peak_load[bone.part.index()];
            *peak = peak.max(share);
        }
        for (peak, broke) in self.part_peak_load.iter_mut().zip(debug.part_break_share) {
            *peak = peak.max(broke);
        }
        if debug.blood_loss > 0.0 {
            self.bleed_frames += 1;
        }
        self.max_impact = self.max_impact.max(debug.impact);
        self.max_bone_load = self.max_bone_load.max(debug.max_bone_load);
        self.max_point_load = self.max_point_load.max(debug.max_point_load);
        self.max_depth = self.max_depth.max(debug.max_depth);
        self.max_fragment_depth = self.max_fragment_depth.max(debug.max_fragment_depth);
        self.max_fragment_impulse = self.max_fragment_impulse.max(debug.max_fragment_impulse);
        self.max_fragment_overlap = self.max_fragment_overlap.max(debug.max_fragment_overlap);
        self.max_post_fracture_joint_stretch = self
            .max_post_fracture_joint_stretch
            .max(debug.max_post_fracture_joint_stretch);
        self.max_post_fracture_joint_angle = self
            .max_post_fracture_joint_angle
            .max(debug.max_post_fracture_joint_angle);
        self.max_bone_joint_subluxation = self
            .max_bone_joint_subluxation
            .max(debug.max_bone_joint_subluxation);
        self.max_wound_pressure = self.max_wound_pressure.max(debug.max_wound_pressure);
        self.max_wound_clot = self.max_wound_clot.max(debug.max_wound_clot);
        self.max_cavity_pressure = self.max_cavity_pressure.max(debug.max_cavity_pressure);
        self.max_cavity_collapse = self.max_cavity_collapse.max(debug.max_cavity_collapse);
        self.max_organ_damage = self.max_organ_damage.max(debug.max_organ_damage);
        self.max_contusion = self.max_contusion.max(debug.max_contusion);
        self.max_tissue_softening = self.max_tissue_softening.max(debug.max_tissue_softening);
        self.max_tissue_fatigue = self.max_tissue_fatigue.max(debug.max_tissue_fatigue);
        self.max_tissue_plasticity = self.max_tissue_plasticity.max(debug.max_tissue_plasticity);
        self.max_bone_angular_speed = self
            .max_bone_angular_speed
            .max(debug.max_bone_angular_speed);
        self.max_active_wounds = self.max_active_wounds.max(debug.active_wounds);
        self.max_active_contusions = self.max_active_contusions.max(debug.active_contusions);
        self.wound_leaks += debug.wound_leaks;
        self.muscle_fiber_tears += debug.muscle_fiber_tears;
        self.muscle_crush_ruptures += debug.muscle_crush_ruptures;
        self.cavity_pressure_events += debug.cavity_pressure_events;
        self.cavity_ruptures += debug.cavity_ruptures;
        self.organ_damage_events += debug.organ_damage_events;
        self.organ_penetrations += debug.organ_penetrations;
        self.rib_organ_punctures += debug.rib_organ_punctures;
        self.organ_ruptures += debug.organ_ruptures;
        self.skin_flap_detachments += debug.skin_flap_detachments;
        self.vessel_lacerations += debug.vessel_lacerations;
        self.fragment_vessel_lacerations += debug.fragment_vessel_lacerations;
        self.wound_reopens += debug.wound_reopens;
        self.max_active_blood_stains = self.max_active_blood_stains.max(debug.active_blood_stains);
        self.fragment_bone_contacts += debug.fragment_bone_contacts;
        self.fragment_bone_damping_events += debug.fragment_bone_damping_events;
        self.fragment_bone_resting_contacts += debug.fragment_bone_resting_contacts;
        self.fragment_pair_contacts += debug.fragment_pair_contacts;
        self.fragment_pair_damping_events += debug.fragment_pair_damping_events;
        self.fragment_pair_resting_contacts += debug.fragment_pair_resting_contacts;
        self.fragment_floor_contacts += debug.fragment_floor_contacts;
        self.fragment_floor_resting_contacts += debug.fragment_floor_resting_contacts;
        self.post_fracture_joint_corrections += debug.post_fracture_joint_corrections;
        self.max_active_fluids = self.max_active_fluids.max(active_fluid_count(world));
        self.max_active_fragments = self.max_active_fragments.max(debug.active_fragments);
        self.max_sleeping_fragments = self.max_sleeping_fragments.max(debug.sleeping_fragments);
        self.fragment_sleep_events += debug.fragment_sleep_events;
        self.fragment_wake_events += debug.fragment_wake_events;
        self.bone_joint_subluxations += debug.bone_joint_subluxations;
        self.joint_ligament_damage_events += debug.joint_ligament_damage_events;
        self.fragment_budget_skips += debug.fragment_budget_skips;
        self.fracture_budget_blocks += debug.fracture_budget_blocks;
        self.fragment_bone_checks += debug.fragment_bone_checks;
        self.fragment_bone_budget_skips += debug.fragment_bone_budget_skips;
        self.fragment_pair_checks += debug.fragment_pair_checks;
        self.fragment_pair_budget_skips += debug.fragment_pair_budget_skips;
        self.fragment_tissue_checks += debug.fragment_tissue_checks;
        self.fragment_tissue_budget_skips += debug.fragment_tissue_budget_skips;
        self.fragment_skin_punctures += debug.fragment_skin_punctures;
        self.fluid_budget_replacements += debug.fluid_budget_replacements;
        self.blood_stain_budget_replacements += debug.blood_stain_budget_replacements;
        self.wound_budget_replacements += debug.wound_budget_replacements;
        self.max_solver_iterations = self.max_solver_iterations.max(debug.solver_iterations);
    }

    fn accumulate_steadiness(&mut self, world: &World, input: &InputState) {
        let axis = tool_axis(world);
        if let (Some(last), Some(axis)) = (self.last_tool_axis, axis) {
            let turn = turn_degrees(last, axis);
            self.max_tool_turn = self.max_tool_turn.max(turn);
            self.tool_turning += turn;
            if turn > TOOL_SNAP_DEGREES {
                self.tool_snaps += 1;
            }
        }
        self.last_tool_axis = axis;

        let touching = tool_touching(world);
        if input.down {
            if touching && !self.last_touching && self.touched_this_press {
                self.contact_toggles += 1;
            }
            self.touched_this_press |= touching;
        } else {
            self.touched_this_press = false;
        }
        if !touching && tool_axis(world).is_some() {
            self.max_free_lag = self.max_free_lag.max(tool_lag(world, input));
        }
        self.final_lag = if tool_axis(world).is_some() {
            tool_lag(world, input)
        } else {
            0.0
        };
        self.last_touching = touching;
        self.max_point_speed = self
            .max_point_speed
            .max(fastest_point_speed(world, &self.last_positions));
        self.last_positions.clear();
        self.last_positions
            .extend(world.points().iter().map(|point| point.position));
    }

    /// Takes the running totals and final state from the world at the end.
    pub fn finish(&mut self, world: &World) {
        let stats = world.stats();
        self.skin_tears = stats.broken_skin;
        self.muscle_tears = stats.broken_muscle;
        self.muscle_fiber_tears = stats.muscle_fiber_tears;
        self.contusion_events = stats.contusion_events;
        self.tissue_fatigue_events = stats.tissue_fatigue_events;
        self.tissue_plastic_events = stats.tissue_plastic_events;
        self.tear_propagations = stats.tear_propagations;
        self.muscle_crush_ruptures = stats.muscle_crush_ruptures;
        self.cavity_pressure_events = stats.cavity_pressure_events;
        self.cavity_ruptures = stats.cavity_ruptures;
        self.organ_damage_events = stats.organ_damage_events;
        self.organ_penetrations = stats.organ_penetrations;
        self.rib_organ_punctures = stats.rib_organ_punctures;
        self.organ_ruptures = stats.organ_ruptures;
        self.skin_flap_detachments = stats.skin_flap_detachments;
        self.cut_openings = stats.cut_openings;
        self.vessel_lacerations = stats.vessel_lacerations;
        self.fragment_vessel_lacerations = stats.fragment_vessel_lacerations;
        self.wound_reopens = stats.wound_reopens;
        self.detachments = stats.broken_attachments;
        self.bone_detachments = stats.broken_bone_attachments;
        self.bone_joint_breaks = stats.broken_bone_joints;
        self.bone_joint_subluxations = stats.bone_joint_subluxations;
        self.joint_ligament_damage_events = stats.joint_ligament_damage_events;
        self.bone_fractures = stats.fractured_bones;
        self.rib_fractures = stats.fractured_ribs;
        self.open_fractures = stats.open_fractures;
        self.final_bones = world.bones().len() as i32;
        self.fluid_emitted = stats.emitted_fluid_particles;
        self.wound_fluid = stats.wound_fluid_particles;
        self.internal_bleeding = stats.internal_bleeding;
        self.blood_loss = stats.blood_loss;
        self.final_blood_volume = world.blood_volume_fraction();
        self.final_blood_turgor = world.blood_turgor_scale();
        self.blood_stain_deposits = stats.blood_stain_deposits;
        self.skin_blood_drops = stats.skin_blood_drops;
        self.blood_drips = stats.blood_drips;
        self.blood_trail_length = stats.blood_trail_length;
        self.fracture_marrow_sources = stats.fracture_marrow_sources;
        self.butterfly_fragments = stats.butterfly_fragments;
        self.opened_wounds = stats.opened_wounds;
        self.fragment_hits = stats.fragment_tissue_hits;
        self.fragment_tears = stats.fragment_tissue_tears;
        self.fragment_skin_punctures = stats.fragment_skin_punctures;
        self.final_free_fragments = free_fragment_count(world);
        self.final_spinning_fragments = spinning_fragment_count(world);
        self.final_sleeping_fragments = sleeping_fragment_count(world);
        self.measure_cut(world);
    }

    /// Measures the knife's cut against the blade's path (see
    /// `blade_path_length` and the fields after it). Each pair of lips gives
    /// where the cut ran in the rest shape, between the two lips' rest
    /// places, and how far apart they are now.
    fn measure_cut(&mut self, world: &World) {
        let points = world.points();
        let springs = world.springs();
        let path = &self.blade_path;
        self.blade_path_length = path.iter().map(|&(a, b)| span(a, b)).sum();
        let mut lines = Vec::new();
        for (index, lip) in springs.iter().enumerate() {
            if lip.twin == MISSING_SPRING || lip.twin < index {
                continue;
            }
            let twin = springs[lip.twin];
            let start = midpoint(points[lip.a].home, points[twin.a].home);
            let end = midpoint(points[lip.b].home, points[twin.b].home);
            let gap = (span(points[lip.a].position, points[twin.a].position)
                + span(points[lip.b].position, points[twin.b].position))
                * 0.5;
            lines.push((lip.layer, start, end, gap));
        }
        let widest = |layer: TissueLayer| {
            lines
                .iter()
                .filter(|line| line.0 == layer)
                .map(|line| line.3)
                .fold(0.0, f64::max)
        };
        self.skin_gap_max = widest(TissueLayer::Skin);
        self.muscle_gap_max = widest(TissueLayer::Muscle);
        let skin: Vec<_> = lines
            .iter()
            .filter(|line| line.0 == TissueLayer::Skin)
            .collect();
        self.slit_length = skin.iter().map(|line| span(line.1, line.2)).sum();
        if skin.is_empty() || path.is_empty() {
            return;
        }
        self.slit_off_path = skin
            .iter()
            .flat_map(|line| [line.1, line.2])
            .map(|point| distance_to_path(point, path))
            .sum::<f64>()
            / (skin.len() * 2) as f64;
        let middle = point_along_path(path, self.blade_path_length * 0.5);
        self.skin_gap_middle = skin
            .iter()
            .map(|line| (span(midpoint(line.1, line.2), middle), line.3))
            .fold((f64::INFINITY, 0.0), |best, here| {
                if here.0 < best.0 {
                    here
                } else {
                    best
                }
            })
            .1;
    }
}

fn span(a: Vec2, b: Vec2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    (dx * dx + dy * dy).sqrt()
}

fn midpoint(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: (a.x + b.x) * 0.5,
        y: (a.y + b.y) * 0.5,
    }
}

/// The distance from `point` to the nearest of the segments of `path`.
fn distance_to_path(point: Vec2, path: &[(Vec2, Vec2)]) -> f64 {
    path.iter()
        .map(|&(a, b)| {
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let length_sq = dx * dx + dy * dy;
            let t = if length_sq > 1.0e-12 {
                (((point.x - a.x) * dx + (point.y - a.y) * dy) / length_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            span(
                point,
                Vec2 {
                    x: a.x + dx * t,
                    y: a.y + dy * t,
                },
            )
        })
        .fold(f64::INFINITY, f64::min)
}

/// The point `along` pixels down the segments of `path`, taken in order.
fn point_along_path(path: &[(Vec2, Vec2)], along: f64) -> Vec2 {
    let mut left = along;
    for &(a, b) in path {
        let length = span(a, b);
        if left <= length && length > 0.0 {
            let t = left / length;
            return Vec2 {
                x: a.x + (b.x - a.x) * t,
                y: a.y + (b.y - a.y) * t,
            };
        }
        left -= length;
    }
    path[path.len() - 1].1
}

/// Plays `scenario` on a fresh body in a window of the given size, calling
/// `observe` after every step, and returns the final world and the result.
pub fn run(
    scenario: &Scenario,
    width: f64,
    height: f64,
    observe: impl FnMut(i32, &InputState, &World),
) -> (World, ScenarioResult) {
    let world = create_layered_body(width, height, scenario.materials());
    run_on(world, scenario, width, height, observe)
}

/// [`run`] on a body the caller made for that window from the scenario's
/// [`Scenario::materials`], such as one that times its phases
/// ([`World::time_phases`]).
pub fn run_on(
    mut world: World,
    scenario: &Scenario,
    width: f64,
    height: f64,
    mut observe: impl FnMut(i32, &InputState, &World),
) -> (World, ScenarioResult) {
    let mut result = ScenarioResult::default();
    let dt = world.materials().fixed_dt;
    let body = body_frame(width, height);
    for frame in 0..scenario.frames() {
        let input = scenario.input(frame, dt, body);
        world.step(dt, &input, width, height);
        result.accumulate(&world, &input);
        observe(frame, &input, &world);
    }
    result.finish(&world);
    (world, result)
}

/// The direction the handle points from the bat's barrel or the hammer's
/// head, or the knife's blade points, while a tool is in play.
pub fn tool_axis(world: &World) -> Option<Vec2> {
    let pose = world.current_tool_pose()?;
    Some(match pose.tool {
        ToolMode::Sharp => pose.heading,
        ToolMode::Blunt | ToolMode::Heavy => pose.side,
    })
}

/// Angle between two directions, in degrees.
pub fn turn_degrees(from: Vec2, to: Vec2) -> f64 {
    let cross = from.x * to.y - from.y * to.x;
    let dot = from.x * to.x + from.y * to.y;
    libm::atan2(cross, dot).abs().to_degrees()
}

/// How far the tool's driven point is from the hand, in pixels.
pub fn tool_lag(world: &World, input: &InputState) -> f64 {
    let tool = world.tool_position();
    libm::hypot(input.x - tool.x, input.y - tool.y)
}

/// The tool touched tissue or bone on the last step.
pub fn tool_touching(world: &World) -> bool {
    let debug = world.debug();
    debug.tissue_contacts > 0 || debug.bone_contacts > 0
}

/// Fastest any free tissue point moved since it was at `last_positions` one
/// step ago, in pixels per second. (A point's `previous` is not always where
/// it was a step ago, so it cannot stand in for this.)
pub fn fastest_point_speed(world: &World, last_positions: &[Vec2]) -> f64 {
    let dt = world.materials().fixed_dt.max(1.0e-9);
    world
        .points()
        .iter()
        .zip(last_positions)
        .filter(|(point, _)| !point.pinned)
        .map(|(point, last)| libm::hypot(point.position.x - last.x, point.position.y - last.y))
        .fold(0.0, f64::max)
        / dt
}

pub fn tool_name(tool: ToolMode) -> &'static str {
    match tool {
        ToolMode::Blunt => "blunt",
        ToolMode::Sharp => "sharp",
        ToolMode::Heavy => "heavy",
    }
}

pub fn active_fluid_count(world: &World) -> i32 {
    world
        .fluids()
        .iter()
        .filter(|fluid| fluid.life > 0.0)
        .count() as i32
}

/// Broken pieces and splinters, which move on their own.
pub fn free_fragment(bone: &BoneSegment) -> bool {
    !bone.pinned && (bone.fractured || bone.splinter)
}

pub fn free_fragment_count(world: &World) -> i32 {
    world
        .bones()
        .iter()
        .filter(|bone| free_fragment(bone))
        .count() as i32
}

pub fn spinning_fragment_count(world: &World) -> i32 {
    world
        .bones()
        .iter()
        .filter(|bone| free_fragment(bone) && bone.angular_velocity.abs() > 0.08)
        .count() as i32
}

pub fn sleeping_fragment_count(world: &World) -> i32 {
    world
        .bones()
        .iter()
        .filter(|bone| free_fragment(bone) && bone.sleeping)
        .count() as i32
}

/// The tuned scenario with this name.
pub fn scenario(name: &str) -> Option<Scenario> {
    scenarios()
        .into_iter()
        .find(|scenario| scenario.name == name)
}

/// A bat or hammer carried across the body and back with the button up.
const CARRY_ACROSS: &[GestureStep] = &[
    GestureStep::Move {
        to: (-0.40, 0.30),
        frames: 0,
    },
    GestureStep::Wait(5),
    GestureStep::Move {
        to: (0.40, 0.35),
        frames: 30,
    },
    GestureStep::Move {
        to: (-0.40, 0.50),
        frames: 30,
    },
    GestureStep::Wait(10),
];

/// Pressed into the side of the chest from outside the arm, then dragged down
/// the body to the hip and let go.
const DRAG_DOWN_THE_SIDE: &[GestureStep] = &[
    GestureStep::Move {
        to: (-0.30, 0.30),
        frames: 0,
    },
    GestureStep::Wait(10),
    GestureStep::Press(true),
    GestureStep::Move {
        to: (-0.08, 0.30),
        frames: 15,
    },
    GestureStep::Move {
        to: (-0.08, 0.60),
        frames: 60,
    },
    GestureStep::Wait(20),
    GestureStep::Press(false),
    GestureStep::Wait(30),
];

/// From outside the arm into the middle of the chest at about 400 px/s,
/// leaning there, then letting go.
const PUSH_INTO_CHEST_SLOWLY: &[GestureStep] = &push_into_chest(33);
/// The same at about 800 px/s.
const SWING_INTO_CHEST_MODERATELY: &[GestureStep] = &push_into_chest(16);
/// The same at about 1600 px/s.
const SWING_INTO_CHEST_FIRMLY: &[GestureStep] = &push_into_chest(8);

const fn push_into_chest(frames: i32) -> [GestureStep; 7] {
    [
        GestureStep::Move {
            to: (-0.35, 0.34),
            frames: 0,
        },
        GestureStep::Wait(10),
        GestureStep::Press(true),
        GestureStep::Move {
            to: (0.0, 0.34),
            frames,
        },
        GestureStep::Wait(40),
        GestureStep::Press(false),
        GestureStep::Wait(60),
    ]
}

/// The tool rests beside the body, then a finger is put down on the chest:
/// the hand is there at once, with the button down.
const FINGER_LANDS_ON_CHEST: &[GestureStep] = &[
    GestureStep::Move {
        to: (-0.45, 0.34),
        frames: 0,
    },
    GestureStep::Wait(10),
    GestureStep::Move {
        to: (0.0, 0.34),
        frames: 0,
    },
    GestureStep::Press(true),
    GestureStep::Wait(20),
    GestureStep::Press(false),
    GestureStep::Wait(20),
];

/// A finger put down on the chest of a body nothing has touched yet, so the
/// tool is taken up there for the first time with the button down.
const FIRST_TOUCH_ON_CHEST: &[GestureStep] = &[
    GestureStep::Move {
        to: (0.0, 0.34),
        frames: 0,
    },
    GestureStep::Press(true),
    GestureStep::Wait(30),
    GestureStep::Press(false),
    GestureStep::Wait(20),
];

/// Swung across the chest and straight back.
const SWING_ACROSS_AND_BACK: &[GestureStep] = &[
    GestureStep::Move {
        to: (-0.40, 0.35),
        frames: 0,
    },
    GestureStep::Wait(10),
    GestureStep::Press(true),
    GestureStep::Move {
        to: (0.30, 0.35),
        frames: 10,
    },
    GestureStep::Move {
        to: (-0.40, 0.35),
        frames: 10,
    },
    GestureStep::Wait(10),
    GestureStep::Press(false),
    GestureStep::Wait(20),
];

/// Four hard sledgehammer blows through the arm into the chest, which smash a
/// wound open in the outside of the arm, where they land; then the hammer is
/// pressed outside the arm, dragged into the wound at about 550 px/s, and held
/// there.
const DRAG_INTO_A_WOUND: &[GestureStep] = &[
    GestureStep::Move {
        to: (-0.75, 0.34),
        frames: 0,
    },
    GestureStep::Wait(10),
    GestureStep::Press(true),
    GestureStep::Move {
        to: (0.0, 0.34),
        frames: 8,
    },
    GestureStep::Wait(15),
    GestureStep::Press(false),
    GestureStep::Move {
        to: (-0.75, 0.34),
        frames: 20,
    },
    GestureStep::Wait(5),
    GestureStep::Press(true),
    GestureStep::Move {
        to: (0.0, 0.34),
        frames: 8,
    },
    GestureStep::Wait(15),
    GestureStep::Press(false),
    GestureStep::Move {
        to: (-0.75, 0.34),
        frames: 20,
    },
    GestureStep::Wait(5),
    GestureStep::Press(true),
    GestureStep::Move {
        to: (0.0, 0.34),
        frames: 8,
    },
    GestureStep::Wait(15),
    GestureStep::Press(false),
    GestureStep::Move {
        to: (-0.75, 0.34),
        frames: 20,
    },
    GestureStep::Wait(5),
    GestureStep::Press(true),
    GestureStep::Move {
        to: (0.0, 0.34),
        frames: 8,
    },
    GestureStep::Wait(15),
    GestureStep::Press(false),
    GestureStep::Move {
        to: (-0.75, 0.34),
        frames: 20,
    },
    GestureStep::Wait(30),
    GestureStep::Press(true),
    GestureStep::Move {
        to: (-0.15, 0.34),
        frames: 40,
    },
    GestureStep::Wait(20),
];

// The research behind the real-world checks, in short; `docs/INJURY_REFERENCE.md`
// has the numbers and the sources in full. Energies assume a 4.5 kg
// sledgehammer head, a 0.8 kg bat, and the figure as a 1.75 m adult. How
// often a blow breaks a bone follows from its energy against the bone's
// estimated breaking energy (collarbone 5 J, forearm 20 J, upper arm 90 J,
// shin 100 J, thigh 120 J): at 4 times or more always, 2-4 times usually,
// 0.5-2 times sometimes, 0.25-0.5 times rarely, below that never.
const BRUISE: &str = "Desmoulin and Anderson 2007: half of 3.2 J blows bruise";
const FOREARM: &str = "Begeman and Pratima 1999: 4.5 kg at 3 m/s, about 20 J, breaks forearms";
const UPPER_ARM: &str =
    "Santago et al. 2008: the upper arm takes 2.6 times the forearm's bending, roughly 90 J";
const THIGH: &str =
    "Kennedy et al. 2004: 123 J broke all 45 thigh bones held at both ends; Barnes et al. 2022: 47 J on a thigh only bruised";
const SHIN: &str =
    "Nyquist et al. 1985: the shin takes 3 times the forearm's bending, roughly 100 J";
const COLLARBONE: &str =
    "Kemper et al. 2006: the collarbone breaks at 0.73 kN, half a forearm's force";
const RIBS: &str = "Kent and Patrie 2005: ribs break once the chest is pressed in about a third";
const BAT_ASSAULTS: &str =
    "Bryant et al. 1992: bats break the forearm most; Guo et al. 2026: an ordinary man's swing carries 112 J";
const SHIN_BAT: &str = "Levy et al. 1994: bat blows break the shin";
const SKIN_CUT: &str =
    "Bleetman et al. 2003: slashes press up to 212 N; skin gives way far below that";
const SHALLOW_SLASH: &str = "Steel et al. 2021: most slash wounds are shallow";
const BAT_OPEN: &str = "Bryant et al. 1992: a fifth of fractures from bat blows were open";
const IRON_OPEN: &str =
    "Nolan et al. 2000: 44% of fractures from clubs were open; Eames et al. 1997: 70% of iron-bar victims had open fractures";
const BLUNT_SPLIT: &str =
    "Sharkey et al. 2012: blunt blows split skin over bone; Barnes et al. 2022: 47 J left a thigh unsplit";

/// A [`RealCheck`] built as a constant, so a scenario's list of them lasts as
/// long as the program.
macro_rules! check {
    ($outcome:expr, $often:expr, $source:expr) => {
        RealCheck {
            outcome: $outcome,
            often: $often,
            source: $source,
            gap: "",
        }
    };
    ($outcome:expr, $often:expr, $source:expr, gap: $gap:expr) => {
        RealCheck {
            outcome: $outcome,
            often: $often,
            source: $source,
            gap: $gap,
        }
    };
}

/// A knife slashed fast into the upper arm, about 7 m/s, held there, and drawn
/// back.
const SLASH_INTO_THE_ARM_FAST: &[GestureStep] = &[
    GestureStep::Move {
        to: (-0.42, 0.34),
        frames: 0,
    },
    GestureStep::Wait(10),
    GestureStep::Press(true),
    GestureStep::Move {
        to: (0.0, 0.34),
        frames: 6,
    },
    GestureStep::Wait(60),
    GestureStep::Move {
        to: (-0.42, 0.34),
        frames: 20,
    },
    GestureStep::Press(false),
    GestureStep::Wait(30),
];

/// A knife thrust fast into the thigh, about 7 m/s, held there, and drawn back.
const THRUST_INTO_THE_THIGH_FAST: &[GestureStep] = &[
    GestureStep::Move {
        to: (-0.30, 0.66),
        frames: 0,
    },
    GestureStep::Wait(10),
    GestureStep::Press(true),
    GestureStep::Move {
        to: (0.10, 0.66),
        frames: 6,
    },
    GestureStep::Wait(40),
    GestureStep::Move {
        to: (-0.30, 0.66),
        frames: 20,
    },
    GestureStep::Press(false),
    GestureStep::Wait(40),
];

/// The tuned scenarios, played in a 1280x720 window: scripted strikes and
/// gestures played the way the app is. Each one checks what the play does in
/// real life (`real`, from injury research), and that the engine and the tool
/// in hand behave (`expectations`). Speeds are given for the 1.75 m adult the
/// figure stands for.
pub fn scenarios() -> Vec<Scenario> {
    use Often::*;
    use Outcome::*;
    // Every scenario: the tool never snaps round.
    let steady = ScenarioExpectations {
        tool_turn: DoubleBand::range(0.0, 30.0),
        tool_snaps: IntBand::range(0, 0),
        ..ScenarioExpectations::default()
    };
    // A strike reaches the body, and broken bone does not sink into itself
    // or spin wildly.
    let e = ScenarioExpectations {
        contacts: IntBand::at_least(1),
        fragment_overlap: DoubleBand::range(0.0, 18.0),
        bone_spin: DoubleBand::range(0.0, 38.0),
        ..steady
    };
    // A bat or hammer in hand turns smoothly: no faster than it could swing
    // round the hands, which is under 16 degrees a step even at full speed.
    let held = ScenarioExpectations {
        tool_turn: DoubleBand::range(0.0, 16.0),
        tool_turning: DoubleBand::range(0.0, 45.0),
        ..e
    };
    let gesture = |tool, steps| {
        Play::Gesture(Gesture {
            tool,
            power: swing_power(tool),
            steps,
        })
    };
    let strike = |tool, start, end, strike_frames, settle_frames| Strike {
        tool,
        start,
        end,
        windup_frames: 6,
        strike_frames,
        settle_frames,
        power: swing_power(tool),
    };
    vec![
        Scenario {
            name: "torso_blunt_medium",
            region: "torso",
            intent: "medium",
            play: Play::Swing(
                strike(ToolMode::Blunt, (-0.360, 0.330), (0.200, 0.330), 9, 60),
                None,
            ),
            // A bat swung at about 7 m/s, about 21 J, across the upper arm into
            // the chest: a moderate swing, since an ordinary man's hard swing
            // reaches 15.5 m/s.
            expectations: e,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(Breaks(BonePart::UpperArm), Never, UPPER_ARM),
                check!(Breaks(BonePart::Rib), Rarely, RIBS),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "torso_heavy_high",
            region: "torso",
            intent: "high",
            play: Play::Swing(
                strike(ToolMode::Heavy, (-0.260, 0.340), (0.300, 0.340), 6, 60),
                None,
            ),
            // The sledgehammer at its top speed, 10.6 m/s, about 250 J, through
            // the upper arm into the chest.
            expectations: e,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(Breaks(BonePart::UpperArm), Usually, UPPER_ARM),
                check!(Breaks(BonePart::Rib), Sometimes, RIBS),
                check!(OpenFracture, Sometimes, IRON_OPEN),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "torso_sharp_cut",
            region: "torso",
            intent: "cut",
            play: Play::Swing(
                strike(ToolMode::Sharp, (-0.040, 0.360), (0.060, 0.500), 16, 60),
                None,
            ),
            // A knife drawn slowly down and across the belly, about 1 m/s.
            // The belly is narrow between the hanging arms, and the blade
            // trails about 20 cm behind the hand, so a slash straight across
            // would cut the arm's artery; this one trails up toward the
            // chest. The cut opens rather than leaving a line of torn skin,
            // and its blood wells out and runs down the skin.
            expectations: ScenarioExpectations {
                cut_openings: IntBand::at_least(2),
                blood_trail_length: DoubleBand::range(150.0, f64::INFINITY),
                ..e
            },
            real: &[
                check!(OpensSkin, Always, SKIN_CUT),
                check!(CutsArtery, Rarely, SHALLOW_SLASH),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "shoulder_blunt",
            region: "shoulder",
            intent: "medium",
            play: Play::Swing(
                strike(ToolMode::Blunt, (-0.070, 0.000), (-0.070, 0.320), 6, 60),
                None,
            ),
            // A bat brought down on the shoulder at about 7 m/s, about 18 J,
            // onto the outer collarbone, which lies right under the skin.
            expectations: e,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(
                    Breaks(BonePart::Collarbone),
                    Usually,
                    COLLARBONE,
                    gap: "the figure has more flesh over the collarbone than a real shoulder, so a blow from above reaches it weakly"
                ),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "arm_sharp",
            region: "arm",
            intent: "cut",
            play: Play::Swing(
                strike(ToolMode::Sharp, (-0.086, 0.230), (-0.128, 0.450), 24, 48),
                None,
            ),
            // A knife drawn slowly from the armpit down the inside of the arm,
            // over the artery there; the blood runs down the arm.
            expectations: ScenarioExpectations {
                cut_openings: IntBand::at_least(1),
                blood_trail_length: DoubleBand::range(80.0, f64::INFINITY),
                ..e
            },
            real: &[check!(OpensSkin, Always, SKIN_CUT)],
            bone_strength: 1.0,
        },
        Scenario {
            name: "hip_heavy",
            region: "hip",
            intent: "high",
            play: Play::Swing(
                strike(ToolMode::Heavy, (-0.340, 0.620), (0.100, 0.620), 7, 60),
                None,
            ),
            // The sledgehammer into the thigh at about 7.7 m/s, about 130 J.
            expectations: e,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(Breaks(BonePart::Thigh), Sometimes, THIGH),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "leg_blunt",
            region: "leg",
            intent: "medium",
            play: Play::Swing(
                strike(ToolMode::Blunt, (-0.300, 0.780), (0.050, 0.800), 7, 60),
                None,
            ),
            // A bat into the shin at about 6 m/s, about 15 J.
            expectations: e,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(Breaks(BonePart::Shin), Never, SHIN),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "thigh_cut_rebleed",
            region: "leg",
            intent: "rebleed",
            // A knife cut down the outer thigh clots, then a bat swung into the
            // thigh strikes the healed cut. The blade runs ahead of the hand, so
            // the cut lies a little below the hand's path; the bat aims at its
            // lower half, below the hanging hand.
            play: Play::Swing(
                strike(ToolMode::Sharp, (-0.075, 0.585), (-0.075, 0.685), 16, 260),
                Some(strike(
                    ToolMode::Blunt,
                    (-0.340, 0.700),
                    (0.100, 0.700),
                    8,
                    60,
                )),
            ),
            expectations: ScenarioExpectations {
                // A fresh clot is weak, so the blow makes the cut bleed again.
                wound_reopens: IntBand::range(1, 30),
                // The cut's blood runs down the leg.
                blood_trail_length: DoubleBand::range(150.0, f64::INFINITY),
                ..e
            },
            real: &[
                check!(OpensSkin, Always, SKIN_CUT),
                check!(Bruises, Always, BRUISE),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "torso_heavy_fragment_settle",
            region: "torso",
            intent: "settle",
            play: Play::Swing(
                strike(ToolMode::Heavy, (-0.260, 0.340), (0.300, 0.340), 6, 260),
                None,
            ),
            // The pieces of bone broken by a full-force sledgehammer blow to the
            // chest settle against the bones and flesh around them without
            // spinning or flying off.
            expectations: ScenarioExpectations {
                final_free_fragments: IntBand::range(1, 40),
                fragment_bone_contacts: IntBand::at_least(1),
                fragment_pair_contacts: IntBand::at_least(1),
                ..e
            },
            real: &[check!(Breaks(BonePart::UpperArm), Usually, UPPER_ARM)],
            bone_strength: 1.0,
        },
        Scenario {
            name: "bat_carried_steady",
            region: "air",
            intent: "steady",
            play: gesture(ToolMode::Blunt, CARRY_ACROSS),
            // A bat carried over the body with the button up stays upright in
            // the hand instead of turning with every move, and touches nothing.
            expectations: ScenarioExpectations {
                contacts: IntBand::range(0, 0),
                tool_turn: DoubleBand::range(0.0, 1.0),
                tool_turning: DoubleBand::range(0.0, 1.0),
                ..steady
            },
            real: &[],
            bone_strength: 1.0,
        },
        Scenario {
            name: "bat_drag_steady",
            region: "torso",
            intent: "steady",
            play: gesture(ToolMode::Blunt, DRAG_DOWN_THE_SIDE),
            // A bat pressed against the side and dragged down the body slides
            // along it nearly upright; it does not swing round into the body
            // like a spear. At about 1.6 m/s it carries about 1 J.
            expectations: held,
            real: &[check!(BreaksAnyBone, Never, FOREARM)],
            bone_strength: 1.0,
        },
        Scenario {
            name: "bat_swing_back_steady",
            region: "torso",
            intent: "steady",
            play: gesture(ToolMode::Blunt, SWING_ACROSS_AND_BACK),
            // A bat swung across the chest and back keeps its barrel across
            // the swing both ways instead of spinning round at the turn.
            expectations: held,
            real: &[check!(Bruises, Always, BRUISE)],
            bone_strength: 1.0,
        },
        Scenario {
            name: "hammer_drag_steady",
            region: "torso",
            intent: "steady",
            play: gesture(ToolMode::Heavy, DRAG_DOWN_THE_SIDE),
            // The sledgehammer dragged down the side stays as steady; at about
            // 1.7 m/s it carries about 6 J.
            expectations: held,
            real: &[check!(BreaksAnyBone, Never, FOREARM)],
            bone_strength: 1.0,
        },
        Scenario {
            name: "hammer_slow_push",
            region: "torso",
            intent: "speed",
            play: gesture(ToolMode::Heavy, PUSH_INTO_CHEST_SLOWLY),
            // Pushed slowly into the side, about 1.2 m/s and 3 J, then held
            // there: at most a light bruise.
            expectations: ScenarioExpectations {
                contacts: IntBand::at_least(1),
                ..held
            },
            real: &[
                check!(BreaksAnyBone, Never, FOREARM),
                check!(OpensSkin, Never, BLUNT_SPLIT),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "hammer_moderate_swing",
            region: "torso",
            intent: "speed",
            play: gesture(ToolMode::Heavy, SWING_INTO_CHEST_MODERATELY),
            // Swung at about 2.5 m/s, about 14 J, into the upper arm and chest:
            // a bruise.
            expectations: held,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(BreaksAnyBone, Never, FOREARM),
                check!(OpensSkin, Rarely, BLUNT_SPLIT),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "hammer_firm_swing",
            region: "torso",
            intent: "speed",
            play: gesture(ToolMode::Heavy, SWING_INTO_CHEST_FIRMLY),
            // Swung firmly, about 5 m/s and 54 J, into the upper arm and chest:
            // a deep bruise, and sometimes a broken upper arm.
            expectations: held,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(Breaks(BonePart::UpperArm), Sometimes, UPPER_ARM),
                check!(SplitsSkinWithoutBreak, Rarely, BLUNT_SPLIT),
                // A break it only just makes seldom comes out open.
                check!(OpenFracture, Rarely, IRON_OPEN),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "hammer_finger_lands",
            region: "torso",
            intent: "touch",
            play: gesture(ToolMode::Heavy, FINGER_LANDS_ON_CHEST),
            // A finger put down on the chest, far from where the hammer
            // rested, takes the hammer up there; it is not flung across
            // through the arm, and gripped inside the body it passes through.
            expectations: ScenarioExpectations {
                contacts: IntBand::range(0, 0),
                bone_fractures: IntBand::range(0, 0),
                skin_tears: IntBand::range(0, 0),
                contusion_events: IntBand::range(0, 0),
                ..steady
            },
            real: &[],
            bone_strength: 1.0,
        },
        Scenario {
            name: "hammer_first_touch",
            region: "torso",
            intent: "touch",
            play: gesture(ToolMode::Heavy, FIRST_TOUCH_ON_CHEST),
            // A tool first taken up inside the body passes through it rather
            // than appearing in the flesh and blasting it apart, as the first
            // touch on a phone, which also rebuilds the body for finger-sized
            // buttons, used to.
            expectations: ScenarioExpectations {
                contacts: IntBand::range(0, 0),
                bone_fractures: IntBand::range(0, 0),
                skin_tears: IntBand::range(0, 0),
                contusion_events: IntBand::range(0, 0),
                ..steady
            },
            real: &[],
            bone_strength: 1.0,
        },
        Scenario {
            name: "hammer_into_wound",
            region: "torso",
            intent: "wound",
            play: gesture(ToolMode::Heavy, DRAG_INTO_A_WOUND),
            // Dragged into a wound its own blows smashed open and held there,
            // the sledgehammer comes to the hand: flesh torn away is not there
            // to hold it back, as it used to, like a swamp.
            expectations: ScenarioExpectations {
                contacts: IntBand::at_least(1),
                final_lag: DoubleBand::range(0.0, 10.0),
                ..steady
            },
            real: &[],
            bone_strength: 1.0,
        },
        Scenario {
            name: "bat_firm_swing",
            region: "torso",
            intent: "speed",
            play: gesture(ToolMode::Blunt, SWING_INTO_CHEST_FIRMLY),
            // The lighter bat swung at about 5 m/s, about 10 J: a bruise. Its
            // barrel reaches down to the forearm, which half that energy
            // rarely breaks.
            expectations: held,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(Breaks(BonePart::UpperArm), Never, UPPER_ARM),
                check!(Breaks(BonePart::Forearm), Rarely, FOREARM),
                check!(OpensSkin, Rarely, BLUNT_SPLIT),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "bat_hard_swing",
            region: "torso",
            intent: "high",
            play: Play::Swing(
                strike(ToolMode::Blunt, (-0.700, 0.330), (0.200, 0.330), 6, 60),
                None,
            ),
            // An ordinary man's hard bat swing, about 16 m/s and 110 J, across
            // the upper arm into the chest.
            expectations: e,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(Breaks(BonePart::UpperArm), Sometimes, BAT_ASSAULTS),
                check!(OpenFracture, Rarely, BAT_OPEN),
                // The arm takes the blow before the chest.
                check!(Breaks(BonePart::Rib), Rarely, RIBS),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "bat_hard_forearm",
            region: "arm",
            intent: "high",
            play: Play::Swing(
                strike(ToolMode::Blunt, (-0.700, 0.440), (0.200, 0.440), 6, 60),
                None,
            ),
            // The same hard swing into the hanging forearm, the bone bats break
            // most often.
            expectations: e,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(Breaks(BonePart::Forearm), Always, BAT_ASSAULTS),
                check!(OpenFracture, Rarely, BAT_OPEN),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "bat_moderate_forearm",
            region: "arm",
            intent: "medium",
            play: Play::Swing(
                strike(ToolMode::Blunt, (-0.360, 0.440), (0.200, 0.440), 9, 60),
                None,
            ),
            // A bat at about 7 m/s, about 21 J, into the hanging forearm: about
            // what breaks one.
            expectations: e,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(Breaks(BonePart::Forearm), Sometimes, FOREARM),
                check!(OpenFracture, Rarely, BAT_OPEN),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "hammer_forearm",
            region: "arm",
            intent: "medium",
            play: Play::Swing(
                strike(ToolMode::Heavy, (-0.300, 0.440), (0.050, 0.440), 12, 60),
                None,
            ),
            // The sledgehammer at about 3 m/s, about 20 J, into the hanging
            // forearm: the blow that broke forearms in the laboratory, where
            // they were held still.
            expectations: e,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(Breaks(BonePart::Forearm), Sometimes, FOREARM),
                // A break it only just makes seldom comes out open.
                check!(OpenFracture, Rarely, IRON_OPEN),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "bat_hard_shin",
            region: "leg",
            intent: "high",
            play: Play::Swing(
                strike(ToolMode::Blunt, (-0.700, 0.830), (0.200, 0.830), 6, 60),
                None,
            ),
            // The same hard swing into the shin, about as much as it takes.
            expectations: e,
            real: &[
                check!(Bruises, Always, BRUISE),
                check!(
                    Breaks(BonePart::Shin),
                    Sometimes,
                    SHIN_BAT,
                    gap: "the engine passes nearly all of a blow to a bone right under the skin, so a hard bat breaks the shin every time"
                ),
            ],
            bone_strength: 1.0,
        },
        Scenario {
            name: "knife_fast_slash",
            region: "arm",
            intent: "cut",
            play: gesture(ToolMode::Sharp, SLASH_INTO_THE_ARM_FAST),
            // A knife slashed into the upper arm at about 7 m/s, a real
            // slash's speed: it cuts deep but breaks no bone.
            expectations: steady,
            real: &[check!(OpensSkin, Always, SKIN_CUT)],
            bone_strength: 1.0,
        },
        Scenario {
            name: "knife_fast_thrust",
            region: "leg",
            intent: "cut",
            play: gesture(ToolMode::Sharp, THRUST_INTO_THE_THIGH_FAST),
            // A knife thrust into the thigh at about 7 m/s: it cuts deep but
            // breaks no bone.
            expectations: steady,
            real: &[check!(OpensSkin, Always, SKIN_CUT)],
            bone_strength: 1.0,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strike_spec_parses_tool_path_and_options() {
        let strike = Strike::parse("hammer:-0.26,0.34:0.3,0.34:settle=90").unwrap();
        assert_eq!(strike.tool, ToolMode::Heavy);
        assert_eq!(strike.start, (-0.26, 0.34));
        assert_eq!(strike.end, (0.3, 0.34));
        assert_eq!(strike.power, swing_power(ToolMode::Heavy));
        assert_eq!(strike.settle_frames, 90);
        assert_eq!(Strike::parse("knife:0,0:1,1:power=5").unwrap().power, 5.0);
        assert!(Strike::parse("spoon:0,0:1,1").is_err());
        assert!(Strike::parse("bat:0,0").is_err());
    }

    #[test]
    fn strike_spec_round_trips() {
        for scenario in scenarios() {
            let strikes = match scenario.play {
                Play::Swing(strike, followup) => [Some(strike), followup],
                Play::Gesture(_) => continue,
            };
            for strike in strikes.into_iter().flatten() {
                let spec = strike.to_string();
                assert_eq!(Strike::parse(&spec), Ok(strike), "{spec}");
            }
        }
    }

    #[test]
    fn gesture_spec_parses_and_round_trips() {
        let gesture =
            Gesture::parse("bat:-0.3,0.4:wait=10:down:0.05,0.4/12:wait=40:up:wait=30").unwrap();
        assert_eq!(gesture.tool, ToolMode::Blunt);
        assert_eq!(gesture.power, swing_power(ToolMode::Blunt));
        assert_eq!(gesture.frames(), 92);
        assert_eq!(Gesture::parse(&gesture.to_string()), Ok(gesture));
        assert!(Gesture::parse("bat:down:0,0.4/4").is_err());
        assert!(Gesture::parse("bat:0,0.4:sideways").is_err());
    }

    #[test]
    fn gesture_hand_hovers_presses_moves_and_lets_go() {
        let gesture = Gesture::parse("hammer:0,0.5:wait=2:down:0.4,0.5/4:up:wait=1").unwrap();
        assert_eq!(gesture.hand(0), ((0.0, 0.5), false));
        assert_eq!(gesture.hand(1), ((0.0, 0.5), false));
        let ((u, _), down) = gesture.hand(2);
        assert!(down && (u - 0.1).abs() < 1.0e-12);
        assert_eq!(gesture.hand(5), ((0.4, 0.5), true));
        assert_eq!(gesture.hand(6), ((0.4, 0.5), false));
        let body = body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT);
        assert!(gesture.input(0, body).active);
    }

    #[test]
    fn shifted_strike_moves_along_and_across_its_path() {
        let strike = Strike::parse("bat:0,0.5:0.4,0.5").unwrap();
        let moved = strike.shifted(0.01, 0.02);
        assert!((moved.start.0 - 0.01).abs() < 1.0e-12);
        assert!((moved.start.1 - 0.52).abs() < 1.0e-12);
        assert!((moved.end.0 - 0.41).abs() < 1.0e-12);
    }

    #[test]
    fn scenario_input_holds_through_then_lets_go_and_plays_the_followup() {
        let scenario = scenario("thigh_cut_rebleed").unwrap();
        let dt = 1.0 / 60.0;
        let body = body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT);
        let Play::Swing(first, Some(followup)) = scenario.play else {
            panic!("thigh_cut_rebleed plays two swings");
        };
        assert!(!scenario.input(first.windup_frames - 1, dt, body).down);
        assert!(scenario.input(first.windup_frames, dt, body).down);
        let released = first.windup_frames + first.strike_frames + FOLLOW_THROUGH_FRAMES;
        assert!(scenario.input(released - 1, dt, body).down);
        assert!(!scenario.input(released, dt, body).down);
        let swing = first.frames() + followup.windup_frames;
        let input = scenario.input(swing, dt, body);
        assert!(input.down);
        assert_eq!(input.tool, ToolMode::Blunt);
    }
}
