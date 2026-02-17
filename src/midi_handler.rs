use midir::{MidiInput, Ignore};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use crate::lock_free::{LockFreeSynth, MidiEvent, MidiEventQueue};

#[derive(Clone, Debug)]
pub struct MidiMessage {
    pub timestamp: std::time::Instant,
    pub message_type: String,
    pub description: String,
}

pub struct MidiHandler {
    _connection: Option<midir::MidiInputConnection<()>>,
    pub message_history: Arc<Mutex<VecDeque<MidiMessage>>>,
}

impl MidiHandler {
    pub fn new(
        lock_free_synth: Arc<LockFreeSynth>,
        midi_events: Arc<MidiEventQueue>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let message_history = Arc::new(Mutex::new(VecDeque::new()));
        let mut midi_in = MidiInput::new("Rust Synthesizer MIDI Input")?;
        midi_in.ignore(Ignore::None);

        let in_ports = midi_in.ports();

        if in_ports.is_empty() {
            log::info!("No MIDI input ports available");
            return Ok(MidiHandler {
                _connection: None,
                message_history,
            });
        }

        for (i, port) in in_ports.iter().enumerate() {
            log::info!("MIDI port {}: {}", i, midi_in.port_name(port).unwrap_or_else(|_| "Unknown".to_string()));
        }

        let in_port = &in_ports[0];
        let port_name = midi_in.port_name(in_port).unwrap_or_else(|_| "Unknown".to_string());
        log::info!("Connecting to MIDI port: {}", port_name);

        let history_clone = message_history.clone();
        let connection = midi_in.connect(in_port, "synth-input", move |_stamp, message, _| {
            Self::handle_midi_message(message, &lock_free_synth, &midi_events, &history_clone);
        }, ())?;

        Ok(MidiHandler {
            _connection: Some(connection),
            message_history,
        })
    }

    fn handle_midi_message(
        message: &[u8],
        lock_free_synth: &Arc<LockFreeSynth>,
        midi_events: &Arc<MidiEventQueue>,
        history: &Arc<Mutex<VecDeque<MidiMessage>>>,
    ) {
        if message.len() >= 3 {
            let status = message[0];
            let data1 = message[1];
            let data2 = message[2];
            let channel = (status & 0x0F) + 1;

            let (msg_type, description) = match status & 0xF0 {
                0x90 => {
                    if data2 > 0 {
                        midi_events.push(MidiEvent::NoteOn { note: data1, velocity: data2 });
                        ("Note On".to_string(), format!("Note: {} Vel: {} Ch: {}", Self::note_name(data1), data2, channel))
                    } else {
                        midi_events.push(MidiEvent::NoteOff { note: data1 });
                        ("Note Off".to_string(), format!("Note: {} (vel 0) Ch: {}", Self::note_name(data1), channel))
                    }
                },
                0x80 => {
                    midi_events.push(MidiEvent::NoteOff { note: data1 });
                    ("Note Off".to_string(), format!("Note: {} Vel: {} Ch: {}", Self::note_name(data1), data2, channel))
                },
                0xB0 => {
                    Self::handle_cc_message(lock_free_synth, midi_events, data1, data2);
                    ("CC".to_string(), format!("CC: {} Val: {} Ch: {}", data1, data2, channel))
                },
                0xC0 => ("Program".to_string(), format!("Program: {} Ch: {}", data1, channel)),
                0xD0 => ("Pressure".to_string(), format!("Pressure: {} Ch: {}", data1, channel)),
                0xE0 => {
                    let bend_value = ((data2 as u16) << 7) | (data1 as u16);
                    ("Pitch Bend".to_string(), format!("Bend: {} Ch: {}", bend_value, channel))
                },
                _ => ("Unknown".to_string(), format!("Status: 0x{:02X} Data: {} {} Ch: {}", status, data1, data2, channel)),
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
        let notes = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        let octave = (note / 12) as i32 - 1;
        let note_index = note % 12;
        format!("{}{}", notes[note_index as usize], octave)
    }

    fn handle_cc_message(lock_free_synth: &Arc<LockFreeSynth>, midi_events: &Arc<MidiEventQueue>, cc_number: u8, cc_value: u8) {
        let normalized_value = cc_value as f32 / 127.0;
        let mut params = *lock_free_synth.get_params();

        match cc_number {
            1 => params.osc1_level = normalized_value,
            2 => params.osc2_level = normalized_value,
            3 => params.osc1_detune = -12.0 + (normalized_value * 24.0),
            4 => params.osc2_detune = -12.0 + (normalized_value * 24.0),
            5 => params.osc1_pulse_width = 0.1 + (normalized_value * 0.8),
            6 => params.osc2_pulse_width = 0.1 + (normalized_value * 0.8),
            7 => params.mixer_osc1_level = normalized_value,
            8 => params.mixer_osc2_level = normalized_value,
            9 => params.noise_level = normalized_value,
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
                midi_events.push(MidiEvent::SustainPedal { pressed: normalized_value > 0.5 });
                return;
            },
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
