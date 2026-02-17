use eframe::egui;
use std::sync::Arc;
use crate::lock_free::{LockFreeSynth, SynthParameters, MidiEvent, MidiEventQueue};
use crate::synthesizer::{WaveType, ArpPattern, LfoWaveform, Synthesizer};
use crate::audio_engine::AudioEngine;
use crate::midi_handler::MidiHandler;

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
            current_octave: 4, // C4 octave by default
            show_midi_monitor: false,
            show_presets_window: false,
            current_preset_name: String::new(),
            new_preset_name: String::new(),
            params,
        }
    }

    fn draw_vintage_oscillator_panel(&mut self, ui: &mut egui::Ui, osc_num: u8) {
        ui.spacing_mut().item_spacing = egui::vec2(1.0, 1.0);

        let (waveform, detune, pulse_width, amplitude) = if osc_num == 1 {
            (&mut self.params.osc1_waveform, &mut self.params.osc1_detune,
             &mut self.params.osc1_pulse_width, &mut self.params.osc1_level)
        } else {
            (&mut self.params.osc2_waveform, &mut self.params.osc2_detune,
             &mut self.params.osc2_pulse_width, &mut self.params.osc2_level)
        };

        // Frequency controls
        ui.horizontal(|ui| {
            ui.label("freq:");
            ui.add_sized([70.0, 16.0], egui::Slider::new(detune, -12.0..=12.0)
                .step_by(0.1)
                .suffix(" st"));
        });

        // Wave type selector
        let mut wave_type = Synthesizer::u8_to_wave_type_pub(*waveform);
        ui.horizontal(|ui| {
            ui.label("wave:");
            egui::ComboBox::from_id_source(format!("wave_{}", osc_num))
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

        // Pulse Width (only for square waves)
        if wave_type == WaveType::Square {
            ui.horizontal(|ui| {
                ui.label("pw:");
                ui.add_sized([70.0, 16.0], egui::Slider::new(pulse_width, 0.1..=0.9)
                    .step_by(0.01));
            });
        }

        // Level control (always available)
        ui.horizontal(|ui| {
            ui.label("level:");
            ui.add_sized([70.0, 16.0], egui::Slider::new(amplitude, 0.0..=1.0)
                .step_by(0.01));
        });

        // Sync control (only for oscillator B)
        if osc_num == 2 {
            ui.horizontal(|ui| {
                ui.label("sync:");
                ui.checkbox(&mut self.params.osc2_sync, "oscillator A");
            });
        }
    }

    fn draw_mixer_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

        ui.horizontal(|ui| {
            ui.label("oscillator A:");
            ui.add(egui::Slider::new(&mut self.params.mixer_osc1_level, 0.0..=1.0)
                .step_by(0.01)
                .text("Level"));
        });

        ui.horizontal(|ui| {
            ui.label("oscillator B:");
            ui.add(egui::Slider::new(&mut self.params.mixer_osc2_level, 0.0..=1.0)
                .step_by(0.01)
                .text("Level"));
        });

        ui.horizontal(|ui| {
            ui.label("noise:");
            ui.add(egui::Slider::new(&mut self.params.noise_level, 0.0..=1.0)
                .step_by(0.01)
                .text("Level"));
        });
    }

    fn draw_prophet_filter_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

        ui.horizontal(|ui| {
            ui.label("cutoff:");
            ui.add_sized([120.0, 20.0], egui::Slider::new(&mut self.params.filter_cutoff, 20.0..=20000.0)
                .logarithmic(true)
                .step_by(1.0)
                .suffix(" Hz"));
        });

        ui.horizontal(|ui| {
            ui.label("resonance:");
            ui.add_sized([100.0, 20.0], egui::Slider::new(&mut self.params.filter_resonance, 0.0..=4.0)
                .step_by(0.05));
            if self.params.filter_resonance >= 3.8 {
                ui.label("Self-osc");
            }
        });

        ui.horizontal(|ui| {
            ui.label("envelope:");
            ui.add_sized([100.0, 20.0], egui::Slider::new(&mut self.params.filter_envelope_amount, -1.0..=1.0)
                .step_by(0.01));
        });

        ui.horizontal(|ui| {
            ui.label("keyboard:");
            ui.add_sized([100.0, 20.0], egui::Slider::new(&mut self.params.filter_keyboard_tracking, 0.0..=1.0)
                .step_by(0.01));
        });

        ui.horizontal(|ui| {
            ui.label("velocity:");
            ui.add_sized([100.0, 20.0], egui::Slider::new(&mut self.params.velocity_to_cutoff, 0.0..=1.0)
                .step_by(0.01));
        });

        ui.label("FILTER ENVELOPE");
        ui.horizontal(|ui| {
            ui.label("A:");
            ui.add_sized([100.0, 20.0], egui::Slider::new(&mut self.params.filter_attack, 0.001..=2.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });
        ui.horizontal(|ui| {
            ui.label("D:");
            ui.add_sized([100.0, 20.0], egui::Slider::new(&mut self.params.filter_decay, 0.001..=3.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });
        ui.horizontal(|ui| {
            ui.label("S:");
            ui.add_sized([100.0, 20.0], egui::Slider::new(&mut self.params.filter_sustain, 0.0..=1.0)
                .step_by(0.01));
        });
        ui.horizontal(|ui| {
            ui.label("R:");
            ui.add_sized([100.0, 20.0], egui::Slider::new(&mut self.params.filter_release, 0.001..=2.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });
    }

    fn draw_vintage_lfo_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);

        // Waveform selector (vintage analog style)
        let mut lfo_waveform = Synthesizer::u8_to_lfo_waveform_pub(self.params.lfo_waveform);
        ui.horizontal(|ui| {
            ui.label("waveform:");
            egui::ComboBox::from_id_source("lfo_waveform")
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
                    ui.selectable_value(&mut lfo_waveform, LfoWaveform::ReverseSawtooth, "Reverse Saw");
                    ui.selectable_value(&mut lfo_waveform, LfoWaveform::SampleAndHold, "Sample & Hold");
                });
        });
        self.params.lfo_waveform = Synthesizer::lfo_waveform_to_u8_pub(lfo_waveform);

        ui.horizontal(|ui| {
            ui.label("rate:");
            ui.add_sized([90.0, 20.0], egui::Slider::new(&mut self.params.lfo_rate, 0.05..=30.0)
                .logarithmic(true)
                .step_by(0.05)
                .suffix(" Hz"));
        });

        ui.horizontal(|ui| {
            ui.label("amount:");
            ui.add_sized([90.0, 20.0], egui::Slider::new(&mut self.params.lfo_amount, 0.0..=1.0)
                .step_by(0.01));
        });

        // Keyboard sync option (authentic vintage analog feature)
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.params.lfo_sync, "keyboard sync");
            if self.params.lfo_sync {
                ui.label("(resets on note)");
            }
        });

        ui.separator();
        ui.label(egui::RichText::new("modulation destinations").strong());

        // Modulation routing (vintage analog style)
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("filter cutoff:");
                ui.add_sized([70.0, 18.0], egui::Slider::new(&mut self.params.lfo_to_cutoff, 0.0..=1.0)
                    .step_by(0.01));
            });
            ui.horizontal(|ui| {
                ui.label("filter res:");
                ui.add_sized([70.0, 18.0], egui::Slider::new(&mut self.params.lfo_to_resonance, 0.0..=1.0)
                    .step_by(0.01));
            });
            ui.horizontal(|ui| {
                ui.label("osc A pitch:");
                ui.add_sized([70.0, 18.0], egui::Slider::new(&mut self.params.lfo_to_osc1_pitch, 0.0..=1.0)
                    .step_by(0.01));
            });
            ui.horizontal(|ui| {
                ui.label("osc B pitch:");
                ui.add_sized([70.0, 18.0], egui::Slider::new(&mut self.params.lfo_to_osc2_pitch, 0.0..=1.0)
                    .step_by(0.01));
            });
            ui.horizontal(|ui| {
                ui.label("amplitude:");
                ui.add_sized([70.0, 18.0], egui::Slider::new(&mut self.params.lfo_to_amplitude, 0.0..=1.0)
                    .step_by(0.01));
            });
        });
    }


    fn draw_amp_envelope_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        ui.horizontal(|ui| {
            ui.label("A:");
            ui.add(egui::Slider::new(&mut self.params.amp_attack, 0.001..=2.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });

        ui.horizontal(|ui| {
            ui.label("D:");
            ui.add(egui::Slider::new(&mut self.params.amp_decay, 0.001..=3.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });

        ui.horizontal(|ui| {
            ui.label("S:");
            ui.add(egui::Slider::new(&mut self.params.amp_sustain, 0.0..=1.0)
                .step_by(0.01));
        });

        ui.horizontal(|ui| {
            ui.label("R:");
            ui.add(egui::Slider::new(&mut self.params.amp_release, 0.001..=5.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });
    }

    fn draw_filter_envelope_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        ui.horizontal(|ui| {
            ui.label("A:");
            ui.add(egui::Slider::new(&mut self.params.filter_attack, 0.001..=2.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });

        ui.horizontal(|ui| {
            ui.label("D:");
            ui.add(egui::Slider::new(&mut self.params.filter_decay, 0.001..=3.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });

        ui.horizontal(|ui| {
            ui.label("S:");
            ui.add(egui::Slider::new(&mut self.params.filter_sustain, 0.0..=1.0)
                .step_by(0.01));
        });

        ui.horizontal(|ui| {
            ui.label("R:");
            ui.add(egui::Slider::new(&mut self.params.filter_release, 0.001..=5.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });
    }

    fn draw_master_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        ui.horizontal(|ui| {
            ui.label("volume:");
            ui.add(egui::Slider::new(&mut self.params.master_volume, 0.0..=1.0).step_by(0.01));
        });

        ui.label("velocity sensitivity");

        ui.horizontal(|ui| {
            ui.label("-> volume:");
            ui.add(egui::Slider::new(&mut self.params.velocity_to_amplitude, 0.0..=1.0)
                .step_by(0.01));
        });
    }


    fn draw_effects_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        ui.label("reverb");
        ui.horizontal(|ui| {
            ui.label("amount:");
            ui.add(egui::Slider::new(&mut self.params.reverb_amount, 0.0..=1.0)
                .step_by(0.01));
        });
        ui.horizontal(|ui| {
            ui.label("size:");
            ui.add(egui::Slider::new(&mut self.params.reverb_size, 0.0..=1.0)
                .step_by(0.01));
        });

        ui.label("delay");
        ui.horizontal(|ui| {
            ui.label("time:");
            ui.add(egui::Slider::new(&mut self.params.delay_time, 0.01..=2.0)
                .step_by(0.01)
                .suffix(" s"));
        });
        ui.horizontal(|ui| {
            ui.label("feedback:");
            ui.add(egui::Slider::new(&mut self.params.delay_feedback, 0.0..=0.95)
                .step_by(0.01));
        });
        ui.horizontal(|ui| {
            ui.label("amount:");
            ui.add(egui::Slider::new(&mut self.params.delay_amount, 0.0..=1.0)
                .step_by(0.01));
        });
    }

    fn draw_arpeggiator_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        ui.checkbox(&mut self.params.arp_enabled, "enable");

        ui.horizontal(|ui| {
            ui.label("rate:");
            ui.add(egui::Slider::new(&mut self.params.arp_rate, 60.0..=240.0)
                .step_by(1.0)
                .suffix(" BPM"));
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
            ui.add(egui::Slider::new(&mut octaves_f32, 1.0..=4.0)
                .step_by(1.0));
            self.params.arp_octaves = octaves_f32 as u8;
        });

        ui.horizontal(|ui| {
            ui.label("gate:");
            ui.add(egui::Slider::new(&mut self.params.arp_gate_length, 0.1..=1.0)
                .step_by(0.01));
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
                    .desired_width(80.0)
            );

            let save_enabled = !self.new_preset_name.is_empty();
            if ui.add_enabled(save_enabled, egui::Button::new("Save")).clicked() {
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

    fn draw_keyboard_legend(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("OCTAVE: {}", self.current_octave))
                    .size(14.0)
                    .strong()
                    .color(egui::Color32::from_rgb(255, 255, 100)));
                ui.label(egui::RichText::new("(UP/DOWN arrows to change)")
                    .size(10.0)
                    .color(egui::Color32::GRAY));
            });

            ui.add_space(8.0);

            // Keyboard mapping legend
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("WHITE KEYS:")
                        .size(11.0)
                        .strong()
                        .color(egui::Color32::WHITE));
                    ui.horizontal(|ui| {
                        ui.label("A=C");
                        ui.label("S=D");
                        ui.label("D=E");
                        ui.label("F=F");
                        ui.label("G=G");
                        ui.label("H=A");
                        ui.label("J=B");
                        ui.label("K=C+");
                        ui.label("L=D+");
                        ui.label("N=E+");
                    });
                });

                ui.separator();

                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("BLACK KEYS:")
                        .size(11.0)
                        .strong()
                        .color(egui::Color32::LIGHT_GRAY));
                    ui.horizontal(|ui| {
                        ui.label("W=C#");
                        ui.label("E=D#");
                        ui.label("T=F#");
                        ui.label("Y=G#");
                        ui.label("U=A#");
                        ui.label("O=C#+");
                        ui.label("P=D#+");
                    });
                });
            });

            ui.add_space(6.0);

            // Controls legend
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("CONTROLS:")
                        .size(11.0)
                        .strong()
                        .color(egui::Color32::from_rgb(100, 255, 100)));
                    ui.label("Up/Down = Change octave");
                    ui.label("Hold key = Sustain note");
                    ui.label("Release key = Note off");
                });

                ui.separator();

                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("RANGE:")
                        .size(11.0)
                        .strong()
                        .color(egui::Color32::from_rgb(255, 200, 100)));
                    ui.label("Current: 1.5 octaves");
                    ui.label("Total: C0 to B8");
                    ui.label("Default: Octave 4");
                });
            });
        });
    }
}

impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Read current params at start of frame
        self.params = *self.lock_free_synth.get_params();

        // Handle keyboard input
        ctx.input(|i| {
            // Handle octave changes
            if i.key_pressed(egui::Key::ArrowUp) {
                self.current_octave = (self.current_octave + 1).clamp(0, 8);
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                self.current_octave = (self.current_octave - 1).clamp(0, 8);
            }

            // Map keyboard keys to note offsets (1.5 octaves: A to semicolon)
            let key_map = [
                (egui::Key::A, 0),   // C
                (egui::Key::W, 1),   // C#
                (egui::Key::S, 2),   // D
                (egui::Key::E, 3),   // D#
                (egui::Key::D, 4),   // E
                (egui::Key::F, 5),   // F
                (egui::Key::T, 6),   // F#
                (egui::Key::G, 7),   // G
                (egui::Key::Y, 8),   // G#
                (egui::Key::H, 9),   // A
                (egui::Key::U, 10),  // A#
                (egui::Key::J, 11),  // B
                (egui::Key::K, 12),  // C (next octave)
                (egui::Key::O, 13),  // C# (next octave)
                (egui::Key::L, 14),  // D (next octave)
                (egui::Key::P, 15),  // D# (next octave)
                (egui::Key::Semicolon, 16), // E (next octave)
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

        egui::CentralPanel::default().show(ctx, |ui| {
            // Compact Vintage Analog Style Header
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("PROPHET-5 SYNTHESIZER")
                    .size(18.0)
                    .strong());

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

            egui::ScrollArea::vertical().show(ui, |ui| {

                // CLEAN 4-COLUMN LAYOUT - All vertical organization
                ui.columns(4, |columns| {
                    // COLUMN 1 - Sound Generation A
                    columns[0].group(|ui| {
                        ui.set_min_width(185.0);
                        ui.label(egui::RichText::new("OSCILLATOR A")
                            .size(11.0)
                            .strong());
                        self.draw_vintage_oscillator_panel(ui, 1);
                    });

                    columns[0].add_space(2.0);

                    columns[0].group(|ui| {
                        ui.set_min_width(185.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("FILTER")
                                .size(11.0)
                                .strong());
                            ui.label(egui::RichText::new("24dB")
                                .size(9.0));
                        });
                        self.draw_prophet_filter_panel(ui);
                    });

                    // COLUMN 2 - Sound Generation B
                    columns[1].group(|ui| {
                        ui.set_min_width(185.0);
                        ui.label(egui::RichText::new("OSCILLATOR B")
                            .size(11.0)
                            .strong());
                        self.draw_vintage_oscillator_panel(ui, 2);
                    });

                    columns[1].add_space(2.0);

                    columns[1].group(|ui| {
                        ui.set_min_width(185.0);
                        ui.label(egui::RichText::new("LFO")
                            .size(11.0)
                            .strong());
                        self.draw_vintage_lfo_panel(ui);
                    });

                    // COLUMN 3 - Envelopes & Mix
                    columns[2].group(|ui| {
                        ui.set_min_width(150.0);
                        ui.label(egui::RichText::new("AMP ENV")
                            .size(11.0)
                            .strong());
                        self.draw_amp_envelope_panel(ui);
                    });

                    columns[2].add_space(2.0);

                    columns[2].group(|ui| {
                        ui.set_min_width(150.0);
                        ui.label(egui::RichText::new("FILTER ENV")
                            .size(11.0)
                            .strong());
                        self.draw_filter_envelope_panel(ui);
                    });

                    columns[2].add_space(2.0);

                    columns[2].group(|ui| {
                        ui.set_min_width(150.0);
                        ui.label(egui::RichText::new("MIXER")
                            .size(11.0)
                            .strong());
                        self.draw_mixer_panel(ui);
                    });

                    // COLUMN 4 - Performance & Utilities
                    columns[3].group(|ui| {
                        ui.set_min_width(125.0);
                        ui.label(egui::RichText::new("CURRENT PRESET")
                            .size(11.0)
                            .strong());
                        ui.label(if self.current_preset_name.is_empty() {
                            "Default"
                        } else {
                            &self.current_preset_name
                        });
                    });

                    columns[3].add_space(2.0);

                    columns[3].group(|ui| {
                        ui.set_min_width(125.0);
                        ui.label(egui::RichText::new("MASTER")
                            .size(11.0)
                            .strong());
                        self.draw_master_panel(ui);
                    });

                    columns[3].add_space(2.0);

                    columns[3].group(|ui| {
                        ui.set_min_width(125.0);
                        ui.label(egui::RichText::new("EFFECTS")
                            .size(11.0)
                            .strong());
                        self.draw_effects_panel(ui);
                    });

                    columns[3].add_space(2.0);

                    columns[3].group(|ui| {
                        ui.set_min_width(125.0);
                        ui.label(egui::RichText::new("ARP")
                            .size(11.0)
                            .strong());
                        self.draw_arpeggiator_panel(ui);
                    });

                    columns[3].add_space(2.0);

                    // Empty space where preset info was
                    columns[3].add_space(2.0);
                });

                ui.add_space(8.0);

                // KEYBOARD LEGEND SECTION - Compact
                ui.group(|ui| {
                    ui.label(egui::RichText::new("KEYBOARD")
                        .size(12.0)
                        .color(egui::Color32::from_rgb(200, 200, 200))
                        .strong());
                    self.draw_keyboard_legend(ui);
                });
            });
        });

        // MIDI Monitor Window
        if self.show_midi_monitor {
            egui::Window::new("MIDI Monitor")
                .default_size([400.0, 300.0])
                .show(ctx, |ui| {
                    self.draw_midi_monitor(ui);
                });
        }

        // Presets Window
        if self.show_presets_window {
            let mut show_presets_window = self.show_presets_window;
            egui::Window::new("Preset Manager")
                .default_size([350.0, 400.0])
                .open(&mut show_presets_window)
                .show(ctx, |ui| {
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
            if ui.button("clear").clicked() {
                if let Some(ref midi_handler) = self._midi_handler {
                    if let Ok(mut history) = midi_handler.message_history.lock() {
                        history.clear();
                    }
                }
            }
        });

        ui.separator();

        if let Some(ref midi_handler) = self._midi_handler {
            if let Ok(history) = midi_handler.message_history.lock() {
                egui::ScrollArea::vertical()
                    .max_height(250.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for msg in history.iter().rev().take(20) { // Show last 20 messages
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
