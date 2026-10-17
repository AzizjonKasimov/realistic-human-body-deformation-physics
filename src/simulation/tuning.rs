//! Changing the engine's settings by name, and trying out a change behind a
//! switch, without editing and rebuilding it. The tools take `--set
//! NAME=VALUE` for any of [`Materials`], and `trying` reads the experiments
//! named in the `RP_TRY` environment variable.

use std::sync::OnceLock;

use super::Materials;

/// Every field of [`Materials`] by name: the list `set` and `get` read, which
/// `every_setting_has_a_name` holds to the struct.
macro_rules! named_settings {
    ($($field:ident),* $(,)?) => {
        impl Materials {
            /// The name of every setting, in the order the struct lists them.
            pub const NAMES: &'static [&'static str] = &[$(stringify!($field)),*];

            /// Sets the setting called `name` to `value`, written as Rust writes
            /// its type (`2.5`, `4`, `true`).
            pub fn set(&mut self, name: &str, value: &str) -> Result<(), String> {
                match name {
                    $(stringify!($field) => {
                        self.$field = value.trim().parse().map_err(|_| {
                            format!("`{value}` is not a value for `{name}`")
                        })?;
                    })*
                    _ => return Err(unknown_setting(name)),
                }
                Ok(())
            }

            /// The value of the setting called `name`, as text.
            pub fn get(&self, name: &str) -> Option<String> {
                match name {
                    $(stringify!($field) => Some(format!("{:?}", self.$field)),)*
                    _ => None,
                }
            }
        }
    };
}

named_settings!(
    fixed_dt,
    substeps,
    contact_passes,
    warm_starting,
    gravity,
    linear_drag,
    point_spacing,
    floor_friction,
    striker_radius,
    striker_mass,
    tool_inertia_scale,
    tool_bone_mass_scale,
    hand_press_force,
    open_fracture_overload,
    direct_muscle_contact,
    skin_posture_hertz,
    muscle_posture_hertz,
    posture_damping_ratio,
    hip_hold_hertz,
    hip_hold_damping_ratio,
    skin_shear_modulus,
    skin_thickness,
    skin_stiffening_stretch,
    skin_stiffening_ratio,
    skin_compression_share,
    skin_prestretch_along,
    skin_prestretch_across,
    skin_damping_ratio,
    subcutaneous_fat_thickness,
    fat_shear_modulus,
    fat_damping_ratio,
    skin_tear_stretch,
    skin_tear_impulse,
    muscle_fiber_shear_modulus,
    muscle_cross_shear_modulus,
    muscle_fiber_stiffening_stretch,
    muscle_fiber_stiffening_ratio,
    muscle_cross_stiffening_stretch,
    muscle_cross_stiffening_ratio,
    muscle_damping_ratio,
    bone_tie_hertz,
    foot_tie_hertz,
    bone_tie_damping_ratio,
    elbow_untied_radius,
    elbow_slide_radius,
    elbow_rest_opening,
    elbow_slide_fat_share,
    muscle_fiber_rupture_damage_floor,
    muscle_tear_stretch,
    muscle_tear_impulse,
    muscle_exposed_tear_impulse,
    contusion_load_threshold,
    contusion_decay,
    contusion_tear_weakening,
    tissue_fatigue_stretch_threshold,
    tissue_fatigue_load_threshold,
    tissue_fatigue_rate,
    tissue_fatigue_decay,
    tissue_fatigue_tear_weakening,
    tissue_plastic_stretch_threshold,
    tissue_plastic_compression_threshold,
    tissue_plastic_rate,
    tissue_plastic_limit,
    tear_propagation_stress_threshold,
    tear_propagation_fatigue_threshold,
    tear_propagation_load_threshold,
    max_tear_propagations_per_step,
    muscle_crush_rupture_load_threshold,
    muscle_crush_rupture_damage_threshold,
    max_muscle_crush_ruptures_per_step,
    cavity_load_threshold,
    cavity_pressure_stiffness,
    cavity_pressure_load_scale,
    cavity_non_heavy_pressure_load_cap,
    cavity_pressure_decay,
    cavity_min_area_fraction,
    cavity_tissue_push,
    cavity_contusion_pressure,
    cavity_rupture_pressure,
    cavity_rupture_load_scale,
    cavity_non_heavy_rupture_load_scale,
    cavity_rupture_min_collapse,
    max_cavity_ruptures_per_step,
    organ_pressure_damage_threshold,
    organ_load_damage_threshold,
    organ_fragment_damage_threshold,
    organ_penetration_impulse,
    organ_penetration_cut_radius,
    organ_penetration_damage,
    organ_damage_rate,
    organ_rupture_damage,
    organ_bleed_pressure,
    rib_organ_puncture_impulse,
    rib_organ_puncture_radius,
    rib_organ_puncture_damage,
    max_rib_organ_punctures_per_step,
    max_organ_penetrations_per_step,
    max_organ_ruptures_per_step,
    max_total_organ_ruptures,
    skin_flap_load_threshold,
    skin_flap_stress_threshold,
    skin_flap_cut_radius,
    max_skin_flap_detachments_per_step,
    fat_tear_shear,
    fat_loaded_tear_shear,
    attachment_break_impulse,
    bone_fracture_impulse,
    max_bone_fracture_depth,
    min_bone_fragment_length,
    min_rib_fragment_length,
    secondary_bone_fracture_impulse_scale,
    bone_damping,
    bone_angular_damping,
    bone_torque_scale,
    fracture_spin_scale,
    bone_attachment_break_impulse,
    bone_tie_tear_mm,
    bone_tie_loaded_tear_mm,
    bone_joint_hertz,
    bone_joint_angle_hertz,
    bone_joint_damping_ratio,
    bone_joint_subluxation_stretch,
    bone_joint_subluxation_impulse,
    bone_joint_subluxation_angular,
    bone_joint_subluxation_slack,
    bone_joint_subluxation_stiffness_scale,
    joint_ligament_damage_radius,
    joint_ligament_damage_load_scale,
    joint_ligament_damage_contusion_scale,
    bone_joint_break_stretch,
    bone_joint_break_impulse,
    bone_joint_angular_break,
    post_fracture_joint_max_stretch,
    post_fracture_joint_slack,
    post_fracture_joint_angle_slack,
    fracture_link_slack,
    fracture_link_angle_slack,
    bone_impact_transfer,
    bone_direct_contact,
    max_active_bone_fragments,
    max_fragment_bone_checks,
    max_fragment_pair_checks,
    max_fragment_tissue_checks,
    fragment_sleep_speed,
    fragment_sleep_angular_speed,
    fragment_sleep_load,
    fragment_sleep_frames,
    fragment_wake_load,
    max_fluid_particles,
    max_fresh_blood_per_step,
    max_wound_leak_per_step,
    fluid_damping,
    fluid_gravity_scale,
    fluid_lifetime,
    fluid_wall_bounce,
    blood_volume_capacity,
    blood_pressure_min_scale,
    blood_turgor_min_scale,
    max_blood_stains,
    max_blood_trails,
    max_wound_sources,
    wound_leak_rate,
    wound_pressure_decay,
    wound_clot_rate,
    wound_spray_pressure,
    wound_merge_radius,
    wound_reopen_load_threshold,
    wound_reopen_radius,
    wound_reopen_pressure_scale,
    wound_reopen_clot_loss,
    wound_reopen_strain_load,
    max_wound_reopens_per_step,
    major_vessel_cut_radius,
    major_vessel_laceration_impulse,
    major_vessel_blunt_impulse_scale,
    major_vessel_pressure_scale,
    max_vessel_lacerations_per_step,
    fragment_vessel_laceration_impulse,
    fragment_vessel_laceration_radius,
    max_fragment_vessel_lacerations_per_step,
    sharp_tool_tear_pressure,
    fragment_contact_radius,
    fragment_damage_impulse,
    fragment_skin_puncture_impulse,
    max_fragment_skin_punctures_per_step,
    fragment_push,
    fragment_repulsion_stiffness,
    fragment_pair_normal_damping,
    fragment_pair_tangential_friction,
    fragment_pair_angular_friction,
    fragment_pair_rest_speed,
    fragment_pair_rest_stiffness,
    fragment_pair_rest_friction,
    fragment_bone_normal_damping,
    fragment_bone_tangential_friction,
    fragment_bone_angular_friction,
    fragment_bone_rest_speed,
    fragment_bone_rest_stiffness,
    fragment_bone_rest_friction,
    fragment_floor_normal_damping,
    fragment_floor_friction,
    fragment_floor_angular_friction,
    fragment_floor_rest_speed,
    fragment_repulsion_slop,
);

/// What to say about a setting no field is called, with the names that come
/// closest.
fn unknown_setting(name: &str) -> String {
    let close: Vec<&str> = Materials::NAMES
        .iter()
        .copied()
        .filter(|known| {
            known.contains(name)
                || name
                    .split('_')
                    .any(|word| word.len() > 3 && known.contains(word))
        })
        .take(8)
        .collect();
    if close.is_empty() {
        format!("no setting is called `{name}`")
    } else {
        format!("no setting is called `{name}`; close: {}", close.join(", "))
    }
}

/// Sets each `NAME=VALUE` in `settings` on `materials`.
pub fn apply_settings(
    materials: &mut Materials,
    settings: &[(String, String)],
) -> Result<(), String> {
    for (name, value) in settings {
        materials.set(name, value)?;
    }
    Ok(())
}

/// Splits `NAME=VALUE` items, each given whole or several to an item,
/// separated by commas.
pub fn parse_settings(items: &[String]) -> Result<Vec<(String, String)>, String> {
    let mut settings = Vec::new();
    for item in items {
        for part in item
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            let (name, value) = part
                .split_once('=')
                .ok_or_else(|| format!("`{part}` is not NAME=VALUE"))?;
            settings.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    let mut check = Materials::default();
    apply_settings(&mut check, &settings)?;
    Ok(settings)
}

/// Whether the experiment `name` is switched on: named in the `RP_TRY`
/// environment variable, several separated by commas. A change being tried
/// out reads it to run the old way and the new in one build (`RP_TRY=name
/// strike_scenarios ...`); `nothing_is_left_being_tried` fails while any
/// code still asks, so a branch reaches `main` with one way chosen.
pub fn trying(name: &str) -> bool {
    static TRIED: OnceLock<Vec<String>> = OnceLock::new();
    TRIED
        .get_or_init(|| {
            std::env::var("RP_TRY")
                .map(|list| {
                    list.split(',')
                        .map(|item| item.trim().to_string())
                        .filter(|item| !item.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        })
        .iter()
        .any(|tried| tried == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The list of names holds every field of the struct, in its order, so a
    /// new setting can be set by name from the day it is added.
    #[test]
    fn every_setting_has_a_name() {
        let shown = format!("{:?}", Materials::default());
        let fields = shown
            .trim_start_matches("Materials {")
            .trim_end_matches('}')
            .split(", ")
            .filter_map(|pair| pair.split_once(": ").map(|(name, _)| name.trim()))
            .collect::<Vec<_>>();
        assert_eq!(fields, Materials::NAMES);
    }

    #[test]
    fn a_setting_is_set_and_read_by_name() {
        let mut materials = Materials::default();
        materials.set("gravity", "2.5").unwrap();
        assert_eq!(materials.gravity, 2.5);
        materials.set("substeps", " 6 ").unwrap();
        assert_eq!(materials.substeps, 6);
        assert_eq!(materials.get("substeps").as_deref(), Some("6"));
        assert!(materials.set("gravity", "lots").is_err());
        let unknown = materials.set("joint_hertz", "1").unwrap_err();
        assert!(unknown.contains("bone_joint_hertz"), "{unknown}");
        let settings = parse_settings(&[
            "gravity=1.5,substeps=8".into(),
            "bone_joint_hertz=30".into(),
        ])
        .unwrap();
        assert_eq!(settings.len(), 3);
        assert!(parse_settings(&["gravity".into()]).is_err());
    }
}
