//! A game that's taken around the map (#8, Decision 2). Random play stays
//! near where it starts, and the tour plays only half a second at each
//! location, so this plays 10,000 frames under random held keys and, every
//! 300 frames, sends the original into a random location through its own
//! step and entry, as the ending's game does, with Robin's energy topped up.
//! Every pass and every interrupt it makes is compared from the original's
//! state, as in play. It is a supplement: states the original can be in,
//! reached sooner than play reaches them.

use std::collections::BTreeSet;

use robin::assets::Assets;
use zx_recomp::script::Script;
use zx_runtime::{Misses, Zx, loader::boot_128k};

use crate::Verifier;
use crate::capture::Play;
use crate::tour;

/// How many frames are played, from the hand-over, and how often it travels.
const FRAMES: u32 = 10_000;
const EVERY: u32 = 300;

/// 0 to start, then the tape's keys pressed at random (fire 1, up Q, down
/// A, left N, right M).
const SCRIPT: &str = r#"[game]
name = "travel"
tape = "-"
[trace]
frames = 10000
[[trace.input]]
at = 400
hold = 5
keys = ["0"]
[trace.random]
start = 450
every = 12
keys = ["1", "q", "a", "n", "m"]
seed = 8
"#;

/// Where the original leaves a screen, with the direction in E.
const LEAVE: u16 = 0xBECE;
const RIGHT: u8 = 1;
/// The ending's location, which ends the game.
const ENDING: u16 = 0x0069;
/// The locations: 16 a row, 20 rows.
const LOCATIONS: u32 = 320;
/// His energy, and the most the panel's one digit shows.
const ENERGY: u16 = 0xD481;

/// Plays the game. The locations it sent the original into.
pub fn run(
    rom: &[u8],
    tape: &[u8],
    assets: &Assets,
    v: &mut Verifier,
) -> Result<BTreeSet<u16>, String> {
    let cfg = zx_recomp::Config::parse(SCRIPT)?;
    let mut script = Script::new(&cfg.trace)?;
    let blocks = zx_core::tape::load_tzx(tape)?;
    let mut z = boot_128k(rom, blocks, robin::layout::ENTRY_PC, 2000)?;
    let mut misses = Misses::default();
    let mut entered = BTreeSet::new();
    let mut rng: u32 = 0x8BAD_F00D;
    let mut due = tour::AFTER + EVERY;
    for frame in 0..FRAMES {
        script.press(frame, &mut z);
        z.run_frame(
            |z: &mut Zx| {
                if frame >= due && tour::at_main_loop(z) {
                    rng ^= rng << 13;
                    rng ^= rng >> 17;
                    rng ^= rng << 5;
                    let mut to = (rng % LOCATIONS) as u16;
                    if to == ENDING {
                        to += 1;
                    }
                    // One to its left in its row, which stepping right wraps.
                    let from = to & 0xFFF0 | (to.wrapping_sub(1) & 0x0F);
                    let [lo, hi] = from.to_le_bytes();
                    z.memory.poke(0xC440, lo);
                    z.memory.poke(0xC441, hi);
                    z.memory.poke(ENERGY, 9);
                    z.e = RIGHT;
                    z.pc = LEAVE;
                    entered.insert(to);
                    due = frame + EVERY;
                }
                v.observe(
                    z,
                    &Play {
                        script: &script,
                        frame,
                        assets,
                    },
                );
                false
            },
            &mut misses,
        );
    }
    if entered.is_empty() {
        return Err("the traveller's game never reached its main loop".into());
    }
    Ok(entered)
}
