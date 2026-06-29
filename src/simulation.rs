use std::collections::HashMap;
use std::f64::consts::PI;

const EPSILON: f64 = 0.0001;
const FRAGMENT_TISSUE_POINT_RADIUS_SCALE: f64 = 0.36;
const FRAGMENT_TISSUE_RESISTANCE: f64 = 0.72;
const FRAGMENT_TISSUE_NORMAL_DAMPING: f64 = 0.58;
const FRAGMENT_TISSUE_TANGENTIAL_FRICTION: f64 = 0.34;
const FRAGMENT_TISSUE_ANGULAR_FRICTION: f64 = 0.18;
const SKIN_ATTACHMENT_CANDIDATES: usize = 4;
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
    pub skin_tear_stretch: f64,
    pub skin_tear_impulse: f64,
    pub muscle_fiber_stiffness: f64,
    pub muscle_cross_stiffness: f64,
    pub muscle_shear_stiffness: f64,
    pub muscle_area_stiffness: f64,
    pub muscle_tear_stretch: f64,
    pub muscle_tear_impulse: f64,
    pub muscle_exposed_tear_impulse: f64,
    pub attachment_stiffness: f64,
    pub attachment_break_stretch: f64,
    pub attachment_break_impulse: f64,
    pub bone_fracture_impulse: f64,
    pub max_bone_fracture_depth: i32,
    pub min_bone_fragment_length: f64,
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
    pub max_fluid_particles: usize,
    pub fluid_damping: f64,
    pub fluid_gravity_scale: f64,
    pub fluid_lifetime: f64,
    pub fluid_floor_friction: f64,
    pub fluid_impact_scale: f64,
    pub max_wound_sources: usize,
    pub wound_leak_rate: f64,
    pub wound_pressure_decay: f64,
    pub wound_clot_rate: f64,
    pub wound_spray_pressure: f64,
    pub wound_merge_radius: f64,
    pub sharp_tool_tear_pressure: f64,
    pub fragment_contact_radius: f64,
    pub fragment_damage_impulse: f64,
    pub fragment_push: f64,
    pub fragment_repulsion_stiffness: f64,
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
            skin_tear_stretch: 1.68,
            skin_tear_impulse: 820.0,
            muscle_fiber_stiffness: 0.86,
            muscle_cross_stiffness: 0.44,
            muscle_shear_stiffness: 0.38,
            muscle_area_stiffness: 0.36,
            muscle_tear_stretch: 1.92,
            muscle_tear_impulse: 1180.0,
            muscle_exposed_tear_impulse: 620.0,
            attachment_stiffness: 0.46,
            attachment_break_stretch: 2.40,
            attachment_break_impulse: 980.0,
            bone_fracture_impulse: 1850.0,
            max_bone_fracture_depth: 3,
            min_bone_fragment_length: 30.0,
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
            max_fluid_particles: 900,
            fluid_damping: 0.982,
            fluid_gravity_scale: 0.42,
            fluid_lifetime: 4.8,
            fluid_floor_friction: 0.48,
            fluid_impact_scale: 0.08,
            max_wound_sources: 160,
            wound_leak_rate: 4.6,
            wound_pressure_decay: 0.20,
            wound_clot_rate: 0.13,
            wound_spray_pressure: 1.25,
            wound_merge_radius: 16.0,
            sharp_tool_tear_pressure: 0.66,
            fragment_contact_radius: 15.0,
            fragment_damage_impulse: 520.0,
            fragment_push: 0.34,
            fragment_repulsion_stiffness: 0.56,
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
            mass: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub a: usize,
    pub b: usize,
    pub rest: f64,
    pub stiffness: f64,
    pub tear_stretch: f64,
    pub tear_impulse: f64,
    pub layer: TissueLayer,
    pub fiber: bool,
    pub broken: bool,
    pub stress: f64,
}

impl Default for Spring {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            rest: 0.0,
            stiffness: 0.0,
            tear_stretch: 0.0,
            tear_impulse: 0.0,
            layer: TissueLayer::Skin,
            fiber: false,
            broken: false,
            stress: 0.0,
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

#[derive(Clone, Copy, Debug)]
pub struct BoneSegment {
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
}

impl Default for BoneSegment {
    fn default() -> Self {
        Self {
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
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub broken_skin: i32,
    pub broken_muscle: i32,
    pub broken_attachments: i32,
    pub broken_bone_attachments: i32,
    pub broken_bone_joints: i32,
    pub fractured_bones: i32,
    pub emitted_fluid_particles: i32,
    pub wound_fluid_particles: i32,
    pub opened_wounds: i32,
    pub fragment_tissue_hits: i32,
    pub fragment_tissue_tears: i32,
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
    pub max_wound_pressure: f64,
    pub max_wound_clot: f64,
    pub last_fracture_impulse: f64,
    pub bone_contacts: i32,
    pub tissue_contacts: i32,
    pub fractures: i32,
    pub fluid_emitted: i32,
    pub fragment_contacts: i32,
    pub fragment_tears: i32,
    pub fragment_pair_contacts: i32,
    pub post_fracture_joint_corrections: i32,
    pub active_wounds: i32,
    pub wound_leaks: i32,
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
            max_wound_pressure: 0.0,
            max_wound_clot: 0.0,
            last_fracture_impulse: 0.0,
            bone_contacts: 0,
            tissue_contacts: 0,
            fractures: 0,
            fluid_emitted: 0,
            fragment_contacts: 0,
            fragment_tears: 0,
            fragment_pair_contacts: 0,
            post_fracture_joint_corrections: 0,
            active_wounds: 0,
            wound_leaks: 0,
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
    fluids: Vec<FluidParticle>,
    wounds: Vec<WoundSource>,
    stats: Stats,
    debug: ContactDebug,
    fluid_write_cursor: usize,
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
            fluids: Vec::new(),
            wounds: Vec::new(),
            stats: Stats::default(),
            debug: ContactDebug::default(),
            fluid_write_cursor: 0,
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

    pub fn fluids(&self) -> &[FluidParticle] {
        &self.fluids
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
        self.springs.push(Spring {
            a,
            b,
            rest: distance(self.points[a].position, self.points[b].position),
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
        });
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
        let index = self.bones.len();
        self.bones.push(BoneSegment {
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
            ..ContactDebug::default()
        };

        self.update_exposure();
        self.integrate(dt, width, floor_y);
        self.update_wounds(dt);
        self.collide_striker(dt, input);

        for _ in 0..self.materials.solver_iterations {
            self.solve_springs();
            self.solve_attachments();
            self.solve_bone_attachments();
            self.solve_bone_joints();
            self.solve_bones();
            self.solve_post_fracture_joints();
            self.solve_bone_fragment_tissue_contacts();
            self.solve_bone_fragment_repulsion();
            self.solve_areas();
            self.constrain_to_world(width, floor_y);
        }

        self.collide_bone_fragments();
        self.update_triangle_damage();
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
        !bone.pinned
            && !bone.splinter
            && bone.fracture_generation < self.materials.max_bone_fracture_depth
            && bone.rest_length >= self.materials.min_bone_fragment_length * 2.0
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
            };

            if self.fluids.len() < self.materials.max_fluid_particles {
                self.fluids.push(particle);
            } else if !self.fluids.is_empty() {
                let index = self.fluid_write_cursor % self.fluids.len();
                self.fluids[index] = particle;
                self.fluid_write_cursor = (self.fluid_write_cursor + 1) % self.fluids.len();
            }
            self.stats.emitted_fluid_particles += 1;
            self.debug.fluid_emitted += 1;
        }
    }

    fn open_wound(
        &mut self,
        center: Vec2,
        direction: Vec2,
        layer: TissueLayer,
        pressure: f64,
        radius: f64,
        depth: f64,
    ) {
        if self.materials.max_wound_sources == 0 || pressure <= 0.0 {
            return;
        }

        let dir = normalized(direction, Vec2 { x: 0.0, y: -1.0 });
        let clamped_pressure = pressure.clamp(0.12, 4.8);
        let clamped_depth = depth.clamp(0.12, 1.35);
        let clamped_radius = radius.clamp(1.3, 5.2);

        let mut target_index = self.wounds.iter().position(|wound| !wound.active);
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

        let index = if let Some(index) = target_index {
            index
        } else if self.wounds.len() < self.materials.max_wound_sources {
            self.wounds.push(WoundSource::default());
            self.wounds.len() - 1
        } else {
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

        let target = &mut self.wounds[index];
        let was_active = target.active;
        target.position = if was_active {
            lerp(target.position, center, 0.35)
        } else {
            center
        };
        target.direction = normalized(
            add(
                scale(target.direction, if was_active { 0.45 } else { 0.0 }),
                dir,
            ),
            dir,
        );
        target.layer = if target.layer == TissueLayer::Muscle || layer == TissueLayer::Muscle {
            TissueLayer::Muscle
        } else {
            TissueLayer::Skin
        };
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
        target.radius = target.radius.max(clamped_radius);
        target.depth = target.depth.max(clamped_depth);
        target.active = true;
        if !was_active {
            self.stats.opened_wounds += 1;
        }
    }

    fn integrate(&mut self, dt: f64, width: f64, floor_y: f64) {
        for point in &mut self.points {
            point.load *= 0.84;
            point.exposure *= 0.92;
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
            let shape_stiffness = if point.layer == TissueLayer::Skin {
                self.materials.skin_shape_stiffness
            } else {
                self.materials.muscle_shape_stiffness
            };
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
                if vx.abs() + vy.abs() < 1.2 {
                    fluid.settled = true;
                }
            }
        }
    }

    fn update_wounds(&mut self, dt: f64) {
        let mut emissions = Vec::new();
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
                * open_factor
                * layer_scale
                * (0.45 + wound.depth * 0.82);
            if wound.age < 0.42 && wound.pressure > self.materials.wound_spray_pressure {
                wound.accumulator +=
                    dt * (wound.pressure - self.materials.wound_spray_pressure) * 2.1;
            }
            let count = (wound.accumulator.floor() as i32).min(4);
            if count > 0 {
                wound.accumulator -= f64::from(count);
                let spray = ((wound.pressure - self.materials.wound_spray_pressure) / 2.4)
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
                    45.0 + wound.pressure * (38.0 + spray * 92.0),
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
            let before = self.stats.emitted_fluid_particles;
            self.emit_fluid(position, direction, count, speed, radius, intensity);
            let emitted = self.stats.emitted_fluid_particles - before;
            self.stats.wound_fluid_particles += emitted;
            self.debug.wound_leaks += emitted;
        }
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
            point.load = point
                .load
                .max(impact * (depth / influence) * contact_strength * profile.tissue_load_scale);
            self.debug.tissue_contacts += 1;
            if depth > self.debug.max_depth {
                self.debug.max_depth = depth;
                self.debug.strongest_contact = point.position;
            }
            self.debug.max_point_load = self.debug.max_point_load.max(point.load);
        }

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
            let stretch_ratio = len / spring.rest.max(EPSILON);
            let endpoint_load = a.load.max(b.load);
            let tear_impulse = if spring.layer == TissueLayer::Muscle {
                spring.tear_impulse * (1.0 - a.exposure.max(b.exposure) * 0.48)
            } else {
                spring.tear_impulse
            };
            self.springs[i].stress =
                (self.springs[i].stress * 0.9).max((stretch_ratio - 1.0).max(0.0));
            if stretch_ratio > spring.tear_stretch
                || (endpoint_load > tear_impulse && stretch_ratio > 1.12)
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
            let diff = (len - spring.rest) / len;
            self.apply_pair_correction_idx(
                spring.a,
                spring.b,
                delta.x * diff * spring.stiffness,
                delta.y * diff * spring.stiffness,
            );
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
            if stretch_ratio > self.materials.bone_joint_break_stretch
                || (impulse > self.materials.bone_joint_break_impulse && stretch_ratio > 1.35)
            {
                joint.broken = true;
                self.stats.broken_bone_joints += 1;
                self.bone_joints[i] = joint;
                continue;
            }
            let relative_angle = wrap_angle(bone_angle(b) - bone_angle(a) - joint.rest_angle);
            let clamped_angle = relative_angle.clamp(joint.min_angle, joint.max_angle);
            let angle_violation = relative_angle - clamped_angle;
            let overextension = angle_violation.abs();
            joint.torque_stress = (joint.torque_stress * 0.9).max(overextension);
            if overextension > self.materials.bone_joint_angular_break
                || (impulse > self.materials.bone_joint_break_impulse
                    && overextension > self.materials.bone_joint_angular_break * 0.45)
            {
                joint.broken = true;
                self.stats.broken_bone_joints += 1;
                self.bone_joints[i] = joint;
                continue;
            }
            self.bones[joint.a].load = self.bones[joint.a]
                .load
                .max(self.bones[joint.b].load * 0.30);
            self.bones[joint.b].load = self.bones[joint.b]
                .load
                .max(self.bones[joint.a].load * 0.30);
            let diff = (len - joint.rest) / len;
            let correction = scale(delta, diff * self.materials.bone_joint_stiffness * 0.5);
            self.apply_bone_anchor_delta_idx(joint.a, joint.t_a, correction.x, correction.y);
            self.apply_bone_anchor_delta_idx(joint.b, joint.t_b, -correction.x, -correction.y);
            let angle_correction =
                angle_violation * self.materials.bone_joint_angular_stiffness * 0.5;
            self.rotate_bone_around_anchor_idx(joint.a, joint.t_a, angle_correction);
            self.rotate_bone_around_anchor_idx(joint.b, joint.t_b, -angle_correction);
            self.bone_joints[i] = joint;
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
        let count = self.bones.len();
        let point_radius_base =
            (self.materials.point_spacing * FRAGMENT_TISSUE_POINT_RADIUS_SCALE).max(4.0);

        for bone_index in 0..count {
            let mut bone = self.bones[bone_index];
            if !free_bone_fragment(bone) {
                continue;
            }

            let mut contact_count = 0;
            let mut strongest_depth: f64 = 0.0;
            for point_index in 0..self.points.len() {
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
                damp_bone_velocity_against_tissue(
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

    fn solve_bone_fragment_repulsion(&mut self) {
        let count = self.bones.len();
        for i in 0..count {
            if !free_bone_fragment(self.bones[i]) {
                continue;
            }
            for j in (i + 1)..count {
                if !free_bone_fragment(self.bones[j]) {
                    continue;
                }
                let a = self.bones[i];
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
                let correction = overlap * self.materials.fragment_repulsion_stiffness;
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
                self.debug.fragment_pair_contacts += 1;
                self.debug.max_fragment_overlap = self.debug.max_fragment_overlap.max(overlap);
            }
        }
    }

    fn collide_bone_fragments(&mut self) {
        let initial_count = self.bones.len();
        for i in 0..initial_count {
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
            let reachable = spring.layer == TissueLayer::Muscle
                || a.exposure.max(b.exposure) > 0.35
                || impulse > self.materials.fragment_damage_impulse * 1.75;
            if !reachable {
                continue;
            }
            let contact = 1.0 - d / (radius * 1.18);
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
        for area in self.areas.clone() {
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
            let lambda = -constraint * area.stiffness / weighted_gradient;
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
        for bone in &mut self.bones {
            if bone.pinned {
                continue;
            }
            bone.a.x = bone.a.x.clamp(margin, width - margin);
            bone.b.x = bone.b.x.clamp(margin, width - margin);
            bone.a.y = bone.a.y.min(floor_y);
            bone.b.y = bone.b.y.min(floor_y);
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
        for triangle in &mut self.triangles {
            if triangle.layer != TissueLayer::Muscle || triangle.failed {
                continue;
            }
            let a = self.points[triangle.a];
            let b = self.points[triangle.b];
            let c = self.points[triangle.c];
            let load = (a.load + b.load + c.load) / 3.0;
            let exposed = (a.exposure + b.exposure + c.exposure) / 3.0;
            let impulse_threshold =
                self.materials.muscle_exposed_tear_impulse + (1.0 - exposed) * 560.0;
            triangle.damage =
                (triangle.damage * 0.996 + (load - impulse_threshold).max(0.0) / 1500.0).min(1.35);
            if triangle.damage > 1.0 {
                triangle.failed = true;
            }
        }
    }

    fn fracture_bone(
        &mut self,
        bone_index: usize,
        fracture_t: f64,
        impulse_normal: Vec2,
        impulse: f64,
    ) {
        if bone_index >= self.bones.len() || !self.can_fracture_bone(self.bones[bone_index]) {
            return;
        }
        let old = self.bones[bone_index];
        let delta = subtract(old.b, old.a);
        let len = hypot(delta.x, delta.y).max(EPSILON);
        let minimum_piece_length = self
            .materials
            .min_bone_fragment_length
            .max(old.radius * 3.2);
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
        first.fracture_impulse = old.fracture_impulse * 0.82;
        first.load = old.load * 0.28;
        first.angular_velocity = (first.angular_velocity
            + spin_sign * fracture_spin * (1.0 - break_t))
            .clamp(-36.0, 36.0);
        self.bones[bone_index] = first;

        let mut second = BoneSegment {
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
            fracture_impulse: old.fracture_impulse * 0.82,
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
        self.damage_tissue_around_fracture(
            crack,
            (old.radius * 3.8).max(24.0 + overload * 10.0),
            load,
        );
        self.stats.fractured_bones += 1;
        self.debug.fractures += 1;
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

    fn apply_pair_correction_idx(
        &mut self,
        a_index: usize,
        b_index: usize,
        correction_x: f64,
        correction_y: f64,
    ) {
        if a_index == b_index || a_index >= self.points.len() || b_index >= self.points.len() {
            return;
        }
        let (a, b) = two_mut(&mut self.points, a_index, b_index);
        let inv_a = if a.pinned { 0.0 } else { 1.0 / a.mass };
        let inv_b = if b.pinned { 0.0 } else { 1.0 / b.mass };
        let sum = inv_a + inv_b;
        if sum <= 0.0 {
            return;
        }
        if !a.pinned {
            a.position.x += correction_x * (inv_a / sum);
            a.position.y += correction_y * (inv_a / sum);
        }
        if !b.pinned {
            b.position.x -= correction_x * (inv_b / sum);
            b.position.y -= correction_y * (inv_b / sum);
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
        if let Some(point) = self.points.get_mut(index) {
            point.exposure = point.exposure.max(exposure);
            point.load = point.load.max(load);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn average_fragment_speed(bone: BoneSegment) -> f64 {
        let a_speed = distance(bone.a, bone.previous_a);
        let b_speed = distance(bone.b, bone.previous_b);
        (a_speed + b_speed) * 0.5
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

fn damp_bone_velocity_against_tissue(
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

fn bone_angle(bone: BoneSegment) -> f64 {
    (bone.b.y - bone.a.y).atan2(bone.b.x - bone.a.x)
}

fn free_bone_fragment(bone: BoneSegment) -> bool {
    !bone.pinned && (bone.fractured || bone.splinter)
}

fn distance_to_segment(point: Vec2, a: Vec2, b: Vec2) -> f64 {
    distance(point, lerp(a, b, segment_t(point, a, b)))
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
