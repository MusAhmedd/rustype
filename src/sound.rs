//! Procedurally synthesized typewriter sounds (no audio files needed).

use crate::settings::SoundPack;
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Source};
use std::cell::Cell;
use std::f32::consts::TAU;

const RATE: u32 = 44_100;

#[derive(Clone, Copy)]
pub enum Effect {
    Key,
    Space,
    Return,
    Bell,
    /// Two-tone chime for reaching the daily writing goal.
    Goal,
}

struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn len(secs: f32) -> usize {
    (secs * RATE as f32) as usize
}

/// Shape of a single key strike.
#[derive(Clone, Copy)]
struct Strike {
    secs: f32,
    click_decay: f32,
    click_gain: f32,
    thump_hz: f32,
    thump_decay: f32,
    thump_gain: f32,
    /// Metallic resonance (0 = none).
    ping_hz: f32,
    ping_decay: f32,
    ping_gain: f32,
    /// Second click this many seconds later (key release), 0 = none.
    release_at: f32,
}

fn strike(seed: u32, s: Strike, gain: f32) -> Vec<f32> {
    let mut rng = Rng(seed);
    let mut prev = 0.0;
    (0..len(s.secs))
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let noise = rng.next();
            let hp = noise - prev;
            prev = noise;
            let mut v = hp * (-t * s.click_decay).exp() * s.click_gain;
            v += (TAU * s.thump_hz * t).sin() * (-t * s.thump_decay).exp() * s.thump_gain;
            if s.ping_hz > 0.0 {
                v += (TAU * s.ping_hz * t).sin() * (-t * s.ping_decay).exp() * s.ping_gain;
            }
            if s.release_at > 0.0 && t > s.release_at {
                let r = t - s.release_at;
                v += hp * (-r * s.click_decay * 1.3).exp() * s.click_gain * 0.5;
            }
            v * gain
        })
        .collect()
}

fn carriage_return(seed: u32, clicks_hz: f32, clunk_hz: f32, secs: f32) -> Vec<f32> {
    let mut rng = Rng(seed);
    let clunk_at = secs * 0.8;
    (0..len(secs))
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let ratchet = if (t * clicks_hz).fract() < 0.08 { rng.next() * 0.4 } else { rng.next() * 0.03 };
            let ct = (t - clunk_at).max(0.0);
            let clunk = if t > clunk_at { (TAU * clunk_hz * ct).sin() * (-ct * 40.0).exp() } else { 0.0 };
            (ratchet * (1.0 - t / secs) + clunk * 0.7) * 0.6
        })
        .collect()
}

fn motor_return() -> Vec<f32> {
    let mut rng = Rng(0x5151);
    let secs = 0.32;
    (0..len(secs))
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let whir = ((TAU * 120.0 * t).sin() * 0.25 + rng.next() * 0.08) * (1.0 - t / secs);
            let ct = (t - 0.24).max(0.0);
            let clunk = if t > 0.24 { (TAU * 70.0 * ct).sin() * (-ct * 35.0).exp() * 0.8 } else { 0.0 };
            (whir + clunk) * 0.6
        })
        .collect()
}

fn bell(partials: &[(f32, f32)], decay: f32, gain: f32) -> Vec<f32> {
    (0..len(0.9))
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let v: f32 = partials.iter().map(|(hz, g)| (TAU * hz * t).sin() * g).sum();
            v * (-t * decay).exp() * gain
        })
        .collect()
}

/// Two bell strikes, the second a fifth higher, for the goal chime.
fn chime(base_hz: f32) -> Vec<f32> {
    let a = bell(&[(base_hz, 0.5), (base_hz * 2.76, 0.15)], 4.0, 0.3);
    let b = bell(&[(base_hz * 1.5, 0.5), (base_hz * 4.14, 0.12)], 3.5, 0.3);
    let offset = len(0.2);
    let mut out = vec![0.0; offset + b.len()];
    for (i, v) in a.iter().enumerate() {
        out[i] += v;
    }
    for (i, v) in b.iter().enumerate() {
        out[offset + i] += v;
    }
    out
}

struct Bank {
    keys: Vec<Vec<f32>>,
    space: Vec<f32>,
    ret: Vec<f32>,
    bell: Option<Vec<f32>>,
    chime: Vec<f32>,
}

fn bank(pack: SoundPack) -> Bank {
    let base = match pack {
        SoundPack::Classic => Strike {
            secs: 0.06, click_decay: 180.0, click_gain: 0.6, thump_hz: 140.0, thump_decay: 60.0,
            thump_gain: 0.5, ping_hz: 0.0, ping_decay: 0.0, ping_gain: 0.0, release_at: 0.0,
        },
        SoundPack::Electric => Strike {
            secs: 0.05, click_decay: 320.0, click_gain: 0.35, thump_hz: 85.0, thump_decay: 75.0,
            thump_gain: 0.75, ping_hz: 0.0, ping_decay: 0.0, ping_gain: 0.0, release_at: 0.0,
        },
        SoundPack::Vintage => Strike {
            secs: 0.07, click_decay: 220.0, click_gain: 0.55, thump_hz: 310.0, thump_decay: 90.0,
            thump_gain: 0.25, ping_hz: 1850.0, ping_decay: 70.0, ping_gain: 0.12, release_at: 0.0,
        },
        SoundPack::Mechanical => Strike {
            secs: 0.09, click_decay: 420.0, click_gain: 0.7, thump_hz: 220.0, thump_decay: 110.0,
            thump_gain: 0.3, ping_hz: 3200.0, ping_decay: 160.0, ping_gain: 0.08, release_at: 0.045,
        },
    };
    let keys = (0..4)
        .map(|i| {
            let s = Strike { thump_hz: base.thump_hz * (1.0 + i as f32 * 0.07), ..base };
            strike(0x1234 + i * 977, s, 0.8)
        })
        .collect();
    let space = strike(0x9999, Strike { thump_hz: base.thump_hz * 0.7, ..base }, 0.65);
    let (ret, bell) = match pack {
        SoundPack::Classic => (
            carriage_return(0xBEEF, 90.0, 90.0, 0.35),
            Some(bell(&[(2100.0, 0.5), (3170.0, 0.25), (5230.0, 0.1)], 5.0, 0.35)),
        ),
        SoundPack::Electric => (motor_return(), Some(bell(&[(1600.0, 0.5), (2400.0, 0.2)], 7.0, 0.3))),
        SoundPack::Vintage => (
            carriage_return(0xC0DE, 120.0, 160.0, 0.28),
            Some(bell(&[(3000.0, 0.5), (4520.0, 0.2)], 9.0, 0.3)),
        ),
        SoundPack::Mechanical => (strike(0x7777, Strike { thump_hz: 120.0, ..base }, 1.0), None),
    };
    let chime = chime(match pack {
        SoundPack::Electric => 1320.0,
        SoundPack::Vintage => 1760.0,
        _ => 1568.0,
    });
    Bank { keys, space, ret, bell, chime }
}

pub struct Sound {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    pack: SoundPack,
    bank: Bank,
    rng: Cell<u32>,
}

impl Sound {
    /// Returns `None` when no audio device is available.
    pub fn new(pack: SoundPack) -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        Some(Self { _stream: stream, handle, pack, bank: bank(pack), rng: Cell::new(0xA5A5_1234) })
    }

    pub fn set_pack(&mut self, pack: SoundPack) {
        if pack != self.pack {
            self.pack = pack;
            self.bank = bank(pack);
        }
    }

    fn emit(&self, data: &[f32], volume: f32) {
        let src = SamplesBuffer::new(1, RATE, data.to_vec()).amplify(volume);
        let _ = self.handle.play_raw(src);
    }

    pub fn play(&self, effect: Effect, volume: f32) {
        match effect {
            Effect::Key => {
                let mut r = self.rng.get();
                r ^= r << 13;
                r ^= r >> 17;
                r ^= r << 5;
                self.rng.set(r);
                self.emit(&self.bank.keys[(r as usize) % self.bank.keys.len()], volume);
            }
            Effect::Space => self.emit(&self.bank.space, volume),
            Effect::Return => self.emit(&self.bank.ret, volume),
            Effect::Bell => {
                self.play(Effect::Key, volume);
                if let Some(b) = &self.bank.bell {
                    self.emit(b, volume);
                }
            }
            Effect::Goal => self.emit(&self.bank.chime, volume),
        }
    }
}
