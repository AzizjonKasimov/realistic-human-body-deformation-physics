# AGENTS.md

Instructions for AI coding agents working in this repository.

## Project Context

This project is a no-engine 2D physics sandbox for Windows-first experimentation. The current long-term vision is preserved in `docs/VISION.md`; read it before making product or architecture decisions.

The project should prioritize real simulation behavior over scripted visual tricks. Start with focused milestones, but keep the eventual layered-body sandbox in mind:

- outer skin
- middle muscle
- inner bone
- mouse-controlled striker or simple weapon
- stress-based tearing
- soft tissue deformation
- fracture
- particle-based blood/fluid
- single-window toy-like experience

## Working Style

- If the user asks a question, answer the question first. Do not edit code unless the user asks for implementation or the answer clearly requires inspection plus a fix.
- The user runs Codex in WSL2 but runs the project on Windows. Prefer PowerShell commands in user-facing instructions whenever possible.
- Maintain main `README.md` files well as the project evolves.
- Keep `docs/VISION.md` aligned with major product direction changes.
- This repository may grow beyond quick patches. Do not default only to the smallest possible fix. When a problem has a larger, cleaner, more proper solution, propose it clearly, including tradeoffs, even if you also provide a small immediate fix.
- When implementing, prefer scoped, working milestones over broad rewrites unless the broader change is justified by the problem.
- Avoid pre-made game engines for the core simulation unless the user explicitly changes that direction.
- Favor simple, inspectable code while the simulation model is still being discovered.
- If extra tooling, debug views, tests, instrumentation, build scripts, profilers, visualizers, or project setup would make future debugging and fixes meaningfully better, feel free to propose it. Ask the user for approval before adding or installing that tooling.

## Technical Direction

- Keep the core simulation independent from rendering when practical.
- Prefer deterministic, debuggable physics steps.
- Avoid scripted decals or canned damage animations for core damage behavior.
- Use stress, constraints, fracture thresholds, topology changes, or particle state to drive visible outcomes.
- Preserve 60 FPS as a design constraint; reduce fidelity before accepting unstable frame rates.
- Add dependencies deliberately and document why they are needed.

## Git And Safety

- Do not revert user changes unless explicitly asked.
- Before large changes, inspect the current tree and understand what already exists.
- Keep generated artifacts, test screenshots, and temporary output out of git unless they are intentionally part of the project.
- If a change affects how to run the project, update `README.md`.
