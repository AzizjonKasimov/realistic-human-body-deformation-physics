//! What to print about each frame as a scenario plays: the fastest tissue,
//! what each phase did to a traced point or bone, how the body holds its
//! stance, and where a lone scenario's broken pieces ended up.

use super::*;

/// What to print about each frame as a scenario plays.
#[derive(Clone, Copy, Default)]
pub(crate) struct Watch {
    /// Report tissue moving faster than this, in pixels per second.
    pub(crate) fastest: Option<f64>,
    /// Follow a point or bone through each phase of these frames.
    pub(crate) trace: Option<(rp::Traced, i32, i32)>,
    /// Print how the body holds its stance every this many frames.
    pub(crate) stance: Option<i32>,
    /// Keep a journal of what happens, to print when the run ends.
    pub(crate) journal: bool,
    /// Lay the body out in numbers on one frame.
    pub(crate) dump: Option<Dump>,
    /// Draw the flesh's triangles round a place on one frame.
    pub(crate) mesh: Option<MeshShot>,
}

impl Watch {
    /// The body for `scenario`, following what `--trace` names and keeping
    /// the journal `--journal` asks for.
    pub(crate) fn body(&self, scenario: &Scenario) -> rp::World {
        let mut world =
            rp::create_layered_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, scenario.materials());
        if let Some((traced, _, _)) = self.trace {
            world.trace(&[traced]);
        }
        if self.journal {
            world.keep_journal();
        }
        world
    }

    /// Prints what `--fastest` and `--trace` ask about `frame`.
    pub(crate) fn report(
        &self,
        name: &str,
        frame: i32,
        world: &rp::World,
        last_positions: &[rp::Vec2],
    ) {
        if let Some(limit) = self.fastest {
            report_fastest(name, frame, world, last_positions, limit);
        }
        if let Some((traced, from, to)) = self.trace {
            if (from..=to).contains(&frame) {
                report_trace(name, frame, world, traced);
            }
        }
        if let Some(dump) = self.dump {
            dump.report(name, frame, world);
        }
        if let Some(mesh) = self.mesh {
            mesh.draw(name, frame, world);
        }
        if let Some(every) = self.stance {
            if (frame + 1) % every.max(1) == 0 {
                report_stance(name, frame, world, last_positions);
            }
        }
    }
}

/// Prints how the body holds its stance on `frame`: each joint's turn off
/// the guard, in degrees, and its muscles' torque, in newton metres; how far
/// the hips and the fists are from where they stood, the flesh from its
/// place on the bones, and the bones farthest from where they were built;
/// and the fastest tissue.
fn report_stance(name: &str, frame: i32, world: &rp::World, last_positions: &[rp::Vec2]) {
    let bones = world.bones();
    let midline = bones
        .iter()
        .find(|bone| bone.part == rp::BonePart::Spine)
        .map_or(0.0, |spine| spine.home_a.x);
    let mut joints = Vec::new();
    for (index, joint) in world.bone_joints().iter().enumerate() {
        let (Some(a), Some(b)) = (bones.get(joint.a), bones.get(joint.b)) else {
            continue;
        };
        if b.part == rp::BonePart::Rib && !joint.fracture_link {
            continue;
        }
        let side = if b.home_b.x < midline - 1.0 {
            "L "
        } else if b.home_b.x > midline + 1.0 {
            "R "
        } else {
            ""
        };
        let turn = world.joint_turn(index).unwrap_or(0.0).to_degrees();
        let anchor = |bone: &rp::BoneSegment, t: f64| rp::Vec2 {
            x: bone.a.x + (bone.b.x - bone.a.x) * t,
            y: bone.a.y + (bone.b.y - bone.a.y) * t,
        };
        let (from, to) = (anchor(a, joint.t_a), anchor(b, joint.t_b));
        let gap = (to.x - from.x).hypot(to.y - from.y);
        let loose = |what: &str| {
            format!(
                " {what}{} {gap:.0}/{:.0}px",
                if joint.fracture_link { " link" } else { "" },
                joint.post_fracture_rest.max(joint.rest)
            )
        };
        // A whole joint pulled past its rest gap shows by how much.
        let state = if joint.broken {
            loose("broken")
        } else if joint.post_fracture_limited {
            loose("loose")
        } else if let Some(slip) = world.joint_slip(index).filter(|&slip| slip > 0.5) {
            // A whole joint holds its second bone at a place on the first;
            // how far that bone has slipped from there.
            format!(" slipped {slip:.1}px")
        } else if gap > joint.rest + 0.5 {
            format!(" apart +{:.1}px", gap - joint.rest)
        } else {
            String::new()
        };
        joints.push(format!(
            "{side}{}-{} {turn:+.1}deg {:+.1}Nm{state}",
            a.part.name(),
            b.part.name(),
            world.joint_torque(index)
        ));
    }
    let hips: Vec<String> = world
        .hip_holds()
        .iter()
        .filter_map(|hold| {
            let bone = bones.get(hold.bone)?;
            let at = if hold.end_b { bone.b } else { bone.a };
            Some(format!(
                "{:.1}",
                (at.x - hold.home.x).hypot(at.y - hold.home.y)
            ))
        })
        .collect();
    let fists: Vec<String> = bones
        .iter()
        .filter(|bone| bone.part == rp::BonePart::Hand)
        .map(|bone| {
            format!(
                "({:+.1}, {:+.1})",
                bone.b.x - bone.home_b.x,
                bone.b.y - bone.home_b.y
            )
        })
        .collect();
    // How far the body leans off its mirror image: for each whole bone, how
    // far its mirror image lies from the nearest whole bone.
    let whole: Vec<&rp::BoneSegment> = bones
        .iter()
        .filter(|bone| !bone.fractured && !bone.splinter)
        .collect();
    let uneven = whole
        .iter()
        .map(|bone| {
            let flip = |p: rp::Vec2| rp::Vec2 {
                x: 2.0 * midline - p.x,
                y: p.y,
            };
            let gap = |p: rp::Vec2, q: rp::Vec2| (p.x - q.x).hypot(p.y - q.y);
            let (a, b) = (flip(bone.a), flip(bone.b));
            let off = whole
                .iter()
                .map(|other| {
                    (gap(other.a, a).max(gap(other.b, b))).min(gap(other.a, b).max(gap(other.b, a)))
                })
                .fold(f64::INFINITY, f64::min);
            (off, bone.part)
        })
        .fold((0.0, rp::BonePart::Other), |best, here| {
            if here.0 > best.0 {
                here
            } else {
                best
            }
        });
    println!(
        "{name} frame {frame}: most uneven bone {} off its mirror by {:.1} px",
        uneven.1.name(),
        uneven.0
    );
    println!(
        "{name} frame {frame}: hips off {} px, fists off {} px, fastest tissue {:.0} px/s, outline contacts {} (deepest {:.1} px)",
        hips.join(" "),
        fists.join(" "),
        fastest_point_speed(world, last_positions),
        world.debug().outline_contacts,
        world.debug().max_outline_overlap,
    );
    // How far the flesh hangs off its place on the skeleton
    // (`PostureBinding`): on average, at most, and how far down on average.
    let flesh: Vec<String> = [
        (rp::TissueLayer::Skin, "skin"),
        (rp::TissueLayer::Muscle, "muscle"),
    ]
    .iter()
    .map(|&(layer, label)| {
        let offsets: Vec<rp::Vec2> = world
            .points()
            .iter()
            .enumerate()
            .filter(|(_, point)| point.layer == layer && !point.pinned && !point.is_ghost())
            .filter_map(|(index, point)| {
                let place = world.posture_target(index)?;
                Some(rp::Vec2 {
                    x: point.position.x - place.x,
                    y: point.position.y - place.y,
                })
            })
            .collect();
        let count = offsets.len().max(1) as f64;
        let mean = offsets.iter().map(|o| o.x.hypot(o.y)).sum::<f64>() / count;
        let most = offsets.iter().map(|o| o.x.hypot(o.y)).fold(0.0, f64::max);
        let down = offsets.iter().map(|o| o.y).sum::<f64>() / count;
        format!("{label} {mean:.1} (most {most:.1}, down {down:+.1})")
    })
    .collect();
    println!(
        "{name} frame {frame}: flesh off its place on the bones, px: {}",
        flesh.join(", ")
    );
    // The whole bones farthest from where they were built, by their
    // farther end.
    let mut moved: Vec<(f64, String)> = bones
        .iter()
        .filter(|bone| !bone.fractured && !bone.splinter)
        .map(|bone| {
            let off = |p: rp::Vec2, home: rp::Vec2| rp::Vec2 {
                x: p.x - home.x,
                y: p.y - home.y,
            };
            let (a, b) = (off(bone.a, bone.home_a), off(bone.b, bone.home_b));
            let side = if bone.home_b.x < midline - 1.0 {
                "L "
            } else if bone.home_b.x > midline + 1.0 {
                "R "
            } else {
                ""
            };
            (
                a.x.hypot(a.y).max(b.x.hypot(b.y)),
                format!(
                    "{side}{} a ({:+.1}, {:+.1}) b ({:+.1}, {:+.1})",
                    bone.part.name(),
                    a.x,
                    a.y,
                    b.x,
                    b.y
                ),
            )
        })
        .collect();
    moved.sort_by(|p, q| q.0.total_cmp(&p.0));
    let moved: Vec<String> = moved.into_iter().take(4).map(|(_, text)| text).collect();
    println!(
        "{name} frame {frame}: bones farthest from where they were built, px: {}",
        moved.join("  |  ")
    );
    for row in joints.chunks(4) {
        println!("    {}", row.join("  |  "));
    }
}

/// Prints the tissue point that moved fastest on `frame`, when it moved
/// faster than `limit` px/s since `last_positions`: its layer, whether it is
/// a cut's ghost, what it weighs, whether its skin still holds to the muscle,
/// and how fast the tool moves.
fn report_fastest(
    name: &str,
    frame: i32,
    world: &rp::World,
    last_positions: &[rp::Vec2],
    limit: f64,
) {
    let dt = world.materials().fixed_dt;
    let Some((index, speed)) = world
        .points()
        .iter()
        .zip(last_positions)
        .enumerate()
        .filter(|(_, (point, _))| !point.pinned)
        .map(|(index, (point, last))| {
            let moved = (point.position.x - last.x).hypot(point.position.y - last.y);
            (index, moved / dt)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
    else {
        return;
    };
    if speed <= limit {
        return;
    }
    let point = world.points()[index];
    let held = world
        .attachments()
        .iter()
        .filter(|tie| !tie.broken && (tie.skin_point == index || tie.muscle_point == index))
        .count();
    println!(
        "{name} frame {frame}: point {index} ({:?}{}{}) {:.0} g moves {speed:.0} px/s at ({:.0}, {:.0}), {held} skin ties, tool {:.0} px/s, outline contacts {} (deepest {:.1} px)",
        point.layer,
        if point.is_ghost() { ", a cut's ghost" } else { "" },
        if point.on_cut { ", on a tear" } else { "" },
        point.mass * 100.0,
        point.position.x,
        point.position.y,
        world.debug().striker_speed,
        world.debug().outline_contacts,
        world.debug().max_outline_overlap,
    );
}

/// Prints what each phase of `frame` did to the point or bone `traced`, the
/// phases that barely touched it left out.
fn report_trace(name: &str, frame: i32, world: &rp::World, traced: rp::Traced) {
    let place = match traced {
        rp::Traced::Point(index) => world.points().get(index).map(|point| {
            format!(
                "point {index} ({:?}{}, {:.0} g) at ({:.1}, {:.1}), outline hold ({:.0}, {:.0}), on bones {:?} by {:.2?}",
                point.layer,
                if point.is_ghost() {
                    ", a cut's ghost"
                } else {
                    ""
                },
                point.mass * 100.0,
                point.position.x,
                point.position.y,
                point.wrap.x,
                point.wrap.y,
                point.posture.bones,
                point.posture.weights,
            )
        }),
        rp::Traced::Bone(index) => world.bones().get(index).map(|bone| {
            format!(
                "bone {index} ({:?}{}{}) from ({:.1}, {:.1}) to ({:.1}, {:.1})",
                bone.part,
                if bone.fractured { ", broken" } else { "" },
                if bone.splinter { ", a splinter" } else { "" },
                bone.a.x,
                bone.a.y,
                bone.b.x,
                bone.b.y
            )
        }),
    };
    let Some(place) = place else {
        return;
    };
    println!("{name} frame {frame}: {place}");
    for stage in world.traced_stages() {
        let moved = stage.moved.x.hypot(stage.moved.y);
        let change = (stage.velocity_after.x - stage.velocity_before.x)
            .hypot(stage.velocity_after.y - stage.velocity_before.y);
        if moved < 0.05 && change < 20.0 {
            continue;
        }
        println!(
            "    {:<18} moved ({:>6.2}, {:>6.2}) px, speed ({:>6.0}, {:>6.0}) -> ({:>6.0}, {:>6.0}) px/s",
            stage.phase,
            stage.moved.x,
            stage.moved.y,
            stage.velocity_before.x,
            stage.velocity_before.y,
            stage.velocity_after.x,
            stage.velocity_after.y
        );
    }
}

/// Lists each piece of broken bone at the end: what it is, which way it
/// lies, what holds it (ties to its own flesh, holds of the flesh around it,
/// holds of crushed flesh), and, for a piece away from all flesh, how far.
pub(crate) fn report_pieces_out(world: &rp::World) {
    let flesh = world.flesh_points();
    let points = world.points();
    let spacing = world.materials().point_spacing;
    for (index, bone) in world.bones().iter().enumerate() {
        if !bone.fractured && !bone.splinter {
            continue;
        }
        let holds = |kind: fn(&rp::BoneAttachment) -> bool| {
            world
                .bone_attachments()
                .iter()
                .filter(|tie| tie.bone == index && !tie.broken && kind(tie))
                .count()
        };
        let middle = rp::Vec2 {
            x: (bone.a.x + bone.b.x) * 0.5,
            y: (bone.a.y + bone.b.y) * 0.5,
        };
        println!(
            "  piece {index} ({}{}{}, {:.0} px long) at {:.0} degrees, middle ({:.0}, {:.0}) {} the skin: ties {}, flesh holds {}, crushed holds {}",
            bone.part.name(),
            if bone.splinter { ", a splinter" } else { "" },
            if bone.open_break { ", open" } else { "" },
            bone.rest_length,
            (bone.b.y - bone.a.y).atan2(bone.b.x - bone.a.x).to_degrees(),
            middle.x,
            middle.y,
            if inside_the_skin(world, middle) { "inside" } else { "OUTSIDE" },
            holds(|tie| !tie.holds_piece),
            holds(|tie| tie.holds_piece && !tie.in_crushed_flesh),
            holds(|tie| tie.holds_piece && tie.in_crushed_flesh),
        );
        // The tissue that keeps its place on a piece outside the skin, and
        // how far it lies from that place.
        if !bone.splinter && !inside_the_skin(world, middle) {
            for (point_index, point) in points.iter().enumerate() {
                let binding = point.posture;
                let Some(slot) = (0..2).find(|&slot| binding.bones[slot] == index) else {
                    continue;
                };
                let Some(target) = world.posture_target(point_index) else {
                    continue;
                };
                let other = binding.bones[1 - slot];
                println!(
                    "    {:?} point {point_index}{} at ({:.0}, {:.0}), {:.0}% on it{}, {:.1} px off its place",
                    point.layer,
                    if point.is_ghost() { " (ghost)" } else { "" },
                    point.position.x,
                    point.position.y,
                    binding.weights[slot] * 100.0,
                    world.bones().get(other).map_or(String::new(), |bone| format!(
                        " and {:.0}% on bone {other} ({})",
                        binding.weights[1 - slot] * 100.0,
                        bone.part.name()
                    )),
                    (point.position.x - target.x).hypot(point.position.y - target.y),
                );
            }
        }
        let nearest = points
            .iter()
            .zip(&flesh)
            .filter(|(_, &flesh)| flesh)
            .map(|(point, _)| (point.position.x - middle.x).hypot(point.position.y - middle.y))
            .fold(f64::INFINITY, f64::min);
        if nearest <= spacing * 3.0 {
            continue;
        }
        println!(
            "  piece {index} ({}{}{}, {:.0} px long) at ({:.0}, {:.0}) lies {:.1} spacings from all flesh",
            bone.part.name(),
            if bone.splinter { ", a splinter" } else { "" },
            if bone.open_break { ", open" } else { "" },
            bone.rest_length,
            middle.x,
            middle.y,
            nearest / spacing
        );
    }
}
