//! A game that takes the journeys (#79). A doorway's sparkle ends by sending
//! Robin on by what he carries: with three kind-5 items to location `0xCC`,
//! with nothing of kind 5 or 2 to `0x9C`. Play never brings the right items
//! to a doorway. So every 500 frames the original is sent into one of the
//! nine doorways through its own step and entry, as the traveller's game
//! does (#8), carrying, in turn: three kind-5 items and the third item,
//! which the journey takes back; a single kind-4; two kind-5 items, which
//! restore his health instead; a kind-2, which is taken back; and one
//! kind-5, which does nothing. Robin stands there while the sparkle plays
//! and the journey runs. It is a supplement: states the original can be in,
//! reached sooner than play reaches them.

use robin::assets::Assets;
use zx_recomp::script::Script;
use zx_runtime::{Misses, Zx, loader::boot_128k};

use crate::Verifier;
use crate::capture::Play;
use crate::tour;

/// How many frames are played, from the hand-over, and how often it's sent
/// to a doorway.
const FRAMES: u32 = 6000;
const EVERY: u32 = 500;

/// 0 to start; then nothing, as the doorway walks him in.
const SCRIPT: &str = r#"[game]
name = "journey"
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
/// The nine doorways, his inventory, and his energy.
const DOORWAYS: u16 = 0xDAB9;
const INVENTORY: u16 = 0xD472;
const ENERGY: u16 = 0xD481;
/// Whether he has the third item, which a journey takes back.
const THIRD: u16 = 0xD47C;
/// Where a journey enters its new location, in the main loop's hook.
const JOURNEY: u16 = 0xC3D2;

/// Plays the game. How many journeys it took.
pub fn run(rom: &[u8], tape: &[u8], assets: &Assets, v: &mut Verifier) -> Result<u32, String> {
    let cfg = zx_recomp::Config::parse(SCRIPT)?;
    let mut script = Script::new(&cfg.trace)?;
    let blocks = zx_core::tape::load_tzx(tape)?;
    let mut z = boot_128k(rom, blocks, robin::layout::ENTRY_PC, 2000)?;
    let mut misses = Misses::default();
    let mut sent = 0u16;
    let mut journeys = 0;
    let mut due = tour::AFTER;
    for frame in 0..FRAMES {
        script.press(frame, &mut z);
        z.run_frame(
            |z: &mut Zx| {
                if frame >= due && tour::at_main_loop(z) {
                    let to = z.read16(DOORWAYS + (sent % 9) * 2);
                    let from = to & 0xFFF0 | (to.wrapping_sub(1) & 0x0F);
                    let [lo, hi] = from.to_le_bytes();
                    z.memory.poke(0xC440, lo);
                    z.memory.poke(0xC441, hi);
                    let carried: &[u8] = match sent % 5 {
                        0 => &[5, 5, 5],
                        1 => &[4],
                        2 => &[5, 5],
                        3 => &[2],
                        _ => &[5],
                    };
                    z.memory.poke(THIRD, u8::from(sent.is_multiple_of(5)));
                    for n in 0..8 {
                        z.memory.poke(
                            INVENTORY + n,
                            carried.get(usize::from(n)).copied().unwrap_or(0xFF),
                        );
                    }
                    z.memory.poke(ENERGY, 9);
                    z.e = RIGHT;
                    z.pc = LEAVE;
                    sent += 1;
                    due = frame + EVERY;
                }
                if z.pc == JOURNEY && z.memory.slot(3) == zx_runtime::memory::Memory::bank(0) {
                    journeys += 1;
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
        return Err("the journey's game never reached its main loop".into());
    }
    Ok(journeys)
}
