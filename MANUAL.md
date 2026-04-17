# Manual de Usuario — Sintetizador Analógico Vintage

## Descripción General

Emulación del Prophet-5 en Rust: síntesis substractiva con 8 voces, filtro Moog ladder de 24 dB, dual ADSR, LFO avanzado, Poly Mod, y efectos integrados.

Para instalación, requisitos del sistema y compilación ver [README.md](README.md).
Para la arquitectura técnica interna ver [ARQUITECTURA.md](ARQUITECTURA.md).

---

## Modos de Voz

| Modo | Descripción |
|------|-------------|
| **Poly** | Hasta 8 voces simultáneas. Robo inteligente cuando el pool se llena. |
| **Mono** | Una sola voz. Stack de notas con prioridad configurable (Last / Low / High). |
| **Legato** | Como Mono, pero no re-triggeriza envelopes al cambiar nota mientras se mantiene pulsada. |
| **Unison** | Las 8 voces tocan la misma nota con detunings distribuidos uniformemente. |

**Unison Spread**: ajusta cuántos cents se distribuyen entre las 8 voces. Con spread=10, las voces van de −5 a +5 cents.

---

## Osciladores

El sintetizador tiene 2 osciladores (A y B).

### Formas de Onda

| Forma | Carácter |
|-------|----------|
| **Sawtooth** | Rica en armónicos — leads y basses |
| **Square** | Sonido hueco — varía el timbre con Pulse Width |
| **Triangle** | Suave, pocos armónicos — fundamento limpio |
| **Sine** | Tono puro — suboscilador o FM simple |

### Controles

- **Detune**: Afinación en semitonos (−12 a +12)
- **Pulse Width**: Ancho de pulso en ondas Square (0.1 a 0.9)
- **Level**: Nivel del oscilador en el Mixer (0.0 a 1.0)
- **Sync** *(solo Osc B)*: Sincroniza la fase de B con A, creando timbres complejos

---

## Sección de Filtro (24 dB Ladder)

- **Cutoff**: Frecuencia de corte (20 Hz a 20 kHz, escala logarítmica)
  - Valores bajos → sonido cálido / oscuro
  - Valores altos → sonido brillante / abierto
- **Resonance**: Énfasis en el punto de corte (0.0 a 4.0)
  - Por encima de **3.8** → auto-oscilación (el filtro genera su propio tono sinusoidal)
- **Envelope Amount**: Profundidad del Filter Envelope sobre el cutoff (−1.0 a 1.0)
  - Valores negativos → el filtro se cierra cuando llega el envelope
- **Keyboard Tracking**: Cuánto sigue el filtro al teclado (0.0 a 1.0)
  - 1.0 = el cutoff sigue la nota exactamente (útil para auto-oscilación afinada)
- **Velocity → Filter**: Sensibilidad del velocity MIDI sobre el cutoff (0.0 a 1.0)

---

## Envelopes (ADSR)

Dos envelopes independientes con respuesta RC exponencial.

### Amp Envelope

Controla el volumen de cada voz:

- **A (Attack)**: Tiempo de subida al nivel máximo (0.001 s a 5 s)
- **D (Decay)**: Tiempo de caída al nivel de sustain (0.001 s a 5 s)
- **S (Sustain)**: Nivel sostenido mientras la tecla está pulsada (0.0 a 1.0)
- **R (Release)**: Tiempo de caída a silencio tras soltar la tecla (0.001 s a 5 s)

### Filter Envelope

Controla la modulación del filtro. Mismos parámetros ADSR.
La cantidad de modulación se ajusta con **Envelope Amount** en la sección de filtro.

---

## LFO (Low Frequency Oscillator)

### Formas de Onda

- **Triangle**: Modulación suave y continua — vibrato natural
- **Square**: Modulación escalonada — trill, trémolo pulsante
- **Sawtooth**: Rampa ascendente — barrido progresivo
- **Reverse Sawtooth**: Rampa descendente
- **Sample & Hold**: Valores aleatorios cada ~100 veces/s — modulación errática

### Controles

- **Rate**: Frecuencia del LFO (0.05 Hz a 30 Hz)
- **Amount**: Profundidad global de modulación (0.0 a 1.0), escalada por el Mod Wheel (CC 1)
- **Delay**: Tiempo de fade-in del LFO tras el trigger de nota
- **Keyboard Sync**: Reinicia la fase del LFO en cada nota nueva

### Destinos

- **Filter Cutoff**: Barrido de filtro automático (wah, auto-wah)
- **Filter Resonance**: Modulación de la resonancia
- **Osc A Pitch**: Vibrato en oscilador A
- **Osc B Pitch**: Vibrato en oscilador B
- **Amplitude**: Tremolo

---

## Poly Mod (Modulación Polifónica)

Rutas de modulación adicionales por voz, inspiradas en el Prophet-5:

| Fuente | Destino | Efecto |
|--------|---------|--------|
| Filter Envelope | Osc A Pitch | El envelope afina el oscilador A |
| Filter Envelope | Osc A PW | El envelope abre/cierra el pulso de A |
| Osc B | Osc A Pitch | FM clásica (Osc B como modulador) |
| Osc B | Osc A PW | Modulación de ancho de pulso con Osc B |
| Osc B | Filter Cutoff | Barrido de filtro controlado por Osc B |

**Técnica FM**: activa "Osc B → Osc A Pitch", baja el nivel de Osc B en el Mixer a 0 para que no suene directamente, y ajusta el detune de B para cambiar el ratio de modulación.

---

## Mixer

Controla los niveles de las fuentes antes del filtro:

- **Osc A**: Nivel del oscilador A (0.0 a 1.0)
- **Osc B**: Nivel del oscilador B (0.0 a 1.0)
- **Noise**: Nivel del generador de ruido rosa (0.0 a 1.0)

---

## Sección Master

### Pitch Bend

- **Bend Range**: Rango del pitch wheel en semitonos (1 a 24)

### Velocity Curves

Determina cómo el velocity MIDI mapea a amplitud:

| Curva | Fórmula | Uso |
|-------|---------|-----|
| **Linear** | `vel / 127` | Respuesta directa y predecible |
| **Soft** | `√(vel / 127)` | Más sensible en velocidades bajas — teclados táctiles |
| **Hard** | `(vel / 127)²` | Más sensible en velocidades altas — drum pads o teclados duros |

### Aftertouch

El channel pressure (aftertouch) puede modular:
- **→ Cutoff**: La presión abre el filtro
- **→ Amplitude**: La presión sube el volumen de la voz

Ambos sliders van de 0.0 a 1.0 (profundidad de modulación).

---

## Efectos

### Reverb (Freeverb)

- **Amount**: Mezcla húmedo/seco (0.0 a 1.0)
- **Size**: Tamaño de la sala virtual (0.0 a 1.0)

### Delay

- **Time**: Tiempo de repetición (0.01 s a 2 s)
- **Feedback**: Densidad de ecos (0.0 a 0.95)
- **Amount**: Mezcla del delay (0.0 a 1.0)

### Chorus / Ensemble

Dos LFO en cuadratura modulan un delay corto (centro ≈10 ms) — firma clásica Prophet + chorus.

- **Mix**: Mezcla dry/wet (0.0 = bypass, 1.0 = full wet)
- **Rate**: Velocidad de modulación (0.1 a 3 Hz); lento = lush, rápido = warbly
- **Depth**: Profundidad de la modulación de delay (0.0 a 1.0)

---

## VU Meter

La barra de nivel en el encabezado muestra el pico de salida en tiempo real:

| Color | Rango | Significado |
|-------|-------|-------------|
| Verde | 0 – 0.5 | Nivel seguro |
| Amarillo | 0.5 – 0.8 | Nivel óptimo |
| Rojo / CLIP | > 0.8 | Riesgo de saturación |

Si aparece **CLIP**, reduce Master Volume o los niveles de osciladores.

---

## Arpeggiator

- **Enable**: Activa/desactiva el arpegiador
- **Rate**: Velocidad en BPM (60 a 240)
- **Pattern**: Up / Down / Up-Down / Random
- **Octaves**: Número de octavas (1 a 4)
- **Gate**: Duración relativa de cada nota (0.1 a 1.0)

---

## Comparación A/B

Permite comparar dos configuraciones durante el diseño de presets:

| Botón | Acción |
|-------|--------|
| **→A** | Guarda los parámetros actuales en el slot A |
| **A** | Carga los parámetros del slot A |
| **→B** | Guarda los parámetros actuales en el slot B |
| **B** | Carga los parámetros del slot B |

Los botones **A** y **B** están desactivados hasta que el slot tiene datos.

---

## Sistema de Presets

### Presets Incluidos (32 Clásicos)

#### Bass (6)
- **Moog Bass**: Bass profundo y cálido
- **Acid Bass**: Bass acid house, alta resonancia y velocity accent
- **Sub Bass**: Sub-sónico limpio
- **Wobble Bass**: LFO Square sobre cutoff
- **New Order Bass**: "Blue Monday" — pulse width estrecho, tight gate
- **Lately Bass** (Stevie Wonder): filter envelope marcado, cuerpo profundo

#### Lead (8)
- **Supersaw Lead**: Lead potente multi-oscilador con chorus
- **Pluck Lead**: Lead percusivo, KBD tracking alto
- **Screaming Lead**: Agresivo, resonancia alta, vibrato sync
- **Vintage Lead**: Clásico 80s con Poly Mod suave
- **Cars Lead** (Gary Numan): osc sync + chorus
- **Vintage Sync Lead**: sync + filter envelope + Poly Mod
- **Thriller Sync Lead**: sync pitch sweep (filter env → osc A freq)
- **Init Saw Lead**: Prophet fat saw + chorus stock

#### Pad (4)
- **Warm Pad**: Cálido, chorus alto y reverb
- **Choir Pad**: Tipo coro, attacks muy lentos
- **Glass Pad**: Cristalino con Osc2 detune de 24 semitonos
- **Prophet Soft Pad**: Dos saws detuneados + chorus 0.75

#### Strings (2)
- **String Ensemble**: Chorus ensemble denso — firma clásica
- **Vintage Strings**: Prophet saw-strings con chorus 0.7

#### Brass (6)
- **Brass Stab**: Stab con Poly Mod (filter env → osc A)
- **Trumpet Lead**: Lead tipo trompeta, vibrato sync
- **Flute**: Flauta con ruido sutil
- **Sax Lead**: Sax con mixer de noise
- **Jump Brass** (Van Halen "Jump"): Stab con Poly Mod clásico
- **Runaway Brass** (Bon Jovi): Poly Mod doble (osc B + filter env → osc A)

#### FX (5)
- **Arp Sequence**: Secuencia de arpegio
- **Sweep FX**: Barrido con sync + Poly Mod extremo
- **Noise Sweep**: Barrido con ruido y S&H LFO
- **Zap Sound**: Efecto zap con LFO Square
- **Poly Mod Bell**: FM metálica (osc B → osc A freq 0.55)

#### Sequence (1)
- **Berlin School**: Tangerine Dream-style sequence, delay dotted-eighth

### Gestión de Presets

1. **Cargar**: Selecciona en el browser de presets
2. **Filtrar**: Usa el desplegable de categorías para filtrar la lista
3. **Guardar**: Escribe un nombre, elige categoría, y haz clic en "Save"
4. **Random Patch**: Genera un parche aleatorio con parámetros controlados
5. **Preset por defecto**: Usa "save default" / "load default"
6. **Crear clásicos**: El botón "create classic presets" regenera los 32 presets incluidos **sobrescribiendo** los archivos existentes (útil tras personalizaciones accidentales). En arranques normales, los presets se crean solo si faltan, respetando tus ediciones.

---

## Controles del Teclado

### Teclado de Computadora

```
Notas (octava C4):
A  W  S  E  D  F  T  G  Y  H  U  J  K  O  L  P  Ñ
C  C# D  D# E  F  F# G  G# A  A# B  C  C# D  D# E
```

### Controles de Octava

- **Flecha Arriba**: Sube una octava
- **Flecha Abajo**: Baja una octava
- **Rango**: Octavas 0 a 8

---

## Control MIDI

### Mensajes MIDI Soportados

- **Note On/Off**: Reproducción de notas con velocity
- **Pitch Bend**: Rango configurable (1–24 semitonos)
- **Channel Pressure**: Aftertouch de canal
- **Sustain Pedal**: CC 64 — mantiene notas
- **Modulation Wheel**: CC 1 — escala profundidad del LFO
- **Expression Pedal**: CC 11 — volumen expresivo multiplicativo

### Mapeo de Control Change (CC)

#### Expresión y Modulación

| CC | Parámetro | Rango |
|----|-----------|-------|
| 1 | Mod Wheel (profundidad LFO) | 0.0 – 1.0 |
| 11 | Expression pedal (volumen) | 0.0 – 1.0 |

#### Osciladores

| CC | Parámetro | Rango |
|----|-----------|-------|
| 2 | Nivel Osc B | 0.0 – 1.0 |
| 3 | Detune Osc A | −12 a +12 st |
| 4 | Detune Osc B | −12 a +12 st |
| 5 | Pulse Width Osc A | 0.1 – 0.9 |
| 6 | Pulse Width Osc B | 0.1 – 0.9 |

#### Mixer

| CC | Parámetro | Rango |
|----|-----------|-------|
| 7 | Nivel Osc A | 0.0 – 1.0 |
| 8 | Nivel Osc B | 0.0 – 1.0 |
| 9 | Nivel Noise | 0.0 – 1.0 |

#### Filtro

| CC | Parámetro | Rango |
|----|-----------|-------|
| 16 | Cutoff | 20 Hz – 20 kHz |
| 17 | Resonance | 0.0 – 4.0 |
| 18 | Envelope Amount | 0.0 – 1.0 |
| 19 | Keyboard Tracking | 0.0 – 1.0 |

#### Filter Envelope

| CC | Parámetro | Rango |
|----|-----------|-------|
| 20 | Attack | 0 – 5 s |
| 21 | Decay | 0 – 5 s |
| 22 | Sustain | 0.0 – 1.0 |
| 23 | Release | 0 – 5 s |

#### Amp Envelope

| CC | Parámetro | Rango |
|----|-----------|-------|
| 24 | Attack | 0 – 5 s |
| 25 | Decay | 0 – 5 s |
| 26 | Sustain | 0.0 – 1.0 |
| 27 | Release | 0 – 5 s |

#### LFO

| CC | Parámetro | Rango |
|----|-----------|-------|
| 28 | Rate | 0.1 – 20 Hz |
| 29 | Amount | 0.0 – 1.0 |
| 30 | Destino: Osc A Pitch | >63 = ON |
| 31 | Destino: Osc B Pitch | >63 = ON |
| 32 | Destino: Filter Cutoff | >63 = ON |
| 33 | Destino: Amplitude | >63 = ON |

#### Master y Efectos

| CC | Parámetro | Rango |
|----|-----------|-------|
| 34 | Master Volume | 0.0 – 1.0 |
| 40 | Reverb Amount | 0.0 – 1.0 |
| 41 | Reverb Size | 0.0 – 1.0 |
| 42 | Delay Time | 0.01 – 2 s |
| 43 | Delay Feedback | 0.0 – 0.95 |
| 44 | Delay Amount | 0.0 – 1.0 |
| 64 | Sustain Pedal | >63 = ON |

#### Arpeggiator

| CC | Parámetro | Rango |
|----|-----------|-------|
| 50 | Enable | >63 = ON |
| 51 | Rate | 60 – 240 BPM |
| 52 | Pattern | 0=Up, 1=Down, 2=Up-Down, 3=Random |
| 53 | Octaves | 1 – 4 |
| 54 | Gate Length | 0.1 – 1.0 |

### MIDI Learn

El MIDI Learn permite asignar cualquier CC de tu controlador a un parámetro:

1. Haz clic en **"MIDI Learn"** en el encabezado — el botón muestra ●
2. En el panel, haz clic en **"Learn"** junto al parámetro que quieres asignar
3. Mueve el knob o fader en tu controlador — el CC se asigna automáticamente
4. Para eliminar una asignación, haz clic en **×** junto al parámetro

Las asignaciones personalizadas tienen prioridad sobre el mapa CC estándar.

---

## Ejemplos de Uso

### Ejemplo 1: Bass Potente

1. Osc A: Sawtooth, level 0.8
2. Osc B: Square, detune −12, level 0.6
3. Filter: Cutoff 800 Hz, Resonance 2.5
4. Filter Envelope: A=0.01, D=0.5, S=0.3, R=0.8, Env Amount=0.6

### Ejemplo 2: Lead Screaming

1. Osc A: Sawtooth, level 1.0
2. Osc B: Square, detune +7, level 0.7
3. Filter: Cutoff 1.5 kHz, Resonance 3.9
4. LFO: Triangle, Rate 6 Hz, destino Filter Cutoff, Amount 0.4
5. Amp Envelope: A=0.05, D=0.3, S=0.6, R=1.2

### Ejemplo 3: Pad Atmosférico

1. Osc A: Sawtooth, level 0.6
2. Osc B: Triangle, detune +12, level 0.8
3. Filter: Cutoff 2 kHz, Resonance 1.2
4. Reverb: Amount 0.7, Size 0.8
5. Amp Envelope: A=1.5, D=2.0, S=0.7, R=3.0
6. LFO: Triangle, Rate 0.2 Hz, destino Amplitude, Amount 0.2

---

## Técnicas Avanzadas

### Auto-Oscilación del Filtro

- Sube Resonance por encima de 3.8
- El filtro genera su propio tono sinusoidal
- Usa Keyboard Tracking = 1.0 para que el tono siga las notas
- Combina con Envelope Amount negativo para efectos únicos

### Sync entre Osciladores

- Activa "Sync" en Osc B
- Osc B se reinicia cada ciclo de Osc A → armónicos complejos
- Modula el detune de B con LFO para timbres dinámicos

### FM Simple con Poly Mod

- Activa "Osc B → Osc A Pitch" en Poly Mod
- Baja el nivel de Osc B en el Mixer a 0 (Osc B modula, no suena directamente)
- Ajusta el detune de Osc B para cambiar el ratio de FM
- Sube el Amount de Poly Mod para mayor profundidad

### Velocity Expresiva

- **Soft curve**: gran rango dinámico en pianissimo — ideal para teclados táctiles
- **Hard curve**: más fácil llegar al fuerte — ideal para drum pads
- Combina con Filter Velocity para que las notas fuertes sean más brillantes

### Unison Espeso

- Selecciona modo Unison
- Usa Spread alto (20–50 cents) para un muro de sonido
- Añade reverb para pegamento entre voces

---

## Consejos de Rendimiento

1. **Compilación Release**: Siempre usa `cargo run --release` — el modo debug introduce latencia
2. **CPU**: El filtro Moog y las 8 voces en Unison son los puntos más costosos
3. **Polifonía**: 8 voces es el máximo

---

## Solución de Problemas

### Audio

- **Sin sonido**: Verifica que la tarjeta de audio esté activa y seleccionada
- **Distorsión / CLIP**: Reduce Master Volume o niveles de osciladores; el VU meter te avisa
- **Latencia alta**: Verifica la configuración de buffer del sistema de audio

### MIDI

- **MIDI no detectado**: Conecta el dispositivo antes de ejecutar el programa
- **CC no funciona**: Verifica el mapeo en la tabla de CC o usa MIDI Learn
- **Monitor MIDI**: La ventana de monitor muestra todos los mensajes entrantes en tiempo real

### Rendimiento

- **Audio entrecortado**: Cierra otras aplicaciones de audio; usa `cargo run --release`
- **CPU alto**: Evita Unison con reverb y delay simultáneamente
