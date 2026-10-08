//! Robin's actions: his states, attacking, being knocked down, and what
//! they set going (`docs/re/robin.md`, *Robin's actions*).

use crate::game::Game;
use crate::io::Io;
use crate::print;
use crate::sound;

/// The message printed when his energy is low, and the energy's figure,
/// whose digit is at `ENERGY_DIGIT`.
const LOW_ENERGY: u16 = 0xB3DB;
const ENERGY: u16 = 0xD8E6;
const ENERGY_DIGIT: u16 = 0xD8E8;
/// The printer's mode for both.
const PRINTED: u8 = 0x60;

/// The lower panel's attributes, and the colour they're reset to.
const PANEL: u16 = 0x5A40;
const PANEL_COLOUR: u16 = 0xBE47;

/// Robin's slot in the objects in flight (#40): column and way, then row.
const ARROW: u16 = 0xBE3F;

/// After a knock-down: if his energy is below 9 (or `0xFF`), prints the
/// low-energy message, adds 1 to it, and prints its figure (`0:D7F7`).
pub fn energy(g: &mut Game) {
    let e = g.robin.energy;
    if e != 0xFF && e >= 9 {
        return;
    }
    g.printer.replace = PRINTED;
    print::print_at(g, PRINTED, LOW_ENERGY);
    g.robin.energy = g.robin.energy.wrapping_add(1);
    g.write(ENERGY_DIGIT, g.robin.energy.wrapping_add(b'0'));
    g.printer.replace = PRINTED;
    print::print_at(g, PRINTED, ENERGY);
}

/// Resets the lower panel's 64 attribute cells to the colour at `0xBE47`:
/// its bits 1 to 3 as the ink, bit 0 as bright (`0xBE0F`).
pub fn panel_colours(g: &mut Game) {
    let c = g.read(PANEL_COLOUR);
    let colour = (c >> 1) & 7 | (c & 1) << 6;
    for at in PANEL..PANEL + 64 {
        g.write(at, colour);
    }
}

/// On the 7th frame of an attack with the bow, fires an arrow, if his slot
/// in the objects in flight is free and he's clear of the play area's
/// edges: a twang, and his column, row and way into the slot (`0xBB43`).
pub fn launch_arrow(g: &mut Game, io: &mut Io) {
    if g.robin.attack != 7 || g.read(ARROW) != 0 {
        return;
    }
    sound::twang(io, 0x40);
    let row = (g.robin.y >> 3).wrapping_add(1);
    let left = g.robin.state & 1;
    let column = (g.robin.x >> 2).wrapping_sub(2 * left);
    if !(2..0x1E).contains(&column) {
        return;
    }
    g.write(ARROW, column | left << 7);
    g.write(ARROW + 1, row | 0x80);
}

/// Where each state's handler is found, two bytes a state.
const HANDLERS: u16 = 0xCC3C;
/// The four characters' sprite records, 11 bytes apart (*The four on each
/// row*): the bow isn't used with one of them near.
const CHARACTERS: u16 = 0xAAB8;
/// Printed when the last arrow goes.
const NO_ARROWS: u16 = 0xB423;
/// Robin's sprite record, and the frame table he's drawn from.
const ROBIN: u16 = 0xCB76;
const ROBIN_FRAMES: u16 = 0x8C25;
/// The bank the sample played when he's knocked down is in: its entry,
/// `4:C012`, is [`sound::sample`].
const KNOCKED_DOWN_SOUND: u8 = sound::PLAYER_BANK as u8;

/// His controls' bits (*The controls*).
const RIGHT: u8 = 0x01;
const LEFT: u8 = 0x02;
const DOWN: u8 = 0x04;
const UP: u8 = 0x08;
const FIRE: u8 = 0x10;

/// What a handler leaves: his state and sequence as they were, or new ones.
enum Next {
    Stay,
    Set(u8, u16),
}

/// The way he faces, as a direction's bits: right in state bit 0 clear.
fn facing(g: &Game) -> u8 {
    if g.robin.state & 1 == 0 { RIGHT } else { LEFT }
}

/// Robin's update, from the main loop (`0:C59A`): on every call, or every
/// third while he's attacking or down, he walks, acts, and is redrawn.
pub fn update(g: &mut Game, sprites: &crate::sprites::Sprites, io: &mut Io) {
    g.robin.update_counter = g.robin.update_counter.wrapping_sub(1);
    if g.robin.update_counter != 0 {
        return;
    }
    g.robin.update_counter = if g.robin.attack | g.robin.down != 0 {
        3
    } else {
        1
    };
    crate::movement::walk(g, &io.controls);
    act(g, io);
    let flags = ROBIN + 4;
    g.write(flags, g.read(flags) | 0x05);
    g.sprites.table = ROBIN_FRAMES;
    crate::sprites::animate(g, sprites, ROBIN);
}

/// His actions (`0:C8DF`): a knock-down taking effect, or fire starting an
/// attack, then his state's handler.
///
/// # Panics
///
/// If his state's handler isn't one of the sixteen the table names.
pub fn act(g: &mut Game, io: &mut Io) {
    let dir = g.robin.direction;
    if g.robin.down != 0 {
        if g.robin.down == 0x6E {
            knock_down(g, io);
        }
    } else if g.robin.attack == 0
        && dir & FIRE != 0
        && g.robin.x & 3 == 0
        && g.robin.y & 7 == 0
        && dir & (UP | DOWN) == 0
    {
        match dir & (RIGHT | LEFT) {
            0 => start_attack(g, facing(g), false),
            way => start_attack(g, way, true),
        }
    }
    let state = g.robin.state;
    let at = HANDLERS.wrapping_add(u16::from(state) * 2);
    let handler = u16::from_le_bytes([g.read(at), g.read(at.wrapping_add(1))]);
    let next = match handler {
        0xCA63 => standing(dir),
        0xCAF8 => walking(g, dir, RIGHT),
        0xCB19 => walking(g, dir, LEFT),
        0xCB41 => upright(g, dir, DOWN),
        0xCB4C => upright(g, dir, UP),
        0xC9F3 | 0xC9F7 | 0xCA09 | 0xCA0D | 0xCA11 | 0xCA15 => attacking(g),
        0xC9FB | 0xCA02 => {
            launch_arrow(g, io);
            attacking(g)
        }
        0xCAD0 => lying(g, 0, 0xCC07),
        0xCAC1 => lying(g, 1, 0xCC1C),
        _ => panic!("Robin's state {state}: its handler at {handler:#06x} is not rewritten"),
    };
    if let Next::Set(state, sequence) = next {
        g.robin.state = state;
        g.robin.sequence = sequence;
    }
}

/// States 0 and 1: a direction starts him walking.
fn standing(dir: u8) -> Next {
    match dir {
        d if d & 0x0F == 0 => Next::Stay,
        d if d & RIGHT != 0 => Next::Set(2, 0xCBF6),
        d if d & LEFT != 0 => Next::Set(3, 0xCC0B),
        d if d & DOWN != 0 => Next::Set(5, 0xCC27),
        _ => Next::Set(4, 0xCC20),
    }
}

/// States 2 and 3, walking `way`: no direction stands him; up or down
/// alone turns him that way; the other way turns him round.
fn walking(g: &mut Game, dir: u8, way: u8) -> Next {
    match dir {
        d if d & 0x0F == 0 => stand(g, way == LEFT),
        d if d & (RIGHT | LEFT) == 0 => match d {
            d if d & DOWN != 0 => Next::Set(5, 0xCC27),
            d if d & UP != 0 => Next::Set(4, 0xCC20),
            _ => Next::Stay,
        },
        d if d & (RIGHT | LEFT) == way => Next::Stay,
        _ if way == RIGHT => Next::Set(3, 0xCBEE),
        _ => Next::Set(2, 0xCBF2),
    }
}

/// States 4 and 5, walking up or down: no direction returns him to the way
/// he last stood; `back` turns him round; right or left walks him that way.
fn upright(g: &Game, dir: u8, back: u8) -> Next {
    match dir {
        d if d & 0x0F == 0 => Next::Set(g.robin.standing, g.robin.standing_sequence),
        d if d & back != 0 => {
            if back == DOWN {
                Next::Set(5, 0xCC27)
            } else {
                Next::Set(4, 0xCC20)
            }
        }
        d if d & RIGHT != 0 => Next::Set(2, 0xCBF8),
        d if d & LEFT != 0 => Next::Set(3, 0xCC0D),
        _ => Next::Stay,
    }
}

/// Stands him, facing left or right, and keeps that as the way he last
/// stood (`0:CB0D`, `0:CB2E`).
fn stand(g: &mut Game, left: bool) -> Next {
    let (state, sequence) = if left { (1, 0xCC1A) } else { (0, 0xCC05) };
    g.robin.standing = state;
    g.robin.standing_sequence = sequence;
    Next::Set(state, sequence)
}

/// Starts an attack `way` (a direction's bit), with a direction `held` or
/// not: the bow, the sword or his fists, by what he carries (`0:C93C`,
/// `0:C944`).
fn start_attack(g: &mut Game, way: u8, held: bool) {
    let left = u16::from(way & RIGHT == 0);
    let d = left as u8;
    g.robin.attack = 0x10;
    let (state, sequence) = if g.robin.bow != 0 && g.robin.arrows != 0 && !character_near(g) {
        g.robin.arrows = g.robin.arrows.wrapping_sub(1);
        if g.robin.arrows == 0 {
            print::print_at(g, PRINTED, NO_ARROWS);
        }
        (8 + d, 0xCBC3 + 11 * left)
    } else if g.robin.sword != 0 {
        if g.robin.direction & DOWN != 0 {
            (0x0C + d, 0xCB97 + 11 * left)
        } else {
            (6 + d, 0xCBAD + 11 * left)
        }
    } else {
        (0x0A + d, 0xCBD9 + 11 * left)
    };
    g.robin.sequence = if held { sequence - 1 } else { sequence };
    g.robin.state = state;
}

/// Whether one of the four characters on the screen is within `0x32` of
/// him, for the bow.
fn character_near(g: &Game) -> bool {
    (0..4u16).any(|n| {
        let record = CHARACTERS + n * 11;
        let flags = g.read(record + 4);
        let x = g.read(record + 9);
        flags & 0x01 != 0 && flags & 0x20 == 0 && x.abs_diff(g.robin.x) < 0x32
    })
}

/// States 6 to 13: the attack's counter. When it runs out he attacks
/// again if fire is held, or walks the way held, or stands (`0:CA19`).
fn attacking(g: &mut Game) -> Next {
    g.robin.attack = g.robin.attack.wrapping_sub(1);
    if g.robin.attack != 0 {
        return Next::Stay;
    }
    let dir = g.robin.direction;
    if dir & FIRE != 0 {
        match dir & (RIGHT | LEFT) {
            0 => start_attack(g, facing(g), false),
            way => start_attack(g, way, true),
        }
        return Next::Stay;
    }
    match dir & (RIGHT | LEFT) {
        0 => stand(g, g.robin.state & 1 != 0),
        way if way & RIGHT != 0 => Next::Set(2, 0xCBF8),
        _ => Next::Set(3, 0xCC0D),
    }
}

/// Knocked down: the attack ends, a sample plays, his energy drops, his
/// controls do nothing, and he lies down facing the way he was (`0:CA95`).
fn knock_down(g: &mut Game, io: &mut Io) {
    g.robin.attack = 0;
    banked_call(g, io, KNOCKED_DOWN_SOUND, sound::sample);
    g.robin.energy = g.robin.energy.wrapping_sub(2);
    energy(g);
    g.robin.override_controls = [0x1E, 0x00];
    let (state, sequence) = if g.robin.state & 1 != 0 {
        (0x0F, 0xCC2E)
    } else {
        (0x0E, 0xCC35)
    };
    g.robin.state = state;
    g.robin.sequence = sequence;
}

/// States 14 and 15: he lies there until his counter runs out, then gets
/// up: his controls back, and the lower panel's colours reset.
fn lying(g: &mut Game, state: u8, sequence: u16) -> Next {
    g.robin.down = g.robin.down.wrapping_sub(1);
    if g.robin.down != 0 {
        return Next::Stay;
    }
    g.robin.override_controls = [0, 0];
    g.write(PANEL_COLOUR, 0x0F);
    panel_colours(g);
    Next::Set(state, sequence)
}

/// A call into another bank through the trampoline at `0x5B8A`: the bank
/// paged in for `f`, and the one before paged back after.
fn banked_call(g: &mut Game, io: &mut Io, bank: u8, f: fn(&mut Game, &mut Io)) {
    g.banked.saved = g.banked.current;
    g.banked.current = bank;
    io.out(0x7FFD, bank | 0x10);
    f(g, io);
    g.banked.current = g.banked.saved;
    io.out(0x7FFD, g.banked.saved | 0x10);
}
