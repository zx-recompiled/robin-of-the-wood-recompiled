//! Finding the player's copy of the game: where to look, reading a tape out
//! of the zip an archive serves it in, and keeping it. Adapted from
//! starquake-recompiled's (`REUSED.md`). Robin's tape goes by no one name,
//! so every `.tzx`, and every zip holding one, is looked at, and only its
//! SHA-1 decides (`assets/README.md`).
//!
//! Nothing here knows what the tape holds: whether a file is the supported
//! one is the caller's `accept`, so the tests can run on made-up files.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// What the program keeps a located tape as, in the data folder.
const KEPT: &str = "robin-of-the-wood.tzx";

/// The folder under the data folder that is this program's.
const APP: &str = "robin-of-the-wood-recompiled";

/// A tape that passed the check, and the file it came from.
pub struct Tape {
    pub bytes: Vec<u8>,
    #[allow(dead_code, reason = "the tests check which file was found")]
    pub from: PathBuf,
}

/// Why a file was not taken.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// It could not be read, or is not a zip it claims to be.
    Unreadable(String),
    /// It was read, and it is not the tape.
    NotTheTape,
    /// A zip with no tape in it at all.
    NoTapeInZip,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Refused::Unreadable(why) => f.write_str(why),
            Refused::NotTheTape => f.write_str(
                "that is not the Robin of the Wood tape this version needs (the 128K release)",
            ),
            Refused::NoTapeInZip => f.write_str("that zip has no .tzx file in it"),
        }
    }
}

/// Where to look, in order, when the player has not said: beside the
/// program, which is where somebody who has unpacked a release has it; the
/// data folder, where a located tape is kept; and the working folder and its
/// `assets/`, so a development checkout finds its own.
pub fn folders() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        out.push(dir.to_path_buf());
        out.push(dir.join("assets"));
    }
    if let Some(dir) = data_dir() {
        out.push(dir.join(APP));
    }
    out.push(PathBuf::from("."));
    out.push(PathBuf::from("assets"));
    out
}

/// Where this system keeps application data a user installed themselves: on
/// Linux and the other unices, `$XDG_DATA_HOME`, or `~/.local/share`.
#[cfg(all(unix, not(target_os = "macos")))]
fn data_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(xdg));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share"))
}

/// Where this system keeps application data a user installed themselves.
#[cfg(target_os = "macos")]
fn data_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
}

/// Where this system keeps application data a user installed themselves.
#[cfg(windows)]
fn data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// The first tape `accept` takes in `folders`: in each, its `.tzx` files,
/// then its zips, each in name order. A file that fails is passed over, so a
/// wrong one can't hide a good one after it.
pub fn find(folders: &[PathBuf], accept: impl Fn(&[u8]) -> bool) -> Option<Tape> {
    for folder in folders {
        let Ok(entries) = fs::read_dir(folder) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        // Listing order is the file system's; sorting keeps the result the
        // same everywhere.
        files.sort();
        for ext in ["tzx", "zip"] {
            for file in files.iter().filter(|f| has_extension(f, ext)) {
                if let Ok(tape) = load(file, &accept) {
                    return Some(tape);
                }
            }
        }
    }
    None
}

/// Reads the tape in `path`: the file itself, or, for a zip, the first
/// `.tzx` inside it that `accept` takes, whatever it's called.
///
/// # Errors
///
/// If the file cannot be read, or holds no tape `accept` takes.
pub fn load(path: &Path, accept: impl Fn(&[u8]) -> bool) -> Result<Tape, Refused> {
    let bytes = fs::read(path)
        .map_err(|e| Refused::Unreadable(format!("cannot read {}: {e}", path.display())))?;
    let from = path.to_path_buf();
    if !has_extension(path, "zip") {
        return if accept(&bytes) {
            Ok(Tape { bytes, from })
        } else {
            Err(Refused::NotTheTape)
        };
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| {
        Refused::Unreadable(format!(
            "{} is not a zip that can be read: {e}",
            path.display()
        ))
    })?;
    let mut any_tape = false;
    for i in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(i) else {
            continue;
        };
        if !has_extension(Path::new(entry.name()), "tzx") || !entry.is_file() {
            continue;
        }
        any_tape = true;
        let mut inner = Vec::new();
        if entry.read_to_end(&mut inner).is_ok() && accept(&inner) {
            return Ok(Tape { bytes: inner, from });
        }
    }
    Err(if any_tape {
        Refused::NotTheTape
    } else {
        Refused::NoTapeInZip
    })
}

/// Saves a located tape where [`folders`] will find it next time, and
/// returns where that is.
///
/// # Errors
///
/// If this system has no data folder, or it cannot be written.
pub fn keep(bytes: &[u8]) -> Result<PathBuf, String> {
    let dir = data_dir()
        .ok_or("this system has no folder for application data")?
        .join(APP);
    keep_in(&dir, bytes)
}

fn keep_in(dir: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let path = dir.join(KEPT);
    fs::write(&path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

/// The tape's bytes from a file named on the command line: a tape, or a zip
/// holding one.
///
/// # Errors
///
/// If the file cannot be read or is not the supported tape.
pub fn read(path: &Path) -> Result<Vec<u8>, String> {
    load(path, robin::is_the_tape)
        .map(|t| t.bytes)
        .map_err(|why| format!("{}: {why}", path.display()))
}

/// What to tell a player when [`find`] came back empty, with no window to
/// ask in: where it looked.
pub fn not_found_message(folders: &[PathBuf]) -> String {
    let mut msg = String::from(
        "No tape found. This program contains no part of the original game and reads its\n\
         graphics, maps, text and sound from your own copy at startup: the 128K release,\n\
         as a .tzx, or the zip it was downloaded in. Put it next to the program, or name it\n\
         on the command line:\n\n    robin path/to/tape.tzx\n\nLooked in:\n",
    );
    for f in folders {
        msg.push_str(&format!("  {}\n", f.display()));
    }
    msg.push_str("\nSee README.txt, or assets/README.md in the repository, for where to find one.");
    msg
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    /// A made-up "tape": the check is the caller's, so any bytes will do.
    const GOOD: &[u8] = b"the tape";
    const BAD: &[u8] = b"some other tape";

    fn accept(bytes: &[u8]) -> bool {
        bytes == GOOD
    }

    /// A fresh, empty folder of the test's own.
    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("robin-tape-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn zip_with(path: &Path, entries: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in entries {
            z.start_file(*name, options).unwrap();
            z.write_all(bytes).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn any_name_in_any_case() {
        for name in ["Robin Of The Wood - 128k.tzx", "ROBIN.TZX", "x.Tzx"] {
            let dir = folder(&format!("case-{name}"));
            fs::write(dir.join(name), GOOD).unwrap();
            let tape = find(std::slice::from_ref(&dir), accept)
                .unwrap_or_else(|| panic!("{name} not found"));
            assert_eq!(tape.bytes, GOOD);
        }
    }

    #[test]
    fn zips_are_looked_in() {
        let dir = folder("zip");
        zip_with(&dir.join("Robin.zip"), &[("ROBIN128.TZX", GOOD)]);
        let tape = find(std::slice::from_ref(&dir), accept).unwrap();
        assert_eq!(tape.bytes, GOOD);
        assert_eq!(tape.from, dir.join("Robin.zip"));
    }

    #[test]
    fn other_kinds_are_not_looked_at() {
        let dir = folder("other-kinds");
        fs::write(dir.join("robin.tap"), GOOD).unwrap();
        fs::write(dir.join("robin.z80"), GOOD).unwrap();
        assert!(find(&[dir], accept).is_none());
    }

    #[test]
    fn a_wrong_file_does_not_hide_a_good_one() {
        let dir = folder("wrong-first");
        fs::write(dir.join("a.tzx"), BAD).unwrap();
        fs::write(dir.join("b.tzx"), GOOD).unwrap();
        assert_eq!(
            find(std::slice::from_ref(&dir), accept).unwrap().from,
            dir.join("b.tzx")
        );
    }

    #[test]
    fn tapes_before_zips_and_folders_in_order() {
        let first = folder("order-first");
        let second = folder("order-second");
        zip_with(&first.join("a.zip"), &[("x.tzx", GOOD)]);
        fs::write(first.join("z.tzx"), GOOD).unwrap();
        fs::write(second.join("a.tzx"), GOOD).unwrap();
        let tape = find(&[first.clone(), second], accept).unwrap();
        assert_eq!(tape.from, first.join("z.tzx"));
    }

    #[test]
    fn what_a_refusal_says() {
        let dir = folder("refusals");
        let wrong = dir.join("wrong.tzx");
        fs::write(&wrong, BAD).unwrap();
        assert_eq!(load(&wrong, accept).err(), Some(Refused::NotTheTape));
        let wrong_zip = dir.join("wrong.zip");
        zip_with(&wrong_zip, &[("ROBIN.TZX", BAD)]);
        assert_eq!(load(&wrong_zip, accept).err(), Some(Refused::NotTheTape));
        let empty_zip = dir.join("empty.zip");
        zip_with(&empty_zip, &[("README.TXT", b"hello")]);
        assert_eq!(load(&empty_zip, accept).err(), Some(Refused::NoTapeInZip));
        let not_zip = dir.join("broken.zip");
        fs::write(&not_zip, b"not a zip").unwrap();
        assert!(matches!(
            load(&not_zip, accept),
            Err(Refused::Unreadable(_))
        ));
        assert!(matches!(
            load(&dir.join("missing.tzx"), accept),
            Err(Refused::Unreadable(_))
        ));
    }

    #[test]
    fn keeping_creates_the_folder() {
        let dir = folder("keep").join("deeper").join(APP);
        let path = keep_in(&dir, GOOD).unwrap();
        assert_eq!(path, dir.join(KEPT));
        assert_eq!(fs::read(&path).unwrap(), GOOD);
        assert_eq!(find(&[dir], accept).unwrap().bytes, GOOD);
    }
}
