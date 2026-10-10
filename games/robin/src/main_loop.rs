//! The main loop: one pass of it a frame, with its own checks, BREAK and
//! the game over; leaving the screen and entering the next location; and
//! the ending (`docs/re/robin.md`, *The main loop*).

use crate::actions;
use crate::assets::Assets;
use crate::characters;
use crate::fifth;
use crate::fighting;
use crate::game::Game;
use crate::io::Io;
use crate::items;
use crate::journeys;
use crate::map;
use crate::movement;
use crate::print;
use crate::scene;
use crate::screen;
use crate::sound;
use crate::wanderer;

/// Whether BREAK is held, Caps Shift with Space, which starts a new game
/// (`0:C433`).
#[must_use]
pub fn break_held(io: &Io) -> bool {
    io.input(0x7FFE) & 1 == 0 && io.input(0xFEFE) & 1 == 0
}

/// The game over, once his energy is spent and he's lain down for a while:
/// its message and its tune; then the game waits for a key and starts
/// again, which is the main loop's (`0xBF3F`). Whether it's over.
pub fn game_over(g: &mut Game, io: &mut Io) -> bool {
    if g.robin.energy & 0x80 == 0 || g.robin.down != 0x3C {
        return false;
    }
    g.printer.replace = 0x60;
    print::print_at(g, 0x60, 0xB51F);
    actions::banked_call(g, io, sound::MUSIC_BANK, sound::game_over_tune);
    true
}

/// How a pass of the main loop ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// Back at the loop's start, for the next frame.
    Next,
    /// BREAK was held: a new game.
    NewGame,
    /// The game is over, and waits for a key before a new one.
    GameOver,
    /// The ending's message is up: the game waits for every key to be let
    /// go, then for one to be pressed, then starts a new one.
    Ending,
}

/// The location whose entry is the ending.
const ENDING_AT: u16 = 0x0069;

/// One pass of the main loop (`0xBE62`): BREAK; Robin, the objects in
/// flight, the characters, the hits, the scripted scene, the wanderer and
/// the fifth character, meeting the wanderer; the screen's flush; picking
/// things up; the game over, the hook and the trade; and last, leaving the
/// screen, which enters the next location, or the ending.
pub fn frame(g: &mut Game, assets: &Assets, io: &mut Io) -> Pass {
    if break_held(io) {
        return Pass::NewGame;
    }
    let sprites = assets.sprites();
    actions::update(g, sprites, io);
    fighting::flight(g);
    characters::move_one(g, sprites, &mut io.random);
    characters::move_second(g, sprites, &mut io.random);
    fighting::arrow_hits(g, io);
    fighting::shot_hits(g, io);
    fighting::second_group_hits(g, io);
    fighting::strike(g, io);
    if scene::scripted(g, assets, io) {
        return Pass::Next;
    }
    wanderer::walk(g, sprites);
    fifth::bring_on(g, sprites, io);
    wanderer::meet(g, io);
    screen::flush(g);
    items::pick_up(g, io);
    if game_over(g, io) {
        return Pass::GameOver;
    }
    if journeys::hook(g, assets, io) {
        return Pass::Next;
    }
    journeys::trade(g, io);
    match movement::leave_by_edge(g) {
        Some(direction) => leave(g, assets, io, direction),
        None => Pass::Next,
    }
}

/// Off the screen's edge, `direction` the way he left (`0xBECE`): the step
/// to the next location, then its entry, or the ending at `0x69`.
pub fn leave(g: &mut Game, assets: &Assets, io: &mut Io, direction: u8) -> Pass {
    g.map.location = map::step(g.map.location, direction);
    if g.map.location == ENDING_AT {
        ending(g, io);
        return Pass::Ending;
    }
    enter(g, assets, io, direction);
    Pass::Next
}

/// Robin's sprite record; its frame is at `+8`.
const ROBIN: u16 = 0xCB76;

/// Entering the location Robin has stepped into, `direction` the way he
/// came (`0xBF0E`): the play area cleared and drawn, with the special
/// locations' lists; its colours, and a message at a few; the characters'
/// entry and the items; the play area copied to the screen; the hook
/// cleared; and a doorway, if it's one.
pub fn enter(g: &mut Game, assets: &Assets, io: &mut Io, direction: u8) {
    let flags = 0xCB7A;
    g.write(flags, g.read(flags) & !0x02);
    screen::clear_play_area(g, 0);
    map::draw_location(g, assets.map());
    map::draw_special(g, assets.map());
    journeys::darken(g);
    journeys::recolour(g);
    journeys::location_message(g);
    characters::enter(g, assets.sprites(), direction, &mut io.random);
    let zero = drawn_flags(g) & 0x40 != 0;
    items::place(g, &mut io.random, zero);
    screen::copy_attrs(g);
    screen::copy_pixels(g);
    g.write(0xC3CB, 0x03);
    g.write(0xC3CC, 0xDA);
    journeys::doorway(g, io);
}

/// The flags the characters' entry leaves, which placing the items reads
/// (*Items*, *Restocking*). It ends by drawing Robin, and the frame drawing
/// (`0:C5CE`) returns with F taken from a word it pushed: the low byte of
/// his frame's address in its table, plus 2. The table is indexed by the
/// frame doubled in one byte, so its bit 7, the way he faces, drops out.
fn drawn_flags(g: &Game) -> u8 {
    let frame = g.read(ROBIN + 8);
    let entry = g
        .sprites
        .table
        .wrapping_add(u16::from(frame.wrapping_mul(2)));
    let at = u16::from_le_bytes([g.read(entry), g.read(entry.wrapping_add(1))]);
    at.wrapping_add(2) as u8
}

/// The ending, on entering location `0x69` (`0xBEDF`): the play area
/// cleared, its message printed and revealed (`0:CF5F`), then the border
/// flashed with R, `0x960` times. The waits for the keys are the caller's.
pub fn ending(g: &mut Game, io: &mut Io) {
    ending_message(g);
    io.wait(4 + 10); // DI, LD BC,0x960
    for n in 0..0x960 {
        io.wait(9); // LD A,R
        let a = io.random.r();
        io.out(u16::from(a) << 8 | 0xFE, a);
        // OUT, DEC BC, LD A,B, OR C, JR NZ.
        io.wait(11 + 6 + 4 + 4 + if n + 1 == 0x960 { 7 } else { 12 });
    }
}

/// The ending's message: the play area cleared, stock message `0x50` printed
/// at its top left, and revealed (`0:CF5F`).
pub fn ending_message(g: &mut Game) {
    screen::clear_play_area(g, 0);
    print::print_message(g, 0x50, 0, 0);
    screen::reveal(g);
}
