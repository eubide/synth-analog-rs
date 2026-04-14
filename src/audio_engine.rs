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

        // Pre-allocated mono buffer
        let mut mono_buffer = vec![0.0f32; 1024];

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
                            MidiEvent::SustainPedal { pressed: _pressed } => {
                                // Sustain pedal - future enhancement
                            }
                        }
                    }

                    // 2. Apply parameters from GUI/MIDI (lock-free read)
                    let params = lock_free_synth.get_params();
                    synthesizer.apply_params(params);

                    // 3. Process audio
                    let frames = data.len() / channels;
                    if mono_buffer.len() < frames {
                        mono_buffer.resize(frames, 0.0);
                    }
                    for sample in mono_buffer.iter_mut().take(frames) {
                        *sample = 0.0;
                    }

                    synthesizer.process_block(&mut mono_buffer[..frames]);

                    // 4. Apply limiting
                    for sample in mono_buffer.iter_mut().take(frames) {
                        *sample = sample.clamp(-1.0, 1.0);
                        *sample = Self::soft_limiter(*sample);
                    }

                    // 5. Convert mono to multi-channel
                    for (frame_idx, &sample) in mono_buffer.iter().take(frames).enumerate() {
                        for channel in 0..channels {
                            let output_idx = frame_idx * channels + channel;
                            if output_idx < data.len() {
                                data[output_idx] = T::from_sample(sample);
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
