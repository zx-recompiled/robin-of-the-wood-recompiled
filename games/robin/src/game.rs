//! The game's state: what the rewrite works on.
//!
//! Each part the rewrite understands is a typed field, read from where the
//! original keeps it (`docs/re/robin.md`). Everything else is the rest of
//! RAM, carried along untouched. [`Game::from_memory`] reads a state from
//! the 128K's banks and [`Game::to_memory`] writes it back, so the checks can
//! compare all of memory between the original and the rewrite.

use crate::assets::BANK;

/// Where an address's byte is kept.
enum Loc {
    Screen(usize),
    Pixels(usize),
    Attrs(usize),
    Changed(usize),
    Mirror(usize),
    /// A row table entry, and whether it is the high byte.
    Row(usize, bool),
    /// The printer's cell, and whether it is the high byte.
    Cell(bool),
    Recorded(usize),
    Mode,
    Mirrored,
    Replace,
    ColumnOffset,
    AttrPage,
    AttrFlag,
    MirrorLookup,
    Rest(usize, usize),
}

/// Where the byte at `addr` is kept, with bank 0 paged at `0xC000`.
///
/// # Panics
///
/// Below `0x4000`, the ROM.
fn locate(addr: u16) -> Loc {
    let bank = match addr >> 14 {
        0 => panic!("{addr:#06x} is in the ROM, and the game has no ROM"),
        1 => 5,
        2 => 2,
        _ => 0,
    };
    locate_place(bank, usize::from(addr) % BANK)
}

/// Where the byte at `offset` in `bank` is kept.
fn locate_place(bank: usize, offset: usize) -> Loc {
    let at = |part: At, len: usize| {
        (part.bank == bank && (part.offset..part.offset + len).contains(&offset))
            .then(|| offset - part.offset)
    };
    if let Some(i) = at(SCREEN, 6912) {
        Loc::Screen(i)
    } else if let Some(i) = at(PIXELS, 0x1200) {
        Loc::Pixels(i)
    } else if let Some(i) = at(ATTRS, 0x240) {
        Loc::Attrs(i)
    } else if let Some(i) = at(CHANGED, 0x240) {
        Loc::Changed(i)
    } else if let Some(i) = at(MIRROR, 256) {
        Loc::Mirror(i)
    } else if let Some(i) = at(ROWS, 384) {
        Loc::Row(i / 2, i % 2 == 1)
    } else if let Some(i) = at(PRINT_CELL, 2) {
        Loc::Cell(i == 1)
    } else if let Some(i) = at(PRINT_RECORDED, 27) {
        Loc::Recorded(i)
    } else if at(PRINT_MODE, 1).is_some() {
        Loc::Mode
    } else if at(PRINT_MIRRORED, 1).is_some() {
        Loc::Mirrored
    } else if at(PRINT_REPLACE, 1).is_some() {
        Loc::Replace
    } else if at(PRINT_COLUMN_OFFSET, 1).is_some() {
        Loc::ColumnOffset
    } else if at(PRINT_ATTR_PAGE, 1).is_some() {
        Loc::AttrPage
    } else if at(PRINT_ATTR_FLAG, 1).is_some() {
        Loc::AttrFlag
    } else if at(PRINT_MIRROR_LOOKUP, 1).is_some() {
        Loc::MirrorLookup
    } else {
        Loc::Rest(bank, offset)
    }
}

/// A place in RAM: a bank and an offset in it.
#[derive(Clone, Copy)]
struct At {
    bank: usize,
    offset: usize,
}

/// `addr` in the bank paged at `0xC000`.
const fn bank0(addr: u16) -> At {
    At {
        bank: 0,
        offset: addr as usize - 0xC000,
    }
}

/// The screen, in bank 5 at `0x4000`.
const SCREEN: At = At { bank: 5, offset: 0 };
/// The play area's back buffer, attribute buffer and changed-cell map.
const PIXELS: At = bank0(0xEB00);
const ATTRS: At = bank0(0xE800);
const CHANGED: At = bank0(0xE500);
/// The tables built at start-up.
const MIRROR: At = bank0(0xFD00);
const ROWS: At = bank0(0xFE00);
/// The text printer's variables.
const PRINT_MODE: At = bank0(0xD6CE);
const PRINT_MIRRORED: At = bank0(0xD6CD);
const PRINT_REPLACE: At = bank0(0xD6CF);
const PRINT_CELL: At = bank0(0xD4F4);
const PRINT_RECORDED: At = bank0(0xD457);
/// The printer's settings kept in its own code: the operands it rewrites.
const PRINT_COLUMN_OFFSET: At = bank0(0xD629);
const PRINT_ATTR_PAGE: At = bank0(0xD637);
const PRINT_ATTR_FLAG: At = bank0(0xD667);
const PRINT_MIRROR_LOOKUP: At = bank0(0xD5AB);

/// The play area: 18 character rows of 28 columns, drawn off the screen and
/// copied to it (`docs/re/robin.md`, *The play area and its buffers*).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlayArea {
    /// The back buffer: one 256-byte block per character row, one 32-byte
    /// line per pixel row.
    pub pixels: Box<[u8; 0x1200]>,
    /// The attribute buffer, laid out as the screen's. Bit 7 is the game's
    /// own flag.
    pub attrs: Box<[u8; 0x240]>,
    /// The changed-cell map: non-zero means redraw, and is the attribute to
    /// use where the attribute buffer holds zero.
    pub changed: Box<[u8; 0x240]>,
}

/// The text printer's state (`docs/re/robin.md`, *The text printer*).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Printer {
    /// The mode of the last string printed.
    pub mode: u8,
    /// Non-zero while mirroring.
    pub mirrored: u8,
    /// Non-zero to replace what is there for the next string, not XOR.
    pub replace: u8,
    /// Where the last attribute string reached, as an offset into the
    /// changed-cell map.
    pub cell: u16,
    /// The recorded messages: nine of mode, and position, `0xFF` when free.
    pub recorded: [u8; 27],
    /// The column offset of the attributes (0, or 2 into the play area).
    pub column_offset: u8,
    /// Where the attributes go: `0x58` for the screen, `0xE8` for the
    /// attribute buffer.
    pub attr_page: u8,
    /// What is ORed into each attribute: 0, or `0x80`.
    pub attr_flag: u8,
    /// The last glyph byte looked up in the mirror table.
    pub mirror_lookup: u8,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Game {
    /// The screen: 6,144 bytes of pixels, then 768 of attributes.
    pub screen: Box<[u8; 6912]>,
    pub play: PlayArea,
    /// Each byte with its bits reversed, built at start-up.
    pub mirror: [u8; 256],
    /// The screen address of each pixel row, built at start-up.
    pub rows: [u16; 192],
    pub printer: Printer,
    /// The rest of RAM, banks 0 to 7, as it was read: what the rewrite does
    /// not model yet.
    rest: Box<[[u8; BANK]; 8]>,
}

fn bytes<const N: usize>(banks: &[[u8; BANK]; 8], at: At) -> [u8; N] {
    banks[at.bank][at.offset..][..N]
        .try_into()
        .expect("N bytes from a bank")
}

fn boxed<const N: usize>(banks: &[[u8; BANK]; 8], at: At) -> Box<[u8; N]> {
    Box::new(bytes(banks, at))
}

fn put(banks: &mut [[u8; BANK]; 8], at: At, v: &[u8]) {
    banks[at.bank][at.offset..][..v.len()].copy_from_slice(v);
}

impl Game {
    /// The name of the part of the state at `offset` in `bank`, for saying
    /// where two states differ: a typed part, or the rest of RAM.
    #[must_use]
    pub fn part_at(bank: usize, offset: usize) -> &'static str {
        match locate_place(bank, offset) {
            Loc::Screen(_) => "the screen",
            Loc::Pixels(_) => "the back buffer",
            Loc::Attrs(_) => "the attribute buffer",
            Loc::Changed(_) => "the changed-cell map",
            Loc::Mirror(_) => "the mirror table",
            Loc::Row(..) => "the row table",
            Loc::Cell(_) => "printer.cell",
            Loc::Recorded(_) => "printer.recorded",
            Loc::Mode => "printer.mode",
            Loc::Mirrored => "printer.mirrored",
            Loc::Replace => "printer.replace",
            Loc::ColumnOffset => "printer.column_offset",
            Loc::AttrPage => "printer.attr_page",
            Loc::AttrFlag => "printer.attr_flag",
            Loc::MirrorLookup => "printer.mirror_lookup",
            Loc::Rest(..) => "the rest of RAM",
        }
    }

    /// The byte the processor sees at `addr` with bank 0 paged at `0xC000`,
    /// as it is whenever the screen code runs.
    ///
    /// # Panics
    ///
    /// Below `0x4000`, where the original has its ROM: the game has none.
    #[must_use]
    pub fn read(&self, addr: u16) -> u8 {
        let p = &self.printer;
        match locate(addr) {
            Loc::Screen(i) => self.screen[i],
            Loc::Pixels(i) => self.play.pixels[i],
            Loc::Attrs(i) => self.play.attrs[i],
            Loc::Changed(i) => self.play.changed[i],
            Loc::Mirror(i) => self.mirror[i],
            Loc::Row(r, hi) => self.rows[r].to_le_bytes()[usize::from(hi)],
            Loc::Cell(hi) => p.cell.to_le_bytes()[usize::from(hi)],
            Loc::Recorded(i) => p.recorded[i],
            Loc::Mode => p.mode,
            Loc::Mirrored => p.mirrored,
            Loc::Replace => p.replace,
            Loc::ColumnOffset => p.column_offset,
            Loc::AttrPage => p.attr_page,
            Loc::AttrFlag => p.attr_flag,
            Loc::MirrorLookup => p.mirror_lookup,
            Loc::Rest(bank, i) => self.rest[bank][i],
        }
    }

    /// Writes the byte at `addr`, with bank 0 paged at `0xC000`. Writes below
    /// `0x4000`, to the ROM, go nowhere, as on the machine.
    pub fn write(&mut self, addr: u16, v: u8) {
        if addr < 0x4000 {
            return;
        }
        let set_half = |w: &mut u16, hi: bool| {
            let mut b = w.to_le_bytes();
            b[usize::from(hi)] = v;
            *w = u16::from_le_bytes(b);
        };
        let p = &mut self.printer;
        match locate(addr) {
            Loc::Screen(i) => self.screen[i] = v,
            Loc::Pixels(i) => self.play.pixels[i] = v,
            Loc::Attrs(i) => self.play.attrs[i] = v,
            Loc::Changed(i) => self.play.changed[i] = v,
            Loc::Mirror(i) => self.mirror[i] = v,
            Loc::Row(r, hi) => set_half(&mut self.rows[r], hi),
            Loc::Cell(hi) => set_half(&mut p.cell, hi),
            Loc::Recorded(i) => p.recorded[i] = v,
            Loc::Mode => p.mode = v,
            Loc::Mirrored => p.mirrored = v,
            Loc::Replace => p.replace = v,
            Loc::ColumnOffset => p.column_offset = v,
            Loc::AttrPage => p.attr_page = v,
            Loc::AttrFlag => p.attr_flag = v,
            Loc::MirrorLookup => p.mirror_lookup = v,
            Loc::Rest(bank, i) => self.rest[bank][i] = v,
        }
    }

    /// Reads the state from the 128K's eight banks, bank 0 first.
    #[must_use]
    pub fn from_memory(banks: &[[u8; BANK]; 8]) -> Game {
        let byte = |at: At| banks[at.bank][at.offset];
        let word = |at: At| u16::from_le_bytes(bytes(banks, at));
        let rows = std::array::from_fn(|i| {
            word(At {
                offset: ROWS.offset + 2 * i,
                ..ROWS
            })
        });
        Game {
            screen: boxed(banks, SCREEN),
            play: PlayArea {
                pixels: boxed(banks, PIXELS),
                attrs: boxed(banks, ATTRS),
                changed: boxed(banks, CHANGED),
            },
            mirror: bytes(banks, MIRROR),
            rows,
            printer: Printer {
                mode: byte(PRINT_MODE),
                mirrored: byte(PRINT_MIRRORED),
                replace: byte(PRINT_REPLACE),
                cell: word(PRINT_CELL),
                recorded: bytes(banks, PRINT_RECORDED),
                column_offset: byte(PRINT_COLUMN_OFFSET),
                attr_page: byte(PRINT_ATTR_PAGE),
                attr_flag: byte(PRINT_ATTR_FLAG),
                mirror_lookup: byte(PRINT_MIRROR_LOOKUP),
            },
            rest: Box::new(*banks),
        }
    }

    /// Writes the state back over the rest of RAM: the eight banks as the
    /// original would hold them.
    #[must_use]
    pub fn to_memory(&self) -> Box<[[u8; BANK]; 8]> {
        let mut banks = self.rest.clone();
        let b = &mut *banks;
        put(b, SCREEN, &self.screen[..]);
        put(b, PIXELS, &self.play.pixels[..]);
        put(b, ATTRS, &self.play.attrs[..]);
        put(b, CHANGED, &self.play.changed[..]);
        put(b, MIRROR, &self.mirror);
        for (i, r) in self.rows.iter().enumerate() {
            let at = At {
                offset: ROWS.offset + 2 * i,
                ..ROWS
            };
            put(b, at, &r.to_le_bytes());
        }
        let p = &self.printer;
        put(b, PRINT_MODE, &[p.mode]);
        put(b, PRINT_MIRRORED, &[p.mirrored]);
        put(b, PRINT_REPLACE, &[p.replace]);
        put(b, PRINT_CELL, &p.cell.to_le_bytes());
        put(b, PRINT_RECORDED, &p.recorded);
        put(b, PRINT_COLUMN_OFFSET, &[p.column_offset]);
        put(b, PRINT_ATTR_PAGE, &[p.attr_page]);
        put(b, PRINT_ATTR_FLAG, &[p.attr_flag]);
        put(b, PRINT_MIRROR_LOOKUP, &[p.mirror_lookup]);
        banks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eight banks where every byte says where it is.
    fn banks() -> Box<[[u8; BANK]; 8]> {
        let mut b = Box::new([[0u8; BANK]; 8]);
        for (n, bank) in b.iter_mut().enumerate() {
            for (i, v) in bank.iter_mut().enumerate() {
                *v = (i ^ (i >> 8) ^ (n << 5)) as u8;
            }
        }
        b
    }

    #[test]
    fn each_place_is_named_by_its_part() {
        assert_eq!(Game::part_at(5, 0x1AFF), "the screen");
        assert_eq!(Game::part_at(5, 0x1B00), "the rest of RAM");
        assert_eq!(Game::part_at(0, 0x3F7F), "the row table");
        assert_eq!(Game::part_at(0, 0x14F5), "printer.cell");
    }

    #[test]
    fn reading_and_writing_by_address_reach_the_parts() {
        let b = banks();
        let mut g = Game::from_memory(&b);
        assert_eq!(g.read(0x4000), b[5][0]);
        assert_eq!(g.read(0xAAEF), b[2][0x2AEF]);
        assert_eq!(g.read(0xFE01), b[0][0x3E01]);
        g.write(0x5800, 0x47);
        g.write(0xEB02, 0x81);
        g.write(0xFE03, 0x12);
        g.write(0xD4F5, 0x34);
        g.write(0xDDB7, 0x56);
        g.write(0x3000, 0x99);
        assert_eq!(g.screen[6144], 0x47);
        assert_eq!(g.play.pixels[2], 0x81);
        assert_eq!(g.rows[1] >> 8, 0x12);
        assert_eq!(g.printer.cell >> 8, 0x34);
        let m = g.to_memory();
        assert_eq!(
            (m[5][0x1800], m[0][0x2B02], m[0][0x3E03]),
            (0x47, 0x81, 0x12)
        );
        assert_eq!((m[0][0x14F5], m[0][0x1DB7]), (0x34, 0x56));
        assert_eq!(g.read(0xD4F5), 0x34);
    }

    #[test]
    fn writing_back_what_was_read_changes_nothing() {
        let b = banks();
        assert!(Game::from_memory(&b).to_memory() == b);
    }

    #[test]
    fn each_part_is_where_the_notes_say() {
        let b = banks();
        let g = Game::from_memory(&b);
        assert_eq!(g.screen[0], b[5][0]);
        assert_eq!(g.screen[6911], b[5][0x1AFF]);
        assert_eq!(g.play.pixels[0], b[0][0x2B00]);
        assert_eq!(g.play.pixels[0x11FF], b[0][0x3CFF]);
        assert_eq!(g.play.attrs[0], b[0][0x2800]);
        assert_eq!(g.play.changed[0x23F], b[0][0x273F]);
        assert_eq!(g.mirror[255], b[0][0x3DFF]);
        assert_eq!(g.rows[0], u16::from_le_bytes([b[0][0x3E00], b[0][0x3E01]]));
        assert_eq!(
            g.rows[191],
            u16::from_le_bytes([b[0][0x3F7E], b[0][0x3F7F]])
        );
        assert_eq!(g.printer.mode, b[0][0x16CE]);
        assert_eq!(
            g.printer.cell,
            u16::from_le_bytes([b[0][0x14F4], b[0][0x14F5]])
        );
        assert_eq!(g.printer.recorded[26], b[0][0x1471]);
    }

    #[test]
    fn a_change_to_a_part_lands_in_memory_and_nowhere_else() {
        let b = banks();
        let mut g = Game::from_memory(&b);
        g.play.changed[5] ^= 0xFF;
        g.rows[1] = 0x1234;
        g.printer.attr_page ^= 1;
        let m = g.to_memory();
        let differ: Vec<(usize, usize)> = (0..8)
            .flat_map(|n| (0..BANK).map(move |i| (n, i)))
            .filter(|&(n, i)| m[n][i] != b[n][i])
            .collect();
        assert_eq!(differ, [(0, 0x1637), (0, 0x2505), (0, 0x3E02), (0, 0x3E03)]);
    }
}
