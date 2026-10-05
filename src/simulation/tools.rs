//! Tool geometry, pose, and contact. The app draws each tool from the same pose
//! and dimensions used here, so what is on screen is what touches the body.
//!
//! Every tool's striking part is a segment with a radius: the knife's blade runs
//! along its heading with a thin edge, the sledgehammer's head runs along its
//! heading so a face leads, and the bat's barrel lies across its heading so the
//! side of the barrel leads. `side` points from the striking part toward the
//! hand along the handle of the hammer and bat.

use super::*;

/// Below this speed a tool keeps its orientation instead of turning to follow
/// its motion.
const TOOL_TURN_SPEED: f64 = 40.0;
/// How fast a blunt tool pressed into tissue can turn, in radians per second.
const EMBEDDED_TURN_RATE: f64 = 2.5;
/// How fast tissue steers an embedded knife to follow its stroke.
const EMBEDDED_BLADE_TURN_RATE: f64 = 18.0;
/// The front part of the blade, as a fraction of its length, that cuts what it
/// passes; the rest of the blade only parts tissue already cut by the tip.
const CUTTING_EDGE_SHARE: f64 = 0.25;

/// Dimensions of a tool in world pixels, measured from the point the mouse drives.
#[derive(Clone, Copy, Debug)]
pub struct ToolGeometry {
    /// Contact segment extent behind and ahead of the driven point, along the
    /// heading (knife, hammer) or along `-side` toward the bat's end.
    pub contact_back: f64,
    pub contact_front: f64,
    /// Half thickness of the striking part where it touches tissue.
    pub contact_radius: f64,
    /// Extra reach before contact starts, softening first touch.
    pub contact_padding: f64,
    /// Visual half width of the blade, hammer head, or bat barrel.
    pub body_half_width: f64,
    pub handle_length: f64,
    pub handle_half_width: f64,
}

pub fn tool_geometry(tool: ToolMode) -> ToolGeometry {
    match tool {
        ToolMode::Sharp => ToolGeometry {
            contact_back: 33.0,
            contact_front: 33.0,
            contact_radius: 2.0,
            contact_padding: 1.5,
            body_half_width: 6.5,
            handle_length: 44.0,
            handle_half_width: 4.5,
        },
        ToolMode::Heavy => ToolGeometry {
            contact_back: 17.0,
            contact_front: 17.0,
            contact_radius: 16.0,
            contact_padding: 6.0,
            body_half_width: 16.0,
            handle_length: 132.0,
            handle_half_width: 4.5,
        },
        ToolMode::Blunt => ToolGeometry {
            contact_back: 58.0,
            contact_front: 18.0,
            contact_radius: 12.0,
            contact_padding: 5.0,
            body_half_width: 12.0,
            handle_length: 150.0,
            handle_half_width: 5.0,
        },
    }
}

/// Where a tool's striking part is in the world.
#[derive(Clone, Copy, Debug)]
pub struct ToolPose {
    pub tool: ToolMode,
    /// The point the mouse drives.
    pub center: Vec2,
    /// Direction the tool travels and strikes with.
    pub heading: Vec2,
    /// Perpendicular to `heading`, toward the hand for the hammer and bat.
    pub side: Vec2,
    /// Centerline of the striking part: knife guard to tip, hammer back face to
    /// striking face, bat barrel from the handle side to the end.
    pub contact_start: Vec2,
    pub contact_end: Vec2,
    pub contact_radius: f64,
}

pub fn tool_pose(tool: ToolMode, center: Vec2, heading: Vec2, side: Vec2) -> ToolPose {
    let geometry = tool_geometry(tool);
    let (contact_start, contact_end) = match tool {
        ToolMode::Sharp | ToolMode::Heavy => (
            subtract(center, scale(heading, geometry.contact_back)),
            add(center, scale(heading, geometry.contact_front)),
        ),
        ToolMode::Blunt => (
            add(center, scale(side, geometry.contact_back)),
            subtract(center, scale(side, geometry.contact_front)),
        ),
    };
    ToolPose {
        tool,
        center,
        heading,
        side,
        contact_start,
        contact_end,
        contact_radius: geometry.contact_radius,
    }
}

fn contact_shape(pose: &ToolPose) -> ToolContactShape {
    let geometry = tool_geometry(pose.tool);
    ToolContactShape {
        axis_start: pose.contact_start,
        axis_end: pose.contact_end,
        direction: pose.heading,
        blade_normal: pose.side,
        radius: pose.contact_radius,
        influence: pose.contact_radius + geometry.contact_padding,
        cutting_edge: pose.tool == ToolMode::Sharp,
    }
}

fn rotate_toward(from: Vec2, to: Vec2, max_angle: f64) -> Vec2 {
    let angle = wrap_angle(to.y.atan2(to.x) - from.y.atan2(from.x));
    if angle.abs() <= max_angle {
        return to;
    }
    let step = max_angle * angle.signum();
    let (sin, cos) = step.sin_cos();
    Vec2 {
        x: from.x * cos - from.y * sin,
        y: from.x * sin + from.y * cos,
    }
}

/// Where two segments cross, as (point, position along the second segment).
fn segment_crossing(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> Option<(Vec2, f64)> {
    let r = subtract(a1, a0);
    let s = subtract(b1, b0);
    let denominator = cross(r, s);
    if denominator.abs() < EPSILON {
        return None;
    }
    let offset = subtract(b0, a0);
    let t = cross(offset, s) / denominator;
    let u = cross(offset, r) / denominator;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| (lerp(a0, a1, t), u))
}

impl World {
    /// Direction the current tool faces and strikes with.
    pub fn tool_heading(&self) -> Vec2 {
        self.tool_heading
    }

    /// Perpendicular to the heading, toward the hand on the hammer and bat.
    pub fn tool_side(&self) -> Vec2 {
        self.tool_side
    }

    /// Turns the tool to follow its motion. A blunt tool pressed into tissue
    /// turns slowly. Tissue steers an embedded knife to follow its stroke, like
    /// a scalpel, except that a knife pulled backward keeps its line and
    /// withdraws rather than flipping around inside the wound.
    pub(super) fn update_tool_pose(&mut self, input: &InputState, dt: f64) {
        if input.tool != self.tool_mode {
            self.tool_mode = input.tool;
            self.previous_blade = None;
            self.tool_embedded = false;
        }
        let speed = hypot(input.vx, input.vy);
        if speed > TOOL_TURN_SPEED {
            let target = Vec2 {
                x: input.vx / speed,
                y: input.vy / speed,
            };
            if !self.tool_embedded {
                self.tool_heading = target;
            } else if input.tool != ToolMode::Sharp {
                self.tool_heading =
                    rotate_toward(self.tool_heading, target, EMBEDDED_TURN_RATE * dt);
            } else if dot(target, self.tool_heading) > -0.2 {
                self.tool_heading =
                    rotate_toward(self.tool_heading, target, EMBEDDED_BLADE_TURN_RATE * dt);
            }
        }
        let perpendicular = Vec2 {
            x: -self.tool_heading.y,
            y: self.tool_heading.x,
        };
        // Keep the handle on the same side as the tool turns.
        self.tool_side = if dot(perpendicular, self.tool_side) >= 0.0 {
            perpendicular
        } else {
            scale(perpendicular, -1.0)
        };
    }

    pub(super) fn collide_striker(&mut self, dt: f64, input: &InputState) {
        if !input.active {
            self.tool_embedded = false;
            self.previous_blade = None;
            return;
        }
        let profile = tool_profile(input.tool);
        let speed = hypot(input.vx, input.vy);
        let impact = speed * self.materials.striker_mass * input.power * profile.mass_scale;
        let pose = tool_pose(
            input.tool,
            Vec2 {
                x: input.x,
                y: input.y,
            },
            self.tool_heading,
            self.tool_side,
        );
        let shape = contact_shape(&pose);
        let sweep = self.tool_sweep(input, &shape, dt);

        self.collide_tool_with_bones(input, profile, &sweep, impact, speed, dt);
        let tissue_contacts = if shape.cutting_edge {
            let contacts = self.part_tissue_around_blade(input, profile, &shape, impact, dt);
            if input.down {
                self.cut_with_blade(profile, &sweep, impact);
            }
            contacts
        } else {
            self.push_tissue_with_tool(input, profile, &sweep, impact, dt)
        };
        self.tool_embedded = input.down && tissue_contacts > 0;

        for pose in &sweep {
            self.lacerate_major_vessels_from_striker(input, pose, impact);
            self.penetrate_organs_from_striker(input, pose, impact);
        }
        if input.down && profile.crush_tear_scale > 0.0 {
            self.crush_tear_under_tool(profile, &shape, impact);
        }
        self.previous_blade =
            (shape.cutting_edge && input.down).then_some((shape.axis_start, shape.axis_end));
    }

    /// The contact shape at even steps along this frame's motion, ending at the
    /// current pose, so a fast swing cannot skip over a thin limb. The knife
    /// interpolates from where its blade was last frame, rotation included.
    fn tool_sweep(
        &self,
        input: &InputState,
        shape: &ToolContactShape,
        dt: f64,
    ) -> Vec<ToolContactShape> {
        let (start_a, start_b) = match self.previous_blade {
            Some(previous) if shape.cutting_edge => previous,
            _ => {
                let back = Vec2 {
                    x: input.vx * dt,
                    y: input.vy * dt,
                };
                (
                    subtract(shape.axis_start, back),
                    subtract(shape.axis_end, back),
                )
            }
        };
        let travel = distance(start_a, shape.axis_start).max(distance(start_b, shape.axis_end));
        let spacing = (shape.radius * 0.8).max(self.materials.point_spacing * 0.45);
        let steps = ((travel / spacing).ceil() as usize).clamp(1, 10);
        (0..=steps)
            .map(|i| {
                let t = i as f64 / steps as f64;
                let axis_start = lerp(start_a, shape.axis_start, t);
                let axis_end = lerp(start_b, shape.axis_end, t);
                ToolContactShape {
                    axis_start,
                    axis_end,
                    ..*shape
                }
            })
            .collect()
    }

    fn collide_tool_with_bones(
        &mut self,
        input: &InputState,
        profile: ToolProfile,
        sweep: &[ToolContactShape],
        impact: f64,
        speed: f64,
        dt: f64,
    ) {
        let initial_bone_count = self.bones.len();
        for i in 0..initial_bone_count {
            let mut bone = self.bones[i];
            bone.load *= 0.88;
            // Contact where this frame's sweep came closest to the bone.
            let Some((shape, closest_pair)) = sweep
                .iter()
                .map(|shape| {
                    (
                        shape,
                        closest_segment_points(shape.axis_start, shape.axis_end, bone.a, bone.b),
                    )
                })
                .min_by(|a, b| a.1.distance.total_cmp(&b.1.distance))
            else {
                continue;
            };
            let influence = shape.influence;
            let t = closest_pair.t_b;
            let closest = closest_pair.point_b;
            let tool_contact = closest_pair.point_a;
            let mut dist = closest_pair.distance;
            if !input.down || dist > influence + bone.radius {
                self.bones[i] = bone;
                continue;
            }
            let mut normal = normalized(
                subtract(closest, tool_contact),
                if shape.cutting_edge {
                    shape.blade_normal
                } else {
                    shape.direction
                },
            );
            if dist < EPSILON && !shape.cutting_edge && speed > EPSILON {
                normal = shape.direction;
                dist = 1.0;
            }
            let depth = (influence + bone.radius - dist).max(0.0);
            let contact = (1.0 - ((dist - bone.radius) / influence).clamp(0.0, 1.0)).max(0.0);
            let direct_load = (impact + self.materials.bone_direct_pressure * input.power)
                * contact
                * profile.bone_load_scale;
            // The bone solver fractures any bone whose load exceeds its strength,
            // so the tool's fracture resistance is applied to the load itself.
            bone.load = bone
                .load
                .max(direct_load / profile.fracture_scale.max(EPSILON));
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
            let should_fracture = self.can_fracture_bone(bone) && bone.load > bone.fracture_impulse;
            self.bones[i] = bone;
            if should_fracture {
                self.fracture_bone(i, t, normal, direct_load);
            }
        }
    }

    /// Blunt tools shove tissue out of the way and drag it along. Each sweep
    /// step pushes, but load and bruising are recorded once per point from its
    /// hardest hit. Returns how many tissue points were touched.
    fn push_tissue_with_tool(
        &mut self,
        input: &InputState,
        profile: ToolProfile,
        sweep: &[ToolContactShape],
        impact: f64,
        dt: f64,
    ) -> usize {
        // Hardest contact load per point this step; None if untouched.
        let mut hardest: Vec<Option<f64>> = vec![None; self.points.len()];
        // The drag along the motion is shared out over the sweep steps. Earlier
        // steps only nudge what the tool passed over, so a fast swing still
        // catches a thin limb without shoving tissue several times per frame;
        // the current pose pushes fully, as a single contact would.
        let share = 1.0 / sweep.len() as f64;
        for (step, shape) in sweep.iter().enumerate() {
            let push_share = if step + 1 == sweep.len() { 1.0 } else { share };
            let influence = shape.influence;
            for (index, point) in self.points.iter_mut().enumerate() {
                if point.pinned {
                    continue;
                }
                let point_contact = sample_point_contact(point.position, shape);
                if point_contact.distance > influence {
                    continue;
                }
                let mut contact_strength = (if input.down { 0.58 } else { 0.16 })
                    * profile.tissue_push_scale
                    * (0.85 + input.power * 0.15);
                if point.layer == TissueLayer::Muscle {
                    contact_strength *=
                        self.materials.direct_muscle_contact + point.exposure * 0.82;
                }
                let depth = influence - point_contact.distance;
                let drag = dt * 0.45 * contact_strength * profile.drag_scale * share;
                let push = depth * contact_strength * profile.rebound_scale * push_share;
                point.position.x += point_contact.normal.x * push + input.vx * drag;
                point.position.y += point_contact.normal.y * push + input.vy * drag;
                let contact_load =
                    impact * (depth / influence) * contact_strength * profile.tissue_load_scale;
                hardest[index] =
                    Some(hardest[index].map_or(contact_load, |load| load.max(contact_load)));
                if depth > self.debug.max_depth {
                    self.debug.max_depth = depth;
                    self.debug.strongest_contact = point.position;
                }
            }
        }

        let mut touched = 0;
        for (index, contact_load) in hardest.into_iter().enumerate() {
            let Some(contact_load) = contact_load else {
                continue;
            };
            touched += 1;
            let point = &mut self.points[index];
            point.load = point.load.max(contact_load);
            if apply_point_contusion(point, self.materials, contact_load, profile.contusion_scale) {
                self.stats.contusion_events += 1;
                self.debug.contusion_events += 1;
                self.debug.max_contusion = self.debug.max_contusion.max(point.contusion);
            }
            self.debug.tissue_contacts += 1;
            self.debug.max_point_load = self.debug.max_point_load.max(point.load);
        }
        touched
    }

    /// A blade only nudges tissue off its thin edge sideways; it parts tissue
    /// rather than shoving it. Points ahead of the tip are left for the tip to
    /// pierce. Returns how many tissue points lie along the blade, which marks
    /// it as embedded.
    fn part_tissue_around_blade(
        &mut self,
        input: &InputState,
        profile: ToolProfile,
        shape: &ToolContactShape,
        impact: f64,
        dt: f64,
    ) -> usize {
        let along_reach = self.materials.point_spacing * 0.7;
        let mut alongside = 0;
        for point in &mut self.points {
            if point.pinned {
                continue;
            }
            let t = segment_t(point.position, shape.axis_start, shape.axis_end);
            if t <= 0.0 || t >= 1.0 {
                continue;
            }
            let point_contact = sample_point_contact(point.position, shape);
            if point_contact.distance <= along_reach {
                alongside += 1;
            }
            if point_contact.distance > shape.influence {
                continue;
            }
            let depth = shape.influence - point_contact.distance;
            let friction = if input.down { 0.06 } else { 0.0 };
            point.position.x += point_contact.normal.x * depth + input.vx * dt * friction;
            point.position.y += point_contact.normal.y * depth + input.vy * dt * friction;
            let contact_load =
                impact * (depth / shape.influence) * 0.25 * profile.tissue_push_scale;
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
        alongside
    }

    /// Severs the springs the knife's tip and leading edge passed through this
    /// frame. Only fibers the blade actually crosses part, so a cut is a clean
    /// line along the stroke; a slow, light stroke slides through without cutting.
    fn cut_with_blade(&mut self, profile: ToolProfile, sweep: &[ToolContactShape], impact: f64) {
        let reach = sweep
            .iter()
            .flat_map(|shape| [shape.axis_start, shape.axis_end])
            .fold(
                (
                    Vec2 {
                        x: f64::MAX,
                        y: f64::MAX,
                    },
                    Vec2 {
                        x: f64::MIN,
                        y: f64::MIN,
                    },
                ),
                |(min, max), p| {
                    (
                        Vec2 {
                            x: min.x.min(p.x),
                            y: min.y.min(p.y),
                        },
                        Vec2 {
                            x: max.x.max(p.x),
                            y: max.y.max(p.y),
                        },
                    )
                },
            );
        let blade_normal = sweep
            .last()
            .map(|shape| shape.blade_normal)
            .unwrap_or_default();
        let mut events = Vec::new();
        for spring_index in 0..self.springs.len() {
            let spring = self.springs[spring_index];
            if spring.broken {
                continue;
            }
            let a = self.points[spring.a];
            let b = self.points[spring.b];
            if a.position.x.max(b.position.x) < reach.0.x
                || a.position.x.min(b.position.x) > reach.1.x
                || a.position.y.max(b.position.y) < reach.0.y
                || a.position.y.min(b.position.y) > reach.1.y
            {
                continue;
            }
            // Fibers the tip passed through, or that lie across the cutting edge
            // just behind it. The back of the blade follows the tip's path, so
            // letting it cut would scythe through tissue whenever the knife turns.
            let crossed = sweep.iter().enumerate().skip(1).any(|(i, shape)| {
                let edge_start = lerp(shape.axis_end, shape.axis_start, CUTTING_EDGE_SHARE);
                segment_crossing(a.position, b.position, edge_start, shape.axis_end).is_some()
                    || segment_crossing(
                        a.position,
                        b.position,
                        sweep[i - 1].axis_end,
                        shape.axis_end,
                    )
                    .is_some()
            });
            if !crossed {
                continue;
            }
            let layer_scale = if spring.layer == TissueLayer::Skin {
                1.0
            } else {
                0.78 + a.exposure.max(b.exposure) * 0.42
            };
            let pressure = impact * profile.cut_pressure_scale * layer_scale;
            let threshold = spring.tear_impulse * self.materials.sharp_tool_tear_pressure;
            self.springs[spring_index].stress = self.springs[spring_index]
                .stress
                .max(pressure / threshold.max(1.0));
            if pressure <= threshold {
                continue;
            }
            self.springs[spring_index].broken = true;
            let exposure = if spring.layer == TissueLayer::Skin {
                0.92
            } else {
                1.0
            };
            self.bump_point_exposure_load(spring.a, exposure, pressure * 0.18);
            self.bump_point_exposure_load(spring.b, exposure, pressure * 0.18);
            if spring.layer == TissueLayer::Skin {
                self.stats.broken_skin += 1;
                self.fresh_skin_cuts.push(spring_index);
            } else {
                self.stats.broken_muscle += 1;
                if spring.fiber {
                    self.stats.muscle_fiber_tears += 1;
                    self.debug.muscle_fiber_tears += 1;
                }
            }
            let center = midpoint(a.position, b.position);
            let tangent = normalized(subtract(b.position, a.position), Vec2 { x: 1.0, y: 0.0 });
            let spring_normal = Vec2 {
                x: -tangent.y,
                y: tangent.x,
            };
            // Blood leaves the cut sideways, away from the blade plane.
            let normal = if dot(blade_normal, spring_normal) < 0.0 {
                scale(blade_normal, -1.0)
            } else {
                blade_normal
            };
            events.push((center, normal, spring.layer, pressure));
        }
        for (center, normal, layer, pressure) in events {
            let skin = layer == TissueLayer::Skin;
            self.emit_fluid(
                center,
                normal,
                if skin { 6 } else { 4 },
                120.0 + pressure * self.materials.fluid_impact_scale * 0.42,
                if skin { 2.1 } else { 1.8 },
                profile.fluid_scale,
            );
            self.open_wound(
                center,
                normal,
                layer,
                pressure / if skin { 1250.0 } else { 1050.0 },
                if skin { 2.1 } else { 1.8 },
                if skin { 0.58 } else { 0.92 },
            );
        }
    }

    /// A heavy head can split tissue it crushes against bone and itself.
    fn crush_tear_under_tool(
        &mut self,
        profile: ToolProfile,
        shape: &ToolContactShape,
        impact: f64,
    ) {
        let influence = shape.influence;
        let mut events = Vec::new();
        for spring_index in 0..self.springs.len() {
            let spring = self.springs[spring_index];
            if spring.broken {
                continue;
            }
            let a = self.points[spring.a];
            let b = self.points[spring.b];
            let center = midpoint(a.position, b.position);
            let tear_contact = sample_point_contact(center, shape);
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
            let pressure = impact * profile.crush_tear_scale * contact * layer_scale;
            let threshold = spring.tear_impulse * self.materials.sharp_tool_tear_pressure;
            self.springs[spring_index].stress = self.springs[spring_index]
                .stress
                .max(pressure / threshold.max(1.0));
            if pressure <= threshold {
                continue;
            }
            self.springs[spring_index].broken = true;
            let exposure = if spring.layer == TissueLayer::Skin {
                0.92
            } else {
                1.0
            };
            self.bump_point_exposure_load(spring.a, exposure, pressure * 0.18);
            self.bump_point_exposure_load(spring.b, exposure, pressure * 0.18);
            if spring.layer == TissueLayer::Skin {
                self.stats.broken_skin += 1;
            } else {
                self.stats.broken_muscle += 1;
            }
            let tangent = normalized(subtract(b.position, a.position), Vec2 { x: 1.0, y: 0.0 });
            events.push((
                center,
                Vec2 {
                    x: -tangent.y,
                    y: tangent.x,
                },
                spring.layer,
                pressure,
            ));
        }
        for (center, normal, layer, pressure) in events {
            let skin = layer == TissueLayer::Skin;
            self.emit_fluid(
                center,
                normal,
                if skin { 6 } else { 4 },
                120.0 + pressure * self.materials.fluid_impact_scale * 0.42,
                if skin { 2.1 } else { 1.8 },
                profile.fluid_scale,
            );
            self.open_wound(
                center,
                normal,
                layer,
                pressure / if skin { 1250.0 } else { 1050.0 },
                if skin { 2.1 } else { 1.8 },
                if skin { 0.58 } else { 0.92 },
            );
        }
    }
}
