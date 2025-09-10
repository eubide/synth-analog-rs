import numpy as np
import sounddevice as sd
import threading
import time
import sys
import select
from typing import Optional, Dict

class Synthesizer:
    def __init__(self, sample_rate: int = 44100, buffer_size: int = 512):
        self.sample_rate = sample_rate
        self.buffer_size = buffer_size
        self.amplitude = 0.08
        self.is_playing = False
        self.current_frequency = 440.0
        self.waveform = 'sine'
        self.phase = 0.0
        self.stream: Optional[sd.OutputStream] = None
        
        # Dual oscillator support
        self.dual_mode = False
        self.osc2_waveform = 'sine'
        self.osc2_detune = 0.0  # Semitones
        self.osc2_mix = 0.5     # 0.0 = only osc1, 1.0 = only osc2
        self.phase2 = 0.0
        
        # Keyboard mode
        self.keyboard_mode = False
        self.pressed_keys: Dict[str, bool] = {}
        self.current_notes: Dict[str, float] = {}
        self.keyboard_thread = None

    def generate_wave(self, frequency: float, num_samples: int, waveform: str = 'sine') -> np.ndarray:
        """Generate any waveform with given frequency and samples"""
        t = np.arange(num_samples) / self.sample_rate
        
        if waveform == 'sine':
            wave = np.sin(2 * np.pi * frequency * t)
        elif waveform == 'square':
            wave = np.sign(np.sin(2 * np.pi * frequency * t)) * 0.6
        elif waveform == 'triangle':
            wave = 2 * np.arcsin(np.sin(2 * np.pi * frequency * t)) / np.pi
        elif waveform == 'sawtooth':
            wave = 2 * (t * frequency - np.floor(t * frequency + 0.5)) * 0.8
        else:
            wave = np.sin(2 * np.pi * frequency * t)
        
        return wave * self.amplitude

    def apply_envelope(self, wave: np.ndarray) -> np.ndarray:
        """Simple ADSR envelope"""
        length = len(wave)
        attack = int(0.05 * self.sample_rate)  # 50ms attack
        release = int(0.2 * self.sample_rate)  # 200ms release
        
        # Attack
        if attack < length:
            wave[:attack] *= np.linspace(0, 1, attack)
        
        # Release  
        if release < length:
            wave[-release:] *= np.linspace(1, 0, release)
        
        return wave

    def play_note(self, frequency: float, duration: float = 1.0, waveform: str = 'sine'):
        """Play a single note"""
        num_samples = int(duration * self.sample_rate)
        wave = self.generate_wave(frequency, num_samples, waveform)
        wave = self.apply_envelope(wave)
        wave = np.clip(wave, -0.8, 0.8).astype(np.float32)
        
        sd.play(wave, self.sample_rate)
        sd.wait()

    def generate_continuous_buffer(self, num_frames: int) -> np.ndarray:
        """Generate continuous audio buffer for streaming"""
        if not self.is_playing:
            return np.zeros(num_frames, dtype=np.float32)
        
        # Generate oscillator 1
        if self.waveform == 'sine':
            phase_increment = 2 * np.pi * self.current_frequency / self.sample_rate
            phases = self.phase + np.arange(num_frames) * phase_increment
            wave1 = np.sin(phases)
            self.phase = (self.phase + num_frames * phase_increment) % (2 * np.pi)
        else:
            wave1 = self.generate_wave(self.current_frequency, num_frames, self.waveform) / self.amplitude
        
        # Generate oscillator 2 if dual mode is enabled
        if self.dual_mode:
            osc2_freq = self.current_frequency * (2 ** (self.osc2_detune / 12.0))
            
            if self.osc2_waveform == 'sine':
                phase_increment2 = 2 * np.pi * osc2_freq / self.sample_rate
                phases2 = self.phase2 + np.arange(num_frames) * phase_increment2
                wave2 = np.sin(phases2)
                self.phase2 = (self.phase2 + num_frames * phase_increment2) % (2 * np.pi)
            else:
                wave2 = self.generate_wave(osc2_freq, num_frames, self.osc2_waveform) / self.amplitude
            
            # Mix oscillators
            wave = wave1 * (1 - self.osc2_mix) + wave2 * self.osc2_mix
        else:
            wave = wave1
        
        wave = wave * self.amplitude
        return np.clip(wave, -0.8, 0.8).astype(np.float32)

    def audio_callback(self, outdata, frames, time, status):
        """Audio callback for continuous playback"""
        if status:
            print(f"Audio status: {status}")
        
        audio_data = self.generate_continuous_buffer(frames)
        outdata[:] = audio_data.reshape(-1, 1)

    def start_continuous_tone(self, frequency: float, waveform: str = 'sine'):
        """Start continuous tone"""
        self.stop_continuous_tone()
        
        self.current_frequency = frequency
        self.waveform = waveform
        self.is_playing = True
        self.phase = 0.0
        self.phase2 = 0.0
        
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
        """Stop continuous tone"""
        self.is_playing = False
        if self.stream:
            self.stream.stop()
            self.stream.close()
            self.stream = None

    def set_amplitude(self, amplitude: float):
        """Set amplitude (0.0 to 1.0)"""
        self.amplitude = max(0.0, min(1.0, amplitude))
    
    def enable_dual_oscillator(self, osc2_waveform: str = 'square', detune: float = 7.0, mix: float = 0.5):
        """Enable dual oscillator mode"""
        self.dual_mode = True
        self.osc2_waveform = osc2_waveform
        self.osc2_detune = detune
        self.osc2_mix = mix
    
    def disable_dual_oscillator(self):
        """Disable dual oscillator mode"""
        self.dual_mode = False
    
    def get_key_mapping(self) -> Dict[str, str]:
        """Map computer keys to musical notes"""
        return {
            'a': 'C', 's': 'C#', 'd': 'D', 'f': 'D#', 'g': 'E', 'h': 'F',
            'j': 'F#', 'k': 'G', 'l': 'G#', 'ñ': 'A', "'": 'A#', 'enter': 'B',
            'q': 'C', 'w': 'C#', 'e': 'D', 'r': 'D#', 't': 'E', 'y': 'F',
            'u': 'F#', 'i': 'G', 'o': 'G#', 'p': 'A', '[': 'A#', ']': 'B'
        }
    
    def play_melody(self, melody_notes: str, note_duration: float = 0.5):
        """Play a sequence of notes automatically"""
        note_map = {
            'a': ('C', 4), 's': ('C#', 4), 'd': ('D', 4), 'f': ('D#', 4), 
            'g': ('E', 4), 'h': ('F', 4), 'j': ('F#', 4), 'k': ('G', 4), 
            'l': ('G#', 4), 'n': ('A', 4), 'm': ('A#', 4), 'b': ('B', 4),
            'A': ('C', 5), 'S': ('C#', 5), 'D': ('D', 5), 'F': ('D#', 5),
            'G': ('E', 5), 'H': ('F', 5), 'J': ('F#', 5), 'K': ('G', 5),
            'L': ('G#', 5), 'N': ('A', 5), 'M': ('A#', 5), 'B': ('B', 5),
            '-': ('rest', 0)  # Silencio
        }
        
        print(f"🎵 Tocando melodía: {melody_notes}")
        for note_char in melody_notes:
            if note_char in note_map:
                note, octave = note_map[note_char]
                if note == 'rest':
                    time.sleep(note_duration)
                    print("🔇", end=" ", flush=True)
                else:
                    freq = note_to_frequency(note, octave)
                    self.play_note(freq, note_duration, self.waveform)
                    print(f"{note}{octave}", end=" ", flush=True)
            else:
                print(f"❌{note_char}", end=" ", flush=True)
        print("\n🎵 Melodía terminada")
    
    def stop_keyboard_mode(self):
        """Stop keyboard input mode"""
        self.keyboard_mode = False
        self.stop_continuous_tone()

def note_to_frequency(note: str, octave: int = 4) -> float:
    """Convert note name to frequency"""
    notes = {
        'C': 261.63, 'C#': 277.18, 'DB': 277.18,
        'D': 293.66, 'D#': 311.13, 'EB': 311.13,
        'E': 329.63, 'F': 349.23, 'F#': 369.99,
        'GB': 369.99, 'G': 392.00, 'G#': 415.30,
        'AB': 415.30, 'A': 440.00, 'A#': 466.16,
        'BB': 466.16, 'B': 493.88
    }
    
    base_freq = notes.get(note.upper(), 440.0)
    return base_freq * (2 ** (octave - 4))

def main():
    synth = Synthesizer()
    
    print("🎹 Sintetizador Python con Teclado Virtual")
    print("\nComandos básicos:")
    print("  play <nota> [octava] [duracion] [onda]  - Tocar nota")
    print("  start <nota> [octava] [onda]           - Tono continuo")
    print("  stop                                  - Parar tono")
    print("  amp <0.0-1.0>                        - Cambiar volumen")
    print("  quit                                  - Salir")
    print("\nComandos especiales:")
    print("  dual <onda2> <detune> <mix>           - Activar dual osc")
    print("  single                                - Desactivar dual osc")
    print("  melody <notas> [duracion]             - Tocar melodía automática")
    print("\nOndas: sine, square, triangle, sawtooth")
    print("Ejemplos: 'melody adgk 0.5', 'dual square 7 0.5', 'amp 0.05'\n")
    
    while True:
        try:
            cmd = input("> ").strip().split()
            if not cmd:
                continue
                
            if cmd[0] == 'quit':
                synth.stop_continuous_tone()
                break
            elif cmd[0] == 'play':
                note = cmd[1] if len(cmd) > 1 else 'A'
                octave = int(cmd[2]) if len(cmd) > 2 else 4
                duration = float(cmd[3]) if len(cmd) > 3 else 1.0
                waveform = cmd[4] if len(cmd) > 4 else 'sine'
                
                freq = note_to_frequency(note, octave)
                synth.play_note(freq, duration, waveform)
                
            elif cmd[0] == 'start':
                note = cmd[1] if len(cmd) > 1 else 'A'
                octave = int(cmd[2]) if len(cmd) > 2 else 4
                waveform = cmd[3] if len(cmd) > 3 else 'sine'
                
                freq = note_to_frequency(note, octave)
                synth.start_continuous_tone(freq, waveform)
                
            elif cmd[0] == 'stop':
                synth.stop_continuous_tone()
                
            elif cmd[0] == 'amp':
                if len(cmd) > 1:
                    amp = float(cmd[1])
                    synth.set_amplitude(amp)
                    print(f"Volumen: {amp:.2f}")
                    
            elif cmd[0] == 'dual':
                waveform2 = cmd[1] if len(cmd) > 1 else 'square'
                detune = float(cmd[2]) if len(cmd) > 2 else 7.0
                mix = float(cmd[3]) if len(cmd) > 3 else 0.5
                synth.enable_dual_oscillator(waveform2, detune, mix)
                print(f"Dual OSC: {waveform2}, detune {detune:+.1f}st, mix {mix:.1f}")
                
            elif cmd[0] == 'single':
                synth.disable_dual_oscillator()
                print("Modo single oscilador")
                
            elif cmd[0] == 'melody':
                if len(cmd) > 1:
                    notes = cmd[1]
                    duration = float(cmd[2]) if len(cmd) > 2 else 0.5
                    synth.play_melody(notes, duration)
                else:
                    print("Uso: melody <notas> [duracion]")
                    print("Notas: a=Do, s=Do#, d=Re, f=Re#, g=Mi, h=Fa, j=Fa#, k=Sol, l=Sol#, n=La, m=La#, b=Si")
                    print("Mayúsculas = octava 5, - = silencio")
                    print("Ejemplo: melody 'adgk-Adgk' 0.3")
                    
        except KeyboardInterrupt:
            synth.stop_continuous_tone()
            break
        except EOFError:
            synth.stop_continuous_tone()
            break
        except Exception as e:
            print(f"Error: {e}")

if __name__ == "__main__":
    main()