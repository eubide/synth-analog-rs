use std::f32::consts::PI;

// Analog synthesizer lookup tables for performance optimization
#[allow(dead_code)]
pub struct OptimizationTables {
    sine_table: [f32; 4096],
    exp_table: [f32; 256],
    midi_frequencies: [f32; 128],
    voice_scale_table: [f32; 17], // For voice counts 0-16
}

impl OptimizationTables {
    pub fn new() -> Self {
        let mut tables = OptimizationTables {
            sine_table: [0.0; 4096],
            exp_table: [0.0; 256],
            midi_frequencies: [0.0; 128],
            voice_scale_table: [0.0; 17],
        };

        tables.init_sine_table();
        tables.init_exp_table();
        tables.init_midi_frequencies();
        tables.init_voice_scale_table();

        tables
    }

    // Initialize sine table with 4096 entries for smooth oscillators
    fn init_sine_table(&mut self) {
        for i in 0..4096 {
            let phase = (i as f32 / 4096.0) * 2.0 * PI;
            self.sine_table[i] = phase.sin();
        }
    }

    // Initialize exponential table for envelope scaling
    fn init_exp_table(&mut self) {
        for i in 0..256 {
            // Exponential curve for envelopes: 0 to 1 range with exponential mapping
            let normalized = i as f32 / 255.0;
            // Exponential curve: e^(ln(0.001) * (1 - x)) gives range from 0.001 to 1.0
            self.exp_table[i] = if normalized == 0.0 {
                0.0
            } else {
                (-6.907755 * (1.0 - normalized)).exp() // ln(0.001) ≈ -6.907755
            };
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

    // Pre-calculate voice scaling factors for polyphony
    fn init_voice_scale_table(&mut self) {
        self.voice_scale_table[0] = 1.0; // 0 voices = 1.0 (no scaling)
        for i in 1..=16 {
            // Scale down to prevent clipping when multiple voices play
            let voice_count_f = i as f32;
            // Use sqrt scaling for natural loudness perception
            self.voice_scale_table[i] = (1.0 / voice_count_f.sqrt()).min(1.0) * 0.8;
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

    // Fast exponential lookup for envelope values
    pub fn fast_exp(&self, value: f32) -> f32 {
        let clamped = value.clamp(0.0, 1.0);
        let index = (clamped * 255.0) as usize;
        self.exp_table[index.min(255)]
    }

    // Get pre-calculated MIDI frequency
    pub fn get_midi_frequency(&self, midi_note: u8) -> f32 {
        if midi_note < 128 {
            self.midi_frequencies[midi_note as usize]
        } else {
            440.0 // Fallback to A4
        }
    }

    // Get pre-calculated voice scaling factor for polyphony
    pub fn get_voice_scale(&self, voice_count: usize) -> f32 {
        if voice_count <= 16 {
            self.voice_scale_table[voice_count]
        } else {
            // Fallback for > 16 voices
            (1.0 / (voice_count as f32).sqrt()).min(1.0) * 0.8
        }
    }

    // Convert envelope time (0.0-1.0) to time multiplier for sample-based processing
    pub fn envelope_time_to_multiplier(&self, time: f32, sample_rate: f32) -> f32 {
        if time == 0.0 {
            0.0001 // Very fast
        } else {
            // Map 0.0-1.0 to 0.001s-10s range
            let time_seconds = 0.001 + self.fast_exp(time) * 9.999;
            1.0 / (time_seconds * sample_rate)
        }
    }
}

// Global optimization tables instance
lazy_static::lazy_static! {
    #[allow(dead_code)]
    pub static ref OPTIMIZATION_TABLES: OptimizationTables = OptimizationTables::new();
}
