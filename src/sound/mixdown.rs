//! The game's sound mixed offline, step by step the way the app plays it:
//! each step's cues start at the step, and the looped sounds glide to their
//! new volumes over it as they do over a frame. Tools and tests use it to
//! hear a scenario without the app (`strike_scenarios --sound DIR`).

use super::{ease_bed, soft_clip, Heard, SoundBank, Takes, BED_COUNT, SAMPLE_RATE};

pub struct Mixdown<'a> {
    bank: &'a SoundBank,
    samples: Vec<f32>,
    /// Where the next step starts, in samples.
    at: usize,
    step_seconds: f64,
    steps: usize,
    takes: Takes,
    bed_volume: [f32; BED_COUNT],
    /// Where each looped clip has played up to.
    bed_position: [usize; BED_COUNT],
}

impl<'a> Mixdown<'a> {
    /// An empty mix of steps `step_seconds` long.
    pub fn new(bank: &'a SoundBank, step_seconds: f64) -> Mixdown<'a> {
        Mixdown {
            bank,
            samples: Vec::new(),
            at: 0,
            step_seconds,
            steps: 0,
            takes: Takes::default(),
            bed_volume: [0.0; BED_COUNT],
            bed_position: [0; BED_COUNT],
        }
    }

    /// Adds one step that sounded like `heard`.
    pub fn step(&mut self, heard: &Heard) {
        self.steps += 1;
        let end = (self.steps as f64 * self.step_seconds * SAMPLE_RATE as f64).round() as usize;
        let length = end - self.at;
        if self.samples.len() < end {
            self.samples.resize(end, 0.0);
        }
        for &cue in &heard.cues {
            let voice = self.takes.voice(&self.bank.voices, cue);
            let clip = &self.bank.clips[voice.clip].samples;
            if self.samples.len() < self.at + clip.len() {
                self.samples.resize(self.at + clip.len(), 0.0);
            }
            for (sample, value) in self.samples[self.at..].iter_mut().zip(clip) {
                *sample += value * voice.volume;
            }
        }
        let targets = self.bank.voices.beds(heard.beds);
        for (bed, target) in targets.iter().enumerate() {
            let from = self.bed_volume[bed];
            let to = ease_bed(from, target.volume, self.step_seconds as f32);
            self.bed_volume[bed] = to;
            let clip = &self.bank.clips[target.clip].samples;
            let position = &mut self.bed_position[bed];
            for (i, sample) in self.samples[self.at..end].iter_mut().enumerate() {
                let volume = from + (to - from) * (i + 1) as f32 / length as f32;
                *sample += clip[*position] * volume;
                *position = (*position + 1) % clip.len();
            }
        }
        self.at = end;
    }

    /// The mix so far through the output's soft clip, with the tails of
    /// sounds still ringing after the last step.
    pub fn finish(self) -> Vec<f32> {
        self.samples.into_iter().map(soft_clip).collect()
    }
}
