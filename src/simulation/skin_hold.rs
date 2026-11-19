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
//! from inside, as one body meets another: a contact, as Box2D v3 solves a
//! contact point (Erin Catto, MIT, `contact_solver.c`: speculative before
//! they touch, pushing out no faster than 3 m/s), between the bone and the
//! stretch of skin nearest it, the skin pressing with the muscle the fat
//! ties under it, as the skin of two bodies presses (`outline.rs`). The skin
//! is a wall: it stops the place where the drawing would begin to show the
//! bone, half its stroke inside the skin (or four fifths of how deep it lay
//! as the body was built, where it lay shallower, so nothing at rest is
//! pushed), at the stiffest rate a substep allows. A push on the skin
//! reaches the bone through the flesh, as
//! skeleton-driven soft bodies pass a contact on their surface to the bones
//! inside (Jain and Liu 2011, "Controlling physics-based characters using
//! soft contacts"; Shinar, Schroeder and Fedkiw 2008, "Two-way coupling of
//! rigid and deformable bodies"). Only where the skin is torn or cut, or
//! the flesh at it torn away, or round an open break, can bone come out.
//!
//! The sandbox's front-view body is held so too, by more rules that a body
//! struck by tools, its limbs one sheet of flesh, needs. Held as a fighter
//! once was, half its flesh's depth kept, a tool crushing flesh onto a bone
//! pinched the skin against that depth, and a firm bat swing split the
//! skin in every run of the sweep; flesh a heavy blow dents for good was
//! pushed back out for ever, and the body never came to rest; a punch on a
//! front-view fighter's guard passed its push through the hold straight to
//! the forearm's bone and folded the arm down for good; and the skin of an
//! elbow's inner crease, round both the forearm's bone and the upper arm's,
//! was pushed two ways by them and tore. So (1) skin a tool's or a fist's
//! surface reaches, or other skin presses (another body's, or its own
//! across a fold), lets go of the bone under it: the bone may be pressed
//! onto it, as a hammer crushes flesh onto bone, and what presses reaches
//! the bone by its own contacts with bones (`tools.rs`, `clash.rs`); (2) a
//! limb's bone meets only the skin held mostly to it, not its whole limb's;
//! (3) skin turned over in a crease is no wall; and (4) the wall holds
//! softly, as two limbs meeting do: held as firmly as a substep allows,
//! even only once a bone was past it, the elbow's crease tore under every
//! punch on the guard. A closed break's pieces, which no skin is held to,
//! keep their own hold (`hold_closed_pieces_in_the_skin`).

use super::soft::{speed_up_bone, Softness};
use super::*;

/// How stiff a front-view body's skin's push on a bone that reaches it is,
/// in hertz, and how damped: Box2D's contact softness, as two limbs meeting
/// (`clash.rs`), capped at a quarter of the substep's rate as Box2D caps it.
/// Held as firmly as a fighter's, the skin of the sandbox's elbow crease
/// tore under every punch on the guard.
const FRONT_HOLD_HERTZ: f64 = 30.0;
const FRONT_HOLD_DAMPING_RATIO: f64 = 10.0;
/// The fastest a bone found outside its skin is drawn back in, in metres a
/// second: Box2D's `contactSpeed`, as the skin's own contacts push.
const SKIN_HOLD_PUSH_SPEED: f64 = 3.0;
/// How far apart, in point spacings, the places along a bone the skin holds
/// lie: about as far as the skin's own points.
const SKIN_HOLD_SPACING: f64 = 1.0;
/// How far outside the skin round it, in point spacings, a bone's place is
/// still drawn back in: past that the flesh has let it go.
const SKIN_HOLD_REACH: f64 = 3.0;
/// The most of its depth as built a place along a bone is kept inside the
/// skin, so that nothing at rest is pushed.
const MOST_KEPT: f64 = 0.8;
/// How firmly, in damping ratio, the skin stops a bone at its wall: at the
/// stiffest rate a substep allows (a quarter of it, as Box2D caps a soft
/// constraint), critically damped. As soft as a contact between two limbs
/// (30 Hz, damped tenfold, `clash.rs`), a knocked-out fighter's forearm lay
/// with a centimetre of bone out of its skin for seconds: the ties to flesh
/// its fall had shifted pulled it out as hard as the skin pushed it in. Kept
/// softly half as deep as built besides, as pressed flesh is left under a
/// bone, a fighter getting up off its back sat a little higher on its own
/// flesh than its key poses were set on, and took a second try.
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
    /// How far inside the skin the place is kept (`BoneInSkin::margins`).
    margin: f64,
    /// The impulse it has given this substep.
    impulse: f64,
}

/// What the skin holds of one bone, as the body was built: the skin's
/// outline round it, the stretches of that outline round it, and for each
/// place along it, evenly spaced from end to end, how far inside the skin it
/// is kept, or nothing where it lay outside.
#[derive(Clone, Debug, Default)]
pub(super) struct BoneInSkin {
    outline: Option<usize>,
    edges: Vec<usize>,
    margins: Vec<Option<f64>>,
}

/// Which stretches of skin a place may meet: any (as the body was built),
/// whole skin, or whole skin not turned over in a crease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Surface {
    Any,
    Whole,
    Uncreased,
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
    /// Whether the skin holds `bone` in: any bone the normal view draws, of
    /// a side-view fighter or a front-view body, but the pieces of an open
    /// break, which come out through the skin they cut.
    pub(super) fn skin_holds(&self, bone: &BoneSegment) -> bool {
        let on = if self.side_fighter(bone.body as usize) {
            self.materials.skin_holds_bones
        } else {
            self.materials.skin_holds_front
        };
        on && self.skin_could_hold(bone)
    }

    /// Whether the skin would hold `bone` in with its body's setting on
    /// (`skin_holds_bones`, `skin_holds_front`): what is worked out of it
    /// once per mesh (`refresh_bones_in_skin`) whatever the setting, so that
    /// turning it on partway into a play holds it.
    fn skin_could_hold(&self, bone: &BoneSegment) -> bool {
        bone.kind != BoneKind::Spine
            && !bone.open_break
            && bone.a.x.is_finite()
            && bone.b.x.is_finite()
    }

    /// Whether body `body` is a side-view fighter.
    pub(super) fn side_fighter(&self, body: usize) -> bool {
        self.bodies
            .get(body)
            .is_some_and(|state| state.fighter.is_some())
    }

    /// Whether skin point `point` is of the skin round bone `index`: kept in
    /// its place by a bone of the same limb, or, for the head and trunk, by
    /// any bone of them. The same part alone, as a broken piece's skin is
    /// found (`skin_round`), found no skin round a rib: the chest's skin
    /// keeps its place on the spine. A front-view body's limb bone meets
    /// only skin held mostly to it: the limbs are one sheet of flesh, and
    /// the skin of an elbow's inner crease, held to the forearm and the
    /// upper arm both, was pushed two ways by their bones and tore, and at
    /// rest the upper arm's bone lay against it.
    fn skin_round_whole(&self, point: usize, index: usize) -> bool {
        let Some(hold) = self.point_holds.get(point) else {
            return false;
        };
        let bone = &self.bones[index];
        let own = !self.side_fighter(bone.body as usize) && bone.limb != Limb::None;
        let binding = hold.posture;
        binding
            .bones
            .iter()
            .zip(binding.weights)
            .any(|(&other, weight)| {
                weight > 0.0
                    && if own {
                        other == index && weight >= 0.5
                    } else {
                        self.bones
                            .get(other)
                            .is_some_and(|other| other.body == bone.body && other.limb == bone.limb)
                    }
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
                    self.skin_round_whole(points[edge], index)
                        && self.skin_round_whole(points[(edge + 1) % count], index)
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
    /// how far inside the skin it is kept: half the stroke the drawing gives
    /// the bone, so a bone held in is not seen, or four fifths of how deep it
    /// lay as the body was built where that is less, so nothing at rest is
    /// pushed.
    fn refresh_bones_in_skin(&mut self) {
        let key = (
            self.topology_version,
            self.bones.len(),
            self.points.len(),
            self.attachments.len(),
        );
        if self.bones_in_skin_key == Some(key) {
            return;
        }
        let spacing = self.materials.point_spacing.max(1.0);
        let held: Vec<BoneInSkin> = (0..self.bones.len())
            .map(|index| {
                let bone = self.bones[index];
                if !self.skin_could_hold(&bone) {
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
                            Surface::Any,
                            |point| point.home,
                        )?;
                        (found.depth > 0.0).then(|| (found.depth * MOST_KEPT).min(half))
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
        // The fat ties under each skin point, which the skin presses with
        // (`measure_skin_columns`).
        let mut ties: Vec<(usize, usize)> = self
            .attachments
            .iter()
            .enumerate()
            .map(|(tie, attachment)| (attachment.skin_point, tie))
            .collect();
        ties.sort_unstable();
        self.fat_ties_by_skin = ties;
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
        let pressed = self.pressed_skin();
        let mut holds = Vec::new();
        for (index, held) in self.bones_in_skin.iter().enumerate() {
            let Some(outline) = held.outline else {
                continue;
            };
            let bone = self.bones[index];
            if !self.skin_holds(&bone) {
                continue;
            }
            // A front-view body's skin lets go of a bone where something
            // else presses it, and skin creased over is no wall there.
            let yields = !self.side_fighter(bone.body as usize);
            let points = self.outlines[outline].points();
            let count = points.len();
            let samples = held.margins.len().saturating_sub(1).max(1);
            for (sample, &margin) in held.margins.iter().enumerate() {
                let Some(margin) = margin else {
                    continue;
                };
                let along = sample as f64 / samples as f64;
                let at = lerp(bone.previous_a, bone.previous_b, along);
                let Some(found) = self.nearest_skin(
                    outline,
                    &held.edges,
                    at,
                    reach,
                    if yields {
                        Surface::Uncreased
                    } else {
                        Surface::Whole
                    },
                    |point| point.previous,
                ) else {
                    continue;
                };
                let skin = [points[found.edge], points[(found.edge + 1) % count]];
                if yields
                    && (pressed.binary_search(&skin[0]).is_ok()
                        || pressed.binary_search(&skin[1]).is_ok())
                {
                    continue;
                }
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
                    impulse: 0.0,
                });
            }
        }
        self.skin_holds = holds;
        self.measure_skin_columns();
    }

    /// Each point of flesh something else presses as this substep begins,
    /// which lets go of a front-view body's bone under it: where a tool's
    /// or a fist's surface reaches it (`points_touched`, within half a point
    /// spacing; all the tool may reach this step covered a thin limb whole,
    /// and the bone went out of its far side), and skin touching other skin
    /// (`mark_skin_touching`), sorted, each once. Nothing, if no
    /// front-view body is held.
    fn pressed_skin(&self) -> Vec<usize> {
        let front = self.materials.skin_holds_front
            && (0..self.bodies.len()).any(|body| !self.side_fighter(body));
        if !front {
            return Vec::new();
        }
        let mut pressed = self.points_touched(0.5 * self.materials.point_spacing);
        self.mark_skin_touching(&mut pressed);
        pressed.sort_unstable();
        pressed.dedup();
        pressed
    }

    /// Adds to `marks` each skin point touching other skin as this substep
    /// begins: a point and the stretch of outline it meets
    /// (`outline_pairs`), of another body or of its own across a gap,
    /// within a point spacing.
    fn mark_skin_touching(&self, marks: &mut Vec<usize>) {
        let touch = self.materials.point_spacing;
        for pair in &self.outline_pairs {
            let (point, edge, outline) = (pair.point, pair.edge, pair.outline);
            let points = self.outlines[outline].points();
            let count = points.len();
            let (a, b) = (points[edge], points[(edge + 1) % count]);
            let gap = distance_to_segment(
                self.points[point].previous,
                self.points[a].previous,
                self.points[b].previous,
            );
            if gap <= touch {
                marks.extend([point, a, b]);
            }
        }
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
        for skin in touched {
            let from = self
                .fat_ties_by_skin
                .partition_point(|&(point, _)| point < skin);
            for &(point, tie) in &self.fat_ties_by_skin[from..] {
                if point != skin {
                    break;
                }
                let attachment = &self.attachments[tie];
                if !attachment.broken {
                    self.skin_columns.push((skin, attachment.muscle_point));
                }
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
        let moved = scale(change, h);
        self.points[skin].position = add(self.points[skin].position, moved);
        let from = self
            .skin_columns
            .partition_point(|&(point, _)| point < skin);
        for index in from..self.skin_columns.len() {
            let (point, muscle) = self.skin_columns[index];
            if point != skin {
                break;
            }
            let ties = self
                .skin_column_ties
                .binary_search_by_key(&muscle, |&(muscle, _)| muscle)
                .map_or(1, |found| self.skin_column_ties[found].1);
            if self.inverse_masses[muscle] > 0.0 {
                self.points[muscle].position = add(
                    self.points[muscle].position,
                    scale(moved, 1.0 / ties as f64),
                );
            }
        }
    }

    /// Keeps each place a hold found as far inside the skin as its margin,
    /// as Box2D solves a contact point, one after another, as firmly as a
    /// substep allows: the bone pushed in at the place, by its ends' shares,
    /// and the skin with the muscle under it out.
    pub(super) fn solve_skin_holds(&mut self, h: f64) {
        if self.skin_holds.is_empty() {
            return;
        }
        let firmness = Softness::new(2.0 * PI * 0.25 / h, WALL_DAMPING_RATIO, h);
        let softness = Softness::new(
            2.0 * PI * FRONT_HOLD_HERTZ.min(0.25 / h),
            FRONT_HOLD_DAMPING_RATIO,
            h,
        );
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
            let gap = dot(subtract(place, skin), hold.normal) - hold.margin;
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
            let held = if self.side_fighter(bone.body as usize) {
                &firmness
            } else {
                &softness
            };
            let impulse = held.contact_impulse(mass, speed, gap, hold.impulse, max_push, h);
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
    /// end reads as outside. On a `Surface::Whole` or `Uncreased`, a stretch
    /// of skin torn or cut, or of flesh torn away, is no surface, and on an
    /// `Uncreased` one neither is a stretch turned over in a crease.
    fn nearest_skin(
        &self,
        outline: usize,
        edges: &[usize],
        at: Vec2,
        reach: f64,
        surface: Surface,
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
            let (a, b) = (
                place(&self.points[points[edge]]),
                place(&self.points[points[(edge + 1) % count]]),
            );
            let t = segment_t(at, a, b);
            // Squared, as only which is nearer matters: `hypot` on every
            // stretch of skin for every place was most of the hold's cost.
            let gap = distance_sq(at, lerp(a, b, t));
            // Whether it is whole skin is asked only of a stretch nearer
            // than the nearest found yet, which few are.
            if gap > reach * reach
                || nearest.is_some_and(|(closest, ..)| gap >= closest)
                || (surface != Surface::Any && !self.skin_edge_whole(outline, edge))
            {
                continue;
            }
            // A stretch of skin turned over in a crease faces into the body,
            // as the skin's own contacts skip it (`solve_outline_contacts`).
            if surface == Surface::Uncreased {
                let (home_a, home_b) = (
                    self.points[points[edge]].home,
                    self.points[points[(edge + 1) % count]].home,
                );
                if dot(subtract(b, a), subtract(home_b, home_a)) <= 0.0 {
                    continue;
                }
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

    /// The skin holds a fighter's bones in without pushing a fighter at
    /// rest: standing in its stance, no bone comes near enough to its skin
    /// to be pushed.
    #[test]
    fn the_skin_leaves_a_fighter_at_rest_alone() {
        let mut world = create_side_fighters(Materials::default());
        let dt = world.materials.fixed_dt;
        for step in 0..300 {
            world.step(dt, &InputState::default(), crate::scenarios::SCENARIO_WIDTH);
            assert_eq!(
                world.debug().skin_held_bones,
                0,
                "the skin pushed a bone of a fighter at rest on step {step}"
            );
        }
    }

    /// Both kinds of body have their bones held in their skin, and each
    /// kind's setting turns it off, leaving only closed breaks' pieces held
    /// in (`hold_closed_pieces_in_the_skin`), to set the two beside each
    /// other.
    #[test]
    fn every_body_s_bones_are_held_in_their_skin() {
        let fighters = create_side_fighters(Materials::default());
        assert!(fighters.bones.iter().any(|bone| fighters.skin_holds(bone)));
        let sandbox = front_view();
        assert!(sandbox.bones.iter().any(|bone| sandbox.skin_holds(bone)));
        let mut off = Materials::default();
        off.skin_holds_bones = false;
        off.skin_holds_front = false;
        let fighters = create_side_fighters(off);
        assert!(fighters.bones.iter().all(|bone| !fighters.skin_holds(bone)));
        let sandbox = create_layered_body(1280.0, 720.0, off);
        assert!(sandbox.bones.iter().all(|bone| !sandbox.skin_holds(bone)));
    }

    fn front_view() -> World {
        create_layered_body(1280.0, 720.0, Materials::default())
    }

    /// The sandbox's body settling into its guard and hanging there is
    /// never pushed by its skin's hold either.
    #[test]
    fn the_skin_leaves_the_sandbox_body_at_rest_alone() {
        let mut world = front_view();
        let dt = world.materials.fixed_dt;
        for step in 0..300 {
            world.step(dt, &InputState::default(), crate::scenarios::SCENARIO_WIDTH);
            assert_eq!(
                world.debug().skin_held_bones,
                0,
                "the skin pushed a bone of the body at rest on step {step}"
            );
        }
    }

    /// A front-view limb's bone meets only skin held to it: the skin of an
    /// elbow's inner crease, held to the forearm and the upper arm both,
    /// was pushed two ways by their bones and tore.
    #[test]
    fn a_front_view_limb_bone_meets_only_its_own_skin() {
        let world = front_view();
        let mut limb_bones = 0;
        for index in 0..world.bones.len() {
            let bone = world.bones[index];
            if bone.limb == Limb::None || !world.skin_holds(&bone) {
                continue;
            }
            limb_bones += 1;
            let (outline, edges) = world.skin_round_bone(index);
            let outline = outline.expect("a limb's bone has skin round it");
            let points = world.outlines[outline].points();
            for edge in edges {
                for point in [points[edge], points[(edge + 1) % points.len()]] {
                    let binding = world.point_holds[point].posture;
                    assert!(
                        binding
                            .bones
                            .iter()
                            .zip(binding.weights)
                            .any(|(&other, weight)| other == index && weight >= 0.5),
                        "bone {index} meets skin point {point}, which is not held mostly to it"
                    );
                }
            }
        }
        assert!(limb_bones >= 12, "only {limb_bones} limb bones held");
    }

    /// The check for bone through whole skin (`inspect::shown`) sees a
    /// hand's bone moved out past its fist, and calls it a wound once the
    /// skin there is torn.
    #[test]
    fn a_bone_out_through_whole_skin_shows_and_in_a_wound_does_not() {
        use crate::inspect::{bare_bones, shown, Shown};
        let mut world = create_layered_body(1280.0, 720.0, Materials::default());
        let showing = |world: &World, kind: Shown| {
            bare_bones(world)
                .iter()
                .filter(|bare| shown(world, bare) == Some(kind))
                .map(|bare| bare.bone)
                .collect::<Vec<usize>>()
        };
        assert!(
            showing(&world, Shown::Whole).is_empty(),
            "bone shows at rest"
        );
        let hand = world
            .bones
            .iter()
            .position(|bone| bone.part == BonePart::Hand)
            .expect("a hand");
        let bone = world.bones[hand];
        let out = scale(normalized(subtract(bone.b, bone.a), Vec2::default()), 25.0);
        world.bones[hand].a = add(bone.a, out);
        world.bones[hand].b = add(bone.b, out);
        assert_eq!(showing(&world, Shown::Whole), vec![hand]);
        let tip = world.bones[hand].b;
        let reach = 3.0 * world.materials.point_spacing;
        for spring in &mut world.springs {
            let (a, b) = (
                world.points[spring.a].position,
                world.points[spring.b].position,
            );
            if spring.layer == TissueLayer::Skin && distance_to_segment(tip, a, b) < reach {
                spring.broken = true;
            }
        }
        assert!(showing(&world, Shown::Whole).is_empty());
        assert_eq!(showing(&world, Shown::InWound), vec![hand]);
    }
}

#[cfg(test)]
mod print_tests {
    use super::*;

    /// Prints how deep inside its own skin each bone lies as built, the
    /// least along it, against half the stroke it is drawn with, and how
    /// many stretches of skin hold it (`cargo test --release --lib
    /// print_bones_in_skin -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn print_bones_in_skin() {
        let materials = Materials::default();
        for (name, world) in [
            ("side fighters", create_side_fighters(materials)),
            ("front view", create_layered_body(1280.0, 720.0, materials)),
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
                            .nearest_skin(
                                outline,
                                &edges,
                                at,
                                f64::INFINITY,
                                Surface::Any,
                                |point| point.home,
                            )
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
