use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Sample, SampleFormat, Stream, StreamConfig};
use std::sync::{Arc, Mutex};
use crate::synthesizer::Synthesizer;

pub struct AudioEngine {
    _stream: Stream,
}

impl AudioEngine {
    pub fn new(synthesizer: Arc<Mutex<Synthesizer>>) -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().expect("No output device available");
        let config = device.default_output_config().expect("No default output config");

        let stream = match config.sample_format() {
            SampleFormat::F32 => Self::run::<f32>(&device, &config.into(), synthesizer),
            SampleFormat::I16 => Self::run::<i16>(&device, &config.into(), synthesizer),
            SampleFormat::U16 => Self::run::<u16>(&device, &config.into(), synthesizer),
            _ => panic!("Unsupported sample format"),
        };

        stream.play().expect("Failed to play stream");

        Self { _stream: stream }
    }

    fn run<T>(
        device: &Device,
        config: &StreamConfig,
        synthesizer: Arc<Mutex<Synthesizer>>,
    ) -> Stream
    where
        T: Sample + cpal::SizedSample + cpal::FromSample<f32>,
    {
        let channels = config.channels as usize;

        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    Self::write_data(data, channels, &synthesizer)
                },
                err_fn,
                None,
            )
            .expect("Failed to build output stream")
    }

    fn write_data<T>(output: &mut [T], channels: usize, synthesizer: &Arc<Mutex<Synthesizer>>)
    where
        T: Sample + cpal::FromSample<f32>,
    {
        let mut synth = synthesizer.lock().unwrap();
        
        // Create a temporary buffer for mono audio
        let frames = output.len() / channels;
        let mut mono_buffer = vec![0.0f32; frames];
        
        // Process audio
        synth.process_block(&mut mono_buffer);
        
        // Convert mono to multi-channel and write to output
        for (frame_idx, &sample) in mono_buffer.iter().enumerate() {
            for channel in 0..channels {
                let output_idx = frame_idx * channels + channel;
                if output_idx < output.len() {
                    output[output_idx] = T::from_sample(sample);
                }
            }
        }
    }
}