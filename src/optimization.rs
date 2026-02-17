use std::f32::consts::PI;

// Analog synthesizer lookup tables for performance optimization
pub struct OptimizationTables {
    sine_table: [f32; 4096],
    midi_frequencies: [f32; 128],
}

impl OptimizationTables {
    pub fn new() -> Self {
        let mut tables = OptimizationTables {
            sine_table: [0.0; 4096],
            midi_frequencies: [0.0; 128],
        };

        tables.init_sine_table();
        tables.init_midi_frequencies();

        tables
    }

    // Initialize sine table with 4096 entries for smooth oscillators
    fn init_sine_table(&mut self) {
        for i in 0..4096 {
            let phase = (i as f32 / 4096.0) * 2.0 * PI;
            self.sine_table[i] = phase.sin();
        }
    }

    // Pre-calculate MIDI note frequencies (A4 = 440Hz)
    fn init_midi_frequencies(&mut self) {
        for midi_note in 0..128 {
            // MIDI note 69 = A4 = 440Hz
            // Formula: f = 440 * 2^((note - 69) / 12)
            let frequency = 440.0 * 2.0_f32.powf((midi_note as f32 - 69.0) / 12.0);
            self.midi_frequencies[midi_note] = frequency;
        }
    }

    // Optimized sine lookup with cubic interpolation for smoother audio
    pub fn fast_sin(&self, phase: f32) -> f32 {
        // Use multiplication instead of division for better performance
        const INV_TWO_PI: f32 = 1.0 / (2.0 * PI);
        let normalized = (phase * INV_TWO_PI).fract();
        let normalized = if normalized < 0.0 {
            normalized + 1.0
        } else {
            normalized
        };

        let index_f = normalized * 4096.0;
        let index = index_f as usize;
        let frac = index_f - index as f32;

        // Get 4 points for cubic interpolation
        let i0 = (index + 4095) & 4095; // index - 1
        let i1 = index & 4095;
        let i2 = (index + 1) & 4095;
        let i3 = (index + 2) & 4095;

        let y0 = self.sine_table[i0];
        let y1 = self.sine_table[i1];
        let y2 = self.sine_table[i2];
        let y3 = self.sine_table[i3];

        // Cubic interpolation (Catmull-Rom spline)
        let a = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
        let b = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
        let c = -0.5 * y0 + 0.5 * y2;
        let d = y1;

        ((a * frac + b) * frac + c) * frac + d
    }

    // Get pre-calculated MIDI frequency
    pub fn get_midi_frequency(&self, midi_note: u8) -> f32 {
        if midi_note < 128 {
            self.midi_frequencies[midi_note as usize]
        } else {
            440.0 // Fallback to A4
        }
    }
}

// Global optimization tables instance
lazy_static::lazy_static! {
    pub static ref OPTIMIZATION_TABLES: OptimizationTables = OptimizationTables::new();
}
