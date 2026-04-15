use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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
        if self
            .new_data
            .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            let swap_idx = self
                .swap_index
                .swap(self.read_index.load(Ordering::Relaxed), Ordering::AcqRel);
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

    // Poly Mod (Prophet-5) — todas en rango -1.0..=1.0
    pub poly_mod_filter_env_to_osc_a_freq: f32, // ±24 semitones a plena excursión
    pub poly_mod_filter_env_to_osc_a_pw: f32,   // shift de pulse width
    pub poly_mod_osc_b_to_osc_a_freq: f32,      // ±24 semitones a plena excursión
    pub poly_mod_osc_b_to_osc_a_pw: f32,        // shift de pulse width
    pub poly_mod_osc_b_to_filter_cutoff: f32,   // ±4 kHz a plena excursión

    // Glide / Portamento
    pub glide_time: f32, // 0.0 = off, >0 = segundos de deslizamiento

    // Pitch bend
    pub pitch_bend: f32,       // -1.0..=1.0 (centro = 0.0)
    pub pitch_bend_range: u8,  // semitones, typically 2 or 12

    // Aftertouch (channel pressure)
    pub aftertouch: f32,              // 0.0..=1.0
    pub aftertouch_to_cutoff: f32,    // 0.0..=1.0 — amount que mapea a ±4 kHz en cutoff
    pub aftertouch_to_amplitude: f32, // 0.0..=1.0 — amount que mapea a ±amplitude

    // Expression pedal (CC 11): scales master output level
    pub expression: f32, // 0.0..=1.0, default 1.0 (full)

    // Mod wheel (CC 1): scales LFO depth to all active targets
    pub mod_wheel: f32, // 0.0..=1.0

    // Voice mode: 0=Poly, 1=Mono, 2=Legato, 3=Unison
    pub voice_mode: u8,
    // Note priority (for Mono/Legato): 0=Last, 1=Low, 2=High
    pub note_priority: u8,
    // Unison detune spread in cents
    pub unison_spread: f32,
    // Maximum polyphony: 1..=8
    pub max_voices: u8,

    // MIDI clock sync for arpeggiator
    pub arp_sync_to_midi: bool,
}

impl Default for SynthParameters {
    fn default() -> Self {
        Self {
            // Oscillators – waveform 3 = Sawtooth (Prophet-5 default)
            osc1_waveform: 3,
            osc2_waveform: 3,
            osc1_level: 1.0,
            osc2_level: 0.8,
            osc1_detune: 0.0,
            osc2_detune: 0.0,
            osc1_pulse_width: 0.5,
            osc2_pulse_width: 0.5,
            osc2_sync: false,

            // Mixer
            mixer_osc1_level: 0.8,
            mixer_osc2_level: 0.6,
            noise_level: 0.0,

            // Filter – open enough to hear full spectrum
            filter_cutoff: 5000.0,
            filter_resonance: 1.0,
            filter_envelope_amount: 0.0,
            filter_keyboard_tracking: 0.0,

            // Amp envelope – snappy response
            amp_attack: 0.01,
            amp_decay: 0.3,
            amp_sustain: 0.8,
            amp_release: 0.3,

            // Filter envelope
            filter_attack: 0.01,
            filter_decay: 0.3,
            filter_sustain: 0.7,
            filter_release: 0.3,

            // LFO – waveform 0 = Triangle
            lfo_rate: 2.0,
            lfo_amount: 1.0,
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
            master_volume: 0.7,

            // Poly Mod — off por defecto
            poly_mod_filter_env_to_osc_a_freq: 0.0,
            poly_mod_filter_env_to_osc_a_pw: 0.0,
            poly_mod_osc_b_to_osc_a_freq: 0.0,
            poly_mod_osc_b_to_osc_a_pw: 0.0,
            poly_mod_osc_b_to_filter_cutoff: 0.0,

            // Glide — off por defecto
            glide_time: 0.0,

            // Pitch bend — centrado por defecto
            pitch_bend: 0.0,
            pitch_bend_range: 2,

            // Aftertouch — off por defecto
            aftertouch: 0.0,
            aftertouch_to_cutoff: 0.5,
            aftertouch_to_amplitude: 0.0,

            // Expression pedal — abierta al máximo por defecto
            expression: 1.0,

            // Mod wheel — centrado en cero por defecto
            mod_wheel: 0.0,

            // Voice mode — polyphonic por defecto
            voice_mode: 0,
            note_priority: 0,
            unison_spread: 10.0,
            max_voices: 8,

            // MIDI clock sync — desactivado por defecto
            arp_sync_to_midi: false,
        }
    }
}

/// Lock-free synthesizer state for real-time audio processing
pub struct LockFreeSynth {
    pub params: TripleBuffer<SynthParameters>,
}

impl LockFreeSynth {
    pub fn new() -> Self {
        Self {
            params: TripleBuffer::new(SynthParameters::default()),
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
}

/// Discrete MIDI events that need guaranteed delivery (not continuous params)
#[derive(Debug, Clone)]
pub enum MidiEvent {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    SustainPedal { pressed: bool },
    /// Program Change: load preset at position `program` (0-indexed) in sorted list
    ProgramChange { program: u8 },
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
        assert_eq!(params.osc1_level, 1.0);
        assert_eq!(params.osc2_level, 0.8);
        assert_eq!(params.osc1_detune, 0.0);
        assert_eq!(params.osc2_detune, 0.0);
        assert_eq!(params.osc1_pulse_width, 0.5);
        assert_eq!(params.osc2_pulse_width, 0.5);
        assert!(!params.osc2_sync);
        assert_eq!(params.noise_level, 0.0);
        assert_eq!(params.filter_cutoff, 5000.0);
        assert_eq!(params.filter_resonance, 1.0);
        assert_eq!(params.filter_envelope_amount, 0.0);
        assert_eq!(params.filter_keyboard_tracking, 0.0);
        assert!((params.amp_attack - 0.01).abs() < 0.001);
        assert!((params.amp_decay - 0.3).abs() < 0.001);
        assert!((params.amp_sustain - 0.8).abs() < 0.001);
        assert!((params.amp_release - 0.3).abs() < 0.001);
        assert_eq!(params.master_volume, 0.7);
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
        let params = SynthParameters {
            master_volume: 0.42,
            ..SynthParameters::default()
        };
        buf.write(params);
        let read = buf.read();
        assert_eq!(read.master_volume, 0.42);
    }

    #[test]
    fn test_midi_event_queue() {
        let queue = MidiEventQueue::new();
        queue.push(MidiEvent::NoteOn {
            note: 60,
            velocity: 100,
        });
        queue.push(MidiEvent::NoteOff { note: 60 });
        queue.push(MidiEvent::SustainPedal { pressed: true });

        let events = queue.drain();
        assert_eq!(events.len(), 3);
        assert!(matches!(
            events[0],
            MidiEvent::NoteOn {
                note: 60,
                velocity: 100
            }
        ));
        assert!(matches!(events[1], MidiEvent::NoteOff { note: 60 }));
        assert!(matches!(
            events[2],
            MidiEvent::SustainPedal { pressed: true }
        ));

        let events = queue.drain();
        assert!(events.is_empty());
    }
}
