//! Computer-keyboard controller for the GUI.
//!
//! Encapsula el mapeo QWERTY → MIDI, la gestión de octavas y la defensa contra
//! notas colgadas cuando la ventana pierde foco. Expuesto como un único
//! `process(ctx, queue)` que vive fuera del `impl App::update` para que ese
//! método quede como mero orquestador.

use crate::lock_free::{MidiEvent, MidiEventQueue};
use eframe::egui;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Estado del teclado físico mapeado como controlador MIDI.
///
/// Almacenar la nota real pulsada (no sólo la tecla) evita stuck notes cuando
/// el usuario cambia de octava con una tecla mantenida: el NoteOff se calcula
/// con la octava del momento del press, no la actual.
pub struct KeyboardController {
    /// Para cada tecla QWERTY pulsada, guarda (nota MIDI enviada, timestamp).
    last_key_times: HashMap<egui::Key, (u8, Instant)>,
    current_octave: i32,
}

/// Mapeo de teclas QWERTY a offset de semitono sobre la octava actual.
///
/// Fila inferior (Z-M) = octava baja con teclas negras en S,D,G,H,J.
/// Fila superior (Q-P) = octava siguiente con teclas negras en 2,3,5,6,7,9,0.
const KEY_MAP: &[(egui::Key, i32)] = &[
    // Lower octave (Z row)
    (egui::Key::Z, 0),  // C
    (egui::Key::S, 1),  // C#
    (egui::Key::X, 2),  // D
    (egui::Key::D, 3),  // D#
    (egui::Key::C, 4),  // E
    (egui::Key::V, 5),  // F
    (egui::Key::G, 6),  // F#
    (egui::Key::B, 7),  // G
    (egui::Key::H, 8),  // G#
    (egui::Key::N, 9),  // A
    (egui::Key::J, 10), // A#
    (egui::Key::M, 11), // B
    // Upper octave (Q row)
    (egui::Key::Q, 12),
    (egui::Key::Num2, 13),
    (egui::Key::W, 14),
    (egui::Key::Num3, 15),
    (egui::Key::E, 16),
    (egui::Key::R, 17),
    (egui::Key::Num5, 18),
    (egui::Key::T, 19),
    (egui::Key::Num6, 20),
    (egui::Key::Y, 21),
    (egui::Key::Num7, 22),
    (egui::Key::U, 23),
    (egui::Key::I, 24),
    (egui::Key::Num9, 25),
    (egui::Key::O, 26),
    (egui::Key::Num0, 27),
    (egui::Key::P, 28),
];

impl KeyboardController {
    pub fn new() -> Self {
        Self {
            last_key_times: HashMap::new(),
            current_octave: 3, // C3 por defecto
        }
    }

    pub fn current_octave(&self) -> i32 {
        self.current_octave
    }

    /// Dispara AllNotesOff y limpia el estado interno. Invocado desde el botón
    /// PANIC de la GUI y desde Esc.
    pub fn panic(&mut self, midi_events: &Arc<MidiEventQueue>) {
        midi_events.push(MidiEvent::AllNotesOff);
        self.last_key_times.clear();
    }

    /// Procesa input de teclado del frame actual.
    ///
    /// Responsabilidades:
    /// - Focus-loss: si se pierde el foco con teclas pulsadas, dispara
    ///   AllNotesOff (defensivo contra notas colgadas).
    /// - Esc: PANIC universal.
    /// - ArrowUp/Down: octava ±1 con clamp a [0, 8].
    /// - Teclas del mapeo: NoteOn en press (filtrando auto-repeat < 100 ms) y
    ///   NoteOff en release usando la nota grabada en el press.
    pub fn process(&mut self, ctx: &egui::Context, midi_events: &Arc<MidiEventQueue>) {
        let focused = ctx.input(|i| i.focused);
        if !focused && !self.last_key_times.is_empty() {
            self.panic(midi_events);
        }

        ctx.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                midi_events.push(MidiEvent::AllNotesOff);
                self.last_key_times.clear();
            }

            if i.key_pressed(egui::Key::ArrowUp) {
                self.current_octave = (self.current_octave + 1).clamp(0, 8);
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                self.current_octave = (self.current_octave - 1).clamp(0, 8);
            }

            let now = Instant::now();

            for &(key, note_offset) in KEY_MAP {
                if i.key_pressed(key) {
                    // Filtro de auto-repeat: solo disparamos si pasaron >100 ms
                    // desde el último press de esta tecla.
                    let should_trigger = match self.last_key_times.get(&key) {
                        Some((_, last_time)) => now.duration_since(*last_time).as_millis() > 100,
                        None => true,
                    };
                    if should_trigger {
                        let note = (self.current_octave * 12 + note_offset).clamp(0, 127) as u8;
                        self.last_key_times.insert(key, (note, now));
                        midi_events.push(MidiEvent::NoteOn {
                            note,
                            velocity: 100,
                        });
                    }
                }

                if i.key_released(key)
                    && let Some((stored_note, _)) = self.last_key_times.remove(&key)
                {
                    midi_events.push(MidiEvent::NoteOff { note: stored_note });
                }
            }
        });
    }
}

impl Default for KeyboardController {
    fn default() -> Self {
        Self::new()
    }
}
