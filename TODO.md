# TODO

Sintetizador analógico tipo Prophet-5 en Rust. Trabajo pendiente, priorizado por impacto.

## Refactor arquitectónico

> Análisis completo: `synthesizer.rs` (5077 líneas) es un God Object con 79 campos. Orden sugerido: L1→L2→L6→L4→L3+L5.

### L1 — Extraer `EffectsChain` _(bajo riesgo, primer paso)_ ✅
- [x] Crear struct `EffectsChain` con los 12 campos de reverb/delay/chorus
- [x] Mover `apply_delay`, `apply_reverb`, `apply_chorus` → `impl EffectsChain`
- [x] `Synthesizer`: de 79 → 67 campos

### L2 — Extraer `VoiceManager`
- [ ] Mover campos: `voices`, `held_notes`, `note_stack`, `sustain_held`, `voice_mode`, `note_priority`, `unison_spread`, `max_polyphony`
- [ ] Mover métodos: `note_on`, `note_off`, `trigger_note`, `find_voice_to_steal`
- [ ] `Synthesizer`: de 67 → 55 campos

### L3 — Extraer `LfoModulator`
- [ ] Mover campos LFO + los 5 campos de poly mod matrix
- [ ] Mover `generate_lfo_waveform` y cálculos de modulación cruzada
- [ ] Desacopla modulación de la síntesis core

### L4 — Descomponer `process_block` (398 líneas)
- [ ] Convertir en dispatcher que llama a subsistemas: `voices.update()`, `lfo.update()`, `effects.process()`
- [ ] Requiere L1 + L2 completos primero

### L5 — Refactor GUI (`gui.rs`, 1649 líneas)
- [ ] Extraer `KeyboardInput` — 63 líneas de lógica MIDI dentro de `eframe::App::update`
- [ ] Extraer `PresetManager` — `draw_preset_panel` tiene 232 líneas y 5 responsabilidades
- [ ] Builder pattern para los 11 paneles con estructura repetitiva (`draw_xxx_panel`)
- [ ] Mover parámetros A/B comparison fuera de la capa de presentación

### L6 — Abstracción CC (`midi_handler.rs`)
- [ ] Crear `CcMap` — fuente única de verdad para CC→parámetro (elimina switch de 55 líneas)
- [ ] Unificar: `midi_handler.rs`, lista de `midi_learn`, campos de `SynthParameters`
- [ ] Agregar nuevo parámetro editando solo 1 lugar (actualmente 5 archivos)

### Deuda técnica crítica
- [ ] **I/O en hilo de audio** — `save_preset`/`load_preset` llamados desde callback de audio (`audio_engine.rs:144–160`); mover a hilo separado

---

## Opcional / avanzado

- [ ] **Plugin format (CLAP / VST3)** — para usar el sintetizador como instrumento virtual en un DAW (requiere refactorización arquitectónica mayor).
