//! How much of a blow reaches a bone through the flesh over it.
//!
//! A blunt blow meets a bone with the peak force of the mass-spring model
//! Robinovitch, Hayes, and McMahon (1991) built for falls on the hip: a tool
//! striking at speed v through a contact of stiffness k pushes on the part it
//! strikes with F = v sqrt(k m), where m is the tool's mass and the struck
//! part's in series, m_tool m_part / (m_tool + m_part). The force grows with
//! the speed of a blow rather than its energy, so a bone twice as strong
//! takes a blow four times as energetic to break, as the energy estimates in
//! `docs/INJURY_REFERENCE.md` assume, and a light bat meets a limb with
//! nearly all its mass while most of a heavy hammer's goes into shoving the
//! limb aside. The flesh over a bone is a spring in series with it: a
//! thickness h of it softens the contact to k0 / (1 + h / h0), and so the
//! force to F0 / sqrt(1 + h / h0). Robinovitch, McMahon, and Hayes (1995)
//! found that hip tissue cuts the peak force of a fall by about 71 N a
//! millimetre, 27% at 30 mm, which this follows with h0 about 15 mm.
//!
//! The figure is a flat front view, so the flesh drawn between a tool and a
//! bone is not always the flesh over the bone. Above the collarbone it draws
//! the trapezius, which in a real shoulder lies behind the collarbone, whose
//! top lies right under the skin. So a bone takes a blow from the moment the
//! tool presses the skin over it in the figure (`Cover::figure`, measured
//! from the figure's outline when the body is built), and how much the flesh
//! softens the blow is set by what lies over that face of a real bone
//! (`Cover::real`, from ultrasound and CT studies).

use super::body::FOREARM_STRENGTH;
use super::*;
use crate::silhouette::SilhouetteField;

/// Stations along each bone, from its start to its end, and directions
/// around it at which the flesh over it is measured.
const COVER_STATIONS: usize = 9;
const COVER_DIRECTIONS: usize = 16;
/// How far out from a bone the figure's flesh over it is followed, in point
/// spacings: farther, the figure shows no skin over the bone that way.
const MAX_FIGURE_COVER: f64 = 3.0;
/// The step, in pixels, at which the figure's flesh is followed outward.
const COVER_STEP: f64 = 0.5;
/// The thickness of flesh, in millimetres, that halves the stiffness of a
/// blow's contact on a bone (see the module notes). Fitting the spring model
/// to Robinovitch et al. (1995) puts it at 7-34 mm.
const COVER_SOFTENING_MM: f64 = 15.0;
/// The least share of a blow that reaches a bone under thick flesh: a hard
/// blow presses the flesh flat. Robinovitch et al.'s thickest hip tissue
/// still passed 63% of the thinnest's force at 140 J, about 0.35-0.5 of
/// what bare bone would take.
const MIN_COVER_SHARE: f64 = 0.4;
/// The stiffness of a blow's contact on bare bone, in newtons a metre, set so
/// blows break bones as often as the injury research says, as each tool's
/// break scale was before. The 4.5 kg weight that broke cadaver forearms
/// held at both ends at 3 m/s and 1.37 kN (Begeman and Pratima 1999) met
/// them through about 46 kN/m; a blow in the figure lands on a free limb,
/// often off the middle and spread along it, so it takes a stiffer contact
/// to break the same bones.
const BARE_BONE_STIFFNESS: f64 = 600_000.0;
/// The force that breaks a forearm across its middle, in newtons (Begeman
/// and Pratima 1999), which `FOREARM_STRENGTH` of `BONE_FRACTURE_IMPULSE`
/// stands for in the units bone strengths use.
const FOREARM_BREAKING_FORCE: f64 = 1370.0;
const LOAD_PER_NEWTON: f64 = BONE_FRACTURE_IMPULSE * FOREARM_STRENGTH / FOREARM_BREAKING_FORCE;

/// The flesh over one bone, by station along it and direction around it:
/// each row turns from the bone's direction in `COVER_DIRECTIONS` steps.
#[derive(Clone, Debug)]
pub(super) struct BoneCover {
    figure: [[f64; COVER_DIRECTIONS]; COVER_STATIONS],
    real: [[f64; COVER_DIRECTIONS]; COVER_STATIONS],
}

/// What covers a bone at one place on one side.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cover {
    /// The figure's flesh from the bone's surface out to its outline, or to
    /// another bone, in pixels; 0 where it shows no skin over the bone.
    pub figure: f64,
    /// What lies over the bone there on a real person, in millimetres.
    pub real: f64,
}

/// What lies over a bone on a real person, in millimetres, on each face a
/// front view can strike: away from the body's midline, toward it, from
/// above, and from below.
#[derive(Clone, Copy, Debug)]
struct Faces {
    outer: f64,
    inner: f64,
    top: f64,
    bottom: f64,
}

impl Faces {
    const fn all(thickness: f64) -> Self {
        Self {
            outer: thickness,
            inner: thickness,
            top: thickness,
            bottom: thickness,
        }
    }

    /// The flesh on the side toward `toward`, a unit direction, for a bone on
    /// the side of the body `outward` (1 right of the midline, -1 left).
    fn toward(self, toward: Vec2, outward: f64) -> f64 {
        let out = toward.x * outward;
        let up = -toward.y;
        let share = |part: f64| part.max(0.0) * part.max(0.0);
        self.outer * share(out)
            + self.inner * share(-out)
            + self.top * share(up)
            + self.bottom * share(-up)
    }

    fn average(self) -> f64 {
        (self.outer + self.inner + self.top + self.bottom) * 0.25
    }
}

/// What lies over each part of the skeleton on the 70 kg adult the figure
/// stands for (`docs/INJURY_REFERENCE.md`, How blows reach bone).
fn real_faces(part: BonePart) -> Faces {
    match part {
        // The collarbone lies right under the skin along its length. No
        // study has measured the skin, fat, and thin platysma over its top;
        // 8 mm is a judgement. Beneath it lies the chest's pectoral muscle.
        BonePart::Collarbone => Faces {
            outer: 10.0,
            inner: 10.0,
            top: 8.0,
            bottom: 20.0,
        },
        // Skin and fat over the deltoid are about 8 mm in men and 12 in women
        // (Poland et al. 1997), over the deltoid itself.
        BonePart::UpperArm => Faces {
            outer: 25.0,
            inner: 30.0,
            top: 25.0,
            bottom: 10.0,
        },
        // The ulna's border lies under the skin, as does the far end of the
        // radius.
        BonePart::Forearm => Faces {
            outer: 10.0,
            inner: 6.0,
            top: 8.0,
            bottom: 6.0,
        },
        BonePart::Hand => Faces::all(5.0),
        // The outer thigh's muscle and fat, and 35-38 mm over the hip's
        // bony point (Town et al. 2023).
        BonePart::Thigh => Faces {
            outer: 40.0,
            inner: 60.0,
            top: 35.0,
            bottom: 15.0,
        },
        // Outside, the shin's front muscle alone is about 26 mm thick in
        // young men and 22 in women (Martin-Rodriguez et al. 2023); its inner
        // face lies under skin and fat only, about 9 mm in men and 16 in
        // women of healthy weight (DuVall et al. 2025).
        BonePart::Shin => Faces {
            outer: 30.0,
            inner: 12.0,
            top: 15.0,
            bottom: 6.0,
        },
        BonePart::Foot => Faces::all(6.0),
        // The chest wall at the side is about 40 mm thick between the ribs
        // (Laan et al. 2016), so about 30 over a rib.
        BonePart::Rib => Faces::all(30.0),
        BonePart::Pelvis => Faces {
            outer: 35.0,
            inner: 35.0,
            top: 25.0,
            bottom: 35.0,
        },
        BonePart::Skull => Faces::all(6.0),
        BonePart::Spine | BonePart::Other => Faces::all(30.0),
    }
}

/// The mass, in kilograms, that a blow across the middle of each part meets
/// on the 70 kg adult. A limb segment turns about the joint above it, with
/// the limb beyond it held straight by its muscles, so a blow on its middle
/// meets the limb's moment of inertia about that joint over the square of
/// the blow's lever: from Winter's (2009) segment masses, lengths, and radii
/// of gyration, 4.5 kg for the forearm with the hand, 18 for the upper arm
/// with the forearm and hand, 8.7 for the shin with the foot, and 39 for the
/// thigh with the leg below it. The collarbone and ribs are braced on the
/// trunk.
fn struck_mass(part: BonePart) -> f64 {
    match part {
        BonePart::Hand => 0.5,
        BonePart::Forearm => 4.5,
        BonePart::UpperArm => 18.0,
        BonePart::Foot => 1.0,
        BonePart::Shin => 8.7,
        BonePart::Thigh => 39.0,
        BonePart::Skull => 5.7,
        BonePart::Pelvis => 20.0,
        BonePart::Collarbone | BonePart::Rib | BonePart::Spine | BonePart::Other => 10.0,
    }
}

/// The direction `turn` steps around from `direction`.
fn turned(direction: Vec2, turn: usize) -> Vec2 {
    let angle = turn as f64 * PI * 2.0 / COVER_DIRECTIONS as f64;
    let (sin, cos) = (libm::sin(angle), libm::cos(angle));
    Vec2 {
        x: direction.x * cos - direction.y * sin,
        y: direction.x * sin + direction.y * cos,
    }
}

/// Measures the flesh over every bone of a body built at `frame` on `field`:
/// the figure's, out to its outline, and a real person's.
pub(super) fn measure_bone_covers(world: &mut World, frame: BodyFrame, field: &SilhouetteField) {
    let reach = world.materials.point_spacing * MAX_FIGURE_COVER;
    let bones = world.bones.clone();
    for (index, bone) in bones.iter().enumerate() {
        let direction = normalized(subtract(bone.b, bone.a), Vec2 { x: 0.0, y: 1.0 });
        let near: Vec<&BoneSegment> = bones
            .iter()
            .enumerate()
            .filter(|&(other, near)| {
                other != index
                    && closest_segment_points(bone.a, bone.b, near.a, near.b).distance
                        <= bone.radius + reach + near.radius
            })
            .map(|(_, near)| near)
            .collect();
        let faces = real_faces(bone.part);
        let mut cover = BoneCover {
            figure: [[0.0; COVER_DIRECTIONS]; COVER_STATIONS],
            real: [[0.0; COVER_DIRECTIONS]; COVER_STATIONS],
        };
        for station in 0..COVER_STATIONS {
            let t = station as f64 / (COVER_STATIONS - 1) as f64;
            let center = lerp(bone.a, bone.b, t);
            let outward = if frame.body_coords(center).0 >= 0.0 {
                1.0
            } else {
                -1.0
            };
            for turn in 0..COVER_DIRECTIONS {
                let toward = turned(direction, turn);
                let surface = add(center, scale(toward, bone.radius));
                cover.figure[station][turn] =
                    figure_cover(frame, field, &near, surface, toward, reach);
                cover.real[station][turn] = faces.toward(toward, outward);
            }
        }
        world.bones[index].cover = world.bone_covers.len();
        world.bone_covers.push(cover);
    }
}

/// How far the figure's flesh runs from `surface` toward `toward` before its
/// outline or another bone, or 0 if it runs past `reach`.
fn figure_cover(
    frame: BodyFrame,
    field: &SilhouetteField,
    near: &[&BoneSegment],
    surface: Vec2,
    toward: Vec2,
    reach: f64,
) -> f64 {
    let mut depth = 0.0;
    while depth <= reach {
        let at = add(surface, scale(toward, depth));
        let (u, v) = frame.body_coords(at);
        if field.distance(u, v) >= 0.0
            || near
                .iter()
                .any(|other| distance_to_segment(at, other.a, other.b) <= other.radius)
        {
            return depth;
        }
        depth += COVER_STEP;
    }
    0.0
}

impl World {
    /// The most flesh the figure draws over any bone, in pixels.
    pub(super) fn max_figure_cover(&self) -> f64 {
        self.materials.point_spacing * MAX_FIGURE_COVER
    }

    /// What covers bone `index` at `t` along it, on the side toward
    /// `toward`, a unit direction from the bone. A piece broken off with no
    /// cover of its own has none in the figure.
    pub fn bone_cover(&self, index: usize, t: f64, toward: Vec2) -> Cover {
        let bone = &self.bones[index];
        let Some(cover) = self.bone_covers.get(bone.cover) else {
            return Cover {
                figure: 0.0,
                real: real_faces(bone.part).average(),
            };
        };
        let last = (COVER_STATIONS - 1) as f64;
        let along = (bone.cover_from + (bone.cover_to - bone.cover_from) * t.clamp(0.0, 1.0))
            .clamp(0.0, 1.0)
            * last;
        let station = (along.floor() as usize).min(COVER_STATIONS - 2);
        let station_share = (along - station as f64).clamp(0.0, 1.0);
        let direction = normalized(subtract(bone.b, bone.a), toward);
        let mut angle = libm::atan2(cross(direction, toward), dot(direction, toward));
        if angle < 0.0 {
            angle += PI * 2.0;
        }
        let around = angle * COVER_DIRECTIONS as f64 / (PI * 2.0);
        let turn = (around.floor() as usize) % COVER_DIRECTIONS;
        let turn_share = (around - around.floor()).clamp(0.0, 1.0);
        let next = (turn + 1) % COVER_DIRECTIONS;
        let blend = |table: &[[f64; COVER_DIRECTIONS]; COVER_STATIONS]| {
            let at =
                |row: usize| table[row][turn] * (1.0 - turn_share) + table[row][next] * turn_share;
            at(station) * (1.0 - station_share) + at(station + 1) * station_share
        };
        Cover {
            figure: blend(&cover.figure),
            real: blend(&cover.real),
        }
    }

    /// The blow a tool of `tool_mass` kilograms puts on a bone of `part`
    /// under `cover_mm` of flesh, driving into it at `speed` pixels a second
    /// (see the module notes).
    pub(super) fn blow_on_bone(
        &self,
        part: BonePart,
        tool_mass: f64,
        speed: f64,
        cover_mm: f64,
    ) -> Blow {
        let part_mass = struck_mass(part);
        let mass = tool_mass * part_mass / (tool_mass + part_mass).max(EPSILON);
        let softening = (1.0 / (1.0 + cover_mm.max(0.0) / COVER_SOFTENING_MM))
            .sqrt()
            .max(MIN_COVER_SHARE);
        let stiffness = BARE_BONE_STIFFNESS * softening * softening;
        let metres_per_second = speed.max(0.0) * self.mm_per_px() * 0.001;
        Blow {
            force: metres_per_second * (stiffness * mass).sqrt() * LOAD_PER_NEWTON,
            press: metres_per_second * (mass / stiffness).sqrt() * 1000.0
                / self.mm_per_px().max(EPSILON),
        }
    }
}

/// A blow on a bone at its peak.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Blow {
    /// Its force, in the units bone strengths use.
    pub(super) force: f64,
    /// How far, in pixels, the tool has pressed into the flesh and bone
    /// since it met them: the spring's give, v sqrt(m / k).
    pub(super) press: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> World {
        create_layered_body(1280.0, 720.0, Materials::default())
    }

    fn part_index(world: &World, part: BonePart, left: bool) -> usize {
        world
            .bones
            .iter()
            .position(|bone| bone.part == part && (bone.a.x < world.bones[0].a.x) == left)
            .expect("the body has the part")
    }

    #[test]
    fn the_figure_draws_flesh_above_the_collarbone_that_a_real_shoulder_lacks() {
        let world = body();
        let collarbone = world
            .bones
            .iter()
            .position(|bone| bone.part == BonePart::Collarbone)
            .unwrap();
        let up = Vec2 { x: 0.0, y: -1.0 };
        // Over the outer collarbone, where a bat brought down on the
        // shoulder lands.
        let cover = world.bone_cover(collarbone, 0.75, up);
        let mm = cover.figure * world.mm_per_px();
        assert!(
            mm > cover.real * 3.0,
            "the figure draws the trapezius there: {mm:.0} mm against {:.0}",
            cover.real
        );
        assert!(cover.real < 10.0, "{}", cover.real);
    }

    #[test]
    fn the_shin_has_muscle_outside_and_little_inside() {
        let world = body();
        let shin = part_index(&world, BonePart::Shin, true);
        let outside = world.bone_cover(shin, 0.5, Vec2 { x: -1.0, y: 0.0 });
        let inside = world.bone_cover(shin, 0.5, Vec2 { x: 1.0, y: 0.0 });
        // The shin slants a little, so straight out is not quite its side.
        assert!((outside.real - 30.0).abs() < 0.5, "{}", outside.real);
        assert!((inside.real - 12.0).abs() < 0.5, "{}", inside.real);
        assert!(outside.figure > 0.0 && inside.figure > 0.0);
    }

    #[test]
    fn a_blow_grows_with_speed_and_softens_under_flesh() {
        let world = body();
        let slow = world
            .blow_on_bone(BonePart::Forearm, 0.8, 1000.0, 10.0)
            .force;
        let fast = world
            .blow_on_bone(BonePart::Forearm, 0.8, 2000.0, 10.0)
            .force;
        assert!((fast / slow - 2.0).abs() < 1.0e-9, "{}", fast / slow);
        let covered = world
            .blow_on_bone(BonePart::Forearm, 0.8, 1000.0, 25.0)
            .force;
        let share = covered / slow;
        let expected: f64 =
            ((1.0 + 10.0 / COVER_SOFTENING_MM) / (1.0 + 25.0 / COVER_SOFTENING_MM)).sqrt();
        assert!(
            (share - expected).abs() < 1.0e-9,
            "{share} against {expected}"
        );
        let thick = world
            .blow_on_bone(BonePart::Forearm, 0.8, 1000.0, 500.0)
            .force;
        let bare = world
            .blow_on_bone(BonePart::Forearm, 0.8, 1000.0, 0.0)
            .force;
        assert!((thick / bare - MIN_COVER_SHARE).abs() < 1.0e-9);
    }

    #[test]
    fn a_heavy_tool_spends_more_of_its_blow_moving_a_light_limb() {
        let world = body();
        let speed = 1000.0;
        let hammer_on_hand = world.blow_on_bone(BonePart::Hand, 4.5, speed, 5.0).force;
        let hammer_on_thigh = world.blow_on_bone(BonePart::Thigh, 4.5, speed, 5.0).force;
        assert!(hammer_on_hand < hammer_on_thigh * 0.5);
        let bat_on_hand = world.blow_on_bone(BonePart::Hand, 0.8, speed, 5.0).force;
        let bat_on_thigh = world.blow_on_bone(BonePart::Thigh, 0.8, speed, 5.0).force;
        assert!(bat_on_hand > bat_on_thigh * 0.6);
    }

    #[test]
    fn pieces_of_a_broken_bone_keep_the_cover_of_where_they_lay() {
        let mut world = body();
        let shin = part_index(&world, BonePart::Shin, true);
        let outside = Vec2 { x: -1.0, y: 0.0 };
        let whole = world.bone_cover(shin, 0.75, outside);
        let strength = world.bones[shin].fracture_impulse;
        world.bones[shin].break_load = strength * 1.2;
        world.fracture_bone(shin, 0.5, outside, strength * 1.2);
        let lower = world
            .bones
            .iter()
            .rposition(|bone| bone.part == BonePart::Shin && bone.broken_start)
            .unwrap();
        let piece = world.bone_cover(lower, 0.5, outside);
        // The piece turns a little as it breaks away.
        assert!((piece.real - whole.real).abs() < 1.0, "{piece:?} {whole:?}");
        assert!(
            (piece.figure - whole.figure).abs() < 2.0,
            "{piece:?} {whole:?}"
        );
    }
}
