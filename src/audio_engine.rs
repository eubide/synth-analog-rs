use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Sample, SampleFormat, Stream, StreamConfig};
use std::sync::{Arc, Mutex};
use crate::synthesizer::Synthesizer;

pub struct AudioEngine {
    _stream: Stream,
    _buffer: Arc<Mutex<Vec<f32>>>, // Pre-allocated buffer to avoid allocations
}

impl AudioEngine {
    pub fn new(synthesizer: Arc<Mutex<Synthesizer>>) -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or("No output device available")?;
        let config = device.default_output_config()
            .map_err(|e| format!("No default output config: {}", e))?;

        // Pre-allocate buffer with maximum expected size (e.g., 1024 frames)
        let buffer = Arc::new(Mutex::new(vec![0.0f32; 1024]));
        let buffer_clone = buffer.clone();

        let stream = match config.sample_format() {
            SampleFormat::F32 => Self::run::<f32>(&device, &config.into(), synthesizer, buffer_clone)?,
            SampleFormat::I16 => Self::run::<i16>(&device, &config.into(), synthesizer, buffer_clone)?,
            SampleFormat::U16 => Self::run::<u16>(&device, &config.into(), synthesizer, buffer_clone)?,
            sample_format => return Err(format!("Unsupported sample format: {:?}", sample_format).into()),
        };

        stream.play().map_err(|e| format!("Failed to play stream: {}", e))?;

        Ok(Self { 
            _stream: stream,
            _buffer: buffer,
        })
    }

    fn run<T>(
        device: &Device,
        config: &StreamConfig,
        synthesizer: Arc<Mutex<Synthesizer>>,
        buffer: Arc<Mutex<Vec<f32>>>,
    ) -> Result<Stream, Box<dyn std::error::Error>>
    where
        T: Sample + cpal::SizedSample + cpal::FromSample<f32>,
    {
        let channels = config.channels as usize;

        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    Self::write_data(data, channels, &synthesizer, &buffer)
                },
                err_fn,
                None,
            )
            .map_err(|e| format!("Failed to build output stream: {}", e))?;
        
        Ok(stream)
    }

    fn write_data<T>(
        output: &mut [T], 
        channels: usize, 
        synthesizer: &Arc<Mutex<Synthesizer>>,
        buffer: &Arc<Mutex<Vec<f32>>>
    )
    where
        T: Sample + cpal::FromSample<f32>,
    {
        // Try to acquire locks with timeout to prevent blocking
        let synth_result = synthesizer.try_lock();
        let buffer_result = buffer.try_lock();
        
        match (synth_result, buffer_result) {
            (Ok(mut synth), Ok(mut mono_buffer)) => {
                let frames = output.len() / channels;
                
                // Resize pre-allocated buffer if needed (rare case)
                if mono_buffer.len() < frames {
                    mono_buffer.resize(frames, 0.0);
                }
                
                // Clear buffer and process audio
                for sample in mono_buffer.iter_mut().take(frames) {
                    *sample = 0.0;
                }
                
                synth.process_block(&mut mono_buffer[..frames]);
                
                // Apply safety limiting to prevent audio damage
                for sample in mono_buffer.iter_mut().take(frames) {
                    *sample = sample.clamp(-1.0, 1.0); // Hard limiter
                    *sample = Self::soft_limiter(*sample); // Soft limiter for better sound
                }
                
                // Convert mono to multi-channel and write to output
                for (frame_idx, &sample) in mono_buffer.iter().take(frames).enumerate() {
                    for channel in 0..channels {
                        let output_idx = frame_idx * channels + channel;
                        if output_idx < output.len() {
                            output[output_idx] = T::from_sample(sample);
                        }
                    }
                }
            },
            _ => {
                // If we can't acquire locks, output silence to prevent audio glitches
                for sample in output.iter_mut() {
                    *sample = T::from_sample(0.0);
                }
            }
        }
    }
    
    // Soft limiter to prevent harsh clipping while maintaining audio integrity
    fn soft_limiter(x: f32) -> f32 {
        if x.abs() <= 0.8 {
            x
        } else {
            let sign = x.signum();
            sign * (0.8 + 0.2 * (1.0 - (-5.0 * (x.abs() - 0.8)).exp()))
        }
    }
}