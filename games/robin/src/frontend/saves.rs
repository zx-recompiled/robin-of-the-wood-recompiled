//! Save states (#92, #101): the quick slot F5 and F9 use, and the game saved
//! on quit, as files in the program's data folder. A save holds the game's
//! memory, which is the player's own copy of the game, so it stays on their
//! machine like the tape; nothing here is ever committed.

use std::path::PathBuf;

use robin::assets::BANK;
use robin::session::Saved;

use super::journal::Journal;

/// What a save file starts with, and its version.
const MAGIC: &[u8; 11] = b"ROBIN-SAVE\x01";

/// The two slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Quick,
    OnQuit,
}

fn path(slot: Slot) -> Option<PathBuf> {
    let name = match slot {
        Slot::Quick => "quick.sav",
        Slot::OnQuit => "on-quit.sav",
    };
    super::tape::data_folder().map(|d| d.join(name))
}

/// The bytes of a save: the game, then the journal.
#[must_use]
pub fn encode(saved: &Saved, journal: &Journal) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    for bank in saved.memory.iter() {
        out.extend_from_slice(bank);
    }
    out.extend_from_slice(&saved.random.to_le_bytes());
    out.extend_from_slice(&saved.ay);
    out.extend_from_slice(&[saved.ay_latch, saved.border]);
    out.extend_from_slice(&saved.frames.to_le_bytes());
    out.extend_from_slice(&journal.to_bytes());
    out
}

/// A save's game and journal, or `None` if the bytes aren't one.
#[must_use]
pub fn decode(bytes: &[u8]) -> Option<(Saved, Journal)> {
    let rest = bytes.strip_prefix(MAGIC)?;
    let (banks, rest) = rest.split_at_checked(8 * BANK)?;
    let mut memory = Box::new([[0u8; BANK]; 8]);
    for (bank, from) in memory.iter_mut().zip(banks.chunks(BANK)) {
        bank.copy_from_slice(from);
    }
    let (random, rest) = rest.split_at_checked(8)?;
    let (ay, rest) = rest.split_at_checked(16)?;
    let (&[ay_latch, border], rest) = rest.split_first_chunk::<2>()?;
    let (frames, rest) = rest.split_at_checked(8)?;
    let saved = Saved {
        memory,
        random: u64::from_le_bytes(random.try_into().ok()?),
        ay: ay.try_into().ok()?,
        ay_latch,
        border,
        frames: u64::from_le_bytes(frames.try_into().ok()?),
    };
    Some((saved, Journal::from_bytes(rest)?))
}

/// Writes a save to `slot`.
///
/// # Errors
///
/// If there's no data folder, or the file can't be written.
pub fn write(slot: Slot, saved: &Saved, journal: &Journal) -> Result<(), String> {
    let file = path(slot).ok_or("no data folder to save in")?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&file, encode(saved, journal)).map_err(|e| e.to_string())
}

/// The save in `slot`, if there's one that reads.
#[must_use]
pub fn read(slot: Slot) -> Option<(Saved, Journal)> {
    decode(&std::fs::read(path(slot)?).ok()?)
}

/// Forgets the save in `slot`.
pub fn remove(slot: Slot) {
    if let Some(file) = path(slot) {
        let _ = std::fs::remove_file(file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_reads_back_as_it_was_written() {
        let mut memory = Box::new([[0u8; BANK]; 8]);
        memory[3][100] = 0x5A;
        let saved = Saved {
            memory,
            random: 0x1234_5678_9ABC_DEF1,
            ay: [7; 16],
            ay_latch: 14,
            border: 2,
            frames: 9876,
        };
        let journal = Journal::of([0x10, 0x11], 0x12).with([Some(0x13), None, None], [None; 3]);
        let (back, j) = decode(&encode(&saved, &journal)).expect("reads");
        assert_eq!(back, saved);
        assert_eq!(
            (
                j.count(),
                j.visited(0x11),
                j.last_seen(robin::places::Character::Bishop)
            ),
            (3, true, Some(0x13))
        );
        assert!(decode(b"not a save").is_none());
    }
}
