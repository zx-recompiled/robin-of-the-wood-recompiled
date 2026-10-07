//! The characters who walk the forest's rows (`docs/re/robin.md`, *The four
//! on each row*).

use crate::game::Game;

/// The four row lists, 12 bytes each.
const ROW_LISTS: u16 = 0x8B02;
/// Row 12 of the attribute buffer, across the play area: a screen's floor.
const FLOOR_ROW: u16 = 0xE982;
/// Each floor is 28 bytes: left, current, right.
const FLOOR: usize = 28;

/// The list for the row of `location` (its low byte): one of four, by the
/// row modulo 4 (`0:C2F2`).
#[must_use]
pub fn row_list(location: u8) -> u16 {
    ROW_LISTS + u16::from((location >> 4) & 3) * 12
}

/// Saves the current screen's floor from its attribute buffer (`0:DD49`).
pub fn save_floor(g: &mut Game) {
    for i in 0..FLOOR {
        g.characters.floors[FLOOR + i] = g.read(FLOOR_ROW + i as u16);
    }
}

/// The current floor becomes the right neighbour's (`0:DD55`).
pub fn current_to_right(g: &mut Game) {
    g.characters.floors.copy_within(FLOOR..2 * FLOOR, 2 * FLOOR);
}

/// The current floor becomes the left neighbour's (`0:DD5A`).
pub fn current_to_left(g: &mut Game) {
    g.characters.floors.copy_within(FLOOR..2 * FLOOR, 0);
}

/// Forgets the left neighbour's floor (`0:DD66`).
pub fn clear_left(g: &mut Game) {
    g.characters.floors[..FLOOR].fill(0);
}

/// Forgets the right neighbour's floor (`0:DD6B`).
pub fn clear_right(g: &mut Game) {
    g.characters.floors[2 * FLOOR..].fill(0);
}

/// The floors on entering a location by going up or down: no neighbours
/// known (`0:DD79`).
pub fn floors_vertically(g: &mut Game) {
    clear_left(g);
    clear_right(g);
    save_floor(g);
}

/// The floors on entering by going left: the old screen is to the right
/// (`0:DD83`).
pub fn floors_going_left(g: &mut Game) {
    current_to_right(g);
    clear_left(g);
    save_floor(g);
}

/// The floors on entering by going right: the old screen is to the left
/// (`0:DD8D`).
pub fn floors_going_right(g: &mut Game) {
    current_to_left(g);
    clear_right(g);
    save_floor(g);
}
