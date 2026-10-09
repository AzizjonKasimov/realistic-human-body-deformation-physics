use crate::{AudioContext, PlaySoundParams};

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc;

enum AudioMessage {
    AddSound(u32, Vec<f32>),
    Play(u32, u32, bool, f32),
    Stop(u32),
    StopAll(u32),
    SetVolume(u32, f32),
    SetVolumeAll(u32, f32),
    Delete(u32),
}

#[derive(Debug)]
pub struct SoundState {
    sound_id: u32,
    play_id: u32,
    sample: usize,
    data: Arc<[f32]>,
    looped: bool,
    volume: f32,
    // realistic_physics patch: the volume to glide to over the next buffer.
    target_volume: f32,
}

impl SoundState {
    fn get_samples(&mut self, n: usize) -> &[f32] {
        let data = &self.data[self.sample..];

        self.sample += n;

        match data.get(..n) {
            Some(data) => data,
            None => data,
        }
    }

    fn rewind(&mut self) {
        self.sample = 0;
    }
}

pub struct Mixer {
    rx: mpsc::Receiver<AudioMessage>,
    sounds: HashMap<u32, Arc<[f32]>>,
    mixer_state: Vec<SoundState>,
}

pub struct MixerBuilder {
    rx: mpsc::Receiver<AudioMessage>,
}

pub struct MixerControl {
    tx: mpsc::Sender<AudioMessage>,
    sound_id: Cell<u32>,
    play_id: Cell<u32>,
}

pub struct Playback {
    play_id: u32,
}

impl Playback {
    pub fn stop(self, ctx: &AudioContext) {
        ctx.mixer_ctrl.send(AudioMessage::Stop(self.play_id));
    }

    pub fn set_volume(&self, ctx: &AudioContext, volume: f32) {
        ctx.mixer_ctrl
            .send(AudioMessage::SetVolume(self.play_id, volume));
    }
}

impl MixerControl {
    pub fn load(&self, data: &[u8]) -> u32 {
        let sound_id = self.sound_id.get();

        let samples = load_samples_from_file(data).unwrap();

        self.tx
            .send(crate::mixer::AudioMessage::AddSound(sound_id, samples))
            .unwrap_or_else(|_| println!("Audio thread died"));
        self.sound_id.set(sound_id + 1);

        sound_id
    }

    pub fn play(&self, sound_id: u32, params: PlaySoundParams) -> Playback {
        let play_id = self.play_id.get();

        self.send(AudioMessage::Play(
            sound_id,
            play_id,
            params.looped,
            params.volume,
        ));

        self.play_id.set(play_id + 1);

        Playback { play_id }
    }

    pub fn stop(&self, play_id: u32) {
        self.send(AudioMessage::Stop(play_id));
    }

    pub fn stop_all(&self, sound_id: u32) {
        self.send(AudioMessage::StopAll(sound_id));
    }

    pub fn set_volume_all(&self, sound_id: u32, volume: f32) {
        self.send(AudioMessage::SetVolumeAll(sound_id, volume));
    }

    pub fn delete(&self, sound_id: u32) {
        self.send(AudioMessage::Delete(sound_id));
    }

    fn send(&self, message: AudioMessage) {
        self.tx
            .send(message)
            .unwrap_or_else(|_| println!("Audio thread died"))
    }
}

impl MixerBuilder {
    pub fn build(self) -> Mixer {
        Mixer {
            rx: self.rx,
            sounds: HashMap::new(),
            mixer_state: vec![],
        }
    }
}

impl Mixer {
    pub fn new() -> (MixerBuilder, MixerControl) {
        let (tx, rx) = mpsc::channel();

        (
            MixerBuilder { rx },
            MixerControl {
                tx,
                sound_id: Cell::new(0),
                play_id: Cell::new(0),
            },
        )
    }

    pub fn fill_audio_buffer(&mut self, buffer: &mut [f32], frames: usize) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                AudioMessage::AddSound(id, data) => {
                    self.sounds.insert(id, data.into());
                }
                AudioMessage::Play(sound_id, play_id, looped, volume) => {
                    if let Some(data) = self.sounds.get(&sound_id) {
                        self.mixer_state.push(SoundState {
                            sound_id,
                            play_id,
                            sample: 0,
                            data: data.clone(),
                            looped,
                            volume,
                            target_volume: volume,
                        });
                    }
                }
                AudioMessage::Stop(play_id) => {
                    if let Some(i) = self.mixer_state.iter().position(|s| s.play_id == play_id) {
                        self.mixer_state.swap_remove(i);
                    }
                }
                AudioMessage::StopAll(sound_id) => {
                    for i in (0..self.mixer_state.len()).rev() {
                        if self.mixer_state[i].sound_id == sound_id {
                            self.mixer_state.swap_remove(i);
                        }
                    }
                }
                AudioMessage::SetVolume(play_id, volume) => {
                    if let Some(sound) = self.mixer_state.iter_mut().find(|s| s.play_id == play_id)
                    {
                        sound.target_volume = volume;
                    }
                }
                AudioMessage::SetVolumeAll(sound_id, volume) => {
                    for sound in self
                        .mixer_state
                        .iter_mut()
                        .filter(|s| s.sound_id == sound_id)
                    {
                        sound.target_volume = volume;
                    }
                }
                AudioMessage::Delete(sound_id) => {
                    for i in (0..self.mixer_state.len()).rev() {
                        if self.mixer_state[i].sound_id == sound_id {
                            self.mixer_state.swap_remove(i);
                        }
                    }
                    self.sounds.remove(&sound_id);
                }
            }
        }

        // zeroize the buffer
        buffer.fill(0.0);

        // Note: Doing manual iteration so we can remove sounds that finished playing
        let mut i = 0;

        while let Some(sound) = self.mixer_state.get_mut(i) {
            // realistic_physics patch: a new volume glides in over this buffer
            // instead of jumping, so volumes set every frame do not click; and
            // a looped sound that wraps carries on where it left off in the
            // buffer (it restarted at the buffer's start, a glitch per loop).
            let from = sound.volume;
            let step = (sound.target_volume - from) / buffer.len().max(1) as f32;
            let mut written = 0;

            loop {
                let samples = sound.get_samples(buffer.len() - written);

                for (offset, (b, s)) in buffer[written..].iter_mut().zip(samples).enumerate() {
                    *b += s * (from + step * (written + offset) as f32);
                }

                written += samples.len();

                if written < buffer.len() && sound.looped && !sound.data.is_empty() {
                    sound.rewind();
                    continue;
                }

                break;
            }
            sound.volume = sound.target_volume;
            let remainder = buffer.len() - written;

            if remainder > 0 {
                self.mixer_state.swap_remove(i);
            } else {
                i += 1;
            }
        }

        // realistic_physics patch: the mix's peaks bend smoothly toward full
        // scale instead of clipping hard.
        for sample in buffer.iter_mut() {
            *sample = soft_clip(*sample);
        }
    }
}

/// realistic_physics patch: samples pass unchanged up to 0.8; louder ones
/// bend toward 1 along a tanh, the same curve as js/audio.js and
/// `realistic_physics::sound::soft_clip`.
fn soft_clip(sample: f32) -> f32 {
    const KNEE: f32 = 0.8;
    let magnitude = sample.abs();
    if magnitude <= KNEE {
        return sample;
    }
    (KNEE + (1.0 - KNEE) * ((magnitude - KNEE) / (1.0 - KNEE)).tanh()).copysign(sample)
}

/// Parse ogg/wav/etc and get  resampled to 44100, 2 channel data
pub fn load_samples_from_file(bytes: &[u8]) -> Result<Vec<f32>, ()> {
    let mut audio_stream = {
        let file = std::io::Cursor::new(bytes);
        audrey::Reader::new(file).unwrap()
    };

    let description = audio_stream.description();
    let channels_count = description.channel_count();
    assert!(channels_count == 1 || channels_count == 2);

    let mut frames: Vec<f32> = Vec::with_capacity(4096);
    let mut samples_iterator = audio_stream
        .samples::<f32>()
        .map(std::result::Result::unwrap);

    // audrey's frame docs: "TODO: Should consider changing this behaviour to check the audio file's actual number of channels and automatically convert to F's number of channels while reading".
    // lets fix this TODO here
    if channels_count == 1 {
        frames.extend(samples_iterator.flat_map(|sample| [sample, sample]));
    } else if channels_count == 2 {
        frames.extend(samples_iterator);
    }

    let sample_rate = description.sample_rate();

    // stupid nearest-neighbor resampler
    if sample_rate != 44100 {
        let mut new_length = ((44100 as f32 / sample_rate as f32) * frames.len() as f32) as usize;

        // `new_length` must be an even number
        new_length -= new_length % 2;

        let mut resampled = vec![0.0; new_length];

        for (n, sample) in resampled.chunks_exact_mut(2).enumerate() {
            let ix = 2 * ((n as f32 / new_length as f32) * frames.len() as f32) as usize;
            sample[0] = frames[ix];
            sample[1] = frames[ix + 1];
        }
        return Ok(resampled);
    }

    Ok(frames)
}

// realistic_physics patch: tests for the patched mixing. Run them with
// `cargo test --manifest-path vendor/quad-snd/Cargo.toml --target-dir target/vendor`.
#[cfg(test)]
mod tests {
    use super::*;

    /// A mixer already playing `data` (interleaved stereo) on a loop.
    fn mixer_playing(data: Vec<f32>, volume: f32) -> (Mixer, MixerControl) {
        let (builder, control) = Mixer::new();
        control.tx.send(AudioMessage::AddSound(0, data)).unwrap();
        control.play(
            0,
            PlaySoundParams {
                looped: true,
                volume,
            },
        );
        (builder.build(), control)
    }

    #[test]
    fn a_looped_sound_wraps_where_it_left_off() {
        let (mut mixer, _control) = mixer_playing(vec![0.1, 0.1, 0.2, 0.2, 0.3, 0.3], 1.0);
        let mut buffer = vec![0.0; 16];
        mixer.fill_audio_buffer(&mut buffer, 8);
        let expected = [
            0.1, 0.1, 0.2, 0.2, 0.3, 0.3, 0.1, 0.1, 0.2, 0.2, 0.3, 0.3, 0.1, 0.1, 0.2, 0.2,
        ];
        assert_eq!(buffer, expected);
        mixer.fill_audio_buffer(&mut buffer, 8);
        assert_eq!(&buffer[..4], &[0.3, 0.3, 0.1, 0.1]);
    }

    #[test]
    fn a_new_volume_glides_in_over_a_buffer() {
        let (mut mixer, control) = mixer_playing(vec![0.5; 64], 0.0);
        let mut buffer = vec![0.0; 32];
        mixer.fill_audio_buffer(&mut buffer, 16);
        assert!(buffer.iter().all(|&sample| sample == 0.0));
        control.set_volume_all(0, 1.0);
        mixer.fill_audio_buffer(&mut buffer, 16);
        assert!(buffer.windows(2).all(|pair| pair[1] >= pair[0]));
        assert!(buffer[0] < 0.05 && buffer[31] > 0.45);
        mixer.fill_audio_buffer(&mut buffer, 16);
        assert!(buffer.iter().all(|&sample| (sample - 0.5).abs() < 1.0e-6));
    }

    #[test]
    fn loud_peaks_bend_under_full_scale() {
        let (mut mixer, _control) = mixer_playing(vec![0.5, -0.5, 1.5, -1.5], 1.0);
        let mut buffer = vec![0.0; 4];
        mixer.fill_audio_buffer(&mut buffer, 2);
        assert_eq!(&buffer[..2], &[0.5, -0.5]);
        assert!(buffer[2] > 0.8 && buffer[2] < 1.0 && buffer[3] == -buffer[2]);
    }
}
