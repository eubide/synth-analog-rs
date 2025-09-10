use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaveType {
    Sine,
    Square,
    Triangle,
    Sawtooth,
}


#[derive(Debug, Clone)]
pub struct OscillatorParams {
    pub wave_type: WaveType,
    pub amplitude: f32,
    pub detune: f32,
    pub pulse_width: f32,
}

#[derive(Debug, Clone)]
pub struct FilterParams {
    pub cutoff: f32,
    pub resonance: f32,
    pub envelope_amount: f32,
    pub keyboard_tracking: f32,
}

#[derive(Debug, Clone)]
pub struct EnvelopeParams {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

#[derive(Debug, Clone)]
pub struct LfoParams {
    pub frequency: f32,
    pub amplitude: f32,
    pub target_osc1_pitch: bool,
    pub target_osc2_pitch: bool,
    pub target_filter: bool,
    pub target_amplitude: bool,
}

#[derive(Debug, Clone)]
pub struct MixerParams {
    pub osc1_level: f32,
    pub osc2_level: f32,
    pub noise_level: f32,
}

pub struct Voice {
    pub frequency: f32,
    pub note: u8,
    pub phase1: f32,
    pub phase2: f32,
    pub envelope_state: EnvelopeState,
    pub envelope_time: f32,
    pub envelope_value: f32,
    pub filter_envelope_state: EnvelopeState,
    pub filter_envelope_time: f32,
    pub filter_envelope_value: f32,
    pub filter_state: FilterState,
    pub is_active: bool,
    pub sustain_time: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum EnvelopeState {
    Attack,
    Decay,
    Sustain,
    Release,
    Idle,
}

#[derive(Debug, Clone)]
pub struct FilterState {
    pub x1: f32,
    pub x2: f32,
    pub y1: f32,
    pub y2: f32,
    pub dc_x1: f32,
    pub dc_y1: f32,
}

pub struct Synthesizer {
    pub osc1: OscillatorParams,
    pub osc2: OscillatorParams,
    pub osc2_sync: bool,
    pub mixer: MixerParams,
    pub filter: FilterParams,
    pub filter_envelope: EnvelopeParams,
    pub amp_envelope: EnvelopeParams,
    pub lfo: LfoParams,
    pub master_volume: f32,
    pub voices: Vec<Voice>,
    pub sample_rate: f32,
    pub lfo_phase: f32,
}

impl Default for OscillatorParams {
    fn default() -> Self {
        Self {
            wave_type: WaveType::Sawtooth,
            amplitude: 0.5,
            detune: 0.0,
            pulse_width: 0.5,
        }
    }
}

impl Default for FilterParams {
    fn default() -> Self {
        Self {
            cutoff: 1000.0,
            resonance: 1.0,
            envelope_amount: 0.0,
            keyboard_tracking: 0.0,
        }
    }
}

impl Default for EnvelopeParams {
    fn default() -> Self {
        Self {
            attack: 0.1,
            decay: 0.3,
            sustain: 0.7,
            release: 0.5,
        }
    }
}

impl Default for LfoParams {
    fn default() -> Self {
        Self {
            frequency: 2.0,
            amplitude: 0.1,
            target_osc1_pitch: false,
            target_osc2_pitch: false,
            target_filter: false,
            target_amplitude: false,
        }
    }
}

impl Default for MixerParams {
    fn default() -> Self {
        Self {
            osc1_level: 0.5,
            osc2_level: 0.5,
            noise_level: 0.0,
        }
    }
}

impl Voice {
    pub fn new(note: u8, frequency: f32) -> Self {
        Self {
            frequency,
            note,
            phase1: 0.0,
            phase2: 0.0,
            envelope_state: EnvelopeState::Attack,
            envelope_time: 0.0,
            envelope_value: 0.0,
            filter_envelope_state: EnvelopeState::Attack,
            filter_envelope_time: 0.0,
            filter_envelope_value: 0.0,
            filter_state: FilterState {
                x1: 0.0,
                x2: 0.0,
                y1: 0.0,
                y2: 0.0,
                dc_x1: 0.0,
                dc_y1: 0.0,
            },
            is_active: true,
            sustain_time: 0.0,
        }
    }

    pub fn release(&mut self) {
        match self.envelope_state {
            EnvelopeState::Attack | EnvelopeState::Decay | EnvelopeState::Sustain => {
                self.envelope_state = EnvelopeState::Release;
                self.envelope_time = 0.0;
                // Keep current envelope_value as release starting point
            },
            _ => {} // Already in release or idle
        }
        match self.filter_envelope_state {
            EnvelopeState::Attack | EnvelopeState::Decay | EnvelopeState::Sustain => {
                self.filter_envelope_state = EnvelopeState::Release;
                self.filter_envelope_time = 0.0;
                // Keep current filter_envelope_value as release starting point
            },
            _ => {} // Already in release or idle
        }
    }
}

impl Synthesizer {
    pub fn new() -> Self {
        Self {
            osc1: OscillatorParams::default(),
            osc2: OscillatorParams::default(),
            osc2_sync: false,
            mixer: MixerParams::default(),
            filter: FilterParams::default(),
            filter_envelope: EnvelopeParams::default(),
            amp_envelope: EnvelopeParams::default(),
            lfo: LfoParams::default(),
            master_volume: 0.5,
            voices: Vec::new(),
            sample_rate: 44100.0,
            lfo_phase: 0.0,
        }
    }

    pub fn note_on(&mut self, note: u8) {
        let frequency = Self::note_to_frequency(note);
        
        // Check if note is already playing - for intentional re-triggering, we restart it
        for voice in &mut self.voices {
            if voice.note == note {
                // Restart the note with a smooth transition to avoid clicks
                if voice.is_active && voice.envelope_value > 0.1 {
                    // If note is loud, do a quick fade to avoid click
                    voice.envelope_state = EnvelopeState::Attack;
                    voice.envelope_time = 0.0;
                    voice.envelope_value *= 0.5; // Quick fade instead of hard reset
                    voice.filter_envelope_state = EnvelopeState::Attack;
                    voice.filter_envelope_time = 0.0;
                    voice.filter_envelope_value *= 0.5;
                } else {
                    // If note is quiet or inactive, full restart is fine
                    *voice = Voice::new(note, frequency);
                }
                voice.frequency = frequency;
                voice.is_active = true;
                return;
            }
        }
        
        // Find an inactive voice or create new one
        if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active) {
            *voice = Voice::new(note, frequency);
        } else {
            self.voices.push(Voice::new(note, frequency));
        }
    }

    pub fn note_off(&mut self, note: u8) {
        for voice in &mut self.voices {
            if voice.note == note && voice.is_active {
                voice.release();
            }
        }
    }

    pub fn note_to_frequency(note: u8) -> f32 {
        // A4 = 69, 440 Hz
        440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
    }

    pub fn process_block(&mut self, buffer: &mut [f32]) {
        let dt = 1.0 / self.sample_rate;
        
        // Copy synth parameters to avoid borrowing issues
        let osc1_wave_type = self.osc1.wave_type;
        let osc1_amplitude = self.osc1.amplitude;
        let osc1_detune = self.osc1.detune;
        let osc1_pulse_width = self.osc1.pulse_width;
        let osc2_wave_type = self.osc2.wave_type;
        let osc2_amplitude = self.osc2.amplitude;
        let osc2_detune = self.osc2.detune;
        let osc2_pulse_width = self.osc2.pulse_width;
        let osc2_sync = self.osc2_sync;
        let mixer_osc1_level = self.mixer.osc1_level;
        let mixer_osc2_level = self.mixer.osc2_level;
        let mixer_noise_level = self.mixer.noise_level;
        let filter_cutoff = self.filter.cutoff;
        let filter_resonance = self.filter.resonance;
        let filter_envelope_amount = self.filter.envelope_amount;
        let filter_keyboard_tracking = self.filter.keyboard_tracking;
        let envelope_attack = self.amp_envelope.attack;
        let envelope_decay = self.amp_envelope.decay;
        let envelope_sustain = self.amp_envelope.sustain;
        let envelope_release = self.amp_envelope.release;
        let filter_envelope_attack = self.filter_envelope.attack;
        let filter_envelope_decay = self.filter_envelope.decay;
        let filter_envelope_sustain = self.filter_envelope.sustain;
        let filter_envelope_release = self.filter_envelope.release;
        let lfo_frequency = self.lfo.frequency;
        let lfo_amplitude = self.lfo.amplitude;
        let lfo_target_osc1_pitch = self.lfo.target_osc1_pitch;
        let lfo_target_osc2_pitch = self.lfo.target_osc2_pitch;
        let lfo_target_amplitude = self.lfo.target_amplitude;
        let master_volume = self.master_volume;
        let sample_rate = self.sample_rate;
        
        for sample in buffer.iter_mut() {
            *sample = 0.0;
            
            // Update LFO with proper phase wrapping
            self.lfo_phase = (self.lfo_phase + lfo_frequency * dt) % 1.0;
            if self.lfo_phase < 0.0 { self.lfo_phase += 1.0; }
            let lfo_value = (self.lfo_phase * 2.0 * PI).sin() * lfo_amplitude;
            
            // Process all active voices
            for voice in &mut self.voices {
                if !voice.is_active {
                    continue;
                }
                
                // Calculate frequencies with detune and LFO modulation
                let mut freq1 = voice.frequency * (1.0 + osc1_detune / 100.0);
                let mut freq2 = voice.frequency * (1.0 + osc2_detune / 100.0);
                
                if lfo_target_osc1_pitch {
                    freq1 *= 1.0 + lfo_value;
                }
                if lfo_target_osc2_pitch {
                    freq2 *= 1.0 + lfo_value;
                }
                
                // Update phases with proper wrapping to prevent drift
                voice.phase1 = (voice.phase1 + freq1 * dt) % 1.0;
                voice.phase2 = (voice.phase2 + freq2 * dt) % 1.0;
                
                // Ensure phases stay in valid range
                if voice.phase1 < 0.0 { voice.phase1 += 1.0; }
                if voice.phase2 < 0.0 { voice.phase2 += 1.0; }
                
                // Oscillator sync: if enabled, reset osc2 phase when osc1 completes a cycle
                let prev_phase1 = voice.phase1 - freq1 * dt;
                if osc2_sync && prev_phase1 < 0.0 && voice.phase1 >= 0.0 {
                    voice.phase2 = 0.0;
                }
                
                // Generate oscillator outputs
                let osc1_out = Self::generate_oscillator_static(osc1_wave_type, voice.phase1, osc1_pulse_width) * osc1_amplitude;
                let osc2_out = Self::generate_oscillator_static(osc2_wave_type, voice.phase2, osc2_pulse_width) * osc2_amplitude;
                
                // Mix oscillators with individual levels and add noise
                let noise = if mixer_noise_level > 0.0 {
                    (rand::random::<f32>() - 0.5) * 2.0 * mixer_noise_level
                } else {
                    0.0
                };
                let mut mixed = osc1_out * mixer_osc1_level + osc2_out * mixer_osc2_level + noise;
                
                // Apply LFO to amplitude if enabled
                if lfo_target_amplitude {
                    mixed *= 1.0 + lfo_value;
                }
                
                // Process filter envelope
                let filter_envelope_value = Self::process_filter_envelope_static(
                    voice,
                    filter_envelope_attack,
                    filter_envelope_decay,
                    filter_envelope_sustain,
                    filter_envelope_release,
                    sample_rate
                );
                
                // Apply filter envelope to cutoff and keyboard tracking
                let note_frequency = voice.frequency;
                let kbd_track_amount = filter_keyboard_tracking * ((note_frequency / 261.63) - 1.0); // C4 = 261.63 Hz as reference
                let modulated_cutoff = filter_cutoff * (1.0 + filter_envelope_amount * filter_envelope_value + kbd_track_amount);
                let final_cutoff = modulated_cutoff.clamp(20.0, 20000.0);
                
                // Apply filter
                mixed = Self::apply_biquad_filter_static(
                    mixed, 
                    &mut voice.filter_state, 
                    final_cutoff, 
                    filter_resonance, 
                    sample_rate
                );
                
                // Apply amp envelope
                let envelope_value = Self::process_envelope_static(
                    voice, 
                    envelope_attack, 
                    envelope_decay, 
                    envelope_sustain, 
                    envelope_release, 
                    sample_rate
                );
                mixed *= envelope_value;
                
                // Add to output
                *sample += mixed;
            }
            
            // Apply master volume with gentle compression
            *sample *= master_volume;
            
            // Gentle soft clipping using tanh for smoother distortion
            *sample = if sample.abs() > 0.7 {
                sample.signum() * (1.0 - (-sample.abs() * 3.0).exp())
            } else {
                *sample
            };
            
            // Final hard clipping as safety
            *sample = (*sample).clamp(-1.0, 1.0);
            
        }
    }

    fn generate_oscillator_static(wave_type: WaveType, phase: f32, pulse_width: f32) -> f32 {
        let phase = phase % 1.0;
        match wave_type {
            WaveType::Sine => (phase * 2.0 * PI).sin(),
            WaveType::Square => {
                // Pulse wave with variable width
                if phase < pulse_width { 1.0 } else { -1.0 }
            },
            WaveType::Triangle => {
                if phase < 0.5 {
                    4.0 * phase - 1.0
                } else {
                    3.0 - 4.0 * phase
                }
            },
            WaveType::Sawtooth => {
                // Band-limited sawtooth using sin harmonics
                let mut output = 0.0;
                for n in 1..=8 {
                    output += (n as f32 * phase * 2.0 * PI).sin() / n as f32;
                }
                -2.0 * output / PI
            },
        }
    }

    fn apply_biquad_filter_static(
        input: f32, 
        state: &mut FilterState,
        cutoff: f32,
        resonance: f32,
        sample_rate: f32
    ) -> f32 {
        // Clamp cutoff to valid range to prevent instability
        let cutoff = cutoff.clamp(20.0, sample_rate * 0.45);
        // Clamp resonance much more aggressively to prevent instability
        let resonance = resonance.clamp(0.7, 8.0); // Even more conservative range
        
        let omega = 2.0 * PI * cutoff / sample_rate;
        let sin_omega = omega.sin();
        let cos_omega = omega.cos();
        let alpha = sin_omega / (2.0 * resonance);

        // Lowpass filter coefficients (simplified to only lowpass)
        let b1 = 1.0 - cos_omega;
        let b0 = b1 / 2.0;
        let b2 = b0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_omega;
        let a2 = 1.0 - alpha;
        let (b0, b1, b2, a1, a2) = (b0/a0, b1/a0, b2/a0, a1/a0, a2/a0);

        let output = b0 * input + b1 * state.x1 + b2 * state.x2 - a1 * state.y1 - a2 * state.y2;

        // Prevent denormal numbers, NaN, and filter runaway
        let output = if output.is_finite() && output.abs() > 1e-15 && output.abs() < 10.0 {
            output
        } else {
            // Reset filter state if it goes unstable
            state.x1 = 0.0;
            state.x2 = 0.0;
            state.y1 = 0.0;
            state.y2 = 0.0;
            state.dc_x1 = 0.0;
            state.dc_y1 = 0.0;
            0.0
        };

        state.x2 = state.x1;
        state.x1 = input;
        state.y2 = state.y1;
        state.y1 = output;

        // Apply DC blocking filter to prevent buildup
        let dc_alpha = 0.995;
        let dc_blocked = output - state.dc_x1 + dc_alpha * state.dc_y1;
        state.dc_x1 = output;
        state.dc_y1 = dc_blocked;

        // Periodically reset filter state to prevent long-term drift
        if state.x1.abs() < 1e-10 && state.x2.abs() < 1e-10 && 
           state.y1.abs() < 1e-10 && state.y2.abs() < 1e-10 {
            state.x1 = 0.0;
            state.x2 = 0.0;
            state.y1 = 0.0;
            state.y2 = 0.0;
            state.dc_x1 = 0.0;
            state.dc_y1 = 0.0;
        }

        // Apply gentle saturation only if needed, otherwise pass through clean
        if dc_blocked.abs() > 0.95 {
            dc_blocked.tanh() * 0.95
        } else {
            dc_blocked
        }
    }

    fn process_envelope_static(
        voice: &mut Voice,
        attack: f32,
        decay: f32,
        sustain: f32,
        release: f32,
        sample_rate: f32
    ) -> f32 {
        let dt = 1.0 / sample_rate;
        voice.envelope_time += dt;

        match voice.envelope_state {
            EnvelopeState::Attack => {
                if attack <= 0.0 {
                    voice.envelope_value = 1.0;
                    voice.envelope_state = EnvelopeState::Decay;
                    voice.envelope_time = 0.0;
                } else {
                    voice.envelope_value = (voice.envelope_time / attack).min(1.0);
                    if voice.envelope_value >= 1.0 {
                        voice.envelope_state = EnvelopeState::Decay;
                        voice.envelope_time = 0.0;
                    }
                }
            },
            EnvelopeState::Decay => {
                if decay <= 0.0 {
                    voice.envelope_value = sustain;
                    voice.envelope_state = EnvelopeState::Sustain;
                } else {
                    let decay_progress = (voice.envelope_time / decay).min(1.0);
                    voice.envelope_value = 1.0 - decay_progress * (1.0 - sustain);
                    if decay_progress >= 1.0 {
                        voice.envelope_state = EnvelopeState::Sustain;
                    }
                }
            },
            EnvelopeState::Sustain => {
                voice.envelope_value = sustain;
                voice.sustain_time += dt;
                
                // Add small amount of noise reduction during sustain to prevent buildup
                if voice.sustain_time > 1.0 { // After 1 second of sustain
                    // Slightly reduce very small filter state values that can cause drift
                    if voice.filter_state.y1.abs() < 1e-8 { voice.filter_state.y1 = 0.0; }
                    if voice.filter_state.y2.abs() < 1e-8 { voice.filter_state.y2 = 0.0; }
                    if voice.filter_state.x1.abs() < 1e-8 { voice.filter_state.x1 = 0.0; }
                    if voice.filter_state.x2.abs() < 1e-8 { voice.filter_state.x2 = 0.0; }
                    
                    // Reset sustain timer to prevent constant checking
                    voice.sustain_time = 0.0;
                }
            },
            EnvelopeState::Release => {
                if release <= 0.001 {
                    voice.envelope_value = 0.0;
                    voice.is_active = false;
                    voice.envelope_state = EnvelopeState::Idle;
                } else {
                    // Use exponential decay for more natural release
                    let release_rate = 1.0 / release;
                    voice.envelope_value *= (1.0 - release_rate * dt).max(0.0);
                    
                    // Consider voice finished when very quiet
                    if voice.envelope_value < 0.001 {
                        voice.envelope_value = 0.0;
                        voice.is_active = false;
                        voice.envelope_state = EnvelopeState::Idle;
                    }
                }
            },
            EnvelopeState::Idle => {
                voice.envelope_value = 0.0;
                voice.is_active = false;
            },
        }

        voice.envelope_value
    }

    fn process_filter_envelope_static(
        voice: &mut Voice,
        attack: f32,
        decay: f32,
        sustain: f32,
        release: f32,
        sample_rate: f32
    ) -> f32 {
        let dt = 1.0 / sample_rate;
        voice.filter_envelope_time += dt;

        match voice.filter_envelope_state {
            EnvelopeState::Attack => {
                if attack <= 0.0 {
                    voice.filter_envelope_value = 1.0;
                    voice.filter_envelope_state = EnvelopeState::Decay;
                    voice.filter_envelope_time = 0.0;
                } else {
                    voice.filter_envelope_value = (voice.filter_envelope_time / attack).min(1.0);
                    if voice.filter_envelope_value >= 1.0 {
                        voice.filter_envelope_state = EnvelopeState::Decay;
                        voice.filter_envelope_time = 0.0;
                    }
                }
            },
            EnvelopeState::Decay => {
                if decay <= 0.0 {
                    voice.filter_envelope_value = sustain;
                    voice.filter_envelope_state = EnvelopeState::Sustain;
                } else {
                    let decay_progress = (voice.filter_envelope_time / decay).min(1.0);
                    voice.filter_envelope_value = 1.0 - decay_progress * (1.0 - sustain);
                    if decay_progress >= 1.0 {
                        voice.filter_envelope_state = EnvelopeState::Sustain;
                    }
                }
            },
            EnvelopeState::Sustain => {
                voice.filter_envelope_value = sustain;
            },
            EnvelopeState::Release => {
                if release <= 0.001 {
                    voice.filter_envelope_value = 0.0;
                    voice.filter_envelope_state = EnvelopeState::Idle;
                } else {
                    let release_rate = 1.0 / release;
                    voice.filter_envelope_value *= (1.0 - release_rate * dt).max(0.0);
                    
                    if voice.filter_envelope_value < 0.001 {
                        voice.filter_envelope_value = 0.0;
                        voice.filter_envelope_state = EnvelopeState::Idle;
                    }
                }
            },
            EnvelopeState::Idle => {
                voice.filter_envelope_value = 0.0;
            },
        }

        voice.filter_envelope_value
    }

}