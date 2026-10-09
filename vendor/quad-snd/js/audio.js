"use strict";

// realistic_physics patches (listed in vendor/quad-snd/PATCHES.md): audio
// unlocks on any gesture until it really runs, goes quiet in a hidden tab,
// the mix's peaks bend instead of clipping, volume changes glide instead of
// jumping, and stopping a sound stops its source node.

const AudioContext = window.AudioContext || window.webkitAudioContext;
let audio_context;
// Where every sound goes: the soft clip in front of the speakers.
let audio_output;
let sounds = new Map();
let playbacks = [];
let sound_key_next = 1;
let playback_key_next = 1;

// Samples pass unchanged up to 0.8; louder ones bend toward 1 along a tanh,
// the same curve as the native mixer. A wave shaper reads its curve only over
// -1..1, so the mix is halved going in and the curve spans -2..2.
function soft_clip_output() {
    const knee = 0.8;
    const points = 4097;
    const curve = new Float32Array(points);
    for (let i = 0; i < points; i++) {
        const x = (i / (points - 1)) * 4 - 2;
        const magnitude = Math.abs(x);
        const bent = magnitude <= knee
            ? magnitude
            : knee + (1 - knee) * Math.tanh((magnitude - knee) / (1 - knee));
        curve[i] = Math.sign(x) * bent;
    }
    const halve = audio_context.createGain();
    halve.gain.value = 0.5;
    const shaper = audio_context.createWaveShaper();
    shaper.curve = curve;
    halve.connect(shaper);
    shaper.connect(audio_context.destination);
    return halve;
}

function audio_init() {
    if (audio_context == null) {
        audio_context = new AudioContext();
        audio_output = soft_clip_output();

        // Browsers start audio only from a user gesture. Try on every gesture
        // until the context runs: the original gave up after the first event,
        // and on a phone that is a touchstart, which does not count as one.
        const gestures = ["touchend", "pointerup", "mousedown", "keydown", "click"];
        const unlock = function () {
            if (audio_context.state === "running") {
                gestures.forEach(function (name) {
                    document.removeEventListener(name, unlock);
                });
                return;
            }
            audio_context.resume();
            // Older Safari also wants something played inside the gesture.
            const silence = audio_context.createBufferSource();
            silence.buffer = audio_context.createBuffer(1, 1, 22050);
            silence.connect(audio_context.destination);
            silence.start(0);
        };
        gestures.forEach(function (name) {
            document.addEventListener(name, unlock);
        });

        // A hidden tab stops drawing frames, so looped sounds would hold
        // whatever volume they had; let the tab go quiet instead.
        document.addEventListener("visibilitychange", function () {
            if (document.hidden) {
                audio_context.suspend();
            } else {
                audio_context.resume();
            }
        });
    }
}

function audio_add_buffer(content, content_len) {
    let content_array = wasm_memory.buffer.slice(content, content + content_len);

    let sound_key = sound_key_next;
    sound_key_next += 1;

    audio_context.decodeAudioData(content_array, function(buffer) {
        sounds.set(sound_key, buffer);
    }, function(e) {
        // fail
        console.error("Failed to decode audio buffer", e);
    });
    return sound_key;
}

function audio_source_is_loaded(sound_key) {
    return sounds.has(sound_key) && sounds.get(sound_key) != undefined;
}

function recycle_playback() {
    let playback = playbacks.find(playback => playback.sound_key === 0);

    if (playback != null) {
        playback.source = audio_context.createBufferSource();
    } else {
        playback = {
            sound_key: 0,
            playback_key: 0,
            source: audio_context.createBufferSource(),
            gain_node: audio_context.createGain(),
            ended: null,
        };

        playbacks.push(playback);
    }

    return playback;
}

function stop(playback) {
    try {
        playback.source.removeEventListener('ended', playback.ended);

        // A looping source keeps running, unheard, until it is stopped.
        try {
            playback.source.stop();
        } catch (e) {
            // It never started or has already ended.
        }
        playback.source.disconnect();
        playback.gain_node.disconnect();

        playback.sound_key = 0;
        playback.playback_key = 0;
    } catch (e) {
        console.error("Error stopping sound", e);
    }
}

// Volume changes glide over about 10 ms instead of jumping, so volumes set
// every frame do not click.
function glide_gain(gain_node, volume) {
    gain_node.gain.setTargetAtTime(volume, audio_context.currentTime, 0.01);
}

function audio_play_buffer(sound_key, volume, repeat) {
    let playback_key = playback_key_next++;

    let pb = recycle_playback();

    pb.sound_key = sound_key;
    pb.playback_key = playback_key;

    pb.source.connect(pb.gain_node);
    pb.gain_node.connect(audio_output);

    // A recycled gain node may still be gliding toward its last volume.
    pb.gain_node.gain.cancelScheduledValues(0);
    pb.gain_node.gain.value = volume;
    pb.source.loop = repeat;

    pb.ended = function() {
        stop(pb);
    };
    pb.source.addEventListener('ended', pb.ended);

    try {
        pb.source.buffer = sounds.get(sound_key);
        pb.source.start(0);
    } catch (e) {
        console.error("Error starting sound", e);
    }

    return playback_key;
}

function audio_source_set_volume(sound_key, volume) {
    playbacks.forEach(playback => {
        if (playback.sound_key === sound_key) {
            glide_gain(playback.gain_node, volume);
        }
    });
}

function audio_source_stop(sound_key) {
    playbacks.forEach(playback => {
        playback.sound_key === sound_key && stop(playback);
    });
}

function audio_source_delete(sound_key) {
    audio_source_stop(sound_key);

    sounds.delete(sound_key);
}

function audio_playback_stop(playback_key) {
    let playback = playbacks.find(playback => playback.playback_key === playback_key);

    playback != null && stop(playback);
}

function audio_playback_set_volume(playback_key, volume) {
    let playback = playbacks.find(playback => playback.playback_key === playback_key);

    if (playback != null) {
        glide_gain(playback.gain_node, volume);
    }
}

function register_plugin(importObject) {
    importObject.env.audio_init = audio_init;
    importObject.env.audio_add_buffer = audio_add_buffer;
    importObject.env.audio_play_buffer = audio_play_buffer;
    importObject.env.audio_source_is_loaded = audio_source_is_loaded;
    importObject.env.audio_source_set_volume = audio_source_set_volume;
    importObject.env.audio_source_stop = audio_source_stop;
    importObject.env.audio_source_delete = audio_source_delete;
    importObject.env.audio_playback_stop = audio_playback_stop;
    importObject.env.audio_playback_set_volume = audio_playback_set_volume;
}

miniquad_add_plugin({ register_plugin, version: 1, name: "macroquad_audio" });
