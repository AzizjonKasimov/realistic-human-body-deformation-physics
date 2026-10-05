use macroquad::prelude::*;
use realistic_physics as rp;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Normal,
    Anatomy,
}

/// App commands shared by keyboard shortcuts and the on-screen control buttons.
#[derive(Clone, Copy)]
enum ControlAction {
    Tool(rp::ToolMode),
    ToggleView,
    ToggleDebug,
    TogglePause,
    SetMass(f64),
    CycleMass,
    Reset,
}

const KEY_CONTROLS: [(KeyCode, ControlAction); 10] = [
    (KeyCode::B, ControlAction::Tool(rp::ToolMode::Blunt)),
    (KeyCode::S, ControlAction::Tool(rp::ToolMode::Sharp)),
    (KeyCode::H, ControlAction::Tool(rp::ToolMode::Heavy)),
    (KeyCode::D, ControlAction::ToggleDebug),
    (KeyCode::Tab, ControlAction::ToggleView),
    (KeyCode::Space, ControlAction::TogglePause),
    (KeyCode::Key1, ControlAction::SetMass(1.0)),
    (KeyCode::Key2, ControlAction::SetMass(2.0)),
    (KeyCode::Key4, ControlAction::SetMass(4.0)),
    (KeyCode::R, ControlAction::Reset),
];

const FLOOR_HEIGHT: f32 = 38.0;

struct AppState {
    world: rp::World,
    running: bool,
    pointer_down: bool,
    debug_overlay: bool,
    accumulator: f64,
    pointer_initialized: bool,
    /// Where the hand is; the simulation pulls the tool toward it.
    pointer: rp::Vec2,
    impact_power: f64,
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
}

/// A skin spring on the body's outline, with the third corner of its triangle
/// so the inward side is known as the body deforms.
#[derive(Clone, Copy)]
struct OutlineEdge {
    spring: usize,
    inner_point: usize,
}

#[derive(Default)]
struct SkinRim {
    edges: Vec<OutlineEdge>,
    /// Per point: rest distance across the body along the inward direction,
    /// which caps the shading strip so it never spills past a thin limb.
    reach: Vec<f32>,
    /// Per spring: the skin triangles on either side, `NO_TRIANGLE` where there
    /// is none, so wounds can find where intact skin meets an opening.
    edge_triangles: Vec<[usize; 2]>,
}

const NO_TRIANGLE: usize = usize::MAX;

impl AppState {
    fn new(width: f64, height: f64) -> Self {
        let world = rp::create_layered_body(width, height, rp::Materials::default());
        let skin_rim = skin_rim(&world);
        let initial_pointer = rp::Vec2 {
            x: width * 0.28,
            y: height * 0.46,
        };
        Self {
            world,
            running: true,
            pointer_down: false,
            debug_overlay: false,
            accumulator: 0.0,
            pointer_initialized: false,
            pointer: initial_pointer,
            impact_power: 2.0,
            tool: rp::ToolMode::Blunt,
            view_mode: ViewMode::Anatomy,
            ui_capture: false,
            ui_release: None,
            touch_ui: false,
            skin_rim,
            tool_blood: 0.0,
            seen_fluid: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct RenderPalette {
    background: Color,
    background_low: Color,
    floor: Color,
    floor_edge: Color,
    skin_base: Color,
    skin_light: Color,
    skin_mid: Color,
    skin_shade: Color,
    skin_heat: Color,
    skin_contusion: Color,
    skin_outline: Color,
    skin_wire: Color,
    muscle_base: Color,
    muscle_hot: Color,
    muscle_contusion: Color,
    muscle_shadow: Color,
    bone: Color,
    bone_fractured: Color,
    bone_shadow: Color,
    blood_dark: Color,
    blood_mid: Color,
    blood_fresh: Color,
    blood_stain: Color,
    major_vessel: Color,
    major_vessel_shadow: Color,
    wound_core: Color,
    wound_edge: Color,
    wound_shadow: Color,
    attachment: Color,
    hud_back: Color,
    hud_border: Color,
    hud_text: Color,
    hud_muted: Color,
    tool_accent: Color,
}

struct RenderContext<'a> {
    app: &'a AppState,
    palette: RenderPalette,
    width: f32,
    height: f32,
    floor_y: f32,
    anatomy: bool,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Realistic Physics Rust".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        sample_count: 4,
        ..Conf::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut app = AppState::new(screen_width() as f64, screen_height() as f64);

    loop {
        let dt = get_frame_time().min(0.05) as f64;
        handle_input(&mut app);
        step_simulation(&mut app, dt);
        draw_app(&app);
        next_frame().await;
    }
}

fn handle_input(app: &mut AppState) {
    let (mx, my) = mouse_position();
    if is_mouse_button_pressed(MouseButton::Left) {
        let hints = control_hints(app, &render_palette());
        let layout = layout_control_hints(&hints, screen_width(), screen_floor_y(), app.touch_ui);
        let point = vec2(mx, my);
        if layout.panel.contains(point) {
            app.ui_capture = true;
            if let Some(action) = layout.action_at(point) {
                apply_control(app, action);
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
        ControlAction::ToggleView => {
            app.view_mode = if app.view_mode == ViewMode::Anatomy {
                ViewMode::Normal
            } else {
                ViewMode::Anatomy
            };
        }
        ControlAction::ToggleDebug => app.debug_overlay = !app.debug_overlay,
        ControlAction::TogglePause => app.running = !app.running,
        ControlAction::SetMass(power) => app.impact_power = power,
        ControlAction::CycleMass => {
            app.impact_power = if app.impact_power < 2.0 {
                2.0
            } else if app.impact_power < 4.0 {
                4.0
            } else {
                1.0
            };
        }
        ControlAction::Reset => {
            app.world = rp::create_layered_body(
                screen_width() as f64,
                screen_height() as f64,
                rp::Materials::default(),
            );
            app.skin_rim = skin_rim(&app.world);
            app.tool_blood = 0.0;
            app.seen_fluid = 0;
            app.accumulator = 0.0;
        }
    }
}

fn step_simulation(app: &mut AppState, frame_dt: f64) {
    if !app.running {
        return;
    }

    app.accumulator += frame_dt;
    let fixed_dt = app.world.materials().fixed_dt;
    while app.accumulator >= fixed_dt {
        let input = rp::InputState {
            active: true,
            down: app.pointer_down,
            x: app.pointer.x,
            y: app.pointer.y,
            vx: 0.0,
            vy: 0.0,
            power: app.impact_power,
            tool: app.tool,
        };
        app.world.step(
            fixed_dt,
            &input,
            screen_width() as f64,
            screen_height() as f64,
        );
        let emitted = app.world.stats().emitted_fluid_particles;
        if app.pointer_down {
            let fresh = (emitted - app.seen_fluid).max(0) as f32;
            app.tool_blood = (app.tool_blood + fresh * 0.012).min(1.0);
        }
        app.seen_fluid = emitted;
        app.tool_blood *= 0.9985;
        app.accumulator -= fixed_dt;
    }
}

fn draw_app(app: &AppState) {
    let ctx = RenderContext {
        app,
        palette: render_palette(),
        width: screen_width(),
        height: screen_height(),
        floor_y: screen_floor_y(),
        anatomy: app.view_mode == ViewMode::Anatomy,
    };

    draw_background(&ctx);
    draw_body_layers(&ctx);
    draw_effects(&ctx);
    draw_striker(&ctx);
    draw_hud(&ctx);
    draw_controls_hint(&ctx);
    if app.debug_overlay {
        draw_debug_panel(&ctx);
    }
}

fn render_palette() -> RenderPalette {
    RenderPalette {
        background: rgba(15, 15, 17, 255),
        background_low: rgba(26, 22, 22, 255),
        floor: rgba(35, 29, 26, 255),
        floor_edge: rgba(92, 64, 55, 255),
        skin_base: rgba(152, 101, 83, 246),
        skin_light: rgba(201, 146, 119, 250),
        skin_mid: rgba(184, 130, 105, 250),
        skin_shade: rgba(92, 54, 44, 255),
        skin_heat: rgba(223, 77, 55, 246),
        skin_contusion: rgba(55, 31, 86, 235),
        skin_outline: rgba(69, 40, 36, 220),
        skin_wire: rgba(136, 86, 75, 130),
        muscle_base: rgba(112, 22, 31, 190),
        muscle_hot: rgba(190, 35, 47, 225),
        muscle_contusion: rgba(49, 13, 61, 215),
        muscle_shadow: rgba(47, 8, 13, 150),
        bone: rgba(222, 211, 181, 240),
        bone_fractured: rgba(255, 245, 218, 255),
        bone_shadow: rgba(53, 43, 35, 160),
        blood_dark: rgba(43, 2, 7, 235),
        blood_mid: rgba(103, 7, 15, 230),
        blood_fresh: rgba(190, 22, 26, 235),
        blood_stain: rgba(38, 0, 7, 225),
        major_vessel: rgba(168, 12, 24, 230),
        major_vessel_shadow: rgba(28, 0, 5, 210),
        wound_core: rgba(34, 0, 5, 230),
        wound_edge: rgba(156, 18, 24, 235),
        wound_shadow: rgba(17, 0, 4, 225),
        attachment: rgba(70, 148, 235, 48),
        hud_back: rgba(13, 13, 15, 198),
        hud_border: rgba(118, 97, 83, 170),
        hud_text: rgba(232, 226, 212, 245),
        hud_muted: rgba(168, 156, 143, 220),
        tool_accent: rgba(255, 188, 66, 245),
    }
}

fn screen_floor_y() -> f32 {
    screen_height() - FLOOR_HEIGHT
}

fn draw_background(ctx: &RenderContext) {
    clear_background(ctx.palette.background);
    draw_rectangle(
        0.0,
        ctx.height * 0.54,
        ctx.width,
        ctx.height * 0.46,
        ctx.palette.background_low,
    );
    draw_rectangle(0.0, ctx.floor_y, ctx.width, FLOOR_HEIGHT, ctx.palette.floor);
    draw_line(
        0.0,
        ctx.floor_y,
        ctx.width,
        ctx.floor_y,
        1.0,
        ctx.palette.floor_edge,
    );
    for i in 0..6 {
        let y = ctx.floor_y + 7.0 + i as f32 * 5.0;
        let alpha = 0.04 + i as f32 * 0.012;
        draw_line(
            0.0,
            y,
            ctx.width,
            y,
            1.0,
            with_alpha(rgba(104, 65, 52, 255), alpha),
        );
    }
}

fn draw_body_layers(ctx: &RenderContext) {
    let world = &ctx.app.world;

    if ctx.anatomy {
        draw_muscle_layer(ctx);
        draw_muscle_voids(ctx);
        draw_major_vessels(ctx);
        draw_skin_layer(ctx);
        draw_bone_attachments(ctx);
        draw_bones(ctx, BonePass::Anatomy);
    } else {
        // Inside out: a cavity where muscle is torn away, the bones, the
        // muscle, then the skin. Bone shows only through openings in the
        // flesh, or where a broken end sticks out of the body.
        draw_muscle_voids(ctx);
        draw_bones(ctx, BonePass::Buried);
        draw_muscle_layer(ctx);
        draw_skin_layer(ctx);
        draw_closed_cut_skin(ctx);
    }

    draw_skin_wounds(ctx);
    if !ctx.anatomy {
        draw_major_vessels(ctx);
    }
    draw_wound_sources(ctx);

    // Where the tool presses hardest; a debugging aid, so only with the panel.
    let debug = world.debug();
    if ctx.app.debug_overlay && debug.max_depth > 0.0 {
        draw_soft_circle(
            to_mq(debug.strongest_contact),
            13.0,
            4,
            with_alpha(ctx.palette.tool_accent, 0.18),
        );
        draw_circle_lines(
            debug.strongest_contact.x as f32,
            debug.strongest_contact.y as f32,
            7.0,
            1.4,
            with_alpha(ctx.palette.tool_accent, 0.68),
        );
    }
}

fn draw_muscle_layer(ctx: &RenderContext) {
    if !ctx.anatomy {
        draw_muscle_flesh(ctx);
        return;
    }
    let world = &ctx.app.world;
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Muscle || !world.triangle_alive(triangle) {
            continue;
        }

        let (load, exposure) = triangle_point_metrics(world, triangle);
        let contusion = triangle_point_contusion(world, triangle);
        let visible = ctx.anatomy
            || exposure > 0.035
            || triangle.damage > 0.015
            || load > 140.0
            || contusion > 0.08;
        if !visible {
            continue;
        }

        let heat = ((load / 900.0) + triangle.damage * 0.85 + exposure * 0.35).clamp(0.0, 1.0);
        let mut fill = mix(ctx.palette.muscle_base, ctx.palette.muscle_hot, heat as f32);
        fill = mix(
            fill,
            ctx.palette.muscle_contusion,
            (contusion * 0.48).clamp(0.0, 0.62) as f32,
        );
        fill.a = if ctx.anatomy {
            (0.54 + heat as f32 * 0.28 + exposure as f32 * 0.10).min(0.88)
        } else {
            (0.20
                + exposure as f32 * 0.58
                + triangle.damage as f32 * 0.28
                + contusion as f32 * 0.08)
                .clamp(0.18, 0.86)
        };
        let mut shadow = ctx.palette.muscle_shadow;
        shadow.a = if ctx.anatomy {
            0.16 + heat as f32 * 0.10
        } else {
            (0.08 + exposure as f32 * 0.18).min(0.24)
        };
        fill_triangle(world, triangle, shadow);
        fill_triangle(world, triangle, fill);
    }
}

/// Muscle as solid flesh in one mesh. The skin covers it, so it shows only
/// through openings in the skin, as red flesh rather than a faint wash.
fn draw_muscle_flesh(ctx: &RenderContext) {
    let world = &ctx.app.world;
    let points = world.points();
    let mut vertex_of = vec![u16::MAX; points.len()];
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: None,
    };
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Muscle || !world.triangle_alive(triangle) {
            continue;
        }
        for index in [triangle.a, triangle.b, triangle.c] {
            if vertex_of[index] == u16::MAX {
                let point = &points[index];
                let heat = (point.load / 900.0 + point.exposure * 0.35).clamp(0.0, 1.0) as f32;
                let mut color = mix(ctx.palette.muscle_base, ctx.palette.muscle_hot, heat);
                color = mix(
                    color,
                    ctx.palette.muscle_contusion,
                    (point.contusion * 0.48).clamp(0.0, 0.62) as f32,
                );
                color.a = 0.94;
                vertex_of[index] = mesh.vertices.len() as u16;
                mesh.vertices.push(Vertex::new(
                    point.position.x as f32,
                    point.position.y as f32,
                    0.0,
                    0.0,
                    0.0,
                    color,
                ));
            }
            mesh.indices.push(vertex_of[index]);
        }
    }
    draw_mesh(&mesh);
}

/// Torn-through muscle as a dark cavity. It is drawn beneath the skin, so it
/// shows only where the skin over it is open.
fn draw_muscle_voids(ctx: &RenderContext) {
    let world = &ctx.app.world;
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Muscle || world.triangle_alive(triangle) {
            continue;
        }
        let (load, exposure) = triangle_point_metrics(world, triangle);
        let depth = (exposure * 0.55 + triangle.damage * 0.45 + load / 2200.0).clamp(0.0, 1.0);
        fill_triangle(
            world,
            triangle,
            with_alpha(ctx.palette.wound_shadow, 0.45 + depth as f32 * 0.30),
        );
    }
}

fn draw_skin_layer(ctx: &RenderContext) {
    if !ctx.anatomy {
        draw_shaded_skin(ctx);
        return;
    }
    // Anatomy view: a faint skin veil with its wireframe over the exposed layers.
    let world = &ctx.app.world;
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Skin || !world.triangle_alive(triangle) {
            continue;
        }
        let (load, _) = triangle_point_metrics(world, triangle);
        let contusion = triangle_point_contusion(world, triangle);
        let heat = (load / 1300.0).clamp(0.0, 1.0) as f32;
        let mut veil = mix(ctx.palette.skin_base, ctx.palette.skin_heat, heat * 0.35);
        veil = mix(
            veil,
            ctx.palette.skin_contusion,
            (contusion * 0.42).clamp(0.0, 0.55) as f32,
        );
        veil.a = (0.08 + heat * 0.08).min(0.17);
        fill_triangle(world, triangle, veil);
        let wire = mix(ctx.palette.skin_wire, ctx.palette.skin_heat, heat * 0.65);
        outline_triangle(world, triangle, wire, 1.0);
    }
}

/// Intact skin as one mesh with per-point colors, so shading blends smoothly
/// across triangles instead of showing the mesh: darker toward the outline for
/// a rounded look, warmer under load, bruised where contused.
fn draw_shaded_skin(ctx: &RenderContext) {
    let world = &ctx.app.world;
    let points = world.points();
    let mut vertex_of = vec![u16::MAX; points.len()];
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: None,
    };
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Skin || !world.triangle_alive(triangle) {
            continue;
        }
        for index in [triangle.a, triangle.b, triangle.c] {
            if vertex_of[index] == u16::MAX {
                let point = &points[index];
                vertex_of[index] = mesh.vertices.len() as u16;
                mesh.vertices.push(Vertex::new(
                    point.position.x as f32,
                    point.position.y as f32,
                    0.0,
                    0.0,
                    0.0,
                    skin_point_color(ctx, point),
                ));
            }
            mesh.indices.push(vertex_of[index]);
        }
    }
    draw_mesh(&mesh);
    draw_skin_rim(ctx);
}

/// How far a broken skin spring's ends have pulled apart, relative to its rest
/// length. A clean cut starts near 1; tissue torn by stretching starts wide.
fn cut_gap(world: &rp::World, spring: &rp::Spring) -> f64 {
    let points = world.points();
    length(sub(points[spring.b].position, points[spring.a].position)) / spring.rest.max(1.0)
}

/// How open a skin triangle is, from 0 (intact, or split by a cut whose edges
/// still meet) to 1 (gone, showing whatever lies beneath).
fn skin_opening(world: &rp::World, triangle: &rp::Triangle) -> f32 {
    if world.triangle_alive(triangle) {
        return 0.0;
    }
    if triangle.failed {
        return 1.0;
    }
    let springs = world.springs();
    let widest = [triangle.edge_ab, triangle.edge_bc, triangle.edge_ca]
        .iter()
        .filter_map(|&edge| springs.get(edge))
        .filter(|spring| spring.broken)
        .map(|spring| cut_gap(world, spring))
        .fold(1.0, f64::max);
    smoothstep(1.06, 1.4, widest as f32)
}

/// Skin triangles split by a clean cut that has not pulled apart yet. The mesh
/// cannot split inside a triangle, so without this a thin knife cut would show
/// as a whole row of missing skin; instead the skin stays closed over the cut
/// and fades away only as the edges separate.
fn draw_closed_cut_skin(ctx: &RenderContext) {
    let world = &ctx.app.world;
    let points = world.points();
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: None,
    };
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Skin
            || triangle.failed
            || world.triangle_alive(triangle)
        {
            continue;
        }
        let closed = 1.0 - skin_opening(world, triangle);
        if closed <= 0.02 || mesh.vertices.len() + 3 > u16::MAX as usize {
            continue;
        }
        let base = mesh.vertices.len() as u16;
        for index in [triangle.a, triangle.b, triangle.c] {
            let point = &points[index];
            let mut color = mix(skin_point_color(ctx, point), ctx.palette.blood_mid, 0.18);
            color.a *= closed;
            mesh.vertices.push(Vertex::new(
                point.position.x as f32,
                point.position.y as f32,
                0.0,
                0.0,
                0.0,
                color,
            ));
        }
        mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }
    draw_mesh(&mesh);
}

/// Soft shading strips just inside the silhouette plus a thin outline. Kept
/// separate from the mesh colors because thin wrists and ankles are meshed only
/// from outline points, so per-point shading alone would leave them dark.
fn draw_skin_rim(ctx: &RenderContext) {
    let world = &ctx.app.world;
    let points = world.points();
    let springs = world.springs();
    let rim = &ctx.app.skin_rim;
    let mut inward = vec![rp::Vec2 { x: 0.0, y: 0.0 }; points.len()];
    for edge in &rim.edges {
        let spring = springs[edge.spring];
        if spring.broken {
            continue;
        }
        let normal = inward_normal(points, spring, edge.inner_point, |p| p.position);
        inward[spring.a] = add(inward[spring.a], normal);
        inward[spring.b] = add(inward[spring.b], normal);
    }

    // A wide faint strip gives limbs volume; a narrow stronger one defines the edge.
    for (width, alpha, reach_share) in [(24.0_f64, 0.18, 0.5), (9.0, 0.45, 0.42)] {
        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        for edge in &rim.edges {
            let spring = springs[edge.spring];
            if spring.broken {
                continue;
            }
            let base = mesh.vertices.len() as u16;
            for index in [spring.a, spring.b] {
                let position = points[index].position;
                let depth = width.min(rim.reach[index] as f64 * reach_share);
                let direction = normalized(inward[index], rp::Vec2 { x: 0.0, y: 0.0 });
                let inner = add(position, scale(direction, depth));
                mesh.vertices.push(Vertex::new(
                    position.x as f32,
                    position.y as f32,
                    0.0,
                    0.0,
                    0.0,
                    with_alpha(ctx.palette.skin_shade, alpha),
                ));
                mesh.vertices.push(Vertex::new(
                    inner.x as f32,
                    inner.y as f32,
                    0.0,
                    0.0,
                    0.0,
                    with_alpha(ctx.palette.skin_shade, 0.0),
                ));
            }
            mesh.indices.extend_from_slice(&[
                base,
                base + 2,
                base + 1,
                base + 1,
                base + 2,
                base + 3,
            ]);
        }
        draw_mesh(&mesh);
    }

    let outline = with_alpha(ctx.palette.skin_outline, 0.78);
    for edge in &rim.edges {
        let spring = springs[edge.spring];
        if !spring.broken {
            draw_line_vec(
                points[spring.a].position,
                points[spring.b].position,
                1.4,
                outline,
            );
        }
    }
}

/// Unit normal of an outline spring pointing into the body.
fn inward_normal(
    points: &[rp::Point],
    spring: rp::Spring,
    inner_point: usize,
    position: impl Fn(&rp::Point) -> rp::Vec2,
) -> rp::Vec2 {
    let a = position(&points[spring.a]);
    let b = position(&points[spring.b]);
    let along = normalized(sub(b, a), rp::Vec2 { x: 1.0, y: 0.0 });
    let normal = rp::Vec2 {
        x: -along.y,
        y: along.x,
    };
    let toward_inside = sub(position(&points[inner_point]), a);
    if normal.x * toward_inside.x + normal.y * toward_inside.y < 0.0 {
        scale(normal, -1.0)
    } else {
        normal
    }
}

fn skin_point_color(ctx: &RenderContext, point: &rp::Point) -> Color {
    let depth = smoothstep(0.0, 22.0, point.surface_depth as f32);
    let heat = (point.load / 1300.0).clamp(0.0, 1.0) as f32;
    let mut color = mix(ctx.palette.skin_mid, ctx.palette.skin_light, depth);
    color = mix(color, ctx.palette.skin_heat, heat * 0.85);
    color = mix(
        color,
        ctx.palette.skin_contusion,
        (point.contusion as f32 * 0.56).clamp(0.0, 0.72),
    );
    color.a = (0.98 - point.exposure as f32 * 0.24).clamp(0.68, 0.98);
    color
}

/// The skin's outline: springs that border only one skin triangle, and how far
/// across the body each outline point can see along its inward direction.
fn skin_rim(world: &rp::World) -> SkinRim {
    let points = world.points();
    let springs = world.springs();
    let mut owners: Vec<(u8, usize)> = vec![(0, 0); springs.len()];
    let mut edge_triangles = vec![[NO_TRIANGLE; 2]; springs.len()];
    for (index, triangle) in world.triangles().iter().enumerate() {
        if triangle.layer != rp::TissueLayer::Skin {
            continue;
        }
        for (edge, opposite) in [
            (triangle.edge_ab, triangle.c),
            (triangle.edge_bc, triangle.a),
            (triangle.edge_ca, triangle.b),
        ] {
            if let Some(owner) = owners.get_mut(edge) {
                *owner = (owner.0.saturating_add(1), opposite);
            }
            if let Some(sides) = edge_triangles.get_mut(edge) {
                let free = usize::from(sides[0] != NO_TRIANGLE);
                sides[free.min(1)] = index;
            }
        }
    }
    let edges: Vec<OutlineEdge> = owners
        .iter()
        .enumerate()
        .filter_map(|(spring, &(count, inner_point))| {
            (count == 1).then_some(OutlineEdge {
                spring,
                inner_point,
            })
        })
        .collect();

    let mut reach = vec![0.0f32; points.len()];
    for edge in &edges {
        let spring = springs[edge.spring];
        let normal = inward_normal(points, spring, edge.inner_point, |p| p.home);
        for index in [spring.a, spring.b] {
            let origin = points[index].home;
            // Nearest crossing of the inward ray with another outline edge.
            let mut nearest: f64 = 60.0;
            for other in &edges {
                let other_spring = springs[other.spring];
                if other_spring.a == index || other_spring.b == index {
                    continue;
                }
                if let Some(t) = ray_hits_segment(
                    origin,
                    normal,
                    points[other_spring.a].home,
                    points[other_spring.b].home,
                ) {
                    nearest = nearest.min(t);
                }
            }
            reach[index] = reach[index].max(nearest as f32);
        }
    }
    SkinRim {
        edges,
        reach,
        edge_triangles,
    }
}

/// Distance along the ray to segment `a`-`b`, if they cross.
fn ray_hits_segment(
    origin: rp::Vec2,
    direction: rp::Vec2,
    a: rp::Vec2,
    b: rp::Vec2,
) -> Option<f64> {
    let segment = sub(b, a);
    let denominator = direction.x * segment.y - direction.y * segment.x;
    if denominator.abs() < 1.0e-9 {
        return None;
    }
    let offset = sub(a, origin);
    let t = (offset.x * segment.y - offset.y * segment.x) / denominator;
    let s = (offset.x * direction.y - offset.y * direction.x) / denominator;
    (t > 0.5 && (0.0..=1.0).contains(&s)).then_some(t)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn draw_major_vessels(ctx: &RenderContext) {
    for vessel in ctx.app.world.vessels() {
        if ctx.anatomy {
            let opacity = if vessel.lacerated { 0.82 } else { 0.34 };
            draw_vessel_run(ctx, vessel.a, vessel.b, vessel.radius, opacity);
            if vessel.lacerated {
                draw_soft_circle(
                    to_mq(point_along(vessel.a, vessel.b, vessel.laceration_t)),
                    (vessel.radius * 3.8 + 7.0) as f32,
                    4,
                    with_alpha(ctx.palette.blood_fresh, 0.18),
                );
            }
        } else if vessel.lacerated {
            draw_severed_vessel(ctx, vessel);
        }
    }
}

fn draw_vessel_run(ctx: &RenderContext, a: rp::Vec2, b: rp::Vec2, radius: f64, opacity: f32) {
    draw_line_vec(
        a,
        b,
        (radius * 2.5 + 2.0) as f32,
        with_alpha(ctx.palette.major_vessel_shadow, opacity * 0.64),
    );
    draw_line_vec(
        a,
        b,
        (radius * 1.35 + 0.8) as f32,
        with_alpha(ctx.palette.major_vessel, opacity),
    );
}

/// With the skin on, only the cut is visible: two retracted vessel ends in a
/// pool of fresh blood, not the whole artery.
fn draw_severed_vessel(ctx: &RenderContext, vessel: &rp::VesselSegment) {
    let cut = point_along(vessel.a, vessel.b, vessel.laceration_t);
    let along = normalized(sub(vessel.b, vessel.a), rp::Vec2 { x: 0.0, y: 1.0 });
    let gap = vessel.radius * 1.4 + 1.5;
    let stump = vessel.radius * 2.2 + 4.0;
    draw_soft_circle(
        to_mq(cut),
        (vessel.radius * 3.4 + 6.0) as f32,
        4,
        with_alpha(ctx.palette.blood_fresh, 0.22),
    );
    for direction in [1.0, -1.0] {
        let near = add(cut, scale(along, gap * direction));
        let far = add(cut, scale(along, (gap + stump) * direction));
        draw_vessel_run(ctx, near, far, vessel.radius * 0.8, 0.75);
        draw_circle(
            near.x as f32,
            near.y as f32,
            (vessel.radius * 0.75 + 0.6) as f32,
            ctx.palette.wound_core,
        );
    }
}

fn point_along(a: rp::Vec2, b: rp::Vec2, t: f64) -> rp::Vec2 {
    add(a, scale(sub(b, a), t.clamp(0.0, 1.0)))
}

#[derive(Clone, Copy)]
enum BonePass {
    Anatomy,
    /// Beneath the flesh in the normal view.
    Buried,
}

fn draw_bones(ctx: &RenderContext, pass: BonePass) {
    for bone in ctx.app.world.bones() {
        match pass {
            BonePass::Anatomy => draw_bone(ctx, bone, 1.0, true),
            BonePass::Buried => {
                // The spine runs down the back, behind everything a front
                // wound can open.
                if bone.kind == rp::BoneKind::Spine {
                    continue;
                }
                let broken =
                    bone.fractured || bone.splinter || bone.broken_start || bone.broken_end;
                draw_bone(ctx, bone, 1.0, broken);
            }
        }
    }
}

fn draw_bone(ctx: &RenderContext, bone: &rp::BoneSegment, alpha: f32, details: bool) {
    let width = (bone.radius * if details { 1.85 } else { 1.45 }).max(2.5) as f32;
    let shadow = with_alpha(ctx.palette.bone_shadow, alpha * 0.70);
    draw_line_vec(bone.a, bone.b, width + 4.0, shadow);
    let stroke = if bone.fractured || bone.splinter {
        ctx.palette.bone_fractured
    } else if bone.kind == rp::BoneKind::Rib {
        rgba(232, 207, 156, 255)
    } else {
        ctx.palette.bone
    };
    draw_line_vec(bone.a, bone.b, width, with_alpha(stroke, alpha));
    if details {
        let center = mid(bone.a, bone.b);
        let dir = normalized(sub(bone.b, bone.a), rp::Vec2 { x: 1.0, y: 0.0 });
        let normal = rp::Vec2 {
            x: -dir.y,
            y: dir.x,
        };
        draw_line_vec(
            add(center, scale(normal, -bone.radius * 0.45)),
            add(center, scale(normal, bone.radius * 0.45)),
            1.4,
            with_alpha(rgba(255, 250, 232, 255), alpha * 0.35),
        );
    }
    if details && bone.broken_start {
        draw_fracture_cap(ctx, bone, true);
    }
    if details && bone.broken_end {
        draw_fracture_cap(ctx, bone, false);
    }
}

fn draw_fracture_cap(ctx: &RenderContext, bone: &rp::BoneSegment, at_start: bool) {
    let p = if at_start { bone.a } else { bone.b };
    let dir = normalized(sub(bone.b, bone.a), rp::Vec2 { x: 1.0, y: 0.0 });
    let fallback = rp::Vec2 {
        x: -dir.y,
        y: dir.x,
    };
    let stored = if at_start {
        bone.broken_start_normal
    } else {
        bone.broken_end_normal
    };
    let normal = normalized(stored, fallback);
    let tip_dir = if at_start { scale(dir, -1.0) } else { dir };
    let cap = bone.radius * 1.28;

    draw_soft_circle(
        to_mq(p),
        (bone.radius * 1.15) as f32,
        3,
        with_alpha(ctx.palette.wound_core, 0.32),
    );
    draw_line_vec(
        add(p, scale(normal, -cap)),
        add(
            add(p, scale(normal, -cap * 0.22)),
            scale(tip_dir, bone.radius * 0.55),
        ),
        3.0,
        ctx.palette.bone_fractured,
    );
    draw_line_vec(
        add(
            add(p, scale(normal, -cap * 0.20)),
            scale(tip_dir, bone.radius * 0.48),
        ),
        add(
            add(p, scale(normal, cap * 0.24)),
            scale(tip_dir, -bone.radius * 0.15),
        ),
        3.0,
        ctx.palette.bone_fractured,
    );
    draw_line_vec(
        add(
            add(p, scale(normal, cap * 0.24)),
            scale(tip_dir, -bone.radius * 0.15),
        ),
        add(p, scale(normal, cap)),
        3.0,
        ctx.palette.bone_fractured,
    );
    draw_line_vec(
        add(p, scale(normal, -cap * 0.66)),
        add(p, scale(tip_dir, bone.radius * 0.88)),
        2.4,
        ctx.palette.blood_dark,
    );
    draw_line_vec(
        add(p, scale(normal, 0.12 * cap)),
        add(
            add(p, scale(tip_dir, bone.radius * 0.95)),
            scale(normal, cap * 0.38),
        ),
        2.2,
        ctx.palette.blood_fresh,
    );
    draw_line_vec(
        add(p, scale(normal, cap * 0.64)),
        add(p, scale(tip_dir, bone.radius * 0.55)),
        2.2,
        ctx.palette.blood_dark,
    );
}

fn draw_bone_attachments(ctx: &RenderContext) {
    let world = &ctx.app.world;
    for attachment in world.bone_attachments() {
        if attachment.broken
            || attachment.bone >= world.bones().len()
            || attachment.point >= world.points().len()
        {
            continue;
        }
        let anchor = bone_point(world.bones()[attachment.bone], attachment.t);
        let point = world.points()[attachment.point].position;
        draw_line_vec(anchor, point, 1.0, ctx.palette.attachment);
    }
}

/// Skin wounds drawn from the mesh itself. A cut is a line through the middle
/// of every severed skin spring, joined across each triangle it passes through,
/// so it follows the blade's path and fades as the cut pulls open. Where skin
/// has opened, a thin dark rim marks the edge of the intact skin around it.
fn draw_skin_wounds(ctx: &RenderContext) {
    let world = &ctx.app.world;
    let points = world.points();
    let springs = world.springs();
    let triangles = world.triangles();
    let opening: Vec<f32> = triangles
        .iter()
        .map(|triangle| {
            if triangle.layer == rp::TissueLayer::Skin {
                skin_opening(world, triangle)
            } else {
                1.0
            }
        })
        .collect();
    let line_color = mix(ctx.palette.wound_shadow, ctx.palette.wound_edge, 0.35);

    for (index, triangle) in triangles.iter().enumerate() {
        if triangle.layer != rp::TissueLayer::Skin || opening[index] >= 0.98 {
            continue;
        }
        let mut cut = [(rp::Vec2 { x: 0.0, y: 0.0 }, 0.0f32); 3];
        let mut count = 0;
        for edge in [triangle.edge_ab, triangle.edge_bc, triangle.edge_ca] {
            let Some(spring) = springs.get(edge).filter(|spring| spring.broken) else {
                continue;
            };
            // A torn spring gapes at once; only a clean cut reads as a line.
            let clean = 1.0 - smoothstep(1.3, 1.9, cut_gap(world, spring) as f32);
            cut[count] = (
                mid(points[spring.a].position, points[spring.b].position),
                clean,
            );
            count += 1;
        }
        let shown = 1.0 - opening[index];
        let centroid = scale(
            add(
                add(points[triangle.a].position, points[triangle.b].position),
                points[triangle.c].position,
            ),
            1.0 / 3.0,
        );
        let stroke = |a: rp::Vec2, b: rp::Vec2, alpha: f32| {
            if alpha > 0.03 {
                draw_line_vec(a, b, 2.0, with_alpha(line_color, 0.85 * alpha));
            }
        };
        match count {
            1 => stroke(cut[0].0, mid(cut[0].0, centroid), shown * cut[0].1),
            2 => stroke(cut[0].0, cut[1].0, shown * cut[0].1.min(cut[1].1)),
            3 => {
                for &(point, clean) in &cut {
                    stroke(point, centroid, shown * clean);
                }
            }
            _ => {}
        }
    }

    for (spring_index, sides) in ctx.app.skin_rim.edge_triangles.iter().enumerate() {
        if sides[1] == NO_TRIANGLE {
            continue;
        }
        let rim = (opening[sides[0]] - opening[sides[1]]).abs();
        if rim < 0.05 {
            continue;
        }
        let spring = springs[spring_index];
        draw_line_vec(
            points[spring.a].position,
            points[spring.b].position,
            1.6,
            with_alpha(ctx.palette.wound_shadow, 0.72 * rim),
        );
    }
}

fn draw_wound_sources(ctx: &RenderContext) {
    for wound in ctx.app.world.wounds() {
        if !wound.active {
            continue;
        }
        let pressure = (wound.pressure / 6.0).clamp(0.0, 1.0) as f32;
        let clot = wound.clot.clamp(0.0, 1.0) as f32;
        let radius = (wound.radius * (1.5 + wound.depth * 0.38)) as f32;
        let pos = to_mq(wound.position);
        draw_soft_circle(
            pos,
            radius + 3.0 + pressure * 9.0,
            4,
            with_alpha(ctx.palette.blood_dark, 0.14 + pressure * 0.24),
        );
        draw_circle(
            pos.x,
            pos.y,
            radius,
            with_alpha(
                mix(ctx.palette.blood_mid, ctx.palette.blood_fresh, pressure),
                0.84 - clot * 0.28,
            ),
        );
        // Only a spurting, high-pressure bleed gets a streak.
        if pressure > 0.3 {
            let dir = normalized(wound.direction, rp::Vec2 { x: 0.0, y: 1.0 });
            draw_line_vec(
                wound.position,
                add(wound.position, scale(dir, 8.0 + wound.pressure * 3.2)),
                1.4,
                with_alpha(ctx.palette.blood_fresh, 0.30 + pressure * 0.40),
            );
        }
    }
}

fn draw_effects(ctx: &RenderContext) {
    draw_blood_stains(ctx);
    draw_fluids(ctx);
}

fn draw_blood_stains(ctx: &RenderContext) {
    for stain in ctx.app.world.blood_stains() {
        if stain.intensity <= 0.025 {
            continue;
        }
        let intensity = stain.intensity.clamp(0.0, 1.75) as f32;
        let radius = stain.radius.max(1.0) as f32;
        let pos = to_mq(stain.position);
        draw_soft_circle(
            pos,
            radius * (1.16 + intensity * 0.10),
            5,
            with_alpha(ctx.palette.blood_stain, 0.16 + intensity * 0.14),
        );
        draw_circle(
            pos.x,
            pos.y,
            radius * (0.58 + intensity * 0.08),
            with_alpha(ctx.palette.blood_dark, 0.20 + intensity * 0.18),
        );
    }
}

fn draw_fluids(ctx: &RenderContext) {
    for fluid in ctx.app.world.fluids() {
        if fluid.life <= 0.0 {
            continue;
        }
        let fade = (fluid.life / fluid.max_life.max(0.1)).clamp(0.0, 1.0);
        let fade_f = fade as f32;
        let settled_darkening = if fluid.settled { 0.58 } else { 1.0 };
        let travel = sub(fluid.position, fluid.previous);
        let speed_alpha = (length(travel) / 18.0).clamp(0.0, 1.0) as f32;
        let color = Color::new(
            (0.22 + 0.54 * fluid.intensity as f32 * fade_f) * settled_darkening,
            (0.015 + 0.04 * fade_f) * settled_darkening,
            (0.025 + 0.06 * fade_f) * settled_darkening,
            (0.30 + 0.58 * fade_f).min(0.92),
        );
        if speed_alpha > 0.08 && !fluid.settled {
            draw_line_vec(
                fluid.previous,
                fluid.position,
                (fluid.radius * (0.72 + speed_alpha as f64 * 0.55)).max(1.0) as f32,
                with_alpha(ctx.palette.blood_dark, 0.20 + speed_alpha * 0.34),
            );
        }
        let radius = (fluid.radius * (0.82 + 0.32 * fade)).max(1.0) as f32;
        draw_circle(
            fluid.position.x as f32,
            fluid.position.y as f32,
            radius + 1.0,
            with_alpha(ctx.palette.blood_dark, color.a * 0.50),
        );
        draw_circle(
            fluid.position.x as f32,
            fluid.position.y as f32,
            radius,
            color,
        );
    }
}

fn draw_striker(ctx: &RenderContext) {
    let app = ctx.app;
    // The same pose and geometry the simulation collides with.
    if let Some(pose) = app.world.current_tool_pose() {
        let geometry = rp::tool_geometry(pose.tool);
        match pose.tool {
            rp::ToolMode::Sharp => draw_knife(ctx, &pose, &geometry),
            rp::ToolMode::Heavy => draw_sledgehammer(ctx, &pose, &geometry),
            rp::ToolMode::Blunt => draw_bat(ctx, &pose, &geometry),
        }
    }

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

/// Perpendicular to `dir`, rotated a quarter turn.
fn perpendicular(dir: rp::Vec2) -> rp::Vec2 {
    rp::Vec2 {
        x: -dir.y,
        y: dir.x,
    }
}

fn draw_polygon(points: &[rp::Vec2], color: Color) {
    for i in 1..points.len().saturating_sub(1) {
        draw_triangle(
            to_mq(points[0]),
            to_mq(points[i]),
            to_mq(points[i + 1]),
            color,
        );
    }
}

/// Double-edged blade with a center ridge, crossguard, wrapped grip, and pommel.
fn draw_knife(ctx: &RenderContext, pose: &rp::ToolPose, geometry: &rp::ToolGeometry) {
    let heading = pose.heading;
    let across = perpendicular(heading);
    let guard = pose.contact_start;
    let tip = pose.contact_end;
    let length = length(sub(tip, guard));
    let width = geometry.body_half_width;
    let shoulder = add(guard, scale(heading, length * 0.58));

    let blade = [
        add(guard, scale(across, width * 0.85)),
        add(shoulder, scale(across, width)),
        tip,
        sub(shoulder, scale(across, width)),
        sub(guard, scale(across, width * 0.85)),
    ];
    let steel = if ctx.app.pointer_down {
        rgba(214, 222, 226, 255)
    } else {
        rgba(186, 196, 201, 255)
    };
    draw_polygon(&blade, steel);
    // The far bevel catches less light than the near one.
    draw_polygon(
        &[
            guard,
            shoulder,
            tip,
            sub(shoulder, scale(across, width)),
            sub(guard, scale(across, width * 0.85)),
        ],
        rgba(146, 157, 164, 255),
    );
    if ctx.app.tool_blood > 0.02 {
        let bloodied = add(guard, scale(heading, length * 0.35));
        draw_polygon(
            &[
                add(bloodied, scale(across, width * 0.92)),
                add(shoulder, scale(across, width)),
                tip,
                sub(shoulder, scale(across, width)),
                sub(bloodied, scale(across, width * 0.92)),
            ],
            with_alpha(ctx.palette.blood_mid, ctx.app.tool_blood * 0.8),
        );
    }
    draw_line_vec(guard, tip, 1.2, rgba(236, 242, 244, 220));
    draw_polyline_closed(&blade, 1.2, rgba(52, 58, 62, 255));

    let guard_half = width + 5.0;
    draw_line_vec(
        add(guard, scale(across, guard_half)),
        sub(guard, scale(across, guard_half)),
        5.0,
        rgba(64, 66, 70, 255),
    );
    draw_line_vec(
        add(guard, scale(across, guard_half - 1.0)),
        sub(guard, scale(across, guard_half - 1.0)),
        1.4,
        rgba(150, 154, 158, 255),
    );

    let grip_start = sub(guard, scale(heading, 2.5));
    let grip_end = sub(guard, scale(heading, 2.5 + geometry.handle_length));
    draw_line_vec(
        grip_start,
        grip_end,
        (geometry.handle_half_width * 2.0) as f32,
        rgba(44, 32, 26, 255),
    );
    let handle_half = geometry.handle_half_width;
    for i in 1..6 {
        let at = sub(
            grip_start,
            scale(heading, geometry.handle_length * i as f64 / 6.0),
        );
        draw_line_vec(
            add(at, scale(across, handle_half)),
            sub(at, scale(across, handle_half)),
            1.0,
            rgba(92, 70, 54, 255),
        );
    }
    draw_circle(
        grip_end.x as f32,
        grip_end.y as f32,
        (handle_half + 1.5) as f32,
        rgba(118, 122, 126, 255),
    );
}

/// Steel head with polished striking faces on a long wooden handle.
fn draw_sledgehammer(ctx: &RenderContext, pose: &rp::ToolPose, geometry: &rp::ToolGeometry) {
    let heading = pose.heading;
    let side = pose.side;
    let center = pose.center;
    let half_length = length(sub(pose.contact_end, pose.contact_start)) * 0.5 + pose.contact_radius;
    let half_width = geometry.body_half_width;

    // Handle first, so the head covers the joint.
    let handle_start = add(center, scale(side, half_width * 0.6));
    let handle_end = add(center, scale(side, half_width + geometry.handle_length));
    let handle_width = (geometry.handle_half_width * 2.0) as f32;
    draw_line_vec(
        handle_start,
        handle_end,
        handle_width + 2.0,
        rgba(58, 40, 26, 255),
    );
    draw_line_vec(
        handle_start,
        handle_end,
        handle_width,
        rgba(156, 112, 64, 255),
    );
    draw_line_vec(
        add(handle_start, scale(heading, -1.5)),
        add(handle_end, scale(heading, -1.5)),
        1.2,
        rgba(196, 150, 96, 200),
    );
    let wrap_start = add(
        center,
        scale(side, half_width + geometry.handle_length * 0.72),
    );
    draw_line_vec(
        wrap_start,
        handle_end,
        handle_width + 1.0,
        rgba(44, 36, 30, 255),
    );

    let corner = |along: f64, out: f64| add(center, add(scale(heading, along), scale(side, out)));
    let head = [
        corner(half_length, -half_width),
        corner(half_length, half_width),
        corner(-half_length, half_width),
        corner(-half_length, -half_width),
    ];
    draw_quad(head, rgba(74, 78, 83, 255));
    // Light falls on the side away from the handle.
    draw_quad(
        [
            corner(half_length - 2.0, -half_width + 2.0),
            corner(half_length - 2.0, -half_width * 0.15),
            corner(-half_length + 2.0, -half_width * 0.15),
            corner(-half_length + 2.0, -half_width + 2.0),
        ],
        rgba(118, 124, 130, 255),
    );
    for face in [half_length, -half_length] {
        let inset = if face > 0.0 { -4.0 } else { 4.0 };
        draw_quad(
            [
                corner(face, -half_width),
                corner(face, half_width),
                corner(face + inset, half_width),
                corner(face + inset, -half_width),
            ],
            rgba(168, 174, 178, 255),
        );
    }
    if ctx.app.tool_blood > 0.02 {
        // Blood collects on the striking face that leads.
        let face = half_length;
        draw_quad(
            [
                corner(face, -half_width),
                corner(face, half_width),
                corner(face - 12.0, half_width),
                corner(face - 12.0, -half_width),
            ],
            with_alpha(ctx.palette.blood_mid, ctx.app.tool_blood * 0.75),
        );
    }
    draw_polyline_closed(&head, 2.0, rgba(22, 23, 25, 255));
}

/// Ash bat: full barrel where it hits, tapering to a taped handle and knob.
fn draw_bat(ctx: &RenderContext, pose: &rp::ToolPose, geometry: &rp::ToolGeometry) {
    let side = pose.side;
    let center = pose.center;
    let radius = geometry.body_half_width;
    let along = |distance: f64| add(center, scale(side, distance));
    // Positions along the bat from the driven point toward the hand.
    let end = -(geometry.contact_front + radius * 0.5);
    let barrel_end = geometry.contact_back + 2.0;
    let taper_end = barrel_end + 62.0;
    let knob = barrel_end + geometry.handle_length - 30.0;
    let handle_radius = geometry.handle_half_width;

    let wood = rgba(212, 176, 122, 255);
    let wood_dark = rgba(96, 70, 44, 255);
    let across = perpendicular(side);
    let outline = [
        add(along(end), scale(across, radius)),
        add(along(barrel_end), scale(across, radius)),
        add(along(taper_end), scale(across, handle_radius)),
        add(along(knob), scale(across, handle_radius)),
        sub(along(knob), scale(across, handle_radius)),
        sub(along(taper_end), scale(across, handle_radius)),
        sub(along(barrel_end), scale(across, radius)),
        sub(along(end), scale(across, radius)),
    ];
    draw_polygon(&outline, wood);
    let cap = along(end);
    draw_circle(cap.x as f32, cap.y as f32, radius as f32, wood);
    draw_polyline_closed(&outline[..], 1.4, wood_dark);
    draw_circle_lines(cap.x as f32, cap.y as f32, radius as f32, 1.4, wood_dark);
    draw_line_vec(
        add(along(end), scale(across, radius * 0.45)),
        add(along(barrel_end), scale(across, radius * 0.45)),
        2.0,
        rgba(236, 210, 164, 200),
    );
    draw_line_vec(
        sub(along(end + 6.0), scale(across, radius * 0.4)),
        sub(along(barrel_end - 8.0), scale(across, radius * 0.4)),
        1.0,
        rgba(170, 132, 84, 200),
    );
    if ctx.app.tool_blood > 0.02 {
        draw_polygon(
            &[
                add(along(end), scale(across, radius)),
                add(along(end + 52.0), scale(across, radius)),
                sub(along(end + 52.0), scale(across, radius)),
                sub(along(end), scale(across, radius)),
            ],
            with_alpha(ctx.palette.blood_mid, ctx.app.tool_blood * 0.7),
        );
    }

    let tape_start = taper_end + 4.0;
    draw_line_vec(
        along(tape_start),
        along(knob),
        (handle_radius * 2.0 + 1.0) as f32,
        rgba(38, 38, 42, 255),
    );
    let mut wrap = tape_start + 5.0;
    while wrap < knob - 2.0 {
        draw_line_vec(
            add(along(wrap), scale(across, handle_radius)),
            sub(along(wrap + 3.0), scale(across, handle_radius)),
            1.0,
            rgba(84, 84, 90, 255),
        );
        wrap += 6.0;
    }
    let knob_center = along(knob + 2.0);
    draw_circle(
        knob_center.x as f32,
        knob_center.y as f32,
        (handle_radius + 3.0) as f32,
        wood,
    );
    draw_circle_lines(
        knob_center.x as f32,
        knob_center.y as f32,
        (handle_radius + 3.0) as f32,
        1.4,
        wood_dark,
    );
}

fn draw_hud(ctx: &RenderContext) {
    let stats = ctx.app.world.stats();
    let view = if ctx.anatomy { "ANATOMY" } else { "NORMAL" };
    let running = if ctx.app.running { "LIVE" } else { "PAUSED" };
    let items = [
        format!("{view}"),
        format!("{}", tool_name(ctx.app.tool).to_uppercase()),
        format!("MASS {:.0}X", ctx.app.impact_power),
        format!("{running}"),
        format!("SKIN {}", stats.broken_skin),
        format!("MUSCLE {}", stats.broken_muscle),
        format!("BONE {}", stats.fractured_bones),
        format!("FLUID {}", stats.emitted_fluid_particles),
    ];

    let margin = 14.0;
    let mut x = margin;
    let mut y = margin;
    for (index, item) in items.iter().enumerate() {
        let width = chip_width(item);
        if x > margin && x + width > ctx.width - margin {
            x = margin;
            y += 31.0;
        }
        let accent = match index {
            0 => {
                if ctx.anatomy {
                    ctx.palette.tool_accent
                } else {
                    ctx.palette.hud_muted
                }
            }
            1 => tool_color(ctx.app.tool),
            3 => {
                if ctx.app.running {
                    rgba(94, 176, 108, 230)
                } else {
                    rgba(211, 93, 70, 230)
                }
            }
            4 | 5 | 6 | 7 => ctx.palette.blood_fresh,
            _ => ctx.palette.hud_border,
        };
        draw_chip(
            x,
            y,
            item,
            accent,
            ctx.palette.hud_text,
            ctx.palette.hud_back,
        );
        x += width + 7.0;
    }
}

#[derive(Clone, Copy)]
struct ControlHint {
    key: &'static str,
    label: &'static str,
    accent: Color,
    active: bool,
    /// What tapping or clicking the hint does; `None` for instruction-only hints.
    action: Option<ControlAction>,
}

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

fn control_hints(app: &AppState, palette: &RenderPalette) -> [ControlHint; 9] {
    [
        ControlHint {
            key: "DRAG",
            label: "strike",
            accent: palette.tool_accent,
            active: app.pointer_down,
            action: None,
        },
        ControlHint {
            key: "B",
            label: "bat",
            accent: tool_color(rp::ToolMode::Blunt),
            active: app.tool == rp::ToolMode::Blunt,
            action: Some(ControlAction::Tool(rp::ToolMode::Blunt)),
        },
        ControlHint {
            key: "S",
            label: "knife",
            accent: tool_color(rp::ToolMode::Sharp),
            active: app.tool == rp::ToolMode::Sharp,
            action: Some(ControlAction::Tool(rp::ToolMode::Sharp)),
        },
        ControlHint {
            key: "H",
            label: "hammer",
            accent: tool_color(rp::ToolMode::Heavy),
            active: app.tool == rp::ToolMode::Heavy,
            action: Some(ControlAction::Tool(rp::ToolMode::Heavy)),
        },
        ControlHint {
            key: "TAB",
            label: "view",
            accent: palette.tool_accent,
            active: app.view_mode == ViewMode::Anatomy,
            action: Some(ControlAction::ToggleView),
        },
        ControlHint {
            key: "D",
            label: "debug",
            accent: rgba(94, 176, 108, 230),
            active: app.debug_overlay,
            action: Some(ControlAction::ToggleDebug),
        },
        ControlHint {
            key: "SPACE",
            label: "pause",
            accent: rgba(211, 93, 70, 230),
            active: !app.running,
            action: Some(ControlAction::TogglePause),
        },
        ControlHint {
            key: "R",
            label: "reset",
            accent: palette.hud_border,
            active: false,
            action: Some(ControlAction::Reset),
        },
        ControlHint {
            key: "1 2 4",
            label: mass_label(app.impact_power),
            accent: palette.hud_border,
            active: false,
            action: Some(ControlAction::CycleMass),
        },
    ]
}

fn mass_label(power: f64) -> &'static str {
    if power >= 4.0 {
        "mass 4x"
    } else if power >= 2.0 {
        "mass 2x"
    } else {
        "mass 1x"
    }
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
    let layout = layout_control_hints(&hints, ctx.width, ctx.floor_y, ctx.app.touch_ui);
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
    let label = measure_text(hint.label, None, 15, 1.0);
    key.width + label.width + 32.0
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
    draw_text(
        hint.label,
        rect.x + key_width + 11.0,
        text_y,
        15.0,
        ctx.palette.hud_muted,
    );
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
            "damage  skin={} muscle={} fiber={} prop={} deep={} crush={} flaps={} vessels={} attach={}/{} joints={}",
            stats.broken_skin,
            stats.broken_muscle,
            stats.muscle_fiber_tears,
            stats.tear_propagations,
            stats.muscle_cut_transfers,
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
    draw_rectangle(x, y, width, 25.0, back);
    draw_rectangle_lines(x, y, width, 25.0, 1.0, with_alpha(accent, 0.58));
    draw_rectangle(x, y, 4.0, 25.0, accent);
    draw_text(label, x + 10.0, y + 17.0, 17.0, text);
}

fn fill_triangle(world: &rp::World, triangle: &rp::Triangle, color: Color) {
    let points = world.points();
    draw_triangle(
        to_mq(points[triangle.a].position),
        to_mq(points[triangle.b].position),
        to_mq(points[triangle.c].position),
        color,
    );
}

fn outline_triangle(world: &rp::World, triangle: &rp::Triangle, color: Color, width: f32) {
    let points = world.points();
    let a = points[triangle.a].position;
    let b = points[triangle.b].position;
    let c = points[triangle.c].position;
    draw_line_vec(a, b, width, color);
    draw_line_vec(b, c, width, color);
    draw_line_vec(c, a, width, color);
}

fn triangle_point_metrics(world: &rp::World, triangle: &rp::Triangle) -> (f64, f64) {
    let a = world.points()[triangle.a];
    let b = world.points()[triangle.b];
    let c = world.points()[triangle.c];
    (
        (a.load + b.load + c.load) / 3.0,
        (a.exposure + b.exposure + c.exposure) / 3.0,
    )
}

fn triangle_point_contusion(world: &rp::World, triangle: &rp::Triangle) -> f64 {
    let a = world.points()[triangle.a];
    let b = world.points()[triangle.b];
    let c = world.points()[triangle.c];
    (a.contusion + b.contusion + c.contusion) / 3.0
}

fn draw_quad(points: [rp::Vec2; 4], color: Color) {
    draw_triangle(to_mq(points[0]), to_mq(points[1]), to_mq(points[2]), color);
    draw_triangle(to_mq(points[0]), to_mq(points[2]), to_mq(points[3]), color);
}

fn draw_polyline_closed(points: &[rp::Vec2], width: f32, color: Color) {
    if points.len() < 2 {
        return;
    }
    for i in 0..points.len() {
        draw_line_vec(points[i], points[(i + 1) % points.len()], width, color);
    }
}

fn draw_soft_circle(center: Vec2, radius: f32, rings: usize, color: Color) {
    for i in (0..rings).rev() {
        let t = (i + 1) as f32 / rings as f32;
        let mut ring = color;
        ring.a *= (1.0 - t * 0.72).max(0.08);
        draw_circle(center.x, center.y, radius * t, ring);
    }
}

fn draw_line_vec(a: rp::Vec2, b: rp::Vec2, width: f32, color: Color) {
    draw_line(a.x as f32, a.y as f32, b.x as f32, b.y as f32, width, color);
}

fn tool_name(tool: rp::ToolMode) -> &'static str {
    match tool {
        rp::ToolMode::Blunt => "bat",
        rp::ToolMode::Sharp => "knife",
        rp::ToolMode::Heavy => "hammer",
    }
}

fn tool_color(tool: rp::ToolMode) -> Color {
    match tool {
        rp::ToolMode::Blunt => rgba(211, 167, 91, 235),
        rp::ToolMode::Sharp => rgba(192, 220, 224, 235),
        rp::ToolMode::Heavy => rgba(132, 141, 147, 235),
    }
}

fn bone_point(bone: rp::BoneSegment, t: f64) -> rp::Vec2 {
    rp::Vec2 {
        x: bone.a.x + (bone.b.x - bone.a.x) * t,
        y: bone.a.y + (bone.b.y - bone.a.y) * t,
    }
}

fn normalized(value: rp::Vec2, fallback: rp::Vec2) -> rp::Vec2 {
    let len = length(value);
    if len <= 0.0001 {
        fallback
    } else {
        scale(value, 1.0 / len)
    }
}

fn length(value: rp::Vec2) -> f64 {
    (value.x * value.x + value.y * value.y).sqrt()
}

fn add(a: rp::Vec2, b: rp::Vec2) -> rp::Vec2 {
    rp::Vec2 {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}

fn sub(a: rp::Vec2, b: rp::Vec2) -> rp::Vec2 {
    rp::Vec2 {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

fn scale(value: rp::Vec2, amount: f64) -> rp::Vec2 {
    rp::Vec2 {
        x: value.x * amount,
        y: value.y * amount,
    }
}

fn mid(a: rp::Vec2, b: rp::Vec2) -> rp::Vec2 {
    scale(add(a, b), 0.5)
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

fn with_alpha(mut color: Color, alpha: f32) -> Color {
    color.a = alpha.clamp(0.0, 1.0);
    color
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba(r, g, b, a)
}

fn to_mq(value: rp::Vec2) -> Vec2 {
    vec2(value.x as f32, value.y as f32)
}
