use crate::lock_free::{SCOPE_LEN, ScopeRing};
use eframe::egui;
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

/// Visualiser display modes.
#[derive(Copy, Clone, PartialEq, Eq)]
enum VizMode {
    Scope,
    Spectrum,
}

/// FFT size for the spectrum view. 2048 @ 48 kHz → ~23.4 Hz/bin, enough to
/// resolve the fundamental and first dozens of harmonics of any musical note.
const FFT_LEN: usize = 2048;

/// Display floor for the spectrum in dB — anything below this is drawn at zero.
const SPECTRUM_FLOOR_DB: f32 = -60.0;

/// State owned by the visualiser panel: mode selector + preallocated scratch
/// buffers so the render path does no allocation per frame.
pub struct VisualiserState {
    mode: VizMode,
    samples: Vec<f32>,
    fft_scratch: Vec<Complex<f32>>,
    spectrum: Vec<f32>,
    /// FFT plan cached once at construction — avoids a hash lookup per frame.
    fft: Arc<dyn Fft<f32>>,
}

impl VisualiserState {
    pub fn new() -> Self {
        let mut planner = FftPlanner::new();
        Self {
            mode: VizMode::Scope,
            samples: vec![0.0; SCOPE_LEN],
            fft_scratch: vec![Complex::new(0.0, 0.0); FFT_LEN],
            spectrum: vec![0.0; FFT_LEN / 2],
            fft: planner.plan_fft_forward(FFT_LEN),
        }
    }

    /// Oscilloscope + spectrum analyser panel. Reads the live sample ring from
    /// `ScopeRing` (written by the audio thread) and renders whichever mode the
    /// user picked.
    pub fn draw(&mut self, ui: &mut egui::Ui, scope: &ScopeRing, sample_rate: f32) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Visualizer").size(11.0).strong());
                ui.selectable_value(&mut self.mode, VizMode::Scope, "Scope");
                ui.selectable_value(&mut self.mode, VizMode::Spectrum, "Spectrum");
            });

            scope.snapshot(&mut self.samples);

            let avail_w = ui.available_width();
            let h = ui.available_height().clamp(80.0, 300.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(avail_w, h), egui::Sense::hover());
            if !ui.is_rect_visible(rect) {
                return;
            }
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 3.0, egui::Color32::from_gray(12));

            match self.mode {
                VizMode::Scope => self.draw_scope_trace(&painter, rect),
                VizMode::Spectrum => self.draw_spectrum_bars(&painter, rect, sample_rate),
            }
        });
    }

    /// Oscilloscope: trace the last ~1024 samples with a zero-crossing trigger
    /// so periodic waveforms stand still instead of scrolling.
    fn draw_scope_trace(&self, painter: &egui::Painter, rect: egui::Rect) {
        const DISPLAY: usize = 1024;
        let buf = &self.samples;
        // Trigger on a rising zero crossing in the first half of the buffer so
        // repeating waveforms appear stationary. If none is found we fall back
        // to the plain tail — the visual just scrolls a little.
        let start = buf[..buf.len().saturating_sub(DISPLAY)]
            .windows(2)
            .position(|w| w[0] <= 0.0 && w[1] > 0.0)
            .unwrap_or(buf.len().saturating_sub(DISPLAY));
        let end = (start + DISPLAY).min(buf.len());
        let slice = &buf[start..end];

        // Center line
        let mid_y = rect.center().y;
        painter.line_segment(
            [egui::pos2(rect.min.x, mid_y), egui::pos2(rect.max.x, mid_y)],
            egui::Stroke::new(1.0, egui::Color32::from_gray(40)),
        );

        if slice.is_empty() {
            return;
        }

        let x_step = rect.width() / (slice.len() as f32 - 1.0).max(1.0);
        let half_h = rect.height() * 0.45;
        let color = egui::Color32::from_rgb(80, 220, 120);
        let mut points = Vec::with_capacity(slice.len());
        for (i, s) in slice.iter().enumerate() {
            let x = rect.min.x + i as f32 * x_step;
            let y = mid_y - s.clamp(-1.0, 1.0) * half_h;
            points.push(egui::pos2(x, y));
        }
        painter.add(egui::Shape::line(points, egui::Stroke::new(1.3, color)));
    }

    /// Spectrum analyser: Hann-windowed FFT magnitude, log frequency axis,
    /// amplitudes in dB. Harmonics show as distinct peaks; the filter's effect
    /// on the upper harmonics is easy to read.
    fn draw_spectrum_bars(&mut self, painter: &egui::Painter, rect: egui::Rect, sample_rate: f32) {
        let n = FFT_LEN;
        let src_start = self.samples.len().saturating_sub(n);
        let two_pi_over_n = std::f32::consts::TAU / (n as f32 - 1.0);
        for i in 0..n {
            let s = self.samples[src_start + i];
            let w = 0.5 - 0.5 * (i as f32 * two_pi_over_n).cos();
            self.fft_scratch[i] = Complex::new(s * w, 0.0);
        }
        self.fft.process(&mut self.fft_scratch);

        // The Hann window halves the peak — the 2.0 scale compensates so a
        // 0 dBFS sine reads ~0 dB.
        let scale = 2.0 / n as f32;
        let half = n / 2;
        for i in 0..half {
            let c = self.fft_scratch[i];
            let mag = (c.re * c.re + c.im * c.im).sqrt() * scale;
            self.spectrum[i] = 20.0 * mag.max(1e-5).log10();
        }

        let span_db = -SPECTRUM_FLOOR_DB;
        for db_line in [-12.0, -24.0, -36.0, -48.0] {
            let y = rect.min.y + rect.height() * (-db_line / span_db);
            painter.line_segment(
                [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                egui::Stroke::new(1.0, egui::Color32::from_gray(35)),
            );
        }

        let bin_hz = sample_rate / n as f32;
        let fmin = 20.0_f32.ln();
        let fmax = 20_000.0_f32.ln();
        let color = egui::Color32::from_rgb(255, 180, 60);
        let mut last_x = rect.min.x - 10.0;
        let bar_min_w = 2.0;
        for bin in 1..half {
            let freq = bin as f32 * bin_hz;
            if freq < 20.0 {
                continue;
            }
            if freq > 20_000.0 {
                break;
            }
            let x = rect.min.x + rect.width() * (freq.ln() - fmin) / (fmax - fmin);
            if x - last_x < bar_min_w {
                continue;
            }
            last_x = x;
            let db = self.spectrum[bin].clamp(SPECTRUM_FLOOR_DB, 0.0);
            let h = rect.height() * (db - SPECTRUM_FLOOR_DB) / span_db;
            painter.line_segment(
                [egui::pos2(x, rect.max.y), egui::pos2(x, rect.max.y - h)],
                egui::Stroke::new(1.5, color),
            );
        }
    }
}
