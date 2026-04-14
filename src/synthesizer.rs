use std::f32::consts::PI;
use std::fs;
use std::path::Path;

use crate::optimization::OPTIMIZATION_TABLES;

// Phase accumulator constants to prevent drift
const PHASE_SCALE: u64 = 1u64 << 32; // 32-bit fractional phase
const PHASE_MASK: u64 = PHASE_SCALE - 1;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LfoWaveform {
    Triangle,
    Square,
    Sawtooth,
    ReverseSawtooth,
    SampleAndHold,
}

#[derive(Debug, Clone)]
pub struct LfoParams {
    pub frequency: f32,
    pub amplitude: f32,
    pub waveform: LfoWaveform,
    pub sync: bool, // Keyboard sync - resets LFO phase on note trigger
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

#[derive(Debug, Clone)]
pub struct ModulationMatrix {
    pub lfo_to_cutoff: f32,
    pub lfo_to_resonance: f32,
    pub lfo_to_osc1_pitch: f32,
    pub lfo_to_osc2_pitch: f32,
    pub lfo_to_amplitude: f32,
    pub velocity_to_cutoff: f32,
    pub velocity_to_amplitude: f32,
}

#[derive(Debug, Clone)]
pub struct EffectsParams {
    pub reverb_amount: f32,
    pub reverb_size: f32,
    pub delay_time: f32,
    pub delay_feedback: f32,
    pub delay_amount: f32,
}

#[derive(Debug, Clone)]
pub struct ArpeggiatorParams {
    pub enabled: bool,
    pub rate: f32,
    pub pattern: ArpPattern,
    pub octaves: u8,
    pub gate_length: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArpPattern {
    Up,
    Down,
    UpDown,
    Random,
}

#[derive(Debug, Clone)]
pub struct Preset {
    pub name: String,
    pub osc1: OscillatorParams,
    pub osc2: OscillatorParams,
    pub osc2_sync: bool,
    pub mixer: MixerParams,
    pub filter: FilterParams,
    pub filter_envelope: EnvelopeParams,
    pub amp_envelope: EnvelopeParams,
    pub lfo: LfoParams,
    pub modulation_matrix: ModulationMatrix,
    pub effects: EffectsParams,
    pub master_volume: f32,
}

pub struct Voice {
    pub frequency: f32,
    pub note: u8,
    pub velocity: f32,
    pub phase1_accumulator: u64, // Integer phase accumulator to prevent drift
    pub phase2_accumulator: u64,
    pub envelope_state: EnvelopeState,
    pub envelope_time: f32,
    pub envelope_value: f32,
    pub filter_envelope_state: EnvelopeState,
    pub filter_envelope_time: f32,
    pub filter_envelope_value: f32,
    pub filter_state: LadderFilterState,
    pub is_active: bool,
    pub sustain_time: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeState {
    Attack,
    Decay,
    Sustain,
    Release,
    Idle,
}

#[derive(Debug, Clone)]
pub struct LadderFilterState {
    // 4 cascaded 1-pole sections for 24dB/octave rolloff
    pub stage1: f32,
    pub stage2: f32,
    pub stage3: f32,
    pub stage4: f32,
    // Delayed samples for zero-delay feedback
    pub delay1: f32,
    pub delay2: f32,
    pub delay3: f32,
    pub delay4: f32,
    // Feedback amount for resonance
    pub feedback: f32,
    // DC blocking filter state
    pub dc_block_x1: f32,
    pub dc_block_x2: f32,
    pub dc_block_y1: f32,
    pub dc_block_y2: f32,
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
    pub modulation_matrix: ModulationMatrix,
    pub effects: EffectsParams,
    pub arpeggiator: ArpeggiatorParams,
    pub master_volume: f32,
    pub voices: Vec<Voice>,
    pub sample_rate: f32,
    pub lfo_phase_accumulator: u64, // Integer phase accumulator to prevent drift
    pub lfo_sample_hold_value: f32, // Current sample & hold value
    pub lfo_last_sample_time: f32,  // Time since last sample update
    pub max_polyphony: usize,
    pub delay_buffer: Vec<f32>,
    pub delay_index: usize,
    pub reverb_buffers: Vec<Vec<f32>>,
    pub reverb_indices: Vec<usize>,
    pub held_notes: Vec<u8>,
    pub arp_step: usize,
    pub arp_timer: f32,
    pub arp_note_timer: f32,
}

impl Default for OscillatorParams {
    fn default() -> Self {
        Self {
            wave_type: WaveType::Sawtooth,
            amplitude: 1.0,
            detune: 0.0,
            pulse_width: 0.5,
        }
    }
}

impl Default for FilterParams {
    fn default() -> Self {
        Self {
            cutoff: 5000.0,
            resonance: 1.0,
            envelope_amount: 0.0,
            keyboard_tracking: 0.0,
        }
    }
}

impl Default for EnvelopeParams {
    fn default() -> Self {
        Self {
            attack: 0.01,
            decay: 0.3,
            sustain: 0.8,
            release: 0.3,
        }
    }
}

impl Default for LfoParams {
    fn default() -> Self {
        Self {
            frequency: 2.0,
            amplitude: 0.1,
            waveform: LfoWaveform::Triangle, // Vintage analog default
            sync: false,
            target_osc1_pitch: false,
            target_osc2_pitch: false,
            target_filter: false,
            target_amplitude: false,
        }
    }
}

impl Default for ModulationMatrix {
    fn default() -> Self {
        Self {
            lfo_to_cutoff: 0.0,
            lfo_to_resonance: 0.0,
            lfo_to_osc1_pitch: 0.0,
            lfo_to_osc2_pitch: 0.0,
            lfo_to_amplitude: 0.0,
            velocity_to_cutoff: 0.0,
            velocity_to_amplitude: 0.5,
        }
    }
}

impl Default for EffectsParams {
    fn default() -> Self {
        Self {
            reverb_amount: 0.0,
            reverb_size: 0.5,
            delay_time: 0.25,
            delay_feedback: 0.3,
            delay_amount: 0.0,
        }
    }
}

impl Default for ArpeggiatorParams {
    fn default() -> Self {
        Self {
            enabled: false,
            rate: 120.0, // BPM
            pattern: ArpPattern::Up,
            octaves: 1,
            gate_length: 0.8,
        }
    }
}

impl Default for MixerParams {
    fn default() -> Self {
        Self {
            osc1_level: 0.8,
            osc2_level: 0.6,
            noise_level: 0.0,
        }
    }
}

impl Voice {
    pub fn new(note: u8, frequency: f32, velocity: f32) -> Self {
        Self {
            frequency,
            note,
            velocity,
            phase1_accumulator: 0,
            phase2_accumulator: 0,
            envelope_state: EnvelopeState::Attack,
            envelope_time: 0.0,
            envelope_value: 0.0,
            filter_envelope_state: EnvelopeState::Attack,
            filter_envelope_time: 0.0,
            filter_envelope_value: 0.0,
            filter_state: LadderFilterState {
                stage1: 0.0,
                stage2: 0.0,
                stage3: 0.0,
                stage4: 0.0,
                delay1: 0.0,
                delay2: 0.0,
                delay3: 0.0,
                delay4: 0.0,
                feedback: 0.0,
                dc_block_x1: 0.0,
                dc_block_x2: 0.0,
                dc_block_y1: 0.0,
                dc_block_y2: 0.0,
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
            }
            _ => {} // Already in release or idle
        }
        match self.filter_envelope_state {
            EnvelopeState::Attack | EnvelopeState::Decay | EnvelopeState::Sustain => {
                self.filter_envelope_state = EnvelopeState::Release;
                self.filter_envelope_time = 0.0;
                // Keep current filter_envelope_value as release starting point
            }
            _ => {} // Already in release or idle
        }
    }
}

impl Synthesizer {
    pub fn new() -> Self {
        let sample_rate = 44100.0;
        let max_delay_samples = (sample_rate * 2.0) as usize; // 2 second max delay
        let reverb_sizes = [
            (sample_rate * 0.025) as usize, // 25ms
            (sample_rate * 0.041) as usize, // 41ms
            (sample_rate * 0.059) as usize, // 59ms
            (sample_rate * 0.073) as usize, // 73ms
        ];

        Self {
            osc1: OscillatorParams::default(),
            osc2: OscillatorParams::default(),
            osc2_sync: false,
            mixer: MixerParams::default(),
            filter: FilterParams::default(),
            filter_envelope: EnvelopeParams::default(),
            amp_envelope: EnvelopeParams::default(),
            lfo: LfoParams::default(),
            modulation_matrix: ModulationMatrix::default(),
            effects: EffectsParams::default(),
            arpeggiator: ArpeggiatorParams::default(),
            master_volume: 0.7,
            voices: Vec::new(),
            sample_rate,
            lfo_phase_accumulator: 0,
            lfo_sample_hold_value: 0.0,
            lfo_last_sample_time: 0.0,
            max_polyphony: 8,
            delay_buffer: vec![0.0; max_delay_samples],
            delay_index: 0,
            reverb_buffers: reverb_sizes.iter().map(|&size| vec![0.0; size]).collect(),
            reverb_indices: vec![0; reverb_sizes.len()],
            held_notes: Vec::new(),
            arp_step: 0,
            arp_timer: 0.0,
            arp_note_timer: 0.0,
        }
    }

    pub fn note_on(&mut self, note: u8, velocity: u8) {
        if self.arpeggiator.enabled {
            // Add note to held notes if not already there
            if !self.held_notes.contains(&note) {
                self.held_notes.push(note);
                self.held_notes.sort();
            }
        } else {
            // Direct note triggering when arpeggiator is off
            self.trigger_note(note, velocity);
        }
    }

    fn trigger_note(&mut self, note: u8, velocity: u8) {
        let frequency = Self::note_to_frequency(note);
        let velocity_normalized = velocity as f32 / 127.0;

        // Reset LFO phase if keyboard sync is enabled
        if self.lfo.sync {
            self.lfo_phase_accumulator = 0;
            self.lfo_last_sample_time = 0.0;
            // Generate new sample & hold value for consistency
            self.lfo_sample_hold_value = (rand::random::<f32>() - 0.5) * 2.0;
        }

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
                    *voice = Voice::new(note, frequency, velocity_normalized);
                }
                voice.frequency = frequency;
                voice.is_active = true;
                return;
            }
        }

        // Find an inactive voice, create new one, or steal one
        if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active) {
            *voice = Voice::new(note, frequency, velocity_normalized);
        } else if self.voices.len() < self.max_polyphony {
            self.voices
                .push(Voice::new(note, frequency, velocity_normalized));
        } else {
            // Voice stealing: find the best voice to replace
            let steal_index = self.find_voice_to_steal();
            self.voices[steal_index] = Voice::new(note, frequency, velocity_normalized);
        }
    }

    pub fn note_off(&mut self, note: u8) {
        if self.arpeggiator.enabled {
            // Remove note from held notes
            self.held_notes.retain(|&n| n != note);

            // If no notes held, release all voices
            if self.held_notes.is_empty() {
                for voice in &mut self.voices {
                    if voice.is_active {
                        voice.release();
                    }
                }
            }
        } else {
            // Direct note release when arpeggiator is off
            for voice in &mut self.voices {
                if voice.note == note && voice.is_active {
                    voice.release();
                }
            }
        }
    }

    fn find_voice_to_steal(&self) -> usize {
        let mut best_index = 0;
        let mut best_score = f32::MIN;

        for (i, voice) in self.voices.iter().enumerate() {
            let mut score = 0.0;

            // Prefer voices in release phase
            if voice.envelope_state == EnvelopeState::Release {
                score += 100.0;
            }

            // Prefer quieter voices
            score += (1.0 - voice.envelope_value) * 50.0;

            // Prefer older voices (longer time in current state)
            score += voice.envelope_time * 10.0;

            // Prefer voices with lower amplitude
            if voice.envelope_state != EnvelopeState::Attack {
                score += (1.0 - voice.envelope_value) * 30.0;
            }

            if score > best_score {
                best_score = score;
                best_index = i;
            }
        }

        best_index
    }

    pub fn note_to_frequency(note: u8) -> f32 {
        OPTIMIZATION_TABLES.get_midi_frequency(note)
    }

    fn generate_lfo_waveform(waveform: LfoWaveform, phase: f32, sample_hold_value: f32) -> f32 {
        let phase = phase % 1.0;
        match waveform {
            LfoWaveform::Triangle => {
                // Triangle wave: rises from -1 to 1, then falls back to -1
                if phase < 0.5 {
                    -1.0 + 4.0 * phase // Rising edge: -1 to 1
                } else {
                    3.0 - 4.0 * phase // Falling edge: 1 to -1
                }
            }
            LfoWaveform::Square => {
                // Square wave: -1 for first half, +1 for second half
                if phase < 0.5 { -1.0 } else { 1.0 }
            }
            LfoWaveform::Sawtooth => {
                // Sawtooth wave: rises from -1 to 1 linearly
                -1.0 + 2.0 * phase
            }
            LfoWaveform::ReverseSawtooth => {
                // Reverse sawtooth: falls from 1 to -1 linearly
                1.0 - 2.0 * phase
            }
            LfoWaveform::SampleAndHold => {
                // Sample and hold: constant value until next sample
                sample_hold_value
            }
        }
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
        let lfo_waveform = self.lfo.waveform;
        let modulation_matrix = self.modulation_matrix.clone();
        let master_volume = self.master_volume;
        let sample_rate = self.sample_rate;

        for sample in buffer.iter_mut() {
            *sample = 0.0;

            // Update arpeggiator
            self.update_arpeggiator(dt);

            // Update LFO using integer phase accumulator to prevent drift
            let lfo_phase_increment =
                ((lfo_frequency / self.sample_rate) * PHASE_SCALE as f32) as u64;
            self.lfo_phase_accumulator =
                self.lfo_phase_accumulator.wrapping_add(lfo_phase_increment);

            // Update sample & hold if needed (at ~100Hz rate)
            self.lfo_last_sample_time += dt;
            if lfo_waveform == LfoWaveform::SampleAndHold && self.lfo_last_sample_time >= 0.01 {
                self.lfo_sample_hold_value = (rand::random::<f32>() - 0.5) * 2.0;
                self.lfo_last_sample_time = 0.0;
            }

            // Generate LFO value using the selected waveform
            // Convert accumulator to phase (0.0 to 1.0)
            let lfo_phase = (self.lfo_phase_accumulator & PHASE_MASK) as f32 / PHASE_SCALE as f32;
            let lfo_value =
                Self::generate_lfo_waveform(lfo_waveform, lfo_phase, self.lfo_sample_hold_value)
                    * lfo_amplitude;

            // Process all active voices
            for voice in &mut self.voices {
                if !voice.is_active {
                    continue;
                }

                // Calculate frequencies with detune and modulation matrix
                let mut freq1 = voice.frequency * (1.0 + osc1_detune / 100.0);
                let mut freq2 = voice.frequency * (1.0 + osc2_detune / 100.0);

                // Apply modulation matrix to oscillator pitch
                freq1 *= 1.0 + (lfo_value * modulation_matrix.lfo_to_osc1_pitch * 0.1);
                freq2 *= 1.0 + (lfo_value * modulation_matrix.lfo_to_osc2_pitch * 0.1);

                // Update phases using integer accumulators to prevent drift
                let phase1_increment = ((freq1 / self.sample_rate) * PHASE_SCALE as f32) as u64;
                let phase2_increment = ((freq2 / self.sample_rate) * PHASE_SCALE as f32) as u64;

                voice.phase1_accumulator = voice.phase1_accumulator.wrapping_add(phase1_increment);
                voice.phase2_accumulator = voice.phase2_accumulator.wrapping_add(phase2_increment);

                // Convert accumulators to phase values (0.0 to 1.0)
                let phase1 = (voice.phase1_accumulator & PHASE_MASK) as f32 / PHASE_SCALE as f32;
                let mut phase2 =
                    (voice.phase2_accumulator & PHASE_MASK) as f32 / PHASE_SCALE as f32;

                // Oscillator sync: if enabled, reset osc2 phase when osc1 completes a cycle
                let prev_phase1_accumulator =
                    voice.phase1_accumulator.wrapping_sub(phase1_increment);
                let prev_phase1 =
                    (prev_phase1_accumulator & PHASE_MASK) as f32 / PHASE_SCALE as f32;

                if osc2_sync && prev_phase1 > phase1 {
                    // Wrapped around (cycle completed)
                    voice.phase2_accumulator = 0;
                    phase2 = 0.0;
                }

                // Normalised phase increments for PolyBLEP band-limiting.
                let dt1 = freq1 / sample_rate;
                let dt2 = freq2 / sample_rate;

                // Generate oscillator outputs using calculated phases
                let osc1_out = Self::generate_oscillator_static(
                    osc1_wave_type,
                    phase1,
                    dt1,
                    osc1_pulse_width,
                ) * osc1_amplitude;
                let osc2_out = Self::generate_oscillator_static(
                    osc2_wave_type,
                    phase2,
                    dt2,
                    osc2_pulse_width,
                ) * osc2_amplitude;

                // Mix oscillators with individual levels and add noise
                let noise = if mixer_noise_level > 0.0 {
                    (rand::random::<f32>() - 0.5) * 2.0 * mixer_noise_level
                } else {
                    0.0
                };
                let mut mixed = osc1_out * mixer_osc1_level + osc2_out * mixer_osc2_level + noise;

                // Process filter envelope
                let filter_envelope_value = Self::process_filter_envelope_static(
                    voice,
                    filter_envelope_attack,
                    filter_envelope_decay,
                    filter_envelope_sustain,
                    filter_envelope_release,
                    sample_rate,
                );

                // Apply filter envelope to cutoff and keyboard tracking
                let note_frequency = voice.frequency;
                let kbd_track_amount = filter_keyboard_tracking * ((note_frequency / 261.63) - 1.0); // C4 = 261.63 Hz as reference

                // Apply modulation matrix to filter
                let lfo_cutoff_mod = lfo_value * modulation_matrix.lfo_to_cutoff * 1000.0;
                let velocity_cutoff_mod =
                    voice.velocity * modulation_matrix.velocity_to_cutoff * 1000.0;
                let modulated_cutoff = filter_cutoff
                    + lfo_cutoff_mod
                    + velocity_cutoff_mod
                    + (filter_cutoff * filter_envelope_amount * filter_envelope_value)
                    + (filter_cutoff * kbd_track_amount);
                let final_cutoff = modulated_cutoff.clamp(20.0, 20000.0);

                let lfo_resonance_mod = lfo_value * modulation_matrix.lfo_to_resonance * 2.0;
                // Safe resonance limiting to prevent runaway feedback
                let final_resonance = (filter_resonance + lfo_resonance_mod).clamp(0.0, 3.95); // Slightly below 4.0 for safety

                // Apply ladder filter (24dB/octave vintage analog style)
                mixed = Self::apply_ladder_filter_static(
                    mixed,
                    &mut voice.filter_state,
                    final_cutoff,
                    final_resonance,
                    sample_rate,
                );

                // Apply amp envelope
                let envelope_value = Self::process_envelope_static(
                    voice,
                    envelope_attack,
                    envelope_decay,
                    envelope_sustain,
                    envelope_release,
                    sample_rate,
                );

                // Apply modulation matrix to amplitude
                let lfo_amplitude_mod =
                    1.0 + (lfo_value * modulation_matrix.lfo_to_amplitude * 0.5);
                let velocity_amplitude_mod =
                    0.5 + (voice.velocity * modulation_matrix.velocity_to_amplitude * 0.5);

                mixed *= envelope_value * lfo_amplitude_mod * velocity_amplitude_mod;

                // Add to output
                *sample += mixed;
            }

            // Apply master volume with gentle compression
            *sample *= master_volume;

            // Apply effects processing
            *sample = self.apply_delay(*sample);
            *sample = self.apply_reverb(*sample);

            // Continuous saturation. The previous threshold clipper jumped
            // by ~0.18 at |x|=0.7 and buzzed on every loud peak.
            *sample = sample.tanh();

            *sample = (*sample).clamp(-1.0, 1.0);
        }
    }

    fn generate_oscillator_static(
        wave_type: WaveType,
        phase: f32,
        dt: f32,
        pulse_width: f32,
    ) -> f32 {
        let phase = phase.rem_euclid(1.0);
        // dt is the normalised phase increment (freq/sample_rate). Clamp to avoid
        // PolyBLEP regions overlapping at pathological values.
        let dt = dt.clamp(1.0e-6, 0.49);
        match wave_type {
            WaveType::Sine => OPTIMIZATION_TABLES.fast_sin(phase * 2.0 * PI),
            WaveType::Sawtooth => {
                // Naive ramp minus PolyBLEP correction at the wrap-around.
                let value = 2.0 * phase - 1.0;
                value - Self::poly_blep(phase, dt)
            }
            WaveType::Square => {
                // Variable-width pulse with PolyBLEP on both edges.
                let pw = pulse_width.clamp(0.01, 0.99);
                let mut value = if phase < pw { 1.0 } else { -1.0 };
                value += Self::poly_blep(phase, dt);
                let falling_phase = (phase + 1.0 - pw).rem_euclid(1.0);
                value -= Self::poly_blep(falling_phase, dt);
                value
            }
            WaveType::Triangle => {
                // Triangle harmonics already fall at 1/n^2, so naive aliasing is mild;
                // PolyBLAMP smooths the slope discontinuities at phase 0 and 0.5.
                let mut value = if phase < 0.5 {
                    4.0 * phase - 1.0
                } else {
                    3.0 - 4.0 * phase
                };
                value += 8.0 * dt * Self::poly_blamp(phase, dt);
                let half_phase = (phase + 0.5).rem_euclid(1.0);
                value -= 8.0 * dt * Self::poly_blamp(half_phase, dt);
                value
            }
        }
    }

    #[inline]
    fn poly_blep(phase: f32, dt: f32) -> f32 {
        if phase < dt {
            let t = phase / dt;
            2.0 * t - t * t - 1.0
        } else if phase > 1.0 - dt {
            let t = (phase - 1.0) / dt;
            t * t + 2.0 * t + 1.0
        } else {
            0.0
        }
    }

    #[inline]
    fn poly_blamp(phase: f32, dt: f32) -> f32 {
        if phase < dt {
            let t = phase / dt - 1.0;
            -(1.0 / 3.0) * t * t * t
        } else if phase > 1.0 - dt {
            let t = (phase - 1.0) / dt + 1.0;
            (1.0 / 3.0) * t * t * t
        } else {
            0.0
        }
    }

    fn apply_ladder_filter_static(
        input: f32,
        state: &mut LadderFilterState,
        cutoff: f32,
        resonance: f32,
        sample_rate: f32,
    ) -> f32 {
        // Moog ladder filter implementation based on Huovilainen's improved model
        // This provides authentic vintage analog filter sound with self-oscillation capability

        // Convert cutoff frequency to filter coefficient
        let fc = (cutoff / sample_rate).min(0.49); // Limit to Nyquist
        let f = fc * 2.0;

        // Calculate resonance feedback with improved stability
        let res = resonance.clamp(0.0, 3.95); // Safe upper limit
        let feedback = res * (1.0 - 0.15 * f * f);

        // Additional stability check
        let feedback = if feedback > 0.98 { 0.98 } else { feedback };

        // Input with feedback (creates resonance and self-oscillation)
        let input_with_feedback = input - state.feedback * feedback;

        // Soft clipping for saturation (tanh approximation for efficiency)
        let saturated = if input_with_feedback.abs() > 1.0 {
            input_with_feedback.signum() * (1.0 - (-input_with_feedback.abs() * 1.5).exp())
        } else {
            input_with_feedback
        };

        // Process through 4 cascaded 1-pole filters (24dB/octave)
        // Each stage is a simple 1-pole lowpass: y = y + f * (x - y)

        // Stage 1
        state.stage1 += f * (saturated - state.stage1 + state.delay1);
        state.delay1 = saturated - state.stage1;
        let stage1_out = state.stage1;

        // Stage 2
        state.stage2 += f * (stage1_out - state.stage2 + state.delay2);
        state.delay2 = stage1_out - state.stage2;
        let stage2_out = state.stage2;

        // Stage 3
        state.stage3 += f * (stage2_out - state.stage3 + state.delay3);
        state.delay3 = stage2_out - state.stage3;
        let stage3_out = state.stage3;

        // Stage 4
        state.stage4 += f * (stage3_out - state.stage4 + state.delay4);
        state.delay4 = stage3_out - state.stage4;
        let stage4_out = state.stage4;

        // Store feedback for next sample
        state.feedback = stage4_out;

        // DC blocking to prevent offset accumulation
        let dc_block_coeff = 0.995; // High-pass at ~1.6Hz at 44.1kHz
        state.dc_block_x1 = state.dc_block_x2;
        state.dc_block_x2 = stage4_out;
        state.dc_block_y1 = state.dc_block_y2;
        state.dc_block_y2 =
            state.dc_block_x2 - state.dc_block_x1 + dc_block_coeff * state.dc_block_y1;

        let dc_blocked_output = state.dc_block_y2;

        // Flush denormals in every feedback path. Missing any of these causes
        // ~100x slowdown on decayed tails and drops audio-thread deadlines.
        const DENORMAL_FLOOR: f32 = 1.0e-20;
        if state.stage1.abs() < DENORMAL_FLOOR {
            state.stage1 = 0.0;
        }
        if state.stage2.abs() < DENORMAL_FLOOR {
            state.stage2 = 0.0;
        }
        if state.stage3.abs() < DENORMAL_FLOOR {
            state.stage3 = 0.0;
        }
        if state.stage4.abs() < DENORMAL_FLOOR {
            state.stage4 = 0.0;
        }
        if state.delay1.abs() < DENORMAL_FLOOR {
            state.delay1 = 0.0;
        }
        if state.delay2.abs() < DENORMAL_FLOOR {
            state.delay2 = 0.0;
        }
        if state.delay3.abs() < DENORMAL_FLOOR {
            state.delay3 = 0.0;
        }
        if state.delay4.abs() < DENORMAL_FLOOR {
            state.delay4 = 0.0;
        }
        if state.feedback.abs() < DENORMAL_FLOOR {
            state.feedback = 0.0;
        }
        if state.dc_block_x1.abs() < DENORMAL_FLOOR {
            state.dc_block_x1 = 0.0;
        }
        if state.dc_block_x2.abs() < DENORMAL_FLOOR {
            state.dc_block_x2 = 0.0;
        }
        if state.dc_block_y1.abs() < DENORMAL_FLOOR {
            state.dc_block_y1 = 0.0;
        }
        if state.dc_block_y2.abs() < DENORMAL_FLOOR {
            state.dc_block_y2 = 0.0;
        }

        // Output with slight compensation for high resonance volume boost
        let compensation = 1.0 + res * 0.1;
        dc_blocked_output / compensation
    }

    fn process_envelope_static(
        voice: &mut Voice,
        attack: f32,
        decay: f32,
        sustain: f32,
        release: f32,
        sample_rate: f32,
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
            }
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
            }
            EnvelopeState::Sustain => {
                voice.envelope_value = sustain;
                voice.sustain_time += dt;

                // Add small amount of noise reduction during sustain to prevent buildup
                if voice.sustain_time > 1.0 {
                    // After 1 second of sustain
                    // Slightly reduce very small filter state values that can cause drift
                    if voice.filter_state.stage1.abs() < 1e-8 {
                        voice.filter_state.stage1 = 0.0;
                    }
                    if voice.filter_state.stage2.abs() < 1e-8 {
                        voice.filter_state.stage2 = 0.0;
                    }
                    if voice.filter_state.stage3.abs() < 1e-8 {
                        voice.filter_state.stage3 = 0.0;
                    }
                    if voice.filter_state.stage4.abs() < 1e-8 {
                        voice.filter_state.stage4 = 0.0;
                    }

                    // Reset sustain timer to prevent constant checking
                    voice.sustain_time = 0.0;
                }
            }
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
            }
            EnvelopeState::Idle => {
                voice.envelope_value = 0.0;
                voice.is_active = false;
            }
        }

        voice.envelope_value
    }

    fn process_filter_envelope_static(
        voice: &mut Voice,
        attack: f32,
        decay: f32,
        sustain: f32,
        release: f32,
        sample_rate: f32,
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
            }
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
            }
            EnvelopeState::Sustain => {
                voice.filter_envelope_value = sustain;
            }
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
            }
            EnvelopeState::Idle => {
                voice.filter_envelope_value = 0.0;
            }
        }

        voice.filter_envelope_value
    }

    fn update_arpeggiator(&mut self, dt: f32) {
        if !self.arpeggiator.enabled || self.held_notes.is_empty() {
            return;
        }

        let beat_time = 60.0 / self.arpeggiator.rate; // Time per beat in seconds
        self.arp_timer += dt;
        self.arp_note_timer += dt;

        // Check if it's time for the next note
        if self.arp_timer >= beat_time {
            self.arp_timer -= beat_time;

            // Release current arpeggiator note
            for voice in &mut self.voices {
                if voice.is_active {
                    voice.release();
                }
            }

            // Get the next note based on pattern
            if let Some(note) = self.get_next_arp_note() {
                self.trigger_note(note, 100); // Fixed velocity for arp
            }
        }

        // Handle gate length (note off)
        let gate_time = beat_time * self.arpeggiator.gate_length;
        if self.arp_note_timer >= gate_time {
            // Let note ring until next step
        }
    }

    fn get_next_arp_note(&mut self) -> Option<u8> {
        if self.held_notes.is_empty() {
            return None;
        }

        let mut extended_notes = Vec::new();

        // Generate notes across octaves
        for octave in 0..self.arpeggiator.octaves {
            for &note in &self.held_notes {
                let octave_note = note + (octave * 12);
                if octave_note <= 127 {
                    extended_notes.push(octave_note);
                }
            }
        }

        if extended_notes.is_empty() {
            return None;
        }

        let note = match self.arpeggiator.pattern {
            ArpPattern::Up => {
                let note = extended_notes[self.arp_step % extended_notes.len()];
                self.arp_step += 1;
                note
            }
            ArpPattern::Down => {
                extended_notes.reverse();
                let note = extended_notes[self.arp_step % extended_notes.len()];
                self.arp_step += 1;
                note
            }
            ArpPattern::UpDown => {
                let len = extended_notes.len();
                if len == 1 {
                    extended_notes[0]
                } else {
                    let cycle_len = (len - 1) * 2;
                    let pos = self.arp_step % cycle_len;
                    self.arp_step += 1;

                    if pos < len {
                        extended_notes[pos]
                    } else {
                        extended_notes[len - 1 - (pos - len)]
                    }
                }
            }
            ArpPattern::Random => {
                let index = (rand::random::<f32>() * extended_notes.len() as f32) as usize;
                extended_notes[index.min(extended_notes.len() - 1)]
            }
        };

        self.arp_note_timer = 0.0;
        Some(note)
    }

    fn apply_delay(&mut self, sample: f32) -> f32 {
        if self.effects.delay_amount <= 0.0 {
            return sample;
        }

        let delay_samples = (self.effects.delay_time * self.sample_rate) as usize;
        let delay_samples = delay_samples.min(self.delay_buffer.len() - 1);

        let delay_read_index = if self.delay_index >= delay_samples {
            self.delay_index - delay_samples
        } else {
            self.delay_buffer.len() + self.delay_index - delay_samples
        };

        let delayed_sample = self.delay_buffer[delay_read_index];
        // Flush denormals in the feedback tail, otherwise decaying echoes
        // slow the audio thread and cause xruns.
        let delayed_sample = if delayed_sample.abs() < 1.0e-20 {
            0.0
        } else {
            delayed_sample
        };
        let feedback_sample = delayed_sample * self.effects.delay_feedback;

        self.delay_buffer[self.delay_index] = sample + feedback_sample;
        self.delay_index = (self.delay_index + 1) % self.delay_buffer.len();

        sample + (delayed_sample * self.effects.delay_amount)
    }

    fn apply_reverb(&mut self, sample: f32) -> f32 {
        if self.effects.reverb_amount <= 0.0 {
            return sample;
        }

        let mut reverb_output = 0.0;
        let decay = 0.7 * self.effects.reverb_size;

        for (i, buffer) in self.reverb_buffers.iter_mut().enumerate() {
            let delay_sample = buffer[self.reverb_indices[i]];
            // Flush denormals in the comb feedback.
            let delay_sample = if delay_sample.abs() < 1.0e-20 {
                0.0
            } else {
                delay_sample
            };
            buffer[self.reverb_indices[i]] = sample + (delay_sample * decay);
            reverb_output += delay_sample;

            self.reverb_indices[i] = (self.reverb_indices[i] + 1) % buffer.len();
        }

        let reverb_mix =
            (reverb_output / self.reverb_buffers.len() as f32) * self.effects.reverb_amount;
        sample + reverb_mix
    }

    pub fn save_preset(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Ensure presets directory exists
        self.ensure_presets_dir()?;

        let preset = Preset {
            name: name.to_string(),
            osc1: self.osc1.clone(),
            osc2: self.osc2.clone(),
            osc2_sync: self.osc2_sync,
            mixer: self.mixer.clone(),
            filter: self.filter.clone(),
            filter_envelope: self.filter_envelope.clone(),
            amp_envelope: self.amp_envelope.clone(),
            lfo: self.lfo.clone(),
            modulation_matrix: self.modulation_matrix.clone(),
            effects: self.effects.clone(),
            master_volume: self.master_volume,
        };

        let preset_json = self.preset_to_json(&preset)?;
        let filename = format!("presets/{}.json", name.replace(" ", "_"));
        fs::write(&filename, preset_json)?;
        println!("Preset '{}' saved to {}", name, filename);
        Ok(())
    }

    pub fn load_preset(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let filename = format!("presets/{}.json", name.replace(" ", "_"));
        if !Path::new(&filename).exists() {
            return Err(format!("Preset file '{}' not found", filename).into());
        }

        let preset_json = fs::read_to_string(&filename)?;
        let preset = self.json_to_preset(&preset_json)?;

        self.osc1 = preset.osc1;
        self.osc2 = preset.osc2;
        self.osc2_sync = preset.osc2_sync;
        self.mixer = preset.mixer;
        self.filter = preset.filter;
        self.filter_envelope = preset.filter_envelope;
        self.amp_envelope = preset.amp_envelope;
        self.lfo = preset.lfo;
        self.modulation_matrix = preset.modulation_matrix;
        self.effects = preset.effects;
        self.master_volume = preset.master_volume;

        println!("Preset '{}' loaded from {}", name, filename);
        Ok(())
    }

    pub fn list_presets() -> Vec<String> {
        let mut presets = Vec::new();
        if let Ok(entries) = fs::read_dir("presets") {
            for entry in entries.flatten() {
                if let Some(filename) = entry.file_name().to_str()
                    && filename.ends_with(".json")
                {
                    let name = filename.trim_end_matches(".json").replace("_", " ");
                    presets.push(name);
                }
            }
        }
        presets.sort();
        presets
    }

    fn preset_to_json(&self, preset: &Preset) -> Result<String, Box<dyn std::error::Error>> {
        // Simple line-based format for easy parsing
        let lines = vec![
            format!("\"{}\"", preset.name),
            format!("\"{}\"", self.wave_type_to_string(preset.osc1.wave_type)),
            preset.osc1.amplitude.to_string(),
            preset.osc1.detune.to_string(),
            preset.osc1.pulse_width.to_string(),
            format!("\"{}\"", self.wave_type_to_string(preset.osc2.wave_type)),
            preset.osc2.amplitude.to_string(),
            preset.osc2.detune.to_string(),
            preset.osc2.pulse_width.to_string(),
            preset.osc2_sync.to_string(),
            preset.mixer.osc1_level.to_string(),
            preset.mixer.osc2_level.to_string(),
            preset.mixer.noise_level.to_string(),
            preset.filter.cutoff.to_string(),
            preset.filter.resonance.to_string(),
            preset.filter.envelope_amount.to_string(),
            preset.filter.keyboard_tracking.to_string(),
            preset.filter_envelope.attack.to_string(),
            preset.filter_envelope.decay.to_string(),
            preset.filter_envelope.sustain.to_string(),
            preset.filter_envelope.release.to_string(),
            preset.amp_envelope.attack.to_string(),
            preset.amp_envelope.decay.to_string(),
            preset.amp_envelope.sustain.to_string(),
            preset.amp_envelope.release.to_string(),
            preset.lfo.frequency.to_string(),
            preset.lfo.amplitude.to_string(),
            preset.lfo.target_osc1_pitch.to_string(),
            preset.lfo.target_osc2_pitch.to_string(),
            preset.lfo.target_filter.to_string(),
            preset.lfo.target_amplitude.to_string(),
            preset.modulation_matrix.lfo_to_cutoff.to_string(),
            preset.modulation_matrix.lfo_to_resonance.to_string(),
            preset.modulation_matrix.lfo_to_osc1_pitch.to_string(),
            preset.modulation_matrix.lfo_to_osc2_pitch.to_string(),
            preset.modulation_matrix.lfo_to_amplitude.to_string(),
            preset.modulation_matrix.velocity_to_cutoff.to_string(),
            preset.modulation_matrix.velocity_to_amplitude.to_string(),
            preset.effects.reverb_amount.to_string(),
            preset.effects.reverb_size.to_string(),
            preset.effects.delay_time.to_string(),
            preset.effects.delay_feedback.to_string(),
            preset.effects.delay_amount.to_string(),
            preset.master_volume.to_string(),
        ];
        Ok(lines.join("\n"))
    }

    fn json_to_preset(&self, json: &str) -> Result<Preset, Box<dyn std::error::Error>> {
        // Parse the JSON format we use in preset_to_json
        let lines: Vec<&str> = json.lines().collect();
        if lines.len() < 44 {
            return Err("Invalid preset format".into());
        }

        // Parse each field from the JSON lines
        let name = lines[0].trim_matches('"').to_string();

        // Oscillator 1
        let osc1_wave = self.string_to_wave_type(lines[1].trim_matches('"'))?;
        let osc1_amp: f32 = lines[2].parse()?;
        let osc1_detune: f32 = lines[3].parse()?;
        let osc1_pw: f32 = lines[4].parse()?;

        // Oscillator 2
        let osc2_wave = self.string_to_wave_type(lines[5].trim_matches('"'))?;
        let osc2_amp: f32 = lines[6].parse()?;
        let osc2_detune: f32 = lines[7].parse()?;
        let osc2_pw: f32 = lines[8].parse()?;

        // Sync
        let osc2_sync: bool = lines[9].parse()?;

        // Mixer
        let mixer_osc1: f32 = lines[10].parse()?;
        let mixer_osc2: f32 = lines[11].parse()?;
        let mixer_noise: f32 = lines[12].parse()?;

        // Filter
        let filter_cutoff: f32 = lines[13].parse()?;
        let filter_res: f32 = lines[14].parse()?;
        let filter_env: f32 = lines[15].parse()?;
        let filter_kbd: f32 = lines[16].parse()?;

        // Filter Envelope
        let fenv_a: f32 = lines[17].parse()?;
        let fenv_d: f32 = lines[18].parse()?;
        let fenv_s: f32 = lines[19].parse()?;
        let fenv_r: f32 = lines[20].parse()?;

        // Amp Envelope
        let aenv_a: f32 = lines[21].parse()?;
        let aenv_d: f32 = lines[22].parse()?;
        let aenv_s: f32 = lines[23].parse()?;
        let aenv_r: f32 = lines[24].parse()?;

        // LFO
        let lfo_freq: f32 = lines[25].parse()?;
        let lfo_amp: f32 = lines[26].parse()?;
        let lfo_osc1: bool = lines[27].parse()?;
        let lfo_osc2: bool = lines[28].parse()?;
        let lfo_filter: bool = lines[29].parse()?;
        let lfo_amplitude: bool = lines[30].parse()?;

        // Modulation Matrix
        let mod_lfo_cut: f32 = lines[31].parse()?;
        let mod_lfo_res: f32 = lines[32].parse()?;
        let mod_lfo_osc1: f32 = lines[33].parse()?;
        let mod_lfo_osc2: f32 = lines[34].parse()?;
        let mod_lfo_amp: f32 = lines[35].parse()?;
        let mod_vel_cut: f32 = lines[36].parse()?;
        let mod_vel_amp: f32 = lines[37].parse()?;

        // Effects
        let fx_rev_amt: f32 = lines[38].parse()?;
        let fx_rev_size: f32 = lines[39].parse()?;
        let fx_del_time: f32 = lines[40].parse()?;
        let fx_del_fb: f32 = lines[41].parse()?;
        let fx_del_amt: f32 = lines[42].parse()?;

        // Master
        let master_vol: f32 = lines[43].parse()?;

        Ok(Preset {
            name,
            osc1: OscillatorParams {
                wave_type: osc1_wave,
                amplitude: osc1_amp,
                detune: osc1_detune,
                pulse_width: osc1_pw,
            },
            osc2: OscillatorParams {
                wave_type: osc2_wave,
                amplitude: osc2_amp,
                detune: osc2_detune,
                pulse_width: osc2_pw,
            },
            osc2_sync,
            mixer: MixerParams {
                osc1_level: mixer_osc1,
                osc2_level: mixer_osc2,
                noise_level: mixer_noise,
            },
            filter: FilterParams {
                cutoff: filter_cutoff,
                resonance: filter_res,
                envelope_amount: filter_env,
                keyboard_tracking: filter_kbd,
            },
            filter_envelope: EnvelopeParams {
                attack: fenv_a,
                decay: fenv_d,
                sustain: fenv_s,
                release: fenv_r,
            },
            amp_envelope: EnvelopeParams {
                attack: aenv_a,
                decay: aenv_d,
                sustain: aenv_s,
                release: aenv_r,
            },
            lfo: LfoParams {
                frequency: lfo_freq,
                amplitude: lfo_amp,
                waveform: LfoWaveform::Triangle, // Default for older presets
                sync: false,                     // Default for older presets
                target_osc1_pitch: lfo_osc1,
                target_osc2_pitch: lfo_osc2,
                target_filter: lfo_filter,
                target_amplitude: lfo_amplitude,
            },
            modulation_matrix: ModulationMatrix {
                lfo_to_cutoff: mod_lfo_cut,
                lfo_to_resonance: mod_lfo_res,
                lfo_to_osc1_pitch: mod_lfo_osc1,
                lfo_to_osc2_pitch: mod_lfo_osc2,
                lfo_to_amplitude: mod_lfo_amp,
                velocity_to_cutoff: mod_vel_cut,
                velocity_to_amplitude: mod_vel_amp,
            },
            effects: EffectsParams {
                reverb_amount: fx_rev_amt,
                reverb_size: fx_rev_size,
                delay_time: fx_del_time,
                delay_feedback: fx_del_fb,
                delay_amount: fx_del_amt,
            },
            master_volume: master_vol,
        })
    }

    fn string_to_wave_type(&self, wave_str: &str) -> Result<WaveType, Box<dyn std::error::Error>> {
        match wave_str {
            "Sine" => Ok(WaveType::Sine),
            "Square" => Ok(WaveType::Square),
            "Triangle" => Ok(WaveType::Triangle),
            "Sawtooth" => Ok(WaveType::Sawtooth),
            _ => Err(format!("Unknown wave type: {}", wave_str).into()),
        }
    }

    fn wave_type_to_string(&self, wave_type: WaveType) -> &'static str {
        match wave_type {
            WaveType::Sine => "Sine",
            WaveType::Square => "Square",
            WaveType::Triangle => "Triangle",
            WaveType::Sawtooth => "Sawtooth",
        }
    }

    fn ensure_presets_dir(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !Path::new("presets").exists() {
            fs::create_dir("presets")?;
            println!("Created presets directory");
        }
        Ok(())
    }

    pub fn create_all_classic_presets(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Creating classic synthesizer presets...");

        // Bass Sounds
        self.create_moog_bass()?;
        self.create_acid_bass()?;
        self.create_sub_bass()?;
        self.create_wobble_bass()?;

        // Lead Sounds
        self.create_supersaw_lead()?;
        self.create_pluck_lead()?;
        self.create_screaming_lead()?;
        self.create_vintage_lead()?;

        // Pads & Strings
        self.create_warm_pad()?;
        self.create_string_ensemble()?;
        self.create_choir_pad()?;
        self.create_glass_pad()?;

        // Brass & Wind
        self.create_brass_stab()?;
        self.create_trumpet_lead()?;
        self.create_flute()?;
        self.create_sax_lead()?;

        // Effects & Special
        self.create_arp_sequence()?;
        self.create_sweep_fx()?;
        self.create_noise_sweep()?;
        self.create_zap_sound()?;

        // Vintage Analog Classics
        self.create_jump_brass()?;
        self.create_cars_lead()?;
        self.create_prophet_sync_lead()?;
        self.create_new_order_bass()?;
        self.create_berlin_school()?;
        self.create_prophet_strings()?;

        println!("All classic presets created successfully!");
        Ok(())
    }

    // Bass Sounds
    fn create_moog_bass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Square;
        self.osc2.amplitude = 0.5;
        self.osc2.detune = -12.0;
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.6;
        self.filter.cutoff = 800.0;
        self.filter.resonance = 2.5;
        self.filter.envelope_amount = 0.4;
        self.filter_envelope.attack = 0.01;
        self.filter_envelope.decay = 0.3;
        self.filter_envelope.sustain = 0.3;
        self.filter_envelope.release = 0.2;
        self.amp_envelope.attack = 0.01;
        self.amp_envelope.decay = 0.1;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 0.3;
        self.save_preset("Moog Bass")
    }

    fn create_acid_bass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 400.0;
        self.filter.resonance = 3.8;
        self.filter.envelope_amount = 0.8;
        self.filter_envelope.attack = 0.001;
        self.filter_envelope.decay = 0.15;
        self.filter_envelope.sustain = 0.1;
        self.filter_envelope.release = 0.1;
        self.amp_envelope.attack = 0.001;
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 0.7;
        self.amp_envelope.release = 0.1;
        self.lfo.frequency = 0.5;
        self.lfo.amplitude = 0.3;
        self.lfo.target_filter = true;
        self.modulation_matrix.lfo_to_cutoff = 0.6;
        self.save_preset("Acid Bass")
    }

    fn create_sub_bass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Square;
        self.osc1.amplitude = 1.0;
        self.osc1.detune = -24.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 150.0;
        self.filter.resonance = 0.5;
        self.amp_envelope.attack = 0.01;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 1.0;
        self.amp_envelope.release = 0.5;
        self.save_preset("Sub Bass")
    }

    fn create_wobble_bass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 600.0;
        self.filter.resonance = 3.0;
        self.filter.envelope_amount = 0.5;
        self.filter_envelope.attack = 0.01;
        self.filter_envelope.decay = 0.1;
        self.filter_envelope.sustain = 0.8;
        self.filter_envelope.release = 0.2;
        self.amp_envelope.attack = 0.01;
        self.amp_envelope.decay = 0.1;
        self.amp_envelope.sustain = 1.0;
        self.amp_envelope.release = 0.2;
        self.lfo.frequency = 8.0;
        self.lfo.amplitude = 1.0;
        self.lfo.target_filter = true;
        self.modulation_matrix.lfo_to_cutoff = 0.9;
        self.save_preset("Wobble Bass")
    }

    // Lead Sounds
    fn create_supersaw_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.8;
        self.osc2.detune = 7.0;
        self.mixer.osc1_level = 0.7;
        self.mixer.osc2_level = 0.7;
        self.filter.cutoff = 8000.0;
        self.filter.resonance = 1.2;
        self.filter.envelope_amount = 0.3;
        self.filter_envelope.attack = 0.1;
        self.filter_envelope.decay = 0.3;
        self.filter_envelope.sustain = 0.7;
        self.filter_envelope.release = 0.5;
        self.amp_envelope.attack = 0.1;
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 0.8;
        self.effects.reverb_amount = 0.3;
        self.effects.delay_amount = 0.2;
        self.save_preset("Supersaw Lead")
    }

    fn create_pluck_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Triangle;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 4000.0;
        self.filter.resonance = 1.5;
        self.filter.envelope_amount = 0.6;
        self.filter_envelope.attack = 0.001;
        self.filter_envelope.decay = 0.5;
        self.filter_envelope.sustain = 0.2;
        self.filter_envelope.release = 0.3;
        self.amp_envelope.attack = 0.001;
        self.amp_envelope.decay = 0.6;
        self.amp_envelope.sustain = 0.1;
        self.amp_envelope.release = 0.3;
        self.save_preset("Pluck Lead")
    }

    fn create_screaming_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 12000.0;
        self.filter.resonance = 3.5;
        self.filter.envelope_amount = 0.4;
        self.filter_envelope.attack = 0.2;
        self.filter_envelope.decay = 0.4;
        self.filter_envelope.sustain = 0.8;
        self.filter_envelope.release = 1.0;
        self.amp_envelope.attack = 0.1;
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 1.0;
        self.amp_envelope.release = 1.2;
        self.lfo.frequency = 5.0;
        self.lfo.amplitude = 0.3;
        self.lfo.target_osc1_pitch = true;
        self.modulation_matrix.lfo_to_osc1_pitch = 0.4;
        self.save_preset("Screaming Lead")
    }

    fn create_vintage_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Square;
        self.osc2.amplitude = 0.6;
        self.osc2.detune = -5.0;
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.5;
        self.filter.cutoff = 6000.0;
        self.filter.resonance = 2.0;
        self.filter.envelope_amount = 0.5;
        self.filter_envelope.attack = 0.3;
        self.filter_envelope.decay = 0.6;
        self.filter_envelope.sustain = 0.6;
        self.filter_envelope.release = 1.0;
        self.amp_envelope.attack = 0.2;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 1.5;
        self.effects.delay_amount = 0.25;
        self.effects.delay_time = 0.3;
        self.save_preset("Vintage Lead")
    }

    // Pads & Strings
    fn create_warm_pad(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Triangle;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sine;
        self.osc2.amplitude = 0.7;
        self.osc2.detune = 12.0;
        self.mixer.osc1_level = 0.6;
        self.mixer.osc2_level = 0.4;
        self.filter.cutoff = 3000.0;
        self.filter.resonance = 0.8;
        self.filter.envelope_amount = 0.2;
        self.filter_envelope.attack = 1.5;
        self.filter_envelope.decay = 1.0;
        self.filter_envelope.sustain = 0.8;
        self.filter_envelope.release = 2.0;
        self.amp_envelope.attack = 1.8;
        self.amp_envelope.decay = 0.5;
        self.amp_envelope.sustain = 0.9;
        self.amp_envelope.release = 2.5;
        self.effects.reverb_amount = 0.6;
        self.effects.reverb_size = 0.8;
        self.save_preset("Warm Pad")
    }

    fn create_string_ensemble(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.8;
        self.osc2.detune = 3.0;
        self.mixer.osc1_level = 0.7;
        self.mixer.osc2_level = 0.6;
        self.filter.cutoff = 5000.0;
        self.filter.resonance = 1.0;
        self.filter.envelope_amount = 0.3;
        self.filter_envelope.attack = 1.2;
        self.filter_envelope.decay = 0.8;
        self.filter_envelope.sustain = 0.7;
        self.filter_envelope.release = 1.8;
        self.amp_envelope.attack = 1.5;
        self.amp_envelope.decay = 0.6;
        self.amp_envelope.sustain = 0.85;
        self.amp_envelope.release = 2.0;
        self.lfo.frequency = 0.3;
        self.lfo.amplitude = 0.2;
        self.lfo.target_amplitude = true;
        self.modulation_matrix.lfo_to_amplitude = 0.15;
        self.effects.reverb_amount = 0.5;
        self.save_preset("String Ensemble")
    }

    fn create_choir_pad(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sine;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Triangle;
        self.osc2.amplitude = 0.6;
        self.osc2.detune = 5.0;
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.5;
        self.filter.cutoff = 4000.0;
        self.filter.resonance = 0.6;
        self.filter.envelope_amount = 0.25;
        self.filter_envelope.attack = 2.0;
        self.filter_envelope.decay = 1.2;
        self.filter_envelope.sustain = 0.8;
        self.filter_envelope.release = 2.5;
        self.amp_envelope.attack = 2.2;
        self.amp_envelope.decay = 0.8;
        self.amp_envelope.sustain = 0.9;
        self.amp_envelope.release = 3.0;
        self.lfo.frequency = 0.2;
        self.lfo.amplitude = 0.1;
        self.lfo.target_filter = true;
        self.modulation_matrix.lfo_to_cutoff = 0.1;
        self.effects.reverb_amount = 0.8;
        self.effects.reverb_size = 0.9;
        self.save_preset("Choir Pad")
    }

    fn create_glass_pad(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sine;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sine;
        self.osc2.amplitude = 0.4;
        self.osc2.detune = 24.0;
        self.mixer.osc1_level = 0.7;
        self.mixer.osc2_level = 0.3;
        self.filter.cutoff = 8000.0;
        self.filter.resonance = 1.8;
        self.filter.envelope_amount = 0.4;
        self.filter_envelope.attack = 1.8;
        self.filter_envelope.decay = 1.5;
        self.filter_envelope.sustain = 0.6;
        self.filter_envelope.release = 3.0;
        self.amp_envelope.attack = 2.0;
        self.amp_envelope.decay = 1.0;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 3.5;
        self.lfo.frequency = 0.15;
        self.lfo.amplitude = 0.3;
        self.lfo.target_osc2_pitch = true;
        self.modulation_matrix.lfo_to_osc1_pitch = 0.2;
        self.effects.reverb_amount = 0.9;
        self.effects.reverb_size = 1.0;
        self.effects.delay_amount = 0.3;
        self.save_preset("Glass Pad")
    }

    // Brass & Wind
    fn create_brass_stab(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Square;
        self.osc2.amplitude = 0.7;
        self.osc2.detune = -12.0;
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.6;
        self.filter.cutoff = 3000.0;
        self.filter.resonance = 2.2;
        self.filter.envelope_amount = 0.7;
        self.filter_envelope.attack = 0.05;
        self.filter_envelope.decay = 0.2;
        self.filter_envelope.sustain = 0.4;
        self.filter_envelope.release = 0.3;
        self.amp_envelope.attack = 0.02;
        self.amp_envelope.decay = 0.1;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 0.2;
        self.save_preset("Brass Stab")
    }

    fn create_trumpet_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 4500.0;
        self.filter.resonance = 1.8;
        self.filter.envelope_amount = 0.5;
        self.filter_envelope.attack = 0.1;
        self.filter_envelope.decay = 0.4;
        self.filter_envelope.sustain = 0.7;
        self.filter_envelope.release = 0.6;
        self.amp_envelope.attack = 0.08;
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 0.85;
        self.amp_envelope.release = 0.8;
        self.lfo.frequency = 4.5;
        self.lfo.amplitude = 0.2;
        self.lfo.target_osc1_pitch = true;
        self.modulation_matrix.lfo_to_osc1_pitch = 0.15;
        self.save_preset("Trumpet Lead")
    }

    fn create_flute(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sine;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Triangle;
        self.osc2.amplitude = 0.3;
        self.osc2.detune = 12.0;
        self.mixer.osc1_level = 0.9;
        self.mixer.osc2_level = 0.2;
        self.mixer.noise_level = 0.05;
        self.filter.cutoff = 6000.0;
        self.filter.resonance = 0.8;
        self.filter.envelope_amount = 0.3;
        self.filter_envelope.attack = 0.15;
        self.filter_envelope.decay = 0.6;
        self.filter_envelope.sustain = 0.6;
        self.filter_envelope.release = 1.0;
        self.amp_envelope.attack = 0.2;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 1.2;
        self.lfo.frequency = 0.8;
        self.lfo.amplitude = 0.1;
        self.lfo.target_amplitude = true;
        self.modulation_matrix.lfo_to_amplitude = 0.08;
        self.effects.reverb_amount = 0.4;
        self.save_preset("Flute")
    }

    fn create_sax_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Triangle;
        self.osc2.amplitude = 0.6;
        self.osc2.detune = -7.0;
        self.mixer.osc1_level = 0.7;
        self.mixer.osc2_level = 0.5;
        self.mixer.noise_level = 0.08;
        self.filter.cutoff = 3500.0;
        self.filter.resonance = 2.5;
        self.filter.envelope_amount = 0.6;
        self.filter_envelope.attack = 0.12;
        self.filter_envelope.decay = 0.5;
        self.filter_envelope.sustain = 0.6;
        self.filter_envelope.release = 0.8;
        self.amp_envelope.attack = 0.1;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 0.9;
        self.amp_envelope.release = 1.0;
        self.lfo.frequency = 3.0;
        self.lfo.amplitude = 0.25;
        self.lfo.target_osc1_pitch = true;
        self.modulation_matrix.lfo_to_osc1_pitch = 0.2;
        self.effects.reverb_amount = 0.3;
        self.save_preset("Sax Lead")
    }

    // Effects & Special
    fn create_arp_sequence(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Square;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 2500.0;
        self.filter.resonance = 1.5;
        self.filter.envelope_amount = 0.5;
        self.filter_envelope.attack = 0.001;
        self.filter_envelope.decay = 0.2;
        self.filter_envelope.sustain = 0.3;
        self.filter_envelope.release = 0.1;
        self.amp_envelope.attack = 0.001;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 0.2;
        self.amp_envelope.release = 0.1;
        self.arpeggiator.enabled = true;
        self.arpeggiator.rate = 120.0;
        self.arpeggiator.pattern = ArpPattern::Up;
        self.arpeggiator.octaves = 2;
        self.arpeggiator.gate_length = 0.7;
        self.effects.delay_amount = 0.3;
        self.effects.delay_time = 0.25;
        self.save_preset("Arp Sequence")
    }

    fn create_sweep_fx(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 200.0;
        self.filter.resonance = 3.5;
        self.filter.envelope_amount = 1.0;
        self.filter_envelope.attack = 3.0;
        self.filter_envelope.decay = 2.0;
        self.filter_envelope.sustain = 0.5;
        self.filter_envelope.release = 4.0;
        self.amp_envelope.attack = 0.5;
        self.amp_envelope.decay = 1.0;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 3.0;
        self.effects.reverb_amount = 0.7;
        self.effects.delay_amount = 0.4;
        self.save_preset("Sweep FX")
    }

    fn create_noise_sweep(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.mixer.noise_level = 1.0;
        self.filter.cutoff = 100.0;
        self.filter.resonance = 2.0;
        self.filter.envelope_amount = 1.0;
        self.filter_envelope.attack = 2.0;
        self.filter_envelope.decay = 3.0;
        self.filter_envelope.sustain = 0.2;
        self.filter_envelope.release = 2.0;
        self.amp_envelope.attack = 0.1;
        self.amp_envelope.decay = 2.0;
        self.amp_envelope.sustain = 0.5;
        self.amp_envelope.release = 2.5;
        self.effects.reverb_amount = 0.8;
        self.save_preset("Noise Sweep")
    }

    fn create_zap_sound(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.osc1.wave_type = WaveType::Square;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.mixer.noise_level = 0.3;
        self.filter.cutoff = 8000.0;
        self.filter.resonance = 3.0;
        self.filter.envelope_amount = 0.8;
        self.filter_envelope.attack = 0.001;
        self.filter_envelope.decay = 0.05;
        self.filter_envelope.sustain = 0.1;
        self.filter_envelope.release = 0.1;
        self.amp_envelope.attack = 0.001;
        self.amp_envelope.decay = 0.08;
        self.amp_envelope.sustain = 0.2;
        self.amp_envelope.release = 0.1;
        self.lfo.frequency = 15.0;
        self.lfo.amplitude = 1.0;
        self.lfo.target_osc1_pitch = true;
        self.modulation_matrix.lfo_to_osc1_pitch = 0.8;
        self.effects.delay_amount = 0.4;
        self.save_preset("Zap Sound")
    }

    // Vintage Analog Presets
    fn create_jump_brass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        // Van Halen "Jump" brass stab - authentic vintage analog sound
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Square;
        self.osc2.amplitude = 0.8;
        self.osc2.detune = 7.0; // Classic vintage detune
        self.mixer.osc1_level = 0.9;
        self.mixer.osc2_level = 0.7;
        self.filter.cutoff = 2800.0;
        self.filter.resonance = 3.2; // High resonance for characteristic bite
        self.filter.envelope_amount = 0.8; // Strong filter modulation
        self.filter_envelope.attack = 0.01; // Sharp attack
        self.filter_envelope.decay = 0.15; // Short decay
        self.filter_envelope.sustain = 0.2; // Low sustain
        self.filter_envelope.release = 0.1; // Quick release
        self.amp_envelope.attack = 0.005; // Very sharp attack
        self.amp_envelope.decay = 0.12;
        self.amp_envelope.sustain = 0.3;
        self.amp_envelope.release = 0.15;
        self.modulation_matrix.velocity_to_cutoff = 0.6; // Velocity sensitive filter
        self.save_preset("Jump Brass")
    }

    fn create_cars_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        // Gary Numan "Cars" sync lead - classic vintage analog sync
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.9;
        self.osc2.detune = 12.0; // Octave up for sync effect
        self.osc2_sync = true; // Oscillator sync enabled
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.6;
        self.filter.cutoff = 6500.0; // Bright filter
        self.filter.resonance = 1.8; // Medium resonance
        self.filter.envelope_amount = 0.4;
        self.filter_envelope.attack = 0.08; // Medium attack
        self.filter_envelope.decay = 0.6;
        self.filter_envelope.sustain = 0.8; // Sustain-heavy envelope
        self.filter_envelope.release = 1.2;
        self.amp_envelope.attack = 0.05;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 0.9;
        self.amp_envelope.release = 1.0;
        // Characteristic sync harmonics come from oscillator sync
        self.save_preset("Cars Lead")
    }

    fn create_prophet_sync_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        // Classic vintage sync lead with LFO sweep
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.8;
        self.osc2.detune = 24.0; // Two octaves up
        self.osc2_sync = true; // Both oscillators with sync
        self.mixer.osc1_level = 0.7;
        self.mixer.osc2_level = 0.7;
        self.filter.cutoff = 4000.0;
        self.filter.resonance = 2.8;
        self.filter.envelope_amount = 0.5;
        self.filter_envelope.attack = 0.3;
        self.filter_envelope.decay = 0.8;
        self.filter_envelope.sustain = 0.7;
        self.filter_envelope.release = 1.5;
        self.amp_envelope.attack = 0.2;
        self.amp_envelope.decay = 0.4;
        self.amp_envelope.sustain = 0.9;
        self.amp_envelope.release = 1.8;
        // Filter sweep with LFO - classic vintage technique
        self.lfo.frequency = 0.4; // Slow LFO
        self.lfo.amplitude = 0.6;
        self.lfo.target_filter = true;
        self.modulation_matrix.lfo_to_cutoff = 0.7;
        self.save_preset("Vintage Sync Lead")
    }

    fn create_new_order_bass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        // "Blue Monday" style bass - vintage analog classic
        self.osc1.wave_type = WaveType::Square;
        self.osc1.amplitude = 1.0;
        self.osc1.pulse_width = 0.3; // Narrow pulse for characteristic sound
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 450.0; // Low filter cutoff
        self.filter.resonance = 2.2;
        self.filter.envelope_amount = 0.6;
        self.filter_envelope.attack = 0.005; // Very quick attack
        self.filter_envelope.decay = 0.08;
        self.filter_envelope.sustain = 0.2;
        self.filter_envelope.release = 0.06; // Quick release
        self.amp_envelope.attack = 0.001; // Punchy attack
        self.amp_envelope.decay = 0.05;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 0.1; // Quick release for punchiness
        self.modulation_matrix.velocity_to_cutoff = 0.4;
        self.save_preset("New Order Bass")
    }

    fn create_berlin_school(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        // Tangerine Dream style sequence - vintage analog Berlin School
        self.osc1.wave_type = WaveType::Triangle;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.6;
        self.osc2.detune = -5.0; // Slight detune
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.5;
        self.filter.cutoff = 2200.0; // Moderate filter
        self.filter.resonance = 1.5;
        self.filter.envelope_amount = 0.4;
        self.filter_envelope.attack = 0.08;
        self.filter_envelope.decay = 0.4;
        self.filter_envelope.sustain = 0.6;
        self.filter_envelope.release = 0.3;
        self.amp_envelope.attack = 0.05; // Sequence-friendly envelope
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 0.7;
        self.amp_envelope.release = 0.4;
        // Slow LFO modulation - typical of Berlin School
        self.lfo.frequency = 0.25; // Very slow
        self.lfo.amplitude = 0.4;
        self.lfo.target_filter = true;
        self.modulation_matrix.lfo_to_cutoff = 0.3;
        self.effects.delay_amount = 0.2; // Subtle delay
        self.effects.delay_time = 0.375; // Dotted eighth note
        self.save_preset("Berlin School")
    }

    fn create_prophet_strings(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset to defaults without infinite recursion
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        // Lush vintage analog string ensemble
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.8;
        self.osc2.detune = 2.5; // Very slight detune for richness
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.7;
        self.filter.cutoff = 4500.0; // Warm filter setting
        self.filter.resonance = 1.2;
        self.filter.envelope_amount = 0.3;
        self.filter_envelope.attack = 1.8; // Slow attack for strings
        self.filter_envelope.decay = 1.2;
        self.filter_envelope.sustain = 0.8;
        self.filter_envelope.release = 2.5;
        self.amp_envelope.attack = 2.0; // Very slow attack
        self.amp_envelope.decay = 0.8;
        self.amp_envelope.sustain = 0.95; // High sustain
        self.amp_envelope.release = 3.0; // Long release
        // Subtle chorus-like modulation
        self.lfo.frequency = 0.3;
        self.lfo.amplitude = 0.15;
        self.lfo.target_osc2_pitch = true;
        self.modulation_matrix.lfo_to_osc2_pitch = 0.1;
        // Vintage analog string effects
        self.effects.reverb_amount = 0.7; // Lush reverb
        self.effects.reverb_size = 0.9;
        self.effects.delay_amount = 0.15; // Subtle delay for depth
        self.effects.delay_time = 0.4;
        self.save_preset("Vintage Strings")
    }

    /// Extract current parameters as a flat SynthParameters struct
    pub fn to_synth_params(&self) -> crate::lock_free::SynthParameters {
        crate::lock_free::SynthParameters {
            osc1_waveform: Self::wave_type_to_u8_pub(self.osc1.wave_type),
            osc2_waveform: Self::wave_type_to_u8_pub(self.osc2.wave_type),
            osc1_level: self.osc1.amplitude,
            osc2_level: self.osc2.amplitude,
            osc1_detune: self.osc1.detune,
            osc2_detune: self.osc2.detune,
            osc1_pulse_width: self.osc1.pulse_width,
            osc2_pulse_width: self.osc2.pulse_width,
            osc2_sync: self.osc2_sync,
            mixer_osc1_level: self.mixer.osc1_level,
            mixer_osc2_level: self.mixer.osc2_level,
            noise_level: self.mixer.noise_level,
            filter_cutoff: self.filter.cutoff,
            filter_resonance: self.filter.resonance,
            filter_envelope_amount: self.filter.envelope_amount,
            filter_keyboard_tracking: self.filter.keyboard_tracking,
            amp_attack: self.amp_envelope.attack,
            amp_decay: self.amp_envelope.decay,
            amp_sustain: self.amp_envelope.sustain,
            amp_release: self.amp_envelope.release,
            filter_attack: self.filter_envelope.attack,
            filter_decay: self.filter_envelope.decay,
            filter_sustain: self.filter_envelope.sustain,
            filter_release: self.filter_envelope.release,
            lfo_rate: self.lfo.frequency,
            lfo_amount: self.lfo.amplitude,
            lfo_waveform: Self::lfo_waveform_to_u8_pub(self.lfo.waveform),
            lfo_sync: self.lfo.sync,
            lfo_target_osc1_pitch: self.lfo.target_osc1_pitch,
            lfo_target_osc2_pitch: self.lfo.target_osc2_pitch,
            lfo_target_filter: self.lfo.target_filter,
            lfo_target_amplitude: self.lfo.target_amplitude,
            lfo_to_cutoff: self.modulation_matrix.lfo_to_cutoff,
            lfo_to_resonance: self.modulation_matrix.lfo_to_resonance,
            lfo_to_osc1_pitch: self.modulation_matrix.lfo_to_osc1_pitch,
            lfo_to_osc2_pitch: self.modulation_matrix.lfo_to_osc2_pitch,
            lfo_to_amplitude: self.modulation_matrix.lfo_to_amplitude,
            velocity_to_cutoff: self.modulation_matrix.velocity_to_cutoff,
            velocity_to_amplitude: self.modulation_matrix.velocity_to_amplitude,
            reverb_amount: self.effects.reverb_amount,
            reverb_size: self.effects.reverb_size,
            delay_time: self.effects.delay_time,
            delay_feedback: self.effects.delay_feedback,
            delay_amount: self.effects.delay_amount,
            arp_enabled: self.arpeggiator.enabled,
            arp_rate: self.arpeggiator.rate,
            arp_pattern: Self::arp_pattern_to_u8_pub(self.arpeggiator.pattern),
            arp_octaves: self.arpeggiator.octaves,
            arp_gate_length: self.arpeggiator.gate_length,
            master_volume: self.master_volume,
        }
    }

    /// Apply flat SynthParameters to the synthesizer's nested structures
    /// Does NOT touch voice state, buffers, or LFO phase
    pub fn apply_params(&mut self, params: &crate::lock_free::SynthParameters) {
        self.osc1.wave_type = Self::u8_to_wave_type_pub(params.osc1_waveform);
        self.osc2.wave_type = Self::u8_to_wave_type_pub(params.osc2_waveform);
        self.osc1.amplitude = params.osc1_level;
        self.osc2.amplitude = params.osc2_level;
        self.osc1.detune = params.osc1_detune;
        self.osc2.detune = params.osc2_detune;
        self.osc1.pulse_width = params.osc1_pulse_width;
        self.osc2.pulse_width = params.osc2_pulse_width;
        self.osc2_sync = params.osc2_sync;
        self.mixer.osc1_level = params.mixer_osc1_level;
        self.mixer.osc2_level = params.mixer_osc2_level;
        self.mixer.noise_level = params.noise_level;
        self.filter.cutoff = params.filter_cutoff;
        self.filter.resonance = params.filter_resonance;
        self.filter.envelope_amount = params.filter_envelope_amount;
        self.filter.keyboard_tracking = params.filter_keyboard_tracking;
        self.amp_envelope.attack = params.amp_attack;
        self.amp_envelope.decay = params.amp_decay;
        self.amp_envelope.sustain = params.amp_sustain;
        self.amp_envelope.release = params.amp_release;
        self.filter_envelope.attack = params.filter_attack;
        self.filter_envelope.decay = params.filter_decay;
        self.filter_envelope.sustain = params.filter_sustain;
        self.filter_envelope.release = params.filter_release;
        self.lfo.frequency = params.lfo_rate;
        self.lfo.amplitude = params.lfo_amount;
        self.lfo.waveform = Self::u8_to_lfo_waveform_pub(params.lfo_waveform);
        self.lfo.sync = params.lfo_sync;
        self.lfo.target_osc1_pitch = params.lfo_target_osc1_pitch;
        self.lfo.target_osc2_pitch = params.lfo_target_osc2_pitch;
        self.lfo.target_filter = params.lfo_target_filter;
        self.lfo.target_amplitude = params.lfo_target_amplitude;
        self.modulation_matrix.lfo_to_cutoff = params.lfo_to_cutoff;
        self.modulation_matrix.lfo_to_resonance = params.lfo_to_resonance;
        self.modulation_matrix.lfo_to_osc1_pitch = params.lfo_to_osc1_pitch;
        self.modulation_matrix.lfo_to_osc2_pitch = params.lfo_to_osc2_pitch;
        self.modulation_matrix.lfo_to_amplitude = params.lfo_to_amplitude;
        self.modulation_matrix.velocity_to_cutoff = params.velocity_to_cutoff;
        self.modulation_matrix.velocity_to_amplitude = params.velocity_to_amplitude;
        self.effects.reverb_amount = params.reverb_amount;
        self.effects.reverb_size = params.reverb_size;
        self.effects.delay_time = params.delay_time;
        self.effects.delay_feedback = params.delay_feedback;
        self.effects.delay_amount = params.delay_amount;
        self.arpeggiator.enabled = params.arp_enabled;
        self.arpeggiator.rate = params.arp_rate;
        self.arpeggiator.pattern = Self::u8_to_arp_pattern_pub(params.arp_pattern);
        self.arpeggiator.octaves = params.arp_octaves;
        self.arpeggiator.gate_length = params.arp_gate_length;
        self.master_volume = params.master_volume;
    }

    pub fn wave_type_to_u8_pub(wt: WaveType) -> u8 {
        match wt {
            WaveType::Sine => 0,
            WaveType::Square => 1,
            WaveType::Triangle => 2,
            WaveType::Sawtooth => 3,
        }
    }

    pub fn u8_to_wave_type_pub(v: u8) -> WaveType {
        match v {
            0 => WaveType::Sine,
            1 => WaveType::Square,
            2 => WaveType::Triangle,
            3 => WaveType::Sawtooth,
            _ => WaveType::Sawtooth,
        }
    }

    pub fn lfo_waveform_to_u8_pub(wf: LfoWaveform) -> u8 {
        match wf {
            LfoWaveform::Triangle => 0,
            LfoWaveform::Square => 1,
            LfoWaveform::Sawtooth => 2,
            LfoWaveform::ReverseSawtooth => 3,
            LfoWaveform::SampleAndHold => 4,
        }
    }

    pub fn u8_to_lfo_waveform_pub(v: u8) -> LfoWaveform {
        match v {
            0 => LfoWaveform::Triangle,
            1 => LfoWaveform::Square,
            2 => LfoWaveform::Sawtooth,
            3 => LfoWaveform::ReverseSawtooth,
            4 => LfoWaveform::SampleAndHold,
            _ => LfoWaveform::Triangle,
        }
    }

    pub fn arp_pattern_to_u8_pub(p: ArpPattern) -> u8 {
        match p {
            ArpPattern::Up => 0,
            ArpPattern::Down => 1,
            ArpPattern::UpDown => 2,
            ArpPattern::Random => 3,
        }
    }

    pub fn u8_to_arp_pattern_pub(v: u8) -> ArpPattern {
        match v {
            0 => ArpPattern::Up,
            1 => ArpPattern::Down,
            2 => ArpPattern::UpDown,
            3 => ArpPattern::Random,
            _ => ArpPattern::Up,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_to_synth_params_roundtrip() {
        let synth = Synthesizer::new();
        let params = synth.to_synth_params();
        assert_eq!(params.master_volume, synth.master_volume);
        assert_eq!(params.filter_cutoff, synth.filter.cutoff);
        assert_eq!(params.filter_resonance, synth.filter.resonance);
        assert_eq!(params.osc1_detune, synth.osc1.detune);
        assert_eq!(params.amp_attack, synth.amp_envelope.attack);
        assert_eq!(params.lfo_rate, synth.lfo.frequency);
        assert_eq!(params.reverb_amount, synth.effects.reverb_amount);
        assert!(!params.arp_enabled);
    }

    #[test]
    fn test_apply_params_updates_synthesizer() {
        let mut synth = Synthesizer::new();
        let mut params = synth.to_synth_params();
        params.master_volume = 0.9;
        params.filter_cutoff = 5000.0;
        params.osc1_waveform = 0; // Sine
        params.arp_enabled = true;
        params.arp_rate = 180.0;
        params.lfo_sync = true;
        params.delay_amount = 0.5;
        synth.apply_params(&params);
        assert_eq!(synth.master_volume, 0.9);
        assert_eq!(synth.filter.cutoff, 5000.0);
        assert_eq!(synth.osc1.wave_type, WaveType::Sine);
        assert!(synth.arpeggiator.enabled);
        assert_eq!(synth.arpeggiator.rate, 180.0);
        assert!(synth.lfo.sync);
        assert_eq!(synth.effects.delay_amount, 0.5);
    }

    #[test]
    fn test_apply_params_preserves_voice_state() {
        let mut synth = Synthesizer::new();
        synth.note_on(60, 100);
        assert!(!synth.voices.is_empty());
        let params = synth.to_synth_params();
        synth.apply_params(&params);
        assert!(!synth.voices.is_empty());
        assert!(synth.voices.iter().any(|v| v.note == 60 && v.is_active));
    }

    #[test]
    fn test_note_to_frequency_matches_optimization_table() {
        let freq = Synthesizer::note_to_frequency(69);
        assert!((freq - 440.0).abs() < 0.01);
        let freq = Synthesizer::note_to_frequency(60);
        assert!((freq - 261.63).abs() < 0.1);
    }

    #[test]
    fn test_oscillator_sine_output_range() {
        let dt = 440.0 / 48000.0;
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let output = Synthesizer::generate_oscillator_static(WaveType::Sine, phase, dt, 0.5);
            assert!(
                (-1.01..=1.01).contains(&output),
                "Sine at phase {} = {}",
                phase,
                output
            );
        }
    }

    #[test]
    fn test_oscillator_sawtooth_output_range() {
        let dt = 440.0 / 48000.0;
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let output =
                Synthesizer::generate_oscillator_static(WaveType::Sawtooth, phase, dt, 0.5);
            assert!(
                (-1.5..=1.5).contains(&output),
                "Saw at phase {} = {}",
                phase,
                output
            );
        }
    }

    #[test]
    fn test_audio_callback_flow_produces_sound() {
        // Simulate exactly what the audio callback does
        let mut synth = Synthesizer::new();
        synth.sample_rate = 48000.0;

        // Step 1: Trigger a note (like draining MidiEventQueue)
        synth.note_on(60, 100); // C4, velocity 100

        // Step 2: Apply default params (like reading from TripleBuffer)
        let params = crate::lock_free::SynthParameters::default();
        synth.apply_params(&params);

        // Step 3: Process audio blocks (like process_block in callback)
        let mut buffer = vec![0.0f32; 512];
        let mut max_peak = 0.0f32;

        // Process 20 blocks (about 213ms at 48kHz) to get past the attack phase
        for _ in 0..20 {
            for s in buffer.iter_mut() {
                *s = 0.0;
            }
            synth.process_block(&mut buffer);
            let peak = buffer.iter().fold(0.0f32, |max, &s| max.max(s.abs()));
            max_peak = max_peak.max(peak);
        }

        println!(
            "Max peak after 20 blocks with apply_params: {:.6}",
            max_peak
        );
        assert!(
            max_peak > 0.01,
            "Audio output should be audible, got peak={}",
            max_peak
        );

        // Also test WITHOUT apply_params (simulating old code behavior)
        let mut synth_old = Synthesizer::new();
        synth_old.sample_rate = 48000.0;
        synth_old.note_on(60, 100);
        // NO apply_params call - this is what the old code did

        let mut max_peak_old = 0.0f32;
        for _ in 0..20 {
            for s in buffer.iter_mut() {
                *s = 0.0;
            }
            synth_old.process_block(&mut buffer);
            let peak = buffer.iter().fold(0.0f32, |max, &s| max.max(s.abs()));
            max_peak_old = max_peak_old.max(peak);
        }

        println!(
            "Max peak after 20 blocks WITHOUT apply_params: {:.6}",
            max_peak_old
        );
        assert!(
            max_peak_old > 0.01,
            "Old code path should produce sound, got peak={}",
            max_peak_old
        );

        // Compare: new code should produce similar levels to old code
        let ratio = max_peak / max_peak_old;
        println!("New/Old ratio: {:.4}", ratio);
        assert!(
            ratio > 0.5,
            "New code should produce at least 50% of old code level, ratio={}",
            ratio
        );
    }
}
