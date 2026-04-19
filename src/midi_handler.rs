use crate::lock_free::{LockFreeSynth, MidiEvent, MidiEventQueue, UiEvent, UiEventQueue};
use midir::{Ignore, MidiInput};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

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
        let learn_state: Arc<Mutex<MidiLearnState>> = Arc::new(Mutex::new(MidiLearnState::default()));
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
                0xF8 => { midi_events.push(MidiEvent::MidiClock); return; }
                0xFA => { midi_events.push(MidiEvent::MidiClockStart); return; }
                0xFB => { midi_events.push(MidiEvent::MidiClockContinue); return; }
                0xFC => { midi_events.push(MidiEvent::MidiClockStop); return; }
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
                    Self::handle_cc_message(lock_free_synth, midi_events, data1, data2, learn_state);
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

    fn apply_named_param(params: &mut crate::lock_free::SynthParameters, name: &str, v: f32) {
        match name {
            "filter_cutoff" => params.filter_cutoff = 20.0 + v * 19980.0,
            "filter_resonance" => params.filter_resonance = v * 4.0,
            "filter_envelope_amount" => params.filter_envelope_amount = v,
            "amp_attack" => params.amp_attack = v * 5.0,
            "amp_decay" => params.amp_decay = v * 5.0,
            "amp_sustain" => params.amp_sustain = v,
            "amp_release" => params.amp_release = v * 5.0,
            "filter_attack" => params.filter_attack = v * 5.0,
            "filter_decay" => params.filter_decay = v * 5.0,
            "filter_sustain" => params.filter_sustain = v,
            "filter_release" => params.filter_release = v * 5.0,
            "lfo_rate" => params.lfo_rate = 0.1 + v * 19.9,
            "lfo_amount" => params.lfo_amount = v,
            "master_volume" => params.master_volume = v,
            "reverb_amount" => params.reverb_amount = v,
            "delay_feedback" => params.delay_feedback = v * 0.95,
            "delay_amount" => params.delay_amount = v,
            "osc1_detune" => params.osc1_detune = -24.0 + v * 48.0,
            "osc2_detune" => params.osc2_detune = -24.0 + v * 48.0,
            _ => {}
        }
    }

    fn handle_cc_message(
        lock_free_synth: &Arc<LockFreeSynth>,
        midi_events: &Arc<MidiEventQueue>,
        cc_number: u8,
        cc_value: u8,
        learn_state: &Arc<Mutex<MidiLearnState>>,
    ) {
        // MIDI learn: if a param is pending, bind this CC to it
        if let Ok(mut state) = learn_state.try_lock() {
            if let Some(param_name) = state.pending_param.take() {
                log::info!("MIDI Learn: CC {} → {}", cc_number, param_name);
                state.custom_map.insert(cc_number, param_name);
                return;
            }
            // Apply custom binding if present
            if let Some(param_name) = state.custom_map.get(&cc_number).cloned() {
                let normalized = cc_value as f32 / 127.0;
                let mut params = *lock_free_synth.get_params();
                Self::apply_named_param(&mut params, &param_name, normalized);
                lock_free_synth.set_params(params);
                return;
            }
        }

        let normalized_value = cc_value as f32 / 127.0;
        let mut params = *lock_free_synth.get_params();

        match cc_number {
            1 => params.mod_wheel = normalized_value,
            2 => params.osc2_level = normalized_value,
            3 => params.osc1_detune = -12.0 + (normalized_value * 24.0),
            4 => params.osc2_detune = -12.0 + (normalized_value * 24.0),
            5 => params.osc1_pulse_width = 0.1 + (normalized_value * 0.8),
            6 => params.osc2_pulse_width = 0.1 + (normalized_value * 0.8),
            7 => params.mixer_osc1_level = normalized_value,
            8 => params.mixer_osc2_level = normalized_value,
            9 => params.noise_level = normalized_value,
            11 => params.expression = normalized_value,
            16 => params.filter_cutoff = 20.0 + (normalized_value * 19980.0),
            17 => params.filter_resonance = normalized_value * 10.0,
            18 => params.filter_envelope_amount = normalized_value,
            19 => params.filter_keyboard_tracking = normalized_value,
            20 => params.filter_attack = normalized_value * 5.0,
            21 => params.filter_decay = normalized_value * 5.0,
            22 => params.filter_sustain = normalized_value,
            23 => params.filter_release = normalized_value * 5.0,
            24 => params.amp_attack = normalized_value * 5.0,
            25 => params.amp_decay = normalized_value * 5.0,
            26 => params.amp_sustain = normalized_value,
            27 => params.amp_release = normalized_value * 5.0,
            28 => params.lfo_rate = 0.1 + (normalized_value * 19.9),
            29 => params.lfo_amount = normalized_value,
            30 => params.lfo_target_osc1_pitch = normalized_value > 0.5,
            31 => params.lfo_target_osc2_pitch = normalized_value > 0.5,
            32 => params.lfo_target_filter = normalized_value > 0.5,
            33 => params.lfo_target_amplitude = normalized_value > 0.5,
            34 => params.master_volume = normalized_value,
            40 => params.reverb_amount = normalized_value,
            41 => params.reverb_size = normalized_value,
            42 => params.delay_time = 0.01 + (normalized_value * 1.99),
            43 => params.delay_feedback = normalized_value * 0.95,
            44 => params.delay_amount = normalized_value,
            50 => params.arp_enabled = normalized_value > 0.5,
            51 => params.arp_rate = 60.0 + (normalized_value * 180.0),
            52 => params.arp_pattern = (normalized_value * 3.99) as u8,
            53 => params.arp_octaves = 1 + (normalized_value * 3.0) as u8,
            54 => params.arp_gate_length = 0.1 + (normalized_value * 0.9),
            64 => {
                midi_events.push(MidiEvent::SustainPedal {
                    pressed: normalized_value > 0.5,
                });
                return;
            }
            120 | 123 => {
                // CC 120 = All Sound Off, CC 123 = All Notes Off (MIDI standard)
                midi_events.push(MidiEvent::AllNotesOff);
                return;
            }
            _ => return,
        }

        lock_free_synth.set_params(params);
    }
}

impl Drop for MidiHandler {
    fn drop(&mut self) {
        if self._connection.is_some() {
            log::info!("MIDI connection closed");
        }
    }
}
