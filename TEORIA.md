# Cómo Funciona un Sintetizador — De lo Básico a lo Avanzado

Una guía completa para entender el Prophet-5 en Rust desde los primeros
principios. No se asume conocimiento previo de electrónica, matemáticas
avanzadas ni teoría musical.

---

## PARTE I — El Sonido

## 1. Qué Es el Sonido

El sonido es **presión de aire que varía en el tiempo**.

Cuando una cuerda de guitarra vibra, empuja las moléculas de aire hacia
adelante; esas moléculas empujan a las siguientes, que empujan a las
siguientes. Esa cadena de presiones llega a tu oído, mueve el tímpano, y
tu cerebro interpreta el movimiento como "sonido".

En el código, representamos esa presión con un número entre -1.0 y +1.0:
- **0.0** es el silencio (presión de reposo)
- **+1.0** es la máxima compresión
- **-1.0** es la máxima rarefacción (vacío relativo)
- Cualquier valor intermedio es un nivel proporcional de presión

Eso es todo. El audio digital es una secuencia de esos números, tomados
44.100 veces por segundo.

---

## 2. Frecuencia y Tono

La característica más importante de un sonido es **cuántas veces por segundo
oscila**. Eso se llama **frecuencia** y se mide en **Hz (Hercios)**.

```
 Presión
    │
 +1 │  ╭─╮   ╭─╮   ╭─╮   ╭─╮
    │ ╱   ╲ ╱   ╲ ╱   ╲ ╱   ╲
  0 │╱     ╳     ╳     ╳     ╲
    │       ╲   ╱ ╲   ╱ ╲   ╱ ╲
 -1 │        ╰─╯   ╰─╯   ╰─╯   ╰─
    └─────────────────────────────► Tiempo
    ← un ciclo →← un ciclo →← ...

    Esta onda repite 440 veces por segundo → 440 Hz → nota La (A4)
```

Tu oído escucha entre **20 Hz** (subgraves, casi solo los percibes como
presión en el pecho) y **20.000 Hz** (agudísimos, casi inaudibles para
adultos). Dentro de ese rango, frecuencia más alta = tono más agudo.

**La relación entre frecuencias y notas musicales:**

Cada octava dobla la frecuencia:
```
Do 3 = 130.8 Hz
La 4 = 440.0 Hz   (referencia universal de afinación)
La 5 = 880.0 Hz   (el doble → una octava arriba)
La 6 = 1760.0 Hz
```

Esto es logarítmico: el oído no percibe la diferencia de Hz sino el
**ratio** entre frecuencias. Por eso usamos escalas musicales logarítmicas.

---

## 3. La Serie de Fourier — El Secreto de Todo Timbre

Esta es la idea más importante de toda la síntesis de sonido. Entiéndela y
todo lo demás tiene sentido:

> **Cualquier sonido periódico puede descomponerse en sumas de ondas
> sinusoidales puras.**

Esto lo demostró el matemático Joseph Fourier en 1807. Significa que el
violín, la trompeta, la voz humana, la guitarra... todos son simplemente
sumas de senos a distintas frecuencias y amplitudes.

```
Onda compleja = seno(fundamental)
              + seno(fundamental × 2) × amplitud2
              + seno(fundamental × 3) × amplitud3
              + seno(fundamental × 4) × amplitud4
              + ...
```

Los múltiplos de la frecuencia fundamental se llaman **armónicos** u
**overtones**. La distribución de amplitudes entre armónicos es lo que
define el **timbre** de un instrumento.

```
Amplitud
de cada
armónico
    │
100%│████
    │████  ██
 50%│████  ██  ██
    │████  ██  ██  ██  ██
  0%└─────────────────────────► Nº armónico
     1     2   3   4   5 ...

    ↑ Onda sierra: todos los armónicos, decayendo 1/n
    (por eso tiene el timbre más "lleno" y brillante)
```

Esto tiene una consecuencia enorme para el diseño de sonido:
- Para **añadir** armónicos → usar formas de onda ricas (sierra, cuadrada)
- Para **quitar** armónicos → usar el filtro
- Para **esculpir** la distribución → combinar filtro, envelopes, y
  modulación

La síntesis substractiva (la que usa este sintetizador) empieza con una
forma de onda rica en armónicos y **sustrae** los no deseados con el filtro.
Es el enfoque más intuitivo y musical que existe.

---

## 4. Decibeles — La Escala del Oído

El oído percibe el volumen de forma logarítmica. Si duplicas la energía
sonora, el oído no percibe el doble de volumen — percibe un incremento
modesto. Para pasar de "susurro" a "conversación normal" se necesita unas
10 veces más energía. Para pasar a "concierto de rock" se necesita un millón
de veces más.

Por eso medimos volumen en **decibeles (dB)**, una escala logarítmica:

```
  0 dB  → referencia (máximo digital = 1.0)
 -6 dB  → aproximadamente la mitad de amplitud (÷ 2)
-20 dB  → 1/10 de amplitud
-60 dB  → 1/1000 de amplitud (casi inaudible)
-∞ dB  → silencio absoluto (amplitud = 0)
```

En la implementación, el filtro de 24 dB/oct significa que por cada octava
por encima del cutoff, la señal se reduce 16 veces (24 dB). Eso es un corte
muy pronunciado.

---

## 5. Phase (Fase) y Cancelación

Dos ondas a la misma frecuencia pueden estar **en fase** (sus crestas
coinciden) o **desfasadas** (una va "detrás" de la otra).

```
En fase → se suman:          Desfasadas 180° → se cancelan:

 +1│ ╭─╮  ╭─╮                +1│ ╭─╮  ╭─╮
   │╱   ╲╱   ╲  +  mismo    → 0 │                   
 -1│         ╰─╯               -1│      ╰─╯╰─╯

El doble de amplitud               Silencio total
```

Esto explica por qué el detune de dos osciladores crea ese efecto de
"latido": cuando las frecuencias no son exactamente iguales, la fase
relativa entre ellas va cambiando continuamente, alternando entre suma y
cancelación parcial. Eso produce la pulsación característica.

---

## PARTE II — MIDI: El Lenguaje Musical Digital

## 6. Qué Es MIDI (y Qué No Es)

**MIDI no es audio. MIDI son instrucciones.**

La diferencia es la misma que entre una partitura y una grabación. La
partitura dice "toca esta nota con esta fuerza durante este tiempo". La
grabación es el sonido en sí. MIDI es la partitura digital.

El protocolo MIDI data de 1983 y sigue siendo el estándar universal porque
es simple, robusto y eficiente. Un mensaje MIDI tiene típicamente 3 bytes:

```
[byte de estado] [dato 1] [dato 2]

Ejemplo: [0x90, 60, 100]
          ────  ──  ───
          Tipo  Nota Velocidad
          (Note On, canal 1)
```

Todo MIDI cabe en rangos de 0-127 (7 bits). ¿Por qué 7 bits y no 8? El bit
más significativo de cada byte se reserva para indicar si es un byte de
estado o de datos. Una decisión de 1983 que define hasta hoy el rango de
todos los parámetros MIDI.

---

## 7. Los Tipos de Mensaje MIDI

### Note On / Note Off

```
Note On:  [0x90, nota, velocity]   → tecla pulsada
Note Off: [0x80, nota, velocity]   → tecla soltada
          (o [0x90, nota, 0]       → Note On con vel=0 = Note Off)
```

El número de **nota** va de 0 (Do muy grave) a 127 (Si muy agudo).
La nota 60 es el Do central (C4). Cada unidad = un semitono.

Conversión nota MIDI → frecuencia:
```
frecuencia = 440.0 × 2^((nota - 69) / 12.0)
```
- Nota 69 = La 4 = 440 Hz (por definición)
- Nota 70 = La#4 = 440 × 2^(1/12) ≈ 466 Hz
- Nota 81 = La 5 = 880 Hz (12 semitonos arriba = ×2)

En el código esto está en `note_to_frequency()`, que usa una tabla
precalculada (lookup table) en lugar del `powf` para mayor velocidad en
el bucle de audio.

### Velocity (Velocidad / Intensidad)

La velocity mide **con qué fuerza** se pulsa la tecla. Técnicamente, el
teclado MIDI mide el tiempo entre dos contactos internos de la tecla:
rápido = fuerte = velocity alta.

Rango: 0-127, normalizado a 0.0-1.0 en el código.

La velocity puede controlar el volumen, el brillo del filtro, u otros
parámetros. Un piano real suena diferente tocado suave que fuerte no solo
en volumen sino en timbre (más agudo al golpear fuerte). Los buenos
sintetizadores replican esto.

### Control Change (CC) — Los Parámetros Continuos

```
[0xB0, número_cc, valor]
```

Los CC son mensajes para knobs, sliders y pedales. Cada número de CC
tiene un significado estándar (aunque muchos son libres para uso
personalizado):

| CC | Uso estándar |
|----|--------------|
| 1  | Mod Wheel |
| 7  | Volume |
| 10 | Pan |
| 11 | Expression |
| 64 | Sustain Pedal |
| 74 | Filter Cutoff (no oficial pero común) |

Nuestro sintetizador define su propio mapeo (ver `midi_handler.rs`).

### Pitch Bend

```
[0xE0, LSB, MSB]   → valor de 14 bits (0-16383, centro=8192)
```

Es el único mensaje MIDI con resolución de 14 bits (no 7). Esto es
necesario porque el pitch bend debe ser **suave** — con solo 7 bits se
notarían los "escalones" al mover la rueda lentamente.

Normalización: `(valor - 8192) / 8192.0` → rango -1.0 a +1.0.
Aplicado como ratio de frecuencia: `freq × 2^(bend × semitones / 12)`.

### Channel Pressure (Aftertouch)

```
[0xD0, presión]
```

Presión post-pulsación. Algunos teclados miden cuánta fuerza ejerces
sobre la tecla después de haberla pulsado. Es un control expresivo
muy poderoso para modulación en tiempo real: añadir vibrato apretando
más, o abrir el filtro con presión.

### Program Change — Cambiar Preset

```
[0xC0, número_programa]
```

Selecciona un preset. En nuestro sintetizador, carga el preset en la
posición `programa % total_presets` de la lista ordenada.

---

## 8. Por Qué MIDI y Audio Viajan Separados

El MIDI llega por un hilo (hilo MIDI, `midir`). El audio se calcula en
otro hilo (hilo audio, `cpal`). ¿Por qué no procesarlos juntos?

El hilo de audio tiene restricciones durísimas:
- Debe entregar muestras en tiempo estricto (cada ~5 ms en nuestro caso)
- **No puede** esperar por nada: ningún mutex, ningún archivo, ninguna
  reserva de memoria dinámica
- Si llega tarde → dropout audible (un "click" o silencio)

El hilo MIDI no tiene esas restricciones pero sí necesita comunicarse.
La solución: el MIDI deja mensajes en una cola (`MidiEventQueue`), y el
hilo de audio los consume al inicio de cada bloque, cuando sabe que
tiene tiempo para ello.

---

## PARTE III — La Cadena de Síntesis

## 9. Síntesis Substractiva vs. Otros Métodos

Existen varios enfoques para crear sonido electrónicamente:

**Síntesis aditiva:** sumar senos individuales (un oscilador por
armónico). Teóricamente perfecta, en práctica requiere decenas de
osciladores para sonar interesante. Cara computacionalmente.

**Síntesis FM (Modulación de Frecuencia):** un oscilador modula la
frecuencia de otro. Yamaha DX7, 1983. Crea timbres muy complejos con
pocos osciladores, pero difícil de intuir.

**Síntesis por tabla de ondas (Wavetable):** usa formas de onda
grabadas de instrumentos reales. Roland D-50, PPG Wave. Realista pero
poco flexible.

**Síntesis granular:** descompone el sonido en miles de granos
microscópicos (~10-100 ms) y los reorganiza. Texturas imposibles, pero
abstracta.

**Síntesis substractiva:** empieza con formas de onda ricas en
armónicos y las filtra. Es el método del Prophet-5, del Minimoog, del
Roland Juno. **La más intuitiva y musical.** Imita la física de los
instrumentos reales: un clarinete "filtra" el sonido del caño según
la posición de los dedos.

La cadena substractiva siempre sigue el mismo orden:
```
[Oscilador rico] → [Mixer] → [Filtro] → [Amplificador] → [Salida]
       ↑                          ↑              ↑
   genera los              esculpe el       controla el
   armónicos               timbre           volumen
```

Todo lo demás (LFO, envelopes, modulación) son **fuentes de control**
que mueven los parámetros de esa cadena en el tiempo.

---

## PARTE IV — Los Osciladores

## 10. Qué Es un Oscilador

Un oscilador es cualquier cosa que varía de forma periódica y repetitiva.
Un péndulo, una cuerda tensa, un circuito eléctrico, una función matemática.

En síntesis digital, un oscilador es un cálculo que produce un número entre
-1 y +1, y ese número cambia siguiendo una forma predefinida que se repite
a la frecuencia deseada.

La clave es el **phase accumulator**: una variable que avanza de 0.0 a 1.0
y vuelve a 0.0. Representa en qué punto del ciclo está el oscilador.

```
fase = 0.0 → inicio del ciclo
fase = 0.25 → un cuarto del ciclo
fase = 0.5  → mitad del ciclo
fase = 0.99 → casi al final
fase = 1.0  → wrappea a 0.0, empieza de nuevo
```

Cada muestra, la fase se incrementa en `frecuencia / sample_rate`:
- Oscilador a 440 Hz, sample rate 44100 Hz:
  incremento = 440 / 44100 ≈ 0.009977 por muestra
- El ciclo completo tarda 44100/440 ≈ 100 muestras

---

## 11. Las Cuatro Formas de Onda y Sus Armónicos

La forma más importante de entender las formas de onda es conocer qué
armónicos contienen. Recuerda: el timbre ES la distribución de armónicos.

### Seno (Sine Wave)

```
  +1 │  ╭───╮       ╭───╮
     │ ╱     ╲     ╱     ╲
   0 │╱       ╲   ╱       ╲
     │          ╲ ╱         ╲
  -1 │           ╰           ╰
```

La onda más simple. Solo contiene el **fundamental**, sin armónicos.
Suena puro, casi sintético, como un tono de prueba. Es la "nota pura"
de Fourier — el átomo del sonido.

```
Contenido armónico:
Armónico 1 (fundamental): 100%
Armónico 2:                 0%
Armónico 3:                 0%
...                           → Un único pico en el espectro
```

Código: `fast_sin(phase × 2π)` usando tabla LUT.

### Sierra (Sawtooth Wave)

```
  +1 │/|  /|  /|  /|
     │ | /  | /  | /
   0 │ |/   |/   |/
     │                 (cada ciclo: sube linealmente y cae en vertical)
  -1 │
```

La más rica de todas. Contiene **todos los armónicos** (pares e impares),
con amplitud que decae como 1/n:

```
Contenido armónico:
Armónico 1: 100%    (fundamental)
Armónico 2:  50%    (1/2)
Armónico 3:  33%    (1/3)
Armónico 4:  25%    (1/4)
Armónico 5:  20%    (1/5)
...                 → espectro muy denso y brillante
```

Esto explica por qué la sierra es la materia prima favorita en síntesis:
tiene todos los armónicos disponibles para que el filtro los esculpa.

Código:
```
valor = 2.0 × fase - 1.0   (va de -1 a +1 linealmente)
valor -= poly_blep(fase, dt)  (corrección anti-aliasing)
```

### Cuadrada (Square Wave)

```
  +1 │┌───┐   ┌───┐   ┌───┐
     │     │   │     │   │
   0 │     │   │     │   │
     │     │   │     │   │
  -1 │      └──┘       └──┘
```

Solo contiene **armónicos impares**, con amplitud 1/n:

```
Armónico 1: 100%   (fundamental)
Armónico 2:   0%   (par → ausente)
Armónico 3:  33%   (1/3)
Armónico 4:   0%   (par → ausente)
Armónico 5:  20%   (1/5)
...
```

¿Por qué solo impares? La simetría. La cuadrada tiene simetría de media
onda (la segunda mitad es la primera invertida). Matemáticamente, esa
simetría cancela todos los armónicos pares.

Suena "hueca", con ese sonido de clarinete o clavicémbalo. Los armónicos
pares (que dan calidez y "plenitud") están ausentes.

### Triangular (Triangle Wave)

```
  +1 │  /\    /\    /\
     │ /  \  /  \  /
   0 │/    \/    \/
     │
  -1 │
```

Como la cuadrada, solo armónicos impares. Pero decaen mucho más rápido,
como 1/n²:

```
Armónico 1: 100%
Armónico 3:  11%   (1/9 ≈ 11%)
Armónico 5:   4%   (1/25)
Armónico 7:   2%   (1/49)
...           → espectro muy puro, casi como un seno
```

Resultado: más suave que la cuadrada, muy similar al seno pero con un
leve "mordiente" adicional. Sonido de flauta o caja de música.

---

## 12. Pulse Width — El Ancho de Pulso

La onda cuadrada a 50% de ancho de pulso (duty cycle) tiene simetría
perfecta. Pero podemos variar esa proporción:

```
PW = 50%:  ┌───┐   ┌───┐    (50% en +1, 50% en -1)
            └───┘   └───┘

PW = 25%:  ┌─┐     ┌─┐      (25% en +1, 75% en -1)
            └─┘└────┘└───

PW = 10%:  ┌┐      ┌┐       (muy estrecho, muy nasal)
            └┘──────└┘─────
```

A medida que el PW se aleja de 50%, el sonido se vuelve más nasal y fino.
Cada PW diferente tiene un perfil armónico distinto — incluyendo armónicos
pares que aparecen cuando la simetría se rompe.

**Pulse Width Modulation (PWM):** si el LFO mueve el PW lentamente de 30%
a 70% y de vuelta, el timbre "respira" de forma muy orgánica. Es el sonido
característico de cuerdas de sintetizador de los años 80.

---

## 13. Oscillator Sync — Hard Sync

Cuando el sync está activo, cada vez que **OSC A** completa un ciclo
completo (fase pasa de ~0.99 a ~0.0), el sintetizador **resetea
forzosamente la fase de OSC B a cero**.

```
OSC A: ╱|╱|╱|╱|╱|╱|╱|   (controla el reseteo)
OSC B: ╱↺╱↺╱↺╱↺╱↺╱↺     (↺ = reset de fase forzado)
```

Si OSC B está a una frecuencia diferente que OSC A, el reseteo forzado
"parte" el ciclo de OSC B en puntos arbitrarios. Esto crea discontinuidades
que generan armónicos únicos, muy ricos e inarmónicos.

El efecto sonoro: un tono agresivo, brillante, que puede sonar como un
grito o una guitarra distorsionada. Clásico en leads de síntesis.

¿Por qué se llama "hard" sync? Porque el reset es instantáneo y brusco.
Existe también "soft sync" (reset suave), pero no está implementado aquí.

En el código:
```rust
if osc2_sync && prev_phase1 > phase1 {
    // Wrapped around (ciclo completado por OSC A)
    voice.phase2_accumulator = 0;
    phase2 = 0.0;
}
```

---

## 14. Detune — Por Qué Dos Osciladores Suenan Mejor que Uno

Cuando sumas dos señales idénticas, la amplitud dobla y el timbre no cambia.
Pero si desafinas uno ligeramente, ocurren dos cosas:

**1. Batidos (Beats):** las dos frecuencias se adelantan y atrasan
mutuamente a una tasa igual a la diferencia de frecuencias. Si OSC A es
440 Hz y OSC B es 441 Hz, la interferencia pulsa 1 vez por segundo.
A 5 Hz de diferencia, el pulso es rápido y da sensación de coro/ensemble.

**2. Ensanchamiento del espectro:** los armónicos de cada oscilador
también interfieren, creando una "nube" espectral más ancha que un
único oscilador. El resultado suena más vivo y tridimensional.

**Cents:** 100 cents = 1 semitono. El oído es muy sensible a pequeñas
desafinaciones (¡puede detectar diferencias de 3-5 cents!), por eso el
detune fino se mide en esta unidad pequeña.

Conversión: `ratio = 2^(cents/1200)`. Para 5 cents: `2^(5/1200) ≈ 1.00289`.

---

## 15. VCO Drift — La Vida del Analógico

Los osciladores analógicos reales nunca son perfectamente estables. La
temperatura del transistor cambia con el tiempo, con la corriente que pasa,
con el calor del ambiente. Esos cambios de temperatura desvían la frecuencia
de forma lenta e impredecible.

Este "defecto" inadvertido es una de las razones por las que los
sintetizadores analógicos vintage son tan valorados: suenan vivos, no
estáticos. Dos Prophet-5 tocando la misma nota nunca suenan exactamente
igual, y esa sutileza orgánica es parte de su carácter.

En el código, cada voz tiene su propio LFO de deriva lentísimo (0.05-0.25
Hz), con fase inicial aleatoria:

```rust
voice.drift_phase += voice.drift_rate * dt;
let drift_ratio = 1.0 + DRIFT_FREQ_FACTOR
    × fast_sin(voice.drift_phase × 2π);
```

`DRIFT_FREQ_FACTOR = 2.5 × 0.000578` produce una desviación de ±2.5 cents.
Usando la aproximación lineal `2^(c/1200) ≈ 1 + c·ln2/1200` que es
suficientemente precisa para valores pequeños.

---

## 16. Aliasing — El Enemigo Digital de las Formas de Onda

Aquí viene uno de los problemas más importantes de la síntesis digital.

El teorema de Nyquist dice: para representar fielmente una frecuencia f en
digital, necesitas muestrear a al menos 2f. A 44100 Hz de sample rate,
solo podemos representar frecuencias hasta 22050 Hz.

Las formas de onda sierra y cuadrada contienen infinitos armónicos
matemáticamente. En digital, todos los armónicos por encima de 22050 Hz
no desaparecen — **se "doblan" de vuelta al rango audible** como frecuencias
fantasma. Esto se llama **aliasing**.

```
Armónico real en 25000 Hz        → "dobla" a 44100-25000 = 19100 Hz
Armónico real en 30000 Hz        → "dobla" a 44100-30000 = 14100 Hz
```

Esas frecuencias "dobladas" no tienen relación armónica con la fundamental.
Suenan disonantes, metálicas, "digitales" de la peor forma.

Ejemplo: una nota de sierra en 1000 Hz tiene su armónico 23 en 23000 Hz.
Ese armónico se doblaría a 44100-23000=21100 Hz — audible y disonante.

**La solución: PolyBLEP / PolyBLAMP**

En lugar de renderizar la forma de onda naiva y luego filtrar (caro:
requiere renderizar a 4-8× la frecuencia), PolyBLEP **corrige las
discontinuidades en el dominio del tiempo**, justo donde ocurren.

La discontinuidad es el problema: un salto instantáneo en la onda genera
todos esos armónicos infinitos. Si suavizamos el salto con una pequeña
corrección polinómica, los armónicos altos desaparecen naturalmente.

```rust
fn poly_blep(phase: f32, dt: f32) -> f32 {
    // dt = incremento de fase = frecuencia / sample_rate
    // La "zona de corrección" tiene ancho dt (una muestra de ancho)
    if phase < dt {
        let t = phase / dt;
        2.0 * t - t * t - 1.0  // corrección antes de la discontinuidad
    } else if phase > 1.0 - dt {
        let t = (phase - 1.0) / dt;
        t * t + 2.0 * t + 1.0  // corrección después
    } else {
        0.0  // fuera de la zona de corrección
    }
}
```

El resultado: onda sierra con aliasing eliminado sin oversampling.
PolyBLAMP (Band-Limited rAMP) es la versión para discontinuidades en la
derivada, usada para la onda triangular.

---

## 17. El Phase Accumulator — Por Qué Usamos u64 y No f32

El phase accumulator es la variable que registra la posición en el ciclo.
Parece natural usar un número flotante (`f32`), y la mayoría de los tutoriales
así lo hacen. Pero hay un problema sutil:

Los números `f32` tienen 23 bits de mantisa. Para frecuencias bajas en notas
largas (una nota de pad que dura 30 segundos), el incremento de fase es muy
pequeño (~0.00002). Al sumar un número pequeño a un número cercano a 1.0,
el flotante pierde precisión — **el incremento efectivo varía de muestra
a muestra** aunque el deseado sea constante.

Acumulado durante millones de muestras, esto produce **drift de fase** audible:
la frecuencia se desvía ligeramente de la esperada.

Con `u64`: 64 bits de precisión entera, sin error de redondeo, wrappea
perfectamente con `wrapping_add`. El phase se convierte a flotante solo en
el momento de generar la muestra, no en la acumulación.

```rust
// u64: sin drift, siempre exacto
voice.phase1_accumulator =
    voice.phase1_accumulator.wrapping_add(phase1_increment);
let phase1 = (voice.phase1_accumulator & PHASE_MASK) as f32
    / PHASE_SCALE as f32;
```

---

## PARTE V — El Mixer

## 18. Combinar Fuentes de Sonido

El mixer es la sección más simple: suma señales ponderadas.

```
salida = osc1 × nivel_osc1
       + osc2 × nivel_osc2
       + ruido × nivel_ruido
```

Pero hay una sutileza: si sumas dos señales de amplitud máxima (+1.0),
obtienes +2.0, que excede el rango digital. El mixer en sí no lo limita —
eso lo hace el soft limiter al final de la cadena. Esta es una elección
de diseño deliberada: permite que la entrada del filtro se sature ligeramente,
añadiendo calidez armónica.

---

## 19. Ruido Rosa vs. Ruido Blanco

El **ruido blanco** tiene igual energía en todas las frecuencias. Suena
como el siseo de una radio sin señal.

El **ruido rosa** tiene energía que decae -3 dB por octava. Como el oído
humano percibe las octavas logarítmicamente, el ruido rosa suena más
"balanceado" y natural. La mayoría de los sonidos naturales (lluvia, viento,
oleaje) son aproximadamente ruido rosa.

El Prophet-5 original usaba un generador de ruido rosa en hardware. Nuestro
código lo replica con el filtro IIR de Paul Kellett (3 etapas):

```rust
voice.noise_b0 = 0.99886 * voice.noise_b0 + white * 0.0555179;
voice.noise_b1 = 0.99332 * voice.noise_b1 + white * 0.0750759;
voice.noise_b2 = 0.96900 * voice.noise_b2 + white * 0.153852;
pink = (b0 + b1 + b2 + white * 0.0556418) * noise_level;
```

Cada coeficiente cerca de 1.0 es un filtro paso bajo de primer orden.
Los tres en paralelo con diferentes coeficientes crean la pendiente -3
dB/oct característica del rosa sobre el rango audible.

El PRNG es **xorshift32**: una secuencia pseudoaleatoria con solo 3
operaciones XOR + shift, determinista (misma semilla = misma secuencia),
y aproximadamente 8 veces más rápido que `rand::random()`. Cada voz tiene
su propia semilla para que el ruido de cada voz sea independiente.

---

## PARTE VI — El Filtro

## 20. Qué Hace un Filtro

Un filtro deja pasar algunas frecuencias y atenúa otras. Es exactamente
como el ecualizador de un equipo de música, pero con un diseño específico
para síntesis.

**Tipos de filtro por su respuesta:**

```
Paso Bajo (Low Pass):      Paso Alto (High Pass):
   ████████╲                     ╱████████
           ╲──────               ──────╱
  ← deja pasan                        → deja pasan
    los graves                            los agudos

Paso Banda (Band Pass):   Notch (Banda Eliminada):
       ╭─╮                  ████╰───╯████
  ─────╯   ╰─────            ─────────────
  Solo pasa la               Elimina una banda,
  banda central             deja el resto
```

Los sintetizadores substractivos típicamente usan el **paso bajo**: se
parte de un sonido rico y se elimina el brillo progresivamente bajando
el cutoff. Es el movimiento sonoro más icónico de la síntesis.

---

## 21. La Frecuencia de Corte (Cutoff) en Detalle

El cutoff no es una barrera binaria ("hasta aquí pasa, desde aquí no").
Es una frecuencia donde empieza la atenuación, y la señal va cayendo
progresivamente por encima de ese punto.

La pendiente del filtro define qué tan agresiva es esa caída:

```
Amplitud
    │
100%│████████████╲
    │             ╲        ← 6 dB/oct (1 polo): suave
    │              ╲_____
    │
100%│████████████╲
    │              ╲       ← 12 dB/oct (2 polos): pronunciado
    │               ╲____
    │
100%│████████████╲
    │              ╲╲      ← 24 dB/oct (4 polos): muy pronunciado
    │                ╲╲___
    └────────────────────── Frecuencia
                    ↑
                  Cutoff
```

El filtro Moog de 24 dB/oct (4 polos) tiene una caída tan pronunciada que
la diferencia entre "abierto" y "cerrado" es enorme. Eso lo hace tan
expresivo: pequeños movimientos del cutoff tienen un efecto dramático.

---

## 22. La Resonancia — Retroalimentación en el Filtro

La resonancia es el parámetro que más define el carácter de un filtro.
Técnicamente, es retroalimentación: la salida del filtro se realimenta a
su entrada con un cierto factor k.

```
       ┌──────────────────────────────┐
       │                              │  k (resonancia)
Input →+─→ [Filtro Paso Bajo] ──┬──→ Output
                                 │
                                 └──────────────────┘ (feedback)
```

Con retroalimentación positiva y k moderado, las frecuencias cercanas al
cutoff se amplifican. Esto crea el pico característico de la resonancia.

Con k alto (>3.8 en nuestro modelo), la retroalimentación es tan fuerte
que el sistema entra en **autooscilación**: el filtro genera su propia onda
sinusoidal a la frecuencia del cutoff, sin necesidad de entrada. Es un
oscilador adicional gratuito.

La autooscilación produce un seno puro. Muchos sintetistas usan esto
deliberadamente: bajan todos los osciladores y dejan que el filtro
autooscile como fuente de sonido, luego lo modulan con los envelopes para
crear efectos únicos.

Limitación en el código: `resonance.clamp(0.0, 3.95)`. El límite teórico
de autooscilación en el modelo ZDF es k=4.0, pero se deja un margen de
seguridad para evitar inestabilidad numérica.

---

## 23. El Filtro Moog Ladder — Historia y Física

En 1965, Robert Moog diseñó un filtro basado en **cuatro transistores en
cascada**, con retroalimentación de la salida a la entrada. La idea era
brillante: cada transistor actúa como un filtro de 6 dB/oct, y cuatro en
serie dan 24 dB/oct.

Lo que hizo especial a ese diseño no fue el corte — fue la **saturación
no lineal** de los transistores. Cada transistor tiene un rango lineal
limitado; por encima de cierto nivel, la señal se "dobla" suavemente en
lugar de pasar directamente. Eso introduce armónicos **pares** (2ª
armónica, 4ª, 6ª...), que el oído percibe como calidez y plenitud.

La función matemática que modela esa saturación es `tanh(x)`:

```
    tanh(x)
  1 │         ╭──────────
    │       ╱
    │     ╱
  0 │────╱
    │   ╱
    │ ╱
 -1 │╰──────────
    └────────────────────► x
       lineal en el
       centro, satura
       en los extremos
```

Cada etapa del filtro aplica `tanh` a su entrada. Resultado: cuando hay
señal fuerte, los transistores entran en saturación y añaden esos armónicos
cálidos. Con señal suave, el filtro es perfectamente lineal.

---

## 24. La Topología ZDF TPT — La Matemática Detrás del Filtro Digital

Digitalizar un filtro analógico es delicado. El método más simple
(Euler hacia adelante) es inestable. El método bilineal (transformada Z)
funciona pero introduce distorsión de frecuencia.

**ZDF (Zero-Delay Feedback)** con **topología TPT (Topology-Preserving
Transform)**, desarrollado por Vadim Zavalishin, resuelve ambos problemas.
La clave: cada elemento analógico se reemplaza por su equivalente digital
que preserva exactamente la topología del circuito.

Para un filtro de un polo (una etapa del ladder):

```
Analógico:              Digital TPT equivalente:
    ┌─ RC ─┐                g = tan(π·fc/fs)     ← pre-warping exacto
in ─┤       ├─ out      G = g / (1+g)          ← ganancia de integrador
    └──────┘
                        Por muestra:
                          v = G × (entrada - estado)
                          salida = v + estado
                          estado_nuevo = salida + v
```

El **pre-warping** (`g = tan(π·fc/fs)`) es crucial: mapea exactamente la
frecuencia de corte analógica a la digital. Sin él, el cutoff digital
diferiría hasta un 40% del cutoff especificado en frecuencias altas.

Las cuatro etapas en cascada con retroalimentación:

```rust
// Retroalimentación con tanh para saturación
let x = Self::fast_tanh(input - k * state.stage4);

// Etapa 1
let v1 = cap_g * (x - state.stage1);
let y1 = v1 + state.stage1;
state.stage1 = y1 + v1;

// Etapas 2-4 igual, con tanh en cada entrada
// ...

// Compensación de ganancia del passband
let g4 = cap_g.powi(4);
output = y4 * (1.0 + k * g4);
```

La compensación de passband es necesaria porque al aumentar la resonancia,
el filtro atenúa su propio passband (las frecuencias por debajo del cutoff
también se afectan). Multiplicar por `(1 + k·G⁴)` restaura el nivel
percibido.

---

## 25. fast_tanh — La Aproximación de Padé

El filtro llama a `tanh` **5 veces por voz por muestra**: 4 etapas más la
retroalimentación. Con 8 voces a 44100 Hz son 1.76 millones de llamadas
por segundo. `libm::tanh` es costosa (usa series de Taylor con muchos
términos).

La **aproximación de Padé** resuelve esto: es una función racional (un
polinomio dividido entre otro polinomio) que aproxima `tanh` con error
menor al 0.1% para |x| ≤ 3, y luego se clampea a ±1 para valores mayores:

```rust
fn fast_tanh(x: f32) -> f32 {
    if x > 3.0  { return 1.0;  }
    if x < -3.0 { return -1.0; }
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}
```

Solo requiere una multiplicación, una suma y una división — mucho más rápido
que la serie de Taylor. El error máximo de 0.1% es inaudible.

---

## 26. Keyboard Tracking — El Filtro Sigue al Tono

En un instrumento real (una flauta, una trompeta), los armónicos de las
notas agudas también son más agudos. Si el filtro tuviera un cutoff fijo,
las notas graves sonarían brillantes y las agudas sonarían oscuras — lo
contrario de lo natural.

Con keyboard tracking al 100%, el cutoff del filtro se multiplica por el
mismo ratio que la frecuencia de la nota respecto al Do central:

```rust
let kbd_multiplier = semitones_to_ratio(
    (voice.note as f32 - 60.0) * filter_keyboard_tracking
);
modulated_cutoff *= kbd_multiplier;
```

- Nota 60 (Do4): multiplicador = 1.0 (sin cambio)
- Nota 72 (Do5): multiplicador = 2.0 (cutoff se dobla, como la frecuencia)
- Nota 48 (Do3): multiplicador = 0.5 (cutoff a la mitad)

Con tracking=1.0, el filtro "sigue" perfectamente a las notas. Con tracking
entre 0 y 1 se obtienen configuraciones intermedias.

---

## 27. Denormal Flush — Un Detalle Técnico Crítico

Los números `f32` IEEE 754 tienen un rango normal mínimo de ~1.18×10⁻³⁸.
Por debajo de ese valor existen los **números denormalizados** (subnormales),
que el hardware procesa mediante un mecanismo mucho más lento (software en
muchas CPUs): hasta 100× más lento.

En las colas del filtro, cuando una nota se apaga y el envelope lleva a cero,
los valores en los estados del filtro decaen exponencialmente:
1.0 → 0.1 → 0.01 → ... → 1e-10 → 1e-20 → ... → valor denormal.

Si los estados caen al rango denormal, el siguiente bloque de audio puede
ser dramáticamente más lento:

```rust
const DENORMAL_FLOOR: f32 = 1.0e-20;
if state.stage1.abs() < DENORMAL_FLOOR { state.stage1 = 0.0; }
// (para cada etapa)
```

Poner el valor a exactamente 0.0 (que SÍ está en el rango normal) es inocuo
(el filtro está prácticamente silencioso de todas formas) y evita el
slowdown.

---

## PARTE VII — Los Envelopes

## 28. La Forma del Sonido en el Tiempo

Si el filtro esculpe el espectro de frecuencias, el envelope esculpe la
**evolución temporal**. Un piano no suena igual que un órgano aunque tengan
el mismo timbre instantáneo: el piano tiene un ataque brusco y un decay
largo; el órgano sustain plano desde que pulsas hasta que sueltas.

El **ADSR** es el modelo estándar de envelope, con cuatro fases:

```
Amplitud
  │
1 │          ╭──────────────────────╮
  │         ╱│                      │╲
  │        ╱ │                      │ ╲
S │─ ─ ─ ╱  │  (sustain level)     │  ╲
  │      ╱   │                      │   ╲
  │     ╱    │                      │    ╲
0 │────╱─────┴──────────────────────┴─────╲──────────► Tiempo
       ↑A   ↑D      S activo         ↑R
  Pulsas tecla                    Sueltas tecla
```

**Attack (A):** tiempo de 0.0 a 1.0. Si es 0, el sonido arranca de golpe.
Con attack largo, la nota "entra" gradualmente, como un violín que
empieza con el arco lento.

**Decay (D):** tiempo de 1.0 al nivel de Sustain. Después del pico inicial,
la energía decae. En un piano, el Decay es lo más importante — la nota
inmediatamente empieza a apagarse desde el golpe.

**Sustain (S):** nivel (0.0-1.0) que se mantiene mientras la tecla está
pulsada. En un órgano de tubo es 1.0 (máximo mientras pulses). En un piano
es ~0 (la nota sigue decayendo aunque mantengas la tecla).

**Release (R):** tiempo de caída desde el nivel de Sustain hasta 0.0 al
soltar la tecla. Un pad con release largo "cola" mucho después de soltar.
Una percusión tiene release muy corto.

---

## 29. Curvas Lineales vs. Exponenciales — Por Qué Una Suena "Mejor"

La forma más simple de implementar el Attack: incrementar linealmente un
valor de 0 a 1 en N segundos. Pero eso no suena como los instrumentos reales.

El problema: el oído percibe el volumen **logarítmicamente**. Un cambio de
0.0 a 0.5 lo percibe como "la mitad del camino hasta el máximo", no como
la mitad de la amplitud. Una curva lineal suena rápida al principio y lenta
al final.

La solución: curvas exponenciales, que coinciden con la percepción del oído.

Implementación con **circuito RC** (Resistencia-Capacitor). Este es el
circuito más básico de electrónica: un resistor y un capacitor en serie.
Cuando aplicas voltaje, el capacitor se carga de forma exponencial, nunca
llegando exactamente al valor objetivo pero acercándose cada vez más:

```
valor(t) = objetivo × (1 - e^(-t/τ))
```

donde τ = RC es la constante de tiempo. En Rust:

```rust
// Attack: converger hacia 1.0 con coeficiente exp(-5·dt/attack)
voice.envelope_value = 1.0 + (voice.envelope_value - 1.0) * attack_coeff;
// donde attack_coeff = exp(-dt × 5.0 / envelope_attack)
```

El factor 5 hace que el valor llegue al 99.3% del objetivo en el tiempo
especificado (e^(-5) ≈ 0.0067). Sin él, el envelope nunca "llega"
formalmente, lo que en código requería un umbral de parada de todas formas.

La curva RC exponencial suena natural porque los instrumentos físicos
también siguen dinámica exponencial: las cuerdas decaen exponencialmente,
los circuitos RC son la base de los filtros analógicos, y el oído mismo
tiene respuesta logarítmica.

---

## 30. Retrigger Sin Clicks

Cuando pulsas la misma nota dos veces seguidas, los envelopes deben
reiniciarse. El problema: si el envelope estaba en mitad de su Decay a
amplitud 0.7 y de repente salta a empezar desde 0.0, hay un click audible
(una discontinuidad brusca en la señal).

La solución: **reanudar desde el valor actual**. El Attack no comienza en
0.0, comienza desde donde esté el envelope en ese momento.

```rust
// Smooth retrigger: reiniciar estado pero mantener valor actual
voice.envelope_state = EnvelopeState::Attack;
voice.envelope_time = 0.0;
// voice.envelope_value NO se toca → el ataque parte de aquí
```

Dado que el Attack usa una curva exponencial convergiendo hacia 1.0, comenzar
desde 0.7 en lugar de 0.0 simplemente hace que tarde menos en llegar al
pico. La transición es completamente suave.

---

## 31. El Envelope de Filtro

El sintetizador tiene **dos envelopes independientes** con la misma estructura
ADSR:

1. **Amp Envelope:** controla el VCA (volumen de la voz)
2. **Filter Envelope:** controla el cutoff del filtro

El Filter Envelope permite que el brillo de la nota evolucione de forma
diferente al volumen. Ejemplo clásico:

```
Volumen (Amp Envelope):
1 │ ╭───────────────────╮
  │╱                     ╲
0 │                        ╲___

Cutoff relativo (Filter Envelope con amount=0.8):
1 │ ╭╮
  │╱  ╲___________________
0 │                        ╲___
```

Resultado: el sonido "abre" brillante al ataque y luego se oscurece
rápidamente, mientras el volumen se mantiene. Es el sonido clásico de
"wah" sintético de los 80.

El **Filter Envelope Amount** (FEA) controla cuánto desplaza el envelope
al cutoff:
```
cutoff_efectivo = cutoff_base + FEA × cutoff_base × filter_envelope_value
```

Con FEA=0, el filter envelope no hace nada. Con FEA=1, el envelope puede
mover el cutoff hasta el doble de su valor base.

---

## PARTE VIII — El LFO y la Modulación

## 32. El LFO — Un Oscilador que Controla Otros Parámetros

**LFO** = Low Frequency Oscillator. Es matemáticamente igual a OSC A y
OSC B, pero su frecuencia está típicamente entre 0.1 y 20 Hz — demasiado
baja para oírla como nota, pero perfecta para mover parámetros de forma
cíclica y expresiva.

La distinción "oscilador de audio vs LFO" no es una diferencia de naturaleza
sino de uso. Un LFO a 20 Hz empieza a sonar como nota si lo aplicas al
pitch. Un oscilador de audio a 1 Hz funciona exactamente como un LFO.
La frontera es difusa y explorar esa frontera produce los sonidos más
experimentales.

---

## 33. Modulación — Qué Significa

"Modular" significa **usar un valor para cambiar otro valor en tiempo real**.

```
Fuente de modulación → [Amount] → Destino de modulación

LFO → × 0.5 → Cutoff del filtro
            ↓
        El filtro se abre y cierra al ritmo del LFO
```

Casi todo en un sintetizador es modulación:
- El envelope de amplitud modula el VCA
- El envelope de filtro modula el cutoff
- El LFO modula el pitch → vibrato
- La velocity modula la amplitud → tocar más fuerte = más volumen

**Amount (profundidad):** cuánto afecta la fuente al destino. Un LFO a
Amount=0 no hace nada; a Amount=1 tiene el máximo efecto.

**Unipolar vs. Bipolar:**
- **Bipolar:** la fuente va de -1 a +1 (ej: LFO triángulo → el cutoff sube
  y baja simétricamente)
- **Unipolar:** la fuente va de 0 a +1 (ej: envelope → el VCA solo puede
  multiplicar, nunca invertir)

---

## 34. La Matriz de Modulación

Nuestro sintetizador tiene un conjunto de posibles conexiones entre fuentes
y destinos:

```
Fuentes:          Destinos:
┌──────────┐      ┌──────────────────┐
│ LFO      ├─────►│ OSC A pitch      │
│ Velocity ├─────►│ OSC B pitch      │
│Aftertouch├─────►│ Filter cutoff    │
│ Mod Wheel├─────►│ Filter resonance │
│ FiltEnv  ├─────►│ Amplitud VCA     │
│ AmpEnv   ├─────►│ OSC A pulse width│
└──────────┘      └──────────────────┘
```

El "Amount" de cada conexión es un número que escala la señal de la fuente
antes de aplicarla al destino. La suma de todas las modulaciones activas
define el valor final del parámetro.

---

## 35. LFO Aplicado al Pitch — El Vibrato

Vibrato es modulación del tono. Un músico de cuerda hace vibrato moviendo
el dedo que pisa la cuerda: la longitud efectiva oscila ligeramente, lo que
oscila la frecuencia.

```rust
if lfo_target_osc1 {
    freq1 *= 1.0 + (lfo_value * modulation_matrix.lfo_to_osc1_pitch * 0.1);
}
```

El factor 0.1 escala para que un LFO de amplitud máxima (1.0) produzca una
desviación de ±10% de la frecuencia (aproximadamente ±1.66 semitonos), que
es un vibrato muy exagerado. En uso normal, `lfo_amplitude` es 0.1-0.3 y
la desviación es sutil.

El **Mod Wheel** escala la profundidad del LFO en tiempo real:

```rust
let lfo_value = generate_lfo(...) * lfo_amplitude * (1.0 + mod_wheel);
```

Con mod_wheel=0: amplitud normal. Con mod_wheel=1: el doble de profundidad.
El músico puede añadir vibrato expresivo girando la rueda mientras toca.

---

## 36. Sample & Hold — Azar Controlado

La forma de onda S&H del LFO no es una curva continua. Cada cierto tiempo
(actualmente ~100 veces por segundo en el código), elige un nuevo valor
aleatorio y lo mantiene constante hasta el siguiente intervalo.

```
Valor S&H:
  +1│  ████      ████████
    │                        ████
    │      ████████
 -1│
    └─────────────────────────────► Tiempo
```

Aplicado al pitch produce ese sonido de "computadora de ciencia ficción"
que da saltos aleatorios. Aplicado al filtro produce variaciones tímbricas
impredecibles. Es una fuente de modulación estocástica controlada.

---

## 37. Poly Mod — La Joya del Prophet-5

La modulación polifónica es lo que hace único al Prophet-5. En la mayoría
de los sintetizadores, el LFO es **global**: el mismo LFO mueve el filtro
de todas las voces al mismo tiempo y a la misma velocidad.

Con Poly Mod, **cada voz se modula a sí misma** con sus propios valores:

**Filter Envelope → Frecuencia de OSC A:**

El envelope del filtro de la voz (que ya varía según el ADSR) también
modula el tono de OSC A de esa misma voz. Si el amount es positivo, al
inicio de la nota (envelope en Attack) el tono sube, y luego cae al Decay.

Resultado: un "scream" de ataque muy agresivo y expresivo. Con amount alto,
la nota parece "gritar" al inicio y luego asentarse al tono real.

**OSC B → Frecuencia de OSC A:**

La salida de audio de OSC B se usa para modular la frecuencia de OSC A.
Esto es literalmente **síntesis FM (Frequency Modulation)** dentro de un
sintetizador substractivo.

Si OSC B está a frecuencia cercana a OSC A, la modulación FM produce
sidebands (bandas laterales) a frecuencias `fA ± n·fB`. Si la relación
es simple (2:1, 3:1...), los sidebands son armónicos. Si la relación es
compleja, los sidebands son inarmónicos → texturas metálicas, campanas,
efectos imposibles de conseguir por otros medios.

```rust
// 1-sample delay evita dependencia circular
let osc_b_mod = voice.osc2_last_out;
if poly_mod_osc_b_freq.abs() > 0.001 {
    let semitones = poly_mod_osc_b_freq * 24.0 * osc_b_mod;
    freq1 *= Self::semitones_to_ratio(semitones);
}
```

El delay de 1 muestra (usar `osc2_last_out` en lugar del valor del frame
actual) no es un compromiso — es la realidad física del hardware analógico,
donde hay propagación finita de señal entre circuitos.

---

## 38. LFO Delay / Fade-In

Un violinista no aplica vibrato desde el primer instante de la nota. Ataca
limpio y luego va añadiendo vibrato gradualmente. Esta es una cuestión de
fraseo musical.

El LFO Delay replica esto: el efecto del LFO sube de 0 a su valor completo
durante N segundos desde el inicio de la nota.

Cada voz tiene su propio contador `lfo_delay_elapsed` que avanza desde 0 al
pulsar la nota. La profundidad efectiva del LFO para esa voz es:

```
depth_efectiva = lfo_amplitude × min(1.0, lfo_delay_elapsed / lfo_delay)
```

Fade-in lineal. Con 2 segundos de delay, el vibrato tarda 2 segundos en
llegar a su profundidad completa.

---

## PARTE IX — Las Voces y la Polifonía

## 39. Qué Es una Voz

Una **voz** es una instancia completa e independiente de la cadena de síntesis:
- Sus propios osciladores (fase, frecuencia, drift)
- Su propio filtro (con sus estados internos)
- Sus propios envelopes (amp y filtro, con sus estados)
- Su propio generador de ruido rosa (PRNG independiente)
- Su propia nota, velocity, estado de sustain

Cuando suenas un acorde de Do mayor (Do-Mi-Sol), el sintetizador asigna
3 voces: una canta Do, otra Mi, otra Sol, cada una completamente
independiente. Cada nota tiene su propio ataque, su propio filtro, su
propio decaimiento.

El Prophet-5 original tenía 5 voces (de ahí el "-5" en el nombre). Nuestro
sintetizador tiene 8, igual que versiones más avanzadas.

---

## 40. Voice Stealing — El Arte de Robar Voces

Con 8 voces máximas, si el músico toca un acorde de 9 notas, hay que
liberar una voz para la nueva nota. ¿Cuál?

El objetivo: que el robo sea lo más inaudible posible.

El algoritmo puntúa cada voz activa con varios criterios:

```rust
let mut score = 0.0;

// Preferir voces en Release (ya "soltadas" aunque aún sonando)
if voice.envelope_state == EnvelopeState::Release { score += 100.0; }

// Preferir voces más silenciosas
score += (1.0 - voice.envelope_value) * 50.0;

// Preferir voces más antiguas
score += voice.envelope_time * 10.0;
```

Mayor score = mejor candidato para robar. Se roba la que tiene mayor score.

La lógica: si una voz está en Release, ya fue "soltada" por el músico —
interrumpirla tiene poco impacto musical. Entre las que están activas, las
más silenciosas son menos perceptibles al cortarlas.

---

## 41. Modos de Voz en Detalle

### Poly

El modo estándar. Cada nota asigna una voz libre (o roba). Hasta 8 notas
simultáneas. Ideal para acordes y texturas.

### Mono

Solo una voz activa. El sintetizador mantiene un **note stack** (pila de
notas): todas las teclas actualmente pulsadas, en orden de llegada.

Cuando se suena una nota mientras hay otra activa, la nueva reemplaza a la
vieja. Si se suelta la más reciente mientras la anterior aún está pulsada,
vuelve a la anterior sin silencio intermedio.

El **Note Priority** define qué nota del stack "gana":
- **Last:** la más reciente pulsada (más intuitivo para melodía)
- **Low:** la más grave del stack (para bajo monofónico que prioriza graves)
- **High:** la más aguda (para solos que priorizan el treble)

### Legato

Como Mono pero con un matiz crucial: al cambiar de nota **sin soltar la
anterior**, los envelopes no se reinician.

```rust
if !legato {
    voice.envelope_state = EnvelopeState::Attack;
    voice.envelope_time = 0.0;
    voice.filter_envelope_state = EnvelopeState::Attack;
}
// Si legato: solo cambia la frecuencia, los envelopes continúan
```

El efecto: la nota anterior desliza en frecuencia a la nueva sin el "golpe"
del Attack. Como la ligadura en un instrumento de viento. Los solos suenan
más fluidos y vocales.

### Unison

Todas las 8 voces tocan la misma nota, con **detune spread** distribuido
uniformemente entre ellas:

```rust
for i in 0..n_voices {
    let detune_cents = spread * (2.0 * i as f32 / (n_voices - 1) as f32 - 1.0);
    // Para 8 voces y spread=10: -10, -7.14, -4.28, -1.43, 1.43, 4.28, 7.14, 10 cents
}
```

El resultado: un sonido masivo, como un coro de 8 sintetizadores. El
**Unison Spread** (en cents) controla qué tan desafinadas están las voces
entre sí. Con spread pequeño (~2-5 cents): denso y ligeramente "vivo".
Con spread grande (~20-50 cents): coro detuneado y grueso.

La normalización de voces (`1/√N`) previene que 8 voces unison sean 8
veces más fuertes que una voz sola.

---

## 42. Glide / Portamento

Al cambiar de nota con glide activo, la frecuencia no salta — **desliza
exponencialmente** de la frecuencia anterior a la nueva.

La interpolación exponencial es clave: en lugar de moverse linealmente
(igual distancia por unidad de tiempo), se mueve un **porcentaje del
camino restante** cada muestra:

```rust
voice.glide_current_freq = voice.frequency
    + (voice.glide_current_freq - voice.frequency) * glide_coeff;
// glide_coeff = exp(-1 / (glide_time × sample_rate))
```

La interpolación exponencial suena más natural que la lineal porque:
1. Los intervalos musicales son logarítmicos (semitonos, no Hz)
2. El inicio del deslizamiento es rápido y luego desacelera, como un
   músico que llega "hacia" la nota target

---

## PARTE X — Los Efectos

## 43. El Delay — El Eco

El delay es una línea de retardo: graba la señal y la reproduce N
milisegundos después, mezclada con la señal original.

Implementación: un **buffer circular** de tamaño máximo (2 segundos a
44100 Hz = 88200 muestras). Un índice apunta al "cabezal de escritura".
Leer en `índice - delay_samples` da la muestra que se grabó hace
`delay_time` segundos.

```
Buffer circular:
[  ...  |  ...  | muestra_actual |  ...  |  ...  ]
                       ↑ escritura
         ↑ lectura (delay_samples atrás)

salida = señal_actual + lectura × wet_amount
escritura = señal_actual + lectura × feedback
```

El **feedback** es la clave: la señal del delay vuelve al input. Eso crea
ecos que se repiten y se van apagando. Con feedback alto (>0.9), los ecos
duran mucho. Con feedback=0, solo hay un eco.

El delay es también el bloque constructor de otros efectos: chorus (delay
muy corto < 30ms, modulado con LFO), flanger (delay de 1-20ms con feedback
fuerte), vibrato (delay corto modulado), reverb (miles de delays cortos).

---

## 44. La Reverb — Simulando el Espacio

Cuando escuchas en una catedral, el sonido rebota en paredes, techos y
columnas miles de veces antes de llegar a tus oídos. Esas reflexiones se
llaman **reverberación**. Son lo que hace que un espacio "suene grande".

Simular reverb realista requeriría simular la propagación física del sonido
en el espacio — computacionalmente imposible en tiempo real para espacios
grandes.

El algoritmo **Freeverb** (creado por "Jezar at Home" en 1999, dominio
público) usa una aproximación brillante basada en la investigación de
Schroeder y Moorer en los años 60-70:

### Comb Filters (Filtros Peine)

Un comb filter es un delay con feedback. Simula las reflexiones entre
dos superficies paralelas:

```
input → [delay N muestras] → output
            ↑ feedback desde output
```

Con N diferente para cada filtro, y 8 de ellos en paralelo, se obtiene
una densidad de reflexiones muy alta. Los tamaños de delay están elegidos
para que sean **primos entre sí** (evitar periodicidades audibles) y en
rangos de 25-37ms (tiempo típico de primera reflexión en una sala).

```rust
let comb_sizes: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
// ~25-37ms a 44.1kHz
```

Cada comb tiene además un **filtro paso bajo interno** que simula la
absorción de altas frecuencias por el aire y las superficies (las paredes
absorben más los agudos que los graves).

### Allpass Filters

Después de los 8 combs paralelos, la señal pasa por 4 **filtros allpass**
en serie. Un allpass deja pasar todas las frecuencias con la misma amplitud
pero **cambia su fase**: en lugar de retrasarlas uniformemente, retrasa
cada frecuencia una cantidad diferente.

El efecto: **difusión**. Las reflexiones se vuelven más densas y menos
"metálicas". Sin allpass, la reverb suena como un delay con muchos ecos
discretos; con allpass, suena como un espacio acústico continuo.

```rust
// Allpass: output = -input + state + coeff × input
// Deja pasar la energía pero cambia la fase de cada frecuencia
```

---

## 45. Saturación — La Calidez Analógica

Después de la reverb, la señal pasa por `tanh()` aplicado a toda la mezcla.

Un **clipper digital** hace esto:
```
si |x| > 1.0: x = sign(x)   ← corte brusco, suena horrible
```

La saturación `tanh` hace esto:
```
para x pequeño: tanh(x) ≈ x   ← lineal, transparente
para x grande:  tanh(x) → 1   ← satura suavemente, nunca corta
```

```
Amplitud de salida
    │
  1 │          ╭─────────────────
    │        ╱
    │      ╱
    │    ╱
    │  ╱
  0 │╱─────────────────────────── Amplitud de entrada
    │  ↑ zona lineal  ↑ zona de saturación
```

La saturación `tanh` introduce **armónicos pares** (2ª, 4ª, 6ª...) a la
señal. Los armónicos pares suenan musicales y cálidos al oído humano; los
armónicos impares suenan más duros y agresivos. Por eso la saturación
de válvulas (que introduce principalmente pares) se percibe como "cálida",
y la distorsión de transistores (que puede introducir impares) suena más
"fría" o "agresiva".

En nuestro código la saturación sirve también como protección de salida:
ninguna suma de voces, por grande que sea, puede superar la amplitud de
la curva tanh.

---

## 46. DC Blocker — El Filtro Invisible

La saturación asimétrica (cuando la señal pasa más tiempo por encima de 0
que por debajo, o viceversa) puede introducir un **offset de DC** (corriente
continua): el promedio de la señal ya no es exactamente 0.

Esto no se oye directamente, pero:
- Puede saturar el hardware de salida (DAC, amplificador)
- Puede causar problemas al encadenar efectos
- El altavoz se mueve ligeramente de su posición de reposo, reduciendo su
  rango dinámico y aumentando la distorsión

El DC Blocker es un filtro paso alto de frecuencia de corte ~0.7 Hz. Elimina
todo lo que está por debajo de 0.7 Hz (es decir, el DC y variaciones
extremadamente lentas) y deja pasar todo lo audible (por encima de 20 Hz).

Implementación como filtro IIR de primer orden:
```rust
let dc_x = *sample;
*sample = dc_x - self.master_dc_x + MASTER_DC_COEFF * self.master_dc_y;
self.master_dc_x = dc_x;
self.master_dc_y = *sample;
// MASTER_DC_COEFF = 0.9999  →  corte ≈ (1 - 0.9999) × 44100 / (2π) ≈ 0.7 Hz
```

---

## PARTE XI — La Arquitectura del Sistema

## 47. Por Qué el Audio Necesita Su Propio Hilo

El audio digital en tiempo real tiene el requisito más estricto de toda la
programación de sistemas:

**Cada ~5 ms, el sistema necesita un buffer lleno de muestras de audio. Si
no llega, hay un dropout (crujido audible) que no se puede recuperar.**

Comparación con otros sistemas en tiempo real:
- Un videojuego a 60fps tiene 16.7ms para renderizar un frame — si se retrasa
  una vez, el jugador ve un frame perdido (0.016 segundos de problema)
- El audio a 44100Hz con buffers de 256 muestras tiene 5.8ms — si el sistema
  operativo "pausa" el hilo de audio 10ms (lo que es normal en otros
  contextos), hay un click audible

El hilo de audio vive en un mundo con reglas especiales:
- **No puede llamar a funciones que bloqueen:** `Mutex::lock()`, lectura de
  archivos, acceso a red, `malloc()/free()`
- **No puede hacer reservas dinámicas de memoria** (el allocator puede
  bloquear)
- **Debe terminar en tiempo predecible** (sin loops potencialmente infinitos)

Por eso el sintetizador pre-aloca todo:
- `voices: Vec<Voice>` tiene capacidad fija desde el inicio
- `delay_buffer`, `reverb_comb_buffers`: reservados al inicio, nunca
  re-reservados
- `mono_buffer`: pre-alocado para el máximo tamaño de frame

---

## 48. El Triple Buffer — Comunicación Sin Bloqueo

El GUI necesita enviar parámetros al audio. El problema: si el audio espera
por un mutex para leer los parámetros del GUI, puede bloquearse 1ms+ y
causar un dropout.

**Triple Buffer** resuelve esto elegantemente. En lugar de un buffer
compartido (con mutex) o dos buffers (que requieren sincronización), usa tres:

```
Estado: WRITE=0, READ=1, SWAP=2

GUI escribe en buffer[WRITE]:
  buffer[0] = nuevos_params
  SWAP ↔ WRITE:  WRITE=2, READ=1, SWAP=0
  new_data = true

Audio lee:
  si new_data:
    READ ↔ SWAP:  WRITE=2, READ=0, SWAP=1
    new_data = false
  leer buffer[READ]  → buffer[0] (los nuevos params)
```

En ningún momento el GUI y el audio tocan el mismo buffer simultáneamente.
No hay contención, no hay espera, no hay locks. Las operaciones de swap
son **atómicas** (garantizadas por el hardware como operaciones indivisibles).

El precio: puede haber un frame de latencia entre que el GUI escribe y el
audio lee. A 44100Hz con buffers de 256 muestras, eso son ~5.8ms. Completamente
imperceptible.

---

## 49. MidiEventQueue — Eventos Discretos vs. Parámetros Continuos

Hay dos tipos de comunicación entre el GUI/MIDI y el audio:

**Parámetros continuos** (cutoff, resonance, LFO rate...): cambian gradualmente,
pueden perderse frames intermedios sin impacto musical. Van por el triple buffer.

**Eventos discretos** (Note On, Note Off, Sustain): deben llegar en orden y
no se pueden perder. Una nota perdida es audiblemente obvia.

Para los discretos, se usa un `MidiEventQueue` con un `Mutex` simple. Es
aceptable porque:
- Los eventos MIDI llegan a velocidad humana (máximo ~100/segundo en uso
  normal)
- El audio accede a la cola **una vez por bloque** (al inicio), no por muestra
- El tiempo de lock es microsegundos — el riesgo de dropout es aceptable

```rust
// Audio thread: una sola adquisición de lock al inicio del bloque
for event in midi_events.drain() {
    match event {
        MidiEvent::NoteOn { note, velocity } => synthesizer.note_on(note, velocity),
        // ...
    }
}
```

---

## 50. Normalización de Voces — Por Qué un Acorde No Explota

Sin normalización, 8 voces a amplitud máxima (+1.0 cada una) sumarían +8.0,
que excede el rango digital y produce clipping severo.

La solución obvia sería dividir entre N voces (por 8). Pero esto haría que
una sola nota sonara mucho más fuerte que un acorde de 8 notas, lo que
suena antinatural.

La solución correcta: dividir entre **√N** (raíz cuadrada del número de voces).

¿Por qué √N? El análisis estadístico de señales: si tienes N fuentes de
señal no correlacionadas (cada voz tiene su propia fase aleatoria), su
energía total crece proporcionalmente a N, pero su amplitud efectiva
(RMS) crece como √N. Por tanto, dividir entre √N mantiene el nivel RMS
constante independientemente del número de voces.

```rust
let active_voice_count = self.voices.iter().filter(|v| v.is_active).count();
let voice_norm = 1.0_f32 / (active_voice_count.max(1) as f32).sqrt();
// ...
*sample *= voice_norm;
```

Resultado: una nota sola y un acorde de 8 notas tienen aproximadamente
el mismo volumen percibido. Correcto musicalmente.

---

## 51. El Arpeggiador

El arpeggiador convierte acordes en secuencias. Cuando está activo, el
sintetizador no dispara todas las notas del acorde simultáneamente — las
toca de una en una, cíclicamente.

**Rate en BPM:** 120 BPM = 2 corcheas por segundo. El tiempo entre notas
es `60.0 / rate` segundos.

**Gate Length:** qué fracción del tiempo entre notas suena realmente la nota.
Con 0.8, la nota dura el 80% del período y hay un 20% de silencio entre notas.

**Patterns:**
- **Up:** Do3-Mi3-Sol3-Do4-Mi4... (ascendente, saltando octavas si está configurado)
- **Down:** al contrario
- **UpDown:** sube y baja, sin repetir el extremo
- **Random:** orden aleatorio diferente cada ciclo

El arpeggiador también puede sincronizarse con **MIDI Clock**: el sintetizador
recibe pulsos de sincronía de un dispositivo externo (una caja de ritmos, un
DAW) y alinea las notas del arp al tempo del proyecto.

---

## PARTE XII — El Camino Completo, Integrado

## 52. De la Tecla al Altavoz — Todo Junto

Con todo lo aprendido, el camino completo de una nota:

```
╔══════════════════════════════════════════════════════════════╗
║                     HILO MIDI (midir)                        ║
║  Teclado envía: [0x90, 60, 100] = Note On, Do4, fuerza 78%  ║
║  → push(NoteOn { note: 60, velocity: 100 }) en MidiEventQueue ║
╚══════════════════════════════════════════════════════════════╝
                              │
                              ▼
╔══════════════════════════════════════════════════════════════╗
║              HILO AUDIO (cpal callback, cada ~5ms)           ║
║                                                              ║
║  1. drain(MidiEventQueue) → [NoteOn{60,100}]                 ║
║  2. get_params(TripleBuffer) → parámetros actuales           ║
║  3. note_on(60, 100):                                        ║
║     - freq = 261.63 Hz (tabla precalculada)                  ║
║     - vel = 100/127 = 0.787                                  ║
║     - buscar voz libre (o robar la peor candidata)           ║
║     - Voice::new(note=60, freq=261.63, vel=0.787)            ║
║       · phase1/2: fases aleatorias iniciales                 ║
║       · drift_rate: 0.05-0.25 Hz aleatoria                   ║
║       · noise_prng: semilla aleatoria                        ║
║       · envelope_state = Attack                              ║
║                                                              ║
║  4. process_block(&mut buffer[256]):                         ║
║     Por cada una de las 256 muestras:                        ║
║                                                              ║
║     ┌─ LFO ─────────────────────────────────────────────┐   ║
║     │ acc += (lfoFreq/44100) × 2³² (u64, sin drift)     │   ║
║     │ phase = acc / 2³²                                  │   ║
║     │ lfo_val = triangle(phase) × amplitude × (1+modWheel)│  ║
║     └────────────────────────────────────────────────────┘   ║
║                                                              ║
║     Por cada voz activa:                                     ║
║     ┌─ FRECUENCIA ──────────────────────────────────────┐   ║
║     │ glide: freq_actual += (freq_target - freq_actual)  │   ║
║     │         × (1 - glide_coeff)  [exp. interpolation] │   ║
║     │ drift:  ratio = 1 ± 0.0014×sin(drift_phase)       │   ║
║     │ freq1 = base × detune_ratio × drift × pitch_bend  │   ║
║     │       × poly_mod_FE × poly_mod_oscB × lfo_mod     │   ║
║     └────────────────────────────────────────────────────┘   ║
║                                                              ║
║     ┌─ OSCILADORES ─────────────────────────────────────┐   ║
║     │ phase1_acc += freq1/44100 × 2³²                   │   ║
║     │ phase1 = phase1_acc / 2³²                         │   ║
║     │ osc1 = sawtooth(phase1) + polyBLEP(phase1, dt)    │   ║
║     │ osc2 = sawtooth(phase2) + polyBLEP(phase2, dt)    │   ║
║     │        [+ sync: reset phase2 si phase1 wraps]     │   ║
║     │ noise = xorshift32() → IIR Kellett → pink noise   │   ║
║     └────────────────────────────────────────────────────┘   ║
║                                                              ║
║     ┌─ MIXER ───────────────────────────────────────────┐   ║
║     │ mixed = osc1×0.8 + osc2×0.6 + noise×0.0          │   ║
║     └────────────────────────────────────────────────────┘   ║
║                                                              ║
║     ┌─ FILTER ENVELOPE ─────────────────────────────────┐   ║
║     │ Attack:  filt_env = 1 + (filt_env-1) × coeff_atk  │   ║
║     │ Decay:   filt_env = S + (filt_env-S) × coeff_dcy  │   ║
║     │ Sustain: filt_env = S (constante)                 │   ║
║     │ Release: filt_env = filt_env × coeff_rel          │   ║
║     └────────────────────────────────────────────────────┘   ║
║                                                              ║
║     ┌─ CUTOFF MODULADO ─────────────────────────────────┐   ║
║     │ cutoff = base_cutoff                              │   ║
║     │         + lfo_val × lfo_to_cutoff × 1000Hz       │   ║
║     │         + velocity × vel_to_cutoff × 1000Hz      │   ║
║     │         + aftertouch × AT_to_cutoff × 4000Hz     │   ║
║     │         + filt_env × FEA × base_cutoff           │   ║
║     │         + oscB × polymod_osc_b_cutoff × 4000Hz   │   ║
║     │         × kbd_tracking_multiplier                 │   ║
║     │ clamp(20Hz, 20000Hz)                              │   ║
║     └────────────────────────────────────────────────────┘   ║
║                                                              ║
║     ┌─ FILTRO MOOG LADDER (ZDF TPT) ────────────────────┐   ║
║     │ g = tan(π × cutoff/44100)  [pre-warping]          │   ║
║     │ G = g / (1+g)                                     │   ║
║     │ x = tanh(mixed - k × stage4)  [k=resonance]      │   ║
║     │ Etapa 1: v1=G×(x-s1);   y1=v1+s1; s1=y1+v1      │   ║
║     │ Etapa 2: v2=G×(tanh(y1)-s2); y2=v2+s2; s2=y2+v2 │   ║
║     │ Etapa 3: igual con tanh(y2)                       │   ║
║     │ Etapa 4: igual con tanh(y3)                       │   ║
║     │ output = y4 × (1 + k × G⁴)  [compensar ganancia] │   ║
║     │ flush denormals si |s| < 1e-20                    │   ║
║     └────────────────────────────────────────────────────┘   ║
║                                                              ║
║     ┌─ AMP ENVELOPE ────────────────────────────────────┐   ║
║     │ Misma estructura ADSR exponencial                 │   ║
║     └────────────────────────────────────────────────────┘   ║
║                                                              ║
║     ┌─ VCA ──────────────────────────────────────────────┐  ║
║     │ output = filtered                                  │  ║
║     │         × amp_env                                  │  ║
║     │         × (1 + lfo_val × lfo_to_amplitude × 0.5) │  ║
║     │         × (0.5 + velocity × vel_to_amplitude×0.5)│  ║
║     │         × (1 + aftertouch × AT_to_amplitude × 0.5)│  ║
║     └────────────────────────────────────────────────────┘  ║
║                                                              ║
║     sample += voz_output                                     ║
║                                                              ║
║  5. sample × (1/√N_voces)   [normalización RMS]             ║
║  6. sample × master_volume × expression                      ║
║  7. Delay: lectura+escritura en buffer circular              ║
║  8. Reverb Freeverb: 8 combs paralelos + 4 allpass           ║
║  9. sample = tanh(sample)   [saturación suave]               ║
║  10. DC Blocker: HPF 0.7 Hz elimina offset                   ║
║  11. clamp(-1.0, 1.0)                                        ║
║                                                              ║
║  12. Soft Limiter (en AudioEngine):                          ║
║      |x| ≤ 0.8: linear (sin cambio)                         ║
║      |x| > 0.8: 0.8 + 0.2×(1-exp(-5×(|x|-0.8)))           ║
║                                                              ║
║  13. T::from_sample(f32): convertir a formato del hardware   ║
║      (f32, i16, u16 según el DAC)                            ║
║                                                              ║
║  14. Señal → DAC → Cono del altavoz → Aire → Tu oído        ║
╚══════════════════════════════════════════════════════════════╝
```

---

## PARTE XIII — Diseño de Sonidos

## 53. Anatomía de Sonidos Clásicos

Con toda esta comprensión, veamos cómo se construyen sonidos icónicos:

### El Bajo de Techno

```
OSC A:  Sierra,  0 cents
OSC B:  Sierra,  +5 cents  (ligero detune para grosor)
Mixer:  OSC A 100%, OSC B 80%, Ruido 0%
Filtro: Cutoff bajo (200-400 Hz), Resonance media (1.5-2.5)
FiltEnv: Attack 0ms, Decay 150ms, Sustain 30%, Release 50ms
         Amount: 0.8 (el filtro "golpea" en el ataque)
AmpEnv: Attack 5ms, Decay 200ms, Sustain 70%, Release 100ms
LFO:    desactivado
```

El truco: el filter envelope con Decay corto y Amount alto hace que el
filtro se abra dramáticamente en el ataque y luego se cierre — el sonido
"percute" y luego se vuelve oscuro y profundo.

### El Pad de Cuerdas

```
OSC A:  Sierra, 0 cents
OSC B:  Sierra, +7 cents  (detune mayor para chorus rico)
Mixer:  OSC A 80%, OSC B 80%
Filtro: Cutoff 3000 Hz, Resonance 0.5 (suave, sin pico)
FiltEnv: Attack 1s, Decay 0.5s, Sustain 80%, Release 2s
         Amount: 0.3 (abre un poco al inicio)
AmpEnv: Attack 1.5s, Decay 0s, Sustain 100%, Release 3s
LFO:    Triangle, 4Hz, Delay 1s, apuntando a OSC A y OSC B pitch
        Amount 0.15 (vibrato suave que entra gradualmente)
```

El Attack largo del amp envelope es lo que hace que el pad "entre" suavemente.
El detune grande en OSC B crea la sensación de ensemble de cuerdas.

### El Lead Clásico de los 80

```
OSC A:  Cuadrada, PW 50%
OSC B:  Desactivado (o Cuadrada +1 octava)
Filtro: Cutoff 2000 Hz, Resonance 2.0
FiltEnv: Attack 10ms, Decay 300ms, Sustain 50%, Release 200ms
         Amount: 0.6
AmpEnv: Attack 5ms, Decay 0ms, Sustain 100%, Release 150ms
LFO:    Triangle, 5Hz, Delay 0.3s, apuntando a OSC A pitch
        ModWheel controla la profundidad en tiempo real
Voice:  Mono, Legato
Glide:  50ms
```

La cuadrada con algo de resonancia y el filter envelope dan el mordiente
característico. Mono con Legato y glide corto hacen que los solos suenen
vocales y expresivos.

### Campana (Síntesis FM con Poly Mod)

```
OSC A:  Seno, 0 cents
OSC B:  Seno, +700 cents (casi dos octavas, ratio 3.78:1 - inarmónico)
Mixer:  OSC A 70%, OSC B 0% (OSC B solo como modulador)
Filtro: Cutoff alto (12000 Hz), Resonance 0
FiltEnv: Attack 0ms, Decay 0ms, Sustain 0%, Release 0ms
AmpEnv: Attack 0ms, Decay 2s, Sustain 0%, Release 500ms
Poly Mod: OSC B → OSC A freq: 0.8 (FM intensity)
```

OSC B no se oye directamente (nivel 0 en mixer), pero modula la frecuencia
de OSC A. La relación inarmónica 3.78:1 crea sidebands inarmónicos que suenan
metálicos y brillantes — exactamente como una campana. El AmpEnv con Decay
largo deja que la "cola" inarmónica decaiga naturalmente.

---

## PARTE XIV — Glosario Completo

| Término | Explicación completa |
|---------|---------------------|
| **Hz (Hercios)** | Ciclos por segundo. 440 Hz = 440 oscilaciones/segundo = nota La |
| **Frecuencia** | Cuántas veces por segundo vibra algo. Determina el tono |
| **Amplitud** | La "fuerza" de la vibración. Determina el volumen |
| **Timbre** | El "color" sonoro. Definido por la distribución de armónicos |
| **Armónico / Overtone** | Frecuencia múltiplo entera de la fundamental |
| **Serie de Fourier** | Todo sonido periódico = suma de senos a distintas frecuencias |
| **Semitono** | El menor intervalo musical occidental. Una octava = 12 semitonos |
| **Octava** | Intervalo que dobla la frecuencia. Do3→Do4 = octava |
| **Cent** | 1/100 de semitono. Para afinaciones muy finas |
| **dB (Decibel)** | Escala logarítmica de volumen. -6dB = mitad de amplitud |
| **Fase** | Posición en el ciclo de una onda (0° a 360°) |
| **Cancelación de fase** | Dos ondas desfasadas 180° se anulan mutuamente |
| **MIDI** | Protocolo de instrucciones musicales digitales (no es audio) |
| **Note On/Off** | Mensajes MIDI para inicio y fin de nota |
| **Velocity** | Fuerza de pulsación de tecla, 0-127 |
| **CC (Control Change)** | Mensaje MIDI para parámetros continuos (knobs, pedales) |
| **Pitch Bend** | Desviación de tono con rueda, ±N semitonos |
| **Aftertouch** | Presión post-pulsación que modula parámetros |
| **Program Change** | Mensaje MIDI para cambiar de preset |
| **VCO** | Voltage Controlled Oscillator. Genera la onda de audio |
| **Phase Accumulator** | Variable que registra la posición en el ciclo del oscilador |
| **Sawtooth / Sierra** | Forma de onda con todos los armónicos. La más brillante |
| **Square / Cuadrada** | Solo armónicos impares. Sonido hueco, tipo clarinete |
| **Triangle / Triangular** | Armónicos impares decayendo como 1/n². Sonido suave |
| **Sine / Seno** | Solo fundamental, sin armónicos. Tono puro |
| **Pulse Width (PW)** | Proporción de tiempo en +1 vs -1 en la onda cuadrada |
| **PWM** | Pulse Width Modulation. Variar el PW con un LFO |
| **Oscillator Sync** | OSC B reinicia su fase cada vez que OSC A completa un ciclo |
| **Detune** | Dos osciladores ligeramente desafinados para sonido más rico |
| **Drift** | Variación lenta y aleatoria de afinación que imita el analógico |
| **Aliasing** | Frecuencias fantasma que aparecen en formas de onda digitales |
| **PolyBLEP** | Técnica para eliminar aliasing suavizando discontinuidades |
| **Síntesis substractiva** | Partir de formas ricas y filtrar los armónicos no deseados |
| **Síntesis FM** | Modular la frecuencia de un oscilador con otro |
| **VCF / Filtro** | Voltage Controlled Filter. Atenúa selectivamente frecuencias |
| **Cutoff** | Frecuencia a partir de la cual el filtro atenúa |
| **Resonancia** | Retroalimentación en el filtro que crea pico en el cutoff |
| **Autooscilación** | Cuando la resonancia es tan alta que el filtro genera su propia nota |
| **24 dB/oct** | Pendiente del filtro. Muy pronunciada (corte agresivo) |
| **Moog Ladder** | Filtro de 4 transistores en cascada. El filtro analógico más icónico |
| **ZDF TPT** | Zero-Delay Feedback, Topology-Preserving Transform. Método de digitalización |
| **tanh** | Función de saturación suave que introduce armónicos pares "cálidos" |
| **Passband** | Rango de frecuencias que pasa el filtro con poca atenuación |
| **Keyboard Tracking** | El cutoff del filtro sigue proporcionalmente al tono de la nota |
| **ADSR** | Attack, Decay, Sustain, Release. Las cuatro fases del envelope |
| **Envelope** | Curva que define cómo evoluciona un parámetro en el tiempo |
| **Curva RC** | Curva exponencial de carga/descarga de un condensador. Base del ADSR |
| **Retrigger** | Reiniciar el envelope al re-pulsar una nota |
| **LFO** | Low Frequency Oscillator. Modula parámetros a frecuencias sub-audibles |
| **Vibrato** | LFO modulando el pitch de los osciladores |
| **Tremolo** | LFO modulando el volumen (amplitud) |
| **Sample & Hold** | Forma de onda del LFO que salta aleatoriamente a intervalos |
| **Mod Wheel** | Rueda CC1 que escala la profundidad del LFO en tiempo real |
| **Modulación** | Usar un valor variable para modificar otro parámetro en tiempo real |
| **Amount / Depth** | Cuánto afecta una fuente de modulación a su destino |
| **Matriz de modulación** | Sistema que define qué fuentes conectan a qué destinos y con qué intensidad |
| **LFO Delay** | Fade-in gradual del LFO desde el inicio de la nota |
| **VCA** | Voltage Controlled Amplifier. Multiplica la señal por el envelope |
| **Polifonía** | Número de notas que pueden sonar simultáneamente |
| **Voz** | Instancia completa de la cadena de síntesis (osciladores+filtro+VCA) |
| **Voice Stealing** | Liberar una voz activa para asignarla a una nueva nota |
| **Poly** | Modo polifónico: múltiples notas con voces independientes |
| **Mono** | Modo monofónico: una sola nota activa |
| **Legato** | Mono sin retrigger de envelopes al cambiar de nota |
| **Unison** | Todas las voces tocan la misma nota, con detune distribuido |
| **Unison Spread** | Cantidad de desafinación total entre voces en modo Unison |
| **Note Stack** | Lista de teclas pulsadas actualmente para gestión mono/legato |
| **Note Priority** | Regla para elegir nota en Mono: Last/Low/High |
| **Glide / Portamento** | Deslizamiento exponencial de frecuencia entre notas |
| **Poly Mod** | Modulación por voz: filter envelope y OSC B modulan OSC A |
| **FM Synthesis** | Modular la frecuencia de un oscilador con la salida de otro |
| **Sideband** | Frecuencia adicional creada por modulación FM: fA ± n·fB |
| **Arpeggiator** | Toca automáticamente las notas del acorde en secuencia |
| **BPM** | Beats Per Minute. Tempo de la música |
| **Gate Length** | Proporción de tiempo que suena cada nota en el arpeggiador |
| **Delay (efecto)** | Eco: mezcla de la señal original con una versión retrasada |
| **Buffer circular** | Estructura de datos que implementa una línea de retardo |
| **Feedback** | Realimentación: parte de la salida vuelve al input |
| **Reverb** | Simulación del espacio acústico con miles de reflexiones |
| **Freeverb** | Algoritmo de reverb de Jezar: 8 combs + 4 allpass |
| **Comb Filter** | Filtro peine: delay con feedback. Simula reflexiones entre superficies |
| **Allpass Filter** | Pasa todas las frecuencias con la misma amplitud pero cambia la fase |
| **Difusión** | Hacer las reflexiones más densas e irregulares (trabajo de los allpass) |
| **Saturación** | Compresión suave de señales fuertes. Introduce armónicos "cálidos" |
| **DC Blocker** | Filtro paso alto a ~0.7 Hz que elimina offset de corriente continua |
| **Soft Limiter** | Limita la amplitud máxima con curva suave en lugar de corte brusco |
| **Sample Rate** | Muestras de audio por segundo. 44100 Hz = estándar CD |
| **Nyquist** | Teorema: para representar f Hz, necesitas muestrear a ≥ 2f Hz |
| **DAC** | Digital to Analog Converter. Convierte números en voltaje |
| **Buffer de audio** | Bloque de muestras procesadas de una vez (~256 muestras = 5.8ms) |
| **Dropout** | Silencio o clic audible por no entregar audio a tiempo |
| **Hilo (Thread)** | Proceso de ejecución paralelo dentro del programa |
| **Mutex** | Mecanismo de exclusión mutua. Un lock que puede causar esperas |
| **Lock-free** | Algoritmo de sincronización sin locks, usando operaciones atómicas |
| **Triple Buffer** | Tres copias del estado para comunicación sin bloqueo entre hilos |
| **Atómico** | Operación indivisible garantizada por el hardware |
| **Normalización RMS** | Dividir entre √N voces para mantener volumen constante |
| **Denormal** | Número flotante muy pequeño que la CPU procesa lentamente |
| **Aproximación Padé** | Fracción racional que aproxima tanh eficientemente |
| **LUT** | Lookup Table. Tabla precalculada para evitar operaciones costosas |
| **xorshift32** | Generador de números pseudoaleatorios extremadamente rápido |
| **IIR** | Infinite Impulse Response. Tipo de filtro digital recursivo |
| **Pink Noise** | Ruido con -3dB/octava. Más natural y cálido que el ruido blanco |
| **White Noise** | Ruido con energía uniforme en todas las frecuencias |
| **Pre-warping** | Ajuste de frecuencia al digitalizar un filtro analógico |
