//! Telemetry copy generation. Plausible-sounding nonsense with real units —
//! dBm, AZ/EL degrees, Doppler shift, purr-rate. This is half the joke; keep it
//! deadpan and technical.

use rand::seq::IndexedRandom;

fn rssi() -> i32 {
    rand::random_range(-95..=-58)
}
fn azimuth() -> u32 {
    rand::random_range(0..360)
}
fn elevation() -> u32 {
    rand::random_range(5..86)
}
fn doppler() -> f32 {
    (rand::random_range(-42..=42) as f32) / 10.0
}
fn purr_rate() -> f32 {
    (rand::random_range(180..=285) as f32) / 10.0
}

const ALIGN_TAGS: &[&str] = &[
    "whisker-aligned",
    "tail-stabilized",
    "ear-tracking",
    "nose-cone nominal",
    "paw-steady",
    "loaf-locked",
];

const AMBIENT: &[&str] = &[
    "constellation drift within tolerance",
    "purr-rate stable",
    "thermal: warm sunbeam detected",
    "treat reserves nominal",
    "bird telemetry: alert, distractible",
    "antenna licked clean",
    "orbital nap schedule synced",
    "litter of packets queued",
];

/// `ACQUIRING PAWLINK SAT 7 OF 12 · AZ 218° EL 41° · RSSI -71dBm · whisker-aligned`
pub fn acquiring(found: u32, total: u32) -> String {
    let tag = ALIGN_TAGS.choose(&mut rand::rng()).copied().unwrap_or("aligned");
    format!(
        "ACQUIRING PAWLINK SAT {found} OF {total} · AZ {}° EL {}° · RSSI {}dBm · {tag}",
        azimuth(),
        elevation(),
        rssi(),
    )
}

pub fn powering_up() -> String {
    format!("RELAY ENGAGED · bus +{}.{}V · spinning up gyro-paws", 4, rand::random_range(0..9))
}

pub fn handshaking() -> String {
    format!(
        "HANDSHAKE · FSK warble · Doppler {:+.1}kHz · purr-rate {:.1}Hz",
        doppler(),
        purr_rate(),
    )
}

pub fn locked() -> String {
    format!("CARRIER LOCKED · RSSI {}dBm · BER 0.00 · whiskers true", rssi())
}

pub fn transmitting() -> String {
    format!("TX BURST · {} packets · {} kbit/s · tail-wagging", rand::random_range(3..18), rand::random_range(9..96))
}

pub fn awaiting() -> String {
    format!("AWAITING REPLY · RTT pending · bird latency {}ms", rand::random_range(180..2600))
}

pub fn receiving() -> String {
    format!("RX · inbound meow · SNR {}dB · decoding purr", rand::random_range(8..32))
}

/// A random ambient flavor line for idle HUD scroll.
pub fn ambient() -> String {
    AMBIENT.choose(&mut rand::rng()).copied().unwrap_or("nominal").to_uppercase()
}
