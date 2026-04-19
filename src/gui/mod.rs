mod keyboard;
mod midi_windows;
mod panels;
mod preset_browser;
mod visualiser;

use crate::audio_engine::AudioEngine;
use crate::lock_free::{LockFreeSynth, MidiEventQueue, SynthParameters, UiEvent, UiEventQueue};
use crate::midi_handler::MidiHandler;
use crate::synthesizer::Synthesizer;
use eframe::egui;
use keyboard::KeyboardController;
use preset_browser::PresetBrowser;
use std::sync::Arc;
use visualiser::VisualiserState;

/// VU meter display range. Professional meters map ~-48 dB (noise floor) to
/// 0 dBFS (clip) — wider than a linear 0..1 bar, so typical outputs around
/// -20 dBFS (~0.1 linear) now sweep through the middle of the bar instead of
/// barely nudging the left edge.
const VU_FLOOR_DB: f32 = -48.0;
const VU_CEILING_DB: f32 = 0.0;
/// Peak-hold falls at ~12 dB/second — fast enough to feel alive, slow enough
/// that the eye can actually register the peak.
const VU_HOLD_DECAY_DB_PER_SEC: f32 = 12.0;

fn linear_to_db(x: f32) -> f32 {
    // Clamp to floor to avoid log(0); 1e-4 ≈ -80 dB, well below the display range.
    20.0 * x.max(1e-4).log10()
}

fn db_to_unit(db: f32) -> f32 {
    ((db - VU_FLOOR_DB) / (VU_CEILING_DB - VU_FLOOR_DB)).clamp(0.0, 1.0)
}

pub struct SynthApp {
    lock_free_synth: Arc<LockFreeSynth>,
    midi_events: Arc<MidiEventQueue>,
    ui_events: Arc<UiEventQueue>,
    _audio_engine: AudioEngine,
    midi_handler: Option<MidiHandler>,
    keyboard: KeyboardController,
    presets: PresetBrowser,
    show_midi_monitor: bool,
    show_midi_learn: bool,
    show_presets_window: bool,
    show_visualiser: bool,
    params: SynthParameters,
    /// Raw peak read from audio thread (0..1).
    peak_level: f32,
    /// Peak-hold line in dB; decays slowly so the eye can catch transient peaks.
    peak_hold_db: f32,
    /// Last frame instant, to advance the peak-hold decay in real time regardless of frame rate.
    last_frame: Option<std::time::Instant>,
    /// Scope + spectrum analyser state. Preallocates its scratch buffers.
    visualiser: VisualiserState,
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
        Self {
            lock_free_synth,
            midi_events,
            ui_events,
            _audio_engine: audio_engine,
            midi_handler,
            keyboard: KeyboardController::new(),
            presets: PresetBrowser::new(),
            show_midi_monitor: false,
            show_midi_learn: false,
            show_presets_window: false,
            show_visualiser: false,
            params,
            peak_level: 0.0,
            peak_hold_db: VU_FLOOR_DB,
            last_frame: None,
            visualiser: VisualiserState::new(),
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

        // Advance peak-hold: snap upward on a new peak, else decay at a fixed
        // dB/sec rate (a linear-in-dB fall feels more uniform than a scalar decay).
        let now = std::time::Instant::now();
        let dt = match self.last_frame {
            Some(prev) => now.duration_since(prev).as_secs_f32().min(0.1),
            None => 0.0,
        };
        self.last_frame = Some(now);
        let peak_db = linear_to_db(self.peak_level);
        self.peak_hold_db = self.peak_hold_db.max(peak_db) - VU_HOLD_DECAY_DB_PER_SEC * dt;
        if self.peak_hold_db < VU_FLOOR_DB {
            self.peak_hold_db = VU_FLOOR_DB;
        }
        // egui only repaints on input by default; request a repaint so the VU
        // keeps animating even when nothing else changes.
        ctx.request_repaint_after(std::time::Duration::from_millis(16));

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

                // VU meter: dB-scaled LED bar with peak-hold indicator.
                // Log scale makes typical outputs (~-20 dBFS, ≈0.1 linear) sit
                // mid-bar instead of hugging the left edge; peak-hold gives the
                // eye something to track on fast transients.
                let peak_db = linear_to_db(self.peak_level);
                let level_unit = db_to_unit(peak_db);
                let hold_unit = db_to_unit(self.peak_hold_db);
                let clipping = peak_db >= -0.5;
                let (vu_rect, _) =
                    ui.allocate_exact_size(egui::vec2(120.0, 12.0), egui::Sense::hover());
                if ui.is_rect_visible(vu_rect) {
                    let painter = ui.painter();
                    painter.rect_filled(vu_rect, 2.0, egui::Color32::from_gray(20));

                    const SEGMENTS: usize = 24;
                    let seg_w = vu_rect.width() / SEGMENTS as f32;
                    let gap = 1.0_f32;
                    let lit = (level_unit * SEGMENTS as f32).ceil() as usize;
                    for i in 0..SEGMENTS {
                        let seg_db = VU_FLOOR_DB
                            + (i as f32 + 0.5) / SEGMENTS as f32 * (VU_CEILING_DB - VU_FLOOR_DB);
                        let base = if seg_db >= -3.0 {
                            egui::Color32::from_rgb(255, 60, 60)
                        } else if seg_db >= -12.0 {
                            egui::Color32::from_rgb(255, 210, 40)
                        } else {
                            egui::Color32::from_rgb(60, 200, 60)
                        };
                        let color = if i < lit {
                            base
                        } else {
                            egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), 30)
                        };
                        let seg_rect = egui::Rect::from_min_size(
                            egui::pos2(vu_rect.min.x + i as f32 * seg_w, vu_rect.min.y + 1.0),
                            egui::vec2((seg_w - gap).max(1.0), vu_rect.height() - 2.0),
                        );
                        painter.rect_filled(seg_rect, 1.0, color);
                    }

                    // Peak-hold line
                    if self.peak_hold_db > VU_FLOOR_DB {
                        let hold_x = vu_rect.min.x + hold_unit * vu_rect.width();
                        painter.line_segment(
                            [
                                egui::pos2(hold_x, vu_rect.min.y + 1.0),
                                egui::pos2(hold_x, vu_rect.max.y - 1.0),
                            ],
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 255, 255)),
                        );
                    }

                    if clipping {
                        painter.text(
                            vu_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "CLIP",
                            egui::FontId::monospace(9.0),
                            egui::Color32::BLACK,
                        );
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.midi_handler.is_some() {
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

                    if self.midi_handler.is_some() {
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
                        .small_button("Scope")
                        .on_hover_text("Open the waveform / spectrum visualiser")
                        .clicked()
                    {
                        self.show_visualiser = !self.show_visualiser;
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
            let handler = self.midi_handler.as_ref();
            egui::Window::new("MIDI Monitor")
                .default_size([400.0, 300.0])
                .show(ui.ctx(), |ui| {
                    midi_windows::draw_midi_monitor(ui, handler);
                });
        }

        // MIDI Learn Window
        if self.show_midi_learn {
            let learn = self.midi_handler.as_ref().map(|h| &h.learn_state);
            egui::Window::new("MIDI Learn")
                .default_size([280.0, 380.0])
                .show(ui.ctx(), |ui| {
                    midi_windows::draw_midi_learn_panel(ui, learn);
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

        // Visualiser Window
        if self.show_visualiser {
            let mut show = self.show_visualiser;
            let scope = &self.lock_free_synth.scope;
            let viz = &mut self.visualiser;
            egui::Window::new("Visualizer")
                .default_size([360.0, 160.0])
                .resizable(true)
                .open(&mut show)
                .show(ui.ctx(), |ui| {
                    viz.draw(ui, scope);
                });
            self.show_visualiser = show;
        }

        // Write params back at end of frame
        self.lock_free_synth.set_params(self.params);
    }
}

