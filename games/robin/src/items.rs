//! Items: placed in the world, restocked, picked up, carried and dropped
//! (`docs/re/robin.md`, *Items*).

use crate::actions;
use crate::game::Game;
use crate::io::{Io, Random};
use crate::print;
use crate::sound;

/// The thirty fixed places, and the eleven more, five bytes each: location
/// (a word), position, kind.
const WORLD: u16 = 0xD38A;
const MORE: u16 = 0xD420;
/// What's on the screen now: the printer's recorded messages.
const ON_SCREEN: u16 = 0xD457;
/// His inventory, and the counts kept beside it.
const INVENTORY: u16 = 0xD472;
const SWORD: u16 = 0xD47A;
const WORLD_FIVES: u16 = 0xD47F;
const WORLD_SEVENS: u16 = 0xD480;
const TAKEN: u16 = 0xD483;
/// Kept in the code that picks one up: its kind, and where its position is.
const PICKED: u16 = 0xD8D3;
const PICKED_AT: u16 = 0xD8D4;
/// A flag cleared on entering a location (*Trades and journeys*).
const TRADED: u16 = 0xDED2;
/// Where the inventory is shown, and the two messages it's shown with.
const SHOWN_AT: u16 = 0xD8D6;
const SHOWN_FIVE: u16 = 0xB404;
const SHOWN_OTHER: u16 = 0xB3CE;
/// In the dropping code, the operands that name where to drop.
const DROP_LOCATION: u16 = 0xD890;
const DROP_LOCATION_HIGH: u16 = 0xD895;
const DROP_X: u16 = 0xD89A;
const DROP_Y: u16 = 0xD8A6;

fn word(g: &Game, at: u16) -> u16 {
    u16::from_le_bytes([g.read(at), g.read(at.wrapping_add(1))])
}

/// Whether the item at `at` is at Robin's location.
fn here(g: &Game, at: u16) -> bool {
    let [lo, hi] = g.map.location.to_le_bytes();
    g.read(at) == lo && g.read(at + 1) == hi
}

/// Restocks the world, unless `zero` (the Z flag it's called with, which
/// its test reads after a load): items of the kind last taken, 5 or 7,
/// until there are as many as R picks (`0:D2DF`).
pub fn restock(g: &mut Game, random: &mut Random, zero: bool) {
    if zero {
        return;
    }
    if g.read(TAKEN) == 5 {
        restock_kind(g, random, 6, WORLD_FIVES, 5);
    } else {
        restock_kind(g, random, 8, WORLD_SEVENS, 7);
    }
}

/// `kind` placed until `count` reaches `least` plus 0 to 4, by R
/// (`0:D2E7`, `0:D30C`).
fn restock_kind(g: &mut Game, random: &mut Random, least: u8, count: u16, kind: u8) {
    g.write(TAKEN, 0);
    let b = (random.r() & 3).wrapping_add(least);
    let b = (random.r() & 1).wrapping_add(b);
    while g.read(count) < b {
        let at = free_place(g, random);
        g.write(at, kind);
        g.write(count, g.read(count).wrapping_add(1));
    }
}

/// The kind byte of a free place among the thirty: from one R picks, the
/// first free from there on, round to the start (`0:D35A`).
///
/// # Panics
///
/// If none is free, where the original goes round for ever.
pub fn free_place(g: &Game, random: &mut Random) -> u16 {
    let mut n = (random.r() & 0x1F).min(0x1D);
    for _ in 0..30 {
        let at = WORLD + 4 + u16::from(n) * 5;
        if g.read(at) == 0 {
            return at;
        }
        n = if n + 1 == 0x1E { 0 } else { n + 1 };
    }
    panic!("no free place among the thirty: the original goes round for ever");
}

/// On entering a location: the world restocked, then its items printed:
/// the first of the thirty still there, and every one of the eleven
/// (`0:D484`).
///
/// # Panics
///
/// If the eleventh of the eleven is at this location, where the original
/// returns to its caller's caller.
pub fn place(g: &mut Game, random: &mut Random, zero: bool) {
    restock(g, random, zero);
    for i in 0..27 {
        g.write(ON_SCREEN + i, 0xFF);
    }
    g.write(TRADED, 0);
    for n in 0..30 {
        let at = WORLD + n * 5;
        g.printer.cell = at;
        if here(g, at) && g.read(at + 4) != 0 {
            let (x, y, kind) = (g.read(at + 2), g.read(at + 3), g.read(at + 4));
            print::print_message(g, kind, x, y);
            break;
        }
    }
    for n in 0..11 {
        let at = MORE + n * 5;
        g.printer.cell = at;
        if here(g, at) {
            let (x, y, kind) = (g.read(at + 2), g.read(at + 3), g.read(at + 4));
            print::print_message(g, kind, x, y);
            assert!(
                n != 10,
                "the eleventh item is here: the original returns to its caller's caller"
            );
        }
    }
}

/// Picks up the first item on the screen within Robin's reach: arrows only
/// when he has none; a message for some kinds; taken off the screen and
/// out of the world's lists; and what it gives (`0:D6D0`).
pub fn pick_up(g: &mut Game, io: &mut Io) {
    for n in 0..9 {
        let at = ON_SCREEN + n * 3;
        if g.read(at) == 0xFF {
            continue;
        }
        g.printer.cell = at;
        let reach = g.robin.x.max(0x18) - 0x18;
        let (x, y) = (g.read(at + 1), g.read(at + 2));
        if reach >= x || reach.wrapping_add(0x10) < x {
            continue;
        }
        let below = g.robin.y.wrapping_add(0x0C);
        if below >= y || below.wrapping_add(0x10) < y {
            continue;
        }
        take(g, io, at);
        return;
    }
}

/// Taking the item recorded at `at` (`0:D705`).
fn take(g: &mut Game, io: &mut Io, at: u16) {
    let [lo, hi] = (at + 2).to_le_bytes();
    g.write(PICKED_AT, lo);
    g.write(PICKED_AT + 1, hi);
    let (x, y) = (g.read(at + 1), g.read(at + 2));
    let kind = g.read(at);
    g.write(PICKED, kind);
    if kind == 7 {
        if g.robin.arrows != 0 {
            return;
        }
        sound::beep(io, 0x78);
        g.robin.arrows = 10;
    }
    let message = match kind {
        0 => Some(0xB351),
        3 => Some(0xB37D),
        6 => Some(0xB3E9),
        7 => Some(0xB423),
        _ => None,
    };
    let cell = g.printer.cell;
    if let Some(message) = message {
        g.printer.replace = 0x60;
        print::print_at(g, 0x60, message);
    }
    g.printer.cell = cell;
    print::print_message(g, kind | 0x80, x, y);
    // Out of the eleven, matched by location, position and kind.
    for n in 0..11 {
        let more = MORE + n * 5;
        g.printer.cell = more;
        let at = word(g, PICKED_AT);
        if here(g, more)
            && g.read(more + 3) == g.read(at)
            && g.read(more + 2) == g.read(at - 1)
            && g.read(more + 4) == g.read(PICKED)
        {
            for i in 0..5 {
                g.write(more + i, 0xFF);
            }
            return gives(g, io);
        }
    }
    // Or out of the thirty, matched by location alone.
    for n in 0..30 {
        let place = WORLD + n * 5;
        if !here(g, place) {
            continue;
        }
        g.write(place + 4, 0);
        match g.read(PICKED) {
            5 => {
                g.write(TAKEN, 5);
                g.write(WORLD_FIVES, g.read(WORLD_FIVES).wrapping_sub(1));
            }
            7 => {
                g.write(TAKEN, 7);
                g.write(WORLD_SEVENS, g.read(WORLD_SEVENS).wrapping_sub(1));
                return;
            }
            _ => {}
        }
        return gives(g, io);
    }
}

/// What the item taken gives, by its kind (`0:D7DB`).
fn gives(g: &mut Game, io: &mut Io) {
    let kind = g.read(PICKED);
    let flag = match kind {
        0 => Some(SWORD),
        3 => Some(SWORD + 1),
        6 => Some(SWORD + 2),
        _ => None,
    };
    if let Some(flag) = flag {
        sound::beep(io, 0x64);
        g.write(flag, g.read(flag).wrapping_add(1));
    } else if kind == 1 {
        sound::beep(io, 0xC0);
        actions::energy(g);
    } else {
        sound::beep(io, 0x64);
        carry(g, io, kind);
    }
}

/// Into his inventory, first, the others pushed along and shown; the one
/// pushed off the end, if it's a kind 2, dropped (`0:D82D`).
fn carry(g: &mut Game, io: &mut Io, kind: u8) {
    let last = g.read(INVENTORY + 7);
    for i in (0..7).rev() {
        g.write(INVENTORY + i + 1, g.read(INVENTORY + i));
    }
    g.write(INVENTORY, kind);
    for n in 0..8 {
        let item = g.read(INVENTORY + n);
        if item == 0xFF {
            break;
        }
        let (x, y) = (g.read(SHOWN_AT + n * 2), g.read(SHOWN_AT + n * 2 + 1));
        let message = if item == 5 { SHOWN_FIVE } else { SHOWN_OTHER };
        g.printer.mode = 0x60;
        g.printer.replace = 0x60;
        print::print_from(g, x, y, message);
    }
    if last == 2 {
        drop(g, io);
    }
}

/// Drops a kind 2 into the first free of the eleven, at the location and
/// position its code names, and prints it, with a beep, if that's on the
/// screen (`0:D874`).
pub fn drop(g: &mut Game, io: &mut Io) {
    for n in 0..11 {
        let at = MORE + n * 5;
        if g.read(at) != 0xFF || g.read(at + 1) != 0xFF {
            continue;
        }
        g.printer.cell = at;
        let location = word(g, DROP_LOCATION);
        g.write(at, g.read(location));
        g.write(at + 1, g.read(word(g, DROP_LOCATION_HIGH)));
        let x = g.read(word(g, DROP_X)).checked_sub(0x10).unwrap_or(0x10);
        g.write(at + 2, x);
        let y = (g.read(word(g, DROP_Y)) & 0xF8).wrapping_add(0x10);
        g.write(at + 3, y);
        g.write(at + 4, 2);
        if g.map.location == word(g, location) {
            print::print_message(g, 2, x, y);
            sound::beep(io, 0x32);
        }
        return;
    }
}

/// Every item cleared and put out afresh, as a game starts (`0:D298`): the
/// thirty places emptied, what he has and the counts cleared, the fifth
/// character not out, the eleven and his inventory emptied. Then energy at
/// the first place and five more, the kinds 5 and 7 restocked, and the two
/// the game starts with.
pub fn reset(g: &mut Game, random: &mut Random) {
    for n in 0..30 {
        g.write(WORLD + 4 + n * 5, 0);
    }
    for at in SWORD..SWORD + 10 {
        g.write(at, 0);
    }
    g.write(0xBA61, 0);
    for at in MORE..MORE + 0x37 {
        g.write(at, 0xFF);
    }
    for at in INVENTORY..INVENTORY + 8 {
        g.write(at, 0xFF);
    }
    g.write(WORLD + 4, 1);
    for _ in 0..5 {
        let at = free_place(g, random);
        g.write(at, g.read(at).wrapping_add(1));
    }
    restock_kind(g, random, 6, WORLD_FIVES, 5);
    restock_kind(g, random, 8, WORLD_SEVENS, 7);
    crate::journeys::starting_items(g);
}
