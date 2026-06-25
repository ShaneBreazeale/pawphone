//! `PawLinkConnectionManager` — the state machine at the centre of everything.
//!
//! A dedicated worker thread is Rust's natural shape for "drive it as an async
//! sequence": `send()` posts a request, the worker walks
//! `idle → poweringUp → … → connected | failed`, sleeping for the
//! profile-resolved duration of each beat. On every transition it (a) updates
//! `SharedState` for the UI to poll and (b) fires the matching audio + haptic
//! event. Audio/haptic work happens here, off the UI thread; the UI only reads.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use crate::audio::AudioControl;
use crate::haptics::PawHaptics;
use crate::models::{Cat, MeowPhrase};
use crate::personality;
use crate::phase::{PawLinkError, PawLinkPhase, RealismProfile};
use crate::telemetry;

const TELEMETRY_CAP: usize = 80;

/// What the worker produces when a ceremony ends. The app applies it (DB +
/// in-memory model) on the main thread.
#[derive(Clone, Debug)]
pub enum Outcome {
    Delivered { reply: Option<MeowPhrase> },
    Lost(PawLinkError),
}

/// Everything the UI reads each frame. Owned by the worker, read under a brief
/// lock by the main thread. The audio thread does NOT read this (it is driven
/// by lock-free commands), so the lock here is never on the audio hot path.
pub struct SharedState {
    pub phase: PawLinkPhase,
    pub telemetry: VecDeque<String>,
    pub signal: f32,
    /// Bumps each time a new `outcome` is ready, so the app applies it once.
    pub seq: u64,
    pub outcome: Option<Outcome>,
    pub running: bool,
}

impl SharedState {
    fn idle() -> SharedState {
        SharedState {
            phase: PawLinkPhase::Idle,
            telemetry: VecDeque::with_capacity(TELEMETRY_CAP),
            signal: 0.0,
            seq: 0,
            outcome: None,
            running: false,
        }
    }
}

/// A queued send. `incoming` is the human-readable intent passed to the reply
/// logic for (future) context-aware replies.
pub struct SendRequest {
    pub cat: Cat,
    pub incoming: String,
    pub profile: RealismProfile,
}

pub struct PawLinkConnectionManager {
    pub shared: Arc<Mutex<SharedState>>,
    tx: mpsc::Sender<SendRequest>,
    _worker: thread::JoinHandle<()>,
}

impl PawLinkConnectionManager {
    pub fn new(mut audio: AudioControl, haptics: PawHaptics) -> PawLinkConnectionManager {
        let shared = Arc::new(Mutex::new(SharedState::idle()));
        let (tx, rx) = mpsc::channel::<SendRequest>();
        let worker_shared = Arc::clone(&shared);

        let worker = thread::Builder::new()
            .name("pawlink-uplink".into())
            .spawn(move || {
                while let Ok(req) = rx.recv() {
                    run_ceremony(req, &worker_shared, &mut audio, &haptics);
                }
            })
            .expect("failed to spawn uplink worker");

        PawLinkConnectionManager { shared, tx, _worker: worker }
    }

    /// Queue a send. Returns immediately; the ceremony runs on the worker.
    pub fn send(&self, req: SendRequest) {
        let _ = self.tx.send(req);
    }
}

// ── ceremony ────────────────────────────────────────────────────────────────

fn run_ceremony(
    req: SendRequest,
    shared: &Arc<Mutex<SharedState>>,
    audio: &mut AudioControl,
    haptics: &PawHaptics,
) {
    let t = req.profile.timings();

    begin(shared);
    log(shared, telemetry::ambient());

    // poweringUp — relay click + rising hum.
    set_phase(shared, PawLinkPhase::PoweringUp);
    log(shared, telemetry::powering_up());
    audio.power_up();
    thread::sleep(t.powering_up);

    // acquiring — count satellites up, ping per find, light tick each.
    let total = t.total_satellites;
    audio.acquiring();
    set_phase(shared, PawLinkPhase::Acquiring { found: 0, total });
    let step = t.acquiring / total.max(1);
    for found in 1..=total {
        audio.ping(found);
        haptics.satellite_found();
        set_phase(shared, PawLinkPhase::Acquiring { found, total });
        log(shared, telemetry::acquiring(found, total));
        thread::sleep(step);
    }

    // handshaking — the money sound.
    set_phase(shared, PawLinkPhase::Handshaking);
    log(shared, telemetry::handshaking());
    audio.handshake();
    thread::sleep(t.handshaking);

    // locked — chime + thunk.
    set_phase(shared, PawLinkPhase::Locked);
    log(shared, telemetry::locked());
    audio.lock();
    haptics.lock();
    thread::sleep(t.locked);

    // transmitting — outbound chitter + buzz.
    set_phase(shared, PawLinkPhase::Transmitting);
    log(shared, telemetry::transmitting());
    audio.transmit();
    haptics.transmit();
    thread::sleep(t.transmitting);

    // Resolve: uplink failure (flavored) before delivery?
    if let Some(err) = personality::roll_failure(&t) {
        fail(shared, audio, haptics, err);
        return;
    }

    // awaitingReply — latency drama.
    set_phase(shared, PawLinkPhase::AwaitingReply);
    log(shared, telemetry::awaiting());
    thread::sleep(t.awaiting_reply);

    // Reply packet loss (Realistic LEO comedy): drop it, force a resend.
    if personality::roll_packet_loss(&t) {
        log(shared, "REPLY LOST IN ORBIT".into());
        fail(shared, audio, haptics, PawLinkError::BirdDistracted);
        return;
    }

    // receiving — incoming meow.
    set_phase(shared, PawLinkPhase::Receiving);
    log(shared, telemetry::receiving());
    audio.incoming();
    haptics.receive();
    let reply = personality::weighted_reply(&req.cat, &req.incoming);
    thread::sleep(t.receiving);

    // connected — done.
    set_phase(shared, PawLinkPhase::Connected);
    finish(shared, Outcome::Delivered { reply: Some(reply) });
}

fn fail(
    shared: &Arc<Mutex<SharedState>>,
    audio: &mut AudioControl,
    haptics: &PawHaptics,
    err: PawLinkError,
) {
    audio.signal_lost(err.audio_variation());
    haptics.failed();
    set_phase(shared, PawLinkPhase::Failed(err));
    log(shared, format!("LINK FAILED · {}", err.headline()));
    finish(shared, Outcome::Lost(err));
}

// ── shared-state helpers (brief locks; never on the audio path) ──────────────

fn begin(shared: &Arc<Mutex<SharedState>>) {
    if let Ok(mut s) = shared.lock() {
        s.running = true;
        s.outcome = None;
        s.telemetry.clear();
        s.signal = 0.0;
    }
}

fn set_phase(shared: &Arc<Mutex<SharedState>>, phase: PawLinkPhase) {
    if let Ok(mut s) = shared.lock() {
        s.signal = phase.nominal_signal();
        s.phase = phase;
    }
}

fn log(shared: &Arc<Mutex<SharedState>>, line: String) {
    if let Ok(mut s) = shared.lock() {
        if s.telemetry.len() >= TELEMETRY_CAP {
            s.telemetry.pop_front();
        }
        s.telemetry.push_back(line);
    }
}

fn finish(shared: &Arc<Mutex<SharedState>>, outcome: Outcome) {
    if let Ok(mut s) = shared.lock() {
        s.outcome = Some(outcome);
        s.seq = s.seq.wrapping_add(1);
        s.running = false;
    }
}
