# Rust Prophet-5 Synthesizer

A high-fidelity software synthesizer inspired by the classic Sequential Circuits Prophet-5, built in Rust with real-time audio processing and MIDI support.

## Features

### Prophet-5 Inspired Sound Engine
- **Dual Oscillators** with sync, detune, and classic waveforms (Saw, Square, Triangle, Sine)
- **Authentic 24dB/octave Ladder Filter** with self-oscillation capability
- **Dual ADSR Envelopes** for amplitude and filter modulation
- **Advanced LFO** with 5 waveforms (Triangle, Square, Sawtooth, Reverse Saw, Sample & Hold)
- **LFO Keyboard Sync** for rhythmic modulation effects
- **8-Voice Polyphony** with intelligent voice stealing
- **Effects Section** with reverb and delay

### Advanced Features
- **Real-time MIDI Input** support for external controllers
- **Arpeggiator** with multiple patterns (Up, Down, Up-Down, Random)
- **Preset System** with 20 built-in classic synthesizer sounds
- **Modulation Matrix** for complex sound design
- **Self-Oscillating Filter** at high resonance settings

## Installation & Requirements

### System Requirements
- **Operating System**: macOS, Linux, or Windows
- **Audio**: CoreAudio (macOS), ALSA/PulseAudio (Linux), WASAPI (Windows)
- **MIDI**: Any MIDI controller (optional)
- **Rust**: Version 1.70+ required for building

### Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/synth.git
cd synth

# Build in release mode for optimal audio performance
cargo build --release

# Run the synthesizer
cargo run --release
```

## Usage

### Getting Started

1. **Launch** the synthesizer with `cargo run --release`
2. **Play** using your computer keyboard or connect a MIDI controller
3. **Explore** the built-in presets by loading them from the GUI
4. **Experiment** with the filter resonance - set it above 3.8 for self-oscillation!

### Keyboard Controls (Computer Keyboard)

The synthesizer maps your computer keyboard to musical notes:

```
Musical Notes (C4 octave):
  A  S  D  F  G  H  J  K  L
 C  C# D  D# E  F  F# G  G# A

Octave Controls:
- Z/X: Change octave down/up
```

### GUI Controls

#### Oscillator Section
- **Waveform**: Choose between Sawtooth, Square, Triangle, and Sine waves
- **Detune**: Fine-tune oscillator frequency (-12 to +12 semitones)
- **Pulse Width**: Adjust square wave pulse width (Square waves only)
- **Sync**: Enable oscillator sync for harmonically rich sounds

#### Filter Section (24dB Ladder)
- **Cutoff**: Filter frequency (20Hz to 20kHz)
- **Resonance**: Filter emphasis (0.0 to 4.0, self-oscillates at 3.8+)
- **Envelope Amount**: Filter envelope modulation depth
- **Keyboard Tracking**: How much filter tracks keyboard pitch

#### Envelopes
- **Attack**: Time to reach peak level
- **Decay**: Time to fall to sustain level
- **Sustain**: Hold level while key is pressed
- **Release**: Time to fade to silence after key release

#### LFO & Modulation (Prophet-5 Style)
- **Waveform**: Choose from Triangle, Square, Sawtooth, Reverse Sawtooth, or Sample & Hold
- **Rate**: LFO frequency (0.05 to 30 Hz, logarithmic)
- **Amount**: Global LFO modulation depth
- **Keyboard Sync**: Reset LFO phase on every note trigger
- **Destinations**: Route LFO to filter cutoff, resonance, oscillator pitch, amplitude

### MIDI Setup

The synthesizer automatically detects and connects to the first available MIDI input device. Supported MIDI messages:

- **Note On/Off**: Play notes with velocity
- **Sustain Pedal**: Hold notes (CC 64)
- **Modulation Wheel**: Additional modulation (CC 1)

### Preset Management

#### Loading Presets
1. Use the preset dropdown in the GUI
2. Select from 20 built-in classic sounds:
   - **Bass**: Moog Bass, Acid Bass, Sub Bass, Wobble Bass
   - **Leads**: Supersaw Lead, Pluck Lead, Screaming Lead, Vintage Lead
   - **Pads**: Warm Pad, String Ensemble, Choir Pad, Glass Pad
   - **Brass**: Brass Stab, Trumpet Lead, Sax Lead, Flute
   - **FX**: Arp Sequence, Sweep FX, Noise Sweep, Zap Sound

#### Saving Custom Presets
1. Adjust all parameters to your liking
2. Enter a name in the "New Preset" field
3. Click "Save Preset"
4. Your preset will be saved to the `presets/` directory

## Technical Architecture

### Audio Engine
- **Sample Rate**: 44.1kHz fixed
- **Buffer Size**: Optimized for low-latency real-time processing
- **Audio Backend**: CPAL (Cross-Platform Audio Library)
- **Threading**: Lock-free audio processing with Arc<Mutex> for parameter updates

### Filter Implementation
The ladder filter is based on the Huovilainen improved Moog model:
- 4 cascaded one-pole sections for 24dB/octave rolloff
- Zero-delay feedback for accurate resonance behavior
- Built-in saturation modeling for analog warmth
- Self-oscillation at high resonance settings

### Voice Architecture
Each voice maintains independent state for:
- Dual oscillator phases with sync capability
- Amplitude and filter envelope generators
- Ladder filter state (4 stages + feedback)
- Velocity and modulation routing

## Development

### Code Quality
```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run with warnings as errors
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

### Architecture
See [CLAUDE.md](CLAUDE.md) for detailed technical documentation and architecture overview.

## Known Limitations

1. **Sample Rate**: Fixed at 44.1kHz (typical for most audio interfaces)
2. **Polyphony**: Limited to 8 voices (authentic to many vintage synthesizers)
3. **Output**: Mono output duplicated to stereo channels
4. **Platform**: Requires Rust build environment

## Contributing

Contributions are welcome! Please see the [TODO.md](TODO.md) file for planned features and improvements.

### Priority Features to Implement
1. Multiple LFO waveforms (Triangle, Sawtooth, Square, Sample & Hold)
2. Poly-Mod section with authentic Prophet-5 routing
3. Glide/Portamento with adjustable time
4. Extended MIDI CC support for all parameters
5. Unison mode for thick lead sounds

## License

This project is open source and available under the MIT License.

## Acknowledgments

- **Sequential Circuits Prophet-5**: The legendary synthesizer that inspired this project
- **Antti Huovilainen**: For the improved Moog ladder filter model
- **CPAL Community**: For the excellent cross-platform audio library
- **egui**: For the immediate mode GUI framework