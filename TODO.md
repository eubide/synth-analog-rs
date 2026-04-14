# TODO

Sintetizador analógico tipo Prophet-5 en Rust. Esta lista prioriza el trabajo pendiente por impacto en el sonido y la fidelidad al instrumento original.

## Leyenda

- **P1** — Motor de sonido, impacto alto (hacer primero)
- **P2** — Motor de sonido, impacto medio
- **P3** — Features del Prophet-5 que faltan
- **P4** — MIDI pendiente
- **P5** — GUI / UX
- **P6** — Character analógico adicional
- **P7** — Opcional / avanzado

Cada ítem incluye referencia de archivo y línea cuando aplica. Los detalles técnicos de P1 y P2 están en el **Análisis de sonido 2026-04-14** más abajo.

---

## P1 — Motor de sonido: impacto alto

Los cambios aquí son los que más mueven la percepción hacia "suave y natural". Son la ruta crítica para que el sintetizador deje de sonar digital.

- [x] **PolyBLEP en los 4 osciladores.** Sustituir sawtooth (8 armónicos fijos), square y triangle naive por PolyBLEP con doble transición para PWM. `synthesizer.rs:734-758`. *(ver P1 del análisis)*
- [x] **Envelopes exponenciales reales.** Reescribir los dos ADSR con `coeff = exp(-dt/τ)` y `value = target + (value - target) * coeff`. Elimina clics y el tufo digital. `synthesizer.rs:863-1001`. *(ver P2)*
- [x] **Filtro ZDF ladder bien afinado.** Coeficiente `g = tan(π·fc/fs)`, `tanh` dentro de cada una de las 4 etapas, compensación de graves al subir resonancia, y mover el DC blocker al bus master con `coeff ≈ 0.9999`. `synthesizer.rs:760-850`. *(ver P3)*
- [x] **Fix de detune en cents (logarítmico).** Cambiar `freq * (1 + detune/100)` por `freq * 2f32.powf(detune/1200)`. `synthesizer.rs:608-609`. *(ver P4a)*
- [x] **Keyboard tracking exponencial.** Cambiar a `cutoff *= 2f32.powf((midi_note - 60) / 12 * kbd_track)`. `synthesizer.rs:666-677`. *(ver P4b)*

## P2 — Motor de sonido: impacto medio

- [x] **Gain staging + soft clipper continuo.** Normalizar la suma de voces (`1/√N` o headroom fijo), reemplazar el clipper discontinuo en 0.7 por `tanh(x)` o `x/(1+|x|)` aplicado en todo el rango. `synthesizer.rs:712,722-727`. *(ver P5)*
- [x] **Reverb Freeverb-style.** 8 combs en paralelo con damping LP interno + 4 allpass en serie. `synthesizer.rs:apply_reverb`. *(ver P6)*
- [x] **Carácter analógico por voz.** Fase inicial aleatoria en los osciladores, LFO sub-audio de drift por voz (±1–3 cents), pink noise con PRNG xorshift en lugar de `rand::random` blanco. `synthesizer.rs:302-303,649`. *(ver P7)*
- [x] **Retrigger sin clic.** Retrigger suave desde el valor actual de la envolvente — sin multiplicador. `synthesizer.rs:435-448`. *(ver P8)*
- [x] **Glide / Portamento.** Interpolación exponencial por voz con `glide_time` ajustable. `synthesizer.rs:631-638`.

## P3 — Features del Prophet-5 faltantes

- [x] **Poly-Mod section** — routings completos:
  - [x] Filter Envelope → Oscillator A frequency
  - [x] Filter Envelope → Oscillator A pulse width
  - [x] Oscillator B → Oscillator A frequency (1-sample delay, `poly_mod_osc_b_to_osc_a_freq`)
  - [x] Oscillator B → Oscillator A pulse width (`poly_mod_osc_b_to_osc_a_pw`)
  - [x] Oscillator B → Filter cutoff (`poly_mod_osc_b_to_filter_cutoff`)
- [ ] **Unison mode** (todas las voces apiladas sobre una sola nota con detune)
- [ ] **Modo 5-voice auténtico** como opción (actualmente 8)
- [ ] **Vintage voice allocation modes:**
  - [ ] Last-note priority
  - [ ] Low-note priority
  - [ ] High-note priority
- [ ] **LFO delay / fade-in**

## P4 — MIDI pendiente

- [x] **Pitch bend** (status byte `0xE0`). `midi_handler.rs` actualiza `params.pitch_bend` en el triple buffer; `synthesizer.rs` precomputa `2f32.powf(bend * range / 12)` por bloque y lo multiplica a `freq1`/`freq2`.
- [x] **Aftertouch** (status byte `0xD0`). `midi_handler.rs` actualiza `params.aftertouch`; `synthesizer.rs` lo aplica como modulación aditiva al cutoff (×4 kHz máx) y multiplicativa a la amplitud. Amounts `aftertouch_to_cutoff` y `aftertouch_to_amplitude` configurables vía `SynthParameters`.
- [x] **Program Change** (status byte `0xC0`). Push a `MidiEvent::ProgramChange`; el hilo de audio llama a `synthesizer.load_preset()` con el preset en la posición `program % len` del listado ordenado.
- [x] **Expression pedal** (CC 11). Añadido `expression: f32` a `SynthParameters`; se multiplica sobre `master_volume` en el loop de audio. CC 11 lo actualiza en el triple buffer.
- [ ] **MIDI SysEx** para patch dump/load

## P5 — GUI / UX

- [ ] **Mapeo logarítmico del knob de filter cutoff** (percepción natural, independiente de los fixes DSP de P1)
- [ ] **Keyboard velocity curves humanizadas.** Hoy la velocidad es lineal; añadir curvas (soft/linear/hard) configurables.
- [ ] **Patch A/B comparison**
- [ ] **Preset browser con categorías.** Añadir metadato de categoría a los presets (Bass / Lead / Pad / Brass / FX) y agrupar en la UI.

## P6 — Character analógico adicional

Más allá del drift / fase aleatoria / pink noise de P2, modelado de idiosincrasias analógicas:

- [ ] **Component tolerance variations** — pequeñas variaciones por voz en la respuesta del filtro y los envelopes
- [ ] **VCA bleed-through** — ligera fuga del oscilador cuando el VCA está cerrado
- [ ] **Analog noise floor** — ruido de fondo muy bajo tipo "hiss" de circuito
- [ ] **Filter temperature drift** — drift lento del cutoff por "calentamiento" simulado

## P7 — Opcional / avanzado

- [ ] **Oversampling 2×/4×** (baja prioridad una vez PolyBLEP esté en sitio)
- [ ] **Micro-tuning / alternate tuning tables** (Just Intonation, tunings históricos)
- [ ] **A-440 Hz reference tone generator**
- [ ] **Voice panning / stereo spread.** Actualmente el motor es mono y se convierte a multi-channel copiando. Añadir posicionamiento estéreo por voz.

---

## Análisis de sonido 2026-04-14 — detalles técnicos

Este bloque es la referencia técnica para los ítems de P1 y P2. Ordenado por impacto en la percepción de "sonido suave y natural".

### P1 — PolyBLEP en los 4 osciladores (impacto 10/10)
- **Ref:** `synthesizer.rs:734-758` (`generate_oscillator_static`)
- Cuadrado y triángulo son naive (discontinuidad sin band-limit) → aliasing severo sobre C5.
- Sawtooth suma 8 armónicos fijos: en graves (C1 ~32 Hz) suena apagado porque faltan cientos de armónicos disponibles; en agudos sigue aliaseando. Además es ~8× más caro que PolyBLEP.
- **Acción:** sustituir las 4 formas por PolyBLEP (saw, pulse con doble transición para PWM, triángulo por integración del pulse).

### P2 — Envelopes exponenciales reales (impacto 9/10)
- **Ref:** `synthesizer.rs:863-938` (amp) y `synthesizer.rs:941-1001` (filter)
- Attack y decay son rampas lineales → carácter digital inmediato. Prophet-5 usa curvas RC `1 − exp(−t/τ)`.
- El "release exponencial" actual `value *= (1 - release_rate*dt).max(0)` no es una exponencial verdadera: con `release` corto se convierte en corte abrupto (clic) y no respeta el tiempo de caída teórico.
- **Acción:** reescribir los 3 estados con `coeff = exp(-dt/τ)` y `value = target + (value - target) * coeff`.

### P3 — Filtro ladder mal afinado (impacto 9/10)
- **Ref:** `synthesizer.rs:760-850` (`apply_ladder_filter_static`)
- **Coeficiente sin warping:** usa `f = 2·fc/fs`. Debería ser `g = tan(π·fc/fs)` (ZDF/TPT) o mínimo `g = 1 - exp(-2π·fc/fs)`. Sin warping, el corte se desafina en la banda alta.
- **Topología sospechosa:** la forma con `state.delayN = saturated - state.stageN` no corresponde a un TPT correcto — el filtro está fuera del tono teórico.
- **Saturación mal colocada:** aplica `tanh` solo antes de la etapa 1. El Huovilainen real mete `tanh` dentro de cada una de las 4 etapas, de ahí el "calor" característico.
- **Compensación de resonancia arbitraria:** `1 + res*0.1` no compensa la pérdida real de graves al subir la resonancia (passband gain compensation).
- **DC blocker mal ubicado y mal sintonizado:** `synthesizer.rs:818` usa `dc_block_coeff = 0.995` que en 44.1k da un HP a ~35 Hz, no "1.6 Hz" como dice el comentario → los bajos se adelgazan. Moverlo al bus master con coeficiente ~0.9999 o eliminarlo por voz.

### P4 — Bugs de afinación (impacto 8/10)
- **P4a — Detune en "cents" es lineal, no logarítmico.** `synthesizer.rs:608-609`:
  ```rust
  let mut freq1 = voice.frequency * (1.0 + osc1_detune / 100.0);
  ```
  +50 "cents" acaba siendo ×1.5 (una quinta justa) en vez de medio semitono. Fórmula correcta: `freq * 2f32.powf(detune_cents / 1200.0)`. Revisar rango de GUI y presets que hayan guardado valores grandes confiando en el comportamiento actual.
- **P4b — Keyboard tracking no es exponencial.** `synthesizer.rs:666-677`:
  ```rust
  kbd_track = kbd * ((note_freq / 261.63) - 1.0);
  modulated_cutoff += filter_cutoff * kbd_track;
  ```
  No sigue octavas. Debería ser `cutoff *= 2f32.powf((midi_note - 60) / 12.0 * kbd_track)`.

### P5 — Gain staging y soft clip (impacto 7/10)
- **Ref:** `synthesizer.rs:712` (`*sample += mixed;`) y `synthesizer.rs:722-727` (soft clip)
- Las 8 voces se suman sin normalizar. Un acorde de 4 con ambos osciladores al 100% llega a ~8. Con `master_volume = 0.7` y soft clip disparándose a 0.7 → el synth satura constantemente en acordes.
- Soft clip **discontinuo en 0.7**: la expresión `sign * (1 - exp(-abs*3))` no empata con `x` en la transición → kink audible.
- **Acción:** headroom global + clipper continuo (`tanh`, `x/(1+|x|)` o curva hermite) en todo el rango. Opcional: compresor RMS muy suave al final para "pegamento" analógico en acordes.

### P6 — Reverb metálico (impacto 6/10)
- **Ref:** `synthesizer.rs:1118-1137` (`apply_reverb`)
- 4 combs paralelos sin difusión → timbre resonante/vidrioso. Los tamaños 25/41/59/73 ms están muy cercanos y producen flutter.
- **Acción:** migrar a Freeverb-style (8 combs con damping LP interno + 4 allpass en serie).

### P7 — Vida analógica (impacto 6/10)
- **Fase inicial siempre en 0** al crear voz (`synthesizer.rs:302-303`). Las voces son fase-coherentes → cancelaciones/phasiness en acordes. Inicializar `phase1_accumulator` y `phase2_accumulator` a valores aleatorios.
- **Drift por voz:** cada VCO del Prophet-5 deriva ±3 cents lentamente. Añadir un LFO sub-audio pseudoaleatorio por voz con amplitud 1–3 cents.
- **Ruido blanco vs pink:** `rand::random` por muestra en `synthesizer.rs:649`. Prophet-5 usa ruido más cercano a pink (pasa por filtro analógico). Añadir pink noise + PRNG xorshift.

### P8 — Retrigger con clic (impacto 5/10)
- **Ref:** `synthesizer.rs:420-427`
  ```rust
  voice.envelope_value *= 0.5;
  voice.envelope_state = EnvelopeState::Attack;
  ```
- Saltar la envolvente a la mitad y reiniciar el attack produce un zip audible. Opciones: rampa corta ~5–10 ms hacia el nuevo valor, o modo legato real que mantiene envelope y solo cambia pitch.

### Orden de trabajo recomendado

Impacto descendente con riesgo ascendente:

1. **P1 PolyBLEP** — el cambio más audible de todos.
2. **P2 envelopes exponenciales** — elimina el tufo digital, trivial.
3. **P4 fix de detune + keyboard tracking** — corrige tuning, trivial.
4. **P3 filtro ZDF con tanh por etapa + DC blocker al master** — calor Moog real.
5. **P5 gain staging + soft clipper continuo** — acordes limpios.
6. **P7 drift + fase aleatoria + pink noise** — movimiento analógico.
7. **P6 reverb Freeverb-style** — solo si se usa reverb en presets.
8. **P8 retrigger legato + glide** — expresividad.

Los puntos **1–3** cubren probablemente el 70% del camino hacia "suave y natural".

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
- [x] Effects: reverb y delay básicos (bonus, no en el Prophet-5 original)
- [x] **PolyBLEP + PolyBLAMP** en los 4 osciladores — elimina aliasing en sawtooth, square/PWM y triangle
- [x] **Envelopes exponenciales reales** — ambos ADSR (amp y filter) con curvas RC `exp(-dt/τ)` en attack, decay y release; elimina el tufo digital y los clics de retrigger
- [x] **Detune en cents logarítmico** — `freq * 2^(detune/1200)` en ambos osciladores
- [x] **Keyboard tracking exponencial** — `cutoff * 2^((note-60)/12 * amount)` para seguimiento de octavas correcto
- [x] **Filtro ZDF ladder bien afinado** — TPT con `g = tan(π·fc/fs)`, `tanh` en las 4 etapas, compensación de passband, DC blocker maestro `coeff=0.9999`
- [x] **Gain staging + soft clipper continuo** — normalización `1/√N` por voces activas; soft clipper `tanh` ya en sitio
- [x] **Glide / Portamento** — interpolación exponencial por voz con `glide_time` ajustable (`synthesizer.rs:631-638`)
- [x] **Poly-Mod Filter Envelope → Osc A freq/PW** — implementado en el audio loop (`synthesizer.rs:643-652`)

### Estabilidad y rendimiento en audio thread
- [x] Threading lock-free real con `TripleBuffer` de atomics (`lock_free.rs:7-56`)
- [x] Buffer mono pre-alocado y redimensionado dinámicamente en el callback de audio (`audio_engine.rs:81,110-111`)
- [x] Sample rate leído del dispositivo y pasado al synth — ya no está hardcoded a 44.1 kHz (`audio_engine.rs:24,78`)
- [x] Phase drift corregido con acumuladores enteros de 32-bit fractional
- [x] Filter clamping seguro para evitar runaway
- [x] DC blocker maestro en bus de salida (`coeff=0.9999` → ~0.7 Hz HP)
- [x] Limiter de seguridad en audio thread (`audio_engine.rs:143-150`)
- [x] Error handling robusto sin `unwrap()` en audio thread

### MIDI
- [x] Note on/off
- [x] CC mapping completo (CC 1-54) para parámetros de synth
- [x] Sustain pedal (CC 64)
- [x] Modulation wheel (CC 1)
- [x] Auto-conexión al primer MIDI input disponible

### Presets
- [x] Save/load system con formato propio
- [x] 26 presets clásicos (Moog Bass, Warm Pad, Brass Stab, Sax Lead, etc.)

### GUI
- [x] Layout vintage analógico con egui
- [x] Oscilloscope/waveform display
- [x] MIDI activity indicators
