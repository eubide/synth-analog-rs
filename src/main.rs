use eframe::egui;
use std::sync::{Arc, Mutex};

mod synthesizer;
mod audio_engine;
mod gui;
mod midi_handler;

use synthesizer::Synthesizer;
use audio_engine::AudioEngine;
use gui::SynthApp;
use midi_handler::MidiHandler;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 600.0])
            .with_title("Rust Synthesizer"),
        ..Default::default()
    };

    let synth = Arc::new(Mutex::new(Synthesizer::new()));
    let audio_engine = match AudioEngine::new(synth.clone()) {
        Ok(engine) => engine,
        Err(e) => {
            eprintln!("Failed to initialize audio engine: {}", e);
            eprintln!("Please check your audio device configuration.");
            eprintln!("Audio initialization failed: {}", e);
            std::process::exit(1);
        }
    };
    
    // Initialize MIDI input
    let _midi_handler = match MidiHandler::new(synth.clone()) {
        Ok(handler) => {
            println!("MIDI input initialized successfully");
            Some(handler)
        },
        Err(e) => {
            println!("Failed to initialize MIDI input: {}", e);
            println!("Continuing without MIDI support...");
            None
        }
    };
    
    eframe::run_native(
        "Rust Synthesizer",
        options,
        Box::new(move |_cc| Ok(Box::new(SynthApp::new(synth, audio_engine, _midi_handler)))),
    )
}
