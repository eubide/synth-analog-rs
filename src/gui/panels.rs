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

/// Renderiza un grupo con título uniforme.
pub fn section(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.group(|ui| {
        ui.label(egui::RichText::new(title).size(11.0).strong());
        add_contents(ui);
    });
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

/// Panel ADSR compacto sin título. Unifica filter/amp envelope.
pub fn draw_envelope(
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
        labeled(ui, "sync:", |ui| ui.checkbox(&mut params.osc2_sync, "-> A"))
            .on_hover_text(
                "Hard sync osc B to osc A — every osc A cycle resets osc B (classic lead sound)",
            );
    }
}

pub fn draw_mixer(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

    labeled(ui, "osc A:", |ui| {
        ui.add(egui::Slider::new(&mut params.mixer_osc1_level, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Oscillator A level into the filter");
    labeled(ui, "osc B:", |ui| {
        ui.add(egui::Slider::new(&mut params.mixer_osc2_level, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Oscillator B level into the filter");
    labeled(ui, "noise:", |ui| {
        ui.add(egui::Slider::new(&mut params.noise_level, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("White noise generator level (great for percussion or wind effects)");
}

pub fn draw_filter(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

    labeled(ui, "cutoff (Hz):", |ui| {
        ui.add(
            egui::Slider::new(&mut params.filter_cutoff, 20.0..=20000.0)
                .logarithmic(true)
                .step_by(1.0),
        )
    })
    .on_hover_text("Low-pass cutoff frequency — closes the filter to darken the sound");
    labeled(ui, "resonance:", |ui| {
        ui.add(egui::Slider::new(&mut params.filter_resonance, 0.0..=4.0).step_by(0.05))
    })
    .on_hover_text("Filter resonance / Q — emphasises cutoff frequency. >=3.8 self-oscillates");
    if params.filter_resonance >= 3.8 {
        ui.colored_label(egui::Color32::from_rgb(255, 160, 60), "self-osc");
    }
    labeled(ui, "envelope:", |ui| {
        ui.add(egui::Slider::new(&mut params.filter_envelope_amount, -1.0..=1.0).step_by(0.01))
    })
    .on_hover_text("How much the FILTER ENV modulates cutoff (negative inverts the envelope)");
    labeled(ui, "keyboard:", |ui| {
        ui.add(egui::Slider::new(&mut params.filter_keyboard_tracking, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Keyboard tracking — higher notes open the filter more");
    labeled(ui, "velocity:", |ui| {
        ui.add(egui::Slider::new(&mut params.velocity_to_cutoff, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("How much MIDI velocity opens the filter (harder = brighter)");
}

/// LFO timing — waveform, rate, amount, sync, delay. Pareja con `draw_lfo_mod`.
pub fn draw_lfo(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

    let mut lfo_waveform = Synthesizer::u8_to_lfo_waveform_pub(params.lfo_waveform);
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
                ui.selectable_value(&mut lfo_waveform, LfoWaveform::ReverseSawtooth, "Reverse Saw");
                ui.selectable_value(&mut lfo_waveform, LfoWaveform::SampleAndHold, "Sample & Hold");
            })
            .response
    })
    .on_hover_text(
        "LFO waveform — Triangle/Square/Saw for periodic modulation, S&H for random steps",
    );
    params.lfo_waveform = Synthesizer::lfo_waveform_to_u8_pub(lfo_waveform);

    labeled(ui, "rate (Hz):", |ui| {
        ui.add(
            egui::Slider::new(&mut params.lfo_rate, 0.05..=30.0)
                .logarithmic(true)
                .step_by(0.05),
        )
    })
    .on_hover_text("LFO frequency (cycles per second)");
    labeled(ui, "amount:", |ui| {
        ui.add(egui::Slider::new(&mut params.lfo_amount, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Global LFO depth — multiplies all routing amounts in LFO MOD");
    labeled(ui, "delay (s):", |ui| {
        ui.add(egui::Slider::new(&mut params.lfo_delay, 0.0..=5.0).step_by(0.01))
    })
    .on_hover_text("Time after note-on before the LFO fades in (delayed vibrato)");
    ui.checkbox(&mut params.lfo_sync, "key sync (reset on note)")
        .on_hover_text("Reset LFO phase on every note (vs free-running across notes)");
}

/// LFO mod destinations — 5 rutas con toggle de target y amount.
pub fn draw_lfo_mod(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

    labeled_check(ui, Some(&mut params.lfo_target_filter), "cutoff:", |ui| {
        ui.add(egui::Slider::new(&mut params.lfo_to_cutoff, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("LFO modulation depth on filter cutoff (wah-wah). Toggle enables routing");
    labeled_check(ui, None, "res:", |ui| {
        ui.add(egui::Slider::new(&mut params.lfo_to_resonance, 0.0..=1.0).step_by(0.01))
    })
    .on_hover_text("LFO depth on filter resonance (uses the filter target above)");
    labeled_check(
        ui,
        Some(&mut params.lfo_target_osc1_pitch),
        "osc A pitch:",
        |ui| ui.add(egui::Slider::new(&mut params.lfo_to_osc1_pitch, 0.0..=1.0).step_by(0.01)),
    )
    .on_hover_text("LFO depth on osc A pitch (vibrato). Toggle enables routing");
    labeled_check(
        ui,
        Some(&mut params.lfo_target_osc2_pitch),
        "osc B pitch:",
        |ui| ui.add(egui::Slider::new(&mut params.lfo_to_osc2_pitch, 0.0..=1.0).step_by(0.01)),
    )
    .on_hover_text("LFO depth on osc B pitch (vibrato). Toggle enables routing");
    labeled_check(
        ui,
        Some(&mut params.lfo_target_amplitude),
        "amplitude:",
        |ui| ui.add(egui::Slider::new(&mut params.lfo_to_amplitude, 0.0..=1.0).step_by(0.01)),
    )
    .on_hover_text("LFO depth on amplitude (tremolo). Toggle enables routing");
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

/// Analog character panel — las imperfecciones que rompen la limpieza digital.
pub fn draw_analog(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

    labeled(ui, "tolerance:", |ui| {
        ui.add(egui::Slider::new(&mut params.analog_component_tolerance, 0.0..=1.0).step_by(0.01))
            .on_hover_text("Per-voice filter tolerance (±2% cutoff / ±3% Q)")
    });
    labeled(ui, "drift:", |ui| {
        ui.add(egui::Slider::new(&mut params.analog_filter_drift, 0.0..=1.0).step_by(0.01))
            .on_hover_text("Slow filter-temperature drift")
    });
    labeled(ui, "vca bleed:", |ui| {
        ui.add(egui::Slider::new(&mut params.analog_vca_bleed, 0.0..=0.01).step_by(0.0001))
            .on_hover_text("Oscillator leakage through closed VCA")
    });
    labeled(ui, "hiss:", |ui| {
        ui.add(egui::Slider::new(&mut params.analog_noise_floor, 0.0..=0.01).step_by(0.0001))
            .on_hover_text("Constant background noise floor")
    });
}

pub fn draw_arpeggiator(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

    ui.checkbox(&mut params.arp_enabled, "enable")
        .on_hover_text("Activate the arpeggiator — held notes play as a sequence");

    labeled(ui, "rate (BPM):", |ui| {
        ui.add(egui::Slider::new(&mut params.arp_rate, 60.0..=240.0).step_by(1.0))
    })
    .on_hover_text("Arpeggiator tempo (steps per minute)");

    let mut arp_pattern = Synthesizer::u8_to_arp_pattern_pub(params.arp_pattern);
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
    .on_hover_text(
        "Note order: Up = ascending, Down = descending, Up-Down = bounce, Random = shuffle",
    );
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
    ui.checkbox(&mut params.arp_sync_to_midi, "sync to MIDI clock")
        .on_hover_text("Lock arpeggiator rate to incoming MIDI clock instead of internal BPM");
}

pub fn draw_voice_mode(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);

    labeled(ui, "mode:", |ui| {
        egui::ComboBox::from_id_salt("voice_mode")
            .selected_text(match params.voice_mode {
                1 => "Mono",
                2 => "Legato",
                3 => "Unison",
                _ => "Poly",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut params.voice_mode, 0, "Poly");
                ui.selectable_value(&mut params.voice_mode, 1, "Mono");
                ui.selectable_value(&mut params.voice_mode, 2, "Legato");
                ui.selectable_value(&mut params.voice_mode, 3, "Unison");
            })
            .response
    })
    .on_hover_text("Poly: chords / Mono: 1 voice retrigs / Legato: 1 voice slides / Unison: all voices stacked on one note");

    if params.voice_mode == 1 || params.voice_mode == 2 {
        labeled(ui, "priority:", |ui| {
            egui::ComboBox::from_id_salt("note_priority")
                .selected_text(match params.note_priority {
                    1 => "Low",
                    2 => "High",
                    _ => "Last",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut params.note_priority, 0, "Last");
                    ui.selectable_value(&mut params.note_priority, 1, "Low");
                    ui.selectable_value(&mut params.note_priority, 2, "High");
                })
                .response
        })
        .on_hover_text("Which note wins when several are held — Last/Low/High pitch");
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

/// Poly Mod section — rutas de modulación clásicas del Prophet-5.
pub fn draw_poly_mod(ui: &mut egui::Ui, params: &mut SynthParameters) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

    ui.label(
        egui::RichText::new("Filter Env ->")
            .size(10.0)
            .color(egui::Color32::GRAY),
    );
    labeled(ui, "freq A:", |ui| {
        ui.add(
            egui::Slider::new(&mut params.poly_mod_filter_env_to_osc_a_freq, -1.0..=1.0)
                .step_by(0.01),
        )
    })
    .on_hover_text("FILTER ENV modulates osc A pitch (negative = inverted envelope)");
    labeled(ui, "pw A:", |ui| {
        ui.add(
            egui::Slider::new(&mut params.poly_mod_filter_env_to_osc_a_pw, -1.0..=1.0)
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
            egui::Slider::new(&mut params.poly_mod_osc_b_to_osc_a_freq, -1.0..=1.0).step_by(0.01),
        )
    })
    .on_hover_text(
        "Cross-modulation: osc B modulates osc A pitch (audio-rate FM, classic Prophet sound)",
    );
    labeled(ui, "pw A:", |ui| {
        ui.add(egui::Slider::new(&mut params.poly_mod_osc_b_to_osc_a_pw, -1.0..=1.0).step_by(0.01))
    })
    .on_hover_text("Osc B modulates osc A pulse width at audio rate");
    labeled(ui, "cutoff:", |ui| {
        ui.add(
            egui::Slider::new(&mut params.poly_mod_osc_b_to_filter_cutoff, -1.0..=1.0)
                .step_by(0.01),
        )
    })
    .on_hover_text(
        "Osc B modulates filter cutoff at audio rate (creates harmonics / metallic timbres)",
    );
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
