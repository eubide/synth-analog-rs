//! Paneles de la GUI como funciones libres.
//!
//! Cada función toma `&mut SynthParameters` (y en casos puntuales algún
//! parámetro adicional como `osc_num` o el `octave` para la leyenda de
//! teclado). Son puras respecto a cualquier otro estado del `SynthApp`, lo
//! que las hace testeables en aislamiento y reemplazables sin tocar el
//! orquestador.

use crate::lock_free::SynthParameters;
use crate::synthesizer::{ArpPattern, LfoWaveform, Synthesizer, WaveType};
use eframe::egui;

/// Ancho fijo de las etiquetas (incluyen unidad entre paréntesis). El layout
/// reserva exactamente `LABEL_WIDTH + WIDGET_WIDTH` por fila.
pub const LABEL_WIDTH: f32 = 95.0;
pub const WIDGET_WIDTH: f32 = 105.0;

const AMBER: egui::Color32 = egui::Color32::from_rgb(0xe8, 0x97, 0x1a);
const DARK_GRAY: egui::Color32 = egui::Color32::from_rgb(0x2a, 0x2a, 0x2a);
const DIM: egui::Color32 = egui::Color32::from_gray(0xaa);

/// Slider horizontal compacto: etiqueta small (fija 28px) + slider + valor.
/// Diseñado para secciones estrechas donde labeled() (200px) no cabe.
pub fn compact_hslider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(28.0, ui.spacing().interact_size.y),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| ui.label(egui::RichText::new(label).size(9.5).color(DIM)),
        );
        ui.add(egui::Slider::new(value, range).step_by(0.01).fixed_decimals(2))
    })
    .inner
}

/// Fila LFO target: [LED toggle ámbar] + [slider amount].
/// Permite encender/apagar el target y ajustar la profundidad en la misma fila.
pub fn lfo_target_row(
    ui: &mut egui::Ui,
    label: &str,
    active: &mut bool,
    amount: &mut f32,
) {
    ui.horizontal(|ui| {
        let color = if *active { AMBER } else { DARK_GRAY };
        let text_col = if *active { egui::Color32::BLACK } else { DIM };
        if ui
            .add(
                egui::Button::new(egui::RichText::new(label).size(9.0).color(text_col))
                    .fill(color)
                    .corner_radius(2.0)
                    .min_size(egui::vec2(30.0, 16.0)),
            )
            .clicked()
        {
            *active = !*active;
        }
        ui.add(egui::Slider::new(amount, 0.0..=1.0).step_by(0.01).show_value(false));
        ui.label(egui::RichText::new(format!("{:.2}", *amount)).size(9.0).color(DIM));
    });
}

/// Slider vertical con etiqueta debajo. Devuelve la respuesta del slider.
pub fn vslider(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
    height: f32,
) -> egui::Response {
    ui.vertical(|ui| {
        ui.spacing_mut().slider_width = height;
        let resp = ui.add(egui::Slider::new(value, range).vertical().show_value(false));
        ui.label(
            egui::RichText::new(label)
                .size(9.0)
                .color(egui::Color32::from_gray(0xcc)),
        );
        resp
    })
    .inner
}

/// Renderiza un grupo con título uniforme estilo dark amber.
pub fn section(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.group(|ui| {
        ui.label(
            egui::RichText::new(title)
                .size(11.0)
                .strong()
                .color(egui::Color32::WHITE),
        );
        add_contents(ui);
    });
}

/// LED button: amber cuando activo, gris oscuro cuando inactivo.
pub fn led_button(ui: &mut egui::Ui, label: &str, active: &mut bool) -> egui::Response {
    let color = if *active { AMBER } else { DARK_GRAY };
    let text_color = if *active {
        egui::Color32::BLACK
    } else {
        egui::Color32::from_gray(0xaa)
    };
    let resp = ui.add(
        egui::Button::new(egui::RichText::new(label).size(9.5).color(text_color))
            .fill(color)
            .corner_radius(3.0),
    );
    if resp.clicked() {
        *active = !*active;
    }
    resp
}

/// Fila etiquetada: etiqueta de ancho fijo right-aligned + widget en slot de
/// ancho fijo. Cada fila ocupa exactamente `LABEL_WIDTH + WIDGET_WIDTH`.
pub fn labeled<R>(
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

/// Variante con checkbox prefijo (LFO MOD). El slot del checkbox se reserva
/// siempre — si `target` es `None`, queda en blanco para alinear filas con y
/// sin toggle.
#[allow(dead_code)]
pub fn labeled_check<R>(
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

/// Panel ADSR: 4 sliders verticales con etiqueta + valor numérico.
pub fn draw_envelope(
    ui: &mut egui::Ui,
    attack: &mut f32,
    decay: &mut f32,
    sustain: &mut f32,
    release: &mut f32,
) {
    ui.spacing_mut().item_spacing = egui::vec2(5.0, 2.0);
    ui.horizontal(|ui| {
        adsr_col(ui, attack,  0.001..=5.0, "A", "Attack");
        adsr_col(ui, decay,   0.001..=5.0, "D", "Decay");
        adsr_col(ui, sustain, 0.0..=1.0,   "S", "Sustain");
        adsr_col(ui, release, 0.001..=5.0, "R", "Release");
    });
}

fn adsr_col(
    ui: &mut egui::Ui,
    val: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
    hint: &str,
) {
    ui.vertical(|ui| {
        ui.spacing_mut().slider_width = 88.0;
        ui.add(egui::Slider::new(val, range).vertical().show_value(false))
            .on_hover_text(hint);
        ui.label(egui::RichText::new(label).size(10.0).strong().color(DIM));
        ui.label(egui::RichText::new(format!("{:.2}", *val)).size(8.5).color(egui::Color32::from_gray(0x77)));
    });
}


/// Mini ADSR curve — 32px alto, actualiza en tiempo real con los sliders.
pub fn draw_adsr_curve(
    ui: &mut egui::Ui,
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::hover());
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

pub fn draw_oscillator(ui: &mut egui::Ui, params: &mut SynthParameters, osc_num: u8) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

    let (waveform, detune, pulse_width) = if osc_num == 1 {
        (
            &mut params.osc1_waveform,
            &mut params.osc1_detune,
            &mut params.osc1_pulse_width,
        )
    } else {
        (
            &mut params.osc2_waveform,
            &mut params.osc2_detune,
            &mut params.osc2_pulse_width,
        )
    };

    // Octave switch: 0=16' 1=8' 2=4'
    let octave_ref = if osc_num == 1 {
        &mut params.osc1_octave
    } else {
        &mut params.osc2_octave
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("octave:").size(10.0).color(egui::Color32::from_gray(0xaa)));
        for (val, lbl) in [(0i8, "16'"), (1, "8'"), (2, "4'")] {
            let selected = *octave_ref == val;
            let color = if selected { AMBER } else { DARK_GRAY };
            let text_col = if selected { egui::Color32::BLACK } else { egui::Color32::from_gray(0xaa) };
            if ui
                .add(egui::Button::new(egui::RichText::new(lbl).size(9.0).color(text_col)).fill(color).corner_radius(2.0))
                .on_hover_text("Octave range: 16'=sub, 8'=normal, 4'=high")
                .clicked()
            {
                *octave_ref = val;
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("tune:").size(10.0).color(egui::Color32::from_gray(0xaa)));
        ui.spacing_mut().slider_width = 75.0;
        ui.add(egui::Slider::new(detune, -24.0..=24.0).step_by(0.5))
            .on_hover_text("Pitch detune in semitones");
    });

    let mut wave_type = Synthesizer::u8_to_wave_type_pub(*waveform);
    ui.horizontal(|ui| {
        for (wt, lbl, hint) in [
            (WaveType::Sawtooth, "Saw", "Sawtooth"),
            (WaveType::Triangle, "Tri", "Triangle"),
            (WaveType::Square, "Sqr", "Square / Pulse"),
            (WaveType::Sine, "Sin", "Sine"),
        ] {
            let selected = wave_type == wt;
            let color = if selected { AMBER } else { DARK_GRAY };
            let text_col = if selected { egui::Color32::BLACK } else { DIM };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(lbl).size(9.0).color(text_col))
                        .fill(color)
                        .corner_radius(2.0)
                        .min_size(egui::vec2(26.0, 16.0)),
                )
                .on_hover_text(hint)
                .clicked()
            {
                wave_type = wt;
            }
        }
    });
    *waveform = Synthesizer::wave_type_to_u8_pub(wave_type);

    if wave_type == WaveType::Square {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("pw:").size(10.0).color(egui::Color32::from_gray(0xaa)));
            ui.spacing_mut().slider_width = 75.0;
            ui.add(egui::Slider::new(pulse_width, 0.1..=0.9).step_by(0.01))
                .on_hover_text("Pulse width — 0.5 = symmetric square");
        });
    }

    if osc_num == 2 {
        ui.horizontal(|ui| {
            led_button(ui, "sync→A", &mut params.osc2_sync)
                .on_hover_text("Hard sync osc B to osc A — every osc A cycle resets osc B");
            led_button(ui, "sub-osc", &mut params.osc2_lfo_mode)
                .on_hover_text("Osc B in sub-audio range (freq x 0.01)");
            led_button(ui, "kbd", &mut params.osc2_keyboard_track)
                .on_hover_text("Keyboard tracking — off = fixed pitch");
        });
    }
}

/// Mixer: 3 faders verticales con etiqueta + valor. Diseñado para caber en 90px.
pub fn draw_mixer(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);
    ui.horizontal(|ui| {
        for (val, lbl, hint) in [
            (&mut params.mixer_osc1_level, "A", "Oscillator A level"),
            (&mut params.mixer_osc2_level, "B", "Oscillator B level"),
            (&mut params.noise_level,      "N", "Noise generator level"),
        ] {
            ui.vertical(|ui| {
                ui.spacing_mut().slider_width = 88.0;
                ui.add(egui::Slider::new(val, 0.0..=1.0).vertical().show_value(false))
                    .on_hover_text(hint);
                ui.label(egui::RichText::new(lbl).size(10.0).strong().color(DIM));
                ui.label(egui::RichText::new(format!("{:.2}", *val)).size(8.5).color(egui::Color32::from_gray(0x77)));
            });
        }
    });
}

pub fn draw_filter(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

    ui.horizontal(|ui| {
        // cutoff LOG: bottom=20Hz, middle≈632Hz, top=20kHz
        ui.vertical(|ui| {
            ui.spacing_mut().slider_width = 88.0;
            ui.add(
                egui::Slider::new(&mut params.filter_cutoff, 20.0..=20000.0)
                    .vertical()
                    .logarithmic(true)
                    .show_value(false),
            )
            .on_hover_text(format!("Cutoff: {:.0} Hz", params.filter_cutoff));
            ui.label(egui::RichText::new("cut").size(10.0).strong().color(DIM));
            ui.label(egui::RichText::new(format!("{:.0}Hz", params.filter_cutoff)).size(8.5).color(egui::Color32::from_gray(0x77)));
        });
        adsr_col(ui, &mut params.filter_resonance,     0.0..=4.0,  "res", "Resonance (≥3.8 = self-osc)");
        adsr_col(ui, &mut params.filter_envelope_amount, -1.0..=1.0, "env", "Envelope > cutoff mod");
    });
    if params.filter_resonance >= 3.8 {
        ui.colored_label(egui::Color32::from_rgb(255, 160, 60), "◉ self-osc");
    }
    ui.spacing_mut().slider_width = 80.0;
    ui.add(
        egui::Slider::new(&mut params.filter_keyboard_tracking, 0.0..=1.0)
            .text("kbd")
            .step_by(0.01),
    )
    .on_hover_text("Keyboard tracking — higher notes open the filter more");
    ui.add(
        egui::Slider::new(&mut params.velocity_to_cutoff, 0.0..=1.0)
            .text("vel")
            .step_by(0.01),
    )
    .on_hover_text("Velocity → cutoff amount");
}

/// LFO timing — waveform, rate, amount, sync, delay. Pareja con `draw_lfo_mod`.
pub fn draw_lfo(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

    let mut lfo_waveform = Synthesizer::u8_to_lfo_waveform_pub(params.lfo_waveform);
    ui.horizontal(|ui| {
        for (wf, lbl, hint) in [
            (LfoWaveform::Triangle, "Tri", "Triangle"),
            (LfoWaveform::Square, "Sqr", "Square"),
            (LfoWaveform::Sawtooth, "Saw", "Sawtooth"),
            (LfoWaveform::ReverseSawtooth, "Rev", "Reverse Sawtooth"),
            (LfoWaveform::SampleAndHold, "S&H", "Sample & Hold — random steps"),
        ] {
            let selected = lfo_waveform == wf;
            let color = if selected { AMBER } else { DARK_GRAY };
            let text_col = if selected { egui::Color32::BLACK } else { DIM };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(lbl).size(9.0).color(text_col))
                        .fill(color)
                        .corner_radius(2.0)
                        .min_size(egui::vec2(22.0, 16.0)),
                )
                .on_hover_text(hint)
                .clicked()
            {
                lfo_waveform = wf;
            }
        }
    });
    params.lfo_waveform = Synthesizer::lfo_waveform_to_u8_pub(lfo_waveform);

    ui.horizontal(|ui| {
        vslider(ui, &mut params.lfo_rate, 0.05..=30.0, "rate", 60.0)
            .on_hover_text("LFO frequency (Hz)");
        vslider(ui, &mut params.lfo_amount, 0.0..=1.0, "amnt", 60.0)
            .on_hover_text("Global LFO depth");
        vslider(ui, &mut params.lfo_delay, 0.0..=5.0, "dly", 60.0)
            .on_hover_text("Delayed vibrato fade-in (s)");
    });
    led_button(ui, "key sync", &mut params.lfo_sync)
        .on_hover_text("Reset LFO phase on every note");
}

/// LFO mod — LED toggle + slider en la misma fila. Diseñado para 205px.
pub fn draw_lfo_mod(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
    ui.spacing_mut().slider_width = 105.0;

    lfo_target_row(ui, "fq A", &mut params.lfo_target_osc1_pitch, &mut params.lfo_to_osc1_pitch);
    lfo_target_row(ui, "fq B", &mut params.lfo_target_osc2_pitch, &mut params.lfo_to_osc2_pitch);
    lfo_target_row(ui, "filt", &mut params.lfo_target_filter,     &mut params.lfo_to_cutoff);
    lfo_target_row(ui, "amp ", &mut params.lfo_target_amplitude,  &mut params.lfo_to_amplitude);

    // res: sin target LED (siempre activo)
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("res ").size(9.0).color(DIM)
            .background_color(DARK_GRAY));
        ui.add(egui::Slider::new(&mut params.lfo_to_resonance, 0.0..=1.0).step_by(0.01).show_value(false));
        ui.label(egui::RichText::new(format!("{:.2}", params.lfo_to_resonance)).size(9.0).color(DIM));
    });

    // PW targets: LED toggle sin slider (depth fija al 40% en DSP)
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        led_button(ui, "pw A", &mut params.lfo_target_osc1_pw)
            .on_hover_text("LFO > Osc A pulse width (depth fija 40%)");
        led_button(ui, "pw B", &mut params.lfo_target_osc2_pw)
            .on_hover_text("LFO > Osc B pulse width (depth fija 40%)");
    });
}

pub fn draw_master(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

    labeled(ui, "volume:", |ui| {
        ui.add(egui::Slider::new(&mut params.master_volume, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Master output level");
    labeled(ui, "glide (s):", |ui| {
        ui.add(egui::Slider::new(&mut params.glide_time, 0.0..=2.0).step_by(0.01))
    })
    .on_hover_text("Portamento — pitch slide time between consecutive notes");
    let mut range_f32 = params.pitch_bend_range as f32;
    labeled(ui, "bend (st):", |ui| {
        ui.add(egui::Slider::new(&mut range_f32, 1.0..=24.0).step_by(1.0))
    })
    .on_hover_text("Pitch bend wheel range in semitones");
    params.pitch_bend_range = range_f32 as u8;

    ui.separator();
    ui.label(egui::RichText::new("velocity").size(10.0).strong());

    labeled(ui, "-> vol:", |ui| {
        ui.add(egui::Slider::new(&mut params.velocity_to_amplitude, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("How much MIDI velocity affects note loudness");
    labeled(ui, "curve:", |ui| {
        egui::ComboBox::from_id_salt("velocity_curve")
            .selected_text(match params.velocity_curve {
                1 => "Soft",
                2 => "Hard",
                _ => "Linear",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut params.velocity_curve, 0, "Linear");
                ui.selectable_value(&mut params.velocity_curve, 1, "Soft");
                ui.selectable_value(&mut params.velocity_curve, 2, "Hard");
            })
            .response
    })
    .on_hover_text(
        "Velocity response curve — Soft = expressive, Hard = aggressive, Linear = neutral",
    );

    ui.separator();
    ui.label(egui::RichText::new("aftertouch").size(10.0).strong());

    labeled(ui, "-> cutoff:", |ui| {
        ui.add(egui::Slider::new(&mut params.aftertouch_to_cutoff, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Channel pressure modulates filter cutoff (press harder = brighter)");
    labeled(ui, "-> amp:", |ui| {
        ui.add(egui::Slider::new(&mut params.aftertouch_to_amplitude, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Channel pressure modulates loudness (press harder = louder)");

    ui.add_space(2.0);
    labeled(ui, "tuning:", |ui| {
        let modes = ["Equal Temp.", "Just Inton.", "Pythagorean", "Werckmeister"];
        egui::ComboBox::from_id_salt("tuning_mode")
            .selected_text(modes[params.tuning_mode as usize])
            .show_ui(ui, |ui| {
                for (i, name) in modes.iter().enumerate() {
                    ui.selectable_value(&mut params.tuning_mode, i as u8, *name);
                }
            })
            .response
    })
    .on_hover_text("Alternate tuning system — all anchored to A4 = 440 Hz");

    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("oversamp:").size(10.0));
        ui.selectable_value(&mut params.oversampling, 1u8, "1×");
        ui.selectable_value(&mut params.oversampling, 2u8, "2×");
        ui.selectable_value(&mut params.oversampling, 4u8, "4×");
    });

    labeled(ui, "spread (st):", |ui| {
        ui.add(egui::Slider::new(&mut params.stereo_spread, 0.0..=1.0).fixed_decimals(2))
    })
    .on_hover_text(
        "Stereo spread: distributes voices across L/R field (0 = mono, 1 = full spread)",
    );

    ui.add_space(4.0);
    let active = params.reference_tone;
    let btn = egui::Button::new(if active { "A-440 [ON]" } else { "A-440" });
    if ui
        .add(if active {
            btn.fill(egui::Color32::from_rgb(180, 80, 30))
        } else {
            btn
        })
        .on_hover_text("Emite La4 puro a 440 Hz para afinar — bypasea toda la sintesis")
        .clicked()
    {
        params.reference_tone = !active;
    }
}

pub fn draw_effects(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

    ui.label(egui::RichText::new("chorus").size(10.0).strong());
    labeled(ui, "mix:", |ui| {
        ui.add(egui::Slider::new(&mut params.chorus_mix, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Chorus dry/wet mix (0 = bypass, 1 = full wet)");
    labeled(ui, "rate (Hz):", |ui| {
        ui.add(egui::Slider::new(&mut params.chorus_rate, 0.1..=3.0).step_by(0.01))
    })
    .on_hover_text("Chorus modulation rate (slow = lush, fast = warbly)");
    labeled(ui, "depth:", |ui| {
        ui.add(egui::Slider::new(&mut params.chorus_depth, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Chorus modulation depth — how much the delay time wobbles");

    ui.label(egui::RichText::new("reverb").size(10.0).strong());
    labeled(ui, "amount:", |ui| {
        ui.add(egui::Slider::new(&mut params.reverb_amount, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Reverb dry/wet mix");
    labeled(ui, "size:", |ui| {
        ui.add(egui::Slider::new(&mut params.reverb_size, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Reverb room size — small = booth, large = cathedral");

    ui.label(egui::RichText::new("delay").size(10.0).strong());
    labeled(ui, "time (s):", |ui| {
        ui.add(egui::Slider::new(&mut params.delay_time, 0.01..=2.0).step_by(0.01))
    })
    .on_hover_text("Delay time between echoes");
    labeled(ui, "feedback:", |ui| {
        ui.add(egui::Slider::new(&mut params.delay_feedback, 0.0..=0.95).step_by(0.01))
    })
    .on_hover_text(
        "Echo feedback — higher = more repetitions (capped at 0.95 to avoid runaway)",
    );
    labeled(ui, "amount:", |ui| {
        ui.add(egui::Slider::new(&mut params.delay_amount, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Delay dry/wet mix");
}

/// Analog character — sliders horizontales compactos. Diseñado para 130px.
pub fn draw_analog(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(3.0, 4.0);
    ui.spacing_mut().slider_width = 65.0;

    compact_hslider(ui, "toler", &mut params.analog_component_tolerance, 0.0..=1.0)
        .on_hover_text("Per-voice component tolerance (resistor/cap spread)");
    compact_hslider(ui, "drift", &mut params.analog_filter_drift, 0.0..=1.0)
        .on_hover_text("Slow thermal filter drift (temperature walk)");
    compact_hslider(ui, "vca  ", &mut params.analog_vca_bleed, 0.0..=0.01)
        .on_hover_text("VCA leakage through closed gate (bleed)");
    compact_hslider(ui, "hiss ", &mut params.analog_noise_floor, 0.0..=0.01)
        .on_hover_text("Background noise floor");
}

pub fn draw_arpeggiator(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

    led_button(ui, "ENABLE", &mut params.arp_enabled)
        .on_hover_text("Activate the arpeggiator — held notes play as a sequence");

    labeled(ui, "rate (BPM):", |ui| {
        ui.add(egui::Slider::new(&mut params.arp_rate, 60.0..=240.0).step_by(1.0))
    })
    .on_hover_text("Arpeggiator tempo (steps per minute)");

    let mut arp_pattern = Synthesizer::u8_to_arp_pattern_pub(params.arp_pattern);
    ui.horizontal(|ui| {
        for (pat, lbl, hint) in [
            (ArpPattern::Up,     "Up",  "Ascending"),
            (ArpPattern::Down,   "Dn",  "Descending"),
            (ArpPattern::UpDown, "U-D", "Bounce up then down"),
            (ArpPattern::Random, "Rnd", "Random shuffle"),
        ] {
            let selected = arp_pattern == pat;
            let color = if selected { AMBER } else { DARK_GRAY };
            let text_col = if selected { egui::Color32::BLACK } else { DIM };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(lbl).size(9.0).color(text_col))
                        .fill(color)
                        .corner_radius(2.0)
                        .min_size(egui::vec2(24.0, 16.0)),
                )
                .on_hover_text(hint)
                .clicked()
            {
                arp_pattern = pat;
            }
        }
    });
    params.arp_pattern = Synthesizer::arp_pattern_to_u8_pub(arp_pattern);

    let mut octaves_f32 = params.arp_octaves as f32;
    labeled(ui, "octaves:", |ui| {
        ui.add(egui::Slider::new(&mut octaves_f32, 1.0..=4.0).step_by(1.0))
    })
    .on_hover_text("Octave range — 1 = within the held chord, 4 = up to 4 octaves above");
    params.arp_octaves = octaves_f32 as u8;

    labeled(ui, "gate:", |ui| {
        ui.add(egui::Slider::new(&mut params.arp_gate_length, 0.1..=1.0).step_by(0.01))
    })
    .on_hover_text("Note duration as a fraction of one step (1.0 = legato, 0.1 = staccato)");

    ui.separator();
    led_button(ui, "sync MIDI clk", &mut params.arp_sync_to_midi)
        .on_hover_text("Lock arpeggiator rate to incoming MIDI clock instead of internal BPM");
}

pub fn draw_voice_mode(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

    ui.horizontal(|ui| {
        for (mode, lbl, hint) in [
            (0u8, "Poly",   "Polyphonic — chords"),
            (1u8, "Mono",   "Monophonic — single voice, retriggers"),
            (2u8, "Legato", "Legato — single voice, slides"),
            (3u8, "Unison", "Unison — all voices on one note"),
        ] {
            let selected = params.voice_mode == mode;
            let color = if selected { AMBER } else { DARK_GRAY };
            let text_col = if selected { egui::Color32::BLACK } else { DIM };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(lbl).size(9.0).color(text_col))
                        .fill(color)
                        .corner_radius(2.0)
                        .min_size(egui::vec2(34.0, 16.0)),
                )
                .on_hover_text(hint)
                .clicked()
            {
                params.voice_mode = mode;
            }
        }
    });

    if params.voice_mode == 1 || params.voice_mode == 2 {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("prio:").size(9.5).color(DIM));
            for (prio, lbl, hint) in [
                (0u8, "Last", "Last-played note wins"),
                (1u8, "Low",  "Lowest pitch wins"),
                (2u8, "High", "Highest pitch wins"),
            ] {
                let selected = params.note_priority == prio;
                let color = if selected { AMBER } else { DARK_GRAY };
                let text_col = if selected { egui::Color32::BLACK } else { DIM };
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(lbl).size(9.0).color(text_col))
                            .fill(color)
                            .corner_radius(2.0)
                            .min_size(egui::vec2(28.0, 16.0)),
                    )
                    .on_hover_text(hint)
                    .clicked()
                {
                    params.note_priority = prio;
                }
            }
        });
    }

    if params.voice_mode == 3 {
        labeled(ui, "spread (c):", |ui| {
            ui.add(egui::Slider::new(&mut params.unison_spread, 0.0..=50.0).step_by(0.5))
        })
        .on_hover_text("Detune between unison voices in cents (100 c = 1 semitone)");
    }

    let mut max_v = params.max_voices as f32;
    labeled(ui, "voices:", |ui| {
        ui.add(egui::Slider::new(&mut max_v, 1.0..=8.0).step_by(1.0))
    })
    .on_hover_text("Maximum simultaneous voices (1-8). Lower = older notes get stolen sooner");
    params.max_voices = max_v as u8;
}

/// Poly Mod — sliders horizontales compactos. Diseñado para 120px.
pub fn draw_poly_mod(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
    ui.spacing_mut().slider_width = 60.0;

    ui.label(egui::RichText::new("FiltEnv >").size(9.5).color(DIM));
    compact_hslider(ui, "fqA", &mut params.poly_mod_filter_env_to_osc_a_freq, -1.0..=1.0)
        .on_hover_text("Filter ENV > Osc A pitch");
    compact_hslider(ui, "pwA", &mut params.poly_mod_filter_env_to_osc_a_pw, -1.0..=1.0)
        .on_hover_text("Filter ENV > Osc A pulse width");

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Osc B >").size(9.5).color(DIM));
    compact_hslider(ui, "fqA", &mut params.poly_mod_osc_b_to_osc_a_freq, -1.0..=1.0)
        .on_hover_text("Osc B > Osc A pitch (FM)");
    compact_hslider(ui, "pwA", &mut params.poly_mod_osc_b_to_osc_a_pw, -1.0..=1.0)
        .on_hover_text("Osc B > Osc A pulse width");
    compact_hslider(ui, "flt", &mut params.poly_mod_osc_b_to_filter_cutoff, -1.0..=1.0)
        .on_hover_text("Osc B > Filter cutoff");
}

pub fn draw_keyboard_legend(ui: &mut egui::Ui, octave: i32) {
    let legend_color = egui::Color32::from_gray(70);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(72.0);
            ui.label(
                egui::RichText::new(format!("Oct: {}", octave))
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

        ui.vertical(|ui| {
            ui.set_min_width(175.0);
            ui.label(
                egui::RichText::new(format!("  S   D     G   H   J      oct {}", octave))
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

        ui.vertical(|ui| {
            ui.set_min_width(215.0);
            ui.label(
                egui::RichText::new(format!(
                    "  2   3     5   6   7        oct {}",
                    octave + 1
                ))
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
