# Injury reference

The strike scenarios (`src/scenarios.rs`) check what the simulation does against what blows, cuts, and bleeding do to real people, from published biomechanics, forensic, and medical research. This file holds that research. Each scenario's `real` list names the outcome, how often it happens in real life, and the source; the rules every scenario follows are under [Rules for every play](#rules-for-every-play).

Figures marked *checked* were read in the source itself while writing these tests. The others come from the research summaries behind them and were not re-read.

## Scale

- The figure stands for an adult 1.75 m tall, weighing about 70 kg. In the scenarios' 1280x720 window it is 561.6 px tall, so one pixel is about 3.1 mm and 1,000 px/s is about 3.1 m/s (`scenario_metres_per_px`).
- Blood: about 70 ml per kg of body weight, so about 4.9 L (the trauma-care figure, as cited in Gutierrez et al. 2004). The engine's blood loss is a share of that volume (`BLOOD_VOLUME_ML`).
- Tools: a 4.5 kg (10 lb) sledgehammer head and a 0.8 kg bat (the 0.79 kg, 0.71 m bat in Guo et al. 2026). The speed of a tool is the speed of its striking part: the hammer's head, the bat's barrel, the knife's blade.

## How hard people strike

| What | Figure | Source |
|---|---|---|
| Hard bat swing | Ordinary men: the bat's centre of mass at 15.5 m/s, 112 J; women 11.0 m/s, 56 J (36 adults aged 18-30, two hands) | Guo et al. 2026, ["Biomechanical effects of sex and bat size on head-directed blunt strikes"](https://www.frontiersin.org/journals/bioengineering-and-biotechnology/articles/10.3389/fbioe.2026.1797941/full), Table 3. *Checked.* |
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

**Knives.** Pocket knives pierce ribs at 11-16 J (above 0.9-1.2 kN), and a hard stab carries 64-115 J (Bolliger et al. 2016, ["Stabbing energy and force required for pocket-knives to pierce ribs"](https://doi.org/10.1007/s12024-016-9803-z), *checked*; Horsfall et al. 1999). No study found an ordinary knife breaking a long bone; heavy chopping blades do (Lynn and Fairgrieve 2009, axes and hatchets; Gentile et al. 2019, about 15 J for an axe on a forearm-sized bone).

## Flesh and skin

| What | Figure | Source |
|---|---|---|
| Bruising | Half of blows bruise at 3.2 J (5.9 N·s), living limbs, judged a day later | Desmoulin and Anderson 2007, "Contusion mechanics: a minimum tolerance test" (pilot study). *Checked.* |
| A heavy, slow blow | 4.2-7.2 kg dropped 67 cm onto a tensed thigh (up to about 47 J and 1.7 kN): swelling, no significant direct muscle damage | Barnes et al. 2022, ["An experimental model of contusion injury in humans"](https://doi.org/10.1371/journal.pone.0277765). *Checked.* |
| Skin splitting under a blow | At least 4,000 N to split the scalp over the skull; blunt blows split skin trapped against bone | Sharkey et al. 2012, "Investigation of the force associated with the formation of lacerations and skull fractures". *Checked.* |
| Skin strength | 21.6 ± 8.4 MPa, failing at 54 ± 17% stretch | Ní Annaidh et al. 2012, ["Characterising the anisotropic mechanical properties of excised human skin"](https://arxiv.org/abs/1302.3022). *Checked.* |
| Knife through skin | Skin gives way at roughly 10-55 N, far below the force of a slash | Ní Annaidh et al. 2013; O'Callaghan 1999 |
| Slash depth | Most facial slash and stab injuries were superficial; 16 of 214 people had a significant vessel injury | Steel et al. 2021, ["A 10-year study of penetrating head and neck injury"](https://pmc.ncbi.nlm.nih.gov/articles/PMC8215095/). *Checked.* |
| Cuts gape | Skin is under tension, so a cut opens as it is released | Wu et al. 2012, "Interactive residual stress modeling for soft tissue simulation". *Checked.* |

## Bleeding

| Grade | Flow | What bleeds |
|---|---|---|
| 0 | under 1 ml/min | nothing that matters |
| 1 | 1-5 ml/min | capillaries: mild, oozing |
| 2 | 5-10 ml/min | small arteries and veins: moderate, steady |
| 3 | 10-50 ml/min | an artery or vein away from the trunk: spurting |
| 4 | over 50 ml/min | a major artery or vein: gushing |

Source: the validated intraoperative bleeding scale, Lewis et al. 2017, as tabled in Smith et al. 2025 ([PMC11571751](https://pmc.ncbi.nlm.nih.gov/articles/PMC11571751/)). *Checked.* Losing over 15% of blood (about 735 ml here) raises the pulse; over 40% is life-threatening (trauma-care hemorrhage classes).

## Rules for every play

- **Bleeding follows what was cut.** Wounds that miss arteries bleed at most 10 ml a minute (grades 1-2), an open fracture's marrow at most 50, and a cut artery at least 10. The rate is the average since the body first lost blood (`ScenarioResult::bleeding_ml_per_min`).
- **A knife never breaks a long bone** (collarbone, upper arm, forearm, thigh, shin).
- **Struck flesh moves at most twice as fast as the tool that hit it,** the limit of a perfectly elastic collision; faster flesh means the simulation flung it.

## How the scenarios use this

- Each check says how often an outcome happens: never, rarely (in at most a third of runs), sometimes (in some runs but not all), usually (in at least half), or always.
- One run can only judge never and always. The sweep (`strike_scenarios --sweep`) judges the rest over 45 replays of each scenario: the swing moved slightly along and across its path, on bones 20% weaker, as tuned, and 25% stronger, since real bones differ that much between people.
- Counts with no real-world meaning, like torn mesh links or skin flaps, are not checked. `compare.ps1` still reports every change in them, so a change can be looked at in captures.
