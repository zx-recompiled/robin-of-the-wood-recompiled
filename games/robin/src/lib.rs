//! Robin of the Wood (Odin Computer Graphics, 1986), the ZX Spectrum 128K
//! release, rewritten as ordinary Rust.
//!
//! Nothing is rewritten yet. The game reads its graphics, maps, text and
//! music from the player's own tape at startup ([`assets::read_game`]), and
//! the rewrite is checked against the original routine by routine; see
//! `README.md`.

pub mod assets;
pub mod layout;

pub use assets::{TAPE_SHA1, is_the_tape};
