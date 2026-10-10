//! The scripted scene: a character who stands at one location, moves when
//! Robin comes close, and then takes him away (`docs/re/robin.md`, *The
//! scripted scene*).

use crate::actions;
use crate::assets::Assets;
use crate::characters;
use crate::game::Game;
use crate::io::Io;
use crate::journeys;
use crate::map;
use crate::screen;
use crate::sound;
use crate::sprites;

/// Where the scene is, picked when a game starts and again after it plays.
const AT: u16 = 0xD295;
/// Its counter: it acts every fourth call, and once it's moving, counts
/// down to the end.
const COUNTER: u16 = 0xD297;
/// Its sprite record, set up on entering its location (*The characters*),
/// and the frame table it's drawn from.
const RECORD: u16 = 0xB7BD;
const FRAMES: u16 = 0x8C8F;
/// Its animation sequences once it moves, by the way it faces.
const MOVING_RIGHT: u16 = 0xB7C8;
const MOVING_LEFT: u16 = 0xB7CD;
/// Its flags: bit 7 set once it's moving.
const MOVING: u8 = 0x80;
/// The eight places it can be, a word each.
const PLACES: u16 = 0xD27F;
/// Where it takes him.
const TAKEN_TO: u16 = 0x009C;

fn word(g: &Game, at: u16) -> u16 {
    u16::from_le_bytes([g.read(at), g.read(at.wrapping_add(1))])
}

fn set_word(g: &mut Game, at: u16, v: u16) {
    let [lo, hi] = v.to_le_bytes();
    g.write(at, lo);
    g.write(at.wrapping_add(1), hi);
}

/// At the scene's location, every fourth call: its character is drawn;
/// with Robin in the middle of the screen, it starts moving; and once it
/// has moved and its counter has run out, he's taken away (`0xB723`).
/// Whether he was: then it returns to the main loop's start rather than to
/// its caller.
pub fn scripted(g: &mut Game, assets: &Assets, io: &mut Io) -> bool {
    if g.map.location != word(g, AT) {
        return false;
    }
    let count = g.read(COUNTER).wrapping_sub(1);
    g.write(COUNTER, count);
    if count & 3 != 0 {
        return false;
    }
    g.sprites.table = FRAMES;
    let flags = RECORD + 4;
    g.write(flags, g.read(flags) | 0x05);
    sprites::animate(g, assets.sprites(), RECORD);
    if (0x24..0x5C).contains(&g.robin.x) && g.read(flags) & MOVING == 0 {
        g.write(flags, g.read(flags) | MOVING);
        g.write(COUNTER, 0x50);
        let sequence = if g.read(RECORD + 8) & 0x80 != 0 {
            MOVING_LEFT
        } else {
            MOVING_RIGHT
        };
        set_word(g, RECORD + 2, sequence);
        return false;
    }
    if g.read(RECORD + 5) & 0x7F != 2 {
        return false;
    }
    g.robin.override_controls[1] = 0;
    if g.read(COUNTER) != 0 {
        return false;
    }
    taken_away(g, assets, io);
    true
}

/// The end: a sample and a beeper sound, then Robin is taken to `0x9C`,
/// entered as a new game's first location is, and the scene moves on
/// (`0xB790`).
fn taken_away(g: &mut Game, assets: &Assets, io: &mut Io) {
    actions::banked_call(g, io, sound::PLAYER_BANK as u8, sound::scene_sample);
    sound::warble(g, io);
    g.map.location = TAKEN_TO;
    let flags = 0xCB7A;
    g.write(flags, g.read(flags) & !0x02);
    screen::clear_play_area(g, 0);
    map::draw_location(g, assets.map());
    journeys::recolour(g);
    characters::enter(g, assets.sprites(), 4, &mut io.random);
    screen::reveal(g, io);
    move_on(g, io);
}

/// A new place for the scene, one of eight, from `0:CD5F`'s random value
/// (`0:CE8D`).
pub fn move_on(g: &mut Game, io: &mut Io) {
    let n = journeys::random(g, io) & 0x0E;
    let place = word(g, PLACES + u16::from(n));
    set_word(g, AT, place);
}
