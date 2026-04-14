use eframe::egui;
use std::sync::Arc;

mod audio_engine;
mod gui;
mod lock_free;
mod midi_handler;
mod optimization;
mod synthesizer;

use audio_engine::AudioEngine;
use gui::SynthApp;
use lock_free::{LockFreeSynth, MidiEventQueue};
use midi_handler::MidiHandler;

fn main() -> Result<(), eframe::Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Analog Synthesizer");

    // Create preset files on main thread (not on audio thread)
    {
        let mut preset_synth = synthesizer::Synthesizer::new();
        if let Err(e) = preset_synth.create_all_classic_presets() {
            log::warn!("Could not create classic presets: {}", e);
        } else {
            log::info!("Authentic vintage analog presets loaded successfully!");
        }
    } // preset_synth is dropped here — only used for file creation

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1020.0, 620.0])
            .with_min_inner_size([1020.0, 620.0])
            .with_max_inner_size([1020.0, 620.0])
            .with_resizable(false)
            .with_title("Rust Synthesizer"),
        ..Default::default()
    };

    // Create lock-free shared state
    let lock_free_synth = Arc::new(LockFreeSynth::new());
    let midi_events = Arc::new(MidiEventQueue::new());

    // Initialize audio engine (owns the Synthesizer)
    let audio_engine = match AudioEngine::new(lock_free_synth.clone(), midi_events.clone()) {
        Ok(engine) => {
            log::info!("Audio engine initialized successfully");
            engine
        }
        Err(e) => {
            log::error!("Failed to initialize audio engine: {}", e);
            log::error!("Please check your audio device configuration.");
            std::process::exit(1);
        }
    };

    // Initialize MIDI input
    let midi_handler = match MidiHandler::new(lock_free_synth.clone(), midi_events.clone()) {
        Ok(handler) => {
            log::info!("MIDI input initialized successfully");
            Some(handler)
        }
        Err(e) => {
            log::warn!("Failed to initialize MIDI input: {}", e);
            log::warn!("Continuing without MIDI support...");
            None
        }
    };

    eframe::run_native(
        "Rust Synthesizer",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(SynthApp::new(
                lock_free_synth,
                midi_events,
                audio_engine,
                midi_handler,
            )))
        }),
    )
}
