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
                            synth.note_on(data1);
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
}

impl Drop for MidiHandler {
    fn drop(&mut self) {
        if self._connection.is_some() {
            println!("MIDI connection closed");
        }
    }
}