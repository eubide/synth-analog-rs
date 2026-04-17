# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Prophet-5 style vintage analog synthesizer built in Rust with real-time audio processing and MIDI support. It faithfully emulates the classic Sequential Circuits Prophet-5 from the late 1970s, featuring the characteristic dual oscillator architecture, 24dB Moog ladder filter, dual ADSR envelopes, advanced LFO modulation, and the iconic Prophet-5 sound and workflow.

## Development Commands

### Build and Run
```bash
# Build in release mode for optimal audio performance
cargo build --release

# Run the synthesizer application
cargo run --release

# Build in debug mode (for development only - may have audio latency)
cargo build
cargo run
```

### Code Quality
```bash
# Format code according to Rust standards
cargo fmt

# Run linter with standard checks
cargo clippy

# Run linter with strict warnings as errors
cargo clippy --all-targets --all-features -- -D warnings
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
```

## Architecture Overview

### Core Components

The synthesizer follows a modular architecture with clear separation of concerns:

- **main.rs**: Application entry point, initializes audio engine, MIDI handler, and GUI
- **synthesizer.rs**: Core synthesis engine with voice management, oscillators, filter, envelopes
- **audio_engine.rs**: Real-time audio processing using CPAL (Cross-Platform Audio Library)
- **midi_handler.rs**: MIDI input handling with CC mapping for all synth parameters
- **gui.rs**: Immediate mode GUI using egui, styled as vintage analog synthesizer

### Key Dependencies

- **eframe/egui 0.28**: Modern immediate mode GUI framework
- **cpal 0.15**: Cross-platform audio library for real-time audio
- **midir 0.9**: Cross-platform MIDI I/O library
- **rand 0.8**: Random number generation for noise and sample & hold LFO

### Audio Architecture

- **Sample Rate**: Fixed at 44.1kHz
- **Threading**: Lock-free audio processing with Arc<Mutex> for parameter updates
- **Voice Management**: 8-voice polyphony with intelligent voice stealing
- **Filter**: 24dB/octave Moog ladder filter based on Huovilainen model with self-oscillation

### Prophet-5 Synthesis Components

- **Dual Oscillators (A & B)**: Classic Prophet-5 waveforms (Sawtooth, Square, Triangle, Sine) with oscillator sync
- **24dB Ladder Filter**: Authentic Moog-style 4-pole filter with resonance up to self-oscillation (3.8+)
- **Dual ADSR Envelopes**: Separate envelopes for amplitude and filter modulation (Prophet-5 style)
- **Advanced LFO**: 5 waveforms including Sample & Hold, with keyboard sync capability
- **Mixer Section**: Individual level controls for Osc A, Osc B, and noise generator
- **Effects**: Reverb and delay processing
- **Arpeggiator**: Multiple patterns inspired by classic sequencer synthesizers

### Prophet-5 Style Preset System

- Presets are stored as JSON files in `presets/` directory (55-line line-based format)
- Built-in classic presets recreate iconic Prophet-5 sounds across 7 categories: Bass, Lead, Pad, Strings, Brass, FX, Sequence
- Custom presets can be saved and loaded through the vintage-styled GUI
- 32 built-in presets include authentic Prophet-5 inspired sounds (Lately Bass, Jump Brass, Thriller Sync Lead, Poly Mod Bell, etc.)
- Preset format persists all patch parameters including `lfo.waveform`, `lfo.sync`, chorus, and the 5 Poly Mod routes; legacy 45-line presets load with safe defaults
- `create_all_classic_presets` on startup skips regeneration if built-in files already exist (preserves user edits); GUI "create classic presets" button uses `force_create_all_classic_presets`

### MIDI Implementation

- Auto-connects to first available MIDI input device
- Full parameter control via MIDI CC messages (see `midi_handler.rs:129-199` for CC mappings)
- Support for Note On/Off, Sustain Pedal (CC 64), and Modulation Wheel (CC 1)

## Development Notes

- Always build in release mode when working with audio to avoid latency issues
- The synthesizer uses immediate mode GUI (egui) - UI state is rebuilt each frame
- Audio processing happens in a separate thread with minimal allocations
- MIDI messages are processed asynchronously and stored in a circular buffer
- The filter implementation includes saturation modeling for analog warmth

## File Structure

```
src/
├── main.rs           # Application entry point
├── synthesizer.rs    # Core synthesis engine (large file with voice management)
├── audio_engine.rs   # Real-time audio processing
├── midi_handler.rs   # MIDI input and CC mapping
└── gui.rs           # Vintage-styled GUI implementation
```

## Performance Considerations

- Audio processing must remain real-time - avoid allocations in audio callback
- Use release builds for performance testing
- MIDI CC messages can flood the system - implement throttling if needed
- Filter resonance above 3.8 enters self-oscillation mode