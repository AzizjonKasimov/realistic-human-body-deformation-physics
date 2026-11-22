//! Where two bodies' limbs meet, they press on each other's bones through the
//! flesh between them. The skins of two fighters push each other apart
//! (`outline.rs`), but skin moves only skin and the muscle under it: two arms
//! driven into each other by a punch's muscles, as two fighters punching a
//! few steps apart drive their guards elbow into elbow, ran their bones on
//! through each other's flesh until the bones crossed, the skin crumpled, and
//! bone showed.
//!
//! So each bone of a limb, and the skull, is a capsule as thick as the flesh
//! round it (a skull's, which lies nearer the back of the head than the
//! face, as thick each way round it as the head's flesh lies that way:
//! `flesh_toward`), and where two bodies' capsules overlap they are pushed
//! apart as
//! Box2D v3 pushes two bodies' shapes apart (Erin Catto, MIT, `contact_solver.c`:
//! a soft contact at 30 Hz, damped tenfold, pushing out no faster than 3 m/s,
//! speculative before they touch), on the bone and the flesh held to it as one
//! piece, in the same pass as the muscles that drive the limbs, so a driven
//! arm is stopped or turned aside by the other instead of passing through. A
//! punching arm pressed hard gives way (`World::block_punch`).
//!
//! All the pairs are pushed apart at once, each from where things stood as
//! the pass began, so that neither of two fighters is solved first. A piece
//! pressed at several places at once is then pushed at each as if that were
//! the only one, and all of them together push it too hard. Two fighters
//! seen from the front seldom meet at more than a place or two, and there it
//! is left so. Seen from the side, a block's two arms stand in one plane
//! with whatever comes at them, and each moves as one braced piece: two
//! fighters blocking toe to toe pressed one arm on another at seven places,
//! pushed seven times too hard, back and forth harder each pass, and within
//! one step the arms flew apart at three kilometres a second. So a side-view
//! fighter's piece shares its weight among the places it is pressed, each
//! place pushing only its share, as a solver that takes its contacts all at
//! once splits a body's mass among them (Tonge, Benevolenski and Voroshilov,
//! "Mass Splitting for Jitter-Free Parallel Rigid Body Simulation",
//! SIGGRAPH 2012).

use super::sides::BonePiece;
use super::soft::Softness;
use super::*;

/// How stiff the push between two limbs is, in hertz, and how damped: Box2D's
/// contact softness (`b2_contactHertz`, `contactDampingRatio`), capped at a
/// quarter of the substep's rate as it caps it.
const CLASH_HERTZ: f64 = 30.0;
const CLASH_DAMPING_RATIO: f64 = 10.0;

/// The fastest an overlap of two limbs is pushed apart, in metres a second:
/// Box2D's `contactSpeed`, as the skin's own contact (`OUTLINE_PUSH_SPEED`).
const CLASH_PUSH_SPEED: f64 = 3.0;

/// How hard, in newtons, another body may press on an arm its punch drives
/// before the punch gives way: driven on at full strength into an arm held
/// there, a punching arm pressed it into its own body until a forearm's bone
/// came out through its skin, the hips' hold pushing behind both. Since the
/// arms' joints hold their bones with their flesh (`sides.rs`), the guard a
/// punch rides over presses back with the arm's weight: at 400 N the punch
/// gave way to it short of the head, and at 800 N punches thrown again and
/// again pressed on into the guard until a wrist tore.
const CLASH_BLOCK_FORCE: f64 = 600.0;

/// How many ways round a bone its flesh is measured (`BoneSegment::flesh_round`):
/// every eighth of a turn, from beyond its `b` end, counterclockwise.
/// Measured as one thickness all round, a skull, which lies nearer the back
/// of the head than the face, met a fist at the flesh of the back of the
/// head: after a cross landed on the jaw the hand's bone drove about 11 px
/// (3.5 cm) on through its own knuckles' flesh before the skull's flesh
/// stopped it, and the hand's flesh tore off its bone a few punches a
/// minute.
pub const FLESH_WAYS: usize = 8;

/// The way round bone `along` (from its `a` end to its `b`) that `off`
/// points, in [`FLESH_WAYS`]ths of a turn from beyond its `b` end,
/// counterclockwise.
fn way_round(along: Vec2, off: Vec2) -> f64 {
    let turn = libm::atan2(cross(along, off), dot(along, off));
    turn / (2.0 * PI) * FLESH_WAYS as f64
}

/// How far `bone`'s flesh reaches from it the way `way` points, as built:
/// its flesh measured each way round it (`flesh_round`), between the two
/// nearest ways in proportion, its flesh all round (`flesh`) where none of
/// its skin lies a way.
pub(super) fn flesh_toward(bone: &BoneSegment, way: Vec2) -> f64 {
    let along = subtract(bone.b, bone.a);
    if bone.flesh_round == [0.0; FLESH_WAYS]
        || dot(along, along) <= EPSILON
        || dot(way, way) <= EPSILON
    {
        return bone.flesh;
    }
    let turn = way_round(along, way);
    let below = libm::floor(turn);
    let share = turn - below;
    let at = |way: f64| {
        let index = (way as i64).rem_euclid(FLESH_WAYS as i64) as usize;
        let flesh = bone.flesh_round[index];
        if flesh > 0.0 {
            flesh
        } else {
            bone.flesh
        }
    };
    at(below) * (1.0 - share) + at(below + 1.0) * share
}

/// `flesh_round` as a mirror image of the body has it: each way turned the
/// other way round.
pub(super) fn mirrored_flesh(round: [f64; FLESH_WAYS]) -> [f64; FLESH_WAYS] {
    std::array::from_fn(|way| round[(FLESH_WAYS - way) % FLESH_WAYS])
}

/// A bone of one body and a bone of another whose flesh overlaps, or may
/// overlap within this substep, and how they press on each other.
#[derive(Clone, Debug)]
pub(super) struct Clash {
    /// The two bones and the flesh held to each, moved as one piece.
    sides: [ClashSide; 2],
    /// From the first bone toward the second, where they come closest.
    normal: Vec2,
    /// The flesh both carry, axis to axis: how near the bones may come.
    reach: f64,
    /// How readily each side's piece moves along `normal` where the bones
    /// come closest, pushed there alone: one over its mass there.
    gives: [f64; 2],
    /// The impulse it has given this substep, pushing them apart.
    impulse: f64,
}

/// One side of a [`Clash`]: a bone and the flesh held to it as one piece,
/// with the rest of a punching arm beyond it.
#[derive(Clone, Debug)]
struct ClashSide {
    bone: usize,
    /// Where along the bone, from its `a` end, the two bones come closest.
    along: f64,
    /// The bone met with its share of the flesh, the piece the joints hold
    /// it by (`World::measure_bone_pieces`); for a bone of a side-view
    /// fighter's arm closed in a block, the whole arm as one piece, its
    /// mass, its middle, and how hard it is to turn.
    piece: BonePiece,
    /// From the piece's centre of mass to where the bones come closest.
    lever: Vec2,
    /// The pieces of such a braced arm, which move together
    /// (`World::struck_pieces`); empty for a bone met on its own.
    braced: Vec<BonePiece>,
    /// For a side-view fighter's bone, the bone that names the piece it
    /// moves, the bone itself or the first of its braced arm: that piece
    /// shares its weight among the places it is pressed. `None` for a body
    /// seen from the front, whose pieces are pushed at each place alone.
    shares: Option<usize>,
}

/// Where along two segments, from `a0` and `b0`, they come closest, and the
/// two closest points: where they cross, the crossing.
fn closest_between(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> (f64, f64, Vec2, Vec2) {
    let pair = closest_segment_points(a0, a1, b0, b1);
    (pair.t_a, pair.t_b, pair.point_a, pair.point_b)
}

impl World {
    /// Measures, once, how thick each bone's flesh is: the middle distance
    /// from the bone to the points of the skin's outline that keep their
    /// place mostly on it, as built.
    pub(super) fn measure_bone_flesh(&mut self) {
        for (bone, mut reach) in self.held_skin_reaches().into_iter().enumerate() {
            reach.sort_by(|p, q| p.0.total_cmp(&q.0));
            self.bones[bone].flesh = reach.get(reach.len() / 2).map_or(0.0, |&(gap, _)| gap);
        }
    }

    /// Measures, once, how thick a side-view fighter's skull's flesh is each
    /// way round it (`flesh_round`): the middle distance from it to the
    /// points of the skin's outline that keep their place mostly on it and
    /// lie that way from the nearest place on it, as built. Its skull lies
    /// nearer the back of the head than the face (13 px of flesh over the
    /// crown, 19-24 behind, 33-39 toward the face and the jaw). A limb's bone lies
    /// in the middle of its flesh, about as deep all round (a fighter's
    /// forearm 13-15 px every way, its hand 12-15, its shin 16-20), and keeps
    /// one thickness: measured each way round an arm too, a cross into a low
    /// block met the blocking hand a pixel and a half later, harder than a
    /// punching arm stands, and turned back short of the arms.
    pub(super) fn measure_skull_flesh(&mut self) {
        for (bone, reach) in self.held_skin_reaches().into_iter().enumerate() {
            if self.bones[bone].part != BonePart::Skull {
                continue;
            }
            self.bones[bone].flesh_round = std::array::from_fn(|way| {
                let mut gaps: Vec<f64> = reach
                    .iter()
                    .filter(|&&(_, at)| at == way)
                    .map(|&(gap, _)| gap)
                    .collect();
                gaps.sort_by(f64::total_cmp);
                gaps.get(gaps.len() / 2).copied().unwrap_or(0.0)
            });
        }
    }

    /// For each bone, how far from it, as built, lies each point of the
    /// skin's outline that keeps its place mostly on it, and which way round
    /// it from the nearest place on it ([`FLESH_WAYS`]).
    fn held_skin_reaches(&self) -> Vec<Vec<(f64, usize)>> {
        let mut reaches: Vec<Vec<(f64, usize)>> = vec![Vec::new(); self.bones.len()];
        for outline in &self.outlines {
            for &index in outline.points() {
                let point = self.points[index];
                if point.layer != TissueLayer::Skin || point.is_ghost() {
                    continue;
                }
                let binding = self.point_holds[index].posture;
                for slot in 0..2 {
                    let bone = binding.bones[slot];
                    if binding.weights[slot] < 0.5 {
                        continue;
                    }
                    if let Some(segment) = self.bones.get(bone) {
                        let along = subtract(segment.home_b, segment.home_a);
                        let nearest = lerp(
                            segment.home_a,
                            segment.home_b,
                            segment_t(point.home, segment.home_a, segment.home_b),
                        );
                        let off = subtract(point.home, nearest);
                        let way = (libm::round(way_round(along, off)) as i64)
                            .rem_euclid(FLESH_WAYS as i64)
                            as usize;
                        reaches[bone].push((
                            distance_to_segment(point.home, segment.home_a, segment.home_b),
                            way,
                        ));
                    }
                }
            }
        }
        reaches
    }

    /// Whether bone `index` meets other bodies' limbs: a whole bone of an
    /// arm or a leg, or the skull, with flesh round it. The hand of an arm
    /// whose punch is on its way out meets only another such fist
    /// (`punching_fist`); on anything else it is left to its knuckles, which
    /// land as the bare fist does (`land_fist`). Coming back, left out, it
    /// sank into the arm that had blocked it.
    fn clashes_with_others(&self, index: usize) -> bool {
        let Some(bone) = self.bones.get(index) else {
            return false;
        };
        let limb_part = matches!(
            bone.part,
            BonePart::UpperArm
                | BonePart::Forearm
                | BonePart::Hand
                | BonePart::Thigh
                | BonePart::Shin
                | BonePart::Foot
                | BonePart::Skull
        );
        limb_part && bone.flesh > 0.0 && !bone.fractured && !bone.splinter && !bone.pinned
    }

    /// Whether bone `index` is the fist of a punch still on its way out.
    /// Two such fists meet bone on bone, as two arms do: left both to their
    /// knuckles, crossing punches' fists rode over each other, and one
    /// folded the other's hand back until the wrist tore.
    fn punching_fist(&self, index: usize) -> bool {
        self.bones.get(index).is_some_and(|bone| {
            bone.part == BonePart::Hand
                && (self
                    .punch(bone.body as usize, bone.limb)
                    .is_some_and(|punch| punch.going_out(&self.materials))
                    || self.fist_in_flight(bone.body as usize, bone.limb))
        })
    }

    /// Finds, as this substep begins, the bones of different bodies whose
    /// flesh overlaps or may overlap before it ends, and how hard each side
    /// is to move there.
    pub(super) fn measure_clashes(&mut self, h: f64) {
        self.clashes.clear();
        if self.bodies.len() < 2 {
            return;
        }
        let candidates: Vec<usize> = (0..self.bones.len())
            .filter(|&index| self.clashes_with_others(index))
            .collect();
        for (slot, &first) in candidates.iter().enumerate() {
            for &second in &candidates[slot + 1..] {
                let (a, b) = (self.bones[first], self.bones[second]);
                if a.body == b.body && !self.guard_meets_own_head(&a, &b) {
                    continue;
                }
                if self.punching_fist(first) != self.punching_fist(second) {
                    continue;
                }
                // A side-view fighter's arm held beside it is out of the
                // other's way, and two punching arms pass each other
                // (`World::parts_meet`).
                if !self.parts_meet((a.body, a.sheet), (b.body, b.sheet)) {
                    continue;
                }
                let (t_a, t_b, p_a, p_b) =
                    closest_between(a.previous_a, a.previous_b, b.previous_a, b.previous_b);
                let apart = distance(p_a, p_b);
                // Nearly crossed, the normal is the way the second body
                // stands from the first.
                let normal = if apart > EPSILON {
                    scale(subtract(p_b, p_a), 1.0 / apart)
                } else {
                    let toward = self.midline_of(b.body as usize).unwrap_or(0.0)
                        - self.midline_of(a.body as usize).unwrap_or(0.0);
                    Vec2 {
                        x: if toward >= 0.0 { 1.0 } else { -1.0 },
                        y: 0.0,
                    }
                };
                // The flesh each carries the way the other lies.
                let reach = flesh_toward(&a, normal) + flesh_toward(&b, scale(normal, -1.0));
                // How far the two could close within the substep.
                let velocity = |bone: &BoneSegment, t: f64| {
                    lerp(
                        scale(subtract(bone.a, bone.previous_a), 1.0 / h),
                        scale(subtract(bone.b, bone.previous_b), 1.0 / h),
                        t,
                    )
                };
                let closing_velocity = subtract(velocity(&a, t_a), velocity(&b, t_b));
                let margin = hypot(closing_velocity.x, closing_velocity.y) * h;
                if apart > reach + margin {
                    continue;
                }
                let (Some(side_a), Some(side_b)) = (
                    self.clash_side(first, t_a, p_a),
                    self.clash_side(second, t_b, p_b),
                ) else {
                    continue;
                };
                let turning = |side: &ClashSide| {
                    let arm = side.lever.x * normal.y - side.lever.y * normal.x;
                    1.0 / side.piece.mass + arm * arm / side.piece.inertia
                };
                let gives = [turning(&side_a), turning(&side_b)];
                self.clashes.push(Clash {
                    sides: [side_a, side_b],
                    normal,
                    reach,
                    gives,
                    impulse: 0.0,
                });
            }
        }
    }

    /// A line for each pair of limbs pressing on each other as a substep
    /// `h` long ends: the two bones, how many pieces each moved as one, how
    /// hard they pressed, in newtons, and how far apart their axes stand
    /// against the flesh both carry. For the tools that look for what flung
    /// a bone (`World::traced_clashes`).
    pub(super) fn clash_lines(&self, h: f64) -> Vec<String> {
        let newtons = self.newtons();
        self.clashes
            .iter()
            .map(|clash| {
                let name = |side: &ClashSide| {
                    let bone = self.bones[side.bone];
                    format!(
                        "bone {} ({} of body {}, {:?}, {} piece{})",
                        side.bone,
                        bone.part.name(),
                        bone.body,
                        bone.limb,
                        side.braced.len().max(1),
                        if side.braced.len() > 1 { "s" } else { "" },
                    )
                };
                let [first, second] = &clash.sides;
                let (a, b) = (self.bones[first.bone], self.bones[second.bone]);
                let apart = dot(
                    subtract(lerp(b.a, b.b, second.along), lerp(a.a, a.b, first.along)),
                    clash.normal,
                );
                format!(
                    "{} on {}: {:.0} N, {:.1} px apart against {:.1}",
                    name(&clash.sides[0]),
                    name(&clash.sides[1]),
                    clash.impulse / h / newtons,
                    apart,
                    clash.reach
                )
            })
            .collect()
    }

    /// Whether `a` and `b` are the forearm or the hand of a fighter's arm
    /// closed in a block and that fighter's own skull. A body's own parts
    /// pass over each other, but a block stands right in front of the face
    /// it covers: knocked back by a punch, the forearm meets the head and
    /// pushes it, and the head holds the forearm up. Left to pass, a blocked
    /// punch drove the light forearm back through its own face.
    fn guard_meets_own_head(&self, a: &BoneSegment, b: &BoneSegment) -> bool {
        let meets = |arm: &BoneSegment, head: &BoneSegment| {
            head.part == BonePart::Skull
                && matches!(arm.part, BonePart::Forearm | BonePart::Hand)
                && self.stands(arm.body, arm.sheet) == super::mover::Stands::Guarding
        };
        meets(a, b) || meets(b, a)
    }

    /// Whether bone `bone` is part of an arm its punch still drives, met by
    /// another body's bone `other`.
    fn punching_arm(&self, bone: usize, other: usize) -> bool {
        let (Some(bone), Some(other)) = (self.bones.get(bone), self.bones.get(other)) else {
            return false;
        };
        bone.body != other.body
            && matches!(
                bone.part,
                BonePart::UpperArm | BonePart::Forearm | BonePart::Hand
            )
            && (self
                .punch(bone.body as usize, bone.limb)
                .is_some_and(Punch::drives_arm)
                || self.striking_out(bone.body as usize, bone.limb))
    }

    /// Whether bone `index` is of a limb hanging limp (`World::let_limb_go`)
    /// or of a body knocked out (`knockout.rs`), which knocked aside blocks
    /// no punch: held at its joints with its flesh, a dropped guard's
    /// hanging hand pressed on the punching arm going over it harder than an
    /// arm gives way to, and the punch turned back short of the head.
    fn hangs_limp(&self, index: usize) -> bool {
        self.bones
            .get(index)
            .is_some_and(|bone| self.muscles_let_go(bone.body as usize, bone.limb))
    }

    /// Bone `index` with its share of the flesh, the piece the joints hold
    /// it by (`World::measure_bone_pieces`), as the substep began, pressed at
    /// `at`, `along` the bone. The joints carry the push on to the bones
    /// beyond: held on their light bones alone, a punching forearm pressed
    /// on its own, its hand left behind, pulled the wrist apart, and the
    /// bones out to the hand were pushed with it.
    fn clash_side(&self, index: usize, along: f64, at: Vec2) -> Option<ClashSide> {
        let mut piece = self.bone_pieces.get(index)?.clone()?;
        // A fighter's blocking arm is braced and meets a limb as one piece
        // (`World::struck_pieces`): pressed on its hand alone, a piece of
        // half a kilogram, the hand was knocked off the forearm and the
        // wrist slipped.
        let mut braced = self.struck_pieces(index);
        if braced.len() > 1 {
            let (mass, centre, inertia) = Self::pieces_together(&braced)?;
            piece = BonePiece {
                bone: index,
                mass,
                centre,
                inertia,
            };
        } else {
            braced.clear();
        }
        let lever = subtract(at, piece.centre);
        let fighter = self
            .bodies
            .get(self.bones[index].body as usize)
            .is_some_and(|state| state.fighter.is_some());
        let shares = fighter.then(|| braced.first().map_or(index, |piece| piece.bone));
        Some(ClashSide {
            bone: index,
            along,
            piece,
            lever,
            braced,
            shares,
        })
    }

    /// Gives one side of a clash the momentum `push` where the bones come
    /// closest: its piece, or its braced arm's pieces together.
    fn push_clash_side(&mut self, side: &ClashSide, push: Vec2, h: f64) {
        if side.braced.is_empty() {
            self.push_piece(&side.piece, side.lever, push, h);
        } else {
            let at = add(side.piece.centre, side.lever);
            self.push_together(&side.braced, at, push, h);
        }
    }

    /// Pushes apart every pair of limbs whose flesh overlaps, as Box2D's
    /// soft contact does, each pair by how fast and how far it presses from
    /// where things stand as this pass reaches it, all at once (Jacobi), so
    /// that two fighters pressing on each other are solved alike, neither
    /// first; a side-view fighter's piece shares its weight among the pairs
    /// pressing on it (the module's own words say why). An arm its punch
    /// drives, pressed harder than `CLASH_BLOCK_FORCE`, gives way
    /// (`World::block_punch`).
    pub(super) fn solve_clashes(&mut self, h: f64) {
        if self.clashes.is_empty() {
            return;
        }
        let hertz = CLASH_HERTZ.min(0.25 / h);
        let softness = Softness::new(2.0 * PI * hertz, CLASH_DAMPING_RATIO, h);
        let max_push = CLASH_PUSH_SPEED * 1000.0 / self.mm_per_px();
        // How far apart and how fast each pair stands as the pass begins,
        // and how many pressing pairs each sharing piece is in: a pair
        // presses if it has pushed already this substep, or would now.
        let mut pressed = vec![0.0_f64; self.bones.len()];
        let measured: Vec<(f64, f64)> = self
            .clashes
            .iter()
            .map(|clash| {
                let [first, second] = &clash.sides;
                let (a, b) = (self.bones[first.bone], self.bones[second.bone]);
                let gap = dot(
                    subtract(lerp(b.a, b.b, second.along), lerp(a.a, a.b, first.along)),
                    clash.normal,
                ) - clash.reach;
                let velocity = |bone: &BoneSegment, t: f64| {
                    lerp(
                        scale(subtract(bone.a, bone.previous_a), 1.0 / h),
                        scale(subtract(bone.b, bone.previous_b), 1.0 / h),
                        t,
                    )
                };
                let speed = dot(
                    subtract(velocity(&b, second.along), velocity(&a, first.along)),
                    clash.normal,
                );
                let presses = clash.impulse > 0.0
                    || softness.contact_impulse(1.0, speed, gap, 0.0, max_push, h) > 0.0;
                if presses {
                    for whole in clash.sides.iter().filter_map(|side| side.shares) {
                        pressed[whole] += 1.0;
                    }
                }
                (gap, speed)
            })
            .collect();
        let mut pushes = Vec::with_capacity(self.clashes.len());
        for (index, clash) in self.clashes.iter_mut().enumerate() {
            let (gap, speed) = measured[index];
            // Each side as heavy as its share of its piece.
            let shared =
                |side: &ClashSide| side.shares.map_or(1.0, |whole| pressed[whole].max(1.0));
            let normal_mass = 1.0
                / (shared(&clash.sides[0]) * clash.gives[0]
                    + shared(&clash.sides[1]) * clash.gives[1]);
            let impulse =
                softness.contact_impulse(normal_mass, speed, gap, clash.impulse, max_push, h);
            let total = (clash.impulse + impulse).max(0.0);
            let applied = total - clash.impulse;
            clash.impulse = total;
            if applied != 0.0 {
                pushes.push((index, scale(clash.normal, applied)));
            }
        }
        let clashes = std::mem::take(&mut self.clashes);
        for (index, push) in pushes {
            let [first, second] = &clashes[index].sides;
            self.push_clash_side(first, scale(push, -1.0), h);
            self.push_clash_side(second, push, h);
        }
        self.clashes = clashes;
        self.debug.limb_clashes = self.clashes.len() as i32;
        let newtons = self.newtons();
        let mut blocked = Vec::new();
        // A fighter's punching arm that shares its weight among the pairs
        // pressing on it is pressed by all of them together: each arm with
        // how hard in all, and the pair that pressed it hardest.
        let mut arms: Vec<((u8, Limb), f64, (usize, usize, f64))> = Vec::new();
        for clash in &self.clashes {
            let force = clash.impulse / h / newtons;
            self.debug.max_clash_force = self.debug.max_clash_force.max(force);
            let [first, second] = &clash.sides;
            for (side, other) in [(first, second.bone), (second, first.bone)] {
                let bone = side.bone;
                if side.shares.is_none() {
                    if force > CLASH_BLOCK_FORCE
                        && self.punching_arm(bone, other)
                        && !self.hangs_limp(other)
                    {
                        blocked.push((bone, other, force));
                    }
                    continue;
                }
                if force <= 0.0 || !self.punching_arm(bone, other) || self.hangs_limp(other) {
                    continue;
                }
                let arm = (self.bones[bone].body, self.bones[bone].limb);
                match arms.iter_mut().find(|(known, ..)| *known == arm) {
                    Some((_, total, hardest)) => {
                        *total += force;
                        if force > hardest.2 {
                            *hardest = (bone, other, force);
                        }
                    }
                    None => arms.push((arm, force, (bone, other, force))),
                }
            }
        }
        for (_, total, (bone, other, _)) in arms {
            if total > CLASH_BLOCK_FORCE {
                blocked.push((bone, other, total));
            }
        }
        for (bone, other, force) in blocked {
            self.block_punch(bone, other, Measure::Force, force, CLASH_BLOCK_FORCE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::SCENARIO_WIDTH;

    /// Two fighters each throwing a punch with the arm nearer the other,
    /// the left one `apart` steps after the right, played for a second and
    /// a half: how near the two near arms' bones came, as a share of the
    /// flesh both carry, and the world after.
    fn crossing_punches(apart: usize) -> (f64, World) {
        let mut world = create_fighters(Materials::default());
        let dt = world.materials.fixed_dt;
        let arms = [world.inner_arm(0), world.inner_arm(1)];
        let arm_bones: Vec<usize> = (0..world.bones.len())
            .filter(|&index| {
                let bone = world.bones[index];
                matches!(bone.part, BonePart::UpperArm | BonePart::Forearm)
                    && bone.limb == arms[bone.body as usize]
            })
            .collect();
        let mut nearest = f64::INFINITY;
        for step in 0..90 {
            let mut input = InputState::default();
            if step == 10 {
                input.punches[0] = Some((1, arms[1]));
            }
            if step == 10 + apart {
                input.punches[1] = Some((0, arms[0]));
            }
            world.step(dt, &input, SCENARIO_WIDTH);
            for &a in &arm_bones {
                for &b in &arm_bones {
                    let (first, second) = (world.bones[a], world.bones[b]);
                    if first.body != 0 || second.body != 1 {
                        continue;
                    }
                    let pair = closest_segment_points(first.a, first.b, second.a, second.b);
                    nearest = nearest.min(pair.distance / (first.flesh + second.flesh));
                }
            }
        }
        (nearest, world)
    }

    /// Two fighters punching at nearly the same moment drive their near arms
    /// into each other, guard on guard and then arm across arm: the arms
    /// press on each other's bones through their flesh and give way, so the
    /// bones keep most of their flesh between them and nothing tears. With
    /// only skin pushing on skin, the bones crossed, the skin crumpled, the
    /// muscle came off the bone, and a wrist tore.
    #[test]
    fn crossing_punches_keep_the_arms_whole() {
        for apart in [0, 4, 8] {
            let (nearest, world) = crossing_punches(apart);
            assert!(
                nearest > 0.5,
                "{apart} steps apart, the arms' bones came within {:.0}% of the flesh between them",
                nearest * 100.0
            );
            let stats = world.stats;
            assert_eq!(
                (
                    stats.broken_skin,
                    stats.broken_muscle,
                    stats.broken_bone_joints,
                    stats.fractured_bones
                ),
                (0, 0, 0, 0),
                "{apart} steps apart, the crossing punches hurt an arm"
            );
        }
    }

    /// A skull lies nearer the back of the head than the face, and meets
    /// another body's limb at the flesh lying that way (`flesh_toward`): a
    /// fist landing on the face or the jaw at that flesh, a third deeper than
    /// all round, and one at the back of the head at less, both fighters
    /// alike, the second built as the first's mirror image. A limb's bone,
    /// in the middle of its flesh, meets one at its flesh all round.
    #[test]
    fn a_skull_meets_a_fist_at_its_face() {
        let world = create_side_fighters(Materials::default());
        let mut skulls = Vec::new();
        for bone in &world.bones {
            if bone.part != BonePart::Skull {
                for way in [(1.0, 0.0), (0.0, 1.0), (-0.6, 0.8)] {
                    let way = Vec2 { x: way.0, y: way.1 };
                    assert_eq!(flesh_toward(bone, way), bone.flesh);
                }
                continue;
            }
            // The other fighter stands the way this one faces.
            let facing = if world.bodies[bone.body as usize].mirrored {
                -1.0
            } else {
                1.0
            };
            let toward = |x: f64, y: f64| {
                let length = hypot(x, y);
                flesh_toward(
                    bone,
                    Vec2 {
                        x: x * facing / length,
                        y: y / length,
                    },
                )
            };
            let (face, jaw, back) = (toward(1.0, 0.0), toward(1.0, 0.5), toward(-1.0, 0.0));
            assert!(
                face > 1.3 * bone.flesh && jaw > 1.3 * bone.flesh && back < bone.flesh,
                "body {}: the face {face:.1} px, the jaw {jaw:.1}, the back of the head {back:.1}, all round {:.1}",
                bone.body,
                bone.flesh
            );
            skulls.push([face, jaw, back]);
        }
        assert_eq!(skulls.len(), 2);
        for (first, second) in skulls[0].iter().zip(&skulls[1]) {
            assert!(
                (first - second).abs() < 1.0e-9,
                "the mirror image's skull {skulls:?}"
            );
        }
    }

    /// Crosses thrown flat out at the other fighter's jaw leave the
    /// punching hand's flesh on its bone, its muscle ties well short of
    /// tearing: met at the flesh of the back of the head, the hand's bone
    /// drove on through its knuckles' flesh after each landing, the ties
    /// slipped 27 mm of the 40 they tear at, and a few tore every minute of
    /// a bout, until the hand's bone showed through the knuckles.
    #[test]
    fn crosses_on_the_jaw_leave_the_hand_s_flesh_on_its_bone() {
        // Near enough for a cross to land (`print_fighter_punches`).
        let mut world = create_side_fighters_at(Materials::default(), 0.62);
        let dt = world.materials.fixed_dt;
        let hand = world.bodies[0].fighter.as_ref().expect("a fighter").hands[1];
        let tears_at = world.materials.bone_tie_tear_mm;
        let (mut landed, mut slipped) = (0, 0.0_f64);
        for step in 0..600 {
            let mut input = InputState::default();
            if step % 20 == 5 {
                input.moves[0] = Some((0, Move::Cross));
            }
            world.step(dt, &input, SCENARIO_WIDTH);
            landed += world
                .landed_punches()
                .iter()
                .filter(|punch| punch.by == 0 && punch.struck == Struck::Head)
                .count();
            for tie in world.bone_attachments.iter().filter(|tie| tie.bone == hand) {
                slipped = slipped.max(world.bone_tie_slip(tie) * world.mm_per_px());
            }
        }
        let torn = world
            .bone_attachments
            .iter()
            .filter(|tie| tie.bone == hand && tie.broken)
            .count();
        assert!(landed >= 10, "{landed} crosses landed on the head");
        assert!(
            torn == 0 && slipped < 0.6 * tears_at,
            "{landed} crosses on the jaw slipped the hand's ties {slipped:.0} mm and tore {torn}"
        );
    }

    /// The two fighters punching on the same step stay each other's mirror
    /// images, their arms meeting alike: neither is solved first.
    #[test]
    fn fighters_punching_together_stay_mirror_images() {
        let mut world = create_fighters(Materials::default());
        let dt = world.materials.fixed_dt;
        let [left, right] = crate::fighter_frames();
        let middle = (left.origin.x + right.origin.x) * 0.5;
        let half = world.points.len() / 2;
        let input = InputState {
            punches: [
                Some((0, world.inner_arm(0))),
                Some((1, world.inner_arm(1))),
                None,
                None,
            ],
            ..InputState::default()
        };
        world.step(dt, &input, SCENARIO_WIDTH);
        // Holding one punch a step, the second pressed on it was lost.
        assert!(
            world.punch(0, world.inner_arm(0)).is_some()
                && world.punch(1, world.inner_arm(1)).is_some(),
            "both fighters' punches pressed on one step throw"
        );
        for _ in 0..45 {
            world.step(dt, &InputState::default(), SCENARIO_WIDTH);
        }
        let apart = (0..half)
            .map(|index| {
                let (a, b) = (
                    world.points[index].position,
                    world.points[half + index].position,
                );
                (a.x + b.x - 2.0 * middle).abs().max((a.y - b.y).abs())
            })
            .fold(0.0, f64::max);
        assert!(
            apart < 1.0e-3,
            "punching together, the fighters stood {apart:.2e} px off mirror images"
        );
    }
}
