//! Paw Phone — powered by PawLink™.
//!
//! A whimsical, fully on-device toy: cats message cats over a fictional
//! satellite constellation. There is NO networking anywhere in this crate —
//! the "other cat" is simulated locally. That is an architectural invariant,
//! not an oversight. (No `std::net`, no sockets, no HTTP, no entitlements.)

// The window should be a GUI app, not a console app, in release on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod connection;
mod haptics;
mod models;
mod persistence;
mod personality;
mod phase;
mod telemetry;
mod views;

use app::PawPhoneApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 780.0])
            .with_min_inner_size([380.0, 560.0])
            .with_title("Paw Phone — powered by PawLink"),
        ..Default::default()
    };

    eframe::run_native(
        "Paw Phone",
        options,
        Box::new(|cc| Ok(Box::new(PawPhoneApp::new(cc)))),
    )
}
