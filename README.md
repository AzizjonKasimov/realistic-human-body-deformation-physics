# Realistic Physics

A no-engine 2D physics sandbox rewritten in Rust. The simulation is kept separate from the renderer so the physics model can be tested, tuned, and shipped on more than one platform, including the web.

**[Play it in your browser](https://azizjonkasimov.github.io/realistic-human-body-deformation-physics/)**: no install, and the simulation runs entirely on your own device.

See [docs/VISION.md](docs/VISION.md) for the original project description and agreed direction.
See [docs/DESTRUCTION_ARCHITECTURE.md](docs/DESTRUCTION_ARCHITECTURE.md) for the research-backed simulation architecture the Rust prototype is moving toward.

## Demo Videos

[![Watch the latest Realistic Physics video on YouTube](https://img.youtube.com/vi/YSuN8D9t4ZI/hqdefault.jpg)](https://www.youtube.com/watch?v=YSuN8D9t4ZI)

- [I Made My Body Physics Engine More Realistic With Claude Opus 5.5](https://www.youtube.com/watch?v=YSuN8D9t4ZI) (October 2026): damage that follows the speed of a blow, the mannequin body, and the browser version.
- [My Body Physics Engine Got a Big Update](https://youtube.com/shorts/L87wyjLrdNE) (October 2026): a 20-second Short rendered by the app itself.
- [Game Engine with Realistic Body Destruction Physics](https://www.youtube.com/watch?v=hCXVPSU6etE) (June 2026): the first version, written in C++ before the port to Rust.

## Current Rust Milestone

- Rust Cargo project with a reusable `realistic_physics` simulation library.
- Cross-platform `macroquad` app for windowing, input, and rendering, built as a desktop app and as a WebAssembly browser version that GitHub Pages hosts.
- On-screen control buttons that work with mouse and touch, so the browser version is playable on phones and tablets. The body is fitted between the status chips at the top and the buttons at the bottom in any window shape, so no control covers it, and it refits when the window changes until it is first hurt. The simulation steps at 60 Hz, but the app spreads each frame's pointer motion over its steps and draws the body, bones, and tool between the last two steps, so they move as smoothly as the screen refreshes, 120 or 144 Hz included.
- Verlet/PBD-style soft body points, springs, area constraints, and attachments.
- XPBD-style compliant spring and area-constraint projection is available behind material knobs, with focused tests covering compliant residual stretch/area behavior before production tuning raises those defaults.
- Front-view mannequin body: a neutral, exactly mirror-symmetric figure defined in code (egg-shaped head, smooth torso, tapered limbs, mitten hands, simple feet, legs apart). Skin and muscle sheets are meshed to follow its outline (outline points plus an interior hexagonal lattice, Delaunay-triangulated), with the muscle sheet inset just inside the skin, and the mesh mirrors across the midline too.
- Dynamic segmented bones built on the figure's own limb joints, so every limb bone runs down the middle of its limb, plus a low-resolution rib-cage proxy and hands and feet on wrist and ankle joints, attached to nearby muscle points, and connected by breakable bone joints.
- Bone joints can subluxate under traumatic stretch/overextension before full breakage, adding limited slack, weaker correction, and first-time local ligament/capsule tissue damage so dislocation exists between intact articulation and total separation.
- Post-fracture joint limits let broken or remapped limb joints sag and twist with slack instead of snapping rigidly or separating without bounds.
- Three hand-held tools whose drawings match their collision shapes exactly: a baseball bat (blunt; the barrel lies across the swing), a double-edged knife (sharp; the tip leads), and a sledgehammer (heavy; a striking face leads). Each tool is a solid object with momentum held firmly in the hand: it moves with the pointer, easing only into sudden starts and stops, and once it touches the body the arm presses it in harder the farther the pointer is past it, up to what an arm can push whatever the tool weighs (half that behind a broad bat or hammer face), while the body pushes back. A bat or sledgehammer is held by its handle and turns no faster than it could be swung round the hands: carried with the button up it stays upright in the hand, swung it turns until its striking part leads with the handle across the swing, and flesh holds it nearly in line, so it slides along the body instead of swinging round into it. A knife turns at once to lead with its tip, follows its stroke in tissue, and withdraws when pulled back instead of flipping around in the wound. Each step's motion is swept so fast swings cannot skip thin limbs. Each tool swings with one fixed strength, the way a person would swing it: the knife and the bat in one hand, the heavier sledgehammer with both.
- The knife moves only through fibers it severs. Each fiber its tip or cutting edge reaches is cut when the knife's momentum or the hand's push overcomes it; otherwise the knife stops against it, so a light push rests on the skin and pressing harder cuts in, and it slides along fibers and bone it cannot cut. Skin is the tough layer; the muscle under it parts readily. Bone stops the blade without breaking. A knife put down onto the body goes into the flesh there and cuts its way along. In this front view the spine lies behind the chest and belly, so it does not stop a blade cutting the front of the trunk; ribs, pelvis, collarbones, and limb bones do.
- Knife cuts open. The skin and muscle part along the mesh edges nearest the blade's path, whose points move onto the path so the cut runs straight, and each lip draws back from the cut, as skin and muscle under tension do, so the cut gapes and shows the muscle under cut skin, deep raw flesh inside cut muscle, and bone where the knife reached it. No tissue is lost: the mesh splits its points along the cut and every triangle stays whole. A cut just inside a limb's outline opens along the first points inside it and leaves a strip of skin hanging, and a cut too close to the outline to open still shows as a thin line along the blade's path. The technique is rebuilt from Rapier's soft-body cuts, which split points as Müller et al.'s position-based tearing does, and the node snapping of surgery simulators (Nienhuys and van der Stappen, 2001), which keeps cuts from zigzagging through the mesh.
- A bat or hammer gives its momentum to the tissue and bone it shoves, so it slows and stops in the body instead of sweeping through it, and the tissue keeps pushing back on it in every solver pass. Damage grows with the speed of the blow the way real blunt injuries do: bone breaks under the force of a blow, which grows with its momentum, so a tool merely resting on a bone does not load it, while flesh is crushed and bruised in step with the blow's energy, so half the speed does a quarter of the damage. A slow push or tap leaves at most a small bruise, a firm blow bruises the flesh around where it lands, and only a hard swing breaks bone; breaking a bone takes much of the blow's speed, so the tool does not plow on through the limb. A broad face spreads the force, so a bat rarely splits the skin, and an arm takes a blow aimed past it. A bat or hammer gripped while inside the body passes through until it is clear, so it cannot appear in flesh and blast it apart. Tools meet only what is drawn: flesh torn away (all of whose mesh triangles have failed or torn, which the app draws as a dark cavity or not at all) is not there for them, and neither is the spine, which lies behind the trunk, so a tool moves through a wound as freely as through the air and meets only its edges and the bone left in it. Loose bone fragments pass through torn-away flesh too. A blunt tool's motion is divided into steps of a quarter of the mesh spacing, and each point's load comes from how far the tool pressed it over the whole step, so a blow does the same damage however finely its motion is divided.
- Stress-based tearing from overstretched or high-impulse springs.
- Sharp cuts propagate from existing wound edges into adjacent stressed or fatigued skin springs under a per-step cap, so cuts can grow along local stress instead of appearing only as isolated threshold breaks; a tear that runs on from a cut opens as the cut does.
- Fresh knife cuts in the skin deepen into exposed or loaded muscle directly beneath them under a separate cap, so deep cuts follow the layered anatomy, while older wounds do not keep widening as the knife moves elsewhere.
- Sharp cut edges can delaminate nearby skin-to-muscle attachments under local load, creating capped skin-flap peeling and more physically driven exposure of the muscle layer.
- Fiber-aligned muscle springs now report separate rupture telemetry from cross-fiber muscle tears, with an opt-in damage-detail floor for later anisotropic tissue tuning.
- Soft-tissue springs accumulate persistent fatigue from repeated subcritical stretch/load, which lowers local tear thresholds and feeds muscle damage detail before full rupture.
- Damaged tissue can take a bounded permanent set during post-impact settling, so surviving springs keep small plastic stretch/crush deformation instead of always snapping back to their original rest shape; the default plastic rate is tuned through long-settle strike telemetry.
- Blunt and heavy contact now leaves persistent tissue contusion/crush state on impacted points, and contused springs locally soften and tear at lower load so repeated trauma changes material behavior instead of only tinting the surface.
- Failed muscle triangles can rupture into capped crush-bleeding sources, so heavy blunt/internal damage produces persistent fluid and wound evidence instead of only visual void shading.
- A low-resolution torso cavity pressure proxy groups internal muscle area constraints, builds bounded pressure/collapse state under deep compression, pushes back on surrounding tissue, and uses separate non-heavy pressure/load caps so medium blunt hits can bruise internal tissue without opening the capped internal rupture path reserved for heavier trauma.
- Anchored low-resolution organ proxies for lungs, liver, and spleen accumulate pressure/load/fragment damage and can rupture into capped internal bleeding when severe torso-cavity trauma supports it, when a sharp/deep striker explicitly penetrates an organ proxy, or when severe fractured-rib tip motion punctures a nearby organ proxy.
- Low-resolution major vessel paths follow nearby muscle anchors and can lacerate under deep sharp or heavy contact, feeding high-pressure wound sources rather than treating every wound as the same bleed. The arm arteries run down the inside of each arm between the bone and the skin, as the brachial artery does.
- Exposed muscle is a second mesh coupled to skin through breakable attachments, whose corrections leave no hidden motion behind, so a body left alone comes fully to rest.
- Parts of the body press against each other instead of passing through: the skin and muscle sheets are each bounded by one outline, and a point of it that is pushed behind another stretch of it (the inside of an arm driven against the chest, a hand against a thigh, one leg against the other) is pushed back out and pushes that stretch in. Only surfaces that face each other across a gap in the body's rest shape can touch, and flesh torn away no longer counts.
- Bones fracture at loaded contact points into recursive fragments, release nearby muscle-to-bone anchors, keep rotational inertia, and continue damaging tissue from broken ends. Most fractures stay closed: the bone ends tear the muscle right at the break and bruise the flesh around it while the skin and the bleeding stay inside. Only a break loaded far past the bone's strength throws its pieces apart, chips a splinter, and tears out through the flesh as an open, bleeding fracture.
- Moving splinter or broken-bone tips can puncture intact skin from inside the body under impulse, and severe fractured-rib tips can puncture organ proxies, with capped telemetry separate from generic fragment-tissue tearing.
- Severe moving fracture fragments can lacerate nearby major vessel paths through a capped swept-tip query, opening the same pressure-wound system while keeping fragment-driven vascular injury separate from direct tool cuts.
- Open fractures create bone-anchored marrow bleeding sources that follow the broken fragment and leak through the persistent wound system instead of being only a one-frame particle burst.
- Runtime budgets, broad-phase spatial filtering, and low-energy fragment sleeping cap active fragment work, fragment-bone checks, fragment-pair checks, fragment-tissue checks, vessel lacerations, wound sources, and fluid particles so heavier destruction remains PC-real-time.
- Broken fragments now collide with nearby intact bones, push against them, and transfer load back into the skeleton instead of only interacting with tissue and other fragments.
- Slow fragment-to-intact-bone overlaps and late-settle near contacts add damping, friction, and resting support so debris can jam against remaining skeleton instead of sliding through or rattling endlessly.
- Fragment-pair contacts damp closing velocity, tangential sliding, and angular jitter so piles of debris settle instead of endlessly rattling.
- Slow fragment-pair overlaps receive extra resting-contact support so settled debris resists tiny sinking/rattling under sustained load without raising global solver iterations.
- Free fragments use radius-aware floor contacts with damping, friction, angular drag, and resting-contact telemetry so debris can settle against the environment instead of being only center-clamped.
- Recursive fracture density is tuned through explicit material controls for maximum fracture depth, generic/rib-specific minimum fragment length, and secondary fragment strength, with deterministic strike gates covering long-settle debris.
- Fluid particles emit from tissue tears, attachment releases, and fractures, then fall, settle, and fade through the same simulation step. Blood leaves the body only where the skin is broken: under unbroken skin, torn muscle, broken bone, and ruptured organs bleed into the tissue and darken the bruise there. Fresh injuries throw out at most 15 drops a step, open wounds together leak at most 4, and blood leaves a wound at most at 560 px/s, so a wound wells up and drips rather than bursting into a cloud.
- Settled fluid particles now deposit capped, mergeable blood stains/pools on the environment so bleeding leaves persistent physical evidence instead of disappearing as particle fade only.
- Persistent wound sources anchor to nearby moving tissue or bone features, leak or briefly spray based on layer, depth, and pressure, clot down over time, and reopen when later local load, stretching of the tissue around the clot, a blunt blow landing on it, or fresh damage at the same spot disturbs it; a clotted source keeps its place until the wound budget needs the slot, so a healed cut can bleed again when struck later.
- Wound leakage drains a finite normalized blood reserve, and remaining reserve feeds back into wound pressure/leak strength and passive tissue turgor so long severe bleeding does not behave like an infinite source or fully supported tissue.
- Wounds are drawn from the mesh itself, with no overlay marks: a knife cut is a thin line along the blade's path that opens into a gap between dark lips, with deep raw flesh inside where it cut the muscle; skin torn by stretching shows a line through the torn springs that fades as the tear gapes; open skin shows the muscle beneath as solid flesh with a thin dark rim where intact skin meets the opening, torn-out muscle shows as a dark cavity, and bones lie beneath the flesh so they show only through openings or where a broken end sticks out.
- Visual damage diagnostics replay deterministic sharp and heavy strikes, then write a damage-focused SVG and primitive-count CSV so incision, wound-rim, exposed-muscle, lacerated-vessel, fluid, and fracture rendering can be inspected without launching the app.
- Rust diagnostics, strike scenario playback, and simulation tests.
- Smoothly shaded skin: one mesh with per-point color for load and bruising, soft shading strips inside the silhouette for volume, and a thin outline.
- Sound made from the simulation, with no recordings: after every step the game hears what happened and plays it. A bat or sledgehammer lands with a thud as loud and deep as the blow was fast, so a slow push barely makes a sound; a bone cracks as it breaks; flesh torn by a blow tears wetly; fresh blood spatters as it lands; air rushes past a swung tool, brighter as it goes faster; and the knife slices as long as it cuts. The sounds are synthesized when the app starts, the same way the project's videos get their sound, and `M` or the speaker button mutes them (see [Sound](#sound)).

## Realism Target

The project target is to get as close as practical to real-life body destruction physics. Graphic injury detail, visible gore, exposed anatomy, blood, tearing, fracture, and tissue deformation are intentional baseline behavior when they come from the simulation. Future renderer and simulation work should assume a darker, more physically explicit direction by default rather than asking whether gore should be reduced.

## Install Rust

From PowerShell:

```powershell
winget install Rustlang.Rustup
```

Restart PowerShell after installation so `cargo.exe` is on `PATH`, then confirm:

```powershell
cargo --version
```

The scripts under `tools\` need PowerShell 7 (`pwsh`). Windows PowerShell 5.1 stops on cargo's normal progress output.

## Run On Windows

Build the Rust app and copy the release executable to the repository root:

```powershell
.\tools\build_app.ps1
.\realistic_physics.exe
```

If the app is already open and the build cannot replace `realistic_physics.exe`, close it or run:

```powershell
.\tools\build_app.ps1 -StopRunningApp
```

You can also run directly through Cargo:

```powershell
cargo run --release --bin realistic_physics
```

## Run On macOS

The Rust app uses `macroquad`, so it is designed to build on macOS as well as Windows. macOS has not been verified from this Windows workspace yet.

On a Mac with Rust installed:

```bash
cargo run --release --bin realistic_physics
```

## Run In A Browser

The live version is at <https://azizjonkasimov.github.io/realistic-human-body-deformation-physics/>. The same Rust app compiles to WebAssembly and runs entirely in the visitor's browser, so the site is only static files and needs no server-side compute. [`.github/workflows/deploy-web.yml`](.github/workflows/deploy-web.yml) rebuilds and redeploys it on every push to `main`. If the build fails, the previous version stays live.

To build and try it locally, install the WebAssembly target once:

```powershell
rustup target add wasm32-unknown-unknown
```

Then build and serve it:

```powershell
.\tools\build_web.ps1 -Serve
```

Open <http://localhost:8080>, or <http://localhost:8080/?stats> to show the frame rate and how long each frame takes. <http://localhost:8080/?selftest> runs a self-test instead: the browser build plays every tuned strike scenario at fixed simulation steps, checks each against its injury bands, and reports how long a simulation step takes in WebAssembly, on screen, in the status line, and as `window.__selftest`. It does not depend on the frame rate, so it also works while the browser throttles a hidden or covered tab; it takes about 15 seconds. `realistic_physics.exe --selftest` runs the same natively and prints the report as JSON, so the two can be compared (they give the same injuries). `.\tools\serve_web.ps1` serves the last build again without rebuilding. The site is assembled in `target\web`: the page from `web\index.html`, the wasm module, the `gl.js` loader copied from the miniquad version in `Cargo.lock`, and the sound plugin `audio.js` from the patched quad-snd in `vendor\quad-snd`. Browsers start sound only after a click or tap, which the start button provides.

## Controls

- Left-drag to swing the selected tool into the body. The tool moves with your pointer like a tool held firmly in hand: swing from outside the body for a hard hit, or hold the tool against the body and keep dragging past it to press harder. The ring marks where your hand is. Damage comes from the tool's shape and weight, its speed when it lands, and how hard you press; it grows steeply with speed, so slow moves only bruise.
- `H`, `B`, and `S` select the sledgehammer (heavy), bat (blunt), and knife (sharp). The sledgehammer is in hand when the app starts.
- `D` toggles the contact debug overlay.
- `R` resets the body.
- `Space` pauses or resumes.
- `M`, or the speaker button, turns the sound off and on.
- The control chips along the bottom are also buttons: click or tap them for the same actions.
- On touch screens, drag a finger to strike. The chips grow to finger size after the first touch. A finger put down somewhere new takes the tool up there rather than flinging it across the screen, and a tool taken up on the body passes through it until clear.

## Verify

From the repository root in PowerShell:

```powershell
.\tools\verify.ps1
```

The Rust verifier runs formatting checks, simulation tests, deterministic strike playback and its offset sweep, anatomy diagnostics, and visual damage diagnostics, then prints how long each step took (about half a minute in all). The scenario and diagnostic programs run as release builds, which give the same results as debug builds several times faster. Options:

- `-BuildApp` also rebuilds the root `realistic_physics.exe` (`-StopRunningApp` closes a running copy that blocks it).
- `-Capture` also saves real app screenshots of the body at rest at 1280x720, 800x600, and 390x844 (see [Screenshots](#screenshots)).
- `-SoundCheck` also runs `.\tools\sound_check.ps1` on the root `realistic_physics.exe`, which checks that its sound reaches the speakers (see [Sound](#sound)).
- `-SkipSweep` and `-SkipDiagnostics` leave out the sweep and the two diagnostics.

The tests include the patched quad-snd's own mixer tests (`vendor\quad-snd`).

## Compare A Change

To see how a change moves the simulation, compare the working tree with a commit (by default `HEAD`, so it shows what uncommitted changes do):

```powershell
.\tools\compare.ps1
.\tools\compare.ps1 -Ref HEAD~3 -Sweep
.\tools\compare.ps1 -Capture torso_heavy_high
```

It plays the strike scenarios and the visual damage and anatomy diagnostics on both sides and lists every changed number per scenario, tuning warnings that appeared or went away, and changed mesh counts. The commit is built in a git worktree under `target\compare` and its results are cached by commit, so repeated comparisons only replay the working tree. `-Sweep` also compares how often each scenario stays in band when its swing moves slightly, and `-Capture NAME` puts app screenshots of that scenario from both sides into one contact sheet; both need a commit that already has those features. The full report is `output\compare\report.txt`, with each side's raw outputs in `output\compare\base` and `output\compare\current`. `-Clean` removes the cached worktrees and results.

## Screenshots

The app can play a scripted strike and save the screen without anyone at the keyboard:

```powershell
.\tools\capture.ps1
.\tools\capture.ps1 -Scenario torso_heavy_high -Sizes 1280x720,800x600,390x844
.\tools\capture.ps1 -Strike "hammer:-0.26,0.34:0.30,0.34:frames=14" -NoUi
.\tools\capture.ps1 -Gesture "bat:-0.3,0.3:wait=10:down:-0.08,0.3/15:-0.08,0.6/60:up:wait=20" -Every 5 -View normal
```

Each capture briefly opens the app window at the given size, plays the strike at fixed steps, saves the screen as PNG files in `output\captures`, and closes; several captures are also laid out in one contact sheet (`NAME-sheet.png`). Besides the view players see, a capture can draw an anatomy view with see-through skin that shows the muscle, vessels, and bones; it exists only for checking, and the app itself always shows the one normal view. `-View normal|anatomy|both` picks the views (both by default), `-Frames N` stops after N steps, `-Every N` also saves the screen every N steps as a film strip of how things move (laid out in the sheet), and `-NoUi` and `-NoLabel` leave out the HUD and control buttons and the name and step label, for clean images. `-Gesture` plays a gesture instead of a swing (see [Strike Scenarios](#strike-scenarios)). The script drives `realistic_physics.exe --capture OUT.png` (options `--size`, `--view`, `--scenario`, `--strike`, `--gesture`, `--frames`, `--every`, `--no-ui`, `--label`, `--no-label`, `--body-frame`, `--events`), and `contact_sheet.exe OUT.png [--height H] [--columns N] IN.png...` makes the sheet.

Captures can also become videos: `--every 1 --no-ui --no-label` saves every 60 Hz step as a clean frame. Injuries depend on the body's size in pixels, because tools keep their pixel size, and a tall window draws the body larger than the 561.6 px the tuned scenarios use (its height in a 1280x720 window). `--body-frame X,Y,H` puts the top of the head at `X,Y` and makes the body `H` pixels tall, so `--size 576x1024 --body-frame 288,240,561.6` films a vertical video whose swings do what the scenarios check, with room above the head for captions. `--events FILE` writes what happens at each step as CSV (the tool, its speed and impact, whether it touches the body, and running counts of broken bones, torn skin and muscle, bruises, and blood drops), so a video's sound can be made from the same run as its picture.

## Strike Scenarios

`.\tools\verify.ps1` builds and runs deterministic strike playback across representative torso, shoulder, arm, hip, and leg strikes with blunt, sharp, and heavy tools, plus gestures that check how steadily a tool moves in hand. The scenarios live in `src/scenarios.rs`, where the strike runner, the visual damage diagnostic, and the app's capture mode all play them from. Each scripted swing moves the hand from outside the body and holds briefly at the end with the button down, so the tool, which trails the hand, lands as a real swing would. A gesture instead plays the tool the way the app does: the tool is in hand from the start and hovers with the button up, and the hand moves, presses, holds, and lets go on cue. The scenario target writes frame-by-frame contact telemetry to:

```text
output\strike_scenarios.csv
```

It also writes a compact per-scenario tuning summary to:

```text
output\strike_summary.csv
```

It also writes a warning-only tuning report that compares each scenario against expected damage and steadiness bands:

```text
output\strike_tuning_report.txt
```

The CSV outputs include region, intent, tool mode, striker speed, impact, contact counts, contact depth, tissue/bone loads, sharp cut propagation counts, skin-to-muscle cut transfer counts, sharp skin-flap delamination counts, fiber-aligned muscle tear counts, muscle crush-rupture counts, torso cavity pressure/collapse/rupture counts, organ damage/direct-penetration/rib-puncture/rupture counts, direct and fragment-driven major vessel laceration counts, soft-tissue contusion counts, local tissue-softening maxima, spring-fatigue events and local fatigue maxima, plastic deformation events and local plasticity maxima, joint subluxation/breakage, ligament/capsule damage events, fracture events, rib-fracture counts, fracture marrow-source counts, post-fracture joint limit corrections, wound counts, wound reopen counts, wound pressure/clotting, blood loss, final blood reserve, final blood-turgor scale, blood stain/pool deposits, broken-end tissue contacts, inside-out skin puncture counts, fragment-bone contacts/damping/resting support, fragment-pair contacts/damping/resting support, fragment-floor contacts/resting support, overlap depth, fragment angular speed, free/spinning/sleeping fragment counts, runtime budget checks/skips/replacements, fluid emission, final fragment counts, and accumulated damage stats. The bands gate realistic injury for each tool: hard bat swings into the arm, shoulder, and leg must bruise widely while tearing little skin, cutting no major vessel, and breaking at most an arm; a full-force sledgehammer swing (as fast as the hammer goes, about 3400 px/s) into the chest must break the arm, often a rib too, and bruise deeply without pulping the chest, while the thigh's femur holds against it; knife cuts across the belly and down the inside of the arm must leave an incision through skin and muscle that opens, with deep cut transfer, lift skin flaps on the belly and open an artery in the arm, and break no bone at all, so the knife cannot regress into a club; `thigh_cut_rebleed` lets a knife cut down the thigh clot and then requires a bat blow to make it bleed again; and `torso_heavy_fragment_settle` waits after the sledgehammer blow so fragment contact, resting support, and sleep behavior are covered. Every scenario also checks that the tool never snaps round more than 45 degrees in one step and that no tissue flies off faster than 4000 px/s. The gesture scenarios hold a bat or hammer to more: `bat_carried_steady` carries the bat across the body and back with the button up, where it must stay upright; `bat_drag_steady` and `hammer_drag_steady` press the tool against the side of the chest and drag it down the body, and `bat_swing_back_steady` swings the bat across the chest and back, where the tool may turn at most 16 degrees a step and 45 degrees in all. A speed ladder holds damage to the speed of the blow: `hammer_slow_push`, `hammer_moderate_swing`, and `hammer_firm_swing` push the sledgehammer into the side of the chest at about 400, 800, and 1600 px/s, where a slow push must do no harm, a moderate swing only bruise, and a firm one bruise deeply and at most break the arm without anything bursting open, and `bat_firm_swing` must bruise without breaking bone. `hammer_finger_lands` and `hammer_first_touch` play the way a phone is used: a finger put down on the chest, far from where the hammer rested or on a body nothing has touched yet, must do no harm at all. `hammer_into_wound` smashes a wound open with four full-force blows, then drags the sledgehammer slowly into it and holds it there, where it must end up within 10 px of the hand: flesh torn away must not hold a tool back.

A strike's outcome can swing a lot with a few pixels of aim, because a fast tool moves about one mesh spacing per step. So the verifier also replays every scenario with its swing moved by up to 0.01 body heights along its path and 0.005 across it (gestures move 0.01 across the body and 0.005 down it; 15 runs each, on all cores) and writes how often each one stays in band, with the spread of its main injuries and of how steady the tool was, to:

```text
output\strike_sweep_report.txt
output\strike_sweep.csv
```

The runner also takes options for quick experiments:

```powershell
cargo run --release --bin strike_scenarios -- --list
cargo run --release --bin strike_scenarios -- --only torso_heavy_high --sweep
cargo run --release --bin strike_scenarios -- --strike "knife:-0.08,0.38:0.07,0.48"
cargo run --release --bin strike_scenarios -- --gesture "bat:-0.35,0.33:wait=10:down:0,0.33/12:wait=60:up:wait=30"
```

`--list` prints every scenario's swing, `--only` limits a run or sweep to some scenarios, and `--strike TOOL:U0,V0:U1,V1[:power=P][:frames=N][:windup=N][:settle=N]` plays one custom swing in body coordinates (fractions of body height from the top of the head on the midline) and prints its injuries, or their spread with `--sweep`. Tools are `bat`, `knife`, and `hammer`, and each swings with its strength in the app unless `power=P` tries another. `--gesture TOOL:U,V[:STEP...]` plays a gesture the way the app does: the hand starts at `U,V`, and the steps are `U,V/N` to move there over N steps (`U,V` jumps), `down` and `up` for the button, and `wait=N`. A custom swing or gesture prints its injuries, how the skin opened (fibers a blade cut, fibers torn, and the lips of cuts that opened), for a knife how well its cut follows the blade (the blade's path on the steps it cut fibers, the slit's length in the rest shape against it, how far the slit runs from it, and how wide the cut gapes in skin and muscle), and how steady the tool was (its largest turn in one step, its turning in all, snaps, how often a held tool lost and found the body again, how far it trailed the hand in the air and at the end, and the fastest tissue), and writes its frame-by-frame telemetry, including the tool's angle and position and what its striking part reaches (points of flesh, points of flesh torn away, bones, and the spine), to `output\strike_custom_frames.csv`.

## Sound

The game's sound comes from the simulation, with no recordings or samples. `src/sound.rs` listens to the world after every step and reads what the video foley reads from `--events`: the tool's speed and whether it touches the body, and the running counts of broken bones, torn skin and muscle, and blood drops. From those it hears:

- a thud when a bat or sledgehammer lands, as loud and deep as the blow was fast (silent below 150 px/s, full strength at 2550 px/s), so the speed ladder sounds like one: a slow push is a faint tap and a full swing a heavy blow;
- a crack when a bone breaks, a wet tear when a blow tears skin or muscle, and a spatter when fresh blood lands (a wound's slow leak stays quiet);
- air rushing past a tool swung with the button down, louder and brighter as it goes faster, thinner for the knife;
- a slice while the knife cuts, as loud as the fibers it severs each step.

A sound plays no sooner than 60 to 80 ms after the last of its kind, unless it is much stronger, so a crush that breaks bone after bone does not stack into noise. `src/sound/synth.rs` synthesizes the 26 clips (three takes of each one-off sound, the thud at three strengths) in about 40 ms when the app starts, rebuilt from `video_tools\tools\foley.py`, which makes the sound of the project's videos, so the game and the videos sound alike; the air and the knife are loops whose volume follows the action. Everything plays through macroquad's audio, natively and in the browser, and a soft clip rounds off peaks when several loud sounds land at once. Sound only reads the world, so it never changes what the simulation does.

To hear a scenario without playing it, or check how loud things are:

```powershell
cargo run --release --bin strike_scenarios -- --sound output\sound
cargo run --release --bin strike_scenarios -- --only hammer_firm_swing --sound output\sound
cargo run --release --bin strike_scenarios -- --gesture "knife:-0.3,0.66:wait=10:down:0.1,0.66/8:wait=40:up" --sound output\sound
```

`--sound DIR` plays the scenarios (or a custom swing or gesture) and mixes what the app would play, step by step, into `DIR\NAME.wav`, writes each clip of the sound bank to `DIR\bank`, and lists every scenario's sounds, strongest strength, and levels in `DIR\sound_report.txt`. A full sledgehammer blow reaches about -16 LUFS over its loudest 400 ms, a knife cut -22, and a slow push -37.

To check that the desktop app's sound really reaches the speakers:

```powershell
.\tools\sound_check.ps1
```

It opens `realistic_physics.exe` for a few seconds, presses `H` and swings the sledgehammer by sending messages to the app's window (the real mouse and keyboard are left alone), mutes and swings again, and reads the app's own level meter in the Windows mixer the whole time. It passes when the app is silent at rest, clicks, thuds, and is silent again while muted.

macroquad's sound backend, quad-snd, is used from a patched copy in `vendor\quad-snd` (MIT; `Cargo.toml` points Cargo at it). The patches, listed in `vendor\quad-snd\PATCHES.md`, cut the Windows output latency from 83-93 ms to 13-23 ms, glide volume changes so they do not click, fix a glitch at every loop point, add the soft clip, and make browser audio start on the first tap on phones too. Its mixer tests run with `cargo test --manifest-path vendor\quad-snd\Cargo.toml --target-dir target\vendor`.

## Anatomy Diagnostics

Use this whenever changing body generation, anatomy layers, bones, constraints, or rendering assumptions:

```powershell
.\tools\verify.ps1
```

Open `output\anatomy_debug.svg` to inspect the generated body without launching the app. Skin is translucent, muscle is red, major vessels are dark red, bones are pale, ribs are slightly warmer, muscle-to-bone attachments are blue, bone joints are yellow, and bone sample markers turn red if they fall outside the skin mesh. The diagnostic exits nonzero if sampled bone centerlines are outside skin.

## Visual Damage Diagnostics

Use this whenever changing damage rendering, wound detail, fluid appearance, fracture display, or visual assumptions:

```powershell
.\tools\verify.ps1
```

Open `output\damage_visual_debug.svg` to inspect two deterministic damage captures side by side: a knife cut on the belly, which must leave every bone intact, and a long-settle sledgehammer blow to the chest. The diagnostic also writes primitive counts to:

```text
output\damage_visual_summary.csv
```

The visual diagnostic exits nonzero if the captures no longer include expected wound edges, incision lines, failed-muscle voids, contusion discoloration, direct/fragment lacerated vessel evidence, wound sources, visible fluid particles, finite blood-loss, turgor, cavity-pressure, organ-penetration/rib-puncture/injury metrics, blood stain pools, fractured bones, rib-fracture evidence, fracture caps, or overall damage primitives.

## Development Notes

- `Cargo.toml` defines the Rust library, app, diagnostics, strike scenario, and contact sheet binaries.
- `src/simulation.rs` contains the physics data model, integration, constraints, tearing, bone fracture, major vessels, wounds, and fluid particles; `src/simulation/body.rs` generates the layered body; `src/simulation/tools.rs` holds the tools: their shapes, the hand that drives them, and their contact with tissue and bone; `src/simulation/cuts.rs` opens the cuts a blade makes by splitting the tissue's points along them.
- `src/bin/realistic_physics/main.rs` owns the `macroquad` app shell, input, timing, and rendering. It also runs in the browser, so the app and simulation must avoid file I/O, threads, and `std::time`, none of which work on `wasm32-unknown-unknown`. `src/bin/realistic_physics/capture.rs` is the native-only screenshot mode, and `src/bin/realistic_physics/selftest.rs` the self-test mode, which runs in both builds (the page hands it a clock and takes its report through a small gl.js plugin).
- `src/scenarios.rs` holds the scripted strikes and gestures and the tuned scenarios with their injury and steadiness bands, shared by the strike runner, the visual damage diagnostic, the capture mode, and the self-test, which also ships in the browser build.
- `src/sound.rs` hears what each simulation step sounded like and picks the clip and volume for each sound; `src/sound/synth.rs` synthesizes the clips and `src/sound/mixdown.rs` mixes a run offline the way the app plays it. `src/bin/realistic_physics/audio.rs` plays the sound in the app, and `tests/sound_tests.rs` checks it. `vendor/quad-snd` is the patched sound backend.
- `web/index.html` is the browser page: start screen with a content warning, loader, error messages, and the `?stats` frame-time overlay (also exposed as `window.__perf` for automated checks).
- `tools/build_web.ps1` builds the browser version into `target\web`; `tools/serve_web.ps1` serves it locally with the `application/wasm` content type browsers require.
- `.cargo/config.toml` lets the wasm linker leave miniquad's WebGL functions as imports for `gl.js` to provide; recent Rust versions no longer do that by default.
- `src/bin/anatomy_diagnostics.rs` writes a deterministic SVG anatomy snapshot and reports geometry validation metrics.
- `src/bin/strike_scenarios.rs` writes deterministic strike telemetry and tuning summaries, sweeps scenarios over small swing offsets, and plays custom swings.
- `src/bin/visual_damage_diagnostics.rs` writes deterministic SVG damage captures and visual primitive metrics.
- `src/bin/contact_sheet.rs` lays PNG screenshots out in one image.
- `tools/verify.ps1`, `tools/compare.ps1`, `tools/capture.ps1`, and `tools/sound_check.ps1` are the checking workflows described above.
- `tests/simulation_tests.rs` contains focused Rust simulation checks, including mirror symmetry of the body and skin covering the muscle at rest in several window sizes.
- `src/silhouette.rs` defines the mannequin figure (torso outline, limb joints and radii, head, hands, feet) for one side and mirrors it into a signed distance field; `src/simulation/body.rs` meshes the body from it, builds the limb bones on the same joints, and places the rest of the anatomy. `body_frame` maps body coordinates (fractions of body height) to the window, which the strike scenarios use to aim at anatomy; the app moves and shrinks the body with `body_frame_between` when that placement would run under its status chips or control buttons.

The next Rust simulation milestones are:

1. Expand the Rust test matrix to cover more strike and fracture edge cases.
2. Raise tissue spring/area compliance from the current neutral defaults in measured XPBD tuning passes, using strike telemetry to keep fracture, cavity, vessel, and debris behavior separated.
3. Tighten strike tuning bands once material behavior has settled enough for intentional regression gates.
4. Keep increasing fracture density in small measured steps, using the long-settle telemetry to catch fragment sleep, budget, and stability regressions.
5. Verify the `macroquad` app on macOS and document any platform-specific packaging steps.
6. Add a pixel-level comparison on top of the app captures, so rendering regressions against a baseline commit show up without looking at the images.

## Toolchain

The primary build now uses Rust and Cargo. The app frontend uses `macroquad` for a cross-platform window, input, sound, and 2D drawing path while the simulation stays in a reusable Rust library. The browser version is the same app compiled for `wasm32-unknown-unknown`, drawn with WebGL and heard through Web Audio.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE).
