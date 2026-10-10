//! Sound (#67): the beeper and the AY, from the game's writes to their
//! ports at their times, mixed and played through the sound card, adapted
//! from starquake-recompiled's (`REUSED.md`). The AY is RustZX's `aym`,
//! from the maintainer's copy in zx-sidekick.
//!
//! The beeper's level is EAR, bit 4. MIC, bit 3, which the 128K mixes in
//! far quieter, is left out (`README.md`, *Status*).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use aym::{AyMode, AySample, AymBackend, AymPrecise, SoundChip};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use robin::session::FRAME_T;

const CPU_HZ: f64 = zx_core::timing::SPECTRUM_128.cpu_hz as f64;
/// The AY's clock on the 128K: half the processor's.
const AY_HZ: usize = zx_core::timing::SPECTRUM_128.cpu_hz as usize / 2;
const VOLUME: f32 = 0.25;
/// The AY's part of the mix: one channel at full volume swings a little
/// over half as far as the beeper, as Fuse balances them. The machine's
/// own balance isn't measured: a **guess** (`README.md`, *Status*).
const AY_VOLUME: f32 = 0.2;

/// A write the sound plays, at its time.
#[derive(Clone, Copy, Debug)]
enum Write {
    /// The ULA's port: the beeper's EAR.
    Ula(u8),
    /// An AY register and its value.
    Ay(u8, u8),
}

/// Turns the speaker's levels and the AY's registers over time, in
/// T-states, into samples.
pub struct Mixer {
    rate: f64,
    level: bool,
    ay: AymPrecise,
    /// T-states into the current sample, and the level summed over it.
    sample_t: f64,
    acc: f64,
    samples: Vec<f32>,
    /// A DC blocker, so a speaker left high doesn't sit off centre.
    dc_in: f32,
    dc_out: f32,
    /// Writes still to play, from a sound that ran past its frame's end,
    /// timed from the start of the next frame to be played.
    later: Vec<(u32, Write)>,
}

impl Mixer {
    pub fn new(rate: u32) -> Mixer {
        let mut ay = AymPrecise::new(SoundChip::AY, AyMode::Mono, AY_HZ, rate as usize);
        ay.enable_dc_filter();
        Mixer {
            rate: f64::from(rate),
            level: false,
            ay,
            sample_t: 0.0,
            acc: 0.0,
            samples: Vec::new(),
            // The speaker starts low: that's silence, not a click.
            dc_in: -VOLUME,
            dc_out: 0.0,
            later: Vec::new(),
        }
    }

    /// Holds the current level for `t` T-states.
    fn advance(&mut self, mut t: f64) {
        let per_sample = CPU_HZ / self.rate;
        let level = if self.level { 1.0 } else { -1.0 };
        while t > 0.0 {
            let step = t.min(per_sample - self.sample_t);
            self.acc += level * step;
            self.sample_t += step;
            t -= step;
            if self.sample_t >= per_sample {
                let x = (self.acc / per_sample) as f32 * VOLUME;
                let y = x - self.dc_in + 0.995 * self.dc_out;
                self.dc_in = x;
                self.dc_out = y;
                let ay = self.ay.next_sample();
                let ay = (ay.left.to_f32() + ay.right.to_f32()) / 2.0 * AY_VOLUME;
                self.samples.push(y + ay);
                self.sample_t = 0.0;
                self.acc = 0.0;
            }
        }
    }

    /// Plays a frame of the 128K: its writes to the ULA's port
    /// (`robin::io::Io::ula_writes`) and to the AY's registers
    /// (`robin::io::Io::ay_writes`), timed from its start. A sound that
    /// holds the game runs past the frame's end, and what's past it is
    /// played in the frames that follow, while the game stands still, in
    /// time with what they write.
    pub fn frame(
        &mut self,
        ula: impl IntoIterator<Item = (u32, u8)>,
        ay: impl IntoIterator<Item = (u32, u8, u8)>,
    ) {
        let mut writes = std::mem::take(&mut self.later);
        writes.extend(ula.into_iter().map(|(t, v)| (t, Write::Ula(v))));
        writes.extend(ay.into_iter().map(|(t, r, v)| (t, Write::Ay(r, v))));
        // Stable, so writes at the same time keep their order.
        writes.sort_by_key(|&(t, _)| t);
        let mut now = 0;
        for (n, &(at, write)) in writes.iter().enumerate() {
            if at >= FRAME_T {
                self.later = writes[n..]
                    .iter()
                    .map(|&(at, write)| (at - FRAME_T, write))
                    .collect();
                break;
            }
            self.advance(f64::from(at.saturating_sub(now)));
            now = now.max(at);
            match write {
                Write::Ula(v) => self.level = v & 0x10 != 0,
                Write::Ay(r, v) => self.ay.write_register(r, v),
            }
        }
        self.advance(f64::from(FRAME_T - now));
    }

    /// The samples made since the last [`Mixer::clear_samples`], kept so
    /// the buffer is reused rather than a new one made every frame.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    pub fn clear_samples(&mut self) {
        self.samples.clear();
    }
}

/// `samples` as a WAV file: mono, 16 bits, at `rate`.
pub fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let data = u32::try_from(samples.len() * 2).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for &s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    out
}

/// Builds the output stream for whatever sample format the device wants,
/// converting from the mono f32 the mixer makes.
fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    queue: Arc<Mutex<VecDeque<f32>>>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let mut q = queue.lock().unwrap();
                for frame in data.chunks_mut(channels) {
                    frame.fill(T::from_sample(q.pop_front().unwrap_or(0.0)));
                }
            },
            |e| eprintln!("sound error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

/// The sound card, fed through a queue of mono samples.
pub struct Output {
    queue: Arc<Mutex<VecDeque<f32>>>,
    rate: u32,
}

impl Output {
    /// The default output device, playing. The stream must be held for as
    /// long as the sound should play.
    pub fn start() -> Result<(Output, cpal::Stream), String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("no output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let config: cpal::StreamConfig = supported.config();
        let channels = usize::from(config.channels);
        let rate = config.sample_rate;
        let queue = Arc::new(Mutex::new(VecDeque::<f32>::new()));
        // The device decides the sample format: WASAPI in shared mode and
        // ALSA `hw:` devices that default to 16 bits reject an f32 stream
        // outright (starquake-recompiled).
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(&device, config, channels, queue.clone())?,
            cpal::SampleFormat::I16 => build::<i16>(&device, config, channels, queue.clone())?,
            cpal::SampleFormat::U16 => build::<u16>(&device, config, channels, queue.clone())?,
            other => return Err(format!("sample format {other} is not supported")),
        };
        stream.play().map_err(|e| e.to_string())?;
        Ok((Output { queue, rate }, stream))
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn push(&self, samples: &[f32]) {
        let mut q = self.queue.lock().unwrap();
        q.extend(samples);
        // If the game ever runs ahead, the oldest go rather than the sound
        // falling behind the picture for good. A frame's sound is a frame
        // long, even one that holds the game, so a second is plenty.
        let cap = self.rate as usize;
        if q.len() > cap {
            let excess = q.len() - cap;
            q.drain(..excess);
        }
    }

    pub fn queued(&self) -> usize {
        self.queue.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    #[test]
    fn every_frame_is_a_frame_of_sound() {
        let mut b = Mixer::new(RATE);
        b.frame([(100, 0x10), (FRAME_T * 3, 0)], []);
        for _ in 0..4 {
            b.frame([], []);
        }
        let per_frame = f64::from(RATE) * f64::from(FRAME_T) / CPU_HZ;
        let made = b.samples().len() as f64;
        assert!((made - 5.0 * per_frame).abs() <= 1.0, "{made} samples");
    }

    #[test]
    fn a_sound_past_the_frames_end_plays_in_the_frames_after() {
        let mut b = Mixer::new(RATE);
        b.frame([(FRAME_T + FRAME_T / 2, 0x10)], []);
        assert!(
            b.samples().iter().all(|&s| s <= 0.0),
            "low all the first frame"
        );
        b.clear_samples();
        b.frame([], []);
        let half = b.samples().len() / 2;
        // The sample the change falls in is between the two.
        assert!(b.samples()[..half - 2].iter().all(|&s| s <= 0.0));
        assert!(b.samples()[half + 2] > 0.0, "high from halfway");
    }

    #[test]
    fn the_ay_plays_a_tone_from_its_writes() {
        let mut b = Mixer::new(RATE);
        b.frame([], []);
        assert!(
            b.samples().iter().all(|&s| s.abs() < 1e-3),
            "silent at first"
        );
        b.clear_samples();
        // Channel A's tone at about 440 Hz, on, at full volume, from halfway.
        let half = FRAME_T / 2;
        b.frame(
            [],
            [
                (half, 0, 0xFC),
                (half, 1, 0),
                (half, 7, 0x3E),
                (half, 8, 0x0F),
            ],
        );
        let n = b.samples().len();
        let quiet = b.samples()[..n / 2 - 1].iter().all(|&s| s.abs() < 1e-3);
        let loud = b.samples()[n / 2 + 1..]
            .iter()
            .fold(0f32, |m, &s| m.max(s.abs()));
        assert!(quiet, "nothing before the writes");
        assert!(loud > 0.05, "a tone after them: {loud}");
    }

    #[test]
    fn mic_alone_is_silent() {
        let mut b = Mixer::new(RATE);
        b.frame(
            (0..100).map(|n| (n * 500, if n % 2 == 0 { 0x08 } else { 0 })),
            [],
        );
        assert!(b.samples().iter().all(|&s| s <= 0.0));
    }
}
