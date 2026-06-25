//! Haptics, mapped to the same event vocabulary as audio.
//!
//! Reality check: a desktop Mac has no general haptic engine (only Force Touch
//! trackpads expose `NSHapticFeedbackManager`, with a fixed handful of canned
//! patterns and only while the cursor is on the trackpad). Rather than pull an
//! Objective-C bridge for a feature that mostly can't fire, this degrades to a
//! clean no-op — exactly the "no-ops gracefully where unsupported" contract.
//!
//! The structure (one method per phase event, gated by `enabled`) is kept so a
//! trackpad backend, or an iOS port, can drop straight in.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Phase-driven haptic events. The connection worker calls these on each
/// transition; on macOS desktop they currently do nothing.
///
/// `enabled` is a shared atomic so the Settings screen (main thread) can gate
/// output while the worker thread holds this handle.
pub struct PawHaptics {
    enabled: Arc<AtomicBool>,
    /// `true` if a real haptic backend was found. Always `false` on desktop.
    supported: bool,
}

impl PawHaptics {
    pub fn new(enabled: Arc<AtomicBool>) -> PawHaptics {
        // No supported backend on macOS desktop. If a future backend is wired
        // in (trackpad / iOS), flip `supported` based on engine creation.
        PawHaptics { enabled, supported: false }
    }

    #[inline]
    fn fire(&self, _intensity: f32, _sharpness: f32) {
        // Graceful no-op unless a real backend exists (none on macOS desktop).
        if self.enabled.load(Ordering::Relaxed) && self.supported {
            // A trackpad/iOS backend would emit a transient here.
        }
    }

    /// Light tick — one per satellite found.
    pub fn satellite_found(&self) {
        self.fire(0.4, 0.5);
    }
    /// Sharp transient thunk on `.locked`.
    pub fn lock(&self) {
        self.fire(1.0, 1.0);
    }
    /// Continuous low buzz during `.transmitting` (single pulse stand-in).
    pub fn transmit(&self) {
        self.fire(0.3, 0.2);
    }
    /// Sharp double-tap on `.receiving`.
    pub fn receive(&self) {
        self.fire(0.8, 0.9);
        self.fire(0.8, 0.9);
    }
    /// Dull thud on `.failed`.
    pub fn failed(&self) {
        self.fire(0.6, 0.1);
    }
}
