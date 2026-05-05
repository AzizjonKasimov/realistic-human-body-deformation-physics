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
- Post-fracture joint limits let broken or remapped limb joints sag and twist with slack instead of snapping rigidly or separating without bounds.
- Mouse-controlled tool head with blunt, sharp, and heavy modes, a spring-driven handle/target, impact direction, mode-specific handling, and distinct tool silhouettes.
- Stress-based tearing from overstretched or high-impulse springs.
- Exposed muscle is a real second mesh coupled to skin through breakable attachments.
- Bones fracture at the loaded contact point into separate fragments, can re-fracture while pieces are still large enough, release nearby muscle-to-bone anchors, keep rotational inertia after separation, and continue damaging nearby tissue from sharp broken ends and splinters.
- Fractured bone pieces and splinters use capsule-style fragment repulsion so loose pieces separate instead of piling through each other.
- Fluid particles emit from real tissue tears, attachment releases, and fracture-adjacent damage, then fall, settle, and fade through the same simulation step.
- Persistent wound sources leak or briefly spray based on layer, depth, and pressure, then clot down over time.
- Anatomy view for inspecting muscle and bones without waiting for skin exposure.
- Contact debug overlay for inspecting tool mode, striker speed, mass, impact, contact counts, loads, fracture impulses, fragment tissue contacts, fragment-pair contacts, post-fracture joint limits, wound leaks, fragment spin, and fluid emission.
- Small console test, anatomy diagnostics, and a calibrated 22-case deterministic strike tuning matrix for core simulation checks from WSL or Windows.

The current striker is a spring-driven tool head: the mouse controls a target/handle, while the active head lags behind and carries velocity into the body. Blunt mode balances crushing and tearing, sharp mode uses the rendered blade segment for narrower edge/tip contact and blade-motion wound normals, and heavy mode drives stronger bone loads with slower hammer-like handling. The current bone layer now participates in the simulation through simple rigid segment constraints, breakable hinge-like bone joints, post-fracture slack/twist limits, muscle attachments, contact-local fracture, bounded re-fracture, local tissue damage, small deterministic splinters, rotational inertia for free fragments, fragment-to-fragment repulsion, continued broken-end tissue contact, pressure-based wound leaks, and fluid bursts from damaged tissue. Fractured pieces no longer get pulled back toward their original pose. It is not a full articulated skeleton yet: richer tuning and collision responses should move into focused follow-up milestones.

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

The core test verifies that the generated body has nested skin/muscle layers, muscle-to-bone attachments, bone joints, and bones; remains stable at rest without fluid, wounds, or fragment damage; inactive input leaves contact telemetry idle; tool selection is reflected in debug telemetry; sharp mode can cut skin and open a clotting wound source; heavy mode applies larger bone loads than blunt; joints transfer motion under moderate load; broken and fracture-remapped joints limit impossible post-fracture stretch and twist while still allowing sag; direct striker contact moves and fractures a bone while exposing contact debug metrics, damaging nearby tissue, emitting fluid particles, opening persistent wound leaks, and reporting broken-end tissue contact; off-center bone contact cracks near the contact point with a persistent gap, seeds fragment angular velocity, keeps free fragments rotating while settling, and avoids deep fragment overlap; overlapping fractured bones report fragment-pair contacts and separate while settling; long fractured fragments can re-fracture; tears open skin triangles; and splits bone segments under a high-energy strike.

To run tests, diagnostics, and rebuild the double-click app in one pass:

```powershell
.\tools\verify.ps1 -BuildApp
```

## Strike Scenarios

`.\tools\verify.ps1` also builds and runs deterministic strike playback across torso, shoulder, arm, hip, and leg strikes with blunt, sharp, and heavy tools at low/medium/high energies. The scenario target writes frame-by-frame contact telemetry to:

```text
E:\PersonalProjects\realistic_physics\output\strike_scenarios.csv
```

It also writes a compact per-scenario tuning summary to:

```text
E:\PersonalProjects\realistic_physics\output\strike_summary.csv
```

It also writes a warning-only tuning report that compares each scenario against calibrated expected damage bands:

```text
E:\PersonalProjects\realistic_physics\output\strike_tuning_report.txt
```

The CSV outputs include region, intent, tool mode, striker speed, impact, contact counts, contact depth, tissue/bone loads, joint breakage, fracture events, post-fracture joint limit corrections, wound counts, wound pressure/clotting, broken-end tissue contacts, fragment-pair contacts and overlap depth, fragment angular speed, free/spinning fragment counts, fluid emission, final fragment counts, and accumulated damage stats.

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

1. Add richer fragment collision responses, such as fragment-to-intact-bone contacts and less jitter when many splinters pile together.
2. Add historical baseline comparison for strike summaries so regressions are easier to spot.
3. Tighten selected tuning bands once material behavior has settled enough for intentional regression gates.
4. Anchor wound sources to moving tissue/bone features instead of keeping them at fixed world positions.
5. Add contact-normal telemetry and visual compression/spark feedback for clearer tool impacts.

## Toolchain

The primary build uses CMake and a Windows C++ compiler, preferably Visual Studio 2022 Build Tools or the full Visual Studio IDE. The current renderer uses plain Win32/GDI so the project can stay dependency-free while the simulation is still being discovered.
