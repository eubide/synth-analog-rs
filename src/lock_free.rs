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
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SynthParameters {
    // Global controls
    pub master_volume: f32,
    pub master_tune: f32,

    // Oscillator parameters
    pub osc1_waveform: u8, // 0=sine, 1=saw, 2=square, 3=triangle
    pub osc2_waveform: u8,
    pub osc1_level: f32,
    pub osc2_level: f32,
    pub osc1_detune: f32,
    pub osc2_detune: f32,
    pub osc_mix: f32,

    // Filter parameters
    pub filter_cutoff: f32,
    pub filter_resonance: f32,
    pub filter_envelope_amount: f32,

    // Envelope parameters
    pub amp_attack: f32,
    pub amp_decay: f32,
    pub amp_sustain: f32,
    pub amp_release: f32,

    pub filter_attack: f32,
    pub filter_decay: f32,
    pub filter_sustain: f32,
    pub filter_release: f32,

    // LFO parameters
    pub lfo_rate: f32,
    pub lfo_amount: f32,

    // Effects
    pub reverb_amount: f32,
    pub delay_amount: f32,
}

impl Default for SynthParameters {
    fn default() -> Self {
        Self {
            master_volume: 0.7,
            master_tune: 0.0,
            osc1_waveform: 1, // Saw
            osc2_waveform: 1, // Saw
            osc1_level: 0.5,
            osc2_level: 0.5,
            osc1_detune: 0.0,
            osc2_detune: 0.02, // Slight detune for richness
            osc_mix: 0.5,
            filter_cutoff: 1000.0,
            filter_resonance: 0.5,
            filter_envelope_amount: 0.5,
            amp_attack: 0.01,
            amp_decay: 0.3,
            amp_sustain: 0.7,
            amp_release: 0.5,
            filter_attack: 0.01,
            filter_decay: 0.5,
            filter_sustain: 0.5,
            filter_release: 1.0,
            lfo_rate: 5.0,
            lfo_amount: 0.0,
            reverb_amount: 0.2,
            delay_amount: 0.0,
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
