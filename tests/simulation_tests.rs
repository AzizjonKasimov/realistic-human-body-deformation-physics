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

#[test]
fn checked_in_front_facing_pixel_silhouette_reference_is_available() {
    let mask =
        include_str!("../docs/reference/pixel_human_silhouettes/front_adult_silhouette_41x96.mask");
    if !mask.contains("front-facing adult silhouette") {
        fail("pixel silhouette reference should explicitly identify the adult front-facing source");
    }
    if mask.contains("side-facing") || mask.contains("back-facing") {
        fail("body-generation silhouette reference must stay front-facing only");
    }

    let mut width = 0usize;
    let mut height = 0usize;
    let mut rows = Vec::new();
    for line in mask.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("# ") {
            continue;
        }
        if let Some(size) = line.strip_prefix("size ") {
            let mut parts = size.split_whitespace();
            width = parts
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            height = parts
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            continue;
        }
        if line.chars().any(|value| value != '#' && value != '.') {
            fail("pixel silhouette mask should contain only occupied and transparent pixels");
        }
        rows.push(line);
    }
    if width != 41 || height != 96 || rows.len() != height {
        fail("front-facing adult pixel silhouette mask should stay at the expected 41x96 size");
    }
    if rows.iter().any(|row| row.len() != width) {
        fail("front-facing pixel silhouette mask rows should match the declared width");
    }

    let attribution = include_str!("../docs/reference/README.md");
    if !attribution.contains("front-facing adult silhouette")
        || !attribution.contains("front_adult_silhouette_41x96.mask")
    {
        fail("pixel silhouette reference should keep front-facing source and usage notes");
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
    let head_width = skin_band_width(&world, 0.04, 0.20);
    let neck_width = central_skin_band_width(&world, 0.20, 0.27);
    let shoulder_width = skin_band_width(&world, 0.35, 0.48);
    let chest_width = central_skin_band_width(&world, 0.45, 0.57);
    let waist_width = central_skin_band_width(&world, 0.57, 0.69);
    let hip_width = central_skin_band_width(&world, 0.70, 0.78);
    let (left_leg_points, lower_leg_gap_points, right_leg_points) =
        skin_band_region_counts(&world, 0.82, 0.94, 0.018);
    if head_width <= 0.0
        || neck_width <= 0.0
        || shoulder_width <= 0.0
        || chest_width <= 0.0
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
    if chest_width < waist_width * 1.10 {
        panic!(
            "FAIL: front-facing adult ribcage should read wider than the waist: chest={chest_width:.2} waist={waist_width:.2}"
        );
    }
    if waist_width > shoulder_width * 0.78 {
        fail("torso should taper from shoulders toward the waist");
    }
    if hip_width < waist_width * 1.05 {
        panic!(
            "FAIL: pelvis should widen again below the waist: hip={hip_width:.2} waist={waist_width:.2}"
        );
    }
    if lower_leg_gap_points >= left_leg_points.min(right_leg_points) {
        fail("separated legs should keep a readable center gap below the pelvis");
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
