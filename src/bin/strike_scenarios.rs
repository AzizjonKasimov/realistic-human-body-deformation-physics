use realistic_physics as rp;
use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
struct IntBand {
    min: i32,
    max: i32,
}

impl Default for IntBand {
    fn default() -> Self {
        Self {
            min: 0,
            max: i32::MAX,
        }
    }
}

#[derive(Clone, Copy)]
struct DoubleBand {
    min: f64,
    max: f64,
}

impl Default for DoubleBand {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: f64::INFINITY,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct ScenarioExpectations {
    contacts: IntBand,
    bone_fractures: IntBand,
    skin_tears: IntBand,
    fluid_emitted: IntBand,
    wound_fluid: IntBand,
    opened_wounds: IntBand,
    fragment_pair_contacts: IntBand,
    joint_corrections: IntBand,
    fragment_overlap: DoubleBand,
    bone_spin: DoubleBand,
}

#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    region: &'static str,
    intent: &'static str,
    tool: rp::ToolMode,
    start: rp::Vec2,
    end: rp::Vec2,
    windup_frames: i32,
    strike_frames: i32,
    settle_frames: i32,
    power: f64,
    expectations: ScenarioExpectations,
}

#[derive(Default)]
struct ScenarioResult {
    tissue_contacts: i32,
    bone_contacts: i32,
    fractures: i32,
    skin_tears: i32,
    muscle_tears: i32,
    detachments: i32,
    bone_detachments: i32,
    bone_joint_breaks: i32,
    bone_fractures: i32,
    final_bones: i32,
    fluid_emitted: i32,
    wound_fluid: i32,
    opened_wounds: i32,
    max_active_wounds: i32,
    wound_leaks: i32,
    max_active_fluids: i32,
    fragment_hits: i32,
    fragment_tears: i32,
    fragment_pair_contacts: i32,
    post_fracture_joint_corrections: i32,
    max_impact: f64,
    max_bone_load: f64,
    max_point_load: f64,
    max_depth: f64,
    max_fragment_depth: f64,
    max_fragment_impulse: f64,
    max_fragment_overlap: f64,
    max_post_fracture_joint_stretch: f64,
    max_post_fracture_joint_angle: f64,
    max_wound_pressure: f64,
    max_wound_clot: f64,
    max_bone_angular_speed: f64,
    final_free_fragments: i32,
    final_spinning_fragments: i32,
}

fn main() {
    let csv_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("output/strike_scenarios.csv"));
    if let Some(parent) = csv_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).expect("create output directory");
        }
    }

    let summary_path = csv_path
        .parent()
        .unwrap_or_else(|| Path::new("output"))
        .join("strike_summary.csv");
    let report_path = csv_path
        .parent()
        .unwrap_or_else(|| Path::new("output"))
        .join("strike_tuning_report.txt");

    let mut csv = BufWriter::new(File::create(&csv_path).expect("create strike CSV"));
    write_frame_header(&mut csv).expect("write frame header");
    let mut summary_rows = Vec::new();
    let mut warnings = Vec::new();

    for scenario in scenarios() {
        let result = run_scenario(&scenario, &mut csv).expect("run scenario");
        validate_result(&scenario, &result, &mut warnings);
        summary_rows.push((scenario, result));
    }

    write_summary(&summary_path, &summary_rows).expect("write summary");
    write_report(&report_path, &warnings).expect("write report");
    println!("wrote {}", csv_path.display());
    println!("wrote {}", summary_path.display());
    println!("wrote {}", report_path.display());

    if warnings.is_empty() {
        println!("PASS: strike scenarios are inside expected bands");
    } else {
        println!("WARN: {} strike tuning warnings", warnings.len());
    }
}

fn scenarios() -> Vec<Scenario> {
    let e = ScenarioExpectations {
        contacts: IntBand {
            min: 1,
            max: i32::MAX,
        },
        fragment_overlap: DoubleBand {
            min: 0.0,
            max: 18.0,
        },
        bone_spin: DoubleBand {
            min: 0.0,
            max: 28.0,
        },
        ..ScenarioExpectations::default()
    };
    vec![
        Scenario {
            name: "torso_blunt_medium",
            region: "torso",
            intent: "medium",
            tool: rp::ToolMode::Blunt,
            start: v(515.0, 360.0),
            end: v(690.0, 360.0),
            windup_frames: 12,
            strike_frames: 34,
            settle_frames: 42,
            power: 3.0,
            expectations: e,
        },
        Scenario {
            name: "torso_heavy_high",
            region: "torso",
            intent: "high",
            tool: rp::ToolMode::Heavy,
            start: v(500.0, 365.0),
            end: v(720.0, 365.0),
            windup_frames: 12,
            strike_frames: 30,
            settle_frames: 54,
            power: 4.0,
            expectations: e,
        },
        Scenario {
            name: "torso_sharp_cut",
            region: "torso",
            intent: "cut",
            tool: rp::ToolMode::Sharp,
            start: v(575.0, 300.0),
            end: v(710.0, 410.0),
            windup_frames: 8,
            strike_frames: 28,
            settle_frames: 48,
            power: 3.0,
            expectations: e,
        },
        Scenario {
            name: "shoulder_blunt",
            region: "shoulder",
            intent: "medium",
            tool: rp::ToolMode::Blunt,
            start: v(480.0, 270.0),
            end: v(650.0, 285.0),
            windup_frames: 10,
            strike_frames: 30,
            settle_frames: 42,
            power: 3.2,
            expectations: e,
        },
        Scenario {
            name: "arm_sharp",
            region: "arm",
            intent: "cut",
            tool: rp::ToolMode::Sharp,
            start: v(592.0, 282.0),
            end: v(528.0, 446.0),
            windup_frames: 8,
            strike_frames: 32,
            settle_frames: 42,
            power: 3.4,
            expectations: e,
        },
        Scenario {
            name: "hip_heavy",
            region: "hip",
            intent: "high",
            tool: rp::ToolMode::Heavy,
            start: v(500.0, 470.0),
            end: v(690.0, 490.0),
            windup_frames: 10,
            strike_frames: 30,
            settle_frames: 54,
            power: 4.0,
            expectations: e,
        },
        Scenario {
            name: "leg_blunt",
            region: "leg",
            intent: "medium",
            tool: rp::ToolMode::Blunt,
            start: v(545.0, 575.0),
            end: v(660.0, 620.0),
            windup_frames: 10,
            strike_frames: 32,
            settle_frames: 42,
            power: 3.4,
            expectations: e,
        },
    ]
}

fn run_scenario(scenario: &Scenario, csv: &mut dyn Write) -> std::io::Result<ScenarioResult> {
    let width = 1280.0;
    let height = 720.0;
    let mut world = rp::create_layered_body(width, height, rp::Materials::default());
    let mut result = ScenarioResult::default();
    let dt = world.materials().fixed_dt;
    let total_frames = scenario.windup_frames + scenario.strike_frames + scenario.settle_frames;

    for frame in 0..total_frames {
        let input = if frame < scenario.windup_frames + scenario.strike_frames {
            make_strike_input(scenario, frame, dt)
        } else {
            rp::InputState::default()
        };
        world.step(dt, &input, width, height);
        accumulate_result(&world, &mut result);
        write_frame(csv, scenario, frame, &world)?;
    }

    let stats = world.stats();
    result.skin_tears = stats.broken_skin;
    result.muscle_tears = stats.broken_muscle;
    result.detachments = stats.broken_attachments;
    result.bone_detachments = stats.broken_bone_attachments;
    result.bone_joint_breaks = stats.broken_bone_joints;
    result.bone_fractures = stats.fractured_bones;
    result.final_bones = world.bones().len() as i32;
    result.fluid_emitted = stats.emitted_fluid_particles;
    result.wound_fluid = stats.wound_fluid_particles;
    result.opened_wounds = stats.opened_wounds;
    result.fragment_hits = stats.fragment_tissue_hits;
    result.fragment_tears = stats.fragment_tissue_tears;
    result.final_free_fragments = free_fragment_count(&world);
    result.final_spinning_fragments = spinning_fragment_count(&world);
    Ok(result)
}

fn make_strike_input(scenario: &Scenario, frame: i32, dt: f64) -> rp::InputState {
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

fn accumulate_result(world: &rp::World, result: &mut ScenarioResult) {
    let debug = world.debug();
    result.tissue_contacts += debug.tissue_contacts;
    result.bone_contacts += debug.bone_contacts;
    result.fractures += debug.fractures;
    result.max_impact = result.max_impact.max(debug.impact);
    result.max_bone_load = result.max_bone_load.max(debug.max_bone_load);
    result.max_point_load = result.max_point_load.max(debug.max_point_load);
    result.max_depth = result.max_depth.max(debug.max_depth);
    result.max_fragment_depth = result.max_fragment_depth.max(debug.max_fragment_depth);
    result.max_fragment_impulse = result.max_fragment_impulse.max(debug.max_fragment_impulse);
    result.max_fragment_overlap = result.max_fragment_overlap.max(debug.max_fragment_overlap);
    result.max_post_fracture_joint_stretch = result
        .max_post_fracture_joint_stretch
        .max(debug.max_post_fracture_joint_stretch);
    result.max_post_fracture_joint_angle = result
        .max_post_fracture_joint_angle
        .max(debug.max_post_fracture_joint_angle);
    result.max_wound_pressure = result.max_wound_pressure.max(debug.max_wound_pressure);
    result.max_wound_clot = result.max_wound_clot.max(debug.max_wound_clot);
    result.max_bone_angular_speed = result
        .max_bone_angular_speed
        .max(debug.max_bone_angular_speed);
    result.max_active_wounds = result.max_active_wounds.max(debug.active_wounds);
    result.wound_leaks += debug.wound_leaks;
    result.fragment_pair_contacts += debug.fragment_pair_contacts;
    result.post_fracture_joint_corrections += debug.post_fracture_joint_corrections;
    result.max_active_fluids = result.max_active_fluids.max(active_fluid_count(world));
}

fn write_frame_header(csv: &mut dyn Write) -> std::io::Result<()> {
    writeln!(csv, "scenario,region,intent,tool,frame,striker_x,striker_y,striker_speed,impact,tissue_contacts,bone_contacts,max_depth,max_point_load,max_bone_load,fractures,skin_tears,muscle_tears,detachments,bone_detachments,bone_joint_breaks,bone_fractures,fluid_emitted_frame,active_fluids,total_fluid,opened_wounds,active_wounds,wound_leaks,wound_fluid,max_wound_pressure,max_wound_clot,fragment_contacts,fragment_tears,fragment_pair_contacts,max_fragment_depth,max_fragment_impulse,max_fragment_overlap,post_fracture_joint_corrections,max_post_fracture_joint_stretch,max_post_fracture_joint_angle,fragment_hits,fragment_tissue_tears,max_bone_angular_speed,free_fragments,spinning_fragments")
}

fn write_frame(
    csv: &mut dyn Write,
    scenario: &Scenario,
    frame: i32,
    world: &rp::World,
) -> std::io::Result<()> {
    let debug = world.debug();
    let stats = world.stats();
    writeln!(
        csv,
        "{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{},{},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{},{},{},{:.3},{:.3},{:.3},{},{:.3},{:.3},{},{},{:.3},{},{}",
        scenario.name,
        scenario.region,
        scenario.intent,
        tool_name(debug.tool),
        frame,
        debug.striker_position.x,
        debug.striker_position.y,
        debug.striker_speed,
        debug.impact,
        debug.tissue_contacts,
        debug.bone_contacts,
        debug.max_depth,
        debug.max_point_load,
        debug.max_bone_load,
        debug.fractures,
        stats.broken_skin,
        stats.broken_muscle,
        stats.broken_attachments,
        stats.broken_bone_attachments,
        stats.broken_bone_joints,
        stats.fractured_bones,
        debug.fluid_emitted,
        active_fluid_count(world),
        stats.emitted_fluid_particles,
        stats.opened_wounds,
        debug.active_wounds,
        debug.wound_leaks,
        stats.wound_fluid_particles,
        debug.max_wound_pressure,
        debug.max_wound_clot,
        debug.fragment_contacts,
        debug.fragment_tears,
        debug.fragment_pair_contacts,
        debug.max_fragment_depth,
        debug.max_fragment_impulse,
        debug.max_fragment_overlap,
        debug.post_fracture_joint_corrections,
        debug.max_post_fracture_joint_stretch,
        debug.max_post_fracture_joint_angle,
        stats.fragment_tissue_hits,
        stats.fragment_tissue_tears,
        debug.max_bone_angular_speed,
        free_fragment_count(world),
        spinning_fragment_count(world)
    )
}

fn write_summary(path: &Path, rows: &[(Scenario, ScenarioResult)]) -> std::io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "scenario,region,intent,tool,tissue_contacts,bone_contacts,skin_tears,muscle_tears,detachments,bone_detachments,bone_joint_breaks,bone_fractures,final_bones,fluid_emitted,wound_fluid,opened_wounds,max_active_wounds,wound_leaks,fragment_hits,fragment_tears,fragment_pair_contacts,post_fracture_joint_corrections,max_impact,max_bone_load,max_point_load,max_depth,max_fragment_depth,max_fragment_impulse,max_fragment_overlap,max_post_fracture_joint_stretch,max_post_fracture_joint_angle,max_wound_pressure,max_wound_clot,max_bone_angular_speed,final_free_fragments,final_spinning_fragments")?;
    for (scenario, result) in rows {
        writeln!(
            out,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{}",
            scenario.name,
            scenario.region,
            scenario.intent,
            tool_name(scenario.tool),
            result.tissue_contacts,
            result.bone_contacts,
            result.skin_tears,
            result.muscle_tears,
            result.detachments,
            result.bone_detachments,
            result.bone_joint_breaks,
            result.bone_fractures,
            result.final_bones,
            result.fluid_emitted,
            result.wound_fluid,
            result.opened_wounds,
            result.max_active_wounds,
            result.wound_leaks,
            result.fragment_hits,
            result.fragment_tears,
            result.fragment_pair_contacts,
            result.post_fracture_joint_corrections,
            result.max_impact,
            result.max_bone_load,
            result.max_point_load,
            result.max_depth,
            result.max_fragment_depth,
            result.max_fragment_impulse,
            result.max_fragment_overlap,
            result.max_post_fracture_joint_stretch,
            result.max_post_fracture_joint_angle,
            result.max_wound_pressure,
            result.max_wound_clot,
            result.max_bone_angular_speed,
            result.final_free_fragments,
            result.final_spinning_fragments
        )?;
    }
    Ok(())
}

fn write_report(path: &Path, warnings: &[String]) -> std::io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    if warnings.is_empty() {
        writeln!(out, "All strike scenarios are inside expected bands.")?;
    } else {
        for warning in warnings {
            writeln!(out, "{warning}")?;
        }
    }
    Ok(())
}

fn validate_result(scenario: &Scenario, result: &ScenarioResult, warnings: &mut Vec<String>) {
    check_int(
        scenario,
        "contacts",
        result.tissue_contacts + result.bone_contacts,
        scenario.expectations.contacts,
        warnings,
    );
    check_int(
        scenario,
        "bone_fractures",
        result.bone_fractures,
        scenario.expectations.bone_fractures,
        warnings,
    );
    check_int(
        scenario,
        "skin_tears",
        result.skin_tears,
        scenario.expectations.skin_tears,
        warnings,
    );
    check_int(
        scenario,
        "fluid_emitted",
        result.fluid_emitted,
        scenario.expectations.fluid_emitted,
        warnings,
    );
    check_int(
        scenario,
        "wound_fluid",
        result.wound_fluid,
        scenario.expectations.wound_fluid,
        warnings,
    );
    check_int(
        scenario,
        "opened_wounds",
        result.opened_wounds,
        scenario.expectations.opened_wounds,
        warnings,
    );
    check_int(
        scenario,
        "fragment_pair_contacts",
        result.fragment_pair_contacts,
        scenario.expectations.fragment_pair_contacts,
        warnings,
    );
    check_int(
        scenario,
        "joint_corrections",
        result.post_fracture_joint_corrections,
        scenario.expectations.joint_corrections,
        warnings,
    );
    check_double(
        scenario,
        "fragment_overlap",
        result.max_fragment_overlap,
        scenario.expectations.fragment_overlap,
        warnings,
    );
    check_double(
        scenario,
        "bone_spin",
        result.max_bone_angular_speed,
        scenario.expectations.bone_spin,
        warnings,
    );
}

fn check_int(
    scenario: &Scenario,
    name: &str,
    value: i32,
    band: IntBand,
    warnings: &mut Vec<String>,
) {
    if value < band.min || value > band.max {
        warnings.push(format!(
            "{}: {}={} outside {}..{}",
            scenario.name, name, value, band.min, band.max
        ));
    }
}

fn check_double(
    scenario: &Scenario,
    name: &str,
    value: f64,
    band: DoubleBand,
    warnings: &mut Vec<String>,
) {
    if value < band.min || value > band.max {
        warnings.push(format!(
            "{}: {}={:.3} outside {:.3}..{:.3}",
            scenario.name, name, value, band.min, band.max
        ));
    }
}

fn active_fluid_count(world: &rp::World) -> i32 {
    world
        .fluids()
        .iter()
        .filter(|fluid| fluid.life > 0.0)
        .count() as i32
}

fn free_fragment_count(world: &rp::World) -> i32 {
    world
        .bones()
        .iter()
        .filter(|bone| free_fragment(bone))
        .count() as i32
}

fn spinning_fragment_count(world: &rp::World) -> i32 {
    world
        .bones()
        .iter()
        .filter(|bone| free_fragment(bone) && bone.angular_velocity.abs() > 0.08)
        .count() as i32
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
