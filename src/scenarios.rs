//! Scripted strikes and gestures: hand paths in body coordinates and the
//! pointer input that plays them, plus the tuned scenarios with the injury and
//! steadiness bands each one should land in. The strike scenario runner, the
//! visual damage diagnostic, and the app's capture mode all play them from
//! here, so a scenario plays the same everywhere. Native only; none of this
//! ships in the browser build.

use std::fmt;

use crate::{
    body_frame, create_layered_body, swing_power, BodyFrame, BoneSegment, InputState, Materials,
    ToolMode, Vec2, World,
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
        let length = du.hypot(dv).max(1.0e-9);
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
    pub muscle_cut_transfers: IntBand,
    pub muscle_fiber_tears: IntBand,
    pub muscle_crush_ruptures: IntBand,
    pub cavity_pressure_events: IntBand,
    pub cavity_ruptures: IntBand,
    pub organ_damage_events: IntBand,
    pub organ_penetrations: IntBand,
    pub rib_organ_punctures: IntBand,
    pub organ_ruptures: IntBand,
    pub skin_flap_detachments: IntBand,
    pub vessel_lacerations: IntBand,
    pub fragment_vessel_lacerations: IntBand,
    pub wound_reopens: IntBand,
    pub fluid_emitted: IntBand,
    pub wound_fluid: IntBand,
    pub blood_stain_deposits: IntBand,
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
}

/// A tool whose axis turns more than this in one step has snapped round
/// rather than turned.
pub const TOOL_SNAP_DEGREES: f64 = 45.0;

#[derive(Clone, Copy, Debug)]
pub struct Scenario {
    pub name: &'static str,
    pub region: &'static str,
    pub intent: &'static str,
    pub play: Play,
    pub expectations: ScenarioExpectations,
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
                "muscle_cut_transfers",
                r.muscle_cut_transfers,
                e.muscle_cut_transfers,
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
        ];
        checks
            .into_iter()
            .flatten()
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
    pub muscle_cut_transfers: i32,
    pub muscle_crush_ruptures: i32,
    pub cavity_pressure_events: i32,
    pub cavity_ruptures: i32,
    pub organ_damage_events: i32,
    pub organ_penetrations: i32,
    pub rib_organ_punctures: i32,
    pub organ_ruptures: i32,
    pub skin_flap_detachments: i32,
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
    pub fracture_marrow_sources: i32,
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
    last_positions: Vec<Vec2>,
    last_tool_axis: Option<Vec2>,
    last_touching: bool,
    touched_this_press: bool,
}

impl ScenarioResult {
    /// Adds one step's events and peaks; `input` is what the step was played
    /// with.
    pub fn accumulate(&mut self, world: &World, input: &InputState) {
        self.accumulate_steadiness(world, input);
        let debug = world.debug();
        self.tissue_contacts += debug.tissue_contacts;
        self.bone_contacts += debug.bone_contacts;
        self.fractures += debug.fractures;
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
        self.muscle_cut_transfers = stats.muscle_cut_transfers;
        self.muscle_crush_ruptures = stats.muscle_crush_ruptures;
        self.cavity_pressure_events = stats.cavity_pressure_events;
        self.cavity_ruptures = stats.cavity_ruptures;
        self.organ_damage_events = stats.organ_damage_events;
        self.organ_penetrations = stats.organ_penetrations;
        self.rib_organ_punctures = stats.rib_organ_punctures;
        self.organ_ruptures = stats.organ_ruptures;
        self.skin_flap_detachments = stats.skin_flap_detachments;
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
        self.final_bones = world.bones().len() as i32;
        self.fluid_emitted = stats.emitted_fluid_particles;
        self.wound_fluid = stats.wound_fluid_particles;
        self.internal_bleeding = stats.internal_bleeding;
        self.blood_loss = stats.blood_loss;
        self.final_blood_volume = world.blood_volume_fraction();
        self.final_blood_turgor = world.blood_turgor_scale();
        self.blood_stain_deposits = stats.blood_stain_deposits;
        self.fracture_marrow_sources = stats.fracture_marrow_sources;
        self.opened_wounds = stats.opened_wounds;
        self.fragment_hits = stats.fragment_tissue_hits;
        self.fragment_tears = stats.fragment_tissue_tears;
        self.fragment_skin_punctures = stats.fragment_skin_punctures;
        self.final_free_fragments = free_fragment_count(world);
        self.final_spinning_fragments = spinning_fragment_count(world);
        self.final_sleeping_fragments = sleeping_fragment_count(world);
    }
}

/// Plays `scenario` on a fresh body in a window of the given size, calling
/// `observe` after every step, and returns the final world and the result.
pub fn run(
    scenario: &Scenario,
    width: f64,
    height: f64,
    mut observe: impl FnMut(i32, &InputState, &World),
) -> (World, ScenarioResult) {
    let mut world = create_layered_body(width, height, Materials::default());
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
    cross.atan2(dot).abs().to_degrees()
}

/// How far the tool's driven point is from the hand, in pixels.
pub fn tool_lag(world: &World, input: &InputState) -> f64 {
    let tool = world.tool_position();
    (input.x - tool.x).hypot(input.y - tool.y)
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
        .map(|(point, last)| (point.position.x - last.x).hypot(point.position.y - last.y))
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

/// The tuned scenarios, played in a 1280x720 window: scripted strikes with the
/// injuries they should cause, and gestures played the way the app is, which
/// check that a bat or hammer moves steadily in hand.
pub fn scenarios() -> Vec<Scenario> {
    // Every scenario: the tool never snaps round, and no tissue flies off.
    let steady = ScenarioExpectations {
        tool_turn: DoubleBand::range(0.0, 30.0),
        tool_snaps: IntBand::range(0, 0),
        point_speed: DoubleBand::range(0.0, 4000.0),
        ..ScenarioExpectations::default()
    };
    let e = ScenarioExpectations {
        contacts: IntBand::at_least(1),
        contusion_events: IntBand::at_least(1),
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
            // A hard bat swing into the upper arm and chest: deep bruising
            // and perhaps a broken arm, but the torso is not torn open.
            expectations: ScenarioExpectations {
                skin_tears: IntBand::range(0, 30),
                muscle_tears: IntBand::range(0, 60),
                muscle_fiber_tears: IntBand::range(0, 40),
                contusion_events: IntBand::at_least(40),
                bone_fractures: IntBand::range(0, 2),
                rib_fractures: IntBand::range(0, 1),
                vessel_lacerations: IntBand::range(0, 0),
                fragment_vessel_lacerations: IntBand::range(0, 0),
                organ_penetrations: IntBand::range(0, 0),
                rib_organ_punctures: IntBand::range(0, 0),
                organ_ruptures: IntBand::range(0, 0),
                cavity_ruptures: IntBand::range(0, 0),
                blood_loss: DoubleBand::range(0.0, 0.02),
                ..e
            },
        },
        Scenario {
            name: "torso_heavy_high",
            region: "torso",
            intent: "high",
            play: Play::Swing(
                strike(ToolMode::Heavy, (-0.260, 0.340), (0.300, 0.340), 6, 60),
                None,
            ),
            // A full-force sledgehammer blow, as fast as the hammer goes,
            // through the arm into the chest breaks the arm, often a rib too,
            // bruises deeply,
            // and leaves a wound. Breaking the arm takes much of the blow, so
            // the chest behind it is not pulped.
            expectations: ScenarioExpectations {
                bone_fractures: IntBand::range(1, 10),
                rib_fractures: IntBand::range(0, 6),
                skin_tears: IntBand::range(10, 120),
                muscle_tears: IntBand::range(20, 220),
                muscle_fiber_tears: IntBand::range(5, 80),
                contusion_events: IntBand::at_least(60),
                muscle_crush_ruptures: IntBand::range(0, 40),
                cavity_pressure_events: IntBand::range(0, 60),
                cavity_ruptures: IntBand::range(0, 1),
                organ_damage_events: IntBand::range(0, 60),
                organ_penetrations: IntBand::range(0, 0),
                fracture_marrow_sources: IntBand::range(0, 12),
                vessel_lacerations: IntBand::range(0, 4),
                cavity_pressure: DoubleBand::range(0.05, 1.20),
                organ_damage: DoubleBand::range(0.0, 1.81),
                blood_loss: DoubleBand::range(0.0, 0.08),
                fluid_emitted: IntBand::range(0, 2500),
                ..e
            },
        },
        Scenario {
            name: "torso_sharp_cut",
            region: "torso",
            intent: "cut",
            play: Play::Swing(
                strike(ToolMode::Sharp, (-0.075, 0.380), (0.065, 0.480), 16, 60),
                None,
            ),
            // A knife slashed across the belly cuts a line through skin and
            // muscle and can reach a vessel or organ, but breaks no bone. Blood
            // from muscle cut under skin left whole bruises it.
            expectations: ScenarioExpectations {
                bone_fractures: IntBand::range(0, 0),
                rib_fractures: IntBand::range(0, 0),
                skin_tears: IntBand::range(6, 40),
                muscle_tears: IntBand::range(15, 100),
                contusion_events: IntBand::range(0, 100),
                tear_propagations: IntBand::range(0, 20),
                muscle_cut_transfers: IntBand::range(10, 80),
                skin_flap_detachments: IntBand::range(8, 60),
                organ_penetrations: IntBand::range(0, 2),
                organ_ruptures: IntBand::range(0, 1),
                vessel_lacerations: IntBand::range(0, 2),
                cavity_ruptures: IntBand::range(0, 0),
                blood_loss: DoubleBand::range(0.002, 0.05),
                ..e
            },
        },
        Scenario {
            name: "shoulder_blunt",
            region: "shoulder",
            intent: "medium",
            play: Play::Swing(
                strike(ToolMode::Blunt, (-0.100, 0.000), (-0.100, 0.320), 6, 60),
                None,
            ),
            // A bat brought down on the shoulder bruises it.
            expectations: ScenarioExpectations {
                skin_tears: IntBand::range(0, 12),
                muscle_tears: IntBand::range(0, 20),
                contusion_events: IntBand::at_least(30),
                bone_fractures: IntBand::range(0, 1),
                vessel_lacerations: IntBand::range(0, 0),
                organ_penetrations: IntBand::range(0, 0),
                organ_ruptures: IntBand::range(0, 0),
                cavity_ruptures: IntBand::range(0, 0),
                ..e
            },
        },
        Scenario {
            name: "arm_sharp",
            region: "arm",
            intent: "cut",
            play: Play::Swing(
                strike(ToolMode::Sharp, (-0.086, 0.230), (-0.128, 0.450), 24, 48),
                None,
            ),
            // A knife drawn from the armpit down the inside of the arm cuts
            // along it and opens the arteries there, but breaks no bone.
            expectations: ScenarioExpectations {
                bone_fractures: IntBand::range(0, 0),
                rib_fractures: IntBand::range(0, 0),
                skin_tears: IntBand::range(10, 60),
                muscle_tears: IntBand::range(20, 120),
                contusion_events: IntBand::range(0, 60),
                tear_propagations: IntBand::range(0, 20),
                muscle_cut_transfers: IntBand::range(15, 100),
                vessel_lacerations: IntBand::range(1, 3),
                bone_joint_subluxations: IntBand::range(0, 0),
                ..e
            },
        },
        Scenario {
            name: "hip_heavy",
            region: "hip",
            intent: "high",
            play: Play::Swing(
                strike(ToolMode::Heavy, (-0.340, 0.620), (0.100, 0.620), 7, 60),
                None,
            ),
            // A full-force sledgehammer into the thigh bruises it deeply and
            // can split the skin; the femur, the strongest bone, holds.
            expectations: ScenarioExpectations {
                contusion_events: IntBand::at_least(60),
                skin_tears: IntBand::range(0, 60),
                bone_fractures: IntBand::range(0, 1),
                vessel_lacerations: IntBand::range(0, 1),
                organ_penetrations: IntBand::range(0, 0),
                organ_ruptures: IntBand::range(0, 0),
                cavity_ruptures: IntBand::range(0, 0),
                ..e
            },
        },
        Scenario {
            name: "leg_blunt",
            region: "leg",
            intent: "medium",
            play: Play::Swing(
                strike(ToolMode::Blunt, (-0.300, 0.780), (0.050, 0.800), 7, 60),
                None,
            ),
            // A hard bat swing into the shin bruises it and can split the thin
            // skin over the bone.
            expectations: ScenarioExpectations {
                contusion_events: IntBand::at_least(30),
                skin_tears: IntBand::range(0, 30),
                bone_fractures: IntBand::range(0, 1),
                vessel_lacerations: IntBand::range(0, 0),
                organ_penetrations: IntBand::range(0, 0),
                cavity_ruptures: IntBand::range(0, 0),
                ..e
            },
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
                strike(ToolMode::Sharp, (-0.075, 0.585), (-0.075, 0.685), 16, 190),
                Some(strike(
                    ToolMode::Blunt,
                    (-0.340, 0.700),
                    (0.100, 0.700),
                    8,
                    60,
                )),
            ),
            expectations: ScenarioExpectations {
                bone_fractures: IntBand::range(0, 1),
                skin_tears: IntBand::range(8, 50),
                muscle_tears: IntBand::range(10, 80),
                muscle_cut_transfers: IntBand::range(8, 60),
                skin_flap_detachments: IntBand::range(0, 40),
                contusion_events: IntBand::at_least(30),
                // The bat must make the clotted cut bleed again.
                wound_reopens: IntBand::range(1, 30),
                organ_penetrations: IntBand::range(0, 0),
                organ_ruptures: IntBand::range(0, 0),
                cavity_ruptures: IntBand::range(0, 0),
                blood_loss: DoubleBand::range(0.0003, 0.05),
                ..e
            },
        },
        Scenario {
            name: "torso_heavy_fragment_settle",
            region: "torso",
            intent: "settle",
            play: Play::Swing(
                strike(ToolMode::Heavy, (-0.260, 0.340), (0.300, 0.340), 6, 260),
                None,
            ),
            // The bone fragments from a full-force sledgehammer blow to the
            // chest settle against the bones and flesh around them without
            // spinning or flying off.
            expectations: ScenarioExpectations {
                bone_fractures: IntBand::range(1, 10),
                rib_fractures: IntBand::range(0, 6),
                contusion_events: IntBand::at_least(60),
                final_free_fragments: IntBand::range(1, 40),
                fragment_bone_contacts: IntBand::at_least(1),
                fragment_pair_contacts: IntBand::at_least(1),
                wound_reopens: IntBand::range(0, 60),
                organ_penetrations: IntBand::range(0, 0),
                blood_loss: DoubleBand::range(0.01, 0.15),
                ..e
            },
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
        },
        Scenario {
            name: "bat_drag_steady",
            region: "torso",
            intent: "steady",
            play: gesture(ToolMode::Blunt, DRAG_DOWN_THE_SIDE),
            // A bat pressed against the side and dragged down the body slides
            // along it nearly upright without hurting it much; it does not
            // swing round into the body like a spear.
            expectations: ScenarioExpectations {
                contusion_events: IntBand::at_least(0),
                skin_tears: IntBand::range(0, 4),
                bone_fractures: IntBand::range(0, 0),
                ..held
            },
        },
        Scenario {
            name: "bat_swing_back_steady",
            region: "torso",
            intent: "steady",
            play: gesture(ToolMode::Blunt, SWING_ACROSS_AND_BACK),
            // A bat swung across the chest and back keeps its barrel across
            // the swing both ways instead of spinning round at the turn.
            expectations: ScenarioExpectations {
                contusion_events: IntBand::at_least(10),
                bone_fractures: IntBand::range(0, 2),
                ..held
            },
        },
        Scenario {
            name: "hammer_drag_steady",
            region: "torso",
            intent: "steady",
            play: gesture(ToolMode::Heavy, DRAG_DOWN_THE_SIDE),
            // The sledgehammer dragged down the side stays as steady, and its
            // weight bruises without breaking anything on the way.
            expectations: ScenarioExpectations {
                contusion_events: IntBand::at_least(0),
                skin_tears: IntBand::range(0, 6),
                bone_fractures: IntBand::range(0, 0),
                ..held
            },
        },
        Scenario {
            name: "hammer_slow_push",
            region: "torso",
            intent: "speed",
            play: gesture(ToolMode::Heavy, PUSH_INTO_CHEST_SLOWLY),
            // Pushed slowly into the side, about 400 px/s, then held there,
            // the sledgehammer does no harm: hurting takes speed, and a hand
            // cannot press a broad face through skin.
            expectations: ScenarioExpectations {
                contacts: IntBand::at_least(1),
                contusion_events: IntBand::range(0, 30),
                skin_tears: IntBand::range(0, 1),
                muscle_tears: IntBand::range(0, 1),
                bone_fractures: IntBand::range(0, 0),
                fluid_emitted: IntBand::range(0, 10),
                ..held
            },
        },
        Scenario {
            name: "hammer_moderate_swing",
            region: "torso",
            intent: "speed",
            play: gesture(ToolMode::Heavy, SWING_INTO_CHEST_MODERATELY),
            // Swung at about 800 px/s, it bruises the arm but breaks and
            // tears nothing much.
            expectations: ScenarioExpectations {
                contusion_events: IntBand::at_least(10),
                skin_tears: IntBand::range(0, 6),
                muscle_tears: IntBand::range(0, 4),
                bone_fractures: IntBand::range(0, 0),
                fluid_emitted: IntBand::range(0, 60),
                ..held
            },
        },
        Scenario {
            name: "hammer_firm_swing",
            region: "torso",
            intent: "speed",
            play: gesture(ToolMode::Heavy, SWING_INTO_CHEST_FIRMLY),
            // Swung firmly, about 1600 px/s, it bruises deeply and may break
            // the arm, but nothing bursts open.
            expectations: ScenarioExpectations {
                contusion_events: IntBand::at_least(30),
                skin_tears: IntBand::range(0, 40),
                muscle_tears: IntBand::range(0, 60),
                bone_fractures: IntBand::range(0, 2),
                rib_fractures: IntBand::range(0, 0),
                fracture_marrow_sources: IntBand::range(0, 0),
                fluid_emitted: IntBand::range(0, 1600),
                ..held
            },
        },
        Scenario {
            name: "bat_firm_swing",
            region: "torso",
            intent: "speed",
            play: gesture(ToolMode::Blunt, SWING_INTO_CHEST_FIRMLY),
            // The lighter bat swung as firmly bruises without breaking bone,
            // and rarely splits the skin.
            expectations: ScenarioExpectations {
                contusion_events: IntBand::at_least(20),
                skin_tears: IntBand::range(0, 12),
                bone_fractures: IntBand::range(0, 0),
                fluid_emitted: IntBand::range(0, 100),
                ..held
            },
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
