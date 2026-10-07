//! The map (`docs/re/robin.md`, *The map*): a wrapping grid of 16 × 20
//! locations, each drawn from a layout of blocks, read from the tape.

/// How many locations there are: 16 columns by 20 rows.
pub const COLUMNS: u16 = 16;
pub const ROWS: u16 = 20;
pub const LOCATIONS: usize = (COLUMNS * ROWS) as usize;

/// Where the tables are.
const LOCATION_TABLE: u16 = 0x7AC2;
const LAYOUT_TABLE: u16 = 0x7C02;
const EXTRAS_TABLE: u16 = 0x85DA;
const BLOCK_TABLE: u16 = 0x5BC9;
/// The three special locations' lists, drawn by `0:C056`.
const SPECIAL_LISTS: [u16; 3] = [0xC462, 0xC46B, 0xC474];
/// How many locations have extras: those below 256.
const EXTRAS: usize = 256;

/// One location of the grid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Location {
    pub layout: u8,
    /// Drawn mirrored left to right.
    pub mirrored: bool,
}

/// One block placed in a layout or a list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Item {
    /// The character row of the play area.
    pub row: u8,
    /// The column, in steps of 4 characters: 0 to 7.
    pub column: u8,
    pub block: u8,
    /// This block mirrored.
    pub mirrored: bool,
}

/// Where a block is and its shape. Its bytes are not here: the game mirrors
/// a block in place when it needs it the other way round, so they are state,
/// kept in [`crate::Game`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Block {
    /// Its header, then its pixels, then its attributes.
    pub at: u16,
    /// Its height in character rows.
    pub rows: u8,
    /// One attribute colours all of it.
    pub one_attr: bool,
}

impl Block {
    /// How many bytes its attributes take.
    #[must_use]
    pub fn attr_len(&self) -> u16 {
        if self.one_attr {
            1
        } else {
            u16::from(self.rows) * 4
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Map {
    pub locations: Vec<Location>,
    pub layouts: Vec<Vec<Item>>,
    /// For each location below 256, the blocks drawn on top of its layout.
    pub extras: Vec<Vec<Item>>,
    /// The lists for the three special locations.
    pub specials: [Vec<Item>; 3],
    pub blocks: Vec<Block>,
}

/// A record of a layout or a list: a count, then two bytes an item. Returns
/// the items and the address after them.
fn record(read: &impl Fn(u16) -> u8, at: u16) -> (Vec<Item>, u16) {
    let n = read(at);
    let items = (0..u16::from(n))
        .map(|k| {
            let p = read(at.wrapping_add(1 + 2 * k));
            let b = read(at.wrapping_add(2 + 2 * k));
            Item {
                row: p >> 3,
                column: p & 7,
                block: b & 0x7F,
                mirrored: b & 0x80 != 0,
            }
        })
        .collect();
    (items, at.wrapping_add(1 + 2 * u16::from(n)))
}

impl Map {
    /// Parses the map from the game's memory as the tape loads it, `read`
    /// giving the byte at an address with bank 0 paged at `0xC000`. Any bytes
    /// parse; the supported tape's are the shape the notes describe, which
    /// its own test checks.
    #[must_use]
    pub fn parse(read: impl Fn(u16) -> u8) -> Map {
        let locations: Vec<Location> = (0..LOCATIONS as u16)
            .map(|i| {
                let b = read(LOCATION_TABLE + i);
                Location {
                    layout: b & 0x7F,
                    mirrored: b & 0x80 != 0,
                }
            })
            .collect();
        let count = usize::from(locations.iter().map(|l| l.layout).max().unwrap_or(0)) + 1;
        let mut at = LAYOUT_TABLE;
        let layouts = (0..count)
            .map(|_| {
                let (items, next) = record(&read, at);
                at = next;
                items
            })
            .collect::<Vec<_>>();
        let mut at = EXTRAS_TABLE;
        let extras = (0..EXTRAS)
            .map(|_| {
                let (items, next) = record(&read, at);
                at = next;
                items
            })
            .collect::<Vec<_>>();
        let specials = SPECIAL_LISTS.map(|a| record(&read, a).0);
        let used = layouts
            .iter()
            .chain(&extras)
            .chain(&specials)
            .flatten()
            .map(|i| i.block)
            .max()
            .unwrap_or(0);
        let blocks = (0..=u16::from(used))
            .map(|n| {
                let entry = BLOCK_TABLE + 2 * n;
                let at = u16::from_le_bytes([read(entry), read(entry + 1)]);
                let header = read(at);
                Block {
                    at,
                    rows: header & 7,
                    one_attr: header & 0x40 != 0,
                }
            })
            .collect();
        Map {
            locations,
            layouts,
            extras,
            specials,
            blocks,
        }
    }
}

/// The location one step from `at` by the bits of `direction` (`0:C127`):
/// bit 0 right, bit 1 left, bit 2 down, bit 3 up, each wrapping round. As
/// the original does it for any word, not only the grid's: the row is the
/// byte above the column's four bits, and going up wraps to the last row
/// when the row taken from it has its top bit set.
#[must_use]
pub fn step(at: u16, direction: u8) -> u16 {
    let mut column = (at & 0xF) as u8;
    let mut row = (at >> 4) as u8;
    if direction & 1 != 0 {
        column = column.wrapping_add(1);
    }
    if direction & 2 != 0 {
        column = column.wrapping_sub(1);
    }
    column &= 0xF;
    if direction & 4 != 0 {
        row = row.wrapping_add(1);
        if u16::from(row) >= ROWS {
            row = 0;
        }
    }
    if direction & 8 != 0 {
        row = row.wrapping_sub(1);
        if row & 0x80 != 0 {
            row = (ROWS - 1) as u8;
        }
    }
    u16::from(row) << 4 | u16::from(column)
}

/// The address of record `n` of the run of records at `table` (`0:C0B9`):
/// each a count and two bytes an item, stepped over one by one.
#[must_use]
pub fn find_record(g: &crate::Game, table: u16, n: u8) -> u16 {
    let mut at = table;
    for _ in 0..n {
        let skip = (g.read(at) << 1).wrapping_add(1);
        at = at.wrapping_add(u16::from(skip));
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Memory holding a made-up map: location 0 is layout 1 mirrored,
    /// location 1 layout 0; layout 0 has no items, layout 1 two; location 0
    /// has one extra; block 2 is one row high with one attribute.
    fn memory() -> HashMap<u16, u8> {
        let mut m = HashMap::new();
        let mut put = |at: u16, bytes: &[u8]| {
            for (i, &b) in bytes.iter().enumerate() {
                m.insert(at + i as u16, b);
            }
        };
        put(LOCATION_TABLE, &[0x81, 0x00]);
        put(LAYOUT_TABLE, &[0, 2, 0x0B, 0x02, 0x10, 0x81]);
        put(EXTRAS_TABLE, &[1, 0x23, 0x00]);
        put(BLOCK_TABLE, &[0x00, 0x90, 0x10, 0x90, 0x20, 0x90]);
        put(0x9000, &[0x02]);
        put(0x9010, &[0x01]);
        put(0x9020, &[0x41]);
        m
    }

    #[test]
    fn the_tables_parse_as_the_notes_say() {
        let m = memory();
        let map = Map::parse(|a| m.get(&a).copied().unwrap_or(0));
        assert_eq!(map.locations.len(), 320);
        assert_eq!(
            map.locations[0],
            Location {
                layout: 1,
                mirrored: true
            }
        );
        assert_eq!(map.layouts.len(), 2, "as many as the locations name");
        assert!(map.layouts[0].is_empty());
        assert_eq!(
            map.layouts[1],
            [
                Item {
                    row: 1,
                    column: 3,
                    block: 2,
                    mirrored: false
                },
                Item {
                    row: 2,
                    column: 0,
                    block: 1,
                    mirrored: true
                },
            ]
        );
        assert_eq!(map.extras.len(), 256);
        assert_eq!(
            map.extras[0],
            [Item {
                row: 4,
                column: 3,
                block: 0,
                mirrored: false
            }]
        );
        assert!(map.extras[1].is_empty());
        assert_eq!(map.blocks.len(), 3, "as many as the lists name");
        assert_eq!(
            map.blocks[2],
            Block {
                at: 0x9020,
                rows: 1,
                one_attr: true
            }
        );
        assert_eq!(map.blocks[0].attr_len(), 8);
    }

    #[test]
    fn a_step_wraps_round_every_edge() {
        let at = |row: u16, column: u16| row << 4 | column;
        assert_eq!(step(at(3, 4), 1), at(3, 5));
        assert_eq!(step(at(3, 15), 1), at(3, 0));
        assert_eq!(step(at(3, 0), 2), at(3, 15));
        assert_eq!(step(at(19, 4), 4), at(0, 4));
        assert_eq!(step(at(0, 4), 8), at(19, 4));
        assert_eq!(step(at(5, 5), 0), at(5, 5));
        assert_eq!(step(at(19, 15), 5), at(0, 0), "right and down at once");
        assert_eq!(step(0x13F, 1), 0x130);
        assert_eq!(
            step(0x900, 8),
            at(0x13, 0),
            "a row with its top bit set wraps up too"
        );
    }
}
