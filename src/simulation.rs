use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

const EPSILON: f64 = 0.0001;
const FRAGMENT_TISSUE_POINT_RADIUS_SCALE: f64 = 0.36;
const FRAGMENT_TISSUE_RESISTANCE: f64 = 0.72;
const FRAGMENT_TISSUE_NORMAL_DAMPING: f64 = 0.58;
const FRAGMENT_TISSUE_TANGENTIAL_FRICTION: f64 = 0.34;
const FRAGMENT_TISSUE_ANGULAR_FRICTION: f64 = 0.18;
const SKIN_ATTACHMENT_CANDIDATES: usize = 4;
pub const MISSING_ANCHOR: usize = usize::MAX;
pub const MISSING_SPRING: usize = usize::MAX;
const FRONT_PIXEL_SILHOUETTE_REFERENCE: &str =
    include_str!("../docs/reference/pixel_human_silhouettes/front_adult_silhouette_41x96.mask");
const FRONT_PIXEL_SILHOUETTE_WORLD_WIDTH: f64 = 0.60;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TissueLayer {
    Skin,
    Muscle,
}

impl Default for TissueLayer {
    fn default() -> Self {
        Self::Skin
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolMode {
    Blunt,
    Sharp,
    Heavy,
}

impl Default for ToolMode {
    fn default() -> Self {
        Self::Blunt
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrganKind {
    LeftLung,
    RightLung,
    Liver,
    Spleen,
}

impl Default for OrganKind {
    fn default() -> Self {
        Self::Liver
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneKind {
    Generic,
    Rib,
}

impl Default for BoneKind {
    fn default() -> Self {
        Self::Generic
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InputState {
    pub active: bool,
    pub down: bool,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub power: f64,
    pub tool: ToolMode,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            active: false,
            down: false,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            power: 2.0,
            tool: ToolMode::Blunt,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Materials {
    pub fixed_dt: f64,
    pub solver_iterations: usize,
    pub gravity: f64,
    pub damping: f64,
    pub point_spacing: f64,
    pub floor_friction: f64,
    pub striker_radius: f64,
    pub striker_mass: f64,
    pub direct_muscle_contact: f64,
    pub skin_shape_stiffness: f64,
    pub muscle_shape_stiffness: f64,
    pub skin_structural_stiffness: f64,
    pub skin_shear_stiffness: f64,
    pub skin_area_stiffness: f64,
    pub skin_spring_compliance: f64,
    pub skin_area_compliance: f64,
    pub skin_tear_stretch: f64,
    pub skin_tear_impulse: f64,
    pub muscle_fiber_stiffness: f64,
    pub muscle_cross_stiffness: f64,
    pub muscle_shear_stiffness: f64,
    pub muscle_area_stiffness: f64,
    pub muscle_fiber_rupture_damage_floor: f64,
    pub muscle_spring_compliance: f64,
    pub muscle_area_compliance: f64,
    pub muscle_tear_stretch: f64,
    pub muscle_tear_impulse: f64,
    pub muscle_exposed_tear_impulse: f64,
    pub contusion_load_threshold: f64,
    pub contusion_decay: f64,
    pub contusion_tear_weakening: f64,
    pub contusion_stiffness_softening: f64,
    pub tissue_fatigue_stretch_threshold: f64,
    pub tissue_fatigue_load_threshold: f64,
    pub tissue_fatigue_rate: f64,
    pub tissue_fatigue_decay: f64,
    pub tissue_fatigue_tear_weakening: f64,
    pub tissue_fatigue_stiffness_softening: f64,
    pub tissue_plastic_stretch_threshold: f64,
    pub tissue_plastic_compression_threshold: f64,
    pub tissue_plastic_rate: f64,
    pub tissue_plastic_limit: f64,
    pub tear_propagation_stress_threshold: f64,
    pub tear_propagation_fatigue_threshold: f64,
    pub tear_propagation_load_threshold: f64,
    pub max_tear_propagations_per_step: usize,
    pub muscle_cut_transfer_exposure_threshold: f64,
    pub muscle_cut_transfer_load_threshold: f64,
    pub muscle_cut_transfer_radius: f64,
    pub max_muscle_cut_transfers_per_step: usize,
    pub muscle_crush_rupture_load_threshold: f64,
    pub muscle_crush_rupture_damage_threshold: f64,
    pub max_muscle_crush_ruptures_per_step: usize,
    pub cavity_load_threshold: f64,
    pub cavity_pressure_stiffness: f64,
    pub cavity_pressure_load_scale: f64,
    pub cavity_non_heavy_pressure_load_cap: f64,
    pub cavity_pressure_decay: f64,
    pub cavity_min_area_fraction: f64,
    pub cavity_tissue_push: f64,
    pub cavity_contusion_pressure: f64,
    pub cavity_rupture_pressure: f64,
    pub cavity_rupture_load_scale: f64,
    pub cavity_non_heavy_rupture_load_scale: f64,
    pub cavity_rupture_min_collapse: f64,
    pub max_cavity_ruptures_per_step: usize,
    pub organ_pressure_damage_threshold: f64,
    pub organ_load_damage_threshold: f64,
    pub organ_fragment_damage_threshold: f64,
    pub organ_penetration_impulse: f64,
    pub organ_penetration_cut_radius: f64,
    pub organ_penetration_damage: f64,
    pub organ_damage_rate: f64,
    pub organ_rupture_damage: f64,
    pub organ_bleed_pressure: f64,
    pub rib_organ_puncture_impulse: f64,
    pub rib_organ_puncture_radius: f64,
    pub rib_organ_puncture_damage: f64,
    pub max_rib_organ_punctures_per_step: usize,
    pub max_organ_penetrations_per_step: usize,
    pub max_organ_ruptures_per_step: usize,
    pub max_total_organ_ruptures: usize,
    pub skin_flap_load_threshold: f64,
    pub skin_flap_stress_threshold: f64,
    pub skin_flap_cut_radius: f64,
    pub max_skin_flap_detachments_per_step: usize,
    pub attachment_stiffness: f64,
    pub attachment_break_stretch: f64,
    pub attachment_break_impulse: f64,
    pub bone_fracture_impulse: f64,
    pub max_bone_fracture_depth: i32,
    pub min_bone_fragment_length: f64,
    pub min_rib_fragment_length: f64,
    pub secondary_bone_fracture_impulse_scale: f64,
    pub bone_damping: f64,
    pub bone_angular_damping: f64,
    pub bone_torque_scale: f64,
    pub fracture_spin_scale: f64,
    pub bone_shape_stiffness: f64,
    pub bone_attachment_stiffness: f64,
    pub bone_attachment_break_impulse: f64,
    pub bone_attachment_break_stretch: f64,
    pub bone_joint_stiffness: f64,
    pub bone_joint_angular_stiffness: f64,
    pub bone_joint_subluxation_stretch: f64,
    pub bone_joint_subluxation_impulse: f64,
    pub bone_joint_subluxation_angular: f64,
    pub bone_joint_subluxation_slack: f64,
    pub bone_joint_subluxation_stiffness_scale: f64,
    pub joint_ligament_damage_radius: f64,
    pub joint_ligament_damage_load_scale: f64,
    pub joint_ligament_damage_contusion_scale: f64,
    pub bone_joint_break_stretch: f64,
    pub bone_joint_break_impulse: f64,
    pub bone_joint_angular_break: f64,
    pub post_fracture_joint_stiffness: f64,
    pub post_fracture_joint_angular_stiffness: f64,
    pub post_fracture_joint_max_stretch: f64,
    pub post_fracture_joint_slack: f64,
    pub post_fracture_joint_angle_slack: f64,
    pub bone_impact_transfer: f64,
    pub bone_direct_contact: f64,
    pub bone_direct_pressure: f64,
    pub max_active_bone_fragments: usize,
    pub max_fragment_bone_checks: usize,
    pub max_fragment_pair_checks: usize,
    pub max_fragment_tissue_checks: usize,
    pub fragment_sleep_speed: f64,
    pub fragment_sleep_angular_speed: f64,
    pub fragment_sleep_load: f64,
    pub fragment_sleep_frames: i32,
    pub fragment_wake_load: f64,
    pub max_fluid_particles: usize,
    pub fluid_damping: f64,
    pub fluid_gravity_scale: f64,
    pub fluid_lifetime: f64,
    pub fluid_floor_friction: f64,
    pub fluid_impact_scale: f64,
    pub blood_volume_capacity: f64,
    pub blood_loss_per_wound_particle: f64,
    pub blood_pressure_min_scale: f64,
    pub blood_turgor_min_scale: f64,
    pub max_blood_stains: usize,
    pub blood_stain_merge_radius: f64,
    pub blood_stain_decay: f64,
    pub blood_stain_spread: f64,
    pub max_wound_sources: usize,
    pub wound_leak_rate: f64,
    pub wound_pressure_decay: f64,
    pub wound_clot_rate: f64,
    pub wound_spray_pressure: f64,
    pub wound_merge_radius: f64,
    pub wound_reopen_load_threshold: f64,
    pub wound_reopen_radius: f64,
    pub wound_reopen_pressure_scale: f64,
    pub wound_reopen_clot_loss: f64,
    pub max_wound_reopens_per_step: usize,
    pub major_vessel_cut_radius: f64,
    pub major_vessel_laceration_impulse: f64,
    pub major_vessel_blunt_impulse_scale: f64,
    pub major_vessel_pressure_scale: f64,
    pub max_vessel_lacerations_per_step: usize,
    pub fragment_vessel_laceration_impulse: f64,
    pub fragment_vessel_laceration_radius: f64,
    pub max_fragment_vessel_lacerations_per_step: usize,
    pub sharp_tool_tear_pressure: f64,
    pub fragment_contact_radius: f64,
    pub fragment_damage_impulse: f64,
    pub fragment_skin_puncture_impulse: f64,
    pub max_fragment_skin_punctures_per_step: usize,
    pub fragment_push: f64,
    pub fragment_repulsion_stiffness: f64,
    pub fragment_pair_normal_damping: f64,
    pub fragment_pair_tangential_friction: f64,
    pub fragment_pair_angular_friction: f64,
    pub fragment_pair_rest_speed: f64,
    pub fragment_pair_rest_stiffness: f64,
    pub fragment_pair_rest_friction: f64,
    pub fragment_bone_normal_damping: f64,
    pub fragment_bone_tangential_friction: f64,
    pub fragment_bone_angular_friction: f64,
    pub fragment_bone_rest_speed: f64,
    pub fragment_bone_rest_stiffness: f64,
    pub fragment_bone_rest_friction: f64,
    pub fragment_floor_normal_damping: f64,
    pub fragment_floor_friction: f64,
    pub fragment_floor_angular_friction: f64,
    pub fragment_floor_rest_speed: f64,
    pub fragment_repulsion_slop: f64,
}

impl Default for Materials {
    fn default() -> Self {
        Self {
            fixed_dt: 1.0 / 60.0,
            solver_iterations: 12,
            gravity: 920.0,
            damping: 0.988,
            point_spacing: 11.5,
            floor_friction: 0.78,
            striker_radius: 34.0,
            striker_mass: 2.9,
            direct_muscle_contact: 0.18,
            skin_shape_stiffness: 0.012,
            muscle_shape_stiffness: 0.030,
            skin_structural_stiffness: 0.92,
            skin_shear_stiffness: 0.58,
            skin_area_stiffness: 0.070,
            skin_spring_compliance: 0.0,
            skin_area_compliance: 0.0,
            skin_tear_stretch: 1.68,
            skin_tear_impulse: 820.0,
            muscle_fiber_stiffness: 0.86,
            muscle_cross_stiffness: 0.44,
            muscle_shear_stiffness: 0.38,
            muscle_area_stiffness: 0.36,
            muscle_fiber_rupture_damage_floor: 0.0,
            muscle_spring_compliance: 0.0,
            muscle_area_compliance: 0.0,
            muscle_tear_stretch: 1.92,
            muscle_tear_impulse: 1180.0,
            muscle_exposed_tear_impulse: 620.0,
            contusion_load_threshold: 420.0,
            contusion_decay: 0.014,
            contusion_tear_weakening: 0.10,
            contusion_stiffness_softening: 0.0,
            tissue_fatigue_stretch_threshold: 1.18,
            tissue_fatigue_load_threshold: 520.0,
            tissue_fatigue_rate: 0.0008,
            tissue_fatigue_decay: 0.0012,
            tissue_fatigue_tear_weakening: 0.10,
            tissue_fatigue_stiffness_softening: 0.0,
            tissue_plastic_stretch_threshold: 1.08,
            tissue_plastic_compression_threshold: 0.72,
            tissue_plastic_rate: 0.00016,
            tissue_plastic_limit: 0.12,
            tear_propagation_stress_threshold: 0.22,
            tear_propagation_fatigue_threshold: 0.22,
            tear_propagation_load_threshold: 620.0,
            max_tear_propagations_per_step: 4,
            muscle_cut_transfer_exposure_threshold: 0.42,
            muscle_cut_transfer_load_threshold: 520.0,
            muscle_cut_transfer_radius: 18.0,
            max_muscle_cut_transfers_per_step: 3,
            muscle_crush_rupture_load_threshold: 820.0,
            muscle_crush_rupture_damage_threshold: 1.0,
            max_muscle_crush_ruptures_per_step: 5,
            cavity_load_threshold: 560.0,
            cavity_pressure_stiffness: 0.72,
            cavity_pressure_load_scale: 0.34,
            cavity_non_heavy_pressure_load_cap: 1.85,
            cavity_pressure_decay: 2.8,
            cavity_min_area_fraction: 0.70,
            cavity_tissue_push: 0.34,
            cavity_contusion_pressure: 0.46,
            cavity_rupture_pressure: 0.80,
            cavity_rupture_load_scale: 3.2,
            cavity_non_heavy_rupture_load_scale: 2.05,
            cavity_rupture_min_collapse: 0.05,
            max_cavity_ruptures_per_step: 1,
            organ_pressure_damage_threshold: 0.58,
            organ_load_damage_threshold: 1450.0,
            organ_fragment_damage_threshold: 760.0,
            organ_penetration_impulse: 980.0,
            organ_penetration_cut_radius: 4.5,
            organ_penetration_damage: 0.82,
            organ_damage_rate: 0.18,
            organ_rupture_damage: 1.0,
            organ_bleed_pressure: 1.12,
            rib_organ_puncture_impulse: 1500.0,
            rib_organ_puncture_radius: 8.5,
            rib_organ_puncture_damage: 0.52,
            max_rib_organ_punctures_per_step: 2,
            max_organ_penetrations_per_step: 2,
            max_organ_ruptures_per_step: 2,
            max_total_organ_ruptures: 2,
            skin_flap_load_threshold: 760.0,
            skin_flap_stress_threshold: 0.18,
            skin_flap_cut_radius: 20.0,
            max_skin_flap_detachments_per_step: 5,
            attachment_stiffness: 0.46,
            attachment_break_stretch: 2.40,
            attachment_break_impulse: 980.0,
            bone_fracture_impulse: 1850.0,
            max_bone_fracture_depth: 4,
            min_bone_fragment_length: 26.0,
            min_rib_fragment_length: 12.0,
            secondary_bone_fracture_impulse_scale: 0.76,
            bone_damping: 0.984,
            bone_angular_damping: 0.955,
            bone_torque_scale: 0.35,
            fracture_spin_scale: 1.10,
            bone_shape_stiffness: 0.008,
            bone_attachment_stiffness: 0.52,
            bone_attachment_break_impulse: 2100.0,
            bone_attachment_break_stretch: 2.8,
            bone_joint_stiffness: 0.66,
            bone_joint_angular_stiffness: 0.22,
            bone_joint_subluxation_stretch: 1.46,
            bone_joint_subluxation_impulse: 1650.0,
            bone_joint_subluxation_angular: 0.62,
            bone_joint_subluxation_slack: 18.0,
            bone_joint_subluxation_stiffness_scale: 0.56,
            joint_ligament_damage_radius: 24.0,
            joint_ligament_damage_load_scale: 0.34,
            joint_ligament_damage_contusion_scale: 0.65,
            bone_joint_break_stretch: 2.15,
            bone_joint_break_impulse: 2600.0,
            bone_joint_angular_break: 1.20,
            post_fracture_joint_stiffness: 0.13,
            post_fracture_joint_angular_stiffness: 0.052,
            post_fracture_joint_max_stretch: 2.6,
            post_fracture_joint_slack: 34.0,
            post_fracture_joint_angle_slack: 0.78,
            bone_impact_transfer: 0.62,
            bone_direct_contact: 0.86,
            bone_direct_pressure: 780.0,
            max_active_bone_fragments: 48,
            max_fragment_bone_checks: 16_384,
            max_fragment_pair_checks: 8192,
            max_fragment_tissue_checks: 500_000,
            fragment_sleep_speed: 18.0,
            fragment_sleep_angular_speed: 0.08,
            fragment_sleep_load: 90.0,
            fragment_sleep_frames: 36,
            fragment_wake_load: 260.0,
            max_fluid_particles: 900,
            fluid_damping: 0.982,
            fluid_gravity_scale: 0.42,
            fluid_lifetime: 4.8,
            fluid_floor_friction: 0.48,
            fluid_impact_scale: 0.08,
            blood_volume_capacity: 1.0,
            blood_loss_per_wound_particle: 0.00016,
            blood_pressure_min_scale: 0.34,
            blood_turgor_min_scale: 0.55,
            max_blood_stains: 320,
            blood_stain_merge_radius: 18.0,
            blood_stain_decay: 0.008,
            blood_stain_spread: 2.4,
            max_wound_sources: 160,
            wound_leak_rate: 4.6,
            wound_pressure_decay: 0.20,
            wound_clot_rate: 0.13,
            wound_spray_pressure: 1.25,
            wound_merge_radius: 16.0,
            wound_reopen_load_threshold: 540.0,
            wound_reopen_radius: 24.0,
            wound_reopen_pressure_scale: 0.52,
            wound_reopen_clot_loss: 0.22,
            max_wound_reopens_per_step: 6,
            major_vessel_cut_radius: 10.5,
            major_vessel_laceration_impulse: 1450.0,
            major_vessel_blunt_impulse_scale: 2.45,
            major_vessel_pressure_scale: 1.38,
            max_vessel_lacerations_per_step: 3,
            fragment_vessel_laceration_impulse: 2200.0,
            fragment_vessel_laceration_radius: 7.5,
            max_fragment_vessel_lacerations_per_step: 2,
            sharp_tool_tear_pressure: 0.66,
            fragment_contact_radius: 15.0,
            fragment_damage_impulse: 520.0,
            fragment_skin_puncture_impulse: 1080.0,
            max_fragment_skin_punctures_per_step: 8,
            fragment_push: 0.34,
            fragment_repulsion_stiffness: 0.56,
            fragment_pair_normal_damping: 0.32,
            fragment_pair_tangential_friction: 0.16,
            fragment_pair_angular_friction: 0.10,
            fragment_pair_rest_speed: 95.0,
            fragment_pair_rest_stiffness: 0.24,
            fragment_pair_rest_friction: 0.22,
            fragment_bone_normal_damping: 0.28,
            fragment_bone_tangential_friction: 0.14,
            fragment_bone_angular_friction: 0.08,
            fragment_bone_rest_speed: 112.0,
            fragment_bone_rest_stiffness: 0.20,
            fragment_bone_rest_friction: 0.18,
            fragment_floor_normal_damping: 0.78,
            fragment_floor_friction: 0.42,
            fragment_floor_angular_friction: 0.18,
            fragment_floor_rest_speed: 70.0,
            fragment_repulsion_slop: 0.85,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub position: Vec2,
    pub previous: Vec2,
    pub home: Vec2,
    pub layer: TissueLayer,
    pub pinned: bool,
    pub load: f64,
    pub exposure: f64,
    pub contusion: f64,
    pub mass: f64,
}

impl Default for Point {
    fn default() -> Self {
        Self {
            position: Vec2::default(),
            previous: Vec2::default(),
            home: Vec2::default(),
            layer: TissueLayer::Skin,
            pinned: false,
            load: 0.0,
            exposure: 0.0,
            contusion: 0.0,
            mass: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub a: usize,
    pub b: usize,
    pub rest: f64,
    pub rest_reference: f64,
    pub stiffness: f64,
    pub tear_stretch: f64,
    pub tear_impulse: f64,
    pub layer: TissueLayer,
    pub fiber: bool,
    pub broken: bool,
    pub stress: f64,
    pub fatigue: f64,
    pub plastic_strain: f64,
    pub lambda: f64,
}

impl Default for Spring {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            rest: 0.0,
            rest_reference: 0.0,
            stiffness: 0.0,
            tear_stretch: 0.0,
            tear_impulse: 0.0,
            layer: TissueLayer::Skin,
            fiber: false,
            broken: false,
            stress: 0.0,
            fatigue: 0.0,
            plastic_strain: 0.0,
            lambda: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AreaConstraint {
    pub a: usize,
    pub b: usize,
    pub c: usize,
    pub edge_ab: usize,
    pub edge_bc: usize,
    pub edge_ca: usize,
    pub rest_area: f64,
    pub stiffness: f64,
    pub layer: TissueLayer,
    pub lambda: f64,
}

impl Default for AreaConstraint {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            c: 0,
            edge_ab: MISSING_SPRING,
            edge_bc: MISSING_SPRING,
            edge_ca: MISSING_SPRING,
            rest_area: 0.0,
            stiffness: 0.0,
            layer: TissueLayer::Skin,
            lambda: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Attachment {
    pub skin_point: usize,
    pub muscle_point: usize,
    pub rest: f64,
    pub broken: bool,
    pub stress: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    pub a: usize,
    pub b: usize,
    pub c: usize,
    pub edge_ab: usize,
    pub edge_bc: usize,
    pub edge_ca: usize,
    pub layer: TissueLayer,
    pub failed: bool,
    pub damage: f64,
}

impl Default for Triangle {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            c: 0,
            edge_ab: MISSING_SPRING,
            edge_bc: MISSING_SPRING,
            edge_ca: MISSING_SPRING,
            layer: TissueLayer::Skin,
            failed: false,
            damage: 0.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CavityRegion {
    pub area_indices: Vec<usize>,
    pub rest_area: f64,
    pub pressure: f64,
    pub collapse: f64,
    pub centroid: Vec2,
    pub ruptured: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct OrganRegion {
    pub kind: OrganKind,
    pub center: Vec2,
    pub radius: Vec2,
    pub anchor_point: usize,
    pub anchor_offset: Vec2,
    pub damage: f64,
    pub pressure_damage: f64,
    pub load_damage: f64,
    pub penetration_damage: f64,
    pub penetrated: bool,
    pub rib_punctured: bool,
    pub ruptured: bool,
}

impl Default for OrganRegion {
    fn default() -> Self {
        Self {
            kind: OrganKind::default(),
            center: Vec2::default(),
            radius: Vec2 { x: 1.0, y: 1.0 },
            anchor_point: MISSING_ANCHOR,
            anchor_offset: Vec2::default(),
            damage: 0.0,
            pressure_damage: 0.0,
            load_damage: 0.0,
            penetration_damage: 0.0,
            penetrated: false,
            rib_punctured: false,
            ruptured: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BoneSegment {
    pub kind: BoneKind,
    pub a: Vec2,
    pub b: Vec2,
    pub previous_a: Vec2,
    pub previous_b: Vec2,
    pub home_a: Vec2,
    pub home_b: Vec2,
    pub radius: f64,
    pub rest_length: f64,
    pub fracture_impulse: f64,
    pub load: f64,
    pub angular_velocity: f64,
    pub fractured: bool,
    pub broken_start: bool,
    pub broken_end: bool,
    pub broken_start_normal: Vec2,
    pub broken_end_normal: Vec2,
    pub fracture_generation: i32,
    pub splinter: bool,
    pub pinned: bool,
    pub sleeping: bool,
    pub sleep_frames: i32,
}

impl Default for BoneSegment {
    fn default() -> Self {
        Self {
            kind: BoneKind::default(),
            a: Vec2::default(),
            b: Vec2::default(),
            previous_a: Vec2::default(),
            previous_b: Vec2::default(),
            home_a: Vec2::default(),
            home_b: Vec2::default(),
            radius: 5.0,
            rest_length: 1.0,
            fracture_impulse: 2600.0,
            load: 0.0,
            angular_velocity: 0.0,
            fractured: false,
            broken_start: false,
            broken_end: false,
            broken_start_normal: Vec2::default(),
            broken_end_normal: Vec2::default(),
            fracture_generation: 0,
            splinter: false,
            pinned: false,
            sleeping: false,
            sleep_frames: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BoneAttachment {
    pub point: usize,
    pub bone: usize,
    pub t: f64,
    pub offset: Vec2,
    pub rest: f64,
    pub stress: f64,
    pub broken: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct BoneJoint {
    pub a: usize,
    pub b: usize,
    pub t_a: f64,
    pub t_b: f64,
    pub rest: f64,
    pub rest_angle: f64,
    pub min_angle: f64,
    pub max_angle: f64,
    pub stress: f64,
    pub torque_stress: f64,
    pub subluxated: bool,
    pub subluxation: f64,
    pub broken: bool,
    pub post_fracture_limited: bool,
    pub post_fracture_rest: f64,
    pub post_fracture_rest_angle: f64,
}

impl Default for BoneJoint {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            t_a: 0.0,
            t_b: 0.0,
            rest: 0.0,
            rest_angle: 0.0,
            min_angle: -0.95,
            max_angle: 0.95,
            stress: 0.0,
            torque_stress: 0.0,
            subluxated: false,
            subluxation: 0.0,
            broken: false,
            post_fracture_limited: false,
            post_fracture_rest: 0.0,
            post_fracture_rest_angle: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FluidParticle {
    pub position: Vec2,
    pub previous: Vec2,
    pub radius: f64,
    pub life: f64,
    pub max_life: f64,
    pub intensity: f64,
    pub settled: bool,
    pub stained: bool,
}

impl Default for FluidParticle {
    fn default() -> Self {
        Self {
            position: Vec2::default(),
            previous: Vec2::default(),
            radius: 2.0,
            life: 0.0,
            max_life: 1.0,
            intensity: 1.0,
            settled: false,
            stained: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BloodStain {
    pub position: Vec2,
    pub radius: f64,
    pub intensity: f64,
    pub age: f64,
}

impl Default for BloodStain {
    fn default() -> Self {
        Self {
            position: Vec2::default(),
            radius: 2.0,
            intensity: 0.0,
            age: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VesselSegment {
    pub a: Vec2,
    pub b: Vec2,
    pub radius: f64,
    pub pressure: f64,
    pub laceration_impulse: f64,
    pub lacerated: bool,
    pub anchor_a: usize,
    pub anchor_b: usize,
    pub offset_a: Vec2,
    pub offset_b: Vec2,
}

impl Default for VesselSegment {
    fn default() -> Self {
        Self {
            a: Vec2::default(),
            b: Vec2::default(),
            radius: 2.0,
            pressure: 1.0,
            laceration_impulse: 1450.0,
            lacerated: false,
            anchor_a: MISSING_ANCHOR,
            anchor_b: MISSING_ANCHOR,
            offset_a: Vec2::default(),
            offset_b: Vec2::default(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WoundSource {
    pub position: Vec2,
    pub direction: Vec2,
    pub layer: TissueLayer,
    pub pressure: f64,
    pub clot: f64,
    pub age: f64,
    pub radius: f64,
    pub depth: f64,
    pub accumulator: f64,
    pub active: bool,
    pub anchor_point: usize,
    pub anchor_bone: usize,
    pub anchor_t: f64,
    pub anchor_offset: Vec2,
}

impl Default for WoundSource {
    fn default() -> Self {
        Self {
            position: Vec2::default(),
            direction: Vec2::default(),
            layer: TissueLayer::Skin,
            pressure: 0.0,
            clot: 0.0,
            age: 0.0,
            radius: 2.0,
            depth: 0.0,
            accumulator: 0.0,
            active: false,
            anchor_point: MISSING_ANCHOR,
            anchor_bone: MISSING_ANCHOR,
            anchor_t: 0.0,
            anchor_offset: Vec2::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub broken_skin: i32,
    pub broken_muscle: i32,
    pub muscle_fiber_tears: i32,
    pub broken_attachments: i32,
    pub broken_bone_attachments: i32,
    pub broken_bone_joints: i32,
    pub bone_joint_subluxations: i32,
    pub joint_ligament_damage_events: i32,
    pub fractured_bones: i32,
    pub fractured_ribs: i32,
    pub emitted_fluid_particles: i32,
    pub wound_fluid_particles: i32,
    pub blood_loss: f64,
    pub fracture_marrow_sources: i32,
    pub blood_stain_deposits: i32,
    pub contusion_events: i32,
    pub tissue_fatigue_events: i32,
    pub tissue_plastic_events: i32,
    pub tear_propagations: i32,
    pub muscle_cut_transfers: i32,
    pub muscle_crush_ruptures: i32,
    pub cavity_pressure_events: i32,
    pub cavity_ruptures: i32,
    pub organ_damage_events: i32,
    pub organ_penetrations: i32,
    pub rib_organ_punctures: i32,
    pub organ_ruptures: i32,
    pub skin_flap_detachments: i32,
    pub vessel_lacerations: i32,
    pub fragment_vessel_lacerations: i32,
    pub wound_reopens: i32,
    pub opened_wounds: i32,
    pub fragment_tissue_hits: i32,
    pub fragment_tissue_tears: i32,
    pub fragment_skin_punctures: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct ContactDebug {
    pub active: bool,
    pub down: bool,
    pub striker_position: Vec2,
    pub striker_velocity: Vec2,
    pub strongest_contact: Vec2,
    pub striker_speed: f64,
    pub striker_mass: f64,
    pub striker_radius: f64,
    pub tool: ToolMode,
    pub impact: f64,
    pub max_depth: f64,
    pub max_bone_load: f64,
    pub max_point_load: f64,
    pub max_bone_angular_speed: f64,
    pub max_fragment_depth: f64,
    pub max_fragment_impulse: f64,
    pub max_fragment_overlap: f64,
    pub max_post_fracture_joint_stretch: f64,
    pub max_post_fracture_joint_angle: f64,
    pub max_bone_joint_subluxation: f64,
    pub max_wound_pressure: f64,
    pub max_wound_clot: f64,
    pub max_cavity_pressure: f64,
    pub max_cavity_collapse: f64,
    pub max_organ_damage: f64,
    pub blood_loss: f64,
    pub blood_volume_fraction: f64,
    pub blood_turgor_scale: f64,
    pub max_contusion: f64,
    pub max_tissue_softening: f64,
    pub max_tissue_fatigue: f64,
    pub max_tissue_plasticity: f64,
    pub last_fracture_impulse: f64,
    pub bone_contacts: i32,
    pub tissue_contacts: i32,
    pub fractures: i32,
    pub rib_fractures: i32,
    pub fluid_emitted: i32,
    pub fragment_contacts: i32,
    pub fragment_tears: i32,
    pub fragment_skin_punctures: i32,
    pub fragment_bone_contacts: i32,
    pub fragment_bone_damping_events: i32,
    pub fragment_bone_resting_contacts: i32,
    pub fragment_pair_contacts: i32,
    pub fragment_pair_damping_events: i32,
    pub fragment_pair_resting_contacts: i32,
    pub fragment_floor_contacts: i32,
    pub fragment_floor_resting_contacts: i32,
    pub post_fracture_joint_corrections: i32,
    pub bone_joint_subluxations: i32,
    pub joint_ligament_damage_events: i32,
    pub active_wounds: i32,
    pub active_fluids: i32,
    pub active_blood_stains: i32,
    pub active_contusions: i32,
    pub active_fragments: i32,
    pub sleeping_fragments: i32,
    pub solver_iterations: i32,
    pub fragment_sleep_events: i32,
    pub fragment_wake_events: i32,
    pub fragment_budget_skips: i32,
    pub fracture_budget_blocks: i32,
    pub fragment_bone_checks: i32,
    pub fragment_bone_budget_skips: i32,
    pub fragment_pair_checks: i32,
    pub fragment_pair_budget_skips: i32,
    pub fragment_tissue_checks: i32,
    pub fragment_tissue_budget_skips: i32,
    pub fluid_budget_replacements: i32,
    pub blood_stain_deposits: i32,
    pub blood_stain_budget_replacements: i32,
    pub contusion_events: i32,
    pub tissue_fatigue_events: i32,
    pub tissue_plastic_events: i32,
    pub muscle_fiber_tears: i32,
    pub tear_propagations: i32,
    pub muscle_cut_transfers: i32,
    pub muscle_crush_ruptures: i32,
    pub cavity_pressure_events: i32,
    pub cavity_ruptures: i32,
    pub organ_damage_events: i32,
    pub organ_penetrations: i32,
    pub rib_organ_punctures: i32,
    pub organ_ruptures: i32,
    pub skin_flap_detachments: i32,
    pub vessel_lacerations: i32,
    pub fragment_vessel_lacerations: i32,
    pub wound_reopens: i32,
    pub wound_budget_replacements: i32,
    pub wound_leaks: i32,
    pub fracture_marrow_sources: i32,
}

impl Default for ContactDebug {
    fn default() -> Self {
        Self {
            active: false,
            down: false,
            striker_position: Vec2::default(),
            striker_velocity: Vec2::default(),
            strongest_contact: Vec2::default(),
            striker_speed: 0.0,
            striker_mass: 0.0,
            striker_radius: 0.0,
            tool: ToolMode::Blunt,
            impact: 0.0,
            max_depth: 0.0,
            max_bone_load: 0.0,
            max_point_load: 0.0,
            max_bone_angular_speed: 0.0,
            max_fragment_depth: 0.0,
            max_fragment_impulse: 0.0,
            max_fragment_overlap: 0.0,
            max_post_fracture_joint_stretch: 0.0,
            max_post_fracture_joint_angle: 0.0,
            max_bone_joint_subluxation: 0.0,
            max_wound_pressure: 0.0,
            max_wound_clot: 0.0,
            max_cavity_pressure: 0.0,
            max_cavity_collapse: 0.0,
            max_organ_damage: 0.0,
            blood_loss: 0.0,
            blood_volume_fraction: 1.0,
            blood_turgor_scale: 1.0,
            max_contusion: 0.0,
            max_tissue_softening: 0.0,
            max_tissue_fatigue: 0.0,
            max_tissue_plasticity: 0.0,
            last_fracture_impulse: 0.0,
            bone_contacts: 0,
            tissue_contacts: 0,
            fractures: 0,
            rib_fractures: 0,
            fluid_emitted: 0,
            fragment_contacts: 0,
            fragment_tears: 0,
            fragment_skin_punctures: 0,
            fragment_bone_contacts: 0,
            fragment_bone_damping_events: 0,
            fragment_bone_resting_contacts: 0,
            fragment_pair_contacts: 0,
            fragment_pair_damping_events: 0,
            fragment_pair_resting_contacts: 0,
            fragment_floor_contacts: 0,
            fragment_floor_resting_contacts: 0,
            post_fracture_joint_corrections: 0,
            bone_joint_subluxations: 0,
            joint_ligament_damage_events: 0,
            active_wounds: 0,
            active_fluids: 0,
            active_blood_stains: 0,
            active_contusions: 0,
            active_fragments: 0,
            sleeping_fragments: 0,
            solver_iterations: 0,
            fragment_sleep_events: 0,
            fragment_wake_events: 0,
            fragment_budget_skips: 0,
            fracture_budget_blocks: 0,
            fragment_bone_checks: 0,
            fragment_bone_budget_skips: 0,
            fragment_pair_checks: 0,
            fragment_pair_budget_skips: 0,
            fragment_tissue_checks: 0,
            fragment_tissue_budget_skips: 0,
            fluid_budget_replacements: 0,
            blood_stain_deposits: 0,
            blood_stain_budget_replacements: 0,
            contusion_events: 0,
            tissue_fatigue_events: 0,
            tissue_plastic_events: 0,
            muscle_fiber_tears: 0,
            tear_propagations: 0,
            muscle_cut_transfers: 0,
            muscle_crush_ruptures: 0,
            cavity_pressure_events: 0,
            cavity_ruptures: 0,
            organ_damage_events: 0,
            organ_penetrations: 0,
            rib_organ_punctures: 0,
            organ_ruptures: 0,
            skin_flap_detachments: 0,
            vessel_lacerations: 0,
            fragment_vessel_lacerations: 0,
            wound_reopens: 0,
            wound_budget_replacements: 0,
            wound_leaks: 0,
            fracture_marrow_sources: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AnatomyValidation {
    pub skin_points: i32,
    pub muscle_points: i32,
    pub bone_samples: i32,
    pub bone_samples_outside_skin: i32,
    pub bone_samples_outside_muscle: i32,
    pub bone_segments_outside_skin: i32,
    pub bone_segments_outside_muscle: i32,
}

#[derive(Clone, Copy, Debug)]
struct ToolProfile {
    radius_scale: f64,
    reach_padding: f64,
    mass_scale: f64,
    tissue_push_scale: f64,
    tissue_load_scale: f64,
    contusion_scale: f64,
    bone_push_scale: f64,
    bone_load_scale: f64,
    fracture_scale: f64,
    tear_pressure_scale: f64,
    fluid_scale: f64,
    blade_normal_bias: f64,
    drag_scale: f64,
    rebound_scale: f64,
    blade_front_scale: f64,
    blade_back_scale: f64,
    blade_contact_radius_scale: f64,
}

impl Default for ToolProfile {
    fn default() -> Self {
        Self {
            radius_scale: 1.0,
            reach_padding: 12.0,
            mass_scale: 1.0,
            tissue_push_scale: 1.0,
            tissue_load_scale: 1.0,
            contusion_scale: 1.0,
            bone_push_scale: 1.0,
            bone_load_scale: 1.0,
            fracture_scale: 1.0,
            tear_pressure_scale: 0.0,
            fluid_scale: 1.0,
            blade_normal_bias: 0.0,
            drag_scale: 1.0,
            rebound_scale: 1.0,
            blade_front_scale: 0.0,
            blade_back_scale: 0.0,
            blade_contact_radius_scale: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ToolContactShape {
    center: Vec2,
    axis_start: Vec2,
    axis_end: Vec2,
    direction: Vec2,
    blade_normal: Vec2,
    influence: f64,
    blade_segment: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct ToolPointContact {
    normal: Vec2,
    distance: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct OrganToolContact {
    tool_point: Vec2,
    contact: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GridKey {
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Debug, Default)]
struct SegmentClosestPoints {
    t_a: f64,
    t_b: f64,
    point_a: Vec2,
    point_b: Vec2,
    distance: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct Aabb {
    min: Vec2,
    max: Vec2,
}

#[derive(Clone, Copy, Debug)]
struct WoundAnchorCandidate {
    point: usize,
    bone: usize,
    t: f64,
    offset: Vec2,
    distance_sq: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct CavityMetrics {
    current_area: f64,
    average_load: f64,
    peak_load: f64,
    centroid: Vec2,
    samples: usize,
}

#[derive(Clone, Debug)]
pub struct World {
    materials: Materials,
    points: Vec<Point>,
    springs: Vec<Spring>,
    areas: Vec<AreaConstraint>,
    attachments: Vec<Attachment>,
    triangles: Vec<Triangle>,
    bones: Vec<BoneSegment>,
    bone_attachments: Vec<BoneAttachment>,
    bone_joints: Vec<BoneJoint>,
    vessels: Vec<VesselSegment>,
    cavities: Vec<CavityRegion>,
    organs: Vec<OrganRegion>,
    fluids: Vec<FluidParticle>,
    blood_stains: Vec<BloodStain>,
    wounds: Vec<WoundSource>,
    stats: Stats,
    debug: ContactDebug,
    blood_volume: f64,
    fluid_write_cursor: usize,
    blood_stain_write_cursor: usize,
    fluid_seed: u32,
}

impl Default for World {
    fn default() -> Self {
        Self::new(Materials::default())
    }
}

impl World {
    pub fn new(materials: Materials) -> Self {
        Self {
            materials,
            points: Vec::new(),
            springs: Vec::new(),
            areas: Vec::new(),
            attachments: Vec::new(),
            triangles: Vec::new(),
            bones: Vec::new(),
            bone_attachments: Vec::new(),
            bone_joints: Vec::new(),
            vessels: Vec::new(),
            cavities: Vec::new(),
            organs: Vec::new(),
            fluids: Vec::new(),
            blood_stains: Vec::new(),
            wounds: Vec::new(),
            stats: Stats::default(),
            debug: ContactDebug::default(),
            blood_volume: materials.blood_volume_capacity.max(0.0),
            fluid_write_cursor: 0,
            blood_stain_write_cursor: 0,
            fluid_seed: 0x9e3779b9,
        }
    }

    pub fn materials(&self) -> &Materials {
        &self.materials
    }

    pub fn points(&self) -> &[Point] {
        &self.points
    }

    pub fn springs(&self) -> &[Spring] {
        &self.springs
    }

    pub fn areas(&self) -> &[AreaConstraint] {
        &self.areas
    }

    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    pub fn triangles(&self) -> &[Triangle] {
        &self.triangles
    }

    pub fn bones(&self) -> &[BoneSegment] {
        &self.bones
    }

    pub fn bone_attachments(&self) -> &[BoneAttachment] {
        &self.bone_attachments
    }

    pub fn bone_joints(&self) -> &[BoneJoint] {
        &self.bone_joints
    }

    pub fn vessels(&self) -> &[VesselSegment] {
        &self.vessels
    }

    pub fn cavities(&self) -> &[CavityRegion] {
        &self.cavities
    }

    pub fn organs(&self) -> &[OrganRegion] {
        &self.organs
    }

    pub fn fluids(&self) -> &[FluidParticle] {
        &self.fluids
    }

    pub fn blood_stains(&self) -> &[BloodStain] {
        &self.blood_stains
    }

    pub fn wounds(&self) -> &[WoundSource] {
        &self.wounds
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    pub fn debug(&self) -> &ContactDebug {
        &self.debug
    }

    pub fn blood_volume_fraction(&self) -> f64 {
        self.blood_volume_fraction_internal()
    }

    pub fn blood_turgor_scale(&self) -> f64 {
        self.blood_turgor_scale_internal()
    }

    pub fn add_point(&mut self, position: Vec2, layer: TissueLayer, pinned: bool) -> usize {
        let index = self.points.len();
        self.points.push(Point {
            position,
            previous: position,
            home: position,
            layer,
            pinned,
            mass: if layer == TissueLayer::Muscle {
                1.25
            } else {
                1.0
            },
            ..Point::default()
        });
        index
    }

    pub fn add_spring(
        &mut self,
        a: usize,
        b: usize,
        layer: TissueLayer,
        stiffness: f64,
        tear_stretch: f64,
        tear_impulse: f64,
        fiber: bool,
    ) {
        if a == b || a >= self.points.len() || b >= self.points.len() {
            return;
        }
        if self.springs.iter().any(|spring| {
            spring.layer == layer
                && ((spring.a == a && spring.b == b) || (spring.a == b && spring.b == a))
        }) {
            return;
        }
        let rest = distance(self.points[a].position, self.points[b].position);
        self.springs.push(Spring {
            a,
            b,
            rest,
            rest_reference: rest,
            stiffness,
            tear_stretch,
            tear_impulse,
            layer,
            fiber,
            ..Spring::default()
        });
    }

    pub fn add_area(&mut self, a: usize, b: usize, c: usize, layer: TissueLayer, stiffness: f64) {
        if a >= self.points.len() || b >= self.points.len() || c >= self.points.len() {
            return;
        }
        self.areas.push(AreaConstraint {
            a,
            b,
            c,
            edge_ab: self.find_spring_index(a, b, layer),
            edge_bc: self.find_spring_index(b, c, layer),
            edge_ca: self.find_spring_index(c, a, layer),
            rest_area: signed_area(
                self.points[a].position,
                self.points[b].position,
                self.points[c].position,
            ),
            stiffness,
            layer,
            lambda: 0.0,
        });
    }

    pub fn add_cavity_from_areas(&mut self, area_indices: Vec<usize>) -> usize {
        let mut filtered = Vec::new();
        let mut rest_area = 0.0;
        let mut weighted_centroid = Vec2::default();
        for index in area_indices {
            if index >= self.areas.len() {
                continue;
            }
            let area = self.areas[index];
            if area.layer != TissueLayer::Muscle {
                continue;
            }
            let weight = area.rest_area.abs();
            if weight <= EPSILON {
                continue;
            }
            let centroid = Vec2 {
                x: (self.points[area.a].home.x
                    + self.points[area.b].home.x
                    + self.points[area.c].home.x)
                    / 3.0,
                y: (self.points[area.a].home.y
                    + self.points[area.b].home.y
                    + self.points[area.c].home.y)
                    / 3.0,
            };
            weighted_centroid.x += centroid.x * weight;
            weighted_centroid.y += centroid.y * weight;
            rest_area += weight;
            filtered.push(index);
        }
        if filtered.is_empty() || rest_area <= EPSILON {
            return MISSING_ANCHOR;
        }
        let centroid = scale(weighted_centroid, 1.0 / rest_area);
        let index = self.cavities.len();
        self.cavities.push(CavityRegion {
            area_indices: filtered,
            rest_area,
            pressure: 0.0,
            collapse: 0.0,
            centroid,
            ruptured: false,
        });
        index
    }

    pub fn add_organ_region(&mut self, kind: OrganKind, center: Vec2, radius: Vec2) -> usize {
        let (anchor_point, anchor_offset) = self.nearest_vessel_point_anchor(center);
        let index = self.organs.len();
        self.organs.push(OrganRegion {
            kind,
            center,
            radius: Vec2 {
                x: radius.x.max(2.0),
                y: radius.y.max(2.0),
            },
            anchor_point,
            anchor_offset,
            ..OrganRegion::default()
        });
        index
    }

    pub fn add_attachment(&mut self, skin_point: usize, muscle_point: usize) {
        if skin_point >= self.points.len() || muscle_point >= self.points.len() {
            return;
        }
        if self.attachments.iter().any(|attachment| {
            attachment.skin_point == skin_point && attachment.muscle_point == muscle_point
        }) {
            return;
        }
        self.attachments.push(Attachment {
            skin_point,
            muscle_point,
            rest: distance(
                self.points[skin_point].position,
                self.points[muscle_point].position,
            )
            .max(self.materials.point_spacing * 0.34),
            broken: false,
            stress: 0.0,
        });
    }

    pub fn add_triangle(&mut self, a: usize, b: usize, c: usize, layer: TissueLayer) {
        if a >= self.points.len() || b >= self.points.len() || c >= self.points.len() {
            return;
        }
        self.triangles.push(Triangle {
            a,
            b,
            c,
            edge_ab: self.find_spring_index(a, b, layer),
            edge_bc: self.find_spring_index(b, c, layer),
            edge_ca: self.find_spring_index(c, a, layer),
            layer,
            ..Triangle::default()
        });
    }

    pub fn add_bone_segment(
        &mut self,
        a: Vec2,
        b: Vec2,
        radius: f64,
        fracture_impulse: f64,
        pinned: bool,
    ) -> usize {
        self.add_bone_segment_with_kind(a, b, radius, fracture_impulse, pinned, BoneKind::Generic)
    }

    pub fn add_bone_segment_with_kind(
        &mut self,
        a: Vec2,
        b: Vec2,
        radius: f64,
        fracture_impulse: f64,
        pinned: bool,
        kind: BoneKind,
    ) -> usize {
        let index = self.bones.len();
        self.bones.push(BoneSegment {
            kind,
            a,
            b,
            previous_a: a,
            previous_b: b,
            home_a: a,
            home_b: b,
            radius,
            rest_length: distance(a, b).max(EPSILON),
            fracture_impulse,
            pinned,
            ..BoneSegment::default()
        });
        index
    }

    pub fn add_bone_attachment(&mut self, point: usize, bone: usize, t: f64) {
        if point >= self.points.len() || bone >= self.bones.len() {
            return;
        }
        let t = t.clamp(0.0, 1.0);
        let anchor = bone_point(self.bones[bone], t);
        let point_position = self.points[point].position;
        self.bone_attachments.push(BoneAttachment {
            point,
            bone,
            t,
            offset: subtract(point_position, anchor),
            rest: distance(point_position, anchor).max(self.materials.point_spacing * 0.42),
            stress: 0.0,
            broken: false,
        });
    }

    pub fn add_bone_joint(
        &mut self,
        a: usize,
        t_a: f64,
        b: usize,
        t_b: f64,
        min_angle: f64,
        max_angle: f64,
    ) {
        if a >= self.bones.len() || b >= self.bones.len() || a == b {
            return;
        }
        let t_a = t_a.clamp(0.0, 1.0);
        let t_b = t_b.clamp(0.0, 1.0);
        self.bone_joints.push(BoneJoint {
            a,
            b,
            t_a,
            t_b,
            rest: distance(
                bone_point(self.bones[a], t_a),
                bone_point(self.bones[b], t_b),
            )
            .max(self.materials.point_spacing * 0.70),
            rest_angle: wrap_angle(bone_angle(self.bones[b]) - bone_angle(self.bones[a])),
            min_angle: min_angle.min(max_angle),
            max_angle: min_angle.max(max_angle),
            ..BoneJoint::default()
        });
    }

    pub fn add_vessel_segment(&mut self, a: Vec2, b: Vec2, radius: f64, pressure: f64) -> usize {
        let (anchor_a, offset_a) = self.nearest_vessel_point_anchor(a);
        let (anchor_b, offset_b) = self.nearest_vessel_point_anchor(b);
        let index = self.vessels.len();
        self.vessels.push(VesselSegment {
            a,
            b,
            radius: radius.max(0.8),
            pressure: pressure.max(0.1),
            laceration_impulse: self.materials.major_vessel_laceration_impulse,
            anchor_a,
            anchor_b,
            offset_a,
            offset_b,
            ..VesselSegment::default()
        });
        index
    }

    pub fn step(&mut self, dt: f64, input: &InputState, width: f64, height: f64) {
        let floor_y = height - 38.0;
        let profile = tool_profile(input.tool);
        let striker_radius = self.materials.striker_radius * profile.radius_scale;
        let striker_mass = self.materials.striker_mass * input.power * profile.mass_scale;
        let striker_speed = hypot(input.vx, input.vy);

        self.debug = ContactDebug {
            active: input.active,
            down: input.down,
            striker_position: Vec2 {
                x: input.x,
                y: input.y,
            },
            striker_velocity: Vec2 {
                x: input.vx,
                y: input.vy,
            },
            striker_speed,
            striker_mass,
            striker_radius,
            tool: input.tool,
            impact: striker_speed * striker_mass,
            solver_iterations: self.materials.solver_iterations as i32,
            blood_loss: self.stats.blood_loss,
            blood_volume_fraction: self.blood_volume_fraction_internal(),
            blood_turgor_scale: self.blood_turgor_scale_internal(),
            ..ContactDebug::default()
        };

        self.update_exposure();
        self.integrate(dt, width, floor_y);
        self.update_vessel_anchors();
        self.update_organ_anchors();
        self.update_wound_anchors();
        self.update_wounds(dt);
        self.collide_striker(dt, input);
        self.update_organ_anchors();
        self.update_cavities(dt);
        self.update_organs(dt);
        self.disturb_wounds_from_loaded_tissue();
        self.reset_constraint_lambdas();

        for _ in 0..self.materials.solver_iterations {
            self.solve_springs();
            self.solve_attachments();
            self.solve_bone_attachments();
            self.solve_bone_joints();
            self.solve_bones();
            self.solve_post_fracture_joints();
            self.solve_bone_fragment_bone_contacts();
            self.solve_bone_fragment_tissue_contacts();
            self.solve_bone_fragment_repulsion();
            self.solve_areas();
            self.constrain_to_world(width, floor_y);
        }

        self.update_vessel_anchors();
        self.update_organ_anchors();
        self.propagate_skin_tears();
        self.transfer_sharp_cut_to_exposed_muscle();
        self.delaminate_skin_flaps_from_cut_edges();
        self.collide_bone_fragments();
        self.update_triangle_damage();
        self.update_fragment_sleep_states();
        self.debug.active_fluids = self.active_fluid_count() as i32;
        self.debug.active_blood_stains = self.active_blood_stain_count() as i32;
        let (active_contusions, max_contusion) = self.contusion_metrics();
        self.debug.active_contusions = active_contusions as i32;
        self.debug.max_contusion = self.debug.max_contusion.max(max_contusion);
        self.debug.active_fragments = self
            .debug
            .active_fragments
            .max(self.free_fragment_count() as i32);
        self.debug.sleeping_fragments = self.sleeping_fragment_count() as i32;
    }

    fn reset_constraint_lambdas(&mut self) {
        for spring in &mut self.springs {
            spring.lambda = 0.0;
        }
        for area in &mut self.areas {
            area.lambda = 0.0;
        }
    }

    pub fn triangle_alive(&self, triangle: &Triangle) -> bool {
        !triangle.failed
            && self.live_edge_count(triangle.edge_ab, triangle.edge_bc, triangle.edge_ca) >= 2
    }

    pub fn has_live_spring(&self, a: usize, b: usize, layer: TissueLayer) -> bool {
        self.spring_alive(self.find_spring_index(a, b, layer))
    }

    fn find_spring_index(&self, a: usize, b: usize, layer: TissueLayer) -> usize {
        self.springs
            .iter()
            .position(|spring| {
                spring.layer == layer
                    && ((spring.a == a && spring.b == b) || (spring.a == b && spring.b == a))
            })
            .unwrap_or(MISSING_SPRING)
    }

    fn spring_alive(&self, index: usize) -> bool {
        index != MISSING_SPRING && index < self.springs.len() && !self.springs[index].broken
    }

    fn live_edge_count(&self, a: usize, b: usize, c: usize) -> usize {
        usize::from(self.spring_alive(a))
            + usize::from(self.spring_alive(b))
            + usize::from(self.spring_alive(c))
    }

    fn can_fracture_bone(&self, bone: BoneSegment) -> bool {
        self.can_fracture_bone_shape(bone) && self.fracture_budget_allows(bone)
    }

    fn can_fracture_bone_shape(&self, bone: BoneSegment) -> bool {
        let min_fragment_length = if bone.kind == BoneKind::Rib {
            self.materials.min_rib_fragment_length
        } else {
            self.materials.min_bone_fragment_length
        };
        !bone.pinned
            && !bone.splinter
            && bone.fracture_generation < self.materials.max_bone_fracture_depth
            && bone.rest_length >= min_fragment_length * 2.0
    }

    fn fracture_budget_allows(&self, bone: BoneSegment) -> bool {
        let budget = self.materials.max_active_bone_fragments;
        if budget == 0 {
            return false;
        }
        let added_fragments = if free_bone_fragment(bone) { 2 } else { 3 };
        self.free_fragment_count() + added_fragments <= budget
    }

    fn free_fragment_count(&self) -> usize {
        self.bones
            .iter()
            .filter(|bone| awake_free_bone_fragment(**bone))
            .count()
    }

    fn sleeping_fragment_count(&self) -> usize {
        self.bones
            .iter()
            .filter(|bone| free_bone_fragment(**bone) && bone.sleeping)
            .count()
    }

    fn active_fluid_count(&self) -> usize {
        self.fluids.iter().filter(|fluid| fluid.life > 0.0).count()
    }

    fn active_blood_stain_count(&self) -> usize {
        self.blood_stains
            .iter()
            .filter(|stain| stain.intensity > 0.025)
            .count()
    }

    fn active_wound_count(&self) -> usize {
        self.wounds.iter().filter(|wound| wound.active).count()
    }

    fn contusion_metrics(&self) -> (usize, f64) {
        let mut active = 0;
        let mut max_contusion: f64 = 0.0;
        for point in &self.points {
            max_contusion = max_contusion.max(point.contusion);
            if point.contusion > 0.035 {
                active += 1;
            }
        }
        (active, max_contusion)
    }

    fn fragment_work_priority(&self, index: usize) -> f64 {
        let bone = self.bones[index];
        let endpoint_speed = fragment_endpoint_speed(bone, self.materials.fixed_dt);
        bone.load
            + endpoint_speed * bone.radius.max(1.0) * 0.18
            + bone.angular_velocity.abs() * bone.radius.max(1.0) * 18.0
            + bone.rest_length * if bone.splinter { 0.10 } else { 0.32 }
            + if bone.broken_start || bone.broken_end {
                180.0
            } else {
                0.0
            }
    }

    fn budgeted_fragment_indices(&mut self) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .bones
            .iter()
            .enumerate()
            .filter_map(|(index, bone)| awake_free_bone_fragment(*bone).then_some(index))
            .collect();
        self.debug.active_fragments = self.debug.active_fragments.max(indices.len() as i32);

        let budget = self.materials.max_active_bone_fragments;
        if indices.len() <= budget {
            return indices;
        }
        if budget == 0 {
            self.debug.fragment_budget_skips += indices.len() as i32;
            return Vec::new();
        }

        indices.sort_by(|a, b| {
            self.fragment_work_priority(*b)
                .partial_cmp(&self.fragment_work_priority(*a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let skipped = indices.len() - budget;
        self.debug.fragment_budget_skips += skipped as i32;
        indices.truncate(budget);
        indices
    }

    fn consume_fragment_pair_check(&mut self) -> bool {
        if (self.debug.fragment_pair_checks as usize) >= self.materials.max_fragment_pair_checks {
            self.debug.fragment_pair_budget_skips += 1;
            false
        } else {
            self.debug.fragment_pair_checks += 1;
            true
        }
    }

    fn consume_fragment_bone_check(&mut self) -> bool {
        if (self.debug.fragment_bone_checks as usize) >= self.materials.max_fragment_bone_checks {
            self.debug.fragment_bone_budget_skips += 1;
            false
        } else {
            self.debug.fragment_bone_checks += 1;
            true
        }
    }

    fn consume_fragment_tissue_check(&mut self) -> bool {
        if (self.debug.fragment_tissue_checks as usize) >= self.materials.max_fragment_tissue_checks
        {
            self.debug.fragment_tissue_budget_skips += 1;
            false
        } else {
            self.debug.fragment_tissue_checks += 1;
            true
        }
    }

    fn fragment_tissue_cell_size(&self) -> f64 {
        (self
            .materials
            .fragment_contact_radius
            .max(self.materials.point_spacing * 1.6))
        .max(8.0)
    }

    fn fragment_pair_cell_size(&self) -> f64 {
        (self.materials.fragment_contact_radius * 2.6)
            .max(self.materials.point_spacing * 2.0)
            .max(16.0)
    }

    fn fragment_bone_cell_size(&self) -> f64 {
        (self.materials.fragment_contact_radius * 2.8)
            .max(self.materials.point_spacing * 2.0)
            .max(18.0)
    }

    fn build_point_spatial_grid(&self, cell_size: f64) -> HashMap<GridKey, Vec<usize>> {
        let mut grid = HashMap::new();
        for (index, point) in self.points.iter().enumerate() {
            grid.entry(spatial_key(point.position, cell_size))
                .or_insert_with(Vec::new)
                .push(index);
        }
        grid
    }

    fn build_fragment_spatial_grid(
        &self,
        fragment_indices: &[usize],
        cell_size: f64,
    ) -> HashMap<GridKey, Vec<usize>> {
        let mut grid = HashMap::new();
        for &index in fragment_indices {
            let bone = self.bones[index];
            let margin = bone.radius + self.materials.fragment_repulsion_slop + 1.0;
            add_index_to_spatial_cells(
                &mut grid,
                segment_aabb(bone.a, bone.b, margin),
                cell_size,
                index,
            );
        }
        grid
    }

    fn build_intact_bone_spatial_grid(&self, cell_size: f64) -> HashMap<GridKey, Vec<usize>> {
        let mut grid = HashMap::new();
        for (index, bone) in self.bones.iter().enumerate() {
            if free_bone_fragment(*bone) {
                continue;
            }
            let margin = bone.radius + self.materials.fragment_repulsion_slop + 1.0;
            add_index_to_spatial_cells(
                &mut grid,
                segment_aabb(bone.a, bone.b, margin),
                cell_size,
                index,
            );
        }
        grid
    }

    fn point_candidates_near_aabb(
        &self,
        grid: &HashMap<GridKey, Vec<usize>>,
        aabb: Aabb,
        cell_size: f64,
    ) -> Vec<usize> {
        let mut candidates = Vec::new();
        for_spatial_cells(aabb, cell_size, |key| {
            if let Some(indices) = grid.get(&key) {
                candidates.extend(indices.iter().copied());
            }
        });
        candidates
    }

    fn fragment_candidates_near_aabb(
        &self,
        grid: &HashMap<GridKey, Vec<usize>>,
        aabb: Aabb,
        cell_size: f64,
    ) -> Vec<usize> {
        let mut seen = HashSet::new();
        let mut candidates = Vec::new();
        for_spatial_cells(aabb, cell_size, |key| {
            if let Some(indices) = grid.get(&key) {
                for &index in indices {
                    if seen.insert(index) {
                        candidates.push(index);
                    }
                }
            }
        });
        candidates
    }

    fn wake_fragment(&mut self, bone: &mut BoneSegment) {
        if bone.sleeping {
            bone.sleeping = false;
            bone.sleep_frames = 0;
            self.debug.fragment_wake_events += 1;
        }
    }

    fn update_fragment_sleep_states(&mut self) {
        for bone in &mut self.bones {
            if !free_bone_fragment(*bone) {
                bone.sleeping = false;
                bone.sleep_frames = 0;
                continue;
            }

            if bone.load > self.materials.fragment_wake_load {
                if bone.sleeping {
                    bone.sleeping = false;
                    self.debug.fragment_wake_events += 1;
                }
                bone.sleep_frames = 0;
                continue;
            }

            let speed = fragment_endpoint_speed(*bone, self.materials.fixed_dt);
            let quiet = speed < self.materials.fragment_sleep_speed
                && bone.angular_velocity.abs() < self.materials.fragment_sleep_angular_speed
                && bone.load < self.materials.fragment_sleep_load;
            if quiet {
                bone.sleep_frames += 1;
                if !bone.sleeping && bone.sleep_frames >= self.materials.fragment_sleep_frames {
                    bone.sleeping = true;
                    bone.angular_velocity = 0.0;
                    bone.previous_a = bone.a;
                    bone.previous_b = bone.b;
                    self.debug.fragment_sleep_events += 1;
                }
            } else {
                if bone.sleeping {
                    bone.sleeping = false;
                    self.debug.fragment_wake_events += 1;
                }
                bone.sleep_frames = 0;
            }
        }
    }

    fn next_fluid_random(&mut self) -> f64 {
        self.fluid_seed = self
            .fluid_seed
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        f64::from((self.fluid_seed >> 8) & 0x00ff_ffff) / f64::from(0x0100_0000u32)
    }

    fn emit_fluid(
        &mut self,
        center: Vec2,
        direction: Vec2,
        count: i32,
        speed: f64,
        radius: f64,
        intensity: f64,
    ) {
        if self.materials.max_fluid_particles == 0 || count <= 0 {
            return;
        }

        let dir = normalized(direction, Vec2 { x: 0.0, y: -1.0 });
        let tangent = Vec2 {
            x: -dir.y,
            y: dir.x,
        };
        let clamped_speed = speed.clamp(55.0, 980.0);
        let clamped_radius = radius.clamp(1.35, 4.8);
        let clamped_intensity = intensity.clamp(0.35, 1.35);
        let dt = self.materials.fixed_dt;

        for _ in 0..count {
            let spread = (self.next_fluid_random() - 0.5) * 1.35;
            let launch = clamped_speed * (0.44 + self.next_fluid_random() * 0.74);
            let jitter = radius * (self.next_fluid_random() - 0.5) * 2.4;
            let velocity = add(scale(dir, launch), scale(tangent, spread * launch));
            let position = add(center, scale(tangent, jitter));
            let max_life = self.materials.fluid_lifetime * (0.62 + self.next_fluid_random() * 0.76);
            let particle = FluidParticle {
                position,
                previous: Vec2 {
                    x: position.x - velocity.x * dt,
                    y: position.y - velocity.y * dt,
                },
                radius: clamped_radius * (0.72 + self.next_fluid_random() * 0.58),
                max_life,
                life: max_life,
                intensity: clamped_intensity,
                settled: false,
                stained: false,
            };

            if self.fluids.len() < self.materials.max_fluid_particles {
                self.fluids.push(particle);
            } else if !self.fluids.is_empty() {
                let index = self.fluid_write_cursor % self.fluids.len();
                self.fluids[index] = particle;
                self.fluid_write_cursor = (self.fluid_write_cursor + 1) % self.fluids.len();
                self.debug.fluid_budget_replacements += 1;
            }
            self.stats.emitted_fluid_particles += 1;
            self.debug.fluid_emitted += 1;
        }
    }

    fn update_blood_stains(&mut self, dt: f64) {
        for stain in &mut self.blood_stains {
            if stain.intensity <= 0.0 {
                continue;
            }
            stain.age += dt;
            stain.intensity = (stain.intensity - dt * self.materials.blood_stain_decay).max(0.0);
        }
    }

    fn deposit_blood_stain(&mut self, center: Vec2, radius: f64, intensity: f64) {
        if self.materials.max_blood_stains == 0 || intensity <= 0.0 {
            return;
        }

        let clamped_radius = radius.clamp(2.0, 38.0);
        let clamped_intensity = intensity.clamp(0.05, 1.55);
        let merge_radius = self.materials.blood_stain_merge_radius + clamped_radius * 0.35;

        if let Some(stain) = self
            .blood_stains
            .iter_mut()
            .filter(|stain| stain.intensity > 0.025)
            .find(|stain| distance(stain.position, center) <= merge_radius + stain.radius * 0.40)
        {
            let existing_weight = stain.intensity.max(0.05) * stain.radius.max(1.0);
            let added_weight = clamped_intensity * clamped_radius.max(1.0);
            let weight = existing_weight + added_weight;
            stain.position = Vec2 {
                x: (stain.position.x * existing_weight + center.x * added_weight) / weight,
                y: (stain.position.y * existing_weight + center.y * added_weight) / weight,
            };
            stain.radius = (stain.radius * stain.radius + clamped_radius * clamped_radius * 0.52)
                .sqrt()
                .min(52.0);
            stain.intensity = (stain.intensity + clamped_intensity * 0.42).min(1.75);
            stain.age = 0.0;
            self.stats.blood_stain_deposits += 1;
            self.debug.blood_stain_deposits += 1;
            self.debug.active_blood_stains = self.active_blood_stain_count() as i32;
            return;
        }

        let stain = BloodStain {
            position: center,
            radius: clamped_radius,
            intensity: clamped_intensity,
            age: 0.0,
        };

        if let Some(index) = self
            .blood_stains
            .iter()
            .position(|stain| stain.intensity <= 0.025)
        {
            self.blood_stains[index] = stain;
        } else if self.blood_stains.len() < self.materials.max_blood_stains {
            self.blood_stains.push(stain);
        } else if !self.blood_stains.is_empty() {
            let index = self.blood_stain_write_cursor % self.blood_stains.len();
            self.blood_stains[index] = stain;
            self.blood_stain_write_cursor =
                (self.blood_stain_write_cursor + 1) % self.blood_stains.len();
            self.debug.blood_stain_budget_replacements += 1;
        }

        self.stats.blood_stain_deposits += 1;
        self.debug.blood_stain_deposits += 1;
        self.debug.active_blood_stains = self.active_blood_stain_count() as i32;
    }

    fn open_wound(
        &mut self,
        center: Vec2,
        direction: Vec2,
        layer: TissueLayer,
        pressure: f64,
        radius: f64,
        depth: f64,
    ) -> bool {
        self.open_wound_with_anchor(
            center, direction, layer, pressure, radius, depth, None, true,
        )
    }

    fn open_wound_with_anchor(
        &mut self,
        center: Vec2,
        direction: Vec2,
        layer: TissueLayer,
        pressure: f64,
        radius: f64,
        depth: f64,
        forced_anchor: Option<WoundAnchorCandidate>,
        merge_active_sources: bool,
    ) -> bool {
        if self.materials.max_wound_sources == 0 || pressure <= 0.0 {
            return false;
        }

        let dir = normalized(direction, Vec2 { x: 0.0, y: -1.0 });
        let clamped_pressure = (pressure * self.blood_pressure_scale()).clamp(0.12, 4.8);
        let clamped_depth = depth.clamp(0.12, 1.35);
        let clamped_radius = radius.clamp(1.3, 5.2);

        let mut target_index = self.wounds.iter().position(|wound| !wound.active);
        if merge_active_sources {
            let mut best_distance = self.materials.wound_merge_radius;
            for (index, wound) in self.wounds.iter().enumerate() {
                if !wound.active {
                    continue;
                }
                let d = distance(wound.position, center);
                if d < best_distance {
                    best_distance = d;
                    target_index = Some(index);
                }
            }
        }

        let index = if let Some(index) = target_index {
            index
        } else if self.wounds.len() < self.materials.max_wound_sources {
            self.wounds.push(WoundSource::default());
            self.wounds.len() - 1
        } else {
            self.debug.wound_budget_replacements += 1;
            self.wounds
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let av = a.pressure * (1.0 - a.clot);
                    let bv = b.pressure * (1.0 - b.clot);
                    av.partial_cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(index, _)| index)
                .unwrap_or(0)
        };

        let was_active = self.wounds[index].active;
        let previous_position = self.wounds[index].position;
        let previous_layer = self.wounds[index].layer;
        let merged_position = if was_active {
            lerp(previous_position, center, 0.35)
        } else {
            center
        };
        let merged_layer = if layer == TissueLayer::Muscle
            || (was_active && previous_layer == TissueLayer::Muscle)
        {
            TissueLayer::Muscle
        } else {
            TissueLayer::Skin
        };
        let anchor = forced_anchor
            .unwrap_or_else(|| self.choose_wound_anchor(merged_position, merged_layer));

        let target = &mut self.wounds[index];
        target.position = merged_position;
        target.direction = normalized(
            add(
                scale(target.direction, if was_active { 0.45 } else { 0.0 }),
                dir,
            ),
            dir,
        );
        target.layer = merged_layer;
        target.pressure = (target.pressure * 0.72).max(clamped_pressure) + clamped_pressure * 0.34;
        target.pressure = target.pressure.min(6.0);
        target.clot = if was_active {
            (target.clot - clamped_pressure * 0.045).max(0.0)
        } else {
            0.0
        };
        target.age = if was_active {
            target.age.min(0.45)
        } else {
            0.0
        };
        target.radius = if was_active {
            target.radius.max(clamped_radius)
        } else {
            clamped_radius
        };
        target.depth = if was_active {
            target.depth.max(clamped_depth)
        } else {
            clamped_depth
        };
        target.anchor_point = anchor.point;
        target.anchor_bone = anchor.bone;
        target.anchor_t = anchor.t;
        target.anchor_offset = anchor.offset;
        target.active = true;
        if !was_active {
            self.stats.opened_wounds += 1;
        }
        true
    }

    fn open_bone_marrow_wound(
        &mut self,
        bone_index: usize,
        t: f64,
        direction: Vec2,
        pressure: f64,
        radius: f64,
        depth: f64,
    ) {
        if bone_index >= self.bones.len() {
            return;
        }
        let t = t.clamp(0.0, 1.0);
        let position = bone_point(self.bones[bone_index], t);
        let anchor = WoundAnchorCandidate {
            point: MISSING_ANCHOR,
            bone: bone_index,
            t,
            offset: Vec2::default(),
            distance_sq: 0.0,
        };
        if self.open_wound_with_anchor(
            position,
            direction,
            TissueLayer::Muscle,
            pressure,
            radius,
            depth,
            Some(anchor),
            false,
        ) {
            self.stats.fracture_marrow_sources += 1;
            self.debug.fracture_marrow_sources += 1;
        }
    }

    fn choose_wound_anchor(&self, center: Vec2, layer: TissueLayer) -> WoundAnchorCandidate {
        let mut best_point = self.nearest_wound_point_anchor(center, layer, true);
        if best_point.is_none() {
            best_point = self.nearest_wound_point_anchor(center, layer, false);
        }
        let best_bone = self.nearest_wound_bone_anchor(center);

        match (best_point, best_bone) {
            (Some(point), Some(bone)) => {
                let point_limit = self.materials.point_spacing * 1.9;
                if bone.distance_sq < point.distance_sq * 0.65
                    || point.distance_sq > point_limit * point_limit
                {
                    bone
                } else {
                    point
                }
            }
            (Some(point), None) => point,
            (None, Some(bone)) => bone,
            (None, None) => WoundAnchorCandidate {
                point: MISSING_ANCHOR,
                bone: MISSING_ANCHOR,
                t: 0.0,
                offset: Vec2::default(),
                distance_sq: f64::INFINITY,
            },
        }
    }

    fn nearest_wound_point_anchor(
        &self,
        center: Vec2,
        layer: TissueLayer,
        same_layer_only: bool,
    ) -> Option<WoundAnchorCandidate> {
        let mut best_index = MISSING_ANCHOR;
        let mut best_distance_sq = f64::INFINITY;
        for (index, point) in self.points.iter().enumerate() {
            if same_layer_only && point.layer != layer {
                continue;
            }
            let d = distance_sq(point.position, center);
            if d < best_distance_sq {
                best_distance_sq = d;
                best_index = index;
            }
        }
        if best_index == MISSING_ANCHOR {
            None
        } else {
            Some(WoundAnchorCandidate {
                point: best_index,
                bone: MISSING_ANCHOR,
                t: 0.0,
                offset: subtract(center, self.points[best_index].position),
                distance_sq: best_distance_sq,
            })
        }
    }

    fn nearest_wound_bone_anchor(&self, center: Vec2) -> Option<WoundAnchorCandidate> {
        let mut best = None;
        let mut best_distance_sq = f64::INFINITY;
        for (index, bone) in self.bones.iter().enumerate() {
            let t = segment_t(center, bone.a, bone.b);
            let anchor = bone_point(*bone, t);
            let delta = subtract(center, anchor);
            let d = dot(delta, delta);
            if d >= best_distance_sq {
                continue;
            }
            let tangent = normalized(subtract(bone.b, bone.a), Vec2 { x: 1.0, y: 0.0 });
            let normal = Vec2 {
                x: -tangent.y,
                y: tangent.x,
            };
            best_distance_sq = d;
            best = Some(WoundAnchorCandidate {
                point: MISSING_ANCHOR,
                bone: index,
                t,
                offset: Vec2 {
                    x: dot(delta, tangent),
                    y: dot(delta, normal),
                },
                distance_sq: d,
            });
        }
        best
    }

    fn nearest_vessel_point_anchor(&self, position: Vec2) -> (usize, Vec2) {
        let mut best_index = MISSING_ANCHOR;
        let mut best_distance_sq = f64::INFINITY;
        for (index, point) in self.points.iter().enumerate() {
            if point.layer != TissueLayer::Muscle {
                continue;
            }
            let d = distance_sq(point.position, position);
            if d < best_distance_sq {
                best_distance_sq = d;
                best_index = index;
            }
        }
        if best_index == MISSING_ANCHOR {
            for (index, point) in self.points.iter().enumerate() {
                let d = distance_sq(point.position, position);
                if d < best_distance_sq {
                    best_distance_sq = d;
                    best_index = index;
                }
            }
        }
        if best_index == MISSING_ANCHOR {
            (MISSING_ANCHOR, Vec2::default())
        } else {
            (
                best_index,
                subtract(position, self.points[best_index].position),
            )
        }
    }

    fn update_vessel_anchors(&mut self) {
        for vessel in &mut self.vessels {
            if vessel.anchor_a != MISSING_ANCHOR && vessel.anchor_a < self.points.len() {
                vessel.a = add(self.points[vessel.anchor_a].position, vessel.offset_a);
            }
            if vessel.anchor_b != MISSING_ANCHOR && vessel.anchor_b < self.points.len() {
                vessel.b = add(self.points[vessel.anchor_b].position, vessel.offset_b);
            }
        }
    }

    fn update_organ_anchors(&mut self) {
        for organ in &mut self.organs {
            if organ.anchor_point != MISSING_ANCHOR && organ.anchor_point < self.points.len() {
                organ.center = add(
                    self.points[organ.anchor_point].position,
                    organ.anchor_offset,
                );
            }
        }
    }

    fn update_wound_anchors(&mut self) {
        for index in 0..self.wounds.len() {
            let point = self.wounds[index].anchor_point;
            let bone = self.wounds[index].anchor_bone;
            let t = self.wounds[index].anchor_t;
            let offset = self.wounds[index].anchor_offset;
            if point != MISSING_ANCHOR && point < self.points.len() {
                self.wounds[index].position = add(self.points[point].position, offset);
            } else if bone != MISSING_ANCHOR && bone < self.bones.len() {
                let segment = self.bones[bone];
                let anchor = bone_point(segment, t);
                let tangent = normalized(subtract(segment.b, segment.a), Vec2 { x: 1.0, y: 0.0 });
                let normal = Vec2 {
                    x: -tangent.y,
                    y: tangent.x,
                };
                self.wounds[index].position = add(
                    anchor,
                    add(scale(tangent, offset.x), scale(normal, offset.y)),
                );
            }
        }
    }

    fn disturb_wounds_from_loaded_tissue(&mut self) {
        let budget = self.materials.max_wound_reopens_per_step;
        let radius = self.materials.wound_reopen_radius.max(0.0);
        let threshold = self.materials.wound_reopen_load_threshold.max(EPSILON);
        if budget == 0 || radius <= EPSILON || self.wounds.is_empty() || self.points.is_empty() {
            return;
        }

        let radius_sq = radius * radius;
        let mut events = Vec::new();
        for (wound_index, wound) in self.wounds.iter().enumerate() {
            if events.len() >= budget {
                break;
            }
            let clot_gate = if wound.active { 0.42 } else { 0.18 };
            if wound.age < 0.24 || wound.clot < clot_gate {
                continue;
            }

            let mut best_load = 0.0;
            let mut best_direction = wound.direction;
            for point in &self.points {
                let d2 = distance_sq(point.position, wound.position);
                if d2 > radius_sq {
                    continue;
                }
                let layer_scale = if point.layer == wound.layer {
                    1.0
                } else if wound.layer == TissueLayer::Muscle {
                    0.74
                } else {
                    0.58
                };
                let falloff = 1.0 - (d2.sqrt() / radius).clamp(0.0, 1.0) * 0.45;
                let local_speed =
                    distance(point.position, point.previous) / self.materials.fixed_dt.max(EPSILON);
                let motion_load = local_speed * point.mass * 0.11;
                let contusion_load =
                    point.contusion * self.materials.contusion_load_threshold * 0.30;
                let local_load =
                    (point.load + motion_load + contusion_load) * layer_scale * falloff;
                if local_load > best_load {
                    best_load = local_load;
                    best_direction =
                        normalized(subtract(wound.position, point.position), wound.direction);
                }
            }

            if best_load <= threshold {
                continue;
            }
            let severity = ((best_load - threshold) / threshold).clamp(0.0, 1.6);
            events.push((wound_index, severity, best_direction));
        }

        for (wound_index, severity, direction) in events {
            let wound = &mut self.wounds[wound_index];
            let reopened_pressure =
                0.18 + self.materials.wound_reopen_pressure_scale * (1.0 + severity);
            wound.pressure = wound.pressure.max(reopened_pressure).min(6.0);
            wound.clot =
                (wound.clot - self.materials.wound_reopen_clot_loss * (0.8 + severity)).max(0.0);
            wound.clot = wound.clot.min(0.36);
            wound.accumulator = wound.accumulator.max(0.16 + severity * 0.22);
            wound.direction = normalized(
                add(scale(wound.direction, 0.58), scale(direction, 0.42)),
                direction,
            );
            wound.age = wound.age.min(0.40);
            wound.active = true;
            self.stats.wound_reopens += 1;
            self.debug.wound_reopens += 1;
            self.debug.max_wound_pressure = self.debug.max_wound_pressure.max(wound.pressure);
            self.debug.max_wound_clot = self.debug.max_wound_clot.max(wound.clot);
        }

        if self.debug.wound_reopens > 0 {
            self.debug.active_wounds = self.active_wound_count() as i32;
        }
    }

    fn integrate(&mut self, dt: f64, width: f64, floor_y: f64) {
        let blood_turgor = self.blood_turgor_scale_internal();
        for point in &mut self.points {
            point.load *= 0.84;
            point.exposure *= 0.92;
            point.contusion = (point.contusion - dt * self.materials.contusion_decay).max(0.0);
            if point.pinned {
                point.position = point.home;
                point.previous = point.position;
                continue;
            }
            let vx = (point.position.x - point.previous.x) * self.materials.damping;
            let vy = (point.position.y - point.previous.y) * self.materials.damping;
            point.previous = point.position;
            point.position.x += vx;
            point.position.y += vy + self.materials.gravity * dt * dt;
            let base_shape_stiffness = if point.layer == TissueLayer::Skin {
                self.materials.skin_shape_stiffness
            } else {
                self.materials.muscle_shape_stiffness
            };
            let shape_stiffness = base_shape_stiffness * blood_turgor;
            point.position.x += (point.home.x - point.position.x) * shape_stiffness;
            point.position.y += (point.home.y - point.position.y) * shape_stiffness;
            if point.position.y > floor_y {
                point.position.y = floor_y;
                point.previous.x = point.position.x
                    + (point.previous.x - point.position.x) * self.materials.floor_friction;
            }
        }

        for index in 0..self.bones.len() {
            let mut bone = self.bones[index];
            bone.load *= 0.88;
            if bone.pinned {
                bone.a = bone.home_a;
                bone.b = bone.home_b;
                bone.previous_a = bone.a;
                bone.previous_b = bone.b;
                self.bones[index] = bone;
                continue;
            }
            if bone.sleeping && free_bone_fragment(bone) {
                bone.load *= 0.50;
                bone.angular_velocity = 0.0;
                bone.previous_a = bone.a;
                bone.previous_b = bone.b;
                self.bones[index] = bone;
                continue;
            }
            let avx = (bone.a.x - bone.previous_a.x) * self.materials.bone_damping;
            let avy = (bone.a.y - bone.previous_a.y) * self.materials.bone_damping;
            let bvx = (bone.b.x - bone.previous_b.x) * self.materials.bone_damping;
            let bvy = (bone.b.y - bone.previous_b.y) * self.materials.bone_damping;
            let shape_stiffness = if bone.fractured {
                0.0
            } else {
                self.materials.bone_shape_stiffness
            };
            bone.angular_velocity *= self.materials.bone_angular_damping;
            let free_fragment = bone.fractured || bone.splinter;
            if !free_fragment && bone.angular_velocity.abs() < 0.01 {
                bone.angular_velocity = 0.0;
            }
            if free_fragment {
                self.debug.max_bone_angular_speed = self
                    .debug
                    .max_bone_angular_speed
                    .max(bone.angular_velocity.abs());
            }
            bone.previous_a = bone.a;
            bone.previous_b = bone.b;
            bone.a.x += avx + (bone.home_a.x - bone.a.x) * shape_stiffness;
            bone.a.y += avy
                + self.materials.gravity * dt * dt
                + (bone.home_a.y - bone.a.y) * shape_stiffness;
            bone.b.x += bvx + (bone.home_b.x - bone.b.x) * shape_stiffness;
            bone.b.y += bvy
                + self.materials.gravity * dt * dt
                + (bone.home_b.y - bone.b.y) * shape_stiffness;
            if free_fragment {
                let angular_step = bone.angular_velocity * dt;
                rotate_bone_around_center(&mut bone, angular_step);
            }
            self.bones[index] = bone;
        }

        self.update_blood_stains(dt);
        let mut stain_deposits = Vec::new();
        for fluid in &mut self.fluids {
            if fluid.life <= 0.0 {
                continue;
            }
            fluid.life = (fluid.life - dt).max(0.0);
            if fluid.life <= 0.0 {
                continue;
            }
            if fluid.settled {
                fluid.life = (fluid.life - dt * 0.45).max(0.0);
                continue;
            }
            let vx = (fluid.position.x - fluid.previous.x) * self.materials.fluid_damping;
            let vy = (fluid.position.y - fluid.previous.y) * self.materials.fluid_damping;
            fluid.previous = fluid.position;
            fluid.position.x += vx;
            fluid.position.y +=
                vy + self.materials.gravity * self.materials.fluid_gravity_scale * dt * dt;
            let margin = fluid.radius + 1.0;
            if fluid.position.x < margin {
                fluid.position.x = margin;
                fluid.previous.x = fluid.position.x + vx * self.materials.fluid_floor_friction;
            } else if fluid.position.x > width - margin {
                fluid.position.x = width - margin;
                fluid.previous.x = fluid.position.x + vx * self.materials.fluid_floor_friction;
            }
            if fluid.position.y > floor_y - fluid.radius {
                fluid.position.y = floor_y - fluid.radius;
                fluid.previous.x = fluid.position.x + vx * self.materials.fluid_floor_friction;
                fluid.previous.y = fluid.position.y + vy * self.materials.fluid_floor_friction;
                let floor_hit_speed = vy.abs() + vx.abs() * 0.32;
                if !fluid.stained && floor_hit_speed > 0.45 {
                    let stain_radius = fluid.radius
                        * (self.materials.blood_stain_spread
                            + fluid.intensity * 0.78
                            + floor_hit_speed.min(18.0) * 0.035);
                    let stain_intensity = (fluid.intensity
                        * (0.72 + floor_hit_speed.min(22.0) * 0.018))
                        .clamp(0.08, 1.45);
                    stain_deposits.push((
                        Vec2 {
                            x: fluid.position.x,
                            y: floor_y - fluid.radius * 0.18,
                        },
                        stain_radius,
                        stain_intensity,
                    ));
                    fluid.stained = true;
                }
                if vx.abs() + vy.abs() < 1.2 {
                    fluid.settled = true;
                }
            }
        }
        for (position, radius, intensity) in stain_deposits {
            self.deposit_blood_stain(position, radius, intensity);
        }
    }

    fn blood_volume_fraction_internal(&self) -> f64 {
        let capacity = self.materials.blood_volume_capacity.max(EPSILON);
        (self.blood_volume / capacity).clamp(0.0, 1.0)
    }

    fn blood_pressure_scale(&self) -> f64 {
        let min_scale = self.materials.blood_pressure_min_scale.clamp(0.05, 1.0);
        min_scale + (1.0 - min_scale) * self.blood_volume_fraction_internal()
    }

    fn blood_turgor_scale_internal(&self) -> f64 {
        let min_scale = self.materials.blood_turgor_min_scale.clamp(0.10, 1.0);
        min_scale + (1.0 - min_scale) * self.blood_volume_fraction_internal()
    }

    fn drain_blood_volume(&mut self, count: i32, intensity: f64) {
        if count <= 0 || self.materials.blood_volume_capacity <= 0.0 {
            return;
        }
        let loss = f64::from(count)
            * self.materials.blood_loss_per_wound_particle.max(0.0)
            * intensity.clamp(0.25, 2.2);
        if loss <= 0.0 {
            return;
        }
        let drained = loss.min(self.blood_volume.max(0.0));
        self.blood_volume = (self.blood_volume - drained).max(0.0);
        self.stats.blood_loss += drained;
        self.debug.blood_loss = self.stats.blood_loss;
        self.debug.blood_volume_fraction = self.blood_volume_fraction_internal();
        self.debug.blood_turgor_scale = self.blood_turgor_scale_internal();
    }

    fn update_wounds(&mut self, dt: f64) {
        let mut emissions = Vec::new();
        let systemic_pressure = self.blood_pressure_scale();
        self.debug.blood_loss = self.stats.blood_loss;
        self.debug.blood_volume_fraction = self.blood_volume_fraction_internal();
        self.debug.blood_turgor_scale = self.blood_turgor_scale_internal();
        for wound in &mut self.wounds {
            if !wound.active {
                continue;
            }
            wound.age += dt;
            let layer_scale = if wound.layer == TissueLayer::Muscle {
                1.35
            } else {
                0.78
            };
            let open_factor = (1.0 - wound.clot).max(0.0);
            self.debug.max_wound_pressure = self.debug.max_wound_pressure.max(wound.pressure);
            self.debug.max_wound_clot = self.debug.max_wound_clot.max(wound.clot);
            wound.accumulator += dt
                * self.materials.wound_leak_rate
                * wound.pressure
                * systemic_pressure
                * open_factor
                * layer_scale
                * (0.45 + wound.depth * 0.82);
            let effective_pressure = wound.pressure * systemic_pressure;
            if wound.age < 0.42 && effective_pressure > self.materials.wound_spray_pressure {
                wound.accumulator +=
                    dt * (effective_pressure - self.materials.wound_spray_pressure) * 2.1;
            }
            let count = (wound.accumulator.floor() as i32).min(4);
            if count > 0 {
                wound.accumulator -= f64::from(count);
                let spray = ((effective_pressure - self.materials.wound_spray_pressure) / 2.4)
                    .clamp(0.0, 1.0)
                    * (1.0 - wound.age / 0.9).clamp(0.0, 1.0);
                let leak_direction = normalized(
                    Vec2 {
                        x: wound.direction.x * (0.25 + spray * 0.85),
                        y: wound.direction.y * (0.25 + spray * 0.85) + 0.85 - spray * 0.30,
                    },
                    Vec2 { x: 0.0, y: 1.0 },
                );
                emissions.push((
                    wound.position,
                    leak_direction,
                    count,
                    45.0 + effective_pressure * (38.0 + spray * 92.0),
                    wound.radius * (0.64 + wound.depth * 0.18),
                    0.58 + wound.depth * 0.42 + spray * 0.18,
                ));
            }
            wound.pressure = (wound.pressure
                - dt * self.materials.wound_pressure_decay * (0.36 + wound.clot))
                .max(0.0);
            let clot_layer_scale = if wound.layer == TissueLayer::Skin {
                1.16
            } else {
                0.72
            };
            wound.clot = (wound.clot
                + dt * self.materials.wound_clot_rate
                    * clot_layer_scale
                    * (0.82 + 0.32 / wound.pressure.max(0.35)))
            .min(1.0);
            if wound.pressure < 0.055 || wound.clot > 0.985 {
                wound.active = false;
                wound.accumulator = 0.0;
            } else {
                self.debug.active_wounds += 1;
            }
        }

        for (position, direction, count, speed, radius, intensity) in emissions {
            self.drain_blood_volume(count, intensity);
            let before = self.stats.emitted_fluid_particles;
            self.emit_fluid(position, direction, count, speed, radius, intensity);
            let emitted = self.stats.emitted_fluid_particles - before;
            self.stats.wound_fluid_particles += emitted;
            self.debug.wound_leaks += emitted;
        }
    }

    fn update_cavities(&mut self, dt: f64) {
        if self.cavities.is_empty() {
            return;
        }

        let mut rupture_events = Vec::new();
        for index in 0..self.cavities.len() {
            let area_indices = self.cavities[index].area_indices.clone();
            let metrics = self.cavity_metrics(&area_indices);
            if metrics.samples == 0 {
                continue;
            }

            let rest_area = self.cavities[index].rest_area.max(EPSILON);
            let area_fraction = (metrics.current_area / rest_area).clamp(0.0, 1.35);
            let min_fraction = self.materials.cavity_min_area_fraction.clamp(0.35, 0.96);
            let collapse =
                ((1.0 - area_fraction) / (1.0 - min_fraction).max(EPSILON)).clamp(0.0, 1.0);
            let load_drive_cap = if self.debug.tool == ToolMode::Heavy {
                2.4
            } else {
                self.materials
                    .cavity_non_heavy_pressure_load_cap
                    .clamp(0.4, 2.4)
            };
            let load_drive = ((metrics.peak_load - self.materials.cavity_load_threshold)
                / self.materials.cavity_load_threshold.max(1.0))
            .clamp(0.0, load_drive_cap)
                * self.materials.cavity_pressure_load_scale.max(0.0);
            let target_pressure = (collapse * self.materials.cavity_pressure_stiffness.max(0.0)
                + load_drive)
                .clamp(0.0, 3.0);
            let decay = (1.0 - dt * self.materials.cavity_pressure_decay.max(0.0)).clamp(0.0, 1.0);
            let pressure = (self.cavities[index].pressure * decay)
                .max(target_pressure)
                .clamp(0.0, 3.0);
            let stored_collapse = (self.cavities[index].collapse * decay)
                .max(collapse)
                .clamp(0.0, 1.0);

            self.cavities[index].pressure = pressure;
            self.cavities[index].collapse = stored_collapse;
            self.cavities[index].centroid = metrics.centroid;
            self.debug.max_cavity_pressure = self.debug.max_cavity_pressure.max(pressure);
            self.debug.max_cavity_collapse = self.debug.max_cavity_collapse.max(stored_collapse);

            if pressure > 0.02 {
                self.apply_cavity_pressure_to_points(
                    &area_indices,
                    metrics.centroid,
                    pressure,
                    stored_collapse,
                    metrics.average_load,
                );
            }

            if pressure >= self.materials.cavity_contusion_pressure {
                self.stats.cavity_pressure_events += 1;
                self.debug.cavity_pressure_events += 1;
            }

            let heavy_impact_drive = self.debug.tool == ToolMode::Heavy
                && self.debug.impact
                    >= self.materials.cavity_load_threshold
                        * self.materials.cavity_rupture_load_scale.max(1.0)
                        * 2.0;
            let rupture_load_scale = if self.debug.tool == ToolMode::Heavy {
                self.materials.cavity_rupture_load_scale.max(0.0)
            } else {
                self.materials.cavity_rupture_load_scale.max(0.0)
                    * self.materials.cavity_non_heavy_rupture_load_scale.max(1.0)
            };
            if !self.cavities[index].ruptured
                && pressure >= self.materials.cavity_rupture_pressure
                && stored_collapse >= self.materials.cavity_rupture_min_collapse
                && (metrics.peak_load >= self.materials.cavity_load_threshold * rupture_load_scale
                    || heavy_impact_drive)
                && rupture_events.len() < self.materials.max_cavity_ruptures_per_step
            {
                self.cavities[index].ruptured = true;
                let (site, normal, site_load) =
                    self.cavity_rupture_site(&area_indices, metrics.centroid);
                rupture_events.push((site, normal, pressure, stored_collapse, site_load));
            }
        }

        for (position, normal, pressure, collapse, site_load) in rupture_events {
            let drive = site_load.max(self.materials.cavity_load_threshold * pressure);
            self.emit_fluid(
                position,
                normal,
                2 + (pressure * 3.0).clamp(0.0, 6.0) as i32,
                76.0 + drive * self.materials.fluid_impact_scale * 0.32,
                2.0 + collapse * 0.55,
                0.82 + pressure.min(1.4) * 0.12,
            );
            self.open_wound(
                position,
                normal,
                TissueLayer::Muscle,
                pressure * (0.82 + collapse * 0.28),
                2.05 + collapse * 0.45,
                0.96 + collapse * 0.22,
            );
            self.stats.cavity_ruptures += 1;
            self.debug.cavity_ruptures += 1;
        }
    }

    fn cavity_metrics(&self, area_indices: &[usize]) -> CavityMetrics {
        let mut metrics = CavityMetrics::default();
        let mut weighted_centroid = Vec2::default();
        let mut centroid_weight = 0.0;
        let mut load_sum = 0.0;
        for &area_index in area_indices {
            if area_index >= self.areas.len() {
                continue;
            }
            let area = self.areas[area_index];
            if area.layer != TissueLayer::Muscle
                || area.a >= self.points.len()
                || area.b >= self.points.len()
                || area.c >= self.points.len()
            {
                continue;
            }
            let a = self.points[area.a];
            let b = self.points[area.b];
            let c = self.points[area.c];
            let current_area = signed_area(a.position, b.position, c.position).abs();
            let weight = area.rest_area.abs().max(current_area).max(1.0);
            let centroid = Vec2 {
                x: (a.position.x + b.position.x + c.position.x) / 3.0,
                y: (a.position.y + b.position.y + c.position.y) / 3.0,
            };
            metrics.current_area += current_area;
            weighted_centroid.x += centroid.x * weight;
            weighted_centroid.y += centroid.y * weight;
            centroid_weight += weight;
            let area_load = a.load.max(b.load).max(c.load);
            load_sum += area_load;
            metrics.peak_load = metrics.peak_load.max(area_load);
            metrics.samples += 1;
        }
        if metrics.samples > 0 {
            metrics.centroid = scale(weighted_centroid, 1.0 / centroid_weight.max(EPSILON));
            metrics.average_load = load_sum / metrics.samples as f64;
        }
        metrics
    }

    fn apply_cavity_pressure_to_points(
        &mut self,
        area_indices: &[usize],
        centroid: Vec2,
        pressure: f64,
        collapse: f64,
        _average_load: f64,
    ) {
        let mut point_indices = HashSet::new();
        for &area_index in area_indices {
            if let Some(area) = self.areas.get(area_index) {
                point_indices.insert(area.a);
                point_indices.insert(area.b);
                point_indices.insert(area.c);
            }
        }

        let push = pressure
            * self.materials.cavity_tissue_push.max(0.0)
            * (0.35 + collapse.clamp(0.0, 1.0) * 0.65);
        let pressure_ratio = pressure / self.materials.cavity_contusion_pressure.max(0.1);
        let cavity_load =
            self.materials.cavity_load_threshold * pressure_ratio.clamp(0.0, 2.2) * 0.38;
        for point_index in point_indices {
            if point_index >= self.points.len() || self.points[point_index].pinned {
                continue;
            }
            let fallback = subtract(self.points[point_index].home, centroid);
            let direction = normalized(
                subtract(self.points[point_index].position, centroid),
                fallback,
            );
            self.points[point_index].position.x += direction.x * push;
            self.points[point_index].position.y += direction.y * push;
            self.points[point_index].load = self.points[point_index].load.max(cavity_load);
            if pressure >= self.materials.cavity_contusion_pressure {
                let contusion_load =
                    self.materials.contusion_load_threshold * pressure_ratio.clamp(0.0, 2.2) * 0.74;
                if apply_point_contusion(
                    &mut self.points[point_index],
                    self.materials,
                    contusion_load,
                    0.58,
                ) {
                    self.stats.contusion_events += 1;
                    self.debug.contusion_events += 1;
                    self.debug.max_contusion = self
                        .debug
                        .max_contusion
                        .max(self.points[point_index].contusion);
                }
            }
        }
    }

    fn cavity_rupture_site(
        &self,
        area_indices: &[usize],
        fallback_centroid: Vec2,
    ) -> (Vec2, Vec2, f64) {
        let mut best_score = f64::NEG_INFINITY;
        let mut best_site = fallback_centroid;
        let mut best_normal = Vec2 { x: 0.0, y: -1.0 };
        let mut best_load = 0.0;
        for &area_index in area_indices {
            if area_index >= self.areas.len() {
                continue;
            }
            let area = self.areas[area_index];
            if area.layer != TissueLayer::Muscle
                || area.a >= self.points.len()
                || area.b >= self.points.len()
                || area.c >= self.points.len()
            {
                continue;
            }
            let a = self.points[area.a];
            let b = self.points[area.b];
            let c = self.points[area.c];
            let load = (a.load + b.load + c.load) / 3.0;
            let contusion = (a.contusion + b.contusion + c.contusion) / 3.0;
            let compression = (1.0
                - signed_area(a.position, b.position, c.position).abs()
                    / area.rest_area.abs().max(EPSILON))
            .max(0.0);
            let score =
                load + contusion * self.materials.cavity_load_threshold + compression * 220.0;
            if score <= best_score {
                continue;
            }
            let site = Vec2 {
                x: (a.position.x + b.position.x + c.position.x) / 3.0,
                y: (a.position.y + b.position.y + c.position.y) / 3.0,
            };
            let ab = subtract(b.position, a.position);
            let ac = subtract(c.position, a.position);
            let normal = normalized(
                Vec2 {
                    x: -(ab.y + ac.y * 0.45),
                    y: ab.x + ac.x * 0.45 - 0.35,
                },
                normalized(subtract(site, fallback_centroid), Vec2 { x: 0.0, y: -1.0 }),
            );
            best_score = score;
            best_site = site;
            best_normal = normal;
            best_load = load;
        }
        (best_site, best_normal, best_load)
    }

    fn update_organs(&mut self, dt: f64) {
        if self.organs.is_empty() {
            return;
        }

        let mut rupture_events = Vec::new();
        for index in 0..self.organs.len() {
            let organ = self.organs[index];
            let pressure = self.organ_cavity_pressure(organ);
            let cavity_collapse = self.organ_cavity_collapse(organ);
            let local_load = self.organ_local_point_load(organ);
            let fragment_load = self.organ_fragment_load(organ);

            let pressure_damage = ((pressure - self.materials.organ_pressure_damage_threshold)
                / self.materials.organ_pressure_damage_threshold.max(EPSILON))
            .clamp(0.0, 2.4);
            let load_damage = ((local_load.max(fragment_load)
                - self.materials.organ_load_damage_threshold)
                / self.materials.organ_load_damage_threshold.max(EPSILON))
            .clamp(0.0, 2.8);
            let fragment_damage = ((fragment_load
                - self.materials.organ_fragment_damage_threshold)
                / self.materials.organ_fragment_damage_threshold.max(EPSILON))
            .clamp(0.0, 2.5);
            let kind_scale = match organ.kind {
                OrganKind::LeftLung | OrganKind::RightLung => 1.10,
                OrganKind::Liver => 1.0,
                OrganKind::Spleen => 1.24,
            };
            let delta = (pressure_damage * 0.46 + load_damage * 0.42 + fragment_damage * 0.72)
                * self.materials.organ_damage_rate.max(0.0)
                * (dt * 60.0).clamp(0.25, 2.0)
                * kind_scale;
            if delta > 0.0005 {
                self.organs[index].damage = (self.organs[index].damage + delta).min(1.8);
                self.organs[index].pressure_damage =
                    self.organs[index].pressure_damage.max(pressure_damage);
                self.organs[index].load_damage = self.organs[index]
                    .load_damage
                    .max(load_damage.max(fragment_damage));
                self.stats.organ_damage_events += 1;
                self.debug.organ_damage_events += 1;
            }
            self.debug.max_organ_damage =
                self.debug.max_organ_damage.max(self.organs[index].damage);

            let heavy_cavity_drive = self.debug.tool == ToolMode::Heavy
                && pressure >= self.materials.organ_pressure_damage_threshold * 1.15
                && cavity_collapse >= self.materials.cavity_rupture_min_collapse;
            let cavity_rupture_drive = self.debug.tool == ToolMode::Heavy
                && self.stats.cavity_ruptures > 0
                && pressure >= self.materials.organ_pressure_damage_threshold * 0.75;
            let fragment_drive = fragment_damage >= 1.0
                && pressure >= self.materials.organ_pressure_damage_threshold
                && cavity_collapse >= self.materials.cavity_rupture_min_collapse;
            if !self.organs[index].ruptured
                && self.organs[index].damage >= self.materials.organ_rupture_damage
                && (heavy_cavity_drive || cavity_rupture_drive || fragment_drive)
                && rupture_events.len() < self.materials.max_organ_ruptures_per_step
                && self.stats.organ_ruptures as usize + rupture_events.len()
                    < self.materials.max_total_organ_ruptures
            {
                self.organs[index].ruptured = true;
                let direction = normalized(
                    subtract(self.organs[index].center, self.nearest_body_center()),
                    Vec2 { x: 0.0, y: -1.0 },
                );
                rupture_events.push((
                    self.organs[index].center,
                    direction,
                    self.organs[index].damage,
                    pressure,
                    local_load.max(fragment_load),
                ));
            }
        }

        for (position, direction, damage, pressure, load) in rupture_events {
            self.open_organ_rupture(position, direction, damage, pressure, load);
        }
    }

    fn open_organ_rupture(
        &mut self,
        position: Vec2,
        direction: Vec2,
        damage: f64,
        pressure: f64,
        load: f64,
    ) {
        let bleed_pressure = self.materials.organ_bleed_pressure
            * (0.82 + pressure.max(0.0) * 0.28 + damage.min(1.5) * 0.22);
        self.emit_fluid(
            position,
            direction,
            3 + (damage * 3.0).clamp(0.0, 5.0) as i32,
            68.0 + load * self.materials.fluid_impact_scale * 0.22,
            2.0 + damage.min(1.5) * 0.42,
            0.92 + damage.min(1.5) * 0.16,
        );
        self.open_wound(
            position,
            direction,
            TissueLayer::Muscle,
            bleed_pressure,
            2.1 + damage.min(1.5) * 0.36,
            1.05,
        );
        self.stats.organ_ruptures += 1;
        self.debug.organ_ruptures += 1;
    }

    fn organ_cavity_pressure(&self, organ: OrganRegion) -> f64 {
        self.cavities
            .iter()
            .map(|cavity| {
                let dx = (organ.center.x - cavity.centroid.x) / (organ.radius.x * 3.2).max(1.0);
                let dy = (organ.center.y - cavity.centroid.y) / (organ.radius.y * 3.4).max(1.0);
                let influence = (1.0 - (dx * dx + dy * dy).sqrt() * 0.34).clamp(0.0, 1.0);
                cavity.pressure * influence * (0.74 + cavity.collapse * 0.36)
            })
            .fold(0.0, f64::max)
    }

    fn organ_cavity_collapse(&self, organ: OrganRegion) -> f64 {
        self.cavities
            .iter()
            .map(|cavity| {
                let dx = (organ.center.x - cavity.centroid.x) / (organ.radius.x * 3.2).max(1.0);
                let dy = (organ.center.y - cavity.centroid.y) / (organ.radius.y * 3.4).max(1.0);
                let influence = (1.0 - (dx * dx + dy * dy).sqrt() * 0.34).clamp(0.0, 1.0);
                cavity.collapse * influence
            })
            .fold(0.0, f64::max)
    }

    fn organ_local_point_load(&self, organ: OrganRegion) -> f64 {
        self.points
            .iter()
            .filter(|point| point.layer == TissueLayer::Muscle)
            .filter(|point| point_in_organ(point.position, organ))
            .map(|point| point.load)
            .fold(0.0, f64::max)
    }

    fn organ_fragment_load(&self, organ: OrganRegion) -> f64 {
        self.bones
            .iter()
            .filter(|bone| bone.fractured || bone.splinter)
            .filter_map(|bone| {
                let t = segment_t(organ.center, bone.a, bone.b);
                let closest = bone_point(*bone, t);
                if point_in_organ(closest, organ) {
                    let speed = (distance(bone.a, bone.previous_a)
                        + distance(bone.b, bone.previous_b))
                        * 0.5;
                    Some(bone.load.max(speed * bone.radius * 14.0))
                } else {
                    None
                }
            })
            .fold(0.0, f64::max)
    }

    fn puncture_organs_from_rib_tip(
        &mut self,
        bone: BoneSegment,
        tip: Vec2,
        previous_tip: Vec2,
        tip_normal: Vec2,
        impulse: f64,
        radius: f64,
    ) {
        let budget = self.materials.max_rib_organ_punctures_per_step;
        let threshold = self.materials.rib_organ_puncture_impulse.max(EPSILON);
        let severe_body_drive = self.debug.tool == ToolMode::Heavy
            || self.debug.impact >= threshold * 3.0
            || impulse >= threshold * 2.8;
        if budget == 0
            || bone.kind != BoneKind::Rib
            || (!bone.fractured && !bone.splinter)
            || (self.debug.rib_organ_punctures as usize) >= budget
            || impulse < threshold * 0.45
            || !severe_body_drive
            || self.organs.is_empty()
        {
            return;
        }

        let travel = distance(tip, previous_tip);
        let travel_dir = normalized(subtract(tip, previous_tip), tip_normal);
        let reach = self
            .materials
            .rib_organ_puncture_radius
            .max(radius * if bone.splinter { 1.9 } else { 1.45 });
        let mut rupture_events = Vec::new();

        for index in 0..self.organs.len() {
            if (self.debug.rib_organ_punctures as usize) >= budget {
                break;
            }
            let organ = self.organs[index];
            if organ.rib_punctured {
                continue;
            }

            let path_t = if travel > EPSILON {
                segment_t(organ.center, previous_tip, tip)
            } else {
                1.0
            };
            let path_point = lerp(previous_tip, tip, path_t);
            let dx = (path_point.x - organ.center.x) / organ.radius.x.max(EPSILON);
            let dy = (path_point.y - organ.center.y) / organ.radius.y.max(EPSILON);
            let normalized_distance = (dx * dx + dy * dy).sqrt();
            let average_radius = ((organ.radius.x + organ.radius.y) * 0.5).max(1.0);
            let normalized_reach = (reach / average_radius).max(0.06);
            if normalized_distance > 1.0 + normalized_reach {
                continue;
            }

            let contact = if normalized_distance <= 1.0 {
                (1.0 - normalized_distance * 0.12).clamp(0.74, 1.0)
            } else {
                (1.0 - (normalized_distance - 1.0) / normalized_reach).clamp(0.0, 1.0)
            };
            if contact <= EPSILON {
                continue;
            }

            let inward = normalized(subtract(organ.center, path_point), tip_normal);
            let alignment = dot(travel_dir, inward).clamp(0.0, 1.0);
            let drive = impulse
                * contact
                * (0.72 + alignment * 0.34)
                * if bone.splinter { 1.16 } else { 1.0 };
            if drive <= threshold {
                continue;
            }

            let severity = ((drive - threshold) / threshold).clamp(0.0, 2.4);
            let kind_scale = match organ.kind {
                OrganKind::LeftLung | OrganKind::RightLung => 1.18,
                OrganKind::Liver => 0.88,
                OrganKind::Spleen => 0.94,
            };
            let damage = self.materials.rib_organ_puncture_damage.max(0.0)
                * (0.74 + severity * 0.52)
                * kind_scale;
            let new_damage = (self.organs[index].damage + damage).min(1.8);
            self.organs[index].damage = new_damage;
            self.organs[index].penetration_damage = self.organs[index]
                .penetration_damage
                .max((damage / self.materials.organ_rupture_damage.max(EPSILON)).clamp(0.0, 1.8));
            self.organs[index].load_damage = self.organs[index]
                .load_damage
                .max((drive / threshold).clamp(0.0, 2.8));
            self.organs[index].penetrated = true;
            self.organs[index].rib_punctured = true;
            self.stats.organ_damage_events += 1;
            self.debug.organ_damage_events += 1;
            self.stats.rib_organ_punctures += 1;
            self.debug.rib_organ_punctures += 1;
            self.debug.max_organ_damage = self.debug.max_organ_damage.max(new_damage);

            if new_damage >= self.materials.organ_rupture_damage
                && rupture_events.len() < self.materials.max_organ_ruptures_per_step
                && self.stats.organ_ruptures as usize + rupture_events.len()
                    < self.materials.max_total_organ_ruptures
            {
                self.organs[index].ruptured = true;
                let direction = normalized(
                    add(scale(travel_dir, 0.72), scale(tip_normal, 0.28)),
                    travel_dir,
                );
                rupture_events.push((
                    path_point,
                    direction,
                    new_damage,
                    0.0,
                    drive * (0.34 + severity * 0.10),
                ));
            }
        }

        for (position, direction, damage, pressure, load) in rupture_events {
            self.open_organ_rupture(position, direction, damage, pressure, load);
        }
    }

    fn nearest_body_center(&self) -> Vec2 {
        if self.cavities.is_empty() {
            if self.points.is_empty() {
                return Vec2::default();
            }
            let sum = self
                .points
                .iter()
                .fold(Vec2::default(), |sum, point| add(sum, point.position));
            return scale(sum, 1.0 / self.points.len() as f64);
        }
        self.cavities[0].centroid
    }

    fn collide_striker(&mut self, dt: f64, input: &InputState) {
        if !input.active {
            return;
        }
        let profile = tool_profile(input.tool);
        let speed = hypot(input.vx, input.vy);
        let radius = self.materials.striker_radius * profile.radius_scale;
        let impact = speed * self.materials.striker_mass * input.power * profile.mass_scale;
        let shape = make_tool_contact_shape(input, profile, radius);
        let influence = shape.influence;

        let initial_bone_count = self.bones.len();
        for i in 0..initial_bone_count {
            let mut bone = self.bones[i];
            bone.load *= 0.88;
            let mut t = segment_t(shape.center, bone.a, bone.b);
            let mut closest = bone_point(bone, t);
            let mut tool_contact = shape.center;
            let mut dist = distance(closest, tool_contact);
            if shape.blade_segment {
                let closest_pair =
                    closest_segment_points(shape.axis_start, shape.axis_end, bone.a, bone.b);
                t = closest_pair.t_b;
                closest = closest_pair.point_b;
                tool_contact = closest_pair.point_a;
                dist = closest_pair.distance;
            }
            if !input.down || dist > influence + bone.radius {
                self.bones[i] = bone;
                continue;
            }
            let mut normal = normalized(
                subtract(closest, tool_contact),
                if shape.blade_segment {
                    shape.blade_normal
                } else {
                    shape.direction
                },
            );
            if dist < EPSILON && !shape.blade_segment && speed > EPSILON {
                normal = shape.direction;
                dist = 1.0;
            }
            let depth = (influence + bone.radius - dist).max(0.0);
            let contact = (1.0 - ((dist - bone.radius) / influence).clamp(0.0, 1.0)).max(0.0);
            let direct_load = (impact + self.materials.bone_direct_pressure * input.power)
                * contact
                * profile.bone_load_scale;
            bone.load = bone.load.max(direct_load);
            if direct_load > self.materials.fragment_wake_load
                || speed > self.materials.fragment_sleep_speed * 2.0
            {
                self.wake_fragment(&mut bone);
            }
            self.debug.bone_contacts += 1;
            if depth > self.debug.max_depth {
                self.debug.max_depth = depth;
                self.debug.strongest_contact = closest;
            }
            self.debug.max_bone_load = self.debug.max_bone_load.max(bone.load);
            if !bone.pinned {
                let contact_strength = self.materials.bone_direct_contact
                    * profile.bone_push_scale
                    * (0.78 + input.power * 0.12);
                let push_x = normal.x * depth * contact_strength * profile.rebound_scale
                    + input.vx * dt * contact_strength * 0.58 * profile.drag_scale;
                let push_y = normal.y * depth * contact_strength * profile.rebound_scale
                    + input.vy * dt * contact_strength * 0.58 * profile.drag_scale;
                bone.a.x += push_x * (1.0 - t);
                bone.a.y += push_y * (1.0 - t);
                bone.b.x += push_x * t;
                bone.b.y += push_y * t;
                apply_bone_torque(
                    &self.materials,
                    &mut self.debug,
                    &mut bone,
                    closest,
                    Vec2 {
                        x: normal.x * direct_load * 0.16 * profile.rebound_scale
                            + input.vx
                                * contact
                                * profile.bone_load_scale
                                * 0.22
                                * profile.drag_scale,
                        y: normal.y * direct_load * 0.16 * profile.rebound_scale
                            + input.vy
                                * contact
                                * profile.bone_load_scale
                                * 0.22
                                * profile.drag_scale,
                    },
                );
            }
            let should_fracture = self.can_fracture_bone(bone)
                && bone.load > bone.fracture_impulse * profile.fracture_scale;
            self.bones[i] = bone;
            if should_fracture {
                self.fracture_bone(i, t, normal, direct_load);
            }
        }

        for point in &mut self.points {
            if point.pinned {
                continue;
            }
            let point_contact = sample_point_contact(point.position, &shape);
            if point_contact.distance > influence {
                continue;
            }
            let mut contact_strength = (if input.down { 0.58 } else { 0.16 })
                * profile.tissue_push_scale
                * (0.85 + input.power * 0.15);
            if point.layer == TissueLayer::Muscle {
                contact_strength *= self.materials.direct_muscle_contact + point.exposure * 0.82;
            }
            let depth = influence - point_contact.distance;
            point.position.x +=
                point_contact.normal.x * depth * contact_strength * profile.rebound_scale
                    + input.vx * dt * 0.45 * contact_strength * profile.drag_scale;
            point.position.y +=
                point_contact.normal.y * depth * contact_strength * profile.rebound_scale
                    + input.vy * dt * 0.45 * contact_strength * profile.drag_scale;
            let contact_load =
                impact * (depth / influence) * contact_strength * profile.tissue_load_scale;
            point.load = point.load.max(contact_load);
            if apply_point_contusion(point, self.materials, contact_load, profile.contusion_scale) {
                self.stats.contusion_events += 1;
                self.debug.contusion_events += 1;
                self.debug.max_contusion = self.debug.max_contusion.max(point.contusion);
            }
            self.debug.tissue_contacts += 1;
            if depth > self.debug.max_depth {
                self.debug.max_depth = depth;
                self.debug.strongest_contact = point.position;
            }
            self.debug.max_point_load = self.debug.max_point_load.max(point.load);
        }

        self.lacerate_major_vessels_from_striker(input, profile, &shape, impact);
        self.penetrate_organs_from_striker(input, profile, &shape, impact);

        if input.down && profile.tear_pressure_scale > 0.0 {
            let mut events = Vec::new();
            for spring_index in 0..self.springs.len() {
                let spring = self.springs[spring_index];
                if spring.broken {
                    continue;
                }
                let a = self.points[spring.a];
                let b = self.points[spring.b];
                let midpoint = midpoint(a.position, b.position);
                let tear_contact = sample_point_contact(midpoint, &shape);
                if tear_contact.distance > influence * 0.82 {
                    continue;
                }
                let contact =
                    1.0 - (tear_contact.distance / (influence * 0.82).max(1.0)).clamp(0.0, 1.0);
                let layer_scale = if spring.layer == TissueLayer::Skin {
                    1.0
                } else {
                    0.78 + a.exposure.max(b.exposure) * 0.42
                };
                let pressure = impact * profile.tear_pressure_scale * contact * layer_scale;
                let threshold = spring.tear_impulse * self.materials.sharp_tool_tear_pressure;
                self.springs[spring_index].stress = self.springs[spring_index]
                    .stress
                    .max(pressure / threshold.max(1.0));
                if pressure <= threshold {
                    continue;
                }
                let tangent = normalized(subtract(b.position, a.position), Vec2 { x: 1.0, y: 0.0 });
                let mut cut_normal = shape.blade_normal;
                let spring_normal = normalized(
                    Vec2 {
                        x: -tangent.y,
                        y: tangent.x - 0.25,
                    },
                    Vec2 {
                        x: -tangent.y,
                        y: tangent.x,
                    },
                );
                if dot(cut_normal, spring_normal) < 0.0 {
                    cut_normal = scale(cut_normal, -1.0);
                }
                let normal = normalized(
                    Vec2 {
                        x: spring_normal.x * (1.0 - profile.blade_normal_bias)
                            + cut_normal.x * profile.blade_normal_bias,
                        y: spring_normal.y * (1.0 - profile.blade_normal_bias)
                            + cut_normal.y * profile.blade_normal_bias,
                    },
                    spring_normal,
                );
                self.springs[spring_index].broken = true;
                self.bump_point_exposure_load(
                    spring.a,
                    if spring.layer == TissueLayer::Skin {
                        0.92
                    } else {
                        1.0
                    },
                    pressure * 0.18,
                );
                self.bump_point_exposure_load(
                    spring.b,
                    if spring.layer == TissueLayer::Skin {
                        0.92
                    } else {
                        1.0
                    },
                    pressure * 0.18,
                );
                if spring.layer == TissueLayer::Skin {
                    self.stats.broken_skin += 1;
                } else {
                    self.stats.broken_muscle += 1;
                }
                events.push((
                    midpoint,
                    normal,
                    spring.layer,
                    pressure,
                    if spring.layer == TissueLayer::Skin {
                        6
                    } else {
                        4
                    },
                    if spring.layer == TissueLayer::Skin {
                        2.1
                    } else {
                        1.8
                    },
                    if spring.layer == TissueLayer::Skin {
                        0.58
                    } else {
                        0.92
                    },
                ));
            }
            for (midpoint, normal, layer, pressure, count, radius, depth) in events {
                self.emit_fluid(
                    midpoint,
                    normal,
                    count,
                    120.0 + pressure * self.materials.fluid_impact_scale * 0.42,
                    radius,
                    profile.fluid_scale,
                );
                self.open_wound(
                    midpoint,
                    normal,
                    layer,
                    pressure
                        / (if layer == TissueLayer::Skin {
                            1250.0
                        } else {
                            1050.0
                        }),
                    radius,
                    depth,
                );
            }
        }
    }

    fn lacerate_major_vessels_from_striker(
        &mut self,
        input: &InputState,
        profile: ToolProfile,
        shape: &ToolContactShape,
        impact: f64,
    ) {
        let budget = self.materials.max_vessel_lacerations_per_step;
        if !input.down || budget == 0 || self.vessels.is_empty() {
            return;
        }

        let mut events = Vec::new();
        for index in 0..self.vessels.len() {
            if events.len() >= budget {
                break;
            }
            let vessel = self.vessels[index];
            if vessel.lacerated {
                continue;
            }

            let (tool_point, vessel_point, closest_distance) = if shape.blade_segment {
                let closest =
                    closest_segment_points(shape.axis_start, shape.axis_end, vessel.a, vessel.b);
                (closest.point_a, closest.point_b, closest.distance)
            } else {
                let t = segment_t(shape.center, vessel.a, vessel.b);
                let vessel_point = lerp(vessel.a, vessel.b, t);
                (
                    shape.center,
                    vessel_point,
                    distance(shape.center, vessel_point),
                )
            };

            let reach = self.materials.major_vessel_cut_radius + vessel.radius;
            if closest_distance > reach {
                continue;
            }

            let contact = (1.0 - closest_distance / reach.max(EPSILON)).clamp(0.0, 1.0);
            let tool_scale = match input.tool {
                ToolMode::Sharp => 1.30 + profile.tear_pressure_scale * 0.18,
                ToolMode::Heavy => 0.54,
                ToolMode::Blunt => 0.18,
            };
            let threshold = match input.tool {
                ToolMode::Sharp => vessel.laceration_impulse,
                ToolMode::Heavy => {
                    vessel.laceration_impulse * self.materials.major_vessel_blunt_impulse_scale
                }
                ToolMode::Blunt => {
                    vessel.laceration_impulse
                        * self.materials.major_vessel_blunt_impulse_scale
                        * 1.35
                }
            };
            let drive = impact * contact * tool_scale;
            if drive <= threshold {
                continue;
            }

            let severity = ((drive - threshold) / threshold.max(EPSILON)).clamp(0.0, 2.2);
            let mut direction = normalized(subtract(vessel_point, tool_point), shape.blade_normal);
            if dot(direction, shape.direction) < -0.35 {
                direction = normalized(add(direction, scale(shape.direction, 0.35)), direction);
            }
            let wound_pressure = vessel.pressure
                * self.materials.major_vessel_pressure_scale
                * (1.0 + severity * 0.55);
            let wound_radius = 2.2 + vessel.radius * (0.42 + severity * 0.16);
            events.push((
                index,
                vessel_point,
                direction,
                wound_pressure,
                wound_radius,
                1.10 + severity * 0.12,
            ));
        }

        for (index, position, direction, pressure, radius, depth) in events {
            if self.open_wound(
                position,
                direction,
                TissueLayer::Muscle,
                pressure,
                radius,
                depth,
            ) {
                self.vessels[index].lacerated = true;
                self.stats.vessel_lacerations += 1;
                self.debug.vessel_lacerations += 1;
                self.debug.max_wound_pressure = self.debug.max_wound_pressure.max(pressure);
            }
        }
    }

    fn lacerate_vessels_from_fragment_tip(
        &mut self,
        bone: BoneSegment,
        tip: Vec2,
        previous_tip: Vec2,
        tip_normal: Vec2,
        impulse: f64,
        _radius: f64,
    ) {
        let budget = self.materials.max_fragment_vessel_lacerations_per_step;
        let threshold = self
            .materials
            .fragment_vessel_laceration_impulse
            .max(EPSILON);
        let severe_body_drive = self.debug.tool == ToolMode::Heavy
            || impulse >= threshold * 2.2
            || (bone.splinter && impulse >= threshold * 1.45);
        if budget == 0
            || self.vessels.is_empty()
            || (!bone.fractured && !bone.splinter)
            || (self.debug.fragment_vessel_lacerations as usize) >= budget
            || impulse < threshold * 0.45
            || !severe_body_drive
        {
            return;
        }

        let travel = distance(tip, previous_tip);
        let travel_dir = normalized(subtract(tip, previous_tip), tip_normal);
        let reach = self
            .materials
            .fragment_vessel_laceration_radius
            .max(bone.radius * if bone.splinter { 2.6 } else { 2.0 });
        let mut events = Vec::new();

        for index in 0..self.vessels.len() {
            if (self.debug.fragment_vessel_lacerations as usize) + events.len() >= budget {
                break;
            }
            let vessel = self.vessels[index];
            if vessel.lacerated {
                continue;
            }

            let closest = closest_segment_points(previous_tip, tip, vessel.a, vessel.b);
            let contact_reach = reach + vessel.radius;
            if closest.distance > contact_reach {
                continue;
            }

            let contact = (1.0 - closest.distance / contact_reach.max(EPSILON)).clamp(0.0, 1.0);
            if contact <= EPSILON {
                continue;
            }
            let toward_vessel = normalized(subtract(closest.point_b, closest.point_a), tip_normal);
            let alignment = if travel > EPSILON {
                dot(travel_dir, toward_vessel).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let drive = impulse
                * contact
                * (0.72 + alignment * 0.34)
                * if bone.splinter { 1.14 } else { 1.0 };
            if drive <= threshold {
                continue;
            }

            let severity = ((drive - threshold) / threshold).clamp(0.0, 2.4);
            let direction = normalized(
                add(scale(toward_vessel, 0.72), scale(tip_normal, 0.28)),
                toward_vessel,
            );
            let wound_pressure = vessel.pressure
                * self.materials.major_vessel_pressure_scale
                * (0.92 + severity * 0.52);
            let wound_radius = 2.0 + vessel.radius * (0.36 + severity * 0.18);
            events.push((
                index,
                closest.point_b,
                direction,
                wound_pressure,
                wound_radius,
                1.04 + severity * 0.14,
            ));
        }

        for (index, position, direction, pressure, radius, depth) in events {
            if self.open_wound(
                position,
                direction,
                TissueLayer::Muscle,
                pressure,
                radius,
                depth,
            ) {
                self.vessels[index].lacerated = true;
                self.stats.vessel_lacerations += 1;
                self.stats.fragment_vessel_lacerations += 1;
                self.debug.vessel_lacerations += 1;
                self.debug.fragment_vessel_lacerations += 1;
                self.debug.max_wound_pressure = self.debug.max_wound_pressure.max(pressure);
            }
        }
    }

    fn penetrate_organs_from_striker(
        &mut self,
        input: &InputState,
        profile: ToolProfile,
        shape: &ToolContactShape,
        impact: f64,
    ) {
        let budget = self.materials.max_organ_penetrations_per_step;
        if !input.down || input.tool != ToolMode::Sharp || budget == 0 || self.organs.is_empty() {
            return;
        }

        let mut penetrations = 0;
        let mut rupture_events = Vec::new();
        for index in 0..self.organs.len() {
            if penetrations >= budget {
                break;
            }
            let organ = self.organs[index];
            if organ.penetrated || organ.ruptured {
                continue;
            }
            let Some(contact) =
                tool_organ_contact(shape, organ, self.materials.organ_penetration_cut_radius)
            else {
                continue;
            };

            let drive =
                impact * contact.contact * (1.22 + profile.tear_pressure_scale.max(0.0) * 0.24);
            let threshold = self.materials.organ_penetration_impulse.max(EPSILON);
            if drive <= threshold {
                continue;
            }

            let severity = ((drive - threshold) / threshold).clamp(0.0, 2.2);
            let kind_scale = match organ.kind {
                OrganKind::LeftLung | OrganKind::RightLung => 0.92,
                OrganKind::Liver => 1.04,
                OrganKind::Spleen => 1.18,
            };
            let damage = self.materials.organ_penetration_damage.max(0.0)
                * (0.68 + severity * 0.58)
                * kind_scale;
            let new_damage = (self.organs[index].damage + damage).min(1.8);
            self.organs[index].damage = new_damage;
            self.organs[index].penetration_damage = self.organs[index]
                .penetration_damage
                .max((damage / self.materials.organ_rupture_damage.max(EPSILON)).clamp(0.0, 1.8));
            self.organs[index].load_damage = self.organs[index]
                .load_damage
                .max((drive / threshold).clamp(0.0, 2.8));
            self.organs[index].penetrated = true;
            self.stats.organ_damage_events += 1;
            self.debug.organ_damage_events += 1;
            self.stats.organ_penetrations += 1;
            self.debug.organ_penetrations += 1;
            self.debug.max_organ_damage = self.debug.max_organ_damage.max(new_damage);

            if new_damage >= self.materials.organ_rupture_damage
                && rupture_events.len() < self.materials.max_organ_ruptures_per_step
                && self.stats.organ_ruptures as usize + rupture_events.len()
                    < self.materials.max_total_organ_ruptures
            {
                self.organs[index].ruptured = true;
                let site = lerp(contact.tool_point, self.organs[index].center, 0.62);
                let mut direction =
                    normalized(subtract(site, contact.tool_point), shape.blade_normal);
                if dot(direction, shape.direction) < -0.25 {
                    direction = normalized(add(direction, scale(shape.direction, 0.30)), direction);
                }
                rupture_events.push((
                    site,
                    direction,
                    new_damage,
                    0.0,
                    drive * (0.42 + severity * 0.12),
                ));
            }

            penetrations += 1;
        }

        for (position, direction, damage, pressure, load) in rupture_events {
            self.open_organ_rupture(position, direction, damage, pressure, load);
        }
    }

    fn solve_springs(&mut self) {
        let mut events = Vec::new();
        for i in 0..self.springs.len() {
            let spring = self.springs[i];
            if spring.broken {
                continue;
            }
            let a = self.points[spring.a];
            let b = self.points[spring.b];
            let delta = subtract(b.position, a.position);
            let len = hypot(delta.x, delta.y);
            if len < EPSILON {
                continue;
            }
            let mut stretch_ratio = len / spring.rest.max(EPSILON);
            let endpoint_load = a.load.max(b.load);
            let contusion = a.contusion.max(b.contusion).clamp(0.0, 1.0);
            let fatigue_delta = tissue_fatigue_delta(
                self.materials,
                spring,
                stretch_ratio,
                endpoint_load,
                contusion,
            );
            let fatigue = if fatigue_delta > 0.0 {
                (self.springs[i].fatigue + fatigue_delta).min(1.35)
            } else {
                (self.springs[i].fatigue
                    * (1.0 - self.materials.tissue_fatigue_decay).clamp(0.94, 1.0))
                .max(0.0)
            };
            self.springs[i].fatigue = fatigue;
            if fatigue_delta > 0.0004 {
                self.stats.tissue_fatigue_events += 1;
                self.debug.tissue_fatigue_events += 1;
            }
            self.debug.max_tissue_fatigue = self.debug.max_tissue_fatigue.max(fatigue);

            if self.springs[i].rest_reference <= EPSILON {
                self.springs[i].rest_reference = spring.rest.max(EPSILON);
            }
            let plastic_delta = tissue_plastic_rest_delta(
                self.materials,
                spring,
                stretch_ratio,
                endpoint_load,
                contusion,
                fatigue,
            );
            if self.debug.impact <= EPSILON && plastic_delta.abs() > EPSILON {
                let reference = self.springs[i].rest_reference.max(EPSILON);
                let limit = self.materials.tissue_plastic_limit.clamp(0.0, 0.45);
                let min_rest = reference * (1.0 - limit * 0.62);
                let max_rest = reference * (1.0 + limit);
                let old_rest = self.springs[i].rest;
                let new_rest = (old_rest + plastic_delta).clamp(min_rest, max_rest);
                if (new_rest - old_rest).abs() > 0.0005 {
                    self.springs[i].rest = new_rest;
                    self.springs[i].plastic_strain =
                        ((new_rest - reference) / reference).abs().clamp(0.0, 1.0);
                    self.stats.tissue_plastic_events += 1;
                    self.debug.tissue_plastic_events += 1;
                    stretch_ratio = len / new_rest.max(EPSILON);
                }
            }
            self.debug.max_tissue_plasticity = self
                .debug
                .max_tissue_plasticity
                .max(self.springs[i].plastic_strain);

            let contusion_tear_weakening = tissue_tear_weakening(self.materials, contusion);
            let fatigue_tear_weakening = tissue_fatigue_tear_weakening(self.materials, fatigue);
            let tear_weakening =
                (contusion_tear_weakening + fatigue_tear_weakening).clamp(0.0, 0.58);
            let contusion_stiffness_softening =
                tissue_stiffness_softening(self.materials, contusion);
            let fatigue_stiffness_softening =
                tissue_fatigue_stiffness_softening(self.materials, fatigue);
            let stiffness_softening =
                (contusion_stiffness_softening + fatigue_stiffness_softening).clamp(0.0, 0.46);
            let tear_impulse = if spring.layer == TissueLayer::Muscle {
                spring.tear_impulse * (1.0 - a.exposure.max(b.exposure) * 0.48)
            } else {
                spring.tear_impulse
            } * (1.0 - tear_weakening).clamp(0.48, 1.0);
            let tear_stretch =
                spring.tear_stretch * (1.0 - contusion * 0.18 - fatigue * 0.10).clamp(0.76, 1.0);
            let load_tear_stretch = (1.12 - contusion * 0.055 - fatigue * 0.045).clamp(1.04, 1.12);
            let stiffness = spring.stiffness * (1.0 - stiffness_softening).clamp(0.52, 1.0);
            self.debug.max_tissue_softening = self
                .debug
                .max_tissue_softening
                .max(tear_weakening.max(stiffness_softening));
            self.springs[i].stress =
                (self.springs[i].stress * 0.9).max((stretch_ratio - 1.0).max(0.0));
            if stretch_ratio > tear_stretch
                || (endpoint_load > tear_impulse && stretch_ratio > load_tear_stretch)
            {
                let midpoint = midpoint(a.position, b.position);
                let tangent = normalized(delta, Vec2 { x: 1.0, y: 0.0 });
                let normal = Vec2 {
                    x: -tangent.y,
                    y: tangent.x - 0.35,
                };
                self.springs[i].broken = true;
                if spring.layer == TissueLayer::Skin {
                    self.stats.broken_skin += 1;
                    events.push((
                        midpoint,
                        normal,
                        TissueLayer::Skin,
                        endpoint_load,
                        5,
                        900.0,
                        110.0,
                        2.3,
                        1.12,
                        1350.0,
                        0.55,
                    ));
                } else {
                    self.stats.broken_muscle += 1;
                    if spring.fiber {
                        self.stats.muscle_fiber_tears += 1;
                        self.debug.muscle_fiber_tears += 1;
                    }
                    events.push((
                        midpoint,
                        normal,
                        TissueLayer::Muscle,
                        endpoint_load,
                        3,
                        1050.0,
                        85.0,
                        1.9,
                        0.86,
                        1250.0,
                        0.86,
                    ));
                }
                self.bump_point_exposure_load(spring.a, 1.0, endpoint_load * 0.35);
                self.bump_point_exposure_load(spring.b, 1.0, endpoint_load * 0.35);
                continue;
            }
            let inv_a = if a.pinned { 0.0 } else { 1.0 / a.mass };
            let inv_b = if b.pinned { 0.0 } else { 1.0 / b.mass };
            let weighted_gradient = inv_a + inv_b;
            if weighted_gradient <= EPSILON {
                continue;
            }
            let compliance = tissue_spring_compliance(self.materials, spring.layer);
            let alpha = xpbd_alpha(compliance, self.materials.fixed_dt);
            let constraint = len - self.springs[i].rest;
            let delta_lambda = (-constraint * stiffness - alpha * self.springs[i].lambda)
                / (weighted_gradient + alpha);
            self.springs[i].lambda += delta_lambda;
            let nx = delta.x / len;
            let ny = delta.y / len;
            let (point_a, point_b) = two_mut(&mut self.points, spring.a, spring.b);
            if !point_a.pinned {
                point_a.position.x -= nx * delta_lambda * inv_a;
                point_a.position.y -= ny * delta_lambda * inv_a;
            }
            if !point_b.pinned {
                point_b.position.x += nx * delta_lambda * inv_b;
                point_b.position.y += ny * delta_lambda * inv_b;
            }
        }
        for (
            midpoint,
            normal,
            layer,
            load,
            base_count,
            load_div,
            base_speed,
            radius,
            intensity,
            pressure_div,
            depth,
        ) in events
        {
            self.emit_fluid(
                midpoint,
                normal,
                base_count + (load / load_div).clamp(0.0, 8.0) as i32,
                base_speed + load * self.materials.fluid_impact_scale,
                radius,
                intensity,
            );
            self.open_wound(midpoint, normal, layer, load / pressure_div, radius, depth);
        }
    }

    fn propagate_skin_tears(&mut self) {
        let max_propagations = self.materials.max_tear_propagations_per_step;
        if max_propagations == 0
            || self.debug.tool != ToolMode::Sharp
            || self.debug.impact <= EPSILON
            || self.springs.is_empty()
            || self.points.is_empty()
        {
            return;
        }

        let mut broken_skin_endpoint = vec![false; self.points.len()];
        for spring in &self.springs {
            if spring.broken && spring.layer == TissueLayer::Skin {
                if spring.a < broken_skin_endpoint.len() {
                    broken_skin_endpoint[spring.a] = true;
                }
                if spring.b < broken_skin_endpoint.len() {
                    broken_skin_endpoint[spring.b] = true;
                }
            }
        }
        if !broken_skin_endpoint.iter().any(|broken| *broken) {
            return;
        }

        let mut candidates = Vec::new();
        for (index, spring) in self.springs.iter().enumerate() {
            if spring.broken
                || spring.layer != TissueLayer::Skin
                || spring.a >= self.points.len()
                || spring.b >= self.points.len()
            {
                continue;
            }
            if !broken_skin_endpoint[spring.a] && !broken_skin_endpoint[spring.b] {
                continue;
            }

            let a = self.points[spring.a];
            let b = self.points[spring.b];
            let endpoint_load = a.load.max(b.load);
            let exposure = a.exposure.max(b.exposure);
            let fatigue = spring.fatigue.clamp(0.0, 1.35);
            let stress = spring.stress.max(0.0);
            let load_ratio = endpoint_load / spring.tear_impulse.max(1.0);
            let score = stress + fatigue * 0.72 + exposure * 0.36 + load_ratio * 0.30;
            let stress_threshold = (self.materials.tear_propagation_stress_threshold
                - exposure * 0.08)
                .clamp(0.08, 1.0);
            if stress >= stress_threshold
                || fatigue >= self.materials.tear_propagation_fatigue_threshold
                || endpoint_load >= self.materials.tear_propagation_load_threshold
            {
                candidates.push((score, index));
            }
        }

        if candidates.is_empty() {
            return;
        }
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut events = Vec::new();
        for (_, index) in candidates.into_iter().take(max_propagations) {
            let spring = self.springs[index];
            if spring.broken || spring.a >= self.points.len() || spring.b >= self.points.len() {
                continue;
            }
            let a = self.points[spring.a];
            let b = self.points[spring.b];
            let delta = subtract(b.position, a.position);
            let len = hypot(delta.x, delta.y);
            if len < EPSILON {
                continue;
            }
            let endpoint_load = a.load.max(b.load).max(spring.tear_impulse * 0.36);
            let tangent = scale(delta, 1.0 / len);
            let normal = Vec2 {
                x: -tangent.y,
                y: tangent.x - 0.24,
            };
            self.springs[index].broken = true;
            self.springs[index].stress = 1.0;
            self.stats.broken_skin += 1;
            self.stats.tear_propagations += 1;
            self.debug.tear_propagations += 1;
            self.bump_point_exposure_load(spring.a, 1.0, endpoint_load * 0.22);
            self.bump_point_exposure_load(spring.b, 1.0, endpoint_load * 0.22);
            events.push((midpoint(a.position, b.position), normal, endpoint_load));
        }

        for (midpoint, normal, load) in events {
            self.emit_fluid(
                midpoint,
                normal,
                2 + (load / 1250.0).clamp(0.0, 4.0) as i32,
                70.0 + load * self.materials.fluid_impact_scale * 0.58,
                1.55,
                0.48,
            );
            self.open_wound(
                midpoint,
                normal,
                TissueLayer::Skin,
                load / 1650.0,
                1.55,
                0.42,
            );
        }
    }

    fn transfer_sharp_cut_to_exposed_muscle(&mut self) {
        let max_transfers = self.materials.max_muscle_cut_transfers_per_step;
        if max_transfers == 0
            || self.debug.tool != ToolMode::Sharp
            || self.debug.impact <= EPSILON
            || self.springs.is_empty()
            || self.points.is_empty()
        {
            return;
        }

        let mut skin_openings = Vec::new();
        for spring in &self.springs {
            if !spring.broken || spring.layer != TissueLayer::Skin {
                continue;
            }
            if spring.a >= self.points.len() || spring.b >= self.points.len() {
                continue;
            }
            let a = self.points[spring.a];
            let b = self.points[spring.b];
            let opening = midpoint(a.position, b.position);
            let direction = normalized(subtract(b.position, a.position), Vec2 { x: 1.0, y: 0.0 });
            let severity = a.exposure.max(b.exposure) + a.load.max(b.load) / 1600.0;
            skin_openings.push((opening, direction, severity));
        }
        if skin_openings.is_empty() {
            return;
        }

        let radius = self.materials.muscle_cut_transfer_radius.max(1.0);
        let mut candidates = Vec::new();
        for (index, spring) in self.springs.iter().enumerate() {
            if spring.broken
                || spring.layer != TissueLayer::Muscle
                || spring.a >= self.points.len()
                || spring.b >= self.points.len()
            {
                continue;
            }
            let a = self.points[spring.a];
            let b = self.points[spring.b];
            let midpoint = midpoint(a.position, b.position);
            let exposure = a.exposure.max(b.exposure);
            let endpoint_load = a.load.max(b.load);
            let fatigue = spring.fatigue.clamp(0.0, 1.35);
            if exposure < self.materials.muscle_cut_transfer_exposure_threshold
                && endpoint_load < self.materials.muscle_cut_transfer_load_threshold
                && fatigue < self.materials.tear_propagation_fatigue_threshold
            {
                continue;
            }

            let spring_dir = normalized(subtract(b.position, a.position), Vec2 { x: 1.0, y: 0.0 });
            let mut best_score = 0.0;
            let mut best_normal = Vec2 { x: 0.0, y: -1.0 };
            for (opening, opening_dir, severity) in &skin_openings {
                let d = distance(midpoint, *opening);
                if d > radius {
                    continue;
                }
                let proximity = 1.0 - d / radius;
                let alignment = cross(spring_dir, *opening_dir).abs().clamp(0.0, 1.0);
                let score = proximity
                    * (0.42 + alignment * 0.34)
                    * (0.70 + exposure * 0.26 + fatigue * 0.18 + endpoint_load / 3400.0)
                    * (0.78 + severity * 0.18);
                if score > best_score {
                    best_score = score;
                    best_normal = normalized(
                        Vec2 {
                            x: -opening_dir.y,
                            y: opening_dir.x - 0.28,
                        },
                        Vec2 { x: 0.0, y: -1.0 },
                    );
                }
            }
            if best_score > 0.18 {
                candidates.push((best_score, index, best_normal));
            }
        }

        if candidates.is_empty() {
            return;
        }
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut events = Vec::new();
        for (_, index, normal) in candidates.into_iter().take(max_transfers) {
            let spring = self.springs[index];
            if spring.broken || spring.a >= self.points.len() || spring.b >= self.points.len() {
                continue;
            }
            let a = self.points[spring.a];
            let b = self.points[spring.b];
            let load = a
                .load
                .max(b.load)
                .max(spring.tear_impulse * (0.34 + spring.fatigue.clamp(0.0, 1.0) * 0.16));
            self.springs[index].broken = true;
            self.springs[index].stress = 1.0;
            self.stats.broken_muscle += 1;
            self.stats.muscle_cut_transfers += 1;
            self.debug.muscle_cut_transfers += 1;
            self.bump_point_exposure_load(spring.a, 1.0, load * 0.24);
            self.bump_point_exposure_load(spring.b, 1.0, load * 0.24);
            events.push((midpoint(a.position, b.position), normal, load));
        }

        for (midpoint, normal, load) in events {
            self.emit_fluid(
                midpoint,
                normal,
                2 + (load / 1350.0).clamp(0.0, 4.0) as i32,
                82.0 + load * self.materials.fluid_impact_scale * 0.54,
                1.75,
                0.74,
            );
            self.open_wound(
                midpoint,
                normal,
                TissueLayer::Muscle,
                load / 1500.0,
                1.75,
                0.78,
            );
        }
    }

    fn delaminate_skin_flaps_from_cut_edges(&mut self) {
        let max_detachments = self.materials.max_skin_flap_detachments_per_step;
        if max_detachments == 0
            || self.debug.tool != ToolMode::Sharp
            || self.debug.impact <= EPSILON
            || self.attachments.is_empty()
            || self.springs.is_empty()
            || self.points.is_empty()
        {
            return;
        }

        let mut skin_openings = Vec::new();
        for spring in &self.springs {
            if !spring.broken
                || spring.layer != TissueLayer::Skin
                || spring.a >= self.points.len()
                || spring.b >= self.points.len()
            {
                continue;
            }
            let a = self.points[spring.a];
            let b = self.points[spring.b];
            skin_openings.push((
                midpoint(a.position, b.position),
                normalized(subtract(b.position, a.position), Vec2 { x: 1.0, y: 0.0 }),
                a.load.max(b.load),
                a.exposure.max(b.exposure),
            ));
        }
        if skin_openings.is_empty() {
            return;
        }

        let radius = self.materials.skin_flap_cut_radius.max(1.0);
        let load_threshold = self.materials.skin_flap_load_threshold.max(1.0);
        let stress_threshold = self.materials.skin_flap_stress_threshold.max(0.01);
        let mut candidates = Vec::new();
        for (index, attachment) in self.attachments.iter().enumerate() {
            if attachment.broken
                || attachment.skin_point >= self.points.len()
                || attachment.muscle_point >= self.points.len()
            {
                continue;
            }
            let skin = self.points[attachment.skin_point];
            let muscle = self.points[attachment.muscle_point];
            let mut best_score = 0.0;
            let mut best_direction = Vec2 { x: 0.0, y: -1.0 };
            let mut best_load = 0.0;
            let mut best_exposure = 0.0;
            for (opening, direction, opening_load, opening_exposure) in &skin_openings {
                let d = distance(skin.position, *opening);
                if d > radius {
                    continue;
                }
                let proximity = 1.0 - d / radius;
                let cut_load = opening_load * (0.35 + proximity * 0.65);
                let score = proximity * (0.55 + *opening_exposure * 0.30 + cut_load / 2600.0);
                if score > best_score {
                    best_score = score;
                    best_direction = *direction;
                    best_load = cut_load;
                    best_exposure = *opening_exposure * proximity;
                }
            }
            if best_score <= 0.0 {
                continue;
            }

            let attachment_distance = distance(skin.position, muscle.position);
            let attachment_stress = attachment
                .stress
                .max((attachment_distance / attachment.rest.max(1.0) - 1.0).max(0.0));
            let exposure = skin.exposure.max(muscle.exposure).max(best_exposure);
            let local_load = skin.load.max(muscle.load).max(best_load);
            let weakened_load_threshold = load_threshold * (1.0 - exposure * 0.24).clamp(0.66, 1.0);
            let weakened_stress_threshold =
                stress_threshold * (1.0 - exposure * 0.22).clamp(0.70, 1.0);
            if local_load < weakened_load_threshold && attachment_stress < weakened_stress_threshold
            {
                continue;
            }

            let score = best_score
                * (0.54
                    + local_load / load_threshold
                    + attachment_stress / stress_threshold * 0.72
                    + exposure * 0.26);
            candidates.push((score, index, best_direction, local_load));
        }

        if candidates.is_empty() {
            return;
        }
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut events = Vec::new();
        for (_, index, direction, load) in candidates.into_iter().take(max_detachments) {
            let attachment = self.attachments[index];
            if attachment.broken
                || attachment.skin_point >= self.points.len()
                || attachment.muscle_point >= self.points.len()
            {
                continue;
            }
            let skin = self.points[attachment.skin_point];
            let muscle = self.points[attachment.muscle_point];
            let normal = normalized(
                Vec2 {
                    x: -direction.y,
                    y: direction.x - 0.38,
                },
                normalized(
                    subtract(skin.position, muscle.position),
                    Vec2 { x: 0.0, y: -1.0 },
                ),
            );
            self.attachments[index].broken = true;
            self.attachments[index].stress = 1.0;
            self.stats.broken_attachments += 1;
            self.stats.skin_flap_detachments += 1;
            self.debug.skin_flap_detachments += 1;
            self.bump_point_exposure_load(attachment.skin_point, 1.0, load * 0.18);
            self.bump_point_exposure_load(attachment.muscle_point, 1.0, load * 0.24);
            events.push((midpoint(skin.position, muscle.position), normal, load));
        }

        for (point, normal, load) in events {
            self.emit_fluid(
                point,
                normal,
                1 + (load / 1450.0).clamp(0.0, 3.0) as i32,
                68.0 + load * self.materials.fluid_impact_scale * 0.44,
                1.5,
                0.62,
            );
            self.open_wound(point, normal, TissueLayer::Muscle, load / 1750.0, 1.5, 0.62);
        }
    }

    fn solve_attachments(&mut self) {
        let mut events = Vec::new();
        for i in 0..self.attachments.len() {
            let attachment = self.attachments[i];
            if attachment.broken {
                continue;
            }
            let skin = self.points[attachment.skin_point];
            let muscle = self.points[attachment.muscle_point];
            let delta = subtract(muscle.position, skin.position);
            let len = hypot(delta.x, delta.y);
            if len < EPSILON {
                continue;
            }
            let stretch_ratio = len / attachment.rest.max(1.0);
            let impulse = skin.load.max(muscle.load);
            self.attachments[i].stress =
                (self.attachments[i].stress * 0.88).max((stretch_ratio - 1.0).max(0.0));
            if stretch_ratio > self.materials.attachment_break_stretch
                || (impulse > self.materials.attachment_break_impulse && stretch_ratio > 1.25)
            {
                self.attachments[i].broken = true;
                self.stats.broken_attachments += 1;
                self.bump_point_exposure_load(attachment.skin_point, 1.0, impulse * 0.2);
                self.bump_point_exposure_load(attachment.muscle_point, 1.0, impulse * 0.2);
                events.push((
                    midpoint(skin.position, muscle.position),
                    Vec2 {
                        x: skin.position.x - muscle.position.x,
                        y: skin.position.y - muscle.position.y - 0.4,
                    },
                    impulse,
                ));
                continue;
            }
            let diff = (len - attachment.rest) / len;
            let correction = scale(delta, diff * self.materials.attachment_stiffness);
            let skin_share = if skin.pinned {
                0.0
            } else {
                0.72 / skin.mass.max(0.25)
            };
            let muscle_share = if muscle.pinned {
                0.0
            } else {
                0.28 / muscle.mass.max(0.25)
            };
            let share_sum = skin_share + muscle_share;
            if share_sum <= EPSILON {
                continue;
            }
            let skin_amount = skin_share / share_sum;
            let muscle_amount = muscle_share / share_sum;
            if !self.points[attachment.skin_point].pinned {
                self.points[attachment.skin_point].position = add(
                    self.points[attachment.skin_point].position,
                    scale(correction, skin_amount),
                );
                self.points[attachment.skin_point].previous = add(
                    self.points[attachment.skin_point].previous,
                    scale(correction, skin_amount * 0.18),
                );
            }
            if !self.points[attachment.muscle_point].pinned {
                self.points[attachment.muscle_point].position = subtract(
                    self.points[attachment.muscle_point].position,
                    scale(correction, muscle_amount),
                );
                self.points[attachment.muscle_point].previous = subtract(
                    self.points[attachment.muscle_point].previous,
                    scale(correction, muscle_amount * 0.10),
                );
            }
        }
        for (point, normal, impulse) in events {
            self.emit_fluid(
                point,
                normal,
                3 + (impulse / 1200.0).clamp(0.0, 5.0) as i32,
                80.0 + impulse * self.materials.fluid_impact_scale * 0.60,
                1.8,
                0.78,
            );
            self.open_wound(
                point,
                normal,
                TissueLayer::Muscle,
                impulse / 1550.0,
                1.8,
                0.74,
            );
        }
    }

    fn solve_bone_attachments(&mut self) {
        let mut events = Vec::new();
        for i in 0..self.bone_attachments.len() {
            let attachment = self.bone_attachments[i];
            if attachment.broken
                || attachment.point >= self.points.len()
                || attachment.bone >= self.bones.len()
            {
                continue;
            }
            let point = self.points[attachment.point];
            let mut bone = self.bones[attachment.bone];
            bone.load = bone
                .load
                .max(point.load * self.materials.bone_impact_transfer);
            let raw_anchor = bone_point(bone, attachment.t);
            let current_distance = distance(point.position, raw_anchor);
            let stretch_ratio = current_distance / attachment.rest.max(1.0);
            let impulse = point.load.max(bone.load);
            self.bone_attachments[i].stress =
                (self.bone_attachments[i].stress * 0.9).max((stretch_ratio - 1.0).max(0.0));
            if stretch_ratio > self.materials.bone_attachment_break_stretch
                || (impulse > self.materials.bone_attachment_break_impulse && stretch_ratio > 1.45)
            {
                self.bone_attachments[i].broken = true;
                self.stats.broken_bone_attachments += 1;
                self.bump_point_exposure_load(attachment.point, 0.85, impulse * 0.2);
                events.push((
                    midpoint(point.position, raw_anchor),
                    Vec2 {
                        x: point.position.x - raw_anchor.x,
                        y: point.position.y - raw_anchor.y - 0.35,
                    },
                    impulse,
                ));
                self.bones[attachment.bone] = bone;
                continue;
            }
            let target = add(raw_anchor, attachment.offset);
            let correction = scale(
                subtract(target, point.position),
                self.materials.bone_attachment_stiffness,
            );
            if !self.points[attachment.point].pinned {
                self.points[attachment.point].position =
                    add(self.points[attachment.point].position, correction);
            }
            if !bone.pinned {
                let bone_share = 0.10;
                bone.a.x -= correction.x * bone_share * (1.0 - attachment.t);
                bone.a.y -= correction.y * bone_share * (1.0 - attachment.t);
                bone.b.x -= correction.x * bone_share * attachment.t;
                bone.b.y -= correction.y * bone_share * attachment.t;
            }
            self.bones[attachment.bone] = bone;
        }
        for (point, normal, impulse) in events {
            self.emit_fluid(
                point,
                normal,
                4 + (impulse / 1050.0).clamp(0.0, 7.0) as i32,
                95.0 + impulse * self.materials.fluid_impact_scale * 0.72,
                2.0,
                0.94,
            );
            self.open_wound(
                point,
                normal,
                TissueLayer::Muscle,
                impulse / 1250.0,
                2.0,
                0.98,
            );
        }
    }

    fn solve_bone_joints(&mut self) {
        let count = self.bone_joints.len();
        let mut ligament_events = Vec::new();
        for i in 0..count {
            let mut joint = self.bone_joints[i];
            if joint.broken
                || joint.post_fracture_limited
                || joint.a >= self.bones.len()
                || joint.b >= self.bones.len()
            {
                continue;
            }
            let a = self.bones[joint.a];
            let b = self.bones[joint.b];
            let anchor_a = bone_point(a, joint.t_a);
            let anchor_b = bone_point(b, joint.t_b);
            let delta = subtract(anchor_b, anchor_a);
            let len = hypot(delta.x, delta.y);
            if len < EPSILON {
                continue;
            }
            let stretch_ratio = len / joint.rest.max(1.0);
            let impulse = a.load.max(b.load);
            joint.stress = (joint.stress * 0.9).max((stretch_ratio - 1.0).max(0.0));
            let relative_angle = wrap_angle(bone_angle(b) - bone_angle(a) - joint.rest_angle);
            let clamped_angle = relative_angle.clamp(joint.min_angle, joint.max_angle);
            let angle_violation = relative_angle - clamped_angle;
            let overextension = angle_violation.abs();
            joint.torque_stress = (joint.torque_stress * 0.9).max(overextension);
            if stretch_ratio > self.materials.bone_joint_break_stretch
                || (impulse > self.materials.bone_joint_break_impulse && stretch_ratio > 1.35)
            {
                joint.broken = true;
                self.stats.broken_bone_joints += 1;
                self.bone_joints[i] = joint;
                continue;
            }
            if overextension > self.materials.bone_joint_angular_break
                || (impulse > self.materials.bone_joint_break_impulse
                    && overextension > self.materials.bone_joint_angular_break * 0.45)
            {
                joint.broken = true;
                self.stats.broken_bone_joints += 1;
                self.bone_joints[i] = joint;
                continue;
            }
            let stretch_subluxation = ((stretch_ratio
                - self.materials.bone_joint_subluxation_stretch)
                / (self.materials.bone_joint_break_stretch
                    - self.materials.bone_joint_subluxation_stretch)
                    .max(0.12))
            .clamp(0.0, 1.4);
            let impulse_subluxation = if impulse > self.materials.bone_joint_subluxation_impulse
                && stretch_ratio > 1.12
            {
                ((impulse - self.materials.bone_joint_subluxation_impulse)
                    / self.materials.bone_joint_subluxation_impulse.max(1.0))
                .clamp(0.0, 1.2)
                    * (stretch_ratio - 1.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let angular_subluxation = ((overextension
                - self.materials.bone_joint_subluxation_angular)
                / (self.materials.bone_joint_angular_break
                    - self.materials.bone_joint_subluxation_angular)
                    .max(0.12))
            .clamp(0.0, 1.4);
            let subluxation_drive = stretch_subluxation
                .max(impulse_subluxation)
                .max(angular_subluxation);
            let loaded_subluxation = impulse > self.materials.bone_joint_subluxation_impulse * 0.35
                || stretch_ratio
                    > (self.materials.bone_joint_subluxation_stretch
                        + self.materials.bone_joint_break_stretch)
                        * 0.50
                || overextension
                    > (self.materials.bone_joint_subluxation_angular
                        + self.materials.bone_joint_angular_break)
                        * 0.50;
            if subluxation_drive > 0.020 && loaded_subluxation {
                if !joint.subluxated {
                    self.stats.bone_joint_subluxations += 1;
                    self.debug.bone_joint_subluxations += 1;
                    let center = midpoint(anchor_a, anchor_b);
                    let event_load = impulse
                        .max(self.materials.bone_joint_subluxation_impulse * 0.65)
                        * (0.55 + subluxation_drive.clamp(0.0, 1.0) * 0.65);
                    ligament_events.push((center, event_load));
                }
                joint.subluxated = true;
                joint.subluxation = joint
                    .subluxation
                    .max((0.22 + subluxation_drive * 0.68).clamp(0.0, 1.0));
            }
            self.debug.max_bone_joint_subluxation =
                self.debug.max_bone_joint_subluxation.max(joint.subluxation);
            self.bones[joint.a].load = self.bones[joint.a]
                .load
                .max(self.bones[joint.b].load * 0.30);
            self.bones[joint.b].load = self.bones[joint.b]
                .load
                .max(self.bones[joint.a].load * 0.30);
            let subluxation_slack = self.materials.bone_joint_subluxation_slack * joint.subluxation;
            let effective_rest = joint.rest + subluxation_slack;
            let diff = if joint.subluxated && len < effective_rest {
                0.0
            } else {
                (len - effective_rest) / len
            };
            let stiffness_scale = if joint.subluxated {
                self.materials.bone_joint_subluxation_stiffness_scale
            } else {
                1.0
            };
            let correction = scale(
                delta,
                diff * self.materials.bone_joint_stiffness * 0.5 * stiffness_scale,
            );
            self.apply_bone_anchor_delta_idx(joint.a, joint.t_a, correction.x, correction.y);
            self.apply_bone_anchor_delta_idx(joint.b, joint.t_b, -correction.x, -correction.y);
            let angle_slack =
                self.materials.bone_joint_subluxation_angular * joint.subluxation * 0.55;
            let relaxed_angle = relative_angle
                - relative_angle
                    .clamp(joint.min_angle - angle_slack, joint.max_angle + angle_slack);
            let angle_correction =
                relaxed_angle * self.materials.bone_joint_angular_stiffness * 0.5 * stiffness_scale;
            self.rotate_bone_around_anchor_idx(joint.a, joint.t_a, angle_correction);
            self.rotate_bone_around_anchor_idx(joint.b, joint.t_b, -angle_correction);
            self.bone_joints[i] = joint;
        }
        for (center, load) in ligament_events {
            self.stats.joint_ligament_damage_events += 1;
            self.debug.joint_ligament_damage_events += 1;
            self.apply_joint_ligament_damage(center, load);
        }
    }

    fn solve_bones(&mut self) {
        let initial_count = self.bones.len();
        let mut fractures = Vec::new();
        for i in 0..initial_count {
            let mut bone = self.bones[i];
            if bone.pinned {
                bone.a = bone.home_a;
                bone.b = bone.home_b;
                bone.previous_a = bone.a;
                bone.previous_b = bone.b;
                self.bones[i] = bone;
                continue;
            }
            if bone.sleeping && free_bone_fragment(bone) {
                bone.previous_a = bone.a;
                bone.previous_b = bone.b;
                self.bones[i] = bone;
                continue;
            }
            let delta = subtract(bone.b, bone.a);
            let len = hypot(delta.x, delta.y);
            if len < EPSILON {
                self.bones[i] = bone;
                continue;
            }
            let diff = (len - bone.rest_length) / len;
            let correction = scale(delta, diff * 0.5);
            bone.a = add(bone.a, correction);
            bone.b = subtract(bone.b, correction);
            let should_fracture = self.can_fracture_bone(bone) && bone.load > bone.fracture_impulse;
            self.bones[i] = bone;
            if should_fracture {
                fractures.push(i);
            }
        }
        for i in fractures {
            self.fracture_bone(
                i,
                0.5,
                Vec2::default(),
                self.bones.get(i).map(|b| b.load).unwrap_or(0.0),
            );
        }
    }

    fn solve_post_fracture_joints(&mut self) {
        for i in 0..self.bone_joints.len() {
            let mut joint = self.bone_joints[i];
            if (!joint.broken && !joint.post_fracture_limited)
                || joint.a >= self.bones.len()
                || joint.b >= self.bones.len()
            {
                continue;
            }
            let a = self.bones[joint.a];
            let b = self.bones[joint.b];
            if a.splinter || b.splinter || (a.pinned && b.pinned) {
                continue;
            }
            let inv_mass_a = if a.pinned {
                0.0
            } else {
                1.0 / (a.rest_length * a.radius * a.radius).max(1.0)
            };
            let inv_mass_b = if b.pinned {
                0.0
            } else {
                1.0 / (b.rest_length * b.radius * b.radius).max(1.0)
            };
            let inv_mass_sum = inv_mass_a + inv_mass_b;
            if inv_mass_sum <= EPSILON {
                continue;
            }
            let share_a = inv_mass_a / inv_mass_sum;
            let share_b = inv_mass_b / inv_mass_sum;
            let rest = if joint.post_fracture_rest > 0.0 {
                joint.post_fracture_rest
            } else {
                joint.rest
            }
            .max(1.0);
            let rest_angle = if joint.post_fracture_rest > 0.0 {
                joint.post_fracture_rest_angle
            } else {
                joint.rest_angle
            };
            let anchor_a = bone_point(a, joint.t_a);
            let anchor_b = bone_point(b, joint.t_b);
            let delta = subtract(anchor_b, anchor_a);
            let len = hypot(delta.x, delta.y);
            let mut corrected = false;
            if len > EPSILON {
                let max_len = (rest + self.materials.post_fracture_joint_slack)
                    .max(rest * self.materials.post_fracture_joint_max_stretch);
                let stretch_ratio = len / rest;
                self.debug.max_post_fracture_joint_stretch = self
                    .debug
                    .max_post_fracture_joint_stretch
                    .max(stretch_ratio);
                joint.stress = (joint.stress * 0.94).max((stretch_ratio - 1.0).max(0.0));
                if len > max_len {
                    let correction = scale(
                        delta,
                        (len - max_len) / len * self.materials.post_fracture_joint_stiffness,
                    );
                    self.apply_bone_anchor_delta_idx(
                        joint.a,
                        joint.t_a,
                        correction.x * share_a,
                        correction.y * share_a,
                    );
                    self.apply_bone_anchor_delta_idx(
                        joint.b,
                        joint.t_b,
                        -correction.x * share_b,
                        -correction.y * share_b,
                    );
                    corrected = true;
                }
            }
            let relative_angle = wrap_angle(
                bone_angle(self.bones[joint.b]) - bone_angle(self.bones[joint.a]) - rest_angle,
            );
            let min_angle = joint.min_angle - self.materials.post_fracture_joint_angle_slack;
            let max_angle = joint.max_angle + self.materials.post_fracture_joint_angle_slack;
            let clamped_angle = relative_angle.clamp(min_angle, max_angle);
            let angle_violation = relative_angle - clamped_angle;
            let overextension = angle_violation.abs();
            self.debug.max_post_fracture_joint_angle = self
                .debug
                .max_post_fracture_joint_angle
                .max(relative_angle.abs());
            joint.torque_stress = (joint.torque_stress * 0.94).max(overextension);
            if overextension > EPSILON {
                let correction =
                    angle_violation * self.materials.post_fracture_joint_angular_stiffness;
                self.rotate_bone_around_anchor_idx(joint.a, joint.t_a, correction * share_a);
                self.rotate_bone_around_anchor_idx(joint.b, joint.t_b, -correction * share_b);
                self.bones[joint.a].angular_velocity *= 1.0 - 0.08 * share_a;
                self.bones[joint.b].angular_velocity *= 1.0 - 0.08 * share_b;
                corrected = true;
            }
            if corrected {
                self.debug.post_fracture_joint_corrections += 1;
            }
            self.bone_joints[i] = joint;
        }
    }

    fn solve_bone_fragment_tissue_contacts(&mut self) {
        let fragment_indices = self.budgeted_fragment_indices();
        let point_radius_base =
            (self.materials.point_spacing * FRAGMENT_TISSUE_POINT_RADIUS_SCALE).max(4.0);
        let cell_size = self.fragment_tissue_cell_size();
        let point_grid = self.build_point_spatial_grid(cell_size);

        for bone_index in fragment_indices {
            let mut bone = self.bones[bone_index];
            if !free_bone_fragment(bone) {
                continue;
            }

            let mut contact_count = 0;
            let mut strongest_depth: f64 = 0.0;
            let query_radius = bone.radius
                + point_radius_base * 1.12
                + if bone.splinter {
                    bone.radius * 0.22
                } else {
                    bone.radius * 0.08
                }
                + 1.0;
            let candidate_points = self.point_candidates_near_aabb(
                &point_grid,
                segment_aabb(bone.a, bone.b, query_radius),
                cell_size,
            );
            for point_index in candidate_points {
                if !self.consume_fragment_tissue_check() {
                    continue;
                }
                let point = self.points[point_index];
                if point.pinned {
                    continue;
                }

                let point_radius = point_radius_base
                    * if point.layer == TissueLayer::Skin {
                        0.92
                    } else {
                        1.12
                    };
                let target_distance = bone.radius
                    + point_radius
                    + if bone.splinter {
                        bone.radius * 0.22
                    } else {
                        bone.radius * 0.08
                    };
                let t = segment_t(point.position, bone.a, bone.b);
                let anchor = bone_point(bone, t);
                let delta = subtract(point.position, anchor);
                let distance_to_tissue = hypot(delta.x, delta.y);
                if distance_to_tissue >= target_distance {
                    continue;
                }

                let depth = target_distance - distance_to_tissue;
                let contact = (depth / target_distance.max(1.0)).clamp(0.0, 1.0);
                let normal = normalized(
                    delta,
                    fragment_tissue_fallback_normal(bone, point.position, t),
                );
                let layer_resistance = if point.layer == TissueLayer::Skin {
                    1.34 - point.exposure.min(1.0) * 0.22
                } else {
                    0.92 + point.exposure.min(1.0) * 0.20
                };
                let sharp_slip = if bone.splinter { 0.78 } else { 1.0 };
                let correction = depth
                    * self.materials.fragment_repulsion_stiffness.max(0.1)
                    * FRAGMENT_TISSUE_RESISTANCE
                    * layer_resistance
                    * sharp_slip;
                let mut bone_share = if point.layer == TissueLayer::Skin {
                    0.58
                } else {
                    0.43
                };
                if bone.splinter {
                    bone_share *= 0.86;
                }
                let point_share = 1.0 - bone_share;

                let previous_anchor = lerp(bone.previous_a, bone.previous_b, t);
                let bone_velocity = subtract(anchor, previous_anchor);
                let point_velocity = subtract(point.position, point.previous);
                let relative_velocity = subtract(bone_velocity, point_velocity);
                let tangent_velocity = subtract(
                    relative_velocity,
                    scale(normal, dot(relative_velocity, normal)),
                );
                let tissue_drag = clamp_magnitude(
                    tangent_velocity,
                    self.materials.point_spacing * 0.13 * contact,
                );

                let point_position = add(
                    add(point.position, scale(normal, correction * point_share)),
                    scale(
                        tissue_drag,
                        if point.layer == TissueLayer::Skin {
                            0.10
                        } else {
                            0.22
                        },
                    ),
                );
                self.points[point_index].position = point_position;
                self.points[point_index].previous = add(
                    point.previous,
                    scale(
                        tissue_drag,
                        if point.layer == TissueLayer::Skin {
                            0.04
                        } else {
                            0.10
                        },
                    ),
                );
                self.points[point_index].load = self.points[point_index].load.max(
                    depth * 68.0 * layer_resistance
                        + contact * self.materials.fragment_damage_impulse * 0.30,
                );
                if point.layer == TissueLayer::Muscle {
                    self.points[point_index].exposure = self.points[point_index].exposure.max(0.92);
                } else if contact > 0.42 || bone.splinter {
                    self.points[point_index].exposure =
                        self.points[point_index].exposure.max(0.48 + contact * 0.34);
                }

                apply_bone_anchor_delta(
                    &mut bone,
                    t,
                    -normal.x * correction * bone_share,
                    -normal.y * correction * bone_share,
                );
                damp_bone_velocity_against_contact(
                    &mut bone,
                    normal,
                    (contact * FRAGMENT_TISSUE_NORMAL_DAMPING * layer_resistance).min(0.86),
                    (contact * FRAGMENT_TISSUE_TANGENTIAL_FRICTION * layer_resistance).min(0.62),
                );
                bone.angular_velocity *= (1.0
                    - contact * FRAGMENT_TISSUE_ANGULAR_FRICTION * layer_resistance)
                    .clamp(0.58, 1.0);
                bone.load = bone.load.max(self.points[point_index].load * 0.36);

                contact_count += 1;
                strongest_depth = strongest_depth.max(depth);
                self.debug.max_point_load =
                    self.debug.max_point_load.max(self.points[point_index].load);
                self.debug.max_fragment_depth = self.debug.max_fragment_depth.max(depth);
            }

            if contact_count > 0 {
                self.debug.fragment_contacts += contact_count.min(6);
                self.debug.max_fragment_depth = self.debug.max_fragment_depth.max(strongest_depth);
                self.debug.max_bone_angular_speed = self
                    .debug
                    .max_bone_angular_speed
                    .max(bone.angular_velocity.abs());
            }
            self.bones[bone_index] = bone;
        }
    }

    fn solve_bone_fragment_bone_contacts(&mut self) {
        let fragment_indices = self.budgeted_fragment_indices();
        let cell_size = self.fragment_bone_cell_size();
        let intact_grid = self.build_intact_bone_spatial_grid(cell_size);
        for fragment_index in fragment_indices {
            let fragment = self.bones[fragment_index];
            if !free_bone_fragment(fragment) {
                continue;
            }
            let candidates = self.fragment_candidates_near_aabb(
                &intact_grid,
                segment_aabb(
                    fragment.a,
                    fragment.b,
                    fragment.radius + self.materials.fragment_repulsion_slop + 1.0,
                ),
                cell_size,
            );
            for bone_index in candidates {
                if bone_index == fragment_index {
                    continue;
                }
                let support = self.bones[bone_index];
                if free_bone_fragment(support) {
                    continue;
                }
                if !self.consume_fragment_bone_check() {
                    continue;
                }

                let closest = closest_segment_points(fragment.a, fragment.b, support.a, support.b);
                let target_distance =
                    fragment.radius + support.radius + self.materials.fragment_repulsion_slop;
                let normal = normalized(
                    subtract(closest.point_a, closest.point_b),
                    normalized(
                        subtract(
                            midpoint(fragment.a, fragment.b),
                            midpoint(support.a, support.b),
                        ),
                        Vec2 { x: 1.0, y: 0.0 },
                    ),
                );
                let velocity_fragment = bone_anchor_velocity(fragment, closest.t_a);
                let velocity_support = bone_anchor_velocity(support, closest.t_b);
                let relative_velocity = subtract(velocity_fragment, velocity_support);
                let normal_speed = dot(relative_velocity, normal);
                let tangent_velocity = subtract(relative_velocity, scale(normal, normal_speed));
                let dt = self.materials.fixed_dt.max(EPSILON);
                let closing_speed = (-normal_speed).max(0.0) / dt;
                let tangential_speed = hypot(tangent_velocity.x, tangent_velocity.y) / dt;
                let rest_speed = self.materials.fragment_bone_rest_speed.max(1.0);
                let resting_contact =
                    closing_speed < rest_speed && tangential_speed < rest_speed * 1.75;
                let penetration = target_distance - closest.distance;
                let contact_skin = self
                    .materials
                    .fragment_repulsion_slop
                    .mul_add(0.85, 0.25)
                    .clamp(0.25, 1.0);
                let use_resting_contact_skin = self.debug.impact <= EPSILON;
                if penetration <= 0.0
                    && (!use_resting_contact_skin
                        || !resting_contact
                        || closest.distance >= target_distance + contact_skin)
                {
                    continue;
                }
                let rest_factor = if resting_contact {
                    (1.0 - closing_speed / rest_speed).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let overlap = if penetration > 0.0 {
                    penetration
                } else {
                    ((target_distance + contact_skin - closest.distance) * 0.10).max(0.0)
                };
                let fragment_mass =
                    bone_contact_mass(fragment, if fragment.splinter { 0.35 } else { 1.0 });
                let support_mass = bone_contact_mass(support, 2.8);
                let inv_fragment = 1.0 / fragment_mass;
                let inv_support = if support.pinned {
                    0.0
                } else {
                    1.0 / support_mass
                };
                let inv_sum = inv_fragment + inv_support;
                if inv_sum <= EPSILON {
                    continue;
                }

                let correction = overlap
                    * (self.materials.fragment_repulsion_stiffness * 0.92
                        + self.materials.fragment_bone_rest_stiffness * rest_factor);
                let fragment_share = inv_fragment / inv_sum;
                let support_share = inv_support / inv_sum;
                self.apply_bone_anchor_delta_idx(
                    fragment_index,
                    closest.t_a,
                    normal.x * correction * fragment_share,
                    normal.y * correction * fragment_share,
                );
                self.apply_bone_anchor_delta_idx(
                    bone_index,
                    closest.t_b,
                    -normal.x * correction * support_share,
                    -normal.y * correction * support_share,
                );

                let contact = (overlap / target_distance.max(1.0)).clamp(0.0, 1.0);
                let normal_damping = (contact
                    * (self.materials.fragment_bone_normal_damping
                        + self.materials.fragment_bone_rest_friction * rest_factor * 0.62))
                    .min(0.78);
                let tangential_friction = (contact
                    * (self.materials.fragment_bone_tangential_friction
                        + self.materials.fragment_bone_rest_friction * rest_factor))
                    .min(0.54);
                if normal_damping > EPSILON || tangential_friction > EPSILON {
                    if let Some(bone) = self.bones.get_mut(fragment_index) {
                        damp_bone_velocity_against_contact(
                            bone,
                            scale(normal, -1.0),
                            normal_damping,
                            tangential_friction,
                        );
                        bone.angular_velocity *= (1.0
                            - contact
                                * (self.materials.fragment_bone_angular_friction
                                    + self.materials.fragment_bone_rest_friction
                                        * rest_factor
                                        * 0.30))
                            .clamp(0.76, 1.0);
                    }
                    if !support.pinned {
                        if let Some(bone) = self.bones.get_mut(bone_index) {
                            damp_bone_velocity_against_contact(
                                bone,
                                normal,
                                normal_damping * 0.45,
                                tangential_friction * 0.45,
                            );
                        }
                    }
                    self.debug.fragment_bone_damping_events += 1;
                }
                let contact_load = overlap * 84.0 + closing_speed * fragment.radius.max(1.0) * 0.24;

                self.bones[fragment_index].load =
                    self.bones[fragment_index].load.max(contact_load * 0.72);
                self.bones[bone_index].load = self.bones[bone_index].load.max(contact_load * 0.54);
                self.debug.fragment_bone_contacts += 1;
                if resting_contact {
                    self.debug.fragment_bone_resting_contacts += 1;
                }
                self.debug.max_fragment_overlap = self.debug.max_fragment_overlap.max(overlap);
                self.debug.max_bone_load = self.debug.max_bone_load.max(
                    self.bones[fragment_index]
                        .load
                        .max(self.bones[bone_index].load),
                );
            }
        }
    }

    fn solve_bone_fragment_repulsion(&mut self) {
        let fragment_indices = self.budgeted_fragment_indices();
        let cell_size = self.fragment_pair_cell_size();
        let fragment_grid = self.build_fragment_spatial_grid(&fragment_indices, cell_size);
        let mut seen_pairs = HashSet::new();
        for i in fragment_indices.iter().copied() {
            let a = self.bones[i];
            let candidates = self.fragment_candidates_near_aabb(
                &fragment_grid,
                segment_aabb(
                    a.a,
                    a.b,
                    a.radius + self.materials.fragment_repulsion_slop + 1.0,
                ),
                cell_size,
            );
            for j in candidates {
                if i == j {
                    continue;
                }
                let pair = if i < j { (i, j) } else { (j, i) };
                if !seen_pairs.insert(pair) {
                    continue;
                }
                if !self.consume_fragment_pair_check() {
                    continue;
                }
                let b = self.bones[j];
                let closest = closest_segment_points(a.a, a.b, b.a, b.b);
                let target_distance = a.radius + b.radius + self.materials.fragment_repulsion_slop;
                if closest.distance >= target_distance {
                    continue;
                }
                let normal = normalized(
                    subtract(closest.point_a, closest.point_b),
                    normalized(
                        subtract(midpoint(a.a, a.b), midpoint(b.a, b.b)),
                        normalized(
                            Vec2 {
                                x: -(a.b.y - a.a.y),
                                y: a.b.x - a.a.x,
                            },
                            Vec2 { x: 1.0, y: 0.0 },
                        ),
                    ),
                );
                let overlap = target_distance - closest.distance;
                let mass_a =
                    (a.rest_length * a.radius * a.radius * (if a.splinter { 0.35 } else { 1.0 }))
                        .max(1.0);
                let mass_b =
                    (b.rest_length * b.radius * b.radius * (if b.splinter { 0.35 } else { 1.0 }))
                        .max(1.0);
                let inv_mass_a = 1.0 / mass_a;
                let inv_mass_b = 1.0 / mass_b;
                let inv_sum = inv_mass_a + inv_mass_b;
                if inv_sum <= EPSILON {
                    continue;
                }
                let velocity_a = bone_anchor_velocity(a, closest.t_a);
                let velocity_b = bone_anchor_velocity(b, closest.t_b);
                let relative_velocity = subtract(velocity_a, velocity_b);
                let normal_speed = dot(relative_velocity, normal);
                let tangent_velocity = subtract(relative_velocity, scale(normal, normal_speed));
                let dt = self.materials.fixed_dt.max(EPSILON);
                let closing_speed = (-normal_speed).max(0.0) / dt;
                let tangential_speed = hypot(tangent_velocity.x, tangent_velocity.y) / dt;
                let rest_speed = self.materials.fragment_pair_rest_speed.max(1.0);
                let resting_contact =
                    closing_speed < rest_speed && tangential_speed < rest_speed * 1.65;
                let rest_factor = if resting_contact {
                    (1.0 - closing_speed / rest_speed).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let correction = overlap
                    * (self.materials.fragment_repulsion_stiffness
                        + self.materials.fragment_pair_rest_stiffness * rest_factor);
                let share_a = inv_mass_a / inv_sum;
                let share_b = inv_mass_b / inv_sum;
                self.apply_bone_anchor_delta_idx(
                    i,
                    closest.t_a,
                    normal.x * correction * share_a,
                    normal.y * correction * share_a,
                );
                self.apply_bone_anchor_delta_idx(
                    j,
                    closest.t_b,
                    -normal.x * correction * share_b,
                    -normal.y * correction * share_b,
                );
                let contact = (overlap / target_distance.max(1.0)).clamp(0.0, 1.0);
                let normal_damping = (contact
                    * (self.materials.fragment_pair_normal_damping
                        + self.materials.fragment_pair_rest_friction * rest_factor * 0.65))
                    .min(0.82);
                let tangential_friction = (contact
                    * (self.materials.fragment_pair_tangential_friction
                        + self.materials.fragment_pair_rest_friction * rest_factor))
                    .min(0.62);
                if normal_damping > EPSILON || tangential_friction > EPSILON {
                    if let Some(bone) = self.bones.get_mut(i) {
                        damp_bone_velocity_against_contact(
                            bone,
                            scale(normal, -1.0),
                            normal_damping,
                            tangential_friction,
                        );
                        bone.angular_velocity *= (1.0
                            - contact
                                * (self.materials.fragment_pair_angular_friction
                                    + self.materials.fragment_pair_rest_friction
                                        * rest_factor
                                        * 0.35))
                            .clamp(0.72, 1.0);
                    }
                    if let Some(bone) = self.bones.get_mut(j) {
                        damp_bone_velocity_against_contact(
                            bone,
                            normal,
                            normal_damping,
                            tangential_friction,
                        );
                        bone.angular_velocity *= (1.0
                            - contact
                                * (self.materials.fragment_pair_angular_friction
                                    + self.materials.fragment_pair_rest_friction
                                        * rest_factor
                                        * 0.35))
                            .clamp(0.72, 1.0);
                    }
                    self.debug.fragment_pair_damping_events += 1;
                }
                if resting_contact {
                    self.debug.fragment_pair_resting_contacts += 1;
                }
                self.debug.fragment_pair_contacts += 1;
                self.debug.max_fragment_overlap = self.debug.max_fragment_overlap.max(overlap);
            }
        }
    }

    fn collide_bone_fragments(&mut self) {
        let fragment_indices = self.budgeted_fragment_indices();
        for i in fragment_indices {
            let bone = self.bones[i];
            if bone.pinned || (!bone.fractured && !bone.splinter) {
                continue;
            }
            if bone.broken_start || bone.splinter {
                self.process_fragment_tip(
                    i,
                    bone.a,
                    bone.previous_a,
                    bone.broken_start_normal,
                    bone.broken_start,
                );
            }
            let bone = self.bones[i];
            if bone.broken_end || bone.splinter {
                self.process_fragment_tip(
                    i,
                    bone.b,
                    bone.previous_b,
                    bone.broken_end_normal,
                    bone.broken_end,
                );
            }
        }
    }

    fn process_fragment_tip(
        &mut self,
        bone_index: usize,
        tip: Vec2,
        previous_tip: Vec2,
        normal: Vec2,
        strong_tip: bool,
    ) {
        if bone_index >= self.bones.len() {
            return;
        }
        let bone = self.bones[bone_index];
        let travel = distance(tip, previous_tip);
        let speed = travel / self.materials.fixed_dt.max(EPSILON);
        let radius = self
            .materials
            .fragment_contact_radius
            .max(bone.radius * (if strong_tip { 1.75 } else { 1.25 }));
        let impulse =
            bone.load * 0.22 + speed * bone.radius * (if bone.splinter { 0.75 } else { 1.0 });
        if impulse < self.materials.fragment_damage_impulse * 0.34 {
            return;
        }
        self.debug.max_fragment_impulse = self.debug.max_fragment_impulse.max(impulse);
        let tip_normal = normalized(normal, Vec2 { x: 0.0, y: -1.0 });
        if bone.kind == BoneKind::Rib {
            self.puncture_organs_from_rib_tip(bone, tip, previous_tip, tip_normal, impulse, radius);
        }
        self.lacerate_vessels_from_fragment_tip(
            bone,
            tip,
            previous_tip,
            tip_normal,
            impulse,
            radius,
        );
        let mut torque_impulse = Vec2::default();
        let mut torque_weight = 0.0;
        let mut emissions = Vec::new();

        for point in &mut self.points {
            if point.pinned {
                continue;
            }
            let delta = subtract(point.position, tip);
            let d = hypot(delta.x, delta.y);
            if d > radius || d <= EPSILON {
                continue;
            }
            let deep_tissue = point.layer == TissueLayer::Muscle
                || point.exposure > 0.20
                || impulse > self.materials.fragment_damage_impulse * 1.35;
            if !deep_tissue {
                continue;
            }
            let contact = 1.0 - d / radius;
            self.debug.max_fragment_depth = self.debug.max_fragment_depth.max(radius - d);
            let away = normalized(delta, tip_normal);
            let push =
                contact * self.materials.fragment_push * (if bone.splinter { 1.25 } else { 1.0 });
            point.position.x += (away.x * radius * 0.42 + tip_normal.x * radius * 0.18) * push;
            point.position.y += (away.y * radius * 0.42 + tip_normal.y * radius * 0.18) * push;
            point.load = point.load.max(impulse * contact * 0.46);
            point.exposure = point.exposure.max(if point.layer == TissueLayer::Muscle {
                1.0
            } else {
                0.86
            });
            self.stats.fragment_tissue_hits += 1;
            self.debug.fragment_contacts += 1;
            self.debug.max_point_load = self.debug.max_point_load.max(point.load);
            torque_impulse = add(
                torque_impulse,
                Vec2 {
                    x: -away.x * impulse * contact * 0.06 - tip_normal.x * impulse * contact * 0.02,
                    y: -away.y * impulse * contact * 0.06 - tip_normal.y * impulse * contact * 0.02,
                },
            );
            torque_weight += 1.0;
            if contact > 0.62 || impulse > self.materials.fragment_damage_impulse {
                emissions.push((
                    point.position,
                    Vec2 {
                        x: away.x + tip_normal.x * 0.45,
                        y: away.y + tip_normal.y * 0.45 - 0.15,
                    },
                    point.layer,
                    impulse,
                    if point.layer == TissueLayer::Muscle {
                        1.7
                    } else {
                        1.9
                    },
                    if point.layer == TissueLayer::Muscle {
                        0.72
                    } else {
                        0.56
                    },
                ));
            }
        }

        let mut spring_events = Vec::new();
        for i in 0..self.springs.len() {
            let spring = self.springs[i];
            if spring.broken {
                continue;
            }
            let a = self.points[spring.a];
            let b = self.points[spring.b];
            let mid = midpoint(a.position, b.position);
            let d = distance(mid, tip);
            if d > radius * 1.18 {
                continue;
            }
            let contact = 1.0 - d / (radius * 1.18);
            if spring.layer == TissueLayer::Skin
                && (self.debug.fragment_skin_punctures as usize)
                    < self.materials.max_fragment_skin_punctures_per_step
            {
                let to_skin = normalized(subtract(mid, tip), tip_normal);
                let travel_dir = normalized(subtract(tip, previous_tip), tip_normal);
                let tip_alignment = dot(tip_normal, to_skin).clamp(0.0, 1.0);
                let travel_alignment = dot(travel_dir, to_skin).clamp(0.0, 1.0);
                let exposed = a.exposure.max(b.exposure).clamp(0.0, 1.0);
                let puncture_drive = impulse
                    * contact
                    * (0.58 + tip_alignment * 0.24 + travel_alignment * 0.28)
                    * if bone.splinter { 1.14 } else { 1.0 };
                let puncture_threshold =
                    self.materials.fragment_skin_puncture_impulse * (1.0 - exposed * 0.18);
                if puncture_drive > puncture_threshold {
                    self.springs[i].broken = true;
                    self.springs[i].stress = 1.0;
                    self.bump_point_exposure_load(spring.a, 1.0, puncture_drive * 0.42);
                    self.bump_point_exposure_load(spring.b, 1.0, puncture_drive * 0.42);
                    self.stats.broken_skin += 1;
                    self.stats.fragment_tissue_tears += 1;
                    self.stats.fragment_skin_punctures += 1;
                    self.debug.fragment_tears += 1;
                    self.debug.fragment_skin_punctures += 1;
                    spring_events.push((mid, TissueLayer::Skin, puncture_drive));
                    continue;
                }
            }
            let reachable = spring.layer == TissueLayer::Muscle
                || a.exposure.max(b.exposure) > 0.35
                || impulse > self.materials.fragment_damage_impulse * 1.75;
            if !reachable {
                continue;
            }
            let threshold = spring.tear_impulse
                * (if spring.layer == TissueLayer::Muscle {
                    0.46
                } else {
                    0.72
                });
            self.springs[i].stress = self.springs[i]
                .stress
                .max(impulse * contact / threshold.max(1.0));
            if impulse * contact <= threshold {
                continue;
            }
            self.springs[i].broken = true;
            self.bump_point_exposure_load(spring.a, 1.0, impulse * contact * 0.35);
            self.bump_point_exposure_load(spring.b, 1.0, impulse * contact * 0.35);
            if spring.layer == TissueLayer::Skin {
                self.stats.broken_skin += 1;
            } else {
                self.stats.broken_muscle += 1;
            }
            self.stats.fragment_tissue_tears += 1;
            self.debug.fragment_tears += 1;
            spring_events.push((mid, spring.layer, impulse));
        }

        for i in 0..self.bone_attachments.len() {
            let attachment = self.bone_attachments[i];
            if attachment.broken || attachment.point >= self.points.len() {
                continue;
            }
            let point = self.points[attachment.point];
            let d = distance(point.position, tip);
            if d > radius * 1.12 || impulse < self.materials.bone_attachment_break_impulse * 0.38 {
                continue;
            }
            self.bone_attachments[i].broken = true;
            self.bone_attachments[i].stress = self.bone_attachments[i]
                .stress
                .max(impulse / self.materials.bone_attachment_break_impulse.max(1.0));
            self.bump_point_exposure_load(attachment.point, 0.95, impulse * 0.32);
            self.stats.broken_bone_attachments += 1;
            self.stats.fragment_tissue_tears += 1;
            self.debug.fragment_tears += 1;
            spring_events.push((point.position, TissueLayer::Muscle, impulse));
        }

        for i in 0..self.triangles.len() {
            let triangle = self.triangles[i];
            if triangle.failed || triangle.layer != TissueLayer::Muscle {
                continue;
            }
            let centroid = Vec2 {
                x: (self.points[triangle.a].position.x
                    + self.points[triangle.b].position.x
                    + self.points[triangle.c].position.x)
                    / 3.0,
                y: (self.points[triangle.a].position.y
                    + self.points[triangle.b].position.y
                    + self.points[triangle.c].position.y)
                    / 3.0,
            };
            let d = distance(centroid, tip);
            if d > radius * 1.28 {
                continue;
            }
            let contact = 1.0 - d / (radius * 1.28);
            self.triangles[i].damage =
                (self.triangles[i].damage + contact * impulse / 1800.0).min(1.35);
            if self.triangles[i].damage > 1.0 {
                self.triangles[i].failed = true;
                self.bump_point_exposure_load(triangle.a, 1.0, impulse * 0.12);
                self.bump_point_exposure_load(triangle.b, 1.0, impulse * 0.12);
                self.bump_point_exposure_load(triangle.c, 1.0, impulse * 0.12);
                self.stats.fragment_tissue_tears += 1;
                self.debug.fragment_tears += 1;
            }
        }

        if torque_weight > 0.0 {
            let mut bone = self.bones[bone_index];
            apply_bone_torque(
                &self.materials,
                &mut self.debug,
                &mut bone,
                tip,
                scale(torque_impulse, 1.0 / torque_weight),
            );
            self.bones[bone_index] = bone;
        }

        for (position, direction, layer, impulse, radius, depth) in emissions {
            self.emit_fluid(
                position,
                direction,
                1 + (impulse / 1200.0).clamp(0.0, 4.0) as i32,
                80.0 + impulse * self.materials.fluid_impact_scale * 0.28,
                radius,
                if layer == TissueLayer::Muscle {
                    0.72
                } else {
                    0.92
                },
            );
            self.open_wound(
                position,
                direction,
                layer,
                impulse
                    / (if layer == TissueLayer::Muscle {
                        1500.0
                    } else {
                        1750.0
                    }),
                radius,
                depth,
            );
        }
        for (mid, layer, impulse) in spring_events {
            let direction = Vec2 {
                x: mid.x - tip.x + tip_normal.x * 0.5,
                y: mid.y - tip.y + tip_normal.y * 0.5 - 0.18,
            };
            self.emit_fluid(
                mid,
                direction,
                if layer == TissueLayer::Skin { 4 } else { 3 },
                115.0 + impulse * self.materials.fluid_impact_scale * 0.36,
                if layer == TissueLayer::Skin { 2.1 } else { 1.8 },
                if layer == TissueLayer::Skin {
                    1.02
                } else {
                    0.82
                },
            );
            self.open_wound(
                mid,
                direction,
                layer,
                impulse
                    / (if layer == TissueLayer::Skin {
                        1450.0
                    } else {
                        1250.0
                    }),
                if layer == TissueLayer::Skin { 2.1 } else { 1.8 },
                if layer == TissueLayer::Skin {
                    0.62
                } else {
                    0.95
                },
            );
        }
    }

    fn solve_areas(&mut self) {
        let blood_turgor = self.blood_turgor_scale_internal();
        for index in 0..self.areas.len() {
            let area = self.areas[index];
            if self.live_edge_count(area.edge_ab, area.edge_bc, area.edge_ca) < 2 {
                continue;
            }
            let pa = self.points[area.a];
            let pb = self.points[area.b];
            let pc = self.points[area.c];
            let current = signed_area(pa.position, pb.position, pc.position);
            let constraint = current - area.rest_area;
            let inv_a = if pa.pinned { 0.0 } else { 1.0 / pa.mass };
            let inv_b = if pb.pinned { 0.0 } else { 1.0 / pb.mass };
            let inv_c = if pc.pinned { 0.0 } else { 1.0 / pc.mass };
            let ax = pb.position.y - pc.position.y;
            let ay = pc.position.x - pb.position.x;
            let bx = pc.position.y - pa.position.y;
            let by = pa.position.x - pc.position.x;
            let cx = pa.position.y - pb.position.y;
            let cy = pb.position.x - pa.position.x;
            let weighted_gradient = inv_a * (ax * ax + ay * ay)
                + inv_b * (bx * bx + by * by)
                + inv_c * (cx * cx + cy * cy);
            if weighted_gradient <= EPSILON {
                continue;
            }
            let compliance = tissue_area_compliance(self.materials, area.layer);
            let alpha = xpbd_alpha(compliance, self.materials.fixed_dt);
            let lambda = (-constraint * area.stiffness * blood_turgor
                - alpha * self.areas[index].lambda)
                / (weighted_gradient + alpha);
            self.areas[index].lambda += lambda;
            if !self.points[area.a].pinned {
                self.points[area.a].position.x += ax * lambda * inv_a;
                self.points[area.a].position.y += ay * lambda * inv_a;
            }
            if !self.points[area.b].pinned {
                self.points[area.b].position.x += bx * lambda * inv_b;
                self.points[area.b].position.y += by * lambda * inv_b;
            }
            if !self.points[area.c].pinned {
                self.points[area.c].position.x += cx * lambda * inv_c;
                self.points[area.c].position.y += cy * lambda * inv_c;
            }
        }
    }

    fn constrain_to_world(&mut self, width: f64, floor_y: f64) {
        let margin = 8.0;
        for point in &mut self.points {
            if point.pinned {
                continue;
            }
            point.position.x = point.position.x.clamp(margin, width - margin);
            point.position.y = point.position.y.min(floor_y);
        }
        for index in 0..self.bones.len() {
            let mut bone = self.bones[index];
            if bone.pinned {
                continue;
            }
            bone.a.x = bone.a.x.clamp(margin, width - margin);
            bone.b.x = bone.b.x.clamp(margin, width - margin);
            if free_bone_fragment(bone) {
                let contact_floor_y = floor_y - bone.radius.max(1.0);
                let (a_contact, a_resting) = constrain_fragment_endpoint_to_floor(
                    self.materials,
                    &mut bone.a,
                    &mut bone.previous_a,
                    contact_floor_y,
                );
                let (b_contact, b_resting) = constrain_fragment_endpoint_to_floor(
                    self.materials,
                    &mut bone.b,
                    &mut bone.previous_b,
                    contact_floor_y,
                );
                let contacts = usize::from(a_contact) + usize::from(b_contact);
                if contacts > 0 {
                    let resting = usize::from(a_resting) + usize::from(b_resting);
                    self.debug.fragment_floor_contacts += contacts as i32;
                    self.debug.fragment_floor_resting_contacts += resting as i32;
                    let angular_friction =
                        (self.materials.fragment_floor_angular_friction * contacts as f64 * 0.5)
                            .clamp(0.0, 0.48);
                    bone.angular_velocity *= 1.0 - angular_friction;
                    self.debug.max_bone_angular_speed = self
                        .debug
                        .max_bone_angular_speed
                        .max(bone.angular_velocity.abs());
                }
            } else {
                bone.a.y = bone.a.y.min(floor_y);
                bone.b.y = bone.b.y.min(floor_y);
            }
            self.bones[index] = bone;
        }
    }

    fn update_exposure(&mut self) {
        for attachment in self.attachments.clone() {
            if attachment.broken {
                self.bump_point_exposure_load(attachment.skin_point, 1.0, 0.0);
                self.bump_point_exposure_load(attachment.muscle_point, 1.0, 0.0);
            }
        }
        for spring in self.springs.clone() {
            if spring.broken && spring.layer == TissueLayer::Skin {
                self.bump_point_exposure_load(spring.a, 0.85, 0.0);
                self.bump_point_exposure_load(spring.b, 0.85, 0.0);
            }
        }
    }

    fn update_triangle_damage(&mut self) {
        let mut rupture_events = Vec::new();
        for i in 0..self.triangles.len() {
            let triangle = self.triangles[i];
            if triangle.layer != TissueLayer::Muscle || triangle.failed {
                continue;
            }
            let a = self.points[triangle.a];
            let b = self.points[triangle.b];
            let c = self.points[triangle.c];
            let load = (a.load + b.load + c.load) / 3.0;
            let exposed = (a.exposure + b.exposure + c.exposure) / 3.0;
            let edge_fatigue = spring_fatigue(&self.springs, triangle.edge_ab)
                .max(spring_fatigue(&self.springs, triangle.edge_bc))
                .max(spring_fatigue(&self.springs, triangle.edge_ca));
            let fiber_rupture_floor = if muscle_fiber_spring_broken(&self.springs, triangle.edge_ab)
                || muscle_fiber_spring_broken(&self.springs, triangle.edge_bc)
                || muscle_fiber_spring_broken(&self.springs, triangle.edge_ca)
            {
                self.materials.muscle_fiber_rupture_damage_floor
            } else {
                0.0
            };
            let impulse_threshold =
                self.materials.muscle_exposed_tear_impulse + (1.0 - exposed) * 560.0;
            let load_damage = (load - impulse_threshold).max(0.0) / 1500.0;
            let fatigue_floor = edge_fatigue * 0.38;
            let damage = (triangle.damage * 0.996 + load_damage)
                .max(fatigue_floor)
                .max(fiber_rupture_floor)
                .min(1.35);
            self.triangles[i].damage = damage;
            if damage > 1.0 {
                self.triangles[i].failed = true;
                let rupture_drive = load
                    .max(edge_fatigue * self.materials.tissue_fatigue_load_threshold * 1.25)
                    .max(damage * self.materials.muscle_crush_rupture_load_threshold);
                let should_bleed = rupture_events.len()
                    < self.materials.max_muscle_crush_ruptures_per_step
                    && rupture_drive >= self.materials.muscle_crush_rupture_load_threshold
                    && damage >= self.materials.muscle_crush_rupture_damage_threshold;
                if should_bleed {
                    let centroid = Vec2 {
                        x: (a.position.x + b.position.x + c.position.x) / 3.0,
                        y: (a.position.y + b.position.y + c.position.y) / 3.0,
                    };
                    let ab = subtract(b.position, a.position);
                    let ac = subtract(c.position, a.position);
                    let raw_normal = normalized(
                        Vec2 {
                            x: -(ab.y + ac.y * 0.45),
                            y: ab.x + ac.x * 0.45 - 0.35,
                        },
                        Vec2 { x: 0.0, y: -1.0 },
                    );
                    rupture_events.push((centroid, raw_normal, rupture_drive, damage));
                }
            }
        }

        for (position, normal, load, damage) in rupture_events {
            self.emit_fluid(
                position,
                normal,
                2 + (load / 1200.0).clamp(0.0, 5.0) as i32,
                84.0 + load * self.materials.fluid_impact_scale * 0.36,
                1.85 + damage.min(1.35) * 0.35,
                0.76 + damage.min(1.35) * 0.18,
            );
            self.open_wound(
                position,
                normal,
                TissueLayer::Muscle,
                load / 1450.0,
                1.95,
                0.88,
            );
            self.stats.muscle_crush_ruptures += 1;
            self.debug.muscle_crush_ruptures += 1;
        }
    }

    fn fracture_bone(
        &mut self,
        bone_index: usize,
        fracture_t: f64,
        impulse_normal: Vec2,
        impulse: f64,
    ) {
        if bone_index >= self.bones.len() {
            return;
        }
        let candidate = self.bones[bone_index];
        if !self.can_fracture_bone_shape(candidate) {
            return;
        }
        if !self.fracture_budget_allows(candidate) {
            self.debug.fracture_budget_blocks += 1;
            return;
        }
        let old = self.bones[bone_index];
        let delta = subtract(old.b, old.a);
        let len = hypot(delta.x, delta.y).max(EPSILON);
        let material_min_piece = if old.kind == BoneKind::Rib {
            self.materials.min_rib_fragment_length
        } else {
            self.materials.min_bone_fragment_length
        };
        let minimum_piece_length = material_min_piece.max(old.radius * 3.2);
        let min_break_t = (minimum_piece_length / len).clamp(0.18, 0.46);
        if min_break_t >= 0.5 {
            return;
        }
        let dir = scale(delta, 1.0 / len);
        let base_normal = Vec2 {
            x: -dir.y,
            y: dir.x,
        };
        let contact_normal = normalized(impulse_normal, base_normal);
        let normal = normalized(
            Vec2 {
                x: base_normal.x * 0.35 + contact_normal.x * 0.65,
                y: base_normal.y * 0.35 + contact_normal.y * 0.65,
            },
            base_normal,
        );
        let break_t = fracture_t.clamp(min_break_t, 1.0 - min_break_t);
        let crack = lerp(old.a, old.b, break_t);
        let previous_crack = lerp(old.previous_a, old.previous_b, break_t);
        let home_crack = lerp(old.home_a, old.home_b, break_t);
        let overload = ((impulse.max(old.load) - old.fracture_impulse)
            / old.fracture_impulse.max(1.0))
        .clamp(0.0, 1.4);
        let gap = (5.0_f64.max(old.radius * (0.75 + overload * 0.25))).min(len * 0.10);
        let snap = (5.5_f64.max(old.radius * (1.05 + overload * 0.42))).min(len * 0.09);
        let shear = (2.5_f64.max(old.radius * 0.42)).min(len * 0.035);
        let recoil = snap * (2.1 + overload * 0.8);
        let left_cap = Vec2 {
            x: crack.x - dir.x * gap * 0.5 - normal.x * snap - dir.x * shear,
            y: crack.y - dir.y * gap * 0.5 - normal.y * snap - dir.y * shear,
        };
        let right_cap = Vec2 {
            x: crack.x + dir.x * gap * 0.5 + normal.x * snap + dir.x * shear,
            y: crack.y + dir.y * gap * 0.5 + normal.y * snap + dir.y * shear,
        };
        let spin_sign = if cross(dir, normal) >= 0.0 { 1.0 } else { -1.0 };
        let fracture_spin = ((impulse.max(old.load) / old.fracture_impulse.max(1.0))
            * self.materials.fracture_spin_scale
            * (0.55 + (break_t - 0.5).abs() * 1.9))
            .clamp(0.0, 16.0);

        let secondary_fracture_impulse = old.fracture_impulse
            * self
                .materials
                .secondary_bone_fracture_impulse_scale
                .clamp(0.45, 1.10);

        let mut first = old;
        first.a = Vec2 {
            x: old.a.x - normal.x * snap * 0.18,
            y: old.a.y - normal.y * snap * 0.18,
        };
        first.b = left_cap;
        first.previous_a = Vec2 {
            x: old.previous_a.x + normal.x * recoil * 0.20,
            y: old.previous_a.y + normal.y * recoil * 0.20,
        };
        first.previous_b = Vec2 {
            x: previous_crack.x + normal.x * recoil + dir.x * shear * 0.7,
            y: previous_crack.y + normal.y * recoil + dir.y * shear * 0.7,
        };
        first.home_b = Vec2 {
            x: home_crack.x - dir.x * gap * 0.5 - normal.x * snap * 0.35,
            y: home_crack.y - dir.y * gap * 0.5 - normal.y * snap * 0.35,
        };
        first.rest_length = distance(first.a, first.b).max(EPSILON);
        first.fractured = true;
        first.broken_end = true;
        first.broken_end_normal = normal;
        first.fracture_generation += 1;
        first.fracture_impulse = secondary_fracture_impulse;
        first.load = old.load * 0.28;
        first.angular_velocity = (first.angular_velocity
            + spin_sign * fracture_spin * (1.0 - break_t))
            .clamp(-36.0, 36.0);
        self.bones[bone_index] = first;

        let mut second = BoneSegment {
            kind: old.kind,
            a: right_cap,
            b: Vec2 {
                x: old.b.x + normal.x * snap * 0.18,
                y: old.b.y + normal.y * snap * 0.18,
            },
            previous_a: Vec2 {
                x: previous_crack.x - normal.x * recoil - dir.x * shear * 0.7,
                y: previous_crack.y - normal.y * recoil - dir.y * shear * 0.7,
            },
            previous_b: Vec2 {
                x: old.previous_b.x - normal.x * recoil * 0.20,
                y: old.previous_b.y - normal.y * recoil * 0.20,
            },
            home_a: Vec2 {
                x: home_crack.x + dir.x * gap * 0.5 + normal.x * snap * 0.35,
                y: home_crack.y + dir.y * gap * 0.5 + normal.y * snap * 0.35,
            },
            home_b: old.home_b,
            radius: old.radius,
            rest_length: 1.0,
            fracture_impulse: secondary_fracture_impulse,
            load: old.load * 0.28,
            fractured: true,
            broken_start: true,
            broken_start_normal: normal,
            fracture_generation: first.fracture_generation,
            pinned: old.pinned,
            ..BoneSegment::default()
        };
        second.rest_length = distance(second.a, second.b).max(EPSILON);
        second.angular_velocity = (first.angular_velocity
            - spin_sign * fracture_spin * (0.65 + break_t))
            .clamp(-36.0, 36.0);
        let second_index = self.bones.len();
        self.bones.push(second);

        for attachment in &mut self.bone_attachments {
            if attachment.bone != bone_index || attachment.broken {
                continue;
            }
            let original_t = attachment.t;
            let detach_zone =
                (0.11 + overload * 0.05 + old.radius / len.max(1.0) * 1.5).clamp(0.10, 0.22);
            if (original_t - break_t).abs() <= detach_zone {
                attachment.broken = true;
                attachment.stress = 1.0 + overload;
                if attachment.point < self.points.len() {
                    self.points[attachment.point].exposure =
                        self.points[attachment.point].exposure.max(0.95);
                    self.points[attachment.point].load =
                        self.points[attachment.point].load.max(old.load * 0.35);
                }
                self.stats.broken_bone_attachments += 1;
                continue;
            }
            if original_t < break_t {
                attachment.t = (original_t / break_t.max(0.05)).clamp(0.0, 1.0);
                if attachment.point < self.points.len() {
                    let anchor = bone_point(self.bones[bone_index], attachment.t);
                    attachment.offset = subtract(self.points[attachment.point].position, anchor);
                    attachment.rest =
                        distance(self.points[attachment.point].position, anchor).max(1.0);
                }
            } else {
                attachment.bone = second_index;
                attachment.t = ((original_t - break_t) / (1.0 - break_t).max(0.05)).clamp(0.0, 1.0);
                if attachment.point < self.points.len() {
                    let anchor = bone_point(self.bones[second_index], attachment.t);
                    attachment.offset = subtract(self.points[attachment.point].position, anchor);
                    attachment.rest =
                        distance(self.points[attachment.point].position, anchor).max(1.0);
                }
            }
        }

        for joint in &mut self.bone_joints {
            if joint.broken {
                continue;
            }
            let mut affected = false;
            remap_joint_end(
                &mut joint.a,
                &mut joint.t_a,
                bone_index,
                second_index,
                break_t,
                old.radius,
                len,
                overload,
                &mut joint.broken,
                &mut affected,
            );
            remap_joint_end(
                &mut joint.b,
                &mut joint.t_b,
                bone_index,
                second_index,
                break_t,
                old.radius,
                len,
                overload,
                &mut joint.broken,
                &mut affected,
            );
            if joint.broken {
                joint.stress = 1.0 + overload;
                self.stats.broken_bone_joints += 1;
            }
            if affected && joint.a < self.bones.len() && joint.b < self.bones.len() {
                let remapped_rest = distance(
                    bone_point(self.bones[joint.a], joint.t_a),
                    bone_point(self.bones[joint.b], joint.t_b),
                )
                .max(1.0);
                let remapped_rest_angle =
                    wrap_angle(bone_angle(self.bones[joint.b]) - bone_angle(self.bones[joint.a]));
                joint.post_fracture_limited = true;
                joint.post_fracture_rest = remapped_rest;
                joint.post_fracture_rest_angle = remapped_rest_angle;
                joint.torque_stress = 0.0;
                if !joint.broken {
                    joint.rest = remapped_rest;
                    joint.rest_angle = remapped_rest_angle;
                }
            }
        }

        let chip_length = (old.radius * 1.8).max(8.0).min(len * 0.13);
        let mut splinter = BoneSegment {
            kind: old.kind,
            a: Vec2 {
                x: crack.x - dir.x * chip_length * 0.45 + normal.x * snap * 0.35,
                y: crack.y - dir.y * chip_length * 0.45 + normal.y * snap * 0.35,
            },
            b: Vec2 {
                x: crack.x + dir.x * chip_length * 0.55 + normal.x * snap * 1.35,
                y: crack.y + dir.y * chip_length * 0.55 + normal.y * snap * 1.35,
            },
            radius: (old.radius * 0.42).max(2.5),
            fracture_impulse: old.fracture_impulse,
            load: old.load * 0.5,
            fractured: true,
            broken_start: true,
            broken_end: true,
            broken_start_normal: normal,
            broken_end_normal: normal,
            fracture_generation: self.materials.max_bone_fracture_depth,
            splinter: true,
            angular_velocity: (spin_sign * fracture_spin * 1.65).clamp(-42.0, 42.0),
            ..BoneSegment::default()
        };
        splinter.previous_a = Vec2 {
            x: splinter.a.x - normal.x * recoil * 0.55,
            y: splinter.a.y - normal.y * recoil * 0.55,
        };
        splinter.previous_b = Vec2 {
            x: splinter.b.x - normal.x * recoil * 0.55,
            y: splinter.b.y - normal.y * recoil * 0.55,
        };
        splinter.home_a = Vec2 {
            x: home_crack.x - dir.x * chip_length * 0.45 + normal.x * snap * 0.2,
            y: home_crack.y - dir.y * chip_length * 0.45 + normal.y * snap * 0.2,
        };
        splinter.home_b = Vec2 {
            x: home_crack.x + dir.x * chip_length * 0.55 + normal.x * snap * 0.2,
            y: home_crack.y + dir.y * chip_length * 0.55 + normal.y * snap * 0.2,
        };
        splinter.rest_length = distance(splinter.a, splinter.b).max(EPSILON);
        self.debug.max_bone_angular_speed = self.debug.max_bone_angular_speed.max(
            first
                .angular_velocity
                .abs()
                .max(second.angular_velocity.abs())
                .max(splinter.angular_velocity.abs()),
        );
        self.bones.push(splinter);

        let blood_direction = Vec2 {
            x: normal.x + dir.x * 0.28,
            y: normal.y + dir.y * 0.28 - 0.30,
        };
        let load = impulse.max(old.load);
        self.emit_fluid(
            crack,
            blood_direction,
            10 + (load / 720.0).clamp(0.0, 16.0) as i32,
            180.0 + load * self.materials.fluid_impact_scale,
            2.7,
            1.28,
        );
        self.open_wound(
            crack,
            blood_direction,
            TissueLayer::Muscle,
            load / 980.0,
            2.9,
            1.18,
        );
        self.open_bone_marrow_wound(
            bone_index,
            1.0,
            add(blood_direction, scale(dir, -0.35)),
            load / 1780.0,
            2.15,
            1.05,
        );
        self.open_bone_marrow_wound(
            second_index,
            0.0,
            add(blood_direction, scale(dir, 0.35)),
            load / 1780.0,
            2.15,
            1.05,
        );
        self.damage_tissue_around_fracture(
            crack,
            (old.radius * 3.8).max(24.0 + overload * 10.0),
            load,
        );
        self.stats.fractured_bones += 1;
        self.debug.fractures += 1;
        if old.kind == BoneKind::Rib {
            self.stats.fractured_ribs += 1;
            self.debug.rib_fractures += 1;
        }
        self.debug.last_fracture_impulse = self.debug.last_fracture_impulse.max(load);
    }

    fn damage_tissue_around_fracture(&mut self, center: Vec2, radius: f64, impulse: f64) {
        let radius_sq = radius * radius;
        let skin_radius = radius * 0.62;
        let mut events = Vec::new();
        for i in 0..self.springs.len() {
            let spring = self.springs[i];
            if spring.broken {
                continue;
            }
            let a = self.points[spring.a];
            let b = self.points[spring.b];
            let d = distance_to_segment(center, a.position, b.position);
            if spring.layer == TissueLayer::Muscle && d <= radius {
                let mid = midpoint(a.position, b.position);
                self.springs[i].broken = true;
                self.springs[i].stress = 1.0;
                self.bump_point_exposure_load(spring.a, 1.0, impulse * 0.30);
                self.bump_point_exposure_load(spring.b, 1.0, impulse * 0.30);
                self.stats.broken_muscle += 1;
                events.push((
                    mid,
                    Vec2 {
                        x: mid.x - center.x,
                        y: mid.y - center.y - radius * 0.15,
                    },
                    TissueLayer::Muscle,
                    impulse,
                    3,
                    900.0,
                    90.0,
                    2.2,
                    1.05,
                    1050.0,
                    1.0,
                ));
            } else if spring.layer == TissueLayer::Skin
                && d <= skin_radius
                && impulse > self.materials.skin_tear_impulse * 1.18
            {
                let mid = midpoint(a.position, b.position);
                self.springs[i].broken = true;
                self.springs[i].stress = 1.0;
                self.bump_point_exposure_load(spring.a, 1.0, impulse * 0.18);
                self.bump_point_exposure_load(spring.b, 1.0, impulse * 0.18);
                self.stats.broken_skin += 1;
                events.push((
                    mid,
                    Vec2 {
                        x: mid.x - center.x,
                        y: mid.y - center.y - skin_radius * 0.25,
                    },
                    TissueLayer::Skin,
                    impulse,
                    5,
                    780.0,
                    120.0,
                    2.4,
                    1.18,
                    1250.0,
                    0.68,
                ));
            }
        }
        for i in 0..self.attachments.len() {
            let attachment = self.attachments[i];
            if attachment.broken {
                continue;
            }
            let skin = self.points[attachment.skin_point];
            let muscle = self.points[attachment.muscle_point];
            let skin_d = distance_sq(skin.position, center);
            let muscle_d = distance_sq(muscle.position, center);
            if skin_d.min(muscle_d) <= radius_sq {
                let mid = midpoint(skin.position, muscle.position);
                self.attachments[i].broken = true;
                self.bump_point_exposure_load(attachment.skin_point, 1.0, impulse * 0.15);
                self.bump_point_exposure_load(attachment.muscle_point, 1.0, impulse * 0.15);
                self.stats.broken_attachments += 1;
                events.push((
                    mid,
                    Vec2 {
                        x: mid.x - center.x,
                        y: mid.y - center.y - radius * 0.10,
                    },
                    TissueLayer::Muscle,
                    impulse,
                    2,
                    1200.0,
                    75.0,
                    1.8,
                    0.78,
                    1500.0,
                    0.74,
                ));
            }
        }
        for i in 0..self.triangles.len() {
            let triangle = self.triangles[i];
            if triangle.failed || triangle.layer != TissueLayer::Muscle {
                continue;
            }
            let centroid = Vec2 {
                x: (self.points[triangle.a].position.x
                    + self.points[triangle.b].position.x
                    + self.points[triangle.c].position.x)
                    / 3.0,
                y: (self.points[triangle.a].position.y
                    + self.points[triangle.b].position.y
                    + self.points[triangle.c].position.y)
                    / 3.0,
            };
            if distance_sq(centroid, center) <= radius_sq {
                self.triangles[i].damage = self.triangles[i].damage.max(1.08);
                self.triangles[i].failed = true;
                self.bump_point_exposure_load(triangle.a, 1.0, impulse * 0.12);
                self.bump_point_exposure_load(triangle.b, 1.0, impulse * 0.12);
                self.bump_point_exposure_load(triangle.c, 1.0, impulse * 0.12);
                events.push((
                    centroid,
                    Vec2 {
                        x: centroid.x - center.x,
                        y: centroid.y - center.y - radius * 0.08,
                    },
                    TissueLayer::Muscle,
                    impulse,
                    2,
                    1600.0,
                    70.0,
                    1.7,
                    0.72,
                    1750.0,
                    0.86,
                ));
            }
        }
        for (
            pos,
            dir,
            layer,
            impulse,
            base_count,
            div,
            base_speed,
            radius,
            intensity,
            pressure_div,
            depth,
        ) in events
        {
            self.emit_fluid(
                pos,
                dir,
                base_count + (impulse / div).clamp(0.0, 9.0) as i32,
                base_speed + impulse * self.materials.fluid_impact_scale,
                radius,
                intensity,
            );
            self.open_wound(pos, dir, layer, impulse / pressure_div, radius, depth);
        }
    }

    fn apply_bone_anchor_delta_idx(&mut self, index: usize, t: f64, dx: f64, dy: f64) {
        if let Some(bone) = self.bones.get_mut(index) {
            apply_bone_anchor_delta(bone, t, dx, dy);
        }
    }

    fn rotate_bone_around_anchor_idx(&mut self, index: usize, t: f64, angle: f64) {
        if let Some(bone) = self.bones.get_mut(index) {
            rotate_bone_around_anchor(bone, t, angle);
        }
    }

    fn bump_point_exposure_load(&mut self, index: usize, exposure: f64, load: f64) {
        let mut contused = false;
        let mut max_contusion = 0.0;
        if let Some(point) = self.points.get_mut(index) {
            point.exposure = point.exposure.max(exposure);
            point.load = point.load.max(load);
            contused = apply_point_contusion(point, self.materials, load, 0.42);
            max_contusion = point.contusion;
        }
        if contused {
            self.stats.contusion_events += 1;
            self.debug.contusion_events += 1;
            self.debug.max_contusion = self.debug.max_contusion.max(max_contusion);
        }
    }

    fn apply_joint_ligament_damage(&mut self, center: Vec2, load: f64) {
        let radius = self
            .materials
            .joint_ligament_damage_radius
            .max(self.materials.point_spacing);
        if load <= 0.0 || radius <= 0.0 {
            return;
        }

        let materials = self.materials;
        let mut contusion_events = 0;
        let mut max_contusion = self.debug.max_contusion;
        for point in &mut self.points {
            if point.layer != TissueLayer::Muscle {
                continue;
            }
            let distance_to_joint = distance(point.position, center);
            if distance_to_joint > radius {
                continue;
            }
            let falloff = (1.0 - distance_to_joint / radius).clamp(0.0, 1.0);
            let local_load = load * falloff;
            point.load = point
                .load
                .max(local_load * materials.joint_ligament_damage_load_scale);
            if apply_point_contusion(
                point,
                materials,
                local_load,
                materials.joint_ligament_damage_contusion_scale,
            ) {
                contusion_events += 1;
                max_contusion = max_contusion.max(point.contusion);
            }
        }
        if contusion_events > 0 {
            self.stats.contusion_events += contusion_events;
            self.debug.contusion_events += contusion_events;
            self.debug.max_contusion = self.debug.max_contusion.max(max_contusion);
        }
    }
}

fn apply_point_contusion(
    point: &mut Point,
    materials: Materials,
    load: f64,
    scale_factor: f64,
) -> bool {
    if load <= 0.0 || scale_factor <= 0.0 {
        return false;
    }
    let layer_threshold = if point.layer == TissueLayer::Skin {
        1.0
    } else {
        1.22
    };
    let threshold = materials.contusion_load_threshold * layer_threshold;
    if load <= threshold {
        return false;
    }
    let layer_scale = if point.layer == TissueLayer::Skin {
        1.0
    } else {
        0.72
    };
    let amount =
        (((load - threshold) / threshold) * 0.24 * scale_factor * layer_scale).clamp(0.0, 0.46);
    if amount <= 0.0 {
        return false;
    }
    let before = point.contusion;
    point.contusion = (point.contusion + amount).min(1.35);
    point.contusion > before + 0.002
}

fn tissue_tear_weakening(materials: Materials, contusion: f64) -> f64 {
    (contusion.clamp(0.0, 1.0) * materials.contusion_tear_weakening).clamp(0.0, 0.52)
}

fn tissue_stiffness_softening(materials: Materials, contusion: f64) -> f64 {
    (contusion.clamp(0.0, 1.0) * materials.contusion_stiffness_softening).clamp(0.0, 0.44)
}

fn tissue_spring_compliance(materials: Materials, layer: TissueLayer) -> f64 {
    match layer {
        TissueLayer::Skin => materials.skin_spring_compliance,
        TissueLayer::Muscle => materials.muscle_spring_compliance,
    }
    .max(0.0)
}

fn tissue_area_compliance(materials: Materials, layer: TissueLayer) -> f64 {
    match layer {
        TissueLayer::Skin => materials.skin_area_compliance,
        TissueLayer::Muscle => materials.muscle_area_compliance,
    }
    .max(0.0)
}

fn xpbd_alpha(compliance: f64, dt: f64) -> f64 {
    if compliance <= EPSILON {
        0.0
    } else {
        compliance / dt.max(1.0e-5).powi(2)
    }
}

fn tissue_fatigue_delta(
    materials: Materials,
    spring: Spring,
    stretch_ratio: f64,
    endpoint_load: f64,
    contusion: f64,
) -> f64 {
    if spring.broken || materials.tissue_fatigue_rate <= 0.0 {
        return 0.0;
    }
    let stretch_threshold = materials
        .tissue_fatigue_stretch_threshold
        .min(spring.tear_stretch * 0.86)
        .max(1.0);
    let stretch_window = (spring.tear_stretch - stretch_threshold).max(0.08);
    let stretch_term = ((stretch_ratio - stretch_threshold) / stretch_window).clamp(0.0, 1.35);

    let layer_load_scale = if spring.layer == TissueLayer::Muscle {
        1.18
    } else {
        1.0
    };
    let load_threshold = materials.tissue_fatigue_load_threshold * layer_load_scale;
    let load_window = (spring.tear_impulse - load_threshold).max(140.0);
    let load_term = ((endpoint_load - load_threshold) / load_window).clamp(0.0, 1.45);

    let combined = stretch_term * 0.62 + load_term * 0.38 + stretch_term * load_term * 0.28;
    if combined <= 0.0 {
        return 0.0;
    }
    let layer_scale = if spring.layer == TissueLayer::Muscle {
        0.84
    } else {
        1.0
    };
    let contusion_boost = 1.0 + contusion.clamp(0.0, 1.0) * 0.55;
    (combined * materials.tissue_fatigue_rate * layer_scale * contusion_boost).clamp(0.0, 0.075)
}

fn tissue_fatigue_tear_weakening(materials: Materials, fatigue: f64) -> f64 {
    (fatigue.clamp(0.0, 1.0) * materials.tissue_fatigue_tear_weakening).clamp(0.0, 0.42)
}

fn tissue_fatigue_stiffness_softening(materials: Materials, fatigue: f64) -> f64 {
    (fatigue.clamp(0.0, 1.0) * materials.tissue_fatigue_stiffness_softening).clamp(0.0, 0.26)
}

fn tissue_plastic_rest_delta(
    materials: Materials,
    spring: Spring,
    stretch_ratio: f64,
    endpoint_load: f64,
    contusion: f64,
    fatigue: f64,
) -> f64 {
    if spring.broken || spring.rest_reference <= EPSILON || materials.tissue_plastic_rate <= 0.0 {
        return 0.0;
    }

    let stretch_threshold = materials
        .tissue_plastic_stretch_threshold
        .min(spring.tear_stretch * 0.88)
        .max(1.01);
    let stretch_drive = ((stretch_ratio - stretch_threshold)
        / (spring.tear_stretch - stretch_threshold).max(0.10))
    .clamp(0.0, 1.25);

    let compression_threshold = materials
        .tissue_plastic_compression_threshold
        .clamp(0.42, 0.98);
    let compression_drive = ((compression_threshold - stretch_ratio)
        / (compression_threshold - 0.42).max(0.08))
    .clamp(0.0, 1.0);

    let layer_load_scale = if spring.layer == TissueLayer::Muscle {
        1.12
    } else {
        1.0
    };
    let load_threshold = materials.tissue_fatigue_load_threshold * 0.78 * layer_load_scale;
    let load_drive = ((endpoint_load - load_threshold)
        / (spring.tear_impulse - load_threshold).max(180.0))
    .clamp(0.0, 1.15);

    let damage_memory = (fatigue * 0.62 + contusion.clamp(0.0, 1.0) * 0.32).clamp(0.0, 1.0);
    let memory_boost = 0.55 + damage_memory;
    let load_boost = 0.45 + load_drive * 0.55;
    if stretch_drive >= compression_drive && stretch_drive > 0.0 {
        return spring.rest_reference
            * materials.tissue_plastic_rate
            * stretch_drive
            * memory_boost
            * load_boost;
    }
    if compression_drive > 0.0 && load_drive > 0.0 {
        return -spring.rest_reference
            * materials.tissue_plastic_rate
            * 0.62
            * compression_drive
            * memory_boost
            * load_boost;
    }
    0.0
}

fn spring_fatigue(springs: &[Spring], index: usize) -> f64 {
    springs
        .get(index)
        .map(|spring| spring.fatigue)
        .unwrap_or(0.0)
        .clamp(0.0, 1.35)
}

fn muscle_fiber_spring_broken(springs: &[Spring], index: usize) -> bool {
    springs
        .get(index)
        .map(|spring| spring.layer == TissueLayer::Muscle && spring.fiber && spring.broken)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn average_fragment_speed(bone: BoneSegment) -> f64 {
        let a_speed = distance(bone.a, bone.previous_a);
        let b_speed = distance(bone.b, bone.previous_b);
        (a_speed + b_speed) * 0.5
    }

    fn average_fragment_velocity(bone: BoneSegment) -> Vec2 {
        Vec2 {
            x: ((bone.a.x - bone.previous_a.x) + (bone.b.x - bone.previous_b.x)) * 0.5,
            y: ((bone.a.y - bone.previous_a.y) + (bone.b.y - bone.previous_b.y)) * 0.5,
        }
    }

    fn pair_closing_speed(a: BoneSegment, b: BoneSegment, normal_from_b_to_a: Vec2) -> f64 {
        dot(
            average_fragment_velocity(a),
            scale(normal_from_b_to_a, -1.0),
        )
        .max(0.0)
            + dot(average_fragment_velocity(b), normal_from_b_to_a).max(0.0)
    }

    #[test]
    fn skin_attachment_pulls_surface_toward_muscle() {
        let mut materials = Materials::default();
        materials.attachment_stiffness = 0.34;
        materials.attachment_break_stretch = 3.0;

        let mut world = World::new(materials);
        let skin = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Skin, false);
        let muscle = world.add_point(Vec2 { x: 40.0, y: 0.0 }, TissueLayer::Muscle, false);
        world.add_attachment(skin, muscle);

        world.points[muscle].position = Vec2 { x: 70.0, y: 0.0 };
        world.solve_attachments();

        assert!(
            !world.attachments[0].broken,
            "moderate skin-to-muscle stretch should stay attached"
        );
        assert!(
            world.points[skin].position.x > 6.0,
            "skin should be pulled toward underlying muscle"
        );
        assert!(
            world.points[muscle].position.x < 70.0,
            "muscle should receive some opposing tether reaction"
        );
    }

    #[test]
    fn stretched_bone_joint_subluxates_before_breaking() {
        let mut materials = Materials::default();
        materials.bone_joint_subluxation_stretch = 1.18;
        materials.bone_joint_break_stretch = 2.8;
        materials.bone_joint_subluxation_slack = 16.0;
        materials.bone_joint_subluxation_stiffness_scale = 0.45;
        let mut world = World::new(materials);
        let a = world.add_bone_segment(
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 40.0, y: 0.0 },
            5.0,
            9999.0,
            false,
        );
        let b = world.add_bone_segment(
            Vec2 { x: 42.0, y: 0.0 },
            Vec2 { x: 82.0, y: 0.0 },
            5.0,
            9999.0,
            false,
        );
        world.add_bone_joint(a, 1.0, b, 0.0, -0.5, 0.5);
        world.bones[b].a.x += 8.5;
        world.bones[b].b.x += 8.5;
        world.bones[b].load = 760.0;

        world.solve_bone_joints();

        assert!(
            world.bone_joints[0].subluxated,
            "moderate joint stretch should enter a subluxated state"
        );
        assert!(
            !world.bone_joints[0].broken,
            "subluxation should happen before total joint break"
        );
        assert!(world.bone_joints[0].subluxation > 0.0);
        assert_eq!(world.stats.bone_joint_subluxations, 1);
        assert_eq!(world.debug.bone_joint_subluxations, 1);
        assert!(world.debug.max_bone_joint_subluxation > 0.0);
    }

    #[test]
    fn subluxated_joint_marks_ligament_tissue_damage() {
        let mut materials = Materials::default();
        materials.bone_joint_subluxation_stretch = 1.18;
        materials.bone_joint_break_stretch = 2.8;
        materials.joint_ligament_damage_radius = 16.0;
        materials.joint_ligament_damage_load_scale = 0.50;
        materials.joint_ligament_damage_contusion_scale = 0.90;
        materials.contusion_load_threshold = 120.0;
        let mut world = World::new(materials);
        let tissue = world.add_point(Vec2 { x: 45.0, y: 0.0 }, TissueLayer::Muscle, false);
        let a = world.add_bone_segment(
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 40.0, y: 0.0 },
            5.0,
            9999.0,
            false,
        );
        let b = world.add_bone_segment(
            Vec2 { x: 42.0, y: 0.0 },
            Vec2 { x: 82.0, y: 0.0 },
            5.0,
            9999.0,
            false,
        );
        world.add_bone_joint(a, 1.0, b, 0.0, -0.5, 0.5);
        world.bones[b].a.x += 8.5;
        world.bones[b].b.x += 8.5;
        world.bones[b].load = 760.0;

        world.solve_bone_joints();

        assert!(world.bone_joints[0].subluxated);
        assert_eq!(world.stats.joint_ligament_damage_events, 1);
        assert_eq!(world.debug.joint_ligament_damage_events, 1);
        assert!(world.points[tissue].load > 0.0);
        assert!(world.points[tissue].contusion > 0.0);
        assert!(world.stats.contusion_events >= 1);
    }

    #[test]
    fn quiet_bone_joint_does_not_subluxate_below_threshold() {
        let mut materials = Materials::default();
        materials.bone_joint_subluxation_stretch = 1.50;
        materials.bone_joint_break_stretch = 2.8;
        let mut world = World::new(materials);
        let a = world.add_bone_segment(
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 40.0, y: 0.0 },
            5.0,
            9999.0,
            false,
        );
        let b = world.add_bone_segment(
            Vec2 { x: 42.0, y: 0.0 },
            Vec2 { x: 82.0, y: 0.0 },
            5.0,
            9999.0,
            false,
        );
        world.add_bone_joint(a, 1.0, b, 0.0, -0.5, 0.5);
        world.bones[b].a.x += 0.6;
        world.bones[b].b.x += 0.6;

        world.solve_bone_joints();

        assert!(!world.bone_joints[0].subluxated);
        assert!(!world.bone_joints[0].broken);
        assert_eq!(world.stats.bone_joint_subluxations, 0);
        assert_eq!(world.debug.bone_joint_subluxations, 0);
        assert_eq!(world.stats.joint_ligament_damage_events, 0);
        assert_eq!(world.debug.joint_ligament_damage_events, 0);
    }

    #[test]
    fn embedded_fractured_fragment_resists_and_drags_tissue() {
        let mut materials = Materials::default();
        materials.gravity = 0.0;
        materials.bone_damping = 1.0;
        materials.bone_angular_damping = 1.0;
        materials.fragment_repulsion_stiffness = 1.0;

        let mut world = World::new(materials);
        let point_index = world.add_point(Vec2 { x: 156.0, y: 107.0 }, TissueLayer::Muscle, false);
        let bone_index = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 200.0, y: 100.0 },
            7.0,
            9999.0,
            false,
        );

        world.bones[bone_index].fractured = true;
        world.bones[bone_index].broken_end = true;
        world.bones[bone_index].broken_end_normal = Vec2 { x: 0.0, y: 1.0 };
        world.bones[bone_index].previous_a = Vec2 { x: 70.0, y: 100.0 };
        world.bones[bone_index].previous_b = Vec2 { x: 170.0, y: 100.0 };
        world.bones[bone_index].angular_velocity = 8.0;

        let before_speed = average_fragment_speed(world.bones[bone_index]);
        world.solve_bone_fragment_tissue_contacts();
        let after_bone = world.bones[bone_index];
        let after_point = world.points[point_index];

        assert!(
            after_point.position.y > 107.4,
            "embedded muscle point should be displaced by fragment contact"
        );
        assert!(
            after_bone.a.y < 100.0 || after_bone.b.y < 100.0,
            "fragment should receive an opposing contact correction"
        );
        assert!(
            average_fragment_speed(after_bone) < before_speed * 0.95,
            "fragment velocity should be damped by tissue friction"
        );
        assert!(
            after_bone.angular_velocity.abs() < 8.0,
            "embedded fragment spin should be damped by tissue friction"
        );
        assert!(
            after_point.load > 0.0 && world.debug.max_fragment_depth > 0.0,
            "fragment contact should report load/depth telemetry"
        );
    }

    #[test]
    fn wound_source_follows_anchored_tissue_point() {
        let mut world = World::new(Materials::default());
        let point = world.add_point(Vec2 { x: 100.0, y: 100.0 }, TissueLayer::Muscle, false);

        world.open_wound(
            Vec2 { x: 102.0, y: 103.0 },
            Vec2 { x: 1.0, y: 0.0 },
            TissueLayer::Muscle,
            2.0,
            2.0,
            1.0,
        );

        assert_eq!(
            world.wounds[0].anchor_point, point,
            "wound should anchor to the nearest matching tissue point"
        );
        let before = world.wounds[0].position;
        world.points[point].position.x += 23.0;
        world.points[point].position.y += 7.0;

        world.update_wound_anchors();

        let after = world.wounds[0].position;
        assert!(
            (after.x - (before.x + 23.0)).abs() < 0.001
                && (after.y - (before.y + 7.0)).abs() < 0.001,
            "anchored wound should move with damaged tissue"
        );
    }

    #[test]
    fn wound_source_can_follow_bone_when_no_tissue_anchor_exists() {
        let mut world = World::new(Materials::default());
        let bone = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 200.0, y: 100.0 },
            5.0,
            9999.0,
            false,
        );

        world.open_wound(
            Vec2 { x: 150.0, y: 110.0 },
            Vec2 { x: 0.0, y: 1.0 },
            TissueLayer::Muscle,
            2.0,
            2.0,
            1.0,
        );

        assert_eq!(
            world.wounds[0].anchor_bone, bone,
            "wound should use a bone anchor when no tissue points exist"
        );
        let before = world.wounds[0].position;
        world.bones[bone].a.x += 31.0;
        world.bones[bone].b.x += 31.0;
        world.bones[bone].a.y -= 9.0;
        world.bones[bone].b.y -= 9.0;

        world.update_wound_anchors();

        let after = world.wounds[0].position;
        assert!(
            (after.x - (before.x + 31.0)).abs() < 0.001
                && (after.y - (before.y - 9.0)).abs() < 0.001,
            "anchored wound should move with the damaged bone feature"
        );
    }

    #[test]
    fn fracture_marrow_source_anchors_to_broken_bone_cap() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        let mut world = World::new(materials);
        let bone = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 180.0, y: 100.0 },
            6.0,
            9999.0,
            false,
        );
        world.bones[bone].fractured = true;
        world.bones[bone].broken_end = true;
        world.bones[bone].broken_end_normal = Vec2 { x: 0.0, y: -1.0 };

        world.open_bone_marrow_wound(bone, 1.0, Vec2 { x: 0.0, y: -1.0 }, 1.6, 2.2, 1.0);

        assert_eq!(world.stats.fracture_marrow_sources, 1);
        assert_eq!(world.debug.fracture_marrow_sources, 1);
        assert_eq!(world.wounds[0].anchor_bone, bone);
        assert_eq!(world.wounds[0].anchor_point, MISSING_ANCHOR);
        assert!((world.wounds[0].anchor_t - 1.0).abs() < 0.001);

        world.bones[bone].b.x += 19.0;
        world.bones[bone].b.y += 8.0;
        world.update_wound_anchors();

        assert!(
            distance(world.wounds[0].position, world.bones[bone].b) < 0.001,
            "marrow source should stay fixed to the moving broken cap"
        );
    }

    #[test]
    fn fracture_marrow_source_leaks_through_wound_update() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 32;
        materials.wound_leak_rate = 18.0;
        let mut world = World::new(materials);
        let bone = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 180.0, y: 100.0 },
            6.0,
            9999.0,
            false,
        );
        world.bones[bone].fractured = true;
        world.bones[bone].broken_end = true;
        world.open_bone_marrow_wound(bone, 1.0, Vec2 { x: 0.0, y: -1.0 }, 2.4, 2.2, 1.0);

        for _ in 0..8 {
            world.update_wounds(world.materials.fixed_dt);
        }

        assert!(
            world.stats.wound_fluid_particles > 0,
            "marrow source should use the persistent wound leak path"
        );
        assert!(world.fluids.iter().any(|fluid| fluid.life > 0.0));
    }

    #[test]
    fn persistent_wound_leak_drains_finite_blood_volume() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.wound_leak_rate = 120.0;
        materials.wound_clot_rate = 0.0;
        materials.blood_volume_capacity = 1.0;
        materials.blood_loss_per_wound_particle = 0.05;
        let mut world = World::new(materials);
        world.open_wound(
            Vec2 { x: 10.0, y: 10.0 },
            Vec2 { x: 0.0, y: 1.0 },
            TissueLayer::Muscle,
            4.0,
            2.4,
            1.0,
        );

        world.update_wounds(world.materials.fixed_dt);

        assert!(
            world.stats.blood_loss > 0.0,
            "persistent wound emission should drain finite blood volume"
        );
        assert!(world.blood_volume_fraction() < 1.0);
        assert_eq!(world.debug.blood_loss, world.stats.blood_loss);
        assert!(world.debug.blood_volume_fraction < 1.0);
    }

    #[test]
    fn low_blood_volume_reduces_later_wound_pressure_and_loss() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.wound_leak_rate = 120.0;
        materials.wound_clot_rate = 0.0;
        materials.blood_volume_capacity = 1.0;
        materials.blood_loss_per_wound_particle = 0.05;
        materials.blood_pressure_min_scale = 0.34;

        let mut full = World::new(materials);
        full.open_wound(
            Vec2 { x: 10.0, y: 10.0 },
            Vec2 { x: 0.0, y: 1.0 },
            TissueLayer::Muscle,
            4.0,
            2.4,
            1.0,
        );

        let mut depleted = World::new(materials);
        depleted.blood_volume = 0.10;
        depleted.open_wound(
            Vec2 { x: 10.0, y: 10.0 },
            Vec2 { x: 0.0, y: 1.0 },
            TissueLayer::Muscle,
            4.0,
            2.4,
            1.0,
        );

        assert!(
            depleted.wounds[0].pressure < full.wounds[0].pressure,
            "new wound pressure should scale down after systemic blood loss"
        );

        full.update_wounds(full.materials.fixed_dt);
        depleted.update_wounds(depleted.materials.fixed_dt);

        assert!(
            full.stats.blood_loss > depleted.stats.blood_loss,
            "the same wound should drain less per frame after severe depletion lowers pressure"
        );
    }

    #[test]
    fn low_blood_volume_reduces_passive_shape_turgor() {
        let mut materials = Materials::default();
        materials.gravity = 0.0;
        materials.damping = 1.0;
        materials.skin_shape_stiffness = 0.20;
        materials.blood_volume_capacity = 1.0;
        materials.blood_turgor_min_scale = 0.25;

        let mut full = World::new(materials);
        let full_point = full.add_point(Vec2 { x: 100.0, y: 100.0 }, TissueLayer::Skin, false);
        full.points[full_point].position.x = 200.0;
        full.points[full_point].previous = full.points[full_point].position;

        let mut depleted = World::new(materials);
        depleted.blood_volume = 0.0;
        let depleted_point =
            depleted.add_point(Vec2 { x: 100.0, y: 100.0 }, TissueLayer::Skin, false);
        depleted.points[depleted_point].position.x = 200.0;
        depleted.points[depleted_point].previous = depleted.points[depleted_point].position;

        full.integrate(full.materials.fixed_dt, 400.0, 360.0);
        depleted.integrate(depleted.materials.fixed_dt, 400.0, 360.0);

        assert!(
            full.points[full_point].position.x < depleted.points[depleted_point].position.x,
            "full blood reserve should preserve stronger passive shape recovery"
        );
        assert!((full.blood_turgor_scale() - 1.0).abs() < 0.001);
        assert!((depleted.blood_turgor_scale() - 0.25).abs() < 0.001);
    }

    #[test]
    fn low_blood_volume_reduces_area_constraint_turgor() {
        fn area_error(world: &World, area_index: usize) -> f64 {
            let area = world.areas[area_index];
            let current = signed_area(
                world.points[area.a].position,
                world.points[area.b].position,
                world.points[area.c].position,
            );
            (current - area.rest_area).abs()
        }

        fn build_area_world(materials: Materials, blood_volume: f64) -> World {
            let mut world = World::new(materials);
            world.blood_volume = blood_volume;
            let a = world.add_point(Vec2 { x: 100.0, y: 100.0 }, TissueLayer::Skin, false);
            let b = world.add_point(Vec2 { x: 200.0, y: 100.0 }, TissueLayer::Skin, false);
            let c = world.add_point(Vec2 { x: 100.0, y: 200.0 }, TissueLayer::Skin, false);
            world.add_spring(a, b, TissueLayer::Skin, 1.0, 99.0, 9999.0, false);
            world.add_spring(b, c, TissueLayer::Skin, 1.0, 99.0, 9999.0, false);
            world.add_spring(c, a, TissueLayer::Skin, 1.0, 99.0, 9999.0, false);
            world.add_area(a, b, c, TissueLayer::Skin, 0.80);
            world.points[c].position.y = 250.0;
            world
        }

        let mut materials = Materials::default();
        materials.blood_volume_capacity = 1.0;
        materials.blood_turgor_min_scale = 0.20;

        let mut full = build_area_world(materials, 1.0);
        let mut depleted = build_area_world(materials, 0.0);
        let initial_error = area_error(&full, 0);

        full.solve_areas();
        depleted.solve_areas();

        let full_error = area_error(&full, 0);
        let depleted_error = area_error(&depleted, 0);
        assert!(full_error < initial_error);
        assert!(
            full_error < depleted_error,
            "depleted reserve should make triangle area preservation less forceful"
        );
    }

    #[test]
    fn spring_compliance_softens_single_iteration_projection() {
        fn stretched_spring_world(mut materials: Materials) -> World {
            materials.skin_spring_compliance = 0.0;
            materials.muscle_spring_compliance = 0.0;
            let mut world = World::new(materials);
            let a = world.add_point(Vec2 { x: 100.0, y: 100.0 }, TissueLayer::Skin, true);
            let b = world.add_point(Vec2 { x: 120.0, y: 100.0 }, TissueLayer::Skin, false);
            world.add_spring(a, b, TissueLayer::Skin, 1.0, 99.0, 9999.0, false);
            world.points[b].position.x = 150.0;
            world
        }

        fn spring_length(world: &World) -> f64 {
            let spring = world.springs[0];
            distance(
                world.points[spring.a].position,
                world.points[spring.b].position,
            )
        }

        let legacy_materials = Materials::default();
        let mut legacy = stretched_spring_world(legacy_materials);

        let mut compliant_materials = Materials::default();
        compliant_materials.skin_spring_compliance = 0.020;
        let mut compliant = stretched_spring_world(compliant_materials);
        compliant.materials.skin_spring_compliance = compliant_materials.skin_spring_compliance;

        legacy.solve_springs();
        compliant.solve_springs();

        let legacy_error = (spring_length(&legacy) - legacy.springs[0].rest).abs();
        let compliant_error = (spring_length(&compliant) - compliant.springs[0].rest).abs();

        assert!(
            compliant_error > legacy_error + 5.0,
            "XPBD compliance should leave a controlled residual spring stretch in one solve"
        );
        assert!(
            compliant.springs[0].lambda.abs() > 0.0,
            "compliant spring projection should accumulate a constraint lambda"
        );
    }

    #[test]
    fn area_compliance_softens_single_iteration_projection() {
        fn area_error(world: &World) -> f64 {
            let area = world.areas[0];
            let current = signed_area(
                world.points[area.a].position,
                world.points[area.b].position,
                world.points[area.c].position,
            );
            (current - area.rest_area).abs()
        }

        fn distorted_area_world(mut materials: Materials) -> World {
            materials.skin_area_compliance = 0.0;
            materials.muscle_area_compliance = 0.0;
            let mut world = World::new(materials);
            let a = world.add_point(Vec2 { x: 100.0, y: 100.0 }, TissueLayer::Skin, true);
            let b = world.add_point(Vec2 { x: 200.0, y: 100.0 }, TissueLayer::Skin, true);
            let c = world.add_point(Vec2 { x: 100.0, y: 200.0 }, TissueLayer::Skin, false);
            world.add_spring(a, b, TissueLayer::Skin, 1.0, 99.0, 9999.0, false);
            world.add_spring(b, c, TissueLayer::Skin, 1.0, 99.0, 9999.0, false);
            world.add_spring(c, a, TissueLayer::Skin, 1.0, 99.0, 9999.0, false);
            world.add_area(a, b, c, TissueLayer::Skin, 1.0);
            world.points[c].position.y = 260.0;
            world
        }

        let legacy_materials = Materials::default();
        let mut legacy = distorted_area_world(legacy_materials);

        let mut compliant_materials = Materials::default();
        compliant_materials.skin_area_compliance = 8.0;
        let mut compliant = distorted_area_world(compliant_materials);
        compliant.materials.skin_area_compliance = compliant_materials.skin_area_compliance;

        legacy.solve_areas();
        compliant.solve_areas();

        assert!(
            area_error(&compliant) > area_error(&legacy) + 500.0,
            "XPBD compliance should leave a controlled residual area error in one solve"
        );
        assert!(
            compliant.areas[0].lambda.abs() > 0.0,
            "compliant area projection should accumulate a constraint lambda"
        );
    }

    #[test]
    fn compressed_muscle_cavity_builds_pressure_and_pushes_tissue() {
        let mut materials = Materials::default();
        materials.cavity_pressure_stiffness = 0.90;
        materials.cavity_contusion_pressure = 0.24;
        materials.cavity_rupture_pressure = 2.0;
        let mut world = World::new(materials);
        let a = world.add_point(Vec2 { x: 100.0, y: 100.0 }, TissueLayer::Muscle, false);
        let b = world.add_point(Vec2 { x: 200.0, y: 100.0 }, TissueLayer::Muscle, false);
        let c = world.add_point(Vec2 { x: 100.0, y: 200.0 }, TissueLayer::Muscle, false);
        world.add_spring(a, b, TissueLayer::Muscle, 1.0, 99.0, 9999.0, false);
        world.add_spring(b, c, TissueLayer::Muscle, 1.0, 99.0, 9999.0, false);
        world.add_spring(c, a, TissueLayer::Muscle, 1.0, 99.0, 9999.0, false);
        world.add_area(a, b, c, TissueLayer::Muscle, 0.6);
        let cavity = world.add_cavity_from_areas(vec![0]);

        world.points[c].position.y = 128.0;
        let compressed_y = world.points[c].position.y;
        world.update_cavities(world.materials.fixed_dt);

        assert_ne!(cavity, MISSING_ANCHOR);
        assert!(
            world.cavities[cavity].pressure > 0.45,
            "compressed internal volume should build bounded pressure"
        );
        assert!(
            world.cavities[cavity].collapse > 0.60,
            "cavity should report a meaningful collapse ratio"
        );
        assert!(
            world.points[c].position.y > compressed_y,
            "cavity pressure should push compressed tissue back outward"
        );
        assert!(world.points[c].load > 0.0);
        assert!(world.stats.cavity_pressure_events > 0);
        assert!(world.debug.max_cavity_pressure >= world.cavities[cavity].pressure);
    }

    #[test]
    fn high_cavity_pressure_opens_capped_internal_rupture_wound() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 32;
        materials.max_wound_sources = 8;
        materials.cavity_pressure_stiffness = 1.15;
        materials.cavity_rupture_pressure = 0.42;
        materials.cavity_rupture_load_scale = 1.0;
        materials.max_cavity_ruptures_per_step = 1;
        let mut world = World::new(materials);
        world.debug.tool = ToolMode::Heavy;
        world.debug.impact = 5000.0;
        let a = world.add_point(Vec2 { x: 100.0, y: 100.0 }, TissueLayer::Muscle, false);
        let b = world.add_point(Vec2 { x: 200.0, y: 100.0 }, TissueLayer::Muscle, false);
        let c = world.add_point(Vec2 { x: 100.0, y: 200.0 }, TissueLayer::Muscle, false);
        world.add_spring(a, b, TissueLayer::Muscle, 1.0, 99.0, 9999.0, false);
        world.add_spring(b, c, TissueLayer::Muscle, 1.0, 99.0, 9999.0, false);
        world.add_spring(c, a, TissueLayer::Muscle, 1.0, 99.0, 9999.0, false);
        world.add_area(a, b, c, TissueLayer::Muscle, 0.6);
        let cavity = world.add_cavity_from_areas(vec![0]);

        world.points[c].position.y = 120.0;
        world.points[a].load = 980.0;
        world.points[b].load = 980.0;
        world.points[c].load = 980.0;
        world.update_cavities(world.materials.fixed_dt);
        world.update_cavities(world.materials.fixed_dt);

        assert_ne!(cavity, MISSING_ANCHOR);
        assert!(world.cavities[cavity].ruptured);
        assert_eq!(world.stats.cavity_ruptures, 1);
        assert_eq!(world.debug.cavity_ruptures, 1);
        assert!(
            world
                .wounds
                .iter()
                .any(|wound| wound.layer == TissueLayer::Muscle),
            "cavity rupture should use the persistent muscle wound path"
        );
        assert!(world.stats.emitted_fluid_particles > 0);
    }

    #[test]
    fn medium_blunt_cavity_pressure_does_not_open_internal_rupture() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 32;
        materials.max_wound_sources = 8;
        materials.cavity_pressure_stiffness = 0.72;
        materials.cavity_rupture_pressure = 0.80;
        materials.cavity_rupture_load_scale = 3.2;
        materials.cavity_non_heavy_pressure_load_cap = 1.85;
        materials.cavity_non_heavy_rupture_load_scale = 2.05;
        let mut world = World::new(materials);
        world.debug.tool = ToolMode::Blunt;
        world.debug.impact = 2800.0;
        let a = world.add_point(Vec2 { x: 100.0, y: 100.0 }, TissueLayer::Muscle, false);
        let b = world.add_point(Vec2 { x: 200.0, y: 100.0 }, TissueLayer::Muscle, false);
        let c = world.add_point(Vec2 { x: 100.0, y: 200.0 }, TissueLayer::Muscle, false);
        world.add_spring(a, b, TissueLayer::Muscle, 1.0, 99.0, 9999.0, false);
        world.add_spring(b, c, TissueLayer::Muscle, 1.0, 99.0, 9999.0, false);
        world.add_spring(c, a, TissueLayer::Muscle, 1.0, 99.0, 9999.0, false);
        world.add_area(a, b, c, TissueLayer::Muscle, 0.6);
        let cavity = world.add_cavity_from_areas(vec![0]);

        world.points[c].position.y = 195.0;
        world.points[a].load = 1900.0;
        world.points[b].load = 1900.0;
        world.points[c].load = 1900.0;
        world.update_cavities(world.materials.fixed_dt);
        world.update_cavities(world.materials.fixed_dt);

        assert_ne!(cavity, MISSING_ANCHOR);
        assert!(
            world.cavities[cavity].pressure < world.materials.cavity_rupture_pressure,
            "medium blunt cavity pressure should stay below the internal rupture threshold"
        );
        assert!(!world.cavities[cavity].ruptured);
        assert_eq!(world.stats.cavity_ruptures, 0);
        assert_eq!(world.debug.cavity_ruptures, 0);
        assert!(
            world.stats.cavity_pressure_events > 0,
            "medium blunt compression should still produce internal pressure telemetry"
        );
    }

    #[test]
    fn cavity_pressure_accumulates_organ_damage_without_immediate_rupture() {
        let mut materials = Materials::default();
        materials.organ_pressure_damage_threshold = 0.20;
        materials.organ_rupture_damage = 5.0;
        materials.organ_damage_rate = 0.40;
        let mut world = World::new(materials);
        world.cavities.push(CavityRegion {
            pressure: 0.82,
            collapse: 0.18,
            centroid: Vec2 { x: 100.0, y: 100.0 },
            rest_area: 1.0,
            ..CavityRegion::default()
        });
        let organ = world.add_organ_region(
            OrganKind::Liver,
            Vec2 { x: 106.0, y: 104.0 },
            Vec2 { x: 30.0, y: 24.0 },
        );

        world.update_organs(world.materials.fixed_dt);

        assert!(
            world.organs[organ].damage > 0.0,
            "organ pressure proxy should accumulate internal injury state"
        );
        assert!(!world.organs[organ].ruptured);
        assert!(world.stats.organ_damage_events > 0);
        assert!(world.debug.max_organ_damage >= world.organs[organ].damage);
    }

    #[test]
    fn severe_organ_damage_opens_internal_bleeding_wound() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 32;
        materials.max_wound_sources = 8;
        materials.organ_pressure_damage_threshold = 0.10;
        materials.organ_rupture_damage = 0.15;
        materials.organ_damage_rate = 1.0;
        materials.max_organ_ruptures_per_step = 1;
        let mut world = World::new(materials);
        world.cavities.push(CavityRegion {
            pressure: 1.2,
            collapse: 0.24,
            centroid: Vec2 { x: 100.0, y: 100.0 },
            rest_area: 1.0,
            ..CavityRegion::default()
        });
        let organ = world.add_organ_region(
            OrganKind::Spleen,
            Vec2 { x: 108.0, y: 104.0 },
            Vec2 { x: 28.0, y: 22.0 },
        );
        world.debug.tool = ToolMode::Heavy;
        world.debug.impact = 6000.0;

        world.update_organs(world.materials.fixed_dt);

        assert!(world.organs[organ].ruptured);
        assert_eq!(world.stats.organ_ruptures, 1);
        assert_eq!(world.debug.organ_ruptures, 1);
        assert!(
            world
                .wounds
                .iter()
                .any(|wound| wound.layer == TissueLayer::Muscle),
            "organ rupture should reuse persistent internal muscle wound sources"
        );
        assert!(world.stats.emitted_fluid_particles > 0);
    }

    #[test]
    fn sharp_striker_penetrates_organ_and_opens_internal_bleeding_wound() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 32;
        materials.max_wound_sources = 8;
        materials.organ_penetration_impulse = 80.0;
        materials.organ_penetration_damage = 1.0;
        materials.organ_rupture_damage = 0.65;
        materials.max_organ_penetrations_per_step = 1;
        materials.max_organ_ruptures_per_step = 1;
        materials.max_total_organ_ruptures = 1;
        let mut world = World::new(materials);
        let organ = world.add_organ_region(
            OrganKind::Liver,
            Vec2 { x: 120.0, y: 100.0 },
            Vec2 { x: 28.0, y: 20.0 },
        );

        world.collide_striker(
            world.materials.fixed_dt,
            &InputState {
                active: true,
                down: true,
                x: 104.0,
                y: 100.0,
                vx: 640.0,
                vy: 0.0,
                power: 2.0,
                tool: ToolMode::Sharp,
            },
        );

        assert!(world.organs[organ].penetrated);
        assert!(world.organs[organ].ruptured);
        assert_eq!(world.stats.organ_penetrations, 1);
        assert_eq!(world.debug.organ_penetrations, 1);
        assert_eq!(world.stats.organ_ruptures, 1);
        assert_eq!(world.debug.organ_ruptures, 1);
        assert!(
            world
                .wounds
                .iter()
                .any(|wound| wound.layer == TissueLayer::Muscle),
            "direct organ penetration should reuse persistent internal muscle wounds"
        );
    }

    #[test]
    fn blunt_striker_overlap_does_not_count_as_direct_organ_penetration() {
        let mut materials = Materials::default();
        materials.organ_penetration_impulse = 10.0;
        materials.organ_penetration_damage = 1.0;
        let mut world = World::new(materials);
        let organ = world.add_organ_region(
            OrganKind::Spleen,
            Vec2 { x: 120.0, y: 100.0 },
            Vec2 { x: 28.0, y: 20.0 },
        );

        world.collide_striker(
            world.materials.fixed_dt,
            &InputState {
                active: true,
                down: true,
                x: 120.0,
                y: 100.0,
                vx: 1200.0,
                vy: 0.0,
                power: 4.0,
                tool: ToolMode::Blunt,
            },
        );

        assert!(!world.organs[organ].penetrated);
        assert_eq!(world.stats.organ_penetrations, 0);
        assert_eq!(world.debug.organ_penetrations, 0);
        assert_eq!(world.stats.organ_ruptures, 0);
    }

    #[test]
    fn fractured_rib_tip_can_puncture_organ_proxy() {
        let mut materials = Materials::default();
        materials.rib_organ_puncture_impulse = 80.0;
        materials.rib_organ_puncture_damage = 1.0;
        materials.organ_rupture_damage = 0.65;
        materials.max_rib_organ_punctures_per_step = 1;
        materials.max_organ_ruptures_per_step = 1;
        materials.max_total_organ_ruptures = 1;
        let mut world = World::new(materials);

        let organ = world.add_organ_region(
            OrganKind::RightLung,
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 22.0, y: 18.0 },
        );
        let rib = world.add_bone_segment_with_kind(
            Vec2 { x: 68.0, y: 100.0 },
            Vec2 { x: 92.0, y: 100.0 },
            3.2,
            100.0,
            false,
            BoneKind::Rib,
        );
        world.bones[rib].fractured = true;
        world.bones[rib].broken_end = true;
        world.bones[rib].load = 420.0;

        world.process_fragment_tip(
            rib,
            Vec2 { x: 101.0, y: 100.0 },
            Vec2 { x: 74.0, y: 100.0 },
            Vec2 { x: 1.0, y: 0.0 },
            true,
        );

        assert!(world.organs[organ].penetrated);
        assert!(world.organs[organ].damage >= materials.organ_rupture_damage);
        assert!(world.organs[organ].ruptured);
        assert_eq!(world.stats.rib_organ_punctures, 1);
        assert_eq!(world.debug.rib_organ_punctures, 1);
        assert_eq!(world.stats.organ_ruptures, 1);
        assert!(
            world
                .wounds
                .iter()
                .any(|wound| wound.active && wound.layer == TissueLayer::Muscle),
            "rib organ puncture should reuse the persistent internal wound path"
        );
    }

    #[test]
    fn generic_fragment_tip_does_not_count_as_rib_organ_puncture() {
        let mut materials = Materials::default();
        materials.rib_organ_puncture_impulse = 80.0;
        materials.rib_organ_puncture_damage = 1.0;
        materials.organ_rupture_damage = 0.65;
        let mut world = World::new(materials);

        let organ = world.add_organ_region(
            OrganKind::RightLung,
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 22.0, y: 18.0 },
        );
        let fragment = world.add_bone_segment(
            Vec2 { x: 68.0, y: 100.0 },
            Vec2 { x: 92.0, y: 100.0 },
            3.2,
            100.0,
            false,
        );
        world.bones[fragment].fractured = true;
        world.bones[fragment].broken_end = true;
        world.bones[fragment].load = 420.0;

        world.process_fragment_tip(
            fragment,
            Vec2 { x: 101.0, y: 100.0 },
            Vec2 { x: 74.0, y: 100.0 },
            Vec2 { x: 1.0, y: 0.0 },
            true,
        );

        assert!(!world.organs[organ].penetrated);
        assert_eq!(world.stats.rib_organ_punctures, 0);
        assert_eq!(world.debug.rib_organ_punctures, 0);
    }

    #[test]
    fn fragment_work_budget_prioritizes_loaded_fragments() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 1;
        let mut world = World::new(materials);
        let quiet = world.add_bone_segment(
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 40.0, y: 0.0 },
            4.0,
            9999.0,
            false,
        );
        let loaded = world.add_bone_segment(
            Vec2 { x: 0.0, y: 20.0 },
            Vec2 { x: 40.0, y: 20.0 },
            4.0,
            9999.0,
            false,
        );
        let splinter = world.add_bone_segment(
            Vec2 { x: 0.0, y: 40.0 },
            Vec2 { x: 16.0, y: 40.0 },
            2.5,
            9999.0,
            false,
        );

        for index in [quiet, loaded, splinter] {
            world.bones[index].fractured = true;
        }
        world.bones[loaded].load = 900.0;
        world.bones[splinter].splinter = true;
        world.bones[splinter].load = 100.0;

        let budgeted = world.budgeted_fragment_indices();

        assert_eq!(budgeted, vec![loaded]);
        assert_eq!(world.debug.active_fragments, 3);
        assert_eq!(world.debug.fragment_budget_skips, 2);
    }

    #[test]
    fn fragment_pair_check_budget_stops_repulsion_work() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 4;
        materials.max_fragment_pair_checks = 1;
        let mut world = World::new(materials);
        for y in [100.0, 101.0, 102.0] {
            let bone = world.add_bone_segment(
                Vec2 { x: 100.0, y },
                Vec2 { x: 160.0, y },
                8.0,
                9999.0,
                false,
            );
            world.bones[bone].fractured = true;
        }

        world.solve_bone_fragment_repulsion();

        assert_eq!(world.debug.fragment_pair_checks, 1);
        assert!(
            world.debug.fragment_pair_budget_skips > 0,
            "pair budget should skip lower-priority checks after the cap"
        );
    }

    #[test]
    fn fragment_tissue_check_budget_stops_contact_work() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 1;
        materials.max_fragment_tissue_checks = 1;
        let mut world = World::new(materials);
        let bone = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 160.0, y: 100.0 },
            8.0,
            9999.0,
            false,
        );
        world.bones[bone].fractured = true;
        world.add_point(Vec2 { x: 120.0, y: 105.0 }, TissueLayer::Muscle, false);
        world.add_point(Vec2 { x: 130.0, y: 105.0 }, TissueLayer::Muscle, false);

        world.solve_bone_fragment_tissue_contacts();

        assert_eq!(world.debug.fragment_tissue_checks, 1);
        assert!(
            world.debug.fragment_tissue_budget_skips > 0,
            "tissue budget should skip remaining point checks after the cap"
        );
    }

    #[test]
    fn fragment_tissue_broadphase_excludes_distant_points() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 1;
        materials.max_fragment_tissue_checks = 32;
        let mut world = World::new(materials);
        let bone = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 160.0, y: 100.0 },
            8.0,
            9999.0,
            false,
        );
        world.bones[bone].fractured = true;
        world.add_point(Vec2 { x: 120.0, y: 105.0 }, TissueLayer::Muscle, false);
        world.add_point(Vec2 { x: 520.0, y: 520.0 }, TissueLayer::Muscle, false);

        world.solve_bone_fragment_tissue_contacts();

        assert_eq!(
            world.debug.fragment_tissue_checks, 1,
            "broad phase should only spend tissue checks on nearby point candidates"
        );
        assert_eq!(world.debug.fragment_tissue_budget_skips, 0);
    }

    #[test]
    fn moving_splinter_tip_punctures_intact_skin_spring() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.max_wound_sources = 0;
        materials.fragment_skin_puncture_impulse = 420.0;
        materials.max_fragment_skin_punctures_per_step = 2;
        let mut world = World::new(materials);
        let skin_a = world.add_point(Vec2 { x: 8.0, y: 0.0 }, TissueLayer::Skin, false);
        let skin_b = world.add_point(Vec2 { x: 12.0, y: 0.0 }, TissueLayer::Skin, false);
        world.add_spring(skin_a, skin_b, TissueLayer::Skin, 0.8, 1.7, 900.0, false);
        let bone = world.add_bone_segment(
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 10.0, y: 0.0 },
            4.0,
            9999.0,
            false,
        );
        world.bones[bone].fractured = true;
        world.bones[bone].splinter = true;
        world.bones[bone].broken_end = true;
        world.bones[bone].broken_end_normal = Vec2 { x: 0.0, y: 1.0 };
        world.bones[bone].load = 900.0;
        world.bones[bone].previous_b = Vec2 { x: 10.0, y: -6.0 };

        world.process_fragment_tip(
            bone,
            world.bones[bone].b,
            world.bones[bone].previous_b,
            world.bones[bone].broken_end_normal,
            true,
        );

        assert!(
            world.springs[0].broken,
            "a loaded splinter tip should puncture nearby intact skin"
        );
        assert_eq!(world.stats.broken_skin, 1);
        assert_eq!(world.stats.fragment_tissue_tears, 1);
        assert_eq!(world.stats.fragment_skin_punctures, 1);
        assert_eq!(world.debug.fragment_skin_punctures, 1);
    }

    #[test]
    fn quiet_splinter_tip_does_not_puncture_intact_skin_spring() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.max_wound_sources = 0;
        materials.fragment_skin_puncture_impulse = 900.0;
        materials.max_fragment_skin_punctures_per_step = 2;
        let mut world = World::new(materials);
        let skin_a = world.add_point(Vec2 { x: 8.0, y: 0.0 }, TissueLayer::Skin, false);
        let skin_b = world.add_point(Vec2 { x: 12.0, y: 0.0 }, TissueLayer::Skin, false);
        world.add_spring(skin_a, skin_b, TissueLayer::Skin, 0.8, 1.7, 900.0, false);
        let bone = world.add_bone_segment(
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 10.0, y: 0.0 },
            4.0,
            9999.0,
            false,
        );
        world.bones[bone].fractured = true;
        world.bones[bone].splinter = true;
        world.bones[bone].broken_end = true;
        world.bones[bone].broken_end_normal = Vec2 { x: 0.0, y: 1.0 };
        world.bones[bone].load = 80.0;
        world.bones[bone].previous_b = Vec2 { x: 10.0, y: -0.4 };

        world.process_fragment_tip(
            bone,
            world.bones[bone].b,
            world.bones[bone].previous_b,
            world.bones[bone].broken_end_normal,
            true,
        );

        assert!(
            !world.springs[0].broken,
            "a quiet splinter tip should not puncture intact skin by proximity alone"
        );
        assert_eq!(world.stats.fragment_skin_punctures, 0);
        assert_eq!(world.debug.fragment_skin_punctures, 0);
    }

    #[test]
    fn fragment_pair_broadphase_excludes_distant_pairs() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 4;
        materials.max_fragment_pair_checks = 32;
        let mut world = World::new(materials);
        for y in [100.0, 105.0, 320.0] {
            let bone = world.add_bone_segment(
                Vec2 { x: 100.0, y },
                Vec2 { x: 160.0, y },
                8.0,
                9999.0,
                false,
            );
            world.bones[bone].fractured = true;
        }

        world.solve_bone_fragment_repulsion();

        assert_eq!(
            world.debug.fragment_pair_checks, 1,
            "broad phase should only spend pair checks on nearby fragment candidates"
        );
        assert_eq!(world.debug.fragment_pair_budget_skips, 0);
    }

    #[test]
    fn fragment_pair_contact_damps_closing_velocity_and_spin() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 2;
        materials.max_fragment_pair_checks = 8;
        materials.fragment_repulsion_stiffness = 0.35;
        materials.fragment_pair_normal_damping = 0.72;
        materials.fragment_pair_tangential_friction = 0.24;
        materials.fragment_pair_angular_friction = 0.35;
        let mut world = World::new(materials);
        let upper = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 160.0, y: 100.0 },
            8.0,
            9999.0,
            false,
        );
        let lower = world.add_bone_segment(
            Vec2 { x: 100.0, y: 106.0 },
            Vec2 { x: 160.0, y: 106.0 },
            8.0,
            9999.0,
            false,
        );
        for index in [upper, lower] {
            world.bones[index].fractured = true;
        }
        world.bones[upper].previous_a = Vec2 { x: 100.0, y: 94.0 };
        world.bones[upper].previous_b = Vec2 { x: 160.0, y: 94.0 };
        world.bones[lower].previous_a = Vec2 { x: 100.0, y: 112.0 };
        world.bones[lower].previous_b = Vec2 { x: 160.0, y: 112.0 };
        world.bones[upper].angular_velocity = 8.0;
        world.bones[lower].angular_velocity = -7.0;
        let normal = Vec2 { x: 0.0, y: -1.0 };
        let before_closing = pair_closing_speed(world.bones[upper], world.bones[lower], normal);
        let before_spin =
            world.bones[upper].angular_velocity.abs() + world.bones[lower].angular_velocity.abs();

        world.solve_bone_fragment_repulsion();

        let after_closing = pair_closing_speed(world.bones[upper], world.bones[lower], normal);
        let after_spin =
            world.bones[upper].angular_velocity.abs() + world.bones[lower].angular_velocity.abs();
        assert_eq!(world.debug.fragment_pair_contacts, 1);
        assert_eq!(world.debug.fragment_pair_damping_events, 1);
        assert!(
            after_closing < before_closing * 0.82,
            "pair damping should reduce closing velocity through an overlap"
        );
        assert!(
            after_spin < before_spin,
            "pair contact should bleed angular jitter while fragments overlap"
        );
    }

    #[test]
    fn fragment_pair_resting_contact_adds_support_for_slow_overlap() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 2;
        materials.max_fragment_pair_checks = 8;
        materials.fragment_repulsion_stiffness = 0.0;
        materials.fragment_pair_normal_damping = 0.0;
        materials.fragment_pair_tangential_friction = 0.0;
        materials.fragment_pair_angular_friction = 0.0;
        materials.fragment_pair_rest_speed = 120.0;
        materials.fragment_pair_rest_stiffness = 0.65;
        materials.fragment_pair_rest_friction = 0.0;
        let mut world = World::new(materials);
        let upper = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 160.0, y: 100.0 },
            8.0,
            9999.0,
            false,
        );
        let lower = world.add_bone_segment(
            Vec2 { x: 100.0, y: 106.0 },
            Vec2 { x: 160.0, y: 106.0 },
            8.0,
            9999.0,
            false,
        );
        for index in [upper, lower] {
            world.bones[index].fractured = true;
        }
        world.bones[upper].previous_a = Vec2 { x: 100.0, y: 99.85 };
        world.bones[upper].previous_b = Vec2 { x: 160.0, y: 99.85 };
        world.bones[lower].previous_a = Vec2 {
            x: 100.0,
            y: 106.15,
        };
        world.bones[lower].previous_b = Vec2 {
            x: 160.0,
            y: 106.15,
        };
        let before_gap = distance(
            midpoint(world.bones[upper].a, world.bones[upper].b),
            midpoint(world.bones[lower].a, world.bones[lower].b),
        );

        world.solve_bone_fragment_repulsion();

        let after_gap = distance(
            midpoint(world.bones[upper].a, world.bones[upper].b),
            midpoint(world.bones[lower].a, world.bones[lower].b),
        );
        assert_eq!(world.debug.fragment_pair_contacts, 1);
        assert_eq!(world.debug.fragment_pair_resting_contacts, 1);
        assert!(
            after_gap > before_gap,
            "resting support should separate gently overlapping fragments even without impact repulsion"
        );
    }

    #[test]
    fn fast_fragment_pair_contact_is_not_treated_as_resting() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 2;
        materials.max_fragment_pair_checks = 8;
        materials.fragment_pair_rest_speed = 30.0;
        let mut world = World::new(materials);
        let upper = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 160.0, y: 100.0 },
            8.0,
            9999.0,
            false,
        );
        let lower = world.add_bone_segment(
            Vec2 { x: 100.0, y: 106.0 },
            Vec2 { x: 160.0, y: 106.0 },
            8.0,
            9999.0,
            false,
        );
        for index in [upper, lower] {
            world.bones[index].fractured = true;
        }
        world.bones[upper].previous_a = Vec2 { x: 100.0, y: 80.0 };
        world.bones[upper].previous_b = Vec2 { x: 160.0, y: 80.0 };
        world.bones[lower].previous_a = Vec2 { x: 100.0, y: 126.0 };
        world.bones[lower].previous_b = Vec2 { x: 160.0, y: 126.0 };

        world.solve_bone_fragment_repulsion();

        assert_eq!(world.debug.fragment_pair_contacts, 1);
        assert_eq!(
            world.debug.fragment_pair_resting_contacts, 0,
            "high-speed impacts should use impact damping without resting-contact support"
        );
    }

    #[test]
    fn fragment_bone_contact_pushes_fragment_and_loads_intact_bone() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 1;
        materials.max_fragment_bone_checks = 8;
        materials.fragment_repulsion_stiffness = 1.0;
        let mut world = World::new(materials);
        let support = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 170.0, y: 100.0 },
            8.0,
            9999.0,
            false,
        );
        let fragment = world.add_bone_segment(
            Vec2 { x: 112.0, y: 105.0 },
            Vec2 { x: 182.0, y: 105.0 },
            8.0,
            9999.0,
            false,
        );
        world.bones[fragment].fractured = true;
        world.bones[fragment].previous_a = Vec2 { x: 112.0, y: 97.0 };
        world.bones[fragment].previous_b = Vec2 { x: 182.0, y: 97.0 };

        world.solve_bone_fragment_bone_contacts();

        assert_eq!(world.debug.fragment_bone_checks, 1);
        assert_eq!(world.debug.fragment_bone_contacts, 1);
        assert!(
            world.bones[fragment].a.y > 105.0,
            "fragment should be pushed away from the intact bone"
        );
        assert!(
            world.bones[support].load > 0.0,
            "intact bone should receive contact load from the fragment"
        );
        assert!(world.debug.max_bone_load >= world.bones[support].load);
    }

    #[test]
    fn fragment_bone_resting_contact_supports_slow_jam() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 1;
        materials.max_fragment_bone_checks = 8;
        materials.fragment_repulsion_stiffness = 0.0;
        materials.fragment_bone_normal_damping = 0.0;
        materials.fragment_bone_tangential_friction = 0.0;
        materials.fragment_bone_angular_friction = 0.0;
        materials.fragment_bone_rest_speed = 120.0;
        materials.fragment_bone_rest_stiffness = 0.70;
        materials.fragment_bone_rest_friction = 0.0;
        let mut world = World::new(materials);
        let support = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 170.0, y: 100.0 },
            8.0,
            9999.0,
            true,
        );
        let fragment = world.add_bone_segment(
            Vec2 { x: 112.0, y: 105.0 },
            Vec2 { x: 182.0, y: 105.0 },
            8.0,
            9999.0,
            false,
        );
        world.bones[fragment].fractured = true;
        world.bones[fragment].previous_a = Vec2 { x: 112.0, y: 105.1 };
        world.bones[fragment].previous_b = Vec2 { x: 182.0, y: 105.1 };
        let before_y = world.bones[fragment].a.y;

        world.solve_bone_fragment_bone_contacts();

        assert_eq!(world.debug.fragment_bone_contacts, 1);
        assert_eq!(world.debug.fragment_bone_resting_contacts, 1);
        assert!(
            world.bones[fragment].a.y > before_y,
            "resting fragment-bone support should push a slow jammed fragment away from the support bone"
        );
        assert_eq!(
            world.bones[support].a.y, 100.0,
            "pinned support bone should remain fixed while supporting debris"
        );
    }

    #[test]
    fn fragment_bone_resting_contact_skin_supports_slow_near_jam() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 1;
        materials.max_fragment_bone_checks = 8;
        materials.fragment_repulsion_stiffness = 0.0;
        materials.fragment_bone_normal_damping = 0.0;
        materials.fragment_bone_tangential_friction = 0.0;
        materials.fragment_bone_angular_friction = 0.0;
        materials.fragment_bone_rest_speed = 120.0;
        materials.fragment_bone_rest_stiffness = 0.70;
        materials.fragment_bone_rest_friction = 0.0;
        let mut world = World::new(materials);
        world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 170.0, y: 100.0 },
            8.0,
            9999.0,
            true,
        );
        let fragment = world.add_bone_segment(
            Vec2 {
                x: 112.0,
                y: 116.95,
            },
            Vec2 {
                x: 182.0,
                y: 116.95,
            },
            8.0,
            9999.0,
            false,
        );
        world.bones[fragment].fractured = true;
        world.bones[fragment].previous_a = Vec2 {
            x: 112.0,
            y: 116.90,
        };
        world.bones[fragment].previous_b = Vec2 {
            x: 182.0,
            y: 116.90,
        };
        let before_y = world.bones[fragment].a.y;

        world.solve_bone_fragment_bone_contacts();

        assert_eq!(world.debug.fragment_bone_contacts, 1);
        assert_eq!(world.debug.fragment_bone_resting_contacts, 1);
        assert!(
            world.bones[fragment].a.y > before_y,
            "resting fragment-bone contact skin should support a slow near-contact fragment"
        );
    }

    #[test]
    fn fast_fragment_bone_contact_is_not_treated_as_resting() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 1;
        materials.max_fragment_bone_checks = 8;
        materials.fragment_bone_rest_speed = 30.0;
        let mut world = World::new(materials);
        world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 170.0, y: 100.0 },
            8.0,
            9999.0,
            true,
        );
        let fragment = world.add_bone_segment(
            Vec2 { x: 112.0, y: 105.0 },
            Vec2 { x: 182.0, y: 105.0 },
            8.0,
            9999.0,
            false,
        );
        world.bones[fragment].fractured = true;
        world.bones[fragment].previous_a = Vec2 { x: 112.0, y: 125.0 };
        world.bones[fragment].previous_b = Vec2 { x: 182.0, y: 125.0 };

        world.solve_bone_fragment_bone_contacts();

        assert_eq!(world.debug.fragment_bone_contacts, 1);
        assert_eq!(
            world.debug.fragment_bone_resting_contacts, 0,
            "high-speed fragment-bone impacts should not get resting-contact support"
        );
    }

    #[test]
    fn fragment_bone_check_budget_stops_contact_work() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 1;
        materials.max_fragment_bone_checks = 1;
        let mut world = World::new(materials);
        let fragment = world.add_bone_segment(
            Vec2 { x: 100.0, y: 105.0 },
            Vec2 { x: 160.0, y: 105.0 },
            8.0,
            9999.0,
            false,
        );
        world.bones[fragment].fractured = true;
        for y in [100.0, 110.0] {
            world.add_bone_segment(
                Vec2 { x: 100.0, y },
                Vec2 { x: 160.0, y },
                8.0,
                9999.0,
                false,
            );
        }

        world.solve_bone_fragment_bone_contacts();

        assert_eq!(world.debug.fragment_bone_checks, 1);
        assert!(
            world.debug.fragment_bone_budget_skips > 0,
            "bone-contact budget should skip remaining intact-bone checks after the cap"
        );
    }

    #[test]
    fn fragment_bone_broadphase_excludes_distant_bones() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 1;
        materials.max_fragment_bone_checks = 32;
        let mut world = World::new(materials);
        let fragment = world.add_bone_segment(
            Vec2 { x: 100.0, y: 105.0 },
            Vec2 { x: 160.0, y: 105.0 },
            8.0,
            9999.0,
            false,
        );
        world.bones[fragment].fractured = true;
        world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 160.0, y: 100.0 },
            8.0,
            9999.0,
            false,
        );
        world.add_bone_segment(
            Vec2 { x: 520.0, y: 520.0 },
            Vec2 { x: 580.0, y: 520.0 },
            8.0,
            9999.0,
            false,
        );

        world.solve_bone_fragment_bone_contacts();

        assert_eq!(
            world.debug.fragment_bone_checks, 1,
            "broad phase should only spend bone checks on nearby intact bones"
        );
        assert_eq!(world.debug.fragment_bone_budget_skips, 0);
    }

    #[test]
    fn fragment_floor_contact_uses_radius_and_damps_slide() {
        let mut materials = Materials::default();
        materials.fragment_floor_friction = 0.60;
        materials.fragment_floor_normal_damping = 0.90;
        materials.fragment_floor_angular_friction = 0.30;
        materials.fragment_floor_rest_speed = 1.0;
        let mut world = World::new(materials);
        let bone = world.add_bone_segment(
            Vec2 { x: 100.0, y: 116.0 },
            Vec2 { x: 160.0, y: 110.0 },
            8.0,
            9999.0,
            false,
        );
        world.bones[bone].fractured = true;
        world.bones[bone].previous_a = Vec2 { x: 90.0, y: 106.0 };
        world.bones[bone].previous_b = world.bones[bone].b;
        world.bones[bone].angular_velocity = 10.0;

        world.constrain_to_world(300.0, 120.0);

        assert_eq!(world.debug.fragment_floor_contacts, 1);
        assert_eq!(world.debug.fragment_floor_resting_contacts, 0);
        assert!(
            world.bones[bone].a.y <= 112.0,
            "fragment radius should keep the bone center above the floor plane"
        );
        let remaining_x_velocity = world.bones[bone].a.x - world.bones[bone].previous_a.x;
        let remaining_y_velocity = world.bones[bone].a.y - world.bones[bone].previous_a.y;
        assert!(
            remaining_x_velocity < 5.0,
            "floor contact should apply tangential friction to sliding fragments"
        );
        assert!(
            remaining_y_velocity < 2.0,
            "floor contact should damp downward fragment velocity"
        );
        assert!(
            world.bones[bone].angular_velocity.abs() < 10.0,
            "floor contact should bleed angular jitter"
        );
    }

    #[test]
    fn slow_fragment_floor_contact_is_counted_as_resting() {
        let mut materials = Materials::default();
        materials.fragment_floor_rest_speed = 30.0;
        let mut world = World::new(materials);
        let bone = world.add_bone_segment(
            Vec2 { x: 100.0, y: 112.4 },
            Vec2 { x: 160.0, y: 110.0 },
            8.0,
            9999.0,
            false,
        );
        world.bones[bone].fractured = true;
        world.bones[bone].previous_a = Vec2 { x: 100.0, y: 112.2 };
        world.bones[bone].previous_b = world.bones[bone].b;

        world.constrain_to_world(300.0, 120.0);

        assert_eq!(world.debug.fragment_floor_contacts, 1);
        assert_eq!(world.debug.fragment_floor_resting_contacts, 1);
        assert!(
            (world.bones[bone].a.y - world.bones[bone].previous_a.y).abs() <= EPSILON,
            "resting floor contact should remove tiny downward velocity"
        );
    }

    #[test]
    fn quiet_fragment_sleeps_and_leaves_active_budget() {
        let mut materials = Materials::default();
        materials.fragment_sleep_frames = 2;
        materials.fragment_sleep_speed = 4.0;
        materials.fragment_sleep_angular_speed = 0.04;
        materials.fragment_sleep_load = 10.0;
        let mut world = World::new(materials);
        let bone = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 160.0, y: 100.0 },
            6.0,
            9999.0,
            false,
        );
        world.bones[bone].fractured = true;
        world.bones[bone].previous_a = world.bones[bone].a;
        world.bones[bone].previous_b = world.bones[bone].b;

        world.update_fragment_sleep_states();
        assert!(
            !world.bones[bone].sleeping,
            "fragment should wait for the configured quiet frame count"
        );
        world.update_fragment_sleep_states();

        assert!(world.bones[bone].sleeping);
        assert_eq!(world.debug.fragment_sleep_events, 1);
        assert_eq!(world.sleeping_fragment_count(), 1);
        assert!(
            world.budgeted_fragment_indices().is_empty(),
            "sleeping debris should not consume active fragment budget"
        );
    }

    #[test]
    fn direct_striker_contact_wakes_sleeping_fragment() {
        let mut materials = Materials::default();
        materials.gravity = 0.0;
        materials.fragment_wake_load = 20.0;
        let mut world = World::new(materials);
        let bone = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 160.0, y: 100.0 },
            6.0,
            999_999.0,
            false,
        );
        world.bones[bone].fractured = true;
        world.bones[bone].sleeping = true;
        world.bones[bone].previous_a = world.bones[bone].a;
        world.bones[bone].previous_b = world.bones[bone].b;

        world.collide_striker(
            world.materials.fixed_dt,
            &InputState {
                active: true,
                down: true,
                x: 130.0,
                y: 100.0,
                vx: 900.0,
                vy: 0.0,
                power: 2.0,
                tool: ToolMode::Blunt,
            },
        );

        assert!(
            !world.bones[bone].sleeping,
            "meaningful direct contact should wake a sleeping fragment"
        );
        assert_eq!(world.debug.fragment_wake_events, 1);
        assert!(world.debug.bone_contacts > 0);
    }

    #[test]
    fn blunt_tissue_contact_creates_contusion_without_tearing() {
        let mut materials = Materials::default();
        materials.gravity = 0.0;
        materials.contusion_load_threshold = 120.0;
        let mut world = World::new(materials);
        world.add_point(Vec2 { x: 130.0, y: 100.0 }, TissueLayer::Skin, false);

        world.collide_striker(
            world.materials.fixed_dt,
            &InputState {
                active: true,
                down: true,
                x: 130.0,
                y: 100.0,
                vx: 900.0,
                vy: 0.0,
                power: 2.0,
                tool: ToolMode::Blunt,
            },
        );

        assert!(
            world.points[0].contusion > 0.0,
            "blunt load should leave a persistent tissue contusion"
        );
        assert_eq!(world.stats.broken_skin, 0);
        assert_eq!(world.stats.contusion_events, 1);
        assert_eq!(world.debug.contusion_events, 1);
    }

    #[test]
    fn contused_spring_tears_at_lower_load() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.max_wound_sources = 0;
        materials.contusion_tear_weakening = 0.50;
        materials.contusion_stiffness_softening = 0.35;
        let mut world = World::new(materials);
        world.debug.tool = ToolMode::Sharp;
        world.debug.impact = 1200.0;
        let a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Skin, false);
        let b = world.add_point(Vec2 { x: 100.0, y: 0.0 }, TissueLayer::Skin, false);
        world.add_spring(a, b, TissueLayer::Skin, 0.9, 1.68, 1000.0, false);
        world.points[a].contusion = 1.0;
        world.points[b].contusion = 1.0;
        world.points[a].load = 700.0;
        world.points[b].load = 700.0;
        world.points[b].position.x = 112.0;

        world.solve_springs();

        assert!(
            world.springs[0].broken,
            "contusion should lower the load/stretch needed for a damaged spring to tear"
        );
        assert_eq!(world.stats.broken_skin, 1);
        assert!(
            world.debug.max_tissue_softening > 0.0,
            "spring solve should report local contusion-driven softening"
        );
    }

    #[test]
    fn repeated_subcritical_load_fatigues_spring_until_it_tears() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.max_wound_sources = 0;
        materials.tissue_fatigue_stretch_threshold = 1.05;
        materials.tissue_fatigue_load_threshold = 500.0;
        materials.tissue_fatigue_rate = 0.08;
        materials.tissue_fatigue_decay = 0.0;
        materials.tissue_fatigue_tear_weakening = 0.45;
        materials.tissue_fatigue_stiffness_softening = 0.12;
        let mut world = World::new(materials);
        let a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Skin, true);
        let b = world.add_point(Vec2 { x: 100.0, y: 0.0 }, TissueLayer::Skin, true);
        world.add_spring(a, b, TissueLayer::Skin, 0.9, 1.36, 1000.0, false);
        world.points[a].load = 760.0;
        world.points[b].load = 760.0;
        world.points[b].position.x = 121.0;
        world.points[b].previous.x = 121.0;

        world.solve_springs();
        assert!(
            !world.springs[0].broken,
            "a single subcritical load should accumulate fatigue without immediately tearing"
        );
        assert!(world.springs[0].fatigue > 0.0);

        for _ in 0..24 {
            if world.springs[0].broken {
                break;
            }
            world.solve_springs();
        }

        assert!(
            world.springs[0].broken,
            "repeated subcritical load should become a tear once local fatigue weakens the spring"
        );
        assert_eq!(world.stats.broken_skin, 1);
        assert!(
            world.stats.tissue_fatigue_events > 0,
            "fatigue accumulation should be visible in cumulative telemetry"
        );
        assert!(
            world.debug.max_tissue_fatigue > 0.5,
            "spring solve should report the local fatigue that drove the tear"
        );
    }

    #[test]
    fn spring_fatigue_contributes_to_muscle_triangle_damage_detail() {
        let mut world = World::new(Materials::default());
        let a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Muscle, false);
        let b = world.add_point(Vec2 { x: 12.0, y: 0.0 }, TissueLayer::Muscle, false);
        let c = world.add_point(Vec2 { x: 0.0, y: 12.0 }, TissueLayer::Muscle, false);
        world.add_spring(a, b, TissueLayer::Muscle, 0.8, 1.9, 1100.0, true);
        world.add_spring(b, c, TissueLayer::Muscle, 0.8, 1.9, 1100.0, true);
        world.add_spring(c, a, TissueLayer::Muscle, 0.8, 1.9, 1100.0, true);
        world.add_triangle(a, b, c, TissueLayer::Muscle);
        world.springs[0].fatigue = 0.80;

        world.update_triangle_damage();

        assert!(
            world.triangles[0].damage >= 0.30,
            "muscle detail should expose persistent spring fatigue before catastrophic failure"
        );
        assert!(
            world.triangle_alive(&world.triangles[0]),
            "fatigue detail should not immediately fail the whole muscle triangle"
        );
    }

    #[test]
    fn fiber_aligned_muscle_tear_reports_and_marks_local_detail() {
        let mut materials = Materials::default();
        materials.tissue_fatigue_rate = 0.0;
        materials.muscle_fiber_rupture_damage_floor = 0.62;
        let mut world = World::new(materials);
        let a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Muscle, false);
        let b = world.add_point(Vec2 { x: 20.0, y: 0.0 }, TissueLayer::Muscle, false);
        let c = world.add_point(Vec2 { x: 0.0, y: 20.0 }, TissueLayer::Muscle, false);
        world.add_spring(a, b, TissueLayer::Muscle, 0.8, 1.18, 9999.0, true);
        world.add_spring(b, c, TissueLayer::Muscle, 0.8, 9.0, 9999.0, false);
        world.add_spring(c, a, TissueLayer::Muscle, 0.8, 9.0, 9999.0, false);
        world.add_triangle(a, b, c, TissueLayer::Muscle);

        world.points[b].position.x = 28.0;
        world.solve_springs();
        world.update_triangle_damage();

        assert!(world.springs[0].broken);
        assert_eq!(world.stats.broken_muscle, 1);
        assert_eq!(world.stats.muscle_fiber_tears, 1);
        assert_eq!(world.debug.muscle_fiber_tears, 1);
        assert!(
            world.triangles[0].damage >= 0.62,
            "broken fiber-aligned muscle springs should feed local muscle detail"
        );
    }

    #[test]
    fn cross_fiber_muscle_tear_does_not_count_as_fiber_rupture() {
        let mut materials = Materials::default();
        materials.tissue_fatigue_rate = 0.0;
        materials.muscle_fiber_rupture_damage_floor = 0.62;
        let mut world = World::new(materials);
        let a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Muscle, false);
        let b = world.add_point(Vec2 { x: 20.0, y: 0.0 }, TissueLayer::Muscle, false);
        let c = world.add_point(Vec2 { x: 0.0, y: 20.0 }, TissueLayer::Muscle, false);
        world.add_spring(a, b, TissueLayer::Muscle, 0.8, 1.18, 9999.0, false);
        world.add_spring(b, c, TissueLayer::Muscle, 0.8, 9.0, 9999.0, false);
        world.add_spring(c, a, TissueLayer::Muscle, 0.8, 9.0, 9999.0, false);
        world.add_triangle(a, b, c, TissueLayer::Muscle);

        world.points[b].position.x = 28.0;
        world.solve_springs();
        world.update_triangle_damage();

        assert!(world.springs[0].broken);
        assert_eq!(world.stats.broken_muscle, 1);
        assert_eq!(world.stats.muscle_fiber_tears, 0);
        assert_eq!(world.debug.muscle_fiber_tears, 0);
        assert!(
            world.triangles[0].damage < materials.muscle_fiber_rupture_damage_floor,
            "ordinary cross-fiber muscle tears should not get fiber rupture detail"
        );
    }

    #[test]
    fn loaded_muscle_triangle_failure_creates_crush_rupture_bleeding() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 32;
        materials.max_wound_sources = 8;
        materials.muscle_exposed_tear_impulse = 120.0;
        materials.muscle_crush_rupture_load_threshold = 180.0;
        materials.muscle_crush_rupture_damage_threshold = 0.90;
        materials.max_muscle_crush_ruptures_per_step = 2;
        let mut world = World::new(materials);
        let a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Muscle, false);
        let b = world.add_point(Vec2 { x: 12.0, y: 0.0 }, TissueLayer::Muscle, false);
        let c = world.add_point(Vec2 { x: 0.0, y: 12.0 }, TissueLayer::Muscle, false);
        world.add_triangle(a, b, c, TissueLayer::Muscle);
        for point in [a, b, c] {
            world.points[point].load = 1900.0;
            world.points[point].exposure = 1.0;
        }

        world.update_triangle_damage();

        assert!(world.triangles[0].failed);
        assert_eq!(world.stats.muscle_crush_ruptures, 1);
        assert_eq!(world.debug.muscle_crush_ruptures, 1);
        assert!(world.stats.emitted_fluid_particles > 0);
        assert_eq!(world.stats.opened_wounds, 1);
    }

    #[test]
    fn quiet_muscle_triangle_damage_does_not_create_crush_rupture() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 32;
        materials.max_wound_sources = 8;
        materials.muscle_exposed_tear_impulse = 900.0;
        materials.muscle_crush_rupture_load_threshold = 600.0;
        let mut world = World::new(materials);
        let a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Muscle, false);
        let b = world.add_point(Vec2 { x: 12.0, y: 0.0 }, TissueLayer::Muscle, false);
        let c = world.add_point(Vec2 { x: 0.0, y: 12.0 }, TissueLayer::Muscle, false);
        world.add_triangle(a, b, c, TissueLayer::Muscle);
        for point in [a, b, c] {
            world.points[point].load = 420.0;
            world.points[point].exposure = 0.10;
        }

        world.update_triangle_damage();

        assert!(!world.triangles[0].failed);
        assert_eq!(world.stats.muscle_crush_ruptures, 0);
        assert_eq!(world.debug.muscle_crush_ruptures, 0);
        assert_eq!(world.stats.emitted_fluid_particles, 0);
        assert_eq!(world.stats.opened_wounds, 0);
    }

    #[test]
    fn sharp_deep_contact_lacerates_major_vessel_and_opens_pressure_wound() {
        let mut materials = Materials::default();
        materials.gravity = 0.0;
        materials.max_fluid_particles = 32;
        materials.max_wound_sources = 8;
        materials.major_vessel_laceration_impulse = 240.0;
        materials.major_vessel_cut_radius = 8.0;
        let mut world = World::new(materials);
        let anchor_a = world.add_point(Vec2 { x: 48.0, y: -14.0 }, TissueLayer::Muscle, false);
        let anchor_b = world.add_point(Vec2 { x: 48.0, y: 14.0 }, TissueLayer::Muscle, false);
        world.add_vessel_segment(
            Vec2 { x: 48.0, y: -14.0 },
            Vec2 { x: 48.0, y: 14.0 },
            3.0,
            1.55,
        );
        world.points[anchor_a].exposure = 0.75;
        world.points[anchor_b].exposure = 0.75;

        world.collide_striker(
            world.materials.fixed_dt,
            &InputState {
                active: true,
                down: true,
                x: 44.0,
                y: 0.0,
                vx: 900.0,
                vy: 0.0,
                power: 3.0,
                tool: ToolMode::Sharp,
            },
        );

        assert!(world.vessels[0].lacerated);
        assert_eq!(world.stats.vessel_lacerations, 1);
        assert_eq!(world.debug.vessel_lacerations, 1);
        assert_eq!(world.stats.opened_wounds, 1);
        assert!(
            world.wounds[0].pressure > 2.0,
            "major vessel damage should open a stronger pressure wound than ordinary tissue tears"
        );
    }

    #[test]
    fn quiet_blunt_contact_does_not_lacerate_major_vessel_by_proximity() {
        let mut materials = Materials::default();
        materials.gravity = 0.0;
        materials.max_fluid_particles = 32;
        materials.max_wound_sources = 8;
        materials.major_vessel_laceration_impulse = 900.0;
        materials.major_vessel_cut_radius = 8.0;
        let mut world = World::new(materials);
        world.add_point(Vec2 { x: 48.0, y: -14.0 }, TissueLayer::Muscle, false);
        world.add_point(Vec2 { x: 48.0, y: 14.0 }, TissueLayer::Muscle, false);
        world.add_vessel_segment(
            Vec2 { x: 48.0, y: -14.0 },
            Vec2 { x: 48.0, y: 14.0 },
            3.0,
            1.55,
        );

        world.collide_striker(
            world.materials.fixed_dt,
            &InputState {
                active: true,
                down: true,
                x: 44.0,
                y: 0.0,
                vx: 180.0,
                vy: 0.0,
                power: 1.0,
                tool: ToolMode::Blunt,
            },
        );

        assert!(!world.vessels[0].lacerated);
        assert_eq!(world.stats.vessel_lacerations, 0);
        assert_eq!(world.debug.vessel_lacerations, 0);
        assert_eq!(world.stats.opened_wounds, 0);
    }

    #[test]
    fn fractured_fragment_tip_can_lacerate_major_vessel() {
        let mut materials = Materials::default();
        materials.fragment_vessel_laceration_impulse = 90.0;
        materials.fragment_vessel_laceration_radius = 8.0;
        materials.max_fragment_vessel_lacerations_per_step = 1;
        let mut world = World::new(materials);

        world.add_vessel_segment(
            Vec2 { x: 100.0, y: 82.0 },
            Vec2 { x: 100.0, y: 118.0 },
            3.0,
            1.6,
        );
        let fragment = world.add_bone_segment(
            Vec2 { x: 68.0, y: 100.0 },
            Vec2 { x: 92.0, y: 100.0 },
            3.2,
            100.0,
            false,
        );
        world.bones[fragment].fractured = true;
        world.bones[fragment].broken_end = true;
        world.bones[fragment].load = 420.0;

        world.process_fragment_tip(
            fragment,
            Vec2 { x: 101.0, y: 100.0 },
            Vec2 { x: 74.0, y: 100.0 },
            Vec2 { x: 1.0, y: 0.0 },
            true,
        );

        assert!(world.vessels[0].lacerated);
        assert_eq!(world.stats.vessel_lacerations, 1);
        assert_eq!(world.stats.fragment_vessel_lacerations, 1);
        assert_eq!(world.debug.fragment_vessel_lacerations, 1);
        assert!(
            world
                .wounds
                .iter()
                .any(|wound| wound.active && wound.pressure > 1.5),
            "fragment-driven vessel laceration should reuse the persistent pressure wound path"
        );
    }

    #[test]
    fn quiet_fragment_tip_does_not_lacerate_major_vessel_by_proximity() {
        let mut materials = Materials::default();
        materials.fragment_vessel_laceration_impulse = 900.0;
        materials.fragment_vessel_laceration_radius = 8.0;
        let mut world = World::new(materials);

        world.add_vessel_segment(
            Vec2 { x: 100.0, y: 82.0 },
            Vec2 { x: 100.0, y: 118.0 },
            3.0,
            1.6,
        );
        let fragment = world.add_bone_segment(
            Vec2 { x: 68.0, y: 100.0 },
            Vec2 { x: 92.0, y: 100.0 },
            3.2,
            100.0,
            false,
        );
        world.bones[fragment].fractured = true;
        world.bones[fragment].broken_end = true;
        world.bones[fragment].load = 20.0;

        world.process_fragment_tip(
            fragment,
            Vec2 { x: 96.0, y: 100.0 },
            Vec2 { x: 95.4, y: 100.0 },
            Vec2 { x: 1.0, y: 0.0 },
            true,
        );

        assert!(!world.vessels[0].lacerated);
        assert_eq!(world.stats.vessel_lacerations, 0);
        assert_eq!(world.stats.fragment_vessel_lacerations, 0);
        assert_eq!(world.debug.fragment_vessel_lacerations, 0);
    }

    #[test]
    fn repeated_subcritical_stretch_plastically_lengthens_spring_rest_shape() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.max_wound_sources = 0;
        materials.tissue_fatigue_rate = 0.0;
        materials.tissue_plastic_stretch_threshold = 1.04;
        materials.tissue_plastic_rate = 0.08;
        materials.tissue_plastic_limit = 0.35;
        let mut world = World::new(materials);
        let a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Skin, true);
        let b = world.add_point(Vec2 { x: 100.0, y: 0.0 }, TissueLayer::Skin, true);
        world.add_spring(a, b, TissueLayer::Skin, 0.9, 2.20, 5000.0, false);
        let initial_rest = world.springs[0].rest;
        world.points[a].load = 900.0;
        world.points[b].load = 900.0;
        world.points[b].position.x = 138.0;

        world.solve_springs();

        assert!(
            !world.springs[0].broken,
            "subcritical plastic stretching should not require immediate rupture"
        );
        assert!(
            world.springs[0].rest > initial_rest,
            "loaded overstretch should lengthen the spring rest shape"
        );
        assert!(world.springs[0].plastic_strain > 0.0);
        assert_eq!(world.stats.tissue_plastic_events, 1);
        assert!(world.debug.max_tissue_plasticity > 0.0);
    }

    #[test]
    fn loaded_crush_plastically_shortens_spring_rest_shape() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.max_wound_sources = 0;
        materials.tissue_fatigue_rate = 0.0;
        materials.tissue_plastic_compression_threshold = 0.86;
        materials.tissue_plastic_rate = 0.10;
        materials.tissue_plastic_limit = 0.35;
        let mut world = World::new(materials);
        let a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Muscle, true);
        let b = world.add_point(Vec2 { x: 100.0, y: 0.0 }, TissueLayer::Muscle, true);
        world.add_spring(a, b, TissueLayer::Muscle, 0.9, 2.20, 5000.0, true);
        let initial_rest = world.springs[0].rest;
        world.points[a].load = 900.0;
        world.points[b].load = 900.0;
        world.points[b].position.x = 62.0;

        world.solve_springs();

        assert!(
            world.springs[0].rest < initial_rest,
            "loaded compression should shorten the spring rest shape"
        );
        assert!(world.springs[0].plastic_strain > 0.0);
        assert_eq!(world.stats.tissue_plastic_events, 1);
        assert!(world.debug.max_tissue_plasticity > 0.0);
    }

    #[test]
    fn stressed_skin_spring_propagates_from_existing_wound_edge() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.max_wound_sources = 0;
        materials.tear_propagation_stress_threshold = 0.20;
        materials.tear_propagation_fatigue_threshold = 0.60;
        materials.tear_propagation_load_threshold = 5000.0;
        materials.max_tear_propagations_per_step = 4;
        let mut world = World::new(materials);
        world.debug.tool = ToolMode::Sharp;
        world.debug.impact = 1200.0;
        let a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Skin, false);
        let b = world.add_point(Vec2 { x: 10.0, y: 0.0 }, TissueLayer::Skin, false);
        let c = world.add_point(Vec2 { x: 20.0, y: 0.0 }, TissueLayer::Skin, false);
        world.add_spring(a, b, TissueLayer::Skin, 0.8, 1.7, 1000.0, false);
        world.add_spring(b, c, TissueLayer::Skin, 0.8, 1.7, 1000.0, false);
        world.springs[0].broken = true;
        world.springs[1].stress = 0.34;

        world.propagate_skin_tears();

        assert!(
            world.springs[1].broken,
            "a stressed skin edge adjacent to an existing wound should tear by propagation"
        );
        assert_eq!(world.stats.broken_skin, 1);
        assert_eq!(world.stats.tear_propagations, 1);
        assert_eq!(world.debug.tear_propagations, 1);
    }

    #[test]
    fn quiet_skin_spring_does_not_propagate_from_existing_wound_edge() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.max_wound_sources = 0;
        materials.tear_propagation_stress_threshold = 0.60;
        materials.tear_propagation_fatigue_threshold = 0.60;
        materials.tear_propagation_load_threshold = 5000.0;
        materials.max_tear_propagations_per_step = 4;
        let mut world = World::new(materials);
        let a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Skin, false);
        let b = world.add_point(Vec2 { x: 10.0, y: 0.0 }, TissueLayer::Skin, false);
        let c = world.add_point(Vec2 { x: 20.0, y: 0.0 }, TissueLayer::Skin, false);
        world.add_spring(a, b, TissueLayer::Skin, 0.8, 1.7, 1000.0, false);
        world.add_spring(b, c, TissueLayer::Skin, 0.8, 1.7, 1000.0, false);
        world.springs[0].broken = true;
        world.springs[1].stress = 0.12;

        world.propagate_skin_tears();

        assert!(
            !world.springs[1].broken,
            "tear propagation should require local stress, fatigue, or load"
        );
        assert_eq!(world.stats.tear_propagations, 0);
        assert_eq!(world.debug.tear_propagations, 0);
    }

    #[test]
    fn sharp_skin_opening_transfers_cut_into_exposed_muscle() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.max_wound_sources = 0;
        materials.muscle_cut_transfer_exposure_threshold = 0.40;
        materials.muscle_cut_transfer_load_threshold = 800.0;
        materials.muscle_cut_transfer_radius = 16.0;
        materials.max_muscle_cut_transfers_per_step = 2;
        let mut world = World::new(materials);
        world.debug.tool = ToolMode::Sharp;
        world.debug.impact = 1200.0;
        let skin_a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Skin, false);
        let skin_b = world.add_point(Vec2 { x: 12.0, y: 0.0 }, TissueLayer::Skin, false);
        let muscle_a = world.add_point(Vec2 { x: 0.0, y: 5.0 }, TissueLayer::Muscle, false);
        let muscle_b = world.add_point(Vec2 { x: 12.0, y: 5.0 }, TissueLayer::Muscle, false);
        world.add_spring(skin_a, skin_b, TissueLayer::Skin, 0.8, 1.7, 1000.0, false);
        world.add_spring(
            muscle_a,
            muscle_b,
            TissueLayer::Muscle,
            0.8,
            1.9,
            1200.0,
            true,
        );
        world.springs[0].broken = true;
        world.points[muscle_a].exposure = 0.62;
        world.points[muscle_b].exposure = 0.62;
        world.points[muscle_a].load = 520.0;
        world.points[muscle_b].load = 520.0;

        world.transfer_sharp_cut_to_exposed_muscle();

        assert!(
            world.springs[1].broken,
            "open stressed skin should transfer a sharp cut into nearby exposed muscle"
        );
        assert_eq!(world.stats.broken_muscle, 1);
        assert_eq!(world.stats.muscle_cut_transfers, 1);
        assert_eq!(world.debug.muscle_cut_transfers, 1);
    }

    #[test]
    fn quiet_muscle_does_not_get_cut_by_nearby_skin_opening() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.max_wound_sources = 0;
        materials.muscle_cut_transfer_exposure_threshold = 0.70;
        materials.muscle_cut_transfer_load_threshold = 1200.0;
        materials.muscle_cut_transfer_radius = 16.0;
        materials.max_muscle_cut_transfers_per_step = 2;
        let mut world = World::new(materials);
        world.debug.tool = ToolMode::Sharp;
        world.debug.impact = 1200.0;
        let skin_a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Skin, false);
        let skin_b = world.add_point(Vec2 { x: 12.0, y: 0.0 }, TissueLayer::Skin, false);
        let muscle_a = world.add_point(Vec2 { x: 0.0, y: 5.0 }, TissueLayer::Muscle, false);
        let muscle_b = world.add_point(Vec2 { x: 12.0, y: 5.0 }, TissueLayer::Muscle, false);
        world.add_spring(skin_a, skin_b, TissueLayer::Skin, 0.8, 1.7, 1000.0, false);
        world.add_spring(
            muscle_a,
            muscle_b,
            TissueLayer::Muscle,
            0.8,
            1.9,
            1200.0,
            true,
        );
        world.springs[0].broken = true;
        world.points[muscle_a].exposure = 0.15;
        world.points[muscle_b].exposure = 0.15;
        world.points[muscle_a].load = 120.0;
        world.points[muscle_b].load = 120.0;

        world.transfer_sharp_cut_to_exposed_muscle();

        assert!(
            !world.springs[1].broken,
            "nearby quiet muscle should require exposure, load, or fatigue before cut transfer"
        );
        assert_eq!(world.stats.muscle_cut_transfers, 0);
        assert_eq!(world.debug.muscle_cut_transfers, 0);
    }

    #[test]
    fn loaded_skin_cut_edge_delaminates_nearby_attachment() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.max_wound_sources = 0;
        materials.skin_flap_load_threshold = 120.0;
        materials.skin_flap_stress_threshold = 0.70;
        materials.skin_flap_cut_radius = 18.0;
        materials.max_skin_flap_detachments_per_step = 2;
        let mut world = World::new(materials);
        let skin_a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Skin, false);
        let skin_b = world.add_point(Vec2 { x: 12.0, y: 0.0 }, TissueLayer::Skin, false);
        let muscle = world.add_point(Vec2 { x: 0.0, y: 7.0 }, TissueLayer::Muscle, false);
        world.debug.tool = ToolMode::Sharp;
        world.debug.impact = 900.0;
        world.add_spring(skin_a, skin_b, TissueLayer::Skin, 0.8, 1.7, 1000.0, false);
        world.add_attachment(skin_a, muscle);
        world.springs[0].broken = true;
        world.points[skin_a].load = 180.0;
        world.points[skin_b].load = 180.0;
        world.points[skin_a].exposure = 0.85;
        world.points[skin_b].exposure = 0.85;

        world.delaminate_skin_flaps_from_cut_edges();

        assert!(
            world.attachments[0].broken,
            "a loaded skin cut should peel nearby skin-muscle coupling"
        );
        assert_eq!(world.stats.broken_attachments, 1);
        assert_eq!(world.stats.skin_flap_detachments, 1);
        assert_eq!(world.debug.skin_flap_detachments, 1);
        assert!(world.points[muscle].exposure > 0.9);
    }

    #[test]
    fn quiet_skin_cut_edge_does_not_delaminate_attachment() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.max_wound_sources = 0;
        materials.skin_flap_load_threshold = 400.0;
        materials.skin_flap_stress_threshold = 0.60;
        materials.skin_flap_cut_radius = 18.0;
        materials.max_skin_flap_detachments_per_step = 2;
        let mut world = World::new(materials);
        let skin_a = world.add_point(Vec2 { x: 0.0, y: 0.0 }, TissueLayer::Skin, false);
        let skin_b = world.add_point(Vec2 { x: 12.0, y: 0.0 }, TissueLayer::Skin, false);
        let muscle = world.add_point(Vec2 { x: 0.0, y: 7.0 }, TissueLayer::Muscle, false);
        world.debug.tool = ToolMode::Sharp;
        world.debug.impact = 900.0;
        world.add_spring(skin_a, skin_b, TissueLayer::Skin, 0.8, 1.7, 1000.0, false);
        world.add_attachment(skin_a, muscle);
        world.springs[0].broken = true;
        world.points[skin_a].load = 80.0;
        world.points[skin_b].load = 80.0;
        world.points[skin_a].exposure = 0.20;
        world.points[skin_b].exposure = 0.20;

        world.delaminate_skin_flaps_from_cut_edges();

        assert!(
            !world.attachments[0].broken,
            "a quiet cut edge should not peel attachment by proximity alone"
        );
        assert_eq!(world.stats.skin_flap_detachments, 0);
        assert_eq!(world.debug.skin_flap_detachments, 0);
    }

    #[test]
    fn inactive_wound_source_keeps_following_its_tissue_anchor() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        let mut world = World::new(materials);
        let point = world.add_point(Vec2 { x: 10.0, y: 10.0 }, TissueLayer::Skin, false);
        world.open_wound(
            Vec2 { x: 10.0, y: 10.0 },
            Vec2 { x: 0.0, y: -1.0 },
            TissueLayer::Skin,
            1.0,
            2.0,
            0.6,
        );
        world.wounds[0].active = false;
        world.wounds[0].clot = 1.0;
        world.points[point].position = Vec2 { x: 34.0, y: 18.0 };

        world.update_wound_anchors();

        assert_eq!(world.wounds[0].position, world.points[point].position);
    }

    #[test]
    fn loaded_tissue_reopens_clotted_wound_source() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.wound_reopen_load_threshold = 100.0;
        materials.wound_reopen_radius = 14.0;
        materials.wound_reopen_pressure_scale = 0.50;
        materials.wound_reopen_clot_loss = 0.25;
        materials.max_wound_reopens_per_step = 2;
        let mut world = World::new(materials);
        let point = world.add_point(Vec2 { x: 10.0, y: 10.0 }, TissueLayer::Skin, false);
        world.open_wound(
            Vec2 { x: 10.0, y: 10.0 },
            Vec2 { x: 0.0, y: -1.0 },
            TissueLayer::Skin,
            1.0,
            2.0,
            0.6,
        );
        world.wounds[0].active = false;
        world.wounds[0].age = 2.0;
        world.wounds[0].pressure = 0.0;
        world.wounds[0].clot = 0.92;
        world.wounds[0].accumulator = 0.0;
        world.points[point].load = 180.0;

        world.disturb_wounds_from_loaded_tissue();

        assert!(
            world.wounds[0].active,
            "a loaded clotted wound source should become active again"
        );
        assert!(world.wounds[0].pressure > 0.0);
        assert!(world.wounds[0].clot < 0.92);
        assert!(world.wounds[0].accumulator > 0.0);
        assert_eq!(world.stats.wound_reopens, 1);
        assert_eq!(world.debug.wound_reopens, 1);
        assert_eq!(world.debug.active_wounds, 1);
    }

    #[test]
    fn quiet_tissue_does_not_reopen_clotted_wound_source() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 0;
        materials.wound_reopen_load_threshold = 100.0;
        materials.wound_reopen_radius = 14.0;
        let mut world = World::new(materials);
        let point = world.add_point(Vec2 { x: 10.0, y: 10.0 }, TissueLayer::Skin, false);
        world.open_wound(
            Vec2 { x: 10.0, y: 10.0 },
            Vec2 { x: 0.0, y: -1.0 },
            TissueLayer::Skin,
            1.0,
            2.0,
            0.6,
        );
        world.wounds[0].active = false;
        world.wounds[0].age = 2.0;
        world.wounds[0].pressure = 0.0;
        world.wounds[0].clot = 0.92;
        world.points[point].load = 80.0;

        world.disturb_wounds_from_loaded_tissue();

        assert!(!world.wounds[0].active);
        assert_eq!(world.wounds[0].pressure, 0.0);
        assert_eq!(world.wounds[0].clot, 0.92);
        assert_eq!(world.stats.wound_reopens, 0);
        assert_eq!(world.debug.wound_reopens, 0);
    }

    #[test]
    fn sleeping_fragment_skips_fragment_contact_work() {
        let mut materials = Materials::default();
        materials.max_active_bone_fragments = 1;
        let mut world = World::new(materials);
        let bone = world.add_bone_segment(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 160.0, y: 100.0 },
            8.0,
            9999.0,
            false,
        );
        world.bones[bone].fractured = true;
        world.bones[bone].sleeping = true;
        world.add_point(Vec2 { x: 120.0, y: 105.0 }, TissueLayer::Muscle, false);

        world.solve_bone_fragment_tissue_contacts();
        world.solve_bone_fragment_repulsion();

        assert_eq!(world.debug.fragment_tissue_checks, 0);
        assert_eq!(world.debug.fragment_pair_checks, 0);
        assert_eq!(world.debug.active_fragments, 0);
    }

    #[test]
    fn settled_fluid_deposits_blood_stain() {
        let mut materials = Materials::default();
        materials.gravity = 920.0;
        materials.max_fluid_particles = 4;
        materials.max_blood_stains = 8;
        let mut world = World::new(materials);

        world.emit_fluid(
            Vec2 { x: 100.0, y: 108.0 },
            Vec2 { x: 0.0, y: 1.0 },
            1,
            90.0,
            3.0,
            1.0,
        );
        for _ in 0..24 {
            world.integrate(world.materials.fixed_dt, 220.0, 120.0);
        }

        assert!(
            world.stats.blood_stain_deposits > 0,
            "fluid floor impact should leave a persistent stain deposit"
        );
        assert!(
            world.blood_stains.iter().any(|stain| stain.intensity > 0.0),
            "deposited blood stain should remain inspectable after the particle settles"
        );
        assert!(
            world.fluids.iter().any(|fluid| fluid.stained),
            "each fluid particle should remember that it has already deposited a stain"
        );
    }

    #[test]
    fn fluid_and_wound_caps_report_budget_replacements() {
        let mut materials = Materials::default();
        materials.max_fluid_particles = 1;
        materials.max_wound_sources = 1;
        materials.max_blood_stains = 1;
        let mut world = World::new(materials);

        world.emit_fluid(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: 0.0, y: -1.0 },
            3,
            120.0,
            2.0,
            1.0,
        );
        assert_eq!(world.fluids.len(), 1);
        assert!(
            world.debug.fluid_budget_replacements > 0,
            "fluid ring buffer replacements should be visible in debug telemetry"
        );

        world.open_wound(
            Vec2 { x: 10.0, y: 10.0 },
            Vec2 { x: 1.0, y: 0.0 },
            TissueLayer::Skin,
            1.0,
            2.0,
            0.6,
        );
        world.open_wound(
            Vec2 { x: 100.0, y: 100.0 },
            Vec2 { x: -1.0, y: 0.0 },
            TissueLayer::Muscle,
            1.0,
            2.0,
            0.8,
        );
        assert_eq!(world.wounds.len(), 1);
        assert_eq!(world.debug.wound_budget_replacements, 1);

        world.deposit_blood_stain(Vec2 { x: 10.0, y: 120.0 }, 4.0, 1.0);
        world.deposit_blood_stain(Vec2 { x: 110.0, y: 120.0 }, 4.0, 1.0);
        assert_eq!(world.blood_stains.len(), 1);
        assert_eq!(world.debug.blood_stain_budget_replacements, 1);
    }
}

pub fn create_layered_body(width: f64, height: f64, materials: Materials) -> World {
    let mut world = World::new(materials);
    let body_height = (height * 0.78).min(width * 1.12).min(720.0);
    let body_width = body_height * 0.64;
    let origin_x = width * 0.52;
    let origin_y = height * 0.09;
    let cols = ((body_width / materials.point_spacing).floor() as i32).max(3);
    let rows = ((body_height / materials.point_spacing).floor() as i32).max(3);

    let mut skin_grid: HashMap<GridKey, usize> = HashMap::new();
    let mut muscle_grid: HashMap<GridKey, usize> = HashMap::new();
    let mut skin_points = Vec::new();
    let mut muscle_points = Vec::new();

    for y in 0..=rows {
        for x in 0..=cols {
            let nx = (f64::from(x) / f64::from(cols) - 0.5) * 0.7;
            let ny = f64::from(y) / f64::from(rows);
            let world_x = origin_x + (f64::from(x) / f64::from(cols) - 0.5) * body_width;
            let world_y = origin_y + ny * body_height;
            let pinned = ny < 0.035;
            let key = GridKey { x, y };
            if is_inside_humanoid_layer(nx, ny, 0.0) {
                let skin_point = world.add_point(
                    Vec2 {
                        x: world_x,
                        y: world_y,
                    },
                    TissueLayer::Skin,
                    pinned,
                );
                skin_grid.insert(key, skin_point);
                skin_points.push(skin_point);
            }
            if is_inside_humanoid_layer(nx, ny, 0.16) {
                let muscle_point = world.add_point(
                    Vec2 {
                        x: world_x,
                        y: world_y,
                    },
                    TissueLayer::Muscle,
                    pinned,
                );
                muscle_grid.insert(key, muscle_point);
                muscle_points.push(muscle_point);
            }
        }
    }

    let get = |grid: &HashMap<GridKey, usize>, x: i32, y: i32| -> usize {
        grid.get(&GridKey { x, y }).copied().unwrap_or(usize::MAX)
    };

    for y in 0..=rows {
        for x in 0..=cols {
            let skin_point = get(&skin_grid, x, y);
            let muscle_point = get(&muscle_grid, x, y);
            if skin_point == usize::MAX {
                continue;
            }
            add_layer_spring(
                &mut world,
                skin_point,
                get(&skin_grid, x + 1, y),
                TissueLayer::Skin,
                materials.skin_structural_stiffness,
                materials.skin_tear_stretch,
                materials.skin_tear_impulse,
                false,
            );
            add_layer_spring(
                &mut world,
                skin_point,
                get(&skin_grid, x, y + 1),
                TissueLayer::Skin,
                materials.skin_structural_stiffness,
                materials.skin_tear_stretch,
                materials.skin_tear_impulse,
                false,
            );
            add_layer_spring(
                &mut world,
                skin_point,
                get(&skin_grid, x + 1, y + 1),
                TissueLayer::Skin,
                materials.skin_shear_stiffness,
                materials.skin_tear_stretch * 1.08,
                materials.skin_tear_impulse,
                false,
            );
            add_layer_spring(
                &mut world,
                skin_point,
                get(&skin_grid, x - 1, y + 1),
                TissueLayer::Skin,
                materials.skin_shear_stiffness,
                materials.skin_tear_stretch * 1.08,
                materials.skin_tear_impulse,
                false,
            );

            if muscle_point == usize::MAX {
                continue;
            }
            add_layer_spring(
                &mut world,
                muscle_point,
                get(&muscle_grid, x, y + 1),
                TissueLayer::Muscle,
                materials.muscle_fiber_stiffness,
                materials.muscle_tear_stretch,
                materials.muscle_tear_impulse,
                true,
            );
            add_layer_spring(
                &mut world,
                muscle_point,
                get(&muscle_grid, x, y + 2),
                TissueLayer::Muscle,
                materials.muscle_fiber_stiffness * 0.42,
                materials.muscle_tear_stretch * 1.05,
                materials.muscle_tear_impulse,
                true,
            );
            add_layer_spring(
                &mut world,
                muscle_point,
                get(&muscle_grid, x + 1, y),
                TissueLayer::Muscle,
                materials.muscle_cross_stiffness,
                materials.muscle_tear_stretch,
                materials.muscle_tear_impulse,
                false,
            );
            add_layer_spring(
                &mut world,
                muscle_point,
                get(&muscle_grid, x + 1, y + 1),
                TissueLayer::Muscle,
                materials.muscle_shear_stiffness,
                materials.muscle_tear_stretch * 1.12,
                materials.muscle_tear_impulse,
                false,
            );
            add_layer_spring(
                &mut world,
                muscle_point,
                get(&muscle_grid, x - 1, y + 1),
                TissueLayer::Muscle,
                materials.muscle_shear_stiffness,
                materials.muscle_tear_stretch * 1.12,
                materials.muscle_tear_impulse,
                false,
            );
        }
    }

    for y in 0..rows {
        for x in 0..cols {
            add_cell_triangles(
                &mut world,
                &skin_grid,
                x,
                y,
                TissueLayer::Skin,
                materials.skin_area_stiffness,
            );
            add_cell_triangles(
                &mut world,
                &muscle_grid,
                x,
                y,
                TissueLayer::Muscle,
                materials.muscle_area_stiffness,
            );
        }
    }

    let torso_cavity_areas = world
        .areas
        .iter()
        .enumerate()
        .filter_map(|(index, area)| {
            if area.layer != TissueLayer::Muscle {
                return None;
            }
            let centroid = Vec2 {
                x: (world.points[area.a].home.x
                    + world.points[area.b].home.x
                    + world.points[area.c].home.x)
                    / 3.0,
                y: (world.points[area.a].home.y
                    + world.points[area.b].home.y
                    + world.points[area.c].home.y)
                    / 3.0,
            };
            let nx = ((centroid.x - origin_x) / body_width) * 0.7;
            let ny = (centroid.y - origin_y) / body_height;
            if (-0.17..=0.17).contains(&nx) && (0.255..=0.640).contains(&ny) {
                Some(index)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    world.add_cavity_from_areas(torso_cavity_areas);

    for skin_point in skin_points.iter().copied() {
        let mut nearest = [(usize::MAX, f64::INFINITY); SKIN_ATTACHMENT_CANDIDATES];
        let max_attachment_distance = materials.point_spacing * 3.15;
        for muscle_point in muscle_points.iter().copied() {
            let d = distance(
                world.points[skin_point].position,
                world.points[muscle_point].position,
            );
            if d > max_attachment_distance {
                continue;
            }
            for slot in 0..nearest.len() {
                if d >= nearest[slot].1 {
                    continue;
                }
                for shift in ((slot + 1)..nearest.len()).rev() {
                    nearest[shift] = nearest[shift - 1];
                }
                nearest[slot] = (muscle_point, d);
                break;
            }
        }
        for (muscle_point, _) in nearest {
            if muscle_point != usize::MAX {
                world.add_attachment(skin_point, muscle_point);
            }
        }
    }

    let body_point = |nx: f64, ny: f64| Vec2 {
        x: origin_x + (nx / 0.7) * body_width,
        y: origin_y + ny * body_height,
    };

    world.add_organ_region(
        OrganKind::LeftLung,
        body_point(-0.055, 0.385),
        Vec2 {
            x: body_width * 0.075,
            y: body_height * 0.125,
        },
    );
    world.add_organ_region(
        OrganKind::RightLung,
        body_point(0.055, 0.385),
        Vec2 {
            x: body_width * 0.075,
            y: body_height * 0.125,
        },
    );
    world.add_organ_region(
        OrganKind::Liver,
        body_point(0.058, 0.555),
        Vec2 {
            x: body_width * 0.112,
            y: body_height * 0.074,
        },
    );
    world.add_organ_region(
        OrganKind::Spleen,
        body_point(-0.098, 0.560),
        Vec2 {
            x: body_width * 0.060,
            y: body_height * 0.064,
        },
    );

    let head_bone = world.add_bone_segment(
        body_point(0.0, 0.070),
        body_point(0.0, 0.205),
        8.2,
        materials.bone_fracture_impulse * 0.75,
        true,
    );
    let spine_bone = world.add_bone_segment(
        body_point(0.0, 0.250),
        body_point(0.0, 0.720),
        7.2,
        materials.bone_fracture_impulse,
        false,
    );
    let shoulder_bone = world.add_bone_segment(
        body_point(-0.116, 0.375),
        body_point(0.116, 0.375),
        6.2,
        materials.bone_fracture_impulse * 0.95,
        false,
    );
    let pelvis_bone = world.add_bone_segment(
        body_point(-0.055, 0.720),
        body_point(0.055, 0.720),
        6.4,
        materials.bone_fracture_impulse * 0.9,
        false,
    );
    let rib_specs = [
        (0.335, 0.080, 0.358, 0.62),
        (0.405, 0.095, 0.428, 0.58),
        (0.475, 0.080, 0.500, 0.55),
        (0.520, 0.045, 0.550, 0.52),
    ];
    let mut rib_bones = Vec::new();
    for (root_ny, lateral_nx, tip_ny, strength_scale) in rib_specs {
        let left_rib = world.add_bone_segment_with_kind(
            body_point(-0.012, root_ny),
            body_point(-lateral_nx, tip_ny),
            3.4,
            materials.bone_fracture_impulse * strength_scale,
            false,
            BoneKind::Rib,
        );
        rib_bones.push((left_rib, root_ny));
        let right_rib = world.add_bone_segment_with_kind(
            body_point(0.012, root_ny),
            body_point(lateral_nx, tip_ny),
            3.4,
            materials.bone_fracture_impulse * strength_scale,
            false,
            BoneKind::Rib,
        );
        rib_bones.push((right_rib, root_ny));
    }
    let left_upper_arm_bone = world.add_bone_segment(
        body_point(-0.130, 0.405),
        body_point(-0.165, 0.585),
        5.7,
        materials.bone_fracture_impulse * 0.82,
        false,
    );
    let left_forearm_bone = world.add_bone_segment(
        body_point(-0.165, 0.585),
        body_point(-0.158, 0.700),
        4.8,
        materials.bone_fracture_impulse * 0.72,
        false,
    );
    let right_upper_arm_bone = world.add_bone_segment(
        body_point(0.130, 0.405),
        body_point(0.165, 0.585),
        5.7,
        materials.bone_fracture_impulse * 0.82,
        false,
    );
    let right_forearm_bone = world.add_bone_segment(
        body_point(0.165, 0.585),
        body_point(0.158, 0.700),
        4.8,
        materials.bone_fracture_impulse * 0.72,
        false,
    );
    let left_thigh_bone = world.add_bone_segment(
        body_point(-0.045, 0.755),
        body_point(-0.065, 0.875),
        6.4,
        materials.bone_fracture_impulse * 0.9,
        false,
    );
    let left_shin_bone = world.add_bone_segment(
        body_point(-0.065, 0.875),
        body_point(-0.083, 0.968),
        5.3,
        materials.bone_fracture_impulse * 0.78,
        false,
    );
    let right_thigh_bone = world.add_bone_segment(
        body_point(0.045, 0.755),
        body_point(0.065, 0.875),
        6.4,
        materials.bone_fracture_impulse * 0.9,
        false,
    );
    let right_shin_bone = world.add_bone_segment(
        body_point(0.065, 0.875),
        body_point(0.083, 0.968),
        5.3,
        materials.bone_fracture_impulse * 0.78,
        false,
    );

    world.add_bone_joint(head_bone, 1.0, spine_bone, 0.0, -0.45, 0.45);
    world.add_bone_joint(spine_bone, 0.27, shoulder_bone, 0.5, -0.55, 0.55);
    world.add_bone_joint(spine_bone, 1.0, pelvis_bone, 0.5, -0.45, 0.45);
    for (rib_bone, root_ny) in rib_bones {
        let spine_t = ((root_ny - 0.250) / (0.720 - 0.250)).clamp(0.0, 1.0);
        world.add_bone_joint(spine_bone, spine_t, rib_bone, 0.0, -0.40, 0.40);
    }
    world.add_bone_joint(shoulder_bone, 0.0, left_upper_arm_bone, 0.0, -1.25, 1.05);
    world.add_bone_joint(
        left_upper_arm_bone,
        1.0,
        left_forearm_bone,
        0.0,
        -1.10,
        1.10,
    );
    world.add_bone_joint(shoulder_bone, 1.0, right_upper_arm_bone, 0.0, -1.05, 1.25);
    world.add_bone_joint(
        right_upper_arm_bone,
        1.0,
        right_forearm_bone,
        0.0,
        -1.10,
        1.10,
    );
    world.add_bone_joint(pelvis_bone, 0.18, left_thigh_bone, 0.0, -0.78, 0.78);
    world.add_bone_joint(left_thigh_bone, 1.0, left_shin_bone, 0.0, -0.85, 0.85);
    world.add_bone_joint(pelvis_bone, 0.82, right_thigh_bone, 0.0, -0.78, 0.78);
    world.add_bone_joint(right_thigh_bone, 1.0, right_shin_bone, 0.0, -0.85, 0.85);

    for muscle_point in muscle_points {
        let mut nearest_bone = usize::MAX;
        let mut nearest_distance = materials.point_spacing * 2.7;
        let mut nearest_t = 0.0;
        for i in 0..world.bones.len() {
            let bone = world.bones[i];
            let t = segment_t(world.points[muscle_point].position, bone.a, bone.b);
            let anchor = bone_point(bone, t);
            let d = distance(world.points[muscle_point].position, anchor);
            if d < nearest_distance {
                nearest_distance = d;
                nearest_bone = i;
                nearest_t = t;
            }
        }
        if nearest_bone != usize::MAX {
            world.add_bone_attachment(muscle_point, nearest_bone, nearest_t);
        }
    }

    world.add_vessel_segment(body_point(0.0, 0.255), body_point(0.0, 0.725), 4.1, 1.65);
    world.add_vessel_segment(
        body_point(-0.035, 0.710),
        body_point(-0.072, 0.952),
        3.0,
        1.22,
    );
    world.add_vessel_segment(
        body_point(0.035, 0.710),
        body_point(0.072, 0.952),
        3.0,
        1.22,
    );
    world.add_vessel_segment(
        body_point(-0.078, 0.385),
        body_point(-0.158, 0.698),
        2.5,
        1.05,
    );
    world.add_vessel_segment(
        body_point(0.078, 0.385),
        body_point(0.158, 0.698),
        2.5,
        1.05,
    );
    world.add_vessel_segment(
        body_point(-0.030, 0.205),
        body_point(-0.105, 0.382),
        2.6,
        1.10,
    );
    world.add_vessel_segment(
        body_point(0.030, 0.205),
        body_point(0.105, 0.382),
        2.6,
        1.10,
    );

    world
}

pub fn distance(a: Vec2, b: Vec2) -> f64 {
    hypot(b.x - a.x, b.y - a.y)
}

pub fn signed_area(a: Vec2, b: Vec2, c: Vec2) -> f64 {
    ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)) * 0.5
}

pub fn point_inside_layer(world: &World, point: Vec2, layer: TissueLayer) -> bool {
    world.triangles.iter().any(|triangle| {
        triangle.layer == layer
            && world.triangle_alive(triangle)
            && point_in_triangle(
                point,
                world.points[triangle.a].position,
                world.points[triangle.b].position,
                world.points[triangle.c].position,
            )
    })
}

pub fn validate_anatomy(world: &World, samples_per_bone: usize) -> AnatomyValidation {
    let mut validation = AnatomyValidation {
        skin_points: world
            .points
            .iter()
            .filter(|point| point.layer == TissueLayer::Skin)
            .count() as i32,
        muscle_points: world
            .points
            .iter()
            .filter(|point| point.layer == TissueLayer::Muscle)
            .count() as i32,
        ..AnatomyValidation::default()
    };
    let sample_count = samples_per_bone.max(2);
    for bone in &world.bones {
        let mut segment_outside_skin = false;
        let mut segment_outside_muscle = false;
        for i in 0..sample_count {
            let t = if sample_count == 1 {
                0.0
            } else {
                i as f64 / (sample_count - 1) as f64
            };
            let sample = bone_point(*bone, t);
            validation.bone_samples += 1;
            if !point_inside_layer(world, sample, TissueLayer::Skin) {
                validation.bone_samples_outside_skin += 1;
                segment_outside_skin = true;
            }
            if !point_inside_layer(world, sample, TissueLayer::Muscle) {
                validation.bone_samples_outside_muscle += 1;
                segment_outside_muscle = true;
            }
        }
        if segment_outside_skin {
            validation.bone_segments_outside_skin += 1;
        }
        if segment_outside_muscle {
            validation.bone_segments_outside_muscle += 1;
        }
    }
    validation
}

fn add_layer_spring(
    world: &mut World,
    a: usize,
    b: usize,
    layer: TissueLayer,
    stiffness: f64,
    tear_stretch: f64,
    tear_impulse: f64,
    fiber: bool,
) {
    if a == usize::MAX || b == usize::MAX {
        return;
    }
    world.add_spring(a, b, layer, stiffness, tear_stretch, tear_impulse, fiber);
}

fn add_cell_triangles(
    world: &mut World,
    grid: &HashMap<GridKey, usize>,
    x: i32,
    y: i32,
    layer: TissueLayer,
    area_stiffness: f64,
) {
    let a = grid.get(&GridKey { x, y }).copied();
    let b = grid.get(&GridKey { x: x + 1, y }).copied();
    let c = grid.get(&GridKey { x, y: y + 1 }).copied();
    let d = grid.get(&GridKey { x: x + 1, y: y + 1 }).copied();
    if let (Some(a), Some(b), Some(c)) = (a, b, c) {
        world.add_triangle(a, b, c, layer);
        world.add_area(a, b, c, layer, area_stiffness);
    }
    if let (Some(b), Some(d), Some(c)) = (b, d, c) {
        world.add_triangle(b, d, c, layer);
        world.add_area(b, d, c, layer, area_stiffness);
    }
}

fn tool_profile(tool: ToolMode) -> ToolProfile {
    match tool {
        ToolMode::Sharp => ToolProfile {
            radius_scale: 0.48,
            reach_padding: 5.0,
            mass_scale: 0.72,
            tissue_push_scale: 0.38,
            tissue_load_scale: 2.05,
            contusion_scale: 0.42,
            bone_push_scale: 0.30,
            bone_load_scale: 0.58,
            fracture_scale: 0.76,
            tear_pressure_scale: 1.0,
            fluid_scale: 1.35,
            blade_normal_bias: 0.82,
            drag_scale: 0.72,
            rebound_scale: 0.58,
            blade_front_scale: 1.55,
            blade_back_scale: 0.65,
            blade_contact_radius_scale: 0.42,
        },
        ToolMode::Heavy => ToolProfile {
            radius_scale: 1.18,
            reach_padding: 16.0,
            mass_scale: 1.85,
            tissue_push_scale: 1.24,
            tissue_load_scale: 1.18,
            contusion_scale: 1.55,
            bone_push_scale: 1.46,
            bone_load_scale: 1.72,
            fracture_scale: 1.34,
            tear_pressure_scale: 0.10,
            fluid_scale: 0.92,
            blade_normal_bias: 0.0,
            drag_scale: 0.82,
            rebound_scale: 1.42,
            blade_front_scale: 0.0,
            blade_back_scale: 0.0,
            blade_contact_radius_scale: 1.0,
        },
        ToolMode::Blunt => ToolProfile::default(),
    }
}

fn make_tool_contact_shape(
    input: &InputState,
    profile: ToolProfile,
    radius: f64,
) -> ToolContactShape {
    let center = Vec2 {
        x: input.x,
        y: input.y,
    };
    let direction = normalized(
        Vec2 {
            x: input.vx,
            y: input.vy,
        },
        Vec2 { x: 1.0, y: 0.0 },
    );
    let blade_normal = Vec2 {
        x: -direction.y,
        y: direction.x,
    };
    let blade_segment = profile.blade_front_scale > 0.0;
    let contact_radius = if blade_segment {
        radius * profile.blade_contact_radius_scale
    } else {
        radius
    };
    ToolContactShape {
        center,
        direction,
        blade_normal,
        influence: contact_radius + profile.reach_padding,
        blade_segment,
        axis_start: if blade_segment {
            subtract(center, scale(direction, radius * profile.blade_back_scale))
        } else {
            center
        },
        axis_end: if blade_segment {
            add(center, scale(direction, radius * profile.blade_front_scale))
        } else {
            center
        },
    }
}

fn sample_point_contact(point: Vec2, shape: &ToolContactShape) -> ToolPointContact {
    let contact_point = if shape.blade_segment {
        lerp(
            shape.axis_start,
            shape.axis_end,
            segment_t(point, shape.axis_start, shape.axis_end),
        )
    } else {
        shape.center
    };
    let delta = subtract(point, contact_point);
    ToolPointContact {
        normal: normalized(delta, shape.blade_normal),
        distance: distance(point, contact_point),
    }
}

fn tool_organ_contact(
    shape: &ToolContactShape,
    organ: OrganRegion,
    extra_radius: f64,
) -> Option<OrganToolContact> {
    let tool_point = if shape.blade_segment {
        lerp(
            shape.axis_start,
            shape.axis_end,
            segment_t(organ.center, shape.axis_start, shape.axis_end),
        )
    } else {
        shape.center
    };
    let dx = (tool_point.x - organ.center.x) / organ.radius.x.max(EPSILON);
    let dy = (tool_point.y - organ.center.y) / organ.radius.y.max(EPSILON);
    let normalized_distance = (dx * dx + dy * dy).sqrt();
    let average_radius = ((organ.radius.x + organ.radius.y) * 0.5).max(1.0);
    let reach = if shape.blade_segment {
        extra_radius.max(0.0) + shape.influence * 0.55
    } else {
        extra_radius.max(0.0) + shape.influence
    };
    let normalized_reach = (reach / average_radius).max(0.04);
    if normalized_distance > 1.0 + normalized_reach {
        return None;
    }
    let contact = if normalized_distance <= 1.0 {
        (1.0 - normalized_distance * 0.18).clamp(0.72, 1.0)
    } else {
        (1.0 - (normalized_distance - 1.0) / normalized_reach).clamp(0.0, 1.0)
    };
    if contact <= EPSILON {
        None
    } else {
        Some(OrganToolContact {
            tool_point,
            contact,
        })
    }
}

fn point_in_organ(point: Vec2, organ: OrganRegion) -> bool {
    let dx = (point.x - organ.center.x) / organ.radius.x.max(EPSILON);
    let dy = (point.y - organ.center.y) / organ.radius.y.max(EPSILON);
    dx * dx + dy * dy <= 1.0
}

fn remap_joint_end(
    joint_bone: &mut usize,
    joint_t: &mut f64,
    bone_index: usize,
    second_index: usize,
    break_t: f64,
    old_radius: f64,
    len: f64,
    overload: f64,
    broken: &mut bool,
    affected: &mut bool,
) {
    if *joint_bone != bone_index {
        return;
    }
    *affected = true;
    let original_t = *joint_t;
    let detach_zone = (0.05 + overload * 0.03 + old_radius / len.max(1.0)).clamp(0.05, 0.14);
    if (original_t - break_t).abs() <= detach_zone {
        *broken = true;
        return;
    }
    if original_t < break_t {
        *joint_t = (original_t / break_t.max(0.05)).clamp(0.0, 1.0);
    } else {
        *joint_bone = second_index;
        *joint_t = ((original_t - break_t) / (1.0 - break_t).max(0.05)).clamp(0.0, 1.0);
    }
}

fn apply_bone_anchor_delta(bone: &mut BoneSegment, t: f64, dx: f64, dy: f64) {
    if bone.pinned {
        return;
    }
    let t = t.clamp(0.0, 1.0);
    bone.a.x += dx * (1.0 - t);
    bone.a.y += dy * (1.0 - t);
    bone.b.x += dx * t;
    bone.b.y += dy * t;
}

fn rotate_bone_around_anchor(bone: &mut BoneSegment, t: f64, angle: f64) {
    if bone.pinned || angle.abs() <= EPSILON {
        return;
    }
    let anchor = bone_point(*bone, t.clamp(0.0, 1.0));
    bone.a = rotate_around(bone.a, anchor, angle);
    bone.b = rotate_around(bone.b, anchor, angle);
}

fn rotate_bone_around_center(bone: &mut BoneSegment, angle: f64) {
    if bone.pinned || angle.abs() <= EPSILON {
        return;
    }
    let center = midpoint(bone.a, bone.b);
    bone.a = rotate_around(bone.a, center, angle);
    bone.b = rotate_around(bone.b, center, angle);
}

fn apply_bone_torque(
    materials: &Materials,
    debug: &mut ContactDebug,
    bone: &mut BoneSegment,
    contact: Vec2,
    impulse: Vec2,
) {
    if bone.pinned {
        return;
    }
    let center = midpoint(bone.a, bone.b);
    let lever = subtract(contact, center);
    let inertia = (bone.rest_length * bone.rest_length * bone.radius.max(1.0)).max(12.0);
    let torque = cross(lever, impulse);
    bone.angular_velocity =
        (bone.angular_velocity + torque / inertia * materials.bone_torque_scale).clamp(-36.0, 36.0);
    if bone.fractured || bone.splinter {
        debug.max_bone_angular_speed = debug
            .max_bone_angular_speed
            .max(bone.angular_velocity.abs());
    }
}

fn fragment_tissue_fallback_normal(bone: BoneSegment, point: Vec2, t: f64) -> Vec2 {
    let tangent = normalized(subtract(bone.b, bone.a), Vec2 { x: 1.0, y: 0.0 });
    let mut normal = Vec2 {
        x: -tangent.y,
        y: tangent.x,
    };
    let anchor = bone_point(bone, t);
    let center = midpoint(bone.a, bone.b);
    let point_side = dot(subtract(point, center), normal);
    if point_side < 0.0 {
        normal = scale(normal, -1.0);
    }
    if t < 0.18 && (bone.broken_start || bone.splinter) {
        normal = normalized(add(normal, bone.broken_start_normal), normal);
    } else if t > 0.82 && (bone.broken_end || bone.splinter) {
        normal = normalized(add(normal, bone.broken_end_normal), normal);
    } else if distance_sq(anchor, point) <= EPSILON {
        normal = normalized(subtract(point, center), normal);
    }
    normal
}

fn damp_bone_velocity_against_contact(
    bone: &mut BoneSegment,
    normal: Vec2,
    normal_damping: f64,
    tangential_damping: f64,
) {
    if bone.pinned {
        return;
    }
    damp_bone_endpoint_velocity(
        bone.a,
        &mut bone.previous_a,
        normal,
        normal_damping,
        tangential_damping,
    );
    damp_bone_endpoint_velocity(
        bone.b,
        &mut bone.previous_b,
        normal,
        normal_damping,
        tangential_damping,
    );
}

fn damp_bone_endpoint_velocity(
    current: Vec2,
    previous: &mut Vec2,
    normal: Vec2,
    normal_damping: f64,
    tangential_damping: f64,
) {
    let velocity = subtract(current, *previous);
    let normal_speed = dot(velocity, normal);
    let normal_velocity = scale(normal, normal_speed);
    let tangent_velocity = subtract(velocity, normal_velocity);
    let kept_normal = if normal_speed > 0.0 {
        scale(normal_velocity, 1.0 - normal_damping.clamp(0.0, 0.94))
    } else {
        normal_velocity
    };
    let kept_tangent = scale(tangent_velocity, 1.0 - tangential_damping.clamp(0.0, 0.84));
    let damped = add(kept_normal, kept_tangent);
    *previous = subtract(current, damped);
}

fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y);
    let d2 = (p.x - c.x) * (b.y - c.y) - (b.x - c.x) * (p.y - c.y);
    let d3 = (p.x - a.x) * (c.y - a.y) - (c.x - a.x) * (p.y - a.y);
    let has_negative = d1 < -EPSILON || d2 < -EPSILON || d3 < -EPSILON;
    let has_positive = d1 > EPSILON || d2 > EPSILON || d3 > EPSILON;
    !(has_negative && has_positive)
}

fn segment_t(point: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = subtract(b, a);
    let len_sq = dot(ab, ab);
    if len_sq <= EPSILON {
        return 0.0;
    }
    (dot(subtract(point, a), ab) / len_sq).clamp(0.0, 1.0)
}

fn closest_segment_points(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> SegmentClosestPoints {
    let d_a = subtract(a1, a0);
    let d_b = subtract(b1, b0);
    let r = subtract(a0, b0);
    let len_a = dot(d_a, d_a);
    let len_b = dot(d_b, d_b);
    let d_bf = dot(d_b, r);
    let mut t_a: f64;
    let t_b: f64;

    if len_a <= EPSILON && len_b <= EPSILON {
        t_a = 0.0;
        t_b = 0.0;
    } else if len_a <= EPSILON {
        t_a = 0.0;
        t_b = (d_bf / len_b).clamp(0.0, 1.0);
    } else {
        let d_ac = dot(d_a, r);
        if len_b <= EPSILON {
            t_b = 0.0;
            t_a = (-d_ac / len_a).clamp(0.0, 1.0);
        } else {
            let d_ab = dot(d_a, d_b);
            let denom = len_a * len_b - d_ab * d_ab;
            t_a = if denom > EPSILON {
                ((d_ab * d_bf - d_ac * len_b) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let t_b_numer = d_ab * t_a + d_bf;
            if t_b_numer < 0.0 {
                t_b = 0.0;
                t_a = (-d_ac / len_a).clamp(0.0, 1.0);
            } else if t_b_numer > len_b {
                t_b = 1.0;
                t_a = ((d_ab - d_ac) / len_a).clamp(0.0, 1.0);
            } else {
                t_b = t_b_numer / len_b;
            }
        }
    }
    let point_a = lerp(a0, a1, t_a);
    let point_b = lerp(b0, b1, t_b);
    SegmentClosestPoints {
        t_a,
        t_b,
        point_a,
        point_b,
        distance: distance(point_a, point_b),
    }
}

fn is_inside_humanoid_layer(nx: f64, ny: f64, inset: f64) -> bool {
    let horizontal_scale = (1.0 - inset * 0.72).clamp(0.34, 1.0);
    let vertical_scale = (1.0 - inset * 0.12).clamp(0.84, 1.0);
    let sample_y = (ny - 0.52) / vertical_scale + 0.52;
    let sample_x = nx / horizontal_scale;
    front_pixel_mask_contains(sample_x, sample_y)
}

fn front_pixel_mask_contains(nx: f64, ny: f64) -> bool {
    if !(0.0..=1.0).contains(&ny) {
        return false;
    }
    let x_ratio =
        (nx + FRONT_PIXEL_SILHOUETTE_WORLD_WIDTH * 0.5) / FRONT_PIXEL_SILHOUETTE_WORLD_WIDTH;
    if !(0.0..=1.0).contains(&x_ratio) {
        return false;
    }

    let mut width = 0usize;
    let mut height = 0usize;
    let mut rows = Vec::new();
    for line in FRONT_PIXEL_SILHOUETTE_REFERENCE.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("# ") {
            continue;
        }
        if let Some(size) = line.strip_prefix("size ") {
            let mut parts = size.split_whitespace();
            width = parts
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            height = parts
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            continue;
        }
        rows.push(line);
    }
    if width == 0 || height == 0 || rows.len() != height {
        return false;
    }

    let col = (x_ratio * width as f64)
        .floor()
        .clamp(0.0, (width - 1) as f64) as usize;
    let row = (ny * height as f64).floor().clamp(0.0, (height - 1) as f64) as usize;
    rows[row].as_bytes().get(col).copied() == Some(b'#')
}
fn bone_point(bone: BoneSegment, t: f64) -> Vec2 {
    Vec2 {
        x: bone.a.x + (bone.b.x - bone.a.x) * t,
        y: bone.a.y + (bone.b.y - bone.a.y) * t,
    }
}

fn bone_anchor_velocity(bone: BoneSegment, t: f64) -> Vec2 {
    subtract(
        bone_point(bone, t),
        lerp(bone.previous_a, bone.previous_b, t.clamp(0.0, 1.0)),
    )
}

fn constrain_fragment_endpoint_to_floor(
    materials: Materials,
    point: &mut Vec2,
    previous: &mut Vec2,
    floor_y: f64,
) -> (bool, bool) {
    if point.y <= floor_y {
        return (false, false);
    }

    let velocity = subtract(*point, *previous);
    let speed = hypot(velocity.x, velocity.y) / materials.fixed_dt.max(EPSILON);
    let resting = speed < materials.fragment_floor_rest_speed.max(1.0);
    point.y = floor_y;

    let tangent_retention = (1.0 - materials.fragment_floor_friction).clamp(0.0, 1.0);
    previous.x = point.x - velocity.x * tangent_retention;
    if velocity.y > 0.0 {
        let normal_retention = if resting {
            0.0
        } else {
            (1.0 - materials.fragment_floor_normal_damping).clamp(0.0, 0.55)
        };
        previous.y = point.y - velocity.y * normal_retention;
    } else {
        previous.y = previous.y.min(point.y);
    }

    (true, resting)
}

fn bone_angle(bone: BoneSegment) -> f64 {
    (bone.b.y - bone.a.y).atan2(bone.b.x - bone.a.x)
}

fn free_bone_fragment(bone: BoneSegment) -> bool {
    !bone.pinned && (bone.fractured || bone.splinter)
}

fn awake_free_bone_fragment(bone: BoneSegment) -> bool {
    free_bone_fragment(bone) && !bone.sleeping
}

fn fragment_endpoint_speed(bone: BoneSegment, dt: f64) -> f64 {
    distance(bone.a, bone.previous_a).max(distance(bone.b, bone.previous_b)) / dt.max(EPSILON)
}

fn bone_contact_mass(bone: BoneSegment, scale: f64) -> f64 {
    (bone.rest_length * bone.radius.max(1.0) * bone.radius.max(1.0) * scale.max(0.1)).max(1.0)
}

fn distance_to_segment(point: Vec2, a: Vec2, b: Vec2) -> f64 {
    distance(point, lerp(a, b, segment_t(point, a, b)))
}

fn segment_aabb(a: Vec2, b: Vec2, margin: f64) -> Aabb {
    let margin = margin.max(0.0);
    Aabb {
        min: Vec2 {
            x: a.x.min(b.x) - margin,
            y: a.y.min(b.y) - margin,
        },
        max: Vec2 {
            x: a.x.max(b.x) + margin,
            y: a.y.max(b.y) + margin,
        },
    }
}

fn spatial_key(point: Vec2, cell_size: f64) -> GridKey {
    let cell_size = cell_size.max(1.0);
    GridKey {
        x: (point.x / cell_size).floor() as i32,
        y: (point.y / cell_size).floor() as i32,
    }
}

fn for_spatial_cells<F>(aabb: Aabb, cell_size: f64, mut visit: F)
where
    F: FnMut(GridKey),
{
    let cell_size = cell_size.max(1.0);
    let min_key = spatial_key(aabb.min, cell_size);
    let max_key = spatial_key(aabb.max, cell_size);
    for y in min_key.y.min(max_key.y)..=min_key.y.max(max_key.y) {
        for x in min_key.x.min(max_key.x)..=min_key.x.max(max_key.x) {
            visit(GridKey { x, y });
        }
    }
}

fn add_index_to_spatial_cells(
    grid: &mut HashMap<GridKey, Vec<usize>>,
    aabb: Aabb,
    cell_size: f64,
    index: usize,
) {
    for_spatial_cells(aabb, cell_size, |key| {
        grid.entry(key).or_insert_with(Vec::new).push(index);
    });
}

fn distance_sq(a: Vec2, b: Vec2) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    dx * dx + dy * dy
}

fn lerp(a: Vec2, b: Vec2, t: f64) -> Vec2 {
    Vec2 {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    }
}

fn wrap_angle(mut angle: f64) -> f64 {
    while angle > PI {
        angle -= PI * 2.0;
    }
    while angle < -PI {
        angle += PI * 2.0;
    }
    angle
}

fn rotate_around(point: Vec2, pivot: Vec2, angle: f64) -> Vec2 {
    let (s, c) = angle.sin_cos();
    let dx = point.x - pivot.x;
    let dy = point.y - pivot.y;
    Vec2 {
        x: pivot.x + dx * c - dy * s,
        y: pivot.y + dx * s + dy * c,
    }
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}

fn subtract(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

fn scale(value: Vec2, amount: f64) -> Vec2 {
    Vec2 {
        x: value.x * amount,
        y: value.y * amount,
    }
}

fn clamp_magnitude(value: Vec2, max_length: f64) -> Vec2 {
    let len = hypot(value.x, value.y);
    if len <= max_length.max(0.0) || len <= EPSILON {
        value
    } else {
        scale(value, max_length / len)
    }
}

fn cross(a: Vec2, b: Vec2) -> f64 {
    a.x * b.y - a.y * b.x
}

fn dot(a: Vec2, b: Vec2) -> f64 {
    a.x * b.x + a.y * b.y
}

fn midpoint(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: (a.x + b.x) * 0.5,
        y: (a.y + b.y) * 0.5,
    }
}

fn normalized(value: Vec2, fallback: Vec2) -> Vec2 {
    let len = hypot(value.x, value.y);
    if len <= EPSILON {
        fallback
    } else {
        Vec2 {
            x: value.x / len,
            y: value.y / len,
        }
    }
}

fn hypot(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

fn two_mut<T>(values: &mut [T], a: usize, b: usize) -> (&mut T, &mut T) {
    assert!(a != b);
    if a < b {
        let (left, right) = values.split_at_mut(b);
        (&mut left[a], &mut right[0])
    } else {
        let (left, right) = values.split_at_mut(a);
        (&mut right[0], &mut left[b])
    }
}
