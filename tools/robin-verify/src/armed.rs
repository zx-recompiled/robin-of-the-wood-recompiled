//! A game with Robin armed from its start (#36, Decision 1): the sword, the
//! bow and ten arrows, set as the game's own code sets them when he's given
//! them, and energy 9. Play never gets that far, so this is how his
//! attacks with them, his arrows landing on a character (#40), and being
//! knocked down and getting up again, are reached and checked: with no
//! energy, a knock-down ends the game. Its seed is the first found that
//! reaches all of those. It
//! is a supplement: states the original can be in, reached sooner than play
//! reaches them. The original boots again from the tape and is played with
//! random presses of the tape's own keys; every call to a rewritten routine
//! is a case, as in play.

use std::collections::BTreeSet;

use robin::assets::Assets;
use zx_recomp::script::Script;
use zx_runtime::memory::Memory;
use zx_runtime::{Misses, Zx, loader::boot_128k};

use crate::Verifier;
use crate::capture::Play;
use crate::tour;

/// How many frames are played, from the hand-over.
const FRAMES: u32 = 7000;

/// The tape's keys (fire 1, up Q, down A, left N, right M), pressed at
/// random once the game is under way. Fire and down are held together
/// now and then, with right or left, once the arrows are likely spent, for
/// the sword's other stroke, which needs both, each way.
const SCRIPT: &str = r#"[game]
name = "armed"
tape = "-"
[trace]
frames = 7000
[[trace.input]]
at = 400
hold = 5
keys = ["0"]
[[trace.input]]
at = 3500
hold = 150
keys = ["1", "a", "m"]
[[trace.input]]
at = 4200
hold = 150
keys = ["1", "a", "n"]
[[trace.input]]
at = 4900
hold = 150
keys = ["1", "a", "m"]
[[trace.input]]
at = 5600
hold = 150
keys = ["1", "a", "n"]
[trace.random]
start = 450
every = 15
keys = ["1", "q", "a", "n", "m"]
seed = 72
"#;

/// What he's given, and the value each takes (`docs/re/robin.md`, *Robin's
/// actions*): the sword, the bow, the arrows the bow comes with, and his
/// energy, at the most the panel's one digit shows.
const ARMS: [(u16, u8); 4] = [(0xD47A, 1), (0xD47B, 1), (0xD47D, 10), (0xD481, 9)];

/// His actions, and his state byte.
const ACTIONS: u16 = 0xC8DF;
const STATE: u16 = 0xCB81;

/// What the game reached: the states his actions were called in.
pub struct Armed {
    pub states: BTreeSet<u8>,
}

/// Plays the game.
pub fn run(rom: &[u8], tape: &[u8], assets: &Assets, v: &mut Verifier) -> Result<Armed, String> {
    let cfg = zx_recomp::Config::parse(SCRIPT)?;
    let mut script = Script::new(&cfg.trace)?;
    let blocks = zx_core::tape::load_tzx(tape)?;
    let mut z = boot_128k(rom, blocks, robin::layout::ENTRY_PC, 2000)?;
    let mut misses = Misses::default();
    let mut armed = false;
    let mut states = BTreeSet::new();
    for frame in 0..FRAMES {
        script.press(frame, &mut z);
        z.run_frame(
            |z: &mut Zx| {
                // Once the game is under way, at the start of its main
                // loop, with bank 0 paged in.
                if !armed && frame > tour::AFTER && tour::at_main_loop(z) {
                    for (at, v) in ARMS {
                        z.memory.poke(at, v);
                    }
                    armed = true;
                }
                if z.pc == ACTIONS && z.memory.slot(3) == Memory::bank(0) {
                    states.insert(z.read(STATE));
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
    if !armed {
        return Err("the armed game never reached its main loop".into());
    }
    Ok(Armed { states })
}
