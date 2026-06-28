use macroquad::prelude::*;
use realistic_physics as rp;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Normal,
    Anatomy,
}

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
}

impl AppState {
    fn new(width: f64, height: f64) -> Self {
        let initial_pointer = rp::Vec2 {
            x: width * 0.28,
            y: height * 0.46,
        };
        Self {
            world: rp::create_layered_body(width, height, rp::Materials::default()),
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
    skin_heat: Color,
    skin_outline: Color,
    skin_wire: Color,
    muscle_base: Color,
    muscle_hot: Color,
    muscle_shadow: Color,
    bone: Color,
    bone_fractured: Color,
    bone_shadow: Color,
    blood_dark: Color,
    blood_mid: Color,
    blood_fresh: Color,
    wound_core: Color,
    wound_edge: Color,
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
    let pointer_down = is_mouse_button_down(MouseButton::Left);
    if app.pointer_initialized || pointer_down || mx.abs() > 1.0 || my.abs() > 1.0 {
        app.pointer = rp::Vec2 {
            x: mx as f64,
            y: my as f64,
        };
        app.pointer_initialized = true;
    }
    app.pointer_down = pointer_down;

    if is_key_pressed(KeyCode::B) {
        app.tool = rp::ToolMode::Blunt;
    }
    if is_key_pressed(KeyCode::S) {
        app.tool = rp::ToolMode::Sharp;
    }
    if is_key_pressed(KeyCode::H) {
        app.tool = rp::ToolMode::Heavy;
    }
    if is_key_pressed(KeyCode::D) {
        app.debug_overlay = !app.debug_overlay;
    }
    if is_key_pressed(KeyCode::Tab) {
        app.view_mode = if app.view_mode == ViewMode::Anatomy {
            ViewMode::Normal
        } else {
            ViewMode::Anatomy
        };
    }
    if is_key_pressed(KeyCode::Space) {
        app.running = !app.running;
    }
    if is_key_pressed(KeyCode::Key1) {
        app.impact_power = 1.0;
    }
    if is_key_pressed(KeyCode::Key2) {
        app.impact_power = 2.0;
    }
    if is_key_pressed(KeyCode::Key4) {
        app.impact_power = 4.0;
    }
    if is_key_pressed(KeyCode::R) {
        app.world = rp::create_layered_body(
            screen_width() as f64,
            screen_height() as f64,
            rp::Materials::default(),
        );
        app.striker = app.pointer;
        app.striker_velocity = rp::Vec2 { x: 0.0, y: 0.0 };
        app.accumulator = 0.0;
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
        floor_y: screen_height() - 38.0,
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
        skin_heat: rgba(223, 77, 55, 246),
        skin_outline: rgba(69, 40, 36, 220),
        skin_wire: rgba(136, 86, 75, 130),
        muscle_base: rgba(112, 22, 31, 190),
        muscle_hot: rgba(190, 35, 47, 225),
        muscle_shadow: rgba(47, 8, 13, 150),
        bone: rgba(222, 211, 181, 240),
        bone_fractured: rgba(255, 245, 218, 255),
        bone_shadow: rgba(53, 43, 35, 160),
        blood_dark: rgba(43, 2, 7, 235),
        blood_mid: rgba(103, 7, 15, 230),
        blood_fresh: rgba(190, 22, 26, 235),
        wound_core: rgba(34, 0, 5, 230),
        wound_edge: rgba(156, 18, 24, 235),
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

fn draw_background(ctx: &RenderContext) {
    clear_background(ctx.palette.background);
    draw_rectangle(
        0.0,
        ctx.height * 0.54,
        ctx.width,
        ctx.height * 0.46,
        ctx.palette.background_low,
    );
    draw_rectangle(0.0, ctx.floor_y, ctx.width, 38.0, ctx.palette.floor);
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
    draw_skin_layer(ctx);

    if ctx.anatomy {
        draw_bone_attachments(ctx);
        draw_bones(ctx, BonePass::Anatomy);
    } else {
        draw_bones(ctx, BonePass::ExposedDamage);
    }

    draw_wound_edges(ctx);
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
        let visible = ctx.anatomy || exposure > 0.035 || triangle.damage > 0.015 || load > 140.0;
        if !visible {
            continue;
        }

        let heat = ((load / 900.0) + triangle.damage * 0.85 + exposure * 0.35).clamp(0.0, 1.0);
        let mut fill = mix(ctx.palette.muscle_base, ctx.palette.muscle_hot, heat as f32);
        fill.a = if ctx.anatomy {
            (0.54 + heat as f32 * 0.28 + exposure as f32 * 0.10).min(0.88)
        } else {
            (0.20 + exposure as f32 * 0.58 + triangle.damage as f32 * 0.28).clamp(0.18, 0.82)
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
    let world = &ctx.app.world;
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Skin || !world.triangle_alive(triangle) {
            continue;
        }

        let (load, exposure) = triangle_point_metrics(world, triangle);
        let heat = (load / 1300.0).clamp(0.0, 1.0);
        if ctx.anatomy {
            let mut veil = mix(
                ctx.palette.skin_base,
                ctx.palette.skin_heat,
                heat as f32 * 0.35,
            );
            veil.a = (0.08 + heat as f32 * 0.08).min(0.17);
            fill_triangle(world, triangle, veil);

            let wire = mix(
                ctx.palette.skin_wire,
                ctx.palette.skin_heat,
                heat as f32 * 0.65,
            );
            outline_triangle(world, triangle, wire, 1.0);
        } else {
            let mut fill = mix(ctx.palette.skin_base, ctx.palette.skin_heat, heat as f32);
            fill.a = (0.96 - exposure as f32 * 0.22).clamp(0.68, 0.98);
            fill_triangle(world, triangle, fill);
            if heat > 0.08 || exposure > 0.10 {
                outline_triangle(
                    world,
                    triangle,
                    with_alpha(
                        ctx.palette.skin_outline,
                        (0.35 + heat as f32 * 0.30).min(0.72),
                    ),
                    1.0,
                );
            }
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

    draw_line_vec(
        add(a_mid, scale(normal, -mark)),
        add(a_mid, scale(normal, mark)),
        4.0,
        ctx.palette.wound_core,
    );
    draw_line_vec(
        add(a_mid, scale(normal, -mark * 0.72)),
        add(a_mid, scale(normal, mark * 0.72)),
        2.0,
        ctx.palette.wound_edge,
    );
    draw_line_vec(
        add(b_mid, scale(normal, -mark)),
        add(b_mid, scale(normal, mark)),
        4.0,
        ctx.palette.wound_core,
    );
    draw_line_vec(
        add(b_mid, scale(normal, -mark * 0.72)),
        add(b_mid, scale(normal, mark * 0.72)),
        2.0,
        ctx.palette.blood_fresh,
    );
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
    draw_fluids(ctx);
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

    let mut x = 14.0;
    for (index, item) in items.iter().enumerate() {
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
        x += draw_chip(
            x,
            14.0,
            item,
            accent,
            ctx.palette.hud_text,
            ctx.palette.hud_back,
        ) + 7.0;
    }
}

#[derive(Clone, Copy)]
struct ControlHint<'a> {
    key: &'a str,
    label: &'a str,
    accent: Color,
    active: bool,
}

fn draw_controls_hint(ctx: &RenderContext) {
    let hints = [
        ControlHint {
            key: "DRAG",
            label: "strike",
            accent: ctx.palette.tool_accent,
            active: ctx.app.pointer_down,
        },
        ControlHint {
            key: "B",
            label: "blunt",
            accent: tool_color(rp::ToolMode::Blunt),
            active: ctx.app.tool == rp::ToolMode::Blunt,
        },
        ControlHint {
            key: "S",
            label: "sharp",
            accent: tool_color(rp::ToolMode::Sharp),
            active: ctx.app.tool == rp::ToolMode::Sharp,
        },
        ControlHint {
            key: "H",
            label: "heavy",
            accent: tool_color(rp::ToolMode::Heavy),
            active: ctx.app.tool == rp::ToolMode::Heavy,
        },
        ControlHint {
            key: "TAB",
            label: "view",
            accent: ctx.palette.tool_accent,
            active: ctx.anatomy,
        },
        ControlHint {
            key: "D",
            label: "debug",
            accent: rgba(94, 176, 108, 230),
            active: ctx.app.debug_overlay,
        },
        ControlHint {
            key: "SPACE",
            label: "pause",
            accent: rgba(211, 93, 70, 230),
            active: !ctx.app.running,
        },
        ControlHint {
            key: "R",
            label: "reset",
            accent: ctx.palette.hud_border,
            active: false,
        },
        ControlHint {
            key: "1 2 4",
            label: "mass",
            accent: ctx.palette.hud_border,
            active: false,
        },
    ];

    let margin = 14.0;
    let pad = 8.0;
    let gap = 7.0;
    let row_h = 24.0;
    let row_gap = 6.0;
    let available_w = (ctx.width - margin * 2.0 - pad * 2.0).max(260.0);
    let row_count = control_hint_row_count(&hints, available_w, gap);
    let panel_h =
        pad * 2.0 + row_count as f32 * row_h + (row_count.saturating_sub(1)) as f32 * row_gap;
    let panel_y = (ctx.floor_y - panel_h - 10.0).max(54.0);
    draw_panel(ctx, margin, panel_y, ctx.width - margin * 2.0, panel_h);

    let mut x = margin + pad;
    let mut y = panel_y + pad;
    for hint in hints {
        let width = control_hint_width(hint);
        if x > margin + pad && x + width > margin + pad + available_w {
            x = margin + pad;
            y += row_h + row_gap;
        }

        draw_control_hint(ctx, x, y, hint);
        x += width + gap;
    }
}

fn control_hint_row_count(hints: &[ControlHint], available_w: f32, gap: f32) -> usize {
    let mut rows = 1usize;
    let mut x = 0.0;
    for hint in hints {
        let width = control_hint_width(*hint);
        let next_x = if x > 0.0 { x + gap + width } else { width };
        if x > 0.0 && next_x > available_w {
            rows += 1;
            x = width;
        } else {
            x = next_x;
        }
    }
    rows
}

fn control_hint_width(hint: ControlHint) -> f32 {
    let key = measure_text(hint.key, None, 15, 1.0);
    let label = measure_text(hint.label, None, 15, 1.0);
    key.width + label.width + 32.0
}

fn draw_control_hint(ctx: &RenderContext, x: f32, y: f32, hint: ControlHint) -> f32 {
    let width = control_hint_width(hint);
    let key_width = measure_text(hint.key, None, 15, 1.0).width + 13.0;
    let back = if hint.active {
        with_alpha(hint.accent, 0.20)
    } else {
        rgba(8, 8, 9, 160)
    };
    let border = if hint.active {
        with_alpha(hint.accent, 0.85)
    } else {
        with_alpha(ctx.palette.hud_border, 0.38)
    };

    draw_rectangle(x, y, width, 24.0, back);
    draw_rectangle_lines(x, y, width, 24.0, 1.0, border);
    draw_rectangle(
        x + 3.0,
        y + 3.0,
        key_width,
        18.0,
        with_alpha(hint.accent, 0.24),
    );
    draw_rectangle_lines(
        x + 3.0,
        y + 3.0,
        key_width,
        18.0,
        1.0,
        with_alpha(hint.accent, 0.72),
    );
    draw_text(hint.key, x + 9.0, y + 16.0, 15.0, ctx.palette.hud_text);
    draw_text(
        hint.label,
        x + key_width + 11.0,
        y + 16.0,
        15.0,
        ctx.palette.hud_muted,
    );
    width
}

fn draw_debug_panel(ctx: &RenderContext) {
    let debug = ctx.app.world.debug();
    let stats = ctx.app.world.stats();
    let active_fluids = ctx
        .app
        .world
        .fluids()
        .iter()
        .filter(|fluid| fluid.life > 0.0)
        .count();
    let panel_x = 14.0;
    let panel_y = 54.0;
    let panel_w = 446.0;
    let panel_h = 252.0;
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
            "damage  skin={} muscle={} attach={}/{} joints={}",
            stats.broken_skin,
            stats.broken_muscle,
            stats.broken_attachments,
            stats.broken_bone_attachments,
            stats.broken_bone_joints
        ),
        format!(
            "fragments  step={} tissue={} pair={} tears={}",
            debug.fractures,
            debug.fragment_contacts,
            debug.fragment_pair_contacts,
            debug.fragment_tears
        ),
        format!(
            "fragment motion  impulse={:.0} spin={:.2} overlap={:.1}",
            debug.max_fragment_impulse, debug.max_bone_angular_speed, debug.max_fragment_overlap
        ),
        format!(
            "joint limits  corrections={} stretch={:.1} angle={:.2}",
            debug.post_fracture_joint_corrections,
            debug.max_post_fracture_joint_stretch,
            debug.max_post_fracture_joint_angle
        ),
        format!(
            "fluid  active={} emitted={} step={} wounds={} leaks={}",
            active_fluids,
            stats.emitted_fluid_particles,
            debug.fluid_emitted,
            debug.active_wounds,
            debug.wound_leaks
        ),
        format!(
            "wounds  pressure={:.2} clot={:.2}",
            debug.max_wound_pressure, debug.max_wound_clot
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

fn draw_chip(x: f32, y: f32, label: &str, accent: Color, text: Color, back: Color) -> f32 {
    let metrics = measure_text(label, None, 17, 1.0);
    let width = metrics.width + 20.0;
    draw_rectangle(x, y, width, 25.0, back);
    draw_rectangle_lines(x, y, width, 25.0, 1.0, with_alpha(accent, 0.58));
    draw_rectangle(x, y, 4.0, 25.0, accent);
    draw_text(label, x + 10.0, y + 17.0, 17.0, text);
    width
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
