# quad-snd, patched for realistic_physics

This is [quad-snd](https://crates.io/crates/quad-snd) 0.2.8 by Fedor Logachev,
the sound backend of macroquad's `audio` feature, copied from crates.io under
its MIT license (the crate is dual MIT/Apache-2.0; see `LICENSE-MIT`). The
project's `Cargo.toml` points Cargo at this copy with `[patch.crates-io]`, and
`tools/build_web.ps1` ships this copy's `js/audio.js` with the browser build.
quad-snd's own README suggests forking it this way.

Changes from 0.2.8, each marked `realistic_physics patch` in the code:

- `src/wasapi_snd.rs`: a 1024-frame buffer instead of 4096. The audio thread
  refills the whole buffer on every device period, so the buffer is the
  latency: measured on the development PC, 4096 frames kept 83-93 ms queued,
  which made hits sound late; Windows rounds 1024 up to its minimum (1036
  frames there), which keeps 13-23 ms queued.
- `src/mixer.rs` (every native backend): a volume change glides in over one
  buffer instead of jumping, so volumes set every frame do not click; a
  looped sound that wraps carries on where it left off in the buffer (it
  restarted at the buffer's beginning, which glitched at every loop point);
  and the finished mix goes through a soft clip, so peaks above 0.8 bend
  toward 1 along a tanh instead of clipping hard when a thud, a crack, and a
  tear land at once.
- `js/audio.js` (browser):
  - every sound goes through the same soft clip (a wave shaper) on its way
    to the speakers;
  - audio unlocks on every gesture (`touchend`, `pointerup`, `mousedown`,
    `keydown`, `click`) until the context really runs, instead of giving up
    after the first event, which on a phone is a `touchstart` that browsers
    do not count as a gesture;
  - the context suspends while the tab is hidden, since frames stop there and
    looped sounds would keep their last volume;
  - volume changes glide over about 10 ms (`setTargetAtTime`), and a recycled
    gain node drops what it was still gliding toward;
  - stopping a sound stops its source node, which a looping source otherwise
    keeps running unheard;
  - the second throwaway `AudioContext` and the `console.log("fix")` are gone.

To update to a newer quad-snd, copy its `src`, `js`, `build.rs`, and original
`Cargo.toml` (`Cargo.toml.orig` in the crates.io package) over these files and
apply the changes above again.
