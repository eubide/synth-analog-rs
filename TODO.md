# TODO

Sintetizador analógico tipo Prophet-5 en Rust. Esta lista prioriza el trabajo pendiente por impacto en el sonido y la fidelidad al instrumento original.

## Leyenda

- **P3** — Features del Prophet-5 que faltan
- **P4** — MIDI pendiente
- **P5** — GUI / UX
- **P6** — Character analógico adicional
- **P7** — Opcional / avanzado

---

## Bugs conocidos

- [ ] **CC 1 mapeado a `osc1_level` en lugar de mod wheel.** `midi_handler.rs:195`. CC 1 es el estándar de Modulation Wheel; debería rutear a `lfo_amount` o a la profundidad de modulación del LFO, no al nivel del oscilador.
- [ ] **`lfo_target_*` booleans son dead code.** Los campos `lfo_target_osc1_pitch / osc2_pitch / filter / amplitude` se guardan en presets y en el struct `LFO` pero el audio loop los ignora — solo los amounts `lfo_to_*` controlan el routing. Hay que o bien usarlos como gates booleanos (si el toggle está off, ignorar el amount) o eliminarlos del formato de preset.
- [ ] **2 de 5 routings de Poly Mod no tienen slider en la GUI.** `poly_mod_osc_b_to_osc_a_pw` y `poly_mod_osc_b_to_filter_cutoff` están implementados en el motor de audio pero el panel POLY MOD solo muestra 3 de los 5 parámetros. `gui.rs:569-615`.

## P3 — Features del Prophet-5 faltantes

- [ ] **Sustain pedal.** El evento `MidiEvent::SustainPedal` llega pero se ignora (`audio_engine.rs:103`). Falta: flag de sustain por voz; si el pedal está pulsado, las notas liberadas entran en sustain extendido en lugar de pasar a release.
- [ ] **Modo monofónico + legato.** Voz única con prioridad configurable (last/low/high). En legato, cambiar nota no retriggeriza los envelopes — solo glisa el pitch. Complementa el glide ya existente.
- [ ] **Unison mode** — todas las voces apiladas sobre una sola nota con detune escalonado (spread configurable)
- [ ] **Modo 5-voice auténtico** como opción (actualmente 8)
- [ ] **Vintage voice allocation modes:**
  - [ ] Last-note priority
  - [ ] Low-note priority
  - [ ] High-note priority
- [ ] **LFO delay / fade-in** — el LFO entra suavemente tras un retardo configurable post-note-on

## P4 — MIDI pendiente

- [ ] **Mod wheel routing real (CC 1).** Una vez corregido el bug, mapear CC 1 a `lfo_amount` o crear un parámetro de "mod depth" independiente que escale el LFO hacia los destinos activos.
- [ ] **MIDI clock sync para el arpeggiador.** El arpeggiador usa BPM interno; debería poderse sincronizar a MIDI clock externo (DAW / drum machine).
- [ ] **MIDI SysEx** para patch dump/load

## P5 — GUI / UX

- [ ] **Completar panel Poly Mod** — añadir sliders para `poly_mod_osc_b_to_osc_a_pw` y `poly_mod_osc_b_to_filter_cutoff`. `gui.rs:569-615`.
- [ ] **Controles de aftertouch en GUI** — sliders para `aftertouch_to_cutoff` y `aftertouch_to_amplitude` (los parámetros existen en `SynthParameters` pero no tienen UI).
- [ ] **Pitch bend range configurable en GUI** — actualmente hardcodeado a 2 semitones en el default; debería ser ajustable por preset (típicamente 2 o 12).
- [ ] **Selector de modo de voz** — polyphonic / mono / legato / unison, una vez implementados los modos en P3.
- [ ] **Mapeo logarítmico del knob de filter cutoff** — percepción natural, independiente de los fixes DSP
- [ ] **Keyboard velocity curves humanizadas** — curvas soft/linear/hard configurables (hoy es lineal)
- [ ] **Patch A/B comparison**
- [ ] **Preset browser con categorías** — metadato de categoría en presets (Bass / Lead / Pad / Brass / FX) y agrupación en la UI
- [ ] **Randomización de preset** — botón "Random patch" para exploración de sonido; randomizar dentro de rangos razonables (evitar cutoff a 20 Hz o attack a 5 s)
- [ ] **VU meter / indicador de clipping** — feedback visual del nivel de salida; alerta si el soft limiter está trabajando continuamente

## P5.5 — Audio quality / ergonomía

- [ ] **Parameter smoothing (anti-zipper noise).** Cambios abruptos de `filter_cutoff`, `master_volume`, etc. vía CC producen zipper audible porque el cambio se aplica de bloque en bloque sin rampa. Añadir un smoother de 1-pole por parámetro crítico (cutoff, resonance, volume) en el audio loop.
- [ ] **MIDI learn mode** — permitir al usuario asignar cualquier CC a cualquier parámetro en lugar de depender del mapa hardcodeado de `midi_handler.rs`.

## P6 — Effects

- [ ] **Chorus / ensemble** — efecto de modulación de pitch+delay muy corto (≈5–25 ms, depth ≈0.3–2 ms, rate ≈0.1–3 Hz). Quintaesencial en el sonido Prophet-5 de estudio; muchos de los sonidos icónicos del instrumento usan chorus externo.
- [ ] **Component tolerance variations** — pequeñas variaciones por voz en la respuesta del filtro y los envelopes
- [ ] **VCA bleed-through** — ligera fuga del oscilador cuando el VCA está cerrado
- [ ] **Analog noise floor** — ruido de fondo muy bajo tipo "hiss" de circuito
- [ ] **Filter temperature drift** — drift lento del cutoff por "calentamiento" simulado

## P7 — Opcional / avanzado

- [ ] **Oversampling 2×/4×** (baja prioridad una vez PolyBLEP está en sitio)
- [ ] **Micro-tuning / alternate tuning tables** (Just Intonation, tunings históricos)
- [ ] **A-440 Hz reference tone generator**
- [ ] **Voice panning / stereo spread** — el motor es mono; añadir posicionamiento estéreo por voz
- [ ] **Plugin format (CLAP / VST3)** — para usar el sintetizador como instrumento virtual en un DAW

---

## Completado

### Proyecto y documentación
- [x] README.md completo con build, system requirements, keyboard controls, preset management, architecture overview
- [x] CLAUDE.md con development commands, Prophet-5 architecture, key components, performance notes

### Motor de sonido
- [x] Filtro 4-pole (24dB/octave) ladder con self-oscillation y saturation básica
- [x] Dual oscillators con oscillator sync
- [x] ADSR envelopes separados para amp y filter
- [x] LFO con 5 waveforms (Triangle, Square, Sawtooth, ReverseSawtooth, Sample & Hold) y keyboard sync
- [x] 8-voice polyphony con voice stealing
- [x] Effects: reverb y delay (bonus, no en el Prophet-5 original)
- [x] **PolyBLEP + PolyBLAMP** en los 4 osciladores — elimina aliasing en sawtooth, square/PWM y triangle
- [x] **Envelopes exponenciales reales** — ambos ADSR con curvas RC `exp(-dt/τ)` en attack, decay y release
- [x] **Detune en cents logarítmico** — `freq * 2^(detune/1200)` en ambos osciladores
- [x] **Keyboard tracking exponencial** — `cutoff * 2^((note-60)/12 * amount)`
- [x] **Filtro ZDF ladder bien afinado** — TPT con `g = tan(π·fc/fs)`, `tanh` en las 4 etapas, compensación de passband, DC blocker maestro `coeff=0.9999`
- [x] **Gain staging + soft clipper continuo** — normalización `1/√N` por voces activas
- [x] **Reverb Freeverb-style** — 8 combs paralelos con LP damping interno + 4 allpass en serie (Jezar 1997)
- [x] **Carácter analógico por voz** — fase inicial aleatoria, drift LFO sub-audio por voz (±2.5 cents, 0.05–0.25 Hz), pink noise via xorshift32 + Kellett IIR
- [x] **Retrigger sin clic** — restart suave desde el valor actual de la envolvente
- [x] **Glide / Portamento** — interpolación exponencial por voz con `glide_time` ajustable
- [x] **Poly-Mod section** — routings completos:
  - [x] Filter Envelope → Oscillator A frequency (±24 semitones)
  - [x] Filter Envelope → Oscillator A pulse width
  - [x] Oscillator B → Oscillator A frequency (1-sample delay, `poly_mod_osc_b_to_osc_a_freq`)
  - [x] Oscillator B → Oscillator A pulse width (`poly_mod_osc_b_to_osc_a_pw`)
  - [x] Oscillator B → Filter cutoff (`poly_mod_osc_b_to_filter_cutoff`)

### Estabilidad y rendimiento en audio thread
- [x] Threading lock-free real con `TripleBuffer` de atomics (`lock_free.rs:7-56`)
- [x] Buffer mono pre-alocado y redimensionado dinámicamente en el callback de audio
- [x] Sample rate leído del dispositivo y pasado al synth — ya no está hardcodeado a 44.1 kHz
- [x] Phase drift corregido con acumuladores enteros de 32-bit fractional
- [x] Filter clamping seguro para evitar runaway
- [x] DC blocker maestro en bus de salida (`coeff=0.9999` → ~0.7 Hz HP)
- [x] Limiter de seguridad en audio thread
- [x] Error handling robusto sin `unwrap()` en audio thread
- [x] Glide coefficient `exp()` precomputado por bloque (fuera del bucle per-sample)

### MIDI
- [x] Note on/off
- [x] CC mapping completo (CC 1-54) para parámetros de synth
- [x] Sustain pedal (CC 64) — evento recibido y parseado
- [x] Auto-conexión al primer MIDI input disponible
- [x] **Pitch bend** (0xE0) — ±`pitch_bend_range` semitones, ratio precomputado por bloque
- [x] **Aftertouch** (0xD0) — modulación aditiva al cutoff (×4 kHz máx) y multiplicativa a amplitud
- [x] **Program Change** (0xC0) — carga preset por índice `program % len`, lista cacheada al inicio
- [x] **Expression pedal** (CC 11) — escala `master_volume` en el bus maestro

### Presets
- [x] Save/load system con formato propio
- [x] 26 presets clásicos (Moog Bass, Warm Pad, Brass Stab, Sax Lead, etc.)

### GUI
- [x] Layout vintage analógico con egui
- [x] Oscilloscope/waveform display
- [x] MIDI activity indicators
