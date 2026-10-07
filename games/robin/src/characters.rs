//! The characters who walk the forest's rows (`docs/re/robin.md`, *The four
//! on each row*).

use crate::game::Game;
use crate::inputs::Random;
use crate::sprites::{self, Sprites};

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

/// Where the floors start, less 4: a character's position, in units of 4
/// pixels, indexes its floor from here (`0:DCDC`).
const FLOORS_LESS_4: u16 = 0xDD97;

/// Whether anything is in the way of a character at floor address `at`: the
/// first of the three bytes from there that isn't 0, with its index, or
/// `None` where all three are clear (`0:DD3D`).
#[must_use]
pub fn in_the_way(g: &Game, at: u16) -> Option<(u8, u8)> {
    (0..3u8)
        .map(|n| (n, g.read(at.wrapping_add(u16::from(n)))))
        .find(|&(_, b)| b != 0)
}

/// Puts each character of the current row that is in Robin's column, or
/// one beside it, on its floor: its position rounded down to 4 pixels, and
/// if something is in the way there, the first clear place from the left
/// (or `0x64` if there's none) (`0:DCDC`).
pub fn place(g: &mut Game) {
    let column = g.map.location as u8;
    let mut at = g.characters.row;
    for _ in 0..4 {
        let theirs = g.read(at);
        if let Some(n) = (0..3u8).find(|&n| column.wrapping_add(n).wrapping_sub(1) & 0x0F == theirs)
        {
            let floor = FLOORS_LESS_4 + u16::from(n) * FLOOR as u16;
            let position = g.read(at.wrapping_add(1)) & 0xFC;
            g.write(at.wrapping_add(1), position);
            if in_the_way(g, floor + u16::from(position >> 2)).is_some() {
                let clear = (0..25u8)
                    .find(|&k| in_the_way(g, floor + u16::from(k)).is_none())
                    .unwrap_or(25);
                g.write(at.wrapping_add(1), clear * 4);
            }
        }
        at = at.wrapping_add(3);
    }
}

/// The frame table the four are drawn from (*Sprites*).
const FRAMES: u16 = 0x8C69;
/// Their sprite records, 11 bytes apart.
const RECORDS: u16 = 0xAAB8;
/// The objects in flight: two bytes for each character that fires (#40).
const SHOTS: u16 = 0xBE41;

/// The state byte's bits: the way it faces (set for right), a copy of it
/// (the way it faced, just after turning), and two flags (#42).
const RIGHT: u8 = 0x80;
const FACED: u8 = 0x40;
const STILL: u8 = 0x10;
const TO_BE_STILL: u8 = 0x20;

/// Their animation sequences (*Sprites*).
pub const WALK_LEFT: u16 = 0xAAF6;
pub const WALK_RIGHT: u16 = 0xAB01;
const TURN_RIGHT: u16 = 0xAB0C;
const TURN_LEFT: u16 = 0xAB10;
const FIRE_RIGHT: u16 = 0xAB14;
const FIRE_LEFT: u16 = 0xAB1A;
const STOPPING: u16 = 0xAB20;
pub const STOPPED: u16 = 0xAB24;

/// `base + size × (n − 1)`, counted as the original counts it: `size` added
/// `n` times (0 meaning 256), then taken away with the last addition's carry.
fn nth(base: u16, size: u16, n: u8) -> u16 {
    let mut at = base;
    let mut carry = false;
    for _ in 0..crate::times(n) {
        (at, carry) = at.overflowing_add(size);
    }
    at.wrapping_sub(size).wrapping_sub(u16::from(carry))
}

/// Moves the next of the four, one a call, and draws it (`0xA8D6`). One
/// within a column of Robin's, and not still, steps along its floor: it
/// turns where the way ahead is blocked, and otherwise turns to face Robin
/// one time in 16. One on Robin's screen, or about to walk onto it from the
/// right, is drawn, and may fire if it faces him from at least `0x30`
/// pixels away; one that isn't is erased.
pub fn move_one(g: &mut Game, sprites: &Sprites, random: &mut Random) {
    g.characters.cycle = g.characters.cycle.wrapping_sub(1);
    if g.characters.cycle == 0 {
        g.characters.cycle = 4;
    }
    g.sprites.table = FRAMES;
    let at = nth(g.characters.row, 3, g.characters.cycle);
    if g.read(at.wrapping_add(2)) & STILL == 0 {
        let column = g.map.location as u8 & 0x0F;
        let near = g.read(at).wrapping_sub(column).wrapping_add(1) & 0x0F;
        if near < 3 {
            step(g, at, near, random);
        }
    }
    let record = nth(RECORDS, 11, g.characters.cycle);
    draw(g, sprites, at, record, random);
}

/// One step along the floor, for a character `near` columns right of the
/// one left of Robin's.
fn step(g: &mut Game, at: u16, near: u8, random: &mut Random) {
    let floor = nth(FLOORS_LESS_4, FLOOR as u16, near + 1);
    let position = g.read(at.wrapping_add(1));
    let width = if position & 3 == 0 { 3 } else { 4 };
    let ahead = floor + u16::from(position >> 2);
    let state = g.read(at.wrapping_add(2));
    let turned = |way: u8| (state & 0x3F) | way | (state >> 1 & FACED);
    if (0..width).any(|k| g.read(ahead + k) != 0) {
        g.write(at.wrapping_add(2), turned(!state & RIGHT));
    } else {
        let robin = FLOORS_LESS_4 + FLOOR as u16 + u16::from(g.robin.x >> 2);
        let toward = if robin >= ahead { RIGHT } else { 0 };
        if random.r() & 0x0F == 0 {
            g.write(at.wrapping_add(2), turned(toward));
        }
    }
    let carry = if g.read(at.wrapping_add(2)) & RIGHT == 0 {
        let position = position.wrapping_sub(1);
        if position & 0x80 == 0 {
            g.write(at.wrapping_add(1), position);
            return;
        }
        g.write(at.wrapping_add(1), position.wrapping_add(0x70));
        0xFF
    } else {
        let position = position.wrapping_add(1);
        g.write(at.wrapping_add(1), position);
        if position < 0x70 {
            return;
        }
        g.write(at.wrapping_add(1), position - 0x70);
        1
    };
    g.write(at, g.read(at).wrapping_add(carry) & 0x0F);
}

/// Sets a sprite record's animation sequence.
fn sequence(g: &mut Game, record: u16, sequence: u16) {
    let [lo, hi] = sequence.to_le_bytes();
    g.write(record.wrapping_add(2), lo);
    g.write(record.wrapping_add(3), hi);
}

/// Draws the character at `at` through its sprite record, or erases it if
/// it's off Robin's screen.
fn draw(g: &mut Game, sprites: &Sprites, at: u16, record: u16, random: &mut Random) {
    let column = g.map.location as u8 & 0x0F;
    let theirs = g.read(at) & 0x0F;
    let position = g.read(at.wrapping_add(1));
    let flags = record.wrapping_add(4);
    let x = if theirs == column {
        position
    } else if theirs == column.wrapping_add(1) & 0x0F && position < 0x10 {
        position + 0x70
    } else {
        g.write(flags, g.read(flags) & !0x04);
        sprites::animate(g, sprites, record);
        g.write(flags, g.read(flags) & !0x01);
        let state = g.read(at.wrapping_add(2));
        if state & TO_BE_STILL == 0 {
            let walk = if state & RIGHT == 0 {
                WALK_LEFT
            } else {
                WALK_RIGHT
            };
            sequence(g, record, walk);
        }
        return;
    };
    g.write(record.wrapping_add(9), x);
    let state = g.read(at.wrapping_add(2));
    if state & STILL == 0 {
        if state & TO_BE_STILL != 0 {
            g.write(at.wrapping_add(2), state & !TO_BE_STILL | STILL);
            sequence(g, record, STOPPING);
        } else {
            if (state ^ state << 1) & 0x80 != 0 {
                let (state, turn) = if state & RIGHT == 0 {
                    (state & !FACED, TURN_LEFT)
                } else {
                    (state | FACED, TURN_RIGHT)
                };
                g.write(at.wrapping_add(2), state);
                sequence(g, record, turn);
            }
            if let Some(fire) = fire(g, at, record, random) {
                sequence(g, record, fire);
            }
        }
    }
    g.write(flags, g.read(flags) | 0x05);
    sprites::animate(g, sprites, record);
}

/// Fires, if the character faces Robin from at least `0x30` pixels away, it
/// always does or R says so, and its slot of the objects in flight is free:
/// the sequence to show it firing.
fn fire(g: &mut Game, at: u16, record: u16, random: &mut Random) -> Option<u16> {
    let x = g.read(record.wrapping_add(9));
    let (apart, left) = g.robin.x.overflowing_sub(x);
    let distance = if left { apart.wrapping_neg() } else { apart };
    let right = g.read(at.wrapping_add(2)) & RIGHT != 0;
    if distance < 0x30 || right == left {
        return None;
    }
    if g.characters.fire_always == 0 && random.r() & 1 == 0 {
        return None;
    }
    let slot = match g.characters.cycle & 3 {
        n @ (1 | 2) => n,
        _ => 0,
    };
    let shot = SHOTS + 2 * u16::from(slot);
    if g.read(shot) != 0 {
        return None;
    }
    let y = g.read(record.wrapping_add(10));
    g.write(shot, x >> 2 | if right { 0 } else { 0x80 });
    g.write(shot + 1, ((y >> 3) + 2) | 0x80);
    Some(if right { FIRE_RIGHT } else { FIRE_LEFT })
}

/// The location of the scripted scene, the record of its character, and the
/// frame table it's drawn from (#42).
const SCENE: u16 = 0xD295;
const SCENE_RECORD: u16 = 0xB7BD;
const SCENE_FRAMES: u16 = 0x8C8F;
/// The second group of four, for the locations from 256 up: their sprite
/// records, the list they're taken from, and the current row's (#42).
const SECOND_RECORDS: u16 = 0xDBF1;
const SECOND_LISTS: u16 = 0xDC2B;
const SECOND_ROW: u16 = 0xC446;
const SECOND_WALK_LEFT: u16 = 0xDC1D;
const SECOND_WALK_RIGHT: u16 = 0xDC24;
/// Robin's sprite record, and the frame table he's drawn from.
const ROBIN: u16 = 0xCB76;
const ROBIN_FRAMES: u16 = 0x8C25;

/// The scripted scene's start (#42): two of the four put either side of
/// Robin's screen, facing in, Robin walked in by the controls' override, and
/// another character's record (`0xB7BD`) set up and drawn.
fn scene(g: &mut Game, sprites: &Sprites, direction: u8) {
    let column = g.map.location as u8;
    let at = g.characters.row;
    for (n, b) in [
        column.wrapping_sub(1) & 0x0F,
        0x6C,
        RIGHT,
        column.wrapping_add(1) & 0x0F,
        0x18,
        0,
    ]
    .into_iter()
    .enumerate()
    {
        g.write(at.wrapping_add(n as u16), b);
    }
    g.sprites.table = SCENE_FRAMES;
    let (seq, x, way) = if direction & 0x01 == 0 {
        (0xB7D2, 4, 2)
    } else {
        (0xB7D6, 0, 1)
    };
    g.robin.override_controls = [0x1E, way];
    g.write(SCENE_RECORD, 0);
    sequence(g, SCENE_RECORD, seq);
    g.write(SCENE_RECORD + 4, 5);
    let x = if g.read(location_flags(g.map.location)) & 0x80 == 0 {
        0x38
    } else {
        0x48
    } + x;
    g.write(SCENE_RECORD + 9, x);
    g.write(SCENE_RECORD + 10, 0x38);
    sprites::animate(g, sprites, SCENE_RECORD);
    g.robin.y = 0x50;
}

/// A byte for each location, bit 7 of which places the scene's character
/// (#42).
fn location_flags(location: u16) -> u16 {
    location.wrapping_add(0x7AC2)
}

/// The four on entering a location (`0:C16E`), Robin having come in the way
/// `direction` says (bit 3 or 2 for up or down, else bit 1 for left). The
/// columns of the four two rows up are shuffled with R. The current row's
/// list is taken, the floors brought up to date, the four put on them,
/// given their sequences and moved once each. At the scripted scene's
/// location, its start is set up first. Then everything else that
/// moves is reset, and Robin is drawn.
///
pub fn enter(g: &mut Game, sprites: &Sprites, direction: u8, random: &mut Random) {
    let location = g.map.location;
    let mut at = row_list((location as u8).wrapping_sub(0x20));
    for _ in 0..4 {
        g.write(at, (random.r() ^ g.read(at)) & 0x0F);
        let state = at.wrapping_add(2);
        g.write(state, g.read(state) & !(TO_BE_STILL | STILL));
        at = at.wrapping_add(3);
    }
    g.characters.row = row_list(location as u8);
    g.robin.override_controls = [0, 0];
    if location == u16::from_le_bytes([g.read(SCENE), g.read(SCENE + 1)]) {
        scene(g, sprites, direction);
    }
    if direction & 0x0C != 0 {
        floors_vertically(g);
    } else if direction & 0x02 != 0 {
        floors_going_left(g);
    } else {
        floors_going_right(g);
    }
    place(g);
    let mut at = g.characters.row.wrapping_add(2);
    for n in 0..4 {
        let record = RECORDS + n * 11;
        let state = g.read(at);
        let (flags, seq, state) = if state & STILL != 0 {
            (0x20, STOPPED, state)
        } else if state & RIGHT != 0 {
            (0, WALK_RIGHT, state | FACED)
        } else {
            (0, WALK_LEFT, state & !FACED)
        };
        g.write(at, state);
        g.write(record + 4, flags);
        sequence(g, record, seq);
        at = at.wrapping_add(3);
    }
    for _ in 0..4 {
        move_one(g, sprites, random);
    }
    if location >= 0x100 {
        let list = SECOND_LISTS + u16::from(location as u8 >> 4) * 12;
        let [lo, hi] = list.to_le_bytes();
        g.write(SECOND_ROW, lo);
        g.write(SECOND_ROW + 1, hi);
        let mut at = list.wrapping_add(2);
        for n in 0..4 {
            let record = SECOND_RECORDS + n * 11;
            let state = g.read(at);
            let (seq, state) = if state & RIGHT != 0 {
                (SECOND_WALK_RIGHT, state | FACED)
            } else {
                (SECOND_WALK_LEFT, state & !FACED)
            };
            g.write(at, state);
            g.write(record + 4, 0);
            sequence(g, record, seq);
            at = at.wrapping_add(3);
        }
    }
    // The objects in flight (#40), the wanderer (#39), and three more
    // things' state (#42), reset.
    for a in 0xBE3F..=0xBE46 {
        g.write(a, 0);
    }
    g.write(0xBB1F, 0);
    g.write(0xBA84, 1);
    g.write(0xBA62, 1);
    g.write(0xBA53, 0);
    g.write(0xAAE8, 0);
    g.sprites.table = ROBIN_FRAMES;
    g.write(ROBIN, 0);
    g.write(ROBIN + 4, g.read(ROBIN + 4) | 0x05);
    sprites::animate(g, sprites, ROBIN);
}
