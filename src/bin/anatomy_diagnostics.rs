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
