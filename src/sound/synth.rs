//! The sounds themselves, built from noise, filters, and decaying tones the
//! way `shared-tools/video-tools/tools/foley.py` builds them for the videos: its `thud`,
//! `crack`, `tear`, `spatter`, and `click`, and the air and knife sounds of
//! `Soundtrack.shot`, here as loops whose volume the game sets. scipy's
//! second-order Butterworth filters become RBJ-cookbook biquads; a band-pass
//! is a high-pass followed by a low-pass. `pitch` below 1 makes a sound slower
//! and deeper; the bank uses it for takes that differ slightly.

use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, TAU};

use super::SAMPLE_RATE;
use crate::Struck;

const RATE: f64 = SAMPLE_RATE as f64;
/// One-shot clips fade out over their last this long, so a tail that is
/// still ringing does not end in a click.
const FADE_OUT: f64 = 0.015;
/// Looped clips' lengths, and how much of the end blends into the start.
const AIR_LOOP: f64 = 2.0;
const SLICE_LOOP: f64 = 3.0;
const LOOP_BLEND: f64 = 0.15;

fn samples(seconds: f64) -> usize {
    (seconds * RATE) as usize
}

fn time(i: usize) -> f64 {
    i as f64 / RATE
}

pub(super) struct Synth {
    state: u64,
}

impl Synth {
    pub fn new(seed: u64) -> Synth {
        Synth { state: seed }
    }

    /// SplitMix64, so the bank comes out the same on every run and platform.
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        let unit = (self.next() >> 11) as f64 / (1u64 << 53) as f64;
        lo + (hi - lo) * unit
    }

    /// Gaussian white noise of unit variance (Box-Muller), as foley.py uses.
    /// The kind matters where a noise is scaled to its peak before mixing: a
    /// crack's snap from uniform noise came out twice as loud against its ring.
    fn noise(&mut self, n: usize) -> Vec<f64> {
        let mut out = Vec::with_capacity(n + 1);
        while out.len() < n {
            let radius = (-2.0 * (1.0 - self.uniform(0.0, 1.0)).ln()).sqrt();
            let (sin, cos) = (TAU * self.uniform(0.0, 1.0)).sin_cos();
            out.push(radius * cos);
            out.push(radius * sin);
        }
        out.truncate(n);
        out
    }

    /// A bare fist landing, short and dry as a real punch is (its force peaks
    /// within about 14 ms of contact: Atha, Yeadon, Sandover and Parsons
    /// 1985), with a game's weight on top, the user's pick of three by ear
    /// (2026-11-27): the smack of skin, brighter for a faster punch; the
    /// knock of the flesh and bone under it; a damped thump of the part
    /// struck, highest for a guarding arm and lowest for the body; a low
    /// sub-thump for weight; a snap on the head; and a little saturation.
    /// Layered as fight foley is, each layer in its own band. `strength`
    /// runs from a punch that only just reaches (0) to an elite straight
    /// punch (1).
    pub fn punch(&mut self, strength: f64, struck: Struck, pitch: f64) -> Vec<f32> {
        let s = strength;
        let n = samples(0.4);
        let smack = decay_from(
            band(
                self.noise(n),
                (900.0 + 700.0 * s) * pitch,
                (4200.0 + 2800.0 * s) * pitch,
            ),
            0.0006,
            0.006 + 0.003 * s,
        );
        let knock = decay_from(
            band(self.noise(n), 220.0 * pitch, 1100.0 * pitch),
            0.001,
            0.022,
        );
        // Smack, knock, and thump, the thump's pitch and how long it lasts.
        let (smack_w, knock_w, thump_w, thump_freq, thump_time) = match struck {
            Struck::Head => (1.0, 0.8, 0.6, 130.0, 0.035),
            Struck::Body | Struck::Leg => (0.45, 0.6, 1.0, 95.0, 0.05),
            Struck::Arm => (0.35, 0.7, 0.4, 150.0, 0.03),
        };
        let part = thump(n, thump_freq * pitch, thump_time, 0.6, 0.012);
        let natural = norm(
            sum(&[
                (smack_w, norm(smack, 1.0)),
                (knock_w, norm(knock, 1.0)),
                (thump_w, norm(part, 1.0)),
            ]),
            1.0,
        );
        let sub = thump(n, 62.0 * pitch, 0.07 + 0.03 * s, 0.9, 0.02);
        let sub_w = 0.8 * (0.5 + 0.5 * s) * if struck == Struck::Arm { 0.4 } else { 1.0 };
        let mut parts = vec![(1.0, natural), (sub_w, norm(sub, 1.0))];
        if struck == Struck::Head {
            let snap = decay_from(high(self.noise(n), 2600.0), 0.0003, 0.003);
            parts.push((0.55 * s, norm(snap, 1.0)));
        }
        let mix = sum(&parts);
        let top = mix.iter().fold(0.0f64, |top, sample| top.max(sample.abs()));
        let drive = 1.8f64;
        let shaped = mix
            .into_iter()
            .map(|sample| (drive * sample / top).tanh() / drive.tanh())
            .collect();
        one_shot(fade_out(shaped, 0.03))
    }

    /// Air past a fist on its way out: a low band of noise giving way to a
    /// higher one, so the air rises in pitch as the fist speeds up, louder
    /// until the fist would land, then dying away fast for a punch that
    /// misses.
    pub fn whoosh(&mut self, pitch: f64) -> Vec<f32> {
        let rise_time = 0.12 / pitch;
        let n = samples(rise_time + 0.06);
        let low_band = norm(band(self.noise(n), 400.0 * pitch, 1300.0 * pitch), 1.0);
        let high_band = norm(band(self.noise(n), 1300.0 * pitch, 3600.0 * pitch), 1.0);
        let mix = (0..n)
            .map(|i| {
                let t = time(i);
                let rise = (t / rise_time).min(1.0);
                let level = if t < rise_time {
                    rise * rise
                } else {
                    (-(t - rise_time) / 0.012).exp()
                };
                (low_band[i] * (1.0 - rise) + high_band[i] * rise) * level
            })
            .collect();
        one_shot(mix)
    }

    /// A fighter's body reaching the floor: the deep thump of its weight,
    /// the broad slap of its flesh on the floor, and its limbs knocking down
    /// a moment after.
    pub fn body_fall(&mut self, strength: f64, pitch: f64) -> Vec<f32> {
        let n = samples(0.7 / pitch);
        let weight = thump(
            n,
            (58.0 + 12.0 * strength) * pitch,
            0.1 + 0.06 * strength,
            0.7,
            0.03,
        );
        let slap = decay_from(
            band(self.noise(n), 300.0 * pitch, 2500.0 * pitch),
            0.001,
            0.025 / pitch,
        );
        let mut limbs = vec![0.0; n];
        for (lag, gain) in [(0.07, 0.5), (0.14, 0.35), (0.21, 0.25)] {
            let at = samples(lag * self.uniform(0.85, 1.15) / pitch);
            let knock = decay_from(
                band(self.noise(n - at), 180.0 * pitch, 900.0 * pitch),
                0.001,
                0.02,
            );
            for (sample, value) in limbs[at..].iter_mut().zip(norm(knock, gain)) {
                *sample += value;
            }
        }
        one_shot(sum(&[
            (1.0, norm(weight, 1.0)),
            (0.6, norm(slap, 1.0)),
            (0.5, limbs),
        ]))
    }

    /// A blow on flesh: a low thump whose pitch drops as it rings, the slap
    /// of skin, the knock of the tool.
    pub fn thud(&mut self, strength: f64, pitch: f64) -> Vec<f32> {
        let n = samples(0.5 / pitch);
        let mut phase = 0.0;
        let body = (0..n)
            .map(|i| {
                let t = time(i);
                phase += TAU
                    * (52.0 + 30.0 * strength)
                    * pitch
                    * (1.0 + 0.9 * (-t * pitch / 0.02).exp())
                    / RATE;
                phase.sin() * (-t * pitch / (0.08 + 0.08 * strength)).exp()
            })
            .collect();
        let slap = decay(
            band(self.noise(n), 700.0 * pitch, 3400.0 * pitch),
            0.016 / pitch,
        );
        let knock = decay(
            band(self.noise(n), 150.0 * pitch, 480.0 * pitch),
            0.045 / pitch,
        );
        let mut mix = sum(&[
            (1.0, norm(body, 1.0)),
            (0.15 + 0.55 * strength, norm(slap, 1.0)),
            (0.5, norm(knock, 1.0)),
        ]);
        for (i, sample) in mix.iter_mut().enumerate().take(24) {
            *sample *= i as f64 / 24.0;
        }
        one_shot(mix)
    }

    /// A bone breaking: a sharp snap with a short ring and a crunch, then a
    /// smaller second snap. (Its strength only changes its volume.)
    pub fn crack(&mut self, pitch: f64) -> Vec<f32> {
        let n = samples(0.4 / pitch);
        let snap = decay(high(self.noise(n), 1700.0 * pitch), 0.011 / pitch);
        let mut ring = vec![0.0; n];
        for (freq, ring_time) in [(1250.0, 0.022), (2300.0, 0.015), (3700.0, 0.009)] {
            let offset = self.uniform(0.0, 6.3);
            for (i, sample) in ring.iter_mut().enumerate() {
                let t = time(i);
                *sample += (TAU * freq * pitch * t + offset).sin() * (-t * pitch / ring_time).exp();
            }
        }
        let crunch = decay(
            band(self.noise(n), 240.0 * pitch, 1200.0 * pitch),
            0.055 / pitch,
        );
        let first = sum(&[
            (1.0, norm(snap, 1.0)),
            (0.3, norm(ring, 1.0)),
            (0.75, norm(crunch, 1.0)),
        ]);
        let lag = samples(0.03 / pitch);
        let mut mix = first.clone();
        for i in lag..n {
            let echo = i - lag;
            mix[i] += 0.45 * first[echo] * (-time(echo) * pitch / 0.02).exp();
        }
        one_shot(mix)
    }

    /// Torn flesh: many tiny wet bursts that thin out; more torn, longer.
    pub fn tear(&mut self, amount: f64, pitch: f64) -> Vec<f32> {
        let n = samples((0.1 + 0.25 * amount) / pitch);
        let mut grains = vec![0.0; n];
        for _ in 0..(10.0 + 40.0 * amount) as usize {
            let length = samples(self.uniform(0.003, 0.018) / pitch).clamp(2, n);
            let at = (self.uniform(0.0, 1.0).powf(1.6) * (n - length).max(1) as f64) as usize;
            let gain = self.uniform(0.3, 1.0);
            let burst = self.noise(length);
            for (j, value) in burst.into_iter().enumerate() {
                let hann = 0.5 - 0.5 * (TAU * j as f64 / (length - 1) as f64).cos();
                if let Some(sample) = grains.get_mut(at + j) {
                    *sample += value * hann * gain;
                }
            }
        }
        let mut wet = band(grains, 220.0 * pitch, 1500.0 * pitch);
        for (i, sample) in wet.iter_mut().enumerate() {
            *sample *= (-(i as f64) / n as f64 * 2.2).exp();
        }
        one_shot(wet)
    }

    /// Drops landing: short high plips spread over half a second, most of
    /// them early; more blood, more drops.
    pub fn spatter(&mut self, amount: f64, pitch: f64) -> Vec<f32> {
        let n = samples(0.6 / pitch);
        let m = samples(0.014 / pitch);
        let mut out = vec![0.0; n];
        for _ in 0..(3.0 + 30.0 * amount) as usize {
            let at = (self.uniform(0.0, 1.0).powi(2) * (n - m) as f64) as usize;
            let freq = self.uniform(1300.0, 4000.0) * pitch;
            let gain = self.uniform(0.25, 1.0);
            for j in 0..m {
                let t = time(j);
                let drop =
                    (TAU * freq * t * (1.0 + 6.0 * t * pitch)).sin() * (-t * pitch / 0.0035).exp();
                out[at + j] += drop * gain;
            }
        }
        one_shot(out)
    }

    /// A soft tap, for the control buttons.
    pub fn click(&mut self) -> Vec<f32> {
        let n = samples(0.05);
        let knock = (0..n)
            .map(|i| (TAU * 1900.0 * time(i)).sin() * (-time(i) / 0.006).exp())
            .collect();
        let tick = decay(band(self.noise(n), 2500.0, 7000.0), 0.002);
        one_shot(sum(&[(1.0, norm(knock, 1.0)), (0.6, norm(tick, 1.0))]))
    }

    /// One band of the air rushing past a swung tool, as a loop.
    pub fn air(&mut self, low: f64, high: f64) -> Vec<f32> {
        let n = samples(AIR_LOOP + LOOP_BLEND);
        looped(band(self.noise(n), low, high))
    }

    /// The knife cutting, as a loop: a hiss with a wet layer under it whose
    /// level flutters, a ragged, wet edge.
    pub fn slice(&mut self) -> Vec<f32> {
        let n = samples(SLICE_LOOP + LOOP_BLEND);
        let hiss = norm(band(self.noise(n), 3000.0, 11000.0), 1.0);
        let wet = norm(band(self.noise(n), 300.0, 1400.0), 1.0);
        let flutter = norm(low(self.noise(n), 40.0), 1.0);
        let mix = (0..n)
            .map(|i| 0.6 * hiss[i] + 0.45 * wet[i] * (0.6 + 0.4 * flutter[i]))
            .collect();
        looped(mix)
    }
}

#[derive(Clone, Copy)]
enum Pass {
    Low,
    High,
}

/// A second-order Butterworth filter at `freq`, kept between 20 Hz and just
/// under the Nyquist frequency as foley.py keeps it.
fn biquad(mut x: Vec<f64>, pass: Pass, freq: f64) -> Vec<f64> {
    let freq = freq.clamp(20.0, RATE * 0.49);
    let (sin, cos) = (TAU * freq / RATE).sin_cos();
    let alpha = sin / (2.0 * FRAC_1_SQRT_2);
    let (b0, b1, b2) = match pass {
        Pass::Low => ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0),
        Pass::High => ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0),
    };
    let a0 = 1.0 + alpha;
    let (b0, b1, b2, a1, a2) = (
        b0 / a0,
        b1 / a0,
        b2 / a0,
        -2.0 * cos / a0,
        (1.0 - alpha) / a0,
    );
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    for sample in &mut x {
        let x0 = *sample;
        let y0 = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        (x2, x1, y2, y1) = (x1, x0, y1, y0);
        *sample = y0;
    }
    x
}

fn low(x: Vec<f64>, freq: f64) -> Vec<f64> {
    biquad(x, Pass::Low, freq)
}

fn high(x: Vec<f64>, freq: f64) -> Vec<f64> {
    biquad(x, Pass::High, freq)
}

fn band(x: Vec<f64>, low_edge: f64, high_edge: f64) -> Vec<f64> {
    low(high(x, low_edge), high_edge)
}

/// `x` fading away with this time constant.
fn decay(mut x: Vec<f64>, seconds: f64) -> Vec<f64> {
    for (i, sample) in x.iter_mut().enumerate() {
        *sample *= (-time(i) / seconds).exp();
    }
    x
}

/// `x` coming in over `attack` seconds, then fading away with time constant
/// `seconds`, as a struck thing sounds.
fn decay_from(mut x: Vec<f64>, attack: f64, seconds: f64) -> Vec<f64> {
    for (i, sample) in x.iter_mut().enumerate() {
        let t = time(i);
        *sample *= (t / attack).min(1.0) * (-(t - attack).max(0.0) / seconds).exp();
    }
    x
}

/// A damped thump `n` samples long: a tone at `freq` that starts `drop`
/// higher and falls to it over `drop_seconds`, fading away with time
/// constant `seconds` after a 1.5 ms attack.
fn thump(n: usize, freq: f64, seconds: f64, drop: f64, drop_seconds: f64) -> Vec<f64> {
    let mut phase = 0.0;
    let tone = (0..n)
        .map(|i| {
            phase += TAU * freq * (1.0 + drop * (-time(i) / drop_seconds).exp()) / RATE;
            phase.sin()
        })
        .collect();
    decay_from(tone, 0.0015, seconds)
}

/// `x` fading to silence over its last `seconds`, so a tail still sounding
/// does not end in a click.
fn fade_out(mut x: Vec<f64>, seconds: f64) -> Vec<f64> {
    let fade = samples(seconds).min(x.len());
    let n = x.len();
    for (k, sample) in x[n - fade..].iter_mut().enumerate() {
        *sample *= 1.0 - (k + 1) as f64 / fade as f64;
    }
    x
}

/// `x` scaled so its loudest sample is `peak`.
fn norm(mut x: Vec<f64>, peak: f64) -> Vec<f64> {
    let top = x.iter().fold(0.0f64, |top, sample| top.max(sample.abs()));
    if top > 0.0 {
        for sample in &mut x {
            *sample *= peak / top;
        }
    }
    x
}

/// Weighted sum of equally long sounds.
fn sum(parts: &[(f64, Vec<f64>)]) -> Vec<f64> {
    let n = parts.iter().map(|(_, x)| x.len()).max().unwrap_or(0);
    let mut out = vec![0.0; n];
    for (weight, x) in parts {
        for (sample, value) in out.iter_mut().zip(x) {
            *sample += weight * value;
        }
    }
    out
}

/// A finished one-shot clip: faded out at its end, loudest at 1.
fn one_shot(mut x: Vec<f64>) -> Vec<f32> {
    let fade = samples(FADE_OUT).min(x.len());
    let n = x.len();
    for (k, sample) in x[n - fade..].iter_mut().enumerate() {
        *sample *= 1.0 - (k + 1) as f64 / fade as f64;
    }
    to_f32(norm(x, 1.0))
}

/// A finished loop: the last `LOOP_BLEND` of `x` blends into its start, with
/// equal power for uncorrelated noise, so it repeats without a seam; loudest
/// at 1.
fn looped(x: Vec<f64>) -> Vec<f32> {
    let blend = samples(LOOP_BLEND);
    let n = x.len() - blend;
    let mut out = x[..n].to_vec();
    for i in 0..blend {
        let angle = (i as f64 + 0.5) / blend as f64 * FRAC_PI_2;
        out[i] = x[i] * angle.sin() + x[n + i] * angle.cos();
    }
    to_f32(norm(out, 1.0))
}

fn to_f32(x: Vec<f64>) -> Vec<f32> {
    x.into_iter().map(|sample| sample as f32).collect()
}
