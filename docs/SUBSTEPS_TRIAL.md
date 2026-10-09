# Small steps trial

The `substeps` branch tries substepping, from Macklin et al., "Small Steps in Physics Simulation" (SCA 2019), which Rapier and Box2D v3 also build on. Instead of one 60 Hz step with 12 solver passes, the engine can split each step into several small steps with fewer passes each. Nothing here is on `main`: on this body it brings no measurable gain, and flesh meets tools differently, so every blow would need retuning.

## Trying it

- `strike_scenarios --solver 6x2 ...` and `realistic_physics.exe --solver 6x2 [--selftest]` run 6 small steps of 2 passes each. `1x12`, the default, is the tuned engine.
- With one small step the branch reproduces `main` exactly: `.\tools\compare.ps1 -Ref main` reports 0 of 20 scenarios changed, and `verify.ps1` passes.

## What changed

- `World::step` does the once-per-step work once (wounds, blood, organs and cavities, outline pairs, fading marks), and motion, tool motion and the solver passes in every small step.
- A point's `previous` is where it was one small step ago, so speeds use the small step's length. Code that moves or kicks the body once per step spreads that over the step (`spread_over_step`, `spread_kick`), and fragment tips sweep the whole step's path (`step_start`).
- Gravity, damping and the pull toward the rest shape are scaled to the small step, and XPBD compliance uses its length.
- The tool moves one small step at a time (`begin_tool_step`, `advance_tool`, `finish_tool_motion`, `finish_tool_substep`). Moving it a whole step at once threw the flesh it pushed at up to 11,000 px/s.
- A blunt tool's drag on flesh is a pull over time, and its contact depth and bone spin per sample count for the share of the step each sample stands for (`ToolSample`). Without this, a tool pressed into flesh paid for its drag and depth once per small step instead of once per step.

## Results

Measured on 2026-10-09 on the development PC, native release build, all 20 tuned scenarios (the self-test).

| Setting | Scenarios in band | Average step |
|---|---|---|
| 1x12 (`main`) | 20/20 | 2.50 ms |
| 12x1 | 15/20 | 2.57 ms |
| 6x2 | 17/20 | 2.55 ms |
| 4x3 | 16/20 | 2.58 ms |

Leftover tissue strain through `torso_heavy_high` (mean of |length / rest - 1| over every intact spring and frame):

| | 1x6 | 1x12 | 1x24 | 1x48 | 12x1 | 6x2 |
|---|---|---|---|---|---|---|
| Skin | 2.77% | 2.57% | 2.32% | 2.41% | 2.43% | 2.63% |
| Muscle | 1.96% | 1.81% | 1.62% | 1.60% | 1.66% | 1.91% |

## Why it does not help here

- **The solver already converges.** At this mesh's 11.5 px point spacing, 12 passes are nearly enough: doubling them lowers the strain by about a tenth, and 48 passes lower it no further. What is left is the balance between gravity, the pull toward the rest shape, and the area constraints, not solver error. 12x1 does about as well as 24 passes, as the paper predicts, but that is only about 5% less strain. Small steps pay off on fine meshes, where passes cannot keep up.
- **Flesh gets stiffer toward tools.** Stiffness here is the share of a constraint's error each pass removes, not a physical stiffness. With small steps, the solver pulls pushed flesh back every small step, and the tool pays to push it again. At 12x1 the sledgehammer stops in the chest's flesh (3,400 to 129 px/s within the impact step, against 549 on `main`) and breaks no ribs. At 6x2 it cannot follow the hand into a wound (`hammer_into_wound` lags 44 px) and turns too far when dragged (`hammer_drag_steady`, 105 degrees).
- **The fast knife bug stays.** A fast knife swing still breaks bone at every setting, and at 12x1 a fast thrust into the thigh breaks 15 bones.

## When to revisit

- When the body mesh gets finer and 12 passes no longer converge.
- Together with physical stiffness: XPBD compliance with damping, or stiffness as a frequency and damping ratio as in Box2D v3's soft step and Rapier. Then flesh behaves the same at any number of small steps, and the blows can be retuned once.
