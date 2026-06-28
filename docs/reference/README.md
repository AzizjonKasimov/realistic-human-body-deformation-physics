# Reference Assets

This folder stores visual references used to tune the procedural body silhouette.

## `human_body_silhouette.svg`

- Source: https://commons.wikimedia.org/wiki/File:Human_body_silhouette.svg
- Original file: https://upload.wikimedia.org/wikipedia/commons/f/f4/Human_body_silhouette.svg
- Description on Commons: human body silhouette, front view.
- Author/derivative attribution on Commons: based on `Upper body front.png` by Mikael Haggstrom, transparent background by Frederic MICHEL, derivative work by RexxS.
- License: public domain / PD-self as stated on the Wikimedia Commons file page.

The Rust body generator in `src/simulation.rs` uses this as the proportion reference for head width, shoulder span, torso taper, pelvis width, hanging arm placement, leg separation, and overall front-view silhouette readability.
