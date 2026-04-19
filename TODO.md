# TODO

Sintetizador analógico tipo Prophet-5 en Rust. Trabajo pendiente, priorizado por impacto.

## Refactor arquitectónico

> Análisis completo: `synthesizer.rs` (5077 líneas) era un God Object con 79 campos. L1–L6 completos; `Synthesizer` queda con 63 campos y `process_block` ~130 líneas. L5 (GUI) ejecutado en Opción B: `gui.rs` (1742 L) descompuesto en `gui/{mod,keyboard,preset_browser,panels,midi_windows}.rs`; `SynthApp` pasa de 22 → 12 campos con rol explícito.

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

### L5 — Refactor GUI (`gui.rs`, 1742 líneas) — **Opción B: componentes con estado** ✅

> Decisión (2026-04-19): sesión de brainstorming descartó el builder pattern
> (YAGNI para 11 paneles fijos) y la partición por columna visual (acopla
> código al layout, que es volátil). Se adoptó **Opción B**: sólo se convierte
> en `struct` aquello con estado no trivial; los paneles planos quedan como
> funciones libres.

- [x] **Scaffolding** — `src/gui.rs` → módulo `src/gui/mod.rs`
- [x] **`KeyboardController`** (`gui/keyboard.rs`, 144 L) — absorbe
      `last_key_times`, `current_octave`; `process(ctx, &queue)` con
      focus-loss panic, Esc, octave Up/Down y filtro de auto-repeat
- [x] **`PresetBrowser`** (`gui/preset_browser.rs`, 343 L) — absorbe
      `search`, `category_filter`, `save_category`, `new_name`,
      `editor_open`, `current_name`, `slot_a`, `slot_b`. Incluye A/B
      comparison (mismo panel, mismo flow). `random_params()` migra aquí
- [x] **Paneles puros** (`gui/panels.rs`, 747 L) — `draw_oscillator`,
      `draw_mixer`, `draw_filter`, `draw_lfo`, `draw_lfo_mod`, `draw_master`,
      `draw_effects`, `draw_analog`, `draw_arpeggiator`, `draw_voice_mode`,
      `draw_poly_mod`, `draw_envelope`, `draw_adsr_curve`,
      `draw_keyboard_legend` como funciones libres con
      `&mut SynthParameters`. Helpers `section`/`labeled`/`labeled_check`
      y constantes `LABEL_WIDTH`/`WIDGET_WIDTH` aquí
- [x] **Ventanas MIDI** (`gui/midi_windows.rs`, 135 L) —
      `draw_midi_monitor(ui, Option<&MidiHandler>)` y
      `draw_midi_learn_panel(ui, Option<&Arc<Mutex<MidiLearnState>>>)`
      como funciones libres
- [x] **`SynthApp` resultante**: 3 handles externos (`lock_free_synth`,
      `midi_events`, `ui_events`) · 1 keep-alive (`_audio_engine`) ·
      1 handle opcional (`midi_handler`, fuente de `learn_state`) ·
      2 componentes (`keyboard`, `presets`) · 3 flags de ventana
      (`show_midi_monitor`, `show_midi_learn`, `show_presets_window`) ·
      2 snapshot de frame (`params`, `peak_level`) = **12 campos** con rol
      legible (antes 22). `mod.rs` queda en ~430 L: orquestador del layout
      de 5 columnas + `drain_ui_events` + header/VU meter.

### L6 — Abstracción CC (`midi_handler.rs`) ✅
- [x] Crear `CC_BINDINGS: &[CcBinding]` — fuente única de verdad (39 entradas) con `{cc, name, label, apply}`
- [x] `handle_cc_message` reducido de ~55 arms a `binding_by_cc(cc)` + 3 especiales (64 sustain, 120/123 notes-off); custom learn via `binding_by_name`
- [x] `draw_midi_learn_panel` itera `CC_BINDINGS` (antes lista hardcodeada de 19) — Learn cubre ahora los 39 parámetros
- [x] Bug de escala corregido de paso: `filter_resonance` (0..10 → 0..4) y detunes (±12 → ±24) alineados con los sliders canónicos del GUI
- [x] `apply_named_param` eliminado (absorbido por el closure `apply` de cada binding)

### Deuda técnica crítica
- [x] **I/O en hilo de audio** — Program Change (0xC0) y SysEx (F0 7D 01/02) ruteados vía `UiEventQueue` al hilo GUI; el callback de audio ya no toca disco ni parsea JSON. `UiEventQueue` es además un `EventQueue<T>` genérico candidato a `synth-core/ipc/`.

---

## Rediseño GUI — Fidelidad visual al panel Prophet-5

> Sesión 2026-04-20. Decisiones: knobs circulares reales (custom widget egui), layout horizontal estricto idéntico al hardware, todos los controles faltantes activados.

### Layout objetivo

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│  Sequential Circuits  PROPHET-5   [◉ 01 INIT]   ████ VU   [VOL ○]  [PANIC]    │
├──────────┬─────────┬─────────┬───────┬──────────┬───────────┬───────┬──────────┤
│ POLY MOD │  OSC A  │  OSC B  │ MIXER │  FILTER  │ FILTER ENV│  AMP  │   LFO   │
│          │         │         │       │          │           │       │          │
│ FEnv→fqA │  freq   │  freq   │  [A]  │  cutoff  │ [A][D][S] │ init  │  rate   │
│ FEnv→pwA │  octave │  octave │  [B]  │  reson   │ [R]       │ [A][D]│ waveform │
│ OscB→fqA │  pw     │  pw     │  [N]  │ env.amt  │           │ [S][R]│ ○ FREQ A │
│ OscB→pwA │  wave   │  wave   │       │ kbd.trk  │           │       │ ○ FREQ B │
│ OscB→flt │  sync   │ lfom.sw │       │ vel.trk  │           │       │ ○ PW A   │
│          │         │ kbd.sw  │       │          │           │       │ ○ PW B   │
│          │         │         │       │          │           │       │ ○ FILTER │
├──────────┴─────────┴─────────┴───────┴──────────┴───────────┴───────┴──────────┤
│  EFFECTS: [Chorus mix/rate/depth]  [Reverb amt/size]  [Delay time/fbk/amt]     │
│  VOICE: mode/spread/voices  |  ARP: on/rate/pattern  |  MASTER: glide/bend/tune│
│  ANALOG: tolerance/drift    |  PRESETS                                          │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### Paso 1 — Widget Knob circular `src/widgets/knob.rs` (nuevo archivo)

```rust
pub fn knob(ui: &mut Ui, value: &mut f32, range: RangeInclusive<f32>, label: &str) -> Response
```

- Drag vertical (↑ sube, ↓ baja): `delta_y / 200px`
- Shift+drag: fine adjustment (`÷10`)
- Doble click: reset al default
- Hover: tooltip con valor numérico
- Render con `Painter`: círculo `#2a2a2a`, arco rango `#444`, arco valor ámbar `#e8971a`, tick indicador
- Declarar en `src/main.rs`: `mod widgets;`

### Paso 2 — Nuevos parámetros en `synthesizer.rs`

| Campo nuevo | Tipo | Descripción | DSP |
|---|---|---|---|
| `osc1_octave` | `i8` (0/1/2) | 16' / 8' / 4' — Osc A | `freq * 2^((octave-1)*12/12)` |
| `osc2_octave` | `i8` (0/1/2) | 16' / 8' / 4' — Osc B | ídem |
| `osc2_lfo_mode` | `bool` | Osc B en rango sub-audio | multiplicar freq × 0.01 |
| `osc2_keyboard_track` | `bool` | false = pitch fijo en Osc B | ignorar `voice.frequency` |
| `lfo.target_osc1_pw` | `bool` | LFO → PW Osc A | modular `pulse_width` Osc A |
| `lfo.target_osc2_pw` | `bool` | LFO → PW Osc B | modular `pulse_width` Osc B |
| `amp_initial_amount` | `f32` (0–1) | Nivel base del VCA | `vca = init + (1-init)*env` |

Preset format: añadir campos al final (backward-compat automático).

### Paso 3 — Tema visual en `gui/`

```
Fondo panel:    #121212
Sección bg:     #1e1e1e
Sección borde:  #333333
Label texto:    #aaaaaa
Título sección: #ffffff bold 12pt
Accent / knob:  #e8971a (ámbar)
LED activo:     #e8971a
LED inactivo:   #2a2a2a
Peligro:        #ff4040
```

### Paso 4 — Reestructura layout en `gui/mod.rs` + `gui/panels.rs`

- Reemplazar `ui.columns(5, ...)` por `ui.horizontal(|ui| { ... })` con 8 secciones
- Anchos: PolyMod=120 | OscA=160 | OscB=170 | Mixer=100 | Filter=150 | FilterEnv=160 | Amp=140 | LFO=170
- Cada sección: `ui.group(...)` con fondo/borde custom

LFO targets como LED buttons:
```rust
fn led_button(ui: &mut Ui, label: &str, active: &mut bool) -> Response {
    let color = if *active { AMBER } else { DARK_GRAY };
    let resp = ui.add(Button::new(label).fill(color).rounding(3.0));
    if resp.clicked() { *active = !*active; }
    resp
}
```

Barra inferior: efectos + voice mode + arp + master + presets (sliders OK aquí).

### Paso 5 — Integrar knobs

Reemplazar todos los `Slider::...` del panel principal (secciones 1–8) por `knob(...)`.
Mantener sliders solo en la barra inferior (efectos, arp, master).

### Archivos críticos

| Archivo | Cambio |
|---|---|
| `src/synthesizer.rs` | +7 nuevos campos, actualizar DSP |
| `src/gui/panels.rs` | Reestructura completa layout + tema visual |
| `src/gui/mod.rs` | Header, VU meter, orquestación de secciones |
| `src/widgets/knob.rs` | Crear nuevo |
| `src/main.rs` | Añadir `mod widgets;` |

### Verificación

1. `cargo build --release` sin errores
2. 8 secciones horizontales visibles
3. Knobs responden a drag; tooltip muestra valor
4. Osc B: LFO mode cambia rango de frecuencia audiblemente
5. Osc B: keyboard track off → pitch fijo al tocar distintas notas
6. LFO targets LED iluminan en ámbar; modulación se aplica al target correcto
7. `amp_initial_amount > 0` → sonido audible sin envelope
8. Presets existentes cargan sin errores
9. Octave switch cambia pitch en octavas (1 step = 1 octava)

---

## Fidelidad al Prophet-5 Rev4 — controles faltantes

### POLY-MOD
- [ ] **UX autenticidad**: el original usa **2 knobs de profundidad** (FE depth, OscB depth) + **3 toggles de destino compartidos** (Freq A, PW A, Filter). Mi synth tiene 5 sliders independientes — funcionalmente equivalente pero UI diferente al hardware.
- [ ] **FE → Filter** como destino de Poly Mod — falta la ruta `poly_mod_filter_env_to_filter_cutoff`.

### WHEEL-MOD (sección completa ausente)
- [ ] `wheel_mod_lfo_amount` — cuánto LFO entra en la mezcla wheel-mod (knob)
- [ ] `wheel_mod_noise_amount` — cuánto ruido entra en la mezcla wheel-mod (knob)
- [ ] Destinos wheel-mod: **Freq A**, **Freq B**, **PW A**, **PW B**, **Filter** (5 toggles)
- El Mod Wheel (CC 1) escala la profundidad total de esta sección, no del LFO directo

### OSC A / OSC B
- [ ] **Conmutadores independientes de forma de onda** — el original permite Saw + Pulse simultáneos. Mi synth usa selector exclusivo.
- [ ] **Triangle en Osc A** no existe en original — solo Sawtooth + Pulse en Osc A; Triangle es exclusivo de Osc B.

### FILTER
- [ ] **HALF / FULL** switch de keyboard tracking — el original es 3 posiciones (Off / Half / Full), no slider continuo.
- [ ] **VINTAGE** knob — carácter analógico como un único knob. Actualmente: 4 sliders separados en panel Analog.

### PERFORMANCE / GLOBALS
- [ ] **MASTER TUNE** knob — afinación global permanente (no pitch bend). Campo no existe en `SynthParameters`.
- [ ] **RELEASE / HOLD** switch — comportamiento del sustain pedal.
- [ ] **FLT AMP / FLT LFO** como toggles on/off — el original usa botones, no sliders continuos de profundidad.

### PROGRAMMER (preset memory)
- [ ] **GROUP / BANK** selectors — Prophet-5 organiza 200 presets en grupos y bancos. Mi sistema usa archivos planos.
- [ ] **RECORD** button — grabación rápida de preset en slot activo.

---

## Extras a decidir — presentes en mi synth, NO en Prophet-5 original

Decidir si mantenerlos o moverlos a sección "Extended" en la UI:

| Control | Comentario |
|---|---|
| **Arpeggiator** | No en original — útil, pero rompe autenticidad |
| **Reverb / Delay / Chorus** | Original sin efectos onboard |
| **Oversampling** (1×/2×/4×) | Concepto de software, no de hardware |
| **Multiple tuning systems** | Original solo temperamento igual |
| **LFO → Resonance** routing | No existe en original |
| **LFO Reverse Sawtooth / S&H** | Solo Tri/Sqr/Saw en original |
| **Sine waveform** en Osc A y B | No en original ni Rev4 |
| **Velocity curve** selector | Original no tiene curva configurable |
| **Analog character panel** (4 sliders) | Original: VINTAGE knob único |

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
