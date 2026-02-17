use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::cell::UnsafeCell;

/// Lock-free triple buffer for real-time parameter updates
/// GUI writes to one buffer, audio reads from another, third is for swapping
/// Single-writer, single-reader assumed.
pub struct TripleBuffer<T: Clone> {
    buffers: UnsafeCell<[T; 3]>,
    write_index: AtomicUsize,
    read_index: AtomicUsize,
    swap_index: AtomicUsize,
    new_data: AtomicBool,
}

impl<T: Clone> TripleBuffer<T> {
    pub fn new(initial_value: T) -> Self {
        Self {
            buffers: UnsafeCell::new([initial_value.clone(), initial_value.clone(), initial_value]),
            write_index: AtomicUsize::new(0),
            read_index: AtomicUsize::new(1),
            swap_index: AtomicUsize::new(2),
            new_data: AtomicBool::new(false),
        }
    }

    /// Write new data (GUI thread only - single writer assumed)
    pub fn write(&self, data: T) {
        let write_idx = self.write_index.load(Ordering::Relaxed);
        unsafe {
            (*self.buffers.get())[write_idx] = data;
        }
        // Swap write and swap buffers
        let swap_idx = self.swap_index.swap(write_idx, Ordering::AcqRel);
        self.write_index.store(swap_idx, Ordering::Release);
        self.new_data.store(true, Ordering::Release);
    }

    /// Lock-free read for audio thread (single reader assumed)
    pub fn read(&self) -> &T {
        if self.new_data.compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            let swap_idx = self.swap_index.swap(
                self.read_index.load(Ordering::Relaxed),
                Ordering::AcqRel,
            );
            self.read_index.store(swap_idx, Ordering::Release);
        }
        let read_idx = self.read_index.load(Ordering::Acquire);
        unsafe { &(*self.buffers.get())[read_idx] }
    }
}

unsafe impl<T: Clone + Send> Send for TripleBuffer<T> {}
unsafe impl<T: Clone + Send> Sync for TripleBuffer<T> {}

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

#[allow(dead_code)]
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
    pub fn set_params(&self, params: SynthParameters) {
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

/// Discrete MIDI events that need guaranteed delivery (not continuous params)
#[derive(Debug, Clone)]
pub enum MidiEvent {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    SustainPedal { pressed: bool },
}

/// Lightweight event queue for MIDI note events
/// Uses Mutex because note events are infrequent (human-speed, not audio-rate)
pub struct MidiEventQueue {
    events: std::sync::Mutex<Vec<MidiEvent>>,
}

impl MidiEventQueue {
    pub fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::with_capacity(32)),
        }
    }

    /// Push an event (called from MIDI/GUI thread)
    pub fn push(&self, event: MidiEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }

    /// Drain all events (called from audio thread at start of each block)
    pub fn drain(&self) -> Vec<MidiEvent> {
        if let Ok(mut events) = self.events.lock() {
            std::mem::take(&mut *events)
        } else {
            Vec::new()
        }
    }
}

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
        let buf = TripleBuffer::new(SynthParameters::default());
        let params = SynthParameters { master_volume: 0.42, ..SynthParameters::default() };
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

    #[test]
    fn test_midi_event_queue() {
        let queue = MidiEventQueue::new();
        queue.push(MidiEvent::NoteOn { note: 60, velocity: 100 });
        queue.push(MidiEvent::NoteOff { note: 60 });
        queue.push(MidiEvent::SustainPedal { pressed: true });

        let events = queue.drain();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], MidiEvent::NoteOn { note: 60, velocity: 100 }));
        assert!(matches!(events[1], MidiEvent::NoteOff { note: 60 }));
        assert!(matches!(events[2], MidiEvent::SustainPedal { pressed: true }));

        let events = queue.drain();
        assert!(events.is_empty());
    }
}
