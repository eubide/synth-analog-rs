# Cómo Funciona un Sintetizador — De Cero

Una guía para entender cada pieza del sintetizador Prophet-5, explicada
desde los fundamentos. No se asume ningún conocimiento previo de electrónica,
audio o música.

---

## Capítulo 1: El Sonido — Qué Es Lo Que Oímos

Antes de entender un sintetizador, hay que entender qué es el sonido.

**El sonido es aire que vibra.** Cuando pulsas una tecla de piano, la cuerda
golpea y vibra, empujando el aire hacia adelante y hacia atrás muy rápido.
Esas oscilaciones viajan por el aire hasta tu oído, que las convierte en señal
nerviosa, y tu cerebro interpreta esa señal como "nota".

Dos propiedades definen cualquier sonido:

### Frecuencia (Hz — Hercios)

La frecuencia es cuántas veces por segundo vibra el aire. Se mide en **Hz
(Hercios)**, que simplemente significa "veces por segundo".

- **440 Hz** = el aire oscila 440 veces por segundo → eso es un La (A4)
- **880 Hz** = 880 veces por segundo → el La una octava más aguda
- **220 Hz** = 220 veces por segundo → el La una octava más grave

El oído humano escucha entre ~20 Hz y ~20.000 Hz. Debajo de 20 Hz no lo
oímos como tono sino como golpes separados; encima de 20.000 Hz deja de ser
audible (los perros sí lo oyen).

> **Clave:** frecuencia más alta = sonido más agudo. Frecuencia más baja =
> sonido más grave.

### Amplitud (Volumen)

La amplitud es cuánto se mueve el aire, es decir, la fuerza de la vibración.
Mayor amplitud = más volumen. En el código, las muestras de audio son números
entre -1.0 y 1.0, donde 0.0 es silencio, 1.0 y -1.0 son el máximo volumen.

### Timbre — Por Qué un Violín No Suena Como una Flauta

Si un violín y una flauta tocan exactamente el mismo La a 440 Hz, los
reconoces al instante como instrumentos distintos. ¿Por qué? Porque la
frecuencia de 440 Hz no viaja sola: arrastra consigo **armónicos**, que son
copias de sí misma a frecuencias múltiplas (880, 1320, 1760 Hz...).

La distribución de esos armónicos —cuáles son fuertes, cuáles débiles— define
el **timbre** o "color" de un sonido. Un sintetizador controla esa
distribución deliberadamente. Eso es exactamente para lo que sirven las formas
de onda y el filtro.

---

## Capítulo 2: MIDI — El Lenguaje entre el Músico y la Máquina

**MIDI no es audio.** Este es el malentendido más común. MIDI es un protocolo
de instrucciones, como un telegrama musical. Cuando pulsas una tecla en un
teclado MIDI, el teclado no envía sonido — envía un mensaje que dice "se pulsó
la nota 60 con fuerza 100".

Es como la diferencia entre una partitura (instrucciones) y un disco de música
(audio). El sintetizador recibe la partitura y produce el audio.

### Note On / Note Off

Cuando pulsas una tecla, el teclado envía tres bytes:

```
[0x90, 60, 100]
 ────  ──  ───
 Tipo  Nota Velocidad
```

- `0x90` significa "Note On en canal 1"
- `60` es el número de nota (cada número = un semitono; 60 = Do central)
- `100` es la velocidad (qué tan fuerte pulsaste, 0-127)

Al soltar la tecla, llega `Note Off` con el mismo número de nota.

> **Analogía:** Note On es como llamar a un músico y decirle "empieza a tocar
> ese La". Note Off es "para de tocar".

### Velocity (Velocidad de Pulsación)

La *velocity* no es qué tan rápido mueves los dedos de izquierda a derecha —
es qué tan fuerte golpeas la tecla. Un teclado MIDI mide el tiempo entre el
primer contacto de la tecla y el contacto final; cuanto más rápido, mayor
velocidad.

Rango: 0–127, donde 0 es un golpe fantasma y 127 es máxima fuerza.

En nuestro sintetizador, la velocity controla principalmente el **volumen** de
la nota y puede también abrir el **filtro** proporcionalmente (tocar más fuerte
= sonido más brillante, igual que en un piano real).

### Control Change (CC) — Los Knobs y Sliders

Los CC son mensajes continuos para controlar parámetros en tiempo real.
Cada CC tiene un número (0–127) que identifica qué parámetro se controla,
y un valor (0–127) que indica la posición del knob.

```
CC #16, valor 64  →  Filter Cutoff al 50%
CC #1,  valor 100 →  Mod Wheel al 78%
```

Es exactamente como los potenciómetros de una mesa de mezclas: giras el knob
físico y el sintetizador recibe un número que dice hasta dónde lo giraste.

### Pitch Bend

La rueda de pitch bend (normalmente una rueda a la izquierda del teclado) envía
valores de 14 bits: 0 en el extremo grave, 16383 en el extremo agudo, 8192 en
el centro. El sintetizador lo normaliza a -1.0..1.0 y lo multiplica por un
rango configurable (por defecto ±2 semitonos).

> **Analogía:** Es como agarrar una guitarra y doblar la cuerda con el dedo
> para subir el tono transitoriamente.

### Aftertouch (Channel Pressure)

Algunos teclados miden la **presión que ejerces después de pulsar** la tecla.
Esto se llama aftertouch. Es un valor continuo 0–127 que puede modular el
filtro, el vibrato, etc.

> **Analogía:** En un instrumento de viento, soplas con más fuerza para dar
> expresividad a la nota. El aftertouch es el equivalente en teclado.

### Sustain Pedal (CC #64)

El pedal de sustain (como el del piano) envía CC #64 con valor >63 al pisarlo
y <64 al soltarlo. Mientras está pisado, las notas no mueren aunque sueltes
las teclas — el sintetizador las mantiene vivas internamente y solo las apaga
cuando sueltas el pedal.

### Program Change — Cambiar de Preset

Un mensaje Program Change le dice al sintetizador "carga el preset número X".
Es como cambiar de instrumento en un teclado de concierto en tiempo real.

---

## Capítulo 3: Los Osciladores (OSC A y OSC B) — La Fuente del Sonido

Un **oscilador** es un circuito (o, en nuestro caso, un cálculo matemático)
que genera una señal que sube y baja de forma repetitiva y periódica.
Es la fuente primaria de sonido de cualquier sintetizador.

> **Analogía perfecta:** Imagina una cuerda de guitarra vibrando. Eso es un
> oscilador analógico. Nuestro código hace lo mismo, pero con números.

El sintetizador tiene **dos osciladores independientes** (OSC A y OSC B). Cada
uno puede tener su propia frecuencia, forma de onda y volumen. Esto permite
crear sonidos más ricos que un solo oscilador.

### Las Formas de Onda — El Timbre Básico

La "forma" en que el oscilador sube y baja determina qué armónicos están
presentes y en qué proporción. Cuatro formas clásicas:

#### Seno (Sine)

La forma más simple posible. Solo existe la frecuencia fundamental, sin
armónicos. Suena "puro", casi artificial, como un tono de teléfono o una
flauta muy suave.

```
   ╭───╮       ╭───╮
  ╱     ╲     ╱     ╲
──         ───         ──
```

#### Sierra (Sawtooth)

Sube linealmente y cae en vertical. Contiene **todos los armónicos** (pares e
impares) decreyendo en amplitud. Es el sonido más "lleno" y brillante de los
cuatro: el clásico sonido de cuerdas de sintetizador, los bajos de techno, los
leads de los años 80.

```
  /|  /|  /|
 / | / | / |
/  |/  |/  |
```

> Por eso el Prophet-5 original tiene sierra como forma por defecto: es la
> materia prima más versátil para esculpir con el filtro.

#### Cuadrada (Square)

Alterna entre +1 y -1 abruptamente. Solo tiene armónicos impares (fundamental,
3ª, 5ª, 7ª...). Suena "hueca" en el centro, similar a un clarinete. La versión
con **ancho de pulso variable** (Pulse Width) puede ir de muy estrecha (fino,
nasal) a muy ancha (profunda y redonda).

```
  ┌──┐  ┌──┐
  │  │  │  │
──┘  └──┘  └──
```

#### Triangular (Triangle)

Como la cuadrada, solo tiene armónicos impares, pero caen mucho más rápido en
amplitud. Resultado: más suave que la cuadrada, casi tan pura como el seno pero
con un poco más de carácter. Sonido flautístico, dulce.

```
  /\    /\
 /  \  /  \
/    \/    \
```

### Pulse Width (Ancho de Pulso)

En la onda cuadrada, el "ancho de pulso" (PW) es qué fracción del ciclo pasa
en +1 versus -1. Al 50% es una cuadrada perfecta. Al 10% es una onda muy
estrecha, muy nasal. Al 90% es casi como una sierra invertida.

Cambiar el PW lentamente (con un LFO, por ejemplo) produce el efecto llamado
**pulse width modulation (PWM)**: ese sonido de cuerda que parece "respirar"
que se escucha en muchísima música electrónica de los 80.

### VCO (Voltage Controlled Oscillator)

En hardware analógico, la frecuencia del oscilador se controla con voltaje.
Más voltaje = frecuencia más alta. De ahí el nombre "VCO". En nuestro código
es todo digital, pero el nombre se conserva por tradición.

### Hz vs. Semitonos vs. Cents

Estas son tres escalas para medir intervalos musicales:

- **Hz:** unidad física. La diferencia en Hz entre notas crece de forma
  no lineal (de Do3 a Do4 son 130 Hz, de Do4 a Do5 son 261 Hz, siempre el
  doble).
- **Semitonos:** la escala musical estándar. Una octava = 12 semitonos. Es
  logarítmica respecto a Hz, que es lo que percibe el oído humano.
- **Cents:** 100 cents = 1 semitono. Es la unidad para afinaciones muy
  pequeñas, por debajo de lo que se oye como nota distinta. Sirve para el
  detune fino.

La fórmula para convertir: `freq × 2^(semitones/12)`. Subir 12 semitonos
dobla la frecuencia (una octava arriba).

### Detune — Dos Osciladores Ligeramente Desafinados

Cuando suenas OSC A y OSC B exactamente a la misma frecuencia, suenan como
uno. Pero si desafinas OSC B unos pocos cents respecto a OSC A, las dos ondas
interfieren entre sí, creando un efecto de **batido** (pulsaciones periódicas
en el volumen) y un sonido mucho más rico, casi "vivo".

> **Analogía:** Dos guitarras tocando el mismo acorde, pero una ligeramente
> desafinada. El resultado es más cálido y animado que una guitarra perfecta.

Esto es el truco básico detrás de los "super saw" de los sintetizadores
modernos: muchas copias del oscilador, cada una ligeramente desafinada.

### VCO Drift — El "Defecto" que lo Hace Sonar Analógico

Los osciladores analógicos reales nunca están perfectamente a la misma
frecuencia segundo tras segundo — la temperatura del transistor cambia y la
frecuencia deriva levemente. Este "defecto" hace que los sintetizadores
analógicos suenen "vivos" comparados con los digitales perfectamente estables.

Nuestro código lo emula: cada voz tiene su propia tasa de deriva aleatoria
entre 0.05 y 0.25 Hz, produciendo una desviación de ±2.5 cents que oscila
lentamente, independiente para cada voz.

### Oscillator Sync

Cuando el sync de OSC B está activo, cada vez que OSC A completa un ciclo
completo, **reinicia la fase de OSC B a cero**. El efecto es que OSC B queda
"forzado" a la frecuencia de OSC A aunque internamente intente ir a otra.

Esto produce armónicos únicos y agresivos que no se pueden conseguir de otra
forma — es el sonido de ese lead de sintetizador que parece un grito o un
Moog pasado por procesadores. Mucho Nirvana, Radiohead, música electrónica
dura.

---

## Capítulo 4: El Mixer — Combinando las Fuentes

El **mixer** es sencillo: toma la salida de OSC A, OSC B y el generador de
ruido, y los mezcla en una sola señal con niveles independientes.

```
OSC A × 0.8  ─┐
OSC B × 0.6  ─┼─► señal mezclada
Ruido × 0.0  ─┘
```

### Ruido Rosa (Pink Noise)

El ruido es una señal completamente aleatoria. Hay varios tipos:

- **Ruido blanco:** todas las frecuencias en igual cantidad. Suena como
  "ssssssh" (estática de radio).
- **Ruido rosa:** las frecuencias bajas son más fuertes que las altas,
  cayendo -3 dB por octava. Suena más cálido y natural. Es más parecido a
  como suenan los instrumentos reales. El Prophet-5 original tenía un generador
  de ruido rosa en el circuito.

En el código se genera con un algoritmo PRNG (generador de números pseudoaleatorios) muy rápido (xorshift32) seguido de un filtro IIR que convierte el ruido blanco en rosa.

El ruido añadido al oscilador puede dar cuerpo a percusiones, simular aire de
flauta o crear efectos de viento.

---

## Capítulo 5: El Filtro (VCF) — Esculpiendo el Timbre

Si los osciladores crean el sonido en bruto, el filtro es donde ocurre
la magia. Es el componente más importante de un sintetizador substractivo.

> **Analogía:** Imagina que tienes el sonido completo y que el filtro es como
> unas persianas que dejan pasar o bloquean ciertas franjas de frecuencias.

### Cutoff Frequency (Frecuencia de Corte)

El **cutoff** es la frecuencia a partir de la cual el filtro empieza a
"cortar" el sonido. Un filtro paso bajo (el más común en síntesis) deja pasar
las frecuencias por debajo del cutoff y atenúa las que están por encima.

- Cutoff bajo (200 Hz): solo pasan los graves. Sonido oscuro, apagado.
- Cutoff alto (10.000 Hz): pasan casi todos los armónicos. Sonido brillante.
- Cutoff en movimiento: el sonido "abre" o "cierra". Eso es lo que
  suena en el 90% de los drops de música electrónica.

### Resonance (Resonancia o Q)

La resonancia crea un **pico de amplificación justo en la frecuencia de
corte**. Cuanta más resonancia, más pronunciado es ese pico. Con resonancia
alta, el filtro empieza a "cantar" en esa frecuencia — un sonido muy
característico. Con resonancia máxima (≥4 en nuestro modelo), el filtro
entra en **autooscilación**: genera su propia nota pura sin necesidad de
ningún oscilador.

> **Analogía:** El cutoff es donde pones tu mano en la boca de una guitarra;
> la resonancia es qué tan "boca cerrada" versus "boca abierta" haces con la
> cavidad, amplificando ciertas frecuencias.

### El Filtro Moog Ladder — Por Qué Es Icónico

Robert Moog diseñó en 1965 un filtro basado en cuatro transistores en
cascada. Cada transistor actúa como un filtro de 6 dB/octava, y encadenados
dan 24 dB/octava. Eso significa que por cada octava por encima del cutoff, el
sonido se atenúa 16 veces en energía. Es un corte muy pronunciado y
definido.

Lo que hace especial al ladder de Moog no es solo el corte — es la
saturación no lineal que produce cada transistor. Esa saturación suave
(la función `tanh` en el código) da un color "cálido" imposible de imitar
con un filtro lineal. Todos los sintetizadores analógicos clásicos de los 70
y 80 tienen filtros derivados de ese diseño.

Nuestro código implementa la versión matemática **ZDF TPT** (Zero-Delay
Feedback, Topology-Preserving Transform), que es la técnica moderna para
digitalizar este circuito con precisión máxima.

### 24 dB/Octava — ¿Qué Significa?

El "24 dB por octava" es la **pendiente** del filtro: qué tan agresivamente
corta las frecuencias que están por encima del cutoff.

- 6 dB/oct (1 polo): suave, apenas filtra
- 12 dB/oct (2 polos): moderado, muchos sintetizadores
- 24 dB/oct (4 polos): pronunciado, característico del Moog

Una octava es doblar la frecuencia. Si el cutoff es 1000 Hz, a 2000 Hz el
sonido es 16 veces más débil (24 dB), a 4000 Hz es 256 veces más débil, etc.

### Keyboard Tracking (Seguimiento de Teclado)

El cutoff del filtro puede seguir al tono de la nota que se toca. Si tocas
más agudo, el filtro se abre proporcionalmente. ¿Por qué? Porque en instrumentos
reales, los armónicos de las notas agudas también son más agudos, y un filtro
fijo sonaría demasiado oscuro en el registro grave y demasiado brillante en el
agudo.

Con tracking al 100%, el filtro sigue exactamente la escala — cada octava
arriba, el cutoff también sube una octava.

---

## Capítulo 6: Los Envelopes ADSR — La Forma del Sonido en el Tiempo

Un sonido real no empieza ni termina de golpe (o si lo hace, suena artificial).
Un piano tiene un ataque instantáneo y un decaimiento largo; un violín tiene
un ataque lento y puede sostener la nota indefinidamente; una campana tiene
un ataque instantáneo y un decaimiento muy largo.

El envelope ADSR controla **cómo evoluciona el volumen (y el filtro) a lo
largo del tiempo de una nota**.

```
Volumen
  │
1 │     ╭──────────────╮
  │    ╱│              │╲
S │   ╱ │              │ ╲
  │  ╱  │              │  ╲
  │ ╱   │              │   ╲
0 │╱    │              │    ╲____________
  └─────┴──────────────┴──────────────────► Tiempo
    A       D              S       R
  (Attack)(Decay)       (Sustain)(Release)
         ▲                              ▲
  Pulsas tecla                   Sueltas tecla
```

### Attack (Ataque)

El tiempo que tarda en subir de 0 al máximo volumen. 

- Attack corto (0.01s): la nota arranca de golpe, como una percusión o un bajo punteado.
- Attack largo (2s): la nota va subiendo suavemente, como las cuerdas de una orquesta que entran poco a poco.

### Decay (Caída)

Después de llegar al pico, el sonido baja hasta el nivel de Sustain. El Decay
controla cuánto tarda esa bajada. Un piano tiene un decay largo y natural;
un órgano de tubos no tiene decay (va directo al sustain).

### Sustain (Sostenimiento)

No es un tiempo — es un **nivel** (0 a 1). Es el volumen al que se mantiene
el sonido mientras mantienes la tecla pulsada, después del Decay.

- Sustain 0: la nota muere después del Decay, aunque sigas pulsando (como un piano)
- Sustain 1: la nota se mantiene al máximo mientras pulses (como un órgano)

### Release (Liberación)

El tiempo que tarda en bajar de 0 cuando sueltas la tecla.

- Release corto: la nota para en seco al soltar (percusiones)
- Release largo: la nota se desvanece lentamente al soltar (pad, cuerdas)

### El Segundo Envelope: Para el Filtro

Hay dos envelopes independientes: uno controla el **volumen** (VCA) y otro
controla la **frecuencia de corte del filtro** (VCF). Esto permite, por ejemplo:

- Filtro que "se abre" rápido y luego "cierra" mientras la nota sostiene
  (sonido de sintetizador clásico)
- Nota con volumen sostenido pero brillo que desaparece progresivamente
- Cualquier combinación imaginable

El parámetro **Envelope Amount** del filtro controla cuánto afecta el
envelope al cutoff. Si es 0, el envelope del filtro no hace nada. Si es 1,
el envelope mueve el cutoff en toda su excursión posible.

---

## Capítulo 7: El LFO — Modulación Lenta y Expresividad

**LFO** significa **Low Frequency Oscillator** (Oscilador de Baja Frecuencia).
Es un oscilador como OSC A y OSC B, pero su frecuencia es tan baja (típicamente
0.1–20 Hz) que no la oímos como nota — la usamos para **mover otros
parámetros** cíclicamente.

> **Analogía:** Imagina girar lentamente un knob de volumen de arriba a abajo
> 3 veces por segundo. Eso es lo que hace el LFO, pero automático y musical.

### Aplicaciones Musicales del LFO

| LFO apuntado a... | Efecto musical | Nombre del efecto |
|---|---|---|
| Pitch (tono) de los osciladores | El tono oscila arriba y abajo | **Vibrato** |
| Amplitud (volumen) en el VCA | El volumen pulsa | **Tremolo** |
| Cutoff del filtro | El brillo oscila | **Wah automático** |
| Pulse Width del oscilador cuadrado | Sonido que "respira" | **PWM** |

### Las Formas de Onda del LFO

El LFO puede tener distintas formas que crean diferentes tipos de movimiento:

- **Triángulo:** cambio suave y gradual, arriba y abajo uniformemente.
  El más musical para vibrato y tremolo.
- **Cuadrada:** salta entre dos valores extremos. Para efectos de trémolo
  duro o cambios de tono abruptos (como el efecto "ring" de Game Boy).
- **Sierra (Sawtooth):** sube lentamente y cae en seco. Crea filtros que
  "barren" continuamente hacia arriba y luego resetean.
- **Sierra inversa:** cae lentamente y sube en seco. Lo contrario.
- **Sample & Hold:** cada cierto tiempo elige un valor aleatorio y se queda
  ahí hasta el siguiente. Clásico en música electrónica experimental; produce
  ese sonido de "computadora de ciencia ficción" que va saltando aleatoriamente.

### Keyboard Sync del LFO

Si está activo, cada vez que pulsas una nueva tecla, el LFO reinicia su ciclo
desde cero. Esto hace que el vibrato u otros efectos empiecen siempre en el
mismo punto del ciclo, dando más control y consistencia a la expresión.

### LFO Delay (Retardo del LFO)

El efecto tarda N segundos en subir de 0 a su intensidad completa. Un músico
de cuerda, por ejemplo, no usa vibrato desde el primer instante de la nota —
deja que la nota "entre" limpia y luego añade vibrato gradualmente.
El LFO Delay replica exactamente ese comportamiento.

### Mod Wheel (Rueda de Modulación)

La rueda de modulación (CC #1, la segunda rueda en el teclado, junto al pitch
bend) escala la **profundidad** del LFO en tiempo real. Con el mod wheel en 0
el LFO no se oye aunque esté activo; girándolo hacia arriba, el efecto
aumenta. Esto permite al músico añadir vibrato de forma expresiva mientras
toca, igual que hace un guitarrista con el dedo.

---

## Capítulo 8: El VCA — El Amplificador Controlado

**VCA** significa **Voltage Controlled Amplifier** (Amplificador Controlado
por Voltaje). En términos simples: multiplica la señal del filtro por un número
entre 0 y 1.

- Si el envelope está en 0 → silencio
- Si el envelope está en 1 → señal completa
- Si el envelope está en 0.5 → señal a la mitad de volumen

Cuatro cosas modulan el VCA simultáneamente:

1. **Amp Envelope (ADSR):** la forma principal del volumen en el tiempo
2. **LFO amplitude mod:** el tremolo
3. **Velocity mod:** notas tocadas con más fuerza suenan más fuerte
4. **Aftertouch amplitude:** presionar más fuerte la tecla sube el volumen

---

## Capítulo 9: La Polifonía y las Voces

### ¿Qué Es una Voz?

Una **voz** es una instancia completa del camino de síntesis: osciladores +
filtro + envelopes + VCA. Es el equivalente a un músico de la orquesta.

Nuestro sintetizador tiene **8 voces**, igual que el Prophet-5 original. Esto
significa que puede sonar hasta 8 notas simultáneas.

> **Analogía:** El sintetizador tiene 8 músicos sentados. Cuando pulsas una
> tecla, le das instrucciones a un músico disponible. Cuando pulsas 8 teclas
> a la vez, los 8 músicos tocan. Si quieres pulsar una novena, tienes que
> "robar" a uno de ellos.

### Voice Stealing (Robo de Voz)

Cuando todas las voces están ocupadas y llega una nota nueva, hay que liberar
una. El algoritmo elige la menos "importante" usando una puntuación:

1. Las voces en fase de Release (ya soltadas) tienen prioridad para ser robadas
2. Entre esas, la más silenciosa
3. Si no hay ninguna en Release, la que lleva más tiempo sonando

Así el robo de voz es lo más imperceptible posible.

### Modos de Voz

#### Poly (Polifónico)
El modo normal. Hasta 8 notas simultáneas, cada una en su voz independiente.

#### Mono (Monofónico)
Solo suena 1 nota a la vez. Si pulsas una segunda nota mientras la primera
sigue sonando, la nueva reemplaza a la vieja. Un stack de notas recuerda qué
teclas sigues pulsando, y si sueltas la más reciente, vuelve a la anterior.

El **Note Priority** define qué nota "gana" cuando hay varias pulsadas:
- **Last:** la más reciente (más intuitivo para melodía)
- **Low:** la más grave (para bajo)
- **High:** la más aguda (para treble)

#### Legato
Como Mono, pero al cambiar de nota sin soltar, **los envelopes no se
reinician**. La nota cambia de frecuencia pero el volumen y el filtro siguen
su curso sin ese "golpe" inicial del Attack. Muy natural para solos de
sintetizador, imita la ligadura de los instrumentos de viento.

#### Unison
Todas las 8 voces tocan la misma nota, pero cada una **ligeramente
desafinada** respecto a las demás. El resultado es un sonido enormemente
grueso, como si hubiera un coro de sintetizadores tocando al unísono (de ahí
el nombre). El **Unison Spread** controla cuántos cents de separación hay
entre la voz más baja y la más alta.

### Glide / Portamento

Cuando el glide está activo, al pasar de una nota a otra el tono no salta
directamente — **desliza progresivamente** de la frecuencia anterior a la
nueva en un tiempo configurable. Exactamente como un trombón que desliza
entre notas, o como un violinista que hace un portamento.

Técnicamente se implementa con interpolación exponencial: en cada muestra,
la frecuencia actual se mueve un porcentaje hacia la frecuencia objetivo
(en lugar de un paso fijo), lo que da un deslizamiento que parece natural
al oído.

---

## Capítulo 10: Poly Mod — Modulación Polifónica del Prophet-5

El **Poly Mod** es una característica única del Prophet-5 original. Permite
que el **envelope del filtro** y la **salida de OSC B** de cada voz modulen
parámetros de esa misma voz de forma individual.

Es "polifónica" porque cada voz se modula a sí misma de forma independiente,
no hay un único LFO global.

### Filter Envelope → Frecuencia de OSC A

El envelope del filtro (que ya controla el cutoff) puede también desviar el
**tono de OSC A**. Si el envelope sube, el tono sube. Esto crea efectos de
ataque dramáticos donde la nota "grita" hacia arriba al inicio y luego se
asienta.

### OSC B → Frecuencia / Pulse Width de OSC A

La salida de OSC B puede modular el **tono o el pulse width de OSC A** en
tiempo real, a la frecuencia de OSC B. Esto es esencialmente **FM synthesis**
(síntesis por modulación de frecuencia) y **PM synthesis** dentro de un
sintetizador substractivo, capaz de crear timbres muy complejos e inarmónicos.

> **Analogía:** es como si el segundo músico de la orquesta, en lugar de
> tocar su propia parte, empujara y jalara del instrumento del primero
> a su propio ritmo.

### OSC B → Cutoff del Filtro

OSC B puede también modular directamente el **cutoff del filtro**. A baja
frecuencia de OSC B (casi LFO), es un wah manual; a alta frecuencia,
produce texturas complejas de síntesis de banda lateral.

---

## Capítulo 11: El Arpeggiador

El arpeggiador es un **secuenciador automático de notas**. Si pulsas un
acorde con el arp activo, en lugar de sonar todas las notas a la vez,
el sintetizador las va tocando una a una en un patrón y a un tempo configurable.

Patrones disponibles:

- **Up (ascendente):** Do-Mi-Sol-Do-Mi-Sol...
- **Down (descendente):** Sol-Mi-Do-Sol-Mi-Do...
- **Up-Down:** Do-Mi-Sol-Mi-Do-Mi-Sol...
- **Random:** orden aleatorio cada vez

**Rate:** la velocidad, en BPM (pulsaciones por minuto). 120 BPM = 2 notas por segundo.

**Gate Length:** cuánto dura cada nota (0.1 = muy staccato, 0.9 = casi legato).

**Octaves:** el arp puede repetir el patrón subiendo octavas. Con 2 octavas
y el acorde Do-Mi-Sol: Do-Mi-Sol-Do-Mi-Sol (octava arriba)-Do-Mi-Sol...

---

## Capítulo 12: Los Efectos

Los efectos procesan la señal ya sintetizada para añadir espacio y carácter.

### Delay (Eco)

El delay graba la señal en un buffer circular y la reproduce N milisegundos
después, mezclada con la señal original. El **feedback** devuelve la señal
retardada de vuelta al input del delay, creando ecos que se repiten y se
van apagando progresivamente.

> **Analogía:** gritar en un cañón y oír tu voz repetirse.

- **Delay time:** cuánto tiempo entre el sonido original y el primer eco
- **Feedback:** cuánta señal del eco vuelve al input (más = más repeticiones)
- **Amount (wet):** cuánto del efecto se mezcla con la señal seca

### Reverb (Reverberación)

La reverb simula el **espacio acústico** — las reflexiones de las paredes,
el techo y el suelo de una sala. A diferencia del delay, que produce ecos
discretos, la reverb produce miles de reflexiones muy densas que se
mezclan en un "halo" sonoro continuo.

Nuestro sintetizador usa el algoritmo **Freeverb** de Jezar at Home, que es
el algoritmo de reverb de código abierto más usado del mundo:

- **8 filtros comb en paralelo:** cada uno retrasa la señal una cantidad
  diferente y la retroalimenta, creando densidad. Las longitudes están
  calibradas para evitar coloración tonal.
- **4 filtros allpass en serie:** añaden difusión, haciendo que las reflexiones
  suenen naturales y no metálicas.

- **Amount:** mezcla de reverb con la señal seca
- **Size:** el tamaño de la sala simulada (corto = habitación, largo = catedral)

### Saturación (Tanh)

La saturación aplica la función matemática **tanh** a toda la señal. La tanh
tiene la propiedad de ser casi lineal para señales pequeñas, pero comprime y
dobla suavemente las señales grandes en lugar de cortarlas abruptamente.

> **Analogía:** un amplificador de tubo (válvulas) que cuando lo fuerzas no
> "parte" la señal sino que la "dobla" suavemente, añadiendo armónicos pares
> que suenan cálidos y musicales. Es el sonido de "calidez analógica".

A diferencia de un **clipper** digital (que corta la onda como una guillotina),
la saturación tanh produce distorsión armónica suave y gradual.

### DC Blocker

Un pequeño problema de los filtros analógicos con resonancia alta y de la
saturación asimétrica: pueden introducir una **componente de corriente
continua** (DC offset) — es decir, la onda promedia un valor distinto de 0.
Esto no se oye directamente pero puede saturar el hardware de salida o causar
problemas al encadenar efectos.

El DC Blocker es un filtro paso alto muy suave (~0.7 Hz) que elimina ese
offset sin tocar nada audible. Es invisible para el oído pero necesario para
la estabilidad del sistema.

### Soft Limiter (Limitador Suave)

La última línea de defensa antes del DAC. Si después de todo el procesado
la señal supera el rango ±1.0 (el máximo digital), el limitador la comprime
suavemente. Entre -0.8 y 0.8 es completamente lineal y transparente; por
encima de 0.8 aplica una curva exponencial que la acerca asintóticamente a
±1 sin jamás cortarla en seco.

---

## Capítulo 13: El Mundo Digital — Conceptos de Implementación

### Sample Rate (Frecuencia de Muestreo)

El audio digital no es una onda continua — son una serie de **muestras**
(snapshots numéricos) tomadas a intervalos regulares. La frecuencia de muestreo
es cuántas muestras se toman por segundo.

Nuestro sintetizador usa **44.100 Hz** (el estándar de CD). Eso significa que
por cada segundo de audio, el programa calcula 44.100 números.

El teorema de Nyquist dice que para representar fielmente una frecuencia, la
frecuencia de muestreo debe ser al menos el doble. A 44.100 Hz, podemos
representar hasta 22.050 Hz — suficiente para el oído humano.

### DAC (Digital to Analog Converter)

El DAC convierte los números del software en voltaje eléctrico real que mueve
el altavoz. Es el puente entre el mundo digital y el mundo físico.

### Buffer

El audio se procesa en bloques llamados **buffers** en lugar de muestra a
muestra. Un buffer típico contiene ~256–1024 muestras (~6–23 ms de audio).

El sistema de audio (cpal) llama al sintetizador periódicamente pidiéndole que
llene un buffer. Si el sintetizador no llena el buffer a tiempo, hay un
**dropout** (corte o crujido audible). Por eso el procesado de audio tiene
las máximas exigencias de rendimiento: no puede esperar, no puede bloquearse,
no puede hacer peticiones lentas al sistema.

### Phase Accumulator (Acumulador de Fase)

Para generar un oscilador digital, el código mantiene una variable que
representa la **fase** del ciclo (de 0.0 a 1.0 = un ciclo completo).
Cada muestra, se incrementa esa variable en `frecuencia / sample_rate`.
Cuando llega a 1.0, wrappea a 0.0 y empieza el siguiente ciclo.

Nuestro código usa un entero `u64` para este acumulador en lugar de un
`f32`. Los números flotantes acumulan pequeños errores de redondeo con cada
suma; después de millones de sumas (notas largas), el error acumulado produce
"drift" de fase audible. El entero de 64 bits wrappea perfectamente sin
nunca acumular error.

### PolyBLEP / PolyBLAMP — Anti-Aliasing

Las ondas de sierra y cuadrada tienen **discontinuidades** — puntos donde la
onda "salta" de valor instantáneamente. En analógico eso no es problema, pero
en digital produce **aliasing**: frecuencias fantasma indeseadas que suenan
como distorsión metálica, especialmente en notas agudas.

**PolyBLEP** (Polynomial Band-Limited Step) es un pequeño ajuste matemático
aplicado justo en la discontinuidad que la suaviza lo suficiente para eliminar
el aliasing sin necesitar un oversampling costoso (calcular todo a 4× la
frecuencia y luego bajar).

### Denormal Flush

Los números de punto flotante IEEE 754 tienen un rango especial de valores muy
pequeños llamados **denormales** (números por debajo del mínimo normalizado). Las
operaciones con denormales pueden ser hasta **100 veces más lentas** en
algunas CPUs porque el hardware las maneja en software. En las colas de silencio
de los filtros, los valores decaen exponencialmente hasta entrar en ese rango.

El código detecta cuando un valor del filtro cae por debajo de 10⁻²⁰ y lo
pone directamente a 0.0. Eso previene el slowdown sin efecto audible.

### Mutex vs. Lock-Free

Un **mutex** es un candado: el hilo que quiere acceder a datos compartidos
espera hasta que nadie más los esté usando. Si el hilo de audio espera aunque
sea 1 ms por un mutex, eso produce un dropout audible.

La solución es diseño **lock-free**: estructuras de datos que permiten leer y
escribir desde múltiples hilos sin candados, usando operaciones atómicas del
procesador.

Nuestro sintetizador usa un **triple buffer**: tres copias del estado de
parámetros. El GUI escribe en una copia, el audio lee de otra, y la tercera
está "disponible" para el siguiente intercambio. Nunca hay contención.

---

## Capítulo 14: El Flujo Completo — Ahora Todo Encaja

Con todo esto en mente, el camino completo de una nota suena así:

```
1. Pulsas una tecla en el teclado MIDI
   → El teclado mide la velocidad del golpe
   → Envía bytes MIDI: [Note On, nota 60, velocity 100]

2. El hilo MIDI recibe los bytes
   → Los decodifica y crea un evento NoteOn{60, 100}
   → Lo encola en MidiEventQueue (sin bloquear al audio)

3. El hilo de audio, cada ~5 ms, hace su turno:
   → Vacía la cola MIDI y ejecuta note_on(60, 100)
   → Asigna una voz libre (o roba la menos importante)
   → Inicializa la voz: freq=261.63 Hz, vel=0.787, envelope=Attack

4. Para cada una de las 44.100 muestras de ese segundo:
   a. El LFO avanza su ciclo (u64, sin drift)
   
   b. Para cada voz activa:
      - Glide: desliza la frecuencia si hay portamento
      - Drift: añade ±2.5 cents de variación analógica aleatoria
      - Calcula frecuencias finales de OSC A y OSC B incluyendo:
        detune, LFO, Poly Mod, Pitch Bend
      - OSC A genera una muestra de onda sierra con PolyBLEP
      - OSC B genera una muestra de onda sierra con PolyBLEP
      - Generador de ruido rosa produce una muestra
      - Mixer combina los tres con sus niveles
      - Filter Envelope avanza su ADSR → valor del envelope de filtro
      - Cutoff final = base + envelope_filtro + LFO + velocidad
                             + keyboard tracking + aftertouch + PolyMod
      - Filtro Moog Ladder (4 etapas TPT) filtra la mezcla
      - Amp Envelope avanza su ADSR → valor del envelope de amplitud
      - VCA multiplica la señal por: envAmp × LFO_amp × velocity × aftertouch
      - La muestra resultante se suma al buffer global
   
   c. Buffer global ÷ √N voces (normalización)
   d. × master_volume × expression
   e. Delay: suma el eco del pasado y guarda para el futuro
   f. Reverb Freeverb: 8 combs + 4 allpass
   g. Saturación tanh (calidez analógica)
   h. DC Blocker HPF 0.7 Hz (estabilidad)
   i. Clamp ±1.0

5. El buffer lleno va al DAC
   → El DAC mueve el cono del altavoz
   → El cono mueve el aire
   → El aire llega a tu oído
   → Tu cerebro dice "La bemol, staccato, con reverb"
```

---

## Glosario Rápido

| Término | Significado en una línea |
|---------|--------------------------|
| **Hz** | Ciclos por segundo; mide frecuencia y tono |
| **Semitono** | El intervalo más pequeño entre notas en música occidental |
| **Cent** | 1/100 de semitono; para afinaciones muy finas |
| **MIDI** | Protocolo de instrucciones musicales, no audio |
| **Velocity** | Fuerza de pulsación de una tecla, 0-127 |
| **CC** | Control Change: mensaje MIDI para knobs y sliders |
| **Aftertouch** | Presión ejercida después de pulsar la tecla |
| **Pitch Bend** | Rueda que desvía el tono arriba o abajo |
| **OSC / VCO** | Oscilador: genera la onda periódica de audio |
| **Sawtooth** | Onda sierra: la más rica en armónicos |
| **Pulse Width** | Ancho de pulso de la onda cuadrada |
| **Sync** | OSC B forzado a reiniciar con cada ciclo de OSC A |
| **Drift** | Variación aleatoria lenta de afinación, imita analógico |
| **VCF / Filtro** | Elimina selectivamente frecuencias del sonido |
| **Cutoff** | Frecuencia a partir de la cual el filtro corta |
| **Resonance** | Pico de amplificación en el cutoff |
| **24 dB/oct** | Qué tan agresivamente corta el filtro |
| **Moog Ladder** | Diseño de filtro de 4 transistores en cascada, icónico |
| **Keyboard Tracking** | El filtro sigue al tono de la nota tocada |
| **ADSR** | Attack, Decay, Sustain, Release: forma temporal de la nota |
| **Envelope** | Curva que controla cómo evoluciona un parámetro en el tiempo |
| **LFO** | Oscilador lento que modula parámetros (vibrato, tremolo...) |
| **Vibrato** | LFO modulando el tono de los osciladores |
| **Tremolo** | LFO modulando el volumen |
| **Mod Wheel** | Rueda que escala la profundidad del LFO en tiempo real |
| **Sample & Hold** | LFO que elige valores aleatorios a intervalos regulares |
| **VCA** | Amplificador controlado: multiplica la señal por el envelope |
| **Polifonía** | Cuántas notas pueden sonar simultáneamente |
| **Voz** | Una instancia completa de osciladores + filtro + VCA |
| **Voice Stealing** | Liberar una voz para asignarla a una nota nueva |
| **Mono** | Solo una voz activa a la vez |
| **Legato** | Mono sin re-triggerizar envelopes al cambiar nota |
| **Unison** | Todas las voces en la misma nota, ligeramente desafinadas |
| **Glide / Portamento** | El tono desliza entre notas en lugar de saltar |
| **Poly Mod** | Modulación donde cada voz se modula a sí misma |
| **Arpeggiator** | Toca automáticamente las notas del acorde en secuencia |
| **Delay** | Eco: la señal retrasada N ms se mezcla con la original |
| **Reverb** | Simulación del espacio acústico de una sala |
| **Saturación** | Compresión suave de la señal; "calidez analógica" |
| **DC Blocker** | Filtro que elimina offset de corriente continua |
| **Sample Rate** | Muestras de audio por segundo (44.100 Hz = calidad CD) |
| **DAC** | Convierte números a voltaje eléctrico para el altavoz |
| **Buffer** | Bloque de muestras procesadas de una vez (~5-23 ms) |
| **Anti-aliasing / PolyBLEP** | Elimina frecuencias fantasma en ondas digitales |
| **Phase Accumulator** | Contador interno que representa el avance del ciclo del oscilador |
| **Lock-free** | Diseño que evita bloqueos entre hilos de ejecución |
| **Triple Buffer** | Tres copias del estado para que GUI y audio nunca esperen |
