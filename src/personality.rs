//! The app's personality — meow translation, reply weighting, and failure
//! injection.
//!
//! ┌───────────────────────────────────────────────────────────────────────┐
//! │  YOUR CODE LIVES HERE (learning mode).                                  │
//! │                                                                         │
//! │  The data banks below are done. The four functions marked TODO(you) ship│
//! │  with the simplest baseline that compiles and runs, so the app works    │
//! │  end-to-end today. Each is a real design decision — replace the baseline│
//! │  with something with taste. Guidance is in the comment above each.      │
//! └───────────────────────────────────────────────────────────────────────┘

use rand::seq::IndexedRandom;

use crate::models::{Cat, CatPersona, MeowPhrase};
use crate::phase::{PawLinkError, Timings};

// ── Data banks (done — these are content, not logic) ────────────────────────

/// Preset utterances for the compose-bar phrasebook picker.
pub fn phrasebook() -> Vec<MeowPhrase> {
    vec![
        MeowPhrase::new("Mrrp?", "where is the food"),
        MeowPhrase::new("MROW", "the food bowl is empty and I am dying"),
        MeowPhrase::new("mew", "hello, small human"),
        MeowPhrase::new("prrrt", "I acknowledge your existence"),
        MeowPhrase::new("MRRRROW", "let me out. also let me in."),
        MeowPhrase::new("hsss", "the vacuum has returned"),
        MeowPhrase::new("chirp chirp", "bird outside — this is extremely urgent"),
        MeowPhrase::new("😼", "I tolerate you (slow blink)"),
        MeowPhrase::new("brrrap", "I have knocked something off the table"),
        MeowPhrase::new("meow meow meow", "pet me. do not pet me. decide later."),
    ]
}

/// Canned replies, grouped by the replying cat's persona.
fn reply_bank(persona: CatPersona) -> Vec<MeowPhrase> {
    match persona {
        CatPersona::FoodObsessed => vec![
            MeowPhrase::new("MROW MROW", "is that food. is what you said food."),
            MeowPhrase::new("mrrrp!", "the bowl. check the bowl. now."),
            MeowPhrase::new("nyom", "I will respond after a snack"),
            MeowPhrase::new("MRRROW", "treats were mentioned. I heard treats."),
        ],
        CatPersona::Aloof => vec![
            MeowPhrase::new("...", "I have read your message and chosen silence"),
            MeowPhrase::new("mew.", "noted. uninterested."),
            MeowPhrase::new("😼", "(slow blink, turns away)"),
            MeowPhrase::new("prt", "fine. acknowledged. don't make it weird."),
        ],
        CatPersona::Chaotic => vec![
            MeowPhrase::new("BRRRAP", "I knocked a glass off the counter, ask me why"),
            MeowPhrase::new("mrr-AH", "ZOOMIES INBOUND take cover"),
            MeowPhrase::new("chrrp chrp chrp", "there is a moth. EVERYTHING has changed."),
            MeowPhrase::new("mlem", "I forgot what we were talking about"),
        ],
        CatPersona::Needy => vec![
            MeowPhrase::new("mrrrrrr", "come back. why did you leave. come back."),
            MeowPhrase::new("MEW MEW", "pet me immediately, it has been 4 seconds"),
            MeowPhrase::new("prrrrt?", "are you still there. are you. still. there."),
            MeowPhrase::new("mrp mrp mrp", "sit down so I can sit on you"),
        ],
    }
}

// ── TODO(you) #1 — English → meow translation ───────────────────────────────
//
// Turn free-typed human text into cat speech, with the original kept as the
// subtitle. The mapping should be DETERMINISTIC (same input → same meow) so a
// resent message looks identical.
//
// Things worth deciding:
//   • Length/energy: long or ALL-CAPS input → longer / louder meow (MROW vs mew)?
//   • A small word→sound lexicon ("food"→"MROW", "no"→"hss", "love"→"prrrt")?
//   • Punctuation: "?" → "Mrrp?", "!" → emphasis.
// Baseline below just scales one meow by input length. Make it characterful.
pub fn translate_to_meow(text: &str) -> MeowPhrase {
    // --- baseline (replace me) ---
    let trimmed = text.trim();
    let len = trimmed.chars().count();
    let meow = if len == 0 {
        "mew".to_string()
    } else if trimmed.chars().filter(|c| c.is_uppercase()).count() * 2 > len.max(1) {
        "MROW".to_string()
    } else {
        let rs = "r".repeat((len / 4).clamp(1, 5));
        format!("m{rs}p")
    };
    MeowPhrase { meow, subtitle: trimmed.to_string() }
    // --- end baseline ---
}

// ── TODO(you) #2 — weighted reply selection ─────────────────────────────────
//
// Choose the other cat's reply. The baseline ignores everything and picks at
// random from the persona bank. Make it feel intentional:
//   • Weight by persona (a FoodObsessed cat should skew to food replies).
//   • React to `incoming` (did the human mention food / "no" / "out"?).
//   • Maybe rarely return a delightful non-sequitur regardless of input.
// Return any MeowPhrase — it need not come from the bank.
pub fn weighted_reply(cat: &Cat, incoming: &str) -> MeowPhrase {
    let _ = incoming; // baseline ignores context — you won't.
    // --- baseline (replace me) ---
    let bank = reply_bank(cat.persona);
    bank.choose(&mut rand::rng()).cloned().unwrap_or_else(|| MeowPhrase::new("mew", "..."))
    // --- end baseline ---
}

// ── TODO(you) #3 — failure injection ────────────────────────────────────────
//
// Decide whether this uplink fails outright (→ a flavored PawLinkError) before
// delivery. `timings.failure_chance` is the profile's base rate (0.05–0.10).
// Worth considering:
//   • Pick the error by context/flavor, not pure random (LowTreats more likely
//     on long messages? CatSatOnRouter when the contact is "napping"?).
//   • An anti-frustration rule (don't fail two sends in a row) — but that needs
//     state, so you'd thread a counter in via ConnectionManager.
// Baseline: flat dice roll, uniform error pick.
pub fn roll_failure(timings: &Timings) -> Option<PawLinkError> {
    // --- baseline (replace me) ---
    if rand::random_bool(timings.failure_chance) {
        PawLinkError::ALL.choose(&mut rand::rng()).copied()
    } else {
        None
    }
    // --- end baseline ---
}

// ── TODO(you) #4 — reply packet loss ────────────────────────────────────────
//
// Even on a delivered message, the *reply* can be dropped (Realistic LEO
// comedy) — the user then sees a signalLost and resends. Use
// `timings.packet_loss_chance`. Baseline: flat roll. Consider tying loss to
// signal strength or making it rarer right after a previous loss.
pub fn roll_packet_loss(timings: &Timings) -> bool {
    // --- baseline (replace me) ---
    rand::random_bool(timings.packet_loss_chance)
    // --- end baseline ---
}
