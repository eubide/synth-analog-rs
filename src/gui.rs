use crate::audio_engine::AudioEngine;
use crate::lock_free::{LockFreeSynth, MidiEvent, MidiEventQueue, SynthParameters};
use crate::midi_handler::MidiHandler;
use crate::synthesizer::{ArpPattern, LfoWaveform, Synthesizer, WaveType};
use eframe::egui;
use std::sync::{Arc, Mutex};

pub struct SynthApp {
    lock_free_synth: Arc<LockFreeSynth>,
    midi_events: Arc<MidiEventQueue>,
    _audio_engine: AudioEngine,
    _midi_handler: Option<MidiHandler>,
    /// Para cada tecla QWERTY pulsada, guarda (nota MIDI enviada, timestamp).
    /// Almacenar la nota real previene stuck notes cuando la octava cambia
    /// entre el press y el release (sin esto, el NoteOff se calcularía con
    /// la octava actual y no liberaría la voz original).
    last_key_times: std::collections::HashMap<egui::Key, (u8, std::time::Instant)>,
    current_octave: i32,
    show_midi_monitor: bool,
    show_midi_learn: bool,
    show_presets_window: bool,
    learn_state: Option<Arc<Mutex<crate::midi_handler::MidiLearnState>>>,
    current_preset_name: String,
    new_preset_name: String,
    preset_category: String,
    preset_category_filter: String,
    preset_search: String,
    show_preset_editor: bool,
    params: SynthParameters,
    params_a: Option<SynthParameters>,
    params_b: Option<SynthParameters>,
    peak_level: f32,
}

/// Ancho fijo de las etiquetas (incluyen unidad entre paréntesis). El layout
/// reserva exactamente `LABEL_WIDTH + WIDGET_WIDTH` por fila — etiqueta a la
/// izquierda con unidad, slider+valor a la derecha en un slot de ancho fijo.
/// Mover la unidad a la etiqueta libera al slider de tener que reservar ancho
/// para sufijos largos como " 5000 Hz" o " 240 BPM" que rompían el layout.
const LABEL_WIDTH: f32 = 95.0;
const WIDGET_WIDTH: f32 = 105.0;

/// Renderiza un grupo con título uniforme — elimina el boilerplate
/// `ui.group { label(title); content }` repetido en cada sección.
fn section(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.group(|ui| {
        ui.label(egui::RichText::new(title).size(11.0).strong());
        add_contents(ui);
    });
}

/// Fila etiquetada: etiqueta de ancho fijo right-aligned + widget en slot de
/// ancho fijo. Sustituye al patrón
/// `ui.horizontal { ui.label("x:"); ui.add(slider); }` para alinear sliders y
/// valores numéricos. Cada fila ocupa exactamente `LABEL_WIDTH + WIDGET_WIDTH`.
fn labeled<R>(
    ui: &mut egui::Ui,
    label: &str,
    add_widget: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        let h = ui.spacing().interact_size.y;
        ui.allocate_ui_with_layout(
            egui::vec2(LABEL_WIDTH, h),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.label(label);
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(WIDGET_WIDTH, h),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| add_widget(ui),
        )
        .inner
    })
    .inner
}

/// Variante con checkbox prefijo (usada en LFO MOD). El slot del checkbox se
/// reserva siempre — si `target` es `None`, se deja en blanco para que las
/// filas con y sin checkbox queden alineadas. Total ancho fila igual que
/// `labeled` (`LABEL_WIDTH + WIDGET_WIDTH`).
fn labeled_check<R>(
    ui: &mut egui::Ui,
    target: Option<&mut bool>,
    label: &str,
    add_widget: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        let h = ui.spacing().interact_size.y;
        let check_w = 16.0;
        if let Some(t) = target {
            ui.allocate_ui_with_layout(
                egui::vec2(check_w, h),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.checkbox(t, "");
                },
            );
        } else {
            ui.allocate_exact_size(egui::vec2(check_w, h), egui::Sense::hover());
        }
        ui.allocate_ui_with_layout(
            egui::vec2(LABEL_WIDTH - check_w, h),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.label(label);
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(WIDGET_WIDTH, h),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| add_widget(ui),
        )
        .inner
    })
    .inner
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

    labeled(ui, "attack (s):", |ui| {
        ui.add(
            egui::Slider::new(attack, 0.001..=2.0)
                .logarithmic(true)
                .step_by(0.001),
        )
    })
    .on_hover_text("Attack — time from note-on to peak level");
    labeled(ui, "decay (s):", |ui| {
        ui.add(
            egui::Slider::new(decay, 0.001..=3.0)
                .logarithmic(true)
                .step_by(0.001),
        )
    })
    .on_hover_text("Decay — time to fall from peak down to the sustain level");
    labeled(ui, "sustain:", |ui| {
        ui.add(egui::Slider::new(sustain, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Sustain — level held while the note is pressed");
    labeled(ui, "release (s):", |ui| {
        ui.add(
            egui::Slider::new(release, 0.001..=5.0)
                .logarithmic(true)
                .step_by(0.001),
        )
    })
    .on_hover_text("Release — time to fade to silence after note-off");
}

impl SynthApp {
    pub fn new(
        lock_free_synth: Arc<LockFreeSynth>,
        midi_events: Arc<MidiEventQueue>,
        audio_engine: AudioEngine,
        midi_handler: Option<MidiHandler>,
    ) -> Self {
        let params = *lock_free_synth.get_params();
        let learn_state = midi_handler.as_ref().map(|h| h.learn_state.clone());
        Self {
            lock_free_synth,
            midi_events,
            _audio_engine: audio_engine,
            _midi_handler: midi_handler,
            last_key_times: std::collections::HashMap::new(),
            current_octave: 3, // C3 octave by default
            show_midi_monitor: false,
            show_midi_learn: false,
            show_presets_window: false,
            learn_state,
            current_preset_name: String::new(),
            new_preset_name: String::new(),
            preset_category: "Other".to_string(),
            preset_category_filter: "All".to_string(),
            preset_search: String::new(),
            show_preset_editor: false,
            params,
            params_a: None,
            params_b: None,
            peak_level: 0.0,
        }
    }

    fn draw_vintage_oscillator_panel(&mut self, ui: &mut egui::Ui, osc_num: u8) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

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

        labeled(ui, "tune (st):", |ui| {
            ui.add(egui::Slider::new(detune, -24.0..=24.0).step_by(0.1))
        })
        .on_hover_text("Pitch detune in semitones (-24 to +24)");

        let mut wave_type = Synthesizer::u8_to_wave_type_pub(*waveform);
        labeled(ui, "wave:", |ui| {
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
                })
                .response
        })
        .on_hover_text("Oscillator waveform shape");
        *waveform = Synthesizer::wave_type_to_u8_pub(wave_type);

        if wave_type == WaveType::Square {
            labeled(ui, "pw:", |ui| {
                ui.add(egui::Slider::new(pulse_width, 0.1..=0.9).step_by(0.01))
            })
            .on_hover_text("Pulse width — 0.5 = symmetric square, off-center = nasal/PWM");
        }

        if osc_num == 2 {
            labeled(ui, "sync:", |ui| ui.checkbox(&mut self.params.osc2_sync, "-> A"))
                .on_hover_text(
                    "Hard sync osc B to osc A — every osc A cycle resets osc B (classic lead sound)",
                );
        }
    }

    fn draw_mixer_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

        labeled(ui, "osc A:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.mixer_osc1_level, 0.0..=1.0).step_by(0.01))
        })
        .on_hover_text("Oscillator A level into the filter");
        labeled(ui, "osc B:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.mixer_osc2_level, 0.0..=1.0).step_by(0.01))
        })
        .on_hover_text("Oscillator B level into the filter");
        labeled(ui, "noise:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.noise_level, 0.0..=1.0).step_by(0.01))
        })
        .on_hover_text("White noise generator level (great for percussion or wind effects)");
    }

    fn draw_prophet_filter_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

        labeled(ui, "cutoff (Hz):", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.filter_cutoff, 20.0..=20000.0)
                    .logarithmic(true)
                    .step_by(1.0),
            )
        })
        .on_hover_text("Low-pass cutoff frequency — closes the filter to darken the sound");
        labeled(ui, "resonance:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.filter_resonance, 0.0..=4.0).step_by(0.05))
        })
        .on_hover_text("Filter resonance / Q — emphasises cutoff frequency. >=3.8 self-oscillates");
        if self.params.filter_resonance >= 3.8 {
            ui.colored_label(egui::Color32::from_rgb(255, 160, 60), "self-osc");
        }
        labeled(ui, "envelope:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.filter_envelope_amount, -1.0..=1.0)
                    .step_by(0.01),
            )
        })
        .on_hover_text("How much the FILTER ENV modulates cutoff (negative inverts the envelope)");
        labeled(ui, "keyboard:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.filter_keyboard_tracking, 0.0..=1.0)
                    .step_by(0.01),
            )
        })
        .on_hover_text("Keyboard tracking — higher notes open the filter more");
        labeled(ui, "velocity:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.velocity_to_cutoff, 0.0..=1.0).step_by(0.01))
        })
        .on_hover_text("How much MIDI velocity opens the filter (harder = brighter)");
    }

    /// LFO timing — waveform, rate, amount, sync, delay. Pareja con
    /// `draw_lfo_mod_panel` (los destinos de modulación viven en otro panel).
    fn draw_lfo_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        let mut lfo_waveform = Synthesizer::u8_to_lfo_waveform_pub(self.params.lfo_waveform);
        labeled(ui, "wave:", |ui| {
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
                })
                .response
        })
        .on_hover_text("LFO waveform — Triangle/Square/Saw for periodic modulation, S&H for random steps");
        self.params.lfo_waveform = Synthesizer::lfo_waveform_to_u8_pub(lfo_waveform);

        labeled(ui, "rate (Hz):", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.lfo_rate, 0.05..=30.0)
                    .logarithmic(true)
                    .step_by(0.05),
            )
        })
        .on_hover_text("LFO frequency (cycles per second)");
        labeled(ui, "amount:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.lfo_amount, 0.0..=1.0).step_by(0.01))
        })
        .on_hover_text("Global LFO depth — multiplies all routing amounts in LFO MOD");
        labeled(ui, "delay (s):", |ui| {
            ui.add(egui::Slider::new(&mut self.params.lfo_delay, 0.0..=5.0).step_by(0.01))
        })
        .on_hover_text("Time after note-on before the LFO fades in (delayed vibrato)");
        ui.checkbox(&mut self.params.lfo_sync, "key sync (reset on note)")
            .on_hover_text("Reset LFO phase on every note (vs free-running across notes)");
    }

    /// LFO mod destinations — 5 rutas con toggle de target y amount. Cada fila
    /// reserva slot de checkbox para que las rutas con y sin toggle queden
    /// alineadas (filter res hereda el target de filter cutoff).
    fn draw_lfo_mod_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        labeled_check(
            ui,
            Some(&mut self.params.lfo_target_filter),
            "cutoff:",
            |ui| {
                ui.add(egui::Slider::new(&mut self.params.lfo_to_cutoff, 0.0..=1.0).step_by(0.01))
            },
        )
        .on_hover_text("LFO modulation depth on filter cutoff (wah-wah). Toggle enables routing");
        labeled_check(ui, None, "res:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.lfo_to_resonance, 0.0..=1.0).step_by(0.01))
        })
        .on_hover_text("LFO depth on filter resonance (uses the filter target above)");
        labeled_check(
            ui,
            Some(&mut self.params.lfo_target_osc1_pitch),
            "osc A pitch:",
            |ui| {
                ui.add(
                    egui::Slider::new(&mut self.params.lfo_to_osc1_pitch, 0.0..=1.0).step_by(0.01),
                )
            },
        )
        .on_hover_text("LFO depth on osc A pitch (vibrato). Toggle enables routing");
        labeled_check(
            ui,
            Some(&mut self.params.lfo_target_osc2_pitch),
            "osc B pitch:",
            |ui| {
                ui.add(
                    egui::Slider::new(&mut self.params.lfo_to_osc2_pitch, 0.0..=1.0).step_by(0.01),
                )
            },
        )
        .on_hover_text("LFO depth on osc B pitch (vibrato). Toggle enables routing");
        labeled_check(
            ui,
            Some(&mut self.params.lfo_target_amplitude),
            "amplitude:",
            |ui| {
                ui.add(
                    egui::Slider::new(&mut self.params.lfo_to_amplitude, 0.0..=1.0).step_by(0.01),
                )
            },
        )
        .on_hover_text("LFO depth on amplitude (tremolo). Toggle enables routing");
    }


    fn draw_master_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        labeled(ui, "volume:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.master_volume, 0.0..=1.0).step_by(0.01))
        })
        .on_hover_text("Master output level");
        labeled(ui, "glide (s):", |ui| {
            ui.add(egui::Slider::new(&mut self.params.glide_time, 0.0..=2.0).step_by(0.01))
        })
        .on_hover_text("Portamento — pitch slide time between consecutive notes");
        let mut range_f32 = self.params.pitch_bend_range as f32;
        labeled(ui, "bend (st):", |ui| {
            ui.add(egui::Slider::new(&mut range_f32, 1.0..=24.0).step_by(1.0))
        })
        .on_hover_text("Pitch bend wheel range in semitones");
        self.params.pitch_bend_range = range_f32 as u8;

        ui.separator();
        ui.label(egui::RichText::new("velocity").size(10.0).strong());

        labeled(ui, "-> vol:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.velocity_to_amplitude, 0.0..=1.0).step_by(0.01),
            )
        })
        .on_hover_text("How much MIDI velocity affects note loudness");
        labeled(ui, "curve:", |ui| {
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
                })
                .response
        })
        .on_hover_text("Velocity response curve — Soft = expressive, Hard = aggressive, Linear = neutral");

        ui.separator();
        ui.label(egui::RichText::new("aftertouch").size(10.0).strong());

        labeled(ui, "-> cutoff:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.aftertouch_to_cutoff, 0.0..=1.0).step_by(0.01),
            )
        })
        .on_hover_text("Channel pressure modulates filter cutoff (press harder = brighter)");
        labeled(ui, "-> amp:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.aftertouch_to_amplitude, 0.0..=1.0)
                    .step_by(0.01),
            )
        })
        .on_hover_text("Channel pressure modulates loudness (press harder = louder)");

        ui.add_space(2.0);
        labeled(ui, "tuning:", |ui| {
            let modes = ["Equal Temp.", "Just Inton.", "Pythagorean", "Werckmeister"];
            egui::ComboBox::from_id_salt("tuning_mode")
                .selected_text(modes[self.params.tuning_mode as usize])
                .show_ui(ui, |ui| {
                    for (i, name) in modes.iter().enumerate() {
                        ui.selectable_value(&mut self.params.tuning_mode, i as u8, *name);
                    }
                })
                .response
        })
        .on_hover_text("Alternate tuning system — all anchored to A4 = 440 Hz");

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("oversamp:").size(10.0));
            ui.selectable_value(&mut self.params.oversampling, 1u8, "1×");
            ui.selectable_value(&mut self.params.oversampling, 2u8, "2×");
            ui.selectable_value(&mut self.params.oversampling, 4u8, "4×");
        });

        labeled(ui, "spread (st):", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.stereo_spread, 0.0..=1.0)
                    .fixed_decimals(2),
            )
        })
        .on_hover_text("Stereo spread: distributes voices across L/R field (0 = mono, 1 = full spread)");

        ui.add_space(4.0);
        let btn_text = if self.params.reference_tone { "A-440 [ON]" } else { "A-440" };
        let btn = egui::Button::new(btn_text);
        let btn = if self.params.reference_tone {
            btn.fill(egui::Color32::from_rgb(180, 80, 30))
        } else {
            btn
        };
        if ui.add(btn)
            .on_hover_text("Emite La4 puro a 440 Hz para afinar — bypasea toda la sintesis")
            .clicked()
        {
            self.params.reference_tone = !self.params.reference_tone;
        }
    }

    fn draw_effects_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        ui.label(egui::RichText::new("chorus").size(10.0).strong());
        labeled(ui, "mix:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.chorus_mix, 0.0..=1.0).step_by(0.01))
        })
        .on_hover_text("Chorus dry/wet mix (0 = bypass, 1 = full wet)");
        labeled(ui, "rate (Hz):", |ui| {
            ui.add(egui::Slider::new(&mut self.params.chorus_rate, 0.1..=3.0).step_by(0.01))
        })
        .on_hover_text("Chorus modulation rate (slow = lush, fast = warbly)");
        labeled(ui, "depth:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.chorus_depth, 0.0..=1.0).step_by(0.01))
        })
        .on_hover_text("Chorus modulation depth — how much the delay time wobbles");

        ui.label(egui::RichText::new("reverb").size(10.0).strong());
        labeled(ui, "amount:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.reverb_amount, 0.0..=1.0).step_by(0.01))
        })
        .on_hover_text("Reverb dry/wet mix");
        labeled(ui, "size:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.reverb_size, 0.0..=1.0).step_by(0.01))
        })
        .on_hover_text("Reverb room size — small = booth, large = cathedral");

        ui.label(egui::RichText::new("delay").size(10.0).strong());
        labeled(ui, "time (s):", |ui| {
            ui.add(egui::Slider::new(&mut self.params.delay_time, 0.01..=2.0).step_by(0.01))
        })
        .on_hover_text("Delay time between echoes");
        labeled(ui, "feedback:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.delay_feedback, 0.0..=0.95).step_by(0.01))
        })
        .on_hover_text("Echo feedback — higher = more repetitions (capped at 0.95 to avoid runaway)");
        labeled(ui, "amount:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.delay_amount, 0.0..=1.0).step_by(0.01))
        })
        .on_hover_text("Delay dry/wet mix");
    }

    /// Analog character panel — the subtle imperfections that break the
    /// mathematical cleanliness of a digital synth: component tolerances, slow
    /// filter temperature drift, VCA bleed, and circuit hiss.
    fn draw_analog_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        labeled(ui, "tolerance:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.analog_component_tolerance, 0.0..=1.0)
                    .step_by(0.01),
            )
            .on_hover_text("Per-voice filter tolerance (±2% cutoff / ±3% Q)")
        });
        labeled(ui, "drift:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.analog_filter_drift, 0.0..=1.0).step_by(0.01),
            )
            .on_hover_text("Slow filter-temperature drift")
        });
        labeled(ui, "vca bleed:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.analog_vca_bleed, 0.0..=0.01).step_by(0.0001),
            )
            .on_hover_text("Oscillator leakage through closed VCA")
        });
        labeled(ui, "hiss:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.analog_noise_floor, 0.0..=0.01).step_by(0.0001),
            )
            .on_hover_text("Constant background noise floor")
        });
    }

    fn draw_arpeggiator_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        ui.checkbox(&mut self.params.arp_enabled, "enable")
            .on_hover_text("Activate the arpeggiator — held notes play as a sequence");

        labeled(ui, "rate (BPM):", |ui| {
            ui.add(egui::Slider::new(&mut self.params.arp_rate, 60.0..=240.0).step_by(1.0))
        })
        .on_hover_text("Arpeggiator tempo (steps per minute)");

        let mut arp_pattern = Synthesizer::u8_to_arp_pattern_pub(self.params.arp_pattern);
        labeled(ui, "pattern:", |ui| {
            let pattern_text = match arp_pattern {
                ArpPattern::Up => "Up",
                ArpPattern::Down => "Down",
                ArpPattern::UpDown => "Up-Down",
                ArpPattern::Random => "Random",
            };
            egui::ComboBox::from_id_salt("arp_pattern")
                .selected_text(pattern_text)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut arp_pattern, ArpPattern::Up, "Up");
                    ui.selectable_value(&mut arp_pattern, ArpPattern::Down, "Down");
                    ui.selectable_value(&mut arp_pattern, ArpPattern::UpDown, "Up-Down");
                    ui.selectable_value(&mut arp_pattern, ArpPattern::Random, "Random");
                })
                .response
        })
        .on_hover_text("Note order: Up = ascending, Down = descending, Up-Down = bounce, Random = shuffle");
        self.params.arp_pattern = Synthesizer::arp_pattern_to_u8_pub(arp_pattern);

        let mut octaves_f32 = self.params.arp_octaves as f32;
        labeled(ui, "octaves:", |ui| {
            ui.add(egui::Slider::new(&mut octaves_f32, 1.0..=4.0).step_by(1.0))
        })
        .on_hover_text("Octave range — 1 = within the held chord, 4 = up to 4 octaves above");
        self.params.arp_octaves = octaves_f32 as u8;

        labeled(ui, "gate:", |ui| {
            ui.add(egui::Slider::new(&mut self.params.arp_gate_length, 0.1..=1.0).step_by(0.01))
        })
        .on_hover_text("Note duration as a fraction of one step (1.0 = legato, 0.1 = staccato)");

        ui.separator();
        ui.checkbox(&mut self.params.arp_sync_to_midi, "sync to MIDI clock")
            .on_hover_text("Lock arpeggiator rate to incoming MIDI clock instead of internal BPM");
    }

    fn draw_voice_mode_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

        labeled(ui, "mode:", |ui| {
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
                })
                .response
        })
        .on_hover_text("Poly: chords / Mono: 1 voice retrigs / Legato: 1 voice slides / Unison: all voices stacked on one note");

        // Note priority — solo relevante en Mono/Legato
        if self.params.voice_mode == 1 || self.params.voice_mode == 2 {
            labeled(ui, "priority:", |ui| {
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
                    })
                    .response
            })
            .on_hover_text("Which note wins when several are held — Last/Low/High pitch");
        }

        // Unison spread — solo relevante en Unison
        if self.params.voice_mode == 3 {
            labeled(ui, "spread (c):", |ui| {
                ui.add(egui::Slider::new(&mut self.params.unison_spread, 0.0..=50.0).step_by(0.5))
            })
            .on_hover_text("Detune between unison voices in cents (100 c = 1 semitone)");
        }

        let mut max_v = self.params.max_voices as f32;
        labeled(ui, "voices:", |ui| {
            ui.add(egui::Slider::new(&mut max_v, 1.0..=8.0).step_by(1.0))
        })
        .on_hover_text("Maximum simultaneous voices (1-8). Lower = older notes get stolen sooner");
        self.params.max_voices = max_v as u8;
    }

    /// All category names used by built-in and user presets. Order defines
    /// how groups are rendered in the browser and listed in the save combo.
    const PRESET_CATEGORIES: &'static [&'static str] =
        &["Bass", "Lead", "Pad", "Strings", "Brass", "FX", "Sequence", "Other"];

    fn draw_preset_panel(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

        // Header: current preset status — always visible at the top.
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("current:").size(11.0).strong());
            if self.current_preset_name.is_empty() {
                ui.colored_label(egui::Color32::GRAY, "(no preset loaded)");
            } else {
                ui.colored_label(
                    egui::Color32::from_rgb(100, 220, 100),
                    &self.current_preset_name,
                );
            }
        });
        ui.separator();

        // ── Primary action: browse & select ───────────────────────────────
        ui.horizontal(|ui| {
            ui.label("search:");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.preset_search)
                    .hint_text("type to filter...")
                    .desired_width(140.0),
            );
            if resp.changed() {
                // Kept for future: debounce / live preview hooks.
            }
            if ui.small_button("x").on_hover_text("Clear search").clicked() {
                self.preset_search.clear();
            }
        });

        ui.horizontal(|ui| {
            ui.label("category:");
            egui::ComboBox::from_id_salt("preset_cat_filter")
                .selected_text(&self.preset_category_filter)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.preset_category_filter, "All".to_string(), "All");
                    for cat in Self::PRESET_CATEGORIES {
                        ui.selectable_value(
                            &mut self.preset_category_filter,
                            cat.to_string(),
                            *cat,
                        );
                    }
                });
        });

        ui.separator();

        // Group presets by category after applying search + category filters.
        let all_presets = Synthesizer::list_presets_with_categories();
        let search_lower = self.preset_search.to_lowercase();
        let filtered: Vec<(String, String)> = all_presets
            .into_iter()
            .filter(|(name, cat)| {
                let cat_ok = self.preset_category_filter == "All"
                    || cat == &self.preset_category_filter;
                let name_ok = search_lower.is_empty()
                    || name.to_lowercase().contains(&search_lower);
                cat_ok && name_ok
            })
            .collect();

        if filtered.is_empty() {
            ui.label(
                egui::RichText::new("no presets match")
                    .color(egui::Color32::GRAY)
                    .italics(),
            );
        } else {
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    let mut last_category: Option<&str> = None;
                    for (preset, category) in &filtered {
                        // Category header (only when category changes).
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

                        let is_current = preset == &self.current_preset_name;
                        let button = egui::Button::new(preset).wrap_mode(egui::TextWrapMode::Truncate);
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
                                self.params = temp_synth.to_synth_params();
                                self.current_preset_name = preset.clone();
                            }
                        }
                    }
                });
        }

        ui.separator();

        // ── Secondary: create / edit (collapsed by default) ───────────────
        egui::CollapsingHeader::new("Create / Edit")
            .id_salt("preset_editor_section")
            .default_open(self.show_preset_editor)
            .show(ui, |ui| {
                // Save current patch as a new preset.
                ui.horizontal(|ui| {
                    ui.label("category:");
                    egui::ComboBox::from_id_salt("preset_cat_save")
                        .selected_text(&self.preset_category)
                        .show_ui(ui, |ui| {
                            for cat in Self::PRESET_CATEGORIES {
                                ui.selectable_value(
                                    &mut self.preset_category,
                                    cat.to_string(),
                                    *cat,
                                );
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_preset_name)
                            .hint_text("preset name...")
                            .desired_width(130.0),
                    );
                    let save_enabled = !self.new_preset_name.is_empty();
                    if ui.add_enabled(save_enabled, egui::Button::new("Save")).clicked() {
                        let mut temp_synth = Synthesizer::new();
                        temp_synth.apply_params(&self.params);
                        if let Err(e) = temp_synth.save_preset_with_category(
                            &self.new_preset_name,
                            &self.preset_category,
                        ) {
                            log::error!("Error saving preset: {}", e);
                        } else {
                            log::info!(
                                "Preset '{}' [{}] saved!",
                                self.new_preset_name,
                                self.preset_category
                            );
                            self.current_preset_name = self.new_preset_name.clone();
                            self.new_preset_name.clear();
                        }
                    }
                });

                ui.separator();

                // A/B comparison.
                ui.label(egui::RichText::new("A/B comparison").size(10.0).strong());
                ui.horizontal(|ui| {
                    if ui.button("-> A").on_hover_text("Store current patch to slot A").clicked() {
                        self.params_a = Some(self.params);
                    }
                    if ui
                        .add_enabled(self.params_a.is_some(), egui::Button::new("A"))
                        .on_hover_text("Load slot A")
                        .clicked()
                    {
                        self.params = self.params_a.unwrap();
                    }
                    ui.separator();
                    if ui.button("-> B").on_hover_text("Store current patch to slot B").clicked() {
                        self.params_b = Some(self.params);
                    }
                    if ui
                        .add_enabled(self.params_b.is_some(), egui::Button::new("B"))
                        .on_hover_text("Load slot B")
                        .clicked()
                    {
                        self.params = self.params_b.unwrap();
                    }
                });

                ui.separator();

                // Utility actions.
                if ui.button("Random patch").clicked() {
                    self.params = Self::random_params();
                    self.current_preset_name.clear();
                }
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
            });
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
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

        ui.label(
            egui::RichText::new("Filter Env ->")
                .size(10.0)
                .color(egui::Color32::GRAY),
        );
        labeled(ui, "freq A:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.poly_mod_filter_env_to_osc_a_freq, -1.0..=1.0)
                    .step_by(0.01),
            )
        })
        .on_hover_text("FILTER ENV modulates osc A pitch (negative = inverted envelope)");
        labeled(ui, "pw A:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.poly_mod_filter_env_to_osc_a_pw, -1.0..=1.0)
                    .step_by(0.01),
            )
        })
        .on_hover_text("FILTER ENV modulates osc A pulse width (PWM via envelope)");

        ui.separator();

        ui.label(
            egui::RichText::new("Osc B ->")
                .size(10.0)
                .color(egui::Color32::GRAY),
        );
        labeled(ui, "freq A:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.poly_mod_osc_b_to_osc_a_freq, -1.0..=1.0)
                    .step_by(0.01),
            )
        })
        .on_hover_text("Cross-modulation: osc B modulates osc A pitch (audio-rate FM, classic Prophet sound)");
        labeled(ui, "pw A:", |ui| {
            ui.add(
                egui::Slider::new(&mut self.params.poly_mod_osc_b_to_osc_a_pw, -1.0..=1.0)
                    .step_by(0.01),
            )
        })
        .on_hover_text("Osc B modulates osc A pulse width at audio rate");
        labeled(ui, "cutoff:", |ui| {
            ui.add(
                egui::Slider::new(
                    &mut self.params.poly_mod_osc_b_to_filter_cutoff,
                    -1.0..=1.0,
                )
                .step_by(0.01),
            )
        })
        .on_hover_text("Osc B modulates filter cutoff at audio rate (creates harmonics / metallic timbres)");
    }

    fn draw_keyboard_legend(&mut self, ui: &mut egui::Ui) {
        let legend_color = egui::Color32::from_gray(70);
        ui.horizontal(|ui| {
            // Octave indicator
            ui.vertical(|ui| {
                ui.set_min_width(72.0);
                ui.label(
                    egui::RichText::new(format!("Oct: {}", self.current_octave))
                        .size(12.0)
                        .strong()
                        .color(legend_color),
                );
                ui.label(
                    egui::RichText::new("Up/Dn to change")
                        .size(9.0)
                        .color(legend_color),
                );
            });

            ui.separator();

            // Lower octave — visual QWERTY layout (black keys row, white keys row)
            ui.vertical(|ui| {
                ui.set_min_width(175.0);
                ui.label(
                    egui::RichText::new(
                        format!("  S   D     G   H   J      oct {}", self.current_octave),
                    )
                    .size(10.0)
                    .monospace()
                    .color(legend_color),
                );
                ui.label(
                    egui::RichText::new("Z   X   C   V   B   N   M")
                        .size(10.0)
                        .monospace()
                        .color(legend_color),
                );
            });

            ui.separator();

            // Upper octave
            ui.vertical(|ui| {
                ui.set_min_width(215.0);
                ui.label(
                    egui::RichText::new(
                        format!("  2   3     5   6   7        oct {}", self.current_octave + 1),
                    )
                    .size(10.0)
                    .monospace()
                    .color(legend_color),
                );
                ui.label(
                    egui::RichText::new("Q   W   E   R   T   Y   U   I   O   P")
                        .size(10.0)
                        .monospace()
                        .color(legend_color),
                );
            });
        });
    }

    fn random_params() -> SynthParameters {
        // rand 0.10 removed thread_rng/gen_range; use rand::random::<f32>() directly.
        // Scale a [0,1) value into [lo, hi].
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
}

impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Read current params at start of frame
        self.params = *self.lock_free_synth.get_params();
        let peak_bits = self.lock_free_synth.peak_level.load(std::sync::atomic::Ordering::Relaxed);
        self.peak_level = f32::from_bits(peak_bits);

        // PANIC on focus loss: si la ventana pierde el foco mientras hay teclas
        // pulsadas, los key_released nunca llegan y las voces quedan enganchadas.
        // Disparamos AllNotesOff defensivamente para limpiar el estado.
        let focused = ctx.input(|i| i.focused);
        if !focused && !self.last_key_times.is_empty() {
            self.midi_events.push(MidiEvent::AllNotesOff);
            self.last_key_times.clear();
        }

        // Handle keyboard input
        ctx.input(|i| {
            // Esc = PANIC universal
            if i.key_pressed(egui::Key::Escape) {
                self.midi_events.push(MidiEvent::AllNotesOff);
                self.last_key_times.clear();
            }

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
                if i.key_pressed(key) {
                    let should_trigger = match self.last_key_times.get(&key) {
                        // If more than 100ms since last press, it's intentional (not auto-repeat)
                        Some((_, last_time)) => now.duration_since(*last_time).as_millis() > 100,
                        // First time pressing this key
                        None => true,
                    };

                    if should_trigger {
                        let note = (self.current_octave * 12 + note_offset).clamp(0, 127) as u8;
                        self.last_key_times.insert(key, (note, now));
                        self.midi_events.push(MidiEvent::NoteOn {
                            note,
                            velocity: 100,
                        });
                    }
                }

                if i.key_released(key)
                    && let Some((stored_note, _)) = self.last_key_times.remove(&key)
                {
                    // Usamos la nota grabada en el press, no la recalculamos:
                    // si el usuario cambió de octava mientras mantenía la tecla,
                    // recalcular enviaría NoteOff a una nota que nunca sonó.
                    self.midi_events.push(MidiEvent::NoteOff { note: stored_note });
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

            // Current preset name — clickable to open the preset manager.
            let (preset_text, preset_color) = if self.current_preset_name.is_empty() {
                ("no preset".to_string(), egui::Color32::from_gray(140))
            } else {
                (self.current_preset_name.clone(), egui::Color32::from_rgb(100, 220, 100))
            };
            if ui
                .add(egui::Label::new(
                    egui::RichText::new(format!("> {}", preset_text))
                        .size(13.0)
                        .color(preset_color),
                ).sense(egui::Sense::click()))
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
                    let btn_text = if self.show_midi_learn { "MIDI Learn *" } else { "MIDI Learn" };
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
                    self.midi_events.push(MidiEvent::AllNotesOff);
                    self.last_key_times.clear();
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
                    section(ui, "OSCILLATOR A", |ui| self.draw_vintage_oscillator_panel(ui, 1));
                    ui.add_space(4.0);
                    section(ui, "OSCILLATOR B", |ui| self.draw_vintage_oscillator_panel(ui, 2));
                    ui.add_space(4.0);
                    section(ui, "ANALOG", |ui| self.draw_analog_panel(ui));
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

                // ── COL 3: ENV/LFO/VOICE (220 px) ───────────────────────
                ui.vertical(|ui| {
                    ui.set_min_width(220.0);
                    ui.set_max_width(220.0);
                    section(ui, "FILTER ENV", |ui| {
                        draw_envelope_panel(
                            ui,
                            &mut self.params.filter_attack,
                            &mut self.params.filter_decay,
                            &mut self.params.filter_sustain,
                            &mut self.params.filter_release,
                        );
                        self.draw_adsr_curve(
                            ui,
                            self.params.filter_attack,
                            self.params.filter_decay,
                            self.params.filter_sustain,
                            self.params.filter_release,
                        );
                    });
                    ui.add_space(4.0);
                    section(ui, "LFO", |ui| self.draw_lfo_panel(ui));
                    ui.add_space(4.0);
                    section(ui, "VOICE MODE", |ui| self.draw_voice_mode_panel(ui));
                });

                // ── COL 4: ENV/LFO MOD/ARP (220 px) ─────────────────────
                ui.vertical(|ui| {
                    ui.set_min_width(220.0);
                    ui.set_max_width(220.0);
                    section(ui, "AMP ENV", |ui| {
                        draw_envelope_panel(
                            ui,
                            &mut self.params.amp_attack,
                            &mut self.params.amp_decay,
                            &mut self.params.amp_sustain,
                            &mut self.params.amp_release,
                        );
                        self.draw_adsr_curve(
                            ui,
                            self.params.amp_attack,
                            self.params.amp_decay,
                            self.params.amp_sustain,
                            self.params.amp_release,
                        );
                    });
                    ui.add_space(4.0);
                    section(ui, "LFO MOD", |ui| self.draw_lfo_mod_panel(ui));
                    ui.add_space(4.0);
                    section(ui, "ARP", |ui| self.draw_arpeggiator_panel(ui));
                });

                // ── COL 5: SALIDA (220 px) ──────────────────────────────
                ui.vertical(|ui| {
                    ui.set_min_width(220.0);
                    ui.set_max_width(220.0);
                    section(ui, "MASTER", |ui| self.draw_master_panel(ui));
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
                        if ui
                            .small_button("manage...")
                            .on_hover_text("Open preset manager — save/load patches by category")
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

    fn draw_midi_learn_panel(&mut self, ui: &mut egui::Ui) {
        let learnable_params: &[(&str, &str)] = &[
            ("filter_cutoff", "Filter Cutoff"),
            ("filter_resonance", "Filter Resonance"),
            ("filter_envelope_amount", "Filter Env Amount"),
            ("amp_attack", "Amp Attack"),
            ("amp_decay", "Amp Decay"),
            ("amp_sustain", "Amp Sustain"),
            ("amp_release", "Amp Release"),
            ("filter_attack", "Filter Attack"),
            ("filter_decay", "Filter Decay"),
            ("filter_sustain", "Filter Sustain"),
            ("filter_release", "Filter Release"),
            ("lfo_rate", "LFO Rate"),
            ("lfo_amount", "LFO Amount"),
            ("master_volume", "Master Volume"),
            ("reverb_amount", "Reverb Amount"),
            ("delay_feedback", "Delay Feedback"),
            ("delay_amount", "Delay Amount"),
            ("osc1_detune", "Osc A Detune"),
            ("osc2_detune", "Osc B Detune"),
        ];

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

            egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                for (param_key, param_label) in learnable_params {
                    // Collect binding info before mutable ops
                    let bound_cc: Option<u8> = learn_arc.try_lock().ok().and_then(|state| {
                        state
                            .custom_map
                            .iter()
                            .find(|(_, v)| v.as_str() == *param_key)
                            .map(|(cc, _)| *cc)
                    });

                    ui.horizontal(|ui| {
                        ui.set_min_width(200.0);
                        ui.label(*param_label);
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
                                state.custom_map.retain(|_, v| v.as_str() != *param_key);
                            }
                        } else {
                            ui.colored_label(egui::Color32::GRAY, "-");
                        }
                    });
                }
            });
        } else {
            ui.label("No MIDI device connected.");
        }
    }
}
