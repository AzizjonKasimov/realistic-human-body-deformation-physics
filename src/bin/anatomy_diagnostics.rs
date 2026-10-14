use realistic_physics as rp;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("output/anatomy_debug.svg"));
    let width = 1280.0;
    let height = 720.0;
    let world = rp::create_layered_body(width, height, rp::Materials::default());
    let validation = rp::validate_anatomy(&world, 16);

    let rib_count = world
        .bones()
        .iter()
        .filter(|bone| bone.kind == rp::BoneKind::Rib)
        .count();
    println!(
        "points={} springs={} triangles={} bones={} ribs={} bone_joints={} bone_attachments={} vessels={} cavities={} organs={}",
        world.points().len(),
        world.springs().len(),
        world.triangles().len(),
        world.bones().len(),
        rib_count,
        world.bone_joints().len(),
        world.bone_attachments().len(),
        world.vessels().len(),
        world.cavities().len(),
        world.organs().len()
    );
    println!(
        "skin_points={} muscle_points={} bone_samples={} bone_samples_outside_skin={} bone_samples_outside_muscle={}",
        validation.skin_points,
        validation.muscle_points,
        validation.bone_samples,
        validation.bone_samples_outside_skin,
        validation.bone_samples_outside_muscle
    );

    print_bone_covers(&world);
    print_outline_holds(&world, rp::body_frame(width, height));

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).expect("create output directory");
        }
    }
    fs::write(&output, build_svg(&world, width, height)).expect("write anatomy SVG");
    println!("wrote {}", output.display());

    if validation.bone_samples_outside_skin == 0 {
        std::process::exit(0);
    }
    std::process::exit(2);
}

/// How hard the hold on the skin's outline, which stands for the skin running
/// on round the back (`outline.rs`), pulls each outline point for its weight,
/// in pixels per second squared: where the outline curves tightly over light
/// points, a hold out of balance throws them.
fn print_outline_holds(world: &rp::World, frame: rp::BodyFrame) {
    let mut pulls: Vec<(f64, rp::Vec2)> = world
        .points()
        .iter()
        .filter(|point| point.wrap != rp::Vec2::default())
        .map(|point| {
            (
                point.wrap.x.hypot(point.wrap.y) / point.mass.max(1.0e-9),
                point.home,
            )
        })
        .collect();
    if pulls.is_empty() {
        return;
    }
    pulls.sort_by(|a, b| a.0.total_cmp(&b.0));
    let at = |share: f64| pulls[((pulls.len() - 1) as f64 * share) as usize].0;
    let (strongest, place) = pulls[pulls.len() - 1];
    let (u, v) = frame.body_coords(place);
    println!(
        "outline hold for its weight, px/s^2: median {:.0}, 90% {:.0}, most {:.0} at body ({u:.3}, {v:.3}), over {} points",
        at(0.5),
        at(0.9),
        strongest,
        pulls.len()
    );
}

/// The flesh over each bone on the figure's right (the screen's left) and
/// down the middle, in millimetres: what the figure draws out to its outline,
/// against what lies over a real bone, on each face a blow can strike.
fn print_bone_covers(world: &rp::World) {
    let mm = world.mm_per_px();
    let middle = world.bones()[0].a.x;
    println!("flesh over the bones, mm (drawn / real), at the middle of each bone:");
    println!(
        "  {:<12} {:>13} {:>13} {:>13} {:>13}",
        "bone", "outer", "inner", "top", "bottom"
    );
    for (index, bone) in world.bones().iter().enumerate() {
        let center = (bone.a.x + bone.b.x) * 0.5;
        let on_left = center < middle - 1.0;
        let on_middle = (center - middle).abs() <= 1.0;
        if !(on_left || on_middle) || bone.part == rp::BonePart::Spine {
            continue;
        }
        if bone.part == rp::BonePart::Rib && !bone_is_first_of_its_part(world, index) {
            continue;
        }
        let outward = if on_middle { 1.0 } else { -1.0 };
        let faces = [
            rp::Vec2 { x: outward, y: 0.0 },
            rp::Vec2 {
                x: -outward,
                y: 0.0,
            },
            rp::Vec2 { x: 0.0, y: -1.0 },
            rp::Vec2 { x: 0.0, y: 1.0 },
        ];
        // A bat brought down on the shoulder lands on the outer collarbone.
        let t = if bone.part == rp::BonePart::Collarbone {
            0.75
        } else {
            0.5
        };
        let cells: Vec<String> = faces
            .iter()
            .map(|&toward| {
                let cover = world.bone_cover(index, t, toward);
                format!("{:>5.0} / {:<5.0}", cover.figure * mm, cover.real)
            })
            .collect();
        println!("  {:<12} {}", bone.part.name(), cells.join(" "));
    }
}

fn bone_is_first_of_its_part(world: &rp::World, index: usize) -> bool {
    let part = world.bones()[index].part;
    world.bones().iter().position(|bone| bone.part == part) == Some(index)
}

fn build_svg(world: &rp::World, width: f64, height: f64) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.2}\" height=\"{height:.2}\" viewBox=\"0 0 {width:.2} {height:.2}\">\n"
    ));
    out.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#181818\"/>\n");

    for triangle in world.triangles() {
        if triangle.layer == rp::TissueLayer::Skin && world.triangle_alive(triangle) {
            write_triangle(&mut out, world, triangle, "#9b705e", 0.22);
        }
    }
    for triangle in world.triangles() {
        if triangle.layer == rp::TissueLayer::Muscle && world.triangle_alive(triangle) {
            write_triangle(&mut out, world, triangle, "#b62e3a", 0.50);
        }
    }
    for vessel in world.vessels() {
        out.push_str(&format!(
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#b51224\" stroke-opacity=\"0.72\" stroke-width=\"{:.2}\" stroke-linecap=\"round\"/>\n",
            vessel.a.x,
            vessel.a.y,
            vessel.b.x,
            vessel.b.y,
            vessel.radius * 1.35 + 1.0
        ));
    }
    for attachment in world.bone_attachments() {
        if attachment.broken
            || attachment.bone >= world.bones().len()
            || attachment.point >= world.points().len()
        {
            continue;
        }
        let anchor = bone_point(world.bones()[attachment.bone], attachment.t);
        let point = world.points()[attachment.point].position;
        out.push_str(&format!(
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#57a6ff\" stroke-opacity=\"0.16\" stroke-width=\"1\"/>\n",
            anchor.x, anchor.y, point.x, point.y
        ));
    }
    for joint in world.bone_joints() {
        if joint.broken || joint.a >= world.bones().len() || joint.b >= world.bones().len() {
            continue;
        }
        let a = bone_point(world.bones()[joint.a], joint.t_a);
        let b = bone_point(world.bones()[joint.b], joint.t_b);
        out.push_str(&format!(
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#ffd36a\" stroke-opacity=\"0.42\" stroke-width=\"2\"/>\n",
            a.x, a.y, b.x, b.y
        ));
        out.push_str(&format!(
            "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"3\" fill=\"#ffd36a\" fill-opacity=\"0.82\"/>\n",
            a.x, a.y
        ));
        out.push_str(&format!(
            "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"3\" fill=\"#ffd36a\" fill-opacity=\"0.82\"/>\n",
            b.x, b.y
        ));
    }
    for bone in world.bones() {
        let intact_stroke = if bone.kind == rp::BoneKind::Rib {
            "#f0dfb6"
        } else {
            "#e5d5aa"
        };
        out.push_str(&format!(
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" stroke-width=\"{:.2}\" stroke-linecap=\"round\"/>\n",
            bone.a.x,
            bone.a.y,
            bone.b.x,
            bone.b.y,
            if bone.fractured { "#fff3d6" } else { intact_stroke },
            bone.radius * 1.8
        ));
    }
    for bone in world.bones() {
        for i in 0..16 {
            let t = i as f64 / 15.0;
            let sample = bone_point(*bone, t);
            let inside_skin = rp::point_inside_layer(world, sample, rp::TissueLayer::Skin);
            out.push_str(&format!(
                "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"2.5\" fill=\"{}\"/>\n",
                sample.x,
                sample.y,
                if inside_skin { "#59d36d" } else { "#ff4f42" }
            ));
        }
    }
    out.push_str("</svg>\n");
    out
}

fn write_triangle(
    out: &mut String,
    world: &rp::World,
    triangle: &rp::Triangle,
    fill: &str,
    opacity: f64,
) {
    let points = world.points();
    let a = points[triangle.a].position;
    let b = points[triangle.b].position;
    let c = points[triangle.c].position;
    out.push_str(&format!(
        "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{}\" fill-opacity=\"{:.2}\" stroke=\"{}\" stroke-opacity=\"0.28\" stroke-width=\"1\"/>\n",
        a.x, a.y, b.x, b.y, c.x, c.y, fill, opacity, fill
    ));
}

fn bone_point(bone: rp::BoneSegment, t: f64) -> rp::Vec2 {
    rp::Vec2 {
        x: bone.a.x + (bone.b.x - bone.a.x) * t,
        y: bone.a.y + (bone.b.y - bone.a.y) * t,
    }
}
