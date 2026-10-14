//! The body laid out in numbers on one frame of a play (`--dump`): each bone
//! where it lies and how far from where it was built, each joint how far it
//! is turned and has slipped, each artery, and how far each bone's flesh has
//! slid off it. The questions it answers are the ones a picture leaves open:
//! is the body where it should be, and by how much is it not.

use super::*;

/// What `--dump WHAT[:FRAME]` prints, and on which frame of the play.
#[derive(Clone, Copy, Default)]
pub(crate) struct Dump {
    pub(crate) bones: bool,
    pub(crate) joints: bool,
    pub(crate) vessels: bool,
    pub(crate) ties: bool,
    /// The flesh held to this bone, point by point.
    pub(crate) flesh: Option<usize>,
    pub(crate) frame: i32,
}

impl Dump {
    /// Reads `bones,joints,vessels,ties` or `all`, then `:FRAME`.
    pub(crate) fn parse(spec: &str) -> Result<Dump, String> {
        let (what, frame) = match spec.split_once(':') {
            Some((what, frame)) => (
                what,
                frame
                    .parse()
                    .map_err(|_| format!("--dump takes WHAT[:FRAME], got `{spec}`"))?,
            ),
            None => (spec, 0),
        };
        let mut dump = Dump {
            frame,
            ..Dump::default()
        };
        for item in what.split(',') {
            match item.trim() {
                "bones" => dump.bones = true,
                "joints" => dump.joints = true,
                "vessels" => dump.vessels = true,
                "ties" => dump.ties = true,
                flesh if flesh.starts_with("flesh=") => {
                    let bone = &flesh["flesh=".len()..];
                    dump.flesh = Some(bone.parse().map_err(|_| {
                        format!("--dump flesh=N takes a bone's number, got `{flesh}`")
                    })?);
                }
                "all" => {
                    dump.bones = true;
                    dump.joints = true;
                    dump.vessels = true;
                    dump.ties = true;
                }
                other => {
                    return Err(format!(
                        "--dump takes bones, joints, vessels, ties, flesh=N, or all, got `{other}`"
                    ))
                }
            }
        }
        Ok(dump)
    }

    /// Prints what was asked for about `world` on `frame` of `name`.
    pub(crate) fn report(&self, name: &str, frame: i32, world: &rp::World) {
        if frame != self.frame {
            return;
        }
        let body = rp::body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT);
        let at = |point: rp::Vec2| {
            let (u, v) = body.body_coords(point);
            format!("({u:+.4}, {v:.4})")
        };
        let off = |now: rp::Vec2, built: rp::Vec2| {
            format!("({:+.1}, {:+.1})", now.x - built.x, now.y - built.y)
        };
        println!("{name} frame {frame}, in body heights from the crown, offsets in px:");
        if self.bones {
            for (index, bone) in world.bones().iter().enumerate() {
                let length = (bone.b.x - bone.a.x).hypot(bone.b.y - bone.a.y);
                println!(
                    "  bone {index:>2} {:<28} {} -> {}  off {} {}  length {length:.1}/{:.1}{}{}",
                    rp::inspect::bone_name(world, index),
                    at(bone.a),
                    at(bone.b),
                    off(bone.a, bone.home_a),
                    off(bone.b, bone.home_b),
                    bone.rest_length,
                    if bone.fractured { ", broken" } else { "" },
                    if bone.splinter { ", a splinter" } else { "" },
                );
            }
        }
        if self.joints {
            for (index, joint) in world.bone_joints().iter().enumerate() {
                let state = if joint.broken {
                    "torn".to_string()
                } else if joint.post_fracture_limited {
                    "loose".to_string()
                } else if joint.subluxated {
                    "out of place".to_string()
                } else {
                    world
                        .joint_slip(index)
                        .map_or("whole".to_string(), |slip| format!("slipped {slip:.1} px"))
                };
                println!(
                    "  joint {index:>2} {:<40} turned {:+.1} deg, {:+.1} Nm, rest gap {:.1} px, {state}",
                    rp::inspect::joint_name(world, index),
                    world.joint_turn(index).unwrap_or(0.0).to_degrees(),
                    world.joint_torque(index),
                    joint.rest,
                );
            }
        }
        if self.vessels {
            for (index, vessel) in world.vessels().iter().enumerate() {
                println!(
                    "  vessel {index} {} -> {} radius {:.1} px{}",
                    at(vessel.a),
                    at(vessel.b),
                    vessel.radius,
                    if vessel.lacerated { ", cut" } else { "" },
                );
            }
        }
        if let Some(bone) = self.flesh {
            report_flesh(world, bone, &at);
        }
        if self.ties {
            // Each bone's muscle ties: how many, how many torn, and how far
            // the flesh of the rest has slid off its place on the bone.
            let mut by_bone: Vec<(usize, usize, usize, f64, f64)> = Vec::new();
            for tie in world.bone_attachments() {
                if tie.bone >= world.bones().len() || tie.point >= world.points().len() {
                    continue;
                }
                let index = match by_bone.iter().position(|entry| entry.0 == tie.bone) {
                    Some(index) => index,
                    None => {
                        by_bone.push((tie.bone, 0, 0, 0.0, 0.0));
                        by_bone.len() - 1
                    }
                };
                let entry = &mut by_bone[index];
                entry.1 += 1;
                if tie.broken {
                    entry.2 += 1;
                } else {
                    let slip = world.bone_tie_slip(tie);
                    entry.3 += slip;
                    entry.4 = entry.4.max(slip);
                }
            }
            by_bone.sort_by_key(|entry| entry.0);
            for (bone, count, torn, total, most) in by_bone {
                let held = (count - torn).max(1);
                println!(
                    "  ties on bone {bone:>2} {:<28} {count} ties, {torn} torn, flesh slid {:.1} px on average, {most:.1} at most",
                    rp::inspect::bone_name(world, bone),
                    total / held as f64,
                );
            }
        }
    }
}

/// Each point of flesh held to `bone` (its place on the skeleton mostly on
/// it, or a muscle tie to it): where it is, how far from its place, what
/// ties it to bone, and the points the fat ties it to across the layers.
fn report_flesh(world: &rp::World, bone: usize, at: &dyn Fn(rp::Vec2) -> String) {
    println!(
        "  flesh held to bone {bone} ({}):",
        rp::inspect::bone_name(world, bone)
    );
    for (index, point) in world.points().iter().enumerate() {
        let binding = point.posture;
        let bound = (0..2).any(|slot| binding.bones[slot] == bone && binding.weights[slot] >= 0.5);
        let ties: Vec<String> = world
            .bone_attachments()
            .iter()
            .filter(|tie| tie.point == index)
            .map(|tie| format!("{}{}", tie.bone, if tie.broken { " torn" } else { "" }))
            .collect();
        let tied = world
            .bone_attachments()
            .iter()
            .any(|tie| tie.point == index && tie.bone == bone);
        if !bound && !tied {
            continue;
        }
        let fat: Vec<String> = world
            .attachments()
            .iter()
            .filter(|tie| tie.skin_point == index || tie.muscle_point == index)
            .map(|tie| {
                let other = if tie.skin_point == index {
                    tie.muscle_point
                } else {
                    tie.skin_point
                };
                format!("{other}{}", if tie.broken { " torn" } else { "" })
            })
            .collect();
        let off = world
            .posture_target(index)
            .map_or("-".to_string(), |place| {
                format!(
                    "{:.1}",
                    (point.position.x - place.x).hypot(point.position.y - place.y)
                )
            });
        println!(
            "    point {index} {:?}{} at {} {off} px off its place; tied to bone {}; fat to {}",
            point.layer,
            if point.is_ghost() { ", a ghost" } else { "" },
            at(point.position),
            if ties.is_empty() {
                "-".to_string()
            } else {
                ties.join(", ")
            },
            if fat.is_empty() {
                "-".to_string()
            } else {
                fat.join(", ")
            },
        );
    }
}
