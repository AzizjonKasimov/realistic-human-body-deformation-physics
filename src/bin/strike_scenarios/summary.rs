//! What a run did, in a line or a row: the headline injuries, how the tool
//! held, the skin's openings, a cut's quality, and the CSV rows.

use super::*;

/// How steady the tool was, in one line.
pub(crate) fn steadiness(result: &ScenarioResult) -> String {
    format!(
        "tool turn max {:.0} deg/step ({:.0} deg in all), snaps {}, contact toggles {}, trails hand by up to {:.0} px in the air and {:.0} px at the end, fastest tissue {:.0} px/s",
        result.max_tool_turn,
        result.tool_turning,
        result.tool_snaps,
        result.contact_toggles,
        result.max_free_lag,
        result.final_lag,
        result.max_point_speed
    )
}

/// How the skin and muscle were opened: fibers a blade cut, fibers torn by
/// stretching or load, and the stretches of cut or tear that opened.
pub(crate) fn skin_openings(world: &rp::World) -> String {
    let points = world.points();
    // A cut fiber's copies on the two sides of its cut count once.
    let flesh_of = |point: usize| {
        let ghost_of = points[point].ghost_of;
        if ghost_of == rp::MISSING_POINT {
            point
        } else {
            ghost_of
        }
    };
    let opened = world.opened_cuts();
    let count = |layer: rp::TissueLayer| {
        let mut cut = Vec::new();
        let mut torn = 0;
        for (spring, wear) in world
            .springs()
            .iter()
            .zip(world.spring_wear())
            .filter(|(spring, _)| spring.layer == layer)
        {
            if wear.cut {
                let (a, b) = (flesh_of(spring.a), flesh_of(spring.b));
                cut.push((a.min(b), a.max(b)));
            }
            torn += usize::from(spring.broken && !wear.cut);
        }
        cut.sort_unstable();
        cut.dedup();
        let lips = opened
            .iter()
            .filter(|opening| opening.layer == layer)
            .count();
        format!(
            "fibers cut {}, torn {torn}; stretches opened {lips}",
            cut.len()
        )
    };
    format!(
        "skin {}; muscle {}",
        count(rp::TissueLayer::Skin),
        count(rp::TissueLayer::Muscle)
    )
}

/// How well a knife cut follows its blade and how far it gapes (see
/// `ScenarioResult::blade_path_length` and the fields after it).
pub(crate) fn cut_quality(result: &ScenarioResult) -> String {
    if result.slit_length <= 0.0 {
        return format!(
            "cut: blade path {:.0} px, no cut opened in the skin",
            result.blade_path_length
        );
    }
    format!(
        "cut: blade path {:.0} px, slit {:.0} px ({:.2} of the path), {:.1} px off the path; skin gap {:.1} px at the middle, {:.1} px at most; muscle gap {:.1} px at most",
        result.blade_path_length,
        result.slit_length,
        result.slit_length / result.blade_path_length.max(1.0),
        result.slit_off_path,
        result.skin_gap_middle,
        result.skin_gap_max,
        result.muscle_gap_max
    )
}

pub(crate) fn headline(result: &ScenarioResult) -> String {
    let metres = rp::scenarios::scenario_metres_per_px();
    let broken: Vec<&str> = rp::BonePart::ALL
        .iter()
        .filter(|part| result.part_fractures[part.index()] > 0)
        .map(|part| part.name())
        .collect();
    let bleeding = result
        .bleeding_ml_per_min()
        .map_or("none".to_string(), |rate| format!("{rate:.1} ml/min"));
    format!(
        "tool up to {:.1} m/s; broken: {}; bleeding {}\n  bones {} (ribs {}), skin {}, muscle {}, cut open {}, bruises {}, vessels {}, organ damage {:.2}, reopens {}, blood loss {:.3}",
        result.max_tool_speed * metres,
        if broken.is_empty() {
            "nothing".to_string()
        } else {
            broken.join(", ")
        },
        bleeding,
        result.bone_fractures,
        result.rib_fractures,
        result.skin_tears,
        result.muscle_tears,
        result.cut_openings,
        result.contusion_events,
        result.vessel_lacerations,
        result.max_organ_damage,
        result.wound_reopens,
        result.blood_loss
    )
}

/// Columns of a summary row: the scenario, then its result.
pub(crate) const SUMMARY_HEADER: &str = "scenario,region,intent,tool,tissue_contacts,bone_contacts,skin_tears,muscle_tears,muscle_fiber_tears,contusion_events,tissue_fatigue_events,tissue_plastic_events,tear_propagations,muscle_crush_ruptures,cavity_pressure_events,cavity_ruptures,organ_damage_events,organ_penetrations,rib_organ_punctures,organ_ruptures,skin_flap_detachments,cut_openings,blade_path_length,slit_length,slit_off_path,skin_gap_middle,skin_gap_max,muscle_gap_max,vessel_lacerations,fragment_vessel_lacerations,wound_reopens,max_active_contusions,detachments,bone_detachments,bone_joint_breaks,bone_joint_subluxations,joint_ligament_damage_events,bone_fractures,rib_fractures,fracture_marrow_sources,butterfly_fragments,final_bones,fluid_emitted,wound_fluid,blood_loss,final_blood_volume,final_blood_turgor,blood_stain_deposits,max_active_blood_stains,skin_blood_drops,blood_drips,blood_trail_length,opened_wounds,max_active_wounds,wound_leaks,fragment_hits,fragment_tears,fragment_skin_punctures,fragment_bone_contacts,fragment_bone_damping_events,fragment_bone_resting_contacts,fragment_pair_contacts,fragment_pair_damping_events,fragment_pair_resting_contacts,fragment_floor_contacts,fragment_floor_resting_contacts,post_fracture_joint_corrections,max_impact,max_bone_load,max_point_load,max_depth,max_fragment_depth,max_fragment_impulse,max_fragment_overlap,max_post_fracture_joint_stretch,max_post_fracture_joint_angle,max_bone_joint_subluxation,max_wound_pressure,max_wound_clot,max_cavity_pressure,max_cavity_collapse,max_organ_damage,max_contusion,max_tissue_softening,max_tissue_fatigue,max_tissue_plasticity,max_bone_angular_speed,final_free_fragments,bone_pieces_out,closed_pieces_out,whole_bones_out,final_spinning_fragments,final_sleeping_fragments,max_active_fragments,max_sleeping_fragments,fragment_sleep_events,fragment_wake_events,fragment_budget_skips,fracture_budget_blocks,fragment_bone_checks,fragment_bone_budget_skips,fragment_pair_checks,fragment_pair_budget_skips,fragment_tissue_checks,fragment_tissue_budget_skips,fluid_budget_replacements,blood_stain_budget_replacements,wound_budget_replacements,max_solver_iterations,max_tool_turn,tool_snaps,tool_turning,contact_toggles,max_point_speed,max_free_lag,internal_bleeding,final_lag,max_head_turn,max_fist_shift,final_fist_offset,final_fist_drop";

pub(crate) fn summary_fields(scenario: &Scenario, result: &ScenarioResult) -> Vec<String> {
    vec![
        scenario.name.to_string(),
        scenario.region.to_string(),
        scenario.intent.to_string(),
        tool_name(scenario.play.tool()).to_string(),
        result.tissue_contacts.to_string(),
        result.bone_contacts.to_string(),
        result.skin_tears.to_string(),
        result.muscle_tears.to_string(),
        result.muscle_fiber_tears.to_string(),
        result.contusion_events.to_string(),
        result.tissue_fatigue_events.to_string(),
        result.tissue_plastic_events.to_string(),
        result.tear_propagations.to_string(),
        result.muscle_crush_ruptures.to_string(),
        result.cavity_pressure_events.to_string(),
        result.cavity_ruptures.to_string(),
        result.organ_damage_events.to_string(),
        result.organ_penetrations.to_string(),
        result.rib_organ_punctures.to_string(),
        result.organ_ruptures.to_string(),
        result.skin_flap_detachments.to_string(),
        result.cut_openings.to_string(),
        format!("{:.1}", result.blade_path_length),
        format!("{:.1}", result.slit_length),
        format!("{:.2}", result.slit_off_path),
        format!("{:.2}", result.skin_gap_middle),
        format!("{:.2}", result.skin_gap_max),
        format!("{:.2}", result.muscle_gap_max),
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
        result.butterfly_fragments.to_string(),
        result.final_bones.to_string(),
        result.fluid_emitted.to_string(),
        result.wound_fluid.to_string(),
        format!("{:.5}", result.blood_loss),
        format!("{:.5}", result.final_blood_volume),
        format!("{:.5}", result.final_blood_turgor),
        result.blood_stain_deposits.to_string(),
        result.max_active_blood_stains.to_string(),
        result.skin_blood_drops.to_string(),
        result.blood_drips.to_string(),
        format!("{:.1}", result.blood_trail_length),
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
        result.bone_pieces_out.to_string(),
        result.closed_pieces_out.to_string(),
        result.whole_bones_out.to_string(),
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
        format!("{:.3}", result.max_tool_turn),
        result.tool_snaps.to_string(),
        format!("{:.3}", result.tool_turning),
        result.contact_toggles.to_string(),
        format!("{:.3}", result.max_point_speed),
        format!("{:.3}", result.max_free_lag),
        result.internal_bleeding.to_string(),
        format!("{:.3}", result.final_lag),
        format!("{:.3}", result.max_head_turn),
        format!("{:.3}", result.max_fist_shift),
        format!("{:.3}", result.final_fist_offset),
        format!("{:.3}", result.final_fist_drop),
    ]
}

pub(crate) fn write_frame_header(csv: &mut dyn Write) -> std::io::Result<()> {
    writeln!(csv, "scenario,region,intent,tool,frame,striker_x,striker_y,striker_speed,impact,tissue_contacts,bone_contacts,max_depth,max_point_load,max_bone_load,fractures,skin_tears,muscle_tears,muscle_fiber_tears_frame,total_muscle_fiber_tears,contusion_events_frame,active_contusions,total_contusion_events,max_contusion,max_tissue_softening,tissue_fatigue_events_frame,total_tissue_fatigue_events,max_tissue_fatigue,tissue_plastic_events_frame,total_tissue_plastic_events,max_tissue_plasticity,tear_propagations_frame,total_tear_propagations,muscle_crush_ruptures_frame,total_muscle_crush_ruptures,cavity_pressure_events_frame,total_cavity_pressure_events,cavity_ruptures_frame,total_cavity_ruptures,organ_damage_events_frame,total_organ_damage_events,organ_penetrations_frame,total_organ_penetrations,rib_organ_punctures_frame,total_rib_organ_punctures,organ_ruptures_frame,total_organ_ruptures,skin_flap_detachments_frame,total_skin_flap_detachments,vessel_lacerations_frame,total_vessel_lacerations,fragment_vessel_lacerations_frame,total_fragment_vessel_lacerations,wound_reopens_frame,total_wound_reopens,detachments,bone_detachments,bone_joint_breaks,bone_joint_subluxations_frame,total_bone_joint_subluxations,joint_ligament_damage_frame,total_joint_ligament_damage,max_bone_joint_subluxation,bone_fractures,rib_fractures_frame,total_rib_fractures,fracture_marrow_sources,fluid_emitted_frame,active_fluids,total_fluid,blood_stain_deposits_frame,active_blood_stains,total_blood_stain_deposits,opened_wounds,active_wounds,wound_leaks,wound_fluid,blood_loss,blood_volume,blood_turgor,max_wound_pressure,max_wound_clot,max_cavity_pressure,max_cavity_collapse,max_organ_damage,fragment_contacts,fragment_tears,fragment_skin_punctures_frame,total_fragment_skin_punctures,fragment_bone_contacts,fragment_bone_damping_events,fragment_bone_resting_contacts,fragment_pair_contacts,fragment_pair_damping_events,fragment_pair_resting_contacts,fragment_floor_contacts,fragment_floor_resting_contacts,max_fragment_depth,max_fragment_impulse,max_fragment_overlap,post_fracture_joint_corrections,max_post_fracture_joint_stretch,max_post_fracture_joint_angle,fragment_hits,fragment_tissue_tears,max_bone_angular_speed,free_fragments,spinning_fragments,active_fragments,sleeping_fragments,fragment_sleep_events,fragment_wake_events,fragment_budget_skips,fracture_budget_blocks,fragment_bone_checks,fragment_bone_budget_skips,fragment_pair_checks,fragment_pair_budget_skips,fragment_tissue_checks,fragment_tissue_budget_skips,fluid_budget_replacements,blood_stain_budget_replacements,wound_budget_replacements,solver_iterations,tool_down,tool_touching,tool_axis_deg,tool_x,tool_y,max_point_speed,hand_x,hand_y,tool_lag,reach_flesh,reach_torn_away,reach_bones,reach_spine")
}

pub(crate) fn write_frame(
    csv: &mut dyn Write,
    scenario: &Scenario,
    frame: i32,
    input: &rp::InputState,
    world: &rp::World,
    last_positions: &mut Vec<rp::Vec2>,
) -> std::io::Result<()> {
    let point_speed = fastest_point_speed(world, last_positions);
    last_positions.clear();
    last_positions.extend(world.points().iter().map(|point| point.position));
    let debug = world.debug();
    let stats = world.stats();
    let reach = world.tool_reach();
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
        u8::from(input.down).to_string(),
        u8::from(tool_touching(world)).to_string(),
        tool_axis(world).map_or(String::new(), |axis| {
            format!("{:.2}", axis.y.atan2(axis.x).to_degrees())
        }),
        format!("{:.3}", world.tool_position().x),
        format!("{:.3}", world.tool_position().y),
        format!("{:.1}", point_speed),
        format!("{:.3}", input.x),
        format!("{:.3}", input.y),
        format!("{:.3}", tool_lag(world, input)),
        reach.flesh.to_string(),
        reach.torn_away.to_string(),
        reach.bones.to_string(),
        reach.spine.to_string(),
    ];
    writeln!(csv, "{}", fields.join(","))
}
