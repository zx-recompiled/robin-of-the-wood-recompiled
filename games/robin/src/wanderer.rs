//! The wanderer: a friendly character who walks the forest's rows from
//! location to location (`docs/re/robin.md`, *The wanderer*).

use crate::actions;
use crate::game::Game;
use crate::io::Io;
use crate::sound;
use crate::sprites::{self, Sprites};

/// Its sprite record, and the frame table it's drawn from.
const RECORD: u16 = 0xBB1B;
const FRAMES: u16 = 0x8C8F;
/// Its animation sequences, walking right and left.
const RIGHT: u16 = 0xBB2B;
const LEFT: u16 = 0xBB37;
/// Its step, as the opcode the original keeps: adds 1 to its position, or
/// takes 1 away.
const INC: u8 = 0x34;
const DEC: u8 = 0x35;
/// Its flags: set when it first turns.
const TURNED: u8 = 0x80;

/// Puts it at the second special location, walking right, as a game starts
/// (`0:CEA1`).
pub fn set_up(g: &mut Game) {
    g.wanderer.location = g.map.specials[1];
    g.wanderer.position = 0x38;
    g.wanderer.timer = 0x70;
    g.wanderer.flags = 0;
    g.wanderer.step &= !1;
}

/// Every fourth call: its timer counts down, turning it round when it runs
/// out; it steps one position, and into the next location past an edge;
/// and it's drawn if it's on Robin's screen, or about to walk onto it from
/// the right (`0xBA83`).
///
/// # Panics
///
/// If its step isn't `INC (HL)` or `DEC (HL)`, the only two it's ever set
/// to.
pub fn walk(g: &mut Game, sprites: &Sprites) {
    let w = &mut g.wanderer;
    w.counter = w.counter.wrapping_sub(1);
    if w.counter & 3 != 0 {
        return;
    }
    g.sprites.table = FRAMES;
    let w = &mut g.wanderer;
    w.timer = w.timer.wrapping_sub(1);
    if w.timer == 0 {
        w.timer = 0xE0;
        w.step ^= 1;
        let sequence = if w.step & 1 == 0 { RIGHT } else { LEFT };
        w.record[2..4].copy_from_slice(&sequence.to_le_bytes());
        w.flags |= TURNED;
    }
    let position = match w.step {
        INC => w.position.wrapping_add(1),
        DEC => w.position.wrapping_sub(1),
        s => panic!("the wanderer's step at 0xBAB6 is {s:#04x}, neither INC (HL) nor DEC (HL)"),
    };
    w.position = position;
    if position & 0x80 != 0 {
        w.location = w.location.wrapping_sub(1);
        w.position = position.wrapping_add(0x70);
    } else if position >= 0x70 {
        w.location = w.location.wrapping_add(1);
        w.position = position - 0x70;
    }
    let here = g.map.location;
    let flags = RECORD + 4;
    let x = if here == w.location {
        Some(w.position)
    } else if here == w.location.wrapping_sub(1) && w.position < 0x10 {
        Some(w.position + 0x70)
    } else {
        None
    };
    match x {
        Some(x) => {
            g.wanderer.record[9] = x;
            g.write(flags, g.read(flags) | 0x05);
            sprites::animate(g, sprites, RECORD);
        }
        None => {
            g.write(flags, g.read(flags) & !0x04);
            sprites::animate(g, sprites, RECORD);
            g.write(flags, g.read(flags) & !0x01);
        }
    }
}

/// Something with a position and the limits it overlaps by: closer than
/// `down` vertically and `across` horizontally.
#[derive(Clone, Copy)]
pub struct Thing {
    pub x: u8,
    pub y: u8,
    pub down: u8,
    pub across: u8,
}

/// Whether `first` and `second` overlap (`0xBDB9`). It goes as the
/// original's register sets do. Vertically, the limit is `second`'s if
/// `first` is at or below it, and `first`'s otherwise. Horizontally, from
/// the one whose limit was used: the other's limit if it's at or to the
/// right of the other, and its own otherwise.
#[must_use]
pub fn overlap(first: Thing, second: Thing) -> bool {
    let (on, off) = if first.y >= second.y {
        (second, first)
    } else {
        (first, second)
    };
    if on.y.abs_diff(off.y) >= on.down {
        return false;
    }
    let limit = if on.x >= off.x { off.across } else { on.across };
    on.x.abs_diff(off.x) < limit
}

/// Its flags: set once Robin has met it.
const MET: u8 = 0x40;
/// The play area's attributes, which flash when they meet.
const PLAY_ATTRIBUTES: u16 = 0x5800;
const PLAY_CELLS: u16 = 0x240;

/// Robin meeting it, once a game: if it's drawn and he overlaps it, a sound
/// starts, the play area flashes, the lower panel's colours are reset, and
/// his energy goes up (`0xBD87`).
pub fn meet(g: &mut Game, io: &mut Io) {
    let w = &g.wanderer;
    if w.record[4] & 0x01 == 0 || w.flags & MET != 0 {
        return;
    }
    let robin = Thing {
        x: g.robin.x,
        y: g.robin.y,
        down: 0x20,
        across: 0x0C,
    };
    let it = Thing {
        x: w.record[9],
        y: w.record[10],
        down: 0x20,
        across: 0x0C,
    };
    if !overlap(robin, it) {
        return;
    }
    flash(g, io);
    g.write(0xBE47, 0x0F);
    actions::panel_colours(g);
    actions::energy(g);
    g.wanderer.flags |= MET;
}

/// The meeting's sound, then the play area's ink moved on by one, 16 times
/// over (`0xBDCD`).
pub fn flash(g: &mut Game, io: &mut Io) {
    actions::banked_call(g, io, sound::MUSIC_BANK, sound::meeting);
    for _ in 0..16 {
        for at in PLAY_ATTRIBUTES..PLAY_ATTRIBUTES + PLAY_CELLS {
            let a = g.read(at);
            g.write(at, a & 0xF8 | (a.wrapping_add(1) & 7));
        }
    }
}
