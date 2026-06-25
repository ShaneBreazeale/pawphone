//! The connection state machine contract.
//!
//! `PawLinkPhase` is the single shared vocabulary between the UI, the audio
//! engine and the haptics layer. Define it once, here, and let everything else
//! react to it. Nothing in this file knows about audio buffers, egui, or
//! SQLite — it is pure description.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Every theatrical beat of the PawLink "uplink ceremony", in order.
///
/// A `send()` walks `idle → poweringUp → … → connected` (or `→ failed`).
/// `acquiring` carries live telemetry (`found` of `total` satellites) so the
/// HUD can animate the count climbing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PawLinkPhase {
    Idle,
    /// ~0.8s — relay click + rising power hum.
    PoweringUp,
    /// ~2.5s — satellite search; telemetry counts up.
    Acquiring { found: u32, total: u32 },
    /// ~1.5s — modem warble / data burst.
    Handshaking,
    /// ~0.3s — confirmation chime + haptic thunk.
    Locked,
    /// ~1.0s — outbound data chitter.
    Transmitting,
    /// Satellite latency drama (configurable by [`RealismProfile`]).
    AwaitingReply,
    /// Incoming meow.
    Receiving,
    /// Ceremony complete; conversation updated.
    Connected,
    /// Flavored failure — see [`PawLinkError`].
    Failed(PawLinkError),
}

impl PawLinkPhase {
    /// Short uppercase label for the HUD phase readout.
    pub fn label(&self) -> &'static str {
        match self {
            PawLinkPhase::Idle => "STANDBY",
            PawLinkPhase::PoweringUp => "ENGAGING RELAY",
            PawLinkPhase::Acquiring { .. } => "ACQUIRING PAWLINK",
            PawLinkPhase::Handshaking => "HANDSHAKING",
            PawLinkPhase::Locked => "SIGNAL LOCKED",
            PawLinkPhase::Transmitting => "TRANSMITTING",
            PawLinkPhase::AwaitingReply => "AWAITING REPLY",
            PawLinkPhase::Receiving => "INCOMING",
            PawLinkPhase::Connected => "CONNECTED",
            PawLinkPhase::Failed(_) => "LINK FAILED",
        }
    }

    /// Rough 0..1 signal strength for the climbing bars, derived from phase.
    pub fn nominal_signal(&self) -> f32 {
        match self {
            PawLinkPhase::Idle => 0.0,
            PawLinkPhase::PoweringUp => 0.1,
            PawLinkPhase::Acquiring { found, total } => {
                0.15 + 0.5 * (*found as f32 / (*total).max(1) as f32)
            }
            PawLinkPhase::Handshaking => 0.7,
            PawLinkPhase::Locked => 0.9,
            PawLinkPhase::Transmitting | PawLinkPhase::AwaitingReply => 1.0,
            PawLinkPhase::Receiving | PawLinkPhase::Connected => 1.0,
            PawLinkPhase::Failed(_) => 0.0,
        }
    }
}

/// Flavored uplink failures. Each maps to its own headline, detail string and
/// a `signalLost` audio variation (see [`PawLinkError::audio_variation`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PawLinkError {
    SolarFlare,
    HairballInterference,
    CatSatOnRouter,
    BirdDistracted,
    LowTreats,
}

impl PawLinkError {
    pub const ALL: [PawLinkError; 5] = [
        PawLinkError::SolarFlare,
        PawLinkError::HairballInterference,
        PawLinkError::CatSatOnRouter,
        PawLinkError::BirdDistracted,
        PawLinkError::LowTreats,
    ];

    /// The terse uppercase headline shown on the failed HUD.
    pub fn headline(&self) -> &'static str {
        match self {
            PawLinkError::SolarFlare => "SOLAR FLARE",
            PawLinkError::HairballInterference => "HAIRBALL INTERFERENCE",
            PawLinkError::CatSatOnRouter => "ROUTER OCCUPIED",
            PawLinkError::BirdDistracted => "BIRD OFF-TASK",
            PawLinkError::LowTreats => "LOW TREATS",
        }
    }

    /// The longer, sillier explanation.
    pub fn detail(&self) -> &'static str {
        match self {
            PawLinkError::SolarFlare => "whiskers scrambled · re-aim antenna and try again",
            PawLinkError::HairballInterference => "uplink obstructed by a coughed-up payload",
            PawLinkError::CatSatOnRouter => "a cat is sitting on the router. it will not move.",
            PawLinkError::BirdDistracted => "uplink bird chasing a moth · ETA unknown",
            PawLinkError::LowTreats => "insufficient treats to maintain orbit",
        }
    }

    /// A 0..1 "color" handed to the audio engine so each failure drops out
    /// differently (e.g. detune amount / dropout sharpness).
    pub fn audio_variation(&self) -> f32 {
        match self {
            PawLinkError::SolarFlare => 0.9,
            PawLinkError::HairballInterference => 0.45,
            PawLinkError::CatSatOnRouter => 0.2,
            PawLinkError::BirdDistracted => 0.65,
            PawLinkError::LowTreats => 0.35,
        }
    }
}

/// How "realistic" — i.e. how comically slow — the constellation behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RealismProfile {
    /// Snappy; barely a ceremony. For the impatient cat.
    Instant,
    /// LEO-plausible (~550ms reply latency), packet loss on.
    RealisticLeo,
    /// Maximum theatre. Long, indulgent, dramatic.
    Dramatic,
}

impl RealismProfile {
    pub const ALL: [RealismProfile; 3] = [
        RealismProfile::Instant,
        RealismProfile::RealisticLeo,
        RealismProfile::Dramatic,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            RealismProfile::Instant => "Instant",
            RealismProfile::RealisticLeo => "Realistic LEO",
            RealismProfile::Dramatic => "Dramatic",
        }
    }

    pub fn blurb(&self) -> &'static str {
        match self {
            RealismProfile::Instant => "for the impatient cat",
            RealismProfile::RealisticLeo => "~550ms latency · packet loss on",
            RealismProfile::Dramatic => "maximum uplink theatre",
        }
    }

    /// Resolved phase durations and probabilities for this profile.
    pub fn timings(&self) -> Timings {
        match self {
            RealismProfile::Instant => Timings {
                powering_up: ms(150),
                acquiring: ms(500),
                handshaking: ms(300),
                locked: ms(120),
                transmitting: ms(350),
                awaiting_reply: ms(200),
                receiving: ms(300),
                total_satellites: 12,
                failure_chance: 0.05,
                packet_loss_chance: 0.0,
            },
            RealismProfile::RealisticLeo => Timings {
                powering_up: ms(800),
                acquiring: ms(2500),
                handshaking: ms(1500),
                locked: ms(300),
                transmitting: ms(1000),
                awaiting_reply: ms(550),
                receiving: ms(700),
                total_satellites: 12,
                failure_chance: 0.08,
                packet_loss_chance: 0.18,
            },
            RealismProfile::Dramatic => Timings {
                powering_up: ms(1200),
                acquiring: ms(4200),
                handshaking: ms(2400),
                locked: ms(450),
                transmitting: ms(1600),
                awaiting_reply: ms(2600),
                receiving: ms(1100),
                total_satellites: 12,
                failure_chance: 0.10,
                packet_loss_chance: 0.12,
            },
        }
    }
}

const fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// Concrete, profile-resolved timing + probability knobs the connection
/// manager reads when running a ceremony.
#[derive(Clone, Copy, Debug)]
pub struct Timings {
    pub powering_up: Duration,
    pub acquiring: Duration,
    pub handshaking: Duration,
    pub locked: Duration,
    pub transmitting: Duration,
    pub awaiting_reply: Duration,
    pub receiving: Duration,
    pub total_satellites: u32,
    /// Probability [0,1] the uplink fails outright before delivery.
    pub failure_chance: f64,
    /// Probability [0,1] the reply is dropped (Realistic LEO comedy).
    pub packet_loss_chance: f64,
}
