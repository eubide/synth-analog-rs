//! Ventanas flotantes de MIDI: monitor de mensajes y learn.
//!
//! Reciben los handles (`MidiHandler`, `MidiLearnState`) como argumentos en vez
//! de vivir como métodos de `SynthApp`: son paneles puros que sólo leen/escriben
//! los handles que les pasen, sin acoplar la lógica de la app.

use crate::midi_handler::{CC_BINDINGS, MidiHandler, MidiLearnState};
use eframe::egui;
use std::sync::{Arc, Mutex};

/// MIDI Monitor — lista los últimos 20 mensajes recibidos con coloreado por
/// edad (verde <100 ms, amarillo <1 s, gris >1 s) y por tipo.
pub fn draw_midi_monitor(ui: &mut egui::Ui, midi_handler: Option<&MidiHandler>) {
    ui.horizontal(|ui| {
        ui.label("recent MIDI messages:");
        if ui.button("clear").clicked()
            && let Some(h) = midi_handler
            && let Ok(mut history) = h.message_history.lock()
        {
            history.clear();
        }
    });

    ui.separator();

    let Some(handler) = midi_handler else {
        ui.label("no MIDI handler available");
        return;
    };

    let Ok(history) = handler.message_history.lock() else {
        return;
    };

    egui::ScrollArea::vertical()
        .max_height(250.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for msg in history.iter().rev().take(20) {
                let elapsed = msg.timestamp.elapsed().as_millis();
                let time_color = if elapsed < 100 {
                    egui::Color32::GREEN
                } else if elapsed < 1000 {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::GRAY
                };

                ui.horizontal(|ui| {
                    ui.colored_label(time_color, format!("{:4}ms", elapsed));

                    let type_color = match msg.message_type.as_str() {
                        "Note On" => egui::Color32::from_rgb(100, 255, 100),
                        "Note Off" => egui::Color32::from_rgb(255, 100, 100),
                        "CC" => egui::Color32::from_rgb(100, 200, 255),
                        "Pitch Bend" => egui::Color32::from_rgb(255, 200, 100),
                        _ => egui::Color32::WHITE,
                    };

                    ui.colored_label(type_color, format!("{:10}", msg.message_type));
                    ui.label(&msg.description);
                });
            }

            if history.is_empty() {
                ui.label("no MIDI messages received yet...");
                ui.label("connect a MIDI device and play some notes!");
            }
        });
}

/// MIDI Learn — permite asignar CCs custom a cualquiera de los parámetros
/// listados en `CC_BINDINGS`. La fuente de verdad es el propio binding; Learn
/// sólo sobrescribe el CC de disparo (en `state.custom_map`), no el efecto.
pub fn draw_midi_learn_panel(ui: &mut egui::Ui, learn_state: Option<&Arc<Mutex<MidiLearnState>>>) {
    let Some(learn_arc) = learn_state else {
        ui.label("No MIDI device connected.");
        return;
    };

    // Status line
    if let Ok(state) = learn_arc.try_lock() {
        if let Some(ref pending) = state.pending_param {
            ui.colored_label(
                egui::Color32::YELLOW,
                format!("Waiting for CC... ({})", pending),
            );
        } else {
            ui.label("Click 'Learn' then move a CC on your controller.");
        }
        if !state.custom_map.is_empty() {
            ui.separator();
            ui.label("Active custom bindings:");
            for (cc, param) in &state.custom_map {
                ui.label(format!("  CC {} -> {}", cc, param));
            }
        }
    }
    ui.separator();

    egui::ScrollArea::vertical()
        .max_height(250.0)
        .show(ui, |ui| {
            for binding in CC_BINDINGS {
                let param_key = binding.name;
                let bound_cc: Option<u8> = learn_arc.try_lock().ok().and_then(|state| {
                    state
                        .custom_map
                        .iter()
                        .find(|(_, v)| v.as_str() == param_key)
                        .map(|(cc, _)| *cc)
                });

                ui.horizontal(|ui| {
                    ui.set_min_width(200.0);
                    ui.label(binding.label);
                    if ui.small_button("Learn").clicked()
                        && let Ok(mut state) = learn_arc.try_lock()
                    {
                        state.pending_param = Some(param_key.to_string());
                    }
                    if let Some(cc) = bound_cc {
                        ui.colored_label(egui::Color32::GREEN, format!("CC {}", cc));
                        if ui.small_button("x").clicked()
                            && let Ok(mut state) = learn_arc.try_lock()
                        {
                            state.custom_map.retain(|_, v| v.as_str() != param_key);
                        }
                    } else {
                        ui.colored_label(egui::Color32::GRAY, format!("CC {}", binding.cc));
                    }
                });
            }
        });
}
