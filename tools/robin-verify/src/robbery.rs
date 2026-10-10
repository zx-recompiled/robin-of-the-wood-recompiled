//! A game in which Robin robs the fifth character (#53). It brings two
//! characters on along a route, and Robin robs the first by striking it,
//! which play never brings about: the pair are seldom on his screen while
//! he's armed. So, as the armed game does, Robin is given the sword, the
//! bow and arrows, and while the first is out and not yet robbed it's put in
//! front of him, at his location, as its own walking would bring it there.
//! Fire is pressed at random. It is a supplement: states the original can be
//! in, reached sooner than play reaches them.

use robin::assets::Assets;
use zx_recomp::script::Script;
use zx_runtime::{Misses, Zx, loader::boot_128k};

use crate::Verifier;
use crate::capture::Play;
use crate::tour;

/// How many frames are played, from the hand-over.
const FRAMES: u32 = 6000;

/// 0 to start; then fire held often, facing right, with steps either way.
const SCRIPT: &str = r#"[game]
name = "robbery"
tape = "-"
[trace]
frames = 6000
[[trace.input]]
at = 400
hold = 5
keys = ["0"]
[trace.random]
start = 450
every = 10
keys = ["1", "1", "1", "m", "n"]
seed = 53
"#;

/// What he's given (the armed game's): the sword, the bow, ten arrows, and
/// energy.
const ARMS: [(u16, u8); 4] = [(0xD47A, 1), (0xD47B, 1), (0xD47D, 10), (0xD481, 9)];

/// The first of the pair's record, who's out (bit 1) and whether it's been
/// robbed (bit 2), and where the robbery starts.
const FIRST: u16 = 0xAAE4;
const OUT: u16 = 0xBA61;
const ROBBED: u16 = 0xB869;

/// Robin's location and position.
const LOCATION: u16 = 0xC440;
const ROBIN_X: u16 = 0xCB7F;
const ROBIN_Y: u16 = 0xCB80;

/// Plays the game. How many times the robbery ran.
pub fn run(rom: &[u8], tape: &[u8], assets: &Assets, v: &mut Verifier) -> Result<u32, String> {
    let cfg = zx_recomp::Config::parse(SCRIPT)?;
    let mut script = Script::new(&cfg.trace)?;
    let blocks = zx_core::tape::load_tzx(tape)?;
    let mut z = boot_128k(rom, blocks, robin::layout::ENTRY_PC, 2000)?;
    let mut misses = Misses::default();
    let mut armed = false;
    let mut robbed = 0;
    for frame in 0..FRAMES {
        script.press(frame, &mut z);
        z.run_frame(
            |z: &mut Zx| {
                if frame > tour::AFTER && tour::at_main_loop(z) {
                    if !armed {
                        for (at, v) in ARMS {
                            z.memory.poke(at, v);
                        }
                        armed = true;
                    }
                    let out = z.read(OUT);
                    if out & 2 != 0 && out & 4 == 0 {
                        let [lo, hi] = z.read16(LOCATION).to_le_bytes();
                        z.memory.poke(FIRST + 0x10, lo);
                        z.memory.poke(FIRST + 0x11, hi);
                        z.memory.poke(FIRST + 9, z.read(ROBIN_X).wrapping_add(0x10));
                        z.memory.poke(FIRST + 10, z.read(ROBIN_Y));
                    }
                }
                if z.pc == ROBBED {
                    robbed += 1;
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
        return Err("the robbery's game never reached its main loop".into());
    }
    Ok(robbed)
}
