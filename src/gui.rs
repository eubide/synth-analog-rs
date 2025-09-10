use eframe::egui;
use std::sync::{Arc, Mutex};
use crate::synthesizer::{Synthesizer, WaveType};
use crate::audio_engine::AudioEngine;
use crate::midi_handler::MidiHandler;

pub struct SynthApp {
    synthesizer: Arc<Mutex<Synthesizer>>,
    _audio_engine: AudioEngine,
    _midi_handler: Option<MidiHandler>,
    last_key_times: std::collections::HashMap<egui::Key, std::time::Instant>,
    current_octave: i32,
    show_midi_monitor: bool,
}

impl SynthApp {
    pub fn new(synthesizer: Arc<Mutex<Synthesizer>>, audio_engine: AudioEngine, midi_handler: Option<MidiHandler>) -> Self {
        Self {
            synthesizer,
            _audio_engine: audio_engine,
            _midi_handler: midi_handler,
            last_key_times: std::collections::HashMap::new(),
            current_octave: 4, // C4 octave by default
            show_midi_monitor: false,
        }
    }

    fn draw_prophet_oscillator_panel(&mut self, ui: &mut egui::Ui, osc_num: u8) {
        let mut synth = self.synthesizer.lock().unwrap();
        let osc = if osc_num == 1 { &mut synth.osc1 } else { &mut synth.osc2 };
        
        // Frequency controls
        ui.horizontal(|ui| {
            ui.label("FREQ:");
            ui.add(egui::Slider::new(&mut osc.detune, -50.0..=50.0)
                .step_by(0.1)
                .text("Fine"));
        });
        
        // Wave type selector
        ui.horizontal(|ui| {
            ui.label("WAVE:");
            egui::ComboBox::from_id_source(format!("wave_{}", osc_num))
                .selected_text(format!("{:?}", osc.wave_type))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut osc.wave_type, WaveType::Sawtooth, "Sawtooth");
                    ui.selectable_value(&mut osc.wave_type, WaveType::Triangle, "Triangle");
                    ui.selectable_value(&mut osc.wave_type, WaveType::Square, "Square/Pulse");
                    ui.selectable_value(&mut osc.wave_type, WaveType::Sine, "Sine");
                });
        });
        
        // Pulse Width (only for square waves)
        if matches!(osc.wave_type, WaveType::Square) {
            ui.horizontal(|ui| {
                ui.label("PW:");
                ui.add(egui::Slider::new(&mut osc.pulse_width, 0.1..=0.9)
                    .step_by(0.01)
                    .text("Width"));
            });
        }
        
        // Level control (always available)
        ui.horizontal(|ui| {
            ui.label("LEVEL:");
            ui.add(egui::Slider::new(&mut osc.amplitude, 0.0..=1.0)
                .step_by(0.01));
        });
        
        // Sync control (only for oscillator B)
        if osc_num == 2 {
            ui.horizontal(|ui| {
                ui.label("SYNC:");
                ui.checkbox(&mut synth.osc2_sync, "Osc A");
            });
        }
    }

    fn draw_mixer_panel(&mut self, ui: &mut egui::Ui) {
        let mut synth = self.synthesizer.lock().unwrap();
        
        ui.horizontal(|ui| {
            ui.label("OSC A:");
            ui.add(egui::Slider::new(&mut synth.mixer.osc1_level, 0.0..=1.0)
                .step_by(0.01)
                .text("Level"));
        });
        
        ui.horizontal(|ui| {
            ui.label("OSC B:");
            ui.add(egui::Slider::new(&mut synth.mixer.osc2_level, 0.0..=1.0)
                .step_by(0.01)
                .text("Level"));
        });
        
        ui.horizontal(|ui| {
            ui.label("NOISE:");
            ui.add(egui::Slider::new(&mut synth.mixer.noise_level, 0.0..=1.0)
                .step_by(0.01)
                .text("Level"));
        });
    }

    fn draw_prophet_filter_panel(&mut self, ui: &mut egui::Ui) {
        let mut synth = self.synthesizer.lock().unwrap();
        
        // Cutoff frequency
        ui.horizontal(|ui| {
            ui.label("CUTOFF:");
            ui.add(egui::Slider::new(&mut synth.filter.cutoff, 20.0..=20000.0)
                .logarithmic(true)
                .step_by(1.0)
                .suffix(" Hz"));
        });
        
        // Resonance
        ui.horizontal(|ui| {
            ui.label("RES:");
            ui.add(egui::Slider::new(&mut synth.filter.resonance, 0.1..=10.0)
                .step_by(0.1));
        });
        
        // Envelope Amount
        ui.horizontal(|ui| {
            ui.label("ENV AMT:");
            ui.add(egui::Slider::new(&mut synth.filter.envelope_amount, -1.0..=1.0)
                .step_by(0.01));
        });
        
        // Keyboard tracking
        ui.horizontal(|ui| {
            ui.label("KBD:");
            ui.add(egui::Slider::new(&mut synth.filter.keyboard_tracking, 0.0..=1.0)
                .step_by(0.01)
                .text("Track"));
        });
    }

    fn draw_prophet_lfo_panel(&mut self, ui: &mut egui::Ui) {
        let mut synth = self.synthesizer.lock().unwrap();
        
        ui.horizontal(|ui| {
            ui.label("RATE:");
            ui.add(egui::Slider::new(&mut synth.lfo.frequency, 0.1..=20.0)
                .step_by(0.1)
                .suffix(" Hz"));
        });
        
        ui.horizontal(|ui| {
            ui.label("AMOUNT:");
            ui.add(egui::Slider::new(&mut synth.lfo.amplitude, 0.0..=1.0)
                .step_by(0.01));
        });
        
        ui.label("DESTINATIONS:");
        ui.checkbox(&mut synth.lfo.target_osc1_pitch, "OSC A");
        ui.checkbox(&mut synth.lfo.target_osc2_pitch, "OSC B");
        ui.checkbox(&mut synth.lfo.target_filter, "FILTER");
        ui.checkbox(&mut synth.lfo.target_amplitude, "AMP");
    }

    fn draw_filter_envelope_panel(&mut self, ui: &mut egui::Ui) {
        let mut synth = self.synthesizer.lock().unwrap();
        
        ui.horizontal(|ui| {
            ui.label("A:");
            ui.add(egui::Slider::new(&mut synth.filter_envelope.attack, 0.001..=5.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });
        
        ui.horizontal(|ui| {
            ui.label("D:");
            ui.add(egui::Slider::new(&mut synth.filter_envelope.decay, 0.001..=5.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });
        
        ui.horizontal(|ui| {
            ui.label("S:");
            ui.add(egui::Slider::new(&mut synth.filter_envelope.sustain, 0.0..=1.0)
                .step_by(0.01));
        });
        
        ui.horizontal(|ui| {
            ui.label("R:");
            ui.add(egui::Slider::new(&mut synth.filter_envelope.release, 0.001..=5.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });
    }

    fn draw_amp_envelope_panel(&mut self, ui: &mut egui::Ui) {
        let mut synth = self.synthesizer.lock().unwrap();
        
        ui.horizontal(|ui| {
            ui.label("A:");
            ui.add(egui::Slider::new(&mut synth.amp_envelope.attack, 0.001..=5.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });
        
        ui.horizontal(|ui| {
            ui.label("D:");
            ui.add(egui::Slider::new(&mut synth.amp_envelope.decay, 0.001..=5.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });
        
        ui.horizontal(|ui| {
            ui.label("S:");
            ui.add(egui::Slider::new(&mut synth.amp_envelope.sustain, 0.0..=1.0)
                .step_by(0.01));
        });
        
        ui.horizontal(|ui| {
            ui.label("R:");
            ui.add(egui::Slider::new(&mut synth.amp_envelope.release, 0.001..=5.0)
                .logarithmic(true)
                .step_by(0.001)
                .suffix(" s"));
        });
    }


    fn draw_master_panel(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label("Master");
            
            let mut synth = self.synthesizer.lock().unwrap();
            
            ui.horizontal(|ui| {
                ui.label("Volume:");
                ui.add(egui::Slider::new(&mut synth.master_volume, 0.0..=1.0).step_by(0.01));
            });
        });
    }

    fn draw_piano_keyboard(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Octave: {}", self.current_octave));
                ui.label("Use ↑↓ arrows to change octave");
            });
            ui.label("Keys: A W S E D F T G Y H U J K O L P Ñ (1.5 octaves)");
            
            let key_width = 25.0;
            let key_height = 80.0;
            let white_keys = ["A", "S", "D", "F", "G", "H", "J", "K", "L", "Ñ"];
            let black_keys = ["W", "E", "T", "Y", "U", "O", "P"];
            let black_positions = [0.7, 1.7, 3.7, 4.7, 5.7, 7.7, 8.7]; // Relative to white keys
            
            ui.horizontal(|ui| {
                let start_x = ui.cursor().min.x;
                
                // Draw white keys first
                for (i, &key_label) in white_keys.iter().enumerate() {
                    let note_offset = match i {
                        0 => 0,  // A -> C
                        1 => 2,  // S -> D  
                        2 => 4,  // D -> E
                        3 => 5,  // F -> F
                        4 => 7,  // G -> G
                        5 => 9,  // H -> A
                        6 => 11, // J -> B
                        7 => 12, // K -> C (next octave)
                        8 => 14, // L -> D (next octave)
                        9 => 16, // Ñ -> E (next octave)
                        _ => 0,
                    };
                    let midi_note = self.current_octave * 12 + note_offset;
                    
                    let (rect, response) = ui.allocate_exact_size(
                        egui::Vec2::new(key_width, key_height),
                        egui::Sense::click()
                    );
                    
                    let color = if response.is_pointer_button_down_on() {
                        egui::Color32::LIGHT_GRAY
                    } else {
                        egui::Color32::WHITE
                    };
                    
                    ui.painter().rect_filled(rect, egui::Rounding::ZERO, color);
                    ui.painter().rect_stroke(rect, egui::Rounding::ZERO, egui::Stroke::new(1.0, egui::Color32::BLACK));
                    
                    // Draw key label
                    ui.painter().text(
                        rect.center() + egui::Vec2::new(0.0, key_height * 0.3),
                        egui::Align2::CENTER_CENTER,
                        key_label,
                        egui::FontId::default(),
                        egui::Color32::BLACK
                    );
                    
                    if response.clicked() {
                        let mut synth = self.synthesizer.lock().unwrap();
                        synth.note_on(midi_note as u8);
                    }
                    
                    if response.drag_stopped() {
                        let mut synth = self.synthesizer.lock().unwrap();
                        synth.note_off(midi_note as u8);
                    }
                }
                
                // Draw black keys on top
                for (i, &key_label) in black_keys.iter().enumerate() {
                    let note_offset = match i {
                        0 => 1,  // W -> C#
                        1 => 3,  // E -> D#
                        2 => 6,  // T -> F#
                        3 => 8,  // Y -> G#
                        4 => 10, // U -> A#
                        5 => 13, // O -> C# (next octave)
                        6 => 15, // P -> D# (next octave)
                        _ => 0,
                    };
                    let midi_note = self.current_octave * 12 + note_offset;
                    let x_pos = start_x + black_positions[i] * key_width - key_width * 0.3;
                    
                    let black_rect = egui::Rect::from_min_size(
                        egui::Pos2::new(x_pos, ui.cursor().min.y),
                        egui::Vec2::new(key_width * 0.6, key_height * 0.6)
                    );
                    
                    let response = ui.allocate_rect(black_rect, egui::Sense::click());
                    
                    let color = if response.is_pointer_button_down_on() {
                        egui::Color32::DARK_GRAY
                    } else {
                        egui::Color32::BLACK
                    };
                    
                    ui.painter().rect_filled(black_rect, egui::Rounding::ZERO, color);
                    ui.painter().rect_stroke(black_rect, egui::Rounding::ZERO, egui::Stroke::new(1.0, egui::Color32::GRAY));
                    
                    // Draw key label
                    ui.painter().text(
                        black_rect.center() + egui::Vec2::new(0.0, key_height * 0.2),
                        egui::Align2::CENTER_CENTER,
                        key_label,
                        egui::FontId::default(),
                        egui::Color32::WHITE
                    );
                    
                    if response.clicked() {
                        let mut synth = self.synthesizer.lock().unwrap();
                        synth.note_on(midi_note as u8);
                    }
                    
                    if response.drag_stopped() {
                        let mut synth = self.synthesizer.lock().unwrap();
                        synth.note_off(midi_note as u8);
                    }
                }
            });
        });
    }
}

impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle keyboard input
        ctx.input(|i| {
            // Handle octave changes
            if i.key_pressed(egui::Key::ArrowUp) {
                self.current_octave = (self.current_octave + 1).clamp(0, 8);
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                self.current_octave = (self.current_octave - 1).clamp(0, 8);
            }
            
            // Map keyboard keys to note offsets (1.5 octaves: A to Ñ)
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
                (egui::Key::Semicolon, 16), // E (next octave) - using semicolon for Ñ
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
                        let mut synth = self.synthesizer.lock().unwrap();
                        synth.note_on(midi_note as u8);
                    }
                }
                
                if i.key_released(key) {
                    self.last_key_times.remove(&key);
                    let mut synth = self.synthesizer.lock().unwrap();
                    synth.note_off(midi_note as u8);
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Prophet-5 Style Synthesizer");
                ui.add_space(20.0);
                if self._midi_handler.is_some() {
                    ui.colored_label(egui::Color32::GREEN, "🎹 MIDI Connected");
                    if ui.button("MIDI Monitor").clicked() {
                        self.show_midi_monitor = !self.show_midi_monitor;
                    }
                } else {
                    ui.colored_label(egui::Color32::RED, "🎹 No MIDI");
                }
            });
            
            // Top row: Oscillators and Mixer
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.label("OSCILLATOR A");
                        self.draw_prophet_oscillator_panel(ui, 1);
                    });
                });
                
                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.label("OSCILLATOR B");
                        self.draw_prophet_oscillator_panel(ui, 2);
                    });
                });
                
                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.label("MIXER");
                        self.draw_mixer_panel(ui);
                    });
                });
            });
            
            ui.add_space(15.0);
            
            // Middle row: Filter and LFO
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.label("FILTER");
                        self.draw_prophet_filter_panel(ui);
                        ui.add_space(10.0);
                        ui.label("FILTER ENVELOPE");
                        self.draw_filter_envelope_panel(ui);
                    });
                });
                
                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.label("LFO");
                        self.draw_prophet_lfo_panel(ui);
                    });
                });
            });
            
            ui.add_space(15.0);
            
            // Bottom row: Amp Envelope and Master
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.label("AMPLIFIER ENVELOPE");
                        self.draw_amp_envelope_panel(ui);
                    });
                });
                
                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.label("MASTER");
                        self.draw_master_panel(ui);
                    });
                });
            });
            
            ui.add_space(20.0);
            
            self.draw_piano_keyboard(ui);
        });

        // MIDI Monitor Window
        if self.show_midi_monitor {
            egui::Window::new("🎹 MIDI Monitor")
                .default_size([400.0, 300.0])
                .show(ctx, |ui| {
                    self.draw_midi_monitor(ui);
                });
        }
    }
}

impl SynthApp {
    fn draw_midi_monitor(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Recent MIDI Messages:");
            if ui.button("Clear").clicked() {
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
                            ui.label("No MIDI messages received yet...");
                            ui.label("Connect a MIDI device and play some notes!");
                        }
                    });
            }
        } else {
            ui.label("No MIDI handler available");
        }
    }
}

