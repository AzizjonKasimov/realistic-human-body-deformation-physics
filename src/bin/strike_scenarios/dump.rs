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

/// Where `--mesh U,V[,R][:FRAME]` draws the flesh's triangles: round (`u`,
/// `v`), in body heights from the crown (negative `u` on the viewer's left),
/// `radius` either way, on `frame` of the play.
#[derive(Clone, Copy)]
pub(crate) struct MeshShot {
    pub(crate) u: f64,
    pub(crate) v: f64,
    pub(crate) radius: f64,
    pub(crate) frame: i32,
}

impl MeshShot {
    /// Reads `U,V` or `U,V,R`, either with `:FRAME`.
    pub(crate) fn parse(spec: &str) -> Result<MeshShot, String> {
        let wrong = || format!("--mesh takes U,V[,R][:FRAME] in body heights, got `{spec}`");
        let (place, frame) = match spec.split_once(':') {
            Some((place, frame)) => (place, frame.parse().map_err(|_| wrong())?),
            None => (spec, 0),
        };
        let numbers = place
            .split(',')
            .map(|number| number.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| wrong())?;
        let (u, v, radius) = match numbers[..] {
            [u, v] => (u, v, 0.04),
            [u, v, radius] => (u, v, radius),
            _ => return Err(wrong()),
        };
        Ok(MeshShot {
            u,
            v,
            radius,
            frame,
        })
    }

    /// Draws the skin's triangles and the muscle's side by side round the
    /// place into `output/mesh_NAME_FRAME.svg`: slivers (quality under 0.3,
    /// 1 for an equilateral triangle) blue, triangles turned over since the
    /// body was built red. The mesh's shape where it tears, gapes, or shows
    /// muscle through the skin is what this shows; the elbow's crease was
    /// meshed with it.
    pub(crate) fn draw(&self, name: &str, frame: i32, world: &rp::World) {
        if frame != self.frame {
            return;
        }
        let body = rp::body_frame(SCENARIO_WIDTH, SCENARIO_HEIGHT);
        let panel = 500.0;
        let to_panel = |at: rp::Vec2, left: f64| {
            let (u, v) = body.body_coords(at);
            (
                left + (u - self.u + self.radius) / (2.0 * self.radius) * panel,
                (v - self.v + self.radius) / (2.0 * self.radius) * panel,
            )
        };
        let signed = |a: rp::Vec2, b: rp::Vec2, c: rp::Vec2| {
            ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)) * 0.5
        };
        let points = world.points();
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" font-family=\"sans-serif\" font-size=\"14\">\n<rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n",
            2.0 * panel + 20.0,
            panel + 24.0
        );
        let mut counts = Vec::new();
        for (index, layer) in [rp::TissueLayer::Skin, rp::TissueLayer::Muscle]
            .into_iter()
            .enumerate()
        {
            let left = index as f64 * (panel + 20.0);
            let (mut shown, mut slivers, mut turned) = (0, 0, 0);
            for triangle in world.triangles() {
                if triangle.layer != layer {
                    continue;
                }
                let corners = [triangle.a, triangle.b, triangle.c].map(|i| points[i].position);
                let near = corners.iter().any(|&at| {
                    let (u, v) = body.body_coords(at);
                    (u - self.u).abs() < self.radius * 1.2 && (v - self.v).abs() < self.radius * 1.2
                });
                if !near {
                    continue;
                }
                let homes = [triangle.a, triangle.b, triangle.c].map(|i| points[i].home);
                let area = signed(corners[0], corners[1], corners[2]);
                let sides: f64 = (0..3)
                    .map(|k| {
                        let (p, q) = (corners[k], corners[(k + 1) % 3]);
                        (q.x - p.x).powi(2) + (q.y - p.y).powi(2)
                    })
                    .sum();
                let quality = 4.0 * 3.0_f64.sqrt() * area.abs() / sides.max(1.0e-9);
                let fill = if area * signed(homes[0], homes[1], homes[2]) < 0.0 {
                    turned += 1;
                    "#ff4040"
                } else if quality < 0.3 {
                    slivers += 1;
                    "#5aa0ff"
                } else if layer == rp::TissueLayer::Skin {
                    "#f0c8aa"
                } else {
                    "#c87878"
                };
                shown += 1;
                let corners: Vec<String> = corners
                    .iter()
                    .map(|&at| {
                        let (x, y) = to_panel(at, left);
                        format!("{x:.1},{y:.1}")
                    })
                    .collect();
                svg.push_str(&format!(
                    "<polygon points=\"{}\" fill=\"{fill}\" stroke=\"#444\" stroke-width=\"0.6\"/>\n",
                    corners.join(" ")
                ));
            }
            let count =
                format!("{layer:?}: {shown} triangles, {slivers} slivers, {turned} turned over");
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\">{count}</text>\n",
                left + 6.0,
                panel + 18.0
            ));
            counts.push(count);
        }
        svg.push_str("</svg>\n");
        let path = PathBuf::from("output").join(format!("mesh_{name}_{frame}.svg"));
        match fs::create_dir_all("output").and_then(|_| fs::write(&path, svg)) {
            Ok(()) => println!(
                "{name} frame {frame}: mesh round ({:+.3}, {:.3}): {}; wrote {}",
                self.u,
                self.v,
                counts.join("; "),
                path.display()
            ),
            Err(error) => eprintln!("could not write {}: {error}", path.display()),
        }
    }
}
