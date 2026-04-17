use crate::lock_free::{LockFreeSynth, MidiEvent, MidiEventQueue};
use crate::synthesizer::Synthesizer;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream, StreamConfig};
use std::sync::Arc;

pub struct AudioEngine {
    _stream: Stream,
}

impl AudioEngine {
    pub fn new(
        lock_free_synth: Arc<LockFreeSynth>,
        midi_events: Arc<MidiEventQueue>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No output device available")?;
        let config = device
            .default_output_config()
            .map_err(|e| format!("No default output config: {}", e))?;

        let sample_rate = config.sample_rate();
        log::info!(
            "Audio engine initialized with {} Hz sample rate",
            sample_rate
        );

        let stream = match config.sample_format() {
            SampleFormat::F32 => Self::run::<f32>(
                &device,
                &config.into(),
                lock_free_synth,
                midi_events,
                sample_rate,
            )?,
            SampleFormat::I16 => Self::run::<i16>(
                &device,
                &config.into(),
                lock_free_synth,
                midi_events,
                sample_rate,
            )?,
            SampleFormat::U16 => Self::run::<u16>(
                &device,
                &config.into(),
                lock_free_synth,
                midi_events,
                sample_rate,
            )?,
            sample_format => {
                return Err(format!("Unsupported sample format: {:?}", sample_format).into());
            }
        };

        stream
            .play()
            .map_err(|e| format!("Failed to play stream: {}", e))?;

        Ok(Self { _stream: stream })
    }

    fn run<T>(
        device: &cpal::Device,
        config: &StreamConfig,
        lock_free_synth: Arc<LockFreeSynth>,
        midi_events: Arc<MidiEventQueue>,
        sample_rate: u32,
    ) -> Result<Stream, Box<dyn std::error::Error>>
    where
        T: Sample + cpal::SizedSample + cpal::FromSample<f32>,
    {
        let channels = config.channels as usize;

        // Synthesizer lives exclusively in the audio thread
        let mut synthesizer = Synthesizer::new();
        synthesizer.sample_rate = sample_rate as f32;

        // Cache preset list before entering the real-time callback.
        // list_presets() does filesystem I/O which must never run on the audio thread.
        let preset_names: std::sync::Arc<Vec<String>> =
            std::sync::Arc::new(Synthesizer::list_presets());

        // Pre-allocated stereo buffers
        let mut left_buffer = vec![0.0f32; 1024];
        let mut right_buffer = vec![0.0f32; 1024];
        let mut over_left: Vec<f32> = Vec::with_capacity(4096);
        let mut over_right: Vec<f32> = Vec::with_capacity(4096);

        // MIDI clock timing: tracks time between ticks using Instant (acceptable overhead at 24ppq)
        let mut last_clock_instant = std::time::Instant::now();

        // A-440 reference tone phase accumulator (lives in audio thread only)
        let mut ref_tone_phase = 0.0f32;

        let err_fn = |err| log::error!("Audio stream error: {}", err);

        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    // 1. Process MIDI events
                    for event in midi_events.drain() {
                        match event {
                            MidiEvent::NoteOn { note, velocity } => {
                                synthesizer.note_on(note, velocity);
                            }
                            MidiEvent::NoteOff { note } => {
                                synthesizer.note_off(note);
                            }
                            MidiEvent::SustainPedal { pressed } => {
                                synthesizer.sustain_pedal(pressed);
                            }
                            MidiEvent::ProgramChange { program } => {
                                if !preset_names.is_empty() {
                                    let idx = (program as usize) % preset_names.len();
                                    if let Err(e) = synthesizer.load_preset(&preset_names[idx]) {
                                        log::warn!("Program Change: failed to load preset '{}': {}", preset_names[idx], e);
                                    } else {
                                        log::info!("Program Change {}: loaded '{}'", program, preset_names[idx]);
                                    }
                                }
                            }
                            MidiEvent::MidiClock => {
                                let now = std::time::Instant::now();
                                let dt = now.duration_since(last_clock_instant).as_secs_f32();
                                last_clock_instant = now;
                                synthesizer.midi_clock_tick(dt);
                            }
                            MidiEvent::MidiClockStart | MidiEvent::MidiClockContinue => {
                                synthesizer.midi_clock_running = true;
                                synthesizer.midi_clock_tick_acc = 0.0;
                                synthesizer.midi_clock_tick_count = 0;
                                last_clock_instant = std::time::Instant::now();
                                log::info!("MIDI clock started");
                            }
                            MidiEvent::MidiClockStop => {
                                synthesizer.midi_clock_running = false;
                                log::info!("MIDI clock stopped");
                            }
                            MidiEvent::SysExRequest => {
                                // Guardar preset actual. Nota: I/O en audio thread es no-ideal
                                // pero SysEx es tan raro que el dropout es aceptable.
                                if let Err(e) = synthesizer.save_preset("sysex_dump") {
                                    log::warn!("SysEx dump failed: {}", e);
                                } else {
                                    log::info!("SysEx: preset guardado como sysex_dump");
                                }
                            }
                            MidiEvent::SysExPatch { data } => {
                                if let Ok(json_str) = std::str::from_utf8(&data) {
                                    if let Err(e) = synthesizer.load_preset_from_json(json_str) {
                                        log::warn!("SysEx patch load failed: {}", e);
                                    }
                                } else {
                                    log::warn!("SysEx: datos no son UTF-8 válido");
                                }
                            }
                            MidiEvent::AllNotesOff => {
                                synthesizer.all_notes_off();
                            }
                        }
                    }

                    // 2. Apply parameters from GUI/MIDI (lock-free read)
                    let params = lock_free_synth.get_params();
                    synthesizer.apply_params(params);

                    // 3. Process audio
                    let frames = data.len() / channels;
                    if left_buffer.len() < frames {
                        left_buffer.resize(frames, 0.0);
                        right_buffer.resize(frames, 0.0);
                    }
                    for i in 0..frames {
                        left_buffer[i] = 0.0;
                        right_buffer[i] = 0.0;
                    }

                    let cur_params = lock_free_synth.get_params();
                    if cur_params.reference_tone {
                        let phase_inc = 440.0 / sample_rate as f32;
                        let vol = cur_params.master_volume;
                        for i in 0..frames {
                            let s = (ref_tone_phase * 2.0 * std::f32::consts::PI).sin() * vol;
                            left_buffer[i] = s;
                            right_buffer[i] = s;
                            ref_tone_phase = (ref_tone_phase + phase_inc) % 1.0;
                        }
                    } else {
                        synthesizer.process_block_oversampled(
                            &mut left_buffer[..frames],
                            &mut right_buffer[..frames],
                            &mut over_left,
                            &mut over_right,
                        );
                    }

                    // 4. Apply limiting
                    for i in 0..frames {
                        left_buffer[i] = Self::soft_limiter(left_buffer[i].clamp(-1.0, 1.0));
                        right_buffer[i] = Self::soft_limiter(right_buffer[i].clamp(-1.0, 1.0));
                    }

                    // 5. Update VU meter peak with slow decay (max of both channels)
                    let block_peak = (0..frames).fold(0.0f32, |a, i| {
                        a.max(left_buffer[i].abs()).max(right_buffer[i].abs())
                    });
                    let stored = f32::from_bits(lock_free_synth.peak_level.load(std::sync::atomic::Ordering::Relaxed));
                    let decayed = (stored - 0.003).max(0.0);
                    let new_peak = block_peak.max(decayed);
                    lock_free_synth.peak_level.store(new_peak.to_bits(), std::sync::atomic::Ordering::Relaxed);

                    // 6. Convert stereo to multi-channel output
                    for frame_idx in 0..frames {
                        let l = left_buffer[frame_idx];
                        let r = right_buffer[frame_idx];
                        for ch in 0..channels {
                            let out_idx = frame_idx * channels + ch;
                            if out_idx < data.len() {
                                let s = match ch {
                                    0 => l,
                                    1 => r,
                                    _ => (l + r) * 0.5,
                                };
                                data[out_idx] = T::from_sample(s);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| format!("Failed to build output stream: {}", e))?;

        Ok(stream)
    }

    fn soft_limiter(x: f32) -> f32 {
        if x.abs() <= 0.8 {
            x
        } else {
            let sign = x.signum();
            sign * (0.8 + 0.2 * (1.0 - (-5.0 * (x.abs() - 0.8)).exp()))
        }
    }
}
