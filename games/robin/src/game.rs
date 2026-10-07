//! The game's state: what the rewrite works on.
//!
//! Each part the rewrite understands is a typed field, read from where the
//! original keeps it (`docs/re/robin.md`). Everything else is the rest of
//! RAM, carried along untouched. [`Game::from_memory`] reads a state from
//! the 128K's banks and [`Game::to_memory`] writes it back, so the checks can
//! compare all of memory between the original and the rewrite.
//!
//! Every typed field is declared once, with its address and a name, and the
//! reading, writing and naming of it all come from that (`parts!`).

use crate::assets::BANK;

/// A value kept in memory, little-endian where it has more than one byte.
trait Bytes {
    const LEN: usize;
    fn zero() -> Self;
    fn byte(&self, i: usize) -> u8;
    fn set_byte(&mut self, i: usize, v: u8);
}

impl Bytes for u8 {
    const LEN: usize = 1;
    fn zero() -> u8 {
        0
    }
    fn byte(&self, _: usize) -> u8 {
        *self
    }
    fn set_byte(&mut self, _: usize, v: u8) {
        *self = v;
    }
}

impl Bytes for u16 {
    const LEN: usize = 2;
    fn zero() -> u16 {
        0
    }
    fn byte(&self, i: usize) -> u8 {
        self.to_le_bytes()[i]
    }
    fn set_byte(&mut self, i: usize, v: u8) {
        let mut b = self.to_le_bytes();
        b[i] = v;
        *self = u16::from_le_bytes(b);
    }
}

impl<T: Bytes + Copy, const N: usize> Bytes for [T; N] {
    const LEN: usize = N * T::LEN;
    fn zero() -> [T; N] {
        [T::zero(); N]
    }
    fn byte(&self, i: usize) -> u8 {
        self[i / T::LEN].byte(i % T::LEN)
    }
    fn set_byte(&mut self, i: usize, v: u8) {
        self[i / T::LEN].set_byte(i % T::LEN, v);
    }
}

impl<T: Bytes> Bytes for Box<T> {
    const LEN: usize = T::LEN;
    fn zero() -> Box<T> {
        Box::new(T::zero())
    }
    fn byte(&self, i: usize) -> u8 {
        (**self).byte(i)
    }
    fn set_byte(&mut self, i: usize, v: u8) {
        (**self).set_byte(i, v);
    }
}

/// Where `addr` falls in a part of `len` bytes at `at`, if it does.
fn within(addr: u16, at: u16, len: usize) -> Option<usize> {
    let i = usize::from(addr.wrapping_sub(at));
    (i < len).then_some(i)
}

/// For a group of typed fields, each with its address (bank 0 paged at
/// `0xC000`) and its name: reading them from memory, writing them back, and
/// finding which holds an address.
macro_rules! parts {
    ($group:ident { $( $f:ident : $t:ty = $addr:expr => $name:expr ),* $(,)? }) => {
        impl $group {
            fn read_parts(&mut self, read: &dyn Fn(u16) -> u8) {
                $( for i in 0..<$t as Bytes>::LEN {
                    self.$f.set_byte(i, read(($addr as u16).wrapping_add(i as u16)));
                } )*
            }
            fn write_parts(&self, write: &mut dyn FnMut(u16, u8)) {
                $( for i in 0..<$t as Bytes>::LEN {
                    write(($addr as u16).wrapping_add(i as u16), self.$f.byte(i));
                } )*
            }
            fn get_part(&self, addr: u16) -> Option<u8> {
                $( if let Some(i) = within(addr, $addr, <$t as Bytes>::LEN) {
                    return Some(self.$f.byte(i));
                } )*
                None
            }
            fn set_part(&mut self, addr: u16, v: u8) -> bool {
                $( if let Some(i) = within(addr, $addr, <$t as Bytes>::LEN) {
                    self.$f.set_byte(i, v);
                    return true;
                } )*
                false
            }
            fn name_of(addr: u16) -> Option<&'static str> {
                $( if within(addr, $addr, <$t as Bytes>::LEN).is_some() {
                    return Some($name);
                } )*
                None
            }
            fn zeroed() -> $group {
                $group { $( $f: <$t as Bytes>::zero(), )* }
            }
        }
    };
}

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

parts!(PlayArea {
    pixels: Box<[u8; 0x1200]> = 0xEB00 => "the back buffer",
    attrs: Box<[u8; 0x240]> = 0xE800 => "the attribute buffer",
    changed: Box<[u8; 0x240]> = 0xE500 => "the changed-cell map",
});

/// The screen and the tables built at start-up.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Display {
    /// The screen: 6,144 bytes of pixels, then 768 of attributes.
    pub screen: Box<[u8; 6912]>,
    /// Each byte with its bits reversed.
    pub mirror: [u8; 256],
    /// The screen address of each pixel row.
    pub rows: [u16; 192],
}

parts!(Display {
    screen: Box<[u8; 6912]> = 0x4000 => "the screen",
    mirror: [u8; 256] = 0xFD00 => "the mirror table",
    rows: [u16; 192] = 0xFE00 => "the row table",
});

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
    /// Kept in its own code, as the operands it rewrites: the column offset
    /// of the attributes (0, or 2 into the play area)...
    pub column_offset: u8,
    /// ...where they go (`0x58` the screen, `0xE8` the attribute buffer)...
    pub attr_page: u8,
    /// ...what is ORed into each (0, or `0x80`)...
    pub attr_flag: u8,
    /// ...and the last glyph byte looked up in the mirror table.
    pub mirror_lookup: u8,
}

parts!(Printer {
    mode: u8 = 0xD6CE => "printer.mode",
    mirrored: u8 = 0xD6CD => "printer.mirrored",
    replace: u8 = 0xD6CF => "printer.replace",
    cell: u16 = 0xD4F4 => "printer.cell",
    recorded: [u8; 27] = 0xD457 => "printer.recorded",
    column_offset: u8 = 0xD629 => "printer.column_offset",
    attr_page: u8 = 0xD637 => "printer.attr_page",
    attr_flag: u8 = 0xD667 => "printer.attr_flag",
    mirror_lookup: u8 = 0xD5AB => "printer.mirror_lookup",
});

/// The map's state (`docs/re/robin.md`, *The map*).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapState {
    /// The current location: row × 16 + column.
    pub location: u16,
    /// The record being drawn: its number, and the table it is in.
    pub record: u8,
    pub table: u16,
    /// The three special locations, set when a game starts.
    pub specials: [u16; 3],
    /// Kept in the drawing's own code, as the operands it rewrites: the
    /// location's table byte (bit 7, mirrored)...
    pub location_byte: u8,
    /// ...what columns are XORed with, for the pixels and the attributes (0,
    /// or `0x1C` mirrored)...
    pub column_xor: u8,
    pub attr_column_xor: u8,
    /// ...the block's height in character rows, for its attributes...
    pub attr_rows: u8,
    /// ...the instruction stepping through its attributes (`0x13`, `INC DE`,
    /// or 0, none: one attribute for all)...
    pub attr_step: u8,
    /// ...and, when mirroring a block, the last two bytes looked up.
    pub mirror_left: u8,
    pub mirror_right: u8,
}

parts!(MapState {
    location: u16 = 0xC440 => "map.location",
    record: u8 = 0xC43F => "map.record",
    table: u16 = 0xC442 => "map.table",
    specials: [u16; 3] = 0xD28F => "map.specials",
    location_byte: u8 = 0xBFE0 => "map.location_byte",
    column_xor: u8 = 0xBFFB => "map.column_xor",
    attr_column_xor: u8 = 0xC03D => "map.attr_column_xor",
    attr_rows: u8 = 0xC044 => "map.attr_rows",
    attr_step: u8 = 0xC049 => "map.attr_step",
    mirror_left: u8 = 0xC0E9 => "map.mirror_left",
    mirror_right: u8 = 0xC0F1 => "map.mirror_right",
});

/// The sprite engine's state (`docs/re/robin.md`, *Sprites*): all of it
/// kept in its own code, as the bytes it rewrites.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SpriteState {
    /// The frame table being drawn from.
    pub table: u16,
    /// A character figure's codes, being drawn.
    pub figure: u16,
    /// The figure drawing's three choices of path, as jump offsets (0 for
    /// the mirrored path), and its attribute step (`XOR A`, `0xAF`, or none).
    pub figure_columns: u8,
    pub figure_bytes: u8,
    pub figure_attrs: u8,
    pub figure_attr_step: u8,
    /// The last figure byte looked up in the mirror table.
    pub figure_mirror: u8,
    /// The four places in the shift chain, 16 bytes apart, where a `RET` is
    /// planted to cut it short, and put back after: `0xCB` while not.
    pub chain_0: u8,
    pub chain_1: u8,
    pub chain_2: u8,
    pub chain_3: u8,
    /// Marking a frame's cells: whether its colour pattern steps along a
    /// row and down a column (`INC BC`, `0x03`, or none).
    pub mark_row_step: u8,
    pub mark_column_step: u8,
    /// The last three frame bytes looked up when mirroring one: the third,
    /// the first and the second of a row.
    pub mirror_third: u8,
    pub mirror_first: u8,
    pub mirror_second: u8,
}

parts!(SpriteState {
    table: u16 = 0xC5D0 => "sprites.table",
    figure: u16 = 0xC598 => "sprites.figure",
    figure_columns: u8 = 0xC4DB => "sprites.figure_columns",
    figure_bytes: u8 = 0xC4FB => "sprites.figure_bytes",
    figure_attrs: u8 = 0xC53F => "sprites.figure_attrs",
    figure_attr_step: u8 = 0xC549 => "sprites.figure_attr_step",
    figure_mirror: u8 = 0xC500 => "sprites.figure_mirror",
    chain_0: u8 = 0xC647 => "sprites.chain_0",
    chain_1: u8 = 0xC657 => "sprites.chain_1",
    chain_2: u8 = 0xC667 => "sprites.chain_2",
    chain_3: u8 = 0xC677 => "sprites.chain_3",
    mark_row_step: u8 = 0xC6E8 => "sprites.mark_row_step",
    mark_column_step: u8 = 0xC6F3 => "sprites.mark_column_step",
    mirror_third: u8 = 0xC7CD => "sprites.mirror_third",
    mirror_first: u8 = 0xC7D5 => "sprites.mirror_first",
    mirror_second: u8 = 0xC7E0 => "sprites.mirror_second",
});

/// Robin, and how he is controlled (`docs/re/robin.md`, *Robin's
/// movement*).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Robin {
    /// His position: the next position of his sprite record.
    pub x: u8,
    pub y: u8,
    /// The direction he is going: bit 0 right, 1 left, 2 down, 3 up, 4 fire.
    pub direction: u8,
    /// His state: at 6 or more he doesn't move.
    pub state: u8,
    /// Non-zero while he's fighting.
    pub fighting: [u8; 2],
    /// The control method, as the address of its code.
    pub method: u16,
    /// The redefined keys: fire, up, down, left, right.
    pub keys: [u8; 5],
    /// Kept in the controls' code: two `NOP`s, or `LD E,n` to put `n` in
    /// place of what the player pressed.
    pub override_controls: [u8; 2],
    /// Kept in his update's code: its counter.
    pub update_counter: u8,
}

parts!(Robin {
    x: u8 = 0xCB7F => "robin.x",
    y: u8 = 0xCB80 => "robin.y",
    direction: u8 = 0xCB85 => "robin.direction",
    state: u8 = 0xCB81 => "robin.state",
    fighting: [u8; 2] = 0xCB74 => "robin.fighting",
    method: u16 = 0xD152 => "robin.method",
    keys: [u8; 5] = 0xD154 => "robin.keys",
    override_controls: [u8; 2] = 0xD0CE => "robin.override_controls",
    update_counter: u8 = 0xC59B => "robin.update_counter",
});

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Game {
    pub display: Display,
    pub play: PlayArea,
    pub printer: Printer,
    pub map: MapState,
    pub sprites: SpriteState,
    pub robin: Robin,
    /// The rest of RAM, banks 0 to 7, as it was read: what the rewrite does
    /// not model yet, and the blocks' bytes, which the game mirrors in place.
    rest: Box<[[u8; BANK]; 8]>,
}

/// The bank and offset of `addr`, with bank 0 paged at `0xC000`.
///
/// # Panics
///
/// Below `0x4000`, where the original has its ROM: the game has none.
fn place(addr: u16) -> (usize, usize) {
    let bank = match addr >> 14 {
        0 => panic!("{addr:#06x} is in the ROM, and the game has no ROM"),
        1 => 5,
        2 => 2,
        _ => 0,
    };
    (bank, usize::from(addr) % BANK)
}

impl Game {
    /// The name of the part of the state at `offset` in `bank`, for saying
    /// where two states differ: a typed part, or the rest of RAM.
    #[must_use]
    pub fn part_at(bank: usize, offset: usize) -> &'static str {
        let base = match bank {
            5 => 0x4000,
            2 => 0x8000,
            0 => 0xC000,
            _ => return "the rest of RAM",
        };
        let addr = base + offset as u16;
        Display::name_of(addr)
            .or_else(|| PlayArea::name_of(addr))
            .or_else(|| Printer::name_of(addr))
            .or_else(|| MapState::name_of(addr))
            .or_else(|| SpriteState::name_of(addr))
            .or_else(|| Robin::name_of(addr))
            .unwrap_or("the rest of RAM")
    }

    /// The byte the processor sees at `addr` with bank 0 paged at `0xC000`,
    /// as it is whenever the screen and map code runs.
    ///
    /// # Panics
    ///
    /// Below `0x4000`, where the original has its ROM: the game has none.
    #[must_use]
    pub fn read(&self, addr: u16) -> u8 {
        let (bank, offset) = place(addr);
        self.display
            .get_part(addr)
            .or_else(|| self.play.get_part(addr))
            .or_else(|| self.printer.get_part(addr))
            .or_else(|| self.map.get_part(addr))
            .or_else(|| self.sprites.get_part(addr))
            .or_else(|| self.robin.get_part(addr))
            .unwrap_or(self.rest[bank][offset])
    }

    /// Writes the byte at `addr`, with bank 0 paged at `0xC000`. Writes below
    /// `0x4000`, to the ROM, go nowhere, as on the machine.
    pub fn write(&mut self, addr: u16, v: u8) {
        if addr < 0x4000 {
            return;
        }
        if !(self.display.set_part(addr, v)
            || self.play.set_part(addr, v)
            || self.printer.set_part(addr, v)
            || self.map.set_part(addr, v)
            || self.sprites.set_part(addr, v)
            || self.robin.set_part(addr, v))
        {
            let (bank, offset) = place(addr);
            self.rest[bank][offset] = v;
        }
    }

    /// Reads the state from the 128K's eight banks, bank 0 first.
    #[must_use]
    pub fn from_memory(banks: &[[u8; BANK]; 8]) -> Game {
        let read = |a: u16| {
            let (bank, offset) = place(a);
            banks[bank][offset]
        };
        let mut g = Game {
            display: Display::zeroed(),
            play: PlayArea::zeroed(),
            printer: Printer::zeroed(),
            map: MapState::zeroed(),
            sprites: SpriteState::zeroed(),
            robin: Robin::zeroed(),
            rest: Box::new(*banks),
        };
        g.display.read_parts(&read);
        g.play.read_parts(&read);
        g.printer.read_parts(&read);
        g.map.read_parts(&read);
        g.sprites.read_parts(&read);
        g.robin.read_parts(&read);
        g
    }

    /// Writes the state back over the rest of RAM: the eight banks as the
    /// original would hold them.
    #[must_use]
    pub fn to_memory(&self) -> Box<[[u8; BANK]; 8]> {
        let mut banks = self.rest.clone();
        let mut write = |a: u16, v: u8| {
            let (bank, offset) = place(a);
            banks[bank][offset] = v;
        };
        self.display.write_parts(&mut write);
        self.play.write_parts(&mut write);
        self.printer.write_parts(&mut write);
        self.map.write_parts(&mut write);
        self.sprites.write_parts(&mut write);
        self.robin.write_parts(&mut write);
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
        assert_eq!(Game::part_at(0, 0x0441), "map.location");
        assert_eq!(Game::part_at(2, 0x3FE0), "map.location_byte");
        assert_eq!(Game::part_at(4, 0x0441), "the rest of RAM");
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
        assert_eq!(g.display.screen[6144], 0x47);
        assert_eq!(g.play.pixels[2], 0x81);
        assert_eq!(g.display.rows[1] >> 8, 0x12);
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
        assert_eq!(g.display.screen[0], b[5][0]);
        assert_eq!(g.display.screen[6911], b[5][0x1AFF]);
        assert_eq!(g.play.pixels[0], b[0][0x2B00]);
        assert_eq!(g.play.pixels[0x11FF], b[0][0x3CFF]);
        assert_eq!(g.play.attrs[0], b[0][0x2800]);
        assert_eq!(g.play.changed[0x23F], b[0][0x273F]);
        assert_eq!(g.display.mirror[255], b[0][0x3DFF]);
        assert_eq!(
            g.display.rows[0],
            u16::from_le_bytes([b[0][0x3E00], b[0][0x3E01]])
        );
        assert_eq!(
            g.display.rows[191],
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
        g.display.rows[1] = 0x1234;
        g.printer.attr_page ^= 1;
        let m = g.to_memory();
        let differ: Vec<(usize, usize)> = (0..8)
            .flat_map(|n| (0..BANK).map(move |i| (n, i)))
            .filter(|&(n, i)| m[n][i] != b[n][i])
            .collect();
        assert_eq!(differ, [(0, 0x1637), (0, 0x2505), (0, 0x3E02), (0, 0x3E03)]);
    }
}
