# Sintetizador Analógico Vintage (estilo Prophet-5)

Software de síntesis analógica inspirado en los sintetizadores clásicos de los años 70, construido en Rust con procesamiento de audio en tiempo real y soporte MIDI.

## Features

### Vintage Analog Sound Engine
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
- **Classic Analog GUI** with proper visual hierarchy and organization
- **Real-time Waveform Display** for visual feedback
- **MIDI Activity Indicators** for connection status

## Installation & Requirements

### System Requirements
- **Operating System**: macOS, Linux, or Windows
- **Audio**: CoreAudio (macOS), ALSA/PulseAudio (Linux), WASAPI (Windows)
- **MIDI**: Any MIDI controller (optional)
- **Rust**: Version 1.70+ required for building

### Building from Source

```bash
# Clonar el repositorio
git clone <repo-url>
cd synth-analog-rs

# Compilar en modo optimizado (recomendado para audio)
cargo build --release

# Ejecutar el sintetizador
cargo run --release
```

## Usage

### Getting Started

1. **Launch** the synthesizer with `cargo run --release`
2. **Play** using your computer keyboard or connect a MIDI controller
3. **Explore** the built-in presets by loading them from the GUI
4. **Experiment** with the filter resonance - set it above 3.8 for self-oscillation!

### Keyboard Controls (Computer Keyboard)

The synthesizer maps your computer keyboard to musical notes. See [MANUAL.md](MANUAL.md) for the full keyboard layout and octave controls.

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

#### LFO & Modulation (Classic Analog Style)
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
2. Select from 20 built-in classic sounds across 5 categories: Bass, Lead, Pad, Brass, FX

#### Saving Custom Presets
1. Adjust all parameters to your liking
2. Enter a name in the "New Preset" field
3. Click "Save Preset"
4. Your preset will be saved to the `presets/` directory

See [MANUAL.md](MANUAL.md) for the full preset list and detailed usage instructions.

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
See [ARQUITECTURA.md](ARQUITECTURA.md) for detailed technical documentation and the full signal chain.

## Known Limitations

1. **Sample Rate**: Fixed at 44.1kHz (typical for most audio interfaces)
2. **Polyphony**: Limited to 8 voices (authentic to many vintage synthesizers)
3. **Output**: Mono output duplicated to stereo channels
4. **Platform**: Requires Rust build environment

## Documentation

| Documento | Descripción |
|-----------|-------------|
| [MANUAL.md](MANUAL.md) | Guía de usuario completa: controles, MIDI CC, presets, técnicas |
| [ARQUITECTURA.md](ARQUITECTURA.md) | Referencia técnica: hilos, cadena de señal DSP, filtro Moog, modulación |
| [TEORIA.md](TEORIA.md) | Guía educativa: de la física del sonido al código Rust |
| [TODO.md](TODO.md) | Trabajo pendiente por prioridad |

## Contributing

Contributions are welcome! See [TODO.md](TODO.md) for planned features and priorities.

## License

This project is open source and available under the MIT License.

## Acknowledgments

- **Classic 1970s Analog Synthesizers**: The legendary instruments that inspired this project
- **Antti Huovilainen**: For the improved Moog ladder filter model
- **CPAL Community**: For the excellent cross-platform audio library
- **egui**: For the immediate mode GUI framework