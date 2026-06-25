//! Plain data: the cats, their messages, and how a message is doing on its
//! way to space. No behaviour here beyond trivial conversions.

use serde::{Deserialize, Serialize};

use crate::phase::PawLinkError;

/// A contact. The other end of every (simulated) conversation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cat {
    pub id: i64,
    pub name: String,
    /// Single emoji avatar — no bundled image assets, on brand.
    pub avatar: String,
    /// 0..1 signal-strength badge.
    pub signal: f32,
    pub status: CatStatus,
    /// Shapes how this cat tends to reply (used by the reply-weighting logic).
    pub persona: CatPersona,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CatStatus {
    Online,
    Napping,
}

impl CatStatus {
    pub fn label(&self) -> &'static str {
        match self {
            CatStatus::Online => "online",
            CatStatus::Napping => "napping",
        }
    }
}

/// A cat's disposition. Hints for the (user-authored) reply weighting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CatPersona {
    /// Everything routes back to food.
    FoodObsessed,
    /// Tolerates you, barely.
    Aloof,
    /// Non-sequiturs and zoomies.
    Chaotic,
    /// Demands attention, now.
    Needy,
}

/// One chat bubble.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    /// The contact this message belongs to (conversation key).
    pub cat_id: i64,
    /// `true` if the human (well, the human's cat) sent it — renders on the right.
    pub from_me: bool,
    /// The meow text actually "sent".
    pub meow: String,
    /// Plain-English subtitle / translation.
    pub subtitle: String,
    pub status: DeliveryStatus,
    /// Epoch milliseconds, for ordering.
    pub created_at: i64,
}

/// Lifecycle of an outbound message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryStatus {
    /// Mid-ceremony, not yet confirmed.
    Sending,
    /// Confirmed delivered (and a reply may follow).
    Delivered,
    /// Uplink failed; user can resend.
    Lost(PawLinkError),
}

impl DeliveryStatus {
    pub fn glyph(&self) -> &'static str {
        match self {
            DeliveryStatus::Sending => "◌ sending",
            DeliveryStatus::Delivered => "✓ delivered",
            DeliveryStatus::Lost(_) => "✕ lost",
        }
    }
}

/// A preset cat utterance with its human translation. Used by both the
/// phrasebook picker and the reply bank.
#[derive(Clone, Debug)]
pub struct MeowPhrase {
    pub meow: String,
    pub subtitle: String,
}

impl MeowPhrase {
    pub fn new(meow: &str, subtitle: &str) -> MeowPhrase {
        MeowPhrase { meow: meow.to_string(), subtitle: subtitle.to_string() }
    }
}

/// Which screen the app is showing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    Contacts,
    Thread { cat_id: i64 },
    Settings,
}
