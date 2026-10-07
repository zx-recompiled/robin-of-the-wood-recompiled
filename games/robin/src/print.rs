//! The text printer (`docs/re/robin.md`, *The text printer*): a string of
//! characters, then a string of their attributes, drawn to the screen or into
//! the play area.

use crate::game::Game;

/// The tape's font: 8 bytes a character, character `c` at `FONT + 8 * c`.
const FONT: u16 = 0xAAEF;
/// The addresses of the stock messages, two bytes each.
const MESSAGES: u16 = 0xD6AB;
/// The list of recorded messages: mode, then position, three bytes each,
/// `0xFF` where free.
const RECORDED: u16 = 0xD457;

/// Mode bit 5: straight to the screen, not into the play area.
const TO_SCREEN: u8 = 0x20;
/// Mode bit 6: neither marks the cells changed nor records the message.
const QUIET: u8 = 0x40;
/// Mode bit 7.
const BIT7: u8 = 0x80;

/// Prints the string whose position is the two bytes at `at`, horizontal
/// first; its characters follow them. Attributes go to the screen. (`0:D4F6`)
pub fn print_at(g: &mut Game, mode: u8, at: u16) {
    g.printer.mode = mode;
    let x = g.read(at);
    let y = g.read(at.wrapping_add(1));
    print_from(g, x, y, at.wrapping_add(1));
}

/// Prints the string after `before` at `x`, `y`, in the mode already set.
/// Attributes go to the screen. (`0:D4FC`)
pub fn print_from(g: &mut Game, x: u8, y: u8, before: u16) {
    g.printer.column_offset = 0;
    g.printer.attr_flag = 0;
    g.printer.attr_page = 0x58;
    print(g, x, y, before);
}

/// Prints stock message `mode & 0x1F` at `x`, `y`. Its attributes go to the
/// attribute buffer, two columns over and flagged with bit 7, except
/// message 14's. (`0:D50E`)
pub fn print_message(g: &mut Game, mode: u8, x: u8, y: u8) {
    g.printer.mode = mode;
    let entry = MESSAGES.wrapping_add(u16::from(mode & 0x1F) * 2);
    let before = u16::from_le_bytes([g.read(entry), g.read(entry.wrapping_add(1))]);
    g.printer.column_offset = 2;
    g.printer.attr_flag = if mode & 0x1F == 0x0E { 0 } else { 0x80 };
    g.printer.attr_page = 0xE8;
    print(g, x, y, before);
}

/// Prints the string at `at` as [`print_at`] does, replacing what is there
/// rather than combining with it, always in mode `0x60`: the caller's A is
/// overwritten before it is passed on. (`0:CEBB`)
pub fn print_replacing(g: &mut Game, at: u16) {
    g.printer.replace = 0x60;
    print_at(g, 0x60, at);
}

/// Steps `p` down one pixel row: within a character on the screen, or one
/// 32-byte line in the back buffer.
fn next_pixel_row(p: u16, to_screen: bool) -> u16 {
    if to_screen {
        p.wrapping_add(0x100)
    } else {
        down_a_line(p, 0x100)
    }
}

/// Adds `0x20` to the low byte of `p`; a carry adds `carry` to the whole.
fn down_a_line(p: u16, carry: u16) -> u16 {
    let (low, over) = (p as u8).overflowing_add(0x20);
    let p = (p & 0xFF00) | u16::from(low);
    if over { p.wrapping_add(carry) } else { p }
}

fn print(g: &mut Game, x: u8, y: u8, before: u16) {
    let mode = g.printer.mode;
    let to_screen = mode & TO_SCREEN != 0;
    let row = if to_screen {
        let entry = 0xFE00u16.wrapping_add(u16::from(y) * 2);
        u16::from_le_bytes([g.read(entry), g.read(entry.wrapping_add(1))])
    } else {
        (u16::from(y) * 32).wrapping_add(0xEB02)
    };
    let mut p = row.wrapping_add(u16::from(x >> 2));
    let mut s = before;
    g.printer.mirrored = 0;
    let mut on_line: u8 = 0;
    loop {
        s = s.wrapping_add(1);
        match g.read(s) {
            0 => break,
            1 => {
                p = p.wrapping_sub(u16::from(on_line));
                p = if to_screen {
                    down_a_line(p, 0x800)
                } else {
                    p.wrapping_add(0x100)
                };
                on_line = 0;
            }
            2 => g.printer.mirrored = 2,
            3 => g.printer.mirrored = 0,
            c @ 0x20.. => {
                let glyph = FONT.wrapping_add(u16::from(c) * 8);
                let mut q = p;
                for r in 0..8 {
                    let mut b = g.read(glyph.wrapping_add(r));
                    if g.printer.mirrored != 0 {
                        g.printer.mirror_lookup = b;
                        b = g.read(0xFD00 | u16::from(b));
                    }
                    if g.printer.replace == 0 {
                        b ^= g.read(q);
                    }
                    g.write(q, b);
                    q = next_pixel_row(q, to_screen);
                }
                on_line = on_line.wrapping_add(1);
                p = p.wrapping_add(1);
            }
            _ => {}
        }
    }
    g.printer.replace = 0;

    if mode & QUIET == 0 {
        if mode & BIT7 != 0 {
            let cell = g.printer.cell;
            g.write(cell, 0xFF);
        } else {
            let mut slot = RECORDED;
            while g.read(slot) != 0xFF {
                slot = slot.wrapping_add(3);
            }
            g.write(slot, mode);
            g.write(slot.wrapping_add(1), x);
            g.write(slot.wrapping_add(2), y);
        }
    }

    // The attributes, one a character, on whole character rows.
    let column = (x >> 2).wrapping_add(g.printer.column_offset);
    let row = u16::from(y & 0xF8) * 4;
    let mut a = (u16::from(g.printer.attr_page) << 8 | u16::from(column)).wrapping_add(row);
    g.printer.cell = row.wrapping_add(u16::from(column));
    let mut on_line: u8 = 0;
    loop {
        s = s.wrapping_add(1);
        let attr = g.read(s);
        if attr == 0 {
            if !to_screen && mode & BIT7 != 0 {
                crate::characters::save_floor(g);
            }
            return;
        }
        if attr & 0x80 != 0 {
            a = a.wrapping_sub(u16::from(on_line)).wrapping_add(32);
            g.printer.cell = g
                .printer
                .cell
                .wrapping_sub(u16::from(on_line))
                .wrapping_add(32);
            on_line = 0;
            continue;
        }
        let v = if mode & BIT7 != 0 {
            0
        } else {
            attr | g.printer.attr_flag
        };
        g.write(a, v);
        if mode & QUIET == 0 {
            let cell = g.printer.cell;
            g.write(0xE500u16.wrapping_add(cell), attr);
            g.printer.cell = cell.wrapping_add(1);
        }
        on_line = on_line.wrapping_add(1);
        a = a.wrapping_add(1);
    }
}
