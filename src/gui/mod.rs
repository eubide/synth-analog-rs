mod keyboard;
mod panels;
mod preset_browser;

use crate::audio_engine::AudioEngine;
use crate::lock_free::{LockFreeSynth, MidiEventQueue, SynthParameters, UiEvent, UiEventQueue};
use crate::midi_handler::{CC_BINDINGS, MidiHandler};
use crate::synthesizer::Synthesizer;
use eframe::egui;
use keyboard::KeyboardController;
use preset_browser::PresetBrowser;
use std::sync::{Arc, Mutex};

pub struct SynthApp {
    lock_free_synth: Arc<LockFreeSynth>,
    midi_events: Arc<MidiEventQueue>,
    ui_events: Arc<UiEventQueue>,
    _audio_engine: AudioEngine,
    _midi_handler: Option<MidiHandler>,
    keyboard: KeyboardController,
    presets: PresetBrowser,
    show_midi_monitor: bool,
    show_midi_learn: bool,
    show_presets_window: bool,
    learn_state: Option<Arc<Mutex<crate::midi_handler::MidiLearnState>>>,
    params: SynthParameters,
    peak_level: f32,
}

impl SynthApp {
    pub fn new(
        lock_free_synth: Arc<LockFreeSynth>,
        midi_events: Arc<MidiEventQueue>,
        ui_events: Arc<UiEventQueue>,
        audio_engine: AudioEngine,
        midi_handler: Option<MidiHandler>,
    ) -> Self {
        let params = *lock_free_synth.get_params();
        let learn_state = midi_handler.as_ref().map(|h| h.learn_state.clone());
        Self {
            lock_free_synth,
            midi_events,
            ui_events,
            _audio_engine: audio_engine,
            _midi_handler: midi_handler,
            keyboard: KeyboardController::new(),
            presets: PresetBrowser::new(),
            show_midi_monitor: false,
            show_midi_learn: false,
            show_presets_window: false,
            learn_state,
            params,
            peak_level: 0.0,
        }
    }

    /// Consume `UiEvent`s queued by the MIDI thread. The audio callback never
    /// sees these — all paths involve disk I/O or JSON parsing.
    fn drain_ui_events(&mut self) {
        for event in self.ui_events.drain() {
            match event {
                UiEvent::ProgramChange { program } => {
                    let presets = Synthesizer::list_presets();
                    if presets.is_empty() {
                        continue;
                    }
                    let name = &presets[(program as usize) % presets.len()];
                    let mut temp = Synthesizer::new();
                    match temp.load_preset(name) {
                        Ok(_) => {
                            self.params = temp.to_synth_params();
                            self.presets.set_current(name.clone());
                            log::info!("Program Change {}: loaded '{}'", program, name);
                        }
                        Err(e) => log::warn!(
                            "Program Change {}: failed to load '{}': {}",
                            program,
                            name,
                            e,
                        ),
                    }
                }
                UiEvent::SysExRequest => {
                    // Snapshot current patch to presets/sysex_dump.json.
                    let mut temp = Synthesizer::new();
                    temp.apply_params(&self.params);
                    if let Err(e) = temp.save_preset("sysex_dump") {
                        log::warn!("SysEx dump failed: {}", e);
                    } else {
                        log::info!("SysEx: patch saved as sysex_dump");
                    }
                }
                UiEvent::SysExPatch { data } => match std::str::from_utf8(&data) {
                    Ok(json_str) => {
                        let mut temp = Synthesizer::new();
                        match temp.load_preset_from_json(json_str) {
                            Ok(_) => {
                                self.params = temp.to_synth_params();
                                log::info!("SysEx patch applied ({} bytes)", data.len());
                            }
                            Err(e) => log::warn!("SysEx patch load failed: {}", e),
                        }
                    }
                    Err(_) => log::warn!("SysEx: payload is not valid UTF-8"),
                },
            }
        }
    }
}

impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Read current params at start of frame
        self.params = *self.lock_free_synth.get_params();
        let peak_bits = self
            .lock_free_synth
            .peak_level
            .load(std::sync::atomic::Ordering::Relaxed);
        self.peak_level = f32::from_bits(peak_bits);

        self.drain_ui_events();

        self.keyboard.process(ctx, &self.midi_events);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Compact Vintage Analog Style Header
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("PROPHET-5 SYNTHESIZER")
                        .size(18.0)
                        .strong(),
                );

                // Current preset name — clickable to open the preset manager.
                let current_name = self.presets.current_name();
                let (preset_text, preset_color) = if current_name.is_empty() {
                    ("no preset".to_string(), egui::Color32::from_gray(140))
                } else {
                    (
                        current_name.to_string(),
                        egui::Color32::from_rgb(100, 220, 100),
                    )
                };
                if ui
                    .add(
                        egui::Label::new(
                            egui::RichText::new(format!("> {}", preset_text))
                                .size(13.0)
                                .color(preset_color),
                        )
                        .sense(egui::Sense::click()),
                    )
                    .on_hover_text("Click to open the preset manager")
                    .clicked()
                {
                    self.show_presets_window = !self.show_presets_window;
                }

                // VU meter bar
                let peak = self.peak_level;
                let clipping = peak > 0.8;
                let vu_color = if clipping {
                    egui::Color32::from_rgb(255, 60, 60)
                } else if peak > 0.5 {
                    egui::Color32::from_rgb(255, 220, 40)
                } else {
                    egui::Color32::from_rgb(60, 200, 60)
                };
                let (vu_rect, _) =
                    ui.allocate_exact_size(egui::vec2(80.0, 12.0), egui::Sense::hover());
                if ui.is_rect_visible(vu_rect) {
                    let painter = ui.painter();
                    painter.rect_filled(vu_rect, 2.0, egui::Color32::from_gray(25));
                    let filled_w = vu_rect.width() * peak.min(1.0);
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            vu_rect.min,
                            egui::vec2(filled_w, vu_rect.height()),
                        ),
                        2.0,
                        vu_color,
                    );
                    if clipping {
                        painter.text(
                            vu_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "CLIP",
                            egui::FontId::monospace(9.0),
                            egui::Color32::WHITE,
                        );
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self._midi_handler.is_some() {
                        if ui
                            .small_button("MIDI")
                            .on_hover_text("Toggle MIDI message monitor window")
                            .clicked()
                        {
                            self.show_midi_monitor = !self.show_midi_monitor;
                        }
                    } else {
                        let _ = ui
                            .small_button("NO MIDI")
                            .on_hover_text("No MIDI device detected at startup");
                    }

                    if self.learn_state.is_some() {
                        let btn_text = if self.show_midi_learn {
                            "MIDI Learn *"
                        } else {
                            "MIDI Learn"
                        };
                        if ui
                            .small_button(btn_text)
                            .on_hover_text("Toggle MIDI Learn window — assign CCs to params")
                            .clicked()
                        {
                            self.show_midi_learn = !self.show_midi_learn;
                        }
                    }

                    if ui
                        .small_button("Presets")
                        .on_hover_text("Open the preset manager window")
                        .clicked()
                    {
                        self.show_presets_window = !self.show_presets_window;
                    }

                    if ui
                        .small_button("PANIC")
                        .on_hover_text("Silence all stuck notes (Esc)")
                        .clicked()
                    {
                        self.keyboard.panic(&self.midi_events);
                    }
                });
            });
            ui.separator();

            // Ventana de tamaño fijo — no se necesita ScrollArea
            {
                // Layout 5 cols × 220 px todas iguales.
                // slider_width compacto (55 px vs default 100) para que el track
                // del slider + valor numérico quepan en `WIDGET_WIDTH=105` sin que
                // el valor se corte por la derecha.
                ui.spacing_mut().slider_width = 55.0;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    // ── COL 1: FUENTES (220 px) ─────────────────────────────
                    ui.vertical(|ui| {
                        ui.set_min_width(220.0);
                        ui.set_max_width(220.0);
                        panels::section(ui, "OSCILLATOR A", |ui| {
                            panels::draw_oscillator(ui, &mut self.params, 1)
                        });
                        ui.add_space(4.0);
                        panels::section(ui, "OSCILLATOR B", |ui| {
                            panels::draw_oscillator(ui, &mut self.params, 2)
                        });
                        ui.add_space(4.0);
                        panels::section(ui, "ANALOG", |ui| panels::draw_analog(ui, &mut self.params));
                    });

                    // ── COL 2: CADENA DE SEÑAL (220 px) ────────────────────
                    ui.vertical(|ui| {
                        ui.set_min_width(220.0);
                        ui.set_max_width(220.0);
                        panels::section(ui, "MIXER", |ui| panels::draw_mixer(ui, &mut self.params));
                        ui.add_space(4.0);
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("FILTER").size(11.0).strong());
                                ui.label(
                                    egui::RichText::new("24dB")
                                        .size(9.0)
                                        .color(egui::Color32::GRAY),
                                );
                            });
                            panels::draw_filter(ui, &mut self.params);
                        });
                        ui.add_space(4.0);
                        panels::section(ui, "POLY MOD", |ui| {
                            panels::draw_poly_mod(ui, &mut self.params)
                        });
                    });

                    // ── COL 3: ENV/LFO/VOICE (220 px) ───────────────────────
                    ui.vertical(|ui| {
                        ui.set_min_width(220.0);
                        ui.set_max_width(220.0);
                        panels::section(ui, "FILTER ENV", |ui| {
                            panels::draw_envelope(
                                ui,
                                &mut self.params.filter_attack,
                                &mut self.params.filter_decay,
                                &mut self.params.filter_sustain,
                                &mut self.params.filter_release,
                            );
                            panels::draw_adsr_curve(
                                ui,
                                self.params.filter_attack,
                                self.params.filter_decay,
                                self.params.filter_sustain,
                                self.params.filter_release,
                            );
                        });
                        ui.add_space(4.0);
                        panels::section(ui, "LFO", |ui| panels::draw_lfo(ui, &mut self.params));
                        ui.add_space(4.0);
                        panels::section(ui, "VOICE MODE", |ui| {
                            panels::draw_voice_mode(ui, &mut self.params)
                        });
                    });

                    // ── COL 4: ENV/LFO MOD/ARP (220 px) ─────────────────────
                    ui.vertical(|ui| {
                        ui.set_min_width(220.0);
                        ui.set_max_width(220.0);
                        panels::section(ui, "AMP ENV", |ui| {
                            panels::draw_envelope(
                                ui,
                                &mut self.params.amp_attack,
                                &mut self.params.amp_decay,
                                &mut self.params.amp_sustain,
                                &mut self.params.amp_release,
                            );
                            panels::draw_adsr_curve(
                                ui,
                                self.params.amp_attack,
                                self.params.amp_decay,
                                self.params.amp_sustain,
                                self.params.amp_release,
                            );
                        });
                        ui.add_space(4.0);
                        panels::section(ui, "LFO MOD", |ui| {
                            panels::draw_lfo_mod(ui, &mut self.params)
                        });
                        ui.add_space(4.0);
                        panels::section(ui, "ARP", |ui| {
                            panels::draw_arpeggiator(ui, &mut self.params)
                        });
                    });

                    // ── COL 5: SALIDA (220 px) ──────────────────────────────
                    ui.vertical(|ui| {
                        ui.set_min_width(220.0);
                        ui.set_max_width(220.0);
                        panels::section(ui, "MASTER", |ui| {
                            panels::draw_master(ui, &mut self.params)
                        });
                        ui.add_space(4.0);
                        panels::section(ui, "EFFECTS", |ui| {
                            panels::draw_effects(ui, &mut self.params)
                        });
                        ui.add_space(4.0);
                        ui.group(|ui| {
                            ui.label(egui::RichText::new("PRESET").size(11.0).strong());
                            let current = self.presets.current_name();
                            ui.label(
                                egui::RichText::new(if current.is_empty() {
                                    "default"
                                } else {
                                    current
                                })
                                .size(10.0)
                                .color(egui::Color32::from_rgb(100, 220, 100)),
                            );
                            if ui
                                .small_button("manage...")
                                .on_hover_text(
                                    "Open preset manager — save/load patches by category",
                                )
                                .clicked()
                            {
                                self.show_presets_window = !self.show_presets_window;
                            }
                        });
                    });
                });

                ui.add_space(4.0);

                // KEYBOARD REFERENCE — barra compacta de 2 líneas
                ui.group(|ui| {
                    panels::draw_keyboard_legend(ui, self.keyboard.current_octave());
                });
            } // fin bloque layout fijo
        }); // CentralPanel::show_inside

        // MIDI Monitor Window
        if self.show_midi_monitor {
            egui::Window::new("MIDI Monitor")
                .default_size([400.0, 300.0])
                .show(ui.ctx(), |ui| {
                    self.draw_midi_monitor(ui);
                });
        }

        // MIDI Learn Window
        if self.show_midi_learn {
            egui::Window::new("MIDI Learn")
                .default_size([280.0, 380.0])
                .show(ui.ctx(), |ui| {
                    self.draw_midi_learn_panel(ui);
                });
        }

        // Presets Window
        if self.show_presets_window {
            let mut show_presets_window = self.show_presets_window;
            egui::Window::new("Preset Manager")
                .default_size([350.0, 400.0])
                .open(&mut show_presets_window)
                .show(ui.ctx(), |ui| {
                    self.presets.draw(ui, &mut self.params);
                });
            self.show_presets_window = show_presets_window;
        }

        // Write params back at end of frame
        self.lock_free_synth.set_params(self.params);
    }
}

impl SynthApp {
    fn draw_midi_monitor(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("recent MIDI messages:");
            if ui.button("clear").clicked()
                && let Some(ref midi_handler) = self._midi_handler
                && let Ok(mut history) = midi_handler.message_history.lock()
            {
                history.clear();
            }
        });

        ui.separator();

        if let Some(ref midi_handler) = self._midi_handler {
            if let Ok(history) = midi_handler.message_history.lock() {
                egui::ScrollArea::vertical()
                    .max_height(250.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for msg in history.iter().rev().take(20) {
                            // Show last 20 messages
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
        } else {
            ui.label("no MIDI handler available");
        }
    }

    fn draw_midi_learn_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(ref learn_arc) = self.learn_state {
            // Status line
            {
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
        } else {
            ui.label("No MIDI device connected.");
        }
    }
}
