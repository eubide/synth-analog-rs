use crate::audio_engine::AudioEngine;
use crate::lock_free::{LockFreeSynth, MidiEvent, MidiEventQueue, SynthParameters};
use crate::midi_handler::MidiHandler;
use crate::synthesizer::{ArpPattern, LfoWaveform, Synthesizer, WaveType};
use eframe::egui;
use std::sync::Arc;

pub struct SynthApp {
    lock_free_synth: Arc<LockFreeSynth>,
    midi_events: Arc<MidiEventQueue>,
    _audio_engine: AudioEngine,
    _midi_handler: Option<MidiHandler>,
    last_key_times: std::collections::HashMap<egui::Key, std::time::Instant>,
    current_octave: i32,
    show_midi_monitor: bool,
    show_presets_window: bool,
    current_preset_name: String,
    new_preset_name: String,
    params: SynthParameters,
    peak_level: f32,
}

/// Altura del sub-panel ADSR (title + 4 sliders + curva 32 px).
/// Usada para dimensionar el separador manual y evitar que ui.separator()
/// expanda el grupo con available_height().
const ADSR_PANEL_HEIGHT: f32 = 106.0;

/// Renderiza un grupo con título uniforme — elimina el boilerplate
/// `ui.group { label(title); content }` repetido en cada sección.
fn section(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.group(|ui| {
        ui.label(egui::RichText::new(title).size(11.0).strong());
        add_contents(ui);
    });
}

/// Panel ADSR compacto sin título. Unifica filter/amp envelope que son idénticos
/// salvo los campos que mutan.
fn draw_envelope_panel(
    ui: &mut egui::Ui,
    attack: &mut f32,
    decay: &mut f32,
    sustain: &mut f32,
    release: &mut f32,
) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 1.0);
    ui.spacing_mut().interact_size.y = 14.0;

    ui.horizontal(|ui| {
        ui.label("A:");
        ui.add(egui::Slider::new(attack, 0.001..=2.0).logarithmic(true).step_by(0.001).suffix(" s"));
    });
    ui.horizontal(|ui| {
        ui.label("D:");
        ui.add(egui::Slider::new(decay, 0.001..=3.0).logarithmic(true).step_by(0.001).suffix(" s"));
    });
    ui.horizontal(|ui| {
        ui.label("S:");
        ui.add(egui::Slider::new(sustain, 0.0..=1.0).step_by(0.01));
    });
    ui.horizontal(|ui| {
        ui.label("R:");
        ui.add(egui::Slider::new(release, 0.001..=5.0).logarithmic(true).step_by(0.001).suffix(" s"));
    });
}

impl SynthApp {
    pub fn new(
        lock_free_synth: Arc<LockFreeSynth>,
        midi_events: Arc<MidiEventQueue>,
        audio_engine: AudioEngine,
        midi_handler: Option<MidiHandler>,
    ) -> Self {
        let params = *lock_free_synth.get_params();
        Self {
            lock_free_synth,
            midi_events,
            _audio_engine: audio_engine,
            _midi_handler: midi_handler,
            last_key_times: std::collections::HashMap::new(),
            current_octave: 3, // C3 octave by default
            show_midi_monitor: false,
            show_presets_window: false,
            current_preset_name: String::new(),
            new_preset_name: String::new(),
            params,
            peak_level: 0.0,
        }
    }

    fn draw_vintage_oscillator_panel(&mut self, ui: &mut egui::Ui, osc_num: u8) {
        ui.spacing_mut().item_spacing = egui::vec2(1.0, 1.0);

        let (waveform, detune, pulse_width) = if osc_num == 1 {
            (
                &mut self.params.osc1_waveform,
                &mut self.params.osc1_detune,
                &mut self.params.osc1_pulse_width,
            )
        } else {
            (
                &mut self.params.osc2_waveform,
                &mut self.params.osc2_detune,
                &mut self.params.osc2_pulse_width,
            )
        };

        ui.horizontal(|ui| {
            ui.label("tune:");
            ui.add_sized(
                [70.0, 16.0],
                egui::Slider::new(detune, -24.0..=24.0)
                    .step_by(0.1)
                    .suffix(" st"),
            );
        });

        let mut wave_type = Synthesizer::u8_to_wave_type_pub(*waveform);
        ui.horizontal(|ui| {
            ui.label("wave:");
            egui::ComboBox::from_id_salt(format!("wave_{}", osc_num))
                .selected_text(match wave_type {
                    WaveType::Sawtooth => "Saw",
                    WaveType::Triangle => "Tri",
                    WaveType::Square => "Sqr",
                    WaveType::Sine => "Sin",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut wave_type, WaveType::Sawtooth, "Sawtooth");
                    ui.selectable_value(&mut wave_type, WaveType::Triangle, "Triangle");
                    ui.selectable_value(&mut wave_type, WaveType::Square, "Square");
                    ui.selectable_value(&mut wave_type, WaveType::Sine, "Sine");
                });
        });
        *waveform = Synthesizer::wave_type_to_u8_pub(wave_type);

        if wave_type == WaveType::Square {
            ui.horizontal(|ui| {
                ui.label("pw:");
                ui.add_sized(
                    [70.0, 16.0],
                    egui::Slider::new(pulse_width, 0.1..=0.9).step_by(0.01),
                );
            });
        }

        if osc_num == 2 {
            ui.horizontal(|ui| {
                ui.label("sync:");
                ui.checkbox(&mut self.params.osc2_sync, "→ A");
            });
        }
    }

    fn draw_mixer_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

        ui.horizontal(|ui| {
            ui.label("oscillator A:");
            ui.add(egui::Slider::new(&mut self.params.mixer_osc1_level, 0.0..=1.0).step_by(0.01));
        });

        ui.horizontal(|ui| {
            ui.label("oscillator B:");
            ui.add(egui::Slider::new(&mut self.params.mixer_osc2_level, 0.0..=1.0).step_by(0.01));
        });

        ui.horizontal(|ui| {
            ui.label("noise:");
            ui.add(egui::Slider::new(&mut self.params.noise_level, 0.0..=1.0).step_by(0.01));
        });
    }

    fn draw_prophet_filter_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

        ui.horizontal(|ui| {
            ui.label("cutoff:");
            ui.add(
                egui::Slider::new(&mut self.params.filter_cutoff, 20.0..=20000.0)
                    .logarithmic(true)
                    .step_by(1.0)
                    .suffix(" Hz"),
            );
        });

        ui.horizontal(|ui| {
            ui.label("resonance:");
            ui.add(egui::Slider::new(&mut self.params.filter_resonance, 0.0..=4.0).step_by(0.05));
            if self.params.filter_resonance >= 3.8 {
                ui.colored_label(egui::Color32::from_rgb(255, 160, 60), "self-osc");
            }
        });

        ui.horizontal(|ui| {
            ui.label("envelope:");
            ui.add(
                egui::Slider::new(&mut self.params.filter_envelope_amount, -1.0..=1.0)
                    .step_by(0.01),
            );
        });

        ui.horizontal(|ui| {
            ui.label("keyboard:");
            ui.add(
                egui::Slider::new(&mut self.params.filter_keyboard_tracking, 0.0..=1.0)
                    .step_by(0.01),
            );
        });

        ui.horizontal(|ui| {
            ui.label("velocity:");
            ui.add(
                egui::Slider::new(&mut self.params.velocity_to_cutoff, 0.0..=1.0).step_by(0.01),
            );
        });
    }

    fn draw_vintage_lfo_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(3.0, 1.0);

        // Waveform selector (vintage analog style)
        let mut lfo_waveform = Synthesizer::u8_to_lfo_waveform_pub(self.params.lfo_waveform);
        ui.horizontal(|ui| {
            ui.label("waveform:");
            egui::ComboBox::from_id_salt("lfo_waveform")
                .selected_text(match lfo_waveform {
                    LfoWaveform::Triangle => "Triangle",
                    LfoWaveform::Square => "Square",
                    LfoWaveform::Sawtooth => "Sawtooth",
                    LfoWaveform::ReverseSawtooth => "Rev Saw",
                    LfoWaveform::SampleAndHold => "S&H",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut lfo_waveform, LfoWaveform::Triangle, "Triangle");
                    ui.selectable_value(&mut lfo_waveform, LfoWaveform::Square, "Square");
                    ui.selectable_value(&mut lfo_waveform, LfoWaveform::Sawtooth, "Sawtooth");
                    ui.selectable_value(
                        &mut lfo_waveform,
                        LfoWaveform::ReverseSawtooth,
                        "Reverse Saw",
                    );
                    ui.selectable_value(
                        &mut lfo_waveform,
                        LfoWaveform::SampleAndHold,
                        "Sample & Hold",
                    );
                });
        });
        self.params.lfo_waveform = Synthesizer::lfo_waveform_to_u8_pub(lfo_waveform);

        ui.horizontal(|ui| {
            ui.label("rate:");
            ui.add(
                egui::Slider::new(&mut self.params.lfo_rate, 0.05..=30.0)
                    .logarithmic(true)
                    .step_by(0.05)
                    .suffix(" Hz"),
            );
        });

        ui.horizontal(|ui| {
            ui.label("amount:");
            ui.add(egui::Slider::new(&mut self.params.lfo_amount, 0.0..=1.0).step_by(0.01));
        });

        ui.checkbox(&mut self.params.lfo_sync, "keyboard sync (resets on note)");

        ui.horizontal(|ui| {
            ui.label("delay:");
            ui.add(
                egui::Slider::new(&mut self.params.lfo_delay, 0.0..=5.0)
                    .step_by(0.01)
                    .suffix(" s"),
            );
        });

        ui.separator();
        ui.label(egui::RichText::new("mod destinations").size(10.0).strong());

        // Modulation routing con toggles de target
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.params.lfo_target_filter, "");
                ui.label("filter cutoff:");
                ui.add(
                    egui::Slider::new(&mut self.params.lfo_to_cutoff, 0.0..=1.0).step_by(0.01),
                );
            });
            ui.horizontal(|ui| {
                ui.label("   filter res:");
                ui.add(
                    egui::Slider::new(&mut self.params.lfo_to_resonance, 0.0..=1.0).step_by(0.01),
                );
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.params.lfo_target_osc1_pitch, "");
                ui.label("osc A pitch:");
                ui.add(
                    egui::Slider::new(&mut self.params.lfo_to_osc1_pitch, 0.0..=1.0).step_by(0.01),
                );
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.params.lfo_target_osc2_pitch, "");
                ui.label("osc B pitch:");
                ui.add(
                    egui::Slider::new(&mut self.params.lfo_to_osc2_pitch, 0.0..=1.0).step_by(0.01),
                );
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.params.lfo_target_amplitude, "");
                ui.label("amplitude:");
                ui.add(
                    egui::Slider::new(&mut self.params.lfo_to_amplitude, 0.0..=1.0).step_by(0.01),
                );
            });
        });
    }


    fn draw_master_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        ui.horizontal(|ui| {
            ui.label("volume:");
            ui.add(egui::Slider::new(&mut self.params.master_volume, 0.0..=1.0).step_by(0.01));
        });

        ui.horizontal(|ui| {
            ui.label("glide:");
            ui.add(
                egui::Slider::new(&mut self.params.glide_time, 0.0..=2.0)
                    .step_by(0.01)
                    .suffix(" s"),
            );
        });

        ui.horizontal(|ui| {
            ui.label("bend range:");
            let mut range_f32 = self.params.pitch_bend_range as f32;
            ui.add(egui::Slider::new(&mut range_f32, 1.0..=24.0).step_by(1.0).suffix(" st"));
            self.params.pitch_bend_range = range_f32 as u8;
        });

        ui.separator();
        ui.label("velocity sensitivity");

        ui.horizontal(|ui| {
            ui.label("-> volume:");
            ui.add(
                egui::Slider::new(&mut self.params.velocity_to_amplitude, 0.0..=1.0).step_by(0.01),
            );
        });

        ui.horizontal(|ui| {
            ui.label("vel curve:");
            egui::ComboBox::from_id_salt("velocity_curve")
                .selected_text(match self.params.velocity_curve {
                    1 => "Soft",
                    2 => "Hard",
                    _ => "Linear",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.params.velocity_curve, 0, "Linear");
                    ui.selectable_value(&mut self.params.velocity_curve, 1, "Soft");
                    ui.selectable_value(&mut self.params.velocity_curve, 2, "Hard");
                });
        });

        ui.separator();
        ui.label("aftertouch");

        ui.horizontal(|ui| {
            ui.label("-> cutoff:");
            ui.add(
                egui::Slider::new(&mut self.params.aftertouch_to_cutoff, 0.0..=1.0).step_by(0.01),
            );
        });

        ui.horizontal(|ui| {
            ui.label("-> amp:");
            ui.add(
                egui::Slider::new(&mut self.params.aftertouch_to_amplitude, 0.0..=1.0)
                    .step_by(0.01),
            );
        });
    }

    fn draw_effects_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        ui.label("reverb");
        ui.horizontal(|ui| {
            ui.label("amount:");
            ui.add(egui::Slider::new(&mut self.params.reverb_amount, 0.0..=1.0).step_by(0.01));
        });
        ui.horizontal(|ui| {
            ui.label("size:");
            ui.add(egui::Slider::new(&mut self.params.reverb_size, 0.0..=1.0).step_by(0.01));
        });

        ui.label("delay");
        ui.horizontal(|ui| {
            ui.label("time:");
            ui.add(
                egui::Slider::new(&mut self.params.delay_time, 0.01..=2.0)
                    .step_by(0.01)
                    .suffix(" s"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("feedback:");
            ui.add(egui::Slider::new(&mut self.params.delay_feedback, 0.0..=0.95).step_by(0.01));
        });
        ui.horizontal(|ui| {
            ui.label("amount:");
            ui.add(egui::Slider::new(&mut self.params.delay_amount, 0.0..=1.0).step_by(0.01));
        });
    }

    fn draw_arpeggiator_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        ui.checkbox(&mut self.params.arp_enabled, "enable");

        ui.horizontal(|ui| {
            ui.label("rate:");
            ui.add(
                egui::Slider::new(&mut self.params.arp_rate, 60.0..=240.0)
                    .step_by(1.0)
                    .suffix(" BPM"),
            );
        });

        ui.horizontal(|ui| {
            ui.label("pattern:");
            let mut arp_pattern = Synthesizer::u8_to_arp_pattern_pub(self.params.arp_pattern);
            let pattern_text = match arp_pattern {
                ArpPattern::Up => "Up",
                ArpPattern::Down => "Down",
                ArpPattern::UpDown => "Up-Down",
                ArpPattern::Random => "Random",
            };

            egui::ComboBox::from_label("")
                .selected_text(pattern_text)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut arp_pattern, ArpPattern::Up, "Up");
                    ui.selectable_value(&mut arp_pattern, ArpPattern::Down, "Down");
                    ui.selectable_value(&mut arp_pattern, ArpPattern::UpDown, "Up-Down");
                    ui.selectable_value(&mut arp_pattern, ArpPattern::Random, "Random");
                });
            self.params.arp_pattern = Synthesizer::arp_pattern_to_u8_pub(arp_pattern);
        });

        ui.horizontal(|ui| {
            ui.label("octaves:");
            let mut octaves_f32 = self.params.arp_octaves as f32;
            ui.add(egui::Slider::new(&mut octaves_f32, 1.0..=4.0).step_by(1.0));
            self.params.arp_octaves = octaves_f32 as u8;
        });

        ui.horizontal(|ui| {
            ui.label("gate:");
            ui.add(egui::Slider::new(&mut self.params.arp_gate_length, 0.1..=1.0).step_by(0.01));
        });

        ui.separator();
        ui.checkbox(&mut self.params.arp_sync_to_midi, "sync to MIDI clock");
    }

    fn draw_voice_mode_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

        ui.horizontal(|ui| {
            ui.label("mode:");
            egui::ComboBox::from_id_salt("voice_mode")
                .selected_text(match self.params.voice_mode {
                    1 => "Mono",
                    2 => "Legato",
                    3 => "Unison",
                    _ => "Poly",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.params.voice_mode, 0, "Poly");
                    ui.selectable_value(&mut self.params.voice_mode, 1, "Mono");
                    ui.selectable_value(&mut self.params.voice_mode, 2, "Legato");
                    ui.selectable_value(&mut self.params.voice_mode, 3, "Unison");
                });
        });

        // Note priority — solo relevante en Mono/Legato
        if self.params.voice_mode == 1 || self.params.voice_mode == 2 {
            ui.horizontal(|ui| {
                ui.label("priority:");
                egui::ComboBox::from_id_salt("note_priority")
                    .selected_text(match self.params.note_priority {
                        1 => "Low",
                        2 => "High",
                        _ => "Last",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.params.note_priority, 0, "Last");
                        ui.selectable_value(&mut self.params.note_priority, 1, "Low");
                        ui.selectable_value(&mut self.params.note_priority, 2, "High");
                    });
            });
        }

        // Unison spread — solo relevante en Unison
        if self.params.voice_mode == 3 {
            ui.horizontal(|ui| {
                ui.label("spread:");
                ui.add(
                    egui::Slider::new(&mut self.params.unison_spread, 0.0..=50.0)
                        .step_by(0.5)
                        .suffix(" c"),
                );
            });
        }

        ui.horizontal(|ui| {
            ui.label("voices:");
            let mut max_v = self.params.max_voices as f32;
            ui.add(egui::Slider::new(&mut max_v, 1.0..=8.0).step_by(1.0));
            self.params.max_voices = max_v as u8;
        });
    }

    fn draw_preset_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        // Show current preset status
        ui.horizontal(|ui| {
            ui.label("current:");
            if self.current_preset_name.is_empty() {
                ui.colored_label(egui::Color32::GRAY, "no preset loaded");
            } else {
                ui.colored_label(egui::Color32::GREEN, &self.current_preset_name);
            }
        });

        ui.separator();

        // Create new preset
        ui.horizontal(|ui| {
            ui.label("new name:");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_preset_name)
                    .hint_text("Enter name...")
                    .desired_width(80.0),
            );

            let save_enabled = !self.new_preset_name.is_empty();
            if ui
                .add_enabled(save_enabled, egui::Button::new("Save"))
                .clicked()
            {
                let mut temp_synth = Synthesizer::new();
                temp_synth.apply_params(&self.params);
                if let Err(e) = temp_synth.save_preset(&self.new_preset_name) {
                    log::error!("Error saving preset: {}", e);
                } else {
                    log::info!("Preset '{}' saved!", self.new_preset_name);
                    self.current_preset_name = self.new_preset_name.clone();
                    self.new_preset_name.clear();
                }
            }
        });

        ui.horizontal(|ui| {
            if ui.button("save default").clicked() {
                let mut temp_synth = Synthesizer::new();
                temp_synth.apply_params(&self.params);
                if let Err(e) = temp_synth.save_preset("default") {
                    log::error!("Error saving default: {}", e);
                } else {
                    log::info!("Default preset saved!");
                    self.current_preset_name = "default".to_string();
                }
            }

            if ui.button("load default").clicked() {
                let mut temp_synth = Synthesizer::new();
                if let Err(e) = temp_synth.load_preset("default") {
                    log::error!("Error loading default: {}", e);
                } else {
                    log::info!("Default preset loaded!");
                    self.params = temp_synth.to_synth_params();
                    self.current_preset_name = "default".to_string();
                }
            }
        });

        if ui.button("create classic presets").clicked() {
            let mut temp_synth = Synthesizer::new();
            if let Err(e) = temp_synth.create_all_classic_presets() {
                log::error!("Error creating classic presets: {}", e);
            } else {
                log::info!("All classic presets created successfully!");
            }
        }

        ui.separator();

        // Show all presets in a scrollable area
        let presets = Synthesizer::list_presets();
        if !presets.is_empty() {
            ui.label("saved presets");

            egui::ScrollArea::vertical()
                .max_height(100.0)
                .show(ui, |ui| {
                    for preset in presets.iter() {
                        let is_current = preset == &self.current_preset_name;
                        let button_text = if is_current {
                            format!("> {}", preset)
                        } else {
                            preset.clone()
                        };

                        let button = egui::Button::new(button_text);
                        let button = if is_current {
                            button.fill(egui::Color32::from_rgb(100, 150, 100))
                        } else {
                            button
                        };

                        if ui.add_sized([ui.available_width(), 18.0], button).clicked() {
                            let mut temp_synth = Synthesizer::new();
                            if let Err(e) = temp_synth.load_preset(preset) {
                                log::error!("Error loading preset {}: {}", preset, e);
                            } else {
                                log::info!("Preset '{}' loaded!", preset);
                                self.params = temp_synth.to_synth_params();
                                self.current_preset_name = preset.clone();
                            }
                        }
                    }
                });
        } else {
            ui.label("no saved presets yet");
        }
    }

    /// Mini ADSR curve — 22px alto, actualiza en tiempo real con los sliders.
    fn draw_adsr_curve(&self, ui: &mut egui::Ui, attack: f32, decay: f32, sustain: f32, release: f32) {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 32.0),
            egui::Sense::hover(),
        );
        if !ui.is_rect_visible(rect) {
            return;
        }
        let painter = ui.painter();
        painter.rect_filled(rect, 2.0, egui::Color32::from_gray(18));

        let sustain_hold = 0.3_f32;
        let total = (attack + decay + sustain_hold + release).max(0.01);
        let w = rect.width();
        let h = rect.height() - 4.0;
        let top = rect.top() + 2.0;
        let bot = rect.bottom() - 2.0;
        let left = rect.left();

        let xa = left + (attack / total) * w;
        let xd = xa + (decay / total) * w;
        let xs = xd + (sustain_hold / total) * w;
        let xr = xs + (release / total) * w;
        let ys = bot - sustain * h;

        let pts = [
            egui::pos2(left, bot),
            egui::pos2(xa, top),
            egui::pos2(xd, ys),
            egui::pos2(xs, ys),
            egui::pos2(xr, bot),
        ];
        let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(80, 200, 80));
        for pair in pts.windows(2) {
            painter.line_segment([pair[0], pair[1]], stroke);
        }
    }

    /// Poly Mod section — 3 rutas de modulación clásicas del Prophet-5.
    fn draw_poly_mod_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

        ui.label(
            egui::RichText::new("Filter Env →")
                .size(10.0)
                .color(egui::Color32::GRAY),
        );
        ui.horizontal(|ui| {
            ui.label("freq A:");
            ui.add(
                egui::Slider::new(&mut self.params.poly_mod_filter_env_to_osc_a_freq, -1.0..=1.0)
                    .step_by(0.01),
            );
        });
        ui.horizontal(|ui| {
            ui.label("pw A:");
            ui.add(
                egui::Slider::new(&mut self.params.poly_mod_filter_env_to_osc_a_pw, -1.0..=1.0)
                    .step_by(0.01),
            );
        });

        ui.separator();

        ui.label(
            egui::RichText::new("Osc B →")
                .size(10.0)
                .color(egui::Color32::GRAY),
        );
        ui.horizontal(|ui| {
            ui.label("freq A:");
            ui.add(
                egui::Slider::new(&mut self.params.poly_mod_osc_b_to_osc_a_freq, -1.0..=1.0)
                    .step_by(0.01),
            );
        });
        ui.horizontal(|ui| {
            ui.label("pw A:");
            ui.add(
                egui::Slider::new(&mut self.params.poly_mod_osc_b_to_osc_a_pw, -1.0..=1.0)
                    .step_by(0.01),
            );
        });
        ui.horizontal(|ui| {
            ui.label("cutoff:");
            ui.add(
                egui::Slider::new(
                    &mut self.params.poly_mod_osc_b_to_filter_cutoff,
                    -1.0..=1.0,
                )
                .step_by(0.01),
            );
        });
    }

    fn draw_keyboard_legend(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Octave indicator
            ui.vertical(|ui| {
                ui.set_min_width(72.0);
                ui.label(
                    egui::RichText::new(format!("Oct: {}", self.current_octave))
                        .size(12.0)
                        .strong()
                        .color(egui::Color32::from_rgb(255, 220, 80)),
                );
                ui.label(
                    egui::RichText::new("↑/↓ to change")
                        .size(9.0)
                        .color(egui::Color32::GRAY),
                );
            });

            ui.separator();

            // Lower octave — visual QWERTY layout (black keys row, white keys row)
            ui.vertical(|ui| {
                ui.set_min_width(175.0);
                ui.label(
                    egui::RichText::new(
                        format!("  S   D     G   H   J      ← oct {}", self.current_octave),
                    )
                    .size(10.0)
                    .monospace()
                    .color(egui::Color32::from_gray(155)),
                );
                ui.label(
                    egui::RichText::new("Z   X   C   V   B   N   M")
                        .size(10.0)
                        .monospace()
                        .color(egui::Color32::WHITE),
                );
            });

            ui.separator();

            // Upper octave
            ui.vertical(|ui| {
                ui.set_min_width(215.0);
                ui.label(
                    egui::RichText::new(
                        format!("  2   3     5   6   7        ← oct {}", self.current_octave + 1),
                    )
                    .size(10.0)
                    .monospace()
                    .color(egui::Color32::from_gray(155)),
                );
                ui.label(
                    egui::RichText::new("Q   W   E   R   T   Y   U   I   O   P")
                        .size(10.0)
                        .monospace()
                        .color(egui::Color32::WHITE),
                );
            });
        });
    }
}

impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Read current params at start of frame
        self.params = *self.lock_free_synth.get_params();
        let peak_bits = self.lock_free_synth.peak_level.load(std::sync::atomic::Ordering::Relaxed);
        self.peak_level = f32::from_bits(peak_bits);

        // Handle keyboard input
        ctx.input(|i| {
            // Handle octave changes
            if i.key_pressed(egui::Key::ArrowUp) {
                self.current_octave = (self.current_octave + 1).clamp(0, 8);
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                self.current_octave = (self.current_octave - 1).clamp(0, 8);
            }

            // Lower octave: Z-M row (white keys) + S,D,G,H,J (black keys)
            // Upper octave: Q-P row (white keys) + 2,3,5,6,7 (black keys)
            let key_map = [
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
                (egui::Key::Q, 12),    // C
                (egui::Key::Num2, 13), // C#
                (egui::Key::W, 14),    // D
                (egui::Key::Num3, 15), // D#
                (egui::Key::E, 16),    // E
                (egui::Key::R, 17),    // F
                (egui::Key::Num5, 18), // F#
                (egui::Key::T, 19),    // G
                (egui::Key::Num6, 20), // G#
                (egui::Key::Y, 21),    // A
                (egui::Key::Num7, 22), // A#
                (egui::Key::U, 23),    // B
                (egui::Key::I, 24),    // C (next)
                (egui::Key::Num9, 25), // C# (next)
                (egui::Key::O, 26),    // D (next)
                (egui::Key::Num0, 27), // D# (next)
                (egui::Key::P, 28),    // E (next)
            ];

            let now = std::time::Instant::now();

            for (key, note_offset) in key_map {
                let midi_note = self.current_octave * 12 + note_offset;

                if i.key_pressed(key) {
                    let should_trigger = if let Some(last_time) = self.last_key_times.get(&key) {
                        // If more than 100ms since last press, it's intentional (not auto-repeat)
                        now.duration_since(*last_time).as_millis() > 100
                    } else {
                        // First time pressing this key
                        true
                    };

                    if should_trigger {
                        self.last_key_times.insert(key, now);
                        self.midi_events.push(MidiEvent::NoteOn {
                            note: midi_note as u8,
                            velocity: 100,
                        });
                    }
                }

                if i.key_released(key) {
                    self.last_key_times.remove(&key);
                    self.midi_events.push(MidiEvent::NoteOff {
                        note: midi_note as u8,
                    });
                }
            }
        });
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
            let (vu_rect, _) = ui.allocate_exact_size(egui::vec2(80.0, 12.0), egui::Sense::hover());
            if ui.is_rect_visible(vu_rect) {
                let painter = ui.painter();
                painter.rect_filled(vu_rect, 2.0, egui::Color32::from_gray(25));
                let filled_w = vu_rect.width() * peak.min(1.0);
                painter.rect_filled(
                    egui::Rect::from_min_size(vu_rect.min, egui::vec2(filled_w, vu_rect.height())),
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
                    if ui.small_button("MIDI").clicked() {
                        self.show_midi_monitor = !self.show_midi_monitor;
                    }
                } else {
                    let _ = ui.small_button("NO MIDI");
                }

                if ui.small_button("Presets").clicked() {
                    self.show_presets_window = !self.show_presets_window;
                }
            });
        });
        ui.separator();

        // Ventana de tamaño fijo — no se necesita ScrollArea
        {
            // Cadena de señal Prophet-5: [OSC A+B] | [MIXER+FILTER+POLY MOD] | [ENVS+LFO] | [MASTER+ARP+EFFECTS]
            // Anchos explícitos (no ui.columns que divide por igual) — layout fijo sin expansión
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                // ── COL 1: FUENTES DE SONIDO (175 px) ──────────────────
                ui.vertical(|ui| {
                    ui.set_min_width(175.0);
                    ui.set_max_width(175.0);
                    section(ui, "OSCILLATOR A", |ui| self.draw_vintage_oscillator_panel(ui, 1));
                    ui.add_space(4.0);
                    section(ui, "OSCILLATOR B", |ui| self.draw_vintage_oscillator_panel(ui, 2));
                });

                // ── COL 2: CADENA DE SEÑAL (220 px) ────────────────────
                ui.vertical(|ui| {
                    ui.set_min_width(220.0);
                    ui.set_max_width(220.0);
                    section(ui, "MIXER", |ui| self.draw_mixer_panel(ui));
                    ui.add_space(4.0);
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("FILTER").size(11.0).strong());
                            ui.label(
                                egui::RichText::new("24dB").size(9.0).color(egui::Color32::GRAY),
                            );
                        });
                        self.draw_prophet_filter_panel(ui);
                    });
                    ui.add_space(4.0);
                    section(ui, "POLY MOD", |ui| self.draw_poly_mod_panel(ui));
                });

                // ── COL 3: TIEMPO Y MODULACIÓN (360 px) ────────────────
                ui.vertical(|ui| {
                    ui.set_min_width(360.0);
                    ui.set_max_width(360.0);
                    ui.group(|ui| {
                        // Dos ADSR lado a lado con anchos explícitos
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.vertical(|ui| {
                                ui.set_min_width(165.0);
                                ui.set_max_width(165.0);
                                ui.label(egui::RichText::new("FILTER ENV").size(11.0).strong());
                                draw_envelope_panel(ui, &mut self.params.filter_attack, &mut self.params.filter_decay, &mut self.params.filter_sustain, &mut self.params.filter_release);
                                self.draw_adsr_curve(
                                    ui,
                                    self.params.filter_attack,
                                    self.params.filter_decay,
                                    self.params.filter_sustain,
                                    self.params.filter_release,
                                );
                            });
                            {
                                // Separator vertical con altura fija — ui.separator() usa
                                // available_height() y expande el grupo hasta el fondo
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(1.0, ADSR_PANEL_HEIGHT),
                                    egui::Sense::empty(),
                                );
                                ui.painter().line_segment(
                                    [rect.center_top(), rect.center_bottom()],
                                    ui.style().visuals.widgets.noninteractive.bg_stroke,
                                );
                            }
                            ui.vertical(|ui| {
                                ui.set_min_width(165.0);
                                ui.set_max_width(165.0);
                                ui.label(egui::RichText::new("AMP ENV").size(11.0).strong());
                                draw_envelope_panel(ui, &mut self.params.amp_attack, &mut self.params.amp_decay, &mut self.params.amp_sustain, &mut self.params.amp_release);
                                self.draw_adsr_curve(
                                    ui,
                                    self.params.amp_attack,
                                    self.params.amp_decay,
                                    self.params.amp_sustain,
                                    self.params.amp_release,
                                );
                            });
                        });
                    });
                    ui.add_space(4.0);
                    section(ui, "LFO", |ui| self.draw_vintage_lfo_panel(ui));
                });

                // ── COL 4: PERFORMANCE (160 px) ─────────────────────────
                ui.vertical(|ui| {
                    ui.set_min_width(160.0);
                    ui.set_max_width(160.0);
                    section(ui, "MASTER", |ui| self.draw_master_panel(ui));
                    ui.add_space(4.0);
                    section(ui, "ARP", |ui| self.draw_arpeggiator_panel(ui));
                    ui.add_space(4.0);
                    section(ui, "VOICE MODE", |ui| self.draw_voice_mode_panel(ui));
                    ui.add_space(4.0);
                    section(ui, "EFFECTS", |ui| self.draw_effects_panel(ui));
                    ui.add_space(4.0);
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("PRESET").size(11.0).strong());
                        ui.label(
                            egui::RichText::new(if self.current_preset_name.is_empty() {
                                "default"
                            } else {
                                &self.current_preset_name
                            })
                            .size(10.0)
                            .color(egui::Color32::from_rgb(100, 220, 100)),
                        );
                        if ui.small_button("manage...").clicked() {
                            self.show_presets_window = !self.show_presets_window;
                        }
                    });
                });
            });

            ui.add_space(4.0);

            // KEYBOARD REFERENCE — barra compacta de 2 líneas
            ui.group(|ui| {
                self.draw_keyboard_legend(ui);
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

        // Presets Window
        if self.show_presets_window {
            let mut show_presets_window = self.show_presets_window;
            egui::Window::new("Preset Manager")
                .default_size([350.0, 400.0])
                .open(&mut show_presets_window)
                .show(ui.ctx(), |ui| {
                    self.draw_preset_panel(ui);
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
}
