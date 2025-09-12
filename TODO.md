# TODO

## Documentation
- [x] Create comprehensive README.md with:
  - [x] Build and run instructions
  - [x] System requirements (audio, MIDI setup)
  - [x] Keyboard controls and shortcuts
  - [x] Preset management guide
  - [x] Architecture overview
- [x] Create CLAUDE.md for Claude Code assistance with:
  - [x] Development commands and workflow
  - [x] Prophet-5 architecture overview
  - [x] Key components and their interactions
  - [x] Performance considerations for real-time audio

## Vintage Analog Features

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
- [ ] Implement Poly-Mod section with vintage analog routings:
  - [ ] Filter Envelope → Oscillator A frequency
  - [ ] Filter Envelope → Oscillator A pulse width
  - [ ] Oscillator B → Oscillator A frequency
  - [ ] Oscillator B → Oscillator A pulse width
  - [ ] Oscillator B → Filter cutoff
- [ ] Add Glide/Portamento with adjustable time
- [ ] Implement Unison mode (all voices stacked on single note with detune)

### MIDI Implementation
- [x] Basic MIDI note on/off support (already implemented)
- [x] Comprehensive MIDI CC support for all parameters (CC 1-54 mapped in midi_handler.rs)
- [x] Sustain pedal (CC 64) support
- [x] Modulation wheel (CC 1) support
- [ ] MIDI CC support for remaining controls:
  - [ ] Pitch bend (CC 0)
  - [ ] Expression pedal (CC 11)
- [ ] MIDI Program Change for preset selection
- [ ] MIDI SysEx for patch dump/load

### Voice Architecture
- [ ] Reduce to authentic 5-voice polyphony mode (currently 8 voices)
- [ ] Add voice panning/spread (vintage analog feature)
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
- [x] Create authentic vintage analog GUI layout
- [x] Add oscilloscope/waveform display
- [ ] Implement patch comparison (A/B)
- [x] Add MIDI activity indicators
- [ ] Create preset browser with categories

## Análisis Crítico del Motor de Sonido - Problemas Identificados

### **Problemas Críticos Que Requieren Atención Inmediata**

#### 1. **Problemas Graves de Aliasing y Anti-aliasing**

**Sawtooth Wave (synthesizer.rs:~line 400)**
- **Problema**: Solo usa 8 armónicos fijos → aliasing severo en frecuencias altas
- **Impacto**: Sonido digital áspero, especialmente en notas agudas
- **Ubicación**: `WaveType::Sawtooth` en `generate_oscillator_static()`
- **Solución**: Implementar osciladores BLEP/PolyBLEP o wavetables pre-calculadas

**Square/Triangle Waves**
- **Problema**: Generación naive sin band-limiting → aliasing masivo
- **Impacto**: Artifacts digitales muy audibles
- **Solución**: Band-limited square waves con PolyBLEP

#### 2. **Problemas Críticos de Precisión Numérica**

**Drift de Fase (synthesizer.rs:~line 580)**
- **Problema**: Acumulación de errores de punto flotante en `voice.phase1/phase2`
- **Código problemático**: 
  ```rust
  voice.phase1 = (voice.phase1 + freq1 * dt) % 1.0;
  ```
- **Impacto**: Desintonización gradual, inestabilidad de pitch
- **Solución**: Usar contadores de muestras enteros + conversión a fase

**LFO Phase Drift (synthesizer.rs:~line 555)**
- **Problema**: `self.lfo_phase` vulnerable a drift temporal
- **Impacto**: LFO pierde sincronización con el tiempo
- **Solución**: Reset periódico o contador entero

#### 3. **Threading y Concurrencia - CRÍTICO PARA AUDIO**

**Mutex Contention (audio_engine.rs:56)**
- **Problema**: `synthesizer.lock().unwrap()` en audio thread
- **Impacto**: Audio dropouts, crackling, latencia variable
- **Código problemático**: 
  ```rust
  let mut synth = synthesizer.lock().unwrap();
  ```
- **Solución**: Lock-free communication con ringbuffers/atomics

**GUI Interference (gui.rs:36)**
- **Problema**: GUI locks pueden bloquear audio thread
- **Impacto**: Crackling cuando se mueven controles
- **Solución**: Separar parámetros GUI de audio engine

**Panic Vulnerability**
- **Problema**: `unwrap()` en audio callback puede crashear
- **Impacto**: Crash completo del programa
- **Solución**: Error handling robusto en audio thread

#### 4. **Problemas Graves en el Filtro**

**Inestabilidad del Filtro (synthesizer.rs:~line 680)**
- **Problema**: Coeficientes pueden volverse inestables
- **Código problemático**: `let fc = (cutoff / sample_rate).min(0.49);`
- **Impacto**: Explosión de señal, saturación
- **Solución**: Clamp más estricto, verificación de estabilidad

**Self-Oscillation Peligrosa**
- **Problema**: `let res = resonance.clamp(0.0, 4.0);` permite runaway
- **Impacto**: Volumen extremo, posible daño a altavoces/oídos
- **Solución**: Limiter de seguridad, resonancia máxima más baja

**Falta DC Blocking**
- **Problema**: Acumulación de DC offset en filtro
- **Impacto**: Pop/clicks, saturación gradual
- **Solución**: High-pass DC blocker

#### 5. **Performance - Problemas en Hot Path**

**Allocations en Audio Thread (audio_engine.rs:60)**
- **Problema**: `let mut mono_buffer = vec![0.0f32; frames];`
- **Impacto**: GC pauses, jitter, dropouts
- **Solución**: Pre-allocar buffer reutilizable

**Random Calls Costosas (synthesizer.rs:~line 575)**
- **Problema**: `rand::random::<f32>()` es pesado para noise
- **Impacto**: CPU spikes, dropouts
- **Solución**: PRNG simple (Linear Congruential Generator)

**Envelope Calculations Ineficientes**
- **Problema**: Cálculos repetidos por voice por sample
- **Impacto**: Alto uso de CPU
- **Solución**: Lookup tables o algoritmos optimizados

#### 6. **Problemas de Calidad DSP**

**Envelope Curves Poco Naturales**
- **Problema**: Envelopes lineales suenan digitales
- **Impacto**: Sonido poco orgánico
- **Solución**: Curves exponenciales/logarítmicas

**Filter Frequency Mapping**
- **Problema**: No hay scaling logarítmico perceptual
- **Impacto**: Controles poco intuitivos
- **Solución**: Mapeo exponencial de frecuencias

**Velocity Response Básica**
- **Problema**: Respuesta linear de velocity
- **Impacto**: Expresividad limitada
- **Solución**: Curves de velocity humanizadas

### **Plan de Correcciones Prioritarias**

#### **Prioridad 1 - CRÍTICO (Seguridad/Estabilidad)**
- [x] Implementar limiter/compressor de seguridad
- [ ] Refactorizar threading lock-free (parcialmente completado - ahora usa try_lock())
- [x] Eliminar allocations en audio thread
- [x] Error handling robusto (no unwrap())

#### **Prioridad 2 - ALTA (Calidad de Audio)**
- [ ] Implementar osciladores anti-aliased (PolyBLEP)
- [x] Estabilizar filtro con clamps seguros
- [x] Agregar DC blocking
- [x] Corregir drift de fase en osciladores y LFO (usando acumuladores enteros)
- [ ] Optimizar envelope calculations

#### **Prioridad 3 - MEDIA (Mejoras de Calidad)**
- [ ] Envelope curves exponenciales
- [ ] Filter frequency mapping logarítmico
- [ ] PRNG optimizado para noise
- [ ] Velocity curves humanizadas

#### **Prioridad 4 - BAJA (Features Avanzadas)**
- [ ] Oversampling para calidad premium
- [ ] Procesamiento estéreo verdadero
- [ ] Buffer size adaptive
- [ ] Sample rate adaptive algorithms

## Completed
- [x] Basic MIDI input support
- [x] Dual oscillators with sync
- [x] Basic filter with envelope
- [x] ADSR envelopes (amp and filter)
- [x] LFO with basic routing
- [x] Preset save/load system
- [x] Effects (reverb, delay) - bonus features not in original