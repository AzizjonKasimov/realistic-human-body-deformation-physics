# Realistic Physics

A no-engine 2D physics sandbox rewritten in Rust. The simulation is kept separate from the renderer so the physics model can be tested, tuned, and eventually shipped on more than one desktop platform.

See [docs/VISION.md](docs/VISION.md) for the original project description and agreed direction.

## Demo Video

[![Watch the Realistic Physics demo video on YouTube](https://img.youtube.com/vi/hCXVPSU6etE/hqdefault.jpg)](https://www.youtube.com/watch?v=hCXVPSU6etE)

## Current Rust Milestone

- Rust Cargo project with a reusable `realistic_physics` simulation library.
- Cross-platform `macroquad` desktop app for windowing, input, and rendering.
- Verlet/PBD-style soft body points, springs, area constraints, and attachments.
- Separate skin and muscle meshes generated from nested body masks so muscle stays inside the skin silhouette.
- Dynamic segmented bones generated from the same body proportions, attached to nearby muscle points, and connected by breakable bone joints.
- Post-fracture joint limits let broken or remapped limb joints sag and twist with slack instead of snapping rigidly or separating without bounds.
- Mouse-controlled tool head with blunt, sharp, and heavy modes, a spring-driven handle/target, impact direction, mode-specific handling, and distinct tool silhouettes.
- Stress-based tearing from overstretched or high-impulse springs.
- Exposed muscle is a second mesh coupled to skin through breakable attachments.
- Bones fracture at loaded contact points into separate fragments and splinters, release nearby muscle-to-bone anchors, keep rotational inertia, and continue damaging tissue from broken ends.
- Fluid particles emit from tissue tears, attachment releases, and fractures, then fall, settle, and fade through the same simulation step.
- Persistent wound sources leak or briefly spray based on layer, depth, and pressure, then clot down over time.
- Anatomy view for inspecting muscle and bones without waiting for skin exposure.
- Rust diagnostics, strike scenario playback, and simulation tests.
- Checked-in human silhouette reference under `docs/reference/` for tuning the generated front-view body proportions.

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

The Rust verifier runs formatting checks, simulation tests, deterministic strike playback, and anatomy diagnostics. To also build the app executable:

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

The CSV outputs include region, intent, tool mode, striker speed, impact, contact counts, contact depth, tissue/bone loads, joint breakage, fracture events, post-fracture joint limit corrections, wound counts, wound pressure/clotting, broken-end tissue contacts, fragment-pair contacts and overlap depth, fragment angular speed, free/spinning fragment counts, fluid emission, final fragment counts, and accumulated damage stats.

## Anatomy Diagnostics

Use this whenever changing body generation, anatomy layers, bones, constraints, or rendering assumptions:

```powershell
.\tools\verify.ps1
```

Open `output\anatomy_debug.svg` to inspect the generated body without launching the app. Skin is translucent, muscle is red, bones are pale, muscle-to-bone attachments are blue, bone joints are yellow, and bone sample markers turn red if they fall outside the skin mesh. The diagnostic exits nonzero if sampled bone centerlines are outside skin.

## Development Notes

- `Cargo.toml` defines the Rust library, app, diagnostics, and strike scenario binaries.
- `src/simulation.rs` contains the physics data model, body generation, integration, constraints, tearing, bone fracture, wounds, and fluid particles.
- `src/bin/realistic_physics.rs` owns the `macroquad` app shell, input, timing, and rendering.
- `src/bin/anatomy_diagnostics.rs` writes a deterministic SVG anatomy snapshot and reports geometry validation metrics.
- `src/bin/strike_scenarios.rs` writes deterministic strike telemetry and tuning summaries.
- `tests/simulation_tests.rs` contains focused Rust simulation checks.
- `docs/reference/human_body_silhouette.svg` is the public-domain front-view silhouette reference used when tuning body proportions.

The next Rust simulation milestones are:

1. Expand the Rust test matrix to cover more strike and fracture edge cases.
2. Tighten strike tuning bands once material behavior has settled enough for intentional regression gates.
3. Anchor wound sources to moving tissue/bone features instead of keeping them at fixed world positions.
4. Verify the `macroquad` app on macOS and document any platform-specific packaging steps.
5. Add richer fragment collision responses, such as fragment-to-intact-bone contacts and less jitter when many splinters pile together.

## Toolchain

The primary build now uses Rust and Cargo. The app frontend uses `macroquad` for a cross-platform window, input, and 2D drawing path while the simulation stays in a reusable Rust library.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE).
