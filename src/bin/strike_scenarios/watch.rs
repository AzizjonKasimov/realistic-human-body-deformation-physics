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
    /// Print how close two fighters' bones come every this many frames.
    pub(crate) between: Option<i32>,
    /// Keep a journal of what happens, to print when the run ends.
    pub(crate) journal: bool,
    /// Lay the body out in numbers on one frame.
    pub(crate) dump: Option<Dump>,
    /// Draw the flesh's triangles round a place on one frame.
    pub(crate) mesh: Option<MeshShot>,
}

impl Watch {
    /// Whether anything is asked to be watched.
    pub(crate) fn any(&self) -> bool {
        self.fastest.is_some()
            || self.trace.is_some()
            || self.stance.is_some()
            || self.between.is_some()
            || self.journal
            || self.dump.is_some()
            || self.mesh.is_some()
    }

    /// The body for `scenario`, following what `--trace` names and keeping
    /// the journal `--journal` asks for.
    pub(crate) fn body(&self, scenario: &Scenario) -> rp::World {
        let mut world = settled_body(SCENARIO_WIDTH, SCENARIO_HEIGHT, scenario.materials());
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
        if let Some(every) = self.between {
            if (frame + 1) % every.max(1) == 0 {
                report_between(name, frame, world);
            }
        }
    }
}

/// For each body, where its hips and feet are against where they stood as
/// built: how far the pelvis has moved and tilted, how far each foot has
/// lifted off the floor (its bone's lower end, which rests just above it)
/// and slid along it, and how far each leg leans from upright. Left and
/// right are as seen on screen, from each body's own middle.
fn feet_and_hips(world: &rp::World) -> Vec<String> {
    let bones = world.bones();
    let angle = |a: rp::Vec2, b: rp::Vec2| (b.y - a.y).atan2(b.x - a.x);
    (0..world.body_count())
        .filter_map(|body| {
            let of = |part: rp::BonePart| {
                bones
                    .iter()
                    .filter(move |bone| {
                        bone.body as usize == body
                            && bone.part == part
                            && !bone.fractured
                            && !bone.splinter
                    })
                    .collect::<Vec<_>>()
            };
            let pelvis = *of(rp::BonePart::Pelvis).first()?;
            let middle = (pelvis.home_a.x + pelvis.home_b.x) * 0.5;
            let moved = rp::Vec2 {
                x: (pelvis.a.x + pelvis.b.x - pelvis.home_a.x - pelvis.home_b.x) * 0.5,
                y: (pelvis.a.y + pelvis.b.y - pelvis.home_a.y - pelvis.home_b.y) * 0.5,
            };
            let tilt = (angle(pelvis.a, pelvis.b) - angle(pelvis.home_a, pelvis.home_b))
                .to_degrees();
            let mut feet: Vec<(f64, String)> = of(rp::BonePart::Foot)
                .into_iter()
                .map(|foot| {
                    let low = foot.a.y.max(foot.b.y);
                    let rest = foot.home_a.y.max(foot.home_b.y);
                    let slid = (foot.a.x + foot.b.x - foot.home_a.x - foot.home_b.x) * 0.5;
                    let side = if (foot.home_a.x + foot.home_b.x) * 0.5 < middle {
                        "left"
                    } else {
                        "right"
                    };
                    (
                        foot.home_a.x,
                        format!("{side} foot up {:.1} px, slid {slid:+.1}", rest - low),
                    )
                })
                .collect();
            feet.sort_by(|p, q| p.0.total_cmp(&q.0));
            let legs: Vec<String> = of(rp::BonePart::Thigh)
                .into_iter()
                .map(|thigh| {
                    let lean = (angle(thigh.a, thigh.b) - angle(thigh.home_a, thigh.home_b))
                        .to_degrees();
                    format!("{lean:+.1}")
                })
                .collect();
            let feet: Vec<String> = feet.into_iter().map(|(_, text)| text).collect();
            Some(format!(
                "body {body}: hips moved ({:+.1}, {:+.1}) px, tilted {tilt:+.1} deg; {}; thighs turned {} deg",
                moved.x,
                moved.y,
                feet.join(", "),
                legs.join(" ")
            ))
        })
        .collect()
}

/// How far each bone's skin lies from it at rest, in pixels: the farthest
/// of the skin points whose place on the skeleton lies mostly on it, as
/// built. A bone no skin keeps its place on, such as a broken piece, has
/// none.
fn flesh_radii(world: &rp::World) -> Vec<f64> {
    let bones = world.bones();
    let mut radii = vec![0.0_f64; bones.len()];
    for (point, hold) in world.points().iter().zip(world.point_holds()) {
        if point.layer != rp::TissueLayer::Skin || point.is_ghost() {
            continue;
        }
        let binding = hold.posture;
        for slot in 0..2 {
            let bone = binding.bones[slot];
            if binding.weights[slot] < 0.5 {
                continue;
            }
            let Some(segment) = bones.get(bone) else {
                continue;
            };
            if segment.fractured || segment.splinter {
                continue;
            }
            let reach = point_to_segment(point.home, segment.home_a, segment.home_b);
            radii[bone] = radii[bone].max(reach);
        }
    }
    radii
}

fn point_to_segment(p: rp::Vec2, a: rp::Vec2, b: rp::Vec2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length_sq = dx * dx + dy * dy;
    let t = if length_sq > 1.0e-12 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / length_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - (a.x + dx * t)).hypot(p.y - (a.y + dy * t))
}

/// How close two segments come: nothing where they cross, or else the
/// nearest any end of one comes to the other.
fn segment_gap(a: rp::Vec2, b: rp::Vec2, c: rp::Vec2, d: rp::Vec2) -> f64 {
    let turn = |p: rp::Vec2, q: rp::Vec2, r: rp::Vec2| {
        (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x)
    };
    let crosses = turn(a, b, c) * turn(a, b, d) < 0.0 && turn(c, d, a) * turn(c, d, b) < 0.0;
    if crosses {
        return 0.0;
    }
    point_to_segment(a, c, d)
        .min(point_to_segment(b, c, d))
        .min(point_to_segment(c, a, b))
        .min(point_to_segment(d, a, b))
}

/// Prints, on `frame`, how close two fighters' bones have come where their
/// flesh meets: each pair of bones of different bodies nearer than the skin
/// both carry at rest, how far apart they lie against that, the closest
/// first, and how deep one fighter's skin pressed into the other's. The
/// flesh between two bones nearer than that is squeezed by as much.
fn report_between(name: &str, frame: i32, world: &rp::World) {
    if world.body_count() < 2 {
        return;
    }
    let bones = world.bones();
    let radii = flesh_radii(world);
    let mut pairs = Vec::new();
    for (i, a) in bones.iter().enumerate() {
        for (j, b) in bones.iter().enumerate().skip(i + 1) {
            let flesh = radii[i] + radii[j];
            if a.body == b.body || flesh <= 0.0 {
                continue;
            }
            let apart = segment_gap(a.a, a.b, b.a, b.b);
            if apart < flesh {
                pairs.push((apart / flesh, i, j, apart, flesh));
            }
        }
    }
    pairs.sort_by(|p, q| p.0.total_cmp(&q.0));
    let squeezed: Vec<String> = pairs
        .iter()
        .take(3)
        .map(|&(share, i, j, apart, flesh)| {
            format!(
                "{} (bone {i}) and {} (bone {j}) {apart:.1} px apart, {flesh:.1} px of skin between at rest ({:.0}%)",
                rp::inspect::bone_name(world, i),
                rp::inspect::bone_name(world, j),
                share * 100.0
            )
        })
        .collect();
    println!(
        "{name} frame {frame}: limbs pressing {} (hardest {:.0} N); skin pressed {:.1} px into the other fighter's; bones nearer than their flesh: {}",
        world.debug().limb_clashes,
        world.debug().max_clash_force,
        world.debug().max_overlap_between_bodies,
        if squeezed.is_empty() {
            "none".to_string()
        } else {
            squeezed.join("; ")
        }
    );
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
        // A joint a punch drives shows where its muscles aim it.
        let aim = if joint.drive.swing != 0.0 {
            format!(" aims {:+.1}deg", joint.drive.swing.to_degrees())
        } else {
            String::new()
        };
        joints.push(format!(
            "{side}{}-{} {turn:+.1}deg{aim} {:+.1}Nm{state}",
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
    for line in feet_and_hips(world) {
        println!("{name} frame {frame}: {line}");
    }
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
        rp::Traced::Point(index) => world
            .points()
            .get(index)
            .zip(world.point_holds().get(index))
            .map(|(point, hold)| {
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
                hold.wrap.x,
                hold.wrap.y,
                hold.posture.bones,
                hold.posture.weights,
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
            for (point_index, (point, hold)) in points.iter().zip(world.point_holds()).enumerate() {
                let binding = hold.posture;
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
