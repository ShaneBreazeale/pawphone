//! Screens. Each view is implemented as an `impl PawPhoneApp` block in its own
//! file, so the app state lives in one place but the UI code stays split.

pub mod theme;

mod contacts;
mod hud;
mod settings;
mod thread;
