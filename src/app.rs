//! The application shell: owns every subsystem, polls the connection worker
//! each frame, applies outcomes to the store, and routes between screens.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use egui::RichText;

use crate::audio::PawLinkAudioEngine;
use crate::connection::{Outcome, PawLinkConnectionManager, SendRequest};
use crate::haptics::PawHaptics;
use crate::models::{Cat, DeliveryStatus, Message, MeowPhrase, Screen};
use crate::persistence::{Store, now_ms};
use crate::personality;
use crate::phase::{PawLinkPhase, RealismProfile};
use crate::views::theme;

/// How long the success HUD lingers after delivery before auto-hiding (seconds).
const CONNECTED_LINGER: f64 = 1.1;

pub struct PawPhoneApp {
    pub store: Store,
    pub audio: PawLinkAudioEngine,
    pub conn: PawLinkConnectionManager,
    /// Shared with the worker's `PawHaptics`; toggled from Settings.
    pub haptics_enabled: Arc<AtomicBool>,

    pub cats: Vec<Cat>,
    /// Messages for the currently open thread (reloaded on change).
    pub current_messages: Vec<Message>,
    pub screen: Screen,
    pub profile: RealismProfile,
    pub compose: String,

    /// Last applied outcome sequence number, so each outcome applies once.
    last_seq: u64,
    pending_message_id: Option<i64>,
    pub(crate) pending_cat_id: Option<i64>,

    pub hud_visible: bool,
    /// egui time at delivery; drives the success-HUD auto-hide.
    connected_at: Option<f64>,
    /// A ceremony is dispatched and awaiting its outcome. Set on send/resend,
    /// cleared when an outcome is applied. Suppresses the failure card's
    /// RESEND/DISMISS buttons during the worker wake-up window so a second
    /// click can't enqueue a duplicate ceremony.
    pub(crate) busy: bool,

    // Live ceremony snapshot, refreshed each frame for the HUD.
    pub live_phase: PawLinkPhase,
    pub live_signal: f32,
    pub live_telemetry: Vec<String>,
}

impl PawPhoneApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> PawPhoneApp {
        theme::install(&cc.egui_ctx);

        let store = Store::open().unwrap_or_else(|_| Store::in_memory());
        let cats = store.load_cats();

        let (audio, audio_ctl) = PawLinkAudioEngine::new();
        let haptics_enabled = Arc::new(AtomicBool::new(true));
        let haptics = PawHaptics::new(Arc::clone(&haptics_enabled));
        let conn = PawLinkConnectionManager::new(audio_ctl, haptics);

        PawPhoneApp {
            store,
            audio,
            conn,
            haptics_enabled,
            cats,
            current_messages: Vec::new(),
            screen: Screen::Contacts,
            profile: RealismProfile::RealisticLeo,
            compose: String::new(),
            last_seq: 0,
            pending_message_id: None,
            pending_cat_id: None,
            hud_visible: false,
            connected_at: None,
            busy: false,
            live_phase: PawLinkPhase::Idle,
            live_signal: 0.0,
            live_telemetry: Vec::new(),
        }
    }

    // ── lookups / navigation ────────────────────────────────────────────────

    pub fn cat(&self, id: i64) -> Option<Cat> {
        self.cats.iter().find(|c| c.id == id).cloned()
    }

    pub fn open_thread(&mut self, cat_id: i64) {
        self.current_messages = self.store.load_messages(cat_id);
        self.screen = Screen::Thread { cat_id };
    }

    fn reload_thread(&mut self) {
        if let Screen::Thread { cat_id } = self.screen {
            self.current_messages = self.store.load_messages(cat_id);
        }
    }

    pub fn haptics_on(&self) -> bool {
        self.haptics_enabled.load(Ordering::Relaxed)
    }
    pub fn set_haptics(&self, on: bool) {
        self.haptics_enabled.store(on, Ordering::Relaxed);
    }

    // ── sending ──────────────────────────────────────────────────────────────

    /// Send a ready-made phrase (from the phrasebook).
    pub fn send_phrase(&mut self, phrase: MeowPhrase) {
        let cat_id = match self.screen {
            Screen::Thread { cat_id } => cat_id,
            _ => return,
        };
        let Some(cat) = self.cat(cat_id) else { return };

        let msg = Message {
            id: 0,
            cat_id,
            from_me: true,
            meow: phrase.meow.clone(),
            subtitle: phrase.subtitle.clone(),
            status: DeliveryStatus::Sending,
            created_at: now_ms(),
        };
        let id = self.store.insert_message(&msg);
        self.pending_message_id = Some(id);
        self.pending_cat_id = Some(cat_id);
        self.reload_thread();

        self.hud_visible = true;
        self.connected_at = None;
        self.busy = true;
        self.conn.send(SendRequest { cat, incoming: phrase.subtitle, profile: self.profile });
    }

    /// Translate the free-text compose buffer into meows and send it.
    pub fn send_typed(&mut self) {
        let text = self.compose.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.compose.clear();
        let phrase = personality::translate_to_meow(&text);
        self.send_phrase(phrase);
    }

    /// Re-run the ceremony for the last failed message.
    pub fn resend(&mut self) {
        let (Some(id), Some(cat_id)) = (self.pending_message_id, self.pending_cat_id) else {
            return;
        };
        let Some(cat) = self.cat(cat_id) else { return };
        let Some(msg) = self.current_messages.iter().find(|m| m.id == id).cloned() else {
            return;
        };

        self.store.update_message_status(id, DeliveryStatus::Sending);
        self.reload_thread();
        self.hud_visible = true;
        self.connected_at = None;
        self.busy = true;
        self.conn.send(SendRequest { cat, incoming: msg.subtitle, profile: self.profile });
    }

    pub fn dismiss_hud(&mut self) {
        self.hud_visible = false;
        self.connected_at = None;
    }

    // ── per-frame plumbing ───────────────────────────────────────────────────

    fn poll(&mut self, ctx: &egui::Context) {
        let snapshot = {
            let Ok(s) = self.conn.shared.lock() else { return };
            let new_outcome = if s.seq > self.last_seq { s.outcome.clone() } else { None };
            (
                s.phase.clone(),
                s.signal,
                s.telemetry.iter().cloned().collect::<Vec<_>>(),
                s.seq,
                new_outcome,
            )
        };
        let (phase, signal, telemetry, seq, outcome) = snapshot;
        self.live_phase = phase;
        self.live_signal = signal;
        self.live_telemetry = telemetry;

        if seq > self.last_seq {
            self.last_seq = seq;
            if let Some(o) = outcome {
                self.apply_outcome(o, ctx);
            }
        }

        if let Some(t0) = self.connected_at {
            let now = ctx.input(|i| i.time);
            if now - t0 > CONNECTED_LINGER {
                self.hud_visible = false;
                self.connected_at = None;
            }
        }
    }

    fn apply_outcome(&mut self, outcome: Outcome, ctx: &egui::Context) {
        // Any outcome means the in-flight ceremony has ended.
        self.busy = false;
        let Some(id) = self.pending_message_id else { return };
        match outcome {
            Outcome::Delivered { reply } => {
                self.store.update_message_status(id, DeliveryStatus::Delivered);
                if let (Some(reply), Some(cat_id)) = (reply, self.pending_cat_id) {
                    let reply_msg = Message {
                        id: 0,
                        cat_id,
                        from_me: false,
                        meow: reply.meow,
                        subtitle: reply.subtitle,
                        status: DeliveryStatus::Delivered,
                        created_at: now_ms(),
                    };
                    self.store.insert_message(&reply_msg);
                }
                self.pending_message_id = None;
                self.connected_at = Some(ctx.input(|i| i.time));
            }
            Outcome::Lost(err) => {
                self.store.update_message_status(id, DeliveryStatus::Lost(err));
                // Keep the HUD up (failure card with resend) and keep `pending`.
            }
        }
        self.reload_thread();
    }

    // ── chrome ───────────────────────────────────────────────────────────────

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("=^.^= PAW PHONE").color(theme::GREEN));
            ui.label(RichText::new("powered by PawLink™").small().color(theme::GREEN_FAINT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙ settings").clicked() {
                    self.screen = Screen::Settings;
                }
                if ui.button("contacts").clicked() {
                    self.screen = Screen::Contacts;
                }
            });
        });
    }
}

fn footer(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("PawLink is a fictional constellation. No actual cats were connected to space.")
                .small()
                .color(theme::GREEN_FAINT),
        );
    });
}

impl eframe::App for PawPhoneApp {
    // eframe 0.34: `ui` is the required entry point; `update` is a deprecated
    // provided method. We get a root `Ui` and lay panels out inside it.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);

        if self.hud_visible {
            self.hud_overlay(ui);
            // Animate the orbit + keep polling the worker for transitions.
            ctx.request_repaint();
            return;
        }

        egui::Panel::top("header").show_inside(ui, |ui| self.header(ui));
        egui::Panel::bottom("footer").show_inside(ui, footer);
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let screen = self.screen.clone();
            match screen {
                Screen::Contacts => self.contacts_view(ui),
                Screen::Thread { .. } => self.thread_view(ui),
                Screen::Settings => self.settings_view(ui),
            }
        });
    }
}
