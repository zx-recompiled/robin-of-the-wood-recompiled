//! What the local checks of the original share: finding the tape and ROM in
//! `assets/`, booting to the hand-over, and playing under random held keys.

#![allow(dead_code)] // Each test uses its own part of this.

use std::path::PathBuf;

use robin::layout::ENTRY_PC;
use zx_runtime::{Misses, Zx, keys::Key, loader::boot_128k};

pub fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// The supported tape and `128.rom`, or `None` (and a note) without them.
pub fn tape_and_rom() -> Option<(Vec<u8>, Vec<u8>)> {
    let tape = std::fs::read_dir(assets())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::read(e.path()).ok())
        .find(|b| robin::is_the_tape(b));
    let rom = std::fs::read(assets().join("128.rom")).ok();
    let (Some(tape), Some(rom)) = (tape, rom) else {
        println!("skipped: needs the supported tape and 128.rom in assets/");
        return None;
    };
    Some((tape, rom))
}

/// The original booted from its tape to the hand-over.
pub fn boot(tape: &[u8], rom: &[u8]) -> Zx {
    let blocks = zx_core::tape::load_tzx(tape).expect("the tape reads");
    boot_128k(rom, blocks, ENTRY_PC, 2000).expect("the original boots")
}

/// Keys the game reads in play (`docs/re/robin.md`, *Input*), and 0 to start.
pub const KEYS: &[&str] = &[
    "1", "2", "3", "4", "5", "q", "w", "e", "r", "t", "a", "s", "d", "f", "g", "caps", "z", "x",
    "c", "v", "enter", "l", "k", "j", "h", "space", "symbol", "m", "n", "b", "0",
];

/// A small fixed-seed generator, so the run is the same every time.
pub struct XorShift(pub u32);
impl XorShift {
    pub fn next(&mut self, n: u32) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 % n
    }
}

/// Plays `frames` frames from where `z` is, a key held for a while, then
/// let go, then another, as hands play. `hook` sees every instruction before
/// it runs, as `Zx::run_frame`'s does, and the same seed gives the same game.
pub fn play(z: &mut Zx, frames: u32, mut hook: impl FnMut(&mut Zx)) {
    let mut rng = XorShift(0x2468_ACE1);
    let mut misses = Misses::default();
    let mut held: Option<(Key, u32)> = None;
    for frame in 0..frames {
        match held {
            Some((key, until)) if frame >= until => {
                z.set_key(key, false);
                held = None;
            }
            None if rng.next(4) == 0 => {
                let key = Key::by_name(KEYS[rng.next(KEYS.len() as u32) as usize]).expect("a key");
                z.set_key(key, true);
                held = Some((key, frame + 5 + rng.next(60)));
            }
            _ => {}
        }
        z.run_frame(
            |z: &mut Zx| {
                hook(z);
                false
            },
            &mut misses,
        );
    }
}
