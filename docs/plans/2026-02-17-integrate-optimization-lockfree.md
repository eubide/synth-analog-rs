# Integration of optimization.rs and lock_free.rs Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Integrate the optimization lookup tables and lock-free triple buffer into the synthesizer, eliminating mutex contention in the audio callback and reducing CPU usage from transcendental math functions.

**Architecture:** The Synthesizer moves to be owned exclusively by the audio thread. GUI and MIDI communicate parameters via a lock-free TripleBuffer (SynthParameters). Discrete MIDI events (note_on/note_off) use a separate lightweight Mutex<VecDeque>. OptimizationTables replace inline sin()/powf() with pre-computed lookup tables.

**Tech Stack:** Rust, cpal, egui, midir, lazy_static (existing deps)

---

### Task 1: Expand SynthParameters to cover all synthesizer parameters

**Files:**
- Modify: `src/lock_free.rs:51-122`

**Step 1: Write the test**

Add to bottom of `src/lock_free.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synth_parameters_default_matches_synthesizer_defaults() {
        let params = SynthParameters::default();
        // Oscillator defaults
        assert_eq!(params.osc1_waveform, 3); // Sawtooth
        assert_eq!(params.osc2_waveform, 3); // Sawtooth
        assert_eq!(params.osc1_level, 0.5);
        assert_eq!(params.osc2_level, 0.5);
        assert_eq!(params.osc1_detune, 0.0);
        assert_eq!(params.osc2_detune, 0.0);
        assert_eq!(params.osc1_pulse_width, 0.5);
        assert_eq!(params.osc2_pulse_width, 0.5);
        assert!(!params.osc2_sync);
        // Mixer
        assert_eq!(params.noise_level, 0.0);
        // Filter
        assert_eq!(params.filter_cutoff, 1000.0);
        assert_eq!(params.filter_resonance, 1.0);
        assert_eq!(params.filter_envelope_amount, 0.0);
        assert_eq!(params.filter_keyboard_tracking, 0.0);
        // Amp envelope
        assert!((params.amp_attack - 0.1).abs() < 0.001);
        assert!((params.amp_decay - 0.3).abs() < 0.001);
        assert!((params.amp_sustain - 0.7).abs() < 0.001);
        assert!((params.amp_release - 0.5).abs() < 0.001);
        // Master
        assert_eq!(params.master_volume, 0.5);
        // LFO
        assert_eq!(params.lfo_waveform, 0); // Triangle
        assert!(!params.lfo_sync);
        // Modulation matrix
        assert_eq!(params.velocity_to_amplitude, 0.5);
        // Arpeggiator
        assert!(!params.arp_enabled);
        assert_eq!(params.arp_rate, 120.0);
    }

    #[test]
    fn test_synth_parameters_is_copy() {
        let params = SynthParameters::default();
        let copy = params; // This would fail to compile if not Copy
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
        // Second check should be false (consumed)
        assert!(!synth.check_panic_request());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib lock_free -- --nocapture`
Expected: FAIL - fields like `osc1_pulse_width`, `noise_level`, etc. don't exist yet

**Step 3: Expand SynthParameters**

Replace the entire `SynthParameters` struct and its `Default` impl in `src/lock_free.rs:51-122`:

```rust
/// Real-time safe analog synthesizer parameters
/// All primitive types to keep Copy + efficient TripleBuffer transfer
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
            // Oscillators
            osc1_waveform: 3,  // Sawtooth
            osc2_waveform: 3,  // Sawtooth
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
            // LFO
            lfo_rate: 2.0,
            lfo_amount: 0.1,
            lfo_waveform: 0, // Triangle
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
            arp_pattern: 0, // Up
            arp_octaves: 1,
            arp_gate_length: 0.8,
            // Global
            master_volume: 0.5,
        }
    }
}
```

Also remove the `#[allow(dead_code)]` annotations from `TripleBuffer`, `SynthParameters`, and `LockFreeSynth`.

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib lock_free -- --nocapture`
Expected: All 4 tests PASS

**Step 5: Commit**

```bash
git add src/lock_free.rs
git commit -m "Expand SynthParameters to cover all synthesizer parameters"
```

---

### Task 2: Add MidiEvent enum for note events

**Files:**
- Modify: `src/lock_free.rs`

**Step 1: Write the test**

Add to existing tests module in `src/lock_free.rs`:

```rust
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

        // Queue should be empty after drain
        let events = queue.drain();
        assert!(events.is_empty());
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib lock_free::tests::test_midi_event_queue -- --nocapture`
Expected: FAIL - MidiEvent and MidiEventQueue don't exist

**Step 3: Add MidiEvent and MidiEventQueue**

Add after `LockFreeSynth` impl block in `src/lock_free.rs`:

```rust
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
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib lock_free -- --nocapture`
Expected: All 5 tests PASS

**Step 5: Commit**

```bash
git add src/lock_free.rs
git commit -m "Add MidiEvent enum and event queue for lock-free note delivery"
```

---

### Task 3: Add apply_params and to_synth_params to Synthesizer

**Files:**
- Modify: `src/synthesizer.rs`

This bridges SynthParameters (flat primitive struct) with the Synthesizer's nested parameter types.

**Step 1: Write the test**

Add to bottom of `src/synthesizer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock_free::SynthParameters;

    #[test]
    fn test_to_synth_params_roundtrip() {
        let synth = Synthesizer::new();
        let params = synth.to_synth_params();

        // Verify key fields match
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

        // Modify some params
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

        // Voices should not be affected
        assert!(!synth.voices.is_empty());
        assert!(synth.voices.iter().any(|v| v.note == 60 && v.is_active));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib synthesizer::tests -- --nocapture`
Expected: FAIL - `to_synth_params` and `apply_params` don't exist

**Step 3: Implement the conversion methods**

Add to the `impl Synthesizer` block in `src/synthesizer.rs`:

```rust
    /// Extract current parameters as a flat SynthParameters struct
    pub fn to_synth_params(&self) -> crate::lock_free::SynthParameters {
        crate::lock_free::SynthParameters {
            osc1_waveform: Self::wave_type_to_u8(self.osc1.wave_type),
            osc2_waveform: Self::wave_type_to_u8(self.osc2.wave_type),
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
            lfo_waveform: Self::lfo_waveform_to_u8(self.lfo.waveform),
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
            arp_pattern: Self::arp_pattern_to_u8(self.arpeggiator.pattern),
            arp_octaves: self.arpeggiator.octaves,
            arp_gate_length: self.arpeggiator.gate_length,
            master_volume: self.master_volume,
        }
    }

    /// Apply flat SynthParameters to the synthesizer's nested structures
    /// Does NOT touch voice state, buffers, or LFO phase
    pub fn apply_params(&mut self, params: &crate::lock_free::SynthParameters) {
        self.osc1.wave_type = Self::u8_to_wave_type(params.osc1_waveform);
        self.osc2.wave_type = Self::u8_to_wave_type(params.osc2_waveform);
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
        self.lfo.waveform = Self::u8_to_lfo_waveform(params.lfo_waveform);
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
        self.arpeggiator.pattern = Self::u8_to_arp_pattern(params.arp_pattern);
        self.arpeggiator.octaves = params.arp_octaves;
        self.arpeggiator.gate_length = params.arp_gate_length;

        self.master_volume = params.master_volume;
    }

    fn wave_type_to_u8(wt: WaveType) -> u8 {
        match wt {
            WaveType::Sine => 0,
            WaveType::Square => 1,
            WaveType::Triangle => 2,
            WaveType::Sawtooth => 3,
        }
    }

    fn u8_to_wave_type(v: u8) -> WaveType {
        match v {
            0 => WaveType::Sine,
            1 => WaveType::Square,
            2 => WaveType::Triangle,
            3 => WaveType::Sawtooth,
            _ => WaveType::Sawtooth,
        }
    }

    fn lfo_waveform_to_u8(wf: LfoWaveform) -> u8 {
        match wf {
            LfoWaveform::Triangle => 0,
            LfoWaveform::Square => 1,
            LfoWaveform::Sawtooth => 2,
            LfoWaveform::ReverseSawtooth => 3,
            LfoWaveform::SampleAndHold => 4,
        }
    }

    fn u8_to_lfo_waveform(v: u8) -> LfoWaveform {
        match v {
            0 => LfoWaveform::Triangle,
            1 => LfoWaveform::Square,
            2 => LfoWaveform::Sawtooth,
            3 => LfoWaveform::ReverseSawtooth,
            4 => LfoWaveform::SampleAndHold,
            _ => LfoWaveform::Triangle,
        }
    }

    fn arp_pattern_to_u8(p: ArpPattern) -> u8 {
        match p {
            ArpPattern::Up => 0,
            ArpPattern::Down => 1,
            ArpPattern::UpDown => 2,
            ArpPattern::Random => 3,
        }
    }

    fn u8_to_arp_pattern(v: u8) -> ArpPattern {
        match v {
            0 => ArpPattern::Up,
            1 => ArpPattern::Down,
            2 => ArpPattern::UpDown,
            3 => ArpPattern::Random,
            _ => ArpPattern::Up,
        }
    }
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib synthesizer::tests -- --nocapture`
Expected: All 3 tests PASS

**Step 5: Commit**

```bash
git add src/synthesizer.rs
git commit -m "Add apply_params and to_synth_params for lock-free parameter bridge"
```

---

### Task 4: Integrate OptimizationTables in synthesizer.rs

**Files:**
- Modify: `src/synthesizer.rs`
- Modify: `src/optimization.rs` (remove `#[allow(dead_code)]`)

**Step 1: Write the test**

Add to `src/synthesizer.rs` tests module:

```rust
    #[test]
    fn test_note_to_frequency_matches_optimization_table() {
        // A4 = MIDI 69 = 440 Hz
        let freq = Synthesizer::note_to_frequency(69);
        assert!((freq - 440.0).abs() < 0.01);

        // Middle C = MIDI 60
        let freq = Synthesizer::note_to_frequency(60);
        assert!((freq - 261.63).abs() < 0.1);
    }

    #[test]
    fn test_oscillator_sine_output_range() {
        // Verify sine oscillator output stays in [-1, 1] range
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let output = Synthesizer::generate_oscillator_static(WaveType::Sine, phase, 0.5);
            assert!(output >= -1.0 && output <= 1.0, "Sine at phase {} = {}", phase, output);
        }
    }

    #[test]
    fn test_oscillator_sawtooth_output_range() {
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let output = Synthesizer::generate_oscillator_static(WaveType::Sawtooth, phase, 0.5);
            assert!(output >= -1.5 && output <= 1.5, "Saw at phase {} = {}", phase, output);
        }
    }
```

**Step 2: Run tests to verify they pass with current implementation**

Run: `cargo test --lib synthesizer::tests -- --nocapture`
Expected: PASS (baseline - these verify behavior doesn't change)

**Step 3: Replace inline calculations with lookup tables**

In `src/synthesizer.rs`, update imports at the top:

```rust
use crate::optimization::OPTIMIZATION_TABLES;
```

Replace `note_to_frequency`:

```rust
    pub fn note_to_frequency(note: u8) -> f32 {
        OPTIMIZATION_TABLES.get_midi_frequency(note)
    }
```

Replace the Sine case in `generate_oscillator_static`:

```rust
            WaveType::Sine => OPTIMIZATION_TABLES.fast_sin(phase * 2.0 * PI),
```

Replace the Sawtooth case in `generate_oscillator_static`:

```rust
            WaveType::Sawtooth => {
                // Band-limited sawtooth using sin harmonics (lookup table)
                let mut output = 0.0;
                for n in 1..=8 {
                    output += OPTIMIZATION_TABLES.fast_sin(n as f32 * phase * 2.0 * PI) / n as f32;
                }
                -2.0 * output / PI
            },
```

In `src/optimization.rs`, remove both `#[allow(dead_code)]` annotations from the struct and the lazy_static.

**Step 4: Run tests to verify they still pass**

Run: `cargo test --lib synthesizer::tests -- --nocapture`
Expected: All tests PASS (behavior preserved)

Also run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: No warnings from optimization.rs dead_code

**Step 5: Commit**

```bash
git add src/synthesizer.rs src/optimization.rs
git commit -m "Integrate optimization lookup tables for sine and MIDI frequency calculations"
```

---

### Task 5: Restructure audio_engine.rs to own Synthesizer

**Files:**
- Modify: `src/audio_engine.rs`

The audio engine now owns the Synthesizer directly and reads parameters from LockFreeSynth.

**Step 1: Rewrite audio_engine.rs**

Replace the entire `src/audio_engine.rs`:

```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream, StreamConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::synthesizer::Synthesizer;
use crate::lock_free::{LockFreeSynth, MidiEvent, MidiEventQueue};

pub struct AudioEngine {
    _stream: Stream,
}

impl AudioEngine {
    pub fn new(
        lock_free_synth: Arc<LockFreeSynth>,
        midi_events: Arc<MidiEventQueue>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or("No output device available")?;
        let config = device.default_output_config()
            .map_err(|e| format!("No default output config: {}", e))?;

        let sample_rate = config.sample_rate().0;
        log::info!("Audio engine initialized with {} Hz sample rate", sample_rate);

        let underrun_counter = Arc::new(AtomicUsize::new(0));

        let stream = match config.sample_format() {
            SampleFormat::F32 => Self::run::<f32>(&device, &config.into(), lock_free_synth, midi_events, underrun_counter, sample_rate)?,
            SampleFormat::I16 => Self::run::<i16>(&device, &config.into(), lock_free_synth, midi_events, underrun_counter, sample_rate)?,
            SampleFormat::U16 => Self::run::<u16>(&device, &config.into(), lock_free_synth, midi_events, underrun_counter, sample_rate)?,
            sample_format => return Err(format!("Unsupported sample format: {:?}", sample_format).into()),
        };

        stream.play().map_err(|e| format!("Failed to play stream: {}", e))?;

        Ok(Self { _stream: stream })
    }

    fn run<T>(
        device: &cpal::Device,
        config: &StreamConfig,
        lock_free_synth: Arc<LockFreeSynth>,
        midi_events: Arc<MidiEventQueue>,
        underrun_counter: Arc<AtomicUsize>,
        sample_rate: u32,
    ) -> Result<Stream, Box<dyn std::error::Error>>
    where
        T: Sample + cpal::SizedSample + cpal::FromSample<f32>,
    {
        let channels = config.channels as usize;

        // Synthesizer lives exclusively in the audio thread
        let mut synthesizer = Synthesizer::new();
        synthesizer.sample_rate = sample_rate as f32;

        // Pre-allocated buffer
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
                                // TODO: implement sustain pedal on Synthesizer
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
```

**Step 2: Verify it compiles (not runnable yet - main.rs still uses old API)**

Run: `cargo check 2>&1 | head -20`
Expected: Errors in main.rs (expected - will fix in Task 7)

**Step 3: Commit**

```bash
git add src/audio_engine.rs
git commit -m "Restructure audio engine to own Synthesizer and use lock-free params"
```

---

### Task 6: Restructure gui.rs to use lock-free parameters

**Files:**
- Modify: `src/gui.rs`

The GUI builds a local `SynthParameters`, modifies it via sliders, and writes to the TripleBuffer. For note events it sends via MidiEventQueue.

**Step 1: Rewrite gui.rs**

The core change: replace `Arc<Mutex<Synthesizer>>` with `Arc<LockFreeSynth>` + `Arc<MidiEventQueue>`. Instead of locking per panel, the GUI:
1. Reads current params from LockFreeSynth at start of frame into a local copy
2. All draw methods modify the local copy
3. At end of frame, writes the modified params back to LockFreeSynth

Replace the struct definition and constructor:

```rust
use eframe::egui;
use std::sync::Arc;
use crate::lock_free::{LockFreeSynth, SynthParameters, MidiEvent, MidiEventQueue};
use crate::synthesizer::{WaveType, ArpPattern, LfoWaveform, Synthesizer};
use crate::audio_engine::AudioEngine;
use crate::midi_handler::MidiHandler;

pub struct SynthApp {
    lock_free_synth: Arc<LockFreeSynth>,
    midi_events: Arc<MidiEventQueue>,
    _audio_engine: AudioEngine,
    _midi_handler: Option<MidiHandler>,
    last_key_times: std::collections::HashMap<egui::Key, std::time::Instant>,
    current_octave: i32,
    show_midi_monitor: bool,
    show_presets_window: bool,
    current_preset_name: String,
    new_preset_name: String,
    // Local copy of params, modified each frame then written back
    params: SynthParameters,
}

impl SynthApp {
    pub fn new(
        lock_free_synth: Arc<LockFreeSynth>,
        midi_events: Arc<MidiEventQueue>,
        audio_engine: AudioEngine,
        midi_handler: Option<MidiHandler>,
    ) -> Self {
        let params = *lock_free_synth.get_params();
        Self {
            lock_free_synth,
            midi_events,
            _audio_engine: audio_engine,
            _midi_handler: midi_handler,
            last_key_times: std::collections::HashMap::new(),
            current_octave: 4,
            show_midi_monitor: false,
            show_presets_window: false,
            current_preset_name: String::new(),
            new_preset_name: String::new(),
            params,
        }
    }
```

Every `draw_*` method changes from locking the mutex to operating on `self.params` directly. For example, `draw_vintage_oscillator_panel` becomes:

```rust
    fn draw_vintage_oscillator_panel(&mut self, ui: &mut egui::Ui, osc_num: u8) {
        ui.spacing_mut().item_spacing = egui::vec2(1.0, 1.0);

        let (waveform, detune, pulse_width, amplitude) = if osc_num == 1 {
            (&mut self.params.osc1_waveform, &mut self.params.osc1_detune,
             &mut self.params.osc1_pulse_width, &mut self.params.osc1_level)
        } else {
            (&mut self.params.osc2_waveform, &mut self.params.osc2_detune,
             &mut self.params.osc2_pulse_width, &mut self.params.osc2_level)
        };

        // Frequency controls
        ui.horizontal(|ui| {
            ui.label("freq:");
            ui.add_sized([70.0, 16.0], egui::Slider::new(detune, -12.0..=12.0)
                .step_by(0.1)
                .suffix(" st"));
        });

        // Wave type selector - convert u8 to WaveType for display
        let mut wave_type = Synthesizer::u8_to_wave_type_pub(*waveform);
        ui.horizontal(|ui| {
            ui.label("wave:");
            egui::ComboBox::from_id_source(format!("wave_{}", osc_num))
                .selected_text(match wave_type {
                    WaveType::Sawtooth => "Saw",
                    WaveType::Triangle => "Tri",
                    WaveType::Square => "Sqr",
                    WaveType::Sine => "Sin",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut wave_type, WaveType::Sawtooth, "Sawtooth");
                    ui.selectable_value(&mut wave_type, WaveType::Triangle, "Triangle");
                    ui.selectable_value(&mut wave_type, WaveType::Square, "Square");
                    ui.selectable_value(&mut wave_type, WaveType::Sine, "Sine");
                });
        });
        *waveform = Synthesizer::wave_type_to_u8_pub(wave_type);

        // Pulse Width (only for square waves)
        if wave_type == WaveType::Square {
            ui.horizontal(|ui| {
                ui.label("pw:");
                ui.add_sized([70.0, 16.0], egui::Slider::new(pulse_width, 0.1..=0.9)
                    .step_by(0.01));
            });
        }

        // Level control
        ui.horizontal(|ui| {
            ui.label("level:");
            ui.add_sized([70.0, 16.0], egui::Slider::new(amplitude, 0.0..=1.0)
                .step_by(0.01));
        });

        // Sync control (only for oscillator B)
        if osc_num == 2 {
            ui.horizontal(|ui| {
                ui.label("sync:");
                ui.checkbox(&mut self.params.osc2_sync, "oscillator A");
            });
        }
    }
```

The `update()` method in `impl eframe::App for SynthApp` reads params at frame start and writes at frame end:

```rust
impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Read current params at start of frame
        self.params = *self.lock_free_synth.get_params();

        // Handle keyboard input - send note events through MidiEventQueue
        ctx.input(|i| {
            // ... octave changes same as before ...

            for (key, note_offset) in key_map {
                let midi_note = self.current_octave * 12 + note_offset;

                if i.key_pressed(key) {
                    // ... same debounce logic ...
                    if should_trigger {
                        self.last_key_times.insert(key, now);
                        self.midi_events.push(MidiEvent::NoteOn {
                            note: midi_note as u8,
                            velocity: 100,
                        });
                    }
                }

                if i.key_released(key) {
                    self.last_key_times.remove(&key);
                    self.midi_events.push(MidiEvent::NoteOff {
                        note: midi_note as u8,
                    });
                }
            }
        });

        // ... all draw code using self.params instead of synth.lock() ...

        // Write params back at end of frame
        self.lock_free_synth.set_params(self.params);
    }
}
```

**Important:** The `draw_preset_panel` needs special handling because `save_preset`/`load_preset` need a Synthesizer instance. For presets, create a temporary Synthesizer, apply params to it for save, or load from it and extract params. This preserves preset functionality without keeping a shared Synthesizer.

For preset save:
```rust
if ui.add_enabled(save_enabled, egui::Button::new("Save")).clicked() {
    let mut temp_synth = Synthesizer::new();
    temp_synth.apply_params(&self.params);
    if let Err(e) = temp_synth.save_preset(&self.new_preset_name) {
        log::error!("Error saving preset: {}", e);
    } else {
        log::info!("Preset '{}' saved!", self.new_preset_name);
        self.current_preset_name = self.new_preset_name.clone();
        self.new_preset_name.clear();
    }
}
```

For preset load:
```rust
if ui.add_sized([ui.available_width(), 18.0], button).clicked() {
    let mut temp_synth = Synthesizer::new();
    if let Err(e) = temp_synth.load_preset(preset) {
        log::error!("Error loading preset {}: {}", preset, e);
    } else {
        log::info!("Preset '{}' loaded!", preset);
        self.params = temp_synth.to_synth_params();
        self.current_preset_name = preset.clone();
    }
}
```

This is a large rewrite. The agent implementing this should convert ALL `draw_*` methods to use `self.params` fields instead of `synth.lock().unwrap()`, following the same pattern shown above for the oscillator panel.

**Note:** The `Synthesizer::u8_to_wave_type_pub` and `Synthesizer::wave_type_to_u8_pub` methods need to be added as public wrappers around the private conversion methods from Task 3, or those methods should be made `pub` instead of private.

Make the conversion methods in `synthesizer.rs` pub:
- `wave_type_to_u8` -> `pub fn wave_type_to_u8_pub`
- `u8_to_wave_type` -> `pub fn u8_to_wave_type_pub`
- `lfo_waveform_to_u8` -> `pub fn lfo_waveform_to_u8_pub`
- `u8_to_lfo_waveform` -> `pub fn u8_to_lfo_waveform_pub`
- `arp_pattern_to_u8` -> `pub fn arp_pattern_to_u8_pub`
- `u8_to_arp_pattern` -> `pub fn u8_to_arp_pattern_pub`

**Step 2: Verify it compiles (not fully runnable yet - main.rs still old)**

Run: `cargo check 2>&1 | head -30`
Expected: Errors only from main.rs

**Step 3: Commit**

```bash
git add src/gui.rs src/synthesizer.rs
git commit -m "Restructure GUI to use lock-free params instead of mutex"
```

---

### Task 7: Restructure midi_handler.rs for lock-free communication

**Files:**
- Modify: `src/midi_handler.rs`

MIDI handler sends note events via MidiEventQueue and parameter changes via LockFreeSynth.

**Step 1: Rewrite midi_handler.rs**

Replace the synthesizer mutex with LockFreeSynth + MidiEventQueue:

```rust
use midir::{MidiInput, Ignore};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use crate::lock_free::{LockFreeSynth, SynthParameters, MidiEvent, MidiEventQueue};

#[derive(Clone, Debug)]
pub struct MidiMessage {
    pub timestamp: std::time::Instant,
    pub message_type: String,
    pub description: String,
}

pub struct MidiHandler {
    _connection: Option<midir::MidiInputConnection<()>>,
    pub message_history: Arc<Mutex<VecDeque<MidiMessage>>>,
}

impl MidiHandler {
    pub fn new(
        lock_free_synth: Arc<LockFreeSynth>,
        midi_events: Arc<MidiEventQueue>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let message_history = Arc::new(Mutex::new(VecDeque::new()));
        let mut midi_in = MidiInput::new("Rust Synthesizer MIDI Input")?;
        midi_in.ignore(Ignore::None);

        let in_ports = midi_in.ports();

        if in_ports.is_empty() {
            log::info!("No MIDI input ports available");
            return Ok(MidiHandler {
                _connection: None,
                message_history,
            });
        }

        for (i, port) in in_ports.iter().enumerate() {
            log::info!("MIDI port {}: {}", i, midi_in.port_name(port).unwrap_or_else(|_| "Unknown".to_string()));
        }

        let in_port = &in_ports[0];
        let port_name = midi_in.port_name(in_port).unwrap_or_else(|_| "Unknown".to_string());
        log::info!("Connecting to MIDI port: {}", port_name);

        let history_clone = message_history.clone();
        let connection = midi_in.connect(in_port, "synth-input", move |_stamp, message, _| {
            Self::handle_midi_message(message, &lock_free_synth, &midi_events, &history_clone);
        }, ())?;

        Ok(MidiHandler {
            _connection: Some(connection),
            message_history,
        })
    }

    fn handle_midi_message(
        message: &[u8],
        lock_free_synth: &Arc<LockFreeSynth>,
        midi_events: &Arc<MidiEventQueue>,
        history: &Arc<Mutex<VecDeque<MidiMessage>>>,
    ) {
        if message.len() >= 3 {
            let status = message[0];
            let data1 = message[1];
            let data2 = message[2];
            let channel = (status & 0x0F) + 1;

            let (msg_type, description) = match status & 0xF0 {
                0x90 => {
                    if data2 > 0 {
                        midi_events.push(MidiEvent::NoteOn { note: data1, velocity: data2 });
                        ("Note On".to_string(), format!("Note: {} Vel: {} Ch: {}", Self::note_name(data1), data2, channel))
                    } else {
                        midi_events.push(MidiEvent::NoteOff { note: data1 });
                        ("Note Off".to_string(), format!("Note: {} (vel 0) Ch: {}", Self::note_name(data1), channel))
                    }
                },
                0x80 => {
                    midi_events.push(MidiEvent::NoteOff { note: data1 });
                    ("Note Off".to_string(), format!("Note: {} Vel: {} Ch: {}", Self::note_name(data1), data2, channel))
                },
                0xB0 => {
                    Self::handle_cc_message(lock_free_synth, data1, data2);
                    ("CC".to_string(), format!("CC: {} Val: {} Ch: {}", data1, data2, channel))
                },
                0xC0 => ("Program".to_string(), format!("Program: {} Ch: {}", data1, channel)),
                0xD0 => ("Pressure".to_string(), format!("Pressure: {} Ch: {}", data1, channel)),
                0xE0 => {
                    let bend_value = ((data2 as u16) << 7) | (data1 as u16);
                    ("Pitch Bend".to_string(), format!("Bend: {} Ch: {}", bend_value, channel))
                },
                _ => ("Unknown".to_string(), format!("Status: 0x{:02X} Data: {} {} Ch: {}", status, data1, data2, channel)),
            };

            if let Ok(mut hist) = history.lock() {
                hist.push_back(MidiMessage {
                    timestamp: std::time::Instant::now(),
                    message_type: msg_type,
                    description,
                });
                if hist.len() > 100 {
                    hist.pop_front();
                }
            }
        }
    }

    fn note_name(note: u8) -> String {
        let notes = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        let octave = (note / 12) as i32 - 1;
        let note_index = note % 12;
        format!("{}{}", notes[note_index as usize], octave)
    }

    fn handle_cc_message(lock_free_synth: &Arc<LockFreeSynth>, cc_number: u8, cc_value: u8) {
        let normalized_value = cc_value as f32 / 127.0;
        // Read current params, modify, write back
        let mut params = *lock_free_synth.get_params();

        match cc_number {
            1 => params.osc1_level = normalized_value,
            2 => params.osc2_level = normalized_value,
            3 => params.osc1_detune = -12.0 + (normalized_value * 24.0),
            4 => params.osc2_detune = -12.0 + (normalized_value * 24.0),
            5 => params.osc1_pulse_width = 0.1 + (normalized_value * 0.8),
            6 => params.osc2_pulse_width = 0.1 + (normalized_value * 0.8),
            7 => params.mixer_osc1_level = normalized_value,
            8 => params.mixer_osc2_level = normalized_value,
            9 => params.noise_level = normalized_value,
            16 => params.filter_cutoff = 20.0 + (normalized_value * 19980.0),
            17 => params.filter_resonance = normalized_value * 10.0,
            18 => params.filter_envelope_amount = normalized_value,
            19 => params.filter_keyboard_tracking = normalized_value,
            20 => params.filter_attack = normalized_value * 5.0,
            21 => params.filter_decay = normalized_value * 5.0,
            22 => params.filter_sustain = normalized_value,
            23 => params.filter_release = normalized_value * 5.0,
            24 => params.amp_attack = normalized_value * 5.0,
            25 => params.amp_decay = normalized_value * 5.0,
            26 => params.amp_sustain = normalized_value,
            27 => params.amp_release = normalized_value * 5.0,
            28 => params.lfo_rate = 0.1 + (normalized_value * 19.9),
            29 => params.lfo_amount = normalized_value,
            30 => params.lfo_target_osc1_pitch = normalized_value > 0.5,
            31 => params.lfo_target_osc2_pitch = normalized_value > 0.5,
            32 => params.lfo_target_filter = normalized_value > 0.5,
            33 => params.lfo_target_amplitude = normalized_value > 0.5,
            34 => params.master_volume = normalized_value,
            40 => params.reverb_amount = normalized_value,
            41 => params.reverb_size = normalized_value,
            42 => params.delay_time = 0.01 + (normalized_value * 1.99),
            43 => params.delay_feedback = normalized_value * 0.95,
            44 => params.delay_amount = normalized_value,
            50 => params.arp_enabled = normalized_value > 0.5,
            51 => params.arp_rate = 60.0 + (normalized_value * 180.0),
            52 => params.arp_pattern = (normalized_value * 3.99) as u8,
            53 => params.arp_octaves = 1 + (normalized_value * 3.0) as u8,
            54 => params.arp_gate_length = 0.1 + (normalized_value * 0.9),
            64 => {
                // Sustain pedal
                midi_events_from_cc(lock_free_synth, normalized_value > 0.5);
                return; // Don't write params for sustain - it's an event
            },
            _ => return, // Unmapped CCs - don't write params
        }

        lock_free_synth.set_params(params);
    }
}

// Note: CC64 sustain pedal needs the MidiEventQueue. Since handle_cc_message
// doesn't have access to it in the current signature, sustain pedal via CC will
// need the queue passed through. For now, sustain pedal via CC is a TODO.
// The MIDI handler already handles note on/off which are the primary events.

impl Drop for MidiHandler {
    fn drop(&mut self) {
        if self._connection.is_some() {
            log::info!("MIDI connection closed");
        }
    }
}
```

**Note:** The `handle_cc_message` function needs `midi_events` for CC64 sustain pedal. The simplest fix is to pass `midi_events` to `handle_midi_message` which already receives it, and thread it through to `handle_cc_message`. The code above already has `midi_events` in `handle_midi_message` - update `handle_cc_message` signature to accept it:

```rust
    fn handle_cc_message(lock_free_synth: &Arc<LockFreeSynth>, midi_events: &Arc<MidiEventQueue>, cc_number: u8, cc_value: u8) {
        // ... same as above but CC64 becomes:
        64 => {
            let pressed = normalized_value > 0.5;
            midi_events.push(MidiEvent::SustainPedal { pressed });
            return;
        },
    }
```

And update the call site in `handle_midi_message`:
```rust
                0xB0 => {
                    Self::handle_cc_message(lock_free_synth, midi_events, data1, data2);
                    ...
                },
```

**Step 2: Commit**

```bash
git add src/midi_handler.rs
git commit -m "Restructure MIDI handler for lock-free parameter and event delivery"
```

---

### Task 8: Restructure main.rs to wire everything together

**Files:**
- Modify: `src/main.rs`

**Step 1: Rewrite main.rs**

```rust
use eframe::egui;
use std::sync::Arc;

mod synthesizer;
mod audio_engine;
mod gui;
mod midi_handler;
mod optimization;
mod lock_free;

use audio_engine::AudioEngine;
use gui::SynthApp;
use midi_handler::MidiHandler;
use lock_free::{LockFreeSynth, MidiEventQueue};

fn main() -> Result<(), eframe::Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Analog Synthesizer");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 600.0])
            .with_title("Rust Synthesizer"),
        ..Default::default()
    };

    // Create lock-free shared state
    let lock_free_synth = Arc::new(LockFreeSynth::new());
    let midi_events = Arc::new(MidiEventQueue::new());

    // Initialize audio engine (owns the Synthesizer)
    let audio_engine = match AudioEngine::new(lock_free_synth.clone(), midi_events.clone()) {
        Ok(engine) => {
            log::info!("Audio engine initialized successfully");
            engine
        },
        Err(e) => {
            log::error!("Failed to initialize audio engine: {}", e);
            log::error!("Please check your audio device configuration.");
            std::process::exit(1);
        }
    };

    // Initialize MIDI input
    let midi_handler = match MidiHandler::new(lock_free_synth.clone(), midi_events.clone()) {
        Ok(handler) => {
            log::info!("MIDI input initialized successfully");
            Some(handler)
        },
        Err(e) => {
            log::warn!("Failed to initialize MIDI input: {}", e);
            log::warn!("Continuing without MIDI support...");
            None
        }
    };

    eframe::run_native(
        "Rust Synthesizer",
        options,
        Box::new(move |_cc| Ok(Box::new(SynthApp::new(
            lock_free_synth,
            midi_events,
            audio_engine,
            midi_handler,
        )))),
    )
}
```

**Step 2: Build and verify**

Run: `cargo build --release 2>&1`
Expected: Compiles with zero errors. May have some warnings if dead_code annotations not fully cleaned.

**Step 3: Run**

Run: `cargo run --release`
Expected: Synthesizer starts, GUI displays, audio plays when keyboard keys pressed.

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "Wire main.rs to use lock-free architecture"
```

---

### Task 9: Fix LockFreeSynth mutability for set_params

**Files:**
- Modify: `src/lock_free.rs`

The current `LockFreeSynth::set_params` takes `&mut self` but we need to call it from behind an `Arc` (shared reference). The `TripleBuffer::write` also takes `&mut self`. We need to fix the API to work with `&self` using interior mutability.

**Step 1: Update TripleBuffer to use UnsafeCell or atomic swap**

The simplest correct approach: use `std::cell::UnsafeCell` for the write buffer since we guarantee single-writer semantics (GUI writes params once per frame).

Actually, the cleaner approach for `SynthParameters` (which is `Copy`) is to use a simpler pattern: store the write buffer in an `UnsafeCell` and protect it with an `AtomicBool` write lock.

However, the simplest fix that maintains safety: make `LockFreeSynth` wrap the `TripleBuffer` in a `Mutex` (lightweight, only GUI writes to it and it's once per frame). This is NOT the audio thread mutex - the audio thread only reads via `get_params()` which uses the atomic swap in TripleBuffer.

Wait - actually the TripleBuffer design already has the issue that `read()` mutates internal atomic state. Let's redesign with `UnsafeCell`:

```rust
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::cell::UnsafeCell;

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

    /// Read current data (audio thread only - single reader assumed)
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
```

And update `LockFreeSynth::set_params` to take `&self`:

```rust
    pub fn set_params(&self, params: SynthParameters) {
        self.params.write(params);
    }
```

**Step 2: Run all tests**

Run: `cargo test -- --nocapture`
Expected: All tests pass

**Step 3: Build and run**

Run: `cargo build --release && cargo run --release`
Expected: Compiles and runs correctly

**Step 4: Commit**

```bash
git add src/lock_free.rs
git commit -m "Fix TripleBuffer to use interior mutability for shared references"
```

---

### Task 10: Clean up dead_code warnings and final verification

**Files:**
- Modify: `src/lock_free.rs` (remove remaining `#[allow(dead_code)]`)
- Modify: `src/optimization.rs` (remove remaining `#[allow(dead_code)]`)

**Step 1: Remove all #[allow(dead_code)] annotations**

Search and remove all `#[allow(dead_code)]` in `src/lock_free.rs` and `src/optimization.rs`.

**Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS with no warnings

**Step 3: Run all tests**

Run: `cargo test -- --nocapture`
Expected: All tests PASS

**Step 4: Build release and verify**

Run: `cargo build --release`
Expected: Zero warnings, zero errors

**Step 5: Manual test**

Run: `cargo run --release`
Expected:
- GUI launches with all panels working
- Keyboard keys produce sound
- Sliders modify parameters in real-time
- Presets can be saved and loaded
- MIDI input works (if device connected)

**Step 6: Commit**

```bash
git add -A
git commit -m "Remove dead_code annotations - all modules now integrated"
```
