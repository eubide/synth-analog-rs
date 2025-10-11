use eframe::egui;
use std::sync::{Arc, Mutex};

mod synthesizer;
mod audio_engine;
mod gui;
mod midi_handler;
mod optimization;
mod lock_free;

use synthesizer::Synthesizer;
use audio_engine::AudioEngine;
use gui::SynthApp;
use midi_handler::MidiHandler;

fn main() -> Result<(), eframe::Error> {
    // Initialize logging system
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Analog Synthesizer");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 600.0])
            .with_title("Rust Synthesizer"),
        ..Default::default()
    };

    let synth = Arc::new(Mutex::new(Synthesizer::new()));
    let audio_engine = match AudioEngine::new(synth.clone()) {
        Ok(engine) => {
            log::info!("Audio engine initialized successfully");
            engine
        },
        Err(e) => {
            log::error!("Failed to initialize audio engine: {}", e);
            log::error!("Please check your audio device configuration.");
            std::process::exit(1);
        }
    };

    // Initialize MIDI input
    let _midi_handler = match MidiHandler::new(synth.clone()) {
        Ok(handler) => {
            log::info!("MIDI input initialized successfully");
            Some(handler)
        },
        Err(e) => {
            log::warn!("Failed to initialize MIDI input: {}", e);
            log::warn!("Continuing without MIDI support...");
            None
        }
    };
    
    eframe::run_native(
        "Rust Synthesizer",
        options,
        Box::new(move |_cc| Ok(Box::new(SynthApp::new(synth, audio_engine, _midi_handler)))),
    )
}
