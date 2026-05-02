# Realistic Physics

A no-engine 2D physics sandbox for Windows-first experimentation. The project is now a native C++ application, with simulation code kept separate from rendering so the physics model can be tested and evolved deliberately.

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

Bones, fracture, fluid particles, richer debug views, and QA scenarios should move into the native code in focused milestones.

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

## Development Notes

The native implementation is split so simulation can remain independent from rendering:

- `CMakeLists.txt` defines the native app and test target.
- `src/cpp/simulation.hpp` declares the physics data model and public simulation API.
- `src/cpp/simulation.cpp` contains body generation, integration, constraints, tearing, and exposure logic.
- `src/cpp/win32_main.cpp` owns the Win32 window, input, timing, and GDI drawing.
- `tests/simulation_tests.cpp` contains smoke tests for the core simulation.

The next native simulation milestones are:

1. Add segmented bones and joint fracture.
2. Add a native debug/QA overlay for rest stability and deterministic strike scenarios.
3. Tune material constants, fracture thresholds, and body topology for more convincing impacts.
4. Add fluid particles emitted from torn constraints.
5. Add simple tool modes, such as blunt fist versus sharp striker.

## Toolchain

The primary build uses CMake and a Windows C++ compiler, preferably Visual Studio 2022 Build Tools or the full Visual Studio IDE. The current renderer uses plain Win32/GDI so the project can stay dependency-free while the simulation is still being discovered.
