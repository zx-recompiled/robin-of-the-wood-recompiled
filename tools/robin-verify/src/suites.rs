//! The routines of the original that have been rewritten, and how each
//! rewrite takes the original's registers (`docs/re/robin.md`).

use robin::{characters, map, movement, print, screen, sprites};

use crate::capture::{Reg, Regs, Routine};

/// The tables the start-up code builds, and must never be rewritten after.
pub const TABLES: [(&str, u16, u16); 2] = [
    ("the mirror table", 0xFD00, 0xFDFF),
    ("the row table", 0xFE00, 0xFF7F),
];

pub fn all() -> Vec<Routine> {
    vec![
        Routine {
            name: "build the mirror table (0:CEC8)",
            bank: Some(0),
            entry: 0xCEC8,
            code: (0xCEC8, 0xCEDD),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                screen::build_mirror(g);
                r
            },
        },
        Routine {
            name: "build the row table (0:CEDE)",
            bank: Some(0),
            entry: 0xCEDE,
            code: (0xCEDE, 0xCEFD),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                screen::build_rows(g);
                r
            },
        },
        Routine {
            name: "clear the screen (0:CEFE)",
            bank: Some(0),
            entry: 0xCEFE,
            code: (0xCEFE, 0xCF0B),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                screen::clear_screen(g);
                r
            },
        },
        Routine {
            name: "fill the attributes (0:CF0C)",
            bank: Some(0),
            entry: 0xCF0C,
            code: (0xCF0C, 0xCF18),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                screen::fill_attrs(g, r.get(Reg::A));
                r
            },
        },
        Routine {
            name: "clear the play area (0:CF19)",
            bank: Some(0),
            entry: 0xCF19,
            code: (0xCF19, 0xCF32),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                screen::clear_play_area(g, r.get(Reg::A));
                r
            },
        },
        Routine {
            name: "clear the changed-cell map (0:CF33)",
            bank: Some(0),
            entry: 0xCF33,
            code: (0xCF33, 0xCF40),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                screen::clear_changed(g);
                r
            },
        },
        Routine {
            name: "clear the lower panel (0:CF41)",
            bank: Some(0),
            entry: 0xCF41,
            code: (0xCF41, 0xCF5E),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                screen::clear_lower_panel(g);
                r
            },
        },
        Routine {
            name: "flush the changed cells (0:C754)",
            bank: Some(0),
            entry: 0xC754,
            code: (0xC754, 0xC7AE),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                screen::flush(g);
                r
            },
        },
        Routine {
            name: "copy the play area's pixels (0:C6FE)",
            bank: Some(0),
            entry: 0xC6FE,
            code: (0xC6FE, 0xC753),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                screen::copy_pixels(g);
                r
            },
        },
        Routine {
            name: "copy the play area's attributes (0:C086)",
            bank: Some(0),
            entry: 0xC086,
            code: (0xC086, 0xC0A3),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                screen::copy_attrs(g);
                r
            },
        },
        Routine {
            name: "print at a position in memory (0:D4F6)",
            bank: Some(0),
            entry: 0xD4F6,
            code: PRINTER,
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                print::print_at(
                    g,
                    r.get(Reg::A),
                    u16::from_be_bytes([r.get(Reg::H), r.get(Reg::L)]),
                );
                r
            },
        },
        Routine {
            name: "print at a position in DE (0:D4FC)",
            bank: Some(0),
            entry: 0xD4FC,
            code: PRINTER,
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                print::print_from(
                    g,
                    r.get(Reg::D),
                    r.get(Reg::E),
                    u16::from_be_bytes([r.get(Reg::H), r.get(Reg::L)]),
                );
                r
            },
        },
        Routine {
            name: "print a stock message (0:D50E)",
            bank: Some(0),
            entry: 0xD50E,
            code: PRINTER,
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                print::print_message(g, r.get(Reg::A), r.get(Reg::D), r.get(Reg::E));
                r
            },
        },
        Routine {
            name: "print replacing (0:CEBB)",
            bank: Some(0),
            entry: 0xCEBB,
            code: (0xCEBB, 0xCEC7),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                print::print_replacing(g, u16::from_be_bytes([r.get(Reg::H), r.get(Reg::L)]));
                r
            },
        },
        Routine {
            name: "step to the next location (0:C127)",
            bank: Some(0),
            entry: 0xC127,
            code: (0xC127, 0xC16D),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                g.map.location = map::step(g.map.location, r.get(Reg::A));
                r
            },
        },
        Routine {
            name: "find a record (0:C0B9)",
            bank: Some(0),
            entry: 0xC0B9,
            code: (0xC0B9, 0xC0CD),
            outputs: &[Reg::H, Reg::L],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, mut r, _| {
                let at = map::find_record(g, g.map.table, g.map.record);
                r.set(Reg::H, (at >> 8) as u8);
                r.set(Reg::L, at as u8);
                r
            },
        },
        Routine {
            name: "draw a block (0xBFC9, 0:C002)",
            // Its code starts in bank 2 and runs on into bank 0.
            bank: Some(0),
            entry: 0xBFC9,
            code: (0xBFC9, 0xC055),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, a, r, _| {
                map::draw_block(g, a.map(), r.get(Reg::A), r.get(Reg::B), r.get(Reg::C));
                r
            },
        },
        Routine {
            name: "mirror a block in place (0:C0CE)",
            bank: Some(0),
            entry: 0xC0CE,
            code: (0xC0CE, 0xC11F),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                map::mirror_block(g, u16::from_be_bytes([r.get(Reg::D), r.get(Reg::E)]));
                r
            },
        },
        Routine {
            name: "draw a list of blocks (0xBFAA)",
            bank: Some(0),
            entry: 0xBFAA,
            code: (0xBFAA, 0xBFC8),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, a, r, _| {
                map::draw_list(
                    g,
                    a.map(),
                    u16::from_be_bytes([r.get(Reg::H), r.get(Reg::L)]),
                );
                r
            },
        },
        Routine {
            name: "draw a record (0xBFA7)",
            bank: Some(0),
            entry: 0xBFA7,
            code: (0xBFA7, 0xBFA9),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, a, r, _| {
                map::draw_record(g, a.map());
                r
            },
        },
        Routine {
            name: "draw a location (0xBF6A)",
            bank: Some(0),
            entry: 0xBF6A,
            code: (0xBF6A, 0xBFA6),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, a, r, _| {
                map::draw_location(g, a.map());
                r
            },
        },
        Routine {
            name: "draw a special location's extras (0:C056)",
            bank: Some(0),
            entry: 0xC056,
            code: (0xC056, 0xC085),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, a, r, _| {
                map::draw_special(g, a.map());
                r
            },
        },
        Routine {
            name: "mirror a frame in place (0:C7AF)",
            bank: Some(0),
            entry: 0xC7AF,
            code: (0xC7AF, 0xC7EE),
            outputs: &[],
            exits: &[],
            preserves: &[Reg::A, Reg::F, Reg::B, Reg::C, Reg::H, Reg::L],
            rewrite: |g, _, r, _| {
                sprites::mirror_frame(
                    g,
                    u16::from_be_bytes([r.get(Reg::H), r.get(Reg::L)]),
                    r.get(Reg::A),
                );
                r
            },
        },
        Routine {
            name: "draw a frame (0:C5CE, with 0:C47D and 0:C645)",
            bank: Some(0),
            entry: 0xC5CE,
            code: (0xC5CE, 0xC687),
            outputs: &[],
            exits: &[],
            preserves: &[Reg::H, Reg::L],
            rewrite: |g, a, r, _| {
                sprites::draw_frame(g, a.sprites(), r.get(Reg::A), r.get(Reg::C), r.get(Reg::B));
                r
            },
        },
        Routine {
            name: "mark a frame's cells (0:C688)",
            bank: Some(0),
            entry: 0xC688,
            code: (0xC688, 0xC6FD),
            outputs: &[],
            exits: &[],
            preserves: &[
                Reg::A,
                Reg::F,
                Reg::B,
                Reg::C,
                Reg::D,
                Reg::E,
                Reg::H,
                Reg::L,
            ],
            rewrite: |g, _, r, _| {
                sprites::mark_cells(
                    g,
                    u16::from_be_bytes([r.get(Reg::H), r.get(Reg::L)]),
                    r.get(Reg::C),
                    r.get(Reg::B),
                );
                r
            },
        },
        Routine {
            name: "animate and redraw a sprite (0:C7EF)",
            bank: Some(0),
            entry: 0xC7EF,
            code: (0xC7EF, 0xC851),
            outputs: &[],
            // Its callers go on using the record (0:DBDA clears a flag
            // through IX), and it leaves IX as it was.
            exits: &[],
            preserves: &[Reg::Ixh, Reg::Ixl],
            rewrite: |g, a, r, _| {
                sprites::animate(
                    g,
                    a.sprites(),
                    u16::from_be_bytes([r.get(Reg::Ixh), r.get(Reg::Ixl)]),
                );
                r
            },
        },
        Routine {
            name: "read the controls (0:D0C6)",
            bank: Some(0),
            entry: 0xD0C6,
            code: (0xD0C6, 0xD0E7),
            outputs: &[Reg::A, Reg::E],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, mut r, i| {
                let e = movement::read_controls(g, &i.controls);
                r.set(Reg::A, e);
                r.set(Reg::E, e);
                r
            },
        },
        Routine {
            name: "read the Kempston joystick (0:D07F)",
            bank: Some(0),
            entry: 0xD07F,
            code: (0xD07F, 0xD087),
            outputs: &[Reg::E],
            exits: &[],
            preserves: &[],
            rewrite: |_, _, mut r, i| {
                r.set(Reg::E, movement::read_kempston(&i.controls));
                r
            },
        },
        Routine {
            name: "read the Sinclair joystick (0:D088)",
            bank: Some(0),
            entry: 0xD088,
            code: (0xD088, 0xD0A0),
            outputs: &[Reg::E],
            exits: &[],
            preserves: &[],
            rewrite: |_, _, mut r, i| {
                r.set(Reg::E, movement::read_sinclair(&i.controls));
                r
            },
        },
        Routine {
            name: "read the redefined keys (0:D06E)",
            bank: Some(0),
            entry: 0xD06E,
            code: (0xD053, 0xD07E),
            outputs: &[Reg::E],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, mut r, i| {
                r.set(Reg::E, movement::read_keys(g, &i.controls));
                r
            },
        },
        Routine {
            name: "walk Robin a step (0:C852)",
            bank: Some(0),
            entry: 0xC852,
            code: (0xC852, 0xC8DE),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, i| {
                movement::walk(g, &i.controls);
                r
            },
        },
        wall("a wall to the right (0:DC6C)", 0xDC6C, (0xDC6C, 0xDC7E)),
        wall("a wall to the left (0:DC5B)", 0xDC5B, (0xDC5B, 0xDC6B)),
        wall("a wall above (0:DC7F)", 0xDC7F, (0xDC7F, 0xDC8A)),
        wall("a wall below (0:DC8B)", 0xDC8B, (0xDC8B, 0xDC9A)),
        Routine {
            name: "find Robin's cell (0:DCC2)",
            bank: Some(0),
            entry: 0xDCC2,
            code: (0xDCC2, 0xDCDB),
            outputs: &[Reg::H, Reg::L],
            exits: &[],
            preserves: &[],
            rewrite: |_, _, mut r, _| {
                r.set_pair(Reg::H, Reg::L, movement::cell(r.get(Reg::L), r.get(Reg::H)));
                r
            },
        },
        Routine {
            name: "leave by the screen's edge (0xBE9B, in the main loop)",
            bank: Some(0),
            entry: 0xBE9B,
            code: (0xBE9B, 0xBECD),
            outputs: &[Reg::E],
            // Off the edge it goes on to take the step; otherwise the main
            // loop starts again.
            exits: &[0xBECE, 0xBE62],
            preserves: &[],
            rewrite: |g, _, mut r, _| {
                if let Some(direction) = movement::leave_by_edge(g) {
                    r.set(Reg::E, direction);
                }
                r
            },
        },
        floor("save the floor (0:DD49)", 0xDD49, (0xDD49, 0xDD54)),
        floor("the floor to the right (0:DD55)", 0xDD55, (0xDD55, 0xDD59)),
        floor("the floor to the left (0:DD5A)", 0xDD5A, (0xDD5A, 0xDD65)),
        floor("forget the left floor (0:DD66)", 0xDD66, (0xDD66, 0xDD6A)),
        floor("forget the right floor (0:DD6B)", 0xDD6B, (0xDD6B, 0xDD78)),
        floor(
            "the floors going up or down (0:DD79)",
            0xDD79,
            (0xDD79, 0xDD82),
        ),
        floor("the floors going left (0:DD83)", 0xDD83, (0xDD83, 0xDD8C)),
        floor("the floors going right (0:DD8D)", 0xDD8D, (0xDD8D, 0xDD96)),
        Routine {
            name: "find a row's characters (0:C2F2)",
            bank: Some(0),
            entry: 0xC2F2,
            code: (0xC2F2, 0xC305),
            outputs: &[Reg::H, Reg::L],
            exits: &[],
            preserves: &[],
            rewrite: |_, _, mut r, _| {
                r.set_pair(Reg::H, Reg::L, characters::row_list(r.get(Reg::A)));
                r
            },
        },
        Routine {
            name: "put the characters on their floors (0:DCDC)",
            bank: Some(0),
            entry: 0xDCDC,
            code: (0xDCDC, 0xDD3C),
            // Its only caller sets every register it uses next.
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                characters::place(g);
                r
            },
        },
        Routine {
            name: "the characters on entering a location (0:C16E)",
            bank: Some(0),
            entry: 0xC16E,
            code: (0xC16E, 0xC2F1),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, a, r, inputs| {
                characters::enter(g, a.sprites(), r.get(Reg::A), &mut inputs.random);
                r
            },
        },
        Routine {
            name: "move and draw one of the four (A8D6)",
            bank: None,
            entry: 0xA8D6,
            code: (0xA8D6, 0xAAB7),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, a, r, inputs| {
                characters::move_one(g, a.sprites(), &mut inputs.random);
                r
            },
        },
        Routine {
            name: "anything in a character's way (0:DD3D)",
            bank: Some(0),
            entry: 0xDD3D,
            code: (0xDD3D, 0xDD48),
            outputs: &[Reg::A, Reg::F, Reg::B],
            exits: &[],
            preserves: &[Reg::C, Reg::D, Reg::E, Reg::H, Reg::L, Reg::Ixh, Reg::Ixl],
            rewrite: |g, _, r, _| {
                let at = u16::from_be_bytes([r.get(Reg::H), r.get(Reg::L)]);
                let found = characters::in_the_way(g, at);
                let mut r = answer(r, found.map(|(_, b)| b));
                r.set(Reg::B, found.map_or(0, |(n, _)| 3 - n));
                r
            },
        },
    ]
}

/// A floor routine: no registers in or out.
fn floor(name: &'static str, entry: u16, code: (u16, u16)) -> Routine {
    let rewrite: fn(&mut robin::Game, &robin::assets::Assets, Regs, &mut robin::io::Io) -> Regs =
        match entry {
            0xDD49 => |g, _, r, _| {
                characters::save_floor(g);
                r
            },
            0xDD55 => |g, _, r, _| {
                characters::current_to_right(g);
                r
            },
            0xDD5A => |g, _, r, _| {
                characters::current_to_left(g);
                r
            },
            0xDD66 => |g, _, r, _| {
                characters::clear_left(g);
                r
            },
            0xDD6B => |g, _, r, _| {
                characters::clear_right(g);
                r
            },
            0xDD79 => |g, _, r, _| {
                characters::floors_vertically(g);
                r
            },
            0xDD83 => |g, _, r, _| {
                characters::floors_going_left(g);
                r
            },
            _ => |g, _, r, _| {
                characters::floors_going_right(g);
                r
            },
        };
    Routine {
        name,
        bank: Some(0),
        entry,
        code,
        outputs: &[],
        exits: &[],
        preserves: &[],
        rewrite,
    }
}

/// A wall test: it takes Robin's position in HL, and answers in A and the
/// flags, as `OR (HL)` leaves them on a wall and `XOR A` does otherwise.
fn wall(name: &'static str, entry: u16, code: (u16, u16)) -> Routine {
    let rewrite: fn(&mut robin::Game, &robin::assets::Assets, Regs, &mut robin::io::Io) -> Regs =
        match entry {
            0xDC6C => |g, _, r, _| answer(r, movement::wall_right(g, r.get(Reg::L), r.get(Reg::H))),
            0xDC5B => |g, _, r, _| answer(r, movement::wall_left(g, r.get(Reg::L), r.get(Reg::H))),
            0xDC7F => |g, _, r, _| answer(r, movement::wall_up(g, r.get(Reg::L), r.get(Reg::H))),
            _ => |g, _, r, _| answer(r, movement::wall_down(g, r.get(Reg::L), r.get(Reg::H))),
        };
    Routine {
        name,
        bank: Some(0),
        entry,
        code,
        outputs: &[Reg::A, Reg::F],
        exits: &[],
        preserves: &[Reg::D, Reg::E, Reg::H, Reg::L],
        rewrite,
    }
}

/// A and the flags as the original's wall tests leave them: the wall's
/// attribute after `OR (HL)`, or 0 after `XOR A`.
fn answer(mut r: Regs, wall: Option<u8>) -> Regs {
    let a = wall.unwrap_or(0);
    let parity = if a.count_ones().is_multiple_of(2) {
        0x04
    } else {
        0
    };
    let zero = if a == 0 { 0x40 } else { 0 };
    r.set(Reg::A, a);
    r.set(Reg::F, (a & 0xA8) | zero | parity);
    r
}

/// The text printer's code, shared by its three ways in.
const PRINTER: (u16, u16) = (0xD4F6, 0xD6AA);

/// The routines allowed to write the start-up tables: their builders.
const BUILDERS: [u16; 2] = [0xCEC8, 0xCEDE];

/// The code of the tables' builders: only an instruction in it may write
/// the tables.
pub fn table_builders(routines: &[Routine]) -> Vec<(u16, u16)> {
    routines
        .iter()
        .filter(|r| BUILDERS.contains(&r.entry))
        .map(|r| r.code)
        .collect()
}
