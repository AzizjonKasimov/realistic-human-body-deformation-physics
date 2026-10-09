# Tearing judged from constraint forces: probe

A survey of techniques worth rebuilding (2026-10-09) proposed judging tears,
fatigue, and fractures by each constraint's XPBD force (its Lagrange
multiplier, `lambda`) rather than by stretch, once per step. This branch
measures whether that would judge tissue differently in this engine. It is
not meant to be merged; the probe prints to stderr.

## What the probe logs

- After the solver, for every intact fiber stretched more than 4% or pulled
  more than 0.4: its layer, stretch, `lambda` (negative is a pull), and the
  tool's load on its ends (`PROBE` lines).
- Every fiber the tearing rule breaks, with which rule broke it (`TEAR`
  lines).

Run with `strike_scenarios --only <scenario> 2> probe.txt`.

## Results (main at 2dbd2ea)

| Scenario | Layer | Stretched samples | r(stretch, pull) | Pull over 2 under 12% stretch |
|---|---|---|---|---|
| torso_heavy_high | skin | 12,704 | 0.72 | 6,097 |
| torso_heavy_high | muscle | 10,698 | 0.75 | 2,613 |
| bat_hard_forearm | skin | 12,746 | 0.72 | 6,318 |
| bat_hard_forearm | muscle | 11,338 | 0.72 | 2,730 |
| hammer_firm_swing | skin | 54,854 | 0.70 | 27,448 |
| hammer_firm_swing | muscle | 43,061 | 0.75 | 10,019 |

Pull of intact fibers (99.9th percentile, then the most): full sledgehammer
blow skin 19.1 / 44.6, muscle 23.1 / 27.1; hard bat skin 17.6 / 28.6, muscle
23.4 / 25.8; firm sledgehammer swing skin 28.5 / 31.3, muscle 25.3 / 29.5.

Fibers the current rule tore: muscle torn under load pulled -8.9 to 11.9;
skin torn by stretch pulled -2.3 to 46.1.

## Conclusion

Not adopted. The engine solves tissue with zero compliance and fixed
stiffness factors, so a fiber's `lambda` is the correction the solver made
to it this step, not a physical tension: it follows stretch only loosely
(r about 0.72), much of it is the solver holding fibers against bone anchors
and joints, it does not grow with the blow (a firm swing's fibers pull
harder than a full-force blow's), and the fibers that tear are not the ones
pulled hardest. A force rule would make tears follow solver artifacts. The
idea needs physical compliance first (each layer's stiffness from its
measured modulus), which would retune every tissue response; that is a
larger project than this item.
