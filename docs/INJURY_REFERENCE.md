# Injury reference

The strike scenarios (`src/scenarios.rs`) check what the simulation does against what blows, cuts, and bleeding do to real people, from published biomechanics, forensic, and medical research. This file holds that research. Each scenario's `real` list names the outcome, how often it happens in real life, and the source; the rules every scenario follows are under [Rules for every play](#rules-for-every-play).

Figures marked *checked* were read in the source itself while writing these tests. The others come from the research summaries behind them and were not re-read.

## Scale

- The figure stands for an adult 1.75 m tall, weighing about 70 kg. In the scenarios' 1280x720 window it is 561.6 px tall, so one pixel is about 3.1 mm and 1,000 px/s is about 3.1 m/s (`scenario_metres_per_px`).
- Blood: about 70 ml per kg of body weight, so about 4.9 L (the trauma-care figure, as cited in Gutierrez et al. 2004). The engine's blood loss is a share of that volume (`BLOOD_VOLUME_ML`).
- Tools: a 4.5 kg (10 lb) sledgehammer head and a 0.8 kg bat (the 0.79 kg, 0.71 m bat in Guo et al. 2026). The speed of a tool is the speed of its striking part: the hammer's head, the bat's barrel, the knife's blade. A bat turns about the hands on its handle, so its barrel meets what it strikes with less than the whole bat's mass, 1 / (1 / M + b² / I): about 0.6 kg for a 0.8 kg bat struck 15 cm past its centre of mass, where I is about 0.045 kg m². A sledgehammer's head strikes with all of its mass.
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

**In the engine.** Each bone is as strong as the bending tests make it against the forearm (`bone_strength` in `src/simulation/body.rs`): the collarbone about half, the upper arm 2.1 times, the shin 2.25, the thigh 2.5. The forearm is set 30% above its share, inside the spread of the forearm studies, so a light bat swing rarely breaks it. How hard a blow presses on a bone follows from its speed, the masses that meet, and the flesh over the bone ([How blows reach bone](#how-blows-reach-bone)). Only blows, pieces of broken bone, and flesh pressed onto a bone break it; what a joint passes on strains the joint but breaks no bone.

**Where a bone breaks.** A bone breaks where a load bends it most, and a load bends a bone by its lever on what holds the bone, not by its force alone. In the tests above the bone rests on two supports and is struck between them, which bends it by F a b / L under the blow, with a and b the blow's distances from the supports and L the span: most in the middle, so the thigh bones of Kennedy et al., struck in the middle, all broke there, under the impactor. In the body a long bone rests on its joints the same way, so a blow a quarter of the way along a bone bends it 0.75 times as hard as the same blow in the middle, and one a tenth of the way 0.36 times; a blow pressed along a stretch of the bone, as a bat lying along a forearm, bends it less than one at a point, half as hard when spread over the whole span. Past a bone's last joint, as a hand's fingers, only the bone's own inertia holds it, so a blow there turns it more than it bends it. A bending break can also break a wedge out of the bone, a butterfly fragment: classically its broad side lies where the blow landed (Messerer's wedge), though bending tests find wedges in only some breaks, on either side (Isa et al. 2021).

**In the engine.** Every load that can break a bone counts by how hard it bends the bone where it lands, against the same load at one point in the middle of a span between two joints, and the bone breaks where it was bent hardest (`src/simulation/bending.rs`). A break twice the bone's strength or more also breaks a wedge out of the struck side. A load along a bone and a blow on a bone's end count as [How blows reach bone](#how-blows-reach-bone) describes. Each collarbone is its own bone, from the top of the breastbone out to its shoulder, so one breaks without moving the other shoulder; it lifts at the breastbone by up to about 31 degrees and drops by up to about 11 (in life it lifts about 45 and drops about 10).

## How blows reach bone

A blunt blow presses on a bone with the peak force of a mass meeting a spring: F = v √(k m), where v is the speed at which the tool meets the bone, k the stiffness of the contact, and m the tool's and the struck part's masses in series, m_tool m_part / (m_tool + m_part). Robinovitch, Hayes, and McMahon built this model for falls on the hip ([J Biomech Eng 113:366, 1991](https://doi.org/10.1115/1.2895414)). The force grows with the speed of a blow rather than its energy, so a bone k times as strong takes a blow k² times as energetic to break, as the energy estimates above assume.

The flesh over a bone is a spring in series with it, so a thickness h of it softens the contact to k₀ / (1 + h / h₀) and the force to F₀ / √(1 + h / h₀). Robinovitch, McMahon, and Hayes found thicker hip tissue gave a lower peak force under a 140 J impact ([J Orthop Res 13:956, 1995](https://doi.org/10.1002/jor.1100130621), *checked (abstract)*); applied as a clinical rule, that is about 71 N less a millimetre, 27% less at 30 mm ([Nielson et al. 2009](https://doi.org/10.1210/jc.2008-1640), *checked*). Fitting the spring to it puts h₀ at 7-34 mm; the engine takes 15 mm, and at least 40% of a blow reaches a bone however thick its cover, since a hard blow presses the flesh flat (the thickest tissue still passed 63% of the thinnest's force).

| What covers the bone | Thickness | Source |
|---|---|---|
| Top of the collarbone | About 8 mm: it lies right under the skin along its length. No study measured it; set by judgement | - |
| Inner face of the shin | 9 mm in men and 16 in women of healthy weight, 2 cm below the knee | DuVall et al. 2025, [West J Emerg Med](https://doi.org/10.5811/westjem.33655), CT of 368 people. *Checked.* |
| Outside of the shin | The front muscle of the shin alone about 26 mm in young men, 22 in women; with skin and fat, about 30 mm | Martín-Rodríguez et al. 2023, [J Anat 243:664](https://doi.org/10.1111/joa.13864), ultrasound. *Checked.* |
| Outside of the upper arm | Skin and fat over the deltoid 8 mm in men, 12 in women, over the deltoid itself; about 25 mm in all | Poland et al. 1997, [JAMA 277:1709](https://jamanetwork.com/journals/jama/article-abstract/416596). *From a summary.* |
| Side of the chest | About 40 mm to the lung's lining between the ribs at the mid-axillary line, so about 30 mm over a rib | Laan et al. 2016, [Injury 47:797](https://doi.org/10.1016/j.injury.2015.11.045). *Checked (abstract).* |
| Over the hip's bony point | 35-38 ± 15 mm in adults aged 16-35 | Town et al. 2023, [PLoS One 18:e0283012](https://doi.org/10.1371/journal.pone.0283012). *Checked.* |
| The forearm, the outside of the thigh, the hand and foot | 6-10 mm (the ulna's border lies under the skin), 40 mm, 5-6 mm: set by judgement. Neumann et al. 2018 measured these sites in 100 adults ([Sci Data 5:180193](https://doi.org/10.1038/sdata.2018.193)), but publish the numbers only as a dataset | - |

The figure is a flat front view, so the flesh drawn between a tool and a bone is not always what covers it: above the collarbone it draws the trapezius, which in a real shoulder lies behind the collarbone, so the figure has 26 mm over the outer collarbone where a person has about 8. A bone therefore takes a blow from the moment the tool presses the skin over it in the figure, measured from the figure's outline when the body is built, and the real thickness above sets how much that flesh softens it. `anatomy_diagnostics` prints the two side by side for each bone.

The struck part's mass is what a blow across its middle meets: a limb segment turns about the joint above it with the limb beyond it held straight, so from Winter's segment masses, lengths, and radii of gyration (*Biomechanics and Motor Control of Human Movement*, 2009) a blow meets 4.5 kg at the forearm, 18 at the upper arm, 8.7 at the shin, and 39 at the thigh; the legforms that stand in for a pedestrian's leg in crash tests weigh 8.6 kg above the knee and 4.8 below (EU Directive 2004/90). The collarbone and ribs are taken as braced on the trunk, 10 kg. A light bat meets a limb with nearly all its mass, while much of a heavy hammer's goes into shoving the limb aside.

The stiffness of a blow on bare bone, k₀, is set so blows break bones as often as the research says, as each tool's break scale was before: 600 kN/m. The 4.5 kg weight that broke forearms held at both ends at 3 m/s and 1.37 kN met them through about 46 kN/m; a blow in the figure lands on a free limb, often off the middle and spread along it, so it takes a stiffer contact to break the same bones.

**Along the bone and at its ends.** A long bone takes several times the force along its length that breaks it across: the lower leg 4.3-11.4 kN under fast loading (Yoganandan et al. 1996, SAE 962426, *from a summary*) against 1-4 kN across, and the curved collarbone about twice, 1.48 ± 0.46 kN (Duprey et al. 2008, [J Biomech 41:200](https://doi.org/10.1016/j.jbiomech.2007.06.020), *checked (abstract)*) and 2.41 ± 0.72 kN (Arregui-Dalmases et al. 2010, [Traffic Inj Prev 11:286](https://doi.org/10.1080/15389581003709902), *checked (abstract)*) against 0.73 kN across (Kemper et al. 2006) and 1.16 ± 0.13 kN front to back (Untaroiu et al. 2009, *checked (abstract)*). The two add, as in the Tibia Index (Mertz 1993). The end of a bone beside a joint can break under a blow there: side impacts at the knee broke the joints' surfaces at 2.4 ± 0.7 kN where thigh bones broke at 1.4 kN (Kajzer et al. 1997 and 1999, SAE 973326 and 1999-01-0712, *from a summary*), and 28% of broken collarbones break in their outer fifth, against 69% in the middle and 3% at the inner end (Robinson 1998, [J Bone Joint Surg Br 80:476](https://doi.org/10.1302/0301-620X.80B3.0800476), *from a summary*).

**In the engine.** A load along a long bone counts a quarter as much as one across it, along a collarbone half, and pressed along its length a bone gives way in the middle of its span; the figure's rib is the half of a real one from the spine round to the side, so a blow along it strikes the side of the chest across the real rib and counts whole. A blow on the end of a long bone, its last 15% (about as long as the end is wide, the AO/OTA square rule, Meinberg et al. 2018), or on the outer fifth of a collarbone crushes the end as 0.6 of the blow, so it takes about 1.7 times the force that breaks the shaft across its middle. A bone takes the blow over the stretch the tool presses at the blow's peak, pressed in as far as the flesh and bone give, v √(m / k).

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

**In the engine.** Skin pressed hard enough tears once stretched 35% past where the living body holds it (`SKIN_LOAD_TEAR_STRETCH`), about 46% from the length it would shrink to cut free, a little under the measured 54 ± 17% since a blow stretches it unevenly, and up to 8 points sooner where it is bruised or worn; muscle tears at 12%. A cut gapes because the skin rests stretched ([Physical tissue](#physical-tissue)). Tearing is judged once a step's push has spread through the tissue, not the instant a tool lands on a few points. The aorta lies deep, against the spine and behind the organs, so it takes far more force to cut than the vessels of the limbs (`AORTA_DEPTH_SCALE`).

## Physical tissue

The body's tissue weighs and stretches as real tissue does (`src/simulation/tissue.rs`), and the solver moves it as damped springs of that stiffness (`src/simulation/soft.rs`, rebuilt from Box2D v3's soft step).

| What | Figure | Source |
|---|---|---|
| Skin thickness | 1.55 mm on the thigh, 1.91 at the waist, 2.02 over the deltoid, 2.54 on the back (ultrasound, 342 adults) | Laurent et al. 2007, [*Vaccine* 25:6423](https://doi.org/10.1016/j.vaccine.2007.05.046). *Checked (abstract).* |
| Living skin | 0.52-0.59 MPa at small strain and 3.1-5.5 MPa further on (forearm, by suction) | Lakhani et al. 2021, [*Front Bioeng Biotechnol* 9:749492](https://doi.org/10.3389/fbioe.2021.749492). *Checked.* |
| Skin's J-shaped curve | Excised back skin: 1.18 ± 0.88 MPa at first and 83.3 ± 34.9 MPa once its collagen straightens, about a quarter stretched (read off the curves), failing at 54 ± 17% | Ní Annaidh et al. 2012 (above). *Checked.* |
| Skin rests stretched | Cut free, skin shrank 23% along its tension lines and 10% across them (dog), so living skin is stretched about 1.3 and 1.11 times; no one has measured it in people. Stretched skin rests at the start of its stiffening | Deroy et al. 2017, [*Skin Res Technol* 23:326](https://arxiv.org/abs/1609.07267), Table 2; Rausch and Kuhl 2013, [*J Mech Phys Solids* 61:1955](https://doi.org/10.1016/j.jmps.2013.04.005). *Checked (Rausch and Kuhl from the abstract).* |
| Fat | 1.6 ± 0.8 kPa at first and 11.7 kPa by 30% strain (people, in tension); 7.5 kPa in shear at 1.6 Hz (pig) | Alkhouli et al. 2013, [*Am J Physiol Endocrinol Metab*](https://doi.org/10.1152/ajpendo.00111.2013); Geerligs et al. 2008, [*Biorheology*](https://doi.org/10.3233/BIR-2008-0517). *Checked (abstracts).* |
| Relaxed muscle | Shears at 3.0-4.5 kPa (9 muscles, shear-wave elastography, 30 people); the biceps 5.2 kPa bent and 16.1 kPa stretched; about 22 kPa across its fibers in tension (rabbit) | Lacourpaille et al. 2012, [*Physiol Meas*](https://doi.org/10.1088/0967-3334/33/3/N19); Eby et al. 2015, [*Clin Biomech*](https://doi.org/10.1016/j.clinbiomech.2014.11.011); Morrow et al. 2010, [*J Mech Behav Biomed Mater*](https://doi.org/10.1016/j.jmbbm.2009.03.004). *Checked.* |
| How tissue rings | Soft tissue on the limbs rings in decaying waves after a jolt, at a few to about 55 Hz | Wakeling and Nigg 2001; Schmitt and Günther 2011. Not re-read. |
| Densities | Skin about 1,090, fat 950, muscle 1,050 kg/m³ | ICRU Report 44, via NIST. |
| Body build | Each part's share of the body's mass: upper arm 2.8%, forearm 1.6%, hand 0.6%, thigh 10%, shank 4.65%, foot 1.45%; bone is about 15% of a man's mass; a man's belly is 32.6 cm wide and 23.8 cm deep | Winter 2009, Table 4.1; Clarys, Martin, and Drinkwater 1984, *Hum Biol* 56:459 (abstract checked); ANSUR II (Gordon et al. 2014). |

**In the engine.** The figure is a flat front view, so each sheet stands for a slab of the body seen from the front. The skin sheet is the skin and the fat under it, which slide over the muscle together and run round a limb about three times as far as the front view shows; the muscle sheet is everything under them, as deep as the body is there (a limb about round, the trunk about three quarters as deep as wide). Each point weighs its share of its sheet's triangles at those densities, and each bone its part's share of the body times its share of bone. A sheet of shear modulus μ and thickness t resists in its plane with μt newtons per metre; a triangle mesh carries that on springs of 8/3 μt times the area of each side's triangles over its length squared (Van Gelder 1998), and the rest of its resistance to growing or shrinking, as tissue that keeps its volume does, on each triangle's area. Skin shears at 0.2 MPa, a third of living skin's Young's modulus, is 2 mm thick, and is 70 times stiffer past a quarter's stretch; muscle shears at 4 kPa along its fibers and 6 across, and is 12 and 4 times stiffer past 15% and 10%; the fat, 10 mm thick, shears at 7.5 kPa, up to 6 times stiffer once it has slid half its thickness, and tears once it has slid six thicknesses, or one and a half under a crushing load. Skin rests stretched 8% every way, the smaller of the two measured stretches: with no back to wrap round, the flat sheet did not balance when pulled harder round the limbs than along them. The body's outline holds the stretch, as the skin running on round the back of a real body does, so a cut gapes and torn skin pulls back on its own. Tissue rings with a damping ratio of 0.3 (the fat 0.5), and muscle springs back onto its bone at 20 Hz. Tears are judged by how far tissue is stretched past where the living body holds it. Set by judgement, not measured: how firmly the body holds its pose as it hangs (1-1.7 Hz), how stiff its joints are (60 Hz, as Box2D holds its joints), and how far flesh slips off its bone before the tie tears (40 mm, or 15 under load).

## Bleeding

| Grade | Flow | What bleeds |
|---|---|---|
| 0 | under 1 ml/min | nothing that matters |
| 1 | 1-5 ml/min | capillaries: mild, oozing |
| 2 | 5-10 ml/min | small arteries and veins: moderate, steady |
| 3 | 10-50 ml/min | an artery or vein away from the trunk: spurting |
| 4 | over 50 ml/min | a major artery or vein: gushing |

Source: the validated intraoperative bleeding scale, Lewis et al. 2017, as tabled in Smith et al. 2025 ([PMC11571751](https://pmc.ncbi.nlm.nih.gov/articles/PMC11571751/)). *Checked.* Losing over 15% of blood (about 735 ml here) raises the pulse; over 40% is life-threatening (trauma-care hemorrhage classes).

**In the engine.** Each wound bleeds by what it cut (`WoundKind`): torn skin and muscle ooze about 6 ml a minute (grades 1-2), an open fracture's marrow about 25 (grade 3), and a cut artery about 250 (grade 4), each more or less with the pressure the wound opened with, and less as it clots and as the body runs short of blood. The blood drawn is the blood lost outside the body, about 0.05 ml a drop (`BLOOD_DROP_ML`), and only a cut artery sprays. A cut artery's blood comes out of the nearest opening in the skin (as up a knife's track), and blood that cannot get out bruises the flesh.

## How injuries look

The engine draws an injury the way it would look on a real person, neither toned down nor exaggerated: no more blood than was lost and no larger than it would be, a bruise as it develops, and broken bone where it would be.

| What | Figure | Source |
|---|---|---|
| A drop of blood | About 0.05 ml (0.0495-0.0516 ml), 4.6 mm across as it falls, at most 7.65 m/s | MacDonell 1971, ["Flight characteristics and stain patterns of human blood"](https://ojp.gov/pdffiles1/Digitization/1747NCJRS.pdf), Table I. *Checked.* |
| The spot a drop leaves | On a smooth, hard surface a 0.05 ml drop leaves a spot about 15 mm wide after falling 0.5 m, 17 mm after 1 m, 17.5 mm after 1.5 m, and 18-19 mm from about 2 m on; tile and glass gave similar spots | MacDonell 1971, Fig. 3 (read off the graph, about ±0.5 mm). *Checked.* |
| A pool | Pools of up to 31 g poured on floor tile average 1.44 ± 0.19 mm deep, about 7 cm² for each millilitre: 40 ml makes a pool about 19 cm across | Smith, Nicloux, and Brutin 2020, ["A new forensic tool to date human blood pools"](https://doi.org/10.1038/s41598-020-65465-4). *Checked.* |
| Blood on surfaces | Blood meets plastics and card at about 87-120 degrees, and its surface tension is about 60 mN/m; no one has measured blood on skin | Attinger 2016, [NIJ report 249851](https://www.ojp.gov/pdffiles1/nij/grants/249851.pdf). *Checked.* |
| When a bruise shows | A paintball's bruise on the upper arm was visible at once in all 157 adults; a fast, light blow's bruise is fully developed within 15-20 minutes (pig skin); a deep blow's may take days to show, and a 3.9 J blow left a visible bruise in only 53% of people | Scafide et al. 2020, [*J Forensic Sci* 65:1191](https://doi.org/10.1111/1556-4029.14294); Randeberg et al. 2007, [*Lasers Surg Med* 39:403](https://doi.org/10.1002/lsm.20494); Nijs et al. 2019, [*Forensic Sci Int* 294:34](https://doi.org/10.1016/j.forsciint.2018.10.029). *Checked (the last two from their abstracts).* |
| A bruise's colour | Red shows at any age; yellow only after about 18 hours | Langlois and Gresham 1991, [*Forensic Sci Int* 50:227](https://doi.org/10.1016/0379-0738(91)90154-B). *Checked (abstract).* |
| Broken bone leaving the body | A bone segment thrown clear of a limb is rare, reported in high-energy injuries such as a motorcycle hitting a car | Hu et al. 2024, [*Front Pediatr* 12:1333575](https://doi.org/10.3389/fped.2024.1333575). *Checked.* |

**In the engine.** Each drop of blood holds its volume and is drawn at the size that blood would have on a person the body's size (`src/simulation/blood.rs`): a falling drop as its sphere, about 4.6 mm across, a bead on the skin as wide as the skin it wets (a cap meeting the skin at 90 degrees), a trail as the film a running bead leaves, a landing drop as MacDonell's spot for how fast it lands, and spots that touch as one pool, as wide as their spots or as that much blood spreads 1.44 mm deep, whichever is wider. No blood shows that the wounds did not lose. A blow reddens the skin at once and its bruise shows over the next quarter hour, fully developed in about 17 minutes, then stays (`src/simulation/bruises.rs`); the skin shows every bruise as a shallow one would. A piece of broken bone is held by the flesh around it and moves with it (`hold_piece_in_flesh`); the flesh lets go only once it is torn away. Not measured, and set by judgement: how big a bead on skin must be to run (more than one drop's worth), and the film a running bead leaves (a tenth of a millimetre).

## Rules for every play

- **Bleeding follows what was cut.** Wounds that miss arteries bleed at most 10 ml a minute each (grades 1-2), an open fracture's marrow at most 50, and a cut artery at least 10. The rate is the average since the body first lost blood (`ScenarioResult::bleeding_ml_per_min`), shared among the wounds open at once unless an artery is cut, since the scale grades each bleeding site.
- **A knife never breaks a long bone** (collarbone, upper arm, forearm, thigh, shin).
- **Broken bone stays in the body.** No piece of broken bone may end up more than three point spacings (about 11 cm) from all flesh (`bone_pieces_out`); at worst a broken end pokes out through the skin. A segment thrown clear of a limb comes only from high-energy injuries (Hu et al. 2024). A closed break's pieces belong inside the skin; the sweep reports how often one ends up outside it (`closed_pieces_out`), since a hard blow can still squeeze the flat body's flesh off a broken bone, but does not yet fail a run for it.
- **Struck flesh moves at most twice as fast as the tool that hit it,** the limit of a perfectly elastic collision; faster flesh means the simulation flung it.

## How the scenarios use this

- Each of the 27 scenarios whose play can injure lists what it does in real life, and every scenario follows the rules above. Each check says how often an outcome happens: never, rarely (in at most a third of runs), sometimes (in some runs but not all), usually (in at least half), or always.
- One run can only judge never and always. The sweep (`strike_scenarios --sweep`) judges the rest over 45 replays of each scenario: the swing moved slightly along and across its path, on bones 20% weaker, as tuned, and 25% stronger, since real bones differ that much between people.
- Counts with no real-world meaning, like torn mesh links or skin flaps, are not checked. `compare.ps1` still reports every change in them, so a change can be looked at in captures.
- A check the engine cannot meet yet is marked as a known gap, with why. The sweep reports known gaps on their own and does not count them as mismatches. None is open. The last two closed when blows began to reach bone through the flesh over it ([How blows reach bone](#how-blows-reach-bone)): a bat brought down on the shoulder never broke the collarbone, because the figure draws more flesh over it than a real shoulder has, and a hard bat swing into the shin broke it every time, often in three places, because the force a blow put on bone grew with its energy rather than its speed, so the hardest swings overloaded the strongest bones.
