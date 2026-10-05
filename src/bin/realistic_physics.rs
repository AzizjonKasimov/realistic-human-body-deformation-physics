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

struct StrikerDriveProfile {
    down_drive: f64,
    idle_drive: f64,
    down_damping: f64,
    idle_damping: f64,
    max_speed: f64,
}

impl Default for StrikerDriveProfile {
    fn default() -> Self {
        Self {
            down_drive: 118.0,
            idle_drive: 62.0,
            down_damping: 15.0,
            idle_damping: 20.0,
            max_speed: 4200.0,
        }
    }
}

struct AppState {
    world: rp::World,
    running: bool,
    pointer_down: bool,
    debug_overlay: bool,
    accumulator: f64,
    pointer_initialized: bool,
    pointer: rp::Vec2,
    striker: rp::Vec2,
    striker_velocity: rp::Vec2,
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
}

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
            striker: initial_pointer,
            striker_velocity: rp::Vec2 { x: 0.0, y: 0.0 },
            impact_power: 2.0,
            tool: rp::ToolMode::Blunt,
            view_mode: ViewMode::Anatomy,
            ui_capture: false,
            ui_release: None,
            touch_ui: false,
            skin_rim,
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
    muscle_fiber: Color,
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
    tool_handle_dark: Color,
    tool_handle_light: Color,
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
        ControlAction::Tool(tool) => app.tool = tool,
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
            app.striker = app.pointer;
            app.striker_velocity = rp::Vec2 { x: 0.0, y: 0.0 };
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
        advance_striker(app, fixed_dt);
        let input = rp::InputState {
            active: true,
            down: app.pointer_down,
            x: app.striker.x,
            y: app.striker.y,
            vx: app.striker_velocity.x,
            vy: app.striker_velocity.y,
            power: app.impact_power,
            tool: app.tool,
        };
        app.world.step(
            fixed_dt,
            &input,
            screen_width() as f64,
            screen_height() as f64,
        );
        app.accumulator -= fixed_dt;
    }
}

fn advance_striker(app: &mut AppState, dt: f64) {
    let dx = app.pointer.x - app.striker.x;
    let dy = app.pointer.y - app.striker.y;
    let profile = striker_drive_profile(app.tool);
    let drive = if app.pointer_down {
        profile.down_drive
    } else {
        profile.idle_drive
    };
    let damping = if app.pointer_down {
        profile.down_damping
    } else {
        profile.idle_damping
    };

    app.striker_velocity.x += (dx * drive - app.striker_velocity.x * damping) * dt;
    app.striker_velocity.y += (dy * drive - app.striker_velocity.y * damping) * dt;
    let speed = length(app.striker_velocity);
    if speed > profile.max_speed {
        let scale = profile.max_speed / speed;
        app.striker_velocity.x *= scale;
        app.striker_velocity.y *= scale;
    }
    app.striker.x += app.striker_velocity.x * dt;
    app.striker.y += app.striker_velocity.y * dt;
}

fn striker_drive_profile(tool: rp::ToolMode) -> StrikerDriveProfile {
    match tool {
        rp::ToolMode::Sharp => StrikerDriveProfile {
            down_drive: 132.0,
            idle_drive: 70.0,
            down_damping: 13.0,
            idle_damping: 18.0,
            max_speed: 4600.0,
        },
        rp::ToolMode::Heavy => StrikerDriveProfile {
            down_drive: 74.0,
            idle_drive: 42.0,
            down_damping: 22.0,
            idle_damping: 28.0,
            max_speed: 3200.0,
        },
        rp::ToolMode::Blunt => StrikerDriveProfile::default(),
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
        muscle_fiber: rgba(235, 82, 79, 218),
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
        tool_handle_dark: rgba(62, 47, 34, 255),
        tool_handle_light: rgba(164, 132, 82, 255),
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

    if !ctx.anatomy {
        draw_bones(ctx, BonePass::Subsurface);
    }

    draw_muscle_layer(ctx);
    if ctx.anatomy {
        draw_major_vessels(ctx);
    }
    draw_skin_layer(ctx);
    draw_exposed_tissue_detail(ctx);

    if ctx.anatomy {
        draw_bone_attachments(ctx);
        draw_bones(ctx, BonePass::Anatomy);
    } else {
        draw_bones(ctx, BonePass::ExposedDamage);
    }

    draw_wound_edges(ctx);
    if !ctx.anatomy {
        draw_major_vessels(ctx);
    }
    draw_wound_sources(ctx);

    let debug = world.debug();
    if debug.max_depth > 0.0 {
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

        if triangle.damage > 0.12 || exposure > 0.35 {
            outline_triangle(
                world,
                triangle,
                with_alpha(
                    ctx.palette.wound_edge,
                    (0.20 + heat as f32 * 0.30).min(0.55),
                ),
                1.0,
            );
        }
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
    for (width, alpha, reach_share) in [(24.0, 0.18, 0.5), (9.0, 0.45, 0.42)] {
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
                let depth = (width as f64).min(rim.reach[index] as f64 * reach_share);
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
    for triangle in world.triangles() {
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
    SkinRim { edges, reach }
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

fn draw_exposed_tissue_detail(ctx: &RenderContext) {
    let world = &ctx.app.world;
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Muscle {
            continue;
        }
        let (load, exposure) = triangle_point_metrics(world, triangle);
        if world.triangle_alive(triangle) {
            let detail = (triangle.damage * 0.75 + exposure * 0.85 + load / 1800.0).clamp(0.0, 1.0);
            if detail > 0.18 {
                draw_muscle_fibers(ctx, triangle, detail as f32);
            }
        } else if exposure > 0.22 || load > 260.0 || triangle.damage > 0.72 {
            draw_failed_muscle_void(ctx, triangle, exposure, load);
        }
    }
}

fn draw_muscle_fibers(ctx: &RenderContext, triangle: &rp::Triangle, detail: f32) {
    let points = ctx.app.world.points();
    let a = points[triangle.a].position;
    let b = points[triangle.b].position;
    let c = points[triangle.c].position;
    let centroid = scale(add(add(a, b), c), 1.0 / 3.0);
    let edges = [(a, b), (b, c), (c, a)];
    let mut longest = edges[0];
    let mut longest_len = length(sub(longest.1, longest.0));
    for edge in edges.iter().skip(1) {
        let len = length(sub(edge.1, edge.0));
        if len > longest_len {
            longest = *edge;
            longest_len = len;
        }
    }
    if longest_len < 6.0 {
        return;
    }
    let fiber_dir = normalized(sub(longest.1, longest.0), rp::Vec2 { x: 1.0, y: 0.0 });
    let normal = rp::Vec2 {
        x: -fiber_dir.y,
        y: fiber_dir.x,
    };
    let span = longest_len * (0.18 + f64::from(detail) * 0.24);
    let rows = if detail > 0.68 { 3 } else { 2 };
    for row in 0..rows {
        let row_t = if rows == 1 {
            0.0
        } else {
            row as f64 / (rows - 1) as f64 - 0.5
        };
        let center = add(centroid, scale(normal, row_t * longest_len * 0.18));
        let trim = 0.72 - f64::from(detail) * 0.16;
        let start = sub(center, scale(fiber_dir, span * trim));
        let end = add(center, scale(fiber_dir, span));
        draw_line_vec(
            start,
            end,
            0.8 + detail * 1.2,
            with_alpha(ctx.palette.muscle_fiber, 0.16 + detail * 0.38),
        );
    }
}

fn draw_failed_muscle_void(ctx: &RenderContext, triangle: &rp::Triangle, exposure: f64, load: f64) {
    let intensity = (exposure * 0.55 + triangle.damage * 0.45 + load / 2200.0).clamp(0.0, 1.0);
    fill_triangle(
        &ctx.app.world,
        triangle,
        with_alpha(
            ctx.palette.wound_shadow,
            (0.12 + intensity as f32 * 0.30).min(0.46),
        ),
    );
    outline_triangle(
        &ctx.app.world,
        triangle,
        with_alpha(
            ctx.palette.wound_edge,
            (0.18 + intensity as f32 * 0.36).min(0.58),
        ),
        1.1,
    );
}

fn draw_major_vessels(ctx: &RenderContext) {
    for vessel in ctx.app.world.vessels() {
        if !ctx.anatomy && !vessel.lacerated {
            continue;
        }
        let opacity = if vessel.lacerated {
            0.82
        } else if ctx.anatomy {
            0.34
        } else {
            0.0
        };
        if opacity <= 0.0 {
            continue;
        }
        draw_line_vec(
            vessel.a,
            vessel.b,
            (vessel.radius * 2.5 + 2.0) as f32,
            with_alpha(ctx.palette.major_vessel_shadow, opacity * 0.64),
        );
        draw_line_vec(
            vessel.a,
            vessel.b,
            (vessel.radius * 1.35 + 0.8) as f32,
            with_alpha(ctx.palette.major_vessel, opacity),
        );
        if vessel.lacerated {
            let center = mid(vessel.a, vessel.b);
            draw_soft_circle(
                to_mq(center),
                (vessel.radius * 3.8 + 7.0) as f32,
                4,
                with_alpha(ctx.palette.blood_fresh, 0.18),
            );
        }
    }
}

#[derive(Clone, Copy)]
enum BonePass {
    Anatomy,
    Subsurface,
    ExposedDamage,
}

fn draw_bones(ctx: &RenderContext, pass: BonePass) {
    for bone in ctx.app.world.bones() {
        match pass {
            BonePass::Anatomy => draw_bone(ctx, bone, 1.0, true),
            BonePass::Subsurface => {
                if !bone.fractured && !bone.splinter {
                    draw_bone(ctx, bone, 0.20, false);
                }
            }
            BonePass::ExposedDamage => {
                if bone.fractured || bone.splinter || bone.broken_start || bone.broken_end {
                    draw_bone(ctx, bone, 0.95, true);
                }
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

fn draw_wound_edges(ctx: &RenderContext) {
    let world = &ctx.app.world;
    for spring in world.springs() {
        if !spring.broken || spring.layer != rp::TissueLayer::Skin {
            continue;
        }
        if spring.a >= world.points().len() || spring.b >= world.points().len() {
            continue;
        }
        draw_wound_edge(ctx, world.points()[spring.a], world.points()[spring.b]);
    }
}

fn draw_wound_edge(ctx: &RenderContext, a: rp::Point, b: rp::Point) {
    let delta = sub(b.position, a.position);
    let len = length(delta);
    if len < 2.0 {
        return;
    }
    let dir = scale(delta, 1.0 / len);
    let normal = rp::Vec2 {
        x: -dir.y,
        y: dir.x,
    };
    let mark = (len * 0.19).clamp(4.0, 9.0);
    let inset = (len * 0.14).clamp(2.0, 8.0);
    let a_mid = add(a.position, scale(dir, inset));
    let b_mid = sub(b.position, scale(dir, inset));
    let exposure = a.exposure.max(b.exposure).clamp(0.0, 1.0);
    let load = a.load.max(b.load);
    let severity = (exposure * 0.58 + load / 1700.0).clamp(0.0, 1.0) as f32;

    draw_line_vec(
        add(a_mid, scale(normal, -mark)),
        add(a_mid, scale(normal, mark)),
        4.0 + severity * 1.8,
        with_alpha(ctx.palette.wound_shadow, 0.58 + severity * 0.30),
    );
    draw_line_vec(
        add(a_mid, scale(normal, -mark * 0.72)),
        add(a_mid, scale(normal, mark * 0.72)),
        2.0 + severity * 0.8,
        with_alpha(ctx.palette.wound_edge, 0.68 + severity * 0.24),
    );
    draw_line_vec(
        add(b_mid, scale(normal, -mark)),
        add(b_mid, scale(normal, mark)),
        4.0 + severity * 1.8,
        with_alpha(ctx.palette.wound_shadow, 0.58 + severity * 0.30),
    );
    draw_line_vec(
        add(b_mid, scale(normal, -mark * 0.72)),
        add(b_mid, scale(normal, mark * 0.72)),
        2.0 + severity * 0.8,
        with_alpha(
            mix(ctx.palette.wound_edge, ctx.palette.blood_fresh, severity),
            0.66 + severity * 0.28,
        ),
    );
    let tear_center = mid(a.position, b.position);
    draw_line_vec(
        sub(tear_center, scale(dir, len * 0.24)),
        add(tear_center, scale(dir, len * 0.24)),
        1.0 + severity * 1.1,
        with_alpha(ctx.palette.wound_core, 0.44 + severity * 0.34),
    );
    if severity > 0.28 {
        let fiber_count = if severity > 0.68 { 3 } else { 2 };
        for i in 0..fiber_count {
            let t = (i + 1) as f64 / (fiber_count + 1) as f64;
            let base = add(a.position, scale(delta, t));
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let start = add(base, scale(normal, side * mark * 0.20));
            let end = add(
                base,
                scale(normal, side * mark * (0.62 + f64::from(severity) * 0.36)),
            );
            draw_line_vec(
                start,
                end,
                0.8 + severity * 0.7,
                with_alpha(ctx.palette.muscle_fiber, 0.30 + severity * 0.34),
            );
        }
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
            radius + 7.0 + pressure * 7.0,
            4,
            with_alpha(ctx.palette.blood_dark, 0.22 + pressure * 0.18),
        );
        draw_circle(
            pos.x,
            pos.y,
            radius + 2.0,
            with_alpha(ctx.palette.wound_core, 0.58),
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
        let dir = normalized(wound.direction, rp::Vec2 { x: 0.0, y: 1.0 });
        draw_line_vec(
            wound.position,
            add(wound.position, scale(dir, 8.0 + wound.pressure * 3.2)),
            1.4,
            with_alpha(ctx.palette.blood_fresh, 0.44 + pressure * 0.28),
        );
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
    let radius = app.world.debug().striker_radius.max(tool_radius(app.tool)) as f32;
    let dir = striker_direction(app);
    let normal = rp::Vec2 {
        x: -dir.y,
        y: dir.x,
    };
    let striker = app.striker;
    let pointer = app.pointer;
    let target_delta = sub(pointer, striker);
    let target_distance = length(target_delta);
    let handle_end = if target_distance > f64::from(radius) * 0.65 {
        add(
            striker,
            scale(normalized(target_delta, dir), f64::from(radius) * 0.72),
        )
    } else {
        sub(striker, scale(dir, f64::from(radius) * 0.55))
    };
    let handle_start = if target_distance > f64::from(radius) * 0.65 {
        pointer
    } else {
        sub(striker, scale(dir, f64::from(radius) + 58.0))
    };

    draw_line_vec(handle_start, handle_end, 10.0, ctx.palette.tool_handle_dark);
    draw_line_vec(handle_start, handle_end, 4.0, ctx.palette.tool_handle_light);
    let pointer_radius = if app.pointer_down { 5.0 } else { 4.0 };
    draw_circle(
        pointer.x as f32,
        pointer.y as f32,
        pointer_radius,
        if app.pointer_down {
            ctx.palette.tool_accent
        } else {
            rgba(130, 119, 96, 235)
        },
    );
    draw_circle_lines(
        pointer.x as f32,
        pointer.y as f32,
        pointer_radius + 1.0,
        1.0,
        rgba(24, 20, 17, 230),
    );

    draw_impact_arrow(ctx, dir, radius);

    match app.tool {
        rp::ToolMode::Sharp => draw_sharp_tool(ctx, striker, dir, normal, radius),
        rp::ToolMode::Heavy => draw_heavy_tool(ctx, striker, dir, normal, radius),
        rp::ToolMode::Blunt => draw_blunt_tool(ctx, striker, dir, radius),
    }
}

fn draw_impact_arrow(ctx: &RenderContext, dir: rp::Vec2, radius: f32) {
    let app = ctx.app;
    let speed = length(app.striker_velocity);
    if !app.pointer_down || speed <= 80.0 {
        return;
    }
    let arrow_length = (speed * 0.030).clamp(18.0, 82.0);
    let start = add(app.striker, scale(dir, f64::from(radius) * 0.35));
    let end = add(app.striker, scale(dir, f64::from(radius) + arrow_length));
    let normal = rp::Vec2 {
        x: -dir.y,
        y: dir.x,
    };
    draw_line_vec(start, end, 3.0, ctx.palette.tool_accent);
    draw_triangle(
        to_mq(end),
        to_mq(add(sub(end, scale(dir, 12.0)), scale(normal, 6.0))),
        to_mq(sub(sub(end, scale(dir, 12.0)), scale(normal, 6.0))),
        ctx.palette.tool_accent,
    );
}

fn draw_sharp_tool(
    ctx: &RenderContext,
    center: rp::Vec2,
    dir: rp::Vec2,
    normal: rp::Vec2,
    radius: f32,
) {
    let r = f64::from(radius);
    let tip = add(center, scale(dir, r * 1.58));
    let spine = sub(center, scale(dir, r * 0.65));
    let waist = sub(center, scale(dir, r * 0.18));
    let blade = [
        tip,
        add(spine, scale(normal, r * 0.55)),
        waist,
        sub(spine, scale(normal, r * 0.55)),
    ];
    draw_quad(
        blade,
        if ctx.app.pointer_down {
            rgba(218, 228, 228, 255)
        } else {
            rgba(160, 177, 178, 245)
        },
    );
    draw_polyline_closed(&blade, 2.0, rgba(47, 55, 58, 255));
    draw_line_vec(
        add(spine, scale(normal, r * 0.62)),
        sub(spine, scale(normal, r * 0.62)),
        5.0,
        rgba(76, 48, 31, 255),
    );
    draw_line_vec(
        add(tip, scale(normal, -r * 0.08)),
        sub(spine, scale(normal, r * 0.34)),
        1.5,
        rgba(255, 255, 246, 190),
    );
}

fn draw_heavy_tool(
    ctx: &RenderContext,
    center: rp::Vec2,
    dir: rp::Vec2,
    normal: rp::Vec2,
    radius: f32,
) {
    let r = f64::from(radius);
    let half_width = r * 0.96;
    let half_height = r * 0.58;
    let head = [
        add(
            add(center, scale(normal, half_width)),
            scale(dir, half_height),
        ),
        add(
            sub(center, scale(normal, half_width)),
            scale(dir, half_height),
        ),
        sub(
            sub(center, scale(normal, half_width)),
            scale(dir, half_height),
        ),
        sub(
            add(center, scale(normal, half_width)),
            scale(dir, half_height),
        ),
    ];
    draw_quad(
        head,
        if ctx.app.pointer_down {
            rgba(74, 80, 84, 255)
        } else {
            rgba(91, 92, 90, 255)
        },
    );
    draw_polyline_closed(&head, 3.0, rgba(18, 19, 21, 255));
    draw_line_vec(
        sub(center, scale(normal, half_width * 0.52)),
        add(center, scale(normal, half_width * 0.52)),
        3.0,
        rgba(152, 157, 154, 230),
    );
}

fn draw_blunt_tool(ctx: &RenderContext, center: rp::Vec2, dir: rp::Vec2, radius: f32) {
    let shell = rgba(31, 27, 24, 255);
    let fill = if ctx.app.pointer_down {
        rgba(181, 51, 40, 255)
    } else {
        rgba(190, 164, 109, 255)
    };
    draw_circle(center.x as f32, center.y as f32, radius + 4.0, shell);
    draw_circle(center.x as f32, center.y as f32, radius, fill);
    draw_circle_lines(
        center.x as f32,
        center.y as f32,
        radius,
        3.0,
        rgba(42, 30, 22, 255),
    );
    let highlight = add(
        sub(center, scale(dir, f64::from(radius) * 0.18)),
        scale(
            rp::Vec2 {
                x: -dir.y,
                y: dir.x,
            },
            f64::from(radius) * 0.20,
        ),
    );
    draw_circle(
        highlight.x as f32,
        highlight.y as f32,
        (radius * 0.25).max(5.0),
        rgba(238, 218, 158, 220),
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
            label: "blunt",
            accent: tool_color(rp::ToolMode::Blunt),
            active: app.tool == rp::ToolMode::Blunt,
            action: Some(ControlAction::Tool(rp::ToolMode::Blunt)),
        },
        ControlHint {
            key: "S",
            label: "sharp",
            accent: tool_color(rp::ToolMode::Sharp),
            active: app.tool == rp::ToolMode::Sharp,
            action: Some(ControlAction::Tool(rp::ToolMode::Sharp)),
        },
        ControlHint {
            key: "H",
            label: "heavy",
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
        rp::ToolMode::Blunt => "blunt",
        rp::ToolMode::Sharp => "sharp",
        rp::ToolMode::Heavy => "heavy",
    }
}

fn tool_radius(tool: rp::ToolMode) -> f64 {
    match tool {
        rp::ToolMode::Sharp => 34.0 * 0.48,
        rp::ToolMode::Heavy => 34.0 * 1.18,
        rp::ToolMode::Blunt => 34.0,
    }
}

fn tool_color(tool: rp::ToolMode) -> Color {
    match tool {
        rp::ToolMode::Blunt => rgba(211, 167, 91, 235),
        rp::ToolMode::Sharp => rgba(192, 220, 224, 235),
        rp::ToolMode::Heavy => rgba(132, 141, 147, 235),
    }
}

fn striker_direction(app: &AppState) -> rp::Vec2 {
    let speed = length(app.striker_velocity);
    if speed > 1.0 {
        return scale(app.striker_velocity, 1.0 / speed);
    }

    let target_delta = sub(app.striker, app.pointer);
    let distance = length(target_delta);
    if distance > 1.0 {
        return scale(target_delta, 1.0 / distance);
    }

    rp::Vec2 { x: 1.0, y: 0.0 }
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
