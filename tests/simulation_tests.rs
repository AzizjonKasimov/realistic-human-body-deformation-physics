use realistic_physics as rp;

fn fail(message: &str) {
    panic!("FAIL: {message}");
}

fn skin_band_width(world: &rp::World, min_t: f64, max_t: f64) -> f64 {
    skin_band_width_with_filter(world, min_t, max_t, f64::INFINITY)
}

fn central_skin_band_width(world: &rp::World, min_t: f64, max_t: f64) -> f64 {
    skin_band_width_with_filter(world, min_t, max_t, 0.14)
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

fn skin_band_region_counts(
    world: &rp::World,
    min_t: f64,
    max_t: f64,
    center_gap: f64,
) -> (usize, usize, usize) {
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
    let gap = height * center_gap;
    let mut left = 0;
    let mut center = 0;
    let mut right = 0;
    for point in skin_points {
        let t = (point.position.y - min_y) / height;
        if !(min_t..=max_t).contains(&t) {
            continue;
        }
        let dx = point.position.x - center_x;
        if dx < -gap {
            left += 1;
        } else if dx > gap {
            right += 1;
        } else {
            center += 1;
        }
    }
    (left, center, right)
}

/// Number of separate horizontal runs of skin (head, arms, torso, legs) where a
/// horizontal line at each height in the band crosses the skin triangles; the
/// largest count in the band is returned.
fn skin_band_clusters(world: &rp::World, min_t: f64, max_t: f64) -> usize {
    let skin_y = world
        .points()
        .iter()
        .filter(|point| point.layer == rp::TissueLayer::Skin)
        .map(|point| point.position.y);
    let min_y = skin_y.clone().fold(f64::INFINITY, f64::min);
    let max_y = skin_y.fold(f64::NEG_INFINITY, f64::max);
    let points = world.points();
    let mut most = 0;
    for step in 0..=8 {
        let t = min_t + (max_t - min_t) * step as f64 / 8.0;
        let y = min_y + (max_y - min_y) * t;
        let mut spans: Vec<(f64, f64)> = Vec::new();
        for triangle in world.triangles() {
            if triangle.layer != rp::TissueLayer::Skin {
                continue;
            }
            let corners = [
                points[triangle.a].position,
                points[triangle.b].position,
                points[triangle.c].position,
            ];
            let mut xs = Vec::new();
            for i in 0..3 {
                let (a, b) = (corners[i], corners[(i + 1) % 3]);
                if (a.y <= y) != (b.y <= y) {
                    xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
                }
            }
            if xs.len() == 2 {
                spans.push((xs[0].min(xs[1]), xs[0].max(xs[1])));
            }
        }
        spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut runs = 0;
        let mut reach = f64::NEG_INFINITY;
        for (start, end) in spans {
            if start > reach + 0.5 {
                runs += 1;
            }
            reach = reach.max(end);
        }
        most = most.max(runs);
    }
    most
}

#[test]
fn body_silhouette_comes_from_the_checked_in_front_view_reference() {
    let svg = include_str!("../docs/reference/human_body_silhouette.svg");
    if svg.matches("<path").count() != 1 || !svg.contains(" d=\"M ") {
        fail("front-view silhouette reference should keep its single outline path");
    }
    let notes = include_str!("../docs/reference/README.md");
    if !notes.contains("human_body_silhouette.svg") || !notes.contains("public domain") {
        fail("silhouette reference should keep its source and license notes");
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
    if world
        .bones()
        .iter()
        .filter(|bone| bone.kind == rp::BoneKind::Rib)
        .count()
        < 8
    {
        fail("body should contain a low-resolution rib cage proxy");
    }
    if world.vessels().len() < 7 {
        fail("body should contain low-resolution major vessel anatomy");
    }
    if world.cavities().is_empty()
        || world
            .cavities()
            .iter()
            .all(|cavity| cavity.rest_area <= 0.0 || cavity.area_indices.is_empty())
    {
        fail("body should contain a low-resolution torso cavity pressure region");
    }
    if world.organs().len() < 4 {
        fail("body should contain low-resolution anchored internal organ proxies");
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
    // Bands are fractions of the skin's height, so they line up with the body's
    // landmark heights: chin ~0.135, shoulders ~0.20, armpits ~0.32, waist ~0.36,
    // wrists ~0.52, crotch ~0.57, knees ~0.70.
    let head_width = skin_band_width(&world, 0.03, 0.11);
    let neck_width = central_skin_band_width(&world, 0.125, 0.150);
    let shoulder_width = skin_band_width(&world, 0.20, 0.24);
    let waist_width = skin_band_width_with_filter(&world, 0.35, 0.39, 0.088);
    let hip_width = skin_band_width_with_filter(&world, 0.48, 0.53, 0.12);
    let (left_leg_points, lower_leg_gap_points, right_leg_points) =
        skin_band_region_counts(&world, 0.74, 0.86, 0.018);
    if head_width <= 0.0
        || neck_width <= 0.0
        || shoulder_width <= 0.0
        || waist_width <= 0.0
        || hip_width <= 0.0
        || left_leg_points == 0
        || right_leg_points == 0
    {
        fail(
            "generated human body should have visible head, neck, shoulders, torso, hips, and legs",
        );
    }
    if neck_width > head_width * 0.85 {
        panic!(
            "FAIL: neck should read narrower than the head: neck={neck_width:.2} head={head_width:.2}"
        );
    }
    if shoulder_width < head_width * 1.30 || shoulder_width < neck_width * 1.75 {
        fail("front-facing adult shoulders should read clearly broader than the head and neck");
    }
    if waist_width > shoulder_width * 0.78 {
        fail("torso should taper from shoulders toward the waist");
    }
    if hip_width < waist_width * 1.12 {
        panic!(
            "FAIL: pelvis should widen again below the waist: hip={hip_width:.2} waist={waist_width:.2}"
        );
    }
    if lower_leg_gap_points >= left_leg_points.min(right_leg_points) {
        fail("separated legs should keep a readable center gap below the pelvis");
    }
    let elbow_runs = skin_band_clusters(&world, 0.40, 0.44);
    if elbow_runs != 3 {
        panic!(
            "FAIL: arms should hang separately beside the torso at elbow height: runs={elbow_runs}"
        );
    }
    let hand_runs = skin_band_clusters(&world, 0.565, 0.60);
    if hand_runs != 4 {
        panic!("FAIL: mitten hands should hang beside the separated thighs: runs={hand_runs}");
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
        || stats.bone_joint_subluxations != 0
        || stats.joint_ligament_damage_events != 0
        || stats.emitted_fluid_particles != 0
        || stats.fracture_marrow_sources != 0
        || stats.blood_loss != 0.0
        || stats.blood_stain_deposits != 0
        || stats.opened_wounds != 0
        || stats.wound_fluid_particles != 0
        || stats.contusion_events != 0
        || stats.tissue_fatigue_events != 0
        || stats.tissue_plastic_events != 0
        || stats.muscle_fiber_tears != 0
        || stats.tear_propagations != 0
        || stats.muscle_cut_transfers != 0
        || stats.muscle_crush_ruptures != 0
        || stats.cavity_ruptures != 0
        || stats.organ_damage_events != 0
        || stats.organ_penetrations != 0
        || stats.rib_organ_punctures != 0
        || stats.organ_ruptures != 0
        || stats.skin_flap_detachments != 0
        || stats.vessel_lacerations != 0
        || stats.fragment_vessel_lacerations != 0
        || stats.wound_reopens != 0
        || stats.fragment_tissue_hits != 0
        || stats.fragment_tissue_tears != 0
        || stats.fragment_skin_punctures != 0
        || stats.fractured_bones != 0
    {
        panic!(
            "FAIL: rest simulation should not tear tissue: skin={} muscle={} fiber_tears={} attachments={} bone_attachments={} bone_joints={} subluxations={} ligament_damage={} emitted_fluid={} marrow_sources={} blood_loss={:.3} stains={} wounds={} wound_fluid={} contusions={} fatigue={} plastic={} propagation={} deep_cut={} crush_ruptures={} cavity={} organ_damage={} organ_penetrations={} rib_organ_punctures={} organ_ruptures={} flaps={} vessel_lacerations={} fragment_vessel_lacerations={} reopens={} fragment_hits={} fragment_tears={} fragment_punctures={} fractures={}",
            stats.broken_skin,
            stats.broken_muscle,
            stats.muscle_fiber_tears,
            stats.broken_attachments,
            stats.broken_bone_attachments,
            stats.broken_bone_joints,
            stats.bone_joint_subluxations,
            stats.joint_ligament_damage_events,
            stats.emitted_fluid_particles,
            stats.fracture_marrow_sources,
            stats.blood_loss,
            stats.blood_stain_deposits,
            stats.opened_wounds,
            stats.wound_fluid_particles,
            stats.contusion_events,
            stats.tissue_fatigue_events,
            stats.tissue_plastic_events,
            stats.tear_propagations,
            stats.muscle_cut_transfers,
            stats.muscle_crush_ruptures,
            stats.cavity_ruptures,
            stats.organ_damage_events,
            stats.organ_penetrations,
            stats.rib_organ_punctures,
            stats.organ_ruptures,
            stats.skin_flap_detachments,
            stats.vessel_lacerations,
            stats.fragment_vessel_lacerations,
            stats.wound_reopens,
            stats.fragment_tissue_hits,
            stats.fragment_tissue_tears,
            stats.fragment_skin_punctures,
            stats.fractured_bones
        );
    }
    if !world.fluids().is_empty() || !world.blood_stains().is_empty() || !world.wounds().is_empty()
    {
        fail("rest simulation should not emit fluid particles, stains, or wounds");
    }
    if world.blood_volume_fraction() < 0.999 {
        fail("rest simulation should retain full finite blood volume");
    }
    let mut skin_displacement_sum = 0.0;
    let mut skin_displacement_count = 0usize;
    let mut max_skin_displacement = 0.0;
    for point in world
        .points()
        .iter()
        .filter(|point| point.layer == rp::TissueLayer::Skin)
    {
        let displacement = ((point.position.x - point.home.x).powi(2)
            + (point.position.y - point.home.y).powi(2))
        .sqrt();
        skin_displacement_sum += displacement;
        skin_displacement_count += 1;
        if displacement > max_skin_displacement {
            max_skin_displacement = displacement;
        }
    }
    let average_skin_displacement = skin_displacement_sum / skin_displacement_count.max(1) as f64;
    if average_skin_displacement > 18.0 || max_skin_displacement > 76.0 {
        panic!(
            "FAIL: idle body should retain passive tissue shape: avg={average_skin_displacement:.2} max={max_skin_displacement:.2}"
        );
    }
    let debug = world.debug();
    if debug.active || debug.impact != 0.0 || debug.bone_contacts != 0 || debug.tissue_contacts != 0
    {
        fail("inactive input should leave contact debug metrics idle");
    }
}

/// One skin spring between `a` and `b`, struck by `tool` driven right through
/// (150, 120). This frame the knife's tip sweeps from about x=168 to x=183.
fn strike_single_spring(tool: rp::ToolMode, a: rp::Vec2, b: rp::Vec2) -> rp::World {
    let mut world = rp::World::new(rp::Materials::default());
    world.add_point(a, rp::TissueLayer::Skin, false);
    world.add_point(b, rp::TissueLayer::Skin, false);
    world.add_spring(0, 1, rp::TissueLayer::Skin, 0.82, 10.0, 1000.0, false);
    let input = rp::InputState {
        active: true,
        down: true,
        x: 150.0,
        y: 120.0,
        vx: 900.0,
        vy: 0.0,
        power: 3.0,
        tool,
    };
    world.step(world.materials().fixed_dt, &input, 640.0, 480.0);
    world
}

#[test]
fn knife_cuts_fibers_it_crosses_and_opens_a_wound() {
    let across = strike_single_spring(
        rp::ToolMode::Sharp,
        rp::Vec2 { x: 176.0, y: 100.0 },
        rp::Vec2 { x: 176.0, y: 140.0 },
    );
    if across.debug().tool != rp::ToolMode::Sharp || across.stats().broken_skin != 1 {
        fail("a knife drawn across a skin fiber should sever it");
    }
    if across.stats().opened_wounds <= 0 || !across.wounds().iter().any(|wound| wound.active) {
        fail("a knife cut should open a persistent wound source");
    }
}

#[test]
fn knife_slides_along_fibers_without_cutting_them() {
    let along = strike_single_spring(
        rp::ToolMode::Sharp,
        rp::Vec2 { x: 130.0, y: 120.0 },
        rp::Vec2 { x: 170.0, y: 120.0 },
    );
    if along.stats().broken_skin != 0 {
        fail("a blade moving along a fiber parts tissue beside it but should not sever it");
    }
}

/// Presses the knife into the abdomen while it moves (`first_vx`, `first_vy`),
/// then draws it straight down; returns severed skin springs.
fn knife_stroke_down_abdomen(first_vx: f64, first_vy: f64) -> i32 {
    let (width, height) = (1280.0, 720.0);
    let frame = rp::body_frame(width, height);
    let mut world = rp::create_layered_body(width, height, rp::Materials::default());
    let dt = world.materials().fixed_dt;
    let start = frame.point(0.03, 0.30);
    let mut input = rp::InputState {
        active: true,
        down: true,
        x: start.x,
        y: start.y,
        vx: first_vx,
        vy: first_vy,
        power: 2.0,
        tool: rp::ToolMode::Sharp,
    };
    world.step(dt, &input, width, height);
    for step in 1..=24 {
        input.vx = 0.0;
        input.vy = 420.0;
        input.y = start.y + 420.0 * dt * f64::from(step);
        world.step(dt, &input, width, height);
    }
    world.stats().broken_skin
}

#[test]
fn knife_pressed_in_at_an_angle_still_cuts_a_line() {
    let aligned = knife_stroke_down_abdomen(0.0, 420.0);
    let turned = knife_stroke_down_abdomen(300.0, 0.0);
    if !(10..=60).contains(&aligned) {
        panic!("FAIL: a straight knife stroke should cut a line of skin: severed={aligned}");
    }
    // A blade that swept sideways while turning would scythe a wide fan.
    if turned > aligned + 40 {
        panic!(
            "FAIL: tissue should steer an embedded knife into its stroke instead of it sweeping a fan: turned={turned} aligned={aligned}"
        );
    }
}

#[test]
fn bat_bruises_a_fiber_the_knife_would_cut() {
    let batted = strike_single_spring(
        rp::ToolMode::Blunt,
        rp::Vec2 { x: 150.0, y: 100.0 },
        rp::Vec2 { x: 150.0, y: 140.0 },
    );
    if batted.stats().broken_skin != 0 {
        fail("a blunt bat should not slice through skin like a blade");
    }
    if batted.stats().contusion_events <= 0 {
        fail("a hard bat hit should leave a contusion");
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
