//! The tour of every location (#6, Decision 8; #28, Decision 2). From the
//! original in play at its main loop, it walks the 16 × 20 grid row by row:
//! each move puts the original at its own leave-the-screen path with the
//! direction, so the original's own step and entry draw the next location;
//! then it lets the original play on a little, with no keys pressed. Every
//! call to a rewritten routine on the way is a case, as in play.

use std::collections::BTreeSet;

use robin::assets::Assets;
use robin::map;
use zx_recomp::script::Script;
use zx_runtime::Zx;

use crate::Verifier;
use crate::capture::Play;

/// The tour starts at the first main loop after this frame of play: the
/// game has started by then (`robin.toml` presses 0 at frame 400).
pub const AFTER: u32 = 450;
/// No keys, for the frames the original plays at each location.
pub const QUIET: &str = "[game]\nname = \"tour\"\ntape = \"-\"\n";
/// The location entered by another path (`docs/re/robin.md`, *The grid*),
/// drawn by the tour through its drawing alone.
pub const THROUGH_ITS_DRAWING: u16 = 0x69;

/// The main loop's first instruction, in bank 2.
const MAIN_LOOP: u16 = 0xBE62;
/// Where the original leaves a screen, with the direction in E.
const LEAVE: u16 = 0xBECE;
/// The step, and the drawing of a location, for the one entered otherwise.
const STEP: u16 = 0xC127;
const DRAW_LOCATION: u16 = 0xBF6A;
/// The location, as the original keeps it.
const LOCATION: u16 = 0xC440;
/// How many frames the original plays at each location before the next move.
const FRAMES_AT_EACH: u64 = 4;
/// How many frames it may take to come back to the main loop after that.
const BACK_WITHIN: u64 = 200;

/// Whether `z` is at the start of the original's main loop.
pub fn at_main_loop(z: &Zx) -> bool {
    z.pc == MAIN_LOOP && z.memory.slot(3) == zx_runtime::memory::Memory::bank(0)
}

pub struct Toured {
    /// The locations the original drew.
    pub drawn: BTreeSet<u16>,
    pub problems: Vec<String>,
}

/// Steps `z` one instruction as play would, with every rewritten routine's
/// calls taken as cases, and the keys for each new frame (none) pressed.
fn step(z: &mut Zx, v: &mut Verifier, script: &mut Script, start: (u64, u32), assets: &Assets) {
    let frame = start.1 + (z.frame - start.0) as u32;
    if z.t >= z.timing.frame {
        script.press(frame + 1, z);
    }
    v.observe(
        z,
        &Play {
            script,
            frame,
            assets,
        },
    );
    z.step_in_frame();
}

/// Runs `z` until the main loop begins again, at least `frames` frames on.
fn play_on(
    z: &mut Zx,
    v: &mut Verifier,
    script: &mut Script,
    start: (u64, u32),
    assets: &Assets,
    frames: u64,
) -> bool {
    let until = z.frame + frames;
    let give_up = until + BACK_WITHIN;
    loop {
        step(z, v, script, start, assets);
        if z.frame >= until && at_main_loop(z) {
            return true;
        }
        if z.frame >= give_up {
            return false;
        }
    }
}

pub fn run(mut z: Zx, frame: u32, quiet: &Script, assets: &Assets, v: &mut Verifier) -> Toured {
    let mut script = quiet.clone();
    let start = (z.frame, frame);
    let mut toured = Toured {
        drawn: BTreeSet::new(),
        problems: Vec::new(),
    };
    let first = z.read16(LOCATION);
    toured.drawn.insert(first);
    // Row by row: fifteen steps right, then one down, twenty times. Then
    // twenty up, so going up, and its wrap from the top row, are taken too.
    let moves = (0..map::ROWS)
        .flat_map(|_| (0..map::COLUMNS - 1).map(|_| 1u8).chain([4]))
        .chain((0..map::ROWS).map(|_| 8));
    for direction in moves {
        let from = z.read16(LOCATION);
        let to = map::step(from, direction);
        if to == THROUGH_ITS_DRAWING {
            // The step, then the drawing, called as the original calls
            // them, returning to the main loop; its own entry is left alone.
            z.sp = z.sp.wrapping_sub(2);
            z.write16(z.sp, MAIN_LOOP);
            z.sp = z.sp.wrapping_sub(2);
            z.write16(z.sp, DRAW_LOCATION);
            z.a = direction;
            z.pc = STEP;
        } else {
            z.e = direction;
            z.pc = LEAVE;
        }
        if !play_on(&mut z, v, &mut script, start, assets, FRAMES_AT_EACH) {
            toured.problems.push(format!(
                "after moving from {from:#05x} to {to:#05x}, the original did not come back to its main loop"
            ));
            break;
        }
        let now = z.read16(LOCATION);
        if now != to {
            toured.problems.push(format!(
                "moving from {from:#05x}, the original went to {now:#05x}, not {to:#05x}"
            ));
        }
        toured.drawn.insert(now);
    }
    toured
}
