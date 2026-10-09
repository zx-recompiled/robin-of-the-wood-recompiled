//! The fifth character and its companion: two who walk a route between two
//! locations together, and drop what they carry when Robin strikes the
//! first (`docs/re/robin.md`, *The fifth character*).

use crate::game::Game;
use crate::io::Io;
use crate::items;
use crate::sprites::{self, Sprites};

/// The two, as extended sprite records: the record, then at `+0x0B` the
/// way (bit 0, set for left), `+0x0C` the route's start, `+0x0E` its end,
/// and `+0x10` the location it's in.
const FIRST: u16 = 0xAAE4;
const SECOND: u16 = 0xBA4F;
/// Which of them is out (bit 1 the first, bit 0 the second) and whether
/// the first has been robbed (bit 2); the counter that paces them; the
/// eight routes; how many times he's been robbed.
const OUT: u16 = 0xBA61;
const PACE: u16 = 0xBA62;
const ROUTES: u16 = 0xBA63;
const ROBBED: u16 = 0xD482;
/// Kept in their code: the pace's mask, the walking's jump (to walk, or to
/// turn), its two sequences and its frame table.
const PACE_MASK: u16 = 0xB854;
const JUMP: u16 = 0xB953;
const SEQUENCE_RIGHT: u16 = 0xB9B7;
const SEQUENCE_LEFT: u16 = 0xB9BD;
const TABLE: u16 = 0xBA2D;
const WALK: u16 = 0xB955;
const TURN: u16 = 0xB9AE;
/// In the dropping code, the operands that name where to drop (*Items*).
const DROP_LOCATION: u16 = 0xD890;
const DROP_LOCATION_HIGH: u16 = 0xD895;
const DROP_X: u16 = 0xD89A;
const DROP_Y: u16 = 0xD8A6;

fn word(g: &Game, at: u16) -> u16 {
    u16::from_le_bytes([g.read(at), g.read(at.wrapping_add(1))])
}

fn set_word(g: &mut Game, at: u16, v: u16) {
    let [lo, hi] = v.to_le_bytes();
    g.write(at, lo);
    g.write(at.wrapping_add(1), hi);
}

/// The location one column to the left in the same row, as the code works
/// it out: the low byte's column moved round within its row.
fn left_of(location: u16) -> u16 {
    let [lo, hi] = location.to_le_bytes();
    u16::from_le_bytes([lo & 0xF0 | (lo.wrapping_sub(1) & 0x0F), hi])
}

/// Brings the two on along a route R picks, when neither is out, until he's
/// been robbed six times; then, on its pace, walks each that's out
/// (`0xB7DB`).
///
/// # Panics
///
/// If the walking's jump, kept in its code, is neither to walk nor to turn.
pub fn bring_on(g: &mut Game, sprites: &Sprites, io: &mut Io) {
    if g.read(OUT) & 3 == 0 {
        if g.read(ROBBED) == 6 {
            return;
        }
        let route = loop {
            let at = ROUTES + u16::from((io.random.r() & 7) << 2);
            if word(g, at) != word(g, SECOND + 0x0C) {
                break at;
            }
        };
        let start = word(g, route);
        for at in [SECOND + 0x0C, SECOND + 0x10, FIRST + 0x0C, FIRST + 0x10] {
            set_word(g, at, start);
        }
        let end = word(g, route + 2);
        set_word(g, SECOND + 0x0E, end);
        set_word(g, FIRST + 0x0E, end);
        set_word(g, SECOND + 2, 0xBA41);
        set_word(g, FIRST + 2, 0xAB01);
        for at in [SECOND + 0x0B, FIRST + 0x0B, SECOND + 4, FIRST + 4] {
            g.write(at, 0);
        }
        g.write(PACE, 1);
        g.write(OUT, 3);
        g.write(PACE_MASK, 3);
        g.write(SECOND + 9, 0x30);
        g.write(FIRST + 9, 0x18);
    }
    let pace = g.read(PACE).wrapping_sub(1) & g.read(PACE_MASK);
    g.write(PACE, pace);
    if pace != 0 {
        return;
    }
    if g.read(OUT) & 2 != 0 {
        first(g, sprites, io);
    }
    set_word(g, JUMP, WALK);
    let out = g.read(OUT);
    if out & 1 == 0 {
        return;
    }
    if out & 4 != 0 {
        let x = g.read(SECOND + 9);
        if word(g, SECOND + 0x10) != g.map.location && (0x14..0x64).contains(&x) {
            g.write(OUT, out & 6);
            return;
        }
    }
    walk(g, sprites, SECOND, (0xBA41, 0xBA48), 0x8C8F);
}

/// The first, while it's out: robbed if Robin has struck it, dropping what
/// it carries; once robbed, turning on the spot on Robin's row, or gone.
fn first(g: &mut Game, sprites: &Sprites, io: &mut Io) {
    let out = g.read(OUT);
    if out & 4 != 0 {
        set_word(g, JUMP, TURN);
        let mine = word(g, FIRST + 0x10).to_le_bytes();
        let his = g.map.location.to_le_bytes();
        if mine[0] & 0xF0 == his[0] & 0xF0 && mine[1] == his[1] {
            walk(g, sprites, FIRST, (0xAB24, 0xAB24), 0x8C69);
        } else {
            g.write(OUT, out & 5);
        }
        return;
    }
    if g.read(FIRST + 4) & 0x20 == 0 {
        walk(g, sprites, FIRST, (0xAB0C, 0xAB10), 0x8C69);
        return;
    }
    // Struck: robbed. What it carries is dropped where the second is.
    g.write(OUT, out | 4);
    set_word(g, DROP_X, SECOND + 9);
    set_word(g, DROP_Y, SECOND + 10);
    set_word(g, DROP_LOCATION, SECOND + 0x10);
    set_word(g, DROP_LOCATION_HIGH, SECOND + 0x11);
    items::drop(g, io);
    let robbed = g.read(ROBBED).wrapping_add(1);
    g.write(ROBBED, robbed);
    if robbed != 6 {
        g.write(ROBBED, robbed.wrapping_add(1));
        let x = g.read(SECOND + 9);
        g.write(SECOND + 9, x.wrapping_add(8));
        items::drop(g, io);
        g.write(SECOND + 9, x);
    }
    set_word(g, DROP_LOCATION, 0xC440);
    set_word(g, DROP_LOCATION_HIGH, 0xC441);
    set_word(g, DROP_X, 0xCB7F);
    set_word(g, DROP_Y, 0xCB80);
    g.write(PACE_MASK, 1);
    set_word(g, JUMP, TURN);
    walk(g, sprites, FIRST, (0xAB20, 0xAB20), 0x8C69);
}

/// One step for the one at `at`, with its sequences for each way and its
/// frame table kept in the code (`0xB947`): it walks along its route,
/// turning at either end, or, if the code's jump says so, turns where it is;
/// and it's drawn if it's on Robin's screen or about to come onto it.
fn walk(g: &mut Game, sprites: &Sprites, at: u16, (right, left): (u16, u16), table: u16) {
    set_word(g, SEQUENCE_RIGHT, right);
    set_word(g, SEQUENCE_LEFT, left);
    set_word(g, TABLE, table);
    let stepped = match word(g, JUMP) {
        WALK => step(g, at),
        TURN => Step::End,
        j => panic!("the walking's jump is to {j:#06x}, neither to walk nor to turn"),
    };
    if stepped == Step::End {
        let way = g.read(at + 0x0B) ^ 1;
        g.write(at + 0x0B, way);
        let sequence = if way & 1 != 0 {
            word(g, SEQUENCE_LEFT)
        } else {
            word(g, SEQUENCE_RIGHT)
        };
        set_word(g, at + 2, sequence);
    }
    draw(g, sprites, at, stepped == Step::LeftInto);
}

/// How a step went.
#[derive(PartialEq)]
enum Step {
    Walked,
    /// Left, out of its location into the one before.
    LeftInto,
    /// At an end of its route: it turns instead.
    End,
}

/// The step itself (`0xB955`).
fn step(g: &mut Game, at: u16) -> Step {
    let location = word(g, at + 0x10);
    let x = g.read(at + 9);
    if g.read(at + 0x0B) & 1 == 0 {
        let x = x.wrapping_add(1);
        if x < 0x70 {
            g.write(at + 9, x);
            return Step::Walked;
        }
        if location == word(g, at + 0x0E) {
            return Step::End;
        }
        g.write(at + 9, 0);
        let [lo, _] = location.to_le_bytes();
        g.write(at + 0x10, lo & 0xF0 | (lo.wrapping_add(1) & 0x0F));
        Step::Walked
    } else {
        let x = x.wrapping_sub(1);
        if x & 0x80 == 0 {
            g.write(at + 9, x);
            return Step::Walked;
        }
        if location == word(g, at + 0x0C) {
            return Step::End;
        }
        g.write(at + 9, 0x6F);
        g.write(at + 0x10, left_of(location).to_le_bytes()[0]);
        Step::LeftInto
    }
}

/// Drawn if it's in Robin's location; or, from the next location to the
/// right, drawn coming on, or erased just as it leaves (`0xB9C7`,
/// `0xB9F4`). `moved_left` is a step left into another location, after
/// which it's only ever erased from there.
fn draw(g: &mut Game, sprites: &Sprites, at: u16, moved_left: bool) {
    let location = word(g, at + 0x10);
    let flags = at + 4;
    let show = if location == g.map.location {
        true
    } else if left_of(location) != g.map.location {
        return;
    } else if moved_left {
        false
    } else {
        let x = g.read(at + 9);
        if x > 0x10 {
            return;
        }
        if x < 0x10 {
            g.write(at + 9, x + 0x70);
            true
        } else {
            false
        }
    };
    let f = g.read(flags);
    g.write(flags, if show { f | 0x05 } else { f & !0x04 });
    g.sprites.table = word(g, TABLE);
    sprites::animate(g, sprites, at);
    let x = g.read(at + 9);
    if x >= 0x70 {
        g.write(at + 9, x - 0x70);
    }
}
