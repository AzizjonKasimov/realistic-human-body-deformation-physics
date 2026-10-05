# Reference Assets

This folder stores the visual reference the procedural body is built from.

## `human_body_silhouette.svg`

- Source: https://commons.wikimedia.org/wiki/File:Human_body_silhouette.svg
- Original file: https://upload.wikimedia.org/wikipedia/commons/f/f4/Human_body_silhouette.svg
- Description on Commons: human body silhouette, front view.
- Author/derivative attribution on Commons: based on `Upper body front.png` by Mikael Haggstrom, transparent background by Frederic MICHEL, derivative work by RexxS.
- License: public domain / PD-self as stated on the Wikimedia Commons file page.

This is the body-shape source used by the simulation. `src/silhouette.rs` embeds the file at build time, parses its single outline path, and rasterizes it into a signed distance field in body-height units. Below the wrists, gaps narrower than the tissue mesh can resolve are closed, so the spread fingers and toes become mitten hands and solid feet. `src/simulation/body.rs` meshes the skin and muscle sheets to follow that outline and places the skeleton, organs, and vessels at landmarks measured on it.

Direction constraint: keep the generated body front-facing, adult, with arms hanging down and separated legs. If the outline file is replaced, keep a single closed path with absolute `M`/`C`/`L`/`Z` commands, and re-check the landmarks in `src/simulation/body.rs` with `.\tools\verify.ps1` (the anatomy diagnostic fails if bones leave the skin).
