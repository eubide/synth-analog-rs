use crate::lock_free::{SCOPE_LEN, ScopeRing};
use eframe::egui;
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

#[derive(Copy, Clone, PartialEq, Eq)]
enum VizMode {
    Scope,
    Spectrum,
}

const FFT_LEN: usize = 2048;
const SPECTRUM_FLOOR_DB: f32 = -60.0;
const LABEL_H: f32 = 14.0;

const FREQ_MARKERS: &[(f32, &str, bool)] = &[
    (50.0,    "50",  false),
    (100.0,   "100", true),
    (200.0,   "200", false),
    (500.0,   "500", false),
    (1000.0,  "1k",  true),
    (2000.0,  "2k",  false),
    (5000.0,  "5k",  false),
    (10000.0, "10k", true),
    (20000.0, "20k", false),
];

pub struct VisualiserState {
    mode: VizMode,
    samples: Vec<f32>,
    fft_scratch: Vec<Complex<f32>>,
    spectrum_smooth: Vec<f32>,
    hann_window: Vec<f32>,
    fft: Arc<dyn Fft<f32>>,
    scope_peak: f32,
    scope_display_samples: usize,
    /// Reusable point buffer for the spectrum curve — avoids a per-frame alloc.
    curve: Vec<egui::Pos2>,
}

impl VisualiserState {
    pub fn new() -> Self {
        let mut planner = FftPlanner::new();
        let hann_window = (0..FFT_LEN)
            .map(|i| {
                let t = i as f32 * std::f32::consts::TAU / (FFT_LEN as f32 - 1.0);
                0.5 - 0.5 * t.cos()
            })
            .collect();
        Self {
            mode: VizMode::Scope,
            samples: vec![0.0; SCOPE_LEN],
            fft_scratch: vec![Complex::new(0.0, 0.0); FFT_LEN],
            spectrum_smooth: vec![SPECTRUM_FLOOR_DB; FFT_LEN / 2],
            hann_window,
            fft: planner.plan_fft_forward(FFT_LEN),
            scope_peak: 0.01,
            scope_display_samples: 1024,
            curve: Vec::with_capacity(FFT_LEN / 2),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, scope: &ScopeRing, sample_rate: f32) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Visualizer").size(11.0).strong());
                ui.selectable_value(&mut self.mode, VizMode::Scope, "Oscilloscope");
                ui.selectable_value(&mut self.mode, VizMode::Spectrum, "Spectrum");

                if self.mode == VizMode::Scope {
                    ui.separator();
                    ui.add_sized(
                        [80.0, 16.0],
                        egui::Slider::new(&mut self.scope_display_samples, 32..=SCOPE_LEN)
                            .logarithmic(true)
                            .show_value(false),
                    )
                    .on_hover_text("Time window — drag left to zoom in on high frequencies, right to show more cycles");
                    let ms = self.scope_display_samples as f32 / sample_rate * 1000.0;
                    ui.label(egui::RichText::new(format!("{:.1}ms", ms)).size(10.0).weak());
                }
            });

            scope.snapshot(&mut self.samples);

            let avail_w = ui.available_width();
            let h = ui.available_height().clamp(80.0, 500.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(avail_w, h), egui::Sense::hover());
            if !ui.is_rect_visible(rect) {
                return;
            }
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 3.0, egui::Color32::from_gray(12));

            match self.mode {
                VizMode::Scope => self.draw_scope_trace(&painter, rect),
                VizMode::Spectrum => self.draw_spectrum(&painter, rect, sample_rate),
            }
        });
    }

    fn draw_scope_trace(&mut self, painter: &egui::Painter, rect: egui::Rect) {
        let display = self.scope_display_samples;
        let buf = &self.samples;

        let search_end = buf.len().saturating_sub(display);
        let threshold = self.scope_peak * 0.15;
        let mut armed = false;
        let mut trigger = None;
        for i in 1..search_end {
            if buf[i - 1] < -threshold {
                armed = true;
            }
            if armed && buf[i - 1] <= threshold && buf[i] > threshold {
                trigger = Some(i);
                break;
            }
        }
        let start = trigger.unwrap_or(search_end);
        let end = (start + display).min(buf.len());
        let slice = &buf[start..end];

        let mid_y = rect.center().y;
        painter.line_segment(
            [egui::pos2(rect.min.x, mid_y), egui::pos2(rect.max.x, mid_y)],
            egui::Stroke::new(1.0, egui::Color32::from_gray(40)),
        );

        if slice.is_empty() {
            return;
        }

        let frame_peak = slice.iter().fold(0.0_f32, |a, &s| a.max(s.abs())).max(0.01);
        let alpha = if frame_peak > self.scope_peak { 0.3 } else { 0.02 };
        self.scope_peak += alpha * (frame_peak - self.scope_peak);
        let half_h = rect.height() * 0.45;

        let x_step = rect.width() / (slice.len() as f32 - 1.0).max(1.0);
        let mut points = Vec::with_capacity(slice.len());
        for (i, s) in slice.iter().enumerate() {
            let x = rect.min.x + i as f32 * x_step;
            let y = mid_y - (s / self.scope_peak) * half_h;
            points.push(egui::pos2(x, y));
        }
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(1.3, egui::Color32::from_rgb(80, 220, 120)),
        ));
    }

    fn draw_spectrum(&mut self, painter: &egui::Painter, rect: egui::Rect, sample_rate: f32) {
        let n = FFT_LEN;
        let src_start = self.samples.len().saturating_sub(n);
        for i in 0..n {
            let s = self.samples[src_start + i];
            self.fft_scratch[i] = Complex::new(s * self.hann_window[i], 0.0);
        }
        self.fft.process(&mut self.fft_scratch);

        let scale = 2.0 / n as f32;
        let half = n / 2;
        for i in 0..half {
            let c = self.fft_scratch[i];
            let mag = (c.re * c.re + c.im * c.im).sqrt() * scale;
            let db = 20.0 * mag.max(1e-5).log10();
            let alpha = if db > self.spectrum_smooth[i] { 0.25 } else { 0.08 };
            self.spectrum_smooth[i] += alpha * (db - self.spectrum_smooth[i]);
        }

        let span_db = -SPECTRUM_FLOOR_DB;
        let fmin = 20.0_f32.ln();
        let fmax = 20_000.0_f32.ln();
        let freq_to_x = |freq: f32| rect.min.x + rect.width() * (freq.ln() - fmin) / (fmax - fmin);

        for db_line in [-12.0_f32, -24.0, -36.0, -48.0] {
            let y = rect.min.y + rect.height() * (-db_line / span_db);
            painter.line_segment(
                [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                egui::Stroke::new(1.0, egui::Color32::from_gray(28)),
            );
        }

        for &(freq, label, major) in FREQ_MARKERS {
            let x = freq_to_x(freq);
            if x < rect.min.x || x > rect.max.x {
                continue;
            }
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y - LABEL_H)],
                egui::Stroke::new(1.0, egui::Color32::from_gray(if major { 42 } else { 28 })),
            );
            painter.text(
                egui::pos2(x, rect.max.y - 2.0),
                egui::Align2::CENTER_BOTTOM,
                label,
                egui::FontId::proportional(9.0),
                if major {
                    egui::Color32::from_rgba_premultiplied(190, 145, 55, 180)
                } else {
                    egui::Color32::from_rgba_premultiplied(130, 95, 35, 130)
                },
            );
        }

        let bin_hz = sample_rate / n as f32;
        let baseline_y = rect.max.y - LABEL_H;
        // Pre-compute scale factor so bars don't overdraw the label strip.
        let bar_scale = (rect.height() - LABEL_H).max(0.0) / rect.height();

        self.curve.clear();
        for bin in 1..half {
            let freq = bin as f32 * bin_hz;
            if freq < 20.0 {
                continue;
            }
            if freq > 20_000.0 {
                break;
            }
            let x = freq_to_x(freq);
            if let Some(last) = self.curve.last() {
                if x - last.x < 0.5 {
                    continue;
                }
            }
            let db = self.spectrum_smooth[bin].clamp(SPECTRUM_FLOOR_DB, 0.0);
            let h = rect.height() * (db - SPECTRUM_FLOOR_DB) / span_db * bar_scale;
            self.curve.push(egui::pos2(x, baseline_y - h));
        }

        let bar_color = egui::Color32::from_rgb(100, 180, 255);
        for p in &self.curve {
            painter.line_segment(
                [egui::pos2(p.x, baseline_y), *p],
                egui::Stroke::new(1.5, bar_color),
            );
        }
    }
}
