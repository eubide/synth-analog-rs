# TODO

Sintetizador analógico tipo Prophet-5 en Rust. Trabajo pendiente, priorizado por impacto.

## Opcional / avanzado

- [x] **A-440 Hz reference tone generator** — botón en MASTER, bypasea toda la síntesis.
- [x] **Voice panning / stereo spread** — panning equal-power por voz, M/S stereo, knob en MASTER.
- [x] **Micro-tuning / alternate tuning tables** — JI (5-limit), Pythagorean, Werckmeister III; ComboBox en MASTER.
- [x] **Oversampling 2×/4×** — biquad LP decimation filter, radio buttons en MASTER.
- [ ] **Plugin format (CLAP / VST3)** — para usar el sintetizador como instrumento virtual en un DAW (requiere refactorización arquitectónica mayor).
