# TODO

Sintetizador analógico tipo Prophet-5 en Rust. Esta lista prioriza el trabajo pendiente por impacto en el sonido y la fidelidad al instrumento original.

## Leyenda

- **P1** — GUI / UX
- **P2** — Audio quality / ergonomía
- **P3** — Character analógico adicional
- **P4** — Opcional / avanzado

---

## P1 — GUI / UX

- [ ] **Controles de aftertouch en GUI** — sliders para `aftertouch_to_cutoff` y `aftertouch_to_amplitude` (parámetros existen en `SynthParameters` pero no tienen UI).
- [ ] **Pitch bend range configurable en GUI** — actualmente hardcodeado a 2 semitones en el default; debería ser ajustable por preset (típicamente 2 o 12).
- [ ] **VU meter / indicador de clipping** — feedback visual del nivel de salida; alerta si el soft limiter está trabajando continuamente.
- [ ] **Keyboard velocity curves humanizadas** — curvas soft/linear/hard configurables (hoy es lineal).
- [ ] **Preset browser con categorías** — metadato de categoría en presets (Bass / Lead / Pad / Brass / FX) y agrupación en la UI.
- [ ] **Randomización de preset** — botón "Random patch" para exploración de sonido; randomizar dentro de rangos razonables (evitar cutoff a 20 Hz o attack a 5 s).
- [ ] **Patch A/B comparison**

## P2 — Audio quality / ergonomía

- [ ] **Parameter smoothing (anti-zipper noise)** — cambios abruptos de `filter_cutoff`, `master_volume`, etc. vía CC producen zipper audible porque el cambio se aplica de bloque en bloque sin rampa. Añadir un smoother de 1-pole por parámetro crítico (cutoff, resonance, volume) en el audio loop.
- [ ] **MIDI learn mode** — permitir al usuario asignar cualquier CC a cualquier parámetro en lugar de depender del mapa hardcodeado de `midi_handler.rs`.

## P3 — Effects y character analógico

- [ ] **Chorus / ensemble** — modulación de pitch+delay muy corto (≈5–25 ms, depth ≈0.3–2 ms, rate ≈0.1–3 Hz). Quintaesencial en el sonido Prophet-5 de estudio.
- [ ] **Component tolerance variations** — pequeñas variaciones por voz en la respuesta del filtro y los envelopes.
- [ ] **VCA bleed-through** — ligera fuga del oscilador cuando el VCA está cerrado.
- [ ] **Analog noise floor** — ruido de fondo muy bajo tipo "hiss" de circuito.
- [ ] **Filter temperature drift** — drift lento del cutoff por "calentamiento" simulado.

## P4 — Opcional / avanzado

- [ ] **Plugin format (CLAP / VST3)** — para usar el sintetizador como instrumento virtual en un DAW.
- [ ] **Voice panning / stereo spread** — el motor es mono; añadir posicionamiento estéreo por voz.
- [ ] **Oversampling 2×/4×** (baja prioridad una vez PolyBLEP está en sitio).
- [ ] **Micro-tuning / alternate tuning tables** (Just Intonation, tunings históricos).
- [ ] **A-440 Hz reference tone generator**
