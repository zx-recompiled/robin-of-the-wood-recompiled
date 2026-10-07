//! The tour of every location (#6, Decision 8; #28, Decision 2). From the
//! original in play at its main loop, it walks the 16 × 20 grid row by row.
//! Each move walks Robin off the screen, holding the direction through the
//! controls until the original's own edge check takes him to the next
//! location (#34, Decision 4). If a wall or a character stops him, the move
//! is undone and made by putting the original at its own leave-the-screen
//! path with the direction instead. Either way, the original's own step and
//! entry draw the next location, and it runs until it is back at its main
//! loop. Then a copy of
//! it plays on for half a second, with no keys pressed, and is thrown away:
//! whatever happens there (Robin walking on, or dying) never disturbs the
//! walk. Every call to a rewritten routine, in the walk and in the copies,
//! is a case, as in play.

use std::collections::BTreeSet;

use robin::assets::Assets;
use robin::map;
use zx_recomp::script::Script;
use zx_runtime::Zx;
use zx_runtime::keys::Key;

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
/// How many frames the original plays at each location before the next move:
/// half a second, so the characters there animate and move (#32).
const FRAMES_AT_EACH: u64 = 25;
/// How many frames entering a location may take, back to the main loop.
const BACK_WITHIN: u64 = 200;

/// The location, from bank 0 whatever is paged: the interrupt pages bank 6
/// in for the music, so reading through the address would see bank 6's
/// bytes then.
fn location(z: &Zx) -> u16 {
    let bank0 = z.memory.page(zx_runtime::memory::Memory::bank(0));
    let at = usize::from(LOCATION - 0xC000);
    u16::from_le_bytes([bank0[at], bank0[at + 1]])
}

/// Whether `z` is at the start of the original's main loop.
pub fn at_main_loop(z: &Zx) -> bool {
    z.pc == MAIN_LOOP && z.memory.slot(3) == zx_runtime::memory::Memory::bank(0)
}

pub struct Toured {
    /// The locations the original drew.
    pub drawn: BTreeSet<u16>,
    /// The moves Robin walked off the screen, and those that fell back to
    /// entering the next location directly.
    pub walked: u32,
    pub fell_back: u32,
    pub problems: Vec<String>,
}

/// How long a walk to the edge may take before the tour gives it up.
const WALK_WITHIN: u64 = 400;

/// The key that walks Robin each way with the tape's own keys (fire 1, up
/// Q, down A, left N, right M; `docs/re/robin.md`, *The controls*): the
/// controls play leaves in use.
fn key_for(direction: u8) -> Key {
    let name = match direction {
        1 => "m",
        2 => "n",
        4 => "a",
        _ => "q",
    };
    Key::by_name(name).expect("a key")
}

/// Walks Robin off the screen by `direction`, holding its key through the
/// controls until the original's own edge check takes him to `to`, and the
/// main loop begins there. Whether it got there.
fn walk(
    z: &mut Zx,
    v: &mut Verifier,
    script: &mut Script,
    start: (u64, u32),
    assets: &Assets,
    direction: u8,
    to: u16,
) -> bool {
    let from = location(z);
    let key = key_for(direction);
    let give_up = z.frame + WALK_WITHIN;
    while location(z) == from {
        z.set_key(key, true);
        step(z, v, script, start, assets);
        if z.frame >= give_up {
            return false;
        }
    }
    z.set_key(key, false);
    location(z) == to && enter(z, v, script, start, assets)
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

/// Runs `z` until the main loop begins again, after the move just made.
fn enter(
    z: &mut Zx,
    v: &mut Verifier,
    script: &mut Script,
    start: (u64, u32),
    assets: &Assets,
) -> bool {
    let give_up = z.frame + BACK_WITHIN;
    loop {
        step(z, v, script, start, assets);
        if at_main_loop(z) {
            return true;
        }
        if z.frame >= give_up {
            return false;
        }
    }
}

/// Plays `z` on for `frames` frames, every call taken as a case.
fn play_on(
    z: &mut Zx,
    v: &mut Verifier,
    script: &mut Script,
    start: (u64, u32),
    assets: &Assets,
    frames: u64,
) {
    let until = z.frame + frames;
    while z.frame < until {
        step(z, v, script, start, assets);
    }
}

/// Walking every move takes about ten minutes (#34, Decision 5), so the gate
/// walks one move in this many and enters the rest directly; `--full` walks
/// them all.
pub const WALK_ONE_IN: usize = 4;

/// Tours every location from `z`, walking one move in `walk_one_in`.
pub fn run(
    mut z: Zx,
    frame: u32,
    quiet: &Script,
    assets: &Assets,
    v: &mut Verifier,
    walk_one_in: usize,
) -> Toured {
    let mut script = quiet.clone();
    let start = (z.frame, frame);
    let mut toured = Toured {
        drawn: BTreeSet::new(),
        walked: 0,
        fell_back: 0,
        problems: Vec::new(),
    };
    // Walking needs the tape's keys, which play leaves in use.
    let tape_keys = z.read16(0xD152) == robin::movement::REDEFINED_KEYS
        && (0..5)
            .map(|i| z.read(0xD154 + i))
            .eq([0x24, 0x25, 0x26, 0x08, 0x10]);
    // One step left and back, so the starting location is drawn by the
    // tour too. Then row by row: fifteen steps right, then one down, twenty
    // times. Then twenty up, so going up, and its wrap from the top row, are
    // taken too.
    let moves = [2u8, 1].into_iter().chain(
        (0..map::ROWS)
            .flat_map(|_| (0..map::COLUMNS - 1).map(|_| 1u8).chain([4]))
            .chain((0..map::ROWS).map(|_| 8)),
    );
    for (n, direction) in moves.enumerate() {
        let from = location(&z);
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
            // Walked off the edge, if he can be; if a wall or a character
            // stops him, back to before the walk, and in at the next
            // location directly, as the original's step and entry do it.
            let before = (z.clone(), script.clone());
            let walking = tape_keys && n % walk_one_in == 0;
            if walking && walk(&mut z, v, &mut script, start, assets, direction, to) {
                toured.walked += 1;
                let (mut copy, mut keys) = (z.clone(), script.clone());
                play_on(&mut copy, v, &mut keys, start, assets, FRAMES_AT_EACH);
                toured.drawn.insert(location(&z));
                continue;
            }
            (z, script) = before;
            if walking {
                toured.fell_back += 1;
            }
            z.e = direction;
            z.pc = LEAVE;
        }
        if !enter(&mut z, v, &mut script, start, assets) {
            toured.problems.push(format!(
                "after moving from {from:#05x} to {to:#05x}, the original did not come back to its main loop"
            ));
            break;
        }
        // A copy plays on, and is thrown away.
        let (mut copy, mut keys) = (z.clone(), script.clone());
        play_on(&mut copy, v, &mut keys, start, assets, FRAMES_AT_EACH);
        let now = location(&z);
        if now != to {
            toured.problems.push(format!(
                "moving from {from:#05x}, the original went to {now:#05x}, not {to:#05x}"
            ));
        }
        toured.drawn.insert(now);
    }
    toured
}
