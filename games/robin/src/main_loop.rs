//! The main loop's own checks: BREAK, and the game over (`docs/re/robin.md`,
//! *The main loop*).

use crate::actions;
use crate::game::Game;
use crate::io::Io;
use crate::print;
use crate::sound;

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
