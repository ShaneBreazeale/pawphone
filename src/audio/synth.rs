//! Procedural synthesis. Every PawLink sound is generated here, sample by
//! sample, inside the audio render callback — nothing is loaded from disk.
//!
//! Real-time discipline: this code runs on the audio thread. It must never
//! allocate, lock, or block. The voice pool is pre-sized; commands arrive
//! lock-free over a ring buffer; randomness comes from a tiny xorshift PRNG.

use std::f32::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use ringbuf::HeapCons;
use ringbuf::traits::Consumer;

use super::AudioCommand;

/// Hard cap on simultaneous voices. The pool is allocated once; the callback
/// never grows it (a dropped voice under extreme load is inaudible here).
const MAX_VOICES: usize = 32;

/// Cheap, allocation-free PRNG for the audio thread. xorshift32.
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng(seed | 1)
    }
    #[inline]
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Uniform white noise in [-1, 1].
    #[inline]
    pub fn noise(&mut self) -> f32 {
        (self.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    /// Uniform in [0, 1).
    #[inline]
    pub fn unit(&mut self) -> f32 {
        self.next_u32() as f32 / (u32::MAX as f32 + 1.0)
    }
    /// Uniform in [lo, hi).
    #[inline]
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}

/// Attack/release envelope — short fades keep every voice click-free.
#[inline]
fn ar_env(t: f32, dur: f32, atk: f32, rel: f32) -> f32 {
    if t < 0.0 || t > dur {
        return 0.0;
    }
    let a = if atk > 0.0 { (t / atk).min(1.0) } else { 1.0 };
    let r = if rel > 0.0 { ((dur - t) / rel).clamp(0.0, 1.0) } else { 1.0 };
    a * r
}

/// Smooth limiter on the master sum so layered voices never hard-clip/pop.
#[inline]
fn soft_clip(x: f32) -> f32 {
    x.tanh()
}

/// One-pole low-pass. `a` is the smoothing coefficient (0..1).
struct OnePole {
    z: f32,
    a: f32,
}
impl OnePole {
    fn new(cutoff_hz: f32, sr: f32) -> OnePole {
        // a = 1 - e^{-2π fc / fs}
        let a = 1.0 - (-TAU * cutoff_hz / sr).exp();
        OnePole { z: 0.0, a }
    }
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        self.z += self.a * (x - self.z);
        self.z
    }
}

/// Chamberlin state-variable filter; we tap the band-pass for meow formants.
struct Svf {
    low: f32,
    band: f32,
    f: f32,
    damp: f32,
}
impl Svf {
    fn new(cutoff_hz: f32, q: f32, sr: f32) -> Svf {
        let f = 2.0 * (std::f32::consts::PI * cutoff_hz / sr).sin();
        Svf { low: 0.0, band: 0.0, f: f.min(1.0), damp: (1.0 / q).min(1.9) }
    }
    #[inline]
    fn band_pass(&mut self, x: f32) -> f32 {
        self.low += self.f * self.band;
        let high = x - self.low - self.damp * self.band;
        self.band += self.f * high;
        self.band
    }
}

/// Every distinct sound is a `Voice`. Stored inline in the pool (no heap),
/// rendered sample by sample, dropped when `done()`.
enum Voice {
    PowerUp { t: f32, dur: f32, phase: f32 },
    NoiseBed { t: f32, dur: f32, lp: OnePole },
    Ping { t: f32, dur: f32, phase: f32, freq: f32 },
    Handshake {
        t: f32,
        dur: f32,
        phase_a: f32,
        phase_b: f32,
        freq_a: f32,
        freq_b: f32,
        hop_t: f32,
        scramble_lp: OnePole,
    },
    Lock { t: f32, dur: f32, p1: f32, p2: f32 },
    Transmit { t: f32, dur: f32, since: f32, interval: f32, grain_env: f32, phase: f32 },
    Incoming { t: f32, dur: f32, saw: f32, formant: Svf },
    SignalLost { t: f32, dur: f32, phase_a: f32, phase_b: f32, variation: f32 },
}

impl Voice {
    /// Seconds elapsed vs total duration.
    fn done(&self) -> bool {
        let (t, dur) = match self {
            Voice::PowerUp { t, dur, .. }
            | Voice::NoiseBed { t, dur, .. }
            | Voice::Ping { t, dur, .. }
            | Voice::Handshake { t, dur, .. }
            | Voice::Lock { t, dur, .. }
            | Voice::Transmit { t, dur, .. }
            | Voice::Incoming { t, dur, .. }
            | Voice::SignalLost { t, dur, .. } => (*t, *dur),
        };
        t >= dur
    }

    /// Render the next mono sample and advance internal state.
    fn render(&mut self, sr: f32, rng: &mut Rng) -> f32 {
        let dt = 1.0 / sr;
        match self {
            // ── powerUp: 40→120 Hz sine sweep + sharp relay click ──────────
            Voice::PowerUp { t, dur, phase } => {
                let freq = 40.0 + (120.0 - 40.0) * (*t / *dur);
                *phase += TAU * freq / sr;
                if *phase > TAU {
                    *phase -= TAU;
                }
                // Hum swells in as the relay engages.
                let hum = phase.sin() * (*t / *dur) * ar_env(*t, *dur, 0.02, 0.12);
                // Single sharp click decaying over ~20ms at the very start.
                let click = rng.noise() * (-(*t) * 220.0).exp() * 0.6;
                *t += dt;
                hum * 0.5 + click
            }
            // ── acquiring bed: low filtered noise "listening to the void" ──
            Voice::NoiseBed { t, dur, lp } => {
                let swell = ar_env(*t, *dur, 0.3, 0.4);
                let n = lp.process(rng.noise());
                *t += dt;
                n * 0.09 * swell
            }
            // ── ping: 1.2kHz sine burst, exp decay, pitched by step ────────
            Voice::Ping { t, dur, phase, freq } => {
                *phase += TAU * *freq / sr;
                if *phase > TAU {
                    *phase -= TAU;
                }
                let decay = (-(*t) * 9.0).exp();
                // Short release so the tail ramps to zero before done() drops
                // the voice — otherwise the exp tail is still ~-40dBFS at cutoff
                // and the hard stop clicks. Matches the click-free discipline.
                let s = phase.sin() * decay * 0.45 * ar_env(*t, *dur, 0.0, 0.02);
                *t += dt;
                s
            }
            // ── handshake: dual-tone FSK warble + data scramble, builds ─────
            Voice::Handshake { t, dur, phase_a, phase_b, freq_a, freq_b, hop_t, scramble_lp } => {
                let progress = (*t / *dur).clamp(0.0, 1.0);
                // Hop faster as the burst builds (every ~90ms → ~25ms).
                let hop_interval = 0.090 - 0.065 * progress;
                *hop_t += dt;
                if *hop_t >= hop_interval {
                    *hop_t = 0.0;
                    *freq_a = rng.range(800.0, 2400.0);
                    *freq_b = rng.range(800.0, 2400.0);
                }
                *phase_a += TAU * *freq_a / sr;
                *phase_b += TAU * *freq_b / sr;
                if *phase_a > TAU {
                    *phase_a -= TAU;
                }
                if *phase_b > TAU {
                    *phase_b -= TAU;
                }
                let tones = (phase_a.sin() + phase_b.sin()) * 0.25;
                // Band-limited scramble, density rises with progress.
                let scramble = scramble_lp.process(rng.noise()) * (0.10 + 0.25 * progress);
                let env = ar_env(*t, *dur, 0.02, 0.10);
                *t += dt;
                (tones + scramble) * env
            }
            // ── lock: bright major-third confirmation chime ────────────────
            Voice::Lock { t, dur, p1, p2 } => {
                *p1 += TAU * 880.0 / sr; // A5
                *p2 += TAU * 1108.73 / sr; // C#6 (major third)
                if *p1 > TAU {
                    *p1 -= TAU;
                }
                if *p2 > TAU {
                    *p2 -= TAU;
                }
                let env = ar_env(*t, *dur, 0.004, *dur * 0.9);
                let s = (p1.sin() + p2.sin()) * 0.22 * env;
                *t += dt;
                s
            }
            // ── transmit: rapid clicky data chitter grains ─────────────────
            Voice::Transmit { t, dur, since, interval, grain_env, phase } => {
                *since += dt;
                if *since >= *interval {
                    *since = 0.0;
                    *interval = rng.range(0.060, 0.120);
                    *grain_env = 1.0;
                    *phase = 0.0;
                }
                // High clicky grain, fast exponential decay (~12ms).
                *phase += TAU * 2600.0 / sr;
                if *phase > TAU {
                    *phase -= TAU;
                }
                let s = phase.sin() * *grain_env * 0.35;
                *grain_env *= (-dt / 0.012).exp();
                let env = ar_env(*t, *dur, 0.01, 0.08);
                *t += dt;
                s * env
            }
            // ── incoming: a synthesized attempt at a meow ──────────────────
            Voice::Incoming { t, dur, saw, formant } => {
                let p = *t / *dur;
                // Pitch contour 300 → 500 → 250 Hz.
                let freq = if p < 0.4 {
                    300.0 + (500.0 - 300.0) * (p / 0.4)
                } else {
                    500.0 + (250.0 - 500.0) * ((p - 0.4) / 0.6)
                };
                *saw += freq / sr;
                if *saw >= 1.0 {
                    *saw -= 1.0;
                }
                let raw = 2.0 * *saw - 1.0; // sawtooth
                // Formant bandpass gives it a vowel-ish "mrow".
                let voiced = formant.band_pass(raw);
                // Amplitude vibrato ~6 Hz for that wavering cat quality.
                let vib = 1.0 + 0.15 * (TAU * 6.0 * *t).sin();
                let env = ar_env(*t, *dur, 0.03, 0.18);
                *t += dt;
                voiced * 0.8 * vib * env
            }
            // ── signalLost: descending detuned sweep + dropout ─────────────
            Voice::SignalLost { t, dur, phase_a, phase_b, variation } => {
                let p = *t / *dur;
                let freq = 600.0 * (-p * 2.2).exp() + 70.0; // swoop down
                let detune = 1.0 + 0.02 * *variation;
                *phase_a += TAU * freq / sr;
                *phase_b += TAU * (freq * detune) / sr;
                if *phase_a > TAU {
                    *phase_a -= TAU;
                }
                if *phase_b > TAU {
                    *phase_b -= TAU;
                }
                let tone = (phase_a.sin() + phase_b.sin()) * 0.25;
                // Sudden gate: tone cuts at ~60% and stutters into silence.
                let gate_at = 0.6;
                let s = if p < gate_at {
                    tone * ar_env(*t, *dur * gate_at, 0.01, 0.05)
                } else {
                    // brief noise stutter, then nothing
                    let stutter = if (p * 30.0) as i32 % 2 == 0 { rng.noise() * 0.15 } else { 0.0 };
                    stutter * (1.0 - (p - gate_at) / (1.0 - gate_at))
                };
                *t += dt;
                s
            }
        }
    }
}

/// Owns the live voice pool and turns [`AudioCommand`]s into sound. Lives
/// entirely on the audio thread.
pub struct SynthCore {
    sr: f32,
    channels: usize,
    voices: Vec<Voice>,
    cons: HeapCons<AudioCommand>,
    rng: Rng,
    volume: Arc<AtomicU32>,
    muted: Arc<AtomicBool>,
}

impl SynthCore {
    pub fn new(
        sr: f32,
        channels: usize,
        cons: HeapCons<AudioCommand>,
        volume: Arc<AtomicU32>,
        muted: Arc<AtomicBool>,
    ) -> SynthCore {
        SynthCore {
            sr,
            channels,
            voices: Vec::with_capacity(MAX_VOICES),
            cons,
            rng: Rng::new(0x1234_5678),
            volume,
            muted,
        }
    }

    fn spawn(&mut self, v: Voice) {
        if self.voices.len() < MAX_VOICES {
            self.voices.push(v);
        }
    }

    fn handle(&mut self, cmd: AudioCommand) {
        let sr = self.sr;
        match cmd {
            AudioCommand::PowerUp => self.spawn(Voice::PowerUp { t: 0.0, dur: 0.8, phase: 0.0 }),
            AudioCommand::Acquiring => {
                self.spawn(Voice::NoiseBed { t: 0.0, dur: 2.6, lp: OnePole::new(450.0, sr) })
            }
            AudioCommand::Ping { step } => {
                let freq = 1200.0 * (1.0 + 0.045 * step as f32);
                self.spawn(Voice::Ping { t: 0.0, dur: 0.42, phase: 0.0, freq });
            }
            AudioCommand::Handshake => self.spawn(Voice::Handshake {
                t: 0.0,
                dur: 1.5,
                phase_a: 0.0,
                phase_b: 0.0,
                freq_a: 1200.0,
                freq_b: 1600.0,
                hop_t: 0.0,
                scramble_lp: OnePole::new(3200.0, sr),
            }),
            AudioCommand::Lock => self.spawn(Voice::Lock { t: 0.0, dur: 0.32, p1: 0.0, p2: 0.0 }),
            AudioCommand::Transmit => self.spawn(Voice::Transmit {
                t: 0.0,
                dur: 1.0,
                since: 1.0,
                interval: 0.08,
                grain_env: 0.0,
                phase: 0.0,
            }),
            AudioCommand::Incoming => self.spawn(Voice::Incoming {
                t: 0.0,
                dur: 0.75,
                saw: 0.0,
                formant: Svf::new(1050.0, 3.5, sr),
            }),
            AudioCommand::SignalLost { variation } => self.spawn(Voice::SignalLost {
                t: 0.0,
                dur: 0.8,
                phase_a: 0.0,
                phase_b: 0.0,
                variation,
            }),
        }
    }

    /// The cpal render callback body: drain commands, sum voices, write out.
    pub fn render(&mut self, data: &mut [f32]) {
        while let Some(cmd) = self.cons.try_pop() {
            self.handle(cmd);
        }

        let gain = if self.muted.load(Ordering::Relaxed) {
            0.0
        } else {
            f32::from_bits(self.volume.load(Ordering::Relaxed))
        };

        let ch = self.channels.max(1);
        for frame in data.chunks_mut(ch) {
            let mut sum = 0.0;
            let mut i = 0;
            while i < self.voices.len() {
                if self.voices[i].done() {
                    self.voices.swap_remove(i);
                    continue;
                }
                sum += self.voices[i].render(self.sr, &mut self.rng);
                i += 1;
            }
            let s = soft_clip(sum * gain);
            for out in frame.iter_mut() {
                *out = s;
            }
        }
    }
}
