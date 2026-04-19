//! Preset browser component.
//!
//! Absorbe todo el estado ligado a la gestión de patches: búsqueda, filtro por
//! categoría, nombre actual, editor de creación y los dos slots A/B de
//! comparación. El A/B vive aquí (y no en un componente propio) porque los
//! controles están embebidos en el mismo panel y comparten flujo: guardar el
//! patch "current", luego probar variaciones. Separarlo sería purismo, no
//! claridad.

use crate::lock_free::SynthParameters;
use crate::synthesizer::Synthesizer;
use eframe::egui;

/// Categorías usadas por los presets built-in y de usuario. El orden define
/// cómo se agrupan visualmente en el browser y cómo aparecen en el combo de
/// guardado.
pub const PRESET_CATEGORIES: &[&str] = &[
    "Bass", "Lead", "Pad", "Strings", "Brass", "FX", "Sequence", "Other",
];

pub struct PresetBrowser {
    pub current_name: String,
    search: String,
    category_filter: String,
    /// Categoría seleccionada al guardar un patch nuevo.
    save_category: String,
    new_name: String,
    editor_open: bool,
    /// Slot A del A/B comparison. `None` hasta que el usuario pulse "-> A".
    slot_a: Option<SynthParameters>,
    slot_b: Option<SynthParameters>,
}

impl PresetBrowser {
    pub fn new() -> Self {
        Self {
            current_name: String::new(),
            search: String::new(),
            category_filter: "All".to_string(),
            save_category: "Other".to_string(),
            new_name: String::new(),
            editor_open: false,
            slot_a: None,
            slot_b: None,
        }
    }

    /// Expone `current_name` de solo lectura para el header principal que lo
    /// muestra clicable fuera de este panel.
    pub fn current_name(&self) -> &str {
        &self.current_name
    }

    /// Intenta cargar el preset `name`. Actualiza `params` y `current_name` en
    /// caso de éxito. Loggea error sin propagar para no reventar la UI.
    pub fn load(&mut self, name: &str, params: &mut SynthParameters) {
        let mut temp_synth = Synthesizer::new();
        match temp_synth.load_preset(name) {
            Ok(_) => {
                *params = temp_synth.to_synth_params();
                self.current_name = name.to_string();
            }
            Err(e) => log::error!("Error loading preset {}: {}", name, e),
        }
    }

    /// Actualiza el nombre visible tras una carga externa (p.ej. Program
    /// Change MIDI o SysEx) para que el browser refleje lo que sonó.
    pub fn set_current(&mut self, name: String) {
        self.current_name = name;
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, params: &mut SynthParameters) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

        // Header: preset actual — siempre visible arriba.
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("current:").size(11.0).strong());
            if self.current_name.is_empty() {
                ui.colored_label(egui::Color32::GRAY, "(no preset loaded)");
            } else {
                ui.colored_label(egui::Color32::from_rgb(100, 220, 100), &self.current_name);
            }
        });
        ui.separator();

        self.draw_filters(ui);
        ui.separator();
        self.draw_preset_list(ui, params);
        ui.separator();
        self.draw_editor(ui, params);
    }

    fn draw_filters(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("search:");
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("type to filter...")
                    .desired_width(140.0),
            );
            if ui.small_button("x").on_hover_text("Clear search").clicked() {
                self.search.clear();
            }
        });

        ui.horizontal(|ui| {
            ui.label("category:");
            egui::ComboBox::from_id_salt("preset_cat_filter")
                .selected_text(&self.category_filter)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.category_filter, "All".to_string(), "All");
                    for cat in PRESET_CATEGORIES {
                        ui.selectable_value(&mut self.category_filter, cat.to_string(), *cat);
                    }
                });
        });
    }

    fn draw_preset_list(&mut self, ui: &mut egui::Ui, params: &mut SynthParameters) {
        let all_presets = Synthesizer::list_presets_with_categories();
        let search_lower = self.search.to_lowercase();
        let filtered: Vec<(String, String)> = all_presets
            .into_iter()
            .filter(|(name, cat)| {
                let cat_ok = self.category_filter == "All" || cat == &self.category_filter;
                let name_ok = search_lower.is_empty() || name.to_lowercase().contains(&search_lower);
                cat_ok && name_ok
            })
            .collect();

        if filtered.is_empty() {
            ui.label(
                egui::RichText::new("no presets match")
                    .color(egui::Color32::GRAY)
                    .italics(),
            );
            return;
        }

        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                let mut last_category: Option<&str> = None;
                for (preset, category) in &filtered {
                    if last_category != Some(category.as_str()) {
                        if last_category.is_some() {
                            ui.add_space(4.0);
                        }
                        ui.label(
                            egui::RichText::new(category.to_uppercase())
                                .size(10.0)
                                .color(egui::Color32::from_rgb(180, 180, 80))
                                .strong(),
                        );
                        last_category = Some(category.as_str());
                    }

                    let is_current = preset == &self.current_name;
                    let button = egui::Button::new(preset).wrap_mode(egui::TextWrapMode::Truncate);
                    let button = if is_current {
                        button.fill(egui::Color32::from_rgb(100, 150, 100))
                    } else {
                        button
                    };
                    if ui.add_sized([ui.available_width(), 18.0], button).clicked() {
                        self.load(preset, params);
                    }
                }
            });
    }

    fn draw_editor(&mut self, ui: &mut egui::Ui, params: &mut SynthParameters) {
        egui::CollapsingHeader::new("Create / Edit")
            .id_salt("preset_editor_section")
            .default_open(self.editor_open)
            .show(ui, |ui| {
                // Guardar patch actual como preset nuevo.
                ui.horizontal(|ui| {
                    ui.label("category:");
                    egui::ComboBox::from_id_salt("preset_cat_save")
                        .selected_text(&self.save_category)
                        .show_ui(ui, |ui| {
                            for cat in PRESET_CATEGORIES {
                                ui.selectable_value(&mut self.save_category, cat.to_string(), *cat);
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_name)
                            .hint_text("preset name...")
                            .desired_width(130.0),
                    );
                    let save_enabled = !self.new_name.is_empty();
                    if ui
                        .add_enabled(save_enabled, egui::Button::new("Save"))
                        .clicked()
                    {
                        let mut temp_synth = Synthesizer::new();
                        temp_synth.apply_params(params);
                        if let Err(e) =
                            temp_synth.save_preset_with_category(&self.new_name, &self.save_category)
                        {
                            log::error!("Error saving preset: {}", e);
                        } else {
                            log::info!("Preset '{}' [{}] saved!", self.new_name, self.save_category);
                            self.current_name = self.new_name.clone();
                            self.new_name.clear();
                        }
                    }
                });

                ui.separator();
                self.draw_ab_comparison(ui, params);
                ui.separator();
                self.draw_utilities(ui, params);
            });
    }

    fn draw_ab_comparison(&mut self, ui: &mut egui::Ui, params: &mut SynthParameters) {
        ui.label(egui::RichText::new("A/B comparison").size(10.0).strong());
        ui.horizontal(|ui| {
            if ui
                .button("-> A")
                .on_hover_text("Store current patch to slot A")
                .clicked()
            {
                self.slot_a = Some(*params);
            }
            if ui
                .add_enabled(self.slot_a.is_some(), egui::Button::new("A"))
                .on_hover_text("Load slot A")
                .clicked()
            {
                *params = self.slot_a.unwrap();
            }
            ui.separator();
            if ui
                .button("-> B")
                .on_hover_text("Store current patch to slot B")
                .clicked()
            {
                self.slot_b = Some(*params);
            }
            if ui
                .add_enabled(self.slot_b.is_some(), egui::Button::new("B"))
                .on_hover_text("Load slot B")
                .clicked()
            {
                *params = self.slot_b.unwrap();
            }
        });
    }

    fn draw_utilities(&mut self, ui: &mut egui::Ui, params: &mut SynthParameters) {
        if ui.button("Random patch").clicked() {
            *params = random_params();
            self.current_name.clear();
        }
        ui.horizontal(|ui| {
            if ui.button("save default").clicked() {
                let mut temp_synth = Synthesizer::new();
                temp_synth.apply_params(params);
                if let Err(e) = temp_synth.save_preset("default") {
                    log::error!("Error saving default: {}", e);
                } else {
                    log::info!("Default preset saved!");
                    self.current_name = "default".to_string();
                }
            }
            if ui.button("load default").clicked() {
                let mut temp_synth = Synthesizer::new();
                if let Err(e) = temp_synth.load_preset("default") {
                    log::error!("Error loading default: {}", e);
                } else {
                    log::info!("Default preset loaded!");
                    *params = temp_synth.to_synth_params();
                    self.current_name = "default".to_string();
                }
            }
        });
        if ui
            .button("create classic presets")
            .on_hover_text("Force-regenerate the 32 built-in presets, overwriting existing files")
            .clicked()
        {
            let mut temp_synth = Synthesizer::new();
            if let Err(e) = temp_synth.force_create_all_classic_presets() {
                log::error!("Error creating classic presets: {}", e);
            } else {
                log::info!("All classic presets created successfully!");
            }
        }
    }
}

impl Default for PresetBrowser {
    fn default() -> Self {
        Self::new()
    }
}

/// Genera un patch con valores aleatorios acotados a rangos musicales
/// razonables (no todo [0,1]: algunos parámetros suenan mal en extremos).
fn random_params() -> SynthParameters {
    // rand 0.10 quitó thread_rng/gen_range; usamos rand::random::<f32>() directo.
    let r = |lo: f32, hi: f32| lo + rand::random::<f32>() * (hi - lo);
    let ri = |n: u8| (rand::random::<f32>() * (n as f32 + 1.0)) as u8;
    SynthParameters {
        osc1_waveform: ri(3),
        osc2_waveform: ri(3),
        osc1_detune: r(-12.0, 12.0),
        osc2_detune: r(-12.0, 12.0),
        osc1_pulse_width: r(0.1, 0.9),
        osc2_pulse_width: r(0.1, 0.9),
        mixer_osc1_level: r(0.5, 1.0),
        mixer_osc2_level: r(0.0, 0.8),
        noise_level: r(0.0, 0.1),
        filter_cutoff: {
            let log_min = 200.0_f32.ln();
            let log_max = 12000.0_f32.ln();
            (log_min + rand::random::<f32>() * (log_max - log_min)).exp()
        },
        filter_resonance: r(0.0, 3.0),
        filter_envelope_amount: r(0.0, 0.8),
        filter_keyboard_tracking: r(0.0, 1.0),
        amp_attack: r(0.001, 0.5),
        amp_decay: r(0.05, 1.0),
        amp_sustain: r(0.3, 1.0),
        amp_release: r(0.05, 1.5),
        filter_attack: r(0.001, 0.5),
        filter_decay: r(0.05, 1.0),
        filter_sustain: r(0.3, 1.0),
        filter_release: r(0.05, 1.5),
        lfo_rate: r(0.1, 8.0),
        lfo_amount: r(0.0, 0.5),
        lfo_waveform: ri(4),
        master_volume: 0.7,
        ..SynthParameters::default()
    }
}
