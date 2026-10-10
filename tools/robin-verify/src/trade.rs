//! A game in which Robin trades (#54). The trade, at the first special
//! location, needs three kind-2 items carried and R at `0x13` as its check
//! runs, once a visit; play never brings the items there. So every 400
//! frames the original is sent into the trade's location through its own
//! step and entry, as the traveller's game does (#8), with three kind-2
//! items in Robin's inventory, and he stands there while R comes round. It
//! is a supplement: states the original can be in, reached sooner than play
//! reaches them.

use robin::assets::Assets;
use zx_recomp::script::Script;
use zx_runtime::{Misses, Zx, loader::boot_128k};

use crate::Verifier;
use crate::capture::Play;
use crate::tour;

/// How many frames are played, from the hand-over, and how often he's
/// sent to the trade again.
const FRAMES: u32 = 6000;
const EVERY: u32 = 400;

/// 0 to start; then nothing, so he stands where he's put.
const SCRIPT: &str = r#"[game]
name = "trade"
tape = "-"
[trace]
frames = 6000
[[trace.input]]
at = 400
hold = 5
keys = ["0"]
"#;

/// Where the original leaves a screen, with the direction in E.
const LEAVE: u16 = 0xBECE;
const RIGHT: u8 = 1;
/// The trade's location, his inventory, and his energy.
const TRADE_AT: u16 = 0xD28F;
const INVENTORY: u16 = 0xD472;
const ENERGY: u16 = 0xD481;
/// Where the trade itself starts, past its tests.
const TRADED: u16 = 0xDE1D;

/// Plays the game. How many times he traded.
pub fn run(rom: &[u8], tape: &[u8], assets: &Assets, v: &mut Verifier) -> Result<u32, String> {
    let cfg = zx_recomp::Config::parse(SCRIPT)?;
    let mut script = Script::new(&cfg.trace)?;
    let blocks = zx_core::tape::load_tzx(tape)?;
    let mut z = boot_128k(rom, blocks, robin::layout::ENTRY_PC, 2000)?;
    let mut misses = Misses::default();
    let mut sent = 0;
    let mut traded = 0;
    let mut due = tour::AFTER;
    for frame in 0..FRAMES {
        script.press(frame, &mut z);
        z.run_frame(
            |z: &mut Zx| {
                if frame >= due && tour::at_main_loop(z) {
                    let to = u16::from(z.read(TRADE_AT));
                    let from = to & 0xFFF0 | (to.wrapping_sub(1) & 0x0F);
                    let [lo, hi] = from.to_le_bytes();
                    z.memory.poke(0xC440, lo);
                    z.memory.poke(0xC441, hi);
                    for n in 0..8 {
                        z.memory.poke(INVENTORY + n, if n < 3 { 2 } else { 0xFF });
                    }
                    z.memory.poke(ENERGY, 9);
                    z.e = RIGHT;
                    z.pc = LEAVE;
                    sent += 1;
                    due = frame + EVERY;
                }
                if z.pc == TRADED && z.memory.slot(3) == zx_runtime::memory::Memory::bank(0) {
                    traded += 1;
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
    if sent == 0 {
        return Err("the trade's game never reached its main loop".into());
    }
    Ok(traded)
}
