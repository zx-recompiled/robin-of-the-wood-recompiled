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
