import pygame
import numpy as np
import sounddevice as sd
import threading
import time
from typing import Dict, Optional, Tuple

class PianoSynth:
    def __init__(self):
        pygame.init()
        
        # Screen setup
        self.WIDTH = 900
        self.HEIGHT = 600
        self.screen = pygame.display.set_mode((self.WIDTH, self.HEIGHT))
        pygame.display.set_caption("🎹 Piano Virtual Synth")
        
        # Colors
        self.WHITE = (255, 255, 255)
        self.BLACK = (0, 0, 0)
        self.GRAY = (128, 128, 128)
        self.LIGHT_GRAY = (200, 200, 200)
        self.DARK_GRAY = (64, 64, 64)
        self.BLUE = (100, 150, 255)
        self.RED = (255, 100, 100)
        self.GREEN = (100, 255, 100)
        
        # Fonts
        self.font_large = pygame.font.Font(None, 36)
        self.font_medium = pygame.font.Font(None, 24)
        self.font_small = pygame.font.Font(None, 18)
        
        # Audio settings
        self.sample_rate = 44100
        self.amplitude = 0.1
        self.waveform = 'sine'
        self.is_playing = False
        self.phase = 0.0
        self.stream: Optional[sd.OutputStream] = None
        
        # Dual oscillator
        self.dual_mode = False
        self.osc2_detune = 7.0
        self.osc2_mix = 0.5
        self.phase2 = 0.0
        
        # Key tracking
        self.pressed_keys: Dict[int, bool] = {}
        self.active_notes: Dict[int, float] = {}
        
        # Piano layout
        self.setup_piano_layout()
        self.setup_audio()
        
        # UI state
        self.waveforms = ['sine', 'square', 'triangle', 'sawtooth']
        self.waveform_index = 0
        
    def setup_piano_layout(self):
        # Key mapping (pygame key codes) - sin cruces
        self.key_notes = {
            # Fila inferior (octava 4)
            pygame.K_z: ('C', 4, 'white'),   # Z -> C4
            pygame.K_s: ('C#', 4, 'black'),  # S -> C#4
            pygame.K_x: ('D', 4, 'white'),   # X -> D4
            pygame.K_d: ('D#', 4, 'black'),  # D -> D#4
            pygame.K_c: ('E', 4, 'white'),   # C -> E4
            pygame.K_v: ('F', 4, 'white'),   # V -> F4
            pygame.K_g: ('F#', 4, 'black'),  # G -> F#4
            pygame.K_b: ('G', 4, 'white'),   # B -> G4
            pygame.K_h: ('G#', 4, 'black'),  # H -> G#4
            pygame.K_n: ('A', 4, 'white'),   # N -> A4
            pygame.K_j: ('A#', 4, 'black'),  # J -> A#4
            pygame.K_m: ('B', 4, 'white'),   # M -> B4
            
            # Fila superior (octava 5)
            pygame.K_q: ('C', 5, 'white'),   # Q -> C5
            pygame.K_2: ('C#', 5, 'black'),  # 2 -> C#5
            pygame.K_w: ('D', 5, 'white'),   # W -> D5
            pygame.K_3: ('D#', 5, 'black'),  # 3 -> D#5
            pygame.K_e: ('E', 5, 'white'),   # E -> E5
            pygame.K_r: ('F', 5, 'white'),   # R -> F5
            pygame.K_5: ('F#', 5, 'black'),  # 5 -> F#5
            pygame.K_t: ('G', 5, 'white'),   # T -> G5
            pygame.K_6: ('G#', 5, 'black'),  # 6 -> G#5
            pygame.K_y: ('A', 5, 'white'),   # Y -> A5
            pygame.K_7: ('A#', 5, 'black'),  # 7 -> A#5
            pygame.K_u: ('B', 5, 'white'),   # U -> B5
            pygame.K_i: ('A', 5, 'white'),   # I -> A5
        }
        
        # Visual piano keys layout
        self.white_key_width = 60
        self.white_key_height = 200
        self.black_key_width = 35
        self.black_key_height = 120
        
        self.piano_start_x = 50
        self.piano_start_y = 250
        
    def setup_audio(self):
        self.stream = sd.OutputStream(
            samplerate=self.sample_rate,
            channels=1,
            callback=self.audio_callback,
            blocksize=512,
            dtype=np.float32,
            latency='low'
        )
        self.stream.start()
    
    def note_to_frequency(self, note: str, octave: int) -> float:
        notes = {
            'C': 261.63, 'C#': 277.18, 'D': 293.66, 'D#': 311.13,
            'E': 329.63, 'F': 349.23, 'F#': 369.99, 'G': 392.00,
            'G#': 415.30, 'A': 440.00, 'A#': 466.16, 'B': 493.88
        }
        base_freq = notes.get(note, 440.0)
        return base_freq * (2 ** (octave - 4))
    
    def generate_wave(self, frequency: float, num_samples: int) -> np.ndarray:
        t = np.arange(num_samples) / self.sample_rate
        
        if self.waveform == 'sine':
            wave = np.sin(2 * np.pi * frequency * t)
        elif self.waveform == 'square':
            wave = np.sign(np.sin(2 * np.pi * frequency * t)) * 0.6
        elif self.waveform == 'triangle':
            wave = 2 * np.arcsin(np.sin(2 * np.pi * frequency * t)) / np.pi
        elif self.waveform == 'sawtooth':
            wave = 2 * (t * frequency - np.floor(t * frequency + 0.5)) * 0.8
        else:
            wave = np.sin(2 * np.pi * frequency * t)
            
        return wave * self.amplitude
    
    def generate_audio_buffer(self, num_frames: int) -> np.ndarray:
        if not self.is_playing or not self.active_notes:
            return np.zeros(num_frames, dtype=np.float32)
        
        mixed_wave = np.zeros(num_frames)
        num_notes = len(self.active_notes)
        
        for frequency in self.active_notes.values():
            # Generate oscillator 1
            if self.waveform == 'sine':
                phase_increment = 2 * np.pi * frequency / self.sample_rate
                phases = self.phase + np.arange(num_frames) * phase_increment
                wave1 = np.sin(phases)
                self.phase = (self.phase + num_frames * phase_increment) % (2 * np.pi)
            else:
                wave1 = self.generate_wave(frequency, num_frames) / self.amplitude
            
            # Generate oscillator 2 if dual mode
            if self.dual_mode:
                osc2_freq = frequency * (2 ** (self.osc2_detune / 12.0))
                wave2 = self.generate_wave(osc2_freq, num_frames) / self.amplitude
                wave = wave1 * (1 - self.osc2_mix) + wave2 * self.osc2_mix
            else:
                wave = wave1
            
            mixed_wave += wave / num_notes
        
        mixed_wave = mixed_wave * self.amplitude
        return np.clip(mixed_wave, -0.8, 0.8).astype(np.float32)
    
    def audio_callback(self, outdata, frames, time, status):
        if status:
            print(f"Audio status: {status}")
        
        audio_data = self.generate_audio_buffer(frames)
        outdata[:] = audio_data.reshape(-1, 1)
    
    def draw_piano_keys(self):
        # Draw white keys
        white_notes = ['C4', 'D4', 'E4', 'F4', 'G4', 'A4', 'B4', 'C5', 'D5', 'E5', 'F5', 'G5', 'A5', 'B5']
        white_keys = [pygame.K_z, pygame.K_x, pygame.K_c, pygame.K_v, pygame.K_b, pygame.K_n, pygame.K_m,
                     pygame.K_q, pygame.K_w, pygame.K_e, pygame.K_r, pygame.K_t, pygame.K_y, pygame.K_u]
        
        for i, (key, note) in enumerate(zip(white_keys, white_notes)):
            x = self.piano_start_x + i * self.white_key_width
            y = self.piano_start_y
            
            # Color based on key press
            color = self.RED if key in self.pressed_keys else self.WHITE
            
            pygame.draw.rect(self.screen, color, 
                           (x, y, self.white_key_width, self.white_key_height))
            pygame.draw.rect(self.screen, self.BLACK, 
                           (x, y, self.white_key_width, self.white_key_height), 2)
            
            # Draw key label
            key_text = pygame.key.name(key).upper()
            note_text = self.font_small.render(f"{key_text}", True, self.BLACK)
            note_label = self.font_small.render(f"{note}", True, self.BLACK)
            
            self.screen.blit(note_text, (x + 5, y + 5))
            self.screen.blit(note_label, (x + 5, y + 175))
        
        # Draw black keys (solo donde corresponde en piano real)
        # Posiciones: C# D# _ F# G# A# _ C# D# _ F# G# A#
        black_positions = [0.7, 1.7, 3.7, 4.7, 5.7, 7.7, 8.7, 10.7, 11.7, 12.7]  # Relative positions
        black_keys = [pygame.K_s, pygame.K_d, pygame.K_g, pygame.K_h, pygame.K_j,
                     pygame.K_2, pygame.K_3, pygame.K_5, pygame.K_6, pygame.K_7]
        black_notes = ['C#4', 'D#4', 'F#4', 'G#4', 'A#4', 'C#5', 'D#5', 'F#5', 'G#5', 'A#5']
        
        for pos, key, note in zip(black_positions, black_keys, black_notes):
            x = self.piano_start_x + pos * self.white_key_width - self.black_key_width // 2
            y = self.piano_start_y
            
            # Color based on key press
            color = self.BLUE if key in self.pressed_keys else self.BLACK
            
            pygame.draw.rect(self.screen, color, 
                           (x, y, self.black_key_width, self.black_key_height))
            
            # Draw key label
            key_text = pygame.key.name(key).upper()
            text_color = self.WHITE if color == self.BLACK else self.BLACK
            key_label = self.font_small.render(f"{key_text}", True, text_color)
            note_label = self.font_small.render(f"{note}", True, text_color)
            
            self.screen.blit(key_label, (x + 5, y + 5))
            self.screen.blit(note_label, (x + 2, y + 95))
    
    def draw_controls(self):
        # Title
        title = self.font_large.render("🎹 Piano Virtual Synth", True, self.WHITE)
        self.screen.blit(title, (self.WIDTH // 2 - title.get_width() // 2, 20))
        
        # Instructions
        instructions = [
            "Fila inferior: Z S X D C V G B H N J M = Do Do# Re Re# Mi Fa Fa# Sol Sol# La La# Si (octava 4)",
            "Fila superior: Q 2 W 3 E R 5 T 6 Y 7 U = Do Do# Re Re# Mi Fa Fa# Sol Sol# La La# Si (octava 5)",
            "ESPACIO = Cambiar onda | ENTER = Dual OSC | ↑↓ = Volumen | ←→ = Detune | ESC = Salir"
        ]
        
        for i, instruction in enumerate(instructions):
            text = self.font_small.render(instruction, True, self.LIGHT_GRAY)
            self.screen.blit(text, (20, 70 + i * 20))
        
        # Current settings
        settings_y = 180
        waveform_text = self.font_medium.render(f"Onda: {self.waveform}", True, self.WHITE)
        self.screen.blit(waveform_text, (20, settings_y))
        
        volume_text = self.font_medium.render(f"Volumen: {self.amplitude:.2f}", True, self.WHITE)
        self.screen.blit(volume_text, (200, settings_y))
        
        dual_status = "ON" if self.dual_mode else "OFF"
        dual_color = self.GREEN if self.dual_mode else self.RED
        dual_text = self.font_medium.render(f"Dual OSC: {dual_status}", True, dual_color)
        self.screen.blit(dual_text, (400, settings_y))
        
        if self.dual_mode:
            detune_text = self.font_medium.render(f"Detune: {self.osc2_detune:+.1f}st", True, self.WHITE)
            self.screen.blit(detune_text, (550, settings_y))
        
        # Active notes
        if self.active_notes:
            notes_text = "Notas activas: " + ", ".join([
                f"{self.key_notes[k][0]}{self.key_notes[k][1]}" 
                for k in self.pressed_keys.keys() if k in self.key_notes
            ])
            active_text = self.font_medium.render(notes_text, True, self.GREEN)
            self.screen.blit(active_text, (20, settings_y + 30))
    
    def handle_key_press(self, key):
        if key in self.key_notes and key not in self.pressed_keys:
            self.pressed_keys[key] = True
            note, octave, _ = self.key_notes[key]
            frequency = self.note_to_frequency(note, octave)
            self.active_notes[key] = frequency
            
            if not self.is_playing:
                self.is_playing = True
    
    def handle_key_release(self, key):
        if key in self.pressed_keys:
            del self.pressed_keys[key]
            if key in self.active_notes:
                del self.active_notes[key]
            
            if not self.active_notes:
                self.is_playing = False
    
    def handle_control_key(self, key):
        if key == pygame.K_SPACE:
            # Change waveform
            self.waveform_index = (self.waveform_index + 1) % len(self.waveforms)
            self.waveform = self.waveforms[self.waveform_index]
        elif key == pygame.K_RETURN:
            # Toggle dual oscillator
            self.dual_mode = not self.dual_mode
        elif key == pygame.K_UP:
            # Increase volume
            self.amplitude = min(0.5, self.amplitude + 0.02)
        elif key == pygame.K_DOWN:
            # Decrease volume
            self.amplitude = max(0.01, self.amplitude - 0.02)
        elif key == pygame.K_LEFT:
            # Decrease detune
            self.osc2_detune = max(-12, self.osc2_detune - 0.5)
        elif key == pygame.K_RIGHT:
            # Increase detune
            self.osc2_detune = min(12, self.osc2_detune + 0.5)
    
    def run(self):
        clock = pygame.time.Clock()
        running = True
        
        while running:
            for event in pygame.event.get():
                if event.type == pygame.QUIT:
                    running = False
                elif event.type == pygame.KEYDOWN:
                    # Control keys
                    if event.key == pygame.K_ESCAPE:
                        running = False
                    elif event.key in [pygame.K_SPACE, pygame.K_RETURN, pygame.K_UP, 
                                   pygame.K_DOWN, pygame.K_LEFT, pygame.K_RIGHT]:
                        self.handle_control_key(event.key)
                    # Musical keys
                    else:
                        self.handle_key_press(event.key)
                elif event.type == pygame.KEYUP:
                    self.handle_key_release(event.key)
            
            # Draw everything
            self.screen.fill(self.DARK_GRAY)
            self.draw_controls()
            self.draw_piano_keys()
            
            pygame.display.flip()
            clock.tick(60)  # 60 FPS
        
        # Cleanup
        if self.stream:
            self.stream.stop()
            self.stream.close()
        pygame.quit()

def main():
    synth = PianoSynth()
    synth.run()

if __name__ == "__main__":
    main()