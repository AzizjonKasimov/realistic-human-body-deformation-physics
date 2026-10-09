# Injury reference

The strike scenarios (`src/scenarios.rs`) check what the simulation does against what blows, cuts, and bleeding do to real people, from published biomechanics, forensic, and medical research. This file holds that research. Each scenario's `real` list names the outcome, how often it happens in real life, and the source; the rules every scenario follows are under [Rules for every play](#rules-for-every-play).

Figures marked *checked* were read in the source itself while writing these tests. The others come from the research summaries behind them and were not re-read.

## Scale

- The figure stands for an adult 1.75 m tall, weighing about 70 kg. In the scenarios' 1280x720 window it is 561.6 px tall, so one pixel is about 3.1 mm and 1,000 px/s is about 3.1 m/s (`scenario_metres_per_px`).
- Blood: about 70 ml per kg of body weight, so about 4.9 L (the trauma-care figure, as cited in Gutierrez et al. 2004). The engine's blood loss is a share of that volume (`BLOOD_VOLUME_ML`).
- Tools: a 4.5 kg (10 lb) sledgehammer head and a 0.8 kg bat (the 0.79 kg, 0.71 m bat in Guo et al. 2026). The speed of a tool is the speed of its striking part: the hammer's head, the bat's barrel, the knife's blade.
- Top speeds in the engine (`max_speed` in `src/simulation/tools.rs`): a knife 4,800 px/s (about 15 m/s, the fastest slash measured), a sledgehammer 3,400 px/s (about 10.6 m/s), and a bat's barrel 8,000 px/s (about 25 m/s, between an ordinary man's swing and a trained hitter's).

## How hard people strike

| What | Figure | Source |
|---|---|---|
| Hard bat swing | Ordinary men: the bat's centre of mass at 15.5 m/s, 112 J; women 11.0 m/s, 56 J (36 adults aged 18-30, two hands) | Guo et al. 2026, ["Biomechanical effects of sex and bat size on head-directed blunt strikes"](https://www.frontiersin.org/journals/bioengineering-and-biotechnology/articles/10.3389/fbioe.2026.1797941/full), Table 3. *Checked.* |
| Trained hitters | Skilled adult batters swing the bat at about 30 m/s at contact | Escamilla et al. 2009, ["Age level and hitting kinematics"](https://doi.org/10.1123/jab.25.3.210). *Checked (abstract).* |
| Strikes with rods | Men 14.0-35.5 m/s, women 10.4-28.3 m/s at the peak | Trinh et al. 2018, ["Maximum striking velocities in strikes with steel rods"](https://doi.org/10.1007/s00414-017-1734-z). *Checked.* |
| Energy of a strike | Average 67-312 J for men, 30-203 J for women, by the object swung | Sprenger et al. 2016, ["The influence of striking object characteristics on the impact energy"](https://doi.org/10.1007/s00414-015-1268-1). *Checked.* |
| Knife slash | Up to 14.9 m/s (95th percentile 9.9) and 212 N | Bleetman et al. 2003, ["Wounding patterns and human performance in knife attacks"](https://doi.org/10.1016/j.jcfm.2003.09.005). *Checked.* |
| Sledgehammer | No study measured a sledgehammer swing. The scenarios take a hard swing as about 10 m/s, inside the energies people deliver | - |

## Bones

Most fracture tests hold a bone at both ends and strike it in the middle, so a free limb, which can move away from the blow, may need somewhat more.

| Bone | Breaking load | Source |
|---|---|---|
| Forearm | A 4.5 kg weight at about 3 m/s (about 20 J) broke cadaver forearms at 1.37 kN and 89 N·m; 50% risk at 100 N·m for an average man | Begeman and Pratima 1999, ["Bending strength of the human cadaveric forearm due to lateral loads"](https://saemobilus.sae.org/content/99SC24/); Pintar et al. 1998 (mean 94 N·m, [SAE 983149](https://saemobilus.sae.org/content/983149/)); Santago et al. 2008, forearm risk function. *Checked.* |
| Upper arm | 50% risk at 257 N·m for an average man, 2.6 times the forearm | Santago et al. 2008, humerus risk function. *Checked.* |
| Collarbone | 732 ± 175 N in dynamic bending, about half the forearm's force | Kemper et al. 2006, "Biomechanical response of the human clavicle subjected to dynamic bending". *Checked.* |
| Thigh | A 9.8 kg weight at 5 m/s (about 123 J) broke all 45 thigh bones tested, at 4.2-4.8 kN; 50% risk at 395 N·m for an average man | Kennedy et al. 2004, ["Lateral and posterior dynamic bending of the mid-shaft femur"](https://doi.org/10.4271/2004-22-0002). *Checked.* |
| Shin | About 280 N·m for women and 320 for men, whichever way it is struck | Nyquist et al. 1985, ["Tibia bending: strength and response"](https://saemobilus.sae.org/content/851728/). *Checked.* |
| Ribs | 50% risk of a broken rib once the chest is pressed in 35% at age 30, 13% at age 70 | Kent and Patrie 2005, "Chest deflection tolerance to blunt anterior loading is sensitive to age". *Checked.* |
| Spread | One standard deviation is 15-28% of the mean in these studies; bone size matters most | The studies above |

Bats break the forearm most: of 146 fractures in 116 people struck with bats, 61 were of the ulna, 27 of the hand, and 14 of the radius, and 20% were open (Bryant et al. 1992, ["Musculoskeletal trauma: the baseball bat"](https://pmc.ncbi.nlm.nih.gov/articles/PMC2571736/), *checked*). Bat blows also break the shin (Levy et al. 1994, 11 cases, *checked*).

**Energy estimates.** The scenarios compare a blow's energy with each bone's limit. For a given contact, the peak force of a blow grows with the square root of its energy, so a bone that takes k times a forearm's breaking force takes about k² times its energy. With the forearm at about 20 J and each bone's force from its bending limit over its length, that gives roughly: collarbone 5 J, forearm 20 J, upper arm 90 J, shin 100 J, thigh 120 J. These are estimates for comparing blows, not measurements.

**How often a blow breaks a bone.** A scenario's check follows its blow's energy against the bone's estimate: 4 times or more, always; 2-4 times, usually; 0.5-2 times, sometimes; 0.25-0.5 times, rarely; below that, never. The bands are wide because the estimates are rough and bones differ by 15-28% from person to person.

**In the engine.** Each bone is as strong as the bending tests make it against the forearm (`bone_strength` in `src/simulation/body.rs`): the collarbone about half, the upper arm 2.1 times, the shin 2.25, the thigh 2.5. The forearm is set 30% above its share, inside the spread of the forearm studies, so a light bat swing rarely breaks it. How hard each tool's blow bears toward a break (`break_scale`) is set so each tool breaks bones as often as blows of its energy do. Only blows, pieces of broken bone, and flesh pressed onto a bone break it; what a joint passes on strains the joint but breaks no bone.

**Where a bone breaks.** A bone breaks where a load bends it most, and a load bends a bone by its lever on what holds the bone, not by its force alone. In the tests above the bone rests on two supports and is struck between them, which bends it by F a b / L under the blow, with a and b the blow's distances from the supports and L the span: most in the middle, so the thigh bones of Kennedy et al., struck in the middle, all broke there, under the impactor. In the body a long bone rests on its joints the same way, so a blow a quarter of the way along a bone bends it 0.75 times as hard as the same blow in the middle, and one a tenth of the way 0.36 times; a blow pressed along a stretch of the bone, as a bat lying along a forearm, bends it less than one at a point, half as hard when spread over the whole span. Past a bone's last joint, as a hand's fingers, only the bone's own inertia holds it, so a blow there turns it more than it bends it. A bending break can also break a wedge out of the bone, a butterfly fragment: classically its broad side lies where the blow landed (Messerer's wedge), though bending tests find wedges in only some breaks, on either side (Isa et al. 2021).

**In the engine.** Every load that can break a bone counts by how hard it bends the bone where it lands, against the same load at one point in the middle of a span between two joints, and the bone breaks where it was bent hardest (`src/simulation/bending.rs`). A break twice the bone's strength or more also breaks a wedge out of the struck side. Since most blows land off the middle of a bone or spread along it, each tool's `break_scale` was set again against the checks with this in place (the bat's from 1.54 to 2.47, the sledgehammer's from 2.57 to 3.08). The ends of bones beside a joint, which real blows there can break (the top of the upper arm, the outer collarbone), are not modelled: a blow right at a joint loads the joint rather than breaking a shaft.

**Open fractures.** How often a break comes out through the skin depends on the weapon:

| Weapon | Open | Source |
|---|---|---|
| Bat | A fifth of 146 fractures | Bryant et al. 1992 (above). *Checked.* |
| Sticks and clubs | 44% of 64 fractures in 28 people | Nolan et al. 2000, ["The price of peace"](https://pubmed.ncbi.nlm.nih.gov/10716049/). *Checked (abstract).* |
| Iron bars or similar | 70% of 18 people had an open fracture (a share of people, not of fractures) | Eames et al. 1997, ["A fractured peace"](https://pubmed.ncbi.nlm.nih.gov/9326144/). *Checked (abstract).* |

In the engine a break comes out open only when the blow goes far enough past the bone's strength, by tool (`open_fracture_scale`): furthest for a bat's smooth, rounded barrel, less for a sledgehammer. The pieces of a closed break do not cut the skin from inside; those of an open break, and splinters, do.

**Knives.** Pocket knives pierce ribs at 11-16 J (above 0.9-1.2 kN), and a hard stab carries 64-115 J (Bolliger et al. 2016, ["Stabbing energy and force required for pocket-knives to pierce ribs"](https://doi.org/10.1007/s12024-016-9803-z), *checked*; Horsfall et al. 1999). No study found an ordinary knife breaking a long bone; heavy chopping blades do (Lynn and Fairgrieve 2009, axes and hatchets; Gentile et al. 2019, about 15 J for an axe on a forearm-sized bone).

## Flesh and skin

| What | Figure | Source |
|---|---|---|
| Bruising | Half of blows bruise at 3.2 J (5.9 N·s), living limbs, judged a day later | Desmoulin and Anderson 2007, "Contusion mechanics: a minimum tolerance test" (pilot study). *Checked.* |
| A heavy, slow blow | 4.2-7.2 kg dropped 67 cm onto a tensed thigh (up to about 47 J and 1.7 kN): swelling, no significant direct muscle damage | Barnes et al. 2022, ["An experimental model of contusion injury in humans"](https://doi.org/10.1371/journal.pone.0277765). *Checked.* |
| Skin splitting under a blow | At least 4,000 N to split the scalp over the skull; blunt blows split skin trapped against bone | Sharkey et al. 2012, "Investigation of the force associated with the formation of lacerations and skull fractures". *Checked.* |
| Skin strength | 21.6 ± 8.4 MPa, failing at 54 ± 17% stretch | Ní Annaidh et al. 2012, ["Characterising the anisotropic mechanical properties of excised human skin"](https://arxiv.org/abs/1302.3022). *Checked.* |
| Muscle strength | Fails across its fibers at about 15% stretch | Takaza et al. 2013, ["The anisotropic mechanical behaviour of passive skeletal muscle tissue"](https://doi.org/10.1016/j.jmbbm.2012.09.001) |
| Knife through skin | Skin gives way at roughly 10-55 N, far below the force of a slash | Ní Annaidh et al. 2013; O'Callaghan 1999 |
| Slash depth | Most facial slash and stab injuries were superficial; 16 of 214 people had a significant vessel injury | Steel et al. 2021, ["A 10-year study of penetrating head and neck injury"](https://pmc.ncbi.nlm.nih.gov/articles/PMC8215095/). *Checked.* |
| Cuts gape | Skin is under tension, so a cut opens as it is released | Wu et al. 2012, "Interactive residual stress modeling for soft tissue simulation". *Checked.* |

**In the engine.** Skin pressed hard enough tears at 35% stretch (`SKIN_LOAD_TEAR_STRETCH`), a little under the measured 54 ± 17% since a blow stretches it unevenly, and up to 8 points sooner where it is bruised or worn; muscle tears at 12%. Tearing is judged once a step's push has spread through the tissue, not the instant a tool lands on a few points. The aorta lies deep, against the spine and behind the organs, so it takes far more force to cut than the vessels of the limbs (`AORTA_DEPTH_SCALE`).

## Bleeding

| Grade | Flow | What bleeds |
|---|---|---|
| 0 | under 1 ml/min | nothing that matters |
| 1 | 1-5 ml/min | capillaries: mild, oozing |
| 2 | 5-10 ml/min | small arteries and veins: moderate, steady |
| 3 | 10-50 ml/min | an artery or vein away from the trunk: spurting |
| 4 | over 50 ml/min | a major artery or vein: gushing |

Source: the validated intraoperative bleeding scale, Lewis et al. 2017, as tabled in Smith et al. 2025 ([PMC11571751](https://pmc.ncbi.nlm.nih.gov/articles/PMC11571751/)). *Checked.* Losing over 15% of blood (about 735 ml here) raises the pulse; over 40% is life-threatening (trauma-care hemorrhage classes).

**In the engine.** Each wound bleeds by what it cut (`WoundKind`): torn skin and muscle ooze about 6 ml a minute (grades 1-2), an open fracture's marrow about 25 (grade 3), and a cut artery about 250 (grade 4), each more or less with the pressure the wound opened with, and less as it clots and as the body runs short of blood. The blood drawn follows what is lost, about 0.05 ml a drop (`BLOOD_DROP_ML`), and only a cut artery sprays.

## Rules for every play

- **Bleeding follows what was cut.** Wounds that miss arteries bleed at most 10 ml a minute each (grades 1-2), an open fracture's marrow at most 50, and a cut artery at least 10. The rate is the average since the body first lost blood (`ScenarioResult::bleeding_ml_per_min`), shared among the wounds open at once unless an artery is cut, since the scale grades each bleeding site.
- **A knife never breaks a long bone** (collarbone, upper arm, forearm, thigh, shin).
- **Struck flesh moves at most twice as fast as the tool that hit it,** the limit of a perfectly elastic collision; faster flesh means the simulation flung it.

## How the scenarios use this

- Each of the 27 scenarios whose play can injure lists what it does in real life, and every scenario follows the rules above. Each check says how often an outcome happens: never, rarely (in at most a third of runs), sometimes (in some runs but not all), usually (in at least half), or always.
- One run can only judge never and always. The sweep (`strike_scenarios --sweep`) judges the rest over 45 replays of each scenario: the swing moved slightly along and across its path, on bones 20% weaker, as tuned, and 25% stronger, since real bones differ that much between people.
- Counts with no real-world meaning, like torn mesh links or skin flaps, are not checked. `compare.ps1` still reports every change in them, so a change can be looked at in captures.
- A check the engine cannot meet yet is marked as a known gap, with why. The sweep reports known gaps on their own and does not count them as mismatches. There are two:
  - A bat brought down on the shoulder should usually break the collarbone, but never does: the figure has more flesh over the collarbone than a real shoulder, so a blow from above reaches it weakly.
  - A hard bat swing into the shin should sometimes break it, but always does: the engine passes nearly all of a blow to a bone right under the skin.
