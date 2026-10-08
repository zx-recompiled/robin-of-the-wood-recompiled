//! Trades and journeys: what the special locations do with what Robin
//! carries (`docs/re/robin.md`, *Trades and journeys*).

use crate::actions;
use crate::assets::Assets;
use crate::characters;
use crate::game::Game;
use crate::io::Io;
use crate::items;
use crate::map;
use crate::print;
use crate::screen;
use crate::sound;

/// His inventory, and the counts beside it (*Items*).
const INVENTORY: u16 = 0xD472;
const ROBBED: u16 = 0xD482;
const THIRD: u16 = 0xD47C;
const PIECES: u16 = 0xD47E;
/// The trade's location, and whether it's been made on this visit.
const TRADE_AT: u16 = 0xD28F;
const TRADED: u16 = 0xDED2;
/// The colour kept in the trade's code, for the cell it flashes.
const TRADE_COLOUR: u16 = 0xDEB7;
/// A byte for each location: bit 7 moves the doorway and the trade's cell
/// three columns over.
const LOCATION_FLAGS: u16 = 0x7AC2;
/// The hook the main loop calls each frame, kept in its call's operand;
/// nothing (`0:DA03`, a return) or the doorway's sparkle (`0:D97D`).
const HOOK: u16 = 0xC3CB;
const NO_HOOK: u16 = 0xDA03;
const SPARKLE: u16 = 0xD97D;
/// The doorways: nine locations, and the one Robin is at.
const DOORWAYS: u16 = 0xDAB9;
const DOORWAY: u16 = 0xDAD7;
/// The sparkle's state: frames left, the location's flag, the last random
/// value, where he's to go (1 location `0x9C`, 2 `0xCC`), and the controls
/// he walked in with.
const FRAMES: u16 = 0xDAD3;
const FLAG: u16 = 0xDAD4;
const SPARKLED: u16 = 0xDAD5;
const GOING: u16 = 0xDAD6;
const WALKED_IN: u16 = 0xDAD9;
/// The sparkle's own code: an instruction that's `LD A,R` or `LD R,A`, and
/// one that's `XOR (HL)` or `OR (HL)`.
const READ_R: u16 = 0xD997;
const COMBINE: u16 = 0xD9AA;
const LD_A_R: u8 = 0x5F;
const LD_R_A: u8 = 0x4F;
const XOR_HL: u8 = 0xAE;
const OR_HL: u8 = 0xB6;
/// The doorway's attributes and pixels, as drawn.
const DOORWAY_ATTRS: u16 = 0xA2F4;
const DOORWAY_PIXELS: u16 = 0xA204;
/// The random value `0:CD5F` keeps.
const RANDOM: u16 = 0xD26A;
/// The inventory's display (*Items*): where, and the messages.
const SHOWN_AT: u16 = 0xD8D6;

fn word(g: &Game, at: u16) -> u16 {
    u16::from_le_bytes([g.read(at), g.read(at.wrapping_add(1))])
}

fn set_word(g: &mut Game, at: u16, v: u16) {
    let [lo, hi] = v.to_le_bytes();
    g.write(at, lo);
    g.write(at.wrapping_add(1), hi);
}

/// Whether this location's flag moves things three columns over.
fn moved(g: &Game) -> bool {
    g.read(LOCATION_FLAGS.wrapping_add(g.map.location)) & 0x80 != 0
}

/// Takes the item at `at` out of the inventory, `left` slots from the end,
/// the rest moving up; returns the slot before, to look at again
/// (`0:DA6E`).
pub fn remove(g: &mut Game, at: u16, left: u8) -> u16 {
    let mut to = at;
    for from in at + 1..at + u16::from(left) {
        g.write(to, g.read(from));
        to += 1;
    }
    g.write(to, 0xFF);
    at.wrapping_sub(1)
}

/// Shows the inventory, all eight slots, kind 5 and 2 by their messages
/// and the rest as blanks (`0:DA84`).
pub fn show_inventory(g: &mut Game) {
    for n in 0..8 {
        let item = g.read(INVENTORY + n);
        let (x, y) = (g.read(SHOWN_AT + n * 2), g.read(SHOWN_AT + n * 2 + 1));
        let message = match item {
            5 => 0xB404,
            2 => 0xB3CE,
            _ => 0xDADC,
        };
        g.printer.mode = 0x60;
        g.printer.replace = 0x60;
        print::print_from(g, x, y, message);
    }
}

/// How many of `kind` he carries, taken out up to `most` of them.
fn take(g: &mut Game, kind: u8, most: u8) -> u8 {
    let mut taken = 0;
    let mut at = INVENTORY;
    let mut left = 8u8;
    while left > 0 {
        if g.read(at) == kind {
            taken += 1;
            at = remove(g, at, left);
            if taken == most {
                return taken;
            }
        }
        at += 1;
        left -= 1;
    }
    taken
}

/// The trade, at the first special location, once a visit: with three
/// kind-2 items, and R at `0x13`, they're taken for the sword, then the
/// bow and ten arrows, then up to three pieces; and a cell flashes
/// (`0:DDEF`).
pub fn trade(g: &mut Game, io: &mut Io) {
    if g.map.location != word(g, TRADE_AT) || g.read(PIECES) == 3 || g.read(TRADED) != 0 {
        return;
    }
    if io.random.r() != 0x13 {
        return;
    }
    let carried = (0..8).filter(|&n| g.read(INVENTORY + n) == 2).count();
    if carried < 3 {
        return;
    }
    take(g, 2, 3);
    show_inventory(g);
    g.write(ROBBED, g.read(ROBBED).wrapping_sub(3));
    let mut message = Some(0xB351);
    if g.robin.sword == 0 {
        g.robin.sword = 1;
    } else if g.robin.bow == 0 {
        g.robin.bow = 1;
        g.robin.arrows = g.robin.arrows.wrapping_add(10);
        g.printer.replace = 0x60;
        print::print_at(g, 0x60, 0xB423);
        message = Some(0xB37D);
    } else {
        let x = match g.read(PIECES) {
            0 => 0x08,
            1 => 0x0C,
            _ => 0x10,
        };
        g.printer.mode = 0x60;
        print::print_from(g, x, 0xA0, 0xB412);
        g.write(PIECES, g.read(PIECES).wrapping_add(1));
        message = None;
    }
    if let Some(message) = message {
        print::print_at(g, 0x60, message);
    }
    g.write(TRADE_COLOUR, 0x12);
    let cell = if moved(g) { 0x5894 } else { 0x588D };
    g.write(cell, 0x12);
    if g.read(TRADED) == 0 {
        g.write(TRADED, 1);
        g.write(TRADE_COLOUR, 0x04);
        g.write(cell, 0x04);
    }
}

/// On entering a location: if it's one of the nine doorways and he
/// carries anything, the doorway's drawn, its sparkle hooked in, and he's
/// walked in, with a sound; then the sparkle's first frame, which its code
/// runs on into (`0:D8EC`).
pub fn doorway(g: &mut Game, io: &mut Io) {
    g.write(GOING, 0);
    if g.read(INVENTORY) == 0xFF {
        return;
    }
    let found = (0..9).any(|n| {
        let place = word(g, DOORWAYS + n * 2);
        set_word(g, DOORWAY, place);
        place == g.map.location
    });
    if !found {
        return;
    }
    let flag = g.read(LOCATION_FLAGS.wrapping_add(g.map.location));
    g.write(FLAG, flag);
    let mut to: u16 = if flag & 0x80 != 0 { 0x588D + 3 } else { 0x588D };
    let mut from = DOORWAY_ATTRS;
    for _ in 0..5 {
        for _ in 0..3 {
            g.write(to, g.read(from));
            from += 1;
            to += 1;
        }
        to += 0x1D;
    }
    set_word(g, HOOK, SPARKLE);
    g.write(READ_R, LD_A_R);
    g.write(COMBINE, XOR_HL);
    g.write(FRAMES, 0x28);
    g.robin.y = 0x50;
    let way = match g.robin.direction & 3 {
        0 | 3 => 1,
        w => w,
    };
    set_word(g, WALKED_IN, u16::from_be_bytes([way, 0x1E]));
    g.robin.override_controls = [0x1E, way];
    actions::banked_call(g, io, sound::MUSIC_BANK, sound::departure);
    sparkle(g, io);
}

/// `0:CD5F`'s random value: the byte at R × `0x101`, R and the last
/// value, mixed.
fn random(g: &mut Game, io: &mut Io) -> u8 {
    let r = io.random.r();
    let v = g.read(u16::from_be_bytes([r, r])) ^ r ^ g.read(RANDOM);
    g.write(RANDOM, v);
    v
}

/// The doorway's sparkle, each frame, from the main loop's hook: its
/// pixels mixed with random ones, in two passes, then he's sent on, or
/// back (`0:D97D`).
pub fn sparkle(g: &mut Game, io: &mut Io) {
    let mut row: u16 = if g.read(FLAG) & 0x80 != 0 {
        0x408D + 3
    } else {
        0x408D
    };
    let mut from = DOORWAY_PIXELS;
    for _ in 0..0x28 {
        for k in 0..3 {
            let (from, to) = (from + k, row.wrapping_add(k));
            let a = g.read(from);
            if a == 0 {
                continue;
            }
            let v = if g.read(READ_R) == LD_A_R {
                io.random.r()
            } else {
                a
            };
            g.write(SPARKLED, v);
            let v = if g.read(READ_R) == LD_R_A {
                v
            } else {
                random(g, io)
            };
            let v = v & a;
            let screen = g.read(to);
            let combined = if g.read(COMBINE) == XOR_HL {
                v ^ screen
            } else {
                v | screen
            };
            g.write(to, combined);
        }
        from += 3;
        let [lo, hi] = row.to_le_bytes();
        let hi = hi.wrapping_add(1);
        row = if hi & 7 == 0 {
            match lo.checked_add(0x20) {
                Some(lo) => u16::from_be_bytes([hi.wrapping_sub(8), lo]),
                None => u16::from_be_bytes([hi, lo.wrapping_add(0x20)]),
            }
        } else {
            u16::from_be_bytes([hi, lo])
        };
    }
    let frames = g.read(FRAMES).wrapping_sub(1);
    g.write(FRAMES, frames);
    if frames != 0 {
        return;
    }
    if g.read(COMBINE) == XOR_HL {
        g.write(COMBINE, OR_HL);
        actions::banked_call(g, io, sound::PLAYER_BANK as u8, sound::departure_sample);
        actions::banked_call(g, io, sound::MUSIC_BANK, sound::arrival_tune);
        g.write(FRAMES, 0x1E);
        g.robin.override_controls[1] = 0;
        return;
    }
    if g.read(READ_R) == LD_A_R {
        g.write(READ_R, LD_R_A);
        g.write(FRAMES, 1);
        return;
    }
    send_on(g);
}

/// The sparkle's end (`0:DA04`): the hook taken out; with three kind 5s, he's
/// sent to `0xCC`; with two, his health's restored; with one, nothing;
/// with none, a kind 2 is taken back, or he's sent to `0x9C`. Then he
/// walks back out, unless he's going.
fn send_on(g: &mut Game) {
    set_word(g, HOOK, NO_HOOK);
    match take(g, 5, 3) {
        3 => g.write(GOING, 2),
        2 => {
            g.fighting.health = 0x0F;
            actions::panel_colours(g);
        }
        1 => {}
        _ => {
            if take(g, 2, 1) == 1 {
                g.write(ROBBED, g.read(ROBBED).wrapping_sub(1));
            } else {
                g.write(GOING, 1);
            }
        }
    }
    show_inventory(g);
    g.robin.override_controls = if g.read(GOING) != 0 {
        [0, 0]
    } else {
        let [way, lo] = word(g, WALKED_IN).to_be_bytes();
        [lo, way ^ 3]
    };
}

/// The main loop's hook, then a journey if one's due: to location `0x9C`
/// or `0xCC`, entered with a wipe; it returns to the main loop's start
/// rather than to its caller then (`0:C3CA`). Whether it went.
///
/// # Panics
///
/// If the hook is neither nothing nor the sparkle.
pub fn hook(g: &mut Game, assets: &Assets, io: &mut Io) -> bool {
    match word(g, HOOK) {
        NO_HOOK => {}
        SPARKLE => sparkle(g, io),
        h => panic!("the main loop's hook is {h:#06x}, neither nothing nor the sparkle"),
    }
    let going = g.read(GOING);
    if going == 0 {
        return false;
    }
    g.write(GOING, 0);
    g.map.location = if going == 1 { 0x009C } else { 0x00CC };
    screen::clear_play_area(g, 0);
    map::draw_location(g, assets.map());
    map::draw_special(g, assets.map());
    recolour(g);
    wipe(g);
    items::place(g, &mut io.random, true);
    screen::copy_attrs(g);
    screen::copy_pixels(g);
    let flags = 0xCB7A;
    g.write(flags, g.read(flags) & !0x02);
    characters::enter(g, assets.sprites(), 4, &mut io.random);
    screen::clear_changed(g);
    true
}

/// The 26 locations of the first row whose attribute buffer is recoloured
/// on drawing: paper to white ink, plain ink to bright cyan, except white
/// (`0:C306`).
pub fn recolour(g: &mut Game) {
    let [lo, hi] = g.map.location.to_le_bytes();
    if hi != 0 || !(0..0x1A).any(|n| g.read(0xC448 + n) == lo) {
        return;
    }
    for at in 0xE800..=0xEA00u16 {
        let a = g.read(at);
        if a == 0 {
            continue;
        }
        if a & 0x38 != 0 {
            g.write(at, 0x07);
        } else if a & 0x07 != 0x07 {
            g.write(at, 0x45);
        }
    }
}

/// A wipe onto the new screen: the changed-cell map's diagonals marked and
/// copied to the screen one by one, then the third item's return
/// (`0:C40F`).
pub fn wipe(g: &mut Game) {
    for a in 0..0x3Fu16 {
        let n = a.min(0x11) + 1;
        let mut at = 0xE500 + a;
        for _ in 0..n {
            g.write(at, g.read(at) | 1);
            at = at.wrapping_add(0x1F);
        }
        screen::flush(g);
    }
    third_returned(g);
}

/// If he has the third item, it's taken back, with its message, and the
/// two the game starts with are put out again (`0:C3AE`).
pub fn third_returned(g: &mut Game) {
    if g.read(THIRD) == 0 {
        return;
    }
    g.write(THIRD, 0);
    g.printer.replace = 0;
    starting_items(g);
    print::print_at(g, 0x60, 0xB3E9);
}

/// The two items the game starts with, put among the eleven (`0:D331`).
pub fn starting_items(g: &mut Game) {
    for (n, b) in [0x3D, 0x00, 0x30, 0x60, 0x06, 0xBB, 0x00, 0x58, 0x58, 0x06]
        .into_iter()
        .enumerate()
    {
        g.write(0xD420 + n as u16, b);
    }
}
