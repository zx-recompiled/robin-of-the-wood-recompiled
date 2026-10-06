//! Robin of the Wood (Odin Computer Graphics, 1986), the ZX Spectrum 128K
//! release, rewritten as ordinary Rust.
//!
//! Nothing is rewritten yet. The game will read its graphics, maps, text and
//! music from the player's own tape at startup, and the rewrite is checked
//! against the original routine by routine; see `README.md`.

/// SHA-1 of the one dump this project supports: the 128K release as a TZX
/// file (`docs/re/robin.md`, *The tape*). Any other file is refused.
pub const TAPE_SHA1: &str = "2aad3402cdc08907000900c5da8c29eeb2f48c9e";

/// Whether `bytes` are the supported dump.
#[must_use]
pub fn is_the_tape(bytes: &[u8]) -> bool {
    zx_core::sha1::sha1_hex(bytes) == TAPE_SHA1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn anything_else_is_not_the_tape() {
        assert!(!is_the_tape(b""));
        assert!(!is_the_tape(b"ZXTape!\x1a\x01\x14"));
    }

    /// The tape in `assets/`, if there is one, must be the supported dump:
    /// a different one would make every later check compare against the
    /// wrong program. Without a tape there is nothing to check, and it says
    /// so.
    #[test]
    fn the_local_tape_is_the_supported_dump() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let tapes: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("tzx")))
            .collect();
        if tapes.is_empty() {
            println!("skipped: no .tzx in {}", dir.display());
            return;
        }
        assert!(
            tapes
                .iter()
                .any(|p| std::fs::read(p).is_ok_and(|b| is_the_tape(&b))),
            "assets/ has a tape, but not the supported dump (SHA-1 {TAPE_SHA1}): {tapes:?}"
        );
    }
}
