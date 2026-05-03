# Realistic Physics

A no-engine 2D physics sandbox for Windows-first experimentation. The project is now a native C++ application, with simulation code kept separate from rendering so the physics model can be tested and evolved deliberately.

See [docs/VISION.md](docs/VISION.md) for the original project description and agreed direction.

## Current Native Milestone

- Native Win32 window written in C++.
- No game engine and no runtime dependencies beyond the Windows toolchain.
- CMake project with a separate `realistic_physics_core` simulation library.
- Verlet/PBD-style soft body points, springs, area constraints, and attachments.
- Separate skin and muscle meshes generated from nested body masks so muscle stays inside the skin silhouette.
- Dynamic segmented bones generated from the same body proportions, attached to nearby muscle points, and connected by breakable bone joints.
- Mouse-controlled tool head with blunt, sharp, and heavy modes, a spring-driven handle/target, and impact direction.
- Stress-based tearing from overstretched or high-impulse springs.
- Exposed muscle is a real second mesh coupled to skin through breakable attachments.
- Bones fracture at the loaded contact point into separate fragments, can re-fracture while pieces are still large enough, release nearby muscle-to-bone anchors, keep rotational inertia after separation, and continue damaging nearby tissue from sharp broken ends and splinters.
- Fluid particles emit from real tissue tears, attachment releases, and fracture-adjacent damage, then fall, settle, and fade through the same simulation step.
- Anatomy view for inspecting muscle and bones without waiting for skin exposure.
- Contact debug overlay for inspecting tool mode, striker speed, mass, impact, contact counts, loads, fracture impulses, fragment contacts, fragment spin, and fluid emission.
- Small console test and deterministic strike-scenario targets for core simulation checks from WSL or Windows.

The current striker is a spring-driven tool head: the mouse controls a target/handle, while the active head lags behind and carries velocity into the body. Blunt mode balances crushing and tearing, sharp mode concentrates pressure into smaller cuts, and heavy mode drives stronger bone loads. The current bone layer now participates in the simulation through simple rigid segment constraints, breakable hinge-like bone joints, muscle attachments, contact-local fracture, bounded re-fracture, local tissue damage, small deterministic splinters, rotational inertia for free fragments, continued broken-end tissue contact, and fluid bursts from damaged tissue. Fractured pieces no longer get pulled back toward their original pose. It is not a full articulated skeleton yet: richer fragment collision and more convincing post-fracture limb behavior should move into focused follow-up milestones.

## Run Native On Windows

After the app has been built once, double-click this file from File Explorer:

```text
E:\PersonalProjects\realistic_physics\realistic_physics.exe
```

When rebuilding after code changes, run:

```powershell
cd E:\PersonalProjects\realistic_physics
.\tools\build_app.ps1
.\realistic_physics.exe
```

If the app is already open and the build cannot replace `realistic_physics.exe`, close the app or run:

```powershell
.\tools\build_app.ps1 -StopRunningApp
```

If Windows blocks script execution, run the same scripts through PowerShell with a one-time bypass:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\verify.ps1 -BuildApp
```

Controls:

- Left-drag to swing the selected tool into the body. Damage comes from tool shape, overlap, swing speed, and selected striker mass.
- `B`, `S`, and `H` select blunt, sharp, and heavy tool modes.
- `D` toggles the contact debug overlay.
- `Tab` toggles anatomy view, where skin is wireframe and muscle/bones are visible.
- `R` resets the body.
- `Space` pauses or resumes.
- `1`, `2`, and `4` change striker mass.

## Run Core Tests

From PowerShell:

```powershell
cd E:\PersonalProjects\realistic_physics
.\tools\verify.ps1
```

The core test verifies that the generated body has nested skin/muscle layers, muscle-to-bone attachments, bone joints, and bones; remains stable at rest without fluid or fragment damage; inactive input leaves contact telemetry idle; tool selection is reflected in debug telemetry; sharp mode can cut skin; heavy mode applies larger bone loads than blunt; joints transfer motion under moderate load; direct striker contact moves and fractures a bone while exposing contact debug metrics, damaging nearby tissue, emitting fluid particles, and reporting broken-end tissue contact; off-center bone contact cracks near the contact point with a persistent gap, seeds fragment angular velocity, and keeps free fragments rotating while settling; long fractured fragments can re-fracture; tears open skin triangles; and splits bone segments under a high-energy strike.

To run tests, diagnostics, and rebuild the double-click app in one pass:

```powershell
.\tools\verify.ps1 -BuildApp
```

## Strike Scenarios

`.\tools\verify.ps1` also builds and runs deterministic strike playback. The scenario target writes frame-by-frame contact telemetry to:

```text
E:\PersonalProjects\realistic_physics\output\strike_scenarios.csv
```

It also writes a compact per-scenario tuning summary to:

```text
E:\PersonalProjects\realistic_physics\output\strike_summary.csv
```

The CSV outputs include tool mode, striker speed, impact, contact counts, contact depth, tissue/bone loads, joint breakage, fracture events, broken-end tissue contacts, fragment angular speed, free/spinning fragment counts, fluid emission, final fragment counts, and accumulated damage stats for repeatable torso, shoulder, and hip strikes.

## Development Notes

The native implementation is split so simulation can remain independent from rendering:

- `CMakeLists.txt` defines the native app and test target.
- `src/cpp/simulation.hpp` declares the physics data model and public simulation API.
- `src/cpp/simulation.cpp` contains body generation, integration, constraints, breakable bone joints, tearing, fluid particles, and exposure logic.
- `src/cpp/win32_main.cpp` owns the Win32 window, input, timing, and GDI drawing.
- `tests/simulation_tests.cpp` contains smoke tests for the core simulation.
- `tools/anatomy_diagnostics.cpp` writes a deterministic SVG anatomy snapshot and reports geometry validation metrics.

## Anatomy Diagnostics

Use this whenever changing body generation, anatomy layers, bones, constraints, or rendering assumptions:

```powershell
cd E:\PersonalProjects\realistic_physics
.\tools\verify.ps1
```

Open `output\anatomy_debug.svg` to inspect the generated body without launching the app. Skin is translucent, muscle is red, bones are pale, muscle-to-bone attachments are blue, bone joints are yellow, and bone sample markers turn red if they fall outside the skin mesh. The diagnostic exits nonzero if sampled bone centerlines are outside skin.

The next native simulation milestones are:

1. Add fragment-to-fragment repulsion so loose splinters and large fragments cannot overlap each other.
2. Expand deterministic strike scenarios into a larger tuning matrix for material constants and body topology changes.
3. Add wound-pressure controls so deep damage can leak, spray, or clot differently by tissue layer.
4. Add per-tool visual polish, such as sharper blade contact normals and heavier hammer rebound.
5. Add simple post-fracture joint limits for partially attached limb sections so broken anatomy can sag and twist without snapping into impossible poses.

## Toolchain

The primary build uses CMake and a Windows C++ compiler, preferably Visual Studio 2022 Build Tools or the full Visual Studio IDE. The current renderer uses plain Win32/GDI so the project can stay dependency-free while the simulation is still being discovered.
