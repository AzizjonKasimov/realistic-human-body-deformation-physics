//! A journal of what happens to the body: each injury the rules decide, with
//! the step and the phase of the step that decided it, what it happened to,
//! where, what did it, and the measure that crossed the rule's limit. The
//! inspection tools keep one (`strike_scenarios --journal`) to say what a
//! blow did and why without reading the code; the app never does. It is kept
//! only when asked for ([`World::keep_journal`]), and keeping it changes
//! nothing the simulation does.

use super::{Limb, ToolMode, Vec2, World};

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EventKind {
    /// A fiber of skin or muscle tore, stretched or loaded past its limit.
    SkinTorn,
    MuscleTorn,
    /// A blade parted a fiber.
    SkinCut,
    MuscleCut,
    /// A heavy head split a fiber it crushed.
    SkinCrushed,
    MuscleCrushed,
    /// A skin wound's tear ran on into the skin beside it.
    TearRanOn,
    /// A cut or tear opened: its points split apart.
    CutOpened,
    /// The fat under the skin tore, and the skin came loose from the muscle.
    SkinLoosened,
    /// The skin beside a cut came away from the muscle as a flap.
    SkinFlap,
    /// Muscle tore off its bone, or let go of a broken piece it held.
    MuscleOffBone,
    /// A patch of muscle was crushed to pulp.
    MusclePulped,
    /// Tissue tired, or took a set, under load.
    Tired,
    Set,
    /// Flesh was bruised.
    Bruised,
    /// Blood from a fresh injury stayed under unbroken skin.
    BledUnderSkin,
    /// A joint slipped out of place, or tore apart.
    JointSlipped,
    JointTorn,
    /// A bone broke: closed, or out through the skin.
    BoneBroken,
    BoneBrokenOpen,
    /// A break chipped a splinter, or broke a butterfly wedge out.
    ChipBrokenOff,
    WedgeBrokenOff,
    /// A limb's muscles let go of it, a bone in it broken.
    LimbWentLimp,
    /// A broken piece of bone pierced the skin from inside, or tore flesh.
    SkinPiercedByBone,
    FleshTornByBone,
    /// A major artery was cut.
    ArteryCut,
    /// An organ was hurt, pierced, or ruptured.
    OrganHurt,
    OrganPierced,
    OrganRuptured,
    /// The chest or belly burst under pressure.
    CavityRuptured,
    /// A wound began to bleed, or a clotted one bled again.
    WoundOpened,
    WoundReopened,
    /// A punching arm met another body's limb on its way out and the punch
    /// gave way (`clash.rs`).
    PunchTurnedBack,
}

impl EventKind {
    /// What happened, in a few words.
    pub fn describe(self) -> &'static str {
        match self {
            EventKind::SkinTorn => "skin torn",
            EventKind::MuscleTorn => "muscle torn",
            EventKind::SkinCut => "skin cut",
            EventKind::MuscleCut => "muscle cut",
            EventKind::SkinCrushed => "skin split by crushing",
            EventKind::MuscleCrushed => "muscle split by crushing",
            EventKind::TearRanOn => "skin tear ran on",
            EventKind::CutOpened => "cut opened",
            EventKind::SkinLoosened => "skin came loose from the muscle",
            EventKind::SkinFlap => "skin flap came away",
            EventKind::MuscleOffBone => "muscle let go of bone",
            EventKind::MusclePulped => "muscle crushed to pulp",
            EventKind::Tired => "tissue tired",
            EventKind::Set => "tissue took a set",
            EventKind::Bruised => "bruised",
            EventKind::BledUnderSkin => "bled under the skin",
            EventKind::JointSlipped => "joint slipped out of place",
            EventKind::JointTorn => "joint torn apart",
            EventKind::BoneBroken => "bone broken",
            EventKind::BoneBrokenOpen => "bone broken open",
            EventKind::ChipBrokenOff => "splinter chipped off",
            EventKind::WedgeBrokenOff => "butterfly wedge broke out",
            EventKind::LimbWentLimp => "limb went limp",
            EventKind::SkinPiercedByBone => "skin pierced from inside",
            EventKind::FleshTornByBone => "flesh torn by a broken bone",
            EventKind::ArteryCut => "artery cut",
            EventKind::OrganHurt => "organ hurt",
            EventKind::OrganPierced => "organ pierced",
            EventKind::OrganRuptured => "organ ruptured",
            EventKind::CavityRuptured => "chest or belly burst",
            EventKind::WoundOpened => "wound opened",
            EventKind::WoundReopened => "wound reopened",
            EventKind::PunchTurnedBack => "punch turned back",
        }
    }

    /// Whether it is an injury worth telling one by one, rather than only
    /// counted: tiring, setting, and bruising happen to hundreds of points.
    pub fn is_minor(self) -> bool {
        matches!(
            self,
            EventKind::Tired | EventKind::Set | EventKind::Bruised | EventKind::BledUnderSkin
        )
    }
}

/// What an event happened to, by its index in the world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Subject {
    #[default]
    Nothing,
    Point(usize),
    Spring(usize),
    /// A tie of skin to the muscle under it, the fat (`World::attachments`).
    SkinTie(usize),
    /// A tie of muscle to a bone (`World::bone_attachments`).
    BoneTie(usize),
    Triangle(usize),
    Bone(usize),
    Joint(usize),
    Vessel(usize),
    Organ(usize),
    Cavity(usize),
    Wound(usize),
    Limb(Limb),
}

/// What did it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Actor {
    /// Nothing outside the tissue itself: what it carried, its own stretch.
    #[default]
    Tissue,
    /// The tool in hand.
    Tool,
    /// A bone, by index: a broken piece moving through flesh, or a break.
    Bone(usize),
    /// The muscle pressed onto a bone.
    Flesh,
    /// Pressure in the chest or belly.
    Pressure,
}

/// The measure a rule judged by, which says how to read `Event::measure`
/// and `Event::limit`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Measure {
    #[default]
    None,
    /// How far a fiber is stretched, as a ratio of its length at rest in
    /// the body.
    Stretch,
    /// A load in the engine's units of impulse, against a limit.
    Load,
    /// A load as a share of a bone's strength.
    Strength,
    /// How far the skin slid over the muscle, in thicknesses of fat.
    Shear,
    /// How far muscle was pulled off its place on a bone, in millimetres.
    Slip,
    /// How far a joint bent past its range, in radians.
    Overbend,
    /// A pressure, as the engine scales it.
    Pressure,
    /// Damage built up, against the damage that ruptures.
    Damage,
    /// How many points or drops.
    Count,
    /// How fast two things met, in metres a second.
    Speed,
    /// How hard one pressed on another, in newtons.
    Force,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    /// The step it happened on, counted from when the journal began.
    pub step: u32,
    /// The phase of the step that decided it (`World::timed`).
    pub phase: &'static str,
    pub kind: EventKind,
    pub subject: Subject,
    pub at: Vec2,
    pub by: Actor,
    pub measure: Measure,
    pub value: f64,
    pub limit: f64,
}

/// What the tool was doing on a step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepNote {
    pub tool: ToolMode,
    pub tool_at: Vec2,
    /// How fast the hand drove the tool, in pixels per second.
    pub hand_speed: f64,
    pub down: bool,
    /// What the tool touched.
    pub tissue_contacts: i32,
    pub bone_contacts: i32,
}

#[derive(Clone, Debug, Default)]
pub struct Journal {
    pub events: Vec<Event>,
    /// One note for each step since the journal began.
    pub steps: Vec<StepNote>,
    phase: &'static str,
    /// What is acting while a rule runs that does not name it.
    actor: Actor,
}

impl Journal {
    /// The step being taken, counted from when the journal began.
    fn step(&self) -> u32 {
        self.steps.len().saturating_sub(1) as u32
    }

    /// Notes `event` in `journal`, if one is kept, on the step and in the
    /// phase being run. A free function, so a rule can note what it does
    /// while it holds other parts of the world.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn note(
        journal: &mut Option<Journal>,
        kind: EventKind,
        subject: Subject,
        at: Vec2,
        by: Option<Actor>,
        measure: Measure,
        value: f64,
        limit: f64,
    ) {
        if let Some(journal) = journal {
            journal.events.push(Event {
                step: journal.step(),
                phase: journal.phase,
                kind,
                subject,
                at,
                by: by.unwrap_or(journal.actor),
                measure,
                value,
                limit,
            });
        }
    }
}

impl World {
    /// Keeps a journal of what happens to the body from now on
    /// ([`World::journal`]), starting afresh.
    pub fn keep_journal(&mut self) {
        self.journal = Some(Journal::default());
    }

    /// What has happened to the body since [`World::keep_journal`], if a
    /// journal is kept.
    pub fn journal(&self) -> Option<&Journal> {
        self.journal.as_ref()
    }

    /// Notes what a rule did, by `by`, or by whatever is acting now
    /// ([`World::acting`]) when `by` is `None`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn note(
        &mut self,
        kind: EventKind,
        subject: Subject,
        at: Vec2,
        by: Option<Actor>,
        measure: Measure,
        value: f64,
        limit: f64,
    ) {
        Journal::note(
            &mut self.journal,
            kind,
            subject,
            at,
            by,
            measure,
            value,
            limit,
        );
    }

    /// Names what acts on the body from now until the next call, for the
    /// rules it sets off that do not name it; returns what acted before.
    pub(super) fn acting(&mut self, actor: Actor) -> Actor {
        match &mut self.journal {
            Some(journal) => std::mem::replace(&mut journal.actor, actor),
            None => Actor::Tissue,
        }
    }

    /// Starts a step in the journal: notes what the tool is doing.
    pub(super) fn begin_journal_step(&mut self) {
        let debug = self.debug;
        if let Some(journal) = &mut self.journal {
            journal.steps.push(StepNote {
                tool: debug.tool,
                tool_at: debug.striker_position,
                hand_speed: debug.striker_speed,
                down: debug.down,
                tissue_contacts: 0,
                bone_contacts: 0,
            });
            journal.actor = Actor::Tissue;
        }
    }

    /// Ends a step in the journal: notes what the tool touched.
    pub(super) fn end_journal_step(&mut self) {
        let debug = self.debug;
        if let Some(note) = self
            .journal
            .as_mut()
            .and_then(|journal| journal.steps.last_mut())
        {
            note.tissue_contacts = debug.tissue_contacts;
            note.bone_contacts = debug.bone_contacts;
        }
    }

    /// Names the phase of the step being run, for the events it notes.
    pub(super) fn journal_phase(&mut self, phase: &'static str) {
        if let Some(journal) = &mut self.journal {
            journal.phase = phase;
        }
    }
}
