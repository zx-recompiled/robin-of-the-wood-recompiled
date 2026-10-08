//! The fighting: the objects in flight, hits both ways, Robin's health
//! (`docs/re/robin.md`, *Fighting*).

use crate::actions;
use crate::game::Game;
use crate::io::Io;
use crate::sound;
use crate::wanderer::{Thing, overlap};

/// The back buffer and the changed-cell map (*The screen*), and the mark a
/// drawn object leaves.
const BACK_BUFFER: u16 = 0xEB00;
const CHANGED: u16 = 0xE500;
const MARK: u8 = 0x46;
/// A slot's row byte: just fired, and hit something.
const FIRED: u8 = 0x80;
const HIT: u8 = 0x40;

/// Moves the four objects in flight a column each: erased (unless just
/// fired), stepped their way, and drawn again, or freed if they've hit
/// something or reached the play area's edge (`0xBB84`). Whether it erases
/// and draws, it keeps as the offsets of two of the original's jumps.
pub fn flight(g: &mut Game) {
    for slot in 0..4 {
        let row_byte = g.fighting.slots[slot * 2 + 1];
        g.fighting.slots[slot * 2 + 1] = row_byte & !(FIRED | HIT);
        g.fighting.skip_erase = if row_byte & FIRED != 0 { 3 } else { 0 };
        g.fighting.skip_draw = if row_byte & HIT == 0 { 5 } else { 0 };
        let first = g.fighting.slots[slot * 2];
        if first == 0 {
            continue;
        }
        let row = g.fighting.slots[slot * 2 + 1];
        let column = first & 0x7F;
        if g.fighting.skip_erase == 0 {
            draw(g, cell(row, column), row, column);
        }
        let column = if first & 0x80 != 0 {
            column.wrapping_sub(1)
        } else {
            column.wrapping_add(1)
        };
        if g.fighting.skip_draw == 0 {
            g.fighting.slots[slot * 2] = 0;
            continue;
        }
        draw(g, cell(row, column), row, column);
        g.fighting.slots[slot * 2] = if (2..0x1E).contains(&column) {
            column | first & 0x80
        } else {
            0
        };
    }
}

/// Where an object at `row` and `column` is drawn: the top pixel row of its
/// cell in the back buffer.
fn cell(row: u8, column: u8) -> u16 {
    BACK_BUFFER.wrapping_add(u16::from_be_bytes([row, column]))
}

/// Draws an object at `at` in the back buffer, or erases it, as it's an
/// XOR of 8 pixels, and marks its cell, `row` and `column`, changed
/// (`0xBBEC`).
pub fn draw(g: &mut Game, at: u16, row: u8, column: u8) {
    g.write(at, g.read(at) ^ 0xFF);
    let cell = u16::from_be_bytes([row >> 3, (row & 7) << 5 | column]);
    g.write(CHANGED.wrapping_add(cell), MARK);
}

/// Robin hit, unless he's already down: his health goes down by one, or
/// at 2 he's knocked down and the wanderer can be met again; the lower
/// panel shows it (`0xBCFB`).
pub fn robin_hit(g: &mut Game) {
    if g.robin.down != 0 {
        return;
    }
    if g.fighting.health == 2 {
        g.robin.down = 0x6E;
        g.wanderer.flags &= !0x40;
    } else {
        g.fighting.health = g.fighting.health.wrapping_sub(1);
    }
    actions::panel_colours(g);
}

/// Strikes the character whose record `which` counts down to (5 the first,
/// 1 the fifth), `record`, the one the caller is at: bit 5 of its flags is
/// set, and for one of the row's four, bit 5 of its state too, so it stops
/// (`0xBDF2`).
pub fn struck(g: &mut Game, which: u8, record: u16) {
    if which != 1 {
        let n = u16::from(5u8.wrapping_sub(which));
        let state = g.characters.row.wrapping_add(n * 3 + 2);
        g.write(state, g.read(state) | 0x20);
    }
    let flags = record.wrapping_add(4);
    g.write(flags, g.read(flags) | 0x20);
}

/// The five records a hit can strike: the four characters' and one more.
const RECORDS: u16 = 0xAAB8;
/// The second group's four records, at the locations from 256 up.
const SECOND_GROUP: u16 = 0xDBF1;

/// The record a count of `which` (5 down to 1) is at.
fn record(which: u8) -> u16 {
    RECORDS + u16::from(5 - which) * 11
}

/// Whether record `at` is drawn and not stopped, so it can be hit.
fn hittable(g: &Game, at: u16) -> bool {
    let flags = g.read(at + 4);
    flags & 0x01 != 0 && flags & 0x20 == 0
}

/// Whether `column`, as an object's, is within 8 to the right of `x`: its
/// column, plus 2, times 4.
fn near(column: u8, x: u8) -> bool {
    let at = column.wrapping_add(2).wrapping_shl(2);
    at.checked_sub(x).is_some_and(|d| d < 8)
}

/// A shot hitting Robin, while he isn't down: marked hit, he's hit, and the
/// fourth sample plays (`0xBC6C`).
pub fn shot_hits(g: &mut Game, io: &mut Io) {
    if g.robin.down != 0 {
        return;
    }
    let (x, y) = (g.robin.x, g.robin.y);
    for slot in 1..4 {
        let column = g.fighting.slots[slot * 2];
        if column == 0 || !near(column, x) {
            continue;
        }
        let row = g.fighting.slots[slot * 2 + 1].wrapping_shl(3);
        if !row.checked_sub(y).is_some_and(|d| d < 0x24) {
            continue;
        }
        g.fighting.slots[slot * 2 + 1] |= HIT;
        robin_hit(g);
        actions::banked_call(g, io, sound::PLAYER_BANK as u8, sound::shot_sample);
        return;
    }
}

/// The tune and the zap a character struck sets off (`0xBC46`).
fn struck_sounds(g: &mut Game, io: &mut Io) {
    actions::banked_call(g, io, sound::MUSIC_BANK, sound::hit_tune);
    sound::zap(io);
}

/// Robin's arrow, in the rows the characters walk, hitting one of the five
/// records: marked hit, the character struck, and the tune and zap
/// (`0xBC0B`).
pub fn arrow_hits(g: &mut Game, io: &mut Io) {
    let column = g.fighting.slots[0];
    if column == 0 || !(0x0A..0x0E).contains(&(g.fighting.slots[1] & 0x7F)) {
        return;
    }
    for which in (1..=5).rev() {
        let at = record(which);
        if !hittable(g, at) || !near(column, g.read(at + 9)) {
            continue;
        }
        g.fighting.slots[1] |= HIT;
        struck(g, which, at);
        struck_sounds(g, io);
        return;
    }
}

/// Robin's sword and fists: on the frames of his attack that strike, a
/// point ahead of him that overlaps one of the five records strikes it,
/// with the tune and zap (`0xBD19`).
pub fn strike(g: &mut Game, io: &mut Io) {
    let frame = g.read(g.robin.sequence) & 0x7F;
    let left = g.robin.state & 1 != 0;
    let (down, across) = match frame {
        0x1A | 0x1F => (0, if left { 0 } else { 4 }),
        0x1C => (8, if left { 0xFA } else { 0x0A }),
        _ => return,
    };
    let point = Thing {
        x: g.robin.x.wrapping_add(across),
        y: g.robin.y.wrapping_add(down),
        down: 0x10,
        across: 0x05,
    };
    for which in (1..=5).rev() {
        let at = record(which);
        if !hittable(g, at) {
            continue;
        }
        let it = Thing {
            x: g.read(at + 9),
            y: g.read(at + 10),
            down: 0x18,
            across: 0x0B,
        };
        if overlap(it, point) {
            struck(g, which, at);
            struck_sounds(g, io);
            return;
        }
    }
}

/// At the locations from 256 up, while Robin isn't down and the cooldown
/// has run out: one of the second group touching him hits him, plays a
/// sample by R, and starts the cooldown (`0xBCAA`).
pub fn second_group_hits(g: &mut Game, io: &mut Io) {
    if g.map.location >> 8 == 0 || g.robin.down != 0 {
        return;
    }
    if g.fighting.cooldown != 0 {
        g.fighting.cooldown -= 1;
        return;
    }
    let robin = Thing {
        x: g.robin.x,
        y: g.robin.y,
        down: 0x20,
        across: 0x0C,
    };
    for n in 0..4 {
        let at = SECOND_GROUP + n * 11;
        if g.read(at + 4) & 0x01 == 0 {
            continue;
        }
        let it = Thing {
            x: g.read(at + 9),
            y: g.read(at + 10),
            down: 0x10,
            across: 0x0C,
        };
        if overlap(it, robin) {
            robin_hit(g);
            actions::banked_call(g, io, sound::PLAYER_BANK as u8, sound::sample);
            g.fighting.cooldown = 0x1F;
            return;
        }
    }
}
