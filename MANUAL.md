# Guía de Uso - Sintetizador Analógico Vintage

## Descripción General

Este sintetizador está inspirado en los sintetizadores analógicos clásicos de los años 70, especialmente el Prophet-5. Es un software de síntesis con procesamiento de audio en tiempo real y soporte MIDI completo.

Para instalación, requisitos del sistema y compilación ver [README.md](README.md).

## Arquitectura del Sintetizador

### Motor de Audio
- **8 Voces de Polifonía** con sistema inteligente de reasignación de voces
- **Frecuencia de Muestreo**: 44.1kHz fija
- **Filtro Ladder 24dB/octava** basado en el modelo Moog mejorado de Huovilainen
- **Auto-oscilación** del filtro a partir de resonancia 3.8

### Osciladores
El sintetizador cuenta con **2 osciladores principales** (A y B):

#### Formas de Onda Disponibles
- **Sawtooth (Diente de Sierra)**: Rica en armónicos, ideal para leads y basses
- **Square (Cuadrada)**: Sonido hueco y característico, con control de ancho de pulso
- **Triangle (Triangular)**: Sonido suave, menos armónicos que sawtooth
- **Sine (Sinusoidal)**: Forma de onda pura, ideal para tonos suaves

#### Controles de Oscilador
- **freq (Detune)**: Afinación fina de -12 a +12 semitonos
- **wave**: Selector de forma de onda
- **pw (Pulse Width)**: Solo disponible en ondas cuadradas (0.1 a 0.9)
- **level**: Nivel/amplitud del oscilador (0.0 a 1.0)
- **sync**: Solo en Oscilador B - sincronización con Oscilador A para efectos de sync

## Sección de Filtro (24dB Ladder)

### Controles Principales
- **Cutoff**: Frecuencia de corte (20Hz a 20kHz)
  - Controla qué frecuencias pasan a través del filtro
  - Valores bajos = sonido más suave/oscuro
  - Valores altos = sonido más brillante
- **Resonance**: Énfasis del filtro (0.0 a 4.0)
  - A partir de 3.8 el filtro se auto-oscila (genera su propio tono)
- **Envelope Amount**: Cantidad de modulación del envelope al filtro (-1.0 a 1.0)
- **Keyboard Tracking**: Qué tanto sigue el filtro al teclado (0.0 a 1.0)
- **Velocity**: Sensibilidad a la velocidad MIDI (0.0 a 1.0)

## Envelopes (ADSR)

El sintetizador tiene **2 envelopes independientes**:

### Amp Envelope (Amplitud)
Controla el volumen de cada nota:
- **A (Attack)**: Tiempo para alcanzar el nivel máximo (0.001s a 2s)
- **D (Decay)**: Tiempo para caer al nivel sustain (0.001s a 3s)  
- **S (Sustain)**: Nivel que mantiene mientras la tecla esté presionada (0.0 a 1.0)
- **R (Release)**: Tiempo para desvanecer después de soltar la tecla (0.001s a 5s)

### Filter Envelope (Filtro)
Controla la modulación del filtro con el mismo formato ADSR.

## LFO (Low Frequency Oscillator)

### Formas de Onda del LFO
- **Triangle**: Modulación suave y continua
- **Square**: Modulación en escalón (on/off)
- **Sawtooth**: Rampa ascendente repetitiva
- **Reverse Sawtooth**: Rampa descendente repetitiva
- **Sample & Hold**: Valores aleatorios sostenidos

### Controles del LFO
- **Rate**: Frecuencia del LFO (0.05Hz a 30Hz)
- **Amount**: Intensidad global de modulación (0.0 a 1.0)
- **Keyboard Sync**: Reinicia el LFO en cada nota nueva

### Destinos de Modulación
- **Filter Cutoff**: Modula la frecuencia de corte del filtro
- **Filter Resonance**: Modula la resonancia del filtro
- **Osc A/B Pitch**: Modula la afinación de los osciladores (vibrato)
- **Amplitude**: Modula el volumen (tremolo)

## Mixer

Controla los niveles de las fuentes sonoras:
- **Oscillator A**: Nivel del oscilador A (0.0 a 1.0)
- **Oscillator B**: Nivel del oscilador B (0.0 a 1.0)
- **Noise**: Nivel de ruido blanco (0.0 a 1.0)

## Efectos

### Reverb
- **Amount**: Cantidad de reverb (0.0 a 1.0)
- **Size**: Tamaño de la sala virtual (0.0 a 1.0)

### Delay
- **Time**: Tiempo de delay (0.01s a 2s)
- **Feedback**: Realimentación del delay (0.0 a 0.95)
- **Amount**: Mezcla del delay (0.0 a 1.0)

## Arpeggiator

### Controles
- **Enable**: Activa/desactiva el arpegiador
- **Rate**: Velocidad en BPM (60 a 240)
- **Pattern**: Patrones de arpegio
  - **Up**: Notas ascendentes
  - **Down**: Notas descendentes  
  - **Up-Down**: Sube y baja
  - **Random**: Orden aleatorio
- **Octaves**: Número de octavas (1 a 4)
- **Gate**: Duración de cada nota (0.1 a 1.0)

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

## Control MIDI

### Mensajes MIDI Soportados
- **Note On/Off**: Reproducción de notas con velocity
- **Sustain Pedal**: Mantiene notas (CC 64)
- **Modulation Wheel**: Modulación adicional (CC 1)

### Mapeo de Control Change (CC)
#### Osciladores
- CC 1/2: Amplitud Osc A/B
- CC 3/4: Detune Osc A/B (-12 a +12 semitonos)
- CC 5/6: Pulse Width Osc A/B

#### Mixer
- CC 7: Nivel Osc A
- CC 8: Nivel Osc B  
- CC 9: Nivel Noise

#### Filtro
- CC 16: Cutoff (20Hz a 20kHz)
- CC 17: Resonance
- CC 18: Envelope Amount
- CC 19: Keyboard Tracking

#### Envelopes
**Filter Envelope:**
- CC 20: Attack
- CC 21: Decay
- CC 22: Sustain
- CC 23: Release

**Amp Envelope:**
- CC 24: Attack
- CC 25: Decay
- CC 26: Sustain
- CC 27: Release

#### LFO
- CC 28: Frequency (0.1Hz a 20Hz)
- CC 29: Amplitude
- CC 30-33: Destinos LFO (>63 = ON)

#### Master & Efectos
- CC 34: Master Volume
- CC 40: Reverb Amount
- CC 41: Reverb Size
- CC 42: Delay Time
- CC 43: Delay Feedback
- CC 44: Delay Amount

#### Arpeggiator
- CC 50: Enable (>63 = ON)
- CC 51: Rate (60-240 BPM)
- CC 52: Pattern (0=Up, 1=Down, 2=Up-Down, 3=Random)
- CC 53: Octaves (1-4)
- CC 54: Gate Length

## Sistema de Presets

### Presets Incluidos (Clásicos)
#### Bass Sounds
- **Moog Bass**: Bass profundo y cálido
- **Acid Bass**: Bass acid house típico
- **Sub Bass**: Bass sub-sónico
- **Wobble Bass**: Bass con LFO en cutoff

#### Lead Sounds  
- **Supersaw Lead**: Lead potente multi-oscilador
- **Pluck Lead**: Lead percusivo
- **Screaming Lead**: Lead agresivo con alta resonancia
- **Vintage Lead**: Lead clásico de los 80s

#### Pad Sounds
- **Warm Pad**: Pad cálido y envolvente
- **String Ensemble**: Emulación de cuerdas
- **Choir Pad**: Pad tipo coro
- **Glass Pad**: Pad cristalino

#### Brass Sounds
- **Brass Stab**: Stab de metales
- **Trumpet Lead**: Lead tipo trompeta
- **Sax Lead**: Lead tipo saxofón  
- **Flute**: Sonido de flauta

#### FX Sounds
- **Arp Sequence**: Secuencia de arpegio
- **Sweep FX**: Efecto de barrido de filtro
- **Noise Sweep**: Barrido con ruido
- **Zap Sound**: Efecto zap electrónico

### Gestión de Presets
1. **Cargar preset**: Haz clic en el preset deseado en la lista
2. **Guardar preset**: Escribe un nombre y haz clic en "Save"
3. **Preset por defecto**: Usa "save default" y "load default"
4. **Crear presets clásicos**: Usa "create classic presets"

## Ejemplos de Uso

### Ejemplo 1: Crear un Bass Potente
1. Configura Osc A en Sawtooth, level 0.8
2. Configura Osc B en Square, detune -12, level 0.6  
3. Filter: Cutoff 800Hz, Resonance 2.5
4. Filter Envelope: A=0.01, D=0.5, S=0.3, R=0.8
5. Envelope Amount: 0.6

### Ejemplo 2: Crear un Lead Screaming
1. Osc A: Sawtooth, level 1.0
2. Osc B: Square, detune +7, level 0.7
3. Filter: Cutoff 1.5kHz, Resonance 3.9 (auto-oscilación)
4. LFO: Triangle, Rate 6Hz, modula Filter Cutoff 0.4
5. Amp Envelope: A=0.05, D=0.3, S=0.6, R=1.2

### Ejemplo 3: Crear un Pad Atmosférico
1. Osc A: Sawtooth, level 0.6
2. Osc B: Triangle, detune +12, level 0.8
3. Filter: Cutoff 2kHz, Resonance 1.2
4. Reverb: Amount 0.7, Size 0.8
5. Amp Envelope: A=1.5, D=2.0, S=0.7, R=3.0
6. LFO: Triangle, Rate 0.2Hz, modula Amplitude 0.2

## Técnicas Avanzadas

### Auto-Oscilación del Filtro
- Sube Resonance por encima de 3.8
- El filtro generará su propio tono sinusoidal
- Usa Keyboard Tracking para que siga las notas
- Combina con envelope amount negativo para efectos únicos

### Sync entre Osciladores
- Activa "sync" en Oscilador B
- Oscilador B se sincroniza con A, creando armónicos complejos
- Modula el detune de B con LFO para efectos dinámicos

### Modulación Compleja con LFO
- Usa Sample & Hold para modulación aleatoria
- Combina múltiples destinos para movimiento complejo
- Keyboard sync crea efectos rítmicos sincronizados

### Técnicas de Velocity
- Mapea velocity a cutoff para filtros expresivos
- Combina con velocity a amplitude para dinámicas realistas

## Consejos de Rendimiento

1. **Compilación Release**: Siempre usa `cargo run --release` para mejor rendimiento
2. **Latencia de Audio**: El buffer está optimizado para baja latencia
3. **CPU**: El filtro ladder es computacionalmente intensivo
4. **Polifonía**: 8 voces es el máximo recomendado para estabilidad

## Solución de Problemas

### Audio
- **Sin sonido**: Verifica que la tarjeta de audio esté funcionando
- **Distorsión**: Reduce Master Volume o los niveles de oscilador
- **Latencia alta**: Verifica configuración de buffer de audio del sistema

### MIDI
- **MIDI no detectado**: Conecta el dispositivo antes de ejecutar el programa  
- **Mensajes no reconocidos**: Verifica el mapeo de CC en la documentación
- **Monitor MIDI**: Usa la ventana de monitor para diagnosticar mensajes

### Rendimiento
- **Audio entrecortado**: Cierra otras aplicaciones de audio
- **CPU alto**: Reduce polifonía o efectos
- **Compilación lenta**: Usa `cargo build --release` solo cuando sea necesario

Esta guía cubre todas las funcionalidades principales del sintetizador. Experimenta con los controles para descubrir sonidos únicos y familiarizarte con el comportamiento del sintetizador analógico vintage.