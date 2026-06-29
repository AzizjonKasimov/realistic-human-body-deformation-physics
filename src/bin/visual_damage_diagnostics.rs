use realistic_physics as rp;
use std::env;
use std::fmt::Write as FmtWrite;
use std::fs::{self, File};
use std::io::{BufWriter, Write as IoWrite};
use std::path::{Path, PathBuf};

const WIDTH: f64 = 1280.0;
const HEIGHT: f64 = 720.0;
const PANEL_GAP: f64 = 24.0;

#[derive(Clone, Copy)]
struct VisualScenario {
    name: &'static str,
    intent: &'static str,
    tool: rp::ToolMode,
    start: rp::Vec2,
    end: rp::Vec2,
    windup_frames: i32,
    strike_frames: i32,
    settle_frames: i32,
    power: f64,
    expectations: VisualExpectations,
}

#[derive(Clone, Copy, Default)]
struct VisualExpectations {
    min_skin_wound_edges: usize,
    min_muscle_fiber_lines: usize,
    min_muscle_fiber_tears: usize,
    min_joint_ligament_damage_events: usize,
    min_failed_muscle_voids: usize,
    min_visible_contusions: usize,
    min_visible_wound_sources: usize,
    min_visible_fluid_particles: usize,
    min_visible_blood_stains: usize,
    min_lacerated_vessels: usize,
    min_fragment_vessel_lacerations: usize,
    min_fractured_bones: usize,
    min_rib_fractures: usize,
    min_fracture_caps: usize,
    min_cavity_ruptures: usize,
    min_cavity_pressure: f64,
    min_organ_penetrations: usize,
    min_rib_organ_punctures: usize,
    min_organ_ruptures: usize,
    min_organ_damage: f64,
    min_damage_primitives: usize,
}

#[derive(Clone, Copy, Default)]
struct VisualMetrics {
    skin_wound_edges: usize,
    wound_edge_fiber_ticks: usize,
    exposed_muscle_triangles: usize,
    muscle_detail_triangles: usize,
    muscle_fiber_lines: usize,
    muscle_fiber_tears: usize,
    joint_ligament_damage_events: usize,
    failed_muscle_voids: usize,
    visible_contusions: usize,
    active_wound_sources: usize,
    visible_wound_sources: usize,
    active_fluid_particles: usize,
    visible_fluid_particles: usize,
    visible_blood_stains: usize,
    lacerated_vessels: usize,
    fragment_vessel_lacerations: usize,
    fractured_bones: usize,
    rib_fractures: usize,
    fracture_caps: usize,
    exposed_points: usize,
    damage_primitives: usize,
    final_free_fragments: usize,
    final_sleeping_fragments: usize,
    cavity_ruptures: usize,
    cavity_pressure: f64,
    cavity_collapse: f64,
    organ_penetrations: usize,
    rib_organ_punctures: usize,
    organ_ruptures: usize,
    organ_damage: f64,
    max_point_load: f64,
    max_point_exposure: f64,
    max_contusion: f64,
    max_muscle_damage: f64,
}

struct VisualCapture {
    scenario: VisualScenario,
    world: rp::World,
    metrics: VisualMetrics,
}

fn main() {
    let svg_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("output/damage_visual_debug.svg"));
    let output_dir = svg_path.parent().unwrap_or_else(|| Path::new("output"));
    let summary_path = output_dir.join("damage_visual_summary.csv");

    if !output_dir.as_os_str().is_empty() {
        fs::create_dir_all(output_dir).expect("create output directory");
    }

    let mut captures = Vec::new();
    let mut warnings = Vec::new();
    for scenario in visual_scenarios() {
        let (world, peak_cavity_pressure, peak_cavity_collapse, peak_organ_damage) =
            run_scenario(&scenario);
        let mut metrics = inspect_visual_damage(&world);
        metrics.cavity_pressure = metrics.cavity_pressure.max(peak_cavity_pressure);
        metrics.cavity_collapse = metrics.cavity_collapse.max(peak_cavity_collapse);
        metrics.organ_damage = metrics.organ_damage.max(peak_organ_damage);
        validate_visual_metrics(&scenario, &metrics, &mut warnings);
        captures.push(VisualCapture {
            scenario,
            world,
            metrics,
        });
    }

    fs::write(&svg_path, build_svg(&captures)).expect("write damage visual SVG");
    write_summary(&summary_path, &captures).expect("write damage visual summary");

    println!("wrote {}", svg_path.display());
    println!("wrote {}", summary_path.display());
    if warnings.is_empty() {
        println!("PASS: visual damage diagnostics include expected wound and fracture detail");
    } else {
        for warning in warnings {
            eprintln!("WARN: {warning}");
        }
        std::process::exit(2);
    }
}

fn visual_scenarios() -> Vec<VisualScenario> {
    vec![
        VisualScenario {
            name: "torso_sharp_cut_visual",
            intent: "cut",
            tool: rp::ToolMode::Sharp,
            start: v(575.0, 300.0),
            end: v(710.0, 410.0),
            windup_frames: 8,
            strike_frames: 28,
            settle_frames: 48,
            power: 3.0,
            expectations: VisualExpectations {
                min_skin_wound_edges: 80,
                min_muscle_fiber_lines: 48,
                min_muscle_fiber_tears: 8,
                min_failed_muscle_voids: 80,
                min_visible_contusions: 50,
                min_visible_wound_sources: 30,
                min_visible_fluid_particles: 120,
                min_lacerated_vessels: 1,
                min_organ_penetrations: 1,
                min_organ_ruptures: 1,
                min_organ_damage: 1.0,
                min_fractured_bones: 8,
                min_rib_fractures: 3,
                min_fracture_caps: 8,
                min_damage_primitives: 1000,
                ..VisualExpectations::default()
            },
        },
        VisualScenario {
            name: "torso_heavy_settle_visual",
            intent: "settle",
            tool: rp::ToolMode::Heavy,
            start: v(500.0, 365.0),
            end: v(720.0, 365.0),
            windup_frames: 12,
            strike_frames: 30,
            settle_frames: 260,
            power: 4.0,
            expectations: VisualExpectations {
                min_skin_wound_edges: 140,
                min_muscle_fiber_lines: 48,
                min_muscle_fiber_tears: 40,
                min_joint_ligament_damage_events: 1,
                min_failed_muscle_voids: 160,
                min_visible_contusions: 120,
                min_visible_wound_sources: 50,
                min_visible_fluid_particles: 120,
                min_visible_blood_stains: 8,
                min_lacerated_vessels: 1,
                min_fragment_vessel_lacerations: 1,
                min_fractured_bones: 18,
                min_rib_fractures: 4,
                min_fracture_caps: 18,
                min_cavity_ruptures: 1,
                min_cavity_pressure: 0.70,
                min_organ_penetrations: 0,
                min_rib_organ_punctures: 1,
                min_organ_ruptures: 1,
                min_organ_damage: 1.0,
                min_damage_primitives: 1400,
            },
        },
    ]
}

fn run_scenario(scenario: &VisualScenario) -> (rp::World, f64, f64, f64) {
    let mut world = rp::create_layered_body(WIDTH, HEIGHT, rp::Materials::default());
    let dt = world.materials().fixed_dt;
    let total_frames = scenario.windup_frames + scenario.strike_frames + scenario.settle_frames;
    let mut peak_cavity_pressure: f64 = 0.0;
    let mut peak_cavity_collapse: f64 = 0.0;
    let mut peak_organ_damage: f64 = 0.0;
    for frame in 0..total_frames {
        let input = if frame < scenario.windup_frames + scenario.strike_frames {
            make_strike_input(scenario, frame, dt)
        } else {
            rp::InputState::default()
        };
        world.step(dt, &input, WIDTH, HEIGHT);
        let debug = world.debug();
        peak_cavity_pressure = peak_cavity_pressure.max(debug.max_cavity_pressure);
        peak_cavity_collapse = peak_cavity_collapse.max(debug.max_cavity_collapse);
        peak_organ_damage = peak_organ_damage.max(debug.max_organ_damage);
    }
    (
        world,
        peak_cavity_pressure,
        peak_cavity_collapse,
        peak_organ_damage,
    )
}

fn make_strike_input(scenario: &VisualScenario, frame: i32, dt: f64) -> rp::InputState {
    let t0 =
        (frame - scenario.windup_frames).max(0) as f64 / (scenario.strike_frames - 1).max(1) as f64;
    let t = t0.clamp(0.0, 1.0);
    let position = rp::Vec2 {
        x: scenario.start.x + (scenario.end.x - scenario.start.x) * t,
        y: scenario.start.y + (scenario.end.y - scenario.start.y) * t,
    };
    let velocity = rp::Vec2 {
        x: (scenario.end.x - scenario.start.x) / ((scenario.strike_frames - 1).max(1) as f64 * dt),
        y: (scenario.end.y - scenario.start.y) / ((scenario.strike_frames - 1).max(1) as f64 * dt),
    };
    let down =
        frame >= scenario.windup_frames && frame < scenario.windup_frames + scenario.strike_frames;
    rp::InputState {
        active: down,
        down,
        x: position.x,
        y: position.y,
        vx: if down { velocity.x } else { 0.0 },
        vy: if down { velocity.y } else { 0.0 },
        power: scenario.power,
        tool: scenario.tool,
    }
}

fn inspect_visual_damage(world: &rp::World) -> VisualMetrics {
    let mut metrics = VisualMetrics::default();

    for point in world.points() {
        metrics.max_point_load = metrics.max_point_load.max(point.load);
        metrics.max_point_exposure = metrics.max_point_exposure.max(point.exposure);
        metrics.max_contusion = metrics.max_contusion.max(point.contusion);
        if point.exposure > 0.12 {
            metrics.exposed_points += 1;
        }
        if point.contusion > 0.05 {
            metrics.visible_contusions += 1;
        }
    }

    for spring in world.springs() {
        if spring.broken && spring.layer == rp::TissueLayer::Skin {
            metrics.skin_wound_edges += 1;
            if spring.a < world.points().len() && spring.b < world.points().len() {
                metrics.wound_edge_fiber_ticks +=
                    wound_edge_fiber_tick_count(world.points()[spring.a], world.points()[spring.b]);
            }
        }
    }

    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Muscle {
            continue;
        }
        let (load, exposure) = triangle_point_metrics(world, triangle);
        metrics.max_muscle_damage = metrics.max_muscle_damage.max(triangle.damage);
        if exposure > 0.035 || triangle.damage > 0.015 || load > 140.0 {
            metrics.exposed_muscle_triangles += 1;
        }
        if world.triangle_alive(triangle) {
            let detail = muscle_detail_amount(load, exposure, triangle.damage);
            if detail > 0.18 && longest_edge_length(world, triangle) >= 6.0 {
                metrics.muscle_detail_triangles += 1;
                metrics.muscle_fiber_lines += muscle_fiber_row_count(detail);
            }
        } else if exposure > 0.22 || load > 260.0 || triangle.damage > 0.72 {
            metrics.failed_muscle_voids += 1;
        }
    }

    for wound in world.wounds() {
        if wound.age > 0.0 || wound.pressure > 0.0 || wound.clot > 0.0 {
            metrics.visible_wound_sources += 1;
        }
        if wound.active {
            metrics.active_wound_sources += 1;
        }
    }

    for fluid in world.fluids() {
        if fluid.life > 0.0 {
            metrics.active_fluid_particles += 1;
            if fluid.intensity > 0.02 && fluid.radius > 0.2 {
                metrics.visible_fluid_particles += 1;
            }
        }
    }

    metrics.visible_blood_stains = world
        .blood_stains()
        .iter()
        .filter(|stain| stain.intensity > 0.025)
        .count();

    metrics.lacerated_vessels = world
        .vessels()
        .iter()
        .filter(|vessel| vessel.lacerated)
        .count();
    metrics.fragment_vessel_lacerations = world.stats().fragment_vessel_lacerations.max(0) as usize;
    metrics.muscle_fiber_tears = world.stats().muscle_fiber_tears.max(0) as usize;
    metrics.joint_ligament_damage_events =
        world.stats().joint_ligament_damage_events.max(0) as usize;
    metrics.rib_fractures = world.stats().fractured_ribs.max(0) as usize;

    for bone in world.bones() {
        if bone.fractured || bone.splinter {
            metrics.fractured_bones += 1;
        }
        if free_fragment(bone) {
            metrics.final_free_fragments += 1;
            if bone.sleeping {
                metrics.final_sleeping_fragments += 1;
            }
        }
        if bone.broken_start {
            metrics.fracture_caps += 1;
        }
        if bone.broken_end {
            metrics.fracture_caps += 1;
        }
    }

    metrics.damage_primitives = metrics.skin_wound_edges
        + metrics.wound_edge_fiber_ticks
        + metrics.muscle_fiber_lines
        + metrics.failed_muscle_voids
        + metrics.visible_contusions
        + metrics.visible_wound_sources
        + metrics.visible_fluid_particles
        + metrics.visible_blood_stains
        + metrics.lacerated_vessels
        + metrics.fractured_bones
        + metrics.fracture_caps;
    metrics.cavity_ruptures = world.stats().cavity_ruptures.max(0) as usize;
    metrics.cavity_pressure = max_cavity_pressure(world);
    metrics.cavity_collapse = max_cavity_collapse(world);
    metrics.organ_penetrations = world.stats().organ_penetrations.max(0) as usize;
    metrics.rib_organ_punctures = world.stats().rib_organ_punctures.max(0) as usize;
    metrics.organ_ruptures = world.stats().organ_ruptures.max(0) as usize;
    metrics.organ_damage = max_organ_damage(world);
    metrics
}

fn validate_visual_metrics(
    scenario: &VisualScenario,
    metrics: &VisualMetrics,
    warnings: &mut Vec<String>,
) {
    check_min(
        scenario,
        "skin_wound_edges",
        metrics.skin_wound_edges,
        scenario.expectations.min_skin_wound_edges,
        warnings,
    );
    check_min(
        scenario,
        "muscle_fiber_lines",
        metrics.muscle_fiber_lines,
        scenario.expectations.min_muscle_fiber_lines,
        warnings,
    );
    check_min(
        scenario,
        "muscle_fiber_tears",
        metrics.muscle_fiber_tears,
        scenario.expectations.min_muscle_fiber_tears,
        warnings,
    );
    check_min(
        scenario,
        "joint_ligament_damage_events",
        metrics.joint_ligament_damage_events,
        scenario.expectations.min_joint_ligament_damage_events,
        warnings,
    );
    check_min(
        scenario,
        "failed_muscle_voids",
        metrics.failed_muscle_voids,
        scenario.expectations.min_failed_muscle_voids,
        warnings,
    );
    check_min(
        scenario,
        "visible_contusions",
        metrics.visible_contusions,
        scenario.expectations.min_visible_contusions,
        warnings,
    );
    check_min(
        scenario,
        "visible_wound_sources",
        metrics.visible_wound_sources,
        scenario.expectations.min_visible_wound_sources,
        warnings,
    );
    check_min(
        scenario,
        "visible_fluid_particles",
        metrics.visible_fluid_particles,
        scenario.expectations.min_visible_fluid_particles,
        warnings,
    );
    check_min(
        scenario,
        "visible_blood_stains",
        metrics.visible_blood_stains,
        scenario.expectations.min_visible_blood_stains,
        warnings,
    );
    check_min(
        scenario,
        "lacerated_vessels",
        metrics.lacerated_vessels,
        scenario.expectations.min_lacerated_vessels,
        warnings,
    );
    check_min(
        scenario,
        "fragment_vessel_lacerations",
        metrics.fragment_vessel_lacerations,
        scenario.expectations.min_fragment_vessel_lacerations,
        warnings,
    );
    check_min(
        scenario,
        "fractured_bones",
        metrics.fractured_bones,
        scenario.expectations.min_fractured_bones,
        warnings,
    );
    check_min(
        scenario,
        "rib_fractures",
        metrics.rib_fractures,
        scenario.expectations.min_rib_fractures,
        warnings,
    );
    check_min(
        scenario,
        "fracture_caps",
        metrics.fracture_caps,
        scenario.expectations.min_fracture_caps,
        warnings,
    );
    check_min(
        scenario,
        "cavity_ruptures",
        metrics.cavity_ruptures,
        scenario.expectations.min_cavity_ruptures,
        warnings,
    );
    check_min_f64(
        scenario,
        "cavity_pressure",
        metrics.cavity_pressure,
        scenario.expectations.min_cavity_pressure,
        warnings,
    );
    check_min(
        scenario,
        "organ_penetrations",
        metrics.organ_penetrations,
        scenario.expectations.min_organ_penetrations,
        warnings,
    );
    check_min(
        scenario,
        "rib_organ_punctures",
        metrics.rib_organ_punctures,
        scenario.expectations.min_rib_organ_punctures,
        warnings,
    );
    check_min(
        scenario,
        "organ_ruptures",
        metrics.organ_ruptures,
        scenario.expectations.min_organ_ruptures,
        warnings,
    );
    check_min_f64(
        scenario,
        "organ_damage",
        metrics.organ_damage,
        scenario.expectations.min_organ_damage,
        warnings,
    );
    check_min(
        scenario,
        "damage_primitives",
        metrics.damage_primitives,
        scenario.expectations.min_damage_primitives,
        warnings,
    );
}

fn check_min_f64(
    scenario: &VisualScenario,
    name: &str,
    value: f64,
    minimum: f64,
    warnings: &mut Vec<String>,
) {
    if value < minimum {
        warnings.push(format!(
            "{}: {name}={value:.3} below minimum {minimum:.3}",
            scenario.name
        ));
    }
}

fn check_min(
    scenario: &VisualScenario,
    name: &str,
    value: usize,
    minimum: usize,
    warnings: &mut Vec<String>,
) {
    if value < minimum {
        warnings.push(format!(
            "{}: {}={} below required {}",
            scenario.name, name, value, minimum
        ));
    }
}

fn build_svg(captures: &[VisualCapture]) -> String {
    let width = WIDTH * captures.len() as f64 + PANEL_GAP * captures.len().saturating_sub(1) as f64;
    let mut out = String::new();
    writeln!(
        &mut out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.2}\" height=\"{HEIGHT:.2}\" viewBox=\"0 0 {width:.2} {HEIGHT:.2}\">"
    )
    .expect("write SVG header");
    writeln!(
        &mut out,
        "<style>text{{font-family:Arial,sans-serif}} .small{{font-size:16px;fill:#e8dccf}} .muted{{font-size:13px;fill:#b8aaa0}}</style>"
    )
    .expect("write SVG style");

    for (index, capture) in captures.iter().enumerate() {
        let offset = index as f64 * (WIDTH + PANEL_GAP);
        writeln!(&mut out, "<g transform=\"translate({offset:.2} 0)\">").expect("open panel");
        draw_panel(&mut out, capture);
        writeln!(&mut out, "</g>").expect("close panel");
    }

    writeln!(&mut out, "</svg>").expect("close SVG");
    out
}

fn draw_panel(out: &mut String, capture: &VisualCapture) {
    writeln!(
        out,
        "<rect width=\"{WIDTH:.2}\" height=\"{HEIGHT:.2}\" fill=\"#171514\"/>"
    )
    .expect("write background");
    writeln!(
        out,
        "<rect x=\"0\" y=\"682\" width=\"{WIDTH:.2}\" height=\"38\" fill=\"#211f1d\"/>"
    )
    .expect("write floor");
    writeln!(
        out,
        "<line x1=\"0\" y1=\"682\" x2=\"{WIDTH:.2}\" y2=\"682\" stroke=\"#66564d\" stroke-opacity=\"0.45\" stroke-width=\"2\"/>"
    )
    .expect("write floor line");

    draw_tissue_layers(out, &capture.world);
    draw_bones(out, &capture.world);
    draw_wound_edges(out, &capture.world);
    draw_major_vessels(out, &capture.world);
    draw_wound_sources(out, &capture.world);
    draw_blood_stains(out, &capture.world);
    draw_fluids(out, &capture.world);
    draw_label(out, capture);
}

fn draw_tissue_layers(out: &mut String, world: &rp::World) {
    for triangle in world.triangles() {
        if triangle.layer == rp::TissueLayer::Skin && world.triangle_alive(triangle) {
            let (load, exposure) = triangle_point_metrics(world, triangle);
            let heat = (load / 1300.0).clamp(0.0, 1.0);
            let opacity = (0.28 - exposure * 0.08 + heat * 0.10).clamp(0.16, 0.40);
            write_triangle(out, world, triangle, "#a97866", opacity, "#db9a86", 0.12);
        }
    }
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Muscle {
            continue;
        }
        let (load, exposure) = triangle_point_metrics(world, triangle);
        if world.triangle_alive(triangle) {
            let visible = exposure > 0.035 || triangle.damage > 0.015 || load > 140.0;
            if !visible {
                continue;
            }
            let heat = ((load / 900.0) + triangle.damage * 0.85 + exposure * 0.35).clamp(0.0, 1.0);
            let opacity = (0.22 + exposure * 0.45 + triangle.damage * 0.22).clamp(0.20, 0.82);
            let fill = if heat > 0.55 { "#df3f46" } else { "#a92732" };
            write_triangle(
                out,
                world,
                triangle,
                fill,
                opacity,
                "#f05f62",
                0.18 + heat * 0.24,
            );
        } else if exposure > 0.22 || load > 260.0 || triangle.damage > 0.72 {
            write_triangle(out, world, triangle, "#21040a", 0.30, "#9f1722", 0.46);
        }
    }
    draw_muscle_detail(out, world);
    draw_contusions(out, world);
}

fn draw_muscle_detail(out: &mut String, world: &rp::World) {
    for triangle in world.triangles() {
        if triangle.layer != rp::TissueLayer::Muscle {
            continue;
        }
        let (load, exposure) = triangle_point_metrics(world, triangle);
        if world.triangle_alive(triangle) {
            let detail = muscle_detail_amount(load, exposure, triangle.damage);
            if detail > 0.18 {
                draw_muscle_fibers(out, world, triangle, detail);
            }
        } else if exposure > 0.22 || load > 260.0 || triangle.damage > 0.72 {
            write_triangle(out, world, triangle, "#150105", 0.26, "#c3212b", 0.34);
        }
    }
}

fn draw_muscle_fibers(out: &mut String, world: &rp::World, triangle: &rp::Triangle, detail: f64) {
    let points = world.points();
    let a = points[triangle.a].position;
    let b = points[triangle.b].position;
    let c = points[triangle.c].position;
    let centroid = scale(add(add(a, b), c), 1.0 / 3.0);
    let edges = [(a, b), (b, c), (c, a)];
    let mut longest = edges[0];
    let mut longest_len = length(subtract(longest.1, longest.0));
    for edge in edges.iter().skip(1) {
        let len = length(subtract(edge.1, edge.0));
        if len > longest_len {
            longest = *edge;
            longest_len = len;
        }
    }
    if longest_len < 6.0 {
        return;
    }
    let fiber_dir = normalized(subtract(longest.1, longest.0), rp::Vec2 { x: 1.0, y: 0.0 });
    let normal = rp::Vec2 {
        x: -fiber_dir.y,
        y: fiber_dir.x,
    };
    let span = longest_len * (0.18 + detail * 0.24);
    let rows = muscle_fiber_row_count(detail);
    for row in 0..rows {
        let row_t = if rows == 1 {
            0.0
        } else {
            row as f64 / (rows - 1) as f64 - 0.5
        };
        let center = add(centroid, scale(normal, row_t * longest_len * 0.18));
        let trim = 0.72 - detail * 0.16;
        let start = subtract(center, scale(fiber_dir, span * trim));
        let end = add(center, scale(fiber_dir, span));
        write_line(
            out,
            start,
            end,
            0.8 + detail * 1.2,
            "#ec605f",
            0.16 + detail * 0.38,
        );
    }
}

fn draw_contusions(out: &mut String, world: &rp::World) {
    for triangle in world.triangles() {
        if !world.triangle_alive(triangle) {
            continue;
        }
        let contusion = triangle_point_contusion(world, triangle);
        if contusion <= 0.04 {
            continue;
        }
        let opacity = (0.10 + contusion * 0.30).clamp(0.10, 0.42);
        let fill = if triangle.layer == rp::TissueLayer::Skin {
            "#3b2056"
        } else {
            "#300d3d"
        };
        write_triangle(out, world, triangle, fill, opacity, fill, opacity * 0.55);
    }
}

fn draw_bones(out: &mut String, world: &rp::World) {
    for bone in world.bones() {
        let visible_damage =
            bone.fractured || bone.splinter || bone.broken_start || bone.broken_end;
        let opacity = if visible_damage { 0.92 } else { 0.20 };
        let stroke = if bone.fractured || bone.splinter {
            "#fff0cf"
        } else {
            "#d9caa6"
        };
        write_line(
            out,
            bone.a,
            bone.b,
            bone.radius * 1.85 + 3.0,
            "#080607",
            opacity * 0.52,
        );
        write_line(out, bone.a, bone.b, bone.radius * 1.85, stroke, opacity);
        if bone.broken_start {
            draw_fracture_cap(out, bone.a, bone.broken_start_normal, bone.radius);
        }
        if bone.broken_end {
            draw_fracture_cap(out, bone.b, bone.broken_end_normal, bone.radius);
        }
    }
}

fn draw_fracture_cap(out: &mut String, center: rp::Vec2, normal: rp::Vec2, radius: f64) {
    write_circle(out, center, radius * 1.38, "#170207", 0.34);
    write_circle(out, center, radius * 0.86, "#fff0cf", 0.92);
    let dir = normalized(normal, rp::Vec2 { x: 1.0, y: 0.0 });
    let tangent = rp::Vec2 {
        x: -dir.y,
        y: dir.x,
    };
    write_line(
        out,
        subtract(center, scale(tangent, radius * 0.78)),
        add(center, scale(tangent, radius * 0.78)),
        1.3,
        "#8d1720",
        0.78,
    );
}

fn draw_major_vessels(out: &mut String, world: &rp::World) {
    for vessel in world.vessels() {
        if !vessel.lacerated {
            continue;
        }
        write_line(
            out,
            vessel.a,
            vessel.b,
            vessel.radius * 2.5 + 2.0,
            "#190004",
            0.56,
        );
        write_line(
            out,
            vessel.a,
            vessel.b,
            vessel.radius * 1.35 + 0.8,
            "#b00f20",
            0.86,
        );
        write_circle(
            out,
            mid(vessel.a, vessel.b),
            vessel.radius * 3.8 + 7.0,
            "#e02032",
            0.16,
        );
    }
}

fn draw_wound_edges(out: &mut String, world: &rp::World) {
    for spring in world.springs() {
        if !spring.broken || spring.layer != rp::TissueLayer::Skin {
            continue;
        }
        if spring.a >= world.points().len() || spring.b >= world.points().len() {
            continue;
        }
        draw_wound_edge(out, world.points()[spring.a], world.points()[spring.b]);
    }
}

fn draw_wound_edge(out: &mut String, a: rp::Point, b: rp::Point) {
    let delta = subtract(b.position, a.position);
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
    let b_mid = subtract(b.position, scale(dir, inset));
    let exposure = a.exposure.max(b.exposure).clamp(0.0, 1.0);
    let load = a.load.max(b.load);
    let severity = (exposure * 0.58 + load / 1700.0).clamp(0.0, 1.0);

    write_line(
        out,
        add(a_mid, scale(normal, -mark)),
        add(a_mid, scale(normal, mark)),
        4.0 + severity * 1.8,
        "#110004",
        0.58 + severity * 0.30,
    );
    write_line(
        out,
        add(a_mid, scale(normal, -mark * 0.72)),
        add(a_mid, scale(normal, mark * 0.72)),
        2.0 + severity * 0.8,
        "#ad1720",
        0.68 + severity * 0.24,
    );
    write_line(
        out,
        add(b_mid, scale(normal, -mark)),
        add(b_mid, scale(normal, mark)),
        4.0 + severity * 1.8,
        "#110004",
        0.58 + severity * 0.30,
    );
    write_line(
        out,
        add(b_mid, scale(normal, -mark * 0.72)),
        add(b_mid, scale(normal, mark * 0.72)),
        2.0 + severity * 0.8,
        "#d51b2c",
        0.66 + severity * 0.28,
    );
    let tear_center = mid(a.position, b.position);
    write_line(
        out,
        subtract(tear_center, scale(dir, len * 0.24)),
        add(tear_center, scale(dir, len * 0.24)),
        1.0 + severity * 1.1,
        "#220005",
        0.44 + severity * 0.34,
    );
    if severity > 0.28 {
        let fiber_count = wound_edge_fiber_tick_count(a, b);
        for i in 0..fiber_count {
            let t = (i + 1) as f64 / (fiber_count + 1) as f64;
            let base = add(a.position, scale(delta, t));
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            write_line(
                out,
                add(base, scale(normal, side * mark * 0.20)),
                add(base, scale(normal, side * mark * (0.62 + severity * 0.36))),
                0.8 + severity * 0.7,
                "#ec605f",
                0.30 + severity * 0.34,
            );
        }
    }
}

fn draw_wound_sources(out: &mut String, world: &rp::World) {
    for wound in world.wounds() {
        if wound.age <= 0.0 && wound.pressure <= 0.0 && wound.clot <= 0.0 {
            continue;
        }
        let pressure = (wound.pressure / 6.0).clamp(0.0, 1.0);
        let clot = wound.clot.clamp(0.0, 1.0);
        let radius = wound.radius * (1.5 + wound.depth * 0.38);
        write_circle(
            out,
            wound.position,
            radius + 7.0 + pressure * 7.0,
            "#3d020b",
            0.22 + pressure * 0.18,
        );
        write_circle(out, wound.position, radius + 2.0, "#220005", 0.58);
        write_circle(out, wound.position, radius, "#ba1022", 0.84 - clot * 0.28);
        let dir = normalized(wound.direction, rp::Vec2 { x: 0.0, y: 1.0 });
        write_line(
            out,
            wound.position,
            add(wound.position, scale(dir, 8.0 + wound.pressure * 3.2)),
            1.4,
            "#f1313f",
            0.44 + pressure * 0.28,
        );
    }
}

fn draw_blood_stains(out: &mut String, world: &rp::World) {
    for stain in world.blood_stains() {
        if stain.intensity <= 0.025 {
            continue;
        }
        let intensity = stain.intensity.clamp(0.0, 1.75);
        write_circle(
            out,
            stain.position,
            stain.radius * (1.16 + intensity * 0.10),
            "#260007",
            0.16 + intensity * 0.14,
        );
        write_circle(
            out,
            stain.position,
            stain.radius * (0.58 + intensity * 0.08),
            "#2b0207",
            0.20 + intensity * 0.18,
        );
    }
}

fn draw_fluids(out: &mut String, world: &rp::World) {
    for fluid in world.fluids() {
        if fluid.life <= 0.0 {
            continue;
        }
        let age = 1.0 - (fluid.life / fluid.max_life.max(0.01)).clamp(0.0, 1.0);
        let opacity = (0.22 + fluid.intensity * 0.42) * (1.0 - age * 0.30);
        write_circle(
            out,
            fluid.position,
            fluid.radius * (0.86 + fluid.intensity * 0.20),
            if fluid.settled { "#5b0611" } else { "#b70d20" },
            opacity.clamp(0.10, 0.78),
        );
    }
}

fn draw_label(out: &mut String, capture: &VisualCapture) {
    let stats = capture.world.stats();
    let metrics = capture.metrics;
    writeln!(
        out,
        "<rect x=\"18\" y=\"18\" width=\"432\" height=\"118\" fill=\"#11100f\" fill-opacity=\"0.72\" stroke=\"#5c4d45\" stroke-opacity=\"0.72\"/>"
    )
    .expect("write label background");
    writeln!(
        out,
        "<text class=\"small\" x=\"34\" y=\"44\">{} ({}, {})</text>",
        capture.scenario.name,
        capture.scenario.intent,
        tool_name(capture.scenario.tool)
    )
    .expect("write label title");
    writeln!(
        out,
        "<text class=\"muted\" x=\"34\" y=\"69\">wound edges={} fiber={} fiberT={} voids={} bruises={} wounds={} fluids={} stains={}</text>",
        metrics.skin_wound_edges,
        metrics.muscle_fiber_lines,
        metrics.muscle_fiber_tears,
        metrics.failed_muscle_voids,
        metrics.visible_contusions,
        metrics.visible_wound_sources,
        metrics.visible_fluid_particles,
        metrics.visible_blood_stains
    )
    .expect("write label line one");
    writeln!(
        out,
        "<text class=\"muted\" x=\"34\" y=\"93\">vessels={} fractured bones={} ribs={} caps={} free fragments={} sleeping={}</text>",
        metrics.lacerated_vessels,
        metrics.fractured_bones,
        metrics.rib_fractures,
        metrics.fracture_caps,
        metrics.final_free_fragments,
        metrics.final_sleeping_fragments
    )
    .expect("write label line two");
    writeln!(
        out,
        "<text class=\"muted\" x=\"34\" y=\"117\">stats skin={} muscle={} crush={} cavity={} organPen={} ribOrg={} organ={} flaps={} vessels={} fragV={} punctures={} sublux={} lig={} marrow={} wounds={} blood={:.2} turgor={:.2} cavityP={:.2} organD={:.2} loss={:.3}</text>",
        stats.broken_skin,
        stats.broken_muscle,
        stats.muscle_crush_ruptures,
        stats.cavity_ruptures,
        stats.organ_penetrations,
        stats.rib_organ_punctures,
        stats.organ_ruptures,
        stats.skin_flap_detachments,
        stats.vessel_lacerations,
        stats.fragment_vessel_lacerations,
        stats.fragment_skin_punctures,
        stats.bone_joint_subluxations,
        stats.joint_ligament_damage_events,
        stats.fracture_marrow_sources,
        stats.opened_wounds,
        capture.world.blood_volume_fraction(),
        capture.world.blood_turgor_scale(),
        metrics.cavity_pressure,
        metrics.organ_damage,
        stats.blood_loss
    )
    .expect("write label line three");
}

fn write_summary(path: &Path, captures: &[VisualCapture]) -> std::io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(
        out,
        "scenario,intent,tool,skin_wound_edges,wound_edge_fiber_ticks,exposed_muscle_triangles,muscle_detail_triangles,muscle_fiber_lines,stats_muscle_fiber_tears,stats_joint_ligament_damage_events,failed_muscle_voids,visible_contusions,visible_wound_sources,active_wound_sources,visible_fluid_particles,active_fluid_particles,visible_blood_stains,lacerated_vessels,fractured_bones,rib_fractures,fracture_caps,final_free_fragments,final_sleeping_fragments,damage_primitives,max_point_load,max_point_exposure,max_contusion,max_muscle_damage,stats_skin_tears,stats_muscle_tears,stats_muscle_crush_ruptures,stats_cavity_pressure_events,stats_cavity_ruptures,peak_cavity_pressure,peak_cavity_collapse,stats_organ_damage_events,stats_organ_penetrations,stats_rib_organ_punctures,stats_organ_ruptures,peak_organ_damage,stats_skin_flap_detachments,stats_vessel_lacerations,stats_fragment_vessel_lacerations,stats_fragment_skin_punctures,stats_bone_joint_subluxations,stats_fracture_marrow_sources,stats_contusion_events,stats_opened_wounds,stats_emitted_fluid,stats_wound_fluid,stats_blood_loss,final_blood_volume,final_blood_turgor,stats_blood_stain_deposits"
    )?;
    for capture in captures {
        let metrics = capture.metrics;
        let stats = capture.world.stats();
        let fields = [
            capture.scenario.name.to_string(),
            capture.scenario.intent.to_string(),
            tool_name(capture.scenario.tool).to_string(),
            metrics.skin_wound_edges.to_string(),
            metrics.wound_edge_fiber_ticks.to_string(),
            metrics.exposed_muscle_triangles.to_string(),
            metrics.muscle_detail_triangles.to_string(),
            metrics.muscle_fiber_lines.to_string(),
            metrics.muscle_fiber_tears.to_string(),
            metrics.joint_ligament_damage_events.to_string(),
            metrics.failed_muscle_voids.to_string(),
            metrics.visible_contusions.to_string(),
            metrics.visible_wound_sources.to_string(),
            metrics.active_wound_sources.to_string(),
            metrics.visible_fluid_particles.to_string(),
            metrics.active_fluid_particles.to_string(),
            metrics.visible_blood_stains.to_string(),
            metrics.lacerated_vessels.to_string(),
            metrics.fractured_bones.to_string(),
            metrics.rib_fractures.to_string(),
            metrics.fracture_caps.to_string(),
            metrics.final_free_fragments.to_string(),
            metrics.final_sleeping_fragments.to_string(),
            metrics.damage_primitives.to_string(),
            format!("{:.3}", metrics.max_point_load),
            format!("{:.3}", metrics.max_point_exposure),
            format!("{:.3}", metrics.max_contusion),
            format!("{:.3}", metrics.max_muscle_damage),
            stats.broken_skin.to_string(),
            stats.broken_muscle.to_string(),
            stats.muscle_crush_ruptures.to_string(),
            stats.cavity_pressure_events.to_string(),
            stats.cavity_ruptures.to_string(),
            format!("{:.5}", metrics.cavity_pressure),
            format!("{:.5}", metrics.cavity_collapse),
            stats.organ_damage_events.to_string(),
            stats.organ_penetrations.to_string(),
            stats.rib_organ_punctures.to_string(),
            stats.organ_ruptures.to_string(),
            format!("{:.5}", metrics.organ_damage),
            stats.skin_flap_detachments.to_string(),
            stats.vessel_lacerations.to_string(),
            stats.fragment_vessel_lacerations.to_string(),
            stats.fragment_skin_punctures.to_string(),
            stats.bone_joint_subluxations.to_string(),
            stats.fracture_marrow_sources.to_string(),
            stats.contusion_events.to_string(),
            stats.opened_wounds.to_string(),
            stats.emitted_fluid_particles.to_string(),
            stats.wound_fluid_particles.to_string(),
            format!("{:.5}", stats.blood_loss),
            format!("{:.5}", capture.world.blood_volume_fraction()),
            format!("{:.5}", capture.world.blood_turgor_scale()),
            stats.blood_stain_deposits.to_string(),
        ];
        writeln!(out, "{}", fields.join(","))?;
    }
    Ok(())
}

fn max_cavity_pressure(world: &rp::World) -> f64 {
    world
        .cavities()
        .iter()
        .map(|cavity| cavity.pressure)
        .fold(0.0, f64::max)
}

fn max_cavity_collapse(world: &rp::World) -> f64 {
    world
        .cavities()
        .iter()
        .map(|cavity| cavity.collapse)
        .fold(0.0, f64::max)
}

fn max_organ_damage(world: &rp::World) -> f64 {
    world
        .organs()
        .iter()
        .map(|organ| organ.damage)
        .fold(0.0, f64::max)
}

fn write_triangle(
    out: &mut String,
    world: &rp::World,
    triangle: &rp::Triangle,
    fill: &str,
    opacity: f64,
    stroke: &str,
    stroke_opacity: f64,
) {
    let points = world.points();
    let a = points[triangle.a].position;
    let b = points[triangle.b].position;
    let c = points[triangle.c].position;
    writeln!(
        out,
        "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"1\"/>",
        a.x, a.y, b.x, b.y, c.x, c.y, fill, opacity, stroke, stroke_opacity
    )
    .expect("write triangle");
}

fn write_line(out: &mut String, a: rp::Vec2, b: rp::Vec2, width: f64, stroke: &str, opacity: f64) {
    writeln!(
        out,
        "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\" stroke-linecap=\"round\"/>",
        a.x, a.y, b.x, b.y, stroke, opacity.clamp(0.0, 1.0), width
    )
    .expect("write line");
}

fn write_circle(out: &mut String, center: rp::Vec2, radius: f64, fill: &str, opacity: f64) {
    writeln!(
        out,
        "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\"/>",
        center.x,
        center.y,
        radius.max(0.0),
        fill,
        opacity.clamp(0.0, 1.0)
    )
    .expect("write circle");
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

fn muscle_detail_amount(load: f64, exposure: f64, damage: f64) -> f64 {
    (damage * 0.75 + exposure * 0.85 + load / 1800.0).clamp(0.0, 1.0)
}

fn muscle_fiber_row_count(detail: f64) -> usize {
    if detail > 0.82 {
        4
    } else if detail > 0.46 {
        3
    } else {
        2
    }
}

fn longest_edge_length(world: &rp::World, triangle: &rp::Triangle) -> f64 {
    let points = world.points();
    let a = points[triangle.a].position;
    let b = points[triangle.b].position;
    let c = points[triangle.c].position;
    rp::distance(a, b)
        .max(rp::distance(b, c))
        .max(rp::distance(c, a))
}

fn wound_edge_fiber_tick_count(a: rp::Point, b: rp::Point) -> usize {
    let delta = subtract(b.position, a.position);
    let len = length(delta);
    if len < 2.0 {
        return 0;
    }
    let exposure = a.exposure.max(b.exposure).clamp(0.0, 1.0);
    let load = a.load.max(b.load);
    let severity = (exposure * 0.58 + load / 1700.0).clamp(0.0, 1.0);
    if severity <= 0.28 {
        0
    } else if severity > 0.68 {
        3
    } else {
        2
    }
}

fn free_fragment(bone: &rp::BoneSegment) -> bool {
    !bone.pinned && (bone.fractured || bone.splinter)
}

fn tool_name(tool: rp::ToolMode) -> &'static str {
    match tool {
        rp::ToolMode::Blunt => "blunt",
        rp::ToolMode::Sharp => "sharp",
        rp::ToolMode::Heavy => "heavy",
    }
}

fn v(x: f64, y: f64) -> rp::Vec2 {
    rp::Vec2 { x, y }
}

fn add(a: rp::Vec2, b: rp::Vec2) -> rp::Vec2 {
    rp::Vec2 {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}

fn subtract(a: rp::Vec2, b: rp::Vec2) -> rp::Vec2 {
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

fn length(value: rp::Vec2) -> f64 {
    value.x.hypot(value.y)
}

fn normalized(value: rp::Vec2, fallback: rp::Vec2) -> rp::Vec2 {
    let len = length(value);
    if len <= 0.0001 {
        fallback
    } else {
        scale(value, 1.0 / len)
    }
}
