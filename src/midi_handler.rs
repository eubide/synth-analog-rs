use midir::{MidiInput, Ignore};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use crate::synthesizer::Synthesizer;

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
    pub fn new(synthesizer: Arc<Mutex<Synthesizer>>) -> Result<Self, Box<dyn std::error::Error>> {
        let message_history = Arc::new(Mutex::new(VecDeque::new()));
        let mut midi_in = MidiInput::new("Rust Synthesizer MIDI Input")?;
        midi_in.ignore(Ignore::None);
        
        // Get available ports
        let in_ports = midi_in.ports();
        
        if in_ports.is_empty() {
            println!("No MIDI input ports available");
            return Ok(MidiHandler { 
                _connection: None,
                message_history,
            });
        }
        
        // List available ports
        println!("Available MIDI input ports:");
        for (i, port) in in_ports.iter().enumerate() {
            println!("  {}: {}", i, midi_in.port_name(port).unwrap_or_else(|_| "Unknown".to_string()));
        }
        
        // Try to connect to the first available port
        let in_port = &in_ports[0];
        let port_name = midi_in.port_name(in_port).unwrap_or_else(|_| "Unknown".to_string());
        println!("Connecting to MIDI port: {}", port_name);
        
        let history_clone = message_history.clone();
        let connection = midi_in.connect(in_port, "synth-input", move |_stamp, message, _| {
            Self::handle_midi_message(message, &synthesizer, &history_clone);
        }, ())?;
        
        Ok(MidiHandler {
            _connection: Some(connection),
            message_history,
        })
    }
    
    fn handle_midi_message(message: &[u8], synthesizer: &Arc<Mutex<Synthesizer>>, history: &Arc<Mutex<VecDeque<MidiMessage>>>) {
        if message.len() >= 3 {
            let status = message[0];
            let data1 = message[1];
            let data2 = message[2];
            let channel = (status & 0x0F) + 1;
            
            let (msg_type, description) = match status & 0xF0 {
                0x90 => { // Note On
                    if data2 > 0 {
                        if let Ok(mut synth) = synthesizer.lock() {
                            synth.note_on(data1, data2);
                        }
                        ("Note On".to_string(), format!("Note: {} Vel: {} Ch: {}", Self::note_name(data1), data2, channel))
                    } else {
                        // Note on with velocity 0 = note off
                        if let Ok(mut synth) = synthesizer.lock() {
                            synth.note_off(data1);
                        }
                        ("Note Off".to_string(), format!("Note: {} (vel 0) Ch: {}", Self::note_name(data1), channel))
                    }
                },
                0x80 => { // Note Off
                    if let Ok(mut synth) = synthesizer.lock() {
                        synth.note_off(data1);
                    }
                    ("Note Off".to_string(), format!("Note: {} Vel: {} Ch: {}", Self::note_name(data1), data2, channel))
                },
                0xB0 => { // Control Change
                    if let Ok(mut synth) = synthesizer.lock() {
                        Self::handle_cc_message(&mut synth, data1, data2);
                    }
                    ("CC".to_string(), format!("CC: {} Val: {} Ch: {}", data1, data2, channel))
                },
                0xC0 => { // Program Change
                    ("Program".to_string(), format!("Program: {} Ch: {}", data1, channel))
                },
                0xD0 => { // Channel Pressure
                    ("Pressure".to_string(), format!("Pressure: {} Ch: {}", data1, channel))
                },
                0xE0 => { // Pitch Bend
                    let bend_value = ((data2 as u16) << 7) | (data1 as u16);
                    ("Pitch Bend".to_string(), format!("Bend: {} Ch: {}", bend_value, channel))
                },
                _ => {
                    ("Unknown".to_string(), format!("Status: 0x{:02X} Data: {} {} Ch: {}", status, data1, data2, channel))
                }
            };
            
            // Add to history
            if let Ok(mut hist) = history.lock() {
                hist.push_back(MidiMessage {
                    timestamp: std::time::Instant::now(),
                    message_type: msg_type,
                    description,
                });
                
                // Keep only last 100 messages
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
    
    fn handle_cc_message(synth: &mut Synthesizer, cc_number: u8, cc_value: u8) {
        let normalized_value = cc_value as f32 / 127.0;
        
        match cc_number {
            // Oscillator Controls
            1 => synth.osc1.amplitude = normalized_value,
            2 => synth.osc2.amplitude = normalized_value,
            3 => synth.osc1.detune = -12.0 + (normalized_value * 24.0),
            4 => synth.osc2.detune = -12.0 + (normalized_value * 24.0),
            5 => synth.osc1.pulse_width = 0.1 + (normalized_value * 0.8),
            6 => synth.osc2.pulse_width = 0.1 + (normalized_value * 0.8),
            
            // Mixer Controls
            7 => synth.mixer.osc1_level = normalized_value,
            8 => synth.mixer.osc2_level = normalized_value,
            9 => synth.mixer.noise_level = normalized_value,
            
            // Filter Controls
            16 => synth.filter.cutoff = 20.0 + (normalized_value * 19980.0),
            17 => synth.filter.resonance = normalized_value * 10.0,
            18 => synth.filter.envelope_amount = normalized_value,
            19 => synth.filter.keyboard_tracking = normalized_value,
            
            // Filter Envelope
            20 => synth.filter_envelope.attack = normalized_value * 5.0,
            21 => synth.filter_envelope.decay = normalized_value * 5.0,
            22 => synth.filter_envelope.sustain = normalized_value,
            23 => synth.filter_envelope.release = normalized_value * 5.0,
            
            // Amp Envelope
            24 => synth.amp_envelope.attack = normalized_value * 5.0,
            25 => synth.amp_envelope.decay = normalized_value * 5.0,
            26 => synth.amp_envelope.sustain = normalized_value,
            27 => synth.amp_envelope.release = normalized_value * 5.0,
            
            // LFO Controls
            28 => synth.lfo.frequency = 0.1 + (normalized_value * 19.9),
            29 => synth.lfo.amplitude = normalized_value,
            30 => synth.lfo.target_osc1_pitch = normalized_value > 0.5,
            31 => synth.lfo.target_osc2_pitch = normalized_value > 0.5,
            32 => synth.lfo.target_filter = normalized_value > 0.5,
            33 => synth.lfo.target_amplitude = normalized_value > 0.5,
            
            // Master Volume
            34 => synth.master_volume = normalized_value,
            
            // Effects
            40 => synth.effects.reverb_amount = normalized_value,
            41 => synth.effects.reverb_size = normalized_value,
            42 => synth.effects.delay_time = 0.01 + (normalized_value * 1.99), // 10ms to 2s
            43 => synth.effects.delay_feedback = normalized_value * 0.95, // Max 95% to avoid runaway
            44 => synth.effects.delay_amount = normalized_value,
            
            // Arpeggiator
            50 => synth.arpeggiator.enabled = normalized_value > 0.5,
            51 => synth.arpeggiator.rate = 60.0 + (normalized_value * 180.0), // 60-240 BPM
            52 => {
                let pattern_index = (normalized_value * 3.99) as usize;
                synth.arpeggiator.pattern = match pattern_index {
                    0 => crate::synthesizer::ArpPattern::Up,
                    1 => crate::synthesizer::ArpPattern::Down,
                    2 => crate::synthesizer::ArpPattern::UpDown,
                    _ => crate::synthesizer::ArpPattern::Random,
                };
            },
            53 => synth.arpeggiator.octaves = 1 + (normalized_value * 3.0) as u8, // 1-4 octaves
            54 => synth.arpeggiator.gate_length = 0.1 + (normalized_value * 0.9), // 10%-100%
            
            _ => {} // Ignore unmapped CCs
        }
    }
}

impl Drop for MidiHandler {
    fn drop(&mut self) {
        if self._connection.is_some() {
            println!("MIDI connection closed");
        }
    }
}