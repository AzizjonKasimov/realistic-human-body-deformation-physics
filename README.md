# Realistic Physics

A no-engine 2D physics sandbox for Windows-first experimentation. The main implementation direction is now native C++, with the original browser prototype kept as reference material while the simulation model is ported properly.

See [docs/VISION.md](docs/VISION.md) for the original project description and agreed direction.

## Current Native Milestone

- Native Win32 window written in C++.
- No game engine and no runtime dependencies beyond the Windows toolchain.
- CMake project with a separate `realistic_physics_core` simulation library.
- Verlet/PBD-style soft body points, springs, area constraints, and attachments.
- Separate skin and muscle meshes generated at startup.
- Mouse-controlled circular striker.
- Stress-based tearing from overstretched or high-impulse springs.
- Exposed muscle is a real second mesh coupled to skin through breakable attachments.
- Small console test target for core simulation checks from WSL or Windows.

The C++ version is the primary path from here. Bones, fracture, fluid particles, richer debug views, and QA scenarios should move into the native code in focused milestones.

## Run Native On Windows

From **Developer PowerShell for Visual Studio**:

```powershell
cd E:\PersonalProjects\realistic_physics
cmake -S . -B build\vs -G "Visual Studio 17 2022" -A x64
cmake --build build\vs --config Debug
.\build\vs\Debug\realistic_physics.exe
```

Controls:

- Left-drag to strike the body.
- `R` resets the body.
- `Space` pauses or resumes.
- `1`, `2`, and `4` change striker impact strength.

## Run Core Tests

From PowerShell after configuring with CMake:

```powershell
cd E:\PersonalProjects\realistic_physics
cmake --build build\vs --config Debug --target realistic_physics_tests
.\build\vs\Debug\realistic_physics_tests.exe
```

The core test verifies that the generated body has points, springs, and triangles; remains stable at rest; and tears under a high-energy strike.

## Browser Prototype Reference

The original JavaScript prototype still exists and remains useful as a behavior reference while native systems are ported. It includes more feature experiments than the first native milestone:

- Single-window browser runtime using HTML5 Canvas.
- No game engine or external dependencies.
- Verlet/PBD-style soft body points, springs, area constraints, and attachments.
- Separate skin and muscle meshes generated at startup.
- Internal segmented bones for skull, spine, arms, and legs.
- Mouse-controlled circular striker.
- Stress-based tearing from overstretched or high-impulse springs.
- Triangles disappear only when their live constraints fail, so wounds come from simulation state instead of decals.
- Exposed muscle is a real second mesh with directional fiber constraints and area preservation.
- Continued impact can break muscle fibers and skin-muscle attachments independently.
- Bones are chains of short rigid-ish segments connected by breakable joints.
- Bone joints accumulate bending, shear, separation, and impact stress, then fail locally.
- Tear boundaries are computed from live/dead triangle adjacency and drawn as exposed wound edges.

Run it from PowerShell:

```powershell
cd E:\PersonalProjects\realistic_physics
start .\index.html
```

Then click **Start** and drag the striker through the body.

Use **Impact** to cycle striker strength:

- `Impact 1x` for gentler poking.
- `Impact 2x` for the current default.
- `Impact 4x` for heavier fracture testing.

## Physics QA

The prototype includes an in-window QA harness so physics milestones are checked by behavior, not just hidden counters.

- **Debug** toggles always-visible bones, bone health bars, labels, contact impulses, impact vectors, and fracture marks.
- **QA Rest** verifies the body stays stable without accidental damage.
- **QA Arm** runs a deterministic right-forearm fracture strike.
- **QA Shin** runs a deterministic right-shin fracture strike.

For a fracture scenario to pass, the target bone must receive measurable impulse, build joint stress, fracture into local segments, and the broken segments must move visibly while remaining inspectable.

QA reports also include average and max physics step time, which helps catch performance regressions after heavy damage.

## Development Notes

The native implementation is split so simulation can remain independent from rendering:

- `CMakeLists.txt` defines the native app and test target.
- `src/cpp/simulation.hpp` declares the physics data model and public simulation API.
- `src/cpp/simulation.cpp` contains body generation, integration, constraints, tearing, and exposure logic.
- `src/cpp/win32_main.cpp` owns the Win32 window, input, timing, and GDI drawing.
- `tests/simulation_tests.cpp` contains smoke tests for the core simulation.

The browser prototype is intentionally left in place for comparison:

- `index.html` hosts the canvas and minimal start/quit controls.
- `src/materials.js` contains current material constants.
- `src/physics.js` contains the layered constraint solver, bone contacts, segmented bone joints, and fracture logic.
- `src/bodyFactory.js` generates the humanoid skin, muscle, and bone layers.
- `src/renderer.js` draws skin, exposed muscle, fibers, wound edges, bones, and stress.
- `src/input.js` owns pointer input.
- `src/qa.js` contains deterministic physics QA scenarios and pass/fail assertions.
- `src/sandbox.js` wires the app together.
- `src/styles.css` contains the window and control styling.

The next native simulation milestones are:

1. Port segmented bones and joint fracture from the browser prototype.
2. Add a native debug/QA overlay for rest stability and deterministic strike scenarios.
3. Tune material constants, fracture thresholds, and body topology for more convincing impacts.
4. Add fluid particles emitted from torn constraints.
5. Add simple tool modes, such as blunt fist versus sharp striker.

## Toolchain

The primary build uses CMake and a Windows C++ compiler, preferably Visual Studio 2022 Build Tools or the full Visual Studio IDE. The current renderer uses plain Win32/GDI so the project can stay dependency-free while the simulation is still being discovered.
