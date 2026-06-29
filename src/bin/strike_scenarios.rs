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
    rib_fractures: IntBand,
    skin_tears: IntBand,
    contusion_events: IntBand,
    tissue_fatigue_events: IntBand,
    tissue_plastic_events: IntBand,
    tear_propagations: IntBand,
    muscle_cut_transfers: IntBand,
    muscle_fiber_tears: IntBand,
    muscle_crush_ruptures: IntBand,
    cavity_pressure_events: IntBand,
    cavity_ruptures: IntBand,
    organ_damage_events: IntBand,
    organ_penetrations: IntBand,
    rib_organ_punctures: IntBand,
    organ_ruptures: IntBand,
    skin_flap_detachments: IntBand,
    vessel_lacerations: IntBand,
    fragment_vessel_lacerations: IntBand,
    wound_reopens: IntBand,
    fluid_emitted: IntBand,
    wound_fluid: IntBand,
    blood_stain_deposits: IntBand,
    fracture_marrow_sources: IntBand,
    opened_wounds: IntBand,
    final_free_fragments: IntBand,
    fragment_bone_contacts: IntBand,
    fragment_bone_damping_events: IntBand,
    fragment_bone_resting_contacts: IntBand,
    sleeping_fragments: IntBand,
    sleep_events: IntBand,
    final_sleeping_fragments: IntBand,
    fragment_pair_damping_events: IntBand,
    fragment_pair_resting_contacts: IntBand,
    fragment_floor_contacts: IntBand,
    fragment_floor_resting_contacts: IntBand,
    fragment_pair_contacts: IntBand,
    fragment_skin_punctures: IntBand,
    bone_joint_subluxations: IntBand,
    joint_ligament_damage_events: IntBand,
    joint_corrections: IntBand,
    fragment_overlap: DoubleBand,
    bone_spin: DoubleBand,
    bone_joint_subluxation: DoubleBand,
    tissue_softening: DoubleBand,
    tissue_fatigue: DoubleBand,
    tissue_plasticity: DoubleBand,
    cavity_pressure: DoubleBand,
    cavity_collapse: DoubleBand,
    organ_damage: DoubleBand,
    blood_loss: DoubleBand,
    final_blood_volume: DoubleBand,
    final_blood_turgor: DoubleBand,
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
    followup: Option<FollowupStrike>,
    expectations: ScenarioExpectations,
}

#[derive(Clone, Copy)]
struct FollowupStrike {
    tool: rp::ToolMode,
    start: rp::Vec2,
    end: rp::Vec2,
    windup_frames: i32,
    strike_frames: i32,
    settle_frames: i32,
    power: f64,
}

#[derive(Default)]
struct ScenarioResult {
    tissue_contacts: i32,
    bone_contacts: i32,
    fractures: i32,
    skin_tears: i32,
    muscle_tears: i32,
    muscle_fiber_tears: i32,
    contusion_events: i32,
    tissue_fatigue_events: i32,
    tissue_plastic_events: i32,
    tear_propagations: i32,
    muscle_cut_transfers: i32,
    muscle_crush_ruptures: i32,
    cavity_pressure_events: i32,
    cavity_ruptures: i32,
    organ_damage_events: i32,
    organ_penetrations: i32,
    rib_organ_punctures: i32,
    organ_ruptures: i32,
    skin_flap_detachments: i32,
    vessel_lacerations: i32,
    fragment_vessel_lacerations: i32,
    wound_reopens: i32,
    max_active_contusions: i32,
    detachments: i32,
    bone_detachments: i32,
    bone_joint_breaks: i32,
    bone_joint_subluxations: i32,
    joint_ligament_damage_events: i32,
    bone_fractures: i32,
    rib_fractures: i32,
    final_bones: i32,
    fluid_emitted: i32,
    wound_fluid: i32,
    blood_loss: f64,
    final_blood_volume: f64,
    final_blood_turgor: f64,
    blood_stain_deposits: i32,
    fracture_marrow_sources: i32,
    opened_wounds: i32,
    max_active_wounds: i32,
    wound_leaks: i32,
    max_active_fluids: i32,
    max_active_blood_stains: i32,
    fragment_hits: i32,
    fragment_tears: i32,
    fragment_skin_punctures: i32,
    fragment_bone_contacts: i32,
    fragment_bone_damping_events: i32,
    fragment_bone_resting_contacts: i32,
    fragment_pair_contacts: i32,
    fragment_pair_damping_events: i32,
    fragment_pair_resting_contacts: i32,
    fragment_floor_contacts: i32,
    fragment_floor_resting_contacts: i32,
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
    max_bone_joint_subluxation: f64,
    max_wound_pressure: f64,
    max_wound_clot: f64,
    max_cavity_pressure: f64,
    max_cavity_collapse: f64,
    max_organ_damage: f64,
    max_contusion: f64,
    max_tissue_softening: f64,
    max_tissue_fatigue: f64,
    max_tissue_plasticity: f64,
    max_bone_angular_speed: f64,
    final_free_fragments: i32,
    final_spinning_fragments: i32,
    final_sleeping_fragments: i32,
    max_active_fragments: i32,
    max_sleeping_fragments: i32,
    fragment_sleep_events: i32,
    fragment_wake_events: i32,
    fragment_budget_skips: i32,
    fracture_budget_blocks: i32,
    fragment_bone_checks: i32,
    fragment_bone_budget_skips: i32,
    fragment_pair_checks: i32,
    fragment_pair_budget_skips: i32,
    fragment_tissue_checks: i32,
    fragment_tissue_budget_skips: i32,
    fluid_budget_replacements: i32,
    blood_stain_budget_replacements: i32,
    wound_budget_replacements: i32,
    max_solver_iterations: i32,
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
        contusion_events: IntBand {
            min: 1,
            max: i32::MAX,
        },
        fragment_overlap: DoubleBand {
            min: 0.0,
            max: 18.0,
        },
        bone_spin: DoubleBand {
            min: 0.0,
            max: 38.0,
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
            followup: None,
            expectations: ScenarioExpectations {
                vessel_lacerations: IntBand { min: 0, max: 0 },
                fragment_vessel_lacerations: IntBand { min: 0, max: 0 },
                muscle_fiber_tears: IntBand { min: 20, max: 90 },
                bone_joint_subluxations: IntBand { min: 1, max: 4 },
                joint_ligament_damage_events: IntBand { min: 1, max: 4 },
                bone_joint_subluxation: DoubleBand {
                    min: 0.20,
                    max: 0.85,
                },
                rib_fractures: IntBand { min: 3, max: 6 },
                cavity_pressure_events: IntBand { min: 0, max: 30 },
                cavity_ruptures: IntBand { min: 0, max: 0 },
                cavity_pressure: DoubleBand {
                    min: 0.35,
                    max: 0.78,
                },
                cavity_collapse: DoubleBand {
                    min: 0.12,
                    max: 0.30,
                },
                organ_damage_events: IntBand { min: 30, max: 80 },
                organ_penetrations: IntBand { min: 0, max: 0 },
                rib_organ_punctures: IntBand { min: 0, max: 0 },
                organ_ruptures: IntBand { min: 0, max: 0 },
                organ_damage: DoubleBand {
                    min: 1.0,
                    max: 1.81,
                },
                ..e
            },
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
            followup: None,
            expectations: ScenarioExpectations {
                fragment_skin_punctures: IntBand { min: 6, max: 60 },
                rib_fractures: IntBand { min: 4, max: 7 },
                fracture_marrow_sources: IntBand { min: 16, max: 30 },
                muscle_crush_ruptures: IntBand { min: 12, max: 40 },
                muscle_fiber_tears: IntBand { min: 40, max: 120 },
                bone_joint_subluxations: IntBand { min: 1, max: 4 },
                joint_ligament_damage_events: IntBand { min: 1, max: 4 },
                bone_joint_subluxation: DoubleBand {
                    min: 0.20,
                    max: 0.95,
                },
                cavity_pressure_events: IntBand { min: 25, max: 70 },
                cavity_ruptures: IntBand { min: 1, max: 1 },
                cavity_pressure: DoubleBand {
                    min: 0.76,
                    max: 0.92,
                },
                cavity_collapse: DoubleBand {
                    min: 0.05,
                    max: 0.16,
                },
                organ_damage_events: IntBand { min: 60, max: 120 },
                organ_penetrations: IntBand { min: 0, max: 0 },
                rib_organ_punctures: IntBand { min: 1, max: 2 },
                organ_ruptures: IntBand { min: 1, max: 2 },
                organ_damage: DoubleBand {
                    min: 1.0,
                    max: 1.81,
                },
                vessel_lacerations: IntBand { min: 1, max: 4 },
                fragment_vessel_lacerations: IntBand { min: 1, max: 2 },
                blood_loss: DoubleBand {
                    min: 0.08,
                    max: 0.20,
                },
                final_blood_volume: DoubleBand {
                    min: 0.78,
                    max: 0.95,
                },
                final_blood_turgor: DoubleBand {
                    min: 0.89,
                    max: 0.99,
                },
                ..e
            },
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
            followup: None,
            expectations: ScenarioExpectations {
                tear_propagations: IntBand { min: 40, max: 160 },
                muscle_cut_transfers: IntBand { min: 60, max: 180 },
                skin_flap_detachments: IntBand { min: 80, max: 220 },
                muscle_fiber_tears: IntBand { min: 8, max: 60 },
                bone_joint_subluxations: IntBand { min: 0, max: 0 },
                joint_ligament_damage_events: IntBand { min: 0, max: 0 },
                vessel_lacerations: IntBand { min: 1, max: 4 },
                fragment_vessel_lacerations: IntBand { min: 0, max: 0 },
                rib_fractures: IntBand { min: 3, max: 6 },
                organ_penetrations: IntBand { min: 1, max: 3 },
                rib_organ_punctures: IntBand { min: 0, max: 0 },
                organ_ruptures: IntBand { min: 1, max: 2 },
                ..e
            },
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
            followup: None,
            expectations: ScenarioExpectations {
                vessel_lacerations: IntBand { min: 0, max: 0 },
                fragment_vessel_lacerations: IntBand { min: 0, max: 0 },
                muscle_fiber_tears: IntBand { min: 15, max: 80 },
                rib_fractures: IntBand { min: 2, max: 5 },
                organ_penetrations: IntBand { min: 0, max: 0 },
                rib_organ_punctures: IntBand { min: 0, max: 0 },
                organ_ruptures: IntBand { min: 0, max: 0 },
                bone_joint_subluxations: IntBand { min: 1, max: 3 },
                joint_ligament_damage_events: IntBand { min: 1, max: 3 },
                bone_joint_subluxation: DoubleBand {
                    min: 0.20,
                    max: 0.80,
                },
                ..e
            },
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
            followup: None,
            expectations: ScenarioExpectations {
                tear_propagations: IntBand { min: 40, max: 160 },
                muscle_cut_transfers: IntBand { min: 60, max: 180 },
                skin_flap_detachments: IntBand { min: 80, max: 220 },
                vessel_lacerations: IntBand { min: 0, max: 0 },
                fragment_vessel_lacerations: IntBand { min: 0, max: 0 },
                muscle_fiber_tears: IntBand { min: 1, max: 30 },
                bone_joint_subluxations: IntBand { min: 1, max: 2 },
                joint_ligament_damage_events: IntBand { min: 1, max: 2 },
                bone_joint_subluxation: DoubleBand {
                    min: 0.20,
                    max: 0.90,
                },
                rib_fractures: IntBand { min: 0, max: 0 },
                organ_penetrations: IntBand { min: 0, max: 0 },
                rib_organ_punctures: IntBand { min: 0, max: 0 },
                ..e
            },
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
            followup: None,
            expectations: ScenarioExpectations {
                vessel_lacerations: IntBand { min: 1, max: 3 },
                fragment_vessel_lacerations: IntBand { min: 1, max: 2 },
                muscle_fiber_tears: IntBand { min: 10, max: 70 },
                rib_fractures: IntBand { min: 1, max: 3 },
                bone_joint_subluxations: IntBand { min: 2, max: 6 },
                joint_ligament_damage_events: IntBand { min: 2, max: 6 },
                bone_joint_subluxation: DoubleBand {
                    min: 0.20,
                    max: 0.85,
                },
                cavity_ruptures: IntBand { min: 0, max: 0 },
                cavity_collapse: DoubleBand {
                    min: 0.0,
                    max: 0.14,
                },
                organ_penetrations: IntBand { min: 0, max: 0 },
                rib_organ_punctures: IntBand { min: 0, max: 0 },
                organ_ruptures: IntBand { min: 0, max: 0 },
                ..e
            },
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
            followup: None,
            expectations: ScenarioExpectations {
                vessel_lacerations: IntBand { min: 0, max: 0 },
                blood_loss: DoubleBand {
                    min: 0.0,
                    max: 0.02,
                },
                final_blood_volume: DoubleBand {
                    min: 0.98,
                    max: 1.0,
                },
                final_blood_turgor: DoubleBand {
                    min: 0.99,
                    max: 1.0,
                },
                fragment_vessel_lacerations: IntBand { min: 0, max: 0 },
                muscle_fiber_tears: IntBand { min: 0, max: 0 },
                bone_joint_subluxations: IntBand { min: 0, max: 0 },
                joint_ligament_damage_events: IntBand { min: 0, max: 0 },
                cavity_pressure_events: IntBand { min: 0, max: 0 },
                cavity_ruptures: IntBand { min: 0, max: 0 },
                organ_damage_events: IntBand { min: 0, max: 0 },
                rib_fractures: IntBand { min: 0, max: 0 },
                organ_penetrations: IntBand { min: 0, max: 0 },
                rib_organ_punctures: IntBand { min: 0, max: 0 },
                organ_ruptures: IntBand { min: 0, max: 0 },
                cavity_pressure: DoubleBand {
                    min: 0.0,
                    max: 0.05,
                },
                ..e
            },
        },
        Scenario {
            name: "torso_cut_rebleed",
            region: "torso",
            intent: "rebleed",
            tool: rp::ToolMode::Sharp,
            start: v(575.0, 300.0),
            end: v(710.0, 410.0),
            windup_frames: 8,
            strike_frames: 28,
            settle_frames: 190,
            power: 3.0,
            followup: Some(FollowupStrike {
                tool: rp::ToolMode::Blunt,
                start: v(710.0, 410.0),
                end: v(575.0, 300.0),
                windup_frames: 8,
                strike_frames: 24,
                settle_frames: 48,
                power: 3.0,
            }),
            expectations: ScenarioExpectations {
                tear_propagations: IntBand { min: 40, max: 160 },
                muscle_cut_transfers: IntBand { min: 60, max: 180 },
                skin_flap_detachments: IntBand { min: 80, max: 220 },
                vessel_lacerations: IntBand { min: 1, max: 4 },
                fragment_vessel_lacerations: IntBand { min: 0, max: 0 },
                muscle_fiber_tears: IntBand { min: 10, max: 70 },
                bone_joint_subluxations: IntBand { min: 0, max: 0 },
                joint_ligament_damage_events: IntBand { min: 0, max: 0 },
                wound_reopens: IntBand { min: 12, max: 140 },
                blood_loss: DoubleBand {
                    min: 0.18,
                    max: 0.36,
                },
                final_blood_volume: DoubleBand {
                    min: 0.62,
                    max: 0.84,
                },
                final_blood_turgor: DoubleBand {
                    min: 0.82,
                    max: 0.94,
                },
                cavity_ruptures: IntBand { min: 0, max: 0 },
                cavity_pressure: DoubleBand {
                    min: 0.60,
                    max: 0.84,
                },
                cavity_collapse: DoubleBand {
                    min: 0.0,
                    max: 0.14,
                },
                organ_penetrations: IntBand { min: 1, max: 3 },
                rib_organ_punctures: IntBand { min: 0, max: 0 },
                organ_ruptures: IntBand { min: 1, max: 2 },
                rib_fractures: IntBand { min: 4, max: 6 },
                bone_spin: DoubleBand {
                    min: 0.0,
                    max: 32.0,
                },
                ..e
            },
        },
        Scenario {
            name: "torso_heavy_fragment_settle",
            region: "torso",
            intent: "settle",
            tool: rp::ToolMode::Heavy,
            start: v(500.0, 365.0),
            end: v(720.0, 365.0),
            windup_frames: 12,
            strike_frames: 30,
            settle_frames: 260,
            power: 4.0,
            followup: None,
            expectations: ScenarioExpectations {
                bone_fractures: IntBand {
                    min: 8,
                    max: i32::MAX,
                },
                blood_stain_deposits: IntBand {
                    min: 350,
                    max: i32::MAX,
                },
                contusion_events: IntBand {
                    min: 900,
                    max: i32::MAX,
                },
                tissue_fatigue_events: IntBand {
                    min: 1000,
                    max: i32::MAX,
                },
                tissue_plastic_events: IntBand {
                    min: 30,
                    max: i32::MAX,
                },
                tissue_softening: DoubleBand {
                    min: 0.10,
                    max: 0.52,
                },
                tissue_fatigue: DoubleBand {
                    min: 0.08,
                    max: 1.35,
                },
                tissue_plasticity: DoubleBand {
                    min: 0.001,
                    max: 0.12,
                },
                final_free_fragments: IntBand { min: 18, max: 40 },
                fragment_bone_contacts: IntBand {
                    min: 1000,
                    max: i32::MAX,
                },
                fragment_bone_damping_events: IntBand {
                    min: 4000,
                    max: i32::MAX,
                },
                fragment_bone_resting_contacts: IntBand {
                    min: 3500,
                    max: i32::MAX,
                },
                sleeping_fragments: IntBand { min: 8, max: 40 },
                sleep_events: IntBand { min: 8, max: 40 },
                final_sleeping_fragments: IntBand { min: 8, max: 40 },
                fragment_pair_damping_events: IntBand {
                    min: 1000,
                    max: i32::MAX,
                },
                fragment_pair_resting_contacts: IntBand {
                    min: 7000,
                    max: i32::MAX,
                },
                fragment_floor_contacts: IntBand {
                    min: 500,
                    max: i32::MAX,
                },
                fragment_floor_resting_contacts: IntBand {
                    min: 500,
                    max: i32::MAX,
                },
                fragment_skin_punctures: IntBand { min: 6, max: 60 },
                rib_fractures: IntBand { min: 4, max: 7 },
                fracture_marrow_sources: IntBand { min: 16, max: 30 },
                muscle_crush_ruptures: IntBand { min: 12, max: 40 },
                muscle_fiber_tears: IntBand { min: 40, max: 120 },
                bone_joint_subluxations: IntBand { min: 1, max: 4 },
                joint_ligament_damage_events: IntBand { min: 1, max: 4 },
                bone_joint_subluxation: DoubleBand {
                    min: 0.20,
                    max: 0.95,
                },
                cavity_pressure_events: IntBand { min: 25, max: 80 },
                cavity_ruptures: IntBand { min: 1, max: 1 },
                cavity_pressure: DoubleBand {
                    min: 0.76,
                    max: 0.92,
                },
                cavity_collapse: DoubleBand {
                    min: 0.05,
                    max: 0.16,
                },
                organ_damage_events: IntBand { min: 60, max: 120 },
                organ_penetrations: IntBand { min: 0, max: 0 },
                rib_organ_punctures: IntBand { min: 1, max: 2 },
                organ_ruptures: IntBand { min: 1, max: 2 },
                organ_damage: DoubleBand {
                    min: 1.0,
                    max: 1.81,
                },
                vessel_lacerations: IntBand { min: 1, max: 4 },
                fragment_vessel_lacerations: IntBand { min: 1, max: 2 },
                blood_loss: DoubleBand {
                    min: 0.20,
                    max: 0.40,
                },
                final_blood_volume: DoubleBand {
                    min: 0.60,
                    max: 0.82,
                },
                final_blood_turgor: DoubleBand {
                    min: 0.80,
                    max: 0.93,
                },
                ..e
            },
        },
    ]
}

fn run_scenario(scenario: &Scenario, csv: &mut dyn Write) -> std::io::Result<ScenarioResult> {
    let width = 1280.0;
    let height = 720.0;
    let mut world = rp::create_layered_body(width, height, rp::Materials::default());
    let mut result = ScenarioResult::default();
    let dt = world.materials().fixed_dt;
    let primary_frames = scenario.windup_frames + scenario.strike_frames + scenario.settle_frames;
    let followup_frames = scenario
        .followup
        .map(|followup| followup.windup_frames + followup.strike_frames + followup.settle_frames)
        .unwrap_or(0);
    let total_frames = primary_frames + followup_frames;

    for frame in 0..total_frames {
        let input = if frame < scenario.windup_frames + scenario.strike_frames {
            make_strike_input(scenario, frame, dt)
        } else if let Some(followup) = scenario.followup {
            let followup_frame = frame - primary_frames;
            if followup_frame >= 0
                && followup_frame < followup.windup_frames + followup.strike_frames
            {
                make_followup_input(followup, followup_frame, dt)
            } else {
                rp::InputState::default()
            }
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
    result.muscle_fiber_tears = stats.muscle_fiber_tears;
    result.contusion_events = stats.contusion_events;
    result.tissue_fatigue_events = stats.tissue_fatigue_events;
    result.tissue_plastic_events = stats.tissue_plastic_events;
    result.tear_propagations = stats.tear_propagations;
    result.muscle_cut_transfers = stats.muscle_cut_transfers;
    result.muscle_crush_ruptures = stats.muscle_crush_ruptures;
    result.cavity_pressure_events = stats.cavity_pressure_events;
    result.cavity_ruptures = stats.cavity_ruptures;
    result.organ_damage_events = stats.organ_damage_events;
    result.organ_penetrations = stats.organ_penetrations;
    result.rib_organ_punctures = stats.rib_organ_punctures;
    result.organ_ruptures = stats.organ_ruptures;
    result.skin_flap_detachments = stats.skin_flap_detachments;
    result.vessel_lacerations = stats.vessel_lacerations;
    result.fragment_vessel_lacerations = stats.fragment_vessel_lacerations;
    result.wound_reopens = stats.wound_reopens;
    result.detachments = stats.broken_attachments;
    result.bone_detachments = stats.broken_bone_attachments;
    result.bone_joint_breaks = stats.broken_bone_joints;
    result.bone_joint_subluxations = stats.bone_joint_subluxations;
    result.joint_ligament_damage_events = stats.joint_ligament_damage_events;
    result.bone_fractures = stats.fractured_bones;
    result.rib_fractures = stats.fractured_ribs;
    result.final_bones = world.bones().len() as i32;
    result.fluid_emitted = stats.emitted_fluid_particles;
    result.wound_fluid = stats.wound_fluid_particles;
    result.blood_loss = stats.blood_loss;
    result.final_blood_volume = world.blood_volume_fraction();
    result.final_blood_turgor = world.blood_turgor_scale();
    result.blood_stain_deposits = stats.blood_stain_deposits;
    result.fracture_marrow_sources = stats.fracture_marrow_sources;
    result.opened_wounds = stats.opened_wounds;
    result.fragment_hits = stats.fragment_tissue_hits;
    result.fragment_tears = stats.fragment_tissue_tears;
    result.fragment_skin_punctures = stats.fragment_skin_punctures;
    result.final_free_fragments = free_fragment_count(&world);
    result.final_spinning_fragments = spinning_fragment_count(&world);
    result.final_sleeping_fragments = sleeping_fragment_count(&world);
    Ok(result)
}

fn make_strike_input(scenario: &Scenario, frame: i32, dt: f64) -> rp::InputState {
    make_pass_input(
        scenario.tool,
        scenario.start,
        scenario.end,
        scenario.windup_frames,
        scenario.strike_frames,
        scenario.power,
        frame,
        dt,
    )
}

fn make_followup_input(followup: FollowupStrike, frame: i32, dt: f64) -> rp::InputState {
    make_pass_input(
        followup.tool,
        followup.start,
        followup.end,
        followup.windup_frames,
        followup.strike_frames,
        followup.power,
        frame,
        dt,
    )
}

fn make_pass_input(
    tool: rp::ToolMode,
    start: rp::Vec2,
    end: rp::Vec2,
    windup_frames: i32,
    strike_frames: i32,
    power: f64,
    frame: i32,
    dt: f64,
) -> rp::InputState {
    let t0 = (frame - windup_frames).max(0) as f64 / (strike_frames - 1).max(1) as f64;
    let t = t0.clamp(0.0, 1.0);
    let position = rp::Vec2 {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
    };
    let velocity = rp::Vec2 {
        x: (end.x - start.x) / ((strike_frames - 1).max(1) as f64 * dt),
        y: (end.y - start.y) / ((strike_frames - 1).max(1) as f64 * dt),
    };
    let down = frame >= windup_frames && frame < windup_frames + strike_frames;
    rp::InputState {
        active: down,
        down,
        x: position.x,
        y: position.y,
        vx: if down { velocity.x } else { 0.0 },
        vy: if down { velocity.y } else { 0.0 },
        power,
        tool,
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
    result.max_bone_joint_subluxation = result
        .max_bone_joint_subluxation
        .max(debug.max_bone_joint_subluxation);
    result.max_wound_pressure = result.max_wound_pressure.max(debug.max_wound_pressure);
    result.max_wound_clot = result.max_wound_clot.max(debug.max_wound_clot);
    result.max_cavity_pressure = result.max_cavity_pressure.max(debug.max_cavity_pressure);
    result.max_cavity_collapse = result.max_cavity_collapse.max(debug.max_cavity_collapse);
    result.max_organ_damage = result.max_organ_damage.max(debug.max_organ_damage);
    result.max_contusion = result.max_contusion.max(debug.max_contusion);
    result.max_tissue_softening = result.max_tissue_softening.max(debug.max_tissue_softening);
    result.max_tissue_fatigue = result.max_tissue_fatigue.max(debug.max_tissue_fatigue);
    result.max_tissue_plasticity = result
        .max_tissue_plasticity
        .max(debug.max_tissue_plasticity);
    result.max_bone_angular_speed = result
        .max_bone_angular_speed
        .max(debug.max_bone_angular_speed);
    result.max_active_wounds = result.max_active_wounds.max(debug.active_wounds);
    result.max_active_contusions = result.max_active_contusions.max(debug.active_contusions);
    result.wound_leaks += debug.wound_leaks;
    result.muscle_fiber_tears += debug.muscle_fiber_tears;
    result.muscle_crush_ruptures += debug.muscle_crush_ruptures;
    result.cavity_pressure_events += debug.cavity_pressure_events;
    result.cavity_ruptures += debug.cavity_ruptures;
    result.organ_damage_events += debug.organ_damage_events;
    result.organ_penetrations += debug.organ_penetrations;
    result.rib_organ_punctures += debug.rib_organ_punctures;
    result.organ_ruptures += debug.organ_ruptures;
    result.skin_flap_detachments += debug.skin_flap_detachments;
    result.vessel_lacerations += debug.vessel_lacerations;
    result.fragment_vessel_lacerations += debug.fragment_vessel_lacerations;
    result.wound_reopens += debug.wound_reopens;
    result.max_active_blood_stains = result
        .max_active_blood_stains
        .max(debug.active_blood_stains);
    result.fragment_bone_contacts += debug.fragment_bone_contacts;
    result.fragment_bone_damping_events += debug.fragment_bone_damping_events;
    result.fragment_bone_resting_contacts += debug.fragment_bone_resting_contacts;
    result.fragment_pair_contacts += debug.fragment_pair_contacts;
    result.fragment_pair_damping_events += debug.fragment_pair_damping_events;
    result.fragment_pair_resting_contacts += debug.fragment_pair_resting_contacts;
    result.fragment_floor_contacts += debug.fragment_floor_contacts;
    result.fragment_floor_resting_contacts += debug.fragment_floor_resting_contacts;
    result.post_fracture_joint_corrections += debug.post_fracture_joint_corrections;
    result.max_active_fluids = result.max_active_fluids.max(active_fluid_count(world));
    result.max_active_fragments = result.max_active_fragments.max(debug.active_fragments);
    result.max_sleeping_fragments = result.max_sleeping_fragments.max(debug.sleeping_fragments);
    result.fragment_sleep_events += debug.fragment_sleep_events;
    result.fragment_wake_events += debug.fragment_wake_events;
    result.bone_joint_subluxations += debug.bone_joint_subluxations;
    result.joint_ligament_damage_events += debug.joint_ligament_damage_events;
    result.fragment_budget_skips += debug.fragment_budget_skips;
    result.fracture_budget_blocks += debug.fracture_budget_blocks;
    result.fragment_bone_checks += debug.fragment_bone_checks;
    result.fragment_bone_budget_skips += debug.fragment_bone_budget_skips;
    result.fragment_pair_checks += debug.fragment_pair_checks;
    result.fragment_pair_budget_skips += debug.fragment_pair_budget_skips;
    result.fragment_tissue_checks += debug.fragment_tissue_checks;
    result.fragment_tissue_budget_skips += debug.fragment_tissue_budget_skips;
    result.fragment_skin_punctures += debug.fragment_skin_punctures;
    result.fluid_budget_replacements += debug.fluid_budget_replacements;
    result.blood_stain_budget_replacements += debug.blood_stain_budget_replacements;
    result.wound_budget_replacements += debug.wound_budget_replacements;
    result.max_solver_iterations = result.max_solver_iterations.max(debug.solver_iterations);
}

fn write_frame_header(csv: &mut dyn Write) -> std::io::Result<()> {
    writeln!(csv, "scenario,region,intent,tool,frame,striker_x,striker_y,striker_speed,impact,tissue_contacts,bone_contacts,max_depth,max_point_load,max_bone_load,fractures,skin_tears,muscle_tears,muscle_fiber_tears_frame,total_muscle_fiber_tears,contusion_events_frame,active_contusions,total_contusion_events,max_contusion,max_tissue_softening,tissue_fatigue_events_frame,total_tissue_fatigue_events,max_tissue_fatigue,tissue_plastic_events_frame,total_tissue_plastic_events,max_tissue_plasticity,tear_propagations_frame,total_tear_propagations,muscle_cut_transfers_frame,total_muscle_cut_transfers,muscle_crush_ruptures_frame,total_muscle_crush_ruptures,cavity_pressure_events_frame,total_cavity_pressure_events,cavity_ruptures_frame,total_cavity_ruptures,organ_damage_events_frame,total_organ_damage_events,organ_penetrations_frame,total_organ_penetrations,rib_organ_punctures_frame,total_rib_organ_punctures,organ_ruptures_frame,total_organ_ruptures,skin_flap_detachments_frame,total_skin_flap_detachments,vessel_lacerations_frame,total_vessel_lacerations,fragment_vessel_lacerations_frame,total_fragment_vessel_lacerations,wound_reopens_frame,total_wound_reopens,detachments,bone_detachments,bone_joint_breaks,bone_joint_subluxations_frame,total_bone_joint_subluxations,joint_ligament_damage_frame,total_joint_ligament_damage,max_bone_joint_subluxation,bone_fractures,rib_fractures_frame,total_rib_fractures,fracture_marrow_sources,fluid_emitted_frame,active_fluids,total_fluid,blood_stain_deposits_frame,active_blood_stains,total_blood_stain_deposits,opened_wounds,active_wounds,wound_leaks,wound_fluid,blood_loss,blood_volume,blood_turgor,max_wound_pressure,max_wound_clot,max_cavity_pressure,max_cavity_collapse,max_organ_damage,fragment_contacts,fragment_tears,fragment_skin_punctures_frame,total_fragment_skin_punctures,fragment_bone_contacts,fragment_bone_damping_events,fragment_bone_resting_contacts,fragment_pair_contacts,fragment_pair_damping_events,fragment_pair_resting_contacts,fragment_floor_contacts,fragment_floor_resting_contacts,max_fragment_depth,max_fragment_impulse,max_fragment_overlap,post_fracture_joint_corrections,max_post_fracture_joint_stretch,max_post_fracture_joint_angle,fragment_hits,fragment_tissue_tears,max_bone_angular_speed,free_fragments,spinning_fragments,active_fragments,sleeping_fragments,fragment_sleep_events,fragment_wake_events,fragment_budget_skips,fracture_budget_blocks,fragment_bone_checks,fragment_bone_budget_skips,fragment_pair_checks,fragment_pair_budget_skips,fragment_tissue_checks,fragment_tissue_budget_skips,fluid_budget_replacements,blood_stain_budget_replacements,wound_budget_replacements,solver_iterations")
}

fn write_frame(
    csv: &mut dyn Write,
    scenario: &Scenario,
    frame: i32,
    world: &rp::World,
) -> std::io::Result<()> {
    let debug = world.debug();
    let stats = world.stats();
    let fields = [
        scenario.name.to_string(),
        scenario.region.to_string(),
        scenario.intent.to_string(),
        tool_name(debug.tool).to_string(),
        frame.to_string(),
        format!("{:.3}", debug.striker_position.x),
        format!("{:.3}", debug.striker_position.y),
        format!("{:.3}", debug.striker_speed),
        format!("{:.3}", debug.impact),
        debug.tissue_contacts.to_string(),
        debug.bone_contacts.to_string(),
        format!("{:.3}", debug.max_depth),
        format!("{:.3}", debug.max_point_load),
        format!("{:.3}", debug.max_bone_load),
        debug.fractures.to_string(),
        stats.broken_skin.to_string(),
        stats.broken_muscle.to_string(),
        debug.muscle_fiber_tears.to_string(),
        stats.muscle_fiber_tears.to_string(),
        debug.contusion_events.to_string(),
        debug.active_contusions.to_string(),
        stats.contusion_events.to_string(),
        format!("{:.3}", debug.max_contusion),
        format!("{:.3}", debug.max_tissue_softening),
        debug.tissue_fatigue_events.to_string(),
        stats.tissue_fatigue_events.to_string(),
        format!("{:.3}", debug.max_tissue_fatigue),
        debug.tissue_plastic_events.to_string(),
        stats.tissue_plastic_events.to_string(),
        format!("{:.3}", debug.max_tissue_plasticity),
        debug.tear_propagations.to_string(),
        stats.tear_propagations.to_string(),
        debug.muscle_cut_transfers.to_string(),
        stats.muscle_cut_transfers.to_string(),
        debug.muscle_crush_ruptures.to_string(),
        stats.muscle_crush_ruptures.to_string(),
        debug.cavity_pressure_events.to_string(),
        stats.cavity_pressure_events.to_string(),
        debug.cavity_ruptures.to_string(),
        stats.cavity_ruptures.to_string(),
        debug.organ_damage_events.to_string(),
        stats.organ_damage_events.to_string(),
        debug.organ_penetrations.to_string(),
        stats.organ_penetrations.to_string(),
        debug.rib_organ_punctures.to_string(),
        stats.rib_organ_punctures.to_string(),
        debug.organ_ruptures.to_string(),
        stats.organ_ruptures.to_string(),
        debug.skin_flap_detachments.to_string(),
        stats.skin_flap_detachments.to_string(),
        debug.vessel_lacerations.to_string(),
        stats.vessel_lacerations.to_string(),
        debug.fragment_vessel_lacerations.to_string(),
        stats.fragment_vessel_lacerations.to_string(),
        debug.wound_reopens.to_string(),
        stats.wound_reopens.to_string(),
        stats.broken_attachments.to_string(),
        stats.broken_bone_attachments.to_string(),
        stats.broken_bone_joints.to_string(),
        debug.bone_joint_subluxations.to_string(),
        stats.bone_joint_subluxations.to_string(),
        debug.joint_ligament_damage_events.to_string(),
        stats.joint_ligament_damage_events.to_string(),
        format!("{:.3}", debug.max_bone_joint_subluxation),
        stats.fractured_bones.to_string(),
        debug.rib_fractures.to_string(),
        stats.fractured_ribs.to_string(),
        stats.fracture_marrow_sources.to_string(),
        debug.fluid_emitted.to_string(),
        active_fluid_count(world).to_string(),
        stats.emitted_fluid_particles.to_string(),
        debug.blood_stain_deposits.to_string(),
        debug.active_blood_stains.to_string(),
        stats.blood_stain_deposits.to_string(),
        stats.opened_wounds.to_string(),
        debug.active_wounds.to_string(),
        debug.wound_leaks.to_string(),
        stats.wound_fluid_particles.to_string(),
        format!("{:.5}", stats.blood_loss),
        format!("{:.5}", world.blood_volume_fraction()),
        format!("{:.5}", world.blood_turgor_scale()),
        format!("{:.3}", debug.max_wound_pressure),
        format!("{:.3}", debug.max_wound_clot),
        format!("{:.3}", debug.max_cavity_pressure),
        format!("{:.3}", debug.max_cavity_collapse),
        format!("{:.3}", debug.max_organ_damage),
        debug.fragment_contacts.to_string(),
        debug.fragment_tears.to_string(),
        debug.fragment_skin_punctures.to_string(),
        stats.fragment_skin_punctures.to_string(),
        debug.fragment_bone_contacts.to_string(),
        debug.fragment_bone_damping_events.to_string(),
        debug.fragment_bone_resting_contacts.to_string(),
        debug.fragment_pair_contacts.to_string(),
        debug.fragment_pair_damping_events.to_string(),
        debug.fragment_pair_resting_contacts.to_string(),
        debug.fragment_floor_contacts.to_string(),
        debug.fragment_floor_resting_contacts.to_string(),
        format!("{:.3}", debug.max_fragment_depth),
        format!("{:.3}", debug.max_fragment_impulse),
        format!("{:.3}", debug.max_fragment_overlap),
        debug.post_fracture_joint_corrections.to_string(),
        format!("{:.3}", debug.max_post_fracture_joint_stretch),
        format!("{:.3}", debug.max_post_fracture_joint_angle),
        stats.fragment_tissue_hits.to_string(),
        stats.fragment_tissue_tears.to_string(),
        format!("{:.3}", debug.max_bone_angular_speed),
        free_fragment_count(world).to_string(),
        spinning_fragment_count(world).to_string(),
        debug.active_fragments.to_string(),
        debug.sleeping_fragments.to_string(),
        debug.fragment_sleep_events.to_string(),
        debug.fragment_wake_events.to_string(),
        debug.fragment_budget_skips.to_string(),
        debug.fracture_budget_blocks.to_string(),
        debug.fragment_bone_checks.to_string(),
        debug.fragment_bone_budget_skips.to_string(),
        debug.fragment_pair_checks.to_string(),
        debug.fragment_pair_budget_skips.to_string(),
        debug.fragment_tissue_checks.to_string(),
        debug.fragment_tissue_budget_skips.to_string(),
        debug.fluid_budget_replacements.to_string(),
        debug.blood_stain_budget_replacements.to_string(),
        debug.wound_budget_replacements.to_string(),
        debug.solver_iterations.to_string(),
    ];
    writeln!(csv, "{}", fields.join(","))
}

fn write_summary(path: &Path, rows: &[(Scenario, ScenarioResult)]) -> std::io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "scenario,region,intent,tool,tissue_contacts,bone_contacts,skin_tears,muscle_tears,muscle_fiber_tears,contusion_events,tissue_fatigue_events,tissue_plastic_events,tear_propagations,muscle_cut_transfers,muscle_crush_ruptures,cavity_pressure_events,cavity_ruptures,organ_damage_events,organ_penetrations,rib_organ_punctures,organ_ruptures,skin_flap_detachments,vessel_lacerations,fragment_vessel_lacerations,wound_reopens,max_active_contusions,detachments,bone_detachments,bone_joint_breaks,bone_joint_subluxations,joint_ligament_damage_events,bone_fractures,rib_fractures,fracture_marrow_sources,final_bones,fluid_emitted,wound_fluid,blood_loss,final_blood_volume,final_blood_turgor,blood_stain_deposits,max_active_blood_stains,opened_wounds,max_active_wounds,wound_leaks,fragment_hits,fragment_tears,fragment_skin_punctures,fragment_bone_contacts,fragment_bone_damping_events,fragment_bone_resting_contacts,fragment_pair_contacts,fragment_pair_damping_events,fragment_pair_resting_contacts,fragment_floor_contacts,fragment_floor_resting_contacts,post_fracture_joint_corrections,max_impact,max_bone_load,max_point_load,max_depth,max_fragment_depth,max_fragment_impulse,max_fragment_overlap,max_post_fracture_joint_stretch,max_post_fracture_joint_angle,max_bone_joint_subluxation,max_wound_pressure,max_wound_clot,max_cavity_pressure,max_cavity_collapse,max_organ_damage,max_contusion,max_tissue_softening,max_tissue_fatigue,max_tissue_plasticity,max_bone_angular_speed,final_free_fragments,final_spinning_fragments,final_sleeping_fragments,max_active_fragments,max_sleeping_fragments,fragment_sleep_events,fragment_wake_events,fragment_budget_skips,fracture_budget_blocks,fragment_bone_checks,fragment_bone_budget_skips,fragment_pair_checks,fragment_pair_budget_skips,fragment_tissue_checks,fragment_tissue_budget_skips,fluid_budget_replacements,blood_stain_budget_replacements,wound_budget_replacements,max_solver_iterations")?;
    for (scenario, result) in rows {
        let fields = [
            scenario.name.to_string(),
            scenario.region.to_string(),
            scenario.intent.to_string(),
            tool_name(scenario.tool).to_string(),
            result.tissue_contacts.to_string(),
            result.bone_contacts.to_string(),
            result.skin_tears.to_string(),
            result.muscle_tears.to_string(),
            result.muscle_fiber_tears.to_string(),
            result.contusion_events.to_string(),
            result.tissue_fatigue_events.to_string(),
            result.tissue_plastic_events.to_string(),
            result.tear_propagations.to_string(),
            result.muscle_cut_transfers.to_string(),
            result.muscle_crush_ruptures.to_string(),
            result.cavity_pressure_events.to_string(),
            result.cavity_ruptures.to_string(),
            result.organ_damage_events.to_string(),
            result.organ_penetrations.to_string(),
            result.rib_organ_punctures.to_string(),
            result.organ_ruptures.to_string(),
            result.skin_flap_detachments.to_string(),
            result.vessel_lacerations.to_string(),
            result.fragment_vessel_lacerations.to_string(),
            result.wound_reopens.to_string(),
            result.max_active_contusions.to_string(),
            result.detachments.to_string(),
            result.bone_detachments.to_string(),
            result.bone_joint_breaks.to_string(),
            result.bone_joint_subluxations.to_string(),
            result.joint_ligament_damage_events.to_string(),
            result.bone_fractures.to_string(),
            result.rib_fractures.to_string(),
            result.fracture_marrow_sources.to_string(),
            result.final_bones.to_string(),
            result.fluid_emitted.to_string(),
            result.wound_fluid.to_string(),
            format!("{:.5}", result.blood_loss),
            format!("{:.5}", result.final_blood_volume),
            format!("{:.5}", result.final_blood_turgor),
            result.blood_stain_deposits.to_string(),
            result.max_active_blood_stains.to_string(),
            result.opened_wounds.to_string(),
            result.max_active_wounds.to_string(),
            result.wound_leaks.to_string(),
            result.fragment_hits.to_string(),
            result.fragment_tears.to_string(),
            result.fragment_skin_punctures.to_string(),
            result.fragment_bone_contacts.to_string(),
            result.fragment_bone_damping_events.to_string(),
            result.fragment_bone_resting_contacts.to_string(),
            result.fragment_pair_contacts.to_string(),
            result.fragment_pair_damping_events.to_string(),
            result.fragment_pair_resting_contacts.to_string(),
            result.fragment_floor_contacts.to_string(),
            result.fragment_floor_resting_contacts.to_string(),
            result.post_fracture_joint_corrections.to_string(),
            format!("{:.3}", result.max_impact),
            format!("{:.3}", result.max_bone_load),
            format!("{:.3}", result.max_point_load),
            format!("{:.3}", result.max_depth),
            format!("{:.3}", result.max_fragment_depth),
            format!("{:.3}", result.max_fragment_impulse),
            format!("{:.3}", result.max_fragment_overlap),
            format!("{:.3}", result.max_post_fracture_joint_stretch),
            format!("{:.3}", result.max_post_fracture_joint_angle),
            format!("{:.3}", result.max_bone_joint_subluxation),
            format!("{:.3}", result.max_wound_pressure),
            format!("{:.3}", result.max_wound_clot),
            format!("{:.3}", result.max_cavity_pressure),
            format!("{:.3}", result.max_cavity_collapse),
            format!("{:.3}", result.max_organ_damage),
            format!("{:.3}", result.max_contusion),
            format!("{:.3}", result.max_tissue_softening),
            format!("{:.3}", result.max_tissue_fatigue),
            format!("{:.3}", result.max_tissue_plasticity),
            format!("{:.3}", result.max_bone_angular_speed),
            result.final_free_fragments.to_string(),
            result.final_spinning_fragments.to_string(),
            result.final_sleeping_fragments.to_string(),
            result.max_active_fragments.to_string(),
            result.max_sleeping_fragments.to_string(),
            result.fragment_sleep_events.to_string(),
            result.fragment_wake_events.to_string(),
            result.fragment_budget_skips.to_string(),
            result.fracture_budget_blocks.to_string(),
            result.fragment_bone_checks.to_string(),
            result.fragment_bone_budget_skips.to_string(),
            result.fragment_pair_checks.to_string(),
            result.fragment_pair_budget_skips.to_string(),
            result.fragment_tissue_checks.to_string(),
            result.fragment_tissue_budget_skips.to_string(),
            result.fluid_budget_replacements.to_string(),
            result.blood_stain_budget_replacements.to_string(),
            result.wound_budget_replacements.to_string(),
            result.max_solver_iterations.to_string(),
        ];
        writeln!(out, "{}", fields.join(","))?;
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
        "rib_fractures",
        result.rib_fractures,
        scenario.expectations.rib_fractures,
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
        "contusion_events",
        result.contusion_events,
        scenario.expectations.contusion_events,
        warnings,
    );
    check_int(
        scenario,
        "tissue_fatigue_events",
        result.tissue_fatigue_events,
        scenario.expectations.tissue_fatigue_events,
        warnings,
    );
    check_int(
        scenario,
        "tissue_plastic_events",
        result.tissue_plastic_events,
        scenario.expectations.tissue_plastic_events,
        warnings,
    );
    check_int(
        scenario,
        "tear_propagations",
        result.tear_propagations,
        scenario.expectations.tear_propagations,
        warnings,
    );
    check_int(
        scenario,
        "muscle_cut_transfers",
        result.muscle_cut_transfers,
        scenario.expectations.muscle_cut_transfers,
        warnings,
    );
    check_int(
        scenario,
        "muscle_fiber_tears",
        result.muscle_fiber_tears,
        scenario.expectations.muscle_fiber_tears,
        warnings,
    );
    check_int(
        scenario,
        "muscle_crush_ruptures",
        result.muscle_crush_ruptures,
        scenario.expectations.muscle_crush_ruptures,
        warnings,
    );
    check_int(
        scenario,
        "cavity_pressure_events",
        result.cavity_pressure_events,
        scenario.expectations.cavity_pressure_events,
        warnings,
    );
    check_int(
        scenario,
        "cavity_ruptures",
        result.cavity_ruptures,
        scenario.expectations.cavity_ruptures,
        warnings,
    );
    check_int(
        scenario,
        "organ_damage_events",
        result.organ_damage_events,
        scenario.expectations.organ_damage_events,
        warnings,
    );
    check_int(
        scenario,
        "organ_penetrations",
        result.organ_penetrations,
        scenario.expectations.organ_penetrations,
        warnings,
    );
    check_int(
        scenario,
        "rib_organ_punctures",
        result.rib_organ_punctures,
        scenario.expectations.rib_organ_punctures,
        warnings,
    );
    check_int(
        scenario,
        "organ_ruptures",
        result.organ_ruptures,
        scenario.expectations.organ_ruptures,
        warnings,
    );
    check_int(
        scenario,
        "skin_flap_detachments",
        result.skin_flap_detachments,
        scenario.expectations.skin_flap_detachments,
        warnings,
    );
    check_int(
        scenario,
        "vessel_lacerations",
        result.vessel_lacerations,
        scenario.expectations.vessel_lacerations,
        warnings,
    );
    check_int(
        scenario,
        "fragment_vessel_lacerations",
        result.fragment_vessel_lacerations,
        scenario.expectations.fragment_vessel_lacerations,
        warnings,
    );
    check_int(
        scenario,
        "wound_reopens",
        result.wound_reopens,
        scenario.expectations.wound_reopens,
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
        "blood_stain_deposits",
        result.blood_stain_deposits,
        scenario.expectations.blood_stain_deposits,
        warnings,
    );
    check_double(
        scenario,
        "blood_loss",
        result.blood_loss,
        scenario.expectations.blood_loss,
        warnings,
    );
    check_double(
        scenario,
        "final_blood_volume",
        result.final_blood_volume,
        scenario.expectations.final_blood_volume,
        warnings,
    );
    check_double(
        scenario,
        "final_blood_turgor",
        result.final_blood_turgor,
        scenario.expectations.final_blood_turgor,
        warnings,
    );
    check_double(
        scenario,
        "max_cavity_pressure",
        result.max_cavity_pressure,
        scenario.expectations.cavity_pressure,
        warnings,
    );
    check_double(
        scenario,
        "max_cavity_collapse",
        result.max_cavity_collapse,
        scenario.expectations.cavity_collapse,
        warnings,
    );
    check_double(
        scenario,
        "max_organ_damage",
        result.max_organ_damage,
        scenario.expectations.organ_damage,
        warnings,
    );
    check_int(
        scenario,
        "fracture_marrow_sources",
        result.fracture_marrow_sources,
        scenario.expectations.fracture_marrow_sources,
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
        "final_free_fragments",
        result.final_free_fragments,
        scenario.expectations.final_free_fragments,
        warnings,
    );
    check_int(
        scenario,
        "fragment_bone_contacts",
        result.fragment_bone_contacts,
        scenario.expectations.fragment_bone_contacts,
        warnings,
    );
    check_int(
        scenario,
        "fragment_bone_damping_events",
        result.fragment_bone_damping_events,
        scenario.expectations.fragment_bone_damping_events,
        warnings,
    );
    check_int(
        scenario,
        "fragment_bone_resting_contacts",
        result.fragment_bone_resting_contacts,
        scenario.expectations.fragment_bone_resting_contacts,
        warnings,
    );
    check_int(
        scenario,
        "sleeping_fragments",
        result.max_sleeping_fragments,
        scenario.expectations.sleeping_fragments,
        warnings,
    );
    check_int(
        scenario,
        "sleep_events",
        result.fragment_sleep_events,
        scenario.expectations.sleep_events,
        warnings,
    );
    check_int(
        scenario,
        "final_sleeping_fragments",
        result.final_sleeping_fragments,
        scenario.expectations.final_sleeping_fragments,
        warnings,
    );
    check_int(
        scenario,
        "fragment_pair_damping_events",
        result.fragment_pair_damping_events,
        scenario.expectations.fragment_pair_damping_events,
        warnings,
    );
    check_int(
        scenario,
        "fragment_pair_resting_contacts",
        result.fragment_pair_resting_contacts,
        scenario.expectations.fragment_pair_resting_contacts,
        warnings,
    );
    check_int(
        scenario,
        "fragment_floor_contacts",
        result.fragment_floor_contacts,
        scenario.expectations.fragment_floor_contacts,
        warnings,
    );
    check_int(
        scenario,
        "fragment_floor_resting_contacts",
        result.fragment_floor_resting_contacts,
        scenario.expectations.fragment_floor_resting_contacts,
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
        "fragment_skin_punctures",
        result.fragment_skin_punctures,
        scenario.expectations.fragment_skin_punctures,
        warnings,
    );
    check_int(
        scenario,
        "bone_joint_subluxations",
        result.bone_joint_subluxations,
        scenario.expectations.bone_joint_subluxations,
        warnings,
    );
    check_int(
        scenario,
        "joint_ligament_damage_events",
        result.joint_ligament_damage_events,
        scenario.expectations.joint_ligament_damage_events,
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
    check_double(
        scenario,
        "bone_joint_subluxation",
        result.max_bone_joint_subluxation,
        scenario.expectations.bone_joint_subluxation,
        warnings,
    );
    check_double(
        scenario,
        "tissue_softening",
        result.max_tissue_softening,
        scenario.expectations.tissue_softening,
        warnings,
    );
    check_double(
        scenario,
        "tissue_fatigue",
        result.max_tissue_fatigue,
        scenario.expectations.tissue_fatigue,
        warnings,
    );
    check_double(
        scenario,
        "tissue_plasticity",
        result.max_tissue_plasticity,
        scenario.expectations.tissue_plasticity,
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

fn sleeping_fragment_count(world: &rp::World) -> i32 {
    world
        .bones()
        .iter()
        .filter(|bone| free_fragment(bone) && bone.sleeping)
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
