//! Sound (#67): the beeper, from the game's writes to the ULA's port at
//! their times, played through the sound card, adapted from
//! starquake-recompiled's (`REUSED.md`). The AY is still to come (#67).
//!
//! The beeper's level is EAR, bit 4. MIC, bit 3, which the 128K mixes in
//! far quieter, is left out (`README.md`, *Status*).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use robin::session::FRAME_T;

const CPU_HZ: f64 = zx_core::timing::SPECTRUM_128.cpu_hz as f64;
const VOLUME: f32 = 0.25;

/// Turns the speaker's levels over time, in T-states, into samples.
pub struct Beeper {
    rate: f64,
    level: bool,
    /// T-states into the current sample, and the level summed over it.
    sample_t: f64,
    acc: f64,
    samples: Vec<f32>,
    /// A DC blocker, so a speaker left high doesn't sit off centre.
    dc_in: f32,
    dc_out: f32,
    /// Changes still to play, from a sound that ran past its frame's end,
    /// timed from the start of the next frame to be played.
    later: Vec<(u32, bool)>,
}

impl Beeper {
    pub fn new(rate: u32) -> Beeper {
        Beeper {
            rate: f64::from(rate),
            level: false,
            sample_t: 0.0,
            acc: 0.0,
            samples: Vec::new(),
            dc_in: 0.0,
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
                self.samples.push(y);
                self.sample_t = 0.0;
                self.acc = 0.0;
            }
        }
    }

    /// Plays a frame of the 128K. `writes` are its writes to the ULA's port
    /// (`robin::io::Io::ula_writes`), timed from its start. A sound that
    /// holds the game runs past the frame's end, and what's past it is
    /// played in the frames that follow, while the game stands still.
    pub fn frame(&mut self, writes: impl IntoIterator<Item = (u32, u8)>) {
        let mut changes = std::mem::take(&mut self.later);
        changes.extend(writes.into_iter().map(|(t, v)| (t, v & 0x10 != 0)));
        let mut now = 0;
        for (n, &(at, level)) in changes.iter().enumerate() {
            if at >= FRAME_T {
                self.later = changes[n..]
                    .iter()
                    .map(|&(at, level)| (at - FRAME_T, level))
                    .collect();
                break;
            }
            self.advance(f64::from(at.saturating_sub(now)));
            now = now.max(at);
            self.level = level;
        }
        self.advance(f64::from(FRAME_T - now));
    }

    /// The samples made since the last [`Beeper::clear_samples`], kept so
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
/// converting from the mono f32 the beeper makes.
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
        let mut b = Beeper::new(RATE);
        b.frame([(100, 0x10), (FRAME_T * 3, 0)]);
        for _ in 0..4 {
            b.frame([]);
        }
        let per_frame = f64::from(RATE) * f64::from(FRAME_T) / CPU_HZ;
        let made = b.samples().len() as f64;
        assert!((made - 5.0 * per_frame).abs() <= 1.0, "{made} samples");
    }

    #[test]
    fn a_sound_past_the_frames_end_plays_in_the_frames_after() {
        let mut b = Beeper::new(RATE);
        b.frame([(FRAME_T + FRAME_T / 2, 0x10)]);
        assert!(
            b.samples().iter().all(|&s| s <= 0.0),
            "low all the first frame"
        );
        b.clear_samples();
        b.frame([]);
        let half = b.samples().len() / 2;
        // The sample the change falls in is between the two.
        assert!(b.samples()[..half - 2].iter().all(|&s| s <= 0.0));
        assert!(b.samples()[half + 2] > 0.0, "high from halfway");
    }

    #[test]
    fn mic_alone_is_silent() {
        let mut b = Beeper::new(RATE);
        b.frame((0..100).map(|n| (n * 500, if n % 2 == 0 { 0x08 } else { 0 })));
        assert!(b.samples().iter().all(|&s| s <= 0.0));
    }
}
