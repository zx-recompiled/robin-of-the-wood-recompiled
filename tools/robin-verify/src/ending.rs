//! A game that reaches the ending (#60, Decision 3). Location `0x69` is
//! entered by its own path, the ending, which play and the tour never take:
//! the tour only draws it. Once the game is under way, the original is put
//! at its own leave-the-screen path at location `0x68`, heading right, as the
//! tour does when a wall is in the way, and its own step takes it into
//! `0x69`. It is a supplement, as the armed game is: a state the original can
//! be in, reached sooner than play reaches it. A key is pressed later, for
//! the ending's wait, and the new game that follows plays on until BREAK is
//! held, which play never holds either.

use robin::assets::Assets;
use zx_recomp::script::Script;
use zx_runtime::{Misses, Zx, loader::boot_128k};

use crate::Verifier;
use crate::capture::Play;
use crate::tour;

/// How many frames are played, from the hand-over.
const FRAMES: u32 = 3000;

/// The key that starts the game, then one for the ending's wait, the one
/// that starts the next game, and BREAK (Caps Shift with Space).
const SCRIPT: &str = r#"[game]
name = "ending"
tape = "-"
[trace]
frames = 3000
[[trace.input]]
at = 400
hold = 5
keys = ["0"]
[[trace.input]]
at = 1200
hold = 5
keys = ["m"]
[[trace.input]]
at = 1800
hold = 5
keys = ["0"]
[[trace.input]]
at = 2600
hold = 5
keys = ["caps", "space"]
"#;

/// Where the original leaves a screen, with the direction in E, and the
/// location it leaves.
const LEAVE: u16 = 0xBECE;
const RIGHT: u8 = 1;
const BESIDE: u16 = 0x0068;
/// The ending's message (`0:CF5F`), reached once it's under way.
const ENDING: u16 = 0xCF5F;

/// What the game reached: the ending, and a new game from BREAK.
pub struct Ending {
    pub ended: bool,
    pub broke: bool,
}

/// Where BREAK goes, for a new game.
const BREAK: u16 = 0xBE5A;

/// Plays the game.
pub fn run(rom: &[u8], tape: &[u8], assets: &Assets, v: &mut Verifier) -> Result<Ending, String> {
    let cfg = zx_recomp::Config::parse(SCRIPT)?;
    let mut script = Script::new(&cfg.trace)?;
    let blocks = zx_core::tape::load_tzx(tape)?;
    let mut z = boot_128k(rom, blocks, robin::layout::ENTRY_PC, 2000)?;
    let mut misses = Misses::default();
    let mut sent = false;
    let mut reached = false;
    let mut broke = false;
    for frame in 0..FRAMES {
        script.press(frame, &mut z);
        z.run_frame(
            |z: &mut Zx| {
                if !sent && frame > tour::AFTER && tour::at_main_loop(z) {
                    let [lo, hi] = BESIDE.to_le_bytes();
                    z.memory.poke(0xC440, lo);
                    z.memory.poke(0xC441, hi);
                    z.e = RIGHT;
                    z.pc = LEAVE;
                    sent = true;
                }
                if z.pc == ENDING {
                    reached = true;
                }
                if reached && frame >= 2600 && z.pc == BREAK {
                    broke = true;
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
    if !sent {
        return Err("the ending's game never reached its main loop".into());
    }
    Ok(Ending {
        ended: reached,
        broke,
    })
}
