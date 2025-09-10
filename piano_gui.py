import tkinter as tk
from tkinter import ttk
import numpy as np
import sounddevice as sd
import threading
import time
from typing import Dict, Optional

class PianoGUI:
    def __init__(self):
        self.root = tk.Tk()
        self.root.title("🎹 Sintetizador Piano Virtual")
        self.root.geometry("800x400")
        self.root.configure(bg='#2b2b2b')
        
        # Synthesizer settings
        self.sample_rate = 44100
        self.amplitude = 0.1
        self.waveform = 'sine'
        self.current_frequency = 440.0
        self.is_playing = False
        self.phase = 0.0
        self.stream: Optional[sd.OutputStream] = None
        
        # Dual oscillator
        self.dual_mode = False
        self.osc2_waveform = 'square'
        self.osc2_detune = 7.0
        self.osc2_mix = 0.5
        self.phase2 = 0.0
        
        # Key tracking
        self.pressed_keys: Dict[str, bool] = {}
        self.active_notes: Dict[str, float] = {}
        
        self.setup_gui()
        self.setup_key_bindings()
        
    def setup_gui(self):
        # Title
        title = tk.Label(self.root, text="🎹 Piano Virtual Synth", 
                        font=("Arial", 20, "bold"), 
                        bg='#2b2b2b', fg='white')
        title.pack(pady=10)
        
        # Instructions
        instructions = tk.Label(self.root, 
                               text="Usa las teclas: A S D F G H J K L para tocar (Do Do# Re Re# Mi Fa Fa# Sol Sol#)\nQ W E R T Y U I O para octava superior", 
                               font=("Arial", 10), 
                               bg='#2b2b2b', fg='#cccccc')
        instructions.pack(pady=5)
        
        # Piano frame
        piano_frame = tk.Frame(self.root, bg='#2b2b2b')
        piano_frame.pack(pady=20, expand=True, fill='both')
        
        # White keys
        white_keys = ['A', 'D', 'F', 'G', 'H', 'J', 'L', 'Q', 'E', 'T', 'Y', 'I']
        white_notes = ['C4', 'D4', 'E4', 'F4', 'G4', 'A4', 'B4', 'C5', 'D5', 'E5', 'F5', 'G5']
        
        self.white_key_buttons = {}
        for i, (key, note) in enumerate(zip(white_keys, white_notes)):
            btn = tk.Button(piano_frame, text=f"{key}\n{note}", 
                           width=6, height=8, 
                           bg='white', fg='black',
                           font=("Arial", 8, "bold"),
                           relief='raised', bd=2)
            btn.grid(row=1, column=i, padx=1, pady=5)
            self.white_key_buttons[key.lower()] = btn
        
        # Black keys
        black_keys = ['S', 'None', 'None', 'None', 'None', 'K', 'None', 'W', 'R', 'None', 'U', 'None']
        black_notes = ['C#4', '', '', '', '', 'A#4', '', 'C#5', 'D#5', '', 'F#5', '']
        
        self.black_key_buttons = {}
        for i, (key, note) in enumerate(zip(black_keys, black_notes)):
            if key != 'None':
                btn = tk.Button(piano_frame, text=f"{key}\n{note}", 
                               width=4, height=5, 
                               bg='#333333', fg='white',
                               font=("Arial", 7, "bold"),
                               relief='raised', bd=1)
                btn.grid(row=0, column=i, columnspan=1, padx=1, sticky='e')
                self.black_key_buttons[key.lower()] = btn
        
        # Controls frame
        controls_frame = tk.Frame(self.root, bg='#2b2b2b')
        controls_frame.pack(pady=10, fill='x')
        
        # Waveform selection
        tk.Label(controls_frame, text="Onda:", bg='#2b2b2b', fg='white', font=("Arial", 10)).grid(row=0, column=0, padx=5)
        self.waveform_var = tk.StringVar(value=self.waveform)
        waveform_combo = ttk.Combobox(controls_frame, textvariable=self.waveform_var, 
                                     values=['sine', 'square', 'triangle', 'sawtooth'],
                                     width=10, state='readonly')
        waveform_combo.grid(row=0, column=1, padx=5)
        waveform_combo.bind('<<ComboboxSelected>>', self.on_waveform_change)
        
        # Volume control
        tk.Label(controls_frame, text="Volumen:", bg='#2b2b2b', fg='white', font=("Arial", 10)).grid(row=0, column=2, padx=5)
        self.volume_var = tk.DoubleVar(value=self.amplitude)
        volume_scale = tk.Scale(controls_frame, from_=0.01, to=0.5, resolution=0.01,
                               orient='horizontal', length=100, variable=self.volume_var,
                               bg='#2b2b2b', fg='white', troughcolor='#555555')
        volume_scale.grid(row=0, column=3, padx=5)
        volume_scale.bind('<Motion>', self.on_volume_change)
        
        # Dual oscillator controls
        self.dual_var = tk.BooleanVar()
        dual_check = tk.Checkbutton(controls_frame, text="Dual OSC", variable=self.dual_var,
                                   bg='#2b2b2b', fg='white', selectcolor='#555555',
                                   command=self.toggle_dual_osc)
        dual_check.grid(row=0, column=4, padx=10)
        
        tk.Label(controls_frame, text="Detune:", bg='#2b2b2b', fg='white', font=("Arial", 9)).grid(row=0, column=5, padx=2)
        self.detune_var = tk.DoubleVar(value=self.osc2_detune)
        detune_scale = tk.Scale(controls_frame, from_=-12, to=12, resolution=0.5,
                               orient='horizontal', length=80, variable=self.detune_var,
                               bg='#2b2b2b', fg='white', troughcolor='#555555')
        detune_scale.grid(row=0, column=6, padx=2)
        detune_scale.bind('<Motion>', self.on_detune_change)
        
    def setup_key_bindings(self):
        # Key mappings
        self.key_notes = {
            'a': ('C', 4), 's': ('C#', 4), 'd': ('D', 4), 'f': ('D#', 4), 'g': ('E', 4),
            'h': ('F', 4), 'j': ('F#', 4), 'k': ('G', 4), 'l': ('G#', 4),
            'q': ('C', 5), 'w': ('C#', 5), 'e': ('D', 5), 'r': ('D#', 5), 't': ('E', 5),
            'y': ('F', 5), 'u': ('F#', 5), 'i': ('G', 5), 'o': ('G#', 5)
        }
        
        # Bind keyboard events
        self.root.bind('<KeyPress>', self.on_key_press)
        self.root.bind('<KeyRelease>', self.on_key_release)
        self.root.focus_set()  # Make sure window can receive key events
        
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
        
        # Mix all active notes
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
            
            mixed_wave += wave / num_notes  # Normalize by number of notes
        
        mixed_wave = mixed_wave * self.amplitude
        return np.clip(mixed_wave, -0.8, 0.8).astype(np.float32)
    
    def audio_callback(self, outdata, frames, time, status):
        if status:
            print(f"Audio status: {status}")
        
        audio_data = self.generate_audio_buffer(frames)
        outdata[:] = audio_data.reshape(-1, 1)
    
    def start_audio_stream(self):
        if not self.stream:
            self.stream = sd.OutputStream(
                samplerate=self.sample_rate,
                channels=1,
                callback=self.audio_callback,
                blocksize=512,
                dtype=np.float32,
                latency='low'
            )
            self.stream.start()
    
    def stop_audio_stream(self):
        if self.stream:
            self.stream.stop()
            self.stream.close()
            self.stream = None
        self.is_playing = False
        self.active_notes.clear()
    
    def on_key_press(self, event):
        key = event.char.lower()
        if key in self.key_notes and key not in self.pressed_keys:
            self.pressed_keys[key] = True
            note, octave = self.key_notes[key]
            frequency = self.note_to_frequency(note, octave)
            self.active_notes[key] = frequency
            
            # Visual feedback
            if key in self.white_key_buttons:
                self.white_key_buttons[key].configure(bg='#ffcccc')
            elif key in self.black_key_buttons:
                self.black_key_buttons[key].configure(bg='#ff6666')
            
            # Start audio if not already playing
            if not self.is_playing:
                self.is_playing = True
                self.start_audio_stream()
    
    def on_key_release(self, event):
        key = event.char.lower()
        if key in self.pressed_keys:
            del self.pressed_keys[key]
            if key in self.active_notes:
                del self.active_notes[key]
            
            # Reset visual feedback
            if key in self.white_key_buttons:
                self.white_key_buttons[key].configure(bg='white')
            elif key in self.black_key_buttons:
                self.black_key_buttons[key].configure(bg='#333333')
            
            # Stop audio if no keys pressed
            if not self.active_notes:
                self.is_playing = False
    
    def on_waveform_change(self, event):
        self.waveform = self.waveform_var.get()
    
    def on_volume_change(self, event):
        self.amplitude = self.volume_var.get()
    
    def on_detune_change(self, event):
        self.osc2_detune = self.detune_var.get()
    
    def toggle_dual_osc(self):
        self.dual_mode = self.dual_var.get()
    
    def run(self):
        try:
            self.root.mainloop()
        finally:
            self.stop_audio_stream()

def main():
    app = PianoGUI()
    app.run()

if __name__ == "__main__":
    main()