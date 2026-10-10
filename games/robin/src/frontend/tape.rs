//! Finding the player's tape: named on the command line, or in `assets/` or
//! the current folder. Asking for it in the window is #68's.

use std::path::PathBuf;

/// What's said when there's no tape.
pub const NOT_FOUND: &str = "No tape found. Robin of the Wood needs your own copy of the 128K tape \
(see assets/README.md for the one dump it supports): name it on the command line, \
or put it in assets/ or this folder.";

/// The first file that is the supported tape, in `assets/`, then here.
#[must_use]
pub fn find() -> Option<PathBuf> {
    ["assets", "."].into_iter().find_map(|dir| {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .find(|p| std::fs::read(p).is_ok_and(|b| robin::is_the_tape(&b)))
    })
}
