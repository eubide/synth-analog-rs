use crate::lock_free::{
    LockFreeSynth, MidiEvent, MidiEventQueue, SynthParameters, UiEvent, UiEventQueue,
};
use midir::{Ignore, MidiInput};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Single row of the CC → parameter map. One table, queried by CC number (default
/// dispatch) or by name (MIDI learn custom bindings). The GUI enumerates it to
/// populate the Learn panel, so adding a new bindable parameter is one edit.
pub struct CcBinding {
    /// Default CC number (can be rebound via MIDI learn).
    pub cc: u8,
    /// Stable identifier used by the MIDI learn custom map + preset serialization.
    pub name: &'static str,
    /// Human-readable label for the Learn panel.
    pub label: &'static str,
    /// Writes the scaled value into `SynthParameters`. `v` is normalized 0..=1.
    pub apply: fn(&mut SynthParameters, f32),
}

/// CC → parameter bindings. Ranges match the canonical GUI sliders so that
/// a CC sweep covers the same territory as dragging the slider end-to-end.
pub const CC_BINDINGS: &[CcBinding] = &[
    CcBinding {
        cc: 1,
        name: "mod_wheel",
        label: "Mod Wheel",
        apply: |p, v| p.mod_wheel = v,
    },
    CcBinding {
        cc: 2,
        name: "osc2_level",
        label: "Osc B Level",
        apply: |p, v| p.osc2_level = v,
    },
    CcBinding {
        cc: 3,
        name: "osc1_detune",
        label: "Osc A Detune",
        apply: |p, v| p.osc1_detune = -24.0 + v * 48.0,
    },
    CcBinding {
        cc: 4,
        name: "osc2_detune",
        label: "Osc B Detune",
        apply: |p, v| p.osc2_detune = -24.0 + v * 48.0,
    },
    CcBinding {
        cc: 5,
        name: "osc1_pulse_width",
        label: "Osc A PW",
        apply: |p, v| p.osc1_pulse_width = 0.1 + v * 0.8,
    },
    CcBinding {
        cc: 6,
        name: "osc2_pulse_width",
        label: "Osc B PW",
        apply: |p, v| p.osc2_pulse_width = 0.1 + v * 0.8,
    },
    CcBinding {
        cc: 7,
        name: "mixer_osc1_level",
        label: "Mix Osc A",
        apply: |p, v| p.mixer_osc1_level = v,
    },
    CcBinding {
        cc: 8,
        name: "mixer_osc2_level",
        label: "Mix Osc B",
        apply: |p, v| p.mixer_osc2_level = v,
    },
    CcBinding {
        cc: 9,
        name: "noise_level",
        label: "Noise",
        apply: |p, v| p.noise_level = v,
    },
    CcBinding {
        cc: 11,
        name: "expression",
        label: "Expression",
        apply: |p, v| p.expression = v,
    },
    CcBinding {
        cc: 16,
        name: "filter_cutoff",
        label: "Filter Cutoff",
        apply: |p, v| p.filter_cutoff = 20.0 + v * 19980.0,
    },
    CcBinding {
        cc: 17,
        name: "filter_resonance",
        label: "Filter Resonance",
        apply: |p, v| p.filter_resonance = v * 4.0,
    },
    CcBinding {
        cc: 18,
        name: "filter_envelope_amount",
        label: "Filter Env Amount",
        apply: |p, v| p.filter_envelope_amount = v,
    },
    CcBinding {
        cc: 19,
        name: "filter_keyboard_tracking",
        label: "Filter Kbd Track",
        apply: |p, v| p.filter_keyboard_tracking = v,
    },
    CcBinding {
        cc: 20,
        name: "filter_attack",
        label: "Filter Attack",
        apply: |p, v| p.filter_attack = v * 5.0,
    },
    CcBinding {
        cc: 21,
        name: "filter_decay",
        label: "Filter Decay",
        apply: |p, v| p.filter_decay = v * 5.0,
    },
    CcBinding {
        cc: 22,
        name: "filter_sustain",
        label: "Filter Sustain",
        apply: |p, v| p.filter_sustain = v,
    },
    CcBinding {
        cc: 23,
        name: "filter_release",
        label: "Filter Release",
        apply: |p, v| p.filter_release = v * 5.0,
    },
    CcBinding {
        cc: 24,
        name: "amp_attack",
        label: "Amp Attack",
        apply: |p, v| p.amp_attack = v * 5.0,
    },
    CcBinding {
        cc: 25,
        name: "amp_decay",
        label: "Amp Decay",
        apply: |p, v| p.amp_decay = v * 5.0,
    },
    CcBinding {
        cc: 26,
        name: "amp_sustain",
        label: "Amp Sustain",
        apply: |p, v| p.amp_sustain = v,
    },
    CcBinding {
        cc: 27,
        name: "amp_release",
        label: "Amp Release",
        apply: |p, v| p.amp_release = v * 5.0,
    },
    CcBinding {
        cc: 28,
        name: "lfo_rate",
        label: "LFO Rate",
        apply: |p, v| p.lfo_rate = 0.1 + v * 19.9,
    },
    CcBinding {
        cc: 29,
        name: "lfo_amount",
        label: "LFO Amount",
        apply: |p, v| p.lfo_amount = v,
    },
    CcBinding {
        cc: 30,
        name: "lfo_target_osc1_pitch",
        label: "LFO -> Osc A",
        apply: |p, v| p.lfo_target_osc1_pitch = v > 0.5,
    },
    CcBinding {
        cc: 31,
        name: "lfo_target_osc2_pitch",
        label: "LFO -> Osc B",
        apply: |p, v| p.lfo_target_osc2_pitch = v > 0.5,
    },
    CcBinding {
        cc: 32,
        name: "lfo_target_filter",
        label: "LFO -> Filter",
        apply: |p, v| p.lfo_target_filter = v > 0.5,
    },
    CcBinding {
        cc: 33,
        name: "lfo_target_amplitude",
        label: "LFO -> Amp",
        apply: |p, v| p.lfo_target_amplitude = v > 0.5,
    },
    CcBinding {
        cc: 34,
        name: "master_volume",
        label: "Master Volume",
        apply: |p, v| p.master_volume = v,
    },
    CcBinding {
        cc: 40,
        name: "reverb_amount",
        label: "Reverb Amount",
        apply: |p, v| p.reverb_amount = v,
    },
    CcBinding {
        cc: 41,
        name: "reverb_size",
        label: "Reverb Size",
        apply: |p, v| p.reverb_size = v,
    },
    CcBinding {
        cc: 42,
        name: "delay_time",
        label: "Delay Time",
        apply: |p, v| p.delay_time = 0.01 + v * 1.99,
    },
    CcBinding {
        cc: 43,
        name: "delay_feedback",
        label: "Delay Feedback",
        apply: |p, v| p.delay_feedback = v * 0.95,
    },
    CcBinding {
        cc: 44,
        name: "delay_amount",
        label: "Delay Amount",
        apply: |p, v| p.delay_amount = v,
    },
    CcBinding {
        cc: 50,
        name: "arp_enabled",
        label: "Arp Enabled",
        apply: |p, v| p.arp_enabled = v > 0.5,
    },
    CcBinding {
        cc: 51,
        name: "arp_rate",
        label: "Arp Rate",
        apply: |p, v| p.arp_rate = 60.0 + v * 180.0,
    },
    CcBinding {
        cc: 52,
        name: "arp_pattern",
        label: "Arp Pattern",
        apply: |p, v| p.arp_pattern = (v * 3.99) as u8,
    },
    CcBinding {
        cc: 53,
        name: "arp_octaves",
        label: "Arp Octaves",
        apply: |p, v| p.arp_octaves = 1 + (v * 3.0) as u8,
    },
    CcBinding {
        cc: 54,
        name: "arp_gate_length",
        label: "Arp Gate",
        apply: |p, v| p.arp_gate_length = 0.1 + v * 0.9,
    },
];

fn binding_by_cc(cc: u8) -> Option<&'static CcBinding> {
    CC_BINDINGS.iter().find(|b| b.cc == cc)
}

fn binding_by_name(name: &str) -> Option<&'static CcBinding> {
    CC_BINDINGS.iter().find(|b| b.name == name)
}

/// Shared state for MIDI learn mode.
#[derive(Default)]
pub struct MidiLearnState {
    /// Param being learned — set by GUI, cleared when CC arrives.
    pub pending_param: Option<String>,
    /// Custom CC→param bindings. Key=CC number, Value=param name.
    pub custom_map: std::collections::HashMap<u8, String>,
}

#[derive(Clone, Debug)]
pub struct MidiMessage {
    pub timestamp: std::time::Instant,
    pub message_type: String,
    pub description: String,
}

pub struct MidiHandler {
    _connection: Option<midir::MidiInputConnection<()>>,
    pub message_history: Arc<Mutex<VecDeque<MidiMessage>>>,
    pub learn_state: Arc<Mutex<MidiLearnState>>,
}

impl MidiHandler {
    pub fn new(
        lock_free_synth: Arc<LockFreeSynth>,
        midi_events: Arc<MidiEventQueue>,
        ui_events: Arc<UiEventQueue>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let message_history = Arc::new(Mutex::new(VecDeque::new()));
        let learn_state: Arc<Mutex<MidiLearnState>> =
            Arc::new(Mutex::new(MidiLearnState::default()));
        let learn_state_clone = learn_state.clone();
        let mut midi_in = MidiInput::new("Rust Synthesizer MIDI Input")?;
        midi_in.ignore(Ignore::None);

        let in_ports = midi_in.ports();

        if in_ports.is_empty() {
            log::info!("No MIDI input ports available");
            return Ok(MidiHandler {
                _connection: None,
                message_history,
                learn_state,
            });
        }

        for (i, port) in in_ports.iter().enumerate() {
            log::info!(
                "MIDI port {}: {}",
                i,
                midi_in
                    .port_name(port)
                    .unwrap_or_else(|_| "Unknown".to_string())
            );
        }

        let in_port = &in_ports[0];
        let port_name = midi_in
            .port_name(in_port)
            .unwrap_or_else(|_| "Unknown".to_string());
        log::info!("Connecting to MIDI port: {}", port_name);

        let history_clone = message_history.clone();
        let connection = midi_in.connect(
            in_port,
            "synth-input",
            move |_stamp, message, _| {
                Self::handle_midi_message(
                    message,
                    &lock_free_synth,
                    &midi_events,
                    &ui_events,
                    &history_clone,
                    &learn_state_clone,
                );
            },
            (),
        )?;

        Ok(MidiHandler {
            _connection: Some(connection),
            message_history,
            learn_state,
        })
    }

    fn handle_midi_message(
        message: &[u8],
        lock_free_synth: &Arc<LockFreeSynth>,
        midi_events: &Arc<MidiEventQueue>,
        ui_events: &Arc<UiEventQueue>,
        history: &Arc<Mutex<VecDeque<MidiMessage>>>,
        learn_state: &Arc<Mutex<MidiLearnState>>,
    ) {
        // Mensajes de sistema en tiempo real (1 byte) — alta prioridad, no entran en history
        if !message.is_empty() {
            match message[0] {
                0xF8 => {
                    midi_events.push(MidiEvent::MidiClock);
                    return;
                }
                0xFA => {
                    midi_events.push(MidiEvent::MidiClockStart);
                    return;
                }
                0xFB => {
                    midi_events.push(MidiEvent::MidiClockContinue);
                    return;
                }
                0xFC => {
                    midi_events.push(MidiEvent::MidiClockStop);
                    return;
                }
                _ => {}
            }
        }

        // SysEx: empieza con 0xF0, termina con 0xF7
        if message.len() >= 4 && message[0] == 0xF0 && message[message.len() - 1] == 0xF7 {
            // Manufacturer ID: 0x7D (non-commercial / educational)
            if message[1] == 0x7D {
                match message[2] {
                    0x01 => {
                        ui_events.push(UiEvent::SysExRequest);
                        if let Ok(mut hist) = history.lock() {
                            hist.push_back(MidiMessage {
                                timestamp: std::time::Instant::now(),
                                message_type: "SysEx".to_string(),
                                description: "Patch dump request".to_string(),
                            });
                        }
                    }
                    0x02 if message.len() > 4 => {
                        let data = message[3..message.len() - 1].to_vec();
                        if let Ok(mut hist) = history.lock() {
                            hist.push_back(MidiMessage {
                                timestamp: std::time::Instant::now(),
                                message_type: "SysEx".to_string(),
                                description: format!("Patch load ({} bytes)", data.len()),
                            });
                        }
                        ui_events.push(UiEvent::SysExPatch { data });
                    }
                    _ => {}
                }
            }
            return;
        }

        if message.len() >= 3 {
            let status = message[0];
            let data1 = message[1];
            let data2 = message[2];
            let channel = (status & 0x0F) + 1;

            let (msg_type, description) = match status & 0xF0 {
                0x90 => {
                    if data2 > 0 {
                        midi_events.push(MidiEvent::NoteOn {
                            note: data1,
                            velocity: data2,
                        });
                        (
                            "Note On".to_string(),
                            format!(
                                "Note: {} Vel: {} Ch: {}",
                                Self::note_name(data1),
                                data2,
                                channel
                            ),
                        )
                    } else {
                        midi_events.push(MidiEvent::NoteOff { note: data1 });
                        (
                            "Note Off".to_string(),
                            format!("Note: {} (vel 0) Ch: {}", Self::note_name(data1), channel),
                        )
                    }
                }
                0x80 => {
                    midi_events.push(MidiEvent::NoteOff { note: data1 });
                    (
                        "Note Off".to_string(),
                        format!(
                            "Note: {} Vel: {} Ch: {}",
                            Self::note_name(data1),
                            data2,
                            channel
                        ),
                    )
                }
                0xB0 => {
                    Self::handle_cc_message(
                        lock_free_synth,
                        midi_events,
                        data1,
                        data2,
                        learn_state,
                    );
                    (
                        "CC".to_string(),
                        format!("CC: {} Val: {} Ch: {}", data1, data2, channel),
                    )
                }
                0xC0 => {
                    ui_events.push(UiEvent::ProgramChange { program: data1 });
                    (
                        "Program".to_string(),
                        format!("Program: {} Ch: {}", data1, channel),
                    )
                }
                0xD0 => {
                    // Channel Pressure (aftertouch): data1 = pressure 0..=127
                    let normalized = data1 as f32 / 127.0;
                    let mut params = *lock_free_synth.get_params();
                    params.aftertouch = normalized;
                    lock_free_synth.set_params(params);
                    (
                        "Pressure".to_string(),
                        format!("Pressure: {:.3} Ch: {}", normalized, channel),
                    )
                }
                0xE0 => {
                    let bend_value = ((data2 as u16) << 7) | (data1 as u16);
                    // 14-bit value: 0..=16383, center = 8192 → normalize to -1.0..=1.0
                    let normalized = (bend_value as f32 - 8192.0) / 8192.0;
                    let mut params = *lock_free_synth.get_params();
                    params.pitch_bend = normalized.clamp(-1.0, 1.0);
                    lock_free_synth.set_params(params);
                    (
                        "Pitch Bend".to_string(),
                        format!("Bend: {:.3} Ch: {}", normalized, channel),
                    )
                }
                _ => (
                    "Unknown".to_string(),
                    format!(
                        "Status: 0x{:02X} Data: {} {} Ch: {}",
                        status, data1, data2, channel
                    ),
                ),
            };

            if let Ok(mut hist) = history.lock() {
                hist.push_back(MidiMessage {
                    timestamp: std::time::Instant::now(),
                    message_type: msg_type,
                    description,
                });
                if hist.len() > 100 {
                    hist.pop_front();
                }
            }
        }
    }

    fn note_name(note: u8) -> String {
        let notes = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let octave = (note / 12) as i32 - 1;
        let note_index = note % 12;
        format!("{}{}", notes[note_index as usize], octave)
    }

    fn handle_cc_message(
        lock_free_synth: &Arc<LockFreeSynth>,
        midi_events: &Arc<MidiEventQueue>,
        cc_number: u8,
        cc_value: u8,
        learn_state: &Arc<Mutex<MidiLearnState>>,
    ) {
        let v = cc_value as f32 / 127.0;

        // Learn is checked first so the user can rebind any CC, including 64/120/123.
        if let Ok(mut state) = learn_state.try_lock() {
            if let Some(param_name) = state.pending_param.take() {
                log::info!("MIDI Learn: CC {} → {}", cc_number, param_name);
                state.custom_map.insert(cc_number, param_name);
                return;
            }
            if let Some(param_name) = state.custom_map.get(&cc_number).cloned() {
                if let Some(b) = binding_by_name(&param_name) {
                    let mut params = *lock_free_synth.get_params();
                    (b.apply)(&mut params, v);
                    lock_free_synth.set_params(params);
                }
                return;
            }
        }

        // Event-producing CCs don't write into SynthParameters.
        match cc_number {
            64 => {
                midi_events.push(MidiEvent::SustainPedal { pressed: v > 0.5 });
                return;
            }
            // 120 = All Sound Off, 123 = All Notes Off (MIDI standard).
            120 | 123 => {
                midi_events.push(MidiEvent::AllNotesOff);
                return;
            }
            _ => {}
        }

        if let Some(b) = binding_by_cc(cc_number) {
            let mut params = *lock_free_synth.get_params();
            (b.apply)(&mut params, v);
            lock_free_synth.set_params(params);
        }
    }
}

impl Drop for MidiHandler {
    fn drop(&mut self) {
        if self._connection.is_some() {
            log::info!("MIDI connection closed");
        }
    }
}
