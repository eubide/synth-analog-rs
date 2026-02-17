# Integration of optimization.rs and lock_free.rs

## Context

The synthesizer has two implemented but unused modules:
- `optimization.rs`: Lookup tables for sine, exponential, MIDI frequencies, and voice scaling
- `lock_free.rs`: Triple buffer and lock-free parameter passing for real-time audio

Currently, the synth uses `Arc<Mutex<Synthesizer>>` shared between GUI, audio, and MIDI threads. The audio callback uses `try_lock()` which causes underruns when the GUI holds the lock.

## Design

### 1. OptimizationTables Integration

Replace expensive per-sample calculations in `synthesizer.rs` with pre-computed lookup tables:

- `note_to_frequency(note)` -> `OPTIMIZATION_TABLES.get_midi_frequency(note)` (eliminates `powf` per note)
- `generate_oscillator_static` Sine case: `.sin()` -> `OPTIMIZATION_TABLES.fast_sin()` (4096-point table + cubic interpolation)
- `generate_oscillator_static` Sawtooth case: 8 harmonic `.sin()` calls -> `OPTIMIZATION_TABLES.fast_sin()` (biggest win: 8 sin calls per voice per sample)

### 2. SynthParameters Expansion

Expand `SynthParameters` in `lock_free.rs` to cover all synthesizer parameters. Add missing fields:

- `osc1_pulse_width`, `osc2_pulse_width`, `osc2_sync`
- `noise_level`, `filter_keyboard_tracking`
- `lfo_waveform`, `lfo_sync`, LFO targets (4 bools)
- Modulation matrix (6 f32 fields)
- Effects: `reverb_size`, `delay_time`, `delay_feedback`
- Arpeggiator: `enabled`, `rate`, `pattern`, `octaves`, `gate_length`

All primitive types (`f32`, `u8`, `bool`) to keep `Copy` + efficient TripleBuffer transfer.

### 3. Thread Communication Restructure

**Before:**
```
GUI --Arc<Mutex<Synthesizer>>--> Audio Thread
MIDI -/                           \-- try_lock() -> underruns on contention
```

**After:**
```
GUI ---\
        +-- Arc<LockFreeSynth> (TripleBuffer<SynthParameters>) --> Audio Thread (owns Synthesizer)
MIDI --/                                                            \-- reads params, applies, processes
```

Changes per file:
- **main.rs**: Create `Arc<LockFreeSynth>` instead of `Arc<Mutex<Synthesizer>>`. Synthesizer created inside audio engine.
- **audio_engine.rs**: Owns `Synthesizer` directly. Each callback reads params from TripleBuffer, applies to local synth, processes audio.
- **gui.rs**: Builds local `SynthParameters`, modifies via sliders, writes to TripleBuffer once per frame. Add `apply_params()` method on Synthesizer to apply SynthParameters.
- **midi_handler.rs**: Reads current params, applies MIDI CC changes, writes back.

### 4. MIDI Event Handling

Note events (note_on/note_off/sustain) are discrete and infrequent compared to audio. Use a separate `Arc<Mutex<VecDeque<MidiEvent>>>` for these events. Audio thread drains the queue at the start of each `process_block()`.

### Non-Goals

- No changes to the GUI layout or visual design
- No changes to the synthesis algorithms themselves
- No new dependencies (use existing `lazy_static` for OPTIMIZATION_TABLES)
