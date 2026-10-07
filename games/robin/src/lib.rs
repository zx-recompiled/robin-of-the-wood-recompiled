//! Robin of the Wood (Odin Computer Graphics, 1986), the ZX Spectrum 128K
//! release, rewritten as ordinary Rust.
//!
//! Nothing is rewritten yet. The game reads the player's own tape at startup
//! ([`assets::read_game`]), and each subsystem, as it is rewritten, will
//! parse its graphics, maps, text or music from what that returns. The
//! rewrite is checked against the original routine by routine; see
//! `README.md`.

pub mod assets;
pub mod game;
pub mod layout;
pub mod map;
pub mod print;
pub mod screen;
pub mod sprites;

pub use assets::{TAPE_SHA1, is_the_tape};
pub use game::Game;
