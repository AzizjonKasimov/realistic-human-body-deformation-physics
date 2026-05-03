# Realistic Physics

A no-engine 2D physics sandbox for Windows-first experimentation. The project is now a native C++ application, with simulation code kept separate from rendering so the physics model can be tested and evolved deliberately.

See [docs/VISION.md](docs/VISION.md) for the original project description and agreed direction.

## Current Native Milestone

- Native Win32 window written in C++.
- No game engine and no runtime dependencies beyond the Windows toolchain.
- CMake project with a separate `realistic_physics_core` simulation library.
- Verlet/PBD-style soft body points, springs, area constraints, and attachments.
- Separate skin and muscle meshes generated from nested body masks so muscle stays inside the skin silhouette.
- Dynamic segmented bones generated from the same body proportions and attached to nearby muscle points.
- Mouse-controlled circular striker.
- Stress-based tearing from overstretched or high-impulse springs.
- Exposed muscle is a real second mesh coupled to skin through breakable attachments.
- Bones can fracture into separate segments, and muscle-to-bone attachments can tear under impact stress.
- Anatomy view for inspecting muscle and bones without waiting for skin exposure.
- Small console test target for core simulation checks from WSL or Windows.

The current bone layer now participates in the simulation through simple rigid segment constraints and muscle attachments. It is not a full articulated skeleton yet: joints, richer rotational inertia, and more convincing post-fracture limb behavior should move into focused follow-up milestones.

## Run Native On Windows

After the app has been built once, double-click this file from File Explorer:

```text
E:\PersonalProjects\realistic_physics\realistic_physics.exe
```

When rebuilding after code changes, run:

```powershell
cd E:\PersonalProjects\realistic_physics
& "C:\Program Files\CMake\bin\cmake.exe" -S . -B build\vs -G "Visual Studio 17 2022" -A x64
& "C:\Program Files\CMake\bin\cmake.exe" --build build\vs --config Release --target realistic_physics
.\realistic_physics.exe
```

Controls:

- Left-drag to strike the body.
- `Tab` toggles anatomy view, where skin is wireframe and muscle/bones are visible.
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

The core test verifies that the generated body has nested skin/muscle layers, muscle-to-bone attachments, and bones; remains stable at rest; tears open skin triangles; and splits bone segments under a high-energy strike.

## Development Notes

The native implementation is split so simulation can remain independent from rendering:

- `CMakeLists.txt` defines the native app and test target.
- `src/cpp/simulation.hpp` declares the physics data model and public simulation API.
- `src/cpp/simulation.cpp` contains body generation, integration, constraints, tearing, and exposure logic.
- `src/cpp/win32_main.cpp` owns the Win32 window, input, timing, and GDI drawing.
- `tests/simulation_tests.cpp` contains smoke tests for the core simulation.
- `tools/anatomy_diagnostics.cpp` writes a deterministic SVG anatomy snapshot and reports geometry validation metrics.

## Anatomy Diagnostics

Use this whenever changing body generation, anatomy layers, bones, constraints, or rendering assumptions:

```powershell
cd E:\PersonalProjects\realistic_physics
& "C:\Program Files\CMake\bin\cmake.exe" --build build\vs --config Debug --target realistic_physics_diagnostics
.\build\vs\Debug\realistic_physics_diagnostics.exe output\anatomy_debug.svg
```

Open `output\anatomy_debug.svg` to inspect the generated body without launching the app. Skin is translucent, muscle is red, bones are pale, muscle-to-bone attachments are blue, and bone sample markers turn red if they fall outside the skin mesh. The diagnostic exits nonzero if sampled bone centerlines are outside skin.

The next native simulation milestones are:

1. Add explicit bone joints so limbs bend and separate through articulated constraints instead of only individual segment motion.
2. Add a native debug/QA overlay for rest stability and deterministic strike scenarios.
3. Tune material constants, fracture thresholds, and body topology for more convincing impacts.
4. Add fluid particles emitted from torn constraints.
5. Add simple tool modes, such as blunt fist versus sharp striker.

## Toolchain

The primary build uses CMake and a Windows C++ compiler, preferably Visual Studio 2022 Build Tools or the full Visual Studio IDE. The current renderer uses plain Win32/GDI so the project can stay dependency-free while the simulation is still being discovered.
