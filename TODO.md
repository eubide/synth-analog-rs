# TODO

Sintetizador analógico tipo Prophet-5 en Rust. Trabajo pendiente, priorizado por impacto.

## Refactor arquitectónico

> Análisis completo: `synthesizer.rs` (5077 líneas) era un God Object con 79 campos. L1–L4 + L6 completos; `Synthesizer` queda con 63 campos y un `process_block` de ~130 líneas. Pendiente: L5 (GUI) — decisión de diseño tomada (Opción B, ver sección L5).

### L1 — Extraer `EffectsChain` _(bajo riesgo, primer paso)_ ✅
- [x] Crear struct `EffectsChain` con los 12 campos de reverb/delay/chorus
- [x] Mover `apply_delay`, `apply_reverb`, `apply_chorus` → `impl EffectsChain`
- [x] `Synthesizer`: de 79 → 67 campos

### L2 — Extraer `VoiceManager` ✅
- [x] Mover campos: `voices`, `held_notes`, `note_stack`, `sustain_held`, `voice_mode`, `note_priority`, `unison_spread`, `max_polyphony`
- [x] Mover métodos puros: `find_voice_to_steal`, `select_mono_note`, `all_notes_off`, `release_sustained`
- [ ] ~~`note_on`, `note_off`, `trigger_note` quedan en Synthesizer~~ — orquestan LFO sync, arpeggiator, tuning_mode; firma de 5+ args si se mueven. Reconsiderar tras L4.
- [x] `Synthesizer`: 71 → 63 campos (post-L3 acumulado)

### L3 — Extraer `LfoModulator` ✅
- [x] Mover 9 campos: LFO runtime (3) + `lfo_delay` + 5 poly_mod
- [x] Mover `generate_lfo_waveform` → `LfoModulator::generate_waveform`
- [x] Split posterior `LfoModulator` → `Lfo` (infra) + `PolyMod` (timbre Prophet-5) para alinear con `synth-core`

### L4 — Descomponer `process_block` ✅
- [x] Extraer `ModulationBus`: snapshot per-block + coeficientes precomputados (exp/powf hoisted)
- [x] Extraer `render_voice_sample(voice, bus, lfo_value)` — cuerpo del loop de voz como método estático
- [x] `process_block` reducido a orquestador: smooth → build bus → per-sample (arp + LFO + voces + master stage)
- [x] Frontera timbre/infra explícita: master stage (M/S + effects + tanh + DC blocker) queda inline como Prophet-5 specific

### L5 — Refactor GUI (`gui.rs`, 1742 líneas) — **Opción B: componentes con estado**

> Decisión (2026-04-19): sesión de brainstorming descartó el builder pattern
> (YAGNI para 11 paneles fijos) y la partición por columna visual (acopla
> código al layout, que es volátil). Se adopta **Opción B**: sólo se convierte
> en `struct` aquello con estado no trivial; los paneles planos quedan como
> funciones libres. `SynthApp` pasa de 22 a ~9 campos con roles explícitos
> (handles externos · componentes · snapshot de frame). Prepara la frontera
> timbre/infra también en la GUI de cara a `synth-core`.

- [ ] **Scaffolding** — convertir `src/gui.rs` en módulo `src/gui/mod.rs`
- [ ] **`KeyboardController`** (`gui/keyboard.rs`) — absorbe `last_key_times`,
      `current_octave`; método `process(ctx, &queue)` con focus-loss panic y Esc
- [ ] **`PresetBrowser`** (`gui/preset_browser.rs`) — absorbe `preset_search`,
      `preset_category_filter`, `current_preset_name`, `new_preset_name`,
      `preset_category`, `show_preset_editor`, `params_a`, `params_b`; incluye
      el A/B comparison (vive en el mismo panel, no justifica struct aparte)
- [ ] **Paneles puros** (`gui/panels.rs`) — funciones libres
      `draw_oscillator`, `draw_mixer`, `draw_filter`, `draw_lfo`,
      `draw_lfo_mod`, `draw_master`, `draw_effects`, `draw_analog`,
      `draw_arpeggiator`, `draw_voice_mode`, `draw_poly_mod`,
      `draw_adsr_curve`. Helpers `section`/`labeled`/`labeled_check` migran
      aquí como `pub(crate)`
- [ ] **Ventanas MIDI** (`gui/midi_windows.rs`) — `draw_midi_monitor` y
      `draw_midi_learn` como funciones libres que reciben los handles
- [ ] **`SynthApp` resultante**: handles (5 Arc/externos) + 2 componentes
      (`keyboard`, `presets`) + snapshot de frame (`params`, `peak_level`) +
      3 flags de ventanas = **11 campos** con rol legible

### L6 — Abstracción CC (`midi_handler.rs`) ✅
- [x] Crear `CC_BINDINGS: &[CcBinding]` — fuente única de verdad (39 entradas) con `{cc, name, label, apply}`
- [x] `handle_cc_message` reducido de ~55 arms a `binding_by_cc(cc)` + 3 especiales (64 sustain, 120/123 notes-off); custom learn via `binding_by_name`
- [x] `draw_midi_learn_panel` itera `CC_BINDINGS` (antes lista hardcodeada de 19) — Learn cubre ahora los 39 parámetros
- [x] Bug de escala corregido de paso: `filter_resonance` (0..10 → 0..4) y detunes (±12 → ±24) alineados con los sliders canónicos del GUI
- [x] `apply_named_param` eliminado (absorbido por el closure `apply` de cada binding)

### Deuda técnica crítica
- [x] **I/O en hilo de audio** — Program Change (0xC0) y SysEx (F0 7D 01/02) ruteados vía `UiEventQueue` al hilo GUI; el callback de audio ya no toca disco ni parsea JSON. `UiEventQueue` es además un `EventQueue<T>` genérico candidato a `synth-core/ipc/`.

---

## Opcional / avanzado

- [ ] **Plugin format (CLAP / VST3)** — para usar el sintetizador como instrumento virtual en un DAW (requiere refactorización arquitectónica mayor).

---

## Futuro: `synth-core` — crate compartido entre sintetizadores

_Contexto: sesión 2026-04-19. Se analizó reutilización entre `synth-analog-rs`, `synth-fm-rs` y `synth-drum-rs` para un futuro Juno-8._

### Módulos candidatos al core (los tres repos los reimplementaron de forma independiente)

| Módulo | Fuente recomendada | Presencia |
|---|---|---|
| `audio_engine.rs` | cualquiera (idénticos) | analog ✅ fm ✅ drum ✅ |
| `midi_handler.rs` | fm-rs (incluye Pitch Bend y Program Change) | analog ✅ fm ✅ drum ✅ |
| `lock_free.rs` / `command_queue.rs` | unificar | analog ✅ fm ✅ drum ✅ |
| `envelope.rs` | fm-rs (key-scaling, velocity, módulo propio) | analog inline fm ✅ drum inline |
| `lfo.rs` | fm-rs (delay, key-sync, módulo propio) | analog inline fm ✅ drum ❌ |
| `effects.rs` | unificar analog+fm | analog ✅ fm ✅ drum ❌ |
| `voice` / polyphony | extraer de synthesizer.rs | analog inline fm inline drum inline |

### Estructura propuesta

```
synth-workspace/
├── Cargo.toml          ← workspace root: members = [core, analog, juno, fm, drum]
├── synth-core/         ← crate lib (audio, midi, dsp, voice, ipc)
├── synth-analog/       ← Prophet-5
├── synth-juno/         ← Juno-8 (futuro)
├── synth-fm/           ← FM/DX7
└── synth-drum/         ← TR-808 style
```

El `synth-core` **no toca audio** — solo mueve datos (MIDI bytes, buffers, envelopes). El timbre y carácter sonoro vive 100% en cada crate hijo. Cada crate usa solo lo que necesita (drum-rs no necesita LFO ni effects del core).

```
synth-core/src/
├── audio/       ← AudioEngine: abstracción CPAL, stream building, buffer management
├── midi/        ← MidiHandler: Note On/Off, CC, Pitch Bend, Program Change
├── ipc/         ← TripleBuffer<T>, MidiEventQueue: lock-free sync GUI↔audio thread
├── dsp/
│   ├── envelope.rs   ← ADSR con key-scaling y velocity (del fm-rs)
│   ├── lfo.rs        ← 6 waveforms, delay, key-sync (del fm-rs)
│   └── effects.rs    ← Chorus, Delay, Reverb (unificado analog+fm)
└── voice/       ← VoiceManager genérico: polyphony, stealing, portamento
```

### Lo que NO va al core (específico de cada instrumento)

- **Prophet-5**: filtro Moog ladder, Poly Mod routing (5 rutas), drift analógico
- **Juno-8**: filtro IR3109, BBD chorus, 1 DCO + sub-osc
- **FM**: 6 operadores, 32 algoritmos DX7, feedback de fase
- **Drum**: síntesis percusiva por instrumento (`bass_drum`, `snare`, `hihat`, `tom`), `Sequencer` step-based, `DrumMachine`

### Cuándo hacerlo

El analog-rs **todavía no está estable**: `audio_engine.rs` y `lock_free.rs` cambiaron 7 veces en los últimos 20 commits (oversampling, micro-tuning, stereo spread en desarrollo activo). Extraer ahora sería refactorizar un blanco en movimiento.

**Momento ideal**: al iniciar el Juno-8 — la presión de tener dos consumers fuerza una API del core bien definida.
