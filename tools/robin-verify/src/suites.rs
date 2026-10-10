//! The routines of the original that have been rewritten, and how each
//! rewrite takes the original's registers (`docs/re/robin.md`).

use robin::{
    actions, characters, fifth, fighting, items, journeys, main_loop, map, movement, new_game,
    print, scene, screen, sound, sprites, wanderer,
};

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
            name: "move and draw one of the second group (0:DAE4)",
            bank: Some(0),
            entry: 0xDAE4,
            code: (0xDAE4, 0xDBF0),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, a, r, io| {
                characters::move_second(g, a.sprites(), &mut io.random);
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
            name: "BREAK held (0:C433)",
            bank: Some(0),
            entry: 0xC433,
            code: (0xC433, 0xC43E),
            // It answers in the carry, from an RRA, which leaves S, Z and
            // P/V as they were.
            outputs: &[Reg::A, Reg::F],
            exits: &[],
            preserves: &[],
            rewrite: |_, _, mut r, io| {
                let first = io.input(0x7FFE);
                // RRA rotates the carry in: the caller's, then 0, as the first
                // left it.
                let a = if first & 1 != 0 {
                    first >> 1 | (r.get(Reg::F) & 1) << 7
                } else {
                    io.input(0xFEFE) >> 1
                };
                let held = main_loop::break_held(io);
                let carry = u8::from(!held);
                r.set(Reg::A, a);
                r.set(Reg::F, r.get(Reg::F) & 0xC4 | a & 0x28 | carry);
                r
            },
        },
        Routine {
            name: "the game over (0xBF3F)",
            bank: None,
            entry: 0xBF3F,
            code: (0xBF3F, 0xBF5C),
            // It ends at the wait for a key that starts the game again,
            // which is the main loop's, or returns.
            outputs: &[],
            exits: &[0xBE95, 0xBF5D],
            preserves: &[],
            rewrite: |g, _, r, io| {
                main_loop::game_over(g, io);
                r
            },
        },
        Routine {
            name: "the menu (0xAB28)",
            bank: None,
            entry: 0xAB28,
            code: (0xAB28, 0xAB6A),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                new_game::menu(g);
                r
            },
        },
        Routine {
            name: "the menu shown, with its sample and tune (0:CCAB)",
            bank: Some(0),
            entry: 0xCCAB,
            code: (0xCCAB, 0xCCB9),
            outputs: &[],
            exits: &[0xCCBA],
            preserves: &[],
            rewrite: |g, _, r, io| {
                new_game::show_menu(g, io);
                r
            },
        },
        Routine {
            name: "a poll of the menu's keys (0:CCBB)",
            bank: Some(0),
            entry: 0xCCBB,
            code: (0xCCBB, 0xCCCC),
            // Its answer is in which way it leaves, so in A and F (#61).
            outputs: &[Reg::A, Reg::F],
            // Polled again, the game started, or a pick to make.
            exits: &[0xCCBB, 0xCCD2, 0xCCCD],
            preserves: &[],
            rewrite: |_, _, r, io| poll_regs(io, r, new_game::menu_keys(io)),
        },
        Routine {
            name: "the menu's pick (0:CFB2)",
            bank: Some(0),
            entry: 0xCFB2,
            code: (0xCFB2, 0xCFDB),
            // Which key it took is in which way it leaves, so in A and F.
            outputs: &[Reg::A, Reg::F],
            // Back to the menu; or, for 1, at the redefinition, whose waits
            // are checked piece by piece.
            exits: &[0xCCD0, 0xCFBA],
            preserves: &[],
            rewrite: |g, _, r, io| {
                let choice = new_game::choose(g, io);
                pick_regs(io, r, choice)
            },
        },
        Routine {
            name: "the redefinition's screen (0:CFDC)",
            bank: Some(0),
            entry: 0xCFDC,
            code: (0xCFDC, 0xCFF7),
            outputs: &[],
            // To its first wait, for the keys to be let go.
            exits: &[0xCFF8],
            preserves: &[],
            rewrite: |g, _, r, _| {
                new_game::redefine(g);
                r
            },
        },
        Routine {
            name: "wait for the keys to be let go (0:D028)",
            bank: Some(0),
            entry: 0xD028,
            code: (0xD028, 0xD032),
            // The keys held, and whether any are, by the Z flag.
            outputs: &[Reg::A, Reg::F],
            // Polled again, or let go.
            exits: &[0xD028, 0xCFFB],
            preserves: &[],
            rewrite: |_, _, mut r, io| {
                let held = new_game::held(io);
                let f = and_flags(held, new_game::any_key(io));
                r.set(Reg::A, held);
                r.set(Reg::F, f);
                r
            },
        },
        Routine {
            name: "a scan of the keyboard (0:D033)",
            bank: Some(0),
            entry: 0xD033,
            code: (0xD033, 0xD052),
            // The key in D, 0xFF for none; NZ if more than one is held.
            outputs: &[Reg::D, Reg::F],
            exits: &[],
            preserves: &[],
            rewrite: |_, _, r, io| scan_regs(io, r, new_game::scan(io)),
        },
        Routine {
            name: "a key recorded and named (0:D000)",
            bank: Some(0),
            entry: 0xD000,
            code: (0xD000, 0xD018),
            // Where the next key and the next prompt go.
            outputs: &[Reg::D, Reg::E, Reg::H, Reg::L],
            exits: &[0xD019],
            preserves: &[],
            rewrite: |g, _, mut r, _| {
                let keys = u16::from_be_bytes([r.get(Reg::D), r.get(Reg::E)]);
                let prompt = u16::from_be_bytes([r.get(Reg::H), r.get(Reg::L)]);
                new_game::record(g, (keys - 0xD154) as u8, r.get(Reg::A));
                r.set_pair(Reg::D, Reg::E, keys + 1);
                r.set_pair(Reg::H, Reg::L, prompt + 0x0E);
                r
            },
        },
        Routine {
            name: "the keys set as the method (0:CFBD)",
            bank: Some(0),
            entry: 0xCFBD,
            code: (0xCFBD, 0xCFDB),
            outputs: &[],
            exits: &[0xCCD0],
            preserves: &[],
            rewrite: |g, _, r, _| {
                new_game::set_method(g, 0);
                r
            },
        },
        Routine {
            name: "the interrupt's table and vector (0:CF6F)",
            bank: Some(0),
            entry: 0xCF6F,
            code: (0xCF6F, 0xCF93),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                new_game::set_interrupt(g);
                r
            },
        },
        Routine {
            name: "the special locations and the start (0:CE4B)",
            bank: Some(0),
            entry: 0xCE4B,
            code: (0xCE4B, 0xCE8C),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                new_game::specials(g, &mut io.random);
                r
            },
        },
        Routine {
            name: "every item afresh (0:D298)",
            bank: Some(0),
            entry: 0xD298,
            code: (0xD298, 0xD2DE),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                items::reset(g, &mut io.random);
                r
            },
        },
        Routine {
            name: "the start, once (0:CC72)",
            bank: Some(0),
            entry: 0xCC72,
            code: (0xCC72, 0xCCAA),
            outputs: &[],
            // Up to the menu.
            exits: &[0xCCAB],
            preserves: &[],
            rewrite: |g, _, r, io| {
                new_game::start(g, io);
                r
            },
        },
        Routine {
            name: "a new game's set-up begun (0:CCD2)",
            bank: Some(0),
            entry: 0xCCD2,
            code: (0xCCD2, 0xCCF5),
            outputs: &[],
            // Up to the sixteen entries, whose random values read the ROM
            // in nearly every run (#21).
            exits: &[0xCCF6],
            preserves: &[],
            rewrite: |g, _, r, io| {
                new_game::prepare(g, io);
                r
            },
        },
        Routine {
            name: "a new game's first location (0:CD1B)",
            bank: Some(0),
            entry: 0xCD1B,
            code: (0xCD1B, 0xCD4D),
            outputs: &[],
            // Up to the warble, whose pause reads the ROM.
            exits: &[0xCD4E],
            preserves: &[],
            rewrite: |g, a, r, io| {
                new_game::first_location(g, a, io);
                r
            },
        },
        Routine {
            name: "a new game's set-up finished (0:CD51)",
            bank: Some(0),
            entry: 0xCD51,
            code: (0xCD51, 0xCD5E),
            outputs: &[],
            // It returns from 0:CC72's call, to the main loop's start.
            exits: &[0xBE61],
            preserves: &[],
            rewrite: |g, _, r, io| {
                new_game::finish(g, io);
                r
            },
        },
        Routine {
            name: "one pass of the main loop (0xBE62)",
            bank: None,
            entry: 0xBE62,
            code: (0xBE62, 0xBF3E),
            outputs: &[],
            // Back at its start; or a new game (BREAK), the game over's
            // wait for a key, or the ending's waits, which are the caller's.
            exits: &[0xBE62, 0xBE5A, 0xBF5D, 0xBEEF],
            preserves: &[],
            rewrite: |g, a, r, io| {
                main_loop::frame(g, a, io);
                r
            },
        },
        Routine {
            name: "leave the screen for the next location (0xBECE)",
            bank: None,
            entry: 0xBECE,
            code: (0xBECE, 0xBF3E),
            outputs: &[],
            exits: &[0xBE62, 0xBEEF],
            preserves: &[],
            rewrite: |g, a, r, io| {
                main_loop::leave(g, a, io, r.get(Reg::E));
                r
            },
        },
        Routine {
            name: "the ending's message (0:CF5F)",
            bank: Some(0),
            entry: 0xCF5F,
            code: (0xCF5F, 0xCF6E),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                main_loop::ending_message(g);
                r
            },
        },
        Routine {
            name: "location 0xBB darkened (0:C33C)",
            bank: Some(0),
            entry: 0xC33C,
            code: (0xC33C, 0xC359),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                journeys::darken(g);
                r
            },
        },
        Routine {
            name: "a message on entering a location (0:C35A)",
            bank: Some(0),
            entry: 0xC35A,
            code: (0xC35A, 0xC3AD),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                journeys::location_message(g);
                r
            },
        },
        Routine {
            name: "the main loop's hook and journeys (0:C3CA)",
            bank: Some(0),
            entry: 0xC3CA,
            code: (0xC3CA, 0xC40E),
            outputs: &[],
            exits: &[0xBE98, 0xBE62],
            preserves: &[],
            rewrite: |g, a, r, io| {
                journeys::hook(g, a, io);
                r
            },
        },
        Routine {
            name: "a doorway on entering (0:D8EC)",
            bank: Some(0),
            entry: 0xD8EC,
            code: (0xD8EC, 0xD97C),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                journeys::doorway(g, io);
                r
            },
        },
        Routine {
            name: "a doorway's sparkle (0:D97D)",
            bank: Some(0),
            entry: 0xD97D,
            code: (0xD97D, 0xDA6D),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                journeys::sparkle(g, io);
                r
            },
        },
        Routine {
            name: "the trade (0:DDEF)",
            bank: Some(0),
            entry: 0xDDEF,
            code: (0xDDEF, 0xDED1),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                journeys::trade(g, io);
                r
            },
        },
        Routine {
            name: "a location recoloured (0:C306)",
            bank: Some(0),
            entry: 0xC306,
            code: (0xC306, 0xC33B),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                journeys::recolour(g);
                r
            },
        },
        Routine {
            name: "the inventory shown (0:DA84)",
            bank: Some(0),
            entry: 0xDA84,
            code: (0xDA84, 0xDAB8),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                journeys::show_inventory(g);
                r
            },
        },
        Routine {
            name: "an item taken out of the inventory (0:DA6E)",
            bank: Some(0),
            entry: 0xDA6E,
            code: (0xDA6E, 0xDA83),
            // The slot before, to look at again; its caller counts in BC.
            outputs: &[Reg::H, Reg::L],
            exits: &[],
            preserves: &[Reg::B, Reg::C],
            rewrite: |g, _, mut r, _| {
                let at = u16::from_be_bytes([r.get(Reg::H), r.get(Reg::L)]);
                let back = journeys::remove(g, at, r.get(Reg::B));
                r.set_pair(Reg::H, Reg::L, back);
                r
            },
        },
        Routine {
            name: "the scripted scene (0xB723)",
            bank: None,
            entry: 0xB723,
            code: (0xB723, 0xB7BC),
            outputs: &[],
            // Its end jumps back to the main loop's start.
            exits: &[0xBE83, 0xBE62],
            preserves: &[],
            rewrite: |g, a, r, io| {
                scene::scripted(g, a, io);
                r
            },
        },
        Routine {
            name: "a new location revealed (0:CD73)",
            bank: Some(0),
            entry: 0xCD73,
            code: (0xCD73, 0xCE4A),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                screen::reveal(g);
                r
            },
        },
        Routine {
            name: "the scene moved on (0:CE8D)",
            bank: Some(0),
            entry: 0xCE8D,
            code: (0xCE8D, 0xCEA0),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                scene::move_on(g, io);
                r
            },
        },
        Routine {
            name: "the fifth character and its companion (0xB7DB)",
            bank: None,
            entry: 0xB7DB,
            code: (0xB7DB, 0xBA40),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, a, r, io| {
                fifth::bring_on(g, a.sprites(), io);
                r
            },
        },
        Routine {
            name: "a location's items placed (0:D484)",
            bank: Some(0),
            entry: 0xD484,
            code: (0xD484, 0xD4F3),
            outputs: &[],
            exits: &[],
            preserves: &[],
            // Its restocking tests the Z flag it's called with.
            rewrite: |g, _, r, io| {
                items::place(g, &mut io.random, r.get(Reg::F) & 0x40 != 0);
                r
            },
        },
        Routine {
            name: "the world restocked (0:D2DF)",
            bank: Some(0),
            entry: 0xD2DF,
            code: (0xD2DF, 0xD330),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                items::restock(g, &mut io.random, r.get(Reg::F) & 0x40 != 0);
                r
            },
        },
        Routine {
            name: "a free place for an item (0:D35A)",
            bank: Some(0),
            entry: 0xD35A,
            code: (0xD35A, 0xD389),
            // A is the free place's kind, 0: the new game's set-up stores
            // it plus 1 (0:D2D0).
            outputs: &[Reg::A, Reg::D, Reg::E],
            exits: &[],
            // Its callers keep the count they're filling to in B.
            preserves: &[
                Reg::B,
                Reg::H,
                Reg::L,
                Reg::Ixh,
                Reg::Ixl,
                Reg::Iyh,
                Reg::Iyl,
            ],
            rewrite: |g, _, mut r, io| {
                let at = items::free_place(g, &mut io.random);
                r.set_pair(Reg::D, Reg::E, at);
                r.set(Reg::A, g.read(at));
                r
            },
        },
        Routine {
            name: "an item picked up (0:D6D0)",
            bank: Some(0),
            entry: 0xD6D0,
            // With what it gives, the inventory and dropping, which follow
            // the energy's figure (0:D7F7) in the code.
            code: (0xD6D0, 0xD8C5),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                items::pick_up(g, io);
                r
            },
        },
        Routine {
            name: "a beep (0:D8C6)",
            bank: Some(0),
            entry: 0xD8C6,
            code: (0xD8C6, 0xD8D1),
            outputs: &[],
            exits: &[],
            preserves: &[
                Reg::D,
                Reg::E,
                Reg::H,
                Reg::L,
                Reg::Ixh,
                Reg::Ixl,
                Reg::Iyh,
                Reg::Iyl,
            ],
            rewrite: |_, _, r, io| {
                sound::beep(io, r.get(Reg::B));
                r
            },
        },
        Routine {
            name: "the objects in flight (0xBB84)",
            bank: None,
            entry: 0xBB84,
            code: (0xBB84, 0xBBEB),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                fighting::flight(g);
                r
            },
        },
        Routine {
            name: "draw an object in flight (0xBBEC)",
            bank: None,
            entry: 0xBBEC,
            code: (0xBBEC, 0xBC0A),
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
                Reg::Ixh,
                Reg::Ixl,
                Reg::Iyh,
                Reg::Iyl,
            ],
            rewrite: |g, _, r, _| {
                // Where in the back buffer in HL; the row and column, for
                // the changed-cell map, in B and C.
                let at = u16::from_be_bytes([r.get(Reg::H), r.get(Reg::L)]);
                fighting::draw(g, at, r.get(Reg::B), r.get(Reg::C));
                r
            },
        },
        Routine {
            name: "a shot hits Robin (0xBC6C)",
            bank: None,
            entry: 0xBC6C,
            code: (0xBC6C, 0xBCA9),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                fighting::shot_hits(g, io);
                r
            },
        },
        Routine {
            name: "Robin's arrow hits a character (0xBC0B)",
            bank: None,
            entry: 0xBC0B,
            code: (0xBC0B, 0xBC6B),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                fighting::arrow_hits(g, io);
                r
            },
        },
        Routine {
            name: "Robin's sword and fists strike (0xBD19)",
            bank: None,
            entry: 0xBD19,
            code: (0xBD19, 0xBD86),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                fighting::strike(g, io);
                r
            },
        },
        Routine {
            name: "the second group touches Robin (0xBCAA)",
            bank: None,
            entry: 0xBCAA,
            code: (0xBCAA, 0xBCFA),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                fighting::second_group_hits(g, io);
                r
            },
        },
        Routine {
            name: "start the hit's tune (6:C048)",
            bank: Some(6),
            entry: 0xC048,
            code: (0xC072, 0xC0A0),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                sound::hit_tune(g, io);
                r
            },
        },
        Routine {
            name: "Robin hit (0xBCFB)",
            bank: None,
            entry: 0xBCFB,
            code: (0xBCFB, 0xBD18),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                fighting::robin_hit(g);
                r
            },
        },
        Routine {
            name: "a character struck (0xBDF2)",
            bank: None,
            entry: 0xBDF2,
            code: (0xBDF2, 0xBE0E),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                let record = u16::from_be_bytes([r.get(Reg::Ixh), r.get(Reg::Ixl)]);
                fighting::struck(g, r.get(Reg::A), record);
                r
            },
        },
        Routine {
            name: "the wanderer's set-up (0:CEA1)",
            bank: Some(0),
            entry: 0xCEA1,
            code: (0xCEA1, 0xCEBA),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                wanderer::set_up(g);
                r
            },
        },
        Routine {
            name: "the wanderer walks (0xBA83)",
            bank: None,
            entry: 0xBA83,
            code: (0xBA83, 0xBB1A),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, a, r, _| {
                wanderer::walk(g, a.sprites());
                r
            },
        },
        Routine {
            name: "Robin meets the wanderer (0xBD87)",
            bank: None,
            entry: 0xBD87,
            code: (0xBD87, 0xBDB8),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                wanderer::meet(g, io);
                r
            },
        },
        Routine {
            name: "the meeting's sound and flash (0xBDCD)",
            bank: None,
            entry: 0xBDCD,
            // After the trampoline's call and the three bytes it takes.
            code: (0xBDD3, 0xBDF1),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                wanderer::flash(g, io);
                r
            },
        },
        Routine {
            name: "start the meeting's sound (6:C00C)",
            bank: Some(6),
            entry: 0xC00C,
            code: (0xC13D, 0xC159),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                sound::meeting(g, io);
                r
            },
        },
        Routine {
            name: "do two things overlap (0xBDB9)",
            bank: None,
            entry: 0xBDB9,
            code: (0xBDB9, 0xBDCC),
            // It answers in the carry, with the difference in A, and leaves
            // whichever register set its subtractions ended in.
            outputs: &[
                Reg::A,
                Reg::F,
                Reg::B,
                Reg::C,
                Reg::D,
                Reg::E,
                Reg::H,
                Reg::L,
                Reg::B_,
                Reg::C_,
                Reg::D_,
                Reg::E_,
                Reg::H_,
                Reg::L_,
            ],
            exits: &[],
            preserves: &[],
            rewrite: |_, _, r, _| overlap_regs(r),
        },
        Routine {
            name: "Robin's actions (0:C8DF)",
            bank: Some(0),
            entry: 0xC8DF,
            code: (0xC8DF, 0xCB73),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                actions::act(g, io);
                r
            },
        },
        Routine {
            name: "Robin's update (0:C59A)",
            bank: Some(0),
            entry: 0xC59A,
            code: (0xC59A, 0xC5CD),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, a, r, io| {
                actions::update(g, a.sprites(), io);
                r
            },
        },
        Routine {
            name: "Robin's energy after a knock-down (0:D7F7)",
            bank: Some(0),
            entry: 0xD7F7,
            code: (0xD7F7, 0xD820),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                actions::energy(g);
                r
            },
        },
        Routine {
            name: "the lower panel's colours (0xBE0F)",
            bank: None,
            entry: 0xBE0F,
            code: (0xBE0F, 0xBE2B),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, _| {
                actions::panel_colours(g);
                r
            },
        },
        Routine {
            name: "a twang (0xBE2C)",
            bank: None,
            entry: 0xBE2C,
            code: (0xBE2C, 0xBE3E),
            outputs: &[],
            exits: &[],
            // Its caller keeps the arrow's slot in HL across it.
            preserves: &[
                Reg::A,
                Reg::F,
                Reg::B,
                Reg::C,
                Reg::D,
                Reg::E,
                Reg::H,
                Reg::L,
                Reg::Ixh,
                Reg::Ixl,
                Reg::Iyh,
                Reg::Iyl,
            ],
            rewrite: |_, _, r, io| {
                sound::twang(io, r.get(Reg::B));
                r
            },
        },
        Routine {
            name: "fire an arrow (0xBB43)",
            bank: None,
            entry: 0xBB43,
            code: (0xBB43, 0xBB83),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                actions::launch_arrow(g, io);
                r
            },
        },
        Routine {
            name: "play a sample, by its delay, length, amplitudes and bytes (4:C090)",
            bank: Some(4),
            entry: 0xC090,
            code: (0xC090, 0xC0DA),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                let pair = |hi, lo| u16::from_be_bytes([r.get(hi), r.get(lo)]);
                sound::play(
                    g,
                    io,
                    r.get(Reg::A),
                    pair(Reg::D, Reg::E),
                    pair(Reg::H, Reg::L),
                    pair(Reg::Ixh, Reg::Ixl),
                );
                r
            },
        },
        Routine {
            name: "pick a sample by R, and play it (4:C012)",
            bank: Some(4),
            entry: 0xC012,
            // Its jump to the picker, then the picker; the next entry and the
            // table between are not code it runs.
            code: (0xC020, 0xC02F),
            outputs: &[],
            exits: &[],
            preserves: &[],
            rewrite: |g, _, r, io| {
                sound::sample(g, io);
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

/// `0xBDB9` as it leaves the registers. It takes one thing's position in
/// BC and the other's in the other set's BC (B vertical, C horizontal), and
/// its limits in DE. It subtracts the vertical positions, flipping to the
/// other set with `EXX` and negating when that borrows, and compares with
/// D, the set it's in; then the same with the horizontal ones and E. The
/// answer, in the carry, is the game's (`wanderer::overlap`); the rest is
/// what those steps leave.
fn overlap_regs(mut r: Regs) -> Regs {
    fn exx(r: &mut Regs) {
        for (a, b) in [
            (Reg::B, Reg::B_),
            (Reg::C, Reg::C_),
            (Reg::D, Reg::D_),
            (Reg::E, Reg::E_),
            (Reg::H, Reg::H_),
            (Reg::L, Reg::L_),
        ] {
            let t = r.get(a);
            r.set(a, r.get(b));
            r.set(b, t);
        }
    }
    let thing = |r: &Regs, [y, x, down, across]: [Reg; 4]| wanderer::Thing {
        x: r.get(x),
        y: r.get(y),
        down: r.get(down),
        across: r.get(across),
    };
    let first = thing(&r, [Reg::B, Reg::C, Reg::D, Reg::E]);
    let second = thing(&r, [Reg::B_, Reg::C_, Reg::D_, Reg::E_]);
    // One difference, as `SUB` then, on a borrow, `EXX` and `NEG`.
    let difference = |r: &mut Regs, a: u8, other: Reg| {
        exx(r);
        let (d, f) = flags::sub(a, r.get(other));
        if f & flags::C != 0 {
            exx(r);
            flags::neg(d)
        } else {
            (d, f)
        }
    };
    let (dy, _) = difference(&mut r, first.y, Reg::B);
    let down = r.get(Reg::D);
    let f = flags::cp(dy, down);
    if f & flags::C == 0 {
        r.set(Reg::A, dy);
        r.set(Reg::F, f);
        return r;
    }
    let c = r.get(Reg::C);
    let (dx, _) = difference(&mut r, c, Reg::C);
    let across = r.get(Reg::E);
    let f = flags::cp(dx, across);
    let hit = wanderer::overlap(first, second);
    r.set(Reg::A, dx);
    r.set(Reg::F, f & !flags::C | if hit { flags::C } else { 0 });
    r
}

/// The flags of the Z80's 8-bit subtractions, as `SUB`, `CP` and `NEG`
/// leave them.
/// The registers `0:D033` leaves, from what the rewrite's scan found: D the
/// key, or `0xFF` for none, and the Z flag set, from its closing `CP A`; A
/// then is the last half-row's key, if it has one. For more than one held,
/// Z clear and D as the code had it when it found the second: at a second
/// half-row, its `INC D`; at a second key in one half-row, the `SRL H` that
/// found the first. Those come from the keys; a scan that says "more than
/// one" when there's one or none leaves Z set, so it shows.
fn scan_regs(io: &robin::io::Io, mut r: Regs, found: new_game::Scan) -> Regs {
    match found {
        new_game::Scan::None => {
            r.set(Reg::D, 0xFF);
            r.set(Reg::F, flags::cp(0, 0));
        }
        new_game::Scan::One(code) => {
            let last = (0x2F - code) % 8 == 7;
            let a = if last { code } else { 0 };
            r.set(Reg::D, code);
            r.set(Reg::F, flags::cp(a, a));
        }
        new_game::Scan::Several => {
            let (d, f) = several(io);
            r.set(Reg::D, d);
            r.set(Reg::F, f);
        }
    }
    r
}

/// D and F where `0:D033` finds a second key; Z set if it never does.
fn several(io: &robin::io::Io) -> (u8, u8) {
    let mut d: u8 = 0xFF;
    for (row, port) in new_game::SCAN_ROWS.into_iter().enumerate() {
        let a = !io.input(port) & 0x1F;
        if a == 0 {
            continue;
        }
        d = d.wrapping_add(1);
        if d != 0 {
            let mut f = (d & 0x80) | (d & 0x28);
            if d & 0x0F == 0 {
                f |= 0x10;
            }
            if d == 0x80 {
                f |= 0x04;
            }
            return (d, f);
        }
        let mut h = a;
        let mut code = 0x2F - row as u8;
        loop {
            code = code.wrapping_sub(8);
            let carry = h & 1;
            h >>= 1;
            if carry != 0 {
                break;
            }
        }
        if h != 0 {
            let mut f = flags::C | (h & 0x28);
            if h.count_ones().is_multiple_of(2) {
                f |= 0x04;
            }
            return (d, f);
        }
        d = code;
    }
    (d, 0x40)
}

/// `AND n`'s flags for the result `a`, with Z from the decision `nonzero`
/// the rewrite made, so a wrong one shows: H set, C and N clear, P/V the
/// parity, bits 5 and 3 from the result.
fn and_flags(a: u8, nonzero: bool) -> u8 {
    let mut f = 0x10 | (a & 0x80) | (a & 0x28);
    if !nonzero {
        f |= 0x40;
    }
    if a.count_ones().is_multiple_of(2) {
        f |= 0x04;
    }
    f
}

/// `IN A,(C)`'s flags for the byte `v`: S, Z, P/V and bits 5 and 3 from it,
/// H and N clear, the carry `f`'s.
fn in_flags(v: u8, f: u8) -> u8 {
    let mut fl = (v & 0x80) | (v & 0x28) | (f & 1);
    if v == 0 {
        fl |= 0x40;
    }
    if v.count_ones().is_multiple_of(2) {
        fl |= 0x04;
    }
    fl
}

/// `RRA`: the result, and the flags with the carry `carry` (the rewrite's
/// decision, so a wrong one shows); S, Z and P/V as they were.
fn rra(a: u8, f: u8, carry: bool) -> (u8, u8) {
    let r = a >> 1 | (f & 1) << 7;
    (r, (f & 0xC4) | (r & 0x28) | u8::from(carry))
}

/// The registers `0:CCBB` leaves. For 0, its `IN` and `RRA` of the half-row
/// with 0 on it, carry clear. Otherwise the `CPL : AND 0x0F` of the half-row with
/// 1 to 5, Z set if it's waiting.
fn poll_regs(io: &robin::io::Io, mut r: Regs, poll: new_game::Menu) -> Regs {
    let (a, f) = if poll == new_game::Menu::Start {
        let v = io.input(0xEFFE);
        rra(v, in_flags(v, r.get(Reg::F)), false)
    } else {
        let a = !io.input(0xF7FE) & 0x0F;
        (a, and_flags(a, poll == new_game::Menu::Pick))
    };
    r.set(Reg::A, a);
    r.set(Reg::F, f);
    r
}

/// The registers `0:CFB2` leaves: an `IN` of the half-row with 1 to 5, an
/// `RRA` for
/// each key it looks at, the carry clear at the one it took; then the
/// method's number in A, if it set one.
fn pick_regs(io: &robin::io::Io, mut r: Regs, choice: new_game::Choice) -> Regs {
    let looked = match choice {
        new_game::Choice::Redefine => 1,
        new_game::Choice::Set(m) => m + 1,
        new_game::Choice::Nothing => 3,
    };
    let row = io.input(0xF7FE);
    let (mut a, mut f) = (row, in_flags(row, r.get(Reg::F)));
    for n in 1..=looked {
        let took = n == looked && choice != new_game::Choice::Nothing;
        (a, f) = rra(a, f, !took);
    }
    if let new_game::Choice::Set(m) = choice {
        a = m;
    }
    r.set(Reg::A, a);
    r.set(Reg::F, f);
    r
}

mod flags {
    pub const C: u8 = 0x01;
    const N: u8 = 0x02;
    const PV: u8 = 0x04;
    const H: u8 = 0x10;
    const Z: u8 = 0x40;
    const S: u8 = 0x80;

    fn of(a: u8, b: u8, r: u8, xy: u8) -> u8 {
        let mut f = N | (r & S) | (xy & 0x28);
        if r == 0 {
            f |= Z;
        }
        if a & 0x0F < b & 0x0F {
            f |= H;
        }
        if (a ^ b) & (a ^ r) & 0x80 != 0 {
            f |= PV;
        }
        if a < b {
            f |= C;
        }
        f
    }

    /// `SUB b`: the result and the flags, bits 5 and 3 from the result.
    pub fn sub(a: u8, b: u8) -> (u8, u8) {
        let r = a.wrapping_sub(b);
        (r, of(a, b, r, r))
    }

    /// `CP b`: the flags, bits 5 and 3 from the operand.
    pub fn cp(a: u8, b: u8) -> u8 {
        of(a, b, a.wrapping_sub(b), b)
    }

    /// `NEG`: 0 − a.
    pub fn neg(a: u8) -> (u8, u8) {
        let r = 0u8.wrapping_sub(a);
        (r, of(0, a, r, r))
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
