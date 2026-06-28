# Reference Assets

This folder stores visual references used to tune the procedural body silhouette.

## `pixel_human_silhouettes/front_adult_silhouette_41x96.mask`

- Primary body-shape reference used by `src/simulation.rs`.
- Custom original pixel mask based on the user-requested direction: slim adult silhouette, facing front, arms hanging down, long separated legs.
- Format: `#` means occupied body pixel, `.` means transparent pixel.
- Direction constraint: front-facing adult silhouette only. Do not use side, back, diagonal, or multi-direction sprite frames for the generated body silhouette.
- The watermarked sample image from the thread is visual direction only and is not copied into this repository.

The Rust body generator samples this mask for the outer skin layer and samples a horizontally inset version for the muscle layer. This keeps the sandbox body tied to a concrete 2D pixel-art front silhouette rather than a smooth vector outline.

## `human_body_silhouette.svg`

- Source: https://commons.wikimedia.org/wiki/File:Human_body_silhouette.svg
- Original file: https://upload.wikimedia.org/wikipedia/commons/f/f4/Human_body_silhouette.svg
- Description on Commons: human body silhouette, front view.
- Author/derivative attribution on Commons: based on `Upper body front.png` by Mikael Haggstrom, transparent background by Frederic MICHEL, derivative work by RexxS.
- License: public domain / PD-self as stated on the Wikimedia Commons file page.

This SVG is retained as an auxiliary front-view anatomy/proportion reference. It is not the primary pixel silhouette source for body generation.
