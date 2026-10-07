//! The player's tape, read at startup: the one way the game gets its data.
//!
//! Nothing here is original data. This module knows which dump it reads
//! (by SHA-1) and where each of its blocks goes ([`crate::layout`]), and
//! builds the 128K's banks from the tape alone, as the original's loader
//! leaves them, with no ROM. Each subsystem parses its tables from these
//! banks, knowing only where they are.

use std::path::Path;

use zx_core::sha1::sha1_hex;

use crate::layout::{self, GAME_BLOCKS, LOADER_BLOCKS};
pub use crate::map::Map;

/// SHA-1 of the one dump this project supports: the 128K release as a TZX
/// file (`docs/re/robin.md`, *The tape*). Any other file is refused.
pub const TAPE_SHA1: &str = "2aad3402cdc08907000900c5da8c29eeb2f48c9e";

/// The size of a 128K memory bank.
pub const BANK: usize = 0x4000;

/// Whether `bytes` are the supported dump.
#[must_use]
pub fn is_the_tape(bytes: &[u8]) -> bool {
    sha1_hex(bytes) == TAPE_SHA1
}

/// The program as the tape loads it: the 128K's eight banks, bank 0 first.
/// Banks 1 and 3, and whatever the blocks do not cover, are zero.
pub struct Assets {
    banks: Box<[[u8; BANK]; 8]>,
    loading_screen: Box<[u8; 6912]>,
    map: Map,
}

impl Assets {
    /// Bank `n`, 0 to 7.
    ///
    /// # Panics
    ///
    /// If `n` is not a bank.
    #[must_use]
    pub fn bank(&self, n: usize) -> &[u8; BANK] {
        assert!(n < 8, "there is no bank {n}");
        &self.banks[n]
    }

    /// The byte the processor sees at `addr` with `bank_at_c000` paged at
    /// `0xC000`: bank 5 at `0x4000`, bank 2 at `0x8000`.
    ///
    /// # Panics
    ///
    /// Below `0x4000`, where the original has its ROM: the game has none.
    #[must_use]
    pub fn read(&self, addr: u16, bank_at_c000: usize) -> u8 {
        let bank = match addr >> 14 {
            0 => panic!("{addr:#06x} is in the ROM, and the game has no ROM"),
            1 => 5,
            2 => 2,
            _ => bank_at_c000,
        };
        self.bank(bank)[usize::from(addr) % BANK]
    }

    /// The picture shown while the game loads, as screen memory: 6,144
    /// bytes of pixels, then 768 of attributes. The first block puts it in
    /// bank 7 at `0xC000`, where the 128K shows it (`docs/re/robin.md`, *The
    /// loader*).
    #[must_use]
    pub fn loading_screen(&self) -> &[u8; 6912] {
        &self.loading_screen
    }

    /// Assets from banks already built, for the checks' own made-up states.
    /// The game reads its tape with [`read_game`].
    #[doc(hidden)]
    #[must_use]
    pub fn from_banks(banks: Box<[[u8; BANK]; 8]>) -> Assets {
        let mut loading_screen = Box::new([0u8; 6912]);
        loading_screen.copy_from_slice(&banks[7][..6912]);
        let mut assets = Assets {
            banks,
            loading_screen,
            map: Map::parse(|_| 0),
        };
        assets.map = Map::parse(|a| if a < 0x4000 { 0 } else { assets.read(a, 0) });
        assets
    }

    /// The map, parsed from the tape.
    #[must_use]
    pub fn map(&self) -> &Map {
        &self.map
    }
}

/// Reads the player's own copy of the game from `path`.
///
/// # Errors
///
/// If the file cannot be read, or is not the supported tape.
pub fn read_game(path: &Path) -> Result<Assets, String> {
    let bytes = std::fs::read(path).map_err(|e| {
        format!(
            "cannot read {}: {e}\nRobin of the Wood needs your own copy of the original \
             game: the 128K release, as a TZX file. See the README.",
            path.display()
        )
    })?;
    read_tape(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Reads the supported tape from its bytes.
///
/// # Errors
///
/// If `bytes` are not the supported tape.
pub fn read_tape(bytes: &[u8]) -> Result<Assets, String> {
    if !is_the_tape(bytes) {
        return Err(format!(
            "this is not the supported tape. Its SHA-1 is {}, and this version reads only \
             {TAPE_SHA1}: the 128K release of Robin of the Wood, as a TZX file. \
             assets/README.md and the README say which dump that is.",
            sha1_hex(bytes)
        ));
    }
    parse(bytes)
}

/// What follows the SHA-1 check: the tape's blocks, and the banks built from
/// them. Separate so the checks can feed it tapes made up for the test.
fn parse(bytes: &[u8]) -> Result<Assets, String> {
    let blocks = zx_core::tape::load_tzx(bytes)?;
    let expected = LOADER_BLOCKS + GAME_BLOCKS.len();
    if blocks.len() != expected {
        return Err(format!(
            "the tape has {} blocks, not {expected}",
            blocks.len()
        ));
    }
    // Numbered from 1, as `docs/re/robin.md` lists them.
    for (i, b) in blocks.iter().enumerate().skip(LOADER_BLOCKS) {
        if b.first() != Some(&0xFF) {
            return Err(format!("block {} is a header, not data", i + 1));
        }
    }
    let ram = layout::banks_from_tape(&blocks)?;
    let mut banks = Box::new([[0u8; BANK]; 8]);
    for (bank, from) in banks.iter_mut().zip(ram.as_chunks::<BANK>().0) {
        *bank = *from;
    }
    let mut loading_screen = Box::new([0u8; 6912]);
    loading_screen.copy_from_slice(&banks[7][..6912]);
    let mut assets = Assets {
        banks,
        loading_screen,
        map: Map::parse(|_| 0),
    };
    assets.map = Map::parse(|a| if a < 0x4000 { 0 } else { assets.read(a, 0) });
    Ok(assets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A made-up tape with the supported one's shape: the loader's four
    /// blocks, then the game's four, each holding a pattern that says which
    /// block and which byte it is.
    fn blocks() -> Vec<Vec<u8>> {
        let loader = [(0x00, 17), (0xFF, 60), (0x00, 17), (0xFF, 256)];
        let loader = loader
            .iter()
            .map(|&(flag, len)| block(flag, &vec![0x55; len]));
        let game = GAME_BLOCKS.iter().enumerate().map(|(i, b)| {
            let data: Vec<u8> = (0..b.len).map(|k| pattern(i, k)).collect();
            block(0xFF, &data)
        });
        loader.chain(game).collect()
    }

    /// Byte `k` of game block `i` in [`blocks`].
    fn pattern(i: usize, k: usize) -> u8 {
        (k.wrapping_mul(7) ^ (k >> 8) ^ (i << 6)) as u8
    }

    /// A tape block: its flag, its data, and a checksum (not checked).
    fn block(flag: u8, data: &[u8]) -> Vec<u8> {
        let mut b = vec![flag];
        b.extend_from_slice(data);
        b.push(0);
        b
    }

    /// The blocks as a TZX file of standard-speed blocks.
    fn tzx(blocks: &[Vec<u8>]) -> Vec<u8> {
        let mut t = b"ZXTape!\x1A\x01\x14".to_vec();
        for b in blocks {
            t.push(0x10);
            t.extend_from_slice(&1000u16.to_le_bytes());
            t.extend_from_slice(&(b.len() as u16).to_le_bytes());
            t.extend_from_slice(b);
        }
        t
    }

    #[test]
    fn anything_but_the_dump_is_refused_by_its_sha1() {
        assert!(!is_the_tape(b""));
        let made_up = tzx(&blocks());
        let e = read_tape(&made_up).err().expect("refused");
        for needle in [
            sha1_hex(&made_up).as_str(),
            TAPE_SHA1,
            "128K",
            "TZX",
            "assets/README.md",
        ] {
            assert!(e.contains(needle), "the message has no {needle:?}: {e}");
        }
        let e = read_game(Path::new("no/such/robin.tzx"))
            .err()
            .expect("refused");
        assert!(
            e.contains("no/such/robin.tzx") && e.contains("your own copy"),
            "{e}"
        );
    }

    #[test]
    fn every_block_lands_in_its_bank() {
        let a = parse(&tzx(&blocks())).expect("parses");
        let (screen, main, bank6, bank4) = (0, 1, 2, 3);
        assert_eq!(a.loading_screen()[0], pattern(screen, 0));
        assert_eq!(a.loading_screen()[6911], pattern(screen, 6911));
        assert_eq!(a.bank(7)[6911], pattern(screen, 6911));
        // The main block runs from 0x5B00 through banks 5 and 2 into bank 0.
        assert_eq!(a.read(0x5B00, 0), pattern(main, 0));
        assert_eq!(a.bank(5)[0x1B00], pattern(main, 0));
        assert_eq!(a.read(0x8000, 6), pattern(main, 0x2500));
        assert_eq!(a.bank(2)[0], pattern(main, 0x2500));
        assert_eq!(a.read(0xC000, 0), pattern(main, 0x6500));
        assert_eq!(a.read(0xE1FF, 0), pattern(main, 34559));
        assert_eq!(a.read(0xE200, 0), 0, "the main block ends at 0xE1FF");
        assert_eq!(a.read(0xC000, 6), pattern(bank6, 0));
        assert_eq!(a.read(0xCFFF, 6), pattern(bank6, 4095));
        assert_eq!(a.read(0xD000, 6), 0, "bank 6 holds 4,096 bytes");
        assert_eq!(a.read(0xC000, 4), pattern(bank4, 0));
        assert_eq!(a.read(0xFFFF, 4), pattern(bank4, 0x3FFF));
        assert!(
            a.bank(1).iter().chain(a.bank(3)).all(|&b| b == 0),
            "banks 1 and 3 are never loaded"
        );
        assert_eq!(
            a.bank(5)[..0x1B00].iter().filter(|&&b| b != 0).count(),
            0,
            "bank 5 below 0x5B00 is not loaded"
        );
    }

    #[test]
    fn a_malformed_tape_is_refused_by_name() {
        let refused = |blocks: Vec<Vec<u8>>, needle: &str| {
            let e = parse(&tzx(&blocks)).err().expect("refused");
            assert!(e.contains(needle), "expected {needle:?}: {e}");
        };
        let mut few = blocks();
        few.pop();
        refused(few, "7 blocks");
        let mut many = blocks();
        many.push(block(0xFF, &[1]));
        refused(many, "9 blocks");
        let mut short = blocks();
        short[LOADER_BLOCKS + 2].remove(1);
        refused(short, "block 7 holds 4095 bytes");
        let mut header = blocks();
        header[LOADER_BLOCKS + 1][0] = 0x00;
        refused(header, "block 6 is a header");

        let t = tzx(&blocks());
        let e = parse(&t[..t.len() - 1]).err().expect("refused");
        assert!(e.contains("runs off the end"), "{e}");
        let e = parse(b"not a tape at all").err().expect("refused");
        assert!(e.contains("not a TZX file"), "{e}");
    }

    #[test]
    #[should_panic(expected = "no ROM")]
    fn there_is_no_rom_to_read() {
        let _ = parse(&tzx(&blocks())).expect("parses").read(0x0000, 0);
    }

    /// The supported tape from `assets/`, if it is there.
    fn local_tape() -> Option<Vec<u8>> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| std::fs::read(e.path()).ok())
            .find(|b| is_the_tape(b))
    }

    /// The tape's blocks are the eight `docs/re/robin.md` lists: the BASIC
    /// loader and `r1`, each with its header, then four headerless blocks.
    /// Each length includes the flag and checksum.
    #[test]
    fn the_tape_has_the_blocks_the_notes_list() {
        let Some(tape) = local_tape() else {
            println!("skipped: no supported tape in assets/");
            return;
        };
        let blocks = zx_core::tape::load_tzx(&tape).expect("the tape reads");
        let lengths: Vec<usize> = blocks.iter().map(Vec::len).collect();
        assert_eq!(lengths, [19, 62, 19, 258, 6914, 34562, 4098, 16386]);
        let flags: Vec<u8> = blocks.iter().map(|b| b[0]).collect();
        assert_eq!(flags, [0x00, 0xFF, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
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

    /// The real tape's map is the shape `docs/re/robin.md` (*The map*) gives.
    #[test]
    fn the_local_tapes_map() {
        let Some(tape) = local_tape() else {
            println!("skipped: no supported tape in assets/");
            return;
        };
        let map = read_tape(&tape).expect("reads").map().clone();
        let count = |lists: &[Vec<crate::map::Item>]| lists.iter().map(Vec::len).sum::<usize>();
        assert_eq!(map.locations.len(), 320);
        assert_eq!(map.locations.iter().filter(|l| l.mirrored).count(), 116);
        assert_eq!((map.layouts.len(), count(&map.layouts)), (128, 1129));
        assert_eq!((map.extras.len(), count(&map.extras)), (256, 531));
        assert_eq!(map.extras.iter().filter(|e| !e.is_empty()).count(), 134);
        assert_eq!(map.blocks.len(), 84);
        assert!(map.specials.iter().all(|s| s.len() == 4));
        let a = read_tape(&tape).expect("reads");
        for (n, b) in map.blocks.iter().enumerate() {
            let header = a.read(b.at, 0);
            assert_eq!(
                header & 0xB8,
                0,
                "block {n}: bits 3-5 clear, and unmirrored as the tape stores it"
            );
        }
    }

    /// The real tape reads, and its loading picture is its first game block.
    /// (Every byte of the banks is checked against the booted original by
    /// `tests/boot.rs`.)
    #[test]
    fn the_local_tape_reads_with_its_loading_picture() {
        let Some(tape) = local_tape() else {
            println!("skipped: no supported tape in assets/");
            return;
        };
        let a = read_tape(&tape).expect("the supported tape reads");
        let blocks = zx_core::tape::load_tzx(&tape).expect("the tape reads");
        assert_eq!(&a.loading_screen()[..], &blocks[LOADER_BLOCKS][1..=6912]);
    }
}
