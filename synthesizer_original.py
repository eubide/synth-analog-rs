import numpy as np
import sounddevice as sd
import threading
import time
from typing import Optional
import queue
from scipy import signal

class Synthesizer:
    def __init__(self, sample_rate: int = 44100, buffer_size: int = 512):
        self.sample_rate = sample_rate
        self.buffer_size = buffer_size
        self.is_playing = False
        self.current_frequency = 440.0
        self.amplitude = 0.08
        self.waveform = 'sine'
        self.phase = 0.0
        self.audio_queue = queue.Queue()
        self.stream = None
        
        # Low-pass filter for anti-aliasing
        self.nyquist = sample_rate / 2
        self.cutoff = self.nyquist * 0.4  # Cut at 40% of Nyquist
        self.filter_b, self.filter_a = signal.butter(4, self.cutoff / self.nyquist, btype='low')
        self.filter_zi = signal.lfilter_zi(self.filter_b, self.filter_a)
        
    def apply_envelope(self, wave: np.ndarray, attack: float = 0.05, decay: float = 0.2, 
                      sustain: float = 0.6, release: float = 0.3) -> np.ndarray:
        length = len(wave)
        envelope = np.ones(length)
        
        attack_samples = int(attack * self.sample_rate)
        decay_samples = int(decay * self.sample_rate)
        release_samples = int(release * self.sample_rate)
        
        if attack_samples > 0:
            envelope[:attack_samples] = np.linspace(0, 1, attack_samples)
        
        if decay_samples > 0 and attack_samples + decay_samples < length:
            decay_start = attack_samples
            decay_end = attack_samples + decay_samples
            envelope[decay_start:decay_end] = np.linspace(1, sustain, decay_samples)
        
        if release_samples > 0:
            release_start = max(0, length - release_samples)
            envelope[release_start:] = np.linspace(envelope[release_start], 0, length - release_start)
        
        return wave * envelope

    def band_limited_square(self, t: np.ndarray, frequency: float, harmonics: int = 8) -> np.ndarray:
        wave = np.zeros_like(t)
        for n in range(1, harmonics + 1, 2):  # Only odd harmonics
            harmonic_freq = n * frequency
            if harmonic_freq < self.nyquist * 0.8:  # Band limit
                wave += np.sin(2 * np.pi * harmonic_freq * t) / n
        return wave * (4 / np.pi)

    def band_limited_sawtooth(self, t: np.ndarray, frequency: float, harmonics: int = 16) -> np.ndarray:
        wave = np.zeros_like(t)
        for n in range(1, harmonics + 1):
            harmonic_freq = n * frequency
            if harmonic_freq < self.nyquist * 0.8:  # Band limit
                wave += np.sin(2 * np.pi * harmonic_freq * t) / n
        return wave * (-2 / np.pi)

    def generate_waveform(self, frequency: float, duration: float, waveform: str = 'sine') -> np.ndarray:
        t = np.linspace(0, duration, int(self.sample_rate * duration), False)
        
        if waveform == 'sine':
            wave = np.sin(2 * np.pi * frequency * t)
        elif waveform == 'square':
            wave = self.band_limited_square(t, frequency) * 0.3
        elif waveform == 'triangle':
            wave = signal.sawtooth(2 * np.pi * frequency * t, 0.5) * 0.5
        elif waveform == 'sawtooth':
            wave = self.band_limited_sawtooth(t, frequency) * 0.4
        else:
            wave = np.sin(2 * np.pi * frequency * t)
        
        wave = wave * self.amplitude
        wave = self.apply_envelope(wave)
        
        # Apply gentle smoothing like in continuous mode
        if len(wave) > 3:
            wave = signal.medfilt(wave, kernel_size=3)
        
        wave = np.clip(wave, -0.8, 0.8)  # Soft clipping like continuous mode
        return wave.astype(np.float32)
    
    def generate_continuous_buffer(self, num_frames: int) -> np.ndarray:
        if not self.is_playing:
            return np.zeros(num_frames, dtype=np.float32)
        
        t = np.arange(num_frames) / self.sample_rate
        
        if self.waveform == 'sine':
            phase_increment = 2 * np.pi * self.current_frequency / self.sample_rate
            phases = self.phase + np.arange(num_frames) * phase_increment
            wave = np.sin(phases)
            self.phase = (self.phase + num_frames * phase_increment) % (2 * np.pi)
        elif self.waveform == 'square':
            wave = self.band_limited_square(t, self.current_frequency) * 0.25
        elif self.waveform == 'triangle':
            wave = signal.sawtooth(2 * np.pi * self.current_frequency * t, 0.5) * 0.4
        elif self.waveform == 'sawtooth':
            wave = self.band_limited_sawtooth(t, self.current_frequency) * 0.3
        else:
            wave = np.sin(2 * np.pi * self.current_frequency * t)
        
        # Apply gentle low-pass filtering for smoothness
        if len(wave) > 3:
            wave = signal.medfilt(wave, kernel_size=3)
        
        wave = wave * self.amplitude
        wave = np.clip(wave, -0.8, 0.8)  # Soft clipping
        
        return wave.astype(np.float32)

    def audio_callback(self, outdata, frames, time, status):
        if status:
            print(status)
        
        audio_data = self.generate_continuous_buffer(frames)
        outdata[:] = audio_data.reshape(-1, 1)

    def play_note(self, frequency: float, duration: float = 1.0, waveform: str = 'sine'):
        audio_data = self.generate_waveform(frequency, duration, waveform)
        sd.play(audio_data, self.sample_rate)
        sd.wait()
    
    def start_continuous_tone(self, frequency: float, waveform: str = 'sine'):
        self.stop_continuous_tone()
        
        self.current_frequency = frequency
        self.waveform = waveform
        self.is_playing = True
        self.phase = 0.0
        
        self.stream = sd.OutputStream(
            samplerate=self.sample_rate,
            channels=1,
            callback=self.audio_callback,
            blocksize=self.buffer_size,
            dtype=np.float32,
            latency='low'
        )
        self.stream.start()
    
    def stop_continuous_tone(self):
        self.is_playing = False
        if self.stream:
            self.stream.stop()
            self.stream.close()
            self.stream = None
    
    def set_amplitude(self, amplitude: float):
        self.amplitude = max(0.0, min(1.0, amplitude))
    
    def set_frequency(self, frequency: float):
        self.current_frequency = frequency
    
    def set_waveform(self, waveform: str):
        if waveform in ['sine', 'square', 'triangle', 'sawtooth']:
            self.waveform = waveform

def note_to_frequency(note: str, octave: int = 4) -> float:
    note_frequencies = {
        'C': 261.63, 'C#': 277.18, 'Db': 277.18,
        'D': 293.66, 'D#': 311.13, 'Eb': 311.13,
        'E': 329.63, 'F': 349.23, 'F#': 369.99,
        'Gb': 369.99, 'G': 392.00, 'G#': 415.30,
        'Ab': 415.30, 'A': 440.00, 'A#': 466.16,
        'Bb': 466.16, 'B': 493.88
    }
    
    base_frequency = note_frequencies.get(note.upper(), 440.0)
    return base_frequency * (2 ** (octave - 4))

if __name__ == "__main__":
    synth = Synthesizer()
    
    print("Sintetizador Python")
    print("Comandos disponibles:")
    print("  play <nota> [octava] [duracion] [forma_onda] - Reproducir una nota")
    print("  start <nota> [octava] [forma_onda] - Iniciar tono continuo")
    print("  stop - Detener tono continuo")
    print("  amp <valor> - Cambiar amplitud (0.0-1.0)")
    print("  quit - Salir")
    print("Formas de onda: sine, square, triangle, sawtooth")
    
    while True:
        try:
            command = input("> ").strip().split()
            if not command:
                continue
                
            if command[0] == 'quit':
                synth.stop_continuous_tone()
                break
            elif command[0] == 'play':
                note = command[1] if len(command) > 1 else 'A'
                octave = int(command[2]) if len(command) > 2 else 4
                duration = float(command[3]) if len(command) > 3 else 1.0
                waveform = command[4] if len(command) > 4 else 'sine'
                
                freq = note_to_frequency(note, octave)
                synth.play_note(freq, duration, waveform)
                
            elif command[0] == 'start':
                note = command[1] if len(command) > 1 else 'A'
                octave = int(command[2]) if len(command) > 2 else 4
                waveform = command[3] if len(command) > 3 else 'sine'
                
                freq = note_to_frequency(note, octave)
                synth.start_continuous_tone(freq, waveform)
                
            elif command[0] == 'stop':
                synth.stop_continuous_tone()
                
            elif command[0] == 'amp':
                if len(command) > 1:
                    amp = float(command[1])
                    synth.set_amplitude(amp)
                    print(f"Amplitud establecida a {amp}")
                    
        except KeyboardInterrupt:
            synth.stop_continuous_tone()
            break
        except EOFError:
            synth.stop_continuous_tone()
            break
        except Exception as e:
            print(f"Error: {e}")