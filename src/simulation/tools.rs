//! Tool geometry, pose, and motion. The app draws each tool from the same pose
//! and dimensions used here, so what is on screen is what touches the body.
//!
//! Every tool's striking part is a segment with a radius: the knife's blade runs
//! along its heading with a thin edge, the sledgehammer's head runs along its
//! heading so a face leads, and the bat's barrel lies across its heading so the
//! side of the barrel leads. `side` points from the striking part toward the
//! hand along the handle of the hammer and bat.
//!
//! A tool is a solid object with momentum. The hand pulls it toward the pointer
//! like a spring, no harder than an arm can push, and the body pushes back. A bat
//! or hammer gives its momentum to the tissue and bone it shoves, so it slows and
//! stops in the body instead of sweeping through it. A knife moves only through
//! fibers it severs: it rests against skin it cannot cut until it moves fast
//! enough or the hand presses hard enough, and bone stops it.
//!
//! Tools meet only what is drawn: flesh torn away (points of the tissue sheets
//! left in no live triangle, and the fibers between them; see
//! `World::flesh_points`) and the spine, which lies behind the trunk, are not
//! there for them, so a tool moves through a wound as freely as through the
//! air and meets only its edges and the bone left in it.

use super::*;

/// Below this speed a tool keeps its orientation instead of turning to follow
/// its motion.
const TOOL_TURN_SPEED: f64 = 40.0;
/// Which way the handle of a bat or hammer points when it is carried with the
/// button up: down from the hand, with the barrel or head standing up.
const CARRIED_HANDLE: Vec2 = Vec2 { x: 0.0, y: 1.0 };
/// How fast a carried bat or hammer settles back to `CARRIED_HANDLE`, in
/// radians per second.
const CARRY_TURN_RATE: f64 = 3.0;
/// Share of its free turning a bat or hammer pressed into tissue keeps; the
/// flesh around it holds it in line.
const EMBEDDED_TURN_SHARE: f64 = 0.2;
/// How fast tissue steers an embedded knife to follow its stroke.
const EMBEDDED_BLADE_TURN_RATE: f64 = 18.0;
/// The front part of the blade, as a fraction of its length, that cuts what it
/// passes; the rest of the blade follows the tip through the cut.
const CUTTING_EDGE_SHARE: f64 = 0.25;
/// Share of a fiber's cutting resistance the knife spends from its momentum to
/// sever it, so a long slash slows down.
const BLADE_CUT_COST: f64 = 0.5;
/// A knife stopped by a fiber or bone keeps this share of its motion along it,
/// so it slides along skin or between fibers it cannot cut.
const BLADE_SLIDE: f64 = 0.5;
/// Most motion steps one tool step is split into, so a fast swing cannot skip
/// over a thin limb or fiber between steps.
const MAX_TOOL_SUBSTEPS: usize = 12;
/// How firmly tissue pushes back out of a blunt tool in each solver pass.
const TOOL_CONTACT_STIFFNESS: f64 = 0.5;
/// Share of the tissue's push-back that becomes the tool's rebound velocity.
const TOOL_REBOUND: f64 = 0.6;
/// Bruising load per unit of momentum a blow gives a point of tissue, so a
/// point knocked hard is bruised while one pressed slowly is not.
const BRUISE_PER_IMPULSE: f64 = 2.0;
/// How long after it last touched the body a held tool is still pressed
/// rather than swung, in seconds, so one that bounces off is not flung
/// straight back at the body.
const TOUCH_GRACE: f64 = 0.1;
/// Share of the newest pointer motion that goes into the hand's velocity each
/// step; the rest smooths over uneven steps between pointer updates.
const HAND_VELOCITY_UPDATE: f64 = 0.5;
/// A press that lands further from the hand than this many steps of the
/// tool's top speed, as a finger put down somewhere new does, takes the tool
/// up there afresh instead of flinging it across the screen.
const REGRIP_JUMP_STEPS: f64 = 2.0;
/// Speed of a firm swing, in pixels per second. A bat or hammer crushes and
/// bruises flesh in proportion to the energy of its blow, so a blow at this
/// speed loads tissue by its momentum, a slower one much less (half the speed,
/// a quarter of the load), and a faster one more.
const FIRM_SWING_SPEED: f64 = 1600.0;
/// Share of a blunt tool's remaining speed spent breaking a bone it hits:
/// snapping bone and driving the limb aside take much of a blow's energy, so a
/// tool does not plow on at full speed through the limb it just broke.
const FRACTURE_SPEED_LOSS: f64 = 0.45;
/// How far around its face a bat or hammer bruises, as a multiple of the
/// face's reach: a blow crushes the flesh around where it lands, too.
const BRUISE_HALO: f64 = 2.2;
/// Share of a blow's load that reaches the flesh around the face.
const BRUISE_HALO_SHARE: f64 = 0.5;
/// Share of the arm's full push a bat or hammer presses with; see
/// `ToolHandling::press_share`.
const PRESS_SHARE: f64 = 0.5;

/// How a hand moves a tool. Carried or swung through the air, the hand grips
/// it firmly: a stiff spring pulls it to where the hand will be and the grip
/// damps any difference from the hand's own motion, so it moves with the
/// pointer, only easing into sudden starts and stops. Touching the body, the
/// arm presses it instead.
#[derive(Clone, Copy, Debug)]
struct ToolHandling {
    /// Acceleration per pixel the tool would miss the hand by, and per unit of
    /// velocity it differs from the hand's, while it moves free.
    drive: f64,
    damping: f64,
    /// Acceleration per pixel the pointer is past a tool touching the body, up
    /// to `press_share` of `hand_press_force`, and velocity damping, so a
    /// light push rests against the body and a harder one presses in.
    press_drive: f64,
    press_damping: f64,
    /// Share of the arm's full push a tool presses with. A broad bat or hammer
    /// face spreads a push the arm can only half put behind it.
    press_share: f64,
    /// Fastest the striking part moves, in pixels per second. For the 1.75 m
    /// adult the figure stands for, 1,000 px/s is about 3.1 m/s
    /// (`docs/INJURY_REFERENCE.md`).
    max_speed: f64,
    /// A press farther from the hand than two steps at this speed is a finger
    /// put down somewhere new, not a swing.
    regrip_speed: f64,
    /// Farthest lead the hand presses with: pulling the pointer farther from a
    /// tool stuck in the body does not press it any harder.
    max_reach: f64,
    /// How far the hand on the handle is from the striking part or the middle
    /// of the blade. A tool turns in the air no faster than that part could
    /// swing round the hand: its speed over this distance.
    swing_radius: f64,
}

fn tool_handling(tool: ToolMode) -> ToolHandling {
    match tool {
        // A knife is gripped a little more gently, so a light push rests its
        // edge on the skin instead of snapping it into the cut.
        ToolMode::Sharp => ToolHandling {
            drive: 400.0,
            damping: 34.0,
            press_drive: 132.0,
            press_damping: 13.0,
            press_share: 1.0,
            // The fastest slashes measured reach about 15 m/s (Bleetman et
            // al. 2003).
            max_speed: 4800.0,
            regrip_speed: 4600.0,
            max_reach: 150.0,
            swing_radius: 60.0,
        },
        // A sledgehammer is slower to get going than a bat and tops out
        // lower, but carries far more momentum once it is swinging.
        ToolMode::Heavy => ToolHandling {
            drive: 600.0,
            damping: 42.0,
            press_drive: 96.0,
            press_damping: 14.0,
            press_share: PRESS_SHARE,
            // About 10.6 m/s: no study has measured a sledgehammer swing, and
            // this is inside the energies people strike with (Sprenger et al.
            // 2016).
            max_speed: 3400.0,
            regrip_speed: 3400.0,
            max_reach: 150.0,
            swing_radius: 140.0,
        },
        ToolMode::Blunt => ToolHandling {
            drive: 800.0,
            damping: 48.0,
            press_drive: 118.0,
            press_damping: 15.0,
            press_share: PRESS_SHARE,
            // About 25 m/s: an ordinary man swings a bat at about 15.5 m/s,
            // trained hitters at over 30 (Guo et al. 2026; Escamilla et al.
            // 2009).
            max_speed: 8000.0,
            regrip_speed: 4200.0,
            max_reach: 150.0,
            swing_radius: 170.0,
        },
    }
}

/// How hard a person swings each tool, as the strike power the app always
/// plays with: a knife or a bat in one hand, a sledgehammer in both.
pub fn swing_power(tool: ToolMode) -> f64 {
    match tool {
        ToolMode::Sharp => 3.0,
        ToolMode::Blunt => 2.1,
        ToolMode::Heavy => 2.6,
    }
}

/// Dimensions of a tool in world pixels, measured from the point the hand drives.
#[derive(Clone, Copy, Debug)]
pub struct ToolGeometry {
    /// Contact segment extent behind and ahead of the driven point, along the
    /// heading (knife, hammer) or along `-side` toward the bat's end.
    pub contact_back: f64,
    pub contact_front: f64,
    /// Half thickness of the striking part where it touches tissue.
    pub contact_radius: f64,
    /// Extra reach before contact starts, softening first touch.
    pub contact_padding: f64,
    /// Visual half width of the blade, hammer head, or bat barrel.
    pub body_half_width: f64,
    pub handle_length: f64,
    pub handle_half_width: f64,
}

pub fn tool_geometry(tool: ToolMode) -> ToolGeometry {
    match tool {
        ToolMode::Sharp => ToolGeometry {
            contact_back: 33.0,
            contact_front: 33.0,
            contact_radius: 2.0,
            contact_padding: 1.5,
            body_half_width: 6.5,
            handle_length: 44.0,
            handle_half_width: 4.5,
        },
        ToolMode::Heavy => ToolGeometry {
            contact_back: 17.0,
            contact_front: 17.0,
            contact_radius: 16.0,
            contact_padding: 6.0,
            body_half_width: 16.0,
            handle_length: 132.0,
            handle_half_width: 4.5,
        },
        ToolMode::Blunt => ToolGeometry {
            contact_back: 58.0,
            contact_front: 18.0,
            contact_radius: 12.0,
            contact_padding: 5.0,
            body_half_width: 12.0,
            handle_length: 150.0,
            handle_half_width: 5.0,
        },
    }
}

/// Where a tool's striking part is in the world.
#[derive(Clone, Copy, Debug)]
pub struct ToolPose {
    pub tool: ToolMode,
    /// The point the hand drives.
    pub center: Vec2,
    /// Direction the tool travels and strikes with.
    pub heading: Vec2,
    /// Perpendicular to `heading`, toward the hand for the hammer and bat.
    pub side: Vec2,
    /// Centerline of the striking part: knife guard to tip, hammer back face to
    /// striking face, bat barrel from the handle side to the end.
    pub contact_start: Vec2,
    pub contact_end: Vec2,
    pub contact_radius: f64,
}

pub fn tool_pose(tool: ToolMode, center: Vec2, heading: Vec2, side: Vec2) -> ToolPose {
    let geometry = tool_geometry(tool);
    let (contact_start, contact_end) = match tool {
        ToolMode::Sharp | ToolMode::Heavy => (
            subtract(center, scale(heading, geometry.contact_back)),
            add(center, scale(heading, geometry.contact_front)),
        ),
        ToolMode::Blunt => (
            add(center, scale(side, geometry.contact_back)),
            subtract(center, scale(side, geometry.contact_front)),
        ),
    };
    ToolPose {
        tool,
        center,
        heading,
        side,
        contact_start,
        contact_end,
        contact_radius: geometry.contact_radius,
    }
}

/// What the striking part of the tool in play reaches where it is, for
/// diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToolReach {
    /// Tissue points of flesh, and of flesh torn away, which nothing draws.
    pub flesh: usize,
    pub torn_away: usize,
    /// Bones, and the spine apart, which is never drawn.
    pub bones: usize,
    pub spine: usize,
}

/// The tool in play: where it is, how it moves, and what it presses on.
#[derive(Clone, Debug)]
pub(super) struct ToolBody {
    mode: ToolMode,
    /// A tool is in play. It appears at the hand when input becomes active.
    pub(super) present: bool,
    /// The point the hand drives, in the middle of the striking part.
    pub(super) position: Vec2,
    velocity: Vec2,
    /// Direction the tool faces; it follows the motion when free.
    pub(super) heading: Vec2,
    /// Perpendicular to the heading, toward the hand on the hammer and bat.
    pub(super) side: Vec2,
    /// The tool was in tissue last step, so it resists turning.
    embedded: bool,
    /// The knife rests against a fiber or bone it could not get through.
    held: bool,
    /// The button was down last step.
    was_down: bool,
    /// Seconds left in which the tool counts as touching the body since it
    /// last did; see `TOUCH_GRACE`.
    touch_grace: f64,
    /// Where the hand was last step, and how fast it moves.
    last_hand: Option<Vec2>,
    hand_velocity: Vec2,
    /// A bat or hammer gripped while inside the body passes through it until
    /// it is clear; it cannot appear in flesh and blast it apart.
    ghost: bool,
    /// A blunt tool's contact this step, which the solver keeps resolving so
    /// the tissue pushes back on the tool.
    contact: Option<ToolSolverContact>,
    /// How far the tissue has pushed the tool back during this step's solver.
    solver_shift: Vec2,
}

impl Default for ToolBody {
    fn default() -> Self {
        Self {
            mode: ToolMode::default(),
            present: false,
            position: Vec2::default(),
            velocity: Vec2::default(),
            heading: Vec2 { x: 1.0, y: 0.0 },
            side: Vec2 { x: 0.0, y: 1.0 },
            embedded: false,
            held: false,
            was_down: false,
            touch_grace: 0.0,
            last_hand: None,
            hand_velocity: Vec2::default(),
            ghost: false,
            contact: None,
            solver_shift: Vec2::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ToolSolverContact {
    shape: ToolContactShape,
    inertia: f64,
    /// Tissue points close enough to touch the tool this step.
    candidates: Vec<usize>,
}

/// What one tool step did to a tissue point it touched.
#[derive(Clone, Copy, Debug, Default)]
struct PointHit {
    /// Hardest contact load, from the tool's momentum and how deep it pressed.
    load: f64,
    /// Momentum the tool gave the point.
    impulse: f64,
    /// How far the tool pressed into the point over the step's sub-steps, so
    /// the load does not depend on how finely the motion is divided.
    depth: f64,
}

/// One step's physical values for the tool in hand.
#[derive(Clone, Copy, Debug)]
struct ToolStrike {
    tool: ToolMode,
    profile: ToolProfile,
    power: f64,
    /// The tool's mass, which sets how hard it hits.
    mass: f64,
    /// The mass that carries momentum into the body: the tool and the arm
    /// swinging it.
    inertia: f64,
}

fn contact_shape(pose: &ToolPose) -> ToolContactShape {
    let geometry = tool_geometry(pose.tool);
    ToolContactShape {
        axis_start: pose.contact_start,
        axis_end: pose.contact_end,
        direction: pose.heading,
        blade_normal: pose.side,
        radius: pose.contact_radius,
        influence: pose.contact_radius + geometry.contact_padding,
        cutting_edge: pose.tool == ToolMode::Sharp,
    }
}

/// The front part of a blade, which cuts: from the end of the cutting edge to
/// the tip.
fn cutting_edge(shape: &ToolContactShape) -> (Vec2, Vec2) {
    (
        lerp(shape.axis_end, shape.axis_start, CUTTING_EDGE_SHARE),
        shape.axis_end,
    )
}

fn rotate_toward(from: Vec2, to: Vec2, max_angle: f64) -> Vec2 {
    let angle = wrap_angle(libm::atan2(to.y, to.x) - libm::atan2(from.y, from.x));
    if angle.abs() <= max_angle {
        return to;
    }
    let step = max_angle * angle.signum();
    let (sin, cos) = (libm::sin(step), libm::cos(step));
    Vec2 {
        x: from.x * cos - from.y * sin,
        y: from.x * sin + from.y * cos,
    }
}

/// Where two segments cross, as (position along the first, position along the
/// second).
fn segment_crossing(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> Option<(f64, f64)> {
    let r = subtract(a1, a0);
    let s = subtract(b1, b0);
    let denominator = cross(r, s);
    if denominator.abs() < EPSILON {
        return None;
    }
    let offset = subtract(b0, a0);
    let t = cross(offset, s) / denominator;
    let u = cross(offset, r) / denominator;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}

/// The part of `velocity` along `along`, scaled by `keep`.
fn slide_along(velocity: Vec2, along: Vec2, keep: f64) -> Vec2 {
    let along = normalized(along, Vec2 { x: 1.0, y: 0.0 });
    scale(along, dot(velocity, along) * keep)
}

impl World {
    /// Direction the current tool faces and strikes with.
    pub fn tool_heading(&self) -> Vec2 {
        self.tool.heading
    }

    /// Perpendicular to the heading, toward the hand on the hammer and bat.
    pub fn tool_side(&self) -> Vec2 {
        self.tool.side
    }

    /// The point the hand drives, in the middle of the tool's striking part.
    pub fn tool_position(&self) -> Vec2 {
        self.tool.position
    }

    pub fn tool_velocity(&self) -> Vec2 {
        self.tool.velocity
    }

    /// The knife is resting against tissue or bone it could not cut through.
    pub fn tool_held(&self) -> bool {
        self.tool.held
    }

    /// The tool's pose for drawing, once a tool is in play.
    pub fn current_tool_pose(&self) -> Option<ToolPose> {
        self.tool.present.then(|| {
            tool_pose(
                self.tool.mode,
                self.tool.position,
                self.tool.heading,
                self.tool.side,
            )
        })
    }

    /// What the tool's striking part reaches where it is now.
    pub fn tool_reach(&self) -> ToolReach {
        let Some(pose) = self.current_tool_pose() else {
            return ToolReach::default();
        };
        let shape = contact_shape(&pose);
        let flesh = self.flesh_points();
        let mut reach = ToolReach::default();
        for (index, point) in self.points.iter().enumerate() {
            if point.pinned
                || sample_point_contact(point.position, &shape).distance > shape.influence
            {
                continue;
            }
            if flesh[index] {
                reach.flesh += 1;
            } else {
                reach.torn_away += 1;
            }
        }
        for bone in &self.bones {
            let gap = closest_segment_points(shape.axis_start, shape.axis_end, bone.a, bone.b);
            if gap.distance > shape.influence + bone.radius {
                continue;
            }
            if bone.kind == BoneKind::Spine {
                reach.spine += 1;
            } else {
                reach.bones += 1;
            }
        }
        reach
    }

    /// Turns the tool for this step's motion; `down` is whether the button is
    /// held.
    fn turn_tool(&mut self, down: bool, dt: f64) {
        if self.tool.mode == ToolMode::Sharp {
            self.turn_blade(dt);
        } else {
            self.turn_held_tool(down, dt);
        }
    }

    /// A knife turns in the air to lead with its tip, as fast as the hand can
    /// swing the blade round, so it does not snap round when the hand turns
    /// back. Tissue steers an embedded knife to follow its stroke, like a
    /// scalpel, except that a knife pulled backward keeps its line and
    /// withdraws rather than flipping around inside the wound.
    fn turn_blade(&mut self, dt: f64) {
        let velocity = self.tool.velocity;
        let speed = hypot(velocity.x, velocity.y);
        if speed > TOOL_TURN_SPEED {
            let target = scale(velocity, 1.0 / speed);
            if !self.tool.embedded {
                let radius = tool_handling(ToolMode::Sharp).swing_radius.max(1.0);
                self.tool.heading = rotate_toward(self.tool.heading, target, speed / radius * dt);
            } else if dot(target, self.tool.heading) > -0.2 {
                self.tool.heading =
                    rotate_toward(self.tool.heading, target, EMBEDDED_BLADE_TURN_RATE * dt);
            }
        }
        self.tool.side = side_toward(self.tool.heading, self.tool.side);
    }

    /// A bat or hammer is held by its handle. Swung, it turns until its
    /// striking part leads with the handle across the path, on the side the
    /// handle already is, but no faster than the striking part could swing
    /// round the hands on the handle, and only a little while flesh holds it.
    /// Carried with the button up, it settles back upright instead of turning
    /// with every move of the hand.
    fn turn_held_tool(&mut self, down: bool, dt: f64) {
        let velocity = self.tool.velocity;
        let speed = hypot(velocity.x, velocity.y);
        let side = self.tool.side;
        if !down {
            self.tool.side = rotate_toward(side, CARRIED_HANDLE, CARRY_TURN_RATE * dt);
        } else if speed > TOOL_TURN_SPEED {
            let across = side_toward(scale(velocity, 1.0 / speed), side);
            let share = if self.tool.embedded {
                EMBEDDED_TURN_SHARE
            } else {
                1.0
            };
            let radius = tool_handling(self.tool.mode).swing_radius.max(1.0);
            self.tool.side = rotate_toward(side, across, speed / radius * share * dt);
        }
        self.tool.side = normalized(self.tool.side, CARRIED_HANDLE);
        // The striking face is across the handle, facing the way the tool
        // moves, or the way it faced when it is nearly still.
        let toward = if speed > TOOL_TURN_SPEED {
            velocity
        } else {
            self.tool.heading
        };
        self.tool.heading = side_toward(self.tool.side, toward);
    }

    /// A tool that appears mid-swing already faces along its motion, as one
    /// being swung would.
    fn align_with_motion(&mut self) {
        let velocity = self.tool.velocity;
        let speed = hypot(velocity.x, velocity.y);
        if speed > TOOL_TURN_SPEED {
            self.tool.heading = scale(velocity, 1.0 / speed);
            self.tool.side = side_toward(self.tool.heading, self.tool.side);
        }
    }

    /// Moves the tool one step: the hand pulls it toward the pointer and the
    /// body resists. `input.x`/`y` is where the hand is. While the button is up
    /// the tool only follows the pointer and passes over the body.
    pub(super) fn move_tool(&mut self, input: &InputState, dt: f64) {
        self.tool.contact = None;
        self.tool.solver_shift = Vec2::default();
        self.blunt_knock.clear();
        self.blunt_knock.resize(self.points.len(), 0.0);
        if !input.active {
            self.tool.present = false;
            self.tool.embedded = false;
            self.tool.held = false;
            self.tool.was_down = false;
            self.tool.touch_grace = 0.0;
            self.tool.last_hand = None;
            self.tool.ghost = false;
            return;
        }
        if input.tool != self.tool.mode {
            self.tool.mode = input.tool;
            self.tool.embedded = false;
            self.tool.held = false;
            self.tool.touch_grace = 0.0;
        }
        let target = Vec2 {
            x: input.x,
            y: input.y,
        };
        let handling = tool_handling(input.tool);
        let profile = tool_profile(input.tool);
        let mass = self.materials.striker_mass * input.power * profile.mass_scale;
        let strike = ToolStrike {
            tool: input.tool,
            profile,
            power: input.power,
            mass,
            inertia: mass * self.materials.tool_inertia_scale.max(EPSILON),
        };
        // A hand carries and swings any tool closely, but presses one into the
        // body no harder than an arm can push, whatever the tool weighs.
        self.tool.touch_grace = if self.tool.embedded || self.tool.held {
            TOUCH_GRACE
        } else {
            (self.tool.touch_grace - dt).max(0.0)
        };
        // A press that lands far from where the hand was, as a finger put
        // down somewhere new does, takes the tool up there afresh instead of
        // flinging it across the screen.
        let pressed = input.down && !self.tool.was_down;
        let regrip = pressed
            && self.tool.present
            && self.tool.last_hand.is_some_and(|last| {
                distance(target, last) > handling.regrip_speed * dt * REGRIP_JUMP_STEPS
            });
        if regrip {
            self.tool.present = false;
            self.tool.last_hand = None;
            self.tool.embedded = false;
            self.tool.held = false;
            self.tool.touch_grace = 0.0;
        }
        let pressing = input.down && self.tool.touch_grace > 0.0;
        // Pressing the button grips the tool in hand, and so does taking a tool
        // up at rest with the button down, as a finger put down on a fresh body
        // does; a bat or hammer gripped inside the body passes through until it
        // is clear. A tool that appears already swinging, as a scripted
        // strike's does, is mid-swing instead.
        let appears_at_rest = !self.tool.present && input.vx == 0.0 && input.vy == 0.0;
        let gripped = pressed && (self.tool.present || regrip || appears_at_rest);
        self.tool.was_down = input.down;
        // How hard the arm pushes the tool toward the pointer, which is what
        // drives a knife through fibers.
        let mut push = Vec2::default();
        let moved = match self.tool.last_hand {
            Some(last) => scale(subtract(target, last), 1.0 / dt.max(EPSILON)),
            None => Vec2 {
                x: input.vx,
                y: input.vy,
            },
        };
        self.tool.hand_velocity = if self.tool.last_hand.is_some() && self.tool.present {
            lerp(self.tool.hand_velocity, moved, HAND_VELOCITY_UPDATE)
        } else {
            moved
        };
        self.tool.last_hand = Some(target);
        if !self.tool.present {
            // A tool appears at the hand already moving with it, so a scripted
            // strike can begin mid-swing.
            self.tool.present = true;
            self.tool.velocity = Vec2 {
                x: input.vx,
                y: input.vy,
            };
            self.tool.position = subtract(target, scale(self.tool.velocity, dt));
            self.tool.embedded = false;
            self.tool.held = false;
            self.align_with_motion();
        } else {
            let reach = if input.down {
                handling.max_reach
            } else {
                f64::MAX
            };
            let lead = clamp_magnitude(subtract(target, self.tool.position), reach);
            push = clamp_magnitude(
                scale(lead, handling.press_drive),
                self.materials.hand_press_force * handling.press_share / strike.inertia,
            );
            let accel = if pressing {
                subtract(push, scale(self.tool.velocity, handling.press_damping))
            } else {
                // Where the tool would miss the hand after this step if it kept
                // its velocity, and how its velocity differs from the hand's.
                let miss = subtract(lead, scale(self.tool.velocity, dt));
                let slip = subtract(self.tool.hand_velocity, self.tool.velocity);
                add(scale(miss, handling.drive), scale(slip, handling.damping))
            };
            self.tool.velocity = add(self.tool.velocity, scale(accel, dt));
            self.tool.velocity = clamp_magnitude(self.tool.velocity, handling.max_speed);
        }

        let speed = hypot(self.tool.velocity.x, self.tool.velocity.y);
        self.debug.striker_position = self.tool.position;
        self.debug.striker_velocity = self.tool.velocity;
        self.debug.striker_speed = speed;
        self.debug.striker_mass = mass;
        self.debug.impact = speed * mass;
        for bone in &mut self.bones {
            bone.load *= 0.88;
            bone.break_load *= 0.88;
        }

        let start = tool_pose(
            input.tool,
            self.tool.position,
            self.tool.heading,
            self.tool.side,
        );
        self.turn_tool(input.down, dt);
        if input.tool == ToolMode::Sharp || !input.down {
            self.tool.ghost = false;
        } else if gripped || self.tool.ghost {
            self.tool.ghost = self.blunt_overlaps_tissue(&start);
        }
        if !input.down || self.tool.ghost {
            self.tool.position = add(self.tool.position, scale(self.tool.velocity, dt));
            self.tool.embedded = false;
            self.tool.held = false;
            return;
        }
        if input.tool == ToolMode::Sharp {
            self.move_blade(input, strike, &start, push, dt);
        } else {
            self.move_blunt(input, strike, &start, dt);
        }
    }

    fn blunt_overlaps_tissue(&self, pose: &ToolPose) -> bool {
        let shape = contact_shape(pose);
        self.points.iter().enumerate().any(|(index, point)| {
            !point.pinned
                && is_flesh(&self.flesh, index)
                && sample_point_contact(point.position, &shape).distance < shape.radius
        })
    }

    /// The knife advances only through fibers it severs. Each fiber its tip or
    /// cutting edge reaches is cut if the knife's momentum or the hand's push
    /// overcomes it; otherwise the knife stops against it, as it does at bone.
    fn move_blade(
        &mut self,
        input: &InputState,
        strike: ToolStrike,
        start_pose: &ToolPose,
        hand_push: Vec2,
        dt: f64,
    ) {
        let start = contact_shape(start_pose);
        let start_center = self.tool.position;
        let end_center = add(start_center, scale(self.tool.velocity, dt));
        let end = contact_shape(&tool_pose(
            strike.tool,
            end_center,
            self.tool.heading,
            self.tool.side,
        ));
        let travel =
            distance(start.axis_end, end.axis_end).max(distance(start.axis_start, end.axis_start));
        let spacing = (self.materials.point_spacing * 0.45).max(1.0);
        let steps = ((travel / spacing).ceil() as usize).clamp(1, MAX_TOOL_SUBSTEPS);
        let candidates = self.springs_near_blade(&start, &end);
        // Fibers already lying across the edge were there when the knife came
        // down into the body; only fibers it reaches now resist it.
        let resting: Vec<usize> = candidates
            .iter()
            .copied()
            .filter(|&index| self.spring_crosses_edge(index, &start))
            .collect();

        let mut speed = hypot(self.tool.velocity.x, self.tool.velocity.y);
        let mut reached = start;
        let mut reached_center = start_center;
        let mut stop_along = None;
        let mut passed = Vec::with_capacity(steps);
        for step in 1..=steps {
            let t = step as f64 / steps as f64;
            let shape = ToolContactShape {
                axis_start: lerp(start.axis_start, end.axis_start, t),
                axis_end: lerp(start.axis_end, end.axis_end, t),
                ..end
            };
            let direction = normalized(subtract(shape.axis_end, reached.axis_end), end.direction);
            let push = strike.mass * dot(hand_push, direction).max(0.0) * dt;
            // Fibers first: the skin over a bone has to be cut before the knife
            // can reach it.
            let mut stopped = None;
            // How far along this part of the stroke the tip got before a fiber
            // held it, so the knife rests right against that fiber.
            let mut stop_fraction = 0.0;
            for (spring_index, along, by_tip, cut_at) in
                self.springs_reached(&candidates, &resting, &reached, &shape)
            {
                let spring = self.springs[spring_index];
                if spring.broken || spring.cut {
                    continue;
                }
                let force = (strike.mass * speed).max(push);
                let pressure =
                    force * strike.profile.cut_pressure_scale * self.blade_layer_scale(spring);
                let threshold = spring.tear_impulse * self.materials.sharp_tool_tear_pressure;
                self.springs[spring_index].stress = self.springs[spring_index]
                    .stress
                    .max(pressure / threshold.max(1.0));
                if pressure <= threshold {
                    let a = self.points[spring.a].position;
                    let b = self.points[spring.b].position;
                    stopped = Some(subtract(b, a));
                    if by_tip {
                        let tip_travel = distance(reached.axis_end, shape.axis_end).max(EPSILON);
                        stop_fraction = (along - 0.5 / tip_travel).max(0.0);
                    }
                    break;
                }
                self.sever_with_blade(spring_index, cut_at, pressure, shape.blade_normal, strike);
                speed = (speed - threshold * BLADE_CUT_COST / strike.inertia).max(0.0);
            }
            if stopped.is_none() {
                let force = (strike.mass * speed).max(push);
                stopped = self.blade_meets_bone(&reached, &shape, strike, force);
            }
            let center = lerp(start_center, end_center, t);
            if stopped.is_some() {
                stop_along = stopped;
                reached_center = lerp(reached_center, center, stop_fraction);
                break;
            }
            reached = shape;
            reached_center = center;
            passed.push((shape, (strike.mass * speed).max(push)));
        }

        let direction = normalized(self.tool.velocity, self.tool.heading);
        self.tool.velocity = match stop_along {
            Some(along) => slide_along(self.tool.velocity, along, BLADE_SLIDE),
            None => scale(direction, speed),
        };
        self.tool.position = reached_center;
        self.tool.held = stop_along.is_some();
        for (shape, impact) in &passed {
            self.lacerate_major_vessels_from_striker(input, shape, *impact);
            self.penetrate_organs_from_striker(input, shape, *impact);
        }
        let alongside = self.part_tissue_around_blade(strike, &reached, strike.mass * speed, dt);
        self.tool.embedded = alongside > 0;
        self.open_blade_cuts();
    }

    /// Unbroken springs whose bounds come near the cutting edge's path this step.
    fn springs_near_blade(&self, start: &ToolContactShape, end: &ToolContactShape) -> Vec<usize> {
        let (start_edge, start_tip) = cutting_edge(start);
        let (end_edge, end_tip) = cutting_edge(end);
        let margin = self.materials.point_spacing;
        let min = Vec2 {
            x: start_edge.x.min(start_tip.x).min(end_edge.x).min(end_tip.x) - margin,
            y: start_edge.y.min(start_tip.y).min(end_edge.y).min(end_tip.y) - margin,
        };
        let max = Vec2 {
            x: start_edge.x.max(start_tip.x).max(end_edge.x).max(end_tip.x) + margin,
            y: start_edge.y.max(start_tip.y).max(end_edge.y).max(end_tip.y) + margin,
        };
        self.springs
            .iter()
            .enumerate()
            .filter(|(_, spring)| {
                if spring.broken || spring.cut || !self.spring_in_flesh(**spring) {
                    return false;
                }
                let a = self.points[spring.a].position;
                let b = self.points[spring.b].position;
                a.x.max(b.x) >= min.x
                    && a.x.min(b.x) <= max.x
                    && a.y.max(b.y) >= min.y
                    && a.y.min(b.y) <= max.y
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn spring_crosses_edge(&self, spring_index: usize, shape: &ToolContactShape) -> bool {
        self.edge_crossing(spring_index, shape).is_some()
    }

    /// For each point on a cut, the side of the cut along `normal` its
    /// triangles lie on: 1 or -1 for a lip, 0 for a point whose triangles
    /// still lie on both sides, and for points not on a cut.
    fn lip_sides(&self, normal: Vec2) -> Vec<f64> {
        let mut lean = vec![0.0; self.points.len()];
        for triangle in &self.triangles {
            if !self.triangle_alive(triangle) {
                continue;
            }
            let corners = [triangle.a, triangle.b, triangle.c];
            if corners.iter().all(|&corner| !self.points[corner].on_cut) {
                continue;
            }
            let centroid = scale(
                add(
                    add(
                        self.points[triangle.a].position,
                        self.points[triangle.b].position,
                    ),
                    self.points[triangle.c].position,
                ),
                1.0 / 3.0,
            );
            for corner in corners {
                if self.points[corner].on_cut {
                    lean[corner] += dot(subtract(centroid, self.points[corner].position), normal);
                }
            }
        }
        let balanced = self.materials.point_spacing * 0.25;
        lean.into_iter()
            .map(|lean| {
                if lean > balanced {
                    1.0
                } else if lean < -balanced {
                    -1.0
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// The blade meets `spring` at an end already on a cut, `cut_at` of the
    /// way along it: it passes through the cut there, not through the fiber.
    fn crosses_at_cut(&self, spring: Spring, cut_at: f64) -> bool {
        const END: f64 = 0.05;
        (cut_at < END && self.points[spring.a].on_cut)
            || (cut_at > 1.0 - END && self.points[spring.b].on_cut)
    }

    /// Where along spring `spring_index`, from its `a` end, the cutting edge
    /// lies across it, if it does.
    fn edge_crossing(&self, spring_index: usize, shape: &ToolContactShape) -> Option<f64> {
        let spring = self.springs[spring_index];
        let (edge, tip) = cutting_edge(shape);
        segment_crossing(
            self.points[spring.a].position,
            self.points[spring.b].position,
            edge,
            tip,
        )
        .map(|(along_spring, _)| along_spring)
    }

    /// Fibers the tip passes through between two blade positions, or that newly
    /// lie across the cutting edge, nearest along the tip's path first, with
    /// where along that path the tip met each one, whether the tip did, and
    /// where along the fiber the blade crossed it. The rest of the blade
    /// follows the tip through the cut, so it neither cuts nor catches;
    /// letting it cut would scythe through tissue whenever the knife turns.
    fn springs_reached(
        &self,
        candidates: &[usize],
        resting: &[usize],
        from: &ToolContactShape,
        to: &ToolContactShape,
    ) -> Vec<(usize, f64, bool, f64)> {
        let mut reached: Vec<(usize, f64, bool, f64)> = candidates
            .iter()
            .filter_map(|&index| {
                let spring = self.springs[index];
                if spring.broken || spring.cut || spring.parted {
                    return None;
                }
                let a = self.points[spring.a].position;
                let b = self.points[spring.b].position;
                if let Some((cut_at, along)) = segment_crossing(a, b, from.axis_end, to.axis_end) {
                    return (!self.crosses_at_cut(spring, cut_at))
                        .then_some((index, along, true, cut_at));
                }
                if resting.contains(&index) {
                    return None;
                }
                self.edge_crossing(index, to)
                    .filter(|&cut_at| !self.crosses_at_cut(spring, cut_at))
                    .map(|cut_at| (index, 1.0, false, cut_at))
            })
            .collect();
        reached.sort_by(|a, b| a.1.total_cmp(&b.1));
        reached
    }

    /// A knife cannot cut bone: it stops where its tip or edge would move into
    /// one. Returns the bone's direction, along which the blade can slide.
    fn blade_meets_bone(
        &mut self,
        from: &ToolContactShape,
        to: &ToolContactShape,
        strike: ToolStrike,
        force: f64,
    ) -> Option<Vec2> {
        let (from_edge, from_tip) = cutting_edge(from);
        let (to_edge, to_tip) = cutting_edge(to);
        for index in 0..self.bones.len() {
            let mut bone = self.bones[index];
            // The spine lies behind the trunk, so a blade cutting the front of
            // the chest or belly passes in front of it.
            if bone.kind == BoneKind::Spine {
                continue;
            }
            let reach = bone.radius + to.radius;
            let before = closest_segment_points(from_edge, from_tip, bone.a, bone.b).distance;
            let path = closest_segment_points(from_tip, to_tip, bone.a, bone.b).distance;
            let after = closest_segment_points(to_edge, to_tip, bone.a, bone.b);
            let touches = path < reach || after.distance < reach;
            // Resting on a bone already, the knife may still move off it.
            let deeper = before >= reach || after.distance < before - 0.05;
            if !touches || !deeper {
                continue;
            }
            let load = force * strike.profile.bone_load_scale / strike.profile.fracture_scale;
            // A blade notches a long bone but does not break it; that takes a
            // heavy chopping blade (Lynn and Fairgrieve 2009). It can still
            // cut through a rib (Bolliger et al. 2016).
            let load = if bone.part.is_long_bone() {
                load.min(bone.fracture_impulse * BLADE_LONG_BONE_LOAD)
            } else {
                load
            };
            bone.load = bone.load.max(load);
            bone.break_load = bone.break_load.max(load * strike.profile.break_scale);
            if load > self.materials.fragment_wake_load {
                self.wake_fragment(&mut bone);
            }
            self.bones[index] = bone;
            self.debug.bone_contacts += 1;
            self.debug.max_bone_load = self.debug.max_bone_load.max(bone.load);
            let depth = (reach - after.distance).max(0.0);
            if depth > self.debug.max_depth {
                self.debug.max_depth = depth;
                self.debug.strongest_contact = after.point_b;
            }
            return Some(subtract(bone.b, bone.a));
        }
        None
    }

    /// How much more easily a blade parts this fiber than skin: skin is the
    /// tough layer, and the muscle under it cuts readily once reached.
    fn blade_layer_scale(&self, spring: Spring) -> f64 {
        if spring.layer == TissueLayer::Skin {
            1.0
        } else {
            let exposure = self.points[spring.a]
                .exposure
                .max(self.points[spring.b].exposure);
            1.3 + exposure * 0.2
        }
    }

    /// Severs one fiber under the blade and opens a wound where it parted.
    fn sever_with_blade(
        &mut self,
        spring_index: usize,
        cut_at: f64,
        pressure: f64,
        blade_normal: Vec2,
        strike: ToolStrike,
    ) {
        let spring = self.springs[spring_index];
        self.cut_spring(spring_index, cut_at);
        let skin = spring.layer == TissueLayer::Skin;
        let exposure = if skin { 0.92 } else { 1.0 };
        self.bump_point_exposure_load(spring.a, exposure, pressure * 0.18);
        self.bump_point_exposure_load(spring.b, exposure, pressure * 0.18);
        if skin {
            self.stats.broken_skin += 1;
            self.fresh_skin_cuts.push(spring_index);
        } else {
            self.stats.broken_muscle += 1;
            if spring.fiber {
                self.stats.muscle_fiber_tears += 1;
                self.debug.muscle_fiber_tears += 1;
            }
        }
        let a = self.points[spring.a].position;
        let b = self.points[spring.b].position;
        let center = midpoint(a, b);
        let tangent = normalized(subtract(b, a), Vec2 { x: 1.0, y: 0.0 });
        let spring_normal = Vec2 {
            x: -tangent.y,
            y: tangent.x,
        };
        // Blood leaves the cut sideways, away from the blade plane.
        let normal = if dot(blade_normal, spring_normal) < 0.0 {
            scale(blade_normal, -1.0)
        } else {
            blade_normal
        };
        // A blade's cut wells blood onto the skin rather than spattering it.
        self.well_fresh_blood(
            center,
            normal,
            if skin { 6 } else { 4 },
            120.0 + pressure * self.materials.fluid_impact_scale * 0.42,
            if skin { 2.1 } else { 1.8 },
            strike.profile.fluid_scale,
        );
        self.open_wound(
            center,
            normal,
            spring.layer,
            pressure / if skin { 1250.0 } else { 1050.0 },
            if skin { 2.1 } else { 1.8 },
            if skin { 0.58 } else { 0.92 },
        );
    }

    /// A blade only nudges tissue off its thin edge sideways; it parts tissue
    /// rather than shoving it. Points ahead of the tip are left for the tip to
    /// pierce. Returns how many tissue points lie along the blade, which marks
    /// it as embedded.
    fn part_tissue_around_blade(
        &mut self,
        strike: ToolStrike,
        shape: &ToolContactShape,
        impact: f64,
        dt: f64,
    ) -> usize {
        let along_reach = self.materials.point_spacing * 0.7;
        let velocity = self.tool.velocity;
        let lip_sides = self.lip_sides(shape.blade_normal);
        let mut alongside = 0;
        for (index, point) in self.points.iter_mut().enumerate() {
            if point.pinned || !is_flesh(&self.flesh, index) {
                continue;
            }
            let t = segment_t(point.position, shape.axis_start, shape.axis_end);
            if t <= 0.0 || t >= 1.0 {
                continue;
            }
            let mut point_contact = sample_point_contact(point.position, shape);
            if point_contact.distance <= along_reach {
                alongside += 1;
            }
            if point_contact.distance > shape.influence {
                continue;
            }
            if point.on_cut {
                // A point on a cut lies on the blade's path, so which way the
                // blade wedges it is the side of the cut its own tissue is on.
                // One the cut has not split yet holds both sides together.
                match lip_sides[index] {
                    0.0 => continue,
                    side => point_contact.normal = scale(shape.blade_normal, side),
                }
            }
            let depth = shape.influence - point_contact.distance;
            point.position.x += point_contact.normal.x * depth + velocity.x * dt * 0.06;
            point.position.y += point_contact.normal.y * depth + velocity.y * dt * 0.06;
            let contact_load =
                impact * (depth / shape.influence) * 0.25 * strike.profile.tissue_push_scale;
            point.load = point.load.max(contact_load);
            if apply_point_contusion(
                point,
                self.materials,
                contact_load,
                strike.profile.contusion_scale,
            ) {
                self.stats.contusion_events += 1;
                self.debug.contusion_events += 1;
                self.debug.max_contusion = self.debug.max_contusion.max(point.contusion);
            }
            self.debug.tissue_contacts += 1;
            if depth > self.debug.max_depth {
                self.debug.max_depth = depth;
                self.debug.strongest_contact = point.position;
            }
            self.debug.max_point_load = self.debug.max_point_load.max(point.load);
        }
        alongside
    }

    /// A bat or hammer shoves the tissue and bone in its way and gives them its
    /// momentum, so it slows and stops in the body instead of sweeping through.
    /// Tissue then keeps pushing back on it while the solver runs.
    fn move_blunt(
        &mut self,
        input: &InputState,
        strike: ToolStrike,
        start_pose: &ToolPose,
        dt: f64,
    ) {
        let start = contact_shape(start_pose);
        let initial_velocity = self.tool.velocity;
        let end_center = add(self.tool.position, scale(initial_velocity, dt));
        let end = contact_shape(&tool_pose(
            strike.tool,
            end_center,
            self.tool.heading,
            self.tool.side,
        ));
        let travel =
            distance(start.axis_start, end.axis_start).max(distance(start.axis_end, end.axis_end));
        let spacing = (self.materials.point_spacing * 0.25).max(1.0);
        let steps = ((travel / spacing).ceil() as usize).clamp(1, MAX_TOOL_SUBSTEPS);
        let sub_dt = dt / steps as f64;

        let mut hits: Vec<Option<PointHit>> = vec![None; self.points.len()];
        let mut velocity = initial_velocity;
        let mut center = self.tool.position;
        let mut shape = start;
        let mut peak_blow: f64 = 0.0;
        for step in 1..=steps {
            let t = step as f64 / steps as f64;
            center = add(center, scale(velocity, sub_dt));
            // The handle turns smoothly through the step, and the face stays
            // across it.
            let side = normalized(lerp(start_pose.side, self.tool.side, t), self.tool.side);
            let heading = side_toward(side, self.tool.heading);
            shape = contact_shape(&tool_pose(strike.tool, center, heading, side));
            let speed = hypot(velocity.x, velocity.y);
            // Bone breaks under the force of the blow, which grows with its
            // momentum; flesh is crushed and bruised by its energy.
            let impact = strike.mass * speed;
            let blow = impact * speed / FIRM_SWING_SPEED;
            let tissue = self.shove_tissue(&shape, velocity, blow, strike, dt, sub_dt, &mut hits);
            let fractures_before = self.stats.fractured_bones;
            let bone = self.shove_bones(&shape, velocity, impact, strike, dt, sub_dt);
            let given = add(tissue, bone);
            if hypot(given.x, given.y) > EPSILON {
                peak_blow = peak_blow.max(blow);
            }
            self.lacerate_major_vessels_from_striker(input, &shape, blow);
            velocity = subtract(velocity, scale(given, 1.0 / strike.inertia));
            if self.stats.fractured_bones > fractures_before {
                velocity = scale(velocity, 1.0 - FRACTURE_SPEED_LOSS);
            }
            if dot(velocity, initial_velocity) <= 0.0 {
                // The body has taken all of the swing's momentum.
                velocity = Vec2::default();
                break;
            }
        }
        self.tool.position = center;
        self.tool.velocity = velocity;
        self.tool.held = false;

        let mut touched = 0;
        for (index, hit) in hits.into_iter().enumerate() {
            let Some(hit) = hit else {
                continue;
            };
            touched += 1;
            self.blunt_knock[index] = hit.load;
            let point = &mut self.points[index];
            // A blow bruises what it knocks hard, but a broad face spreads its
            // force, so it loads the tissue toward tearing far less.
            point.load = point.load.max(hit.load * strike.profile.tissue_load_scale);
            let bruise = hit.load.max(hit.impulse * BRUISE_PER_IMPULSE);
            if apply_point_contusion(
                point,
                self.materials,
                bruise,
                strike.profile.contusion_scale,
            ) {
                self.stats.contusion_events += 1;
                self.debug.contusion_events += 1;
                self.debug.max_contusion = self.debug.max_contusion.max(point.contusion);
            }
            self.debug.tissue_contacts += 1;
            self.debug.max_point_load = self.debug.max_point_load.max(point.load);
        }
        self.tool.embedded = touched > 0;
        if peak_blow > 0.0 {
            self.bruise_around_blow(&shape, peak_blow, strike);
        }
        if strike.profile.crush_tear_scale > 0.0 && peak_blow > 0.0 {
            self.crush_tear_under_tool(strike.profile, &shape, peak_blow);
        }

        let reach = shape.influence + self.materials.point_spacing;
        let candidates: Vec<usize> = self
            .points
            .iter()
            .enumerate()
            .filter(|&(index, point)| {
                !point.pinned
                    && is_flesh(&self.flesh, index)
                    && sample_point_contact(point.position, &shape).distance <= reach
            })
            .map(|(index, _)| index)
            .collect();
        if !candidates.is_empty() {
            self.tool.contact = Some(ToolSolverContact {
                shape,
                inertia: strike.inertia,
                candidates,
            });
        }
    }

    /// Bruises the flesh around where a blow of strength `blow` landed, fading
    /// toward the edge of `BRUISE_HALO`; a slow touch is too weak to bruise.
    fn bruise_around_blow(&mut self, shape: &ToolContactShape, blow: f64, strike: ToolStrike) {
        let reach = shape.influence * BRUISE_HALO;
        let load = blow * 0.58 * strike.profile.tissue_push_scale * BRUISE_HALO_SHARE;
        for (index, point) in self.points.iter_mut().enumerate() {
            if point.pinned || !is_flesh(&self.flesh, index) {
                continue;
            }
            // The face itself bruised what it touched; this is the ring
            // around it.
            let distance = sample_point_contact(point.position, shape).distance;
            if distance <= shape.influence || distance > reach {
                continue;
            }
            let falloff = 1.0 - (distance - shape.influence) / (reach - shape.influence);
            if apply_point_contusion(
                point,
                self.materials,
                load * falloff,
                strike.profile.contusion_scale,
            ) {
                self.stats.contusion_events += 1;
                self.debug.contusion_events += 1;
                self.debug.max_contusion = self.debug.max_contusion.max(point.contusion);
            }
        }
    }

    /// Pushes tissue out of the tool at one point of its motion and drags it
    /// along. Returns the momentum the tissue took. `blow` is how hard the
    /// tool hits at this speed. What happened to each point gathers in `hits`,
    /// so load and bruising are recorded once per point.
    #[allow(clippy::too_many_arguments)]
    fn shove_tissue(
        &mut self,
        shape: &ToolContactShape,
        velocity: Vec2,
        blow: f64,
        strike: ToolStrike,
        dt: f64,
        sub_dt: f64,
        hits: &mut [Option<PointHit>],
    ) -> Vec2 {
        let mut given = Vec2::default();
        // Momentum knocked into a point bruises it in step with the blow's
        // energy, so a tool pressed in slowly bruises far less than a blow.
        let bruising_share = hypot(velocity.x, velocity.y) / FIRM_SWING_SPEED;
        let base_strength = 0.58 * strike.profile.tissue_push_scale * (0.85 + strike.power * 0.15);
        for (index, point) in self.points.iter_mut().enumerate() {
            if point.pinned || !is_flesh(&self.flesh, index) {
                continue;
            }
            let point_contact = sample_point_contact(point.position, shape);
            if point_contact.distance > shape.influence {
                continue;
            }
            // The tool presses on skin and the muscle beneath it together;
            // pushing only the skin would shear it off the muscle.
            let strength = base_strength;
            let depth = shape.influence - point_contact.distance;
            let push = depth * strength * strike.profile.rebound_scale;
            let drag = sub_dt * 0.45 * strength * strike.profile.drag_scale;
            let moved = Vec2 {
                x: point_contact.normal.x * push + velocity.x * drag,
                y: point_contact.normal.y * push + velocity.y * drag,
            };
            point.position = add(point.position, moved);
            let momentum = scale(moved, point.mass / dt);
            given = add(given, momentum);
            let hit = hits[index].get_or_insert_with(PointHit::default);
            hit.depth += depth;
            hit.load = hit
                .load
                .max(blow * (hit.depth / shape.influence).min(1.0) * strength);
            hit.impulse += hypot(momentum.x, momentum.y) * bruising_share;
            if depth > self.debug.max_depth {
                self.debug.max_depth = depth;
                self.debug.strongest_contact = point.position;
            }
        }
        given
    }

    /// Pushes and loads bone the tool meets at one point of its motion, and
    /// breaks it if the load is too much. `impact` is the tool's momentum, so
    /// a tool merely resting on a bone does not load it. Returns the momentum
    /// the bones took.
    fn shove_bones(
        &mut self,
        shape: &ToolContactShape,
        velocity: Vec2,
        impact: f64,
        strike: ToolStrike,
        dt: f64,
        sub_dt: f64,
    ) -> Vec2 {
        let speed = hypot(velocity.x, velocity.y);
        let mut given = Vec2::default();
        let initial_bone_count = self.bones.len();
        for i in 0..initial_bone_count {
            let mut bone = self.bones[i];
            // The spine lies behind the trunk, out of reach of a blow to the
            // front of the body, and is never drawn.
            if bone.kind == BoneKind::Spine {
                continue;
            }
            let pair = closest_segment_points(shape.axis_start, shape.axis_end, bone.a, bone.b);
            let mut dist = pair.distance;
            if dist > shape.influence + bone.radius {
                continue;
            }
            let t = pair.t_b;
            let closest = pair.point_b;
            let mut normal = normalized(subtract(closest, pair.point_a), shape.direction);
            if dist < EPSILON && speed > EPSILON {
                normal = shape.direction;
                dist = 1.0;
            }
            let depth = (shape.influence + bone.radius - dist).max(0.0);
            let contact = (1.0 - ((dist - bone.radius) / shape.influence).clamp(0.0, 1.0)).max(0.0);
            let direct_load = impact * contact * strike.profile.bone_load_scale;
            // The bone solver fractures any bone whose load exceeds its strength,
            // so the tool's fracture resistance is applied to the load itself.
            bone.load = bone
                .load
                .max(direct_load / strike.profile.fracture_scale.max(EPSILON));
            bone.break_load = bone.break_load.max(
                direct_load / strike.profile.fracture_scale.max(EPSILON)
                    * strike.profile.break_scale,
            );
            if direct_load > self.materials.fragment_wake_load
                || speed > self.materials.fragment_sleep_speed * 2.0
            {
                self.wake_fragment(&mut bone);
            }
            self.debug.bone_contacts += 1;
            if depth > self.debug.max_depth {
                self.debug.max_depth = depth;
                self.debug.strongest_contact = closest;
            }
            self.debug.max_bone_load = self.debug.max_bone_load.max(bone.load);
            if bone.pinned {
                // A fixed bone stops whatever of the swing drives into it.
                let into = dot(velocity, normal).max(0.0);
                given = add(given, scale(normal, into * contact * strike.inertia));
            } else {
                let contact_strength = self.materials.bone_direct_contact
                    * strike.profile.bone_push_scale
                    * (0.78 + strike.power * 0.12);
                let push = Vec2 {
                    x: normal.x * depth * contact_strength * strike.profile.rebound_scale
                        + velocity.x * sub_dt * contact_strength * 0.58 * strike.profile.drag_scale,
                    y: normal.y * depth * contact_strength * strike.profile.rebound_scale
                        + velocity.y * sub_dt * contact_strength * 0.58 * strike.profile.drag_scale,
                };
                bone.a.x += push.x * (1.0 - t);
                bone.a.y += push.y * (1.0 - t);
                bone.b.x += push.x * t;
                bone.b.y += push.y * t;
                let mass = (bone.rest_length
                    * bone.radius
                    * bone.radius
                    * self.materials.tool_bone_mass_scale)
                    .max(1.0);
                given = add(given, scale(push, mass / dt));
                apply_bone_torque(
                    &self.materials,
                    &mut self.debug,
                    &mut bone,
                    closest,
                    Vec2 {
                        x: normal.x * direct_load * 0.16 * strike.profile.rebound_scale
                            + velocity.x
                                * contact
                                * strike.profile.bone_load_scale
                                * 0.22
                                * strike.profile.drag_scale,
                        y: normal.y * direct_load * 0.16 * strike.profile.rebound_scale
                            + velocity.y
                                * contact
                                * strike.profile.bone_load_scale
                                * 0.22
                                * strike.profile.drag_scale,
                    },
                );
            }
            let should_fracture =
                self.can_fracture_bone(bone) && bone.break_load > bone.fracture_impulse;
            self.bones[i] = bone;
            if should_fracture {
                self.fracture_bone(i, t, normal, direct_load);
            }
        }
        given
    }

    /// Tissue a blunt tool presses into pushes back on it, split by mass, so a
    /// tool held against the body sinks only as deep as the tissue gives way.
    pub(super) fn solve_tool_contact(&mut self) {
        let Some(contact) = &self.tool.contact else {
            return;
        };
        let shift = self.tool.solver_shift;
        let shape = ToolContactShape {
            axis_start: add(contact.shape.axis_start, shift),
            axis_end: add(contact.shape.axis_end, shift),
            ..contact.shape
        };
        let surface = shape.radius + 1.0;
        let tool_weight = 1.0 / contact.inertia.max(EPSILON);
        let mut pushed_back = Vec2::default();
        for &index in &contact.candidates {
            let point = &mut self.points[index];
            let point_contact = sample_point_contact(point.position, &shape);
            let depth = surface - point_contact.distance;
            if depth <= 0.0 {
                continue;
            }
            let correction = depth * TOOL_CONTACT_STIFFNESS;
            let point_weight = 1.0 / point.mass.max(EPSILON);
            let point_share = point_weight / (point_weight + tool_weight);
            point.position = add(
                point.position,
                scale(point_contact.normal, correction * point_share),
            );
            pushed_back = subtract(
                pushed_back,
                scale(point_contact.normal, correction * (1.0 - point_share)),
            );
        }
        self.tool.solver_shift = add(shift, pushed_back);
    }

    /// Applies the solver's push-back to the tool, partly as a rebound.
    pub(super) fn finish_tool_step(&mut self, dt: f64) {
        let shift = self.tool.solver_shift;
        if hypot(shift.x, shift.y) <= EPSILON {
            return;
        }
        self.tool.position = add(self.tool.position, shift);
        self.tool.velocity = add(
            self.tool.velocity,
            scale(shift, TOOL_REBOUND / dt.max(EPSILON)),
        );
    }

    /// A heavy head can split tissue it crushes against bone and itself;
    /// `blow` is how hard it hit.
    fn crush_tear_under_tool(&mut self, profile: ToolProfile, shape: &ToolContactShape, blow: f64) {
        let influence = shape.influence;
        let mut events = Vec::new();
        for spring_index in 0..self.springs.len() {
            let spring = self.springs[spring_index];
            if spring.broken || !self.spring_in_flesh(spring) {
                continue;
            }
            let a = self.points[spring.a];
            let b = self.points[spring.b];
            let center = midpoint(a.position, b.position);
            let tear_contact = sample_point_contact(center, shape);
            if tear_contact.distance > influence * 0.82 {
                continue;
            }
            let contact =
                1.0 - (tear_contact.distance / (influence * 0.82).max(1.0)).clamp(0.0, 1.0);
            let layer_scale = if spring.layer == TissueLayer::Skin {
                1.0
            } else {
                0.78 + a.exposure.max(b.exposure) * 0.42
            };
            let pressure = blow * profile.crush_tear_scale * contact * layer_scale;
            let threshold = spring.tear_impulse * self.materials.sharp_tool_tear_pressure;
            self.springs[spring_index].stress = self.springs[spring_index]
                .stress
                .max(pressure / threshold.max(1.0));
            if pressure <= threshold {
                continue;
            }
            self.break_spring(spring_index);
            let exposure = if spring.layer == TissueLayer::Skin {
                0.92
            } else {
                1.0
            };
            self.bump_point_exposure_load(spring.a, exposure, pressure * 0.18);
            self.bump_point_exposure_load(spring.b, exposure, pressure * 0.18);
            if spring.layer == TissueLayer::Skin {
                self.stats.broken_skin += 1;
            } else {
                self.stats.broken_muscle += 1;
            }
            let tangent = normalized(subtract(b.position, a.position), Vec2 { x: 1.0, y: 0.0 });
            events.push((
                center,
                Vec2 {
                    x: -tangent.y,
                    y: tangent.x,
                },
                spring.layer,
                pressure,
            ));
        }
        for (center, normal, layer, pressure) in events {
            let skin = layer == TissueLayer::Skin;
            self.emit_fluid(
                center,
                normal,
                if skin { 6 } else { 4 },
                120.0 + pressure * self.materials.fluid_impact_scale * 0.42,
                if skin { 2.1 } else { 1.8 },
                profile.fluid_scale,
            );
            self.open_wound(
                center,
                normal,
                layer,
                pressure / if skin { 1250.0 } else { 1050.0 },
                if skin { 2.1 } else { 1.8 },
                if skin { 0.58 } else { 0.92 },
            );
        }
    }
}

/// The perpendicular to `heading` on the same side as `previous`, so the
/// handle stays on one side as the tool turns. Given a handle direction, it
/// likewise gives the face across the handle nearest a direction.
fn side_toward(heading: Vec2, previous: Vec2) -> Vec2 {
    let perpendicular = Vec2 {
        x: -heading.y,
        y: heading.x,
    };
    if dot(perpendicular, previous) >= 0.0 {
        perpendicular
    } else {
        scale(perpendicular, -1.0)
    }
}
