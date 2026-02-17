use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Lock-free triple buffer for real-time parameter updates
/// GUI writes to one buffer, audio reads from another, third is for swapping
#[allow(dead_code)]
pub struct TripleBuffer<T: Clone> {
    buffers: [T; 3],
    write_index: AtomicUsize,
    read_index: AtomicUsize,
    swap_requested: AtomicBool,
}

impl<T: Clone> TripleBuffer<T> {
    pub fn new(initial_value: T) -> Self {
        Self {
            buffers: [initial_value.clone(), initial_value.clone(), initial_value],
            write_index: AtomicUsize::new(0),
            read_index: AtomicUsize::new(1),
            swap_requested: AtomicBool::new(false),
        }
    }

    /// Non-blocking write for GUI thread
    pub fn write(&mut self, data: T) {
        let write_idx = self.write_index.load(Ordering::Relaxed);
        self.buffers[write_idx] = data;
        self.swap_requested.store(true, Ordering::Release);
    }

    /// Lock-free read for audio thread
    pub fn read(&self) -> &T {
        // Check if GUI requested a swap
        if self
            .swap_requested
            .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            // Swap read and write buffers
            let old_read = self.read_index.load(Ordering::Relaxed);
            let old_write = self.write_index.load(Ordering::Relaxed);

            self.read_index.store(old_write, Ordering::Relaxed);
            self.write_index.store(old_read, Ordering::Relaxed);
        }

        let read_idx = self.read_index.load(Ordering::Relaxed);
        &self.buffers[read_idx]
    }
}

/// Real-time safe analog synthesizer parameters
///
/// Covers ALL parameters from the Synthesizer engine so this struct can serve
/// as the single data payload for lock-free parameter passing between the GUI
/// and audio threads.
#[derive(Debug, Clone, Copy)]
pub struct SynthParameters {
    // Oscillator parameters
    // Waveforms: 0=Sine, 1=Square, 2=Triangle, 3=Sawtooth
    pub osc1_waveform: u8,
    pub osc2_waveform: u8,
    pub osc1_level: f32,
    pub osc2_level: f32,
    pub osc1_detune: f32,
    pub osc2_detune: f32,
    pub osc1_pulse_width: f32,
    pub osc2_pulse_width: f32,
    pub osc2_sync: bool,

    // Mixer
    pub mixer_osc1_level: f32,
    pub mixer_osc2_level: f32,
    pub noise_level: f32,

    // Filter parameters
    pub filter_cutoff: f32,
    pub filter_resonance: f32,
    pub filter_envelope_amount: f32,
    pub filter_keyboard_tracking: f32,

    // Amp envelope
    pub amp_attack: f32,
    pub amp_decay: f32,
    pub amp_sustain: f32,
    pub amp_release: f32,

    // Filter envelope
    pub filter_attack: f32,
    pub filter_decay: f32,
    pub filter_sustain: f32,
    pub filter_release: f32,

    // LFO parameters
    // Waveforms: 0=Triangle, 1=Square, 2=Sawtooth, 3=ReverseSawtooth, 4=SampleAndHold
    pub lfo_rate: f32,
    pub lfo_amount: f32,
    pub lfo_waveform: u8,
    pub lfo_sync: bool,
    pub lfo_target_osc1_pitch: bool,
    pub lfo_target_osc2_pitch: bool,
    pub lfo_target_filter: bool,
    pub lfo_target_amplitude: bool,

    // Modulation matrix
    pub lfo_to_cutoff: f32,
    pub lfo_to_resonance: f32,
    pub lfo_to_osc1_pitch: f32,
    pub lfo_to_osc2_pitch: f32,
    pub lfo_to_amplitude: f32,
    pub velocity_to_cutoff: f32,
    pub velocity_to_amplitude: f32,

    // Effects
    pub reverb_amount: f32,
    pub reverb_size: f32,
    pub delay_time: f32,
    pub delay_feedback: f32,
    pub delay_amount: f32,

    // Arpeggiator
    // Patterns: 0=Up, 1=Down, 2=UpDown, 3=Random
    pub arp_enabled: bool,
    pub arp_rate: f32,
    pub arp_pattern: u8,
    pub arp_octaves: u8,
    pub arp_gate_length: f32,

    // Global controls
    pub master_volume: f32,
}

impl Default for SynthParameters {
    fn default() -> Self {
        Self {
            // Oscillators – waveform 3 = Sawtooth (Prophet-5 default)
            osc1_waveform: 3,
            osc2_waveform: 3,
            osc1_level: 0.5,
            osc2_level: 0.5,
            osc1_detune: 0.0,
            osc2_detune: 0.0,
            osc1_pulse_width: 0.5,
            osc2_pulse_width: 0.5,
            osc2_sync: false,

            // Mixer
            mixer_osc1_level: 0.5,
            mixer_osc2_level: 0.5,
            noise_level: 0.0,

            // Filter
            filter_cutoff: 1000.0,
            filter_resonance: 1.0,
            filter_envelope_amount: 0.0,
            filter_keyboard_tracking: 0.0,

            // Amp envelope
            amp_attack: 0.1,
            amp_decay: 0.3,
            amp_sustain: 0.7,
            amp_release: 0.5,

            // Filter envelope
            filter_attack: 0.1,
            filter_decay: 0.3,
            filter_sustain: 0.7,
            filter_release: 0.5,

            // LFO – waveform 0 = Triangle
            lfo_rate: 2.0,
            lfo_amount: 0.1,
            lfo_waveform: 0,
            lfo_sync: false,
            lfo_target_osc1_pitch: false,
            lfo_target_osc2_pitch: false,
            lfo_target_filter: false,
            lfo_target_amplitude: false,

            // Modulation matrix
            lfo_to_cutoff: 0.0,
            lfo_to_resonance: 0.0,
            lfo_to_osc1_pitch: 0.0,
            lfo_to_osc2_pitch: 0.0,
            lfo_to_amplitude: 0.0,
            velocity_to_cutoff: 0.0,
            velocity_to_amplitude: 0.5,

            // Effects
            reverb_amount: 0.0,
            reverb_size: 0.5,
            delay_time: 0.25,
            delay_feedback: 0.3,
            delay_amount: 0.0,

            // Arpeggiator
            arp_enabled: false,
            arp_rate: 120.0,
            arp_pattern: 0,
            arp_octaves: 1,
            arp_gate_length: 0.8,

            // Global
            master_volume: 0.5,
        }
    }
}

/// Lock-free synthesizer state for real-time audio processing
#[allow(dead_code)]
pub struct LockFreeSynth {
    pub params: TripleBuffer<SynthParameters>,

    // Atomic values for simple controls
    pub panic_requested: AtomicBool,
    pub sustain_pedal: AtomicBool,
    pub mono_mode: AtomicBool,
}

impl LockFreeSynth {
    pub fn new() -> Self {
        Self {
            params: TripleBuffer::new(SynthParameters::default()),
            panic_requested: AtomicBool::new(false),
            sustain_pedal: AtomicBool::new(false),
            mono_mode: AtomicBool::new(false),
        }
    }

    /// Update parameters (GUI thread)
    pub fn set_params(&mut self, params: SynthParameters) {
        self.params.write(params);
    }

    /// Get current parameters (audio thread)
    pub fn get_params(&self) -> &SynthParameters {
        self.params.read()
    }

    /// Request panic (from any thread)
    pub fn request_panic(&self) {
        self.panic_requested.store(true, Ordering::Release);
    }

    /// Check and clear panic request (audio thread)
    pub fn check_panic_request(&self) -> bool {
        self.panic_requested
            .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    /// Set sustain pedal (from any thread)
    pub fn set_sustain_pedal(&self, pressed: bool) {
        self.sustain_pedal.store(pressed, Ordering::Release);
    }

    /// Get sustain pedal state (audio thread)
    pub fn get_sustain_pedal(&self) -> bool {
        self.sustain_pedal.load(Ordering::Acquire)
    }

    /// Set mono mode (from any thread)
    pub fn set_mono_mode(&self, mono: bool) {
        self.mono_mode.store(mono, Ordering::Release);
    }

    /// Get mono mode state (audio thread)
    pub fn is_mono_mode(&self) -> bool {
        self.mono_mode.load(Ordering::Acquire)
    }
}

unsafe impl<T: Clone + Send> Send for TripleBuffer<T> {}
unsafe impl<T: Clone + Send> Sync for TripleBuffer<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synth_parameters_default_matches_synthesizer_defaults() {
        let params = SynthParameters::default();
        assert_eq!(params.osc1_waveform, 3);
        assert_eq!(params.osc2_waveform, 3);
        assert_eq!(params.osc1_level, 0.5);
        assert_eq!(params.osc2_level, 0.5);
        assert_eq!(params.osc1_detune, 0.0);
        assert_eq!(params.osc2_detune, 0.0);
        assert_eq!(params.osc1_pulse_width, 0.5);
        assert_eq!(params.osc2_pulse_width, 0.5);
        assert!(!params.osc2_sync);
        assert_eq!(params.noise_level, 0.0);
        assert_eq!(params.filter_cutoff, 1000.0);
        assert_eq!(params.filter_resonance, 1.0);
        assert_eq!(params.filter_envelope_amount, 0.0);
        assert_eq!(params.filter_keyboard_tracking, 0.0);
        assert!((params.amp_attack - 0.1).abs() < 0.001);
        assert!((params.amp_decay - 0.3).abs() < 0.001);
        assert!((params.amp_sustain - 0.7).abs() < 0.001);
        assert!((params.amp_release - 0.5).abs() < 0.001);
        assert_eq!(params.master_volume, 0.5);
        assert_eq!(params.lfo_waveform, 0);
        assert!(!params.lfo_sync);
        assert_eq!(params.velocity_to_amplitude, 0.5);
        assert!(!params.arp_enabled);
        assert_eq!(params.arp_rate, 120.0);
    }

    #[test]
    fn test_synth_parameters_is_copy() {
        let params = SynthParameters::default();
        let copy = params;
        assert_eq!(copy.master_volume, params.master_volume);
    }

    #[test]
    fn test_triple_buffer_write_read() {
        let mut buf = TripleBuffer::new(SynthParameters::default());
        let mut params = SynthParameters::default();
        params.master_volume = 0.42;
        buf.write(params);
        let read = buf.read();
        assert_eq!(read.master_volume, 0.42);
    }

    #[test]
    fn test_lock_free_synth_panic_request() {
        let synth = LockFreeSynth::new();
        assert!(!synth.check_panic_request());
        synth.request_panic();
        assert!(synth.check_panic_request());
        assert!(!synth.check_panic_request());
    }
}
