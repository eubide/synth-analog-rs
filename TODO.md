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

- [x] **CC 1 mapeado a `osc1_level` en lugar de mod wheel.** Corregido: CC 1 → `mod_wheel`, escala profundidad LFO a todos los destinos activos.
- [x] **`lfo_target_*` booleans son dead code.** Corregido: los booleans ahora gatean las rutas de modulación LFO en `process_block`.
- [x] **2 de 5 routings de Poly Mod no tienen slider en la GUI.** Corregido: añadidos sliders para `poly_mod_osc_b_to_osc_a_pw` y `poly_mod_osc_b_to_filter_cutoff`.

## P3 — Features del Prophet-5 faltantes

- [x] **Sustain pedal.** Implementado: `is_sustained` por voz, `sustain_held` en Synthesizer, manejo real en `audio_engine.rs`.
- [x] **Modo monofónico + legato.** Implementado: `VoiceMode::Mono/Legato`, note stack, prioridad configurable (Last/Low/High). En legato, no retriggeriza envelopes.
- [x] **Unison mode** — implementado: todas las voces apiladas con detune spread configurable en cents.
- [x] **Modo 5-voice auténtico** — implementado: `max_voices` configurable 1–8 vía GUI y CC.
- [x] **Vintage voice allocation modes:**
  - [x] Last-note priority
  - [x] Low-note priority
  - [x] High-note priority
- [x] **LFO delay / fade-in** — implementado: ramp per-voz desde 0 hasta plena profundidad en `lfo_delay` segundos.

## P4 — MIDI pendiente

- [x] **Mod wheel routing real (CC 1).** Implementado: `mod_wheel` param, CC 1 → `mod_wheel`, escala LFO a todos los destinos activos (0=unchanged, 1=double depth).
- [x] **MIDI clock sync para el arpeggiador.** Implementado: eventos `MidiClock/Start/Continue/Stop` (0xF8/FA/FB/FC), 24ppq → BPM, toggle `arp_sync_to_midi` en GUI y ARP panel.
- [x] **MIDI SysEx** para patch dump/load. Implementado: F0 7D 01 F7 = dump (save), F0 7D 02 [json] F7 = load.

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
- [x] **Sustain pedal (CC 64)** — `is_sustained` por voz, `sustain_held` en Synthesizer; release real al soltar
- [x] Auto-conexión al primer MIDI input disponible
- [x] **Pitch bend** (0xE0) — ±`pitch_bend_range` semitones, ratio precomputado por bloque
- [x] **Aftertouch** (0xD0) — modulación aditiva al cutoff (×4 kHz máx) y multiplicativa a amplitud
- [x] **Program Change** (0xC0) — carga preset por índice `program % len`, lista cacheada al inicio
- [x] **Expression pedal** (CC 11) — escala `master_volume` en el bus maestro
- [x] **Mod wheel (CC 1)** — `mod_wheel` param escala profundidad LFO a destinos activos
- [x] **MIDI clock sync** (0xF8/FA/FB/FC) — 24ppq → BPM, toggle `arp_sync_to_midi`
- [x] **MIDI SysEx** — F0 7D 01 F7 dump / F0 7D 02 [json] F7 load

### Presets
- [x] Save/load system con formato propio
- [x] 26 presets clásicos (Moog Bass, Warm Pad, Brass Stab, Sax Lead, etc.)
- [x] `load_preset_from_json` — carga desde memoria (sin I/O), usado por SysEx

### Voces y modulación
- [x] **Voice modes** — Poly / Mono / Legato / Unison con note stack
- [x] **Note priority** — Last / Low / High (para Mono/Legato)
- [x] **Unison spread** — detune escalonado configurable en cents
- [x] **Max voices** — configurable 1–8 (opción 5-voice auténtico)
- [x] **LFO delay / fade-in** — ramp per-voz, lfo_delay_elapsed reset en retrigger
- [x] **lfo_target booleans** — now gate LFO routing in process_block

### GUI
- [x] Layout vintage analógico con egui
- [x] Oscilloscope/waveform display
- [x] MIDI activity indicators
- [x] **Poly Mod panel** — 5 sliders completos (todos los routings visibles)
- [x] **LFO panel** — delay slider + target toggles checkbox por destino
- [x] **Voice Mode panel** — ComboBox mode, note priority, unison spread, max voices
- [x] **Arpeggiator panel** — toggle MIDI clock sync
