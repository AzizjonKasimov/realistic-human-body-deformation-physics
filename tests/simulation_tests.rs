use realistic_physics as rp;

fn fail(message: &str) {
    panic!("FAIL: {message}");
}

fn skin_band_width(world: &rp::World, min_t: f64, max_t: f64) -> f64 {
    skin_band_width_with_filter(world, min_t, max_t, f64::INFINITY)
}

fn central_skin_band_width(world: &rp::World, min_t: f64, max_t: f64) -> f64 {
    skin_band_width_with_filter(world, min_t, max_t, 0.13)
}

fn skin_band_width_with_filter(
    world: &rp::World,
    min_t: f64,
    max_t: f64,
    max_center_distance: f64,
) -> f64 {
    let skin_points: Vec<_> = world
        .points()
        .iter()
        .filter(|point| point.layer == rp::TissueLayer::Skin)
        .collect();
    let min_y = skin_points
        .iter()
        .map(|point| point.position.y)
        .fold(f64::INFINITY, f64::min);
    let max_y = skin_points
        .iter()
        .map(|point| point.position.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_body_x = skin_points
        .iter()
        .map(|point| point.position.x)
        .fold(f64::INFINITY, f64::min);
    let max_body_x = skin_points
        .iter()
        .map(|point| point.position.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let height = (max_y - min_y).max(1.0);
    let center_x = (min_body_x + max_body_x) * 0.5;
    let center_limit = height * max_center_distance;
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    for point in skin_points {
        let t = (point.position.y - min_y) / height;
        if t >= min_t && t <= max_t && (point.position.x - center_x).abs() <= center_limit {
            min_x = min_x.min(point.position.x);
            max_x = max_x.max(point.position.x);
        }
    }
    if min_x.is_finite() && max_x.is_finite() {
        max_x - min_x
    } else {
        0.0
    }
}

#[test]
fn generated_body_has_expected_layers_and_anatomy() {
    let world = rp::create_layered_body(1280.0, 720.0, rp::Materials::default());
    if world.points().is_empty() {
        fail("body should contain points");
    }
    if world.springs().is_empty() {
        fail("body should contain springs");
    }
    if world.triangles().is_empty() {
        fail("body should contain triangles");
    }
    if world.bones().is_empty() {
        fail("body should contain bones");
    }
    if world.bone_attachments().is_empty() {
        fail("muscle should be attached to bones");
    }
    if world.bone_joints().is_empty() {
        fail("bones should be connected by joints");
    }

    let skin_points = world
        .points()
        .iter()
        .filter(|point| point.layer == rp::TissueLayer::Skin)
        .count();
    let muscle_points = world
        .points()
        .iter()
        .filter(|point| point.layer == rp::TissueLayer::Muscle)
        .count();
    if muscle_points >= skin_points {
        fail("muscle layer should be an inner subset of the skin layer");
    }
    if world.attachments().len() < skin_points * 2 {
        fail("skin should be densely tethered to the underlying muscle layer");
    }
    let mut skin_attachment_counts = vec![0usize; world.points().len()];
    for attachment in world.attachments() {
        skin_attachment_counts[attachment.skin_point] += 1;
    }
    if world.points().iter().enumerate().any(|(index, point)| {
        point.layer == rp::TissueLayer::Skin && skin_attachment_counts[index] == 0
    }) {
        fail("every generated skin point should have at least one muscle attachment");
    }
    let head_width = skin_band_width(&world, 0.02, 0.16);
    let shoulder_width = skin_band_width(&world, 0.22, 0.36);
    let waist_width = central_skin_band_width(&world, 0.45, 0.58);
    if head_width <= 0.0 || shoulder_width <= 0.0 || waist_width <= 0.0 {
        fail("generated human body should have visible head, shoulders, and torso");
    }
    if shoulder_width < head_width * 2.05 {
        fail("shoulders should read wider than the head");
    }
    if waist_width > shoulder_width * 0.78 {
        fail("torso should taper from shoulders toward the waist");
    }

    let anatomy = rp::validate_anatomy(&world, 16);
    if anatomy.bone_samples_outside_skin != 0 {
        fail("bone centerlines should stay inside the skin layer");
    }
}

#[test]
fn rest_simulation_stays_stable_and_idle() {
    let mut world = rp::create_layered_body(1280.0, 720.0, rp::Materials::default());
    for _ in 0..120 {
        world.step(
            world.materials().fixed_dt,
            &rp::InputState::default(),
            1280.0,
            720.0,
        );
    }

    if world
        .points()
        .iter()
        .any(|point| !point.position.x.is_finite() || !point.position.y.is_finite())
    {
        fail("rest simulation produced an invalid coordinate");
    }
    let stats = world.stats();
    if stats.broken_skin != 0
        || stats.broken_muscle != 0
        || stats.broken_attachments != 0
        || stats.broken_bone_attachments != 0
        || stats.broken_bone_joints != 0
        || stats.emitted_fluid_particles != 0
        || stats.opened_wounds != 0
        || stats.wound_fluid_particles != 0
        || stats.fragment_tissue_hits != 0
        || stats.fragment_tissue_tears != 0
        || stats.fractured_bones != 0
    {
        fail("rest simulation should not tear tissue");
    }
    if !world.fluids().is_empty() || !world.wounds().is_empty() {
        fail("rest simulation should not emit fluid particles");
    }
    let debug = world.debug();
    if debug.active || debug.impact != 0.0 || debug.bone_contacts != 0 || debug.tissue_contacts != 0
    {
        fail("inactive input should leave contact debug metrics idle");
    }
}

#[test]
fn sharp_tool_cuts_skin_and_opens_wound() {
    let mut world = rp::World::new(rp::Materials::default());
    world.add_point(
        rp::Vec2 { x: 130.0, y: 120.0 },
        rp::TissueLayer::Skin,
        false,
    );
    world.add_point(
        rp::Vec2 { x: 170.0, y: 120.0 },
        rp::TissueLayer::Skin,
        false,
    );
    world.add_spring(0, 1, rp::TissueLayer::Skin, 0.82, 10.0, 1000.0, false);
    let input = rp::InputState {
        active: true,
        down: true,
        x: 150.0,
        y: 120.0,
        vx: 900.0,
        vy: 0.0,
        power: 3.0,
        tool: rp::ToolMode::Sharp,
    };
    world.step(world.materials().fixed_dt, &input, 640.0, 480.0);
    if world.debug().tool != rp::ToolMode::Sharp || world.stats().broken_skin <= 0 {
        fail("sharp tool should concentrate pressure into skin tearing");
    }
    if world.stats().opened_wounds <= 0 || !world.wounds().iter().any(|wound| wound.active) {
        fail("sharp skin tears should open a persistent wound source");
    }
}

#[test]
fn direct_bone_strike_fractures_and_emits_fluid() {
    let mut world = rp::create_layered_body(1280.0, 720.0, rp::Materials::default());
    if world.bones().len() < 2 {
        fail("direct bone strike scenario needs a torso bone");
    }
    let target = world.bones()[1];
    let center = rp::Vec2 {
        x: (target.a.x + target.b.x) * 0.5,
        y: (target.a.y + target.b.y) * 0.5,
    };
    let initial_bones = world.bones().len();
    let input = rp::InputState {
        active: true,
        down: true,
        x: center.x,
        y: center.y,
        vx: 2200.0,
        vy: 120.0,
        power: 4.0,
        tool: rp::ToolMode::Blunt,
    };
    world.step(world.materials().fixed_dt, &input, 1280.0, 720.0);
    if world.stats().fractured_bones <= 0 || world.bones().len() <= initial_bones {
        fail("direct striker contact should fracture a bone");
    }
    if world.stats().emitted_fluid_particles <= 0 || world.fluids().is_empty() {
        fail("bone fracture should emit fluid particles from damaged tissue");
    }
    if world.debug().bone_contacts <= 0 || world.debug().fractures <= 0 {
        fail("direct strike should expose contact debug metrics");
    }
}
