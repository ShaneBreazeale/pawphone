//! The audio engine: owns the `cpal` output stream and hands the connection
//! worker a `Send` control surface. One method per sonic event, matching the
//! connection phases. Driven by lock-free commands so audio never desyncs from
//! the state machine.
//!
//! On macOS the `cpal::Stream` is `!Send`, so [`PawLinkAudioEngine`] (which
//! holds it) lives on the main thread for its whole life. Only [`AudioControl`]
//! — a `Send` ring-buffer producer — crosses to the worker thread.

pub mod synth;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;
use ringbuf::traits::{Producer, Split};
use ringbuf::HeapProd;

use synth::SynthCore;

/// The audio event vocabulary. Sent lock-free from the connection worker to
/// the render callback. Each maps to one (or more) procedural voices.
#[derive(Clone, Copy, Debug)]
pub enum AudioCommand {
    PowerUp,
    /// Start the low noise bed under the satellite search.
    Acquiring,
    /// One sonar ping, pitched up by `step` as satellites are found.
    Ping { step: u32 },
    Handshake,
    Lock,
    Transmit,
    Incoming,
    SignalLost { variation: f32 },
}

/// `Send` handle the connection worker uses to trigger sounds. If the audio
/// device is unavailable this is silent (no-op) and the app still runs.
pub struct AudioControl {
    prod: Option<HeapProd<AudioCommand>>,
}

impl AudioControl {
    fn silent() -> AudioControl {
        AudioControl { prod: None }
    }

    #[inline]
    fn push(&mut self, cmd: AudioCommand) {
        if let Some(p) = &mut self.prod {
            // Drop on full rather than block — the audio thread will catch up.
            let _ = p.try_push(cmd);
        }
    }

    pub fn power_up(&mut self) {
        self.push(AudioCommand::PowerUp);
    }
    pub fn acquiring(&mut self) {
        self.push(AudioCommand::Acquiring);
    }
    pub fn ping(&mut self, step: u32) {
        self.push(AudioCommand::Ping { step });
    }
    pub fn handshake(&mut self) {
        self.push(AudioCommand::Handshake);
    }
    pub fn lock(&mut self) {
        self.push(AudioCommand::Lock);
    }
    pub fn transmit(&mut self) {
        self.push(AudioCommand::Transmit);
    }
    pub fn incoming(&mut self) {
        self.push(AudioCommand::Incoming);
    }
    pub fn signal_lost(&mut self, variation: f32) {
        self.push(AudioCommand::SignalLost { variation });
    }
}

/// Owns the live `cpal` stream and the shared master-gain atoms. Keep this
/// alive for the whole app — dropping it stops the audio.
pub struct PawLinkAudioEngine {
    _stream: Option<cpal::Stream>,
    /// Master volume as `f32` bits (0..1). Shared with the render callback.
    pub volume: Arc<AtomicU32>,
    /// Mute flag. Shared with the render callback (gates all output).
    pub muted: Arc<AtomicBool>,
    /// `true` if a real output device was acquired.
    pub active: bool,
}

impl PawLinkAudioEngine {
    /// Build the graph. Returns the engine (hold it on the main thread) plus a
    /// `Send` control handle for the worker. Never panics: on any failure it
    /// yields a silent engine so the rest of the app is unaffected.
    pub fn new() -> (PawLinkAudioEngine, AudioControl) {
        let volume = Arc::new(AtomicU32::new(0.7f32.to_bits()));
        let muted = Arc::new(AtomicBool::new(false));

        match Self::try_build(volume.clone(), muted.clone()) {
            Some((stream, prod)) => (
                PawLinkAudioEngine { _stream: Some(stream), volume, muted, active: true },
                AudioControl { prod: Some(prod) },
            ),
            None => {
                eprintln!("[pawlink audio] no output device — running silent");
                (
                    PawLinkAudioEngine { _stream: None, volume, muted, active: false },
                    AudioControl::silent(),
                )
            }
        }
    }

    pub fn set_volume(&self, v: f32) {
        self.volume.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }
    pub fn set_muted(&self, m: bool) {
        self.muted.store(m, Ordering::Relaxed);
    }
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    fn try_build(
        volume: Arc<AtomicU32>,
        muted: Arc<AtomicBool>,
    ) -> Option<(cpal::Stream, HeapProd<AudioCommand>)> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let supported = device.default_output_config().ok()?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let sr = config.sample_rate as f32;
        let channels = config.channels as usize;

        let rb = HeapRb::<AudioCommand>::new(256);
        let (prod, cons) = rb.split();
        let synth = SynthCore::new(sr, channels, cons, volume, muted);

        let stream = build_stream(&device, config, sample_format, synth)?;
        stream.play().ok()?;
        Some((stream, prod))
    }
}

/// Upper bound on samples per callback for the non-F32 conversion scratch.
/// The buffer size is host-chosen (`BufferSize::Default`), so we pre-size
/// generously rather than ever resize on the audio thread. 16384 samples =
/// 8192 stereo frames, far above any realistic CoreAudio buffer.
const MAX_SCRATCH_SAMPLES: usize = 16384;

fn build_stream(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    fmt: cpal::SampleFormat,
    mut synth: SynthCore,
) -> Option<cpal::Stream> {
    let err_fn = |e| eprintln!("[pawlink audio] stream error: {e}");
    let result = match fmt {
        cpal::SampleFormat::F32 => device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| synth.render(data),
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => {
            // Pre-sized once here on the main thread; the callback never
            // allocates. An (unexpected) oversized buffer emits silence rather
            // than resizing on the real-time audio thread.
            let mut scratch = vec![0.0f32; MAX_SCRATCH_SAMPLES];
            device.build_output_stream(
                config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    if data.len() <= scratch.len() {
                        let buf = &mut scratch[..data.len()];
                        synth.render(buf);
                        for (o, x) in data.iter_mut().zip(buf.iter()) {
                            *o = (x.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                        }
                    } else {
                        for o in data.iter_mut() {
                            *o = 0;
                        }
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let mut scratch = vec![0.0f32; MAX_SCRATCH_SAMPLES];
            device.build_output_stream(
                config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    if data.len() <= scratch.len() {
                        let buf = &mut scratch[..data.len()];
                        synth.render(buf);
                        for (o, x) in data.iter_mut().zip(buf.iter()) {
                            let v = (x.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32;
                            *o = v as u16;
                        }
                    } else {
                        for o in data.iter_mut() {
                            *o = u16::MAX / 2;
                        }
                    }
                },
                err_fn,
                None,
            )
        }
        other => {
            eprintln!("[pawlink audio] unsupported sample format {other:?} — running silent");
            return None;
        }
    };
    result.ok()
}
