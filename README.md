# Realistic Physics

A no-engine 2D physics sandbox rewritten in Rust. The simulation is kept separate from the renderer so the physics model can be tested, tuned, and shipped on more than one platform, including the web.

**[Play it in your browser](https://azizjonkasimov.github.io/realistic-human-body-deformation-physics/)**: no install, and the simulation runs entirely on your own device.

See [docs/VISION.md](docs/VISION.md) for the original project description and agreed direction.
See [docs/DESTRUCTION_ARCHITECTURE.md](docs/DESTRUCTION_ARCHITECTURE.md) for the research-backed simulation architecture the Rust prototype is moving toward.

## Demo Video

[![Watch the Realistic Physics demo video on YouTube](https://img.youtube.com/vi/hCXVPSU6etE/hqdefault.jpg)](https://www.youtube.com/watch?v=hCXVPSU6etE)

## Current Rust Milestone

- Rust Cargo project with a reusable `realistic_physics` simulation library.
- Cross-platform `macroquad` app for windowing, input, and rendering, built as a desktop app and as a WebAssembly browser version that GitHub Pages hosts.
- On-screen control buttons that work with mouse and touch, so the browser version is playable on phones and tablets.
- Verlet/PBD-style soft body points, springs, area constraints, and attachments.
- XPBD-style compliant spring and area-constraint projection is available behind material knobs, with focused tests covering compliant residual stretch/area behavior before production tuning raises those defaults.
- Realistic front-view human body: skin and muscle sheets are meshed to follow an outline traced from a public-domain anatomical silhouette (outline points plus an interior hexagonal lattice, Delaunay-triangulated), with mitten hands, separated legs, and the muscle sheet inset just inside the skin.
- Dynamic segmented bones placed at landmarks measured on the same silhouette, including a low-resolution rib-cage proxy and hands and feet on wrist and ankle joints, attached to nearby muscle points, and connected by breakable bone joints.
- Bone joints can subluxate under traumatic stretch/overextension before full breakage, adding limited slack, weaker correction, and first-time local ligament/capsule tissue damage so dislocation exists between intact articulation and total separation.
- Post-fracture joint limits let broken or remapped limb joints sag and twist with slack instead of snapping rigidly or separating without bounds.
- Three hand-held tools whose drawings match their collision shapes exactly: a baseball bat (blunt; the barrel lies across the swing), a double-edged knife (sharp; the tip leads), and a sledgehammer (heavy; a striking face leads). Each tool is a solid object with momentum: the hand pulls it toward the pointer like a spring, presses it into the body no harder than an arm can push whatever the tool weighs, and the body pushes back. Tools turn to follow their motion in the air but resist turning once in tissue, a knife pulled back withdraws instead of flipping around in the wound, and each step's motion is swept so fast swings cannot skip thin limbs.
- The knife moves only through fibers it severs. Each fiber its tip or cutting edge reaches is cut when the knife's momentum or the hand's push overcomes it; otherwise the knife stops against it, so a light push rests on the skin and pressing harder cuts in, and it slides along fibers and bone it cannot cut. Skin is the tough layer; the muscle under it parts readily. Bone stops the blade without breaking. A knife put down onto the body goes into the flesh there and cuts its way along. In this front view the spine lies behind the chest and belly, so it does not stop a blade cutting the front of the trunk; ribs, pelvis, collarbones, and limb bones do.
- A bat or hammer gives its momentum to the tissue and bone it shoves, so it slows and stops in the body instead of sweeping through it, and the tissue keeps pushing back on it in every solver pass. A blow bruises in proportion to how hard it knocks the flesh, but a broad face spreads the force, so a bat rarely splits the skin, and an arm takes a blow aimed past it. A bat or hammer gripped while inside the body passes through until it is clear, so it cannot appear in flesh and blast it apart.
- Stress-based tearing from overstretched or high-impulse springs.
- Sharp cuts propagate from existing wound edges into adjacent stressed or fatigued skin springs under a per-step cap, so cuts can grow along local stress instead of appearing only as isolated threshold breaks.
- Fresh knife cuts in the skin deepen into exposed or loaded muscle directly beneath them under a separate cap, so deep cuts follow the layered anatomy, while older wounds do not keep widening as the knife moves elsewhere.
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
- Bones fracture at loaded contact points into recursive fragments, release nearby muscle-to-bone anchors, keep rotational inertia, and continue damaging tissue from broken ends. Most fractures stay closed: the bone ends tear the muscle right at the break and bruise the flesh around it while the skin and the bleeding stay inside. Only a break loaded far past the bone's strength throws its pieces apart, chips a splinter, and tears out through the flesh as an open, bleeding fracture.
- Moving splinter or broken-bone tips can puncture intact skin from inside the body under impulse, and severe fractured-rib tips can puncture organ proxies, with capped telemetry separate from generic fragment-tissue tearing.
- Severe moving fracture fragments can lacerate nearby major vessel paths through a capped swept-tip query, opening the same pressure-wound system while keeping fragment-driven vascular injury separate from direct tool cuts.
- Open fractures create bone-anchored marrow bleeding sources that follow the broken fragment and leak through the persistent wound system instead of being only a one-frame particle burst.
- Runtime budgets, broad-phase spatial filtering, and low-energy fragment sleeping cap active fragment work, fragment-bone checks, fragment-pair checks, fragment-tissue checks, vessel lacerations, wound sources, and fluid particles so heavier destruction remains PC-real-time.
- Broken fragments now collide with nearby intact bones, push against them, and transfer load back into the skeleton instead of only interacting with tissue and other fragments.
- Slow fragment-to-intact-bone overlaps and late-settle near contacts add damping, friction, and resting support so debris can jam against remaining skeleton instead of sliding through or rattling endlessly.
- Fragment-pair contacts damp closing velocity, tangential sliding, and angular jitter so piles of debris settle instead of endlessly rattling.
- Slow fragment-pair overlaps receive extra resting-contact support so settled debris resists tiny sinking/rattling under sustained load without raising global solver iterations.
- Free fragments use radius-aware floor contacts with damping, friction, angular drag, and resting-contact telemetry so debris can settle against the environment instead of being only center-clamped.
- Recursive fracture density is tuned through explicit material controls for maximum fracture depth, generic/rib-specific minimum fragment length, and secondary fragment strength, with deterministic strike gates covering long-settle debris.
- Fluid particles emit from tissue tears, attachment releases, and fractures, then fall, settle, and fade through the same simulation step.
- Settled fluid particles now deposit capped, mergeable blood stains/pools on the environment so bleeding leaves persistent physical evidence instead of disappearing as particle fade only.
- Persistent wound sources anchor to nearby moving tissue or bone features, leak or briefly spray based on layer, depth, and pressure, clot down over time, and reopen when later local load, stretching of the tissue around the clot, or fresh damage at the same spot disturbs it; a clotted source keeps its place until the wound budget needs the slot, so a healed cut can bleed again when struck later.
- Wound leakage drains a finite normalized blood reserve, and remaining reserve feeds back into wound pressure/leak strength and passive tissue turgor so long severe bleeding does not behave like an infinite source or fully supported tissue.
- Wounds are drawn from the mesh itself, with no overlay marks: a clean cut is one continuous line through the middle of every severed skin spring that fades as the cut gapes, open skin shows the muscle beneath as solid flesh with a thin dark rim where intact skin meets the opening, torn-out muscle shows as a dark cavity, and bones lie beneath the flesh so they show only through openings or where a broken end sticks out.
- Visual damage diagnostics replay deterministic sharp and heavy strikes, then write a damage-focused SVG and primitive-count CSV so incision, wound-rim, exposed-muscle, lacerated-vessel, fluid, and fracture rendering can be inspected without launching the app.
- Anatomy view for inspecting muscle, major vessels, and bones without waiting for skin exposure.
- Rust diagnostics, strike scenario playback, and simulation tests.
- Smoothly shaded skin: one mesh with per-point color for load and bruising, soft shading strips inside the silhouette for volume, and a thin outline.

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

The scripts under `tools\` need PowerShell 7 (`pwsh`). Windows PowerShell 5.1 stops on cargo's normal progress output.

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

## Run In A Browser

The live version is at <https://azizjonkasimov.github.io/realistic-human-body-deformation-physics/>. The same Rust app compiles to WebAssembly and runs entirely in the visitor's browser, so the site is only static files and needs no server-side compute. [`.github/workflows/deploy-web.yml`](.github/workflows/deploy-web.yml) rebuilds and redeploys it on every push to `main`. If the build fails, the previous version stays live.

To build and try it locally, install the WebAssembly target once:

```powershell
rustup target add wasm32-unknown-unknown
```

Then build and serve it:

```powershell
.\tools\build_web.ps1 -Serve
```

Open <http://localhost:8080>, or <http://localhost:8080/?stats> to show the frame rate and how long each frame takes. `.\tools\serve_web.ps1` serves the last build again without rebuilding. The site is assembled in `target\web`: the page from `web\index.html`, the wasm module, and the `gl.js` loader copied from the miniquad version in `Cargo.lock`.

## Controls

- Left-drag to swing the selected tool into the body. The tool trails your pointer like a real tool in hand: swing from outside the body for a hard hit, or hold the tool against the body and keep dragging past it to press harder. The ring marks where your hand is. Damage comes from the tool's shape, its speed when it lands, how hard you press, and the selected mass.
- `B`, `S`, and `H` select the bat (blunt), knife (sharp), and sledgehammer (heavy).
- `D` toggles the contact debug overlay.
- `Tab` toggles anatomy view, where skin is wireframe and muscle/bones are visible.
- `R` resets the body.
- `Space` pauses or resumes.
- `1`, `2`, and `4` change striker mass.
- The control chips along the bottom are also buttons: click or tap them for the same actions. The mass chip cycles 1x, 2x, and 4x.
- On touch screens, drag a finger to strike. The chips grow to finger size after the first touch.

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

`.\tools\verify.ps1` builds and runs deterministic strike playback across representative torso, shoulder, arm, hip, and leg strikes with blunt, sharp, and heavy tools. Each scripted swing moves the hand from outside the body and holds briefly at the end with the button down, so the tool, which trails the hand, lands as a real swing would. The scenario target writes frame-by-frame contact telemetry to:

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

The CSV outputs include region, intent, tool mode, striker speed, impact, contact counts, contact depth, tissue/bone loads, sharp cut propagation counts, skin-to-muscle cut transfer counts, sharp skin-flap delamination counts, fiber-aligned muscle tear counts, muscle crush-rupture counts, torso cavity pressure/collapse/rupture counts, organ damage/direct-penetration/rib-puncture/rupture counts, direct and fragment-driven major vessel laceration counts, soft-tissue contusion counts, local tissue-softening maxima, spring-fatigue events and local fatigue maxima, plastic deformation events and local plasticity maxima, joint subluxation/breakage, ligament/capsule damage events, fracture events, rib-fracture counts, fracture marrow-source counts, post-fracture joint limit corrections, wound counts, wound reopen counts, wound pressure/clotting, blood loss, final blood reserve, final blood-turgor scale, blood stain/pool deposits, broken-end tissue contacts, inside-out skin puncture counts, fragment-bone contacts/damping/resting support, fragment-pair contacts/damping/resting support, fragment-floor contacts/resting support, overlap depth, fragment angular speed, free/spinning/sleeping fragment counts, runtime budget checks/skips/replacements, fluid emission, final fragment counts, and accumulated damage stats. The bands gate realistic injury for each tool: bat blows to the arm, shoulder, and leg must bruise widely while tearing little skin, cutting no major vessel, and breaking at most an arm; a full-force sledgehammer into the chest must break the arm and ribs, bruise deeply, and injure organs without pulping the chest, while the thigh's femur holds against it; knife cuts on the belly and arm must leave an incision through skin and muscle with skin flaps and deep cut transfer, reach a major vessel where staged, and break no bone at all, so the knife cannot regress into a club; `thigh_cut_rebleed` lets a knife cut down the thigh clot and then requires a bat blow to make it bleed again; and `torso_heavy_fragment_settle` waits after the sledgehammer blow so fragment contact, resting support, and sleep behavior are covered.

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

Open `output\damage_visual_debug.svg` to inspect two deterministic damage captures side by side: a knife cut on the belly, which must leave every bone intact, and a long-settle sledgehammer blow to the chest. The diagnostic also writes primitive counts to:

```text
output\damage_visual_summary.csv
```

The visual diagnostic exits nonzero if the captures no longer include expected wound edges, incision lines, failed-muscle voids, contusion discoloration, direct/fragment lacerated vessel evidence, wound sources, visible fluid particles, finite blood-loss, turgor, cavity-pressure, organ-penetration/rib-puncture/injury metrics, blood stain pools, fractured bones, rib-fracture evidence, fracture caps, or overall damage primitives.

## Development Notes

- `Cargo.toml` defines the Rust library, app, diagnostics, and strike scenario binaries.
- `src/simulation.rs` contains the physics data model, integration, constraints, tearing, bone fracture, major vessels, wounds, and fluid particles; `src/simulation/body.rs` generates the layered body; `src/simulation/tools.rs` holds the tools: their shapes, the hand that drives them, and their contact with tissue and bone.
- `src/bin/realistic_physics.rs` owns the `macroquad` app shell, input, timing, and rendering. It also runs in the browser, so the app and simulation must avoid file I/O, threads, and `std::time`, none of which work on `wasm32-unknown-unknown`.
- `web/index.html` is the browser page: start screen with a content warning, loader, error messages, and the `?stats` frame-time overlay (also exposed as `window.__perf` for automated checks).
- `tools/build_web.ps1` builds the browser version into `target\web`; `tools/serve_web.ps1` serves it locally with the `application/wasm` content type browsers require.
- `.cargo/config.toml` lets the wasm linker leave miniquad's WebGL functions as imports for `gl.js` to provide; recent Rust versions no longer do that by default.
- `src/bin/anatomy_diagnostics.rs` writes a deterministic SVG anatomy snapshot and reports geometry validation metrics.
- `src/bin/strike_scenarios.rs` writes deterministic strike telemetry and tuning summaries.
- `src/bin/visual_damage_diagnostics.rs` writes deterministic SVG damage captures and visual primitive metrics.
- `tests/simulation_tests.rs` contains focused Rust simulation checks.
- `src/silhouette.rs` turns `docs/reference/human_body_silhouette.svg` (public domain, see `docs/reference/README.md`) into a signed distance field; `src/simulation/body.rs` meshes the body from it and places the anatomy. `body_frame` maps body coordinates (fractions of body height) to the window, which the strike scenarios use to aim at anatomy.

The next Rust simulation milestones are:

1. Expand the Rust test matrix to cover more strike and fracture edge cases.
2. Raise tissue spring/area compliance from the current neutral defaults in measured XPBD tuning passes, using strike telemetry to keep fracture, cavity, vessel, and debris behavior separated.
3. Tighten strike tuning bands once material behavior has settled enough for intentional regression gates.
4. Keep increasing fracture density in small measured steps, using the long-settle telemetry to catch fragment sleep, budget, and stability regressions.
5. Verify the `macroquad` app on macOS and document any platform-specific packaging steps.
6. Add a renderer screenshot or baseline-image comparison path once the headless SVG damage diagnostic has stabilized.

## Toolchain

The primary build now uses Rust and Cargo. The app frontend uses `macroquad` for a cross-platform window, input, and 2D drawing path while the simulation stays in a reusable Rust library. The browser version is the same app compiled for `wasm32-unknown-unknown` and drawn with WebGL.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE).
