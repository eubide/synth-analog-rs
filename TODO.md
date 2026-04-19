# TODO

Sintetizador analógico tipo Prophet-5 en Rust. Trabajo pendiente, priorizado por impacto.

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

### Paso 3 — Tema visual en `gui.rs`

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

### Paso 4 — Reestructura layout en `gui.rs`

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
| `src/gui.rs` | Reestructura completa layout + tema visual |
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

## Refactor GUI — L5 (`gui.rs`, 1649 líneas)

> L1–L4 + L6 de `synthesizer.rs` completados. Queda el refactor de la capa de presentación.

- [ ] Extraer `KeyboardInput` — 63 líneas de lógica MIDI dentro de `eframe::App::update`
- [ ] Extraer `PresetManager` — `draw_preset_panel` tiene 232 líneas y 5 responsabilidades
- [ ] Builder pattern para los 11 paneles con estructura repetitiva (`draw_xxx_panel`)
- [ ] Mover parámetros A/B comparison fuera de la capa de presentación

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

## Futuro: `synth-core` — crate compartido

> Momento ideal: al iniciar el Juno-8. Extraer ahora sería refactorizar un blanco en movimiento (analog-rs aún inestable).

### Módulos candidatos

| Módulo | Fuente recomendada |
|---|---|
| `audio_engine.rs` | cualquiera (idénticos en los 3 repos) |
| `midi_handler.rs` | fm-rs (incluye Pitch Bend y Program Change) |
| `lock_free.rs` / `command_queue.rs` | unificar |
| `envelope.rs` | fm-rs (key-scaling, velocity) |
| `lfo.rs` | fm-rs (delay, key-sync) |
| `effects.rs` | unificar analog+fm |
| `voice` / polyphony | extraer de synthesizer.rs |

### Estructura propuesta

```
synth-workspace/
├── synth-core/    ← audio, midi, dsp, voice, ipc (no toca timbre)
├── synth-analog/  ← Prophet-5 (Moog ladder, Poly Mod, drift analógico)
├── synth-juno/    ← Juno-8 (IR3109, BBD chorus, DCO)
├── synth-fm/      ← FM/DX7 (6 operadores, 32 algoritmos)
└── synth-drum/    ← TR-808 (síntesis percusiva, sequencer)
```

---

## Opcional / avanzado

- [ ] **Plugin format (CLAP / VST3)** — requiere refactorización arquitectónica mayor.
