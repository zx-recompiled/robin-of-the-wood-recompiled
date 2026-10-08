//! The fighting: the objects in flight, hits both ways, Robin's health
//! (`docs/re/robin.md`, *Fighting*).

use crate::actions;
use crate::game::Game;

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
