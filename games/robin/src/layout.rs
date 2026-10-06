//! Where the tape puts the program, and how it starts: the facts the game
//! needs to load itself without a ROM, and the checks need to know where
//! the original begins. Each is from `docs/re/robin.md` (*The loader*,
//! *Where the program starts*), confirmed by booting the original from its
//! tape in the reference machine (`tests/boot.rs`).

/// The tape's blocks before the game's own: the BASIC loader and `r1`, each
/// with its header. They load through the ROM's `LOAD`, into BASIC's memory.
pub const LOADER_BLOCKS: usize = 4;

/// One of the game's headerless blocks: how many bytes it holds, the value
/// `r1` writes to port `0x7FFD` before loading it, and where in the address
/// space it goes.
pub struct Block {
    pub len: usize,
    pub port_7ffd: u8,
    pub at: u16,
}

/// The game's four blocks, in tape order, after [`LOADER_BLOCKS`].
pub const GAME_BLOCKS: [Block; 4] = [
    // The loading screen, into bank 7 at 0xC000, which is also shown.
    Block {
        len: 6912,
        port_7ffd: 0x1F,
        at: 0xC000,
    },
    // The main program: the rest of bank 5, all of bank 2, and the first
    // 0x2200 bytes of bank 0.
    Block {
        len: 34560,
        port_7ffd: 0x18,
        at: 0x5B00,
    },
    // Into bank 6.
    Block {
        len: 4096,
        port_7ffd: 0x1E,
        at: 0xC000,
    },
    // All of bank 4.
    Block {
        len: 16384,
        port_7ffd: 0x1C,
        at: 0xC000,
    },
];

/// Where `r1` hands over once the last block is in: a stub that pages again
/// and jumps to [`START`].
pub const ENTRY_PC: u16 = 0x5B00;
/// The stack `r1` set up for itself, below it.
pub const ENTRY_SP: u16 = 0x508C;
/// Port `0x7FFD` at the hand-over: bank 0 at `0xC000`, the screen in bank 5,
/// ROM 1, paging not locked.
pub const ENTRY_7FFD: u8 = 0x10;
/// Interrupt mode at the hand-over, with interrupts disabled.
pub const ENTRY_IM: u8 = 1;
/// The program's own start, where [`ENTRY_PC`]'s stub jumps.
pub const START: u16 = 0xBE4A;

/// The bank in 0xC000..0xFFFF for a value of port `0x7FFD`.
const fn bank_at_c000(port_7ffd: u8) -> usize {
    (port_7ffd & 7) as usize
}

/// The 128K bank and offset an address goes to, with `port_7ffd` paged.
const fn bank_of(addr: u16, port_7ffd: u8) -> (usize, usize) {
    let bank = match addr >> 14 {
        1 => 5,
        2 => 2,
        _ => bank_at_c000(port_7ffd),
    };
    (bank, addr as usize & 0x3FFF)
}

/// The 128K's eight banks (bank 0 first) with the game's blocks where the
/// facts say, from the tape's blocks and nothing else: what the game builds
/// for itself without a ROM.
///
/// # Errors
///
/// If the tape has too few blocks, or one is not the length the facts say.
pub fn banks_from_tape(blocks: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let mut ram = vec![0u8; 8 * 0x4000];
    for (i, b) in GAME_BLOCKS.iter().enumerate() {
        let n = LOADER_BLOCKS + i;
        // Numbered from 1 in messages, as `docs/re/robin.md` lists them.
        let block = blocks
            .get(n)
            .ok_or_else(|| format!("the tape has no block {}", n + 1))?;
        // Flag, data, checksum.
        let data = block.get(1..block.len().saturating_sub(1)).unwrap_or(&[]);
        if data.len() != b.len {
            return Err(format!(
                "block {} holds {} bytes, not {}",
                n + 1,
                data.len(),
                b.len
            ));
        }
        for (k, &v) in data.iter().enumerate() {
            let (bank, off) = bank_of(b.at.wrapping_add(k as u16), b.port_7ffd);
            ram[bank * 0x4000 + off] = v;
        }
    }
    Ok(ram)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_block_spans_banks_5_2_and_0() {
        let main = &GAME_BLOCKS[1];
        let last = main.at.wrapping_add(main.len as u16 - 1);
        assert_eq!(bank_of(main.at, main.port_7ffd), (5, 0x1B00));
        assert_eq!(bank_of(0x8000, main.port_7ffd), (2, 0));
        assert_eq!(bank_of(last, main.port_7ffd), (0, 0x21FF));
    }

    #[test]
    fn each_block_goes_to_its_bank() {
        let blocks: Vec<Vec<u8>> = (0..LOADER_BLOCKS)
            .map(|_| vec![0, 0])
            .chain(GAME_BLOCKS.iter().enumerate().map(|(i, b)| {
                let mut v = vec![0xFF];
                v.extend(std::iter::repeat_n(i as u8 + 1, b.len));
                v.push(0);
                v
            }))
            .collect();
        let ram = banks_from_tape(&blocks).expect("builds");
        let at = |bank: usize, off: usize| ram[bank * 0x4000 + off];
        assert_eq!(at(7, 0), 1, "the loading screen in bank 7");
        assert_eq!((at(5, 0x1B00), at(2, 0), at(0, 0x21FF)), (2, 2, 2));
        assert_eq!(at(0, 0x2200), 0, "the main block ends there");
        assert_eq!((at(6, 0), at(6, 0xFFF), at(6, 0x1000)), (3, 3, 0));
        assert_eq!((at(4, 0), at(4, 0x3FFF)), (4, 4));
        assert!(banks_from_tape(&blocks[..6]).is_err());
    }
}
