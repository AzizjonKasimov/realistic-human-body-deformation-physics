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
- Mouse-controlled blunt striker with a spring-driven heavy head, visible handle/target, and impact direction.
- Stress-based tearing from overstretched or high-impulse springs.
- Exposed muscle is a real second mesh coupled to skin through breakable attachments.
- Bones fracture at the loaded contact point into separate fragments, can re-fracture while pieces are still large enough, release nearby muscle-to-bone anchors, and damage local tissue so broken pieces separate instead of just slipping out of place.
- Anatomy view for inspecting muscle and bones without waiting for skin exposure.
- Contact debug overlay for inspecting striker speed, mass, impact, contact counts, loads, and fracture impulses.
- Small console test and deterministic strike-scenario targets for core simulation checks from WSL or Windows.

The current striker is a spring-driven blunt mass: the mouse controls a target/handle, while the heavy head lags behind and carries velocity into the body. The current bone layer now participates in the simulation through simple rigid segment constraints, breakable hinge-like bone joints, muscle attachments, contact-local fracture, bounded re-fracture, local tissue damage, and small deterministic splinters. Fractured pieces no longer get pulled back toward their original pose. It is not a full articulated skeleton yet: richer rotational inertia and more convincing post-fracture limb behavior should move into focused follow-up milestones.

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

- Left-drag to swing the blunt striker into the body. Damage comes from heavy-head overlap, swing speed, and selected striker mass.
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

The core test verifies that the generated body has nested skin/muscle layers, muscle-to-bone attachments, bone joints, and bones; remains stable at rest; inactive input leaves contact telemetry idle; joints transfer motion under moderate load; direct striker contact moves and fractures a bone while exposing contact debug metrics and damaging nearby tissue; off-center bone contact cracks near the contact point with a persistent gap; long fractured fragments can re-fracture; tears open skin triangles; and splits bone segments under a high-energy strike.

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

The CSV outputs include striker speed, impact, contact counts, contact depth, tissue/bone loads, joint breakage, fracture events, final fragment counts, and accumulated damage stats for repeatable torso, shoulder, and hip strikes.

## Development Notes

The native implementation is split so simulation can remain independent from rendering:

- `CMakeLists.txt` defines the native app and test target.
- `src/cpp/simulation.hpp` declares the physics data model and public simulation API.
- `src/cpp/simulation.cpp` contains body generation, integration, constraints, breakable bone joints, tearing, and exposure logic.
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

1. Add fluid particles emitted from torn constraints.
2. Add simple tool modes, such as blunt fist versus sharp striker.
3. Add richer post-fracture fragment collision and internal tissue damage from sharp bone ends.
4. Add rotational inertia to free bone fragments so the hinge solver and fracture recoil read less like endpoint-only motion.
5. Expand deterministic strike scenarios into a small tuning matrix for material constants and body topology changes.

## Toolchain

The primary build uses CMake and a Windows C++ compiler, preferably Visual Studio 2022 Build Tools or the full Visual Studio IDE. The current renderer uses plain Win32/GDI so the project can stay dependency-free while the simulation is still being discovered.
