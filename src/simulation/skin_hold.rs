//! A bone stays inside its own skin. Flesh hangs on the bones by soft ties
//! (`soft.rs`) that tear once the flesh slips far enough off its place, and
//! nothing else kept a bone from passing out through its own skin. Whatever
//! moved the skin without its bone, or the bone without its skin, pulled
//! them apart: a punch's fist landing, whose skin the other fighter's skin
//! stops (`outline.rs`) while the hand's bone coasts on, came out bare
//! through its knuckles for a few frames in most random fights, tearing
//! the ties as it went; a fighter down on a knee, the floor pushing the
//! foot's flesh and its bone up by different amounts each step, had the
//! foot's flesh torn off and thrown into the air; a joint by a broken bone
//! drives the light bone without its flesh (`sides.rs`). Only a closed
//! break's pieces were held in, near the skin.
//!
//! So each place along a bone the normal view draws meets the skin round it
//! from inside, as one body meets another: a soft contact, as Box2D v3
//! solves a contact point (Erin Catto, MIT, `contact_solver.c`: a soft
//! contact at 30 Hz, damped tenfold, pushing out no faster than 3 m/s, and
//! speculative before they touch), between the bone and the stretch of skin
//! nearest it, the skin pressing with the muscle the fat ties under it, as
//! the skin of two bodies presses (`outline.rs`). It keeps the place at
//! least half as far inside the skin as it lay as the body was built, as
//! pressed flesh is left under a bone, and as far as the half of the bone
//! the drawing shows where it lay deep enough, so a bone at rest is not
//! touched and a bone held in is not seen. A push on the skin reaches the bone through the flesh, as
//! skeleton-driven soft bodies pass a contact on their surface to the bones
//! inside (Jain and Liu 2011, "Controlling physics-based characters using
//! soft contacts"; Shinar, Schroeder and Fedkiw 2008, "Two-way coupling of
//! rigid and deformable bodies"). Only where the skin is torn or cut, or
//! the flesh at it torn away, or round an open break, can bone come out.

use super::soft::{speed_up_bone, Softness};
use super::*;

/// How stiff the skin's push on a bone that reaches it is, in hertz, and how
/// damped: Box2D's contact softness, as two limbs meeting (`clash.rs`),
/// capped at a quarter of the substep's rate as Box2D caps it.
const SKIN_HOLD_HERTZ: f64 = 30.0;
const SKIN_HOLD_DAMPING_RATIO: f64 = 10.0;
/// The fastest a bone found outside its skin is drawn back in, in metres a
/// second: Box2D's `contactSpeed`, as the skin's own contacts push.
const SKIN_HOLD_PUSH_SPEED: f64 = 3.0;
/// How far apart, in point spacings, the places along a bone the skin holds
/// lie: about as far as the skin's own points.
const SKIN_HOLD_SPACING: f64 = 1.0;
/// How far outside the skin round it, in point spacings, a bone's place is
/// still drawn back in: past that the flesh has let it go.
const SKIN_HOLD_REACH: f64 = 3.0;
/// How much of the flesh between a bone and its skin, as built, a bone
/// pressed into it leaves: about half, as a heel's pad keeps under a
/// standing foot and the floor under a knee a fighter getting up rests on
/// (`fighter.rs`, `HEEL_PAD_LOADED`; `balance.rs`, `KNEE_PAD`). Kept only
/// as far in as the drawing's half stroke, a downed fighter's shin was
/// pressed to within a few pixels of the floor through 4 to 6 cm of its
/// own flesh, which pulled off the bone and sagged.
const PRESSED_FLESH: f64 = 0.5;
/// The most of its depth as built a place along a bone is kept inside the
/// skin, so that nothing at rest is pushed.
const MOST_KEPT: f64 = 0.8;
/// How firmly, in damping ratio, the skin stops a bone at its wall: the
/// half of the bone the drawing shows inside the skin, held at the stiffest
/// rate a substep allows (a quarter of it, as Box2D caps a soft
/// constraint), critically damped. Held only as softly as the flesh, a
/// knocked-out fighter's forearm lay with a centimetre of bone out of its
/// skin for seconds: the ties to flesh the fall had shifted pulled it out as
/// hard as the contact pushed it in.
const WALL_DAMPING_RATIO: f64 = 1.0;

/// A place along a bone and the stretch of skin round it nearest it this
/// substep, which keep apart by at least `margin`.
#[derive(Clone, Copy, Debug)]
pub(super) struct SkinHold {
    bone: usize,
    /// Where along the bone, from its `a` end.
    along: f64,
    /// The stretch of skin: its two points, and where along it the place is
    /// nearest.
    skin: [usize; 2],
    at: f64,
    /// Into the flesh from the skin there, as the substep began.
    normal: Vec2,
    /// How far inside the skin the place is kept: softly, as pressed flesh
    /// keeps it, to `margin`, and firmly, as the skin's wall, to `wall`.
    margin: f64,
    wall: f64,
    /// The impulse it has given this substep.
    impulse: f64,
}

/// What the skin holds of one bone, as the body was built: the skin's
/// outline round it, the stretches of that outline round it, and for each
/// place along it, evenly spaced from end to end, how far inside the skin it
/// is kept, softly and at the wall (`SkinHold::margin`, `wall`), or nothing
/// where it lay outside.
#[derive(Clone, Debug, Default)]
pub(super) struct BoneInSkin {
    outline: Option<usize>,
    edges: Vec<usize>,
    margins: Vec<Option<(f64, f64)>>,
}

/// The nearest stretch of skin round a place: the stretch, where along it,
/// the way into the flesh there, and how far inside the skin the place is
/// (negative outside).
#[derive(Clone, Copy, Debug)]
struct NearestSkin {
    edge: usize,
    at: f64,
    normal: Vec2,
    depth: f64,
}

impl World {
    /// Whether the skin holds `bone` in: any bone of a side-view fighter the
    /// normal view draws, but the pieces of an open break, which come out
    /// through the skin they cut. The sandbox's front-view bodies keep only
    /// their closed breaks' pieces held in (`hold_closed_pieces_in_the_skin`):
    /// held so, a bat or a hammer pressing a bone's flesh down onto it
    /// pinched the skin between them, and a firm bat swing split the skin
    /// in all of the sweep's 45 runs, where real swings rarely do; and their
    /// flesh, which a heavy blow leaves dented for good, was pushed back
    /// out for ever after, so the body never came to rest.
    pub(super) fn skin_holds(&self, bone: &BoneSegment) -> bool {
        self.side_fighter(bone.body as usize)
            && bone.kind != BoneKind::Spine
            && !bone.open_break
            && bone.a.x.is_finite()
            && bone.b.x.is_finite()
    }

    /// Whether body `body` is a side-view fighter.
    fn side_fighter(&self, body: usize) -> bool {
        self.bodies
            .get(body)
            .is_some_and(|state| state.fighter.is_some())
    }

    /// Whether skin point `point` is of the skin round `bone`: kept in its
    /// place by a bone of the same limb, or, for the head and trunk, by any
    /// bone of them. The same part alone, as a broken piece's skin is found
    /// (`skin_round`), found no skin round a rib: the chest's skin keeps its
    /// place on the spine.
    fn skin_round_whole(&self, point: usize, bone: &BoneSegment) -> bool {
        let Some(hold) = self.point_holds.get(point) else {
            return false;
        };
        let binding = hold.posture;
        binding
            .bones
            .iter()
            .zip(binding.weights)
            .any(|(&index, weight)| {
                weight > 0.0
                    && self
                        .bones
                        .get(index)
                        .is_some_and(|other| other.body == bone.body && other.limb == bone.limb)
            })
    }

    /// The skin's outline round bone `index` and its stretches round it: the
    /// skin loop of the bone's own body and sheet, and each edge of it both
    /// of whose points are of the skin round it (`skin_round_whole`).
    fn skin_round_bone(&self, index: usize) -> (Option<usize>, Vec<usize>) {
        let bone = self.bones[index];
        for (number, outline) in self.outlines.iter().enumerate() {
            let points = outline.points();
            let Some(&first) = points.first() else {
                continue;
            };
            let first = self.points[first];
            if first.layer != TissueLayer::Skin
                || first.body != bone.body
                || first.sheet != bone.sheet
            {
                continue;
            }
            let count = points.len();
            let edges: Vec<usize> = (0..count)
                .filter(|&edge| {
                    self.skin_round_whole(points[edge], &bone)
                        && self.skin_round_whole(points[(edge + 1) % count], &bone)
                })
                .collect();
            if !edges.is_empty() {
                return (Some(number), edges);
            }
        }
        (None, Vec::new())
    }

    /// Works out again, once the bones or the mesh have changed since, what
    /// the skin holds of each bone (`BoneInSkin`): for each place along it,
    /// how far inside the skin it is kept, from how deep it lay as the body
    /// was built: half the flesh there pressed ([`PRESSED_FLESH`]), and as
    /// much as the half of the bone the drawing shows where that lay deep
    /// enough, so that a bone held in is not seen and nothing at rest is
    /// pushed.
    fn refresh_bones_in_skin(&mut self) {
        let key = (self.topology_version, self.bones.len(), self.points.len());
        if self.bones_in_skin_key == Some(key) {
            return;
        }
        let spacing = self.materials.point_spacing.max(1.0);
        let held: Vec<BoneInSkin> = (0..self.bones.len())
            .map(|index| {
                let bone = self.bones[index];
                if !self.skin_holds(&bone) {
                    return BoneInSkin::default();
                }
                let (outline, edges) = self.skin_round_bone(index);
                let Some(outline) = outline else {
                    return BoneInSkin::default();
                };
                let samples = (distance(bone.home_a, bone.home_b) / (spacing * SKIN_HOLD_SPACING))
                    .ceil()
                    .max(1.0) as usize;
                let half = Self::drawn_half_width(&bone);
                let margins = (0..=samples)
                    .map(|sample| {
                        let at = lerp(bone.home_a, bone.home_b, sample as f64 / samples as f64);
                        let found = self.nearest_skin(
                            outline,
                            &edges,
                            at,
                            f64::INFINITY,
                            false,
                            |point| point.home,
                        )?;
                        (found.depth > 0.0).then(|| {
                            let wall = (found.depth * MOST_KEPT).min(half);
                            ((found.depth * PRESSED_FLESH).max(wall), wall)
                        })
                    })
                    .collect();
                BoneInSkin {
                    outline: Some(outline),
                    edges,
                    margins,
                }
            })
            .collect();
        self.bones_in_skin = held;
        self.bones_in_skin_key = Some(key);
    }

    /// Finds, as this substep begins, each place along a bone that may reach
    /// the skin round it within the substep, or lies outside it within
    /// reach, and the stretch of skin it meets there.
    pub(super) fn measure_skin_holds(&mut self, h: f64) {
        self.skin_holds.clear();
        if self.outlines.is_empty() {
            return;
        }
        self.refresh_bones_in_skin();
        let spacing = self.materials.point_spacing.max(1.0);
        let reach = SKIN_HOLD_REACH * spacing;
        let mut holds = Vec::new();
        for (index, held) in self.bones_in_skin.iter().enumerate() {
            let Some(outline) = held.outline else {
                continue;
            };
            let bone = self.bones[index];
            if !self.skin_holds(&bone) {
                continue;
            }
            let points = self.outlines[outline].points();
            let count = points.len();
            let samples = held.margins.len().saturating_sub(1).max(1);
            for (sample, &margins) in held.margins.iter().enumerate() {
                let Some((margin, wall)) = margins else {
                    continue;
                };
                let along = sample as f64 / samples as f64;
                let at = lerp(bone.previous_a, bone.previous_b, along);
                let Some(found) =
                    self.nearest_skin(outline, &held.edges, at, reach, true, |point| {
                        point.previous
                    })
                else {
                    continue;
                };
                let skin = [points[found.edge], points[(found.edge + 1) % count]];
                // How far the place and the skin could close within the
                // substep, moving as they do.
                let bone_velocity = lerp(
                    scale(subtract(bone.a, bone.previous_a), 1.0 / h),
                    scale(subtract(bone.b, bone.previous_b), 1.0 / h),
                    along,
                );
                let skin_velocity = lerp(
                    self.point_velocity(skin[0], h),
                    self.point_velocity(skin[1], h),
                    found.at,
                );
                let closing = (-dot(subtract(bone_velocity, skin_velocity), found.normal)).max(0.0);
                if found.depth - margin > closing * h + spacing {
                    continue;
                }
                holds.push(SkinHold {
                    bone: index,
                    along,
                    skin,
                    at: found.at,
                    normal: found.normal,
                    margin,
                    wall,
                    impulse: 0.0,
                });
            }
        }
        self.skin_holds = holds;
        self.measure_skin_columns();
    }

    fn point_velocity(&self, point: usize, h: f64) -> Vec2 {
        let point = &self.points[point];
        scale(subtract(point.position, point.previous), 1.0 / h)
    }

    /// The muscle the fat ties under each skin point a hold presses, which
    /// presses with it, and how many such skin points each of those muscle
    /// points is tied to, as a pair between two bodies presses
    /// (`find_flesh_under_skin`).
    fn measure_skin_columns(&mut self) {
        self.skin_columns.clear();
        self.skin_column_ties.clear();
        if self.skin_holds.is_empty() {
            return;
        }
        let mut touched: Vec<usize> = self.skin_holds.iter().flat_map(|hold| hold.skin).collect();
        touched.sort_unstable();
        touched.dedup();
        for attachment in &self.attachments {
            if !attachment.broken && touched.binary_search(&attachment.skin_point).is_ok() {
                self.skin_columns
                    .push((attachment.skin_point, attachment.muscle_point));
            }
        }
        self.skin_columns.sort_unstable();
        let mut muscles: Vec<usize> = self
            .skin_columns
            .iter()
            .map(|&(_, muscle)| muscle)
            .collect();
        muscles.sort_unstable();
        for muscle in muscles {
            match self.skin_column_ties.last_mut() {
                Some((last, count)) if *last == muscle => *count += 1,
                _ => self.skin_column_ties.push((muscle, 1)),
            }
        }
    }

    /// The muscle points under skin point `skin` that press with it, each
    /// with how many pressed skin points it is tied to.
    fn skin_column(&self, skin: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
        let from = self
            .skin_columns
            .partition_point(|&(point, _)| point < skin);
        self.skin_columns[from..]
            .iter()
            .take_while(move |&&(point, _)| point == skin)
            .map(|&(_, muscle)| {
                let ties = self
                    .skin_column_ties
                    .binary_search_by_key(&muscle, |&(muscle, _)| muscle)
                    .map_or(1, |index| self.skin_column_ties[index].1);
                (muscle, ties)
            })
    }

    /// One over the mass skin point `skin` presses with: its own and its
    /// share of the muscle under it; nothing moves a pinned point.
    fn column_inverse_mass(&self, skin: usize) -> f64 {
        if self.inverse_masses[skin] <= 0.0 {
            return 0.0;
        }
        let column: f64 = self
            .skin_column(skin)
            .map(|(muscle, ties)| self.points[muscle].mass / ties as f64)
            .sum();
        1.0 / (self.points[skin].mass + column).max(EPSILON)
    }

    /// Gives skin point `skin` and the muscle under it the velocity change
    /// `change` over a substep `h` long: the muscle by its share of the
    /// skin points it is tied to.
    fn speed_up_column(&mut self, skin: usize, change: Vec2, h: f64) {
        let column: Vec<(usize, usize)> = self.skin_column(skin).collect();
        let moved = scale(change, h);
        self.points[skin].position = add(self.points[skin].position, moved);
        for (muscle, ties) in column {
            if self.inverse_masses[muscle] > 0.0 {
                self.points[muscle].position = add(
                    self.points[muscle].position,
                    scale(moved, 1.0 / ties as f64),
                );
            }
        }
    }

    /// Keeps each place a hold found as far inside the skin as its margin,
    /// as Box2D solves a contact point, one after another: the bone pushed
    /// in at the place, by its ends' shares, and the skin with the muscle
    /// under it out; softly down to its margin, and as firmly as a substep
    /// allows past its wall.
    pub(super) fn solve_skin_holds(&mut self, h: f64) {
        if self.skin_holds.is_empty() {
            return;
        }
        let hertz = SKIN_HOLD_HERTZ.min(0.25 / h);
        let softness = Softness::new(2.0 * PI * hertz, SKIN_HOLD_DAMPING_RATIO, h);
        let firmness = Softness::new(2.0 * PI * 0.25 / h, WALL_DAMPING_RATIO, h);
        let max_push = SKIN_HOLD_PUSH_SPEED * 1000.0 / self.mm_per_px();
        for index in 0..self.skin_holds.len() {
            let hold = self.skin_holds[index];
            let bone = self.bones[hold.bone];
            let [first, second] = hold.skin;
            let place = lerp(bone.a, bone.b, hold.along);
            let skin = lerp(
                self.points[first].position,
                self.points[second].position,
                hold.at,
            );
            let depth = dot(subtract(place, skin), hold.normal);
            let (gap, softness) = if depth < hold.wall {
                (depth - hold.wall, &firmness)
            } else {
                (depth - hold.margin, &softness)
            };
            let bone_velocity = lerp(
                scale(subtract(bone.a, bone.previous_a), 1.0 / h),
                scale(subtract(bone.b, bone.previous_b), 1.0 / h),
                hold.along,
            );
            let skin_velocity = lerp(
                self.point_velocity(first, h),
                self.point_velocity(second, h),
                hold.at,
            );
            let speed = dot(subtract(bone_velocity, skin_velocity), hold.normal);
            let end = bone.inverse_end_mass();
            let bone_give = end * ((1.0 - hold.along).powi(2) + hold.along.powi(2));
            let (give_first, give_second) = (
                self.column_inverse_mass(first),
                self.column_inverse_mass(second),
            );
            let skin_give = (1.0 - hold.at).powi(2) * give_first + hold.at.powi(2) * give_second;
            if bone_give + skin_give <= EPSILON {
                continue;
            }
            let mass = 1.0 / (bone_give + skin_give);
            let impulse = softness.contact_impulse(mass, speed, gap, hold.impulse, max_push, h);
            let total = (hold.impulse + impulse).max(0.0);
            let applied = total - hold.impulse;
            self.skin_holds[index].impulse = total;
            if applied == 0.0 {
                continue;
            }
            let push = scale(hold.normal, applied);
            speed_up_bone(
                &mut self.bones[hold.bone],
                scale(push, end * (1.0 - hold.along)),
                scale(push, end * hold.along),
                h,
            );
            self.speed_up_column(first, scale(push, -give_first * (1.0 - hold.at)), h);
            self.speed_up_column(second, scale(push, -give_second * hold.at), h);
            self.debug.skin_held_bones += 1;
        }
    }

    /// The nearest whole stretch of skin among `edges` of outline `outline`
    /// to `at`, judged where `place` puts each point, within `reach`: how
    /// far inside the skin `at` lies by the outline's way in there, an
    /// edge's own across it and, where its nearest is a corner, the way in
    /// between the corner's two edges, so a place just outside a rounded
    /// end reads as outside. With `whole_only`, a stretch of skin torn or
    /// cut, or of flesh torn away, is no surface.
    fn nearest_skin(
        &self,
        outline: usize,
        edges: &[usize],
        at: Vec2,
        reach: f64,
        whole_only: bool,
        place: impl Fn(&Point) -> Vec2,
    ) -> Option<NearestSkin> {
        let loop_ = &self.outlines[outline];
        let points = loop_.points();
        let count = points.len();
        let inward = |edge: usize| {
            let (a, b) = (
                place(&self.points[points[edge]]),
                place(&self.points[points[(edge + 1) % count]]),
            );
            let along = subtract(b, a);
            let length = hypot(along.x, along.y);
            (length > EPSILON).then(|| Vec2 {
                x: along.y / length * loop_.outward(),
                y: -along.x / length * loop_.outward(),
            })
        };
        let mut nearest: Option<(f64, usize, f64)> = None;
        for &edge in edges {
            if whole_only && !self.skin_edge_whole(outline, edge) {
                continue;
            }
            let (a, b) = (
                place(&self.points[points[edge]]),
                place(&self.points[points[(edge + 1) % count]]),
            );
            let t = segment_t(at, a, b);
            let gap = distance(at, lerp(a, b, t));
            if gap > reach || nearest.is_some_and(|(closest, ..)| gap >= closest) {
                continue;
            }
            nearest = Some((gap, edge, t));
        }
        let (_, edge, t) = nearest?;
        let own = inward(edge)?;
        // A corner's way in lies between its two edges' (the pseudonormal).
        let normal = if t <= 0.0 || t >= 1.0 {
            let other = if t <= 0.0 {
                (edge + count - 1) % count
            } else {
                (edge + 1) % count
            };
            inward(other).map_or(own, |other| normalized(add(own, other), own))
        } else {
            own
        };
        let (a, b) = (
            place(&self.points[points[edge]]),
            place(&self.points[points[(edge + 1) % count]]),
        );
        let depth = dot(subtract(at, lerp(a, b, t)), normal);
        Some(NearestSkin {
            edge,
            at: t,
            normal,
            depth,
        })
    }

    /// Whether edge `edge` of outline `outline` is whole skin that holds a
    /// bone in: its spring not torn or cut, and both its points flesh.
    fn skin_edge_whole(&self, outline: usize, edge: usize) -> bool {
        let loop_ = &self.outlines[outline];
        let points = loop_.points();
        let count = points.len();
        loop_.edge_spring(edge).is_some_and(|spring| {
            !self.springs[spring].broken
                && is_flesh(&self.flesh, points[edge])
                && is_flesh(&self.flesh, points[(edge + 1) % count])
        })
    }

    /// How far inside the skin the normal view hides bone `bone`: half the
    /// stroke the drawing gives it (`draw::bones`).
    fn drawn_half_width(bone: &BoneSegment) -> f64 {
        let broken = bone.fractured || bone.splinter || bone.broken_start || bone.broken_end;
        (bone.radius * if broken { 1.85 } else { 1.45 }).max(2.5) * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints how deep inside its own skin each bone lies as built, the
    /// least along it, against half the stroke it is drawn with
    /// (`cargo test --release --lib print_bones_in_skin -- --ignored
    /// --nocapture`).
    #[test]
    #[ignore]
    fn print_bones_in_skin() {
        for (name, world) in [
            ("side fighters", create_side_fighters(Materials::default())),
            (
                "front view",
                create_layered_body(1280.0, 720.0, Materials::default()),
            ),
        ] {
            println!("{name}:");
            let spacing = world.materials.point_spacing;
            for index in 0..world.bones.len() {
                let bone = world.bones[index];
                if !world.skin_holds(&bone) {
                    continue;
                }
                let (outline, edges) = world.skin_round_bone(index);
                let Some(outline) = outline else {
                    println!("  {:>3} {:?}: no skin round it", index, bone.part);
                    continue;
                };
                let samples = (distance(bone.home_a, bone.home_b) / (spacing * SKIN_HOLD_SPACING))
                    .ceil()
                    .max(1.0) as usize;
                let depths: Vec<f64> = (0..=samples)
                    .map(|sample| {
                        let at = lerp(bone.home_a, bone.home_b, sample as f64 / samples as f64);
                        world
                            .nearest_skin(outline, &edges, at, f64::INFINITY, false, |point| {
                                point.home
                            })
                            .map_or(f64::NAN, |found| found.depth)
                    })
                    .collect();
                let least = depths.iter().copied().fold(f64::INFINITY, f64::min);
                let ends = (depths[0], depths[depths.len() - 1]);
                println!(
                    "  {:>3} {:>10} body {} sheet {:>2}: least {:6.1} px (ends {:5.1}, {:5.1}), drawn half {:4.1}, {} edges",
                    index,
                    bone.part.name(),
                    bone.body,
                    bone.sheet,
                    least,
                    ends.0,
                    ends.1,
                    World::drawn_half_width(&bone),
                    edges.len()
                );
            }
        }
    }
}

#[cfg(test)]
mod rest_tests {
    use super::*;

    /// The skin holding bones changes nothing in a body at rest: the side
    /// fighters standing come to rest where they do without it, and the
    /// sandbox's body, which it does not hold, hangs in its guard as it
    /// did.
    #[test]
    fn the_skin_holding_bones_leaves_a_body_at_rest_alone() {
        let worlds: [(&str, fn(Materials) -> World); 2] = [
            ("side fighters", create_side_fighters),
            ("front view", |materials| {
                create_layered_body(1280.0, 720.0, materials)
            }),
        ];
        for (name, build) in worlds {
            let settled = |holds: bool| {
                let mut materials = Materials::default();
                materials.skin_holds_bones = holds;
                let mut world = build(materials);
                let dt = world.materials.fixed_dt;
                for _ in 0..300 {
                    world.step(dt, &InputState::default(), 1280.0);
                }
                world
            };
            let (without, with) = (settled(false), settled(true));
            let points = without
                .points
                .iter()
                .zip(&with.points)
                .map(|(a, b)| distance(a.position, b.position))
                .fold(0.0, f64::max);
            let bones = without
                .bones
                .iter()
                .zip(&with.bones)
                .map(|(a, b)| distance(a.a, b.a).max(distance(a.b, b.b)))
                .fold(0.0, f64::max);
            println!("{name}: points moved up to {points:.3} px, bones {bones:.3} px");
            assert!(
                points < 0.5 && bones < 0.5,
                "{name}: the skin holding bones moved a body at rest, points {points:.2} px, bones {bones:.2} px"
            );
        }
    }
}
