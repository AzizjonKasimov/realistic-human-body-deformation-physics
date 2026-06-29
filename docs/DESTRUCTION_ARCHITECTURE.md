# Real-Time Body Destruction Architecture

This project is aiming for physically grounded, game-feasible body destruction rather than a perfect offline biomechanics simulator. The useful direction from real-time destruction literature is a layered approximation: stable skeleton, pre-authored hard fracture, low-resolution soft tissue constraints, and high-detail rendering/particles on top.

## Product Target

- The player should see damage that appears to come from forces, stress, tearing, fracture, and material response.
- The simulation should remain deterministic enough to test with strike scenarios.
- The implementation should keep hard runtime budgets so the sandbox can run on a normal PC.

## Layer Model

1. **Skeleton and hard structure**
   - Segmented bones act as the rigid internal support.
   - A low-resolution rib cage proxy protects the torso cavity/organs and fractures as bounded rib segments, giving chest strikes anatomical structure without simulating every rib in high detail.
   - Bone joints constrain relative motion before breakage.
   - Joints should have an intermediate subluxation/dislocation state under traumatic stretch or overextension, adding slack and reduced correction before total separation.
   - First-time subluxation should leave local ligament/capsule damage in the surrounding tissue proxy, modeled as a small radius-limited load/contusion event rather than a full finite-element capsule solve.
   - Broken bones become independent fragments with caps, splinters, inertia, and fragment-tissue contact.
   - Moving broken tips and splinters should be able to puncture intact skin from inside the body under impulse, tracked separately from generic fragment-tissue rubbing.
   - Fresh fracture caps should spawn capped, bone-anchored marrow bleeding sources so exposed hard-structure injury has persistent fluid evidence tied to the moving fragment.
   - Severe fractured-rib tip motion should be able to puncture nearby organ proxies under a capped swept-tip query, tracked separately from direct sharp-tool organ penetration.
   - Severe moving fracture fragments should be able to lacerate nearby major vessel proxies under a capped swept-tip query, tracked separately from direct tool laceration.
   - Free fragments should also collide with intact skeleton segments and transfer load back into that structure so debris can jam, shove, or contribute to secondary fracture instead of passing through support bones.
   - Slow fragment-to-intact-bone overlaps and late-settle near contacts should add damping/friction/resting support so debris can remain jammed against skeleton surfaces without sliding through or requiring more global solver iterations.
   - Fragment-to-fragment contacts should damp closing velocity, tangential sliding, and angular spin so broken pieces can settle into piles without solver jitter dominating the motion.
   - Slow fragment-to-fragment overlaps should add resting-contact support so piles resist tiny sinking under sustained load without increasing the whole solver iteration count.
   - Free fragments need radius-aware environment contact with damping/friction/resting support so debris settles on floors and other static boundaries without being treated as point centers.

2. **Fracture system**
   - Runtime fracture should behave like a bond/chunk model, not arbitrary high-resolution mesh cutting.
   - In 2D, the practical equivalent is segmented bones with recursive splits, fracture depth limits, generic/rib-specific minimum fragment lengths, and fragment budgets.
   - The Rust prototype now exposes active-fragment and fragment-check budgets, broad-phase spatial filtering before fragment-tissue and fragment-pair narrow-phase checks, and low-energy fragment sleeping/wake-up.
   - Fragment-to-intact-bone checks use the same broad-phase/budget approach so richer debris interaction does not become an unbounded all-pairs problem.
   - Fragment-to-intact-bone damping, near-contact skin, and resting support should stay inside the same broad-phase/budgeted contact pass.
   - Fragment-pair damping/friction should stay coupled to overlap depth and the existing pair-check budget, not a separate unbounded stabilization pass.
   - Fragment-pair resting support should stay coupled to low relative speed and the existing pair-check budget, not a separate unbounded stabilization pass.
   - Fragment-floor support should live in the existing world-constraint stage and expose contact/resting-contact telemetry instead of adding another unbounded solver loop.
   - Recursive fracture density is controlled by maximum fracture depth, generic/rib-specific minimum fragment length, and secondary fragment strength. These controls should be changed only with strike telemetry so extra debris does not silently exceed the real-time budget.

3. **Soft tissue**
   - Skin and muscle use PBD-style points, springs, area constraints, and breakable attachments.
   - XPBD-style compliance is the right next solver upgrade when stiffness tuning starts depending too much on timestep or iteration count; the Rust prototype now has opt-in compliant spring and area projection with per-step lambda reset and focused tests.
   - Sharp cuts should propagate from existing broken skin edges into adjacent high-stress or fatigued skin springs under a per-step cap, so cut growth follows local stress concentration without becoming an unbounded flood fill.
   - Sharp skin openings should transfer into nearby exposed or loaded muscle springs under a separate cap so deep injury follows the layer coupling instead of depending only on direct blade overlap.
   - Sharp cut edges should be able to delaminate nearby skin-to-muscle attachments under local load, creating capped skin flaps and exposure without letting every blunt tear peel the whole body.
   - Muscle should expose an anisotropic damage axis: fiber-aligned spring ruptures are tracked separately from cross-fiber muscle tears, and can feed local muscle detail through an opt-in damage floor once production tuning is ready.
   - Springs should accumulate fatigue from repeated subcritical stretch/load so tissue has material memory instead of behaving as freshly intact until a single threshold is crossed.
   - Surviving springs should be able to take a bounded plastic set from sustained post-impact stretch or crush, preserving small permanent deformations without destabilizing the immediate strike response.
   - Non-cutting impacts should accumulate persistent contusion/crush state from contact load so blunt trauma has visible aftermath even when tissue does not open.
   - Contusion/crush state should feed back into the soft-tissue constraints by locally softening springs and lowering tear thresholds, with clamps so repeated trauma changes material response without destabilizing the whole body.
   - Failed muscle volume proxies should produce capped crush-rupture bleeding events, tying visible voids to persistent fluid/wound evidence instead of leaving them purely visual.
   - A low-resolution torso cavity should aggregate selected muscle area constraints into a scalar pressure/collapse state, giving deep compression a bounded internal-volume response without simulating individual organs. Non-heavy pressure/load caps should let medium blunt torso hits produce internal pressure and organ damage without opening the rupture path reserved for heavier trauma.
   - Low-resolution organ proxies can sit inside that cavity as anchored ellipses with scalar damage/penetration/rupture state, so lungs/liver/spleen can react to pressure, load, fragments, and deep sharp paths without requiring organ meshes.
   - Major bleeding should come from a small, anchored vessel graph with pressure and one-shot laceration state, not from giving every tissue tear the same fluid behavior.
   - Persistent wound leakage should drain a finite body-level blood reserve, and remaining reserve should scale later wound pressure plus passive tissue turgor/area support so severe bleeding has systemic state without simulating circulation.
   - Fatigue should feed both local tear thresholds and muscle damage detail, with conservative production defaults and stronger focused tests proving repeated-load failure.
   - Plastic deformation should be clamped against each spring's original rest shape and gated through long-settle strike telemetry so permanent set remains PC-feasible and does not silently replace tearing/fracture behavior.
   - Clotted wound sources should remain attached to their tissue or bone anchors and reopen under later local load, with capped per-step work so rebleeding behaves like material state instead of a new visual-only effect.
   - Flesh detail should mostly be driven by a low-resolution physical proxy plus rendering detail, not by simulating every visible feature directly.

4. **Coupling**
   - Skin attaches to muscle.
   - Muscle attaches to bones.
   - Skin-to-muscle attachments should fail differently when they are next to a cut edge, so flaps form from local load and cut geometry rather than random attachment breakage.
   - Wounds attach to moving tissue or bone features so fluid emission, clotting, and later rebleeding follow the damaged body.
- Torso cavity pressure should push/load nearby muscle points and open at most capped internal rupture wounds under heavy localized trauma, while medium blunt, limb, and hip strikes should not masquerade as torso-cavity rupture.
   - Organ rupture should require severe torso-cavity involvement, severe fractured-rib tip involvement, fragment involvement, or an explicit sharp/deep penetration gate, and should reuse the persistent wound/fluid system instead of creating a separate hidden damage channel.
   - Major vessel paths should follow nearby muscle anchors and open high-pressure muscle-layer wounds when deep sharp/heavy contact crosses them.
   - Fragment-driven vessel laceration should reuse the same pressure wound path as direct vessel cuts while preserving cause-specific telemetry.
   - Body-level blood reserve should remain a scalar pressure/turgor feedback, not a per-particle circulation network, until the game needs richer systemic injury.
   - Broken fragments continue interacting with nearby tissue instead of becoming purely visual debris.

5. **Visual detail**
   - Rendering should expose the simulation state: failed triangles, torn springs, exposed muscle, contusion, lacerated vessels, bone caps, fluid, wound pressure, and clotting.
   - Particles and wound edges can add detail, but they should be spawned from physical events.
   - Wound-edge and exposed-muscle detail should be derived from broken springs, failed muscle triangles, point exposure/load, and wound pressure/clotting so visual fidelity follows the physical state.
   - Blood pools and stains should be persistent simulation objects deposited by fluid contact/settle events, not decals placed independently of the bleeding simulation.
   - Damage rendering changes should leave deterministic visual evidence: replayed strike captures and primitive counts make wound, exposed-muscle, fluid, and fracture regressions inspectable alongside the numeric strike telemetry.

## Runtime Budget Priorities

- Cap active free fragments per body.
- Use broad-phase spatial filtering before fragment-pair and fragment-tissue narrow-phase checks.
- Use broad-phase spatial filtering before fragment-to-intact-bone checks.
- Cap fragment-pair and fragment-tissue checks per frame after broad-phase filtering.
- Put low-energy free fragments to sleep so settled debris remains visible without consuming active destruction work.
- Wake sleeping fragments when direct contact applies meaningful load.
- Cap wound and fluid sources with priority replacement/ring-buffer replacement.
- Cap major vessel lacerations per step so vascular injury stays a bounded anatomical query instead of an all-pairs fluid source pass.
- Gate finite blood loss, remaining reserve, and turgor scale in long strike playback so high-pressure bleeding cannot regress into an infinite or purely cosmetic emitter.
- Cap blood stains/pools with merge and replacement behavior so persistent bleeding evidence does not grow without bound.
- Surface budget checks, skips, replacements, sleep/wake events, and solver iterations in debug/strike telemetry.
- Keep a long-settle strike scenario in the verifier so sleep thresholds are tuned against real debris behavior, not only unit tests.
- Gate joint subluxation counts and severity in blunt strike playback so partial dislocation remains covered independently of complete joint breakage.
- Gate joint ligament/capsule damage counts separately from joint subluxation so local soft-tissue injury cannot regress into a silent skeletal-only state.
- Gate final free-fragment count in the long-settle strike so fracture tuning cannot regress back to shallow breakage while still passing contact-only checks.
- Gate rib fracture counts separately from generic bone fracture counts so chest-structure tuning can change without hiding limb/skull fracture regressions or letting non-torso strikes break ribs.
- Gate fracture marrow-source counts in heavy torso strikes so fracture bleeding remains tied to broken caps and does not collapse back to one-shot particles.
- Gate fragment-bone contact in the long-settle strike so skeletal debris coupling remains covered by deterministic playback.
- Gate fragment-bone damping and resting contacts in the long-settle strike so intact-body support remains covered by deterministic playback.
- Gate fragment-pair damping events in the long-settle strike so pile stabilization remains covered by deterministic playback.
- Gate fragment-pair resting contacts in the long-settle strike so sustained debris support remains covered by deterministic playback.
- Gate fragment-floor contacts and resting contacts in the long-settle strike so environment support remains covered by deterministic playback.
- Gate blood stain deposits in the long-settle strike so fluid pooling remains covered by deterministic playback.
- Gate inside-out skin punctures from bone fragments in heavy torso strikes so splinter exit behavior remains covered separately from generic fragment tissue tears.
- Gate sharp cut propagation in sharp strike scenarios so stress-driven tear growth remains covered independently of blunt/heavy tearing.
- Gate skin-to-muscle cut transfer in sharp strike scenarios so deep cuts remain covered independently of skin-only propagation.
- Gate sharp skin-flap delamination in sharp strike scenarios so cut-assisted layer peeling remains covered without changing blunt/heavy fracture tuning.
- Gate fiber-aligned muscle tear counts separately from total muscle tears so anisotropic muscle damage cannot collapse back into a generic tear count.
- Gate contusion events and tissue-softening telemetry in strike playback so blunt/crush trauma remains covered independently of open tearing.
- Gate muscle crush-rupture counts in heavy strike playback so failed muscle voids remain coupled to physical bleeding.
- Gate torso cavity pressure, collapse, and capped rupture counts in strike playback so internal-volume response remains covered without becoming a full organ/circulation simulation.
- Gate medium blunt torso pressure separately from heavy torso rupture so cavity pressure can exist below the internal tearing threshold.
- Gate organ damage, direct penetration, and capped organ rupture counts in strike playback so internal injury can progress while representative blunt, limb, and non-penetrating strikes do not become indiscriminate organ rupture.
- Gate severe rib-organ puncture counts separately from direct organ penetrations so heavy chest trauma can produce rib-driven internal injury while medium torso, shoulder, limb, and sharp direct-cut scenarios keep distinct causal evidence.
- Gate major vessel lacerations in sharp/heavy strike playback and gate zero lacerations in representative blunt strikes so pressure bleeding remains spatial and tool-dependent.
- Gate fragment-driven vessel lacerations separately from direct vessel lacerations so severe debris can cause vascular injury without making medium blunt, limb, or sharp direct-cut scenarios lose causal clarity.
- Gate spring-fatigue events and fatigue maxima in strike playback so repeated subcritical tissue damage remains covered independently of single-impact tearing.
- Gate plastic deformation events and plasticity maxima in strike playback so sustained post-impact tissue set remains covered without disrupting primary strike outcomes.
- Raise XPBD tissue spring/area compliance only in measured tuning passes, keeping the neutral default path behavior covered while tests prove compliant residual error when the knobs are enabled.
- Surface wound reopen counts in strike telemetry so later multi-strike scenarios can gate clot disruption and rebleeding without adding unbounded wound work.
- Prefer several small solver substeps or staged solves only where they visibly improve stiffness or contact stability.

## Reference Trail

- NVIDIA Blast SDK: chunk/bond-based destruction runtime and authoring model.  
  https://docs.omniverse.nvidia.com/kit/docs/blast-sdk/latest/docs/api/introduction.html
- Unreal Chaos Destruction: geometry collections, clustering, strain, fields, and caching.  
  https://dev.epicgames.com/documentation/unreal-engine/chaos-destruction-overview
- Position Based Dynamics, Mueller et al.: the foundation for stable real-time constraint projection.  
  https://matthias-research.github.io/pages/publications/posBasedDyn.pdf
- XPBD, Macklin et al.: compliant constraints with stiffness less tied to timestep and iteration count.  
  https://matthias-research.github.io/pages/publications/XPBD.pdf
- Small Steps in Physics Simulation, Macklin et al.: smaller substeps can improve stiffness and stability.  
  https://mmacklin.com/smallsteps.pdf
- PhysX Articulations: stable reduced-coordinate articulated body chains for ragdoll-like skeletons.  
  https://nvidia-omniverse.github.io/PhysX/physx/5.1.3/docs/Articulations.html
- Real-Time Simulation of Deformation and Fracture of Stiff Materials, Mueller et al.: event-driven deformation/fracture for stiff materials.  
  https://matthias-research.github.io/pages/publications/fracture.pdf
- Corotated Finite Elements Made Fast and Stable, Georgii and Westermann: useful reference for future soft-tissue upgrades.  
  https://diglib.eg.org/bitstream/handle/10.2312/PE.vriphys.vriphys08.011-019/011-019.pdf
