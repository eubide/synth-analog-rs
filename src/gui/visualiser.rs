use crate::lock_free::{SCOPE_LEN, ScopeRing};
use eframe::egui;
use rustfft::{FftPlanner, num_complex::Complex};

/// Visualiser display modes.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum VizMode {
    Scope,
    Spectrum,
}

/// FFT size for the spectrum view. 2048 @ 44.1 kHz → ~21.5 Hz/bin,
/// enough to resolve the fundamental and first dozens of harmonics of any
/// musically useful note.
pub const FFT_LEN: usize = 2048;

/// State owned by the visualiser panel: mode selector + preallocated scratch
/// buffers so the render path does no allocation per frame.
pub struct VisualiserState {
    mode: VizMode,
    samples: Vec<f32>,
    fft_scratch: Vec<Complex<f32>>,
    spectrum: Vec<f32>,
    planner: FftPlanner<f32>,
}

impl VisualiserState {
    pub fn new() -> Self {
        Self {
            mode: VizMode::Scope,
            samples: vec![0.0; SCOPE_LEN],
            fft_scratch: vec![Complex::new(0.0, 0.0); FFT_LEN],
            spectrum: vec![0.0; FFT_LEN / 2],
            planner: FftPlanner::new(),
        }
    }

    /// Oscilloscope + spectrum analyser panel. Reads the live sample ring from
    /// `ScopeRing` (written by the audio thread) and renders whichever mode the
    /// user picked.
    pub fn draw(&mut self, ui: &mut egui::Ui, scope: &ScopeRing) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Visualizer").size(11.0).strong());
                ui.selectable_value(&mut self.mode, VizMode::Scope, "Scope");
                ui.selectable_value(&mut self.mode, VizMode::Spectrum, "Spectrum");
            });

            // Snapshot latest samples into our local buffer.
            scope.snapshot(&mut self.samples);

            let avail_w = ui.available_width();
            // Leave height flexible so the user can resize the modal.
            let h = ui.available_height().clamp(80.0, 300.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(avail_w, h), egui::Sense::hover());
            if !ui.is_rect_visible(rect) {
                return;
            }
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 3.0, egui::Color32::from_gray(12));

            match self.mode {
                VizMode::Scope => self.draw_scope_trace(&painter, rect),
                VizMode::Spectrum => self.draw_spectrum_bars(&painter, rect),
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

    /// Spectrum analyser: windowed FFT magnitude on a log frequency axis,
    /// amplitudes in dB. Harmonics show up as distinct peaks above the noise
    /// floor; the filter's effect on the upper harmonics is easy to read.
    fn draw_spectrum_bars(&mut self, painter: &egui::Painter, rect: egui::Rect) {
        // Copy the last FFT_LEN samples, apply a Hann window to reduce spectral
        // leakage, and run an in-place FFT.
        let n = FFT_LEN;
        let src_start = self.samples.len().saturating_sub(n);
        let two_pi_over_n = std::f32::consts::TAU / (n as f32 - 1.0);
        for i in 0..n {
            let s = self.samples[src_start + i];
            let w = 0.5 - 0.5 * (i as f32 * two_pi_over_n).cos();
            self.fft_scratch[i] = Complex::new(s * w, 0.0);
        }
        let fft = self.planner.plan_fft_forward(n);
        fft.process(&mut self.fft_scratch);

        // Magnitude → dB, normalised so that a 0 dBFS sine reads ~0 dB.
        // The Hann window halves the peak, so we add +6 dB to compensate.
        let scale = 2.0 / n as f32;
        let half = n / 2;
        for i in 0..half {
            let c = self.fft_scratch[i];
            let mag = (c.re * c.re + c.im * c.im).sqrt() * scale;
            let db = 20.0 * mag.max(1e-5).log10();
            self.spectrum[i] = db;
        }

        // Grid: horizontal lines every 12 dB from 0 to -60 dB.
        for db_line in [-12.0, -24.0, -36.0, -48.0] {
            let y = rect.min.y + rect.height() * (-db_line / 60.0);
            painter.line_segment(
                [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                egui::Stroke::new(1.0, egui::Color32::from_gray(35)),
            );
        }

        // Log-frequency axis: map 20 Hz..20 kHz across the rect width.
        let sample_rate = 44_100.0_f32;
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
            let db = self.spectrum[bin].clamp(-60.0, 0.0);
            let h = rect.height() * (db + 60.0) / 60.0;
            painter.line_segment(
                [egui::pos2(x, rect.max.y), egui::pos2(x, rect.max.y - h)],
                egui::Stroke::new(1.5, color),
            );
        }
    }
}
