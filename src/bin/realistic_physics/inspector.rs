//! The viewer: `realistic_physics --inspect` plays a scenario, a swing, or a
//! gesture once, then steps through it frame by frame: what the injury rules
//! decided on each frame and what the watchdogs flagged, the stretch,
//! bruising, and speed of the flesh drawn over the body, and, for a point or
//! a bone clicked, what it is, what happened to it, and (`W`) what moved it.
//! Native only; `tools\inspect.ps1` opens it, and `--shot` saves a frame with
//! its overlays instead, to look at a run with no one at the keyboard.
//!
//! ```text
//! --inspect                       open the viewer
//! --scenario NAME | --strike SPEC | --gesture SPEC
//!                                 what to play (by default bat_hard_forearm)
//! --variant ALONG,ACROSS,STRENGTH one run of the sweep
//! --frame N                       start on frame N
//! --overlays LIST                 start with these drawn: stretch, muscle,
//!                                 bruise, speed, events, bones
//! --select point:N | bone:N       start with this selected
//! --why                           with --select, start with what moved it
//! --shot OUT.png                  save the frame as shown and exit
//! ```
//!
//! Keys: Space plays and pauses; Left and Right step a frame, ten with Shift;
//! Home and End jump to the ends; 1 the skin's stretch (again: the
//! muscle's), 2 bruising, 3 speed, 4 what happened, 5 bones and joints; A the
//! see-through view; W what moved the selection over the last 20 frames; Esc
//! drops the selection. Click the body to select the point or bone under the
//! pointer, the timeline to jump there.

use std::env;
use std::path::PathBuf;
use std::process;

use rp::inspect::{
    actor_name, bone_name, joint_name, measure_text as measure_words, place_name, story,
    subject_name, subject_spec, BlameTarget, Trip, Watchdogs, Why,
};
use rp::scenarios::{scenario, Gesture, Play, Scenario, Strike};

use super::*;

/// How many frames apart the viewer keeps the world, to step back from.
const SNAPSHOT_EVERY: i32 = 10;
const TOP_BAR: f32 = 30.0;
const TIMELINE_HEIGHT: f32 = 46.0;
const PANEL_WIDTH: f32 = 410.0;
const TEXT_SIZE: f32 = 15.0;
const LINE: f32 = 17.0;
/// How long a frame shows while playing, in seconds: real time.
const PLAY_FRAME_SECONDS: f64 = 1.0 / 60.0;

#[derive(Clone, Copy, Default, PartialEq)]
enum Stretch {
    #[default]
    Off,
    Skin,
    Muscle,
}

#[derive(Clone, Copy, Default)]
struct Overlays {
    stretch: Stretch,
    bruise: bool,
    speed: bool,
    events: bool,
    bones: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Selection {
    Point(usize),
    Bone(usize),
}

impl Selection {
    fn target(self) -> BlameTarget {
        match self {
            Selection::Point(index) => BlameTarget::Point(index),
            Selection::Bone(index) => BlameTarget::Bone(index),
        }
    }
}

pub struct InspectRequest {
    scenario: Scenario,
    frame: Option<i32>,
    overlays: Overlays,
    select: Option<Selection>,
    why: bool,
    shot: Option<PathBuf>,
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    process::exit(2);
}

/// The viewer asked for on the command line, if any. Exits on bad options.
pub fn request() -> Option<InspectRequest> {
    if !env::args().any(|arg| arg == "--inspect") {
        return None;
    }
    let mut request = InspectRequest {
        scenario: scenario("bat_hard_forearm").expect("a tuned scenario"),
        frame: None,
        overlays: Overlays {
            events: true,
            ..Overlays::default()
        },
        select: None,
        why: false,
        shot: None,
    };
    let mut variant = None;
    let mut settings = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .unwrap_or_else(|| fail(&format!("{name} needs a value")))
        };
        match arg.as_str() {
            "--inspect" => {}
            "--scenario" => {
                let name = value("--scenario");
                request.scenario =
                    scenario(&name).unwrap_or_else(|| fail(&format!("unknown scenario `{name}`")));
            }
            "--strike" => {
                let strike = Strike::parse(&value("--strike")).unwrap_or_else(|error| fail(&error));
                request.scenario = Scenario::custom(Play::Swing(strike, None));
            }
            "--gesture" => {
                let gesture =
                    Gesture::parse(&value("--gesture")).unwrap_or_else(|error| fail(&error));
                request.scenario = Scenario::custom(Play::Gesture(gesture));
            }
            "--variant" => {
                let spec = value("--variant");
                let numbers: Vec<f64> = spec
                    .split(',')
                    .map(|part| part.trim().parse::<f64>())
                    .collect::<Result<_, _>>()
                    .unwrap_or_else(|_| {
                        fail(&format!("--variant takes three numbers, got `{spec}`"))
                    });
                let [along, across, strength] = numbers[..] else {
                    fail(&format!("--variant takes three numbers, got `{spec}`"));
                };
                variant = Some((along, across, strength));
            }
            "--frame" => {
                let text = value("--frame");
                request.frame = Some(
                    text.parse()
                        .unwrap_or_else(|_| fail(&format!("`{text}` is not a frame"))),
                );
            }
            "--overlays" => {
                request.overlays = Overlays::default();
                for name in value("--overlays").split(',') {
                    match name.trim() {
                        "stretch" | "skin" => request.overlays.stretch = Stretch::Skin,
                        "muscle" => request.overlays.stretch = Stretch::Muscle,
                        "bruise" | "bruises" => request.overlays.bruise = true,
                        "speed" => request.overlays.speed = true,
                        "events" => request.overlays.events = true,
                        "bones" => request.overlays.bones = true,
                        "" | "none" => {}
                        other => fail(&format!("unknown overlay `{other}`")),
                    }
                }
            }
            "--select" => {
                let spec = value("--select");
                request.select = Some(match BlameTarget::parse(&spec) {
                    Ok(BlameTarget::Point(index)) => Selection::Point(index),
                    Ok(BlameTarget::Bone(index)) => Selection::Bone(index),
                    _ => fail(&format!("--select takes point:N or bone:N, got `{spec}`")),
                });
            }
            "--why" => request.why = true,
            "--shot" => request.shot = Some(PathBuf::from(value("--shot"))),
            "--set" => settings.push(value("--set")),
            other => fail(&format!("unknown option `{other}`")),
        }
    }
    // Settings changed from the defaults, as the scenario tool takes them,
    // so a change tried there can be looked at here. The browser takes no
    // command line, so its build leaves the settings parser out.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let settings = rp::parse_settings(&settings).unwrap_or_else(|error| fail(&error));
        rp::scenarios::use_settings(settings);
    }
    #[cfg(target_arch = "wasm32")]
    let _ = settings;
    if let Some((along, across, strength)) = variant {
        request.scenario = request
            .scenario
            .shifted(along, across)
            .with_bone_strength(strength);
    }
    Some(request)
}

/// What the app needs to draw a frame and play on from it.
struct Moment {
    world: rp::World,
    tool: rp::ToolMode,
    pointer: rp::Vec2,
    pointer_down: bool,
    tool_blood: f32,
    seen_fluid: i32,
}

impl Moment {
    fn of(app: &AppState) -> Self {
        Self {
            world: app.world.clone(),
            tool: app.tool,
            pointer: app.pointer,
            pointer_down: app.pointer_down,
            tool_blood: app.tool_blood,
            seen_fluid: app.seen_fluid,
        }
    }

    fn restore(&self, app: &mut AppState) {
        app.world = self.world.clone();
        app.tool = self.tool;
        app.pointer = self.pointer;
        app.pointer_down = self.pointer_down;
        app.tool_blood = self.tool_blood;
        app.seen_fluid = self.seen_fluid;
        app.skin_rim = skin_rim(&app.world);
    }
}

struct Viewer {
    app: AppState,
    scenario: Scenario,
    frames: i32,
    /// The world after every `SNAPSHOT_EVERY` steps, from the first.
    moments: Vec<Moment>,
    /// The frame shown: the world after this step (from 0).
    frame: i32,
    /// The run as it ended, whose journal and watchdogs tell about it.
    ended: rp::World,
    trips: Vec<Trip>,
    /// The story's lines for each frame, and how many injuries each saw.
    story: Vec<Vec<String>>,
    injuries: Vec<usize>,
    overlays: Overlays,
    selection: Option<Selection>,
    why: Vec<String>,
    playing: bool,
    play_clock: f64,
    /// How many lines the side panel is scrolled down.
    panel_scroll: usize,
}

/// Plays `frame` of `scenario` on the app, as the scenarios play it.
fn play_frame(app: &mut AppState, scenario: &Scenario, frame: i32) {
    let dt = app.world.materials().fixed_dt;
    let input = scenario.input(frame, dt, rp::world_frame());
    if input.active {
        app.tool = input.tool;
        app.pointer = app.view.to_screen(rp::Vec2 {
            x: input.x,
            y: input.y,
        });
    }
    app.pointer_down = input.down;
    step_world(app, &input);
}

impl Viewer {
    /// Plays the whole of `scenario` once, keeping what stepping through it
    /// needs.
    fn record(scenario: Scenario) -> Self {
        let mut app = AppState::new(screen_width() as f64, screen_height() as f64);
        app.hide_ui = true;
        // A lone body is drawn as the app draws one body alone.
        if scenario.fight().is_none() {
            app.bodies = Bodies::One;
            app.view_for = None;
            app.fit_view(screen_width(), screen_height());
        }
        // A fight scenario plays on its two fighters, as the scenarios do.
        app.world = scenario.ready(rp::create_layered_body_in(
            rp::world_frame(),
            scenario.materials(),
        ));
        app.world.keep_journal();
        app.skin_rim = skin_rim(&app.world);
        let mut watchdogs = Watchdogs::new(&app.world);
        let frames = scenario.frames().max(1);
        let mut moments = Vec::new();
        for step in 0..frames {
            if step % SNAPSHOT_EVERY == 0 {
                moments.push(Moment::of(&app));
            }
            play_frame(&mut app, &scenario, step);
            watchdogs.check(step, &app.world);
        }
        let ended = app.world.clone();
        let mut story_of = vec![Vec::new(); frames as usize];
        let mut injuries = vec![0; frames as usize];
        if let Some(journal) = ended.journal() {
            let mut current: Option<usize> = None;
            for line in story(&ended, journal) {
                if let Some(rest) = line.strip_prefix("frame ") {
                    current = rest
                        .split(':')
                        .next()
                        .and_then(|number| number.parse::<usize>().ok());
                }
                if let Some(lines) = current.and_then(|frame| story_of.get_mut(frame)) {
                    lines.push(line);
                }
            }
            for event in journal.events.iter().filter(|event| !event.kind.is_minor()) {
                if let Some(count) = injuries.get_mut(event.step as usize) {
                    *count += 1;
                }
            }
        }
        let mut viewer = Self {
            app,
            scenario,
            frames,
            moments,
            frame: frames - 1,
            ended,
            trips: watchdogs.trips().to_vec(),
            story: story_of,
            injuries,
            overlays: Overlays::default(),
            selection: None,
            why: Vec::new(),
            playing: false,
            play_clock: 0.0,
            panel_scroll: 0,
        };
        viewer.show(0);
        viewer
    }

    /// Shows the world after step `frame`.
    fn show(&mut self, frame: i32) {
        let frame = frame.clamp(0, self.frames - 1);
        // From the kept world nearest before it, unless the one shown is
        // nearer and not past it.
        let kept = ((frame + 1) / SNAPSHOT_EVERY).min(self.moments.len() as i32 - 1);
        let from_kept = kept * SNAPSHOT_EVERY;
        let mut next = if self.frame < frame && self.frame + 1 >= from_kept {
            self.frame + 1
        } else {
            self.moments[kept as usize].restore(&mut self.app);
            from_kept
        };
        while next <= frame {
            play_frame(&mut self.app, &self.scenario, next);
            next += 1;
        }
        self.frame = frame;
    }

    fn select_at(&mut self, at: rp::Vec2) {
        let world = &self.app.world;
        let bone = world
            .bones()
            .iter()
            .enumerate()
            .map(|(index, bone)| (distance_to_bone(at, bone) - bone.radius, index))
            .min_by(|p, q| p.0.total_cmp(&q.0));
        let point = world
            .points()
            .iter()
            .enumerate()
            .filter(|(_, point)| !point.is_ghost())
            .map(|(index, point)| (length(sub(point.position, at)), index))
            .min_by(|p, q| p.0.total_cmp(&q.0));
        self.selection = match (bone, point) {
            (Some((gap, index)), _) if gap <= 4.0 => Some(Selection::Bone(index)),
            (_, Some((gap, index))) if gap <= 14.0 => Some(Selection::Point(index)),
            _ => None,
        };
        self.why.clear();
    }

    fn explain(&mut self) {
        if let Some(selection) = self.selection {
            self.why = rp::inspect::explain(
                &self.scenario,
                Why {
                    target: selection.target(),
                    from: (self.frame - 20).max(0),
                    to: self.frame,
                },
            );
        }
    }
}

fn distance_to_bone(at: rp::Vec2, bone: &rp::BoneSegment) -> f64 {
    let along = sub(bone.b, bone.a);
    let length_sq = (along.x * along.x + along.y * along.y).max(1.0e-9);
    let t =
        (((at.x - bone.a.x) * along.x + (at.y - bone.a.y) * along.y) / length_sq).clamp(0.0, 1.0);
    length(sub(at, add(bone.a, scale(along, t))))
}

/// Runs the viewer, or saves the frame asked for with `--shot`.
pub async fn run(request: InspectRequest) {
    for _ in 0..4 {
        clear_background(BLACK);
        next_frame().await;
    }
    let mut viewer = Viewer::record(request.scenario);
    viewer.overlays = request.overlays;
    viewer.selection = request.select;
    viewer.show(request.frame.unwrap_or(0));
    if request.why {
        viewer.explain();
    }
    if let Some(path) = request.shot {
        // Draw twice, so text drawn for the first time is whole.
        draw_viewer(&viewer);
        next_frame().await;
        draw_viewer(&viewer);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                fail(&format!("cannot create {}: {error}", parent.display()))
            });
        }
        get_screen_data().export_png(&path.to_string_lossy());
        println!("wrote {}", path.display());
        return;
    }
    loop {
        handle_keys(&mut viewer);
        // A resized window draws the run larger or smaller, as the app does.
        viewer.app.fit_view(screen_width(), screen_height());
        if viewer.playing {
            viewer.play_clock += get_frame_time() as f64;
            while viewer.play_clock >= PLAY_FRAME_SECONDS && viewer.playing {
                viewer.play_clock -= PLAY_FRAME_SECONDS;
                if viewer.frame + 1 >= viewer.frames {
                    viewer.playing = false;
                } else {
                    let next = viewer.frame + 1;
                    viewer.show(next);
                }
            }
        }
        draw_viewer(&viewer);
        next_frame().await;
    }
}

fn handle_keys(viewer: &mut Viewer) {
    let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
    let stride = if shift { 10 } else { 1 };
    let mut go = None;
    if is_key_pressed(KeyCode::Right) {
        go = Some(viewer.frame + stride);
    }
    if is_key_pressed(KeyCode::Left) {
        go = Some(viewer.frame - stride);
    }
    if is_key_pressed(KeyCode::Home) {
        go = Some(0);
    }
    if is_key_pressed(KeyCode::End) {
        go = Some(viewer.frames - 1);
    }
    if is_key_pressed(KeyCode::Space) {
        if viewer.frame + 1 >= viewer.frames {
            go = Some(0);
        }
        viewer.playing = !viewer.playing;
        viewer.play_clock = 0.0;
    }
    let overlays = &mut viewer.overlays;
    if is_key_pressed(KeyCode::Key1) {
        overlays.stretch = match overlays.stretch {
            Stretch::Off => Stretch::Skin,
            Stretch::Skin => Stretch::Muscle,
            Stretch::Muscle => Stretch::Off,
        };
    }
    if is_key_pressed(KeyCode::Key2) {
        overlays.bruise = !overlays.bruise;
    }
    if is_key_pressed(KeyCode::Key3) {
        overlays.speed = !overlays.speed;
    }
    if is_key_pressed(KeyCode::Key4) {
        overlays.events = !overlays.events;
    }
    if is_key_pressed(KeyCode::Key5) {
        overlays.bones = !overlays.bones;
    }
    if is_key_pressed(KeyCode::A) {
        viewer.app.view_mode = match viewer.app.view_mode {
            ViewMode::Normal => ViewMode::Anatomy,
            ViewMode::Anatomy => ViewMode::Normal,
        };
    }
    if is_key_pressed(KeyCode::Escape) {
        viewer.selection = None;
        viewer.why.clear();
        viewer.panel_scroll = 0;
    }
    if is_key_pressed(KeyCode::W) {
        viewer.explain();
        viewer.panel_scroll = 0;
    }
    let (_, wheel) = mouse_wheel();
    if wheel != 0.0 && mouse_position().0 >= screen_width() - PANEL_WIDTH {
        viewer.panel_scroll = if wheel < 0.0 {
            viewer.panel_scroll + 3
        } else {
            viewer.panel_scroll.saturating_sub(3)
        };
    }
    if is_mouse_button_pressed(MouseButton::Left) {
        let (x, y) = mouse_position();
        let timeline_top = screen_height() - TIMELINE_HEIGHT;
        if y >= timeline_top {
            let share = ((x - 12.0) / (screen_width() - 24.0)).clamp(0.0, 1.0);
            go = Some((share * (viewer.frames - 1) as f32).round() as i32);
        } else if x < screen_width() - PANEL_WIDTH {
            let at = viewer.app.view.to_world(rp::Vec2 {
                x: x as f64,
                y: y as f64,
            });
            viewer.select_at(at);
            viewer.panel_scroll = 0;
        }
    }
    if let Some(frame) = go {
        viewer.playing = viewer.playing && frame < viewer.frames - 1;
        viewer.show(frame);
    }
}

fn text(line: &str, x: f32, y: f32, color: Color) {
    draw_text(line, x, y, TEXT_SIZE * 1.1, color);
}

fn panel(x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x, y, w, h, rgba(13, 13, 15, 222));
    draw_rectangle_lines(x, y, w, h, 1.0, rgba(118, 97, 83, 170));
}

/// Lines of `text` cut to fit `width` pixels, at word breaks.
fn wrapped(line: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let indent: String = line.chars().take_while(|c| *c == ' ').collect();
    let mut current = String::new();
    for word in line.split_whitespace() {
        let candidate = if current.is_empty() {
            format!("{indent}{word}")
        } else {
            format!("{current} {word}")
        };
        if measure_text(&candidate, None, (TEXT_SIZE * 1.1) as u16, 1.0).width > width
            && !current.is_empty()
        {
            lines.push(current);
            current = format!("{indent}    {word}");
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn draw_viewer(viewer: &Viewer) {
    draw_app(&viewer.app);
    // The overlays lie on the world, drawn as the app draws it.
    set_camera(&WorldCamera::of(viewer.app.view));
    let overlays = viewer.overlays;
    if overlays.stretch != Stretch::Off {
        draw_stretch(&viewer.app.world, overlays.stretch);
    }
    if overlays.bruise {
        draw_bruising(&viewer.app.world);
    }
    if overlays.speed {
        draw_speeds(&viewer.app.world);
    }
    if overlays.bones {
        draw_skeleton(&viewer.app.world);
    }
    if overlays.events {
        draw_happenings(viewer);
    }
    draw_trips(viewer);
    if let Some(selection) = viewer.selection {
        draw_selected(&viewer.app.world, selection);
    }
    set_default_camera();
    draw_top_bar(viewer);
    draw_side_panel(viewer);
    draw_timeline(viewer);
}

/// Springs coloured by how far the body has them stretched past where it
/// holds them at rest: blue squeezed, pale at rest, yellow to red stretched,
/// white past where they tear.
fn draw_stretch(world: &rp::World, which: Stretch) {
    let layer = match which {
        Stretch::Muscle => rp::TissueLayer::Muscle,
        _ => rp::TissueLayer::Skin,
    };
    let points = world.points();
    for (spring, wear) in world.springs().iter().zip(world.spring_wear()) {
        if spring.broken || spring.layer != layer {
            continue;
        }
        let (a, b) = (points[spring.a].position, points[spring.b].position);
        let stretch = length(sub(b, a)) / spring.rest.max(1.0e-6) / wear.prestretch.max(1.0e-6);
        let color = if stretch < 1.0 {
            mix(
                rgba(150, 150, 150, 70),
                rgba(60, 140, 255, 230),
                ((1.0 - stretch) / 0.15).clamp(0.0, 1.0) as f32,
            )
        } else if stretch < wear.tear_stretch.max(1.01) {
            let toward =
                ((stretch - 1.0) / (wear.tear_stretch - 1.0).max(0.01)).clamp(0.0, 1.0) as f32;
            if toward < 0.5 {
                mix(
                    rgba(150, 150, 150, 70),
                    rgba(255, 220, 60, 230),
                    toward * 2.0,
                )
            } else {
                mix(
                    rgba(255, 220, 60, 230),
                    rgba(255, 40, 30, 245),
                    (toward - 0.5) * 2.0,
                )
            }
        } else {
            rgba(255, 255, 255, 255)
        };
        draw_line_vec(a, b, 1.6, color);
    }
}

/// Points of flesh shaded by their bruising, and failed flesh outlined.
fn draw_bruising(world: &rp::World) {
    for point in world.points() {
        if point.is_ghost() || point.contusion < 0.02 {
            continue;
        }
        let strength = (point.contusion / 1.35).clamp(0.0, 1.0) as f32;
        let at = to_mq(point.position);
        draw_circle(
            at.x,
            at.y,
            2.5 + 3.0 * strength,
            rgba(170, 60, 230, (80.0 + 170.0 * strength) as u8),
        );
    }
    let points = world.points();
    for triangle in world.triangles() {
        if !triangle.failed {
            continue;
        }
        let [a, b, c] = [triangle.a, triangle.b, triangle.c].map(|i| points[i].position);
        for (p, q) in [(a, b), (b, c), (c, a)] {
            draw_line_vec(p, q, 1.0, rgba(20, 20, 20, 200));
        }
    }
}

/// Arrows for what moves faster than 40 px/s, a twentieth of a second long.
fn draw_speeds(world: &rp::World) {
    let dt = world.materials().fixed_dt;
    for point in world.points() {
        let velocity = scale(sub(point.position, point.previous), 1.0 / dt);
        let speed = length(velocity) as f32;
        if speed < 40.0 {
            continue;
        }
        let color = mix(
            rgba(80, 220, 255, 160),
            rgba(255, 60, 200, 240),
            (speed / 1500.0).min(1.0),
        );
        draw_line_vec(
            point.position,
            add(point.position, scale(velocity, 0.05)),
            1.2,
            color,
        );
    }
    for bone in world.bones() {
        let middle = scale(add(bone.a, bone.b), 0.5);
        let before = scale(add(bone.previous_a, bone.previous_b), 0.5);
        let velocity = scale(sub(middle, before), 1.0 / dt);
        if length(velocity) >= 40.0 {
            draw_line_vec(
                middle,
                add(middle, scale(velocity, 0.05)),
                2.4,
                rgba(255, 255, 120, 230),
            );
        }
    }
}

/// Every bone's axis with its number, and every joint with how far it is
/// turned off the angle it was built at.
fn draw_skeleton(world: &rp::World) {
    for (index, bone) in world.bones().iter().enumerate() {
        let color = if bone.fractured || bone.splinter {
            rgba(255, 150, 60, 240)
        } else {
            rgba(120, 230, 160, 220)
        };
        draw_line_vec(bone.a, bone.b, 2.0, color);
        let middle = to_mq(scale(add(bone.a, bone.b), 0.5));
        draw_text(
            &index.to_string(),
            middle.x + 3.0,
            middle.y - 3.0,
            14.0,
            color,
        );
    }
    for (index, joint) in world.bone_joints().iter().enumerate() {
        let (Some(a), Some(b)) = (world.bones().get(joint.a), world.bones().get(joint.b)) else {
            continue;
        };
        let at = to_mq(bone_point(*a, joint.t_a));
        let other = to_mq(bone_point(*b, joint.t_b));
        let color = if joint.broken || joint.post_fracture_limited {
            rgba(255, 90, 60, 230)
        } else if joint.subluxated {
            rgba(255, 200, 60, 230)
        } else {
            rgba(200, 200, 255, 200)
        };
        draw_circle_lines(at.x, at.y, 3.5, 1.2, color);
        draw_line(at.x, at.y, other.x, other.y, 1.0, color);
        if let Some(turn) = world.joint_turn(index) {
            if turn.abs() > 0.035 {
                draw_text(
                    &format!("{:+.0}", turn.to_degrees()),
                    at.x + 5.0,
                    at.y + 12.0,
                    13.0,
                    color,
                );
            }
        }
    }
}

/// How many frames back what happened stays drawn, fading.
const HAPPENINGS_SHOWN: i32 = 3;

/// What happened on this frame, and fading, the two just before: a ring
/// where each injury was decided, each kind labelled once nearby.
fn draw_happenings(viewer: &Viewer) {
    let Some(journal) = viewer.ended.journal() else {
        return;
    };
    let mut labelled: Vec<(Vec2, &str)> = Vec::new();
    for event in journal.events.iter().filter(|event| {
        let age = viewer.frame - event.step as i32;
        (0..HAPPENINGS_SHOWN).contains(&age) && !event.kind.is_minor()
    }) {
        if event.at == rp::Vec2::default() {
            continue;
        }
        let age = (viewer.frame - event.step as i32) as f32;
        let color = with_alpha(event_color(event.kind), 1.0 - age / HAPPENINGS_SHOWN as f32);
        let at = to_mq(event.at);
        draw_circle_lines(at.x, at.y, 3.5, 1.2, color);
        let label = event.kind.describe();
        if age < 1.0
            && labelled.len() < 8
            && !labelled
                .iter()
                .any(|(other, name)| *name == label && (*other - at).length() < 60.0)
        {
            draw_text(label, at.x + 6.0, at.y - 5.0, 14.0, color);
            labelled.push((at, label));
        }
    }
}

fn event_color(kind: rp::EventKind) -> Color {
    use rp::EventKind::*;
    match kind {
        BoneBroken | BoneBrokenOpen | ChipBrokenOff | WedgeBrokenOff => rgba(255, 245, 200, 255),
        JointTorn | JointSlipped | LimbWentLimp => rgba(255, 200, 60, 255),
        ArteryCut | WoundOpened | WoundReopened | OrganRuptured | CavityRuptured => {
            rgba(255, 60, 60, 255)
        }
        SkinTorn | SkinCut | SkinCrushed | TearRanOn | CutOpened | SkinPiercedByBone => {
            rgba(255, 140, 120, 255)
        }
        _ => rgba(200, 160, 255, 240),
    }
}

/// Where the watchdogs flagged something on this frame: a red square.
fn draw_trips(viewer: &Viewer) {
    for trip in viewer
        .trips
        .iter()
        .filter(|trip| (trip.first_frame..=trip.last_frame).contains(&viewer.frame))
    {
        if trip.at == rp::Vec2::default() {
            continue;
        }
        let at = to_mq(trip.at);
        draw_rectangle_lines(
            at.x - 6.0,
            at.y - 6.0,
            12.0,
            12.0,
            2.0,
            rgba(255, 30, 30, 240),
        );
    }
}

fn draw_selected(world: &rp::World, selection: Selection) {
    let color = rgba(80, 255, 255, 255);
    match selection {
        Selection::Point(index) => {
            if let Some(point) = world.points().get(index) {
                let at = to_mq(point.position);
                draw_circle_lines(at.x, at.y, 7.0, 2.0, color);
                if let Some(place) = world.posture_target(index) {
                    let place = to_mq(place);
                    draw_line(at.x, at.y, place.x, place.y, 1.0, with_alpha(color, 0.6));
                    draw_circle(place.x, place.y, 2.0, with_alpha(color, 0.6));
                }
            }
        }
        Selection::Bone(index) => {
            if let Some(bone) = world.bones().get(index) {
                draw_line_vec(
                    bone.a,
                    bone.b,
                    (bone.radius * 2.0 + 4.0) as f32,
                    with_alpha(color, 0.35),
                );
                draw_line_vec(bone.home_a, bone.home_b, 1.0, with_alpha(color, 0.5));
            }
        }
    }
}

fn draw_top_bar(viewer: &Viewer) {
    let width = screen_width();
    draw_rectangle(0.0, 0.0, width, TOP_BAR, rgba(13, 13, 15, 230));
    // What the hand did on this frame, as the journal noted it.
    let note = viewer
        .ended
        .journal()
        .and_then(|journal| journal.steps.get(viewer.frame as usize).copied())
        .unwrap_or_default();
    let mm = viewer.app.world.mm_per_px();
    let overlays = viewer.overlays;
    let on = |flag: bool, name: &str| {
        if flag {
            format!("[{name}]")
        } else {
            name.to_string()
        }
    };
    let stretch = match overlays.stretch {
        Stretch::Off => "1 stretch".to_string(),
        Stretch::Skin => "[1 skin stretch]".to_string(),
        Stretch::Muscle => "[1 muscle stretch]".to_string(),
    };
    let line = format!(
        "{}  frame {}/{}  {} {}, hand at {:.1} m/s   {}  {}  {}  {}  {}   a view  w why  space play",
        viewer.scenario.name,
        viewer.frame,
        viewer.frames - 1,
        rp::inspect::tool_name(note.tool),
        if note.down { "pressed" } else { "carried" },
        note.hand_speed * mm * 0.001,
        stretch,
        on(overlays.bruise, "2 bruise"),
        on(overlays.speed, "3 speed"),
        on(overlays.events, "4 events"),
        on(overlays.bones, "5 bones"),
    );
    text(&line, 10.0, 20.0, rgba(232, 226, 212, 245));
}

/// What happened on this frame, what the watchdogs flag on it, and the
/// selection.
fn draw_side_panel(viewer: &Viewer) {
    let x = screen_width() - PANEL_WIDTH;
    let top = TOP_BAR + 6.0;
    let bottom = screen_height() - TIMELINE_HEIGHT - 6.0;
    panel(x, top, PANEL_WIDTH - 6.0, bottom - top);
    let width = PANEL_WIDTH - 22.0;
    let mut lines: Vec<(String, Color)> = Vec::new();
    let add = |lines: &mut Vec<(String, Color)>, line: &str, color: Color| {
        for part in wrapped(line, width) {
            lines.push((part, color));
        }
    };
    let heading = rgba(255, 210, 140, 255);
    let plain = rgba(225, 220, 208, 240);
    let quiet = rgba(160, 152, 140, 230);
    let story = viewer
        .story
        .get(viewer.frame as usize)
        .cloned()
        .unwrap_or_default();
    match story.first() {
        Some(first) => add(&mut lines, first, heading),
        None => add(
            &mut lines,
            &format!("frame {}: nothing happened", viewer.frame),
            quiet,
        ),
    }
    // The selection and what moved it come first, the frame's injuries
    // after them in what room is left.
    if let Some(selection) = viewer.selection {
        for line in selection_lines(viewer, selection) {
            add(&mut lines, &line, rgba(150, 245, 245, 245));
        }
        for line in &viewer.why {
            add(&mut lines, line, rgba(200, 240, 200, 240));
        }
        lines.push((String::new(), plain));
    }
    let flagged: Vec<&Trip> = viewer
        .trips
        .iter()
        .filter(|trip| (trip.first_frame..=trip.last_frame).contains(&viewer.frame))
        .collect();
    if !flagged.is_empty() {
        add(&mut lines, "watchdogs flag now:", rgba(255, 90, 80, 255));
        for trip in flagged.iter().take(6) {
            let line = format!(
                "  {}: {} ({}), {}",
                trip.watchdog.describe(),
                subject_spec(trip.subject),
                subject_name(&viewer.ended, trip.subject),
                trip.watchdog.measure(trip.worst, trip.limit)
            );
            add(&mut lines, &line, rgba(255, 150, 140, 240));
        }
        lines.push((String::new(), plain));
    }
    for line in story.iter().skip(1) {
        add(&mut lines, line, plain);
    }
    // The frame's heading stays; the wheel scrolls the rest.
    let mut y = top + 18.0;
    let skipped = viewer.panel_scroll.min(lines.len().saturating_sub(2));
    let shown = lines.iter().take(1).chain(lines.iter().skip(1 + skipped));
    for (line, color) in shown {
        if y > bottom - 4.0 {
            break;
        }
        text(line, x + 8.0, y, *color);
        y += LINE;
    }
}

/// What the selection is, how it moves, and what happened to it so far.
fn selection_lines(viewer: &Viewer, selection: Selection) -> Vec<String> {
    let world = &viewer.app.world;
    let dt = world.materials().fixed_dt;
    let mm = world.mm_per_px();
    let mut lines = Vec::new();
    match selection {
        Selection::Point(index) => {
            let Some(point) = world.points().get(index) else {
                return lines;
            };
            let speed = length(sub(point.position, point.previous)) / dt;
            lines.push(format!(
                "point {index}: {} at {}{}{}",
                match point.layer {
                    rp::TissueLayer::Skin => "skin",
                    rp::TissueLayer::Muscle => "muscle",
                },
                place_name(world, point.position),
                if point.on_cut { ", on a tear" } else { "" },
                if point.pinned { ", pinned" } else { "" },
            ));
            lines.push(format!(
                "  moving {:.0} px/s ({:.2} m/s), load {:.0}, bruise {:.2}, exposed {:.2}",
                speed,
                speed * mm * 0.001,
                point.load,
                point.contusion,
                point.exposure
            ));
            if let Some(place) = world.posture_target(index) {
                lines.push(format!(
                    "  {:.1} px off its place on the bones",
                    length(sub(point.position, place))
                ));
            }
        }
        Selection::Bone(index) => {
            let Some(bone) = world.bones().get(index) else {
                return lines;
            };
            let middle = scale(add(bone.a, bone.b), 0.5);
            let before = scale(add(bone.previous_a, bone.previous_b), 0.5);
            let speed = length(sub(middle, before)) / dt;
            lines.push(format!(
                "bone {index}: {}{}",
                bone_name(world, index),
                if bone.open_break {
                    ", from an open break"
                } else {
                    ""
                }
            ));
            lines.push(format!(
                "  moving {:.0} px/s ({:.2} m/s), {:.1} px from where it was built",
                speed,
                speed * mm * 0.001,
                length(sub(middle, scale(add(bone.home_a, bone.home_b), 0.5)))
            ));
            lines.push(format!(
                "  load {:.2} of its strength, by {}",
                bone.break_load / bone.fracture_impulse.max(1.0e-6),
                {
                    let by = actor_name(world, bone.break_by, world.debug().tool);
                    if by.is_empty() {
                        "its own load".to_string()
                    } else {
                        by
                    }
                }
            ));
            for (joint, _) in world
                .bone_joints()
                .iter()
                .enumerate()
                .filter(|(_, joint)| joint.a == index || joint.b == index)
            {
                lines.push(format!(
                    "  joint {joint}: {}, turned {:+.0} degrees",
                    joint_name(world, joint),
                    world.joint_turn(joint).unwrap_or(0.0).to_degrees()
                ));
            }
        }
    }
    // While the why shows, it says what moved the selection; its history
    // waits.
    let Some(journal) = viewer.ended.journal().filter(|_| viewer.why.is_empty()) else {
        return lines;
    };
    let mine: Vec<&rp::Event> = journal
        .events
        .iter()
        .filter(|event| event.step as i32 <= viewer.frame && concerns(world, event, selection))
        .collect();
    if !mine.is_empty() {
        lines.push(format!(
            "  what happened to it ({} in all), latest:",
            mine.len()
        ));
        // Alike events on a frame told once, with how many and the worst
        // measure.
        let mut told: Vec<(String, usize, String)> = Vec::new();
        for event in mine.iter().rev() {
            let tool = journal
                .steps
                .get(event.step as usize)
                .map(|note| note.tool)
                .unwrap_or_default();
            let by = actor_name(&viewer.ended, event.by, tool);
            let line = format!(
                "    frame {}: {}{} [{}]",
                event.step,
                event.kind.describe(),
                if by.is_empty() {
                    String::new()
                } else {
                    format!(", by {by}")
                },
                event.phase
            );
            let measure = measure_words(event.measure, event.value, event.limit);
            match told.iter().position(|(seen, _, _)| *seen == line) {
                Some(index) => told[index].1 += 1,
                None if told.len() < 6 => told.push((line, 1, measure)),
                None => {}
            }
        }
        for (line, count, measure) in told {
            let measure = if measure.is_empty() {
                String::new()
            } else {
                format!(", {measure}")
            };
            lines.push(if count > 1 {
                format!("{line} x{count}{measure}")
            } else {
                format!("{line}{measure}")
            });
        }
    }
    if viewer.why.is_empty() {
        lines.push("  W: what moved it over the last 20 frames".to_string());
    }
    lines
}

/// Whether `event` happened to the selection, or was done by it.
fn concerns(world: &rp::World, event: &rp::Event, selection: Selection) -> bool {
    match selection {
        Selection::Point(index) => match event.subject {
            rp::Subject::Point(point) => point == index,
            rp::Subject::Spring(spring) => world
                .springs()
                .get(spring)
                .is_some_and(|spring| spring.a == index || spring.b == index),
            rp::Subject::SkinTie(tie) => world
                .attachments()
                .get(tie)
                .is_some_and(|tie| tie.skin_point == index || tie.muscle_point == index),
            rp::Subject::BoneTie(tie) => world
                .bone_attachments()
                .get(tie)
                .is_some_and(|tie| tie.point == index),
            rp::Subject::Triangle(triangle) => world
                .triangles()
                .get(triangle)
                .is_some_and(|t| t.a == index || t.b == index || t.c == index),
            _ => false,
        },
        Selection::Bone(index) => {
            event.subject == rp::Subject::Bone(index) || event.by == rp::Actor::Bone(index)
        }
    }
}

/// The frames along the bottom: a tick for each frame something was hurt,
/// taller the more, red where a watchdog first flagged something, and where
/// the frame shown is.
fn draw_timeline(viewer: &Viewer) {
    let (width, height) = (screen_width(), screen_height());
    let top = height - TIMELINE_HEIGHT;
    draw_rectangle(0.0, top, width, TIMELINE_HEIGHT, rgba(13, 13, 15, 235));
    let (left, right) = (12.0, width - 12.0);
    let span = (right - left).max(1.0);
    let x_of = |frame: i32| left + span * frame as f32 / (viewer.frames - 1).max(1) as f32;
    draw_line(
        left,
        top + 30.0,
        right,
        top + 30.0,
        1.0,
        rgba(118, 97, 83, 200),
    );
    let most = viewer.injuries.iter().copied().max().unwrap_or(1).max(1) as f32;
    for (frame, &count) in viewer.injuries.iter().enumerate() {
        if count == 0 {
            continue;
        }
        let x = x_of(frame as i32);
        let tall = 6.0 + 18.0 * (count as f32 / most).sqrt();
        draw_line(
            x,
            top + 30.0,
            x,
            top + 30.0 - tall,
            2.0,
            rgba(255, 200, 120, 230),
        );
    }
    for trip in &viewer.trips {
        let x = x_of(trip.first_frame);
        draw_rectangle(x - 2.0, top + 33.0, 4.0, 8.0, rgba(255, 40, 40, 240));
    }
    let x = x_of(viewer.frame);
    draw_line(
        x,
        top + 2.0,
        x,
        top + TIMELINE_HEIGHT - 2.0,
        2.0,
        rgba(80, 255, 255, 255),
    );
    draw_text(
        &format!("{}", viewer.frame),
        (x + 4.0).min(right - 30.0),
        top + 12.0,
        14.0,
        rgba(80, 255, 255, 255),
    );
}
