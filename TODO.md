# TODO

## Documentation
- [x] Create comprehensive README.md with:
  - [x] Build and run instructions
  - [x] System requirements (audio, MIDI setup)
  - [x] Keyboard controls and shortcuts
  - [x] Preset management guide
  - [x] Architecture overview

## Prophet-5 Missing Features

### Filter Section
- [x] Implement 4-pole (24dB/octave) ladder filter (currently using 2-pole 12dB/octave biquad)
- [x] Add filter self-oscillation capability at high resonance
- [x] Implement proper filter saturation/drive modeling

### LFO Section  
- [x] Add multiple LFO waveforms (currently only sine):
  - [x] Triangle
  - [x] Sawtooth (ramp up/down)  
  - [x] Square
  - [x] Sample & Hold (random)
- [x] Add LFO sync to keyboard trigger option
- [ ] Implement LFO delay/fade-in

### Modulation Features
- [ ] Implement Poly-Mod section with Prophet-5 specific routings:
  - [ ] Filter Envelope → Oscillator A frequency
  - [ ] Filter Envelope → Oscillator A pulse width
  - [ ] Oscillator B → Oscillator A frequency
  - [ ] Oscillator B → Oscillator A pulse width
  - [ ] Oscillator B → Filter cutoff
- [ ] Add Glide/Portamento with adjustable time
- [ ] Implement Unison mode (all voices stacked on single note with detune)

### MIDI Implementation
- [x] Basic MIDI note on/off support (already implemented)
- [ ] MIDI CC support for:
  - [ ] Pitch bend (CC 0)
  - [ ] Modulation wheel (CC 1)
  - [ ] Expression pedal (CC 11)
  - [ ] Sustain pedal (CC 64)
  - [ ] All parameter controls via MIDI CC mapping
- [ ] MIDI Program Change for preset selection
- [ ] MIDI SysEx for patch dump/load

### Voice Architecture
- [ ] Reduce to authentic 5-voice polyphony mode (currently 8 voices)
- [ ] Add voice panning/spread (Prophet-5 Rev3/Rev4 feature)
- [ ] Implement vintage voice allocation modes:
  - [ ] Last-note priority
  - [ ] Low-note priority  
  - [ ] High-note priority
- [ ] Add analog voice detuning/drift simulation

### Analog Modeling
- [ ] Add vintage character options:
  - [ ] Oscillator drift/instability
  - [ ] Filter temperature drift
  - [ ] Component tolerance variations
  - [ ] VCA bleed-through
  - [ ] Analog noise floor
- [ ] Implement vintage vs modern mode toggle

### Performance Features
- [ ] Add keyboard velocity curves
- [ ] Implement aftertouch support
- [ ] Add micro-tuning/alternate tuning tables
- [ ] Implement A-440 Hz reference tone generator

### UI Enhancements
- [ ] Create authentic Prophet-5 style GUI layout
- [ ] Add oscilloscope/waveform display
- [ ] Implement patch comparison (A/B)
- [ ] Add MIDI activity indicators
- [ ] Create preset browser with categories

## Completed
- [x] Basic MIDI input support
- [x] Dual oscillators with sync
- [x] Basic filter with envelope
- [x] ADSR envelopes (amp and filter)
- [x] LFO with basic routing
- [x] Preset save/load system
- [x] Effects (reverb, delay) - bonus features not in original