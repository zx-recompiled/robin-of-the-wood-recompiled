//! The interrupt, once a frame (`docs/re/robin.md`, *Interrupts*): the
//! music player, in bank 6.

use crate::actions;
use crate::game::Game;
use crate::io::Io;
use crate::sound;

/// The interrupt's handler (`0:DED3`): ENTER and the tune, the wobble, and
/// the effect, each through the trampoline into bank 6. It saves and
/// restores every register, and turns interrupts back on, which is the
/// machine's.
pub fn frame(g: &mut Game, io: &mut Io) {
    actions::banked_call(g, io, sound::MUSIC_BANK, sound::music);
    actions::banked_call(g, io, sound::MUSIC_BANK, sound::wobble);
    actions::banked_call(g, io, sound::MUSIC_BANK, sound::effect);
}
