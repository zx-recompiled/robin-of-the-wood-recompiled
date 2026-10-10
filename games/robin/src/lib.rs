//! Robin of the Wood (Odin Computer Graphics, 1986), the ZX Spectrum 128K
//! release, rewritten as ordinary Rust.
//!
//! Nothing is rewritten yet. The game reads the player's own tape at startup
//! ([`assets::read_game`]), and each subsystem, as it is rewritten, will
//! parse its graphics, maps, text or music from what that returns. The
//! rewrite is checked against the original routine by routine; see
//! `README.md`.

pub mod actions;
pub mod assets;
pub mod characters;
pub mod controls;
pub mod fifth;
pub mod fighting;
pub mod game;
pub mod io;
pub mod items;
pub mod journeys;
pub mod layout;
pub mod main_loop;
pub mod map;
pub mod movement;
pub mod new_game;
pub mod print;
pub mod scene;
pub mod screen;
pub mod sound;
pub mod sprites;
pub mod wanderer;

pub use assets::{TAPE_SHA1, is_the_tape};
pub use game::Game;

/// A count the original keeps in an 8-bit register and counts down to zero
/// after the first time round: 0 means 256.
pub(crate) fn times(n: u8) -> u16 {
    if n == 0 { 256 } else { u16::from(n) }
}
