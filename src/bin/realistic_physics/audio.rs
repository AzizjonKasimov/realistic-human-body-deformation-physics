//! Plays the game's sound: `realistic_physics::sound` hears each simulation
//! step and says what to play, and this plays it through macroquad. The
//! sounds are synthesized and handed to the audio device in the background,
//! because a browser decodes each clip asynchronously; the game is silent for
//! the few frames that takes.

use macroquad::audio::{
    load_sound_from_bytes, play_sound, set_sound_volume, PlaySoundParams, Sound,
};
use macroquad::experimental::coroutines::{start_coroutine, Coroutine};
use realistic_physics as rp;
use rp::sound::{
    ease_bed, wav_bytes, Beds, Cue, Foley, SoundBank, Takes, Voice, Voices, BED_COUNT,
};

/// The sound bank, loaded into the audio device.
struct Loaded {
    voices: Voices,
    sounds: Vec<Sound>,
}

pub struct Audio {
    foley: Foley,
    loading: Option<Coroutine<Option<Loaded>>>,
    loaded: Option<Loaded>,
    /// Heard since the last frame played it.
    cues: Vec<Cue>,
    clicked: bool,
    /// The looped sounds' levels after the last step, and their volumes now.
    beds: Beds,
    bed_volume: [f32; BED_COUNT],
    takes: Takes,
}

impl Audio {
    /// Starts synthesizing and loading the sounds.
    pub fn start() -> Audio {
        let loading = start_coroutine(async move {
            let SoundBank { clips, voices } = SoundBank::render();
            let mut sounds = Vec::with_capacity(clips.len());
            for clip in &clips {
                sounds.push(
                    load_sound_from_bytes(&wav_bytes(&clip.samples))
                        .await
                        .ok()?,
                );
            }
            Some(Loaded { voices, sounds })
        });
        Audio {
            foley: Foley::new(),
            loading: Some(loading),
            loaded: None,
            cues: Vec::new(),
            clicked: false,
            beds: Beds::default(),
            bed_volume: [0.0; BED_COUNT],
            takes: Takes::default(),
        }
    }

    /// Hears the step the world just took.
    pub fn hear(&mut self, world: &rp::World) {
        let heard = self.foley.hear(world);
        self.cues.extend(heard.cues);
        self.beds = heard.beds;
    }

    /// A control button or key was pressed.
    pub fn click(&mut self) {
        self.clicked = true;
    }

    /// Once a frame, after its steps: starts what was heard and sets the
    /// looped sounds' volumes. Muted or paused, nothing new starts and the
    /// loops fade out.
    pub fn play(&mut self, dt: f32, muted: bool, running: bool) {
        self.finish_loading();
        let cues = std::mem::take(&mut self.cues);
        let clicked = std::mem::take(&mut self.clicked);
        let Some(loaded) = &self.loaded else {
            return;
        };
        if !muted {
            for cue in cues {
                start(loaded, self.takes.voice(&loaded.voices, cue));
            }
            if clicked {
                start(loaded, loaded.voices.click());
            }
        }
        let beds = if muted || !running {
            Beds::default()
        } else {
            self.beds
        };
        for (bed, target) in loaded.voices.beds(beds).iter().enumerate() {
            let volume = ease_bed(self.bed_volume[bed], target.volume, dt);
            if volume != self.bed_volume[bed] {
                set_sound_volume(&loaded.sounds[target.clip], volume);
                self.bed_volume[bed] = volume;
            }
        }
    }

    fn finish_loading(&mut self) {
        let Some(loading) = &self.loading else {
            return;
        };
        if !loading.is_done() {
            return;
        }
        let loaded = loading.retrieve().flatten();
        self.loading = None;
        if let Some(loaded) = &loaded {
            // The loops play for as long as the app runs, silent until a tool
            // swings or cuts.
            for voice in loaded.voices.beds(Beds::default()) {
                play_sound(
                    &loaded.sounds[voice.clip],
                    PlaySoundParams {
                        looped: true,
                        volume: 0.0,
                    },
                );
            }
        }
        self.loaded = loaded;
    }
}

fn start(loaded: &Loaded, voice: Voice) {
    play_sound(
        &loaded.sounds[voice.clip],
        PlaySoundParams {
            looped: false,
            volume: voice.volume,
        },
    );
}
