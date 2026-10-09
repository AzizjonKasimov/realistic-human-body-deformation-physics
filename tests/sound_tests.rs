//! The game's sound: what the simulation's steps sound like, and the clips
//! they play.

use realistic_physics as rp;
use rp::scenarios::{run, scenario, SCENARIO_HEIGHT, SCENARIO_WIDTH};
use rp::sound::{Cue, CueKind, Foley, Heard, Mixdown, SoundBank};

/// Everything a tuned scenario sounded like, step by step.
fn hear_scenario(name: &str) -> Vec<Heard> {
    let scenario = scenario(name).unwrap_or_else(|| panic!("no scenario `{name}`"));
    let mut foley = Foley::new();
    let mut heard = Vec::new();
    run(&scenario, SCENARIO_WIDTH, SCENARIO_HEIGHT, |_, _, world| {
        heard.push(foley.hear(world))
    });
    heard
}

fn cues(heard: &[Heard], kind: CueKind) -> Vec<Cue> {
    heard
        .iter()
        .flat_map(|step| step.cues.iter().copied())
        .filter(|cue| cue.kind == kind)
        .collect()
}

fn strongest(heard: &[Heard], kind: CueKind) -> f32 {
    cues(heard, kind)
        .iter()
        .map(|cue| cue.strength)
        .fold(0.0, f32::max)
}

#[test]
fn resting_body_makes_no_sound() {
    let mut world = rp::create_layered_body(1280.0, 720.0, rp::Materials::default());
    let mut foley = Foley::new();
    let dt = world.materials().fixed_dt;
    for _ in 0..120 {
        world.step(dt, &rp::InputState::default(), 1280.0, 720.0);
        let heard = foley.hear(&world);
        assert_eq!(heard, Heard::default(), "a body at rest made a sound");
    }
}

#[test]
fn blows_sound_as_hard_as_they_land() {
    let slow = strongest(&hear_scenario("hammer_slow_push"), CueKind::Thud);
    let moderate = strongest(&hear_scenario("hammer_moderate_swing"), CueKind::Thud);
    let firm = strongest(&hear_scenario("hammer_firm_swing"), CueKind::Thud);
    assert!(
        slow > 0.0 && slow < 0.15,
        "a slow push should land with a faint thud, got {slow:.2}"
    );
    assert!(
        slow < moderate && moderate < firm,
        "thuds should grow with the swing: slow {slow:.2}, moderate {moderate:.2}, firm {firm:.2}"
    );
}

#[test]
fn a_full_hammer_blow_thuds_cracks_and_whooshes() {
    let heard = hear_scenario("torso_heavy_high");
    assert!(
        strongest(&heard, CueKind::Thud) > 0.8,
        "a full blow should thud hard"
    );
    assert!(
        !cues(&heard, CueKind::Crack).is_empty(),
        "the blow breaks bone but made no crack"
    );
    assert!(
        heard
            .iter()
            .any(|step| step.beds.rush > 0.5 && !step.beds.sharp),
        "the swing should rush through the air before it lands"
    );
}

#[test]
fn knife_slices_without_thuds() {
    let heard = hear_scenario("torso_sharp_cut");
    assert!(
        cues(&heard, CueKind::Thud).is_empty(),
        "a knife cut should not thud"
    );
    assert!(
        heard.iter().any(|step| step.beds.slice > 0.5),
        "the knife cut flesh without a slicing sound"
    );
}

#[test]
fn a_new_body_is_heard_silently() {
    // A reset swaps a hurt body for a fresh one; its counts start again.
    let scenario = scenario("torso_heavy_high").expect("scenario");
    let (hurt, _) = run(&scenario, SCENARIO_WIDTH, SCENARIO_HEIGHT, |_, _, _| {});
    assert!(hurt.stats().fractured_bones > 0);
    let mut foley = Foley::new();
    foley.hear(&hurt);
    let mut fresh = rp::create_layered_body(1280.0, 720.0, rp::Materials::default());
    let dt = fresh.materials().fixed_dt;
    fresh.step(dt, &rp::InputState::default(), 1280.0, 720.0);
    assert_eq!(foley.hear(&fresh), Heard::default());
}

#[test]
fn sound_bank_clips_are_clean_and_loop_seamlessly() {
    let bank = SoundBank::render();
    for clip in &bank.clips {
        let peak = clip.samples.iter().fold(0.0f32, |top, s| top.max(s.abs()));
        assert!(
            clip.samples.iter().all(|s| s.is_finite()) && (0.99..=1.0001).contains(&peak),
            "{} should be finite and peak at 1, got {peak}",
            clip.name
        );
        let last = *clip.samples.last().expect("empty clip");
        if clip.looped {
            // Wrapping round is no bigger a step than the clip takes anyway.
            let largest_step = clip
                .samples
                .windows(2)
                .map(|pair| (pair[1] - pair[0]).abs())
                .fold(0.0f32, f32::max);
            let seam = (clip.samples[0] - last).abs();
            assert!(
                seam <= largest_step,
                "{} jumps {seam} where it loops",
                clip.name
            );
        } else {
            assert!(last.abs() < 1.0e-3, "{} ends in a click", clip.name);
        }
    }
}

#[test]
fn mixdown_lands_each_sound_on_its_step_without_clipping() {
    let bank = SoundBank::render();
    let mut mix = Mixdown::new(&bank, 1.0 / 60.0);
    let mut heard = vec![Heard::default(); 30];
    // A full blow that breaks bone and tears flesh at once, on step 10.
    heard[10].cues = vec![
        Cue {
            kind: CueKind::Thud,
            strength: 1.0,
        },
        Cue {
            kind: CueKind::Crack,
            strength: 1.0,
        },
        Cue {
            kind: CueKind::Tear,
            strength: 1.0,
        },
    ];
    for step in &heard {
        mix.step(step);
    }
    let samples = mix.finish();
    let start = (10.0 / 60.0 * rp::sound::SAMPLE_RATE as f64).round() as usize;
    assert!(
        samples[..start].iter().all(|&s| s == 0.0),
        "sound before the blow"
    );
    let peak = samples.iter().fold(0.0f32, |top, s| top.max(s.abs()));
    assert!(
        peak > 0.5 && peak < 1.0,
        "the blow should be loud but not clip, got {peak}"
    );
}

#[test]
fn soft_clip_leaves_quiet_samples_alone_and_bends_loud_ones_under_full_scale() {
    for sample in [-0.8f32, -0.3, 0.0, 0.5, 0.8] {
        assert_eq!(rp::sound::soft_clip(sample), sample);
    }
    let mut last = 0.8;
    for step in 1..200 {
        let sample = 0.8 + step as f32 * 0.02;
        let bent = rp::sound::soft_clip(sample);
        assert!(bent >= last && bent <= 1.0, "soft clip of {sample}: {bent}");
        if sample <= 1.2 {
            assert!(
                bent < 0.995,
                "soft clip of {sample} should stay under full scale"
            );
        }
        assert_eq!(rp::sound::soft_clip(-sample), -bent);
        last = bent;
    }
}

#[test]
fn wav_bytes_are_16_bit_mono_pcm() {
    let wav = rp::sound::wav_bytes(&[0.0, 1.0, -1.0, 0.5]);
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 36 + 8);
    assert_eq!(&wav[8..16], b"WAVEfmt ");
    assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 1, "PCM");
    assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 1, "mono");
    assert_eq!(
        u32::from_le_bytes(wav[24..28].try_into().unwrap()),
        rp::sound::SAMPLE_RATE
    );
    assert_eq!(u16::from_le_bytes([wav[34], wav[35]]), 16, "bits");
    assert_eq!(&wav[36..40], b"data");
    assert_eq!(wav.len(), 44 + 8);
    assert_eq!(i16::from_le_bytes([wav[46], wav[47]]), i16::MAX);
    assert_eq!(i16::from_le_bytes([wav[48], wav[49]]), -i16::MAX);
}
