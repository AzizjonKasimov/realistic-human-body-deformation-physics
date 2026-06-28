# Project Vision

This file preserves the original product description so the project intent stays visible as implementation details evolve.

## Original Description

I want to build a 2D physics sandbox that simulates realistic damage to a human body. It runs in a single window. On screen there's a humanoid figure made of three layers: outer skin, middle muscle, and inner bone. The user controls a fist or simple weapon with their mouse and uses it to strike the figure.

When the user hits the body, the physics responds realistically. Skin stretches and, with enough force, tears open along the strike - the cut should propagate naturally based on where stress concentrates, not appear as a pre-made decal. Muscle underneath deforms like soft tissue and becomes visible through the torn skin. Bones are rigid until impact force exceeds their strength, at which point they fracture and the broken pieces move independently. Blood flows from wounds as a particle-based fluid, pools on the ground, and stains surfaces.

The whole thing runs at 60 frames per second. There's no game around it - no menus beyond start and quit, no opponent, no goals, no scoring. It's a toy. The user pokes at the body, watches what happens, and that's the entire experience. Think of it like a digital version of a stress ball, except the stress ball is a person and the physics underneath it is real simulation rather than scripted animation.

The end product is a free standalone download that someone can install, play with for two minutes, and walk away thinking they've seen physics they've never seen in a game before.

## Agreed Direction

- Do not use a pre-made game engine for the core prototype.
- Use the Rust implementation as the main path, with `macroquad` for the cross-platform desktop renderer.
- Target Windows first while keeping the renderer and simulation portable enough for macOS.
- Start one system at a time, beginning with soft-body skin tearing.
- Prioritize real simulation behavior over scripted decals or canned animations.
- Aim as close as practical to real-life body destruction physics.
- Treat graphic injury detail as part of the simulation target: gore, exposed tissue, blood, tearing, fracture, and body deformation should be shown when the physics state supports them.
- Do not ask whether the app should be less gory by default; assume a darker, more realistic destruction model unless a later product decision explicitly says otherwise.
