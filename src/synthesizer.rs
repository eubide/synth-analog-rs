use std::f32::consts::PI;
use std::fs;
use std::path::Path;

use crate::optimization::OPTIMIZATION_TABLES;

// Phase accumulator constants to prevent drift
const PHASE_SCALE: u64 = 1u64 << 32; // 32-bit fractional phase
const PHASE_MASK: u64 = PHASE_SCALE - 1;

// Master DC blocker coefficient: first-order HPF at ~0.7 Hz (44.1 kHz), inaudible but
// removes DC offset introduced by filter self-oscillation or asymmetric saturation.
const MASTER_DC_COEFF: f32 = 0.9999;

// Per-voice VCO drift: each oscillator drifts ±2.5 cents at a slow sub-audio rate.
// Linear approximation 2^(c/1200) ≈ 1 + c·ln2/1200 — error <0.001 % for |c| < 10 cents.
const DRIFT_FREQ_FACTOR: f32 = 2.5 * 0.000578;

// Analog character windows — picked once per voice, scaled by the user-facing knobs.
const TOLERANCE_CUTOFF_RANGE: f32 = 0.04; // ±2 % cutoff per voice
const TOLERANCE_RES_RANGE: f32 = 0.06; // ±3 % resonance per voice
const FILTER_DRIFT_RANGE: f32 = 0.06; // ±3 % cutoff walk target
const FILTER_DRIFT_ALPHA: f32 = 2.0; // IIR α → τ ≈ 0.5 s
const FILTER_DRIFT_MIN_PERIOD: f32 = 1.0; // seconds between re-picks
const FILTER_DRIFT_MAX_PERIOD: f32 = 3.0;

// Chorus geometry — 10 ms centre ± (5 ms × depth) keeps the modulated delay inside
// the ensemble range (5–25 ms) at full depth, avoiding slap-back territory.
const CHORUS_CENTRE_MS: f32 = 10.0;
const CHORUS_MOD_MS: f32 = 5.0;
const CHORUS_VOICE2_RATE_RATIO: f32 = 1.13; // second LFO slightly detuned

// Denormal floor — floats below this magnitude are flushed to 0 to avoid the
// ~100× slowdown of subnormal arithmetic in feedback paths.
const DENORMAL_FLOOR: f32 = 1.0e-20;

// Tuning ratios relative to C within an octave (index 0=C, 1=C#, ..., 11=B).
// All modes are anchored so that MIDI 69 (A4) = 440 Hz exactly.

// 5-limit Just Intonation
const TUNING_JI: [f64; 12] = [
    1.0,         // C  — 1/1
    25.0 / 24.0, // C# — 25/24
    9.0 / 8.0,   // D  — 9/8
    6.0 / 5.0,   // Eb — 6/5
    5.0 / 4.0,   // E  — 5/4
    4.0 / 3.0,   // F  — 4/3
    45.0 / 32.0, // F# — 45/32
    3.0 / 2.0,   // G  — 3/2
    8.0 / 5.0,   // Ab — 8/5
    5.0 / 3.0,   // A  — 5/3
    9.0 / 5.0,   // Bb — 9/5
    15.0 / 8.0,  // B  — 15/8
];

// Pythagorean tuning (stacked pure fifths from C)
const TUNING_PYTHAGOREAN: [f64; 12] = [
    1.0,
    2187.0 / 2048.0,
    9.0 / 8.0,
    32.0 / 27.0,
    81.0 / 64.0,
    4.0 / 3.0,
    729.0 / 512.0,
    3.0 / 2.0,
    128.0 / 81.0,
    27.0 / 16.0,
    16.0 / 9.0,
    243.0 / 128.0,
];

// Werckmeister III (1691) — historical well-temperament
const TUNING_WERCKMEISTER3: [f64; 12] = [
    1.0,            // C
    256.0 / 243.0,  // C# — 256/243
    64.0 / 54.0,    // D  — 64/54 (≈ 9/8 adjusted)
    32.0 / 27.0,    // Eb — 32/27
    81.0 / 64.0,    // E  — 81/64
    4.0 / 3.0,      // F  — 4/3
    1024.0 / 729.0, // F# — 1024/729
    3.0 / 2.0,      // G  — 3/2
    128.0 / 81.0,   // Ab — 128/81
    27.0 / 16.0,    // A  — 27/16 (NOT 5/3 — Werckmeister A is a Pythagorean sixth)
    16.0 / 9.0,     // Bb — 16/9
    243.0 / 128.0,  // B  — 243/128
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaveType {
    Sine,
    Square,
    Triangle,
    Sawtooth,
}

#[derive(Debug, Clone)]
pub struct OscillatorParams {
    pub wave_type: WaveType,
    pub amplitude: f32,
    pub detune: f32,
    pub pulse_width: f32,
}

#[derive(Debug, Clone)]
pub struct FilterParams {
    pub cutoff: f32,
    pub resonance: f32,
    pub envelope_amount: f32,
    pub keyboard_tracking: f32,
}

#[derive(Debug, Clone)]
pub struct EnvelopeParams {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LfoWaveform {
    Triangle,
    Square,
    Sawtooth,
    ReverseSawtooth,
    SampleAndHold,
}

#[derive(Debug, Clone)]
pub struct LfoParams {
    pub frequency: f32,
    pub amplitude: f32,
    pub waveform: LfoWaveform,
    pub sync: bool, // Keyboard sync - resets LFO phase on note trigger
    pub target_osc1_pitch: bool,
    pub target_osc2_pitch: bool,
    pub target_filter: bool,
    pub target_amplitude: bool,
}

#[derive(Debug, Clone)]
pub struct MixerParams {
    pub osc1_level: f32,
    pub osc2_level: f32,
    pub noise_level: f32,
}

#[derive(Debug, Clone)]
pub struct ModulationMatrix {
    pub lfo_to_cutoff: f32,
    pub lfo_to_resonance: f32,
    pub lfo_to_osc1_pitch: f32,
    pub lfo_to_osc2_pitch: f32,
    pub lfo_to_amplitude: f32,
    pub velocity_to_cutoff: f32,
    pub velocity_to_amplitude: f32,
}

#[derive(Debug, Clone)]
pub struct EffectsParams {
    pub reverb_amount: f32,
    pub reverb_size: f32,
    pub delay_time: f32,
    pub delay_feedback: f32,
    pub delay_amount: f32,
    // Chorus / Ensemble — classic Prophet-5 studio shimmer.
    // Two LFOs in quadrature modulate a short delay line (center ≈10 ms).
    pub chorus_mix: f32,   // 0.0 = dry, 1.0 = wet
    pub chorus_rate: f32,  // Hz (0.1..=3.0)
    pub chorus_depth: f32, // 0.0..=1.0 (maps to ±0..5 ms around the 10 ms centre)
}

/// Analog character parameters — subtle circuit imperfections that bring the
/// instrument to life: component tolerances per voice, VCA bleed-through,
/// filter temperature drift, and a faint circuit noise floor.
///
/// These are global to the instrument (not preset-saved), because they model
/// the physical properties of the hardware, not the patch.
#[derive(Debug, Clone)]
pub struct AnalogCharacter {
    /// Scale of per-voice filter cutoff/resonance tolerance [0..1] — 1.0 = full ±2 %/±3 %.
    pub component_tolerance: f32,
    /// Scale of slow per-voice filter-temperature drift [0..1] — 1.0 = ±3 % drift.
    pub filter_drift_amount: f32,
    /// Leakage of oscillator signal around a closed VCA [0..0.01] — ≈ -54 dB at 0.002.
    pub vca_bleed: f32,
    /// Constant background hiss added to the master bus [0..0.01].
    pub noise_floor: f32,
}

#[derive(Debug, Clone)]
pub struct ArpeggiatorParams {
    pub enabled: bool,
    pub rate: f32,
    pub pattern: ArpPattern,
    pub octaves: u8,
    pub gate_length: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArpPattern {
    Up,
    Down,
    UpDown,
    Random,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoiceMode {
    Poly,
    Mono,
    Legato,
    Unison,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NotePriority {
    Last,
    Low,
    High,
}

#[derive(Debug, Clone)]
pub struct Preset {
    pub name: String,
    pub category: String,
    pub osc1: OscillatorParams,
    pub osc2: OscillatorParams,
    pub osc2_sync: bool,
    pub mixer: MixerParams,
    pub filter: FilterParams,
    pub filter_envelope: EnvelopeParams,
    pub amp_envelope: EnvelopeParams,
    pub lfo: LfoParams,
    pub modulation_matrix: ModulationMatrix,
    pub effects: EffectsParams,
    pub master_volume: f32,
    // Poly Mod — iconic Prophet-5 cross-modulation (filter envelope / osc B into osc A / filter).
    pub poly_mod_filter_env_to_osc_a_freq: f32,
    pub poly_mod_filter_env_to_osc_a_pw: f32,
    pub poly_mod_osc_b_to_osc_a_freq: f32,
    pub poly_mod_osc_b_to_osc_a_pw: f32,
    pub poly_mod_osc_b_to_filter_cutoff: f32,
}

pub struct Voice {
    pub frequency: f32,
    pub note: u8,
    pub velocity: f32,
    pub phase1_accumulator: u64, // Integer phase accumulator to prevent drift
    pub phase2_accumulator: u64,
    pub envelope_state: EnvelopeState,
    pub envelope_time: f32,
    pub envelope_value: f32,
    pub filter_envelope_state: EnvelopeState,
    pub filter_envelope_value: f32,
    pub filter_state: LadderFilterState,
    pub is_active: bool,
    pub is_sustained: bool, // nota retenida por sustain pedal
    pub sustain_time: f32,
    pub glide_current_freq: f32,
    // Analog character: per-voice VCO drift
    pub drift_phase: f32, // [0, 1) — current phase of the slow drift LFO
    pub drift_rate: f32,  // Hz — randomized at birth so voices don't phase-lock
    // Analog character: per-voice pink noise generator (xorshift32 + Paul Kellett IIR)
    pub noise_prng: u32,
    pub noise_b0: f32,
    pub noise_b1: f32,
    pub noise_b2: f32,
    // Poly Mod: last Osc B output sample used to cross-modulate Osc A (1-sample delay)
    pub osc2_last_out: f32,
    // LFO delay: tiempo transcurrido desde el note-on (para fade-in del LFO)
    pub lfo_delay_elapsed: f32,
    // Per-voice component tolerance — constant deviations picked at birth that model
    // the analog reality of non-identical resistors/capacitors between voice boards.
    // Centered at 1.0; scaled at use-time by the global component_tolerance setting.
    pub tolerance_cutoff_mul: f32, // ±2 % variation (0.98..=1.02)
    pub tolerance_res_mul: f32,    // ±3 % variation (0.97..=1.03)
    // Filter temperature drift — a slow random walk on the cutoff that wanders a
    // few percent over seconds, simulating thermal changes in the filter caps.
    pub filter_drift_value: f32,  // current smoothed multiplier (~1.0)
    pub filter_drift_target: f32, // target we're interpolating toward
    pub filter_drift_timer: f32,  // seconds until a new target is picked
    // Stereo panning: assigned at note trigger, equal-power L/R gains precomputed.
    pub pan: f32,
    pub pan_left: f32,
    pub pan_right: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeState {
    Attack,
    Decay,
    Sustain,
    Release,
    Idle,
}

#[derive(Debug, Clone)]
pub struct LadderFilterState {
    // TPT integrator states for the 4 cascaded one-pole sections (24 dB/octave)
    pub stage1: f32,
    pub stage2: f32,
    pub stage3: f32,
    pub stage4: f32,
}

/// Master effects bus: chorus → delay → reverb. Owns the DSP runtime state
/// (delay lines, comb/allpass buffers, modulation phases) but reads parameters
/// from `EffectsParams` passed in by the caller, so preset save/load can keep
/// treating params as plain data.
pub struct EffectsChain {
    // Delay state
    pub delay_buffer: Vec<f32>,
    pub delay_index: usize,
    // Freeverb: 8 parallel comb filters + 4 series allpass filters
    pub reverb_comb_buffers: Vec<Vec<f32>>,
    pub reverb_comb_indices: Vec<usize>,
    pub reverb_comb_filters: Vec<f32>, // LP filter state inside each comb
    pub reverb_allpass_buffers: Vec<Vec<f32>>,
    pub reverb_allpass_indices: Vec<usize>,
    // Chorus delay line — sized for 25 ms max delay (≈ centre 10 ms ± 5 ms modulation).
    pub chorus_buffer: Vec<f32>,
    pub chorus_index: usize,
    // Two LFO phases in quadrature give a thicker, Juno-style ensemble sound.
    pub chorus_lfo_phase1: f32,
    pub chorus_lfo_phase2: f32,
}

impl EffectsChain {
    pub fn new(sample_rate: f32) -> Self {
        let max_delay_samples = (sample_rate * 2.0) as usize; // 2 second max delay

        // Freeverb delay line lengths (Jezar's original tuning at 44.1 kHz).
        // Comb delays are prime-ish and spread across 25–37 ms to avoid flutter echo.
        // Allpass delays provide diffusion without coloring the frequency response.
        let rate = sample_rate / 44100.0;
        let comb_sizes: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617]
            .map(|n| ((n as f32 * rate) as usize).max(1));
        let allpass_sizes: [usize; 4] =
            [556, 441, 341, 225].map(|n| ((n as f32 * rate) as usize).max(1));

        Self {
            delay_buffer: vec![0.0; max_delay_samples],
            delay_index: 0,
            reverb_comb_buffers: comb_sizes.iter().map(|&n| vec![0.0f32; n]).collect(),
            reverb_comb_indices: vec![0; 8],
            reverb_comb_filters: vec![0.0; 8],
            reverb_allpass_buffers: allpass_sizes.iter().map(|&n| vec![0.0f32; n]).collect(),
            reverb_allpass_indices: vec![0; 4],
            // Power-of-two size lets apply_chorus replace `% len` with a bitmask.
            chorus_buffer: vec![
                0.0;
                ((sample_rate * 0.025) as usize)
                    .max(1024)
                    .next_power_of_two()
            ],
            chorus_index: 0,
            chorus_lfo_phase1: 0.0,
            chorus_lfo_phase2: 0.25, // start 90° out so the two voices don't phase-lock
        }
    }

    pub fn apply_delay(&mut self, sample: f32, params: &EffectsParams, sample_rate: f32) -> f32 {
        if params.delay_amount <= 0.0 {
            return sample;
        }

        let delay_samples = (params.delay_time * sample_rate) as usize;
        let delay_samples = delay_samples.min(self.delay_buffer.len() - 1);

        let delay_read_index = if self.delay_index >= delay_samples {
            self.delay_index - delay_samples
        } else {
            self.delay_buffer.len() + self.delay_index - delay_samples
        };

        let delayed_sample = self.delay_buffer[delay_read_index];
        // Flush denormals in the feedback tail, otherwise decaying echoes
        // slow the audio thread and cause xruns.
        let delayed_sample = if delayed_sample.abs() < 1.0e-20 {
            0.0
        } else {
            delayed_sample
        };
        let feedback_sample = delayed_sample * params.delay_feedback;

        self.delay_buffer[self.delay_index] = sample + feedback_sample;
        self.delay_index = (self.delay_index + 1) % self.delay_buffer.len();

        sample + (delayed_sample * params.delay_amount)
    }

    /// Mono chorus/ensemble inspired by Juno-6 and Prophet-10: two delay taps
    /// modulated by quadrature LFOs around a 10 ms centre. Short enough to stay
    /// out of slap-back delay territory, long enough to create stereo-like width
    /// from pitch drift between taps.
    pub fn apply_chorus(&mut self, sample: f32, params: &EffectsParams, sample_rate: f32) -> f32 {
        let len = self.chorus_buffer.len();
        // chorus_buffer is constructed as a power of two (see EffectsChain::new),
        // so wrap arithmetic collapses to a bitmask. Keep this invariant.
        let mask = len - 1;
        let mix = params.chorus_mix;
        if mix <= 0.0 {
            // Keep feeding the ring even when bypassed — turning chorus back on
            // should not produce a stale transient from silent history.
            self.chorus_buffer[self.chorus_index] = sample;
            self.chorus_index = (self.chorus_index + 1) & mask;
            return sample;
        }

        let rate = params.chorus_rate.clamp(0.05, 5.0);
        let depth = params.chorus_depth.clamp(0.0, 1.0);

        // Triangle is cheaper than sine and audibly identical at these depths.
        let phase_inc = rate / sample_rate;
        self.chorus_lfo_phase1 = (self.chorus_lfo_phase1 + phase_inc).fract();
        self.chorus_lfo_phase2 =
            (self.chorus_lfo_phase2 + phase_inc * CHORUS_VOICE2_RATE_RATIO).fract();
        let tri = |p: f32| {
            if p < 0.5 {
                4.0 * p - 1.0
            } else {
                3.0 - 4.0 * p
            }
        };
        let lfo1 = tri(self.chorus_lfo_phase1);
        let lfo2 = tri(self.chorus_lfo_phase2);

        let mod_ms = CHORUS_MOD_MS * depth;
        let delay_samples_1 = ((CHORUS_CENTRE_MS + mod_ms * lfo1) * 0.001 * sample_rate).max(2.0);
        let delay_samples_2 = ((CHORUS_CENTRE_MS + mod_ms * lfo2) * 0.001 * sample_rate).max(2.0);

        // Fractional-delay read with linear interpolation — the LFO motion changes
        // the read position every sample so interpolation is required to avoid zipper.
        let read_tap = |buf: &[f32], idx: usize, delay: f32| -> f32 {
            let d_int = delay as usize;
            let d_frac = delay - d_int as f32;
            let base = (idx + buf.len() - d_int) & mask;
            let prev = base.wrapping_sub(1) & mask;
            buf[base] * (1.0 - d_frac) + buf[prev] * d_frac
        };
        let tap1 = read_tap(&self.chorus_buffer, self.chorus_index, delay_samples_1);
        let tap2 = read_tap(&self.chorus_buffer, self.chorus_index, delay_samples_2);
        let mut wet = (tap1 + tap2) * 0.5;
        if wet.abs() < DENORMAL_FLOOR {
            wet = 0.0;
        }

        self.chorus_buffer[self.chorus_index] = sample;
        self.chorus_index = (self.chorus_index + 1) & mask;

        // Slight dry bias so chords don't thin out above mix ≈ 0.7.
        let dry_gain = 1.0 - mix * 0.5;
        sample * dry_gain + wet * mix
    }

    pub fn apply_reverb(&mut self, sample: f32, params: &EffectsParams) -> f32 {
        if params.reverb_amount <= 0.0 {
            return sample;
        }

        // Freeverb-style reverb (Jezar at Dreampoint, 1997).
        //
        // reverb_size maps to Freeverb's "roomsize" (feedback gain g).
        // A fixed damping of 0.5 gives a warm, analog-sounding decay without
        // the metallic flutter of undamped combs.
        let g = (0.56 + params.reverb_size * 0.42).min(0.985); // room feedback
        const DAMP: f32 = 0.5;
        const DAMP_INV: f32 = 1.0 - DAMP;
        const AP_G: f32 = 0.5; // allpass diffusion gain
        const DENORMAL: f32 = 1.0e-20;

        // 8 parallel comb filters with LP damping inside the feedback loop.
        // The LP filter inside each comb is what makes Freeverb sound warm instead
        // of metallic: high frequencies decay faster than low frequencies.
        let mut comb_sum = 0.0f32;
        for i in 0..8 {
            let idx = self.reverb_comb_indices[i];
            let out = self.reverb_comb_buffers[i][idx];
            let out = if out.abs() < DENORMAL { 0.0 } else { out };
            // One-pole LP inside the comb feedback
            self.reverb_comb_filters[i] = out * DAMP_INV + self.reverb_comb_filters[i] * DAMP;
            let filtered = if self.reverb_comb_filters[i].abs() < DENORMAL {
                0.0
            } else {
                self.reverb_comb_filters[i]
            };
            self.reverb_comb_buffers[i][idx] = sample + filtered * g;
            let len = self.reverb_comb_buffers[i].len();
            self.reverb_comb_indices[i] = (idx + 1) % len;
            comb_sum += out;
        }
        let mut reverb = comb_sum * 0.125; // average of 8 combs

        // 4 series allpass filters for diffusion.
        // Allpass filters scatter energy in time without coloring the spectrum,
        // turning the comb-filter echoes into a smooth density tail.
        for i in 0..4 {
            let idx = self.reverb_allpass_indices[i];
            let buf_out = self.reverb_allpass_buffers[i][idx];
            let buf_out = if buf_out.abs() < DENORMAL {
                0.0
            } else {
                buf_out
            };
            self.reverb_allpass_buffers[i][idx] = reverb + buf_out * AP_G;
            let len = self.reverb_allpass_buffers[i].len();
            self.reverb_allpass_indices[i] = (idx + 1) % len;
            reverb = buf_out - reverb;
        }

        sample + reverb * params.reverb_amount
    }
}

pub struct Synthesizer {
    pub osc1: OscillatorParams,
    pub osc2: OscillatorParams,
    pub osc2_sync: bool,
    pub mixer: MixerParams,
    pub filter: FilterParams,
    pub filter_envelope: EnvelopeParams,
    pub amp_envelope: EnvelopeParams,
    pub lfo: LfoParams,
    pub modulation_matrix: ModulationMatrix,
    pub effects: EffectsParams,
    pub arpeggiator: ArpeggiatorParams,
    pub master_volume: f32,
    pub voices: Vec<Voice>,
    pub sample_rate: f32,
    pub lfo_phase_accumulator: u64, // Integer phase accumulator to prevent drift
    pub lfo_sample_hold_value: f32, // Current sample & hold value
    pub lfo_last_sample_time: f32,  // Time since last sample update
    pub max_polyphony: usize,
    pub effects_chain: EffectsChain,
    pub held_notes: Vec<u8>,
    pub sustain_held: bool, // sustain pedal activo
    pub voice_mode: VoiceMode,
    pub note_priority: NotePriority,
    pub unison_spread: f32,        // cents spread en unison mode
    pub note_stack: Vec<(u8, u8)>, // (note, velocity) para mono/legato/unison
    pub arp_step: usize,
    pub arp_timer: f32,
    pub arp_note_timer: f32,
    // Poly Mod
    pub poly_mod_filter_env_to_osc_a_freq: f32,
    pub poly_mod_filter_env_to_osc_a_pw: f32,
    pub poly_mod_osc_b_to_osc_a_freq: f32,
    pub poly_mod_osc_b_to_osc_a_pw: f32,
    pub poly_mod_osc_b_to_filter_cutoff: f32,
    // Glide
    pub glide_time: f32,
    // Pitch bend (updated from MIDI, applied to all voice frequencies)
    pub pitch_bend: f32,      // -1.0..=1.0
    pub pitch_bend_range: u8, // semitones
    // Aftertouch
    pub aftertouch: f32,
    pub aftertouch_to_cutoff: f32,
    pub aftertouch_to_amplitude: f32,
    // Expression pedal
    pub expression: f32, // 0.0..=1.0 multiplier on master output
    // Mod wheel (CC 1): additional LFO depth scaler
    pub mod_wheel: f32, // 0.0..=1.0
    // Velocity curve: 0=Linear, 1=Soft, 2=Hard
    pub velocity_curve: u8,
    // LFO delay/fade-in (segundos, 0 = instantáneo)
    pub lfo_delay: f32,
    // MIDI clock sync
    pub arp_sync_to_midi: bool,
    pub midi_clock_running: bool,
    pub midi_clock_bpm: f32,
    pub midi_clock_tick_acc: f32, // segundos acumulados desde primer tick del quarter note actual
    pub midi_clock_tick_count: u32, // ticks desde el inicio del quarter note actual
    // Master DC blocker (one-pole HPF, coeff ≈ 0.9999 → ~0.7 Hz cutoff at 44.1 kHz)
    pub master_dc_x: f32,
    pub master_dc_y: f32,
    // 1-pole parameter smoothers (anti-zipper noise for CC-controlled params)
    pub filter_cutoff_smooth: f32,
    pub filter_resonance_smooth: f32,
    pub master_volume_smooth: f32,
    // Analog character: global knobs for subtle circuit imperfections.
    pub analog: AnalogCharacter,
    // Master-bus pink noise generator for the constant analog hiss floor.
    pub master_noise_prng: u32,
    pub master_noise_b0: f32,
    pub master_noise_b1: f32,
    pub master_noise_b2: f32,
    // Stereo spread: 0.0 = mono, 1.0 = full alternating L/R spread across voices.
    pub stereo_spread: f32,
    // Tuning mode: 0=Equal Temperament, 1=Just Intonation, 2=Pythagorean, 3=Werckmeister III
    pub tuning_mode: u8,
    // Biquad LP decimation filter state (two cascaded stages for 4× support)
    pub decim_x1_l: f32,
    pub decim_x2_l: f32,
    pub decim_y1_l: f32,
    pub decim_y2_l: f32,
    pub decim_x1_r: f32,
    pub decim_x2_r: f32,
    pub decim_y1_r: f32,
    pub decim_y2_r: f32,
    // Second cascade stage (4× oversampling only)
    pub decim2_x1_l: f32,
    pub decim2_x2_l: f32,
    pub decim2_y1_l: f32,
    pub decim2_y2_l: f32,
    pub decim2_x1_r: f32,
    pub decim2_x2_r: f32,
    pub decim2_y1_r: f32,
    pub decim2_y2_r: f32,
    pub oversampling: u8,
}

impl Default for OscillatorParams {
    fn default() -> Self {
        Self {
            wave_type: WaveType::Sawtooth,
            amplitude: 1.0,
            detune: 0.0,
            pulse_width: 0.5,
        }
    }
}

impl Default for FilterParams {
    fn default() -> Self {
        Self {
            cutoff: 5000.0,
            resonance: 1.0,
            envelope_amount: 0.0,
            keyboard_tracking: 0.0,
        }
    }
}

impl Default for EnvelopeParams {
    fn default() -> Self {
        Self {
            attack: 0.01,
            decay: 0.3,
            sustain: 0.8,
            release: 0.3,
        }
    }
}

impl Default for LfoParams {
    fn default() -> Self {
        Self {
            frequency: 2.0,
            amplitude: 0.1,
            waveform: LfoWaveform::Triangle, // Vintage analog default
            sync: false,
            target_osc1_pitch: false,
            target_osc2_pitch: false,
            target_filter: false,
            target_amplitude: false,
        }
    }
}

impl Default for ModulationMatrix {
    fn default() -> Self {
        Self {
            lfo_to_cutoff: 0.0,
            lfo_to_resonance: 0.0,
            lfo_to_osc1_pitch: 0.0,
            lfo_to_osc2_pitch: 0.0,
            lfo_to_amplitude: 0.0,
            velocity_to_cutoff: 0.0,
            velocity_to_amplitude: 0.5,
        }
    }
}

impl Default for EffectsParams {
    fn default() -> Self {
        Self {
            reverb_amount: 0.0,
            reverb_size: 0.5,
            delay_time: 0.25,
            delay_feedback: 0.3,
            delay_amount: 0.0,
            chorus_mix: 0.0,
            chorus_rate: 0.6,
            chorus_depth: 0.5,
        }
    }
}

impl Default for AnalogCharacter {
    fn default() -> Self {
        Self {
            // Subtle defaults — audible on careful listening but never distracting.
            component_tolerance: 0.3,
            filter_drift_amount: 0.3,
            vca_bleed: 0.002,    // ≈ -54 dB leakage through a closed VCA
            noise_floor: 0.0008, // ≈ -62 dB background hiss
        }
    }
}

impl Default for ArpeggiatorParams {
    fn default() -> Self {
        Self {
            enabled: false,
            rate: 120.0, // BPM
            pattern: ArpPattern::Up,
            octaves: 1,
            gate_length: 0.8,
        }
    }
}

impl Default for MixerParams {
    fn default() -> Self {
        Self {
            osc1_level: 0.8,
            osc2_level: 0.6,
            noise_level: 0.0,
        }
    }
}

impl Voice {
    pub fn new(note: u8, frequency: f32, velocity: f32) -> Self {
        Self {
            frequency,
            note,
            velocity,
            // Random initial phase prevents phase-coherent cancellations in chords.
            phase1_accumulator: rand::random::<u32>() as u64,
            phase2_accumulator: rand::random::<u32>() as u64,
            envelope_state: EnvelopeState::Attack,
            envelope_time: 0.0,
            envelope_value: 0.0,
            filter_envelope_state: EnvelopeState::Attack,
            filter_envelope_value: 0.0,
            filter_state: LadderFilterState {
                stage1: 0.0,
                stage2: 0.0,
                stage3: 0.0,
                stage4: 0.0,
            },
            is_active: true,
            is_sustained: false,
            sustain_time: 0.0,
            glide_current_freq: frequency,
            // Random drift phase and rate so voices oscillate independently.
            // Rate in [0.05, 0.25] Hz → period 4–20 s, mimicking VCO thermal drift.
            drift_phase: rand::random::<f32>(),
            drift_rate: 0.05 + rand::random::<f32>() * 0.20,
            // Non-zero PRNG seed (xorshift produces 0 forever if seeded with 0).
            noise_prng: rand::random::<u32>() | 1,
            noise_b0: 0.0,
            noise_b1: 0.0,
            noise_b2: 0.0,
            osc2_last_out: 0.0,
            lfo_delay_elapsed: 0.0,
            tolerance_cutoff_mul: 1.0 + (rand::random::<f32>() - 0.5) * TOLERANCE_CUTOFF_RANGE,
            tolerance_res_mul: 1.0 + (rand::random::<f32>() - 0.5) * TOLERANCE_RES_RANGE,
            filter_drift_value: 1.0,
            filter_drift_target: 1.0 + (rand::random::<f32>() - 0.5) * FILTER_DRIFT_RANGE,
            // Stagger initial timers so voices re-pick targets at different times.
            filter_drift_timer: FILTER_DRIFT_MIN_PERIOD
                + rand::random::<f32>() * (FILTER_DRIFT_MAX_PERIOD - FILTER_DRIFT_MIN_PERIOD),
            pan: 0.0,
            pan_left: std::f32::consts::FRAC_1_SQRT_2,
            pan_right: std::f32::consts::FRAC_1_SQRT_2,
        }
    }

    pub fn release(&mut self) {
        match self.envelope_state {
            EnvelopeState::Attack | EnvelopeState::Decay | EnvelopeState::Sustain => {
                self.envelope_state = EnvelopeState::Release;
                self.envelope_time = 0.0;
                // Keep current envelope_value as release starting point
            }
            _ => {} // Already in release or idle
        }
        match self.filter_envelope_state {
            EnvelopeState::Attack | EnvelopeState::Decay | EnvelopeState::Sustain => {
                self.filter_envelope_state = EnvelopeState::Release;
                // Keep current filter_envelope_value as release starting point
            }
            _ => {} // Already in release or idle
        }
    }

    /// Si el sustain pedal está activo, marca la voz como sostenida; si no, la libera.
    pub fn release_or_sustain(&mut self, sustain_held: bool) {
        if sustain_held {
            self.is_sustained = true;
        } else {
            self.release();
        }
    }
}

impl Synthesizer {
    pub fn new() -> Self {
        let sample_rate = 44100.0;

        Self {
            osc1: OscillatorParams::default(),
            osc2: OscillatorParams::default(),
            osc2_sync: false,
            mixer: MixerParams::default(),
            filter: FilterParams::default(),
            filter_envelope: EnvelopeParams::default(),
            amp_envelope: EnvelopeParams::default(),
            lfo: LfoParams::default(),
            modulation_matrix: ModulationMatrix::default(),
            effects: EffectsParams::default(),
            arpeggiator: ArpeggiatorParams::default(),
            master_volume: 0.7,
            voices: Vec::new(),
            sample_rate,
            lfo_phase_accumulator: 0,
            lfo_sample_hold_value: 0.0,
            lfo_last_sample_time: 0.0,
            max_polyphony: 8,
            effects_chain: EffectsChain::new(sample_rate),
            held_notes: Vec::new(),
            sustain_held: false,
            voice_mode: VoiceMode::Poly,
            note_priority: NotePriority::Last,
            unison_spread: 10.0,
            note_stack: Vec::new(),
            arp_step: 0,
            arp_timer: 0.0,
            arp_note_timer: 0.0,
            poly_mod_filter_env_to_osc_a_freq: 0.0,
            poly_mod_filter_env_to_osc_a_pw: 0.0,
            poly_mod_osc_b_to_osc_a_freq: 0.0,
            poly_mod_osc_b_to_osc_a_pw: 0.0,
            poly_mod_osc_b_to_filter_cutoff: 0.0,
            glide_time: 0.0,
            pitch_bend: 0.0,
            pitch_bend_range: 2,
            aftertouch: 0.0,
            aftertouch_to_cutoff: 0.5,
            aftertouch_to_amplitude: 0.0,
            expression: 1.0,
            mod_wheel: 0.0,
            velocity_curve: 0,
            lfo_delay: 0.0,
            arp_sync_to_midi: false,
            midi_clock_running: false,
            midi_clock_bpm: 120.0,
            midi_clock_tick_acc: 0.0,
            midi_clock_tick_count: 0,
            master_dc_x: 0.0,
            master_dc_y: 0.0,
            filter_cutoff_smooth: 5000.0,
            filter_resonance_smooth: 1.0,
            master_volume_smooth: 0.7,
            analog: AnalogCharacter::default(),
            master_noise_prng: rand::random::<u32>() | 1,
            master_noise_b0: 0.0,
            master_noise_b1: 0.0,
            master_noise_b2: 0.0,
            stereo_spread: 0.0,
            tuning_mode: 0,
            decim_x1_l: 0.0,
            decim_x2_l: 0.0,
            decim_y1_l: 0.0,
            decim_y2_l: 0.0,
            decim_x1_r: 0.0,
            decim_x2_r: 0.0,
            decim_y1_r: 0.0,
            decim_y2_r: 0.0,
            decim2_x1_l: 0.0,
            decim2_x2_l: 0.0,
            decim2_y1_l: 0.0,
            decim2_y2_l: 0.0,
            decim2_x1_r: 0.0,
            decim2_x2_r: 0.0,
            decim2_y1_r: 0.0,
            decim2_y2_r: 0.0,
            oversampling: 1,
        }
    }

    pub fn note_on(&mut self, note: u8, velocity: u8) {
        match self.voice_mode {
            VoiceMode::Poly => {
                if self.arpeggiator.enabled {
                    if !self.held_notes.contains(&note) {
                        self.held_notes.push(note);
                        self.held_notes.sort();
                    }
                } else {
                    self.trigger_note(note, velocity);
                }
            }
            VoiceMode::Mono | VoiceMode::Legato => {
                // Mantener un stack de notas pulsadas; la prioridad determina cuál suena
                self.note_stack.retain(|&(n, _)| n != note);
                self.note_stack.push((note, velocity));
                let is_legato = self.voice_mode == VoiceMode::Legato;
                let already_playing = self.voices.iter().any(|v| v.is_active);
                if let Some((n, v)) = self.select_mono_note() {
                    self.trigger_mono(n, v, is_legato && already_playing);
                }
            }
            VoiceMode::Unison => {
                self.note_stack.clear();
                self.note_stack.push((note, velocity));
                self.trigger_unison(note, velocity);
            }
        }
    }

    fn trigger_note(&mut self, note: u8, velocity: u8) {
        let frequency = Self::note_to_frequency_tuned(note, self.tuning_mode);
        let velocity_normalized = Self::apply_velocity_curve(velocity, self.velocity_curve);

        // Reset LFO phase if keyboard sync is enabled
        if self.lfo.sync {
            self.lfo_phase_accumulator = 0;
            self.lfo_last_sample_time = 0.0;
            // Generate new sample & hold value for consistency
            self.lfo_sample_hold_value = (rand::random::<f32>() - 0.5) * 2.0;
        }

        // Check if note is already playing - for intentional re-triggering, we restart it
        for voice in &mut self.voices {
            if voice.note == note {
                // Smooth retrigger: restart both envelopes from their current values.
                // The RC-style attack (value → 1.0 exponentially) starts from wherever
                // the envelope is now, so there is no amplitude discontinuity and no click.
                // Halving the value (the old approach) caused a zip on loud notes.
                voice.frequency = frequency;
                voice.velocity = velocity_normalized;
                voice.envelope_state = EnvelopeState::Attack;
                voice.envelope_time = 0.0;
                voice.filter_envelope_state = EnvelopeState::Attack;
                voice.lfo_delay_elapsed = 0.0; // reiniciar fade-in del LFO
                voice.is_active = true;
                return;
            }
        }

        // Find an inactive voice, create new one, or steal one
        if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active) {
            *voice = Voice::new(note, frequency, velocity_normalized);
        } else if self.voices.len() < self.max_polyphony {
            self.voices
                .push(Voice::new(note, frequency, velocity_normalized));
        } else {
            // Voice stealing: find the best voice to replace
            let steal_index = self.find_voice_to_steal();
            self.voices[steal_index] = Voice::new(note, frequency, velocity_normalized);
        }

        // Assign pan based on voice index (alternating L/R for natural spread)
        if let Some(idx) = self
            .voices
            .iter()
            .position(|v| v.is_active && v.note == note)
        {
            let sign = if idx % 2 == 0 { 1.0_f32 } else { -1.0_f32 };
            let spread = self.stereo_spread;
            Self::set_voice_pan(&mut self.voices[idx], sign * spread);
        }
    }

    pub fn note_off(&mut self, note: u8) {
        match self.voice_mode {
            VoiceMode::Poly => {
                if self.arpeggiator.enabled {
                    self.held_notes.retain(|&n| n != note);
                    if self.held_notes.is_empty() {
                        for voice in &mut self.voices {
                            if voice.is_active {
                                voice.release_or_sustain(self.sustain_held);
                            }
                        }
                    }
                } else {
                    for voice in &mut self.voices {
                        if voice.note == note && voice.is_active {
                            if self.sustain_held {
                                voice.is_sustained = true;
                            } else {
                                voice.release();
                            }
                        }
                    }
                }
            }
            VoiceMode::Mono | VoiceMode::Legato => {
                self.note_stack.retain(|&(n, _)| n != note);
                if self.note_stack.is_empty() {
                    for voice in &mut self.voices {
                        if voice.is_active {
                            if self.sustain_held {
                                voice.is_sustained = true;
                            } else {
                                voice.release();
                            }
                        }
                    }
                } else {
                    // Retroceder a la nota anterior del stack, siempre con legato
                    if let Some((n, v)) = self.select_mono_note() {
                        self.trigger_mono(n, v, true);
                    }
                }
            }
            VoiceMode::Unison => {
                self.note_stack.retain(|&(n, _)| n != note);
                if self.note_stack.is_empty() {
                    for voice in &mut self.voices {
                        if voice.is_active {
                            if self.sustain_held {
                                voice.is_sustained = true;
                            } else {
                                voice.release();
                            }
                        }
                    }
                }
            }
        }
    }

    /// Selecciona la nota activa en modo mono según la prioridad configurada.
    fn select_mono_note(&self) -> Option<(u8, u8)> {
        if self.note_stack.is_empty() {
            return None;
        }
        Some(match self.note_priority {
            NotePriority::Last => *self.note_stack.last().unwrap(),
            NotePriority::Low => *self.note_stack.iter().min_by_key(|(n, _)| *n).unwrap(),
            NotePriority::High => *self.note_stack.iter().max_by_key(|(n, _)| *n).unwrap(),
        })
    }

    /// Dispara o actualiza una única voz monofónica.
    /// Si `legato` es true, no retriggeriza los envelopes.
    fn trigger_mono(&mut self, note: u8, velocity: u8, legato: bool) {
        let frequency = Self::note_to_frequency_tuned(note, self.tuning_mode);
        let vel = Self::apply_velocity_curve(velocity, self.velocity_curve);

        if self.lfo.sync && !legato {
            self.lfo_phase_accumulator = 0;
            self.lfo_last_sample_time = 0.0;
            self.lfo_sample_hold_value = (rand::random::<f32>() - 0.5) * 2.0;
        }

        // Reutilizar la primera voz activa (modo mono → máximo 1 voz)
        if let Some(voice) = self.voices.iter_mut().find(|v| v.is_active) {
            voice.note = note;
            voice.frequency = frequency;
            voice.velocity = vel;
            voice.is_sustained = false;
            if !legato {
                voice.envelope_state = EnvelopeState::Attack;
                voice.envelope_time = 0.0;
                voice.filter_envelope_state = EnvelopeState::Attack;
                voice.lfo_delay_elapsed = 0.0; // reiniciar fade-in del LFO
            }
        } else if self.voices.is_empty() {
            self.voices.push(Voice::new(note, frequency, vel));
        } else if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active) {
            *voice = Voice::new(note, frequency, vel);
        } else {
            self.voices[0] = Voice::new(note, frequency, vel);
        }
        // Mono mode: always centered
        if let Some(v) = self.voices.first_mut() {
            Self::set_voice_pan(v, 0.0);
        }
    }

    /// Dispara todas las voces en modo unison con detune spread.
    fn trigger_unison(&mut self, note: u8, velocity: u8) {
        let frequency = Self::note_to_frequency_tuned(note, self.tuning_mode);
        let vel = Self::apply_velocity_curve(velocity, self.velocity_curve);
        let n_voices = self.max_polyphony.max(1);
        let spread = self.unison_spread;

        if self.lfo.sync {
            self.lfo_phase_accumulator = 0;
            self.lfo_last_sample_time = 0.0;
            self.lfo_sample_hold_value = (rand::random::<f32>() - 0.5) * 2.0;
        }

        // Liberar voces activas actuales
        for v in &mut self.voices {
            v.is_active = false;
        }

        // Activar voces con detune spread
        for i in 0..n_voices {
            let detune_cents = if n_voices == 1 {
                0.0
            } else {
                spread * (2.0 * i as f32 / (n_voices - 1) as f32 - 1.0)
            };
            let detuned_freq = frequency * Self::semitones_to_ratio(detune_cents / 100.0);
            let mut v = Voice::new(note, detuned_freq, vel);
            // Unison: always centered pan
            Self::set_voice_pan(&mut v, 0.0);
            if i < self.voices.len() {
                self.voices[i] = v;
            } else {
                self.voices.push(v);
            }
        }
    }

    /// Procesa un tick de MIDI clock (0xF8). Cada 24 ticks = 1 quarter note.
    /// Calcula el BPM desde el intervalo entre ticks y lo aplica al arpeggiador.
    pub fn midi_clock_tick(&mut self, dt_since_last: f32) {
        if !self.arp_sync_to_midi || !self.midi_clock_running {
            return;
        }
        self.midi_clock_tick_acc += dt_since_last;
        self.midi_clock_tick_count += 1;
        if self.midi_clock_tick_count >= 24 {
            // Un quarter note completado — calcular BPM
            let bpm = 60.0 / self.midi_clock_tick_acc;
            self.midi_clock_bpm = bpm;
            self.arpeggiator.rate = bpm.clamp(20.0, 300.0);
            self.midi_clock_tick_acc = 0.0;
            self.midi_clock_tick_count = 0;
        }
    }

    /// Maneja el sustain pedal. Al soltar el pedal, libera todas las voces sostenidas.
    pub fn sustain_pedal(&mut self, pressed: bool) {
        self.sustain_held = pressed;
        if !pressed {
            for voice in &mut self.voices {
                if voice.is_active && voice.is_sustained {
                    voice.is_sustained = false;
                    voice.release();
                }
            }
        }
    }

    /// PANIC: silencia inmediatamente todas las voces y limpia cualquier
    /// estado de notas pulsadas. Es el remedio universal para stuck notes
    /// causadas por pérdida de foco, Note Off perdidos por MIDI o cambios
    /// de voice_mode con voces sonando.
    pub fn all_notes_off(&mut self) {
        for voice in &mut self.voices {
            voice.is_active = false;
            voice.is_sustained = false;
        }
        self.note_stack.clear();
        self.held_notes.clear();
        self.sustain_held = false;
    }

    fn find_voice_to_steal(&self) -> usize {
        let mut best_index = 0;
        let mut best_score = f32::MIN;

        for (i, voice) in self.voices.iter().enumerate() {
            let mut score = 0.0;

            // Prefer voices in release phase
            if voice.envelope_state == EnvelopeState::Release {
                score += 100.0;
            }

            // Prefer quieter voices
            score += (1.0 - voice.envelope_value) * 50.0;

            // Prefer older voices (longer time in current state)
            score += voice.envelope_time * 10.0;

            // Prefer voices with lower amplitude
            if voice.envelope_state != EnvelopeState::Attack {
                score += (1.0 - voice.envelope_value) * 30.0;
            }

            if score > best_score {
                best_score = score;
                best_index = i;
            }
        }

        best_index
    }

    pub fn note_to_frequency(note: u8) -> f32 {
        OPTIMIZATION_TABLES.get_midi_frequency(note)
    }

    /// Return the frequency for a MIDI note in the requested tuning system.
    /// mode: 0=Equal Temperament, 1=Just Intonation (5-limit),
    ///       2=Pythagorean, 3=Werckmeister III.
    /// All modes are anchored so that MIDI 69 (A4) = 440 Hz exactly.
    pub fn note_to_frequency_tuned(note: u8, mode: u8) -> f32 {
        if mode == 0 {
            return Self::note_to_frequency(note);
        }
        let table: &[f64; 12] = match mode {
            1 => &TUNING_JI,
            2 => &TUNING_PYTHAGOREAN,
            3 => &TUNING_WERCKMEISTER3,
            _ => return Self::note_to_frequency(note),
        };
        // All tables express scale degree ratios relative to C (index 0=C, 9=A).
        // Strategy: express each note as (octave_from_c4, pitch_class_from_c),
        // then anchor the whole thing so that pitch_class 9 (A) in octave 5 (C4..B4
        // is octave 4, so A4 is in octave 4 above C0) equals 440 Hz.
        //
        // MIDI 60 = C4. So octave_from_c4 = (note - 60) / 12 (integer div, euclid).
        // pitch_class = note % 12.
        // freq = C4_freq * 2^octave_from_c4 * table[pitch_class]
        //
        // We want A4 = 440 Hz. A4 = MIDI 69 => pitch_class 9, octave_from_c4 = 0
        //   (since 69-60=9, 9 div_euclid 12 = 0, 9 rem_euclid 12 = 9).
        // So freq_a4 = C4_freq * table[9] => C4_freq = 440 / table[9].
        let semitones_from_c4 = note as i32 - 60;
        let octave_from_c4 = semitones_from_c4.div_euclid(12);
        let pitch_class = semitones_from_c4.rem_euclid(12) as usize;
        // C4 is anchored via A4=440: C4_freq = 440 / table[9]
        let c4_freq = 440.0f64 / table[9];
        let freq = c4_freq * (2.0f64).powi(octave_from_c4) * table[pitch_class];
        freq as f32
    }

    /// Set equal-power panning for a voice. pan ∈ [-1.0, 1.0], -1=full left, +1=full right.
    fn set_voice_pan(voice: &mut Voice, pan: f32) {
        voice.pan = pan.clamp(-1.0, 1.0);
        voice.pan_left = ((1.0 - voice.pan) / 2.0).sqrt();
        voice.pan_right = ((1.0 + voice.pan) / 2.0).sqrt();
    }

    fn apply_velocity_curve(velocity: u8, curve: u8) -> f32 {
        let v = velocity as f32 / 127.0;
        match curve {
            1 => v.sqrt(), // Soft: more sensitive at low velocities
            2 => v * v,    // Hard: requires strong playing
            _ => v,        // Linear (default)
        }
    }

    fn generate_lfo_waveform(waveform: LfoWaveform, phase: f32, sample_hold_value: f32) -> f32 {
        let phase = phase % 1.0;
        match waveform {
            LfoWaveform::Triangle => {
                // Triangle wave: rises from -1 to 1, then falls back to -1
                if phase < 0.5 {
                    -1.0 + 4.0 * phase // Rising edge: -1 to 1
                } else {
                    3.0 - 4.0 * phase // Falling edge: 1 to -1
                }
            }
            LfoWaveform::Square => {
                // Square wave: -1 for first half, +1 for second half
                if phase < 0.5 { -1.0 } else { 1.0 }
            }
            LfoWaveform::Sawtooth => {
                // Sawtooth wave: rises from -1 to 1 linearly
                -1.0 + 2.0 * phase
            }
            LfoWaveform::ReverseSawtooth => {
                // Reverse sawtooth: falls from 1 to -1 linearly
                1.0 - 2.0 * phase
            }
            LfoWaveform::SampleAndHold => {
                // Sample and hold: constant value until next sample
                sample_hold_value
            }
        }
    }

    pub fn process_block(&mut self, left: &mut [f32], right: &mut [f32]) {
        let dt = 1.0 / self.sample_rate;

        // Copy synth parameters to avoid borrowing issues
        let osc1_wave_type = self.osc1.wave_type;
        let osc1_amplitude = self.osc1.amplitude;
        let osc1_detune = self.osc1.detune;
        let osc1_pulse_width_base = self.osc1.pulse_width;
        let osc2_wave_type = self.osc2.wave_type;
        let osc2_amplitude = self.osc2.amplitude;
        let osc2_detune = self.osc2.detune;
        let osc2_pulse_width = self.osc2.pulse_width;
        let osc2_sync = self.osc2_sync;
        let mixer_osc1_level = self.mixer.osc1_level;
        let mixer_osc2_level = self.mixer.osc2_level;
        let mixer_noise_level = self.mixer.noise_level;
        // 1-pole smoothers: τ ≈ 1/(2π·20 Hz) ≈ 8ms — eliminates zipper noise from CC updates
        let smooth_k =
            (1.0 - (-2.0 * std::f32::consts::PI * 20.0 / self.sample_rate).exp()).min(1.0);
        self.filter_cutoff_smooth += smooth_k * (self.filter.cutoff - self.filter_cutoff_smooth);
        self.filter_resonance_smooth +=
            smooth_k * (self.filter.resonance - self.filter_resonance_smooth);
        self.master_volume_smooth += smooth_k * (self.master_volume - self.master_volume_smooth);
        let filter_cutoff = self.filter_cutoff_smooth;
        let filter_resonance = self.filter_resonance_smooth;
        let filter_envelope_amount = self.filter.envelope_amount;
        let filter_keyboard_tracking = self.filter.keyboard_tracking;
        let envelope_attack = self.amp_envelope.attack;
        let envelope_decay = self.amp_envelope.decay;
        let envelope_sustain = self.amp_envelope.sustain;
        let envelope_release = self.amp_envelope.release;
        let filter_envelope_attack = self.filter_envelope.attack;
        let filter_envelope_decay = self.filter_envelope.decay;
        let filter_envelope_sustain = self.filter_envelope.sustain;
        let filter_envelope_release = self.filter_envelope.release;
        let lfo_frequency = self.lfo.frequency;
        let lfo_amplitude = self.lfo.amplitude;
        let lfo_waveform = self.lfo.waveform;
        let lfo_target_osc1 = self.lfo.target_osc1_pitch;
        let lfo_target_osc2 = self.lfo.target_osc2_pitch;
        let lfo_target_filter = self.lfo.target_filter;
        let lfo_target_amplitude = self.lfo.target_amplitude;
        let modulation_matrix = self.modulation_matrix.clone();
        let master_volume = self.master_volume_smooth;
        let sample_rate = self.sample_rate;
        let poly_mod_fe_freq = self.poly_mod_filter_env_to_osc_a_freq;
        let poly_mod_fe_pw = self.poly_mod_filter_env_to_osc_a_pw;
        let poly_mod_osc_b_freq = self.poly_mod_osc_b_to_osc_a_freq;
        let poly_mod_osc_b_pw = self.poly_mod_osc_b_to_osc_a_pw;
        let poly_mod_osc_b_cutoff = self.poly_mod_osc_b_to_filter_cutoff;
        let glide_time = self.glide_time;
        let pitch_bend = self.pitch_bend;
        let pitch_bend_range = self.pitch_bend_range;
        let aftertouch = self.aftertouch;
        let aftertouch_to_cutoff = self.aftertouch_to_cutoff;
        let aftertouch_to_amplitude = self.aftertouch_to_amplitude;
        let expression = self.expression;
        let mod_wheel = self.mod_wheel;
        let lfo_delay = self.lfo_delay;
        let component_tolerance = self.analog.component_tolerance;
        let filter_drift_amount = self.analog.filter_drift_amount;
        let vca_bleed = self.analog.vca_bleed;

        // Precompute values that are constant for the entire block.
        // Avoids transcendental calls (powf, exp) inside the per-sample voice loop.
        let osc1_detune_ratio = Self::semitones_to_ratio(osc1_detune / 100.0);
        let osc2_detune_ratio = Self::semitones_to_ratio(osc2_detune / 100.0);
        // Pitch bend ratio: ±pitch_bend_range semitones at full deflection.
        let pitch_bend_ratio = Self::semitones_to_ratio(pitch_bend * pitch_bend_range as f32);
        // Aftertouch modulations are per-block constants (aftertouch doesn't change within a buffer).
        // Velocity is per-voice so it stays inside the loop; aftertouch is channel-wide so it doesn't.
        // Cutoff uses 4× the velocity scale (4000 vs 1000 Hz) because aftertouch sweeps are typically
        // larger and more expressive than velocity-triggered filter movements.
        let aftertouch_cutoff_mod = aftertouch * aftertouch_to_cutoff * 4000.0;
        let aftertouch_amplitude_mod = 1.0 + aftertouch * aftertouch_to_amplitude * 0.5;

        // Voice gain normalization: prevent loud chords from driving the clipper hard.
        // With N voices summing to ±N, the RMS grows as √N so we scale down by 1/√N.
        // Calculated once per buffer — voice count rarely changes within a block.
        let active_voice_count = self.voices.iter().filter(|v| v.is_active).count();
        let voice_norm = 1.0_f32 / (active_voice_count.max(1) as f32).sqrt();
        // Envelope RC coefficients — coeff=0 gives instant transition (handles attack/decay/release=0)
        let amp_attack_coeff = (-dt * 5.0 / envelope_attack).exp();
        let amp_decay_coeff = (-dt * 5.0 / envelope_decay).exp();
        let amp_release_coeff = (-dt * 5.0 / envelope_release).exp();
        let flt_attack_coeff = (-dt * 5.0 / filter_envelope_attack).exp();
        let flt_decay_coeff = (-dt * 5.0 / filter_envelope_decay).exp();
        let flt_release_coeff = (-dt * 5.0 / filter_envelope_release).exp();
        // Glide coefficient: exp(-dt/tau). Constant per block since glide_time and sample_rate don't change.
        // Precomputed here to avoid calling exp() once per voice per sample in the inner loop.
        let glide_coeff = if glide_time > 0.001 {
            (-1.0_f32 / (glide_time * sample_rate)).exp()
        } else {
            0.0
        };

        debug_assert_eq!(left.len(), right.len());
        let n = left.len();
        for i in 0..n {
            left[i] = 0.0;
            right[i] = 0.0;
        }

        for si in 0..n {
            let mut left_acc = 0.0f32;
            let mut right_acc = 0.0f32;

            // Update arpeggiator
            self.update_arpeggiator(dt);

            // Update LFO using integer phase accumulator to prevent drift
            let lfo_phase_increment =
                ((lfo_frequency / self.sample_rate) * PHASE_SCALE as f32) as u64;
            self.lfo_phase_accumulator =
                self.lfo_phase_accumulator.wrapping_add(lfo_phase_increment);

            // Update sample & hold if needed (at ~100Hz rate)
            self.lfo_last_sample_time += dt;
            if lfo_waveform == LfoWaveform::SampleAndHold && self.lfo_last_sample_time >= 0.01 {
                self.lfo_sample_hold_value = (rand::random::<f32>() - 0.5) * 2.0;
                self.lfo_last_sample_time = 0.0;
            }

            // Generate LFO value using the selected waveform
            // Convert accumulator to phase (0.0 to 1.0)
            let lfo_phase = (self.lfo_phase_accumulator & PHASE_MASK) as f32 / PHASE_SCALE as f32;
            // mod_wheel adds extra depth on top of lfo_amplitude (0 = unchanged, 1 = double)
            let lfo_value =
                Self::generate_lfo_waveform(lfo_waveform, lfo_phase, self.lfo_sample_hold_value)
                    * lfo_amplitude
                    * (1.0 + mod_wheel);

            // Process all active voices
            for voice in &mut self.voices {
                if !voice.is_active {
                    continue;
                }

                // LFO delay / fade-in: ramp from 0 to 1 over lfo_delay seconds desde note-on
                let lfo_fade = if lfo_delay > 0.001 {
                    if voice.lfo_delay_elapsed < lfo_delay {
                        voice.lfo_delay_elapsed += dt;
                    }
                    (voice.lfo_delay_elapsed / lfo_delay).min(1.0)
                } else {
                    1.0
                };
                let lfo_value_voice = lfo_value * lfo_fade;

                // Glide: exponential interpolation toward the target frequency
                if glide_coeff > 0.0 {
                    voice.glide_current_freq = voice.frequency
                        + (voice.glide_current_freq - voice.frequency) * glide_coeff;
                } else {
                    voice.glide_current_freq = voice.frequency;
                }
                let base_freq = voice.glide_current_freq;

                // Per-voice VCO drift: advance the slow drift LFO and compute a
                // tiny pitch deviation. Each voice has its own rate so they drift
                // independently, reproducing the tuning "life" of real analog VCOs.
                voice.drift_phase += voice.drift_rate * dt;
                if voice.drift_phase >= 1.0 {
                    voice.drift_phase -= 1.0;
                }
                let drift_ratio = 1.0
                    + DRIFT_FREQ_FACTOR
                        * OPTIMIZATION_TABLES.fast_sin(voice.drift_phase * 2.0 * PI);

                // Calculate frequencies with detune, drift, and modulation matrix
                let mut freq1 = base_freq * osc1_detune_ratio * drift_ratio;
                let mut freq2 = base_freq * osc2_detune_ratio * drift_ratio;

                // Apply modulation matrix to oscillator pitch (gated by lfo_target booleans)
                if lfo_target_osc1 {
                    freq1 *= 1.0 + (lfo_value_voice * modulation_matrix.lfo_to_osc1_pitch * 0.1);
                }
                if lfo_target_osc2 {
                    freq2 *= 1.0 + (lfo_value_voice * modulation_matrix.lfo_to_osc2_pitch * 0.1);
                }

                // Poly Mod: Filter Envelope → Osc A frequency (±24 semitones a plena excursión)
                if poly_mod_fe_freq.abs() > 0.001 {
                    let semitones = poly_mod_fe_freq * 24.0 * voice.filter_envelope_value;
                    freq1 *= Self::semitones_to_ratio(semitones);
                }

                // Poly Mod: Filter Envelope → Osc A pulse width
                let mut osc1_pw_voice = osc1_pulse_width_base;
                if poly_mod_fe_pw.abs() > 0.001 {
                    let pw_shift = poly_mod_fe_pw * 0.4 * voice.filter_envelope_value;
                    osc1_pw_voice = (osc1_pw_voice + pw_shift).clamp(0.05, 0.95);
                }

                // Poly Mod: Osc B → Osc A (1-sample delay avoids circular dependency;
                // matches the finite propagation time in real analog hardware)
                let osc_b_mod = voice.osc2_last_out;
                if poly_mod_osc_b_freq.abs() > 0.001 {
                    let semitones = poly_mod_osc_b_freq * 24.0 * osc_b_mod;
                    freq1 *= Self::semitones_to_ratio(semitones);
                }
                if poly_mod_osc_b_pw.abs() > 0.001 {
                    let pw_shift = poly_mod_osc_b_pw * 0.4 * osc_b_mod;
                    osc1_pw_voice = (osc1_pw_voice + pw_shift).clamp(0.05, 0.95);
                }

                // Pitch bend: applied globally to both oscillators
                freq1 *= pitch_bend_ratio;
                freq2 *= pitch_bend_ratio;

                // Update phases using integer accumulators to prevent drift
                let dt1 = freq1 * dt;
                let dt2 = freq2 * dt;
                let phase1_increment = (dt1 * PHASE_SCALE as f32) as u64;
                let phase2_increment = (dt2 * PHASE_SCALE as f32) as u64;

                voice.phase1_accumulator = voice.phase1_accumulator.wrapping_add(phase1_increment);
                voice.phase2_accumulator = voice.phase2_accumulator.wrapping_add(phase2_increment);

                // Convert accumulators to phase values (0.0 to 1.0)
                let phase1 = (voice.phase1_accumulator & PHASE_MASK) as f32 / PHASE_SCALE as f32;
                let mut phase2 =
                    (voice.phase2_accumulator & PHASE_MASK) as f32 / PHASE_SCALE as f32;

                // Oscillator sync: if enabled, reset osc2 phase when osc1 completes a cycle
                let prev_phase1_accumulator =
                    voice.phase1_accumulator.wrapping_sub(phase1_increment);
                let prev_phase1 =
                    (prev_phase1_accumulator & PHASE_MASK) as f32 / PHASE_SCALE as f32;

                if osc2_sync && prev_phase1 > phase1 {
                    // Wrapped around (cycle completed)
                    voice.phase2_accumulator = 0;
                    phase2 = 0.0;
                }

                // Generate oscillator outputs using calculated phases
                let osc1_out = Self::generate_oscillator_static(
                    osc1_wave_type,
                    phase1,
                    dt1,
                    osc1_pw_voice, // puede estar modulado por poly mod
                ) * osc1_amplitude;
                let osc2_out =
                    Self::generate_oscillator_static(osc2_wave_type, phase2, dt2, osc2_pulse_width)
                        * osc2_amplitude;

                // Pink noise via per-voice xorshift32 PRNG + Paul Kellett 3-stage IIR.
                // Pink noise has -3 dB/octave rolloff, closer to the Prophet-5 noise
                // source (which is filtered before entering the signal path) than white.
                // xorshift32 is deterministic and ~8× cheaper than rand::random().
                let noise = if mixer_noise_level > 0.0 {
                    voice.noise_prng ^= voice.noise_prng << 13;
                    voice.noise_prng ^= voice.noise_prng >> 17;
                    voice.noise_prng ^= voice.noise_prng << 5;
                    let white = (voice.noise_prng as i32) as f32 * (1.0 / 2_147_483_648.0);
                    voice.noise_b0 = 0.99886 * voice.noise_b0 + white * 0.0555179;
                    voice.noise_b1 = 0.99332 * voice.noise_b1 + white * 0.0750759;
                    voice.noise_b2 = 0.96900 * voice.noise_b2 + white * 0.153_852;
                    (voice.noise_b0 + voice.noise_b1 + voice.noise_b2 + white * 0.0556418)
                        * mixer_noise_level
                } else {
                    0.0
                };
                voice.osc2_last_out = osc2_out;

                let mut mixed = osc1_out * mixer_osc1_level + osc2_out * mixer_osc2_level + noise;

                let filter_envelope_value = Self::process_filter_envelope_static(
                    voice,
                    filter_envelope_sustain,
                    flt_attack_coeff,
                    flt_decay_coeff,
                    flt_release_coeff,
                );

                let kbd_multiplier =
                    Self::semitones_to_ratio((voice.note as f32 - 60.0) * filter_keyboard_tracking);

                // Apply modulation matrix to filter (gated by lfo_target_filter)
                let lfo_cutoff_mod = if lfo_target_filter {
                    lfo_value_voice * modulation_matrix.lfo_to_cutoff * 1000.0
                } else {
                    0.0
                };
                let velocity_cutoff_mod =
                    voice.velocity * modulation_matrix.velocity_to_cutoff * 1000.0;
                let osc_b_cutoff_mod = osc_b_mod * poly_mod_osc_b_cutoff * 4000.0;
                let modulated_cutoff = (filter_cutoff
                    + lfo_cutoff_mod
                    + velocity_cutoff_mod
                    + aftertouch_cutoff_mod
                    + osc_b_cutoff_mod
                    + filter_cutoff * filter_envelope_amount * filter_envelope_value)
                    * kbd_multiplier;

                // Filter temperature drift: walk toward a target that's re-picked
                // every 1–3 s; τ ≈ 0.5 s so individual transitions are inaudible
                // but thermal sway shows up over minutes.
                voice.filter_drift_timer -= dt;
                if voice.filter_drift_timer <= 0.0 {
                    voice.filter_drift_target =
                        1.0 + (rand::random::<f32>() - 0.5) * FILTER_DRIFT_RANGE;
                    voice.filter_drift_timer = FILTER_DRIFT_MIN_PERIOD
                        + rand::random::<f32>()
                            * (FILTER_DRIFT_MAX_PERIOD - FILTER_DRIFT_MIN_PERIOD);
                }
                voice.filter_drift_value += (voice.filter_drift_target - voice.filter_drift_value)
                    * FILTER_DRIFT_ALPHA
                    * dt;

                let cutoff_tol = 1.0 + (voice.tolerance_cutoff_mul - 1.0) * component_tolerance;
                let cutoff_drift = 1.0 + (voice.filter_drift_value - 1.0) * filter_drift_amount;
                let final_cutoff =
                    (modulated_cutoff * cutoff_tol * cutoff_drift).clamp(20.0, 20000.0);

                let lfo_resonance_mod = lfo_value_voice * modulation_matrix.lfo_to_resonance * 2.0;
                let res_tol = 1.0 + (voice.tolerance_res_mul - 1.0) * component_tolerance;
                // Clamp just below 4.0 to prevent runaway feedback at self-oscillation.
                let final_resonance =
                    ((filter_resonance + lfo_resonance_mod) * res_tol).clamp(0.0, 3.95);

                // Apply ladder filter (24dB/octave vintage analog style)
                mixed = Self::apply_ladder_filter_static(
                    mixed,
                    &mut voice.filter_state,
                    final_cutoff,
                    final_resonance,
                    sample_rate,
                );

                let envelope_value = Self::process_envelope_static(
                    voice,
                    envelope_sustain,
                    dt,
                    amp_attack_coeff,
                    amp_decay_coeff,
                    amp_release_coeff,
                );

                // Apply modulation matrix to amplitude (gated by lfo_target_amplitude)
                let lfo_amplitude_mod = if lfo_target_amplitude {
                    1.0 + (lfo_value_voice * modulation_matrix.lfo_to_amplitude * 0.5)
                } else {
                    1.0
                };
                let velocity_amplitude_mod =
                    0.5 + (voice.velocity * modulation_matrix.velocity_to_amplitude * 0.5);

                // A real analog VCA never fully shuts — a constant offset lets a
                // sliver of oscillator signal escape, giving notes an "alive" taper.
                let amp_gain = envelope_value + vca_bleed;
                mixed *= amp_gain
                    * lfo_amplitude_mod
                    * velocity_amplitude_mod
                    * aftertouch_amplitude_mod;

                left_acc += mixed * voice.pan_left;
                right_acc += mixed * voice.pan_right;
            }

            // Normalize voice sum: keeps chords at comparable loudness to single notes.
            left_acc *= voice_norm;
            right_acc *= voice_norm;

            // M/S decode: compute mono mid and stereo side signals.
            let stereo_s = (right_acc - left_acc) * std::f32::consts::FRAC_1_SQRT_2;
            let mut mono_m = (left_acc + right_acc) * std::f32::consts::FRAC_1_SQRT_2;

            // Apply master volume with gentle compression; expression pedal scales on top
            mono_m *= master_volume * expression;

            // Apply effects processing. Chorus sits before delay/reverb so the
            // delay taps and reverb tail inherit the ensemble thickness.
            mono_m = self
                .effects_chain
                .apply_chorus(mono_m, &self.effects, self.sample_rate);
            mono_m = self
                .effects_chain
                .apply_delay(mono_m, &self.effects, self.sample_rate);
            mono_m = self.effects_chain.apply_reverb(mono_m, &self.effects);

            // Analog circuit hiss — the faint background the player never notices
            // until it's missing. Added after the effects so the noise feels like
            // it's coming from the amplifier, not the delay line.
            let noise_floor = self.analog.noise_floor;
            if noise_floor > 0.0 {
                mono_m += self.master_noise_sample() * noise_floor;
            }

            // Continuous saturation. The previous threshold clipper jumped
            // by ~0.18 at |x|=0.7 and buzzed on every loud peak.
            mono_m = mono_m.tanh();

            // Master DC blocker: removes DC offset from self-oscillation or asymmetric saturation.
            let dc_x = mono_m;
            mono_m = dc_x - self.master_dc_x + MASTER_DC_COEFF * self.master_dc_y;
            self.master_dc_x = dc_x;
            self.master_dc_y = mono_m;

            // Reconstruct L/R from mono mid and stereo side.
            left[si] = (mono_m - stereo_s * std::f32::consts::FRAC_1_SQRT_2).clamp(-1.0, 1.0);
            right[si] = (mono_m + stereo_s * std::f32::consts::FRAC_1_SQRT_2).clamp(-1.0, 1.0);
        }
    }

    fn generate_oscillator_static(
        wave_type: WaveType,
        phase: f32,
        dt: f32,
        pulse_width: f32,
    ) -> f32 {
        match wave_type {
            WaveType::Sine => OPTIMIZATION_TABLES.fast_sin(phase * 2.0 * PI),
            WaveType::Sawtooth => {
                let value = 2.0 * phase - 1.0;
                value - Self::poly_blep(phase, dt)
            }
            WaveType::Square => {
                let pw = pulse_width.clamp(0.01, 0.99);
                let mut value = if phase < pw { 1.0 } else { -1.0 };
                value += Self::poly_blep(phase, dt);
                let falling_phase = if phase >= pw {
                    phase - pw
                } else {
                    phase + 1.0 - pw
                };
                value -= Self::poly_blep(falling_phase, dt);
                value
            }
            WaveType::Triangle => {
                // Triangle harmonics already fall at 1/n^2, so naive aliasing is mild;
                // PolyBLAMP smooths the slope discontinuities at phase 0 and 0.5.
                let mut value = if phase < 0.5 {
                    4.0 * phase - 1.0
                } else {
                    3.0 - 4.0 * phase
                };
                value += 8.0 * dt * Self::poly_blamp(phase, dt);
                let half_phase = if phase >= 0.5 {
                    phase - 0.5
                } else {
                    phase + 0.5
                };
                value -= 8.0 * dt * Self::poly_blamp(half_phase, dt);
                value
            }
        }
    }

    #[inline]
    fn poly_blep(phase: f32, dt: f32) -> f32 {
        if phase < dt {
            let t = phase / dt;
            2.0 * t - t * t - 1.0
        } else if phase > 1.0 - dt {
            let t = (phase - 1.0) / dt;
            t * t + 2.0 * t + 1.0
        } else {
            0.0
        }
    }

    #[inline]
    fn poly_blamp(phase: f32, dt: f32) -> f32 {
        if phase < dt {
            let t = phase / dt - 1.0;
            -(1.0 / 3.0) * t * t * t
        } else if phase > 1.0 - dt {
            let t = (phase - 1.0) / dt + 1.0;
            (1.0 / 3.0) * t * t * t
        } else {
            0.0
        }
    }

    /// Convert a semitone offset to a frequency ratio: 2^(semitones/12).
    #[inline(always)]
    fn semitones_to_ratio(semitones: f32) -> f32 {
        2.0_f32.powf(semitones / 12.0)
    }

    /// Padé approximant for tanh — accurate to <0.1 % for |x| ≤ 3, clamped to ±1 beyond.
    /// Replaces libm tanh() in the filter hot path (5 calls/voice/sample).
    #[inline]
    fn fast_tanh(x: f32) -> f32 {
        if x > 3.0 {
            return 1.0;
        }
        if x < -3.0 {
            return -1.0;
        }
        let x2 = x * x;
        x * (27.0 + x2) / (27.0 + 9.0 * x2)
    }

    fn apply_ladder_filter_static(
        input: f32,
        state: &mut LadderFilterState,
        cutoff: f32,
        resonance: f32,
        sample_rate: f32,
    ) -> f32 {
        // ZDF (Zero-Delay Feedback) Moog ladder — Zavalishin TPT topology
        //
        // g = tan(π·fc/fs) — bilinear pre-warping maps the analog cutoff exactly.
        //   Without this, the cutoff drifts up to 40% flat at fc = fs/4.
        // G = g/(1+g) — TPT one-pole gain coefficient.
        //   Each stage implements: v = G*(x-s); y = v+s; s_new = y+v
        //   which is unconditionally stable for any g > 0.
        // fast_tanh per stage reproduces the distributed saturation of the real ladder
        //   where every transistor pair clips softly — the source of characteristic Moog warmth.
        let fc = (cutoff / sample_rate).min(0.498);
        let g = (PI * fc).tan();
        let cap_g = g / (1.0 + g);

        // Resonance k ∈ [0, 4). k=4 is the theoretical self-oscillation threshold.
        let k = resonance.clamp(0.0, 3.99);

        // Drive the input through tanh and subtract one-sample-delayed feedback.
        // The delay makes this a semi-implicit scheme — fully implicit would require
        // solving a nonlinear system per sample, which is too expensive here.
        let x = Self::fast_tanh(input - k * state.stage4);

        // Stage 1 — TPT one-pole
        let v1 = cap_g * (x - state.stage1);
        let y1 = v1 + state.stage1;
        state.stage1 = y1 + v1;

        // Stage 2 — fast_tanh at each stage input gives the distributed saturation
        let v2 = cap_g * (Self::fast_tanh(y1) - state.stage2);
        let y2 = v2 + state.stage2;
        state.stage2 = y2 + v2;

        // Stage 3
        let v3 = cap_g * (Self::fast_tanh(y2) - state.stage3);
        let y3 = v3 + state.stage3;
        state.stage3 = y3 + v3;

        // Stage 4
        let v4 = cap_g * (Self::fast_tanh(y3) - state.stage4);
        let y4 = v4 + state.stage4;
        state.stage4 = y4 + v4;

        // Passband gain compensation.
        // A resonant Moog ladder attenuates its passband as k rises: the DC gain is
        // 1/(1 + k·G^4). Multiplying by (1 + k·G^4) restores perceived loudness so
        // turning up resonance doesn't hollow out the low end.
        let g4 = cap_g * cap_g * cap_g * cap_g;
        let output = y4 * (1.0 + k * g4);

        // Flush denormals — prevents ~100× slowdown on decayed tails.
        const DENORMAL_FLOOR: f32 = 1.0e-20;
        if state.stage1.abs() < DENORMAL_FLOOR {
            state.stage1 = 0.0;
        }
        if state.stage2.abs() < DENORMAL_FLOOR {
            state.stage2 = 0.0;
        }
        if state.stage3.abs() < DENORMAL_FLOOR {
            state.stage3 = 0.0;
        }
        if state.stage4.abs() < DENORMAL_FLOOR {
            state.stage4 = 0.0;
        }

        output
    }

    fn process_envelope_static(
        voice: &mut Voice,
        sustain: f32,
        dt: f32,
        attack_coeff: f32,
        decay_coeff: f32,
        release_coeff: f32,
    ) -> f32 {
        voice.envelope_time += dt;

        match voice.envelope_state {
            EnvelopeState::Attack => {
                // coeff=0 when attack=0 → instant transition (exp(-inf)=0)
                voice.envelope_value = 1.0 + (voice.envelope_value - 1.0) * attack_coeff;
                if voice.envelope_value >= 0.999 {
                    voice.envelope_value = 1.0;
                    voice.envelope_state = EnvelopeState::Decay;
                    voice.envelope_time = 0.0;
                }
            }
            EnvelopeState::Decay => {
                voice.envelope_value = sustain + (voice.envelope_value - sustain) * decay_coeff;
                if (voice.envelope_value - sustain).abs() < 0.0005 {
                    voice.envelope_value = sustain;
                    voice.envelope_state = EnvelopeState::Sustain;
                }
            }
            EnvelopeState::Sustain => {
                voice.envelope_value = sustain;
                voice.sustain_time += dt;
                // Flush tiny filter state values after 1 s of sustain to prevent drift buildup
                if voice.sustain_time > 1.0 {
                    if voice.filter_state.stage1.abs() < 1e-8 {
                        voice.filter_state.stage1 = 0.0;
                    }
                    if voice.filter_state.stage2.abs() < 1e-8 {
                        voice.filter_state.stage2 = 0.0;
                    }
                    if voice.filter_state.stage3.abs() < 1e-8 {
                        voice.filter_state.stage3 = 0.0;
                    }
                    if voice.filter_state.stage4.abs() < 1e-8 {
                        voice.filter_state.stage4 = 0.0;
                    }
                    voice.sustain_time = 0.0;
                }
            }
            EnvelopeState::Release => {
                voice.envelope_value *= release_coeff;
                if voice.envelope_value < 0.0001 {
                    voice.envelope_value = 0.0;
                    voice.is_active = false;
                    voice.envelope_state = EnvelopeState::Idle;
                }
            }
            EnvelopeState::Idle => {
                voice.envelope_value = 0.0;
                voice.is_active = false;
            }
        }

        voice.envelope_value
    }

    fn process_filter_envelope_static(
        voice: &mut Voice,
        sustain: f32,
        attack_coeff: f32,
        decay_coeff: f32,
        release_coeff: f32,
    ) -> f32 {
        match voice.filter_envelope_state {
            EnvelopeState::Attack => {
                voice.filter_envelope_value =
                    1.0 + (voice.filter_envelope_value - 1.0) * attack_coeff;
                if voice.filter_envelope_value >= 0.999 {
                    voice.filter_envelope_value = 1.0;
                    voice.filter_envelope_state = EnvelopeState::Decay;
                }
            }
            EnvelopeState::Decay => {
                voice.filter_envelope_value =
                    sustain + (voice.filter_envelope_value - sustain) * decay_coeff;
                if (voice.filter_envelope_value - sustain).abs() < 0.0005 {
                    voice.filter_envelope_value = sustain;
                    voice.filter_envelope_state = EnvelopeState::Sustain;
                }
            }
            EnvelopeState::Sustain => {
                voice.filter_envelope_value = sustain;
            }
            EnvelopeState::Release => {
                voice.filter_envelope_value *= release_coeff;
                if voice.filter_envelope_value < 0.0001 {
                    voice.filter_envelope_value = 0.0;
                    voice.filter_envelope_state = EnvelopeState::Idle;
                }
            }
            EnvelopeState::Idle => {
                voice.filter_envelope_value = 0.0;
            }
        }

        voice.filter_envelope_value
    }

    fn update_arpeggiator(&mut self, dt: f32) {
        if !self.arpeggiator.enabled || self.held_notes.is_empty() {
            return;
        }

        let beat_time = 60.0 / self.arpeggiator.rate; // Time per beat in seconds
        self.arp_timer += dt;
        self.arp_note_timer += dt;

        // Check if it's time for the next note
        if self.arp_timer >= beat_time {
            self.arp_timer -= beat_time;

            // Release current arpeggiator note
            for voice in &mut self.voices {
                if voice.is_active {
                    voice.release();
                }
            }

            // Get the next note based on pattern
            if let Some(note) = self.get_next_arp_note() {
                self.trigger_note(note, 100); // Fixed velocity for arp
            }
        }

        // Handle gate length (note off)
        let gate_time = beat_time * self.arpeggiator.gate_length;
        if self.arp_note_timer >= gate_time {
            // Let note ring until next step
        }
    }

    fn get_next_arp_note(&mut self) -> Option<u8> {
        if self.held_notes.is_empty() {
            return None;
        }

        let mut extended_notes = Vec::new();

        // Generate notes across octaves
        for octave in 0..self.arpeggiator.octaves {
            for &note in &self.held_notes {
                let octave_note = note + (octave * 12);
                if octave_note <= 127 {
                    extended_notes.push(octave_note);
                }
            }
        }

        if extended_notes.is_empty() {
            return None;
        }

        let note = match self.arpeggiator.pattern {
            ArpPattern::Up => {
                let note = extended_notes[self.arp_step % extended_notes.len()];
                self.arp_step += 1;
                note
            }
            ArpPattern::Down => {
                extended_notes.reverse();
                let note = extended_notes[self.arp_step % extended_notes.len()];
                self.arp_step += 1;
                note
            }
            ArpPattern::UpDown => {
                let len = extended_notes.len();
                if len == 1 {
                    extended_notes[0]
                } else {
                    let cycle_len = (len - 1) * 2;
                    let pos = self.arp_step % cycle_len;
                    self.arp_step += 1;

                    if pos < len {
                        extended_notes[pos]
                    } else {
                        extended_notes[len - 1 - (pos - len)]
                    }
                }
            }
            ArpPattern::Random => {
                let index = (rand::random::<f32>() * extended_notes.len() as f32) as usize;
                extended_notes[index.min(extended_notes.len() - 1)]
            }
        };

        self.arp_note_timer = 0.0;
        Some(note)
    }

    /// Faint constant hiss from the analog output stage — Paul Kellett pink noise
    /// at circuit-floor level. Shared across voices, written to the master bus once
    /// per sample (cheap: three MAC + one xorshift).
    #[inline]
    fn master_noise_sample(&mut self) -> f32 {
        self.master_noise_prng ^= self.master_noise_prng << 13;
        self.master_noise_prng ^= self.master_noise_prng >> 17;
        self.master_noise_prng ^= self.master_noise_prng << 5;
        let white = (self.master_noise_prng as i32) as f32 * (1.0 / 2_147_483_648.0);
        self.master_noise_b0 = 0.99886 * self.master_noise_b0 + white * 0.0555179;
        self.master_noise_b1 = 0.99332 * self.master_noise_b1 + white * 0.0750759;
        self.master_noise_b2 = 0.96900 * self.master_noise_b2 + white * 0.153_852;
        self.master_noise_b0 + self.master_noise_b1 + self.master_noise_b2 + white * 0.0556418
    }

    pub fn save_preset(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Ensure presets directory exists
        self.ensure_presets_dir()?;

        let preset = self.snapshot_preset(name, "Other");

        let preset_json = self.preset_to_json(&preset)?;
        let filename = format!("presets/{}.json", name.replace(" ", "_"));
        fs::write(&filename, preset_json)?;
        println!("Preset '{}' saved to {}", name, filename);
        Ok(())
    }

    pub fn save_preset_with_category(
        &self,
        name: &str,
        category: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.ensure_presets_dir()?;
        let preset = self.snapshot_preset(name, category);
        let preset_json = self.preset_to_json(&preset)?;
        let filename = format!("presets/{}.json", name.replace(" ", "_"));
        std::fs::write(&filename, preset_json)?;
        log::info!("Preset '{}' [{}] saved", name, category);
        Ok(())
    }

    fn apply_preset(&mut self, preset: Preset) {
        self.osc1 = preset.osc1;
        self.osc2 = preset.osc2;
        self.osc2_sync = preset.osc2_sync;
        self.mixer = preset.mixer;
        self.filter = preset.filter;
        self.filter_envelope = preset.filter_envelope;
        self.amp_envelope = preset.amp_envelope;
        self.lfo = preset.lfo;
        self.modulation_matrix = preset.modulation_matrix;
        self.effects = preset.effects;
        self.master_volume = preset.master_volume;
        self.poly_mod_filter_env_to_osc_a_freq = preset.poly_mod_filter_env_to_osc_a_freq;
        self.poly_mod_filter_env_to_osc_a_pw = preset.poly_mod_filter_env_to_osc_a_pw;
        self.poly_mod_osc_b_to_osc_a_freq = preset.poly_mod_osc_b_to_osc_a_freq;
        self.poly_mod_osc_b_to_osc_a_pw = preset.poly_mod_osc_b_to_osc_a_pw;
        self.poly_mod_osc_b_to_filter_cutoff = preset.poly_mod_osc_b_to_filter_cutoff;
    }

    pub fn load_preset(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let filename = format!("presets/{}.json", name.replace(" ", "_"));
        if !Path::new(&filename).exists() {
            return Err(format!("Preset file '{}' not found", filename).into());
        }
        let preset_json = fs::read_to_string(&filename)?;
        let preset = self.json_to_preset(&preset_json)?;
        println!("Preset '{}' loaded from {}", name, filename);
        self.apply_preset(preset);
        Ok(())
    }

    /// Carga un preset desde un string JSON en memoria (sin I/O). Usado por MIDI SysEx.
    pub fn load_preset_from_json(&mut self, json: &str) -> Result<(), Box<dyn std::error::Error>> {
        let preset = self.json_to_preset(json)?;
        log::info!("SysEx: preset '{}' loaded from SysEx data", preset.name);
        self.apply_preset(preset);
        Ok(())
    }

    pub fn list_presets() -> Vec<String> {
        let mut presets = Vec::new();
        if let Ok(entries) = fs::read_dir("presets") {
            for entry in entries.flatten() {
                if let Some(filename) = entry.file_name().to_str()
                    && filename.ends_with(".json")
                {
                    let name = filename.trim_end_matches(".json").replace("_", " ");
                    presets.push(name);
                }
            }
        }
        presets.sort();
        presets
    }

    pub fn list_presets_with_categories() -> Vec<(String, String)> {
        let mut presets = Vec::new();
        if let Ok(entries) = std::fs::read_dir("presets") {
            for entry in entries.flatten() {
                if let Some(filename) = entry.file_name().to_str()
                    && filename.ends_with(".json")
                {
                    let name = filename.trim_end_matches(".json").replace("_", " ");
                    let category = std::fs::read_to_string(entry.path())
                        .ok()
                        .and_then(|content| {
                            content
                                .lines()
                                .nth(Self::CATEGORY_LINE_INDEX)
                                .map(|s| s.trim_matches('"').to_string())
                        })
                        .unwrap_or_else(|| "Other".to_string());
                    presets.push((name, category));
                }
            }
        }
        presets.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        presets
    }

    /// Line index of the `category` field in the preset JSON line format.
    /// Line 44 predates the format extension and is pinned for backward compat;
    /// extended fields begin at index 45.
    const CATEGORY_LINE_INDEX: usize = 44;

    fn preset_to_json(&self, preset: &Preset) -> Result<String, Box<dyn std::error::Error>> {
        // Simple line-based format for easy parsing
        let lines = vec![
            format!("\"{}\"", preset.name),
            format!("\"{}\"", self.wave_type_to_string(preset.osc1.wave_type)),
            preset.osc1.amplitude.to_string(),
            preset.osc1.detune.to_string(),
            preset.osc1.pulse_width.to_string(),
            format!("\"{}\"", self.wave_type_to_string(preset.osc2.wave_type)),
            preset.osc2.amplitude.to_string(),
            preset.osc2.detune.to_string(),
            preset.osc2.pulse_width.to_string(),
            preset.osc2_sync.to_string(),
            preset.mixer.osc1_level.to_string(),
            preset.mixer.osc2_level.to_string(),
            preset.mixer.noise_level.to_string(),
            preset.filter.cutoff.to_string(),
            preset.filter.resonance.to_string(),
            preset.filter.envelope_amount.to_string(),
            preset.filter.keyboard_tracking.to_string(),
            preset.filter_envelope.attack.to_string(),
            preset.filter_envelope.decay.to_string(),
            preset.filter_envelope.sustain.to_string(),
            preset.filter_envelope.release.to_string(),
            preset.amp_envelope.attack.to_string(),
            preset.amp_envelope.decay.to_string(),
            preset.amp_envelope.sustain.to_string(),
            preset.amp_envelope.release.to_string(),
            preset.lfo.frequency.to_string(),
            preset.lfo.amplitude.to_string(),
            preset.lfo.target_osc1_pitch.to_string(),
            preset.lfo.target_osc2_pitch.to_string(),
            preset.lfo.target_filter.to_string(),
            preset.lfo.target_amplitude.to_string(),
            preset.modulation_matrix.lfo_to_cutoff.to_string(),
            preset.modulation_matrix.lfo_to_resonance.to_string(),
            preset.modulation_matrix.lfo_to_osc1_pitch.to_string(),
            preset.modulation_matrix.lfo_to_osc2_pitch.to_string(),
            preset.modulation_matrix.lfo_to_amplitude.to_string(),
            preset.modulation_matrix.velocity_to_cutoff.to_string(),
            preset.modulation_matrix.velocity_to_amplitude.to_string(),
            preset.effects.reverb_amount.to_string(),
            preset.effects.reverb_size.to_string(),
            preset.effects.delay_time.to_string(),
            preset.effects.delay_feedback.to_string(),
            preset.effects.delay_amount.to_string(),
            preset.master_volume.to_string(),
            format!("\"{}\"", preset.category),
            // --- Extended fields (backward-compatible; older presets omit these) ---
            format!("\"{}\"", Self::lfo_waveform_to_string(preset.lfo.waveform)),
            preset.lfo.sync.to_string(),
            preset.effects.chorus_mix.to_string(),
            preset.effects.chorus_rate.to_string(),
            preset.effects.chorus_depth.to_string(),
            preset.poly_mod_filter_env_to_osc_a_freq.to_string(),
            preset.poly_mod_filter_env_to_osc_a_pw.to_string(),
            preset.poly_mod_osc_b_to_osc_a_freq.to_string(),
            preset.poly_mod_osc_b_to_osc_a_pw.to_string(),
            preset.poly_mod_osc_b_to_filter_cutoff.to_string(),
        ];
        Ok(lines.join("\n"))
    }

    fn json_to_preset(&self, json: &str) -> Result<Preset, Box<dyn std::error::Error>> {
        // Parse the JSON format we use in preset_to_json
        let lines: Vec<&str> = json.lines().collect();
        if lines.len() < 44 {
            return Err("Invalid preset format".into());
        }

        // Parse each field from the JSON lines
        let name = lines[0].trim_matches('"').to_string();

        // Oscillator 1
        let osc1_wave = self.string_to_wave_type(lines[1].trim_matches('"'))?;
        let osc1_amp: f32 = lines[2].parse()?;
        let osc1_detune: f32 = lines[3].parse()?;
        let osc1_pw: f32 = lines[4].parse()?;

        // Oscillator 2
        let osc2_wave = self.string_to_wave_type(lines[5].trim_matches('"'))?;
        let osc2_amp: f32 = lines[6].parse()?;
        let osc2_detune: f32 = lines[7].parse()?;
        let osc2_pw: f32 = lines[8].parse()?;

        // Sync
        let osc2_sync: bool = lines[9].parse()?;

        // Mixer
        let mixer_osc1: f32 = lines[10].parse()?;
        let mixer_osc2: f32 = lines[11].parse()?;
        let mixer_noise: f32 = lines[12].parse()?;

        // Filter
        let filter_cutoff: f32 = lines[13].parse()?;
        let filter_res: f32 = lines[14].parse()?;
        let filter_env: f32 = lines[15].parse()?;
        let filter_kbd: f32 = lines[16].parse()?;

        // Filter Envelope
        let fenv_a: f32 = lines[17].parse()?;
        let fenv_d: f32 = lines[18].parse()?;
        let fenv_s: f32 = lines[19].parse()?;
        let fenv_r: f32 = lines[20].parse()?;

        // Amp Envelope
        let aenv_a: f32 = lines[21].parse()?;
        let aenv_d: f32 = lines[22].parse()?;
        let aenv_s: f32 = lines[23].parse()?;
        let aenv_r: f32 = lines[24].parse()?;

        // LFO
        let lfo_freq: f32 = lines[25].parse()?;
        let lfo_amp: f32 = lines[26].parse()?;
        let lfo_osc1: bool = lines[27].parse()?;
        let lfo_osc2: bool = lines[28].parse()?;
        let lfo_filter: bool = lines[29].parse()?;
        let lfo_amplitude: bool = lines[30].parse()?;

        // Modulation Matrix
        let mod_lfo_cut: f32 = lines[31].parse()?;
        let mod_lfo_res: f32 = lines[32].parse()?;
        let mod_lfo_osc1: f32 = lines[33].parse()?;
        let mod_lfo_osc2: f32 = lines[34].parse()?;
        let mod_lfo_amp: f32 = lines[35].parse()?;
        let mod_vel_cut: f32 = lines[36].parse()?;
        let mod_vel_amp: f32 = lines[37].parse()?;

        // Effects
        let fx_rev_amt: f32 = lines[38].parse()?;
        let fx_rev_size: f32 = lines[39].parse()?;
        let fx_del_time: f32 = lines[40].parse()?;
        let fx_del_fb: f32 = lines[41].parse()?;
        let fx_del_amt: f32 = lines[42].parse()?;

        // Master
        let master_vol: f32 = lines[43].parse()?;

        let category = lines
            .get(Self::CATEGORY_LINE_INDEX)
            .map(|s| s.trim_matches('"').to_string())
            .unwrap_or_else(|| "Other".to_string());

        // Extended fields — absent in legacy presets (45 lines). Each has a
        // safe default so older files load unchanged.
        let parse_f32 = |n: usize, default: f32| -> f32 {
            lines.get(n).and_then(|s| s.parse().ok()).unwrap_or(default)
        };
        let lfo_waveform = lines
            .get(45)
            .map(|s| Self::string_to_lfo_waveform(s.trim_matches('"')))
            .unwrap_or(LfoWaveform::Triangle);
        let lfo_sync = lines.get(46).and_then(|s| s.parse().ok()).unwrap_or(false);
        let chorus_defaults = EffectsParams::default();
        let chorus_mix = parse_f32(47, chorus_defaults.chorus_mix);
        let chorus_rate = parse_f32(48, chorus_defaults.chorus_rate);
        let chorus_depth = parse_f32(49, chorus_defaults.chorus_depth);
        let pm_fe_freq = parse_f32(50, 0.0);
        let pm_fe_pw = parse_f32(51, 0.0);
        let pm_ob_freq = parse_f32(52, 0.0);
        let pm_ob_pw = parse_f32(53, 0.0);
        let pm_ob_cutoff = parse_f32(54, 0.0);

        Ok(Preset {
            name,
            category,
            osc1: OscillatorParams {
                wave_type: osc1_wave,
                amplitude: osc1_amp,
                detune: osc1_detune,
                pulse_width: osc1_pw,
            },
            osc2: OscillatorParams {
                wave_type: osc2_wave,
                amplitude: osc2_amp,
                detune: osc2_detune,
                pulse_width: osc2_pw,
            },
            osc2_sync,
            mixer: MixerParams {
                osc1_level: mixer_osc1,
                osc2_level: mixer_osc2,
                noise_level: mixer_noise,
            },
            filter: FilterParams {
                cutoff: filter_cutoff,
                resonance: filter_res,
                envelope_amount: filter_env,
                keyboard_tracking: filter_kbd,
            },
            filter_envelope: EnvelopeParams {
                attack: fenv_a,
                decay: fenv_d,
                sustain: fenv_s,
                release: fenv_r,
            },
            amp_envelope: EnvelopeParams {
                attack: aenv_a,
                decay: aenv_d,
                sustain: aenv_s,
                release: aenv_r,
            },
            lfo: LfoParams {
                frequency: lfo_freq,
                amplitude: lfo_amp,
                waveform: lfo_waveform,
                sync: lfo_sync,
                target_osc1_pitch: lfo_osc1,
                target_osc2_pitch: lfo_osc2,
                target_filter: lfo_filter,
                target_amplitude: lfo_amplitude,
            },
            modulation_matrix: ModulationMatrix {
                lfo_to_cutoff: mod_lfo_cut,
                lfo_to_resonance: mod_lfo_res,
                lfo_to_osc1_pitch: mod_lfo_osc1,
                lfo_to_osc2_pitch: mod_lfo_osc2,
                lfo_to_amplitude: mod_lfo_amp,
                velocity_to_cutoff: mod_vel_cut,
                velocity_to_amplitude: mod_vel_amp,
            },
            effects: EffectsParams {
                reverb_amount: fx_rev_amt,
                reverb_size: fx_rev_size,
                delay_time: fx_del_time,
                delay_feedback: fx_del_fb,
                delay_amount: fx_del_amt,
                chorus_mix,
                chorus_rate,
                chorus_depth,
            },
            master_volume: master_vol,
            poly_mod_filter_env_to_osc_a_freq: pm_fe_freq,
            poly_mod_filter_env_to_osc_a_pw: pm_fe_pw,
            poly_mod_osc_b_to_osc_a_freq: pm_ob_freq,
            poly_mod_osc_b_to_osc_a_pw: pm_ob_pw,
            poly_mod_osc_b_to_filter_cutoff: pm_ob_cutoff,
        })
    }

    fn string_to_wave_type(&self, wave_str: &str) -> Result<WaveType, Box<dyn std::error::Error>> {
        match wave_str {
            "Sine" => Ok(WaveType::Sine),
            "Square" => Ok(WaveType::Square),
            "Triangle" => Ok(WaveType::Triangle),
            "Sawtooth" => Ok(WaveType::Sawtooth),
            _ => Err(format!("Unknown wave type: {}", wave_str).into()),
        }
    }

    fn wave_type_to_string(&self, wave_type: WaveType) -> &'static str {
        match wave_type {
            WaveType::Sine => "Sine",
            WaveType::Square => "Square",
            WaveType::Triangle => "Triangle",
            WaveType::Sawtooth => "Sawtooth",
        }
    }

    fn lfo_waveform_to_string(waveform: LfoWaveform) -> &'static str {
        match waveform {
            LfoWaveform::Triangle => "Triangle",
            LfoWaveform::Square => "Square",
            LfoWaveform::Sawtooth => "Sawtooth",
            LfoWaveform::ReverseSawtooth => "ReverseSawtooth",
            LfoWaveform::SampleAndHold => "SampleAndHold",
        }
    }

    fn string_to_lfo_waveform(s: &str) -> LfoWaveform {
        // Unknown waveforms fall back to Triangle so broken or future-extended
        // presets load without crashing.
        match s {
            "Triangle" => LfoWaveform::Triangle,
            "Square" => LfoWaveform::Square,
            "Sawtooth" => LfoWaveform::Sawtooth,
            "ReverseSawtooth" => LfoWaveform::ReverseSawtooth,
            "SampleAndHold" => LfoWaveform::SampleAndHold,
            _ => LfoWaveform::Triangle,
        }
    }

    fn snapshot_preset(&self, name: &str, category: &str) -> Preset {
        Preset {
            name: name.to_string(),
            category: category.to_string(),
            osc1: self.osc1.clone(),
            osc2: self.osc2.clone(),
            osc2_sync: self.osc2_sync,
            mixer: self.mixer.clone(),
            filter: self.filter.clone(),
            filter_envelope: self.filter_envelope.clone(),
            amp_envelope: self.amp_envelope.clone(),
            lfo: self.lfo.clone(),
            modulation_matrix: self.modulation_matrix.clone(),
            effects: self.effects.clone(),
            master_volume: self.master_volume,
            poly_mod_filter_env_to_osc_a_freq: self.poly_mod_filter_env_to_osc_a_freq,
            poly_mod_filter_env_to_osc_a_pw: self.poly_mod_filter_env_to_osc_a_pw,
            poly_mod_osc_b_to_osc_a_freq: self.poly_mod_osc_b_to_osc_a_freq,
            poly_mod_osc_b_to_osc_a_pw: self.poly_mod_osc_b_to_osc_a_pw,
            poly_mod_osc_b_to_filter_cutoff: self.poly_mod_osc_b_to_filter_cutoff,
        }
    }

    fn ensure_presets_dir(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !Path::new("presets").exists() {
            fs::create_dir("presets")?;
            println!("Created presets directory");
        }
        Ok(())
    }

    /// Reset every patch-level parameter to its default. Called at the start of
    /// each built-in preset builder so poly-mod / KBD tracking / velocity routes
    /// from a previous preset don't bleed into the next one.
    fn reset_patch_to_defaults(&mut self) {
        self.osc1 = OscillatorParams::default();
        self.osc2 = OscillatorParams::default();
        self.osc2_sync = false;
        self.mixer = MixerParams::default();
        self.filter = FilterParams::default();
        self.filter_envelope = EnvelopeParams::default();
        self.amp_envelope = EnvelopeParams::default();
        self.lfo = LfoParams::default();
        self.modulation_matrix = ModulationMatrix::default();
        self.effects = EffectsParams::default();
        self.arpeggiator = ArpeggiatorParams::default();
        self.master_volume = 0.5;
        self.poly_mod_filter_env_to_osc_a_freq = 0.0;
        self.poly_mod_filter_env_to_osc_a_pw = 0.0;
        self.poly_mod_osc_b_to_osc_a_freq = 0.0;
        self.poly_mod_osc_b_to_osc_a_pw = 0.0;
        self.poly_mod_osc_b_to_filter_cutoff = 0.0;
    }

    /// Write the 32 built-in presets to disk only if they don't already exist.
    /// Callers on the startup path (main.rs) rely on this to preserve user
    /// edits to built-in preset files across launches. Use
    /// `force_create_all_classic_presets` to overwrite unconditionally.
    pub fn create_all_classic_presets(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Sentinel: if the first bundled preset is present, assume the set has
        // been written before and leave the user's `presets/` directory alone.
        if Path::new("presets/Moog_Bass.json").exists() {
            return Ok(());
        }
        self.force_create_all_classic_presets()
    }

    pub fn force_create_all_classic_presets(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Creating classic synthesizer presets...");

        // Bass Sounds
        self.create_moog_bass()?;
        self.create_acid_bass()?;
        self.create_sub_bass()?;
        self.create_wobble_bass()?;

        // Lead Sounds
        self.create_supersaw_lead()?;
        self.create_pluck_lead()?;
        self.create_screaming_lead()?;
        self.create_vintage_lead()?;

        // Pads & Strings
        self.create_warm_pad()?;
        self.create_string_ensemble()?;
        self.create_choir_pad()?;
        self.create_glass_pad()?;

        // Brass & Wind
        self.create_brass_stab()?;
        self.create_trumpet_lead()?;
        self.create_flute()?;
        self.create_sax_lead()?;

        // Effects & Special
        self.create_arp_sequence()?;
        self.create_sweep_fx()?;
        self.create_noise_sweep()?;
        self.create_zap_sound()?;

        // Vintage Analog Classics
        self.create_jump_brass()?;
        self.create_cars_lead()?;
        self.create_prophet_sync_lead()?;
        self.create_new_order_bass()?;
        self.create_berlin_school()?;
        self.create_prophet_strings()?;

        // Iconic Prophet-5 additions
        self.create_lately_bass()?;
        self.create_runaway_brass()?;
        self.create_thriller_sync_lead()?;
        self.create_poly_mod_bell()?;
        self.create_init_saw_lead()?;
        self.create_soft_pad()?;

        println!("All classic presets created successfully!");
        Ok(())
    }

    // Bass Sounds
    fn create_moog_bass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Square;
        self.osc2.amplitude = 0.5;
        self.osc2.detune = -12.0;
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.6;
        self.filter.cutoff = 800.0;
        self.filter.resonance = 2.5;
        self.filter.envelope_amount = 0.4;
        self.filter.keyboard_tracking = 0.15;
        self.filter_envelope.attack = 0.01;
        self.filter_envelope.decay = 0.3;
        self.filter_envelope.sustain = 0.3;
        self.filter_envelope.release = 0.2;
        self.amp_envelope.attack = 0.01;
        self.amp_envelope.decay = 0.1;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 0.3;
        self.modulation_matrix.velocity_to_cutoff = 0.35;
        self.modulation_matrix.velocity_to_amplitude = 0.2;
        self.save_preset_with_category("Moog Bass", "Bass")
    }

    fn create_acid_bass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 400.0;
        self.filter.resonance = 3.8;
        self.filter.envelope_amount = 0.8;
        self.filter.keyboard_tracking = 0.2;
        self.filter_envelope.attack = 0.001;
        self.filter_envelope.decay = 0.15;
        self.filter_envelope.sustain = 0.1;
        self.filter_envelope.release = 0.1;
        self.amp_envelope.attack = 0.001;
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 0.7;
        self.amp_envelope.release = 0.1;
        self.lfo.frequency = 0.5;
        self.lfo.amplitude = 0.3;
        self.lfo.target_filter = true;
        self.modulation_matrix.lfo_to_cutoff = 0.6;
        self.modulation_matrix.velocity_to_cutoff = 0.45;
        self.modulation_matrix.velocity_to_amplitude = 0.25;
        self.save_preset_with_category("Acid Bass", "Bass")
    }

    fn create_sub_bass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Square;
        self.osc1.amplitude = 1.0;
        self.osc1.detune = -24.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 150.0;
        self.filter.resonance = 0.5;
        self.filter.keyboard_tracking = 0.0;
        self.amp_envelope.attack = 0.01;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 1.0;
        self.amp_envelope.release = 0.5;
        self.modulation_matrix.velocity_to_amplitude = 0.3;
        self.save_preset_with_category("Sub Bass", "Bass")
    }

    fn create_wobble_bass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 600.0;
        self.filter.resonance = 3.0;
        self.filter.envelope_amount = 0.5;
        self.filter.keyboard_tracking = 0.1;
        self.filter_envelope.attack = 0.01;
        self.filter_envelope.decay = 0.1;
        self.filter_envelope.sustain = 0.8;
        self.filter_envelope.release = 0.2;
        self.amp_envelope.attack = 0.01;
        self.amp_envelope.decay = 0.1;
        self.amp_envelope.sustain = 1.0;
        self.amp_envelope.release = 0.2;
        self.lfo.frequency = 8.0;
        self.lfo.amplitude = 1.0;
        self.lfo.waveform = LfoWaveform::Square;
        self.lfo.target_filter = true;
        self.modulation_matrix.lfo_to_cutoff = 0.9;
        self.modulation_matrix.velocity_to_cutoff = 0.3;
        self.save_preset_with_category("Wobble Bass", "Bass")
    }

    // Lead Sounds
    fn create_supersaw_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.8;
        self.osc2.detune = 7.0;
        self.mixer.osc1_level = 0.7;
        self.mixer.osc2_level = 0.7;
        self.filter.cutoff = 8000.0;
        self.filter.resonance = 1.2;
        self.filter.envelope_amount = 0.3;
        self.filter.keyboard_tracking = 0.7;
        self.filter_envelope.attack = 0.1;
        self.filter_envelope.decay = 0.3;
        self.filter_envelope.sustain = 0.7;
        self.filter_envelope.release = 0.5;
        self.amp_envelope.attack = 0.1;
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 0.8;
        self.effects.reverb_amount = 0.3;
        self.effects.delay_amount = 0.2;
        self.effects.chorus_mix = 0.25;
        self.effects.chorus_rate = 0.8;
        self.effects.chorus_depth = 0.6;
        self.modulation_matrix.velocity_to_cutoff = 0.4;
        self.modulation_matrix.velocity_to_amplitude = 0.3;
        self.save_preset_with_category("Supersaw Lead", "Lead")
    }

    fn create_pluck_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Triangle;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 4000.0;
        self.filter.resonance = 1.5;
        self.filter.envelope_amount = 0.6;
        self.filter.keyboard_tracking = 0.8;
        self.filter_envelope.attack = 0.001;
        self.filter_envelope.decay = 0.5;
        self.filter_envelope.sustain = 0.2;
        self.filter_envelope.release = 0.3;
        self.amp_envelope.attack = 0.001;
        self.amp_envelope.decay = 0.6;
        self.amp_envelope.sustain = 0.1;
        self.amp_envelope.release = 0.3;
        self.modulation_matrix.velocity_to_cutoff = 0.5;
        self.modulation_matrix.velocity_to_amplitude = 0.35;
        self.effects.chorus_mix = 0.2;
        self.save_preset_with_category("Pluck Lead", "Lead")
    }

    fn create_screaming_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 12000.0;
        self.filter.resonance = 3.5;
        self.filter.envelope_amount = 0.4;
        self.filter.keyboard_tracking = 0.9;
        self.filter_envelope.attack = 0.2;
        self.filter_envelope.decay = 0.4;
        self.filter_envelope.sustain = 0.8;
        self.filter_envelope.release = 1.0;
        self.amp_envelope.attack = 0.1;
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 1.0;
        self.amp_envelope.release = 1.2;
        self.lfo.frequency = 5.0;
        self.lfo.amplitude = 0.3;
        self.lfo.sync = true;
        self.lfo.target_osc1_pitch = true;
        self.modulation_matrix.lfo_to_osc1_pitch = 0.4;
        self.modulation_matrix.velocity_to_cutoff = 0.45;
        self.modulation_matrix.velocity_to_amplitude = 0.3;
        self.effects.chorus_mix = 0.3;
        self.save_preset_with_category("Screaming Lead", "Lead")
    }

    fn create_vintage_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Square;
        self.osc2.amplitude = 0.6;
        self.osc2.detune = -5.0;
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.5;
        self.filter.cutoff = 6000.0;
        self.filter.resonance = 2.0;
        self.filter.envelope_amount = 0.5;
        self.filter.keyboard_tracking = 0.65;
        self.filter_envelope.attack = 0.3;
        self.filter_envelope.decay = 0.6;
        self.filter_envelope.sustain = 0.6;
        self.filter_envelope.release = 1.0;
        self.amp_envelope.attack = 0.2;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 1.5;
        self.effects.delay_amount = 0.25;
        self.effects.delay_time = 0.3;
        self.effects.chorus_mix = 0.35;
        self.effects.chorus_rate = 0.5;
        self.effects.chorus_depth = 0.55;
        self.modulation_matrix.velocity_to_cutoff = 0.4;
        self.modulation_matrix.velocity_to_amplitude = 0.25;
        self.poly_mod_osc_b_to_osc_a_freq = 0.08;
        self.save_preset_with_category("Vintage Lead", "Lead")
    }

    // Pads & Strings
    fn create_warm_pad(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Triangle;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sine;
        self.osc2.amplitude = 0.7;
        self.osc2.detune = 12.0;
        self.mixer.osc1_level = 0.6;
        self.mixer.osc2_level = 0.4;
        self.filter.cutoff = 3000.0;
        self.filter.resonance = 0.8;
        self.filter.envelope_amount = 0.2;
        self.filter.keyboard_tracking = 0.5;
        self.filter_envelope.attack = 1.5;
        self.filter_envelope.decay = 1.0;
        self.filter_envelope.sustain = 0.8;
        self.filter_envelope.release = 2.0;
        self.amp_envelope.attack = 1.8;
        self.amp_envelope.decay = 0.5;
        self.amp_envelope.sustain = 0.9;
        self.amp_envelope.release = 2.5;
        self.effects.reverb_amount = 0.6;
        self.effects.reverb_size = 0.8;
        self.effects.chorus_mix = 0.6;
        self.effects.chorus_rate = 0.35;
        self.effects.chorus_depth = 0.7;
        self.modulation_matrix.velocity_to_amplitude = 0.15;
        self.save_preset_with_category("Warm Pad", "Pad")
    }

    fn create_string_ensemble(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.8;
        self.osc2.detune = 3.0;
        self.mixer.osc1_level = 0.7;
        self.mixer.osc2_level = 0.6;
        self.filter.cutoff = 5000.0;
        self.filter.resonance = 1.0;
        self.filter.envelope_amount = 0.3;
        self.filter.keyboard_tracking = 0.55;
        self.filter_envelope.attack = 1.2;
        self.filter_envelope.decay = 0.8;
        self.filter_envelope.sustain = 0.7;
        self.filter_envelope.release = 1.8;
        self.amp_envelope.attack = 1.5;
        self.amp_envelope.decay = 0.6;
        self.amp_envelope.sustain = 0.85;
        self.amp_envelope.release = 2.0;
        self.lfo.frequency = 0.3;
        self.lfo.amplitude = 0.2;
        self.lfo.target_amplitude = true;
        self.modulation_matrix.lfo_to_amplitude = 0.15;
        self.modulation_matrix.velocity_to_amplitude = 0.15;
        self.effects.reverb_amount = 0.5;
        self.effects.chorus_mix = 0.75;
        self.effects.chorus_rate = 0.5;
        self.effects.chorus_depth = 0.8;
        self.save_preset_with_category("String Ensemble", "Strings")
    }

    fn create_choir_pad(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sine;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Triangle;
        self.osc2.amplitude = 0.6;
        self.osc2.detune = 5.0;
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.5;
        self.filter.cutoff = 4000.0;
        self.filter.resonance = 0.6;
        self.filter.envelope_amount = 0.25;
        self.filter.keyboard_tracking = 0.45;
        self.filter_envelope.attack = 2.0;
        self.filter_envelope.decay = 1.2;
        self.filter_envelope.sustain = 0.8;
        self.filter_envelope.release = 2.5;
        self.amp_envelope.attack = 2.2;
        self.amp_envelope.decay = 0.8;
        self.amp_envelope.sustain = 0.9;
        self.amp_envelope.release = 3.0;
        self.lfo.frequency = 0.2;
        self.lfo.amplitude = 0.1;
        self.lfo.target_filter = true;
        self.modulation_matrix.lfo_to_cutoff = 0.1;
        self.modulation_matrix.velocity_to_amplitude = 0.2;
        self.effects.reverb_amount = 0.8;
        self.effects.reverb_size = 0.9;
        self.effects.chorus_mix = 0.5;
        self.effects.chorus_rate = 0.25;
        self.effects.chorus_depth = 0.6;
        self.save_preset_with_category("Choir Pad", "Pad")
    }

    fn create_glass_pad(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sine;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sine;
        self.osc2.amplitude = 0.4;
        self.osc2.detune = 24.0;
        self.mixer.osc1_level = 0.7;
        self.mixer.osc2_level = 0.3;
        self.filter.cutoff = 8000.0;
        self.filter.resonance = 1.8;
        self.filter.envelope_amount = 0.4;
        self.filter.keyboard_tracking = 0.6;
        self.filter_envelope.attack = 1.8;
        self.filter_envelope.decay = 1.5;
        self.filter_envelope.sustain = 0.6;
        self.filter_envelope.release = 3.0;
        self.amp_envelope.attack = 2.0;
        self.amp_envelope.decay = 1.0;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 3.5;
        self.lfo.frequency = 0.15;
        self.lfo.amplitude = 0.3;
        self.lfo.target_osc2_pitch = true;
        self.modulation_matrix.lfo_to_osc1_pitch = 0.2;
        self.modulation_matrix.velocity_to_amplitude = 0.2;
        self.effects.reverb_amount = 0.9;
        self.effects.reverb_size = 1.0;
        self.effects.delay_amount = 0.3;
        self.effects.chorus_mix = 0.4;
        self.effects.chorus_rate = 0.3;
        self.effects.chorus_depth = 0.65;
        self.save_preset_with_category("Glass Pad", "Pad")
    }

    // Brass & Wind
    fn create_brass_stab(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Square;
        self.osc2.amplitude = 0.7;
        self.osc2.detune = -12.0;
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.6;
        self.filter.cutoff = 3000.0;
        self.filter.resonance = 2.2;
        self.filter.envelope_amount = 0.7;
        self.filter.keyboard_tracking = 0.4;
        self.filter_envelope.attack = 0.05;
        self.filter_envelope.decay = 0.2;
        self.filter_envelope.sustain = 0.4;
        self.filter_envelope.release = 0.3;
        self.amp_envelope.attack = 0.02;
        self.amp_envelope.decay = 0.1;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 0.2;
        self.modulation_matrix.velocity_to_cutoff = 0.6;
        self.modulation_matrix.velocity_to_amplitude = 0.3;
        self.poly_mod_filter_env_to_osc_a_freq = 0.12;
        self.save_preset_with_category("Brass Stab", "Brass")
    }

    fn create_trumpet_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 4500.0;
        self.filter.resonance = 1.8;
        self.filter.envelope_amount = 0.5;
        self.filter.keyboard_tracking = 0.5;
        self.filter_envelope.attack = 0.1;
        self.filter_envelope.decay = 0.4;
        self.filter_envelope.sustain = 0.7;
        self.filter_envelope.release = 0.6;
        self.amp_envelope.attack = 0.08;
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 0.85;
        self.amp_envelope.release = 0.8;
        self.lfo.frequency = 4.5;
        self.lfo.amplitude = 0.2;
        self.lfo.sync = true;
        self.lfo.target_osc1_pitch = true;
        self.modulation_matrix.lfo_to_osc1_pitch = 0.15;
        self.modulation_matrix.velocity_to_cutoff = 0.55;
        self.modulation_matrix.velocity_to_amplitude = 0.25;
        self.poly_mod_filter_env_to_osc_a_freq = 0.08;
        self.save_preset_with_category("Trumpet Lead", "Brass")
    }

    fn create_flute(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sine;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Triangle;
        self.osc2.amplitude = 0.3;
        self.osc2.detune = 12.0;
        self.mixer.osc1_level = 0.9;
        self.mixer.osc2_level = 0.2;
        self.mixer.noise_level = 0.05;
        self.filter.cutoff = 6000.0;
        self.filter.resonance = 0.8;
        self.filter.envelope_amount = 0.3;
        self.filter.keyboard_tracking = 0.5;
        self.filter_envelope.attack = 0.15;
        self.filter_envelope.decay = 0.6;
        self.filter_envelope.sustain = 0.6;
        self.filter_envelope.release = 1.0;
        self.amp_envelope.attack = 0.2;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 1.2;
        self.lfo.frequency = 0.8;
        self.lfo.amplitude = 0.1;
        self.lfo.sync = true;
        self.lfo.target_amplitude = true;
        self.modulation_matrix.lfo_to_amplitude = 0.08;
        self.modulation_matrix.velocity_to_amplitude = 0.25;
        self.effects.reverb_amount = 0.4;
        self.save_preset_with_category("Flute", "Brass")
    }

    fn create_sax_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Triangle;
        self.osc2.amplitude = 0.6;
        self.osc2.detune = -7.0;
        self.mixer.osc1_level = 0.7;
        self.mixer.osc2_level = 0.5;
        self.mixer.noise_level = 0.08;
        self.filter.cutoff = 3500.0;
        self.filter.resonance = 2.5;
        self.filter.envelope_amount = 0.6;
        self.filter.keyboard_tracking = 0.45;
        self.filter_envelope.attack = 0.12;
        self.filter_envelope.decay = 0.5;
        self.filter_envelope.sustain = 0.6;
        self.filter_envelope.release = 0.8;
        self.amp_envelope.attack = 0.1;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 0.9;
        self.amp_envelope.release = 1.0;
        self.lfo.frequency = 3.0;
        self.lfo.amplitude = 0.25;
        self.lfo.sync = true;
        self.lfo.target_osc1_pitch = true;
        self.modulation_matrix.lfo_to_osc1_pitch = 0.2;
        self.modulation_matrix.velocity_to_cutoff = 0.5;
        self.modulation_matrix.velocity_to_amplitude = 0.3;
        self.effects.reverb_amount = 0.3;
        self.poly_mod_filter_env_to_osc_a_freq = 0.06;
        self.save_preset_with_category("Sax Lead", "Brass")
    }

    // Effects & Special
    fn create_arp_sequence(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Square;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 2500.0;
        self.filter.resonance = 1.5;
        self.filter.envelope_amount = 0.5;
        self.filter.keyboard_tracking = 0.3;
        self.filter_envelope.attack = 0.001;
        self.filter_envelope.decay = 0.2;
        self.filter_envelope.sustain = 0.3;
        self.filter_envelope.release = 0.1;
        self.amp_envelope.attack = 0.001;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 0.2;
        self.amp_envelope.release = 0.1;
        self.arpeggiator.enabled = true;
        self.arpeggiator.rate = 120.0;
        self.arpeggiator.pattern = ArpPattern::Up;
        self.arpeggiator.octaves = 2;
        self.arpeggiator.gate_length = 0.7;
        self.modulation_matrix.velocity_to_cutoff = 0.35;
        self.effects.delay_amount = 0.3;
        self.effects.delay_time = 0.25;
        self.save_preset_with_category("Arp Sequence", "FX")
    }

    fn create_sweep_fx(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.9;
        self.osc2.detune = 7.0;
        self.osc2_sync = true;
        self.mixer.osc1_level = 1.0;
        self.mixer.osc2_level = 0.6;
        self.filter.cutoff = 200.0;
        self.filter.resonance = 3.5;
        self.filter.envelope_amount = 1.0;
        self.filter.keyboard_tracking = 0.3;
        self.filter_envelope.attack = 3.0;
        self.filter_envelope.decay = 2.0;
        self.filter_envelope.sustain = 0.5;
        self.filter_envelope.release = 4.0;
        self.amp_envelope.attack = 0.5;
        self.amp_envelope.decay = 1.0;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 3.0;
        // Extreme Poly Mod: osc B drives osc A pitch over a wide range.
        self.poly_mod_osc_b_to_osc_a_freq = 0.5;
        self.poly_mod_osc_b_to_filter_cutoff = 0.4;
        self.effects.reverb_amount = 0.7;
        self.effects.delay_amount = 0.4;
        self.save_preset_with_category("Sweep FX", "FX")
    }

    fn create_noise_sweep(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.mixer.noise_level = 1.0;
        self.filter.cutoff = 100.0;
        self.filter.resonance = 2.0;
        self.filter.envelope_amount = 1.0;
        self.filter_envelope.attack = 2.0;
        self.filter_envelope.decay = 3.0;
        self.filter_envelope.sustain = 0.2;
        self.filter_envelope.release = 2.0;
        self.amp_envelope.attack = 0.1;
        self.amp_envelope.decay = 2.0;
        self.amp_envelope.sustain = 0.5;
        self.amp_envelope.release = 2.5;
        self.lfo.frequency = 2.5;
        self.lfo.amplitude = 0.6;
        self.lfo.waveform = LfoWaveform::SampleAndHold;
        self.lfo.target_filter = true;
        self.modulation_matrix.lfo_to_cutoff = 0.4;
        self.effects.reverb_amount = 0.8;
        self.save_preset_with_category("Noise Sweep", "FX")
    }

    fn create_zap_sound(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Square;
        self.osc1.amplitude = 1.0;
        self.mixer.osc1_level = 1.0;
        self.mixer.noise_level = 0.3;
        self.filter.cutoff = 8000.0;
        self.filter.resonance = 3.0;
        self.filter.envelope_amount = 0.8;
        self.filter_envelope.attack = 0.001;
        self.filter_envelope.decay = 0.05;
        self.filter_envelope.sustain = 0.1;
        self.filter_envelope.release = 0.1;
        self.amp_envelope.attack = 0.001;
        self.amp_envelope.decay = 0.08;
        self.amp_envelope.sustain = 0.2;
        self.amp_envelope.release = 0.1;
        self.lfo.frequency = 15.0;
        self.lfo.amplitude = 1.0;
        self.lfo.waveform = LfoWaveform::Square;
        self.lfo.target_osc1_pitch = true;
        self.modulation_matrix.lfo_to_osc1_pitch = 0.8;
        // Poly Mod pushes the pitch-bent zap further.
        self.poly_mod_osc_b_to_osc_a_freq = 0.3;
        self.effects.delay_amount = 0.4;
        self.save_preset_with_category("Zap Sound", "FX")
    }

    // Vintage Analog Presets
    fn create_jump_brass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        // Van Halen "Jump": the Prophet brass stab defined the '80s.
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Square;
        self.osc2.amplitude = 0.8;
        self.osc2.detune = 7.0;
        self.mixer.osc1_level = 0.9;
        self.mixer.osc2_level = 0.7;
        self.filter.cutoff = 2800.0;
        self.filter.resonance = 3.2;
        self.filter.envelope_amount = 0.8;
        self.filter.keyboard_tracking = 0.45;
        self.filter_envelope.attack = 0.01;
        self.filter_envelope.decay = 0.15;
        self.filter_envelope.sustain = 0.2;
        self.filter_envelope.release = 0.1;
        self.amp_envelope.attack = 0.005;
        self.amp_envelope.decay = 0.12;
        self.amp_envelope.sustain = 0.3;
        self.amp_envelope.release = 0.15;
        self.modulation_matrix.velocity_to_cutoff = 0.65;
        self.modulation_matrix.velocity_to_amplitude = 0.3;
        // Textbook Prophet brass Poly Mod: filter envelope adds FM bite to osc A.
        self.poly_mod_filter_env_to_osc_a_freq = 0.15;
        self.save_preset_with_category("Jump Brass", "Brass")
    }

    fn create_cars_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        // Gary Numan "Cars": Prophet + osc sync, defining new-wave sound.
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.9;
        self.osc2.detune = 12.0;
        self.osc2_sync = true;
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.6;
        self.filter.cutoff = 6500.0;
        self.filter.resonance = 1.8;
        self.filter.envelope_amount = 0.4;
        self.filter.keyboard_tracking = 0.65;
        self.filter_envelope.attack = 0.08;
        self.filter_envelope.decay = 0.6;
        self.filter_envelope.sustain = 0.8;
        self.filter_envelope.release = 1.2;
        self.amp_envelope.attack = 0.05;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 0.9;
        self.amp_envelope.release = 1.0;
        self.modulation_matrix.velocity_to_cutoff = 0.4;
        self.modulation_matrix.velocity_to_amplitude = 0.2;
        self.effects.chorus_mix = 0.3;
        self.save_preset_with_category("Cars Lead", "Lead")
    }

    fn create_prophet_sync_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        // Prophet sync lead: sync + filter envelope sweep, hallmark of Prophet-5 programming.
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.8;
        self.osc2.detune = 24.0;
        self.osc2_sync = true;
        self.mixer.osc1_level = 0.7;
        self.mixer.osc2_level = 0.7;
        self.filter.cutoff = 4000.0;
        self.filter.resonance = 2.8;
        self.filter.envelope_amount = 0.5;
        self.filter.keyboard_tracking = 0.7;
        self.filter_envelope.attack = 0.3;
        self.filter_envelope.decay = 0.8;
        self.filter_envelope.sustain = 0.7;
        self.filter_envelope.release = 1.5;
        self.amp_envelope.attack = 0.2;
        self.amp_envelope.decay = 0.4;
        self.amp_envelope.sustain = 0.9;
        self.amp_envelope.release = 1.8;
        self.lfo.frequency = 0.4;
        self.lfo.amplitude = 0.6;
        self.lfo.sync = true;
        self.lfo.target_filter = true;
        self.modulation_matrix.lfo_to_cutoff = 0.7;
        self.modulation_matrix.velocity_to_cutoff = 0.4;
        // Iconic Prophet trick: filter envelope modulates osc A for the sync-sweep motion.
        self.poly_mod_filter_env_to_osc_a_freq = 0.18;
        self.effects.chorus_mix = 0.25;
        self.save_preset_with_category("Vintage Sync Lead", "Lead")
    }

    fn create_new_order_bass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        // "Blue Monday" bass: tight punch, narrow pulse.
        self.osc1.wave_type = WaveType::Square;
        self.osc1.amplitude = 1.0;
        self.osc1.pulse_width = 0.3;
        self.mixer.osc1_level = 1.0;
        self.filter.cutoff = 450.0;
        self.filter.resonance = 2.2;
        self.filter.envelope_amount = 0.6;
        self.filter.keyboard_tracking = 0.2;
        self.filter_envelope.attack = 0.005;
        self.filter_envelope.decay = 0.08;
        self.filter_envelope.sustain = 0.2;
        self.filter_envelope.release = 0.06;
        self.amp_envelope.attack = 0.001;
        self.amp_envelope.decay = 0.05;
        self.amp_envelope.sustain = 0.8;
        self.amp_envelope.release = 0.1;
        self.modulation_matrix.velocity_to_cutoff = 0.45;
        self.modulation_matrix.velocity_to_amplitude = 0.2;
        self.save_preset_with_category("New Order Bass", "Bass")
    }

    fn create_berlin_school(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        // Tangerine Dream style sequence: slow evolving filter, tight gate.
        self.osc1.wave_type = WaveType::Triangle;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.6;
        self.osc2.detune = -5.0;
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.5;
        self.filter.cutoff = 2200.0;
        self.filter.resonance = 1.5;
        self.filter.envelope_amount = 0.4;
        self.filter.keyboard_tracking = 0.4;
        self.filter_envelope.attack = 0.08;
        self.filter_envelope.decay = 0.4;
        self.filter_envelope.sustain = 0.6;
        self.filter_envelope.release = 0.3;
        self.amp_envelope.attack = 0.05;
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 0.7;
        self.amp_envelope.release = 0.4;
        self.lfo.frequency = 0.25;
        self.lfo.amplitude = 0.4;
        self.lfo.target_filter = true;
        self.modulation_matrix.lfo_to_cutoff = 0.3;
        self.modulation_matrix.velocity_to_amplitude = 0.2;
        self.effects.delay_amount = 0.2;
        self.effects.delay_time = 0.375;
        self.effects.chorus_mix = 0.3;
        self.save_preset_with_category("Berlin School", "Sequence")
    }

    fn create_prophet_strings(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.8;
        self.osc2.detune = 2.5;
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.7;
        self.filter.cutoff = 4500.0;
        self.filter.resonance = 1.2;
        self.filter.envelope_amount = 0.3;
        self.filter.keyboard_tracking = 0.55;
        self.filter_envelope.attack = 1.8;
        self.filter_envelope.decay = 1.2;
        self.filter_envelope.sustain = 0.8;
        self.filter_envelope.release = 2.5;
        self.amp_envelope.attack = 2.0;
        self.amp_envelope.decay = 0.8;
        self.amp_envelope.sustain = 0.95;
        self.amp_envelope.release = 3.0;
        self.lfo.frequency = 0.3;
        self.lfo.amplitude = 0.15;
        self.lfo.target_osc2_pitch = true;
        self.modulation_matrix.lfo_to_osc2_pitch = 0.1;
        self.modulation_matrix.velocity_to_amplitude = 0.15;
        self.effects.reverb_amount = 0.7;
        self.effects.reverb_size = 0.9;
        self.effects.delay_amount = 0.15;
        self.effects.delay_time = 0.4;
        self.effects.chorus_mix = 0.7;
        self.effects.chorus_rate = 0.45;
        self.effects.chorus_depth = 0.75;
        self.save_preset_with_category("Vintage Strings", "Strings")
    }

    // ─── Iconic Prophet-5 additions ────────────────────────────────────────

    /// Stevie Wonder "Lately": deep Prophet bass with pronounced filter envelope.
    fn create_lately_bass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.9;
        self.osc2.detune = -12.0;
        self.mixer.osc1_level = 0.85;
        self.mixer.osc2_level = 0.75;
        self.filter.cutoff = 550.0;
        self.filter.resonance = 2.0;
        self.filter.envelope_amount = 0.75;
        self.filter.keyboard_tracking = 0.15;
        self.filter_envelope.attack = 0.005;
        self.filter_envelope.decay = 0.4;
        self.filter_envelope.sustain = 0.25;
        self.filter_envelope.release = 0.25;
        self.amp_envelope.attack = 0.005;
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 0.85;
        self.amp_envelope.release = 0.35;
        self.modulation_matrix.velocity_to_cutoff = 0.4;
        self.modulation_matrix.velocity_to_amplitude = 0.2;
        self.save_preset_with_category("Lately Bass", "Bass")
    }

    /// Bon Jovi "Runaway": Prophet brass with gentle Poly Mod animation.
    fn create_runaway_brass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.7;
        self.osc2.detune = -7.0;
        self.mixer.osc1_level = 0.9;
        self.mixer.osc2_level = 0.6;
        self.filter.cutoff = 2600.0;
        self.filter.resonance = 2.4;
        self.filter.envelope_amount = 0.85;
        self.filter.keyboard_tracking = 0.5;
        self.filter_envelope.attack = 0.02;
        self.filter_envelope.decay = 0.35;
        self.filter_envelope.sustain = 0.35;
        self.filter_envelope.release = 0.3;
        self.amp_envelope.attack = 0.01;
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 0.75;
        self.amp_envelope.release = 0.3;
        self.modulation_matrix.velocity_to_cutoff = 0.6;
        self.modulation_matrix.velocity_to_amplitude = 0.3;
        // Classic Prophet brass Poly Mod recipe: gentle osc B into osc A freq
        // plus filter envelope into osc A pitch for the tell-tale wobble.
        self.poly_mod_osc_b_to_osc_a_freq = 0.1;
        self.poly_mod_filter_env_to_osc_a_freq = 0.1;
        self.effects.chorus_mix = 0.15;
        self.save_preset_with_category("Runaway Brass", "Brass")
    }

    /// Thriller-era sync lead: osc sync + sweeping filter envelope.
    fn create_thriller_sync_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 1.0;
        self.osc2.detune = 19.0; // Octave + fifth for an aggressive sync pitch.
        self.osc2_sync = true;
        self.mixer.osc1_level = 0.6;
        self.mixer.osc2_level = 0.8;
        self.filter.cutoff = 3200.0;
        self.filter.resonance = 2.6;
        self.filter.envelope_amount = 0.7;
        self.filter.keyboard_tracking = 0.75;
        self.filter_envelope.attack = 0.05;
        self.filter_envelope.decay = 0.9;
        self.filter_envelope.sustain = 0.55;
        self.filter_envelope.release = 1.1;
        self.amp_envelope.attack = 0.02;
        self.amp_envelope.decay = 0.3;
        self.amp_envelope.sustain = 0.85;
        self.amp_envelope.release = 0.8;
        self.lfo.frequency = 5.5;
        self.lfo.amplitude = 0.2;
        self.lfo.sync = true;
        self.lfo.target_osc1_pitch = true;
        self.modulation_matrix.lfo_to_osc1_pitch = 0.12;
        self.modulation_matrix.velocity_to_cutoff = 0.55;
        self.modulation_matrix.velocity_to_amplitude = 0.3;
        // Filter envelope modulates osc A freq: sync pitch sweep that defines the lead.
        self.poly_mod_filter_env_to_osc_a_freq = 0.22;
        self.effects.chorus_mix = 0.25;
        self.save_preset_with_category("Thriller Sync Lead", "Lead")
    }

    /// Metallic FM bell using Poly Mod osc B → osc A frequency at high depth.
    fn create_poly_mod_bell(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sine;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sine;
        self.osc2.amplitude = 0.9;
        self.osc2.detune = 14.0; // Inharmonic ratio → metallic timbre under FM.
        self.mixer.osc1_level = 1.0;
        self.mixer.osc2_level = 0.0; // Osc B is purely a modulator here.
        self.filter.cutoff = 9000.0;
        self.filter.resonance = 0.6;
        self.filter.envelope_amount = 0.15;
        self.filter.keyboard_tracking = 0.7;
        self.filter_envelope.attack = 0.001;
        self.filter_envelope.decay = 1.5;
        self.filter_envelope.sustain = 0.0;
        self.filter_envelope.release = 1.2;
        self.amp_envelope.attack = 0.001;
        self.amp_envelope.decay = 2.0;
        self.amp_envelope.sustain = 0.0;
        self.amp_envelope.release = 1.5;
        self.modulation_matrix.velocity_to_cutoff = 0.3;
        self.modulation_matrix.velocity_to_amplitude = 0.45; // Bells want dynamic strike response.
        // The defining move: deep osc B → osc A FM, modulated by the filter envelope
        // so the bright "strike" transient decays into a cleaner sine.
        self.poly_mod_osc_b_to_osc_a_freq = 0.55;
        self.poly_mod_filter_env_to_osc_a_freq = 0.3;
        self.effects.reverb_amount = 0.7;
        self.effects.reverb_size = 0.9;
        self.save_preset_with_category("Poly Mod Bell", "FX")
    }

    /// "Init" Prophet-5: fat sawtooth with detune + chorus, the default starting point.
    fn create_init_saw_lead(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 1.0;
        self.osc2.detune = 4.0;
        self.mixer.osc1_level = 0.8;
        self.mixer.osc2_level = 0.8;
        self.filter.cutoff = 5500.0;
        self.filter.resonance = 1.0;
        self.filter.envelope_amount = 0.35;
        self.filter.keyboard_tracking = 0.65;
        self.filter_envelope.attack = 0.05;
        self.filter_envelope.decay = 0.4;
        self.filter_envelope.sustain = 0.7;
        self.filter_envelope.release = 0.6;
        self.amp_envelope.attack = 0.02;
        self.amp_envelope.decay = 0.2;
        self.amp_envelope.sustain = 0.9;
        self.amp_envelope.release = 0.5;
        self.modulation_matrix.velocity_to_cutoff = 0.35;
        self.modulation_matrix.velocity_to_amplitude = 0.25;
        self.effects.chorus_mix = 0.4;
        self.effects.chorus_rate = 0.55;
        self.effects.chorus_depth = 0.6;
        self.save_preset_with_category("Init Saw Lead", "Lead")
    }

    /// Soft Prophet-5 pad: two gently detuned saws, heavy chorus and reverb.
    fn create_soft_pad(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_patch_to_defaults();
        self.osc1.wave_type = WaveType::Sawtooth;
        self.osc1.amplitude = 1.0;
        self.osc2.wave_type = WaveType::Sawtooth;
        self.osc2.amplitude = 0.9;
        self.osc2.detune = 3.5;
        self.mixer.osc1_level = 0.75;
        self.mixer.osc2_level = 0.75;
        self.filter.cutoff = 2600.0;
        self.filter.resonance = 0.7;
        self.filter.envelope_amount = 0.15;
        self.filter.keyboard_tracking = 0.55;
        self.filter_envelope.attack = 2.0;
        self.filter_envelope.decay = 1.2;
        self.filter_envelope.sustain = 0.75;
        self.filter_envelope.release = 3.0;
        self.amp_envelope.attack = 2.2;
        self.amp_envelope.decay = 0.8;
        self.amp_envelope.sustain = 0.9;
        self.amp_envelope.release = 3.5;
        self.lfo.frequency = 0.35;
        self.lfo.amplitude = 0.15;
        self.lfo.target_osc2_pitch = true;
        self.modulation_matrix.lfo_to_osc2_pitch = 0.08;
        self.modulation_matrix.velocity_to_amplitude = 0.15;
        self.effects.reverb_amount = 0.7;
        self.effects.reverb_size = 0.9;
        self.effects.chorus_mix = 0.75;
        self.effects.chorus_rate = 0.4;
        self.effects.chorus_depth = 0.7;
        self.save_preset_with_category("Prophet Soft Pad", "Pad")
    }

    /// Extract current parameters as a flat SynthParameters struct
    pub fn to_synth_params(&self) -> crate::lock_free::SynthParameters {
        crate::lock_free::SynthParameters {
            osc1_waveform: Self::wave_type_to_u8_pub(self.osc1.wave_type),
            osc2_waveform: Self::wave_type_to_u8_pub(self.osc2.wave_type),
            osc1_level: self.osc1.amplitude,
            osc2_level: self.osc2.amplitude,
            osc1_detune: self.osc1.detune,
            osc2_detune: self.osc2.detune,
            osc1_pulse_width: self.osc1.pulse_width,
            osc2_pulse_width: self.osc2.pulse_width,
            osc2_sync: self.osc2_sync,
            mixer_osc1_level: self.mixer.osc1_level,
            mixer_osc2_level: self.mixer.osc2_level,
            noise_level: self.mixer.noise_level,
            filter_cutoff: self.filter.cutoff,
            filter_resonance: self.filter.resonance,
            filter_envelope_amount: self.filter.envelope_amount,
            filter_keyboard_tracking: self.filter.keyboard_tracking,
            amp_attack: self.amp_envelope.attack,
            amp_decay: self.amp_envelope.decay,
            amp_sustain: self.amp_envelope.sustain,
            amp_release: self.amp_envelope.release,
            filter_attack: self.filter_envelope.attack,
            filter_decay: self.filter_envelope.decay,
            filter_sustain: self.filter_envelope.sustain,
            filter_release: self.filter_envelope.release,
            lfo_rate: self.lfo.frequency,
            lfo_amount: self.lfo.amplitude,
            lfo_waveform: Self::lfo_waveform_to_u8_pub(self.lfo.waveform),
            lfo_sync: self.lfo.sync,
            lfo_target_osc1_pitch: self.lfo.target_osc1_pitch,
            lfo_target_osc2_pitch: self.lfo.target_osc2_pitch,
            lfo_target_filter: self.lfo.target_filter,
            lfo_target_amplitude: self.lfo.target_amplitude,
            lfo_to_cutoff: self.modulation_matrix.lfo_to_cutoff,
            lfo_to_resonance: self.modulation_matrix.lfo_to_resonance,
            lfo_to_osc1_pitch: self.modulation_matrix.lfo_to_osc1_pitch,
            lfo_to_osc2_pitch: self.modulation_matrix.lfo_to_osc2_pitch,
            lfo_to_amplitude: self.modulation_matrix.lfo_to_amplitude,
            velocity_to_cutoff: self.modulation_matrix.velocity_to_cutoff,
            velocity_to_amplitude: self.modulation_matrix.velocity_to_amplitude,
            reverb_amount: self.effects.reverb_amount,
            reverb_size: self.effects.reverb_size,
            delay_time: self.effects.delay_time,
            delay_feedback: self.effects.delay_feedback,
            delay_amount: self.effects.delay_amount,
            arp_enabled: self.arpeggiator.enabled,
            arp_rate: self.arpeggiator.rate,
            arp_pattern: Self::arp_pattern_to_u8_pub(self.arpeggiator.pattern),
            arp_octaves: self.arpeggiator.octaves,
            arp_gate_length: self.arpeggiator.gate_length,
            master_volume: self.master_volume,
            poly_mod_filter_env_to_osc_a_freq: self.poly_mod_filter_env_to_osc_a_freq,
            poly_mod_filter_env_to_osc_a_pw: self.poly_mod_filter_env_to_osc_a_pw,
            poly_mod_osc_b_to_osc_a_freq: self.poly_mod_osc_b_to_osc_a_freq,
            poly_mod_osc_b_to_osc_a_pw: self.poly_mod_osc_b_to_osc_a_pw,
            poly_mod_osc_b_to_filter_cutoff: self.poly_mod_osc_b_to_filter_cutoff,
            glide_time: self.glide_time,
            pitch_bend: self.pitch_bend,
            pitch_bend_range: self.pitch_bend_range,
            aftertouch: self.aftertouch,
            aftertouch_to_cutoff: self.aftertouch_to_cutoff,
            aftertouch_to_amplitude: self.aftertouch_to_amplitude,
            expression: self.expression,
            mod_wheel: self.mod_wheel,
            velocity_curve: self.velocity_curve,
            voice_mode: match self.voice_mode {
                VoiceMode::Poly => 0,
                VoiceMode::Mono => 1,
                VoiceMode::Legato => 2,
                VoiceMode::Unison => 3,
            },
            note_priority: match self.note_priority {
                NotePriority::Last => 0,
                NotePriority::Low => 1,
                NotePriority::High => 2,
            },
            unison_spread: self.unison_spread,
            max_voices: self.max_polyphony as u8,
            arp_sync_to_midi: self.arp_sync_to_midi,
            lfo_delay: self.lfo_delay,
            chorus_mix: self.effects.chorus_mix,
            chorus_rate: self.effects.chorus_rate,
            chorus_depth: self.effects.chorus_depth,
            analog_component_tolerance: self.analog.component_tolerance,
            analog_filter_drift: self.analog.filter_drift_amount,
            analog_vca_bleed: self.analog.vca_bleed,
            analog_noise_floor: self.analog.noise_floor,
            stereo_spread: self.stereo_spread,
            // reference_tone is GUI-only state, not part of the synthesizer engine
            reference_tone: false,
            tuning_mode: self.tuning_mode,
            oversampling: self.oversampling,
        }
    }

    /// Apply flat SynthParameters to the synthesizer's nested structures
    /// Does NOT touch voice state, buffers, or LFO phase
    pub fn apply_params(&mut self, params: &crate::lock_free::SynthParameters) {
        self.osc1.wave_type = Self::u8_to_wave_type_pub(params.osc1_waveform);
        self.osc2.wave_type = Self::u8_to_wave_type_pub(params.osc2_waveform);
        self.osc1.amplitude = params.osc1_level;
        self.osc2.amplitude = params.osc2_level;
        self.osc1.detune = params.osc1_detune;
        self.osc2.detune = params.osc2_detune;
        self.osc1.pulse_width = params.osc1_pulse_width;
        self.osc2.pulse_width = params.osc2_pulse_width;
        self.osc2_sync = params.osc2_sync;
        self.mixer.osc1_level = params.mixer_osc1_level;
        self.mixer.osc2_level = params.mixer_osc2_level;
        self.mixer.noise_level = params.noise_level;
        self.filter.cutoff = params.filter_cutoff;
        self.filter.resonance = params.filter_resonance;
        self.filter.envelope_amount = params.filter_envelope_amount;
        self.filter.keyboard_tracking = params.filter_keyboard_tracking;
        self.amp_envelope.attack = params.amp_attack;
        self.amp_envelope.decay = params.amp_decay;
        self.amp_envelope.sustain = params.amp_sustain;
        self.amp_envelope.release = params.amp_release;
        self.filter_envelope.attack = params.filter_attack;
        self.filter_envelope.decay = params.filter_decay;
        self.filter_envelope.sustain = params.filter_sustain;
        self.filter_envelope.release = params.filter_release;
        self.lfo.frequency = params.lfo_rate;
        self.lfo.amplitude = params.lfo_amount;
        self.lfo.waveform = Self::u8_to_lfo_waveform_pub(params.lfo_waveform);
        self.lfo.sync = params.lfo_sync;
        self.lfo.target_osc1_pitch = params.lfo_target_osc1_pitch;
        self.lfo.target_osc2_pitch = params.lfo_target_osc2_pitch;
        self.lfo.target_filter = params.lfo_target_filter;
        self.lfo.target_amplitude = params.lfo_target_amplitude;
        self.modulation_matrix.lfo_to_cutoff = params.lfo_to_cutoff;
        self.modulation_matrix.lfo_to_resonance = params.lfo_to_resonance;
        self.modulation_matrix.lfo_to_osc1_pitch = params.lfo_to_osc1_pitch;
        self.modulation_matrix.lfo_to_osc2_pitch = params.lfo_to_osc2_pitch;
        self.modulation_matrix.lfo_to_amplitude = params.lfo_to_amplitude;
        self.modulation_matrix.velocity_to_cutoff = params.velocity_to_cutoff;
        self.modulation_matrix.velocity_to_amplitude = params.velocity_to_amplitude;
        self.effects.reverb_amount = params.reverb_amount;
        self.effects.reverb_size = params.reverb_size;
        self.effects.delay_time = params.delay_time;
        self.effects.delay_feedback = params.delay_feedback;
        self.effects.delay_amount = params.delay_amount;
        self.arpeggiator.enabled = params.arp_enabled;
        self.arpeggiator.rate = params.arp_rate;
        self.arpeggiator.pattern = Self::u8_to_arp_pattern_pub(params.arp_pattern);
        self.arpeggiator.octaves = params.arp_octaves;
        self.arpeggiator.gate_length = params.arp_gate_length;
        self.master_volume = params.master_volume;
        self.poly_mod_filter_env_to_osc_a_freq = params.poly_mod_filter_env_to_osc_a_freq;
        self.poly_mod_filter_env_to_osc_a_pw = params.poly_mod_filter_env_to_osc_a_pw;
        self.poly_mod_osc_b_to_osc_a_freq = params.poly_mod_osc_b_to_osc_a_freq;
        self.poly_mod_osc_b_to_osc_a_pw = params.poly_mod_osc_b_to_osc_a_pw;
        self.poly_mod_osc_b_to_filter_cutoff = params.poly_mod_osc_b_to_filter_cutoff;
        self.glide_time = params.glide_time;
        self.pitch_bend = params.pitch_bend;
        self.pitch_bend_range = params.pitch_bend_range;
        self.aftertouch = params.aftertouch;
        self.aftertouch_to_cutoff = params.aftertouch_to_cutoff;
        self.aftertouch_to_amplitude = params.aftertouch_to_amplitude;
        self.expression = params.expression;
        self.mod_wheel = params.mod_wheel;
        self.velocity_curve = params.velocity_curve;
        self.voice_mode = match params.voice_mode {
            1 => VoiceMode::Mono,
            2 => VoiceMode::Legato,
            3 => VoiceMode::Unison,
            _ => VoiceMode::Poly,
        };
        self.note_priority = match params.note_priority {
            1 => NotePriority::Low,
            2 => NotePriority::High,
            _ => NotePriority::Last,
        };
        self.unison_spread = params.unison_spread;
        self.max_polyphony = params.max_voices as usize;
        self.arp_sync_to_midi = params.arp_sync_to_midi;
        self.lfo_delay = params.lfo_delay;
        self.effects.chorus_mix = params.chorus_mix;
        self.effects.chorus_rate = params.chorus_rate;
        self.effects.chorus_depth = params.chorus_depth;
        self.analog.component_tolerance = params.analog_component_tolerance;
        self.analog.filter_drift_amount = params.analog_filter_drift;
        self.analog.vca_bleed = params.analog_vca_bleed;
        self.analog.noise_floor = params.analog_noise_floor;
        self.stereo_spread = params.stereo_spread;
        self.tuning_mode = params.tuning_mode;
        self.oversampling = params.oversampling.max(1);
    }

    pub fn wave_type_to_u8_pub(wt: WaveType) -> u8 {
        match wt {
            WaveType::Sine => 0,
            WaveType::Square => 1,
            WaveType::Triangle => 2,
            WaveType::Sawtooth => 3,
        }
    }

    pub fn u8_to_wave_type_pub(v: u8) -> WaveType {
        match v {
            0 => WaveType::Sine,
            1 => WaveType::Square,
            2 => WaveType::Triangle,
            3 => WaveType::Sawtooth,
            _ => WaveType::Sawtooth,
        }
    }

    pub fn lfo_waveform_to_u8_pub(wf: LfoWaveform) -> u8 {
        match wf {
            LfoWaveform::Triangle => 0,
            LfoWaveform::Square => 1,
            LfoWaveform::Sawtooth => 2,
            LfoWaveform::ReverseSawtooth => 3,
            LfoWaveform::SampleAndHold => 4,
        }
    }

    pub fn u8_to_lfo_waveform_pub(v: u8) -> LfoWaveform {
        match v {
            0 => LfoWaveform::Triangle,
            1 => LfoWaveform::Square,
            2 => LfoWaveform::Sawtooth,
            3 => LfoWaveform::ReverseSawtooth,
            4 => LfoWaveform::SampleAndHold,
            _ => LfoWaveform::Triangle,
        }
    }

    pub fn arp_pattern_to_u8_pub(p: ArpPattern) -> u8 {
        match p {
            ArpPattern::Up => 0,
            ArpPattern::Down => 1,
            ArpPattern::UpDown => 2,
            ArpPattern::Random => 3,
        }
    }

    pub fn u8_to_arp_pattern_pub(v: u8) -> ArpPattern {
        match v {
            0 => ArpPattern::Up,
            1 => ArpPattern::Down,
            2 => ArpPattern::UpDown,
            3 => ArpPattern::Random,
            _ => ArpPattern::Up,
        }
    }

    // 2nd-order Butterworth LP at fc=sample_rate/4 (for 2× oversampling).
    // Coefficients: b=[0.2929, 0.5858, 0.2929], a2=0.1716, a1=0.0 (symmetric).
    // Direct Form II Transposed.
    #[inline(always)]
    fn decimate_biquad(x: f32, x1: &mut f32, x2: &mut f32, y1: &mut f32, y2: &mut f32) -> f32 {
        const B0: f32 = 0.2929;
        const B1: f32 = 0.5858;
        const B2: f32 = 0.2929;
        const A2: f32 = 0.1716;
        // a1 = 0.0, so omitted
        let y = B0 * x + B1 * *x1 + B2 * *x2 - A2 * *y2;
        *x2 = *x1;
        *x1 = x;
        *y2 = *y1;
        *y1 = y;
        y
    }

    pub fn process_block_oversampled(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        over_left: &mut Vec<f32>,
        over_right: &mut Vec<f32>,
    ) {
        let factor = self.oversampling as usize;
        let frames = left.len();

        if factor <= 1 {
            self.process_block(left, right);
            return;
        }

        let inner = frames * factor;
        if over_left.len() < inner {
            over_left.resize(inner, 0.0);
            over_right.resize(inner, 0.0);
        }

        // Synthesize at N× sample rate
        let orig_rate = self.sample_rate;
        self.sample_rate = orig_rate * factor as f32;
        self.process_block(&mut over_left[..inner], &mut over_right[..inner]);
        self.sample_rate = orig_rate;

        // Decimate: apply biquad LP filter and pick every factor-th sample
        for i in 0..inner {
            let l = over_left[i];
            let r = over_right[i];

            let lf = Self::decimate_biquad(
                l,
                &mut self.decim_x1_l,
                &mut self.decim_x2_l,
                &mut self.decim_y1_l,
                &mut self.decim_y2_l,
            );
            let rf = Self::decimate_biquad(
                r,
                &mut self.decim_x1_r,
                &mut self.decim_x2_r,
                &mut self.decim_y1_r,
                &mut self.decim_y2_r,
            );

            let (lf, rf) = if factor == 4 {
                let l2 = Self::decimate_biquad(
                    lf,
                    &mut self.decim2_x1_l,
                    &mut self.decim2_x2_l,
                    &mut self.decim2_y1_l,
                    &mut self.decim2_y2_l,
                );
                let r2 = Self::decimate_biquad(
                    rf,
                    &mut self.decim2_x1_r,
                    &mut self.decim2_x2_r,
                    &mut self.decim2_y1_r,
                    &mut self.decim2_y2_r,
                );
                (l2, r2)
            } else {
                (lf, rf)
            };

            if i % factor == factor - 1 {
                let out = i / factor;
                left[out] = lf;
                right[out] = rf;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_to_synth_params_roundtrip() {
        let synth = Synthesizer::new();
        let params = synth.to_synth_params();
        assert_eq!(params.master_volume, synth.master_volume);
        assert_eq!(params.filter_cutoff, synth.filter.cutoff);
        assert_eq!(params.filter_resonance, synth.filter.resonance);
        assert_eq!(params.osc1_detune, synth.osc1.detune);
        assert_eq!(params.amp_attack, synth.amp_envelope.attack);
        assert_eq!(params.lfo_rate, synth.lfo.frequency);
        assert_eq!(params.reverb_amount, synth.effects.reverb_amount);
        assert!(!params.arp_enabled);
    }

    #[test]
    fn test_apply_params_updates_synthesizer() {
        let mut synth = Synthesizer::new();
        let mut params = synth.to_synth_params();
        params.master_volume = 0.9;
        params.filter_cutoff = 5000.0;
        params.osc1_waveform = 0; // Sine
        params.arp_enabled = true;
        params.arp_rate = 180.0;
        params.lfo_sync = true;
        params.delay_amount = 0.5;
        synth.apply_params(&params);
        assert_eq!(synth.master_volume, 0.9);
        assert_eq!(synth.filter.cutoff, 5000.0);
        assert_eq!(synth.osc1.wave_type, WaveType::Sine);
        assert!(synth.arpeggiator.enabled);
        assert_eq!(synth.arpeggiator.rate, 180.0);
        assert!(synth.lfo.sync);
        assert_eq!(synth.effects.delay_amount, 0.5);
    }

    #[test]
    fn test_apply_params_preserves_voice_state() {
        let mut synth = Synthesizer::new();
        synth.note_on(60, 100);
        assert!(!synth.voices.is_empty());
        let params = synth.to_synth_params();
        synth.apply_params(&params);
        assert!(!synth.voices.is_empty());
        assert!(synth.voices.iter().any(|v| v.note == 60 && v.is_active));
    }

    #[test]
    fn test_note_to_frequency_matches_optimization_table() {
        let freq = Synthesizer::note_to_frequency(69);
        assert!((freq - 440.0).abs() < 0.01);
        let freq = Synthesizer::note_to_frequency(60);
        assert!((freq - 261.63).abs() < 0.1);
    }

    #[test]
    fn test_tuning_et_a4_is_440() {
        let f = Synthesizer::note_to_frequency_tuned(69, 0);
        assert!((f - 440.0).abs() < 0.01, "ET A4={}", f);
    }

    #[test]
    fn test_tuning_ji_a4_is_440() {
        let f = Synthesizer::note_to_frequency_tuned(69, 1);
        assert!((f - 440.0).abs() < 0.01, "JI A4={}", f);
    }

    #[test]
    fn test_tuning_pythagorean_a4_is_440() {
        let f = Synthesizer::note_to_frequency_tuned(69, 2);
        assert!((f - 440.0).abs() < 0.01, "Pythagorean A4={}", f);
    }

    #[test]
    fn test_tuning_werckmeister_a4_is_440() {
        let f = Synthesizer::note_to_frequency_tuned(69, 3);
        assert!((f - 440.0).abs() < 0.01, "Werckmeister A4={}", f);
    }

    #[test]
    fn test_tuning_octave_invariant() {
        // A4 (69) and A3 (57) should be 2:1 ratio in all tuning modes
        for mode in 0..4 {
            let a4 = Synthesizer::note_to_frequency_tuned(69, mode);
            let a3 = Synthesizer::note_to_frequency_tuned(57, mode);
            assert!(
                (a4 / a3 - 2.0).abs() < 0.001,
                "mode {}: A4/A3 should be 2.0, got {}",
                mode,
                a4 / a3
            );
        }
    }

    #[test]
    fn test_oscillator_sine_output_range() {
        let dt = 440.0 / 48000.0;
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let output = Synthesizer::generate_oscillator_static(WaveType::Sine, phase, dt, 0.5);
            assert!(
                (-1.01..=1.01).contains(&output),
                "Sine at phase {} = {}",
                phase,
                output
            );
        }
    }

    #[test]
    fn test_oscillator_sawtooth_output_range() {
        let dt = 440.0 / 48000.0;
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let output =
                Synthesizer::generate_oscillator_static(WaveType::Sawtooth, phase, dt, 0.5);
            assert!(
                (-1.5..=1.5).contains(&output),
                "Saw at phase {} = {}",
                phase,
                output
            );
        }
    }

    #[test]
    fn test_audio_callback_flow_produces_sound() {
        // Simulate exactly what the audio callback does
        let mut synth = Synthesizer::new();
        synth.sample_rate = 48000.0;

        // Step 1: Trigger a note (like draining MidiEventQueue)
        synth.note_on(60, 100); // C4, velocity 100

        // Step 2: Apply default params (like reading from TripleBuffer)
        let params = crate::lock_free::SynthParameters::default();
        synth.apply_params(&params);

        // Step 3: Process audio blocks (like process_block in callback)
        let mut buf_l = vec![0.0f32; 512];
        let mut buf_r = vec![0.0f32; 512];
        let mut max_peak = 0.0f32;

        // Process 20 blocks (about 213ms at 48kHz) to get past the attack phase
        for _ in 0..20 {
            for s in buf_l.iter_mut() {
                *s = 0.0;
            }
            for s in buf_r.iter_mut() {
                *s = 0.0;
            }
            synth.process_block(&mut buf_l, &mut buf_r);
            let peak = buf_l
                .iter()
                .chain(buf_r.iter())
                .fold(0.0f32, |max, &s| max.max(s.abs()));
            max_peak = max_peak.max(peak);
        }

        println!(
            "Max peak after 20 blocks with apply_params: {:.6}",
            max_peak
        );
        assert!(
            max_peak > 0.01,
            "Audio output should be audible, got peak={}",
            max_peak
        );

        // Also test WITHOUT apply_params (simulating old code behavior)
        let mut synth_old = Synthesizer::new();
        synth_old.sample_rate = 48000.0;
        synth_old.note_on(60, 100);
        // NO apply_params call - this is what the old code did

        let mut buf_l_old = vec![0.0f32; 512];
        let mut buf_r_old = vec![0.0f32; 512];
        let mut max_peak_old = 0.0f32;
        for _ in 0..20 {
            for s in buf_l_old.iter_mut() {
                *s = 0.0;
            }
            for s in buf_r_old.iter_mut() {
                *s = 0.0;
            }
            synth_old.process_block(&mut buf_l_old, &mut buf_r_old);
            let peak = buf_l_old
                .iter()
                .chain(buf_r_old.iter())
                .fold(0.0f32, |max, &s| max.max(s.abs()));
            max_peak_old = max_peak_old.max(peak);
        }

        println!(
            "Max peak after 20 blocks WITHOUT apply_params: {:.6}",
            max_peak_old
        );
        assert!(
            max_peak_old > 0.01,
            "Old code path should produce sound, got peak={}",
            max_peak_old
        );

        // Compare: new code should produce similar levels to old code
        let ratio = max_peak / max_peak_old;
        println!("New/Old ratio: {:.4}", ratio);
        assert!(
            ratio > 0.5,
            "New code should produce at least 50% of old code level, ratio={}",
            ratio
        );
    }

    // ─── P3 Analog Character ───────────────────────────────────────────────

    #[test]
    fn test_voice_tolerance_within_expected_window() {
        // Sample 200 voices and verify the per-voice tolerance stays inside the
        // documented ±2 % / ±3 % windows. Also verify there's actual variation
        // (standard deviation > 0), otherwise the randomness is broken.
        let mut cutoffs = Vec::with_capacity(200);
        let mut resonances = Vec::with_capacity(200);
        for _ in 0..200 {
            let v = Voice::new(60, 440.0, 1.0);
            cutoffs.push(v.tolerance_cutoff_mul);
            resonances.push(v.tolerance_res_mul);
            assert!(
                (0.98..=1.02).contains(&v.tolerance_cutoff_mul),
                "cutoff tolerance out of ±2%: {}",
                v.tolerance_cutoff_mul
            );
            assert!(
                (0.97..=1.03).contains(&v.tolerance_res_mul),
                "resonance tolerance out of ±3%: {}",
                v.tolerance_res_mul
            );
        }
        // At 200 samples drawn from a ±2 % window, stddev is ≈ 0.012.
        // Anything ≪ 0.005 means the RNG is effectively constant — bug.
        let mean_c: f32 = cutoffs.iter().sum::<f32>() / cutoffs.len() as f32;
        let var_c: f32 =
            cutoffs.iter().map(|c| (c - mean_c).powi(2)).sum::<f32>() / cutoffs.len() as f32;
        assert!(
            var_c.sqrt() > 0.005,
            "cutoff tolerance stddev too small: {}",
            var_c.sqrt()
        );
    }

    #[test]
    fn test_voice_filter_drift_initial_state() {
        for _ in 0..50 {
            let v = Voice::new(60, 440.0, 1.0);
            assert_eq!(v.filter_drift_value, 1.0);
            assert!(
                (0.97..=1.03).contains(&v.filter_drift_target),
                "drift target out of ±3%: {}",
                v.filter_drift_target
            );
            assert!(
                (FILTER_DRIFT_MIN_PERIOD..=FILTER_DRIFT_MAX_PERIOD).contains(&v.filter_drift_timer),
                "drift timer out of [{}, {}]: {}",
                FILTER_DRIFT_MIN_PERIOD,
                FILTER_DRIFT_MAX_PERIOD,
                v.filter_drift_timer
            );
        }
    }

    #[test]
    fn test_chorus_buffer_is_power_of_two() {
        // Power-of-two is load-bearing: apply_chorus uses `& mask` wrap.
        let synth = Synthesizer::new();
        let len = synth.effects_chain.chorus_buffer.len();
        assert!(
            len.is_power_of_two(),
            "chorus_buffer len must be power of two, got {}",
            len
        );
        // 25 ms at 44.1 kHz is 1102.5 → next power of two is 2048.
        assert!(len >= 1024, "chorus_buffer too small: {}", len);
    }

    #[test]
    fn test_chorus_bypassed_is_transparent() {
        // With mix = 0, the chorus must pass the dry signal through unchanged
        // while still feeding its ring buffer (so a later enable doesn't click).
        let mut synth = Synthesizer::new();
        synth.effects.chorus_mix = 0.0;
        let input_samples = [0.3_f32, -0.5, 0.1, 0.8, -0.2];
        for &s in &input_samples {
            let out = synth
                .effects_chain
                .apply_chorus(s, &synth.effects, synth.sample_rate);
            assert_eq!(
                out, s,
                "chorus bypassed must not alter signal: in={} out={}",
                s, out
            );
        }
        // Ring buffer must still have advanced.
        assert_eq!(synth.effects_chain.chorus_index, input_samples.len());
    }

    #[test]
    fn test_chorus_active_modifies_signal() {
        // With mix > 0 and a primed buffer, the wet tap must produce a different
        // output than the dry input on a stationary non-zero signal.
        let mut synth = Synthesizer::new();
        synth.effects.chorus_mix = 0.8;
        synth.effects.chorus_depth = 1.0;
        // Prime the buffer with a ramp so the delayed taps read a value distinct
        // from the current input.
        for i in 0..2000 {
            let s = ((i as f32) * 0.01).sin();
            synth
                .effects_chain
                .apply_chorus(s, &synth.effects, synth.sample_rate);
        }
        let same_input = 0.5_f32;
        let out = synth
            .effects_chain
            .apply_chorus(same_input, &synth.effects, synth.sample_rate);
        assert!(
            (out - same_input).abs() > 0.001,
            "chorus with mix=0.8 should alter signal: in={} out={}",
            same_input,
            out
        );
    }

    #[test]
    fn test_noise_floor_zero_produces_silence_with_no_voices() {
        // No active voices + noise_floor = 0 ⇒ absolute silence.
        let mut synth = Synthesizer::new();
        synth.analog.noise_floor = 0.0;
        // Also disable VCA bleed (bleed*0 envelope doesn't emit because voice inactive).
        let mut buf_l = vec![0.0_f32; 512];
        let mut buf_r = vec![0.0_f32; 512];
        synth.process_block(&mut buf_l, &mut buf_r);
        let peak = buf_l
            .iter()
            .chain(buf_r.iter())
            .fold(0.0_f32, |m, &s| m.max(s.abs()));
        assert!(peak < 1e-6, "silence expected, got peak={}", peak);
    }

    #[test]
    fn test_noise_floor_produces_audible_background() {
        // No voices but noise_floor > 0 should put hiss on the master bus.
        let mut synth = Synthesizer::new();
        synth.analog.noise_floor = 0.005;
        let mut buf_l = vec![0.0_f32; 2048];
        let mut buf_r = vec![0.0_f32; 2048];
        synth.process_block(&mut buf_l, &mut buf_r);
        let all: Vec<f32> = buf_l.iter().chain(buf_r.iter()).copied().collect();
        let rms = (all.iter().map(|s| s * s).sum::<f32>() / all.len() as f32).sqrt();
        assert!(
            rms > 1e-5,
            "noise floor should produce audible hiss, rms={}",
            rms
        );
    }

    #[test]
    fn test_apply_preset_overwrites_chorus_from_preset() {
        // Chorus is now part of the serialized preset format. apply_preset must
        // replace the current chorus settings with those stored in the preset,
        // instead of preserving the user's state as the legacy workaround did.
        let mut synth = Synthesizer::new();
        synth.effects.chorus_mix = 0.7;
        synth.effects.chorus_rate = 1.5;
        synth.effects.chorus_depth = 0.4;

        let incoming_effects = EffectsParams {
            chorus_mix: 0.15,
            chorus_rate: 0.8,
            chorus_depth: 0.9,
            ..EffectsParams::default()
        };

        let preset = Preset {
            name: "fake".into(),
            category: "Other".into(),
            osc1: OscillatorParams::default(),
            osc2: OscillatorParams::default(),
            osc2_sync: false,
            mixer: MixerParams::default(),
            filter: FilterParams::default(),
            filter_envelope: EnvelopeParams::default(),
            amp_envelope: EnvelopeParams::default(),
            lfo: LfoParams::default(),
            modulation_matrix: ModulationMatrix::default(),
            effects: incoming_effects,
            master_volume: 0.5,
            poly_mod_filter_env_to_osc_a_freq: 0.0,
            poly_mod_filter_env_to_osc_a_pw: 0.0,
            poly_mod_osc_b_to_osc_a_freq: 0.0,
            poly_mod_osc_b_to_osc_a_pw: 0.0,
            poly_mod_osc_b_to_filter_cutoff: 0.0,
        };
        synth.apply_preset(preset);

        assert_eq!(synth.effects.chorus_mix, 0.15);
        assert_eq!(synth.effects.chorus_rate, 0.8);
        assert_eq!(synth.effects.chorus_depth, 0.9);
        assert_eq!(synth.effects.reverb_amount, 0.0);
        assert_eq!(synth.master_volume, 0.5);
    }

    #[test]
    fn test_apply_params_propagates_analog_character() {
        let mut synth = Synthesizer::new();
        let mut params = synth.to_synth_params();
        params.chorus_mix = 0.42;
        params.chorus_rate = 1.23;
        params.chorus_depth = 0.77;
        params.analog_component_tolerance = 0.6;
        params.analog_filter_drift = 0.5;
        params.analog_vca_bleed = 0.003;
        params.analog_noise_floor = 0.001;
        synth.apply_params(&params);
        assert_eq!(synth.effects.chorus_mix, 0.42);
        assert_eq!(synth.effects.chorus_rate, 1.23);
        assert_eq!(synth.effects.chorus_depth, 0.77);
        assert_eq!(synth.analog.component_tolerance, 0.6);
        assert_eq!(synth.analog.filter_drift_amount, 0.5);
        assert_eq!(synth.analog.vca_bleed, 0.003);
        assert_eq!(synth.analog.noise_floor, 0.001);
    }

    #[test]
    fn test_analog_character_defaults_are_subtle_not_zero() {
        // Defaults must be non-zero (the synth should sound "alive" out of the
        // box) but also subtle enough to never be the dominant sound.
        let a = AnalogCharacter::default();
        assert!(a.component_tolerance > 0.0 && a.component_tolerance < 0.5);
        assert!(a.filter_drift_amount > 0.0 && a.filter_drift_amount < 0.5);
        assert!(a.vca_bleed > 0.0 && a.vca_bleed < 0.01);
        assert!(a.noise_floor > 0.0 && a.noise_floor < 0.01);
    }

    // ─── Voice lifecycle (pre-existing features, previously untested) ──────

    #[test]
    fn test_note_off_triggers_release_state() {
        let mut synth = Synthesizer::new();
        synth.note_on(60, 100);
        assert!(synth.voices.iter().any(|v| v.note == 60 && v.is_active));
        synth.note_off(60);
        let v = synth.voices.iter().find(|v| v.note == 60).unwrap();
        assert_eq!(v.envelope_state, EnvelopeState::Release);
    }

    #[test]
    fn test_all_notes_off_silences_every_voice() {
        let mut synth = Synthesizer::new();
        for note in [60, 64, 67, 72] {
            synth.note_on(note, 100);
        }
        assert!(synth.voices.iter().filter(|v| v.is_active).count() >= 4);
        synth.all_notes_off();
        assert!(synth.voices.iter().all(|v| !v.is_active));
        assert!(synth.note_stack.is_empty());
        assert!(synth.held_notes.is_empty());
        assert!(!synth.sustain_held);
    }

    #[test]
    fn test_sustain_pedal_keeps_note_active_after_release() {
        let mut synth = Synthesizer::new();
        synth.sustain_pedal(true);
        synth.note_on(60, 100);
        synth.note_off(60);
        // With sustain held, the voice must still be active and not yet releasing.
        let v = synth.voices.iter().find(|v| v.note == 60).unwrap();
        assert!(v.is_active);
        assert!(v.is_sustained);
        // Releasing the pedal must drop the sustained voices into Release.
        synth.sustain_pedal(false);
        let v = synth.voices.iter().find(|v| v.note == 60).unwrap();
        assert_eq!(v.envelope_state, EnvelopeState::Release);
    }

    // ─── LFO waveform generation ───────────────────────────────────────────

    #[test]
    fn test_lfo_triangle_range_and_endpoints() {
        // At phase=0.0 triangle = -1, at phase=0.5 triangle = +1, at phase=1.0 wraps to -1
        let at_zero = Synthesizer::generate_lfo_waveform(LfoWaveform::Triangle, 0.0, 0.0);
        let at_half = Synthesizer::generate_lfo_waveform(LfoWaveform::Triangle, 0.5, 0.0);
        assert!(
            (at_zero - (-1.0)).abs() < 0.01,
            "triangle at 0 should be -1, got {}",
            at_zero
        );
        assert!(
            (at_half - 1.0).abs() < 0.01,
            "triangle at 0.5 should be +1, got {}",
            at_half
        );
        // All values must be in [-1, 1]
        for i in 0..100 {
            let v =
                Synthesizer::generate_lfo_waveform(LfoWaveform::Triangle, i as f32 / 100.0, 0.0);
            assert!(
                (-1.0..=1.0).contains(&v),
                "triangle out of range at phase {}: {}",
                i,
                v
            );
        }
    }

    #[test]
    fn test_lfo_square_only_two_values() {
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let v = Synthesizer::generate_lfo_waveform(LfoWaveform::Square, phase, 0.0);
            assert!(
                v == -1.0 || v == 1.0,
                "square must be ±1, got {} at phase {}",
                v,
                phase
            );
        }
        // First half is -1, second half is +1
        assert_eq!(
            Synthesizer::generate_lfo_waveform(LfoWaveform::Square, 0.0, 0.0),
            -1.0
        );
        assert_eq!(
            Synthesizer::generate_lfo_waveform(LfoWaveform::Square, 0.75, 0.0),
            1.0
        );
    }

    #[test]
    fn test_lfo_sawtooth_range_and_direction() {
        let at_zero = Synthesizer::generate_lfo_waveform(LfoWaveform::Sawtooth, 0.0, 0.0);
        let at_one = Synthesizer::generate_lfo_waveform(LfoWaveform::Sawtooth, 0.9999, 0.0);
        // Rises from -1 to +1
        assert!(
            (at_zero - (-1.0)).abs() < 0.01,
            "saw at 0 should be ~-1, got {}",
            at_zero
        );
        assert!(
            at_one > 0.9,
            "saw near 1.0 should be close to +1, got {}",
            at_one
        );
        // Monotonically increasing within one period
        let mut prev = f32::MIN;
        for i in 0..100 {
            let v =
                Synthesizer::generate_lfo_waveform(LfoWaveform::Sawtooth, i as f32 / 100.0, 0.0);
            assert!(v >= prev, "sawtooth should be monotone increasing");
            prev = v;
        }
    }

    #[test]
    fn test_lfo_reverse_sawtooth_range_and_direction() {
        let at_zero = Synthesizer::generate_lfo_waveform(LfoWaveform::ReverseSawtooth, 0.0, 0.0);
        let at_one = Synthesizer::generate_lfo_waveform(LfoWaveform::ReverseSawtooth, 0.9999, 0.0);
        assert!(
            (at_zero - 1.0).abs() < 0.01,
            "rsaw at 0 should be +1, got {}",
            at_zero
        );
        assert!(
            at_one < -0.9,
            "rsaw near 1.0 should be close to -1, got {}",
            at_one
        );
    }

    #[test]
    fn test_lfo_sample_and_hold_returns_held_value() {
        let held = 0.42_f32;
        for i in 0..10 {
            let v = Synthesizer::generate_lfo_waveform(
                LfoWaveform::SampleAndHold,
                i as f32 / 10.0,
                held,
            );
            assert_eq!(v, held, "S&H must return the held value unchanged");
        }
    }

    // ─── Oscillator waveform range ─────────────────────────────────────────

    #[test]
    fn test_oscillator_triangle_output_range() {
        let dt = 440.0 / 44100.0;
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let out = Synthesizer::generate_oscillator_static(WaveType::Triangle, phase, dt, 0.5);
            assert!(
                (-1.1..=1.1).contains(&out),
                "triangle oscillator out of range at phase {}: {}",
                phase,
                out
            );
        }
    }

    #[test]
    fn test_oscillator_square_output_range() {
        let dt = 440.0 / 44100.0;
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            let out = Synthesizer::generate_oscillator_static(WaveType::Square, phase, dt, 0.5);
            assert!(
                (-1.5..=1.5).contains(&out),
                "square oscillator out of range at phase {}: {}",
                phase,
                out
            );
        }
    }

    // ─── Velocity curves ───────────────────────────────────────────────────

    #[test]
    fn test_velocity_curve_linear() {
        // Curve 0: linear — v / 127
        let out = Synthesizer::apply_velocity_curve(127, 0);
        assert!((out - 1.0).abs() < 0.01);
        let out = Synthesizer::apply_velocity_curve(0, 0);
        assert_eq!(out, 0.0);
        let out = Synthesizer::apply_velocity_curve(64, 0);
        assert!((out - 64.0 / 127.0).abs() < 0.01);
    }

    #[test]
    fn test_velocity_curve_soft() {
        // Curve 1: sqrt — more sensitive at low velocities
        let linear = Synthesizer::apply_velocity_curve(64, 0);
        let soft = Synthesizer::apply_velocity_curve(64, 1);
        assert!(
            soft > linear,
            "soft curve should boost low velocities: linear={} soft={}",
            linear,
            soft
        );
        assert!((Synthesizer::apply_velocity_curve(127, 1) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_velocity_curve_hard() {
        // Curve 2: squared — requires stronger playing
        let linear = Synthesizer::apply_velocity_curve(64, 0);
        let hard = Synthesizer::apply_velocity_curve(64, 2);
        assert!(
            hard < linear,
            "hard curve should reduce mid velocities: linear={} hard={}",
            linear,
            hard
        );
        assert!((Synthesizer::apply_velocity_curve(127, 2) - 1.0).abs() < 0.01);
    }

    // ─── Enum round-trip conversions ───────────────────────────────────────

    #[test]
    fn test_wave_type_roundtrip() {
        for wt in [
            WaveType::Sine,
            WaveType::Square,
            WaveType::Triangle,
            WaveType::Sawtooth,
        ] {
            let u = Synthesizer::wave_type_to_u8_pub(wt);
            assert_eq!(Synthesizer::u8_to_wave_type_pub(u), wt);
        }
        // Out-of-range falls back to Sawtooth
        assert_eq!(Synthesizer::u8_to_wave_type_pub(99), WaveType::Sawtooth);
    }

    #[test]
    fn test_lfo_waveform_roundtrip() {
        for wf in [
            LfoWaveform::Triangle,
            LfoWaveform::Square,
            LfoWaveform::Sawtooth,
            LfoWaveform::ReverseSawtooth,
            LfoWaveform::SampleAndHold,
        ] {
            let u = Synthesizer::lfo_waveform_to_u8_pub(wf);
            assert_eq!(Synthesizer::u8_to_lfo_waveform_pub(u), wf);
        }
        // Out-of-range falls back to Triangle
        assert_eq!(
            Synthesizer::u8_to_lfo_waveform_pub(99),
            LfoWaveform::Triangle
        );
    }

    #[test]
    fn test_arp_pattern_roundtrip() {
        for p in [
            ArpPattern::Up,
            ArpPattern::Down,
            ArpPattern::UpDown,
            ArpPattern::Random,
        ] {
            let u = Synthesizer::arp_pattern_to_u8_pub(p);
            assert_eq!(Synthesizer::u8_to_arp_pattern_pub(u), p);
        }
        // Out-of-range falls back to Up
        assert_eq!(Synthesizer::u8_to_arp_pattern_pub(99), ArpPattern::Up);
    }

    // ─── Voice modes ───────────────────────────────────────────────────────

    #[test]
    fn test_mono_mode_single_voice() {
        let mut synth = Synthesizer::new();
        synth.voice_mode = VoiceMode::Mono;
        synth.note_on(60, 100);
        synth.note_on(64, 100);
        synth.note_on(67, 100);
        // Mono must never exceed one active voice
        let active = synth.voices.iter().filter(|v| v.is_active).count();
        assert_eq!(
            active, 1,
            "mono mode should have exactly 1 active voice, got {}",
            active
        );
    }

    #[test]
    fn test_mono_mode_note_off_returns_to_previous() {
        let mut synth = Synthesizer::new();
        synth.voice_mode = VoiceMode::Mono;
        synth.note_on(60, 100);
        synth.note_on(64, 100); // 64 is now playing
        synth.note_off(64); // should return to 60
        let active: Vec<_> = synth.voices.iter().filter(|v| v.is_active).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(
            active[0].note, 60,
            "mono should return to note 60 after releasing 64"
        );
    }

    #[test]
    fn test_unison_mode_all_voices_same_note() {
        let mut synth = Synthesizer::new();
        synth.voice_mode = VoiceMode::Unison;
        synth.note_on(60, 100);
        let active: Vec<_> = synth.voices.iter().filter(|v| v.is_active).collect();
        assert!(active.len() > 1, "unison should activate multiple voices");
        // All voices must have the same MIDI note number
        for v in &active {
            assert_eq!(v.note, 60, "unison voice has wrong note: {}", v.note);
        }
    }

    #[test]
    fn test_unison_mode_voices_have_different_frequencies() {
        let mut synth = Synthesizer::new();
        synth.voice_mode = VoiceMode::Unison;
        synth.unison_spread = 20.0; // 20 cents spread
        synth.note_on(60, 100);
        let freqs: Vec<f32> = synth
            .voices
            .iter()
            .filter(|v| v.is_active)
            .map(|v| v.frequency)
            .collect();
        assert!(freqs.len() >= 2);
        // At least some voices must differ in frequency
        let all_same = freqs.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01);
        assert!(
            !all_same,
            "unison voices should have different detuned frequencies"
        );
    }

    // ─── Note priority in Mono mode ────────────────────────────────────────

    #[test]
    fn test_mono_note_priority_low() {
        let mut synth = Synthesizer::new();
        synth.voice_mode = VoiceMode::Mono;
        synth.note_priority = NotePriority::Low;
        synth.note_on(72, 100); // high note
        synth.note_on(60, 100); // low note — should take priority
        let active: Vec<_> = synth.voices.iter().filter(|v| v.is_active).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(
            active[0].note, 60,
            "Low priority should play the lowest note"
        );
    }

    #[test]
    fn test_mono_note_priority_high() {
        let mut synth = Synthesizer::new();
        synth.voice_mode = VoiceMode::Mono;
        synth.note_priority = NotePriority::High;
        synth.note_on(60, 100); // low note
        synth.note_on(72, 100); // high note — should take priority
        let active: Vec<_> = synth.voices.iter().filter(|v| v.is_active).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(
            active[0].note, 72,
            "High priority should play the highest note"
        );
    }

    // ─── Polyphony limit & voice stealing ─────────────────────────────────

    #[test]
    fn test_polyphony_limit_not_exceeded() {
        let mut synth = Synthesizer::new();
        // Play more notes than max_polyphony
        for note in 36..=(36 + synth.max_polyphony as u8 + 2) {
            synth.note_on(note, 100);
        }
        let active = synth.voices.iter().filter(|v| v.is_active).count();
        assert!(
            active <= synth.max_polyphony,
            "active voices {} must not exceed max_polyphony {}",
            active,
            synth.max_polyphony
        );
    }

    // ─── Voice::release state machine ─────────────────────────────────────

    #[test]
    fn test_voice_release_from_idle_stays_idle() {
        let mut voice = Voice::new(60, 440.0, 1.0);
        voice.envelope_state = EnvelopeState::Idle;
        voice.filter_envelope_state = EnvelopeState::Idle;
        voice.release();
        assert_eq!(voice.envelope_state, EnvelopeState::Idle);
        assert_eq!(voice.filter_envelope_state, EnvelopeState::Idle);
    }

    #[test]
    fn test_voice_release_resets_envelope_time() {
        let mut voice = Voice::new(60, 440.0, 1.0);
        voice.envelope_state = EnvelopeState::Sustain;
        voice.envelope_time = 1.5; // simulated time in sustain
        voice.release();
        assert_eq!(voice.envelope_state, EnvelopeState::Release);
        assert_eq!(
            voice.envelope_time, 0.0,
            "release should reset envelope_time to 0"
        );
    }

    #[test]
    fn test_voice_release_or_sustain_with_pedal_held() {
        let mut voice = Voice::new(60, 440.0, 1.0);
        voice.envelope_state = EnvelopeState::Sustain;
        voice.release_or_sustain(true); // pedal held
        assert!(voice.is_sustained);
        assert_eq!(
            voice.envelope_state,
            EnvelopeState::Sustain,
            "should not release when pedal held"
        );
    }

    #[test]
    fn test_voice_release_or_sustain_without_pedal_releases() {
        let mut voice = Voice::new(60, 440.0, 1.0);
        voice.envelope_state = EnvelopeState::Sustain;
        voice.release_or_sustain(false);
        assert_eq!(voice.envelope_state, EnvelopeState::Release);
        assert!(!voice.is_sustained);
    }

    // ─── Pitch bend ────────────────────────────────────────────────────────

    #[test]
    fn test_pitch_bend_affects_audio_output() {
        // With pitch_bend = 0 and pitch_bend = 1 the audio must differ.
        let mut synth_no_bend = Synthesizer::new();
        synth_no_bend.note_on(60, 100);
        synth_no_bend.pitch_bend = 0.0;
        let mut buf_no_bend_l = vec![0.0f32; 256];
        let mut buf_no_bend_r = vec![0.0f32; 256];
        synth_no_bend.process_block(&mut buf_no_bend_l, &mut buf_no_bend_r);

        let mut synth_bent = Synthesizer::new();
        synth_bent.note_on(60, 100);
        synth_bent.pitch_bend = 1.0; // full bend up
        let mut buf_bent_l = vec![0.0f32; 256];
        let mut buf_bent_r = vec![0.0f32; 256];
        synth_bent.process_block(&mut buf_bent_l, &mut buf_bent_r);

        let diff: f32 = buf_no_bend_l
            .iter()
            .zip(&buf_bent_l)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            + buf_no_bend_r
                .iter()
                .zip(&buf_bent_r)
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>();
        assert!(
            diff > 0.001,
            "pitch bend should alter audio output, diff={}",
            diff
        );
    }

    // ─── MIDI clock ────────────────────────────────────────────────────────

    #[test]
    fn test_midi_clock_tick_calculates_bpm() {
        let mut synth = Synthesizer::new();
        synth.arp_sync_to_midi = true;
        synth.midi_clock_running = true;
        // 120 BPM = 0.5 s per quarter note → dt per tick = 0.5 / 24 ≈ 0.020833 s
        let dt = 0.5_f32 / 24.0;
        for _ in 0..24 {
            synth.midi_clock_tick(dt);
        }
        assert!(
            (synth.midi_clock_bpm - 120.0).abs() < 1.0,
            "MIDI clock should compute ~120 BPM, got {}",
            synth.midi_clock_bpm
        );
    }

    #[test]
    fn test_midi_clock_tick_noop_when_disabled() {
        let mut synth = Synthesizer::new();
        synth.arp_sync_to_midi = false;
        synth.midi_clock_running = false;
        let initial_bpm = synth.midi_clock_bpm;
        for _ in 0..24 {
            synth.midi_clock_tick(0.02);
        }
        assert_eq!(
            synth.midi_clock_bpm, initial_bpm,
            "disabled clock should not update BPM"
        );
    }

    // ─── Arpeggiator in Poly mode ──────────────────────────────────────────

    #[test]
    fn test_arp_mode_accumulates_held_notes() {
        let mut synth = Synthesizer::new();
        synth.arpeggiator.enabled = true;
        synth.note_on(60, 100);
        synth.note_on(64, 100);
        synth.note_on(67, 100);
        assert_eq!(
            synth.held_notes.len(),
            3,
            "arpeggiator should collect 3 held notes"
        );
        assert!(synth.held_notes.contains(&60));
        assert!(synth.held_notes.contains(&64));
        assert!(synth.held_notes.contains(&67));
    }

    #[test]
    fn test_arp_note_off_removes_from_held() {
        let mut synth = Synthesizer::new();
        synth.arpeggiator.enabled = true;
        synth.note_on(60, 100);
        synth.note_on(64, 100);
        synth.note_off(60);
        assert!(
            !synth.held_notes.contains(&60),
            "note_off should remove note from held_notes"
        );
        assert!(synth.held_notes.contains(&64));
    }

    // ─── Semitones to ratio ────────────────────────────────────────────────

    #[test]
    fn test_semitones_to_ratio_octave() {
        // 12 semitones = 2× frequency
        let ratio = Synthesizer::semitones_to_ratio(12.0);
        assert!(
            (ratio - 2.0).abs() < 0.001,
            "12 semitones should give ratio 2.0, got {}",
            ratio
        );
    }

    #[test]
    fn test_semitones_to_ratio_zero() {
        let ratio = Synthesizer::semitones_to_ratio(0.0);
        assert!(
            (ratio - 1.0).abs() < 0.001,
            "0 semitones should give ratio 1.0, got {}",
            ratio
        );
    }

    #[test]
    fn test_semitones_to_ratio_negative() {
        let ratio = Synthesizer::semitones_to_ratio(-12.0);
        assert!(
            (ratio - 0.5).abs() < 0.001,
            "-12 semitones should give ratio 0.5, got {}",
            ratio
        );
    }

    // ─── fast_tanh ─────────────────────────────────────────────────────────

    #[test]
    fn test_fast_tanh_zero() {
        assert_eq!(Synthesizer::fast_tanh(0.0), 0.0);
    }

    #[test]
    fn test_fast_tanh_clamps_beyond_three() {
        // The Padé approximant's domain is |x| ≤ 3; outside it hard-clamps to ±1.
        assert_eq!(
            Synthesizer::fast_tanh(3.1),
            1.0,
            "fast_tanh(3.1) must be 1.0"
        );
        assert_eq!(
            Synthesizer::fast_tanh(10.0),
            1.0,
            "fast_tanh(10.0) must be 1.0"
        );
        assert_eq!(
            Synthesizer::fast_tanh(-3.1),
            -1.0,
            "fast_tanh(-3.1) must be -1.0"
        );
        assert_eq!(
            Synthesizer::fast_tanh(-10.0),
            -1.0,
            "fast_tanh(-10.0) must be -1.0"
        );
    }

    #[test]
    fn test_fast_tanh_is_odd_function() {
        // tanh is an odd function: f(-x) = -f(x).
        // Any deviation indicates a broken symmetry in the approximant.
        for i in 1..=29 {
            let x = i as f32 * 0.1; // 0.1 .. 2.9 — all within the Padé domain
            let pos = Synthesizer::fast_tanh(x);
            let neg = Synthesizer::fast_tanh(-x);
            assert!(
                (pos + neg).abs() < 1e-6,
                "fast_tanh not symmetric at x={:.1}: f(x)={} f(-x)={}",
                x,
                pos,
                neg
            );
        }
    }

    #[test]
    fn test_fast_tanh_approximates_identity_near_zero() {
        // For very small |x| the filter is in its linear region: tanh(x) ≈ x.
        // The Padé must stay within 0.1 % of x itself here, otherwise the filter
        // would color quiet signals with unwanted harmonic distortion.
        for i in -5..=5 {
            let x = i as f32 * 0.01; // -0.05 .. 0.05, step 0.01
            let approx = Synthesizer::fast_tanh(x);
            if x != 0.0 {
                let relative_error = (approx - x).abs() / x.abs();
                assert!(
                    relative_error < 0.001,
                    "fast_tanh deviates from identity at x={:.2}: approx={:.6} rel_err={:.4}",
                    x,
                    approx,
                    relative_error
                );
            }
        }
    }

    #[test]
    fn test_fast_tanh_is_monotone_increasing() {
        // The saturation function must never decrease — a non-monotone tanh would introduce
        // aliasing artifacts and invert the filter's feedback at some amplitudes.
        let mut prev = f32::MIN;
        for i in -30..=30 {
            let x = i as f32 * 0.1;
            let v = Synthesizer::fast_tanh(x);
            assert!(
                v >= prev,
                "fast_tanh not monotone at x={:.1}: prev={} current={}",
                x,
                prev,
                v
            );
            prev = v;
        }
    }

    #[test]
    fn test_fast_tanh_bounded() {
        // Output must always be in [-1, 1] — the filter relies on this to prevent feedback runaway.
        for i in -100..=100 {
            let x = i as f32 * 0.1;
            let v = Synthesizer::fast_tanh(x);
            assert!(
                (-1.0..=1.0).contains(&v),
                "fast_tanh out of bounds at x={}: {}",
                x,
                v
            );
        }
    }

    // ─── Moog ladder filter ────────────────────────────────────────────────

    #[test]
    fn test_ladder_filter_attenuates_above_cutoff() {
        // Feed a 5 kHz sine into the filter with low (200 Hz) vs wide-open (18 kHz) cutoff.
        // A 24 dB/octave filter should attenuate 5 kHz by more than 20 dB (factor 10) at 200 Hz.
        let sample_rate = 44100.0;
        let freq = 5000.0_f32;
        let resonance = 0.1;
        let n_samples = 4096;
        let skip = 512; // let transient settle

        let mut state_low = LadderFilterState {
            stage1: 0.0,
            stage2: 0.0,
            stage3: 0.0,
            stage4: 0.0,
        };
        let mut state_high = LadderFilterState {
            stage1: 0.0,
            stage2: 0.0,
            stage3: 0.0,
            stage4: 0.0,
        };
        let mut energy_low = 0.0_f32;
        let mut energy_high = 0.0_f32;

        for i in 0..n_samples {
            let input = (2.0 * PI * freq * i as f32 / sample_rate).sin();
            let out_low = Synthesizer::apply_ladder_filter_static(
                input,
                &mut state_low,
                200.0,
                resonance,
                sample_rate,
            );
            let out_high = Synthesizer::apply_ladder_filter_static(
                input,
                &mut state_high,
                18000.0,
                resonance,
                sample_rate,
            );
            if i >= skip {
                energy_low += out_low * out_low;
                energy_high += out_high * out_high;
            }
        }

        let rms_low = (energy_low / (n_samples - skip) as f32).sqrt();
        let rms_high = (energy_high / (n_samples - skip) as f32).sqrt();
        assert!(
            rms_low < rms_high * 0.1,
            "200 Hz cutoff should attenuate 5 kHz by >20 dB vs 18 kHz: rms_low={:.4} rms_high={:.4}",
            rms_low,
            rms_high
        );
    }

    #[test]
    fn test_ladder_filter_passes_signal_at_high_cutoff() {
        // With cutoff wide open, a low-amplitude signal (well inside the tanh linear region)
        // at 440 Hz must pass through mostly unattenuated.
        // Using amplitude 0.05 keeps all 4 tanh stages in their linear region (tanh(x)≈x for |x|≪1).
        let sample_rate = 44100.0;
        let mut state = LadderFilterState {
            stage1: 0.0,
            stage2: 0.0,
            stage3: 0.0,
            stage4: 0.0,
        };
        let n_samples = 2048;
        let skip = 256;
        let amplitude = 0.05_f32;
        let mut energy_in = 0.0_f32;
        let mut energy_out = 0.0_f32;

        for i in 0..n_samples {
            let input = amplitude * (2.0 * PI * 440.0 * i as f32 / sample_rate).sin();
            let output = Synthesizer::apply_ladder_filter_static(
                input,
                &mut state,
                18000.0,
                0.0,
                sample_rate,
            );
            if i >= skip {
                energy_in += input * input;
                energy_out += output * output;
            }
        }
        let ratio = (energy_out / energy_in).sqrt();
        assert!(
            ratio > 0.85,
            "with cutoff=18 kHz and small amplitude, 440 Hz should pass through (ratio={:.3})",
            ratio
        );
    }

    // ─── Voice stealing ────────────────────────────────────────────────────

    #[test]
    fn test_voice_steal_prefers_release_state() {
        let mut synth = Synthesizer::new();
        for note in 36_u8..44 {
            synth.note_on(note, 100);
        }
        // Manually put voice 3 into Release — it should score highest (100 pts base).
        synth.voices[3].envelope_state = EnvelopeState::Release;
        synth.voices[3].envelope_value = 0.5;
        // All other voices stay in Attack (score ≈ 50 pts max from amplitude).
        for i in [0, 1, 2, 4, 5, 6, 7] {
            synth.voices[i].envelope_state = EnvelopeState::Attack;
            synth.voices[i].envelope_value = 0.9; // nearly full — low "quiet" score
        }

        let stolen = synth.find_voice_to_steal();
        assert_eq!(
            stolen, 3,
            "voice stealing should pick the Release-state voice (index 3), got {}",
            stolen
        );
    }

    #[test]
    fn test_voice_steal_among_same_state_picks_quietest() {
        let mut synth = Synthesizer::new();
        for note in 36_u8..44 {
            synth.note_on(note, 100);
        }
        // All voices in Sustain but with different amplitudes.
        for i in 0..8 {
            synth.voices[i].envelope_state = EnvelopeState::Sustain;
            synth.voices[i].envelope_value = 0.9;
            synth.voices[i].envelope_time = 0.0;
        }
        // Voice 5 is the quietest (value close to 0) — should be stolen.
        synth.voices[5].envelope_value = 0.05;

        let stolen = synth.find_voice_to_steal();
        assert_eq!(
            stolen, 5,
            "among Sustain voices, the quietest (index 5) should be stolen, got {}",
            stolen
        );
    }

    // ─── Legato envelope non-retrigger ─────────────────────────────────────

    #[test]
    fn test_legato_second_note_does_not_retrigger_envelope() {
        let mut synth = Synthesizer::new();
        synth.voice_mode = VoiceMode::Legato;
        // Instant attack so we leave Attack quickly.
        synth.amp_envelope.attack = 0.001;
        synth.filter_envelope.attack = 0.001;

        synth.note_on(60, 100);
        // Process enough samples to leave Attack and enter Decay/Sustain.
        // 1024 samples @ 44100 Hz ≈ 23 ms >> 1 ms attack.
        let mut buf_l = vec![0.0f32; 1024];
        let mut buf_r = vec![0.0f32; 1024];
        synth.process_block(&mut buf_l, &mut buf_r);

        let state_before = synth
            .voices
            .iter()
            .find(|v| v.is_active)
            .map(|v| v.envelope_state)
            .expect("must have an active voice after note_on");
        assert_ne!(
            state_before,
            EnvelopeState::Attack,
            "After 23ms with 1ms attack, should have left Attack (got {:?})",
            state_before
        );

        // Second note — legato should NOT reset the envelope.
        synth.note_on(64, 100);

        let state_after = synth
            .voices
            .iter()
            .find(|v| v.is_active)
            .map(|v| v.envelope_state)
            .expect("must still have an active voice");
        assert_ne!(
            state_after,
            EnvelopeState::Attack,
            "Legato second note must not retrigger envelope to Attack (got {:?})",
            state_after
        );
        // Note must have changed to the new pitch.
        assert!(
            synth.voices.iter().any(|v| v.is_active && v.note == 64),
            "Active voice should now be playing note 64"
        );
    }

    // ─── Preset JSON round-trip ────────────────────────────────────────────

    #[test]
    fn test_preset_json_legacy_45_lines_loads_with_defaults() {
        // Synthetic legacy preset: exactly 45 lines (up to and including category).
        // Must load successfully and fall back to defaults for the new extended fields.
        let legacy = r#""LegacyPatch"
"Sawtooth"
1.0
0.0
0.5
"Square"
0.5
-7.0
0.5
false
0.8
0.4
0.0
2200
1.8
0.3
0.0
0.01
0.2
0.6
0.3
0.01
0.1
0.8
0.3
3.5
0.25
false
false
false
false
0.0
0.0
0.0
0.0
0.0
0.0
0.0
0.2
0.5
0.35
0.25
0.15
0.7
"Bass""#;

        let synth = Synthesizer::new();
        let restored = synth
            .json_to_preset(legacy)
            .expect("legacy 45-line preset must load");

        assert_eq!(restored.name, "LegacyPatch");
        assert_eq!(restored.category, "Bass");
        assert_eq!(
            restored.lfo.waveform,
            LfoWaveform::Triangle,
            "legacy defaults to Triangle"
        );
        assert!(!restored.lfo.sync, "legacy sync defaults to false");
        // Chorus defaults come from EffectsParams::default() — chorus_mix=0 keeps legacy
        // presets dry, matching the original pre-chorus behaviour.
        let chorus_defaults = EffectsParams::default();
        assert!((restored.effects.chorus_mix - chorus_defaults.chorus_mix).abs() < 1e-6);
        assert!((restored.effects.chorus_rate - chorus_defaults.chorus_rate).abs() < 1e-6);
        assert!((restored.effects.chorus_depth - chorus_defaults.chorus_depth).abs() < 1e-6);
        assert_eq!(restored.poly_mod_filter_env_to_osc_a_freq, 0.0);
        assert_eq!(restored.poly_mod_filter_env_to_osc_a_pw, 0.0);
        assert_eq!(restored.poly_mod_osc_b_to_osc_a_freq, 0.0);
        assert_eq!(restored.poly_mod_osc_b_to_osc_a_pw, 0.0);
        assert_eq!(restored.poly_mod_osc_b_to_filter_cutoff, 0.0);
    }

    #[test]
    fn test_preset_json_invalid_format_returns_error() {
        let synth = Synthesizer::new();
        let result = synth.json_to_preset("not a valid preset");
        assert!(result.is_err(), "malformed JSON should return an error");
    }

    #[test]
    fn test_preset_full_flow_snapshot_json_apply() {
        // End-to-end flow: configure synth with every extended field set to a
        // distinctive value, snapshot → serialize → parse → apply_preset, and
        // verify the synthesizer's state is identical to the starting point.
        let mut synth = Synthesizer::new();
        synth.osc1.wave_type = WaveType::Triangle;
        synth.osc2.wave_type = WaveType::Square;
        synth.filter.cutoff = 2345.0;
        synth.filter.resonance = 1.7;
        synth.filter.keyboard_tracking = 0.65;
        synth.amp_envelope.attack = 0.3;
        synth.amp_envelope.sustain = 0.6;
        synth.master_volume = 0.42;
        synth.lfo.waveform = LfoWaveform::Square;
        synth.lfo.sync = true;
        synth.effects.chorus_mix = 0.42;
        synth.effects.chorus_rate = 0.9;
        synth.effects.chorus_depth = 0.55;
        synth.modulation_matrix.velocity_to_cutoff = 0.48;
        synth.modulation_matrix.velocity_to_amplitude = 0.27;
        synth.poly_mod_filter_env_to_osc_a_freq = 0.18;
        synth.poly_mod_filter_env_to_osc_a_pw = -0.11;
        synth.poly_mod_osc_b_to_osc_a_freq = 0.33;
        synth.poly_mod_osc_b_to_osc_a_pw = 0.07;
        synth.poly_mod_osc_b_to_filter_cutoff = -0.42;

        let preset = synth.snapshot_preset("FullFlow", "Test");
        let json = synth.preset_to_json(&preset).expect("serialize");
        let parsed = synth.json_to_preset(&json).expect("parse");
        assert_eq!(parsed.name, "FullFlow");
        assert_eq!(parsed.category, "Test");

        // Wipe the synth to defaults, then apply — this proves apply_preset
        // restores every field, not just the subset the user happened to tweak.
        synth.reset_patch_to_defaults();
        synth.apply_preset(parsed);

        assert_eq!(synth.osc1.wave_type, WaveType::Triangle);
        assert_eq!(synth.osc2.wave_type, WaveType::Square);
        assert!((synth.filter.cutoff - 2345.0).abs() < 1.0);
        assert!((synth.filter.resonance - 1.7).abs() < 0.01);
        assert!((synth.filter.keyboard_tracking - 0.65).abs() < 0.01);
        assert!((synth.amp_envelope.attack - 0.3).abs() < 0.01);
        assert!((synth.amp_envelope.sustain - 0.6).abs() < 0.01);
        assert!((synth.master_volume - 0.42).abs() < 0.01);
        assert_eq!(synth.lfo.waveform, LfoWaveform::Square);
        assert!(synth.lfo.sync);
        assert!((synth.effects.chorus_mix - 0.42).abs() < 0.01);
        assert!((synth.effects.chorus_rate - 0.9).abs() < 0.01);
        assert!((synth.effects.chorus_depth - 0.55).abs() < 0.01);
        assert!((synth.modulation_matrix.velocity_to_cutoff - 0.48).abs() < 0.01);
        assert!((synth.modulation_matrix.velocity_to_amplitude - 0.27).abs() < 0.01);
        assert!((synth.poly_mod_filter_env_to_osc_a_freq - 0.18).abs() < 0.01);
        assert!((synth.poly_mod_filter_env_to_osc_a_pw - (-0.11)).abs() < 0.01);
        assert!((synth.poly_mod_osc_b_to_osc_a_freq - 0.33).abs() < 0.01);
        assert!((synth.poly_mod_osc_b_to_osc_a_pw - 0.07).abs() < 0.01);
        assert!((synth.poly_mod_osc_b_to_filter_cutoff - (-0.42)).abs() < 0.01);
    }

    #[test]
    #[ignore = "Regenerate presets/*.json with the extended format. Run manually: \
                cargo test regenerate_preset_files -- --ignored --nocapture"]
    fn regenerate_preset_files() {
        let mut synth = Synthesizer::new();
        synth
            .force_create_all_classic_presets()
            .expect("classic presets must write successfully");
    }

    #[test]
    fn test_reset_patch_to_defaults_clears_poly_mod() {
        // Regression: without this reset, a previous preset's Poly Mod values
        // would bleed into the next preset builder, producing unintended
        // cross-modulation on patches that should be clean.
        let mut synth = Synthesizer::new();
        synth.poly_mod_filter_env_to_osc_a_freq = 0.5;
        synth.poly_mod_osc_b_to_filter_cutoff = -0.3;
        synth.filter.keyboard_tracking = 0.8;
        synth.modulation_matrix.velocity_to_cutoff = 0.6;

        synth.reset_patch_to_defaults();

        assert_eq!(synth.poly_mod_filter_env_to_osc_a_freq, 0.0);
        assert_eq!(synth.poly_mod_osc_b_to_filter_cutoff, 0.0);
        assert_eq!(synth.filter.keyboard_tracking, 0.0);
        assert_eq!(synth.modulation_matrix.velocity_to_cutoff, 0.0);
    }

    #[test]
    fn test_set_voice_pan_center_equal_power() {
        let pan = 0.0f32;
        let left = ((1.0 - pan) / 2.0_f32).sqrt();
        let right = ((1.0 + pan) / 2.0_f32).sqrt();
        let sum_sq = left * left + right * right;
        assert!(
            (sum_sq - 1.0).abs() < 1e-5,
            "equal power at center: L²+R²=1, got {}",
            sum_sq
        );
        assert!((left - right).abs() < 1e-5, "center: L==R");
    }

    #[test]
    fn test_set_voice_pan_full_right() {
        let pan = 1.0f32;
        let left = ((1.0 - pan) / 2.0_f32).sqrt();
        let right = ((1.0 + pan) / 2.0_f32).sqrt();
        assert!(left.abs() < 1e-5, "full right: L≈0, got {}", left);
        assert!((right - 1.0).abs() < 1e-5, "full right: R=1, got {}", right);
    }

    #[test]
    fn test_decimate_biquad_dc_gain_unity() {
        let mut x1 = 0.0f32;
        let mut x2 = 0.0f32;
        let mut y1 = 0.0f32;
        let mut y2 = 0.0f32;
        let mut out = 0.0f32;
        for _ in 0..2000 {
            out = Synthesizer::decimate_biquad(1.0, &mut x1, &mut x2, &mut y1, &mut y2);
        }
        assert!((out - 1.0).abs() < 1e-3, "DC gain=1.0, got {}", out);
    }

    #[test]
    fn test_decimate_biquad_nyquist_attenuated() {
        let mut x1 = 0.0f32;
        let mut x2 = 0.0f32;
        let mut y1 = 0.0f32;
        let mut y2 = 0.0f32;
        let mut out = 0.0f32;
        for i in 0..2000 {
            let x = if i % 2 == 0 { 1.0f32 } else { -1.0f32 };
            out = Synthesizer::decimate_biquad(x, &mut x1, &mut x2, &mut y1, &mut y2);
        }
        assert!(out.abs() < 0.01, "Nyquist fully attenuated, got {}", out);
    }
}
