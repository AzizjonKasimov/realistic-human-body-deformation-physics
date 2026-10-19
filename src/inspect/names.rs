//! Names for what the inspection tools talk about, in words a person reads:
//! where on the body, which bone or joint, and what did it. Left and right
//! are as seen on screen.

use crate::simulation::{
    Actor, BonePart, BoneSegment, Limb, Measure, OrganKind, Subject, TissueLayer, ToolMode, Vec2,
    World, WoundKind,
};

/// Where body `body`'s middle lies across the screen: its spine as built.
fn midline(world: &World, body: u8) -> Option<f64> {
    world
        .bones()
        .iter()
        .find(|bone| bone.part == BonePart::Spine && bone.body == body)
        .map(|spine| (spine.home_a.x + spine.home_b.x) * 0.5)
}

/// "left " or "right " for something of body `body` at `x` across the
/// screen, from that body's own middle, or nothing on its middle.
fn side(world: &World, body: u8, x: f64) -> &'static str {
    match midline(world, body) {
        Some(middle) if x < middle - 2.0 => "left ",
        Some(middle) if x > middle + 2.0 => "right ",
        _ => "",
    }
}

/// Whose a thing of body `body` is, where two fighters stand: "left
/// fighter's " or "right fighter's ", by where each stands; nothing for a
/// body alone. Judged from the first body's middle alone, every part of the
/// fighter on the right read as on the right.
fn whose(world: &World, body: u8) -> &'static str {
    if world.body_count() < 2 {
        return "";
    }
    let Some(middle) = midline(world, body) else {
        return "";
    };
    let others = (0..world.body_count() as u8)
        .filter(|&other| other != body)
        .filter_map(|other| midline(world, other));
    if others.map(|x| x - middle).sum::<f64>() > 0.0 {
        "left fighter's "
    } else {
        "right fighter's "
    }
}

/// Whose a bone is and the side it lies on, by where it was built: a limb
/// swung across the body is still on its own side.
fn bone_side(world: &World, bone: &BoneSegment) -> String {
    format!(
        "{}{}",
        whose(world, bone.body),
        side(world, bone.body, (bone.home_a.x + bone.home_b.x) * 0.5)
    )
}

fn distance_to_segment(at: Vec2, a: Vec2, b: Vec2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length_sq = dx * dx + dy * dy;
    let t = if length_sq > 1.0e-12 {
        (((at.x - a.x) * dx + (at.y - a.y) * dy) / length_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    libm::hypot(at.x - (a.x + dx * t), at.y - (a.y + dy * t))
}

/// The bone nearest `at`, whole or broken, by index.
pub fn nearest_bone(world: &World, at: Vec2) -> Option<usize> {
    world
        .bones()
        .iter()
        .enumerate()
        .map(|(index, bone)| (distance_to_segment(at, bone.a, bone.b), index))
        .min_by(|p, q| p.0.total_cmp(&q.0))
        .map(|(_, index)| index)
}

/// The part of the body a bone of `part` runs through, as a place.
fn region(part: BonePart) -> &'static str {
    match part {
        BonePart::Skull => "head",
        BonePart::Collarbone => "collarbone",
        BonePart::Rib => "ribs",
        BonePart::Pelvis => "hips",
        BonePart::UpperArm => "upper arm",
        BonePart::Forearm => "forearm",
        BonePart::Hand => "hand",
        BonePart::Thigh => "thigh",
        BonePart::Shin => "shin",
        BonePart::Foot => "foot",
        BonePart::Spine | BonePart::Other => "trunk",
    }
}

/// Where `at` is on the body, by the bone it lies over: "the left forearm",
/// "the chest".
pub fn place_name(world: &World, at: Vec2) -> String {
    let Some(index) = nearest_bone(world, at) else {
        return "the body".to_string();
    };
    let bone = &world.bones()[index];
    if bone.part == BonePart::Spine {
        // Down the trunk: the chest above the lowest ribs, the belly below.
        let t = {
            let (dx, dy) = (bone.b.x - bone.a.x, bone.b.y - bone.a.y);
            let length_sq = (dx * dx + dy * dy).max(1.0e-12);
            ((at.x - bone.a.x) * dx + (at.y - bone.a.y) * dy) / length_sq
        };
        let place = if t < 0.55 { "chest" } else { "belly" };
        return format!("the {}{place}", whose(world, bone.body));
    }
    format!("the {}{}", bone_side(world, bone), region(bone.part))
}

/// A bone by name: "the left forearm", or "a piece of the left forearm"
/// once broken, "a splinter of" one chipped off.
pub fn bone_name(world: &World, index: usize) -> String {
    let Some(bone) = world.bones().get(index) else {
        return format!("bone {index}");
    };
    let whole = format!("{}{}", bone_side(world, bone), bone.part.name());
    if bone.splinter {
        format!("a splinter of the {whole} (bone {index})")
    } else if bone.fractured {
        format!("a piece of the {whole} (bone {index})")
    } else {
        format!("the {whole}")
    }
}

/// A joint by name: "the left elbow", or "the break in the left forearm"
/// for the hinge between a broken bone's pieces.
pub fn joint_name(world: &World, index: usize) -> String {
    let Some(joint) = world.bone_joints().get(index) else {
        return format!("joint {index}");
    };
    let bones = world.bones();
    let (Some(a), Some(b)) = (bones.get(joint.a), bones.get(joint.b)) else {
        return format!("joint {index}");
    };
    // The side of the bone beyond the joint, or for one on the body's
    // middle, as the neck's skull is, the side of the bone before it.
    let side = if side(world, b.body, (b.home_a.x + b.home_b.x) * 0.5).is_empty() {
        bone_side(world, a)
    } else {
        bone_side(world, b)
    };
    if joint.fracture_link {
        return format!("the break in the {side}{}", a.part.name());
    }
    use BonePart::*;
    let name = match (a.part, b.part) {
        (Spine, Skull) | (Skull, Spine) => "neck",
        (Spine, Collarbone) | (Collarbone, Spine) => "collarbone's joint at the breastbone",
        (Collarbone, UpperArm) | (UpperArm, Collarbone) => "shoulder",
        (UpperArm, Forearm) | (Forearm, UpperArm) => "elbow",
        (Forearm, Hand) | (Hand, Forearm) => "wrist",
        (Spine, Pelvis) | (Pelvis, Spine) => "lower back",
        (Pelvis, Thigh) | (Thigh, Pelvis) => "hip",
        (Thigh, Shin) | (Shin, Thigh) => "knee",
        (Shin, Foot) | (Foot, Shin) => "ankle",
        (_, Rib) | (Rib, _) => "rib's joint",
        _ => {
            return format!(
                "the joint between the {side}{} and {}",
                a.part.name(),
                b.part.name()
            )
        }
    };
    format!("the {side}{name}")
}

fn layer_name(layer: TissueLayer) -> &'static str {
    match layer {
        TissueLayer::Skin => "skin",
        TissueLayer::Muscle => "muscle",
    }
}

/// What an event happened to, by name.
pub fn subject_name(world: &World, subject: Subject) -> String {
    let point_place = |index: usize| {
        world
            .points()
            .get(index)
            .map(|point| place_name(world, point.position))
    };
    match subject {
        Subject::Nothing => String::new(),
        Subject::Point(index) => match world.points().get(index) {
            Some(point) => format!(
                "{} at {}",
                layer_name(point.layer),
                place_name(world, point.position)
            ),
            None => format!("point {index}"),
        },
        Subject::Spring(index) => match world.springs().get(index) {
            Some(spring) => {
                let points = world.points();
                let (a, b) = (points[spring.a].position, points[spring.b].position);
                let middle = Vec2 {
                    x: (a.x + b.x) * 0.5,
                    y: (a.y + b.y) * 0.5,
                };
                format!(
                    "a {} fiber at {}",
                    layer_name(spring.layer),
                    place_name(world, middle)
                )
            }
            None => format!("fiber {index}"),
        },
        Subject::SkinTie(index) => match world.attachments().get(index) {
            Some(tie) => format!(
                "the fat under the skin at {}",
                point_place(tie.skin_point).unwrap_or_default()
            ),
            None => format!("skin tie {index}"),
        },
        Subject::BoneTie(index) => match world.bone_attachments().get(index) {
            Some(tie) => format!("muscle on {}", bone_name(world, tie.bone)),
            None => format!("bone tie {index}"),
        },
        Subject::Triangle(index) => match world.triangles().get(index) {
            Some(triangle) => format!(
                "{} at {}",
                layer_name(triangle.layer),
                point_place(triangle.a).unwrap_or_default()
            ),
            None => format!("triangle {index}"),
        },
        Subject::Bone(index) => bone_name(world, index),
        Subject::Joint(index) => joint_name(world, index),
        Subject::Vessel(index) => match world.vessels().get(index) {
            Some(vessel) => format!(
                "the artery at {}",
                place_name(
                    world,
                    Vec2 {
                        x: (vessel.a.x + vessel.b.x) * 0.5,
                        y: (vessel.a.y + vessel.b.y) * 0.5,
                    }
                )
            ),
            None => format!("artery {index}"),
        },
        Subject::Organ(index) => match world.organs().get(index) {
            Some(organ) => {
                let body = world
                    .points()
                    .get(organ.anchor_point)
                    .map_or(0, |point| point.body);
                let whose = whose(world, body);
                match organ.kind {
                    OrganKind::LeftLung | OrganKind::RightLung => {
                        format!("the {whose}{}lung", side(world, body, organ.center.x))
                    }
                    OrganKind::Liver => format!("the {whose}liver"),
                    OrganKind::Spleen => format!("the {whose}spleen"),
                }
            }
            None => format!("organ {index}"),
        },
        Subject::Cavity(_) => "the chest and belly".to_string(),
        Subject::Wound(index) => match world.wounds().get(index) {
            Some(wound) => format!(
                "{} at {}",
                match wound.kind {
                    WoundKind::Tissue => "a wound",
                    WoundKind::Marrow => "a broken bone's marrow",
                    WoundKind::Artery => "a cut artery",
                },
                place_name(world, wound.position)
            ),
            None => format!("wound {index}"),
        },
        Subject::Limb(limb) => match limb {
            Limb::LeftArm => "the left arm",
            Limb::RightArm => "the right arm",
            Limb::LeftLeg => "the left leg",
            Limb::RightLeg => "the right leg",
            Limb::None => "the trunk",
        }
        .to_string(),
    }
}

/// A subject as `strike_scenarios --why` takes it, such as `bone:14`, or as
/// it is called where `--why` cannot follow it.
pub fn subject_spec(subject: Subject) -> String {
    match subject {
        Subject::Point(index) => format!("point:{index}"),
        Subject::Bone(index) => format!("bone:{index}"),
        Subject::Joint(index) => format!("joint:{index}"),
        Subject::Spring(index) => format!("spring:{index}"),
        Subject::Triangle(index) => format!("triangle:{index}"),
        other => format!("{other:?}"),
    }
}

/// The tool by name.
pub fn tool_name(tool: ToolMode) -> &'static str {
    match tool {
        ToolMode::Blunt => "the bat",
        ToolMode::Heavy => "the sledgehammer",
        ToolMode::Sharp => "the knife",
        ToolMode::Fist => "the fist",
    }
}

/// What did something, by name; `tool` is the tool in hand. Nothing for the
/// tissue's own load and stretch, which the rule's measure tells.
pub fn actor_name(world: &World, actor: Actor, tool: ToolMode) -> String {
    match actor {
        Actor::Tissue => String::new(),
        Actor::Tool => tool_name(tool).to_string(),
        Actor::Bone(index) => bone_name(world, index),
        Actor::Flesh => "the muscle pressed onto it".to_string(),
        Actor::Pressure => "pressure in the chest and belly".to_string(),
    }
}

/// What a rule judged by, against its limit, in words.
pub fn measure_text(measure: Measure, value: f64, limit: f64) -> String {
    match measure {
        Measure::None => String::new(),
        Measure::Stretch => format!(
            "stretched {:+.0}% (limit {:+.0}%)",
            (value - 1.0) * 100.0,
            (limit - 1.0) * 100.0
        ),
        Measure::Load => format!("load {value:.0} (limit {limit:.0})"),
        Measure::Strength => format!("{value:.2} times its strength"),
        Measure::Shear => format!("slid {value:.1} fat thicknesses (limit {limit:.1})"),
        Measure::Slip => format!("pulled {value:.0} mm off its place (limit {limit:.0} mm)"),
        Measure::Overbend => format!(
            "bent {:.0} degrees past its range (limit {:.0})",
            value.to_degrees(),
            limit.to_degrees()
        ),
        Measure::Pressure if limit > 0.0 => format!("pressure {value:.2} (limit {limit:.2})"),
        Measure::Pressure => format!("pressure {value:.2}"),
        Measure::Damage => format!("damage {value:.2} (ruptures at {limit:.2})"),
        Measure::Count => format!("{value:.0}"),
        Measure::Speed => format!("met at {value:.1} m/s (limit {limit:.1} m/s)"),
        Measure::Force => format!("pressed {value:.0} N (limit {limit:.0} N)"),
    }
}
