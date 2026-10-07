//! Sprites (`docs/re/robin.md`, *Sprites*): records animated through
//! sequences of frames, drawn into the play area's back buffer by XOR.

/// The three frame tables, each a list of frame addresses, and where each
/// ends: the next one starts there, and the colour patterns after the last.
pub const FRAME_TABLES: [u16; 3] = [0x8C25, 0x8C69, 0x8C8F];
const PATTERNS: u16 = 0x8CB7;
/// The character figures' characters, 8 bytes each.
pub const FIGURE_CHARACTERS: u16 = 0x9958;

/// A frame: where it is and its shape. Its bytes are not here: the game
/// mirrors frames in place, so they are state, kept in [`crate::Game`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Frame {
    /// Its header, then the rest.
    pub at: u16,
    /// A figure of character cells, rather than rows of pixels.
    pub figure: bool,
    /// A pixel frame's height in pixel rows.
    pub height: u8,
    /// A pixel frame's colour byte: its pattern, and how the pattern steps.
    pub colour: u8,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sprites {
    /// The frames of each table, by the table's address.
    pub tables: Vec<(u16, Vec<Frame>)>,
    /// The colour patterns' addresses, as many as the frames name.
    pub patterns: Vec<u16>,
}

impl Sprites {
    /// Parses the frame tables from the game's memory as the tape loads it,
    /// `read` giving the byte at an address with bank 0 paged at `0xC000`.
    #[must_use]
    pub fn parse(read: impl Fn(u16) -> u8) -> Sprites {
        let word = |a: u16| u16::from_le_bytes([read(a), read(a.wrapping_add(1))]);
        let ends = [FRAME_TABLES[1], FRAME_TABLES[2], PATTERNS];
        let tables: Vec<(u16, Vec<Frame>)> = FRAME_TABLES
            .iter()
            .zip(ends)
            .map(|(&table, end)| {
                let frames = (0..(end - table) / 2)
                    .map(|k| {
                        let at = word(table + 2 * k);
                        Frame {
                            at,
                            figure: read(at) & 0x0F >= 4,
                            height: read(at.wrapping_add(1)),
                            colour: read(at.wrapping_add(2)),
                        }
                    })
                    .collect();
                (table, frames)
            })
            .collect();
        let most = tables
            .iter()
            .flat_map(|(_, f)| f)
            .filter(|f| !f.figure)
            .map(|f| f.colour & 0x3F)
            .max()
            .unwrap_or(0);
        let patterns = (0..=u16::from(most))
            .map(|n| word(PATTERNS + 2 * n))
            .collect();
        Sprites { tables, patterns }
    }

    /// The frames of the table at `table`, if it is one of the three.
    #[must_use]
    pub fn table(&self, table: u16) -> Option<&[Frame]> {
        self.tables
            .iter()
            .find(|(t, _)| *t == table)
            .map(|(_, f)| f.as_slice())
    }
}

/// A count the original keeps in an 8-bit register and counts down to zero
/// after the first time round: 0 means 256.
fn times(n: u8) -> u16 {
    if n == 0 { 256 } else { u16::from(n) }
}

/// Mirrors the pixel frame at `at` in place if it faces the other way from
/// `want`'s bit 7 (`0:C7AF`): flips its header's bit 7, then, for each row,
/// swaps the outer two bytes and mirrors all three through the mirror table.
pub fn mirror_frame(g: &mut crate::Game, at: u16, want: u8) {
    if (want ^ g.read(at)) & 0x80 == 0 {
        return;
    }
    let header = g.read(at) ^ 0x80;
    g.write(at, header);
    let mut row = at.wrapping_add(3);
    for _ in 0..times(g.read(at.wrapping_add(1))) {
        let (first, second, third) = (row, row.wrapping_add(1), row.wrapping_add(2));
        let look = |g: &crate::Game, b: u8| g.read(0xFD00 | u16::from(b));
        let t = g.read(third);
        g.sprites.mirror_third = t;
        let mt = look(g, t);
        let f = g.read(first);
        g.sprites.mirror_first = f;
        let mf = look(g, f);
        g.write(third, mf);
        g.write(first, mt);
        let s = g.read(second);
        g.sprites.mirror_second = s;
        let ms = look(g, s);
        g.write(second, ms);
        row = row.wrapping_add(3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn the_tables_parse_as_the_notes_say() {
        let mut m = HashMap::new();
        let mut put = |at: u16, bytes: &[u8]| {
            for (i, &b) in bytes.iter().enumerate() {
                m.insert(at + i as u16, b);
            }
        };
        // Table 0 has a pixel frame at 0x9000 and a figure at 0x9100.
        put(FRAME_TABLES[0], &[0x00, 0x90, 0x00, 0x91]);
        put(0x9000, &[0x83, 16, 0x82]);
        put(0x9100, &[0x06]);
        put(PATTERNS, &[0x00, 0xA0, 0x10, 0xA0, 0x20, 0xA0]);
        let s = Sprites::parse(|a| m.get(&a).copied().unwrap_or(0));
        let t = s.table(FRAME_TABLES[0]).expect("a table");
        assert_eq!(t.len(), 34, "as many as fit before the next table");
        assert_eq!(
            t[0],
            Frame {
                at: 0x9000,
                figure: false,
                height: 16,
                colour: 0x82
            }
        );
        assert!(t[1].figure);
        assert_eq!(
            s.patterns,
            [0xA000, 0xA010, 0xA020],
            "as many as the colours name"
        );
        assert!(s.table(0x1234).is_none());
    }
}
