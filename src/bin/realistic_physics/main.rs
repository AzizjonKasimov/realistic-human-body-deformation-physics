use macroquad::prelude::*;
use realistic_physics as rp;
use rp::draw::{rgba, with_alpha, Palette, SkinRim, WorldCamera};
use rp::scenarios::SCENARIO_WIDTH;

mod about;
mod analytics;
#[cfg(not(target_arch = "wasm32"))]
mod capture;
#[cfg(not(target_arch = "wasm32"))]
mod inspector;
mod selftest;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Normal,
    /// See-through skin with the muscle and bones showing. Players only get
    /// the normal view; screenshots use this one to check the anatomy.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Anatomy,
}

/// App commands shared by keyboard shortcuts and the on-screen control buttons.
/// Who stands in the world: two fighters side by side, or one body alone, to
/// strike and punch as a punching bag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bodies {
    Two,
    One,
}

#[derive(Clone, Copy)]
enum ControlAction {
    Tool(rp::ToolMode),
    /// The punch key or button on that side, 0 the left and 1 the right
    /// (`punch_for`, `rp::World::throw_punch`).
    Punch(usize),
    /// Two fighters, or one body alone, afresh.
    ToggleBodies,
    ToggleDebug,
    TogglePause,
    Reset,
    ToggleMute,
    /// What this is, who made it, and links to read more (`about`).
    About,
}

const KEY_CONTROLS: [(KeyCode, ControlAction); 12] = [
    (KeyCode::Left, ControlAction::Punch(0)),
    (KeyCode::Right, ControlAction::Punch(1)),
    (KeyCode::Key2, ControlAction::ToggleBodies),
    (KeyCode::H, ControlAction::Tool(rp::ToolMode::Heavy)),
    (KeyCode::B, ControlAction::Tool(rp::ToolMode::Blunt)),
    (KeyCode::S, ControlAction::Tool(rp::ToolMode::Sharp)),
    (KeyCode::F, ControlAction::Tool(rp::ToolMode::Fist)),
    (KeyCode::D, ControlAction::ToggleDebug),
    (KeyCode::Space, ControlAction::TogglePause),
    (KeyCode::R, ControlAction::Reset),
    (KeyCode::M, ControlAction::ToggleMute),
    (KeyCode::I, ControlAction::About),
];

const FLOOR_HEIGHT: f32 = 38.0;
/// Height of a HUD chip.
const CHIP_HEIGHT: f32 = 25.0;
/// Gap kept between the body and the HUD or the control buttons.
const BODY_UI_GAP: f32 = 4.0;

struct AppState {
    /// The bodies, simulated where every body is (`rp::world_frame`,
    /// `rp::fighter_frames`) whatever the window, so a blow hurts them alike
    /// on every screen.
    world: rp::World,
    /// Two fighters, or one body alone.
    bodies: Bodies,
    /// How the world is drawn in the window: larger or smaller, to fit the
    /// bodies between the HUD and the control buttons.
    view: rp::View,
    /// The window size, button size, and bodies the view was fitted to.
    view_for: Option<(f32, f32, bool, Bodies)>,
    running: bool,
    /// The sound is switched off.
    muted: bool,
    pointer_down: bool,
    debug_overlay: bool,
    accumulator: f64,
    pointer_initialized: bool,
    /// Where the pointer is on the screen. The hand is where it points in
    /// the world, and the simulation pulls the tool toward it.
    pointer: rp::Vec2,
    /// Where the hand was in the world on the last simulation step.
    stepped_hand: rp::Vec2,
    /// Where everything was before the last simulation step, and where it
    /// really is while the frame draws it between the two.
    earlier: rp::MotionSnapshot,
    actual: rp::MotionSnapshot,
    tool: rp::ToolMode,
    view_mode: ViewMode,
    /// The current press started on the control panel, so it must not strike.
    ui_capture: bool,
    /// Where the last control press ended. Touch screens leave the pointer there,
    /// so the striker waits for the pointer to move before following it again.
    ui_release: Option<(f32, f32)>,
    /// Set after the first touch so the control buttons grow to finger size.
    touch_ui: bool,
    /// The skin's silhouette edge, rebuilt with each new body.
    skin_rim: SkinRim,
    /// How bloody the current tool is, from 0 to 1; fades over time.
    tool_blood: f32,
    /// Fluid particles emitted so far, to see how much new blood each step adds.
    seen_fluid: i32,
    /// Draw only the scene, without the HUD, control buttons, and pointer ring.
    hide_ui: bool,
    /// Leave the tool out of the picture, to see what lies under it.
    hide_tool: bool,
    /// The about card was asked for. It opens once no button is held: in the
    /// browser it covers the app, which would then miss the button's release.
    about_requested: bool,
    /// While the browser's about card covers the app: whether the simulation
    /// ran before it opened, so it runs again once the card closes.
    about_open: Option<bool>,
    /// A punch key or button pressed since the last simulation step, which
    /// throws on the next one: which fighter throws it.
    pending_punch: Option<usize>,
    /// How far apart the two fighters stand, in body heights.
    spacing: f64,
}

impl AppState {
    fn new(width: f64, height: f64) -> Self {
        let initial_pointer = rp::Vec2 {
            x: width * 0.28,
            y: height * 0.46,
        };
        let mut app = Self {
            world: rp::World::new(rp::Materials::default()),
            bodies: Bodies::Two,
            view: rp::View::IDENTITY,
            view_for: None,
            running: true,
            muted: false,
            pointer_down: false,
            debug_overlay: false,
            accumulator: 0.0,
            pointer_initialized: false,
            pointer: initial_pointer,
            stepped_hand: initial_pointer,
            earlier: rp::MotionSnapshot::default(),
            actual: rp::MotionSnapshot::default(),
            tool: rp::ToolMode::Heavy,
            view_mode: ViewMode::Normal,
            ui_capture: false,
            ui_release: None,
            touch_ui: false,
            skin_rim: SkinRim::default(),
            tool_blood: 0.0,
            seen_fluid: 0,
            hide_ui: false,
            hide_tool: false,
            about_requested: false,
            about_open: None,
            pending_punch: None,
            spacing: rp::FIGHTER_SPACING,
        };
        app.fit_view(width as f32, height as f32);
        app.stepped_hand = app.view.to_world(app.pointer);
        app.reset_body();
        app
    }

    /// The bodies afresh: two fighters where they stand, or one body alone
    /// where the scenarios strike it.
    fn reset_body(&mut self) {
        let materials = rp::Materials::default();
        self.world = match self.bodies {
            Bodies::Two => rp::create_fighters_at(materials, self.spacing),
            Bodies::One => rp::create_layered_body_in(rp::world_frame(), materials),
        };
        self.world.capture_motion(&mut self.earlier);
        self.skin_rim = SkinRim::of(&self.world);
        self.tool_blood = 0.0;
        self.seen_fluid = 0;
        self.accumulator = 0.0;
    }

    /// Two fighters if one body stood alone, or one body alone if two
    /// fighters stood, afresh and fitted to the window, the hand left under
    /// the pointer so the tool does not sweep across the new view.
    fn switch_bodies(&mut self) {
        self.bodies = match self.bodies {
            Bodies::Two => Bodies::One,
            Bodies::One => Bodies::Two,
        };
        self.reset_body();
        if let Some((width, height, ..)) = self.view_for {
            self.fit_view(width, height);
        }
        self.stepped_hand = self.view.to_world(self.pointer);
    }

    /// Fits the view to a window of this size, to the control buttons as
    /// drawn now, and to the bodies standing, when any has changed. Only the
    /// drawing changes: the bodies, hurt or not, go on being simulated where
    /// they are.
    fn fit_view(&mut self, width: f32, height: f32) {
        let now = Some((width, height, self.touch_ui, self.bodies));
        if now != self.view_for {
            let screen = screen_frame_for(self, width, height);
            self.view = rp::View::fitting(rp::world_frame(), screen);
            self.view_for = now;
        }
    }
}

/// Where the scenarios' body frame (`rp::world_frame`) is drawn: the default
/// placement when it clears the HUD at the top and the control buttons at the
/// bottom, which wrap into more rows in narrow windows, or else as large as
/// fits between them; and with two fighters, across, the two centred in the
/// window, smaller still if they would not fit. At 1280x720 that is where the
/// world is simulated, so it is drawn as it is.
fn screen_frame_for(app: &AppState, width: f32, height: f32) -> rp::BodyFrame {
    let default = rp::body_frame(width as f64, height as f64);
    let top = (hud_bottom(width) + BODY_UI_GAP) as f64;
    let hints = control_hints(app, &Palette::default());
    let panel = layout_control_hints(&hints, width, height - FLOOR_HEIGHT, app.touch_ui).panel;
    let bottom = (panel.y - BODY_UI_GAP) as f64;
    let frame = if default.origin.y >= top && default.origin.y + default.height <= bottom {
        default
    } else {
        rp::body_frame_between(width as f64, top.max(default.origin.y), bottom)
    };
    match app.bodies {
        Bodies::One => frame,
        Bodies::Two => rp::fighters_across(frame, app.spacing, width as f64, BODY_SIDE_MARGIN),
    }
}

/// The least room left beside the fighters at each side of the window.
const BODY_SIDE_MARGIN: f64 = 12.0;

struct RenderContext<'a> {
    app: &'a AppState,
    palette: Palette,
    width: f32,
    height: f32,
    /// Where the floor the body stands on is drawn on the screen, which the
    /// ground is drawn down from.
    floor_y: f32,
    /// The same floor in the world, which blood pools on.
    world_floor_y: f32,
    anatomy: bool,
}

impl RenderContext<'_> {
    /// The world as this frame shows it (`rp::draw`).
    fn scene(&self) -> rp::draw::Scene<'_> {
        rp::draw::Scene {
            world: &self.app.world,
            rim: &self.app.skin_rim,
            palette: self.palette,
            width: self.width,
            height: self.height,
            floor_y: self.floor_y,
            world_floor_y: self.world_floor_y,
            anatomy: self.anatomy,
            debug: self.app.debug_overlay,
            show_tool: !self.app.hide_tool,
            tool_blood: self.app.tool_blood,
            pressing: self.app.pointer_down,
        }
    }
}

fn window_conf() -> Conf {
    #[cfg(not(target_arch = "wasm32"))]
    let (window_width, window_height) = capture::window_size().unwrap_or((1280, 720));
    #[cfg(target_arch = "wasm32")]
    let (window_width, window_height) = (1280, 720);
    Conf {
        window_title: "Realistic Physics Rust".to_owned(),
        window_width,
        window_height,
        high_dpi: true,
        sample_count: 4,
        ..Conf::default()
    }
}

fn main() {
    // `--version` prints the version the game shows and exits before the
    // window opens; video_tools' short.py stamps it on each Short.
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::args().skip(1).any(|arg| arg == "--version") {
        println!("{}", rp::version_label());
        return;
    }
    // What `#[macroquad::main(window_conf)]` would write.
    macroquad::Window::from_config(window_conf(), run());
}

async fn run() {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(request) = capture::request() {
        capture::run(request).await;
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(request) = inspector::request() {
        inspector::run(request).await;
        return;
    }
    if selftest::requested() {
        selftest::run_all().await;
        return;
    }
    let mut app = AppState::new(screen_width() as f64, screen_height() as f64);
    let mut audio = rp::sound::Player::start();

    loop {
        let dt = get_frame_time().min(0.05) as f64;
        if handle_input(&mut app) {
            audio.click();
        }
        app.fit_view(screen_width(), screen_height());
        step_simulation(&mut app, dt, &mut audio);
        audio.play(dt as f32, app.muted, app.running);
        // Draw the body between the last two simulation steps, so it moves
        // as smoothly as the screen refreshes.
        let alpha = app.accumulator / app.world.materials().fixed_dt;
        app.world.blend_motion(&app.earlier, alpha, &mut app.actual);
        draw_app(&app);
        app.world.restore_motion(&app.actual);
        next_frame().await;
    }
}

/// Reads the pointer and keys; true when a control button or key was used.
fn handle_input(app: &mut AppState) -> bool {
    let mut used_control = false;
    let (mx, my) = mouse_position();
    if is_mouse_button_pressed(MouseButton::Left) {
        let hints = control_hints(app, &Palette::default());
        let layout = layout_control_hints(&hints, screen_width(), screen_floor_y(), app.touch_ui);
        let point = vec2(mx, my);
        if layout.panel.contains(point) {
            app.ui_capture = true;
            if let Some(action) = layout.action_at(point) {
                apply_control(app, action);
                used_control = true;
            }
        }
    }
    // Switch to finger-sized buttons only after hit-testing, so the first tap
    // lands on the layout that was actually on screen.
    if !touches().is_empty() {
        app.touch_ui = true;
    }

    let mouse_down = is_mouse_button_down(MouseButton::Left);
    if app.ui_capture && !mouse_down {
        app.ui_capture = false;
        app.ui_release = Some((mx, my));
    }
    if app.ui_release.is_some_and(|(x, y)| x != mx || y != my) {
        app.ui_release = None;
    }

    let pointer_down = mouse_down && !app.ui_capture;
    let follows_pointer = !app.ui_capture && app.ui_release.is_none();
    if follows_pointer
        && (app.pointer_initialized || pointer_down || mx.abs() > 1.0 || my.abs() > 1.0)
    {
        app.pointer = rp::Vec2 {
            x: mx as f64,
            y: my as f64,
        };
        app.pointer_initialized = true;
    }
    app.pointer_down = pointer_down;

    for (key, action) in KEY_CONTROLS {
        if is_key_pressed(key) {
            apply_control(app, action);
            used_control = true;
        }
    }
    update_about(app, mouse_down);
    used_control
}

/// Opens the about card once it was asked for and no button is held, pausing
/// the simulation while the card covers the app, and resumes it when the card
/// closes, unless it was paused before.
fn update_about(app: &mut AppState, mouse_down: bool) {
    if app.about_requested && !mouse_down {
        app.about_requested = false;
        if about::show() {
            app.about_open = Some(app.running);
            app.running = false;
        }
    }
    if let Some(was_running) = app.about_open {
        if !about::is_open() {
            app.about_open = None;
            app.running = was_running;
        }
    }
}

fn apply_control(app: &mut AppState, action: ControlAction) {
    match action {
        ControlAction::Tool(tool) => {
            if app.tool != tool {
                app.tool = tool;
                app.tool_blood = 0.0;
            }
        }
        ControlAction::Punch(side) => app.pending_punch = Some(side),
        ControlAction::ToggleBodies => app.switch_bodies(),
        ControlAction::ToggleDebug => app.debug_overlay = !app.debug_overlay,
        ControlAction::TogglePause => app.running = !app.running,
        ControlAction::Reset => app.reset_body(),
        ControlAction::ToggleMute => app.muted = !app.muted,
        ControlAction::About => app.about_requested = true,
    }
}

/// The body and the arm that the punch key or button on `side` (0 the left,
/// 1 the right) throws with: the fighter on that side of the screen, at the
/// other with its arm toward it, or a body alone, with its arm on that side,
/// out to that side.
fn punch_for(app: &AppState, side: usize) -> (usize, rp::Limb) {
    match app.bodies {
        Bodies::Two => (side, app.world.inner_arm(side)),
        Bodies::One if side == 0 => (0, rp::Limb::LeftArm),
        Bodies::One => (0, rp::Limb::RightArm),
    }
}

/// Runs the simulation steps this frame's time covers, letting `audio` hear
/// each one and the page count the tools used.
fn step_simulation(app: &mut AppState, frame_dt: f64, audio: &mut rp::sound::Player) {
    if !app.running {
        return;
    }

    app.accumulator += frame_dt;
    let fixed_dt = app.world.materials().fixed_dt;
    let steps = (app.accumulator / fixed_dt).floor() as usize;
    // The pointer moved over the time these steps cover, so the hand moves
    // through them evenly instead of jumping on the first and resting after.
    // It is read through the view as fitted this frame, after a first touch
    // has grown the buttons, so a finger lands where the body is drawn.
    let (from, to) = (app.stepped_hand, app.view.to_world(app.pointer));
    for step in 0..steps {
        let t = (step + 1) as f64 / steps as f64;
        let input = rp::InputState {
            active: true,
            down: app.pointer_down,
            x: from.x + (to.x - from.x) * t,
            y: from.y + (to.y - from.y) * t,
            vx: 0.0,
            vy: 0.0,
            power: rp::swing_power(app.tool),
            tool: app.tool,
            punch: app.pending_punch.take().map(|side| punch_for(app, side)),
            lean: None,
        };
        app.world.capture_motion(&mut app.earlier);
        step_world(app, &input);
        audio.hear(&app.world);
        analytics::step(&app.world);
        app.accumulator -= fixed_dt;
    }
    if steps > 0 {
        app.stepped_hand = to;
    }
}

/// One fixed simulation step with this input, in the scenarios' world
/// whatever the window, plus the blood the tool picks up.
fn step_world(app: &mut AppState, input: &rp::InputState) {
    let fixed_dt = app.world.materials().fixed_dt;
    app.world.step(fixed_dt, input, SCENARIO_WIDTH);
    // A cut that opened rejoined the mesh, so the outline is rebuilt.
    app.skin_rim.refresh(&app.world);
    let emitted = app.world.stats().emitted_fluid_particles;
    if input.down {
        let fresh = (emitted - app.seen_fluid).max(0) as f32;
        app.tool_blood = (app.tool_blood + fresh * 0.012).min(1.0);
    }
    app.seen_fluid = emitted;
    app.tool_blood *= 0.9985;
}

fn draw_app(app: &AppState) {
    let floor_y = ground_y(app);
    let floor = app.view.to_world(rp::Vec2 {
        x: 0.0,
        y: floor_y as f64,
    });
    let ctx = RenderContext {
        app,
        palette: Palette::default(),
        width: screen_width(),
        height: screen_height(),
        floor_y,
        world_floor_y: floor.y as f32,
        anatomy: app.view_mode == ViewMode::Anatomy,
    };

    let scene = ctx.scene();
    rp::draw::draw_background(&scene);
    // The world, drawn larger or smaller to fit the window; the HUD and
    // buttons keep their size.
    set_camera(&WorldCamera::of(app.view));
    rp::draw::draw_body_layers(&scene);
    rp::draw::draw_effects(&scene);
    rp::draw::draw_tool(&scene);
    set_default_camera();
    if app.hide_ui {
        return;
    }
    draw_pointer(&ctx);
    draw_hud(&ctx);
    draw_controls_hint(&ctx);
    draw_version(&ctx);
    if app.debug_overlay {
        draw_debug_panel(&ctx);
    }
}

/// The bottom strip of the window the control buttons stand above.
fn screen_floor_y() -> f32 {
    screen_height() - FLOOR_HEIGHT
}

/// Where the ground begins on the screen: the floor the body stands on, at
/// its soles, or the window's bottom strip before there is a body.
fn ground_y(app: &AppState) -> f32 {
    rp::draw::ground_on_screen(&app.world, app.view, screen_floor_y())
}

/// A ring where the pointer is, the same size on any screen.
fn draw_pointer(ctx: &RenderContext) {
    let app = ctx.app;
    let pointer = app.pointer;
    let ring = if app.pointer_down { 6.0 } else { 5.0 };
    draw_circle_lines(
        pointer.x as f32,
        pointer.y as f32,
        ring,
        1.5,
        if app.pointer_down {
            ctx.palette.tool_accent
        } else {
            rgba(150, 138, 112, 200)
        },
    );
}

fn draw_hud(ctx: &RenderContext) {
    let stats = ctx.app.world.stats();
    let items = hud_items(
        ctx.app.tool,
        ctx.app.running,
        [
            stats.broken_skin,
            stats.broken_muscle,
            stats.fractured_bones,
            stats.emitted_fluid_particles,
        ],
    );
    for (index, (item, (x, y))) in items.iter().zip(hud_layout(&items, ctx.width)).enumerate() {
        let accent = match index {
            0 => tool_color(ctx.app.tool),
            1 => {
                if ctx.app.running {
                    rgba(94, 176, 108, 230)
                } else {
                    rgba(211, 93, 70, 230)
                }
            }
            _ => ctx.palette.blood_fresh,
        };
        draw_chip(
            x,
            y,
            item,
            accent,
            ctx.palette.hud_text,
            ctx.palette.hud_back,
        );
    }
}

/// The version, faint in the floor strip's left corner, clear of the body and
/// the controls, so a video of the game shows which version it was made with.
/// Capture mode's label takes the right corner.
fn draw_version(ctx: &RenderContext) {
    draw_text(
        &rp::version_label(),
        14.0,
        ctx.height - 14.0,
        15.0,
        with_alpha(ctx.palette.hud_muted, 0.7),
    );
}

/// HUD chip labels: the tool, whether the simulation runs, and the damage
/// counts (skin, muscle, bone, fluid).
fn hud_items(tool: rp::ToolMode, running: bool, counts: [i32; 4]) -> [String; 6] {
    [
        tool_name(tool).to_uppercase(),
        (if running { "LIVE" } else { "PAUSED" }).to_string(),
        format!("SKIN {}", counts[0]),
        format!("MUSCLE {}", counts[1]),
        format!("BONE {}", counts[2]),
        format!("FLUID {}", counts[3]),
    ]
}

/// Top-left corners of the HUD chips, wrapped into rows across the top.
fn hud_layout(items: &[String], width: f32) -> Vec<(f32, f32)> {
    let margin = 14.0;
    let (mut x, mut y) = (margin, margin);
    let mut corners = Vec::with_capacity(items.len());
    for item in items {
        let chip = chip_width(item);
        if x > margin && x + chip > width - margin {
            x = margin;
            y += 31.0;
        }
        corners.push((x, y));
        x += chip + 7.0;
    }
    corners
}

/// Lowest edge the HUD can reach in this window, with room for large counts.
fn hud_bottom(width: f32) -> f32 {
    let widest = hud_items(rp::ToolMode::Heavy, false, [9999, 9999, 99, 99999]);
    hud_layout(&widest, width)
        .last()
        .map_or(0.0, |&(_, y)| y + CHIP_HEIGHT)
}

#[derive(Clone, Copy)]
struct ControlHint {
    key: &'static str,
    label: HintLabel,
    accent: Color,
    active: bool,
    /// What tapping or clicking the hint does; `None` for instruction-only hints.
    action: Option<ControlAction>,
}

#[derive(Clone, Copy)]
enum HintLabel {
    Text(&'static str),
    /// A speaker, with sound waves, or struck out while the sound is off.
    Speaker {
        muted: bool,
    },
}

/// Width of the speaker drawn as a hint label.
const SPEAKER_WIDTH: f32 = 17.0;

struct ControlChip {
    rect: Rect,
    hint: ControlHint,
}

struct ControlLayout {
    panel: Rect,
    chips: Vec<ControlChip>,
}

impl ControlLayout {
    fn action_at(&self, point: Vec2) -> Option<ControlAction> {
        self.chips
            .iter()
            .find(|chip| chip.rect.contains(point))
            .and_then(|chip| chip.hint.action)
    }
}

fn control_hints(app: &AppState, palette: &Palette) -> [ControlHint; 13] {
    let punching = |side: usize| {
        let (body, arm) = punch_for(app, side);
        app.world.punch(body, arm).is_some()
    };
    [
        ControlHint {
            key: "DRAG",
            label: HintLabel::Text("strike"),
            accent: palette.tool_accent,
            active: app.pointer_down,
            action: None,
        },
        ControlHint {
            key: "H",
            label: HintLabel::Text("hammer"),
            accent: tool_color(rp::ToolMode::Heavy),
            active: app.tool == rp::ToolMode::Heavy,
            action: Some(ControlAction::Tool(rp::ToolMode::Heavy)),
        },
        ControlHint {
            key: "B",
            label: HintLabel::Text("bat"),
            accent: tool_color(rp::ToolMode::Blunt),
            active: app.tool == rp::ToolMode::Blunt,
            action: Some(ControlAction::Tool(rp::ToolMode::Blunt)),
        },
        ControlHint {
            key: "S",
            label: HintLabel::Text("knife"),
            accent: tool_color(rp::ToolMode::Sharp),
            active: app.tool == rp::ToolMode::Sharp,
            action: Some(ControlAction::Tool(rp::ToolMode::Sharp)),
        },
        ControlHint {
            key: "F",
            label: HintLabel::Text("fist"),
            accent: tool_color(rp::ToolMode::Fist),
            active: app.tool == rp::ToolMode::Fist,
            action: Some(ControlAction::Tool(rp::ToolMode::Fist)),
        },
        // The fighter on that side of the screen punches the other, with
        // its arm toward it; a body alone punches with its arm on that side.
        ControlHint {
            key: "LEFT",
            label: HintLabel::Text("punch"),
            accent: palette.tool_accent,
            active: punching(0),
            action: Some(ControlAction::Punch(0)),
        },
        ControlHint {
            key: "RIGHT",
            label: HintLabel::Text("punch"),
            accent: palette.tool_accent,
            active: punching(1),
            action: Some(ControlAction::Punch(1)),
        },
        // Lit while two fighters stand; tapped, one body stands alone, to
        // strike and punch as a punching bag, and tapped again, two.
        ControlHint {
            key: "2",
            label: HintLabel::Text("fighters"),
            accent: palette.tool_accent,
            active: app.bodies == Bodies::Two,
            action: Some(ControlAction::ToggleBodies),
        },
        ControlHint {
            key: "D",
            label: HintLabel::Text("debug"),
            accent: rgba(94, 176, 108, 230),
            active: app.debug_overlay,
            action: Some(ControlAction::ToggleDebug),
        },
        ControlHint {
            key: "SPACE",
            label: HintLabel::Text("pause"),
            accent: rgba(211, 93, 70, 230),
            active: !app.running,
            action: Some(ControlAction::TogglePause),
        },
        ControlHint {
            key: "R",
            label: HintLabel::Text("reset"),
            accent: palette.hud_border,
            active: false,
            action: Some(ControlAction::Reset),
        },
        ControlHint {
            key: "M",
            label: HintLabel::Speaker { muted: app.muted },
            accent: rgba(211, 93, 70, 230),
            active: app.muted,
            action: Some(ControlAction::ToggleMute),
        },
        ControlHint {
            key: "I",
            label: HintLabel::Text("about"),
            accent: rgba(96, 156, 220, 230),
            active: app.about_open.is_some(),
            action: Some(ControlAction::About),
        },
    ]
}

/// Places the control hints above the floor, wrapping rows on narrow screens.
/// Input hit-testing and drawing share this so presses land on what is drawn.
fn layout_control_hints(
    hints: &[ControlHint],
    width: f32,
    floor_y: f32,
    touch_ui: bool,
) -> ControlLayout {
    let margin = 14.0;
    let pad = 8.0;
    let gap = 7.0;
    let row_h = if touch_ui { 36.0 } else { 24.0 };
    let row_gap = 6.0;
    let available_w = (width - margin * 2.0 - pad * 2.0).max(260.0);

    let mut placed = Vec::with_capacity(hints.len());
    let mut x = 0.0;
    let mut row = 0;
    for hint in hints {
        let chip_w = control_hint_width(*hint);
        if x > 0.0 && x + chip_w > available_w {
            x = 0.0;
            row += 1;
        }
        placed.push((x, row, chip_w));
        x += chip_w + gap;
    }

    let rows = row + 1;
    let panel_h = pad * 2.0 + rows as f32 * row_h + (rows - 1) as f32 * row_gap;
    let panel_y = (floor_y - panel_h - 10.0).max(54.0);
    let chips = hints
        .iter()
        .zip(placed)
        .map(|(hint, (x, row, chip_w))| ControlChip {
            rect: Rect::new(
                margin + pad + x,
                panel_y + pad + row as f32 * (row_h + row_gap),
                chip_w,
                row_h,
            ),
            hint: *hint,
        })
        .collect();

    ControlLayout {
        panel: Rect::new(margin, panel_y, width - margin * 2.0, panel_h),
        chips,
    }
}

fn draw_controls_hint(ctx: &RenderContext) {
    let hints = control_hints(ctx.app, &ctx.palette);
    let layout = layout_control_hints(&hints, ctx.width, screen_floor_y(), ctx.app.touch_ui);
    let panel = layout.panel;
    draw_panel(ctx, panel.x, panel.y, panel.w, panel.h);

    let (mx, my) = mouse_position();
    for chip in &layout.chips {
        let hovered =
            !ctx.app.touch_ui && chip.hint.action.is_some() && chip.rect.contains(vec2(mx, my));
        draw_control_hint(ctx, chip.rect, chip.hint, hovered);
    }
}

fn control_hint_width(hint: ControlHint) -> f32 {
    let key = measure_text(hint.key, None, 15, 1.0);
    let label = match hint.label {
        HintLabel::Text(text) => measure_text(text, None, 15, 1.0).width,
        HintLabel::Speaker { .. } => SPEAKER_WIDTH,
    };
    key.width + label + 32.0
}

fn draw_control_hint(ctx: &RenderContext, rect: Rect, hint: ControlHint, hovered: bool) {
    let key_width = measure_text(hint.key, None, 15, 1.0).width + 13.0;
    let text_y = rect.y + rect.h * 0.5 + 4.0;
    let back = if hint.active {
        with_alpha(hint.accent, 0.20)
    } else {
        rgba(8, 8, 9, 160)
    };
    let border = if hint.active || hovered {
        with_alpha(hint.accent, 0.85)
    } else {
        with_alpha(ctx.palette.hud_border, 0.38)
    };

    draw_rectangle(rect.x, rect.y, rect.w, rect.h, back);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, border);
    draw_rectangle(
        rect.x + 3.0,
        rect.y + 3.0,
        key_width,
        rect.h - 6.0,
        with_alpha(hint.accent, 0.24),
    );
    draw_rectangle_lines(
        rect.x + 3.0,
        rect.y + 3.0,
        key_width,
        rect.h - 6.0,
        1.0,
        with_alpha(hint.accent, 0.72),
    );
    draw_text(hint.key, rect.x + 9.0, text_y, 15.0, ctx.palette.hud_text);
    let label_x = rect.x + key_width + 11.0;
    match hint.label {
        HintLabel::Text(text) => {
            draw_text(text, label_x, text_y, 15.0, ctx.palette.hud_muted);
        }
        HintLabel::Speaker { muted } => {
            draw_speaker(label_x, rect.y + rect.h * 0.5, muted, ctx.palette.hud_muted)
        }
    }
}

/// A speaker `SPEAKER_WIDTH` wide from `x`, centered on `y`: sound waves in
/// front of it, or a cross while the sound is off.
fn draw_speaker(x: f32, y: f32, muted: bool, color: Color) {
    draw_rectangle(x, y - 2.5, 3.0, 5.0, color);
    draw_triangle(
        vec2(x + 3.0, y - 2.5),
        vec2(x + 8.0, y - 6.5),
        vec2(x + 8.0, y + 6.5),
        color,
    );
    draw_triangle(
        vec2(x + 3.0, y - 2.5),
        vec2(x + 8.0, y + 6.5),
        vec2(x + 3.0, y + 2.5),
        color,
    );
    if muted {
        draw_line(x + 11.0, y - 3.5, x + 17.0, y + 3.5, 1.5, color);
        draw_line(x + 11.0, y + 3.5, x + 17.0, y - 3.5, 1.5, color);
        return;
    }
    for radius in [4.5f32, 8.0] {
        // An arc of a few straight pieces, opening away from the speaker.
        let pieces = 6;
        let point = |i: usize| {
            let angle = -0.8 + 1.6 * i as f32 / pieces as f32;
            vec2(x + 8.0 + radius * angle.cos(), y + radius * angle.sin())
        };
        for i in 0..pieces {
            let (a, b) = (point(i), point(i + 1));
            draw_line(a.x, a.y, b.x, b.y, 1.3, color);
        }
    }
}

fn draw_debug_panel(ctx: &RenderContext) {
    let debug = ctx.app.world.debug();
    let stats = ctx.app.world.stats();
    let materials = ctx.app.world.materials();
    let active_fluids = ctx
        .app
        .world
        .fluids()
        .iter()
        .filter(|fluid| fluid.life > 0.0)
        .count();
    let panel_x = 14.0;
    let panel_y = 54.0;
    let panel_w = 540.0;
    let panel_h = 340.0;
    draw_panel(ctx, panel_x, panel_y, panel_w, panel_h);

    let lines = [
        format!(
            "CONTACT  tool={}  down={}",
            tool_name(debug.tool),
            if debug.down { "yes" } else { "no" }
        ),
        format!(
            "head=({:.0},{:.0})  speed={:.0}px/s  mass={:.1}",
            debug.striker_position.x,
            debug.striker_position.y,
            debug.striker_speed,
            debug.striker_mass
        ),
        format!(
            "impact={:.0}  tissue={}  bone={}  depth={:.1}",
            debug.impact, debug.tissue_contacts, debug.bone_contacts, debug.max_depth
        ),
        format!(
            "loads  tissue={:.0}  bone={:.0}  fracture={:.0}",
            debug.max_point_load, debug.max_bone_load, debug.last_fracture_impulse
        ),
        format!(
            "damage  skin={} muscle={} fiber={} prop={} crush={} flaps={} vessels={} attach={}/{} joints={}",
            stats.broken_skin,
            stats.broken_muscle,
            stats.muscle_fiber_tears,
            stats.tear_propagations,
            stats.muscle_crush_ruptures,
            stats.skin_flap_detachments,
            stats.vessel_lacerations,
            stats.broken_attachments,
            stats.broken_bone_attachments,
            stats.broken_bone_joints
        ),
        format!(
            "contusion  active={} events={} max={:.2} soften={:.2} fatigue={:.2} plastic={:.2}",
            debug.active_contusions,
            stats.contusion_events,
            debug.max_contusion,
            debug.max_tissue_softening,
            debug.max_tissue_fatigue,
            debug.max_tissue_plasticity
        ),
        format!(
            "fragments  step={} tissue={} pair={} tears={} punctures={}",
            debug.fractures,
            debug.fragment_contacts,
            debug.fragment_pair_contacts,
            debug.fragment_tears,
            stats.fragment_skin_punctures
        ),
        format!(
            "fragment motion  impulse={:.0} spin={:.2} overlap={:.1}",
            debug.max_fragment_impulse, debug.max_bone_angular_speed, debug.max_fragment_overlap
        ),
        format!(
            "joint limits  corrections={} sublux={} lig={} ribfx={} max={:.2} stretch={:.1} angle={:.2}",
            debug.post_fracture_joint_corrections,
            stats.bone_joint_subluxations,
            stats.joint_ligament_damage_events,
            stats.fractured_ribs,
            debug.max_bone_joint_subluxation,
            debug.max_post_fracture_joint_stretch,
            debug.max_post_fracture_joint_angle
        ),
        format!(
            "fluid  active={} stains={} emitted={} marrow={} stain_deposits={} blood={:.2} turgor={:.2} loss={:.3}",
            active_fluids,
            debug.active_blood_stains,
            stats.emitted_fluid_particles,
            stats.fracture_marrow_sources,
            stats.blood_stain_deposits,
            ctx.app.world.blood_volume_fraction(),
            ctx.app.world.blood_turgor_scale(),
            stats.blood_loss
        ),
        format!(
            "wounds  active={} leaks={} reopens={} pressure={:.2} clot={:.2}",
            debug.active_wounds,
            debug.wound_leaks,
            stats.wound_reopens,
            debug.max_wound_pressure,
            debug.max_wound_clot
        ),
        format!(
            "cavity  pressure={:.2} collapse={:.2} events={} ruptures={}",
            debug.max_cavity_pressure,
            debug.max_cavity_collapse,
            stats.cavity_pressure_events,
            stats.cavity_ruptures
        ),
        format!(
            "organs  damage={:.2} events={} penetrations={} ribOrg={} ruptures={} fragVessel={}",
            debug.max_organ_damage,
            stats.organ_damage_events,
            stats.organ_penetrations,
            stats.rib_organ_punctures,
            stats.organ_ruptures,
            stats.fragment_vessel_lacerations
        ),
        format!(
            "budget  fragments={}/{} sleep={} skipped={} blocks={}",
            debug.active_fragments,
            materials.max_active_bone_fragments,
            debug.sleeping_fragments,
            debug.fragment_budget_skips,
            debug.fracture_budget_blocks
        ),
        format!(
            "checks  bone={}/{} pair={}/{} tissue={}/{}",
            debug.fragment_bone_checks,
            materials.max_fragment_bone_checks,
            debug.fragment_pair_checks,
            materials.max_fragment_pair_checks,
            debug.fragment_tissue_checks,
            materials.max_fragment_tissue_checks
        ),
        format!(
            "support bone={}/{} pair={}/{} floor={}/{}",
            debug.fragment_bone_damping_events,
            debug.fragment_bone_resting_contacts,
            debug.fragment_pair_damping_events,
            debug.fragment_pair_resting_contacts,
            debug.fragment_floor_contacts,
            debug.fragment_floor_resting_contacts
        ),
        format!(
            "caps  fluid={} stain={} wound={} sleep/wake={}/{} solver={}",
            debug.fluid_budget_replacements,
            debug.blood_stain_budget_replacements,
            debug.wound_budget_replacements,
            debug.fragment_sleep_events,
            debug.fragment_wake_events,
            debug.solver_iterations
        ),
    ];

    let mut y = panel_y + 24.0;
    for (index, line) in lines.iter().enumerate() {
        let color = if index == 0 {
            ctx.palette.tool_accent
        } else {
            ctx.palette.hud_text
        };
        draw_text(line, panel_x + 14.0, y, 17.0, color);
        y += 21.0;
    }
}

fn draw_panel(ctx: &RenderContext, x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x, y, w, h, ctx.palette.hud_back);
    draw_rectangle_lines(x, y, w, h, 1.0, ctx.palette.hud_border);
    draw_line(
        x + 1.0,
        y + 1.0,
        x + w - 1.0,
        y + 1.0,
        1.0,
        with_alpha(ctx.palette.tool_accent, 0.45),
    );
}

fn chip_width(label: &str) -> f32 {
    measure_text(label, None, 17, 1.0).width + 20.0
}

fn draw_chip(x: f32, y: f32, label: &str, accent: Color, text: Color, back: Color) {
    let width = chip_width(label);
    draw_rectangle(x, y, width, CHIP_HEIGHT, back);
    draw_rectangle_lines(x, y, width, CHIP_HEIGHT, 1.0, with_alpha(accent, 0.58));
    draw_rectangle(x, y, 4.0, CHIP_HEIGHT, accent);
    draw_text(label, x + 10.0, y + 17.0, 17.0, text);
}

fn tool_name(tool: rp::ToolMode) -> &'static str {
    match tool {
        rp::ToolMode::Blunt => "bat",
        rp::ToolMode::Sharp => "knife",
        rp::ToolMode::Heavy => "hammer",
        rp::ToolMode::Fist => "fist",
    }
}

fn tool_color(tool: rp::ToolMode) -> Color {
    match tool {
        rp::ToolMode::Blunt => rgba(211, 167, 91, 235),
        rp::ToolMode::Sharp => rgba(192, 220, 224, 235),
        rp::ToolMode::Heavy => rgba(132, 141, 147, 235),
        rp::ToolMode::Fist => rgba(201, 146, 119, 235),
    }
}
