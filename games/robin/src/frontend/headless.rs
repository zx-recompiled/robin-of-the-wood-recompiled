//! Plays without a window: 0 at the menu, then random held keys, with a
//! screenshot every so many frames, and the sound written to a
//! WAV file. For testing the program end to end.

use std::path::Path;

use robin::controls::Controls;
use robin::picture::{FULL_H, FULL_W};
use robin::session::{Session, State};

use super::audio::{self, Mixer};

/// The sound file's sample rate.
const RATE: u32 = 48_000;

/// The keys it holds: Q, A, N, M (up, down, left, right) and 1 (fire), the
/// tape's own (`docs/re/robin.md`, *The controls*).
const WAYS: [(usize, u8); 5] = [(2, 0), (1, 0), (7, 3), (7, 2), (3, 0)];

/// Plays `frames` frames from `tape`, writing about 40 screenshots to `dir`,
/// and the sound to `sound.wav` there.
///
/// # Errors
///
/// If the tape can't be read, or a screenshot can't be written.
pub fn run(tape: &[u8], frames: u64, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let assets = robin::assets::read_tape(tape)?;
    let mut session = Session::new(&assets, 0x1234_5678);
    let every = (frames / 40).max(1);
    let mut rng: u64 = 0x9E37_79B9;
    let mut controls = Controls::default();
    let mut mixer = Mixer::new(RATE);
    for frame in 0..frames {
        if frame % 15 == 0 {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            controls = Controls::default();
            let (row, bit) = WAYS[(rng % WAYS.len() as u64) as usize];
            controls.keys[row] &= !(1 << bit);
        }
        if session.state != State::Playing && frame % 100 < 5 {
            controls = Controls::default();
            controls.keys[4] &= !1;
        }
        session.frame(&assets, controls);
        mixer.frame(
            session.io.ula_writes(),
            session.io.ay_writes.iter().copied(),
        );
        if frame % every == 0 {
            let png = zx_core::png::encode(&session.picture(), FULL_W, FULL_H);
            let file = dir.join(format!("frame{frame:06}.png"));
            std::fs::write(&file, png).map_err(|e| format!("{}: {e}", file.display()))?;
        }
    }
    let file = dir.join("sound.wav");
    std::fs::write(&file, audio::wav(mixer.samples(), RATE))
        .map_err(|e| format!("{}: {e}", file.display()))?;
    println!(
        "played {frames} frames; at location {:#05x}, {:?}",
        session.game.map.location, session.state
    );
    Ok(())
}
