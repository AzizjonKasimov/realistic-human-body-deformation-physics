# Realistic Physics

A no-engine 2D physics sandbox rewritten in Rust. The simulation is kept separate from the renderer so the physics model can be tested, tuned, and eventually shipped on more than one desktop platform.

See [docs/VISION.md](docs/VISION.md) for the original project description and agreed direction.
See [docs/DESTRUCTION_ARCHITECTURE.md](docs/DESTRUCTION_ARCHITECTURE.md) for the research-backed simulation architecture the Rust prototype is moving toward.

## Demo Video

[![Watch the Realistic Physics demo video on YouTube](https://img.youtube.com/vi/hCXVPSU6etE/hqdefault.jpg)](https://www.youtube.com/watch?v=hCXVPSU6etE)

## Current Rust Milestone

- Rust Cargo project with a reusable `realistic_physics` simulation library.
- Cross-platform `macroquad` desktop app for windowing, input, and rendering.
- Verlet/PBD-style soft body points, springs, area constraints, and attachments.
- XPBD-style compliant spring and area-constraint projection is available behind material knobs, with focused tests covering compliant residual stretch/area behavior before production tuning raises those defaults.
- Separate skin and muscle meshes generated from nested body masks so muscle stays inside the skin silhouette.
- Dynamic segmented bones generated from the same body proportions, including a low-resolution rib-cage proxy, attached to nearby muscle points, and connected by breakable bone joints.
- Bone joints can subluxate under traumatic stretch/overextension before full breakage, adding limited slack, weaker correction, and first-time local ligament/capsule tissue damage so dislocation exists between intact articulation and total separation.
- Post-fracture joint limits let broken or remapped limb joints sag and twist with slack instead of snapping rigidly or separating without bounds.
- Mouse-controlled tool head with blunt, sharp, and heavy modes, a spring-driven handle/target, impact direction, mode-specific handling, and distinct tool silhouettes.
- Stress-based tearing from overstretched or high-impulse springs.
- Sharp cuts propagate from existing wound edges into adjacent stressed or fatigued skin springs under a per-step cap, so cuts can grow along local stress instead of appearing only as isolated threshold breaks.
- Sharp skin openings can transfer into nearby exposed or loaded muscle springs under a separate cap, so deep cuts follow the layered anatomy instead of requiring only direct blade overlap.
- Sharp cut edges can delaminate nearby skin-to-muscle attachments under local load, creating capped skin-flap peeling and more physically driven exposure of the muscle layer.
- Fiber-aligned muscle springs now report separate rupture telemetry from cross-fiber muscle tears, with an opt-in damage-detail floor for later anisotropic tissue tuning.
- Soft-tissue springs accumulate persistent fatigue from repeated subcritical stretch/load, which lowers local tear thresholds and feeds muscle damage detail before full rupture.
- Damaged tissue can take a bounded permanent set during post-impact settling, so surviving springs keep small plastic stretch/crush deformation instead of always snapping back to their original rest shape; the default plastic rate is tuned through long-settle strike telemetry.
- Blunt and heavy contact now leaves persistent tissue contusion/crush state on impacted points, and contused springs locally soften and tear at lower load so repeated trauma changes material behavior instead of only tinting the surface.
- Failed muscle triangles can rupture into capped crush-bleeding sources, so heavy blunt/internal damage produces persistent fluid and wound evidence instead of only visual void shading.
- A low-resolution torso cavity pressure proxy groups internal muscle area constraints, builds bounded pressure/collapse state under deep compression, pushes back on surrounding tissue, and uses separate non-heavy pressure/load caps so medium blunt hits can bruise internal tissue without opening the capped internal rupture path reserved for heavier trauma.
- Anchored low-resolution organ proxies for lungs, liver, and spleen accumulate pressure/load/fragment damage and can rupture into capped internal bleeding when severe torso-cavity trauma supports it, when a sharp/deep striker explicitly penetrates an organ proxy, or when severe fractured-rib tip motion punctures a nearby organ proxy.
- Low-resolution major vessel paths follow nearby muscle anchors and can lacerate under deep sharp or heavy contact, feeding high-pressure wound sources rather than treating every wound as the same bleed.
- Exposed muscle is a second mesh coupled to skin through breakable attachments.
- Bones fracture at loaded contact points into recursive fragments and splinters, release nearby muscle-to-bone anchors, keep rotational inertia, and continue damaging tissue from broken ends.
- Moving splinter or broken-bone tips can puncture intact skin from inside the body under impulse, and severe fractured-rib tips can puncture organ proxies, with capped telemetry separate from generic fragment-tissue tearing.
- Severe moving fracture fragments can lacerate nearby major vessel paths through a capped swept-tip query, opening the same pressure-wound system while keeping fragment-driven vascular injury separate from direct tool cuts.
- Fresh fracture caps create bone-anchored marrow bleeding sources that follow the broken fragment and leak through the persistent wound system instead of being only a one-frame particle burst.
- Runtime budgets, broad-phase spatial filtering, and low-energy fragment sleeping cap active fragment work, fragment-bone checks, fragment-pair checks, fragment-tissue checks, vessel lacerations, wound sources, and fluid particles so heavier destruction remains PC-real-time.
- Broken fragments now collide with nearby intact bones, push against them, and transfer load back into the skeleton instead of only interacting with tissue and other fragments.
- Slow fragment-to-intact-bone overlaps and late-settle near contacts add damping, friction, and resting support so debris can jam against remaining skeleton instead of sliding through or rattling endlessly.
- Fragment-pair contacts damp closing velocity, tangential sliding, and angular jitter so piles of debris settle instead of endlessly rattling.
- Slow fragment-pair overlaps receive extra resting-contact support so settled debris resists tiny sinking/rattling under sustained load without raising global solver iterations.
- Free fragments use radius-aware floor contacts with damping, friction, angular drag, and resting-contact telemetry so debris can settle against the environment instead of being only center-clamped.
- Recursive fracture density is tuned through explicit material controls for maximum fracture depth, generic/rib-specific minimum fragment length, and secondary fragment strength, with deterministic strike gates covering long-settle debris.
- Fluid particles emit from tissue tears, attachment releases, and fractures, then fall, settle, and fade through the same simulation step.
- Settled fluid particles now deposit capped, mergeable blood stains/pools on the environment so bleeding leaves persistent physical evidence instead of disappearing as particle fade only.
- Persistent wound sources anchor to nearby moving tissue or bone features, leak or briefly spray based on layer, depth, and pressure, clot down over time, and can reopen when later local load disturbs the clot.
- Wound leakage drains a finite normalized blood reserve, and remaining reserve feeds back into wound pressure/leak strength and passive tissue turgor so long severe bleeding does not behave like an infinite source or fully supported tissue.
- Wound edges and exposed muscle render extra detail from actual broken springs, failed muscle triangles, point exposure, load, pressure, and clotting rather than a separate visual-only damage layer.
- Visual damage diagnostics replay deterministic sharp and heavy strikes, then write a damage-focused SVG and primitive-count CSV so wound-edge, exposed-muscle, lacerated-vessel, fluid, and fracture rendering can be inspected without launching the app.
- Anatomy view for inspecting muscle, major vessels, and bones without waiting for skin exposure.
- Rust diagnostics, strike scenario playback, and simulation tests.
- Checked-in front-facing adult pixel silhouette mask under `docs/reference/pixel_human_silhouettes/` for tuning and generating the body shape.

## Realism Target

The project target is to get as close as practical to real-life body destruction physics. Graphic injury detail, visible gore, exposed anatomy, blood, tearing, fracture, and tissue deformation are intentional baseline behavior when they come from the simulation. Future renderer and simulation work should assume a darker, more physically explicit direction by default rather than asking whether gore should be reduced.

## Install Rust

From PowerShell:

```powershell
winget install Rustlang.Rustup
```

Restart PowerShell after installation so `cargo.exe` is on `PATH`, then confirm:

```powershell
cargo --version
```

## Run On Windows

Build the Rust app and copy the release executable to the repository root:

```powershell
.\tools\build_app.ps1
.\realistic_physics.exe
```

If the app is already open and the build cannot replace `realistic_physics.exe`, close it or run:

```powershell
.\tools\build_app.ps1 -StopRunningApp
```

You can also run directly through Cargo:

```powershell
cargo run --release --bin realistic_physics
```

## Run On macOS

The Rust app uses `macroquad`, so it is designed to build on macOS as well as Windows. macOS has not been verified from this Windows workspace yet.

On a Mac with Rust installed:

```bash
cargo run --release --bin realistic_physics
```

## Controls

- Left-drag to swing the selected tool into the body. Damage comes from tool shape, overlap, swing speed, and selected striker mass.
- `B`, `S`, and `H` select blunt, sharp, and heavy tool modes.
- `D` toggles the contact debug overlay.
- `Tab` toggles anatomy view, where skin is wireframe and muscle/bones are visible.
- `R` resets the body.
- `Space` pauses or resumes.
- `1`, `2`, and `4` change striker mass.

## Verify

From the repository root in PowerShell:

```powershell
.\tools\verify.ps1
```

The Rust verifier runs formatting checks, simulation tests, deterministic strike playback, anatomy diagnostics, and visual damage diagnostics. To also build the app executable:

```powershell
.\tools\verify.ps1 -BuildApp
```

## Strike Scenarios

`.\tools\verify.ps1` builds and runs deterministic strike playback across representative torso, shoulder, arm, hip, and leg strikes with blunt, sharp, and heavy tools. The scenario target writes frame-by-frame contact telemetry to:

```text
output\strike_scenarios.csv
```

It also writes a compact per-scenario tuning summary to:

```text
output\strike_summary.csv
```

It also writes a warning-only tuning report that compares each scenario against expected damage bands:

```text
output\strike_tuning_report.txt
```

The CSV outputs include region, intent, tool mode, striker speed, impact, contact counts, contact depth, tissue/bone loads, sharp cut propagation counts, skin-to-muscle cut transfer counts, sharp skin-flap delamination counts, fiber-aligned muscle tear counts, muscle crush-rupture counts, torso cavity pressure/collapse/rupture counts, organ damage/direct-penetration/rib-puncture/rupture counts, direct and fragment-driven major vessel laceration counts, soft-tissue contusion counts, local tissue-softening maxima, spring-fatigue events and local fatigue maxima, plastic deformation events and local plasticity maxima, joint subluxation/breakage, ligament/capsule damage events, fracture events, rib-fracture counts, fracture marrow-source counts, post-fracture joint limit corrections, wound counts, wound reopen counts, wound pressure/clotting, blood loss, final blood reserve, final blood-turgor scale, blood stain/pool deposits, broken-end tissue contacts, inside-out skin puncture counts, fragment-bone contacts/damping/resting support, fragment-pair contacts/damping/resting support, fragment-floor contacts/resting support, overlap depth, fragment angular speed, free/spinning/sleeping fragment counts, runtime budget checks/skips/replacements, fluid emission, final fragment counts, and accumulated damage stats. Blunt torso, shoulder, and leg scenarios gate zero major vessel lacerations, torso/shoulder/hip scenarios gate bounded rib fractures, arm/leg scenarios gate zero rib fractures where appropriate, torso scenarios gate cavity pressure and heavy-torso cavity rupture while hip/leg gates prevent false torso-cavity ruptures, heavy torso scenarios gate organ damage, capped organ ruptures, severe rib-organ punctures, and fragment-driven vessel lacerations without direct organ penetrations, sharp torso and rebleed scenarios gate direct organ penetrations plus capped organ rupture while arm/leg/blunt cases gate zero direct organ penetrations and zero rib-organ punctures where appropriate, the sharp torso, arm, and rebleed scenarios gate propagated tear counts, deep muscle cut transfers, skin-flap delamination, fiber-aligned muscle tears, and major vessel lacerations, subluxating torso/shoulder/arm/hip scenarios gate local ligament/capsule damage counts separately from complete joint breakage, heavy torso and hip scenarios gate major vessel lacerations plus fragment-vessel lacerations, severe long/rebleed scenarios gate finite blood-loss bands, remaining reserve, and lowered turgor scale, the heavy torso scenarios gate muscle crush ruptures, fiber-aligned muscle tears, bone-fragment skin punctures, and fracture marrow sources, and the `torso_heavy_fragment_settle` scenario deliberately waits after a heavy fracture so final free-fragment count, fragment-bone contact/resting support, fragment-pair damping/resting support, fragment-floor support, blood stain deposits, contusion counts, spring fatigue, plastic set, tissue softening, cavity pressure/rupture behavior, organ damage/direct-penetration/rib-puncture/rupture behavior, rib-fracture behavior, fragment-driven vessel behavior, joint ligament/capsule damage behavior, fragment sleep bands, and deeper recursive fracture behavior are covered by the verifier.

## Anatomy Diagnostics

Use this whenever changing body generation, anatomy layers, bones, constraints, or rendering assumptions:

```powershell
.\tools\verify.ps1
```

Open `output\anatomy_debug.svg` to inspect the generated body without launching the app. Skin is translucent, muscle is red, major vessels are dark red, bones are pale, ribs are slightly warmer, muscle-to-bone attachments are blue, bone joints are yellow, and bone sample markers turn red if they fall outside the skin mesh. The diagnostic exits nonzero if sampled bone centerlines are outside skin.

## Visual Damage Diagnostics

Use this whenever changing damage rendering, wound detail, fluid appearance, fracture display, or visual assumptions:

```powershell
.\tools\verify.ps1
```

Open `output\damage_visual_debug.svg` to inspect two deterministic damage captures side by side: a torso sharp cut and a long-settle heavy torso fracture. The diagnostic also writes primitive counts to:

```text
output\damage_visual_summary.csv
```

The visual diagnostic exits nonzero if the captures no longer include expected wound-edge lines, exposed-muscle fiber detail, failed-muscle voids, contusion discoloration, direct/fragment lacerated vessel evidence, wound sources, visible fluid particles, finite blood-loss, turgor, cavity-pressure, organ-penetration/rib-puncture/injury metrics, blood stain pools, fractured bones, rib-fracture evidence, fracture caps, or overall damage primitives.

## Development Notes

- `Cargo.toml` defines the Rust library, app, diagnostics, and strike scenario binaries.
- `src/simulation.rs` contains the physics data model, body generation, integration, constraints, tearing, bone fracture, major vessels, wounds, and fluid particles.
- `src/bin/realistic_physics.rs` owns the `macroquad` app shell, input, timing, and rendering.
- `src/bin/anatomy_diagnostics.rs` writes a deterministic SVG anatomy snapshot and reports geometry validation metrics.
- `src/bin/strike_scenarios.rs` writes deterministic strike telemetry and tuning summaries.
- `src/bin/visual_damage_diagnostics.rs` writes deterministic SVG damage captures and visual primitive metrics.
- `tests/simulation_tests.rs` contains focused Rust simulation checks.
- `docs/reference/pixel_human_silhouettes/front_adult_silhouette_41x96.mask` is the front-facing-only adult pixel mask sampled by the body generator. The Commons SVG remains an auxiliary front-view proportion reference.

The next Rust simulation milestones are:

1. Expand the Rust test matrix to cover more strike and fracture edge cases.
2. Raise tissue spring/area compliance from the current neutral defaults in measured XPBD tuning passes, using strike telemetry to keep fracture, cavity, vessel, and debris behavior separated.
3. Tighten strike tuning bands once material behavior has settled enough for intentional regression gates.
4. Keep increasing fracture density in small measured steps, using the long-settle telemetry to catch fragment sleep, budget, and stability regressions.
5. Verify the `macroquad` app on macOS and document any platform-specific packaging steps.
6. Add a renderer screenshot or baseline-image comparison path once the headless SVG damage diagnostic has stabilized.

## Toolchain

The primary build now uses Rust and Cargo. The app frontend uses `macroquad` for a cross-platform window, input, and 2D drawing path while the simulation stays in a reusable Rust library.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE).
