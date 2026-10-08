//! A game in which Robin finds things to pick up where he starts (#41, its
//! plan's Task 6): play almost never walks him into an item, so this is how
//! picking each kind up, carrying them, and dropping one when his inventory
//! is full are reached and checked. It is a supplement, as the armed game
//! is: states the original can be in, reached sooner than play reaches
//! them. Just before the new game places the first location's items, seven
//! more are added to the list of items dropped there (as dropping one puts
//! it there), within his reach, and his inventory is filled. He stands
//! still, and picks them up one a call.

use robin::assets::Assets;
use zx_recomp::script::Script;
use zx_runtime::memory::Memory;
use zx_runtime::{Misses, Zx, loader::boot_128k};

use crate::Verifier;
use crate::capture::Play;
use crate::tour;

/// How many frames are played, from the hand-over.
const FRAMES: u32 = 2000;

/// No keys but the one that starts the game: he stands where he starts.
const SCRIPT: &str = r#"[game]
name = "collector"
tape = "-"
[trace]
frames = 2000
[[trace.input]]
at = 400
hold = 5
keys = ["0"]
"#;

/// Where the items placed on entering a location are read from.
const PLACE: u16 = 0xD484;
/// The list of dropped items: eleven of five bytes. The game puts two
/// there when it starts; these go in the third to the ninth, leaving the
/// last two free (an item in the eleventh at this location makes the
/// original return to its caller's caller, *Items*).
const DROPPED: u16 = 0xD420;
/// The kinds, in the order he'll pick them up: energy, the bow, the sword,
/// the third, two to carry, and arrows last (with arrows left, an arrow
/// item stops him looking at the others).
const KINDS: [u8; 7] = [1, 3, 0, 6, 5, 2, 7];
/// His inventory, full, with a kind 2 last: the first item he carries
/// pushes it out, and it's dropped.
const INVENTORY: u16 = 0xD472;
const CARRIED: [u8; 8] = [5, 5, 5, 2, 2, 2, 2, 2];

/// Plays the game. Returns how many items he picked up.
pub fn run(rom: &[u8], tape: &[u8], assets: &Assets, v: &mut Verifier) -> Result<u32, String> {
    let cfg = zx_recomp::Config::parse(SCRIPT)?;
    let mut script = Script::new(&cfg.trace)?;
    let blocks = zx_core::tape::load_tzx(tape)?;
    let mut z = boot_128k(rom, blocks, robin::layout::ENTRY_PC, 2000)?;
    let mut misses = Misses::default();
    let mut placed = false;
    let mut picked = 0;
    for frame in 0..FRAMES {
        script.press(frame, &mut z);
        z.run_frame(
            |z: &mut Zx| {
                if !placed && frame > tour::AFTER - 50 && z.pc == PLACE && in_bank0(z) {
                    // Within his reach: 0x10 to his left, 0x14 below him.
                    let [lo, hi] = z.read16(0xC440).to_le_bytes();
                    let x = z.read(0xCB7F).wrapping_sub(0x10);
                    let y = z.read(0xCB80).wrapping_add(0x14);
                    for (n, kind) in KINDS.into_iter().enumerate() {
                        let at = DROPPED + 10 + n as u16 * 5;
                        for (i, b) in [lo, hi, x, y, kind].into_iter().enumerate() {
                            z.memory.poke(at + i as u16, b);
                        }
                    }
                    for (n, kind) in CARRIED.into_iter().enumerate() {
                        z.memory.poke(INVENTORY + n as u16, kind);
                    }
                    placed = true;
                }
                // Taking one (`0:D705`, once one's within reach).
                if z.pc == 0xD705 && in_bank0(z) {
                    picked += 1;
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
    if !placed {
        return Err("the collector's game never placed its first location's items".into());
    }
    Ok(picked)
}

fn in_bank0(z: &Zx) -> bool {
    z.memory.slot(3) == Memory::bank(0)
}
