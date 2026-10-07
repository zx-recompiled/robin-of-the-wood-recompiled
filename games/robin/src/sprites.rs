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

/// The table that reverses a figure's six columns, row by row (`0xC57A`).
const FIGURE_COLUMNS_REVERSED: u16 = 0xC57A;
/// The back buffer, the changed-cell map, the colour patterns' table, and
/// the shift chain's first stop.
const BACK_BUFFER: u16 = 0xEB00;
const CHANGED: u16 = 0xE500;
const CHAIN: u16 = 0xC647;

fn word(g: &crate::Game, at: u16) -> u16 {
    u16::from_le_bytes([g.read(at), g.read(at.wrapping_add(1))])
}

/// Marks the cells a pixel frame at `x`, `y` covers changed (`0:C688`), with
/// the attributes of its colour pattern: 4 cells wide, as many rows as its
/// height covers, and one more if `y` is not on a character row.
pub fn mark_cells(g: &mut crate::Game, at: u16, x: u8, y: u8) {
    let x = x.max(16) - 8;
    let mut rows = (g.read(at.wrapping_add(1)) >> 3) & 7;
    if rows == 0 {
        return;
    }
    if y & 7 != 0 {
        rows += 1;
    }
    let colour = g.read(at.wrapping_add(2));
    g.sprites.mark_row_step = if colour & 0x80 != 0 { 3 } else { 0 };
    g.sprites.mark_column_step = if colour & 0x40 != 0 { 3 } else { 0 };
    let mut pattern = word(g, 0x8CB7u16.wrapping_add(u16::from((colour << 1) & 0x7F)));
    let offset = u16::from(y & 0xF8) * 4;
    let mut cell = CHANGED.wrapping_add((offset & 0xFF00) | u16::from(offset as u8 | x >> 2));
    for _ in 0..rows {
        let mut c = cell;
        for _ in 0..4 {
            let v = g.read(pattern);
            g.write(c, v);
            if g.sprites.mark_row_step != 0 {
                pattern = pattern.wrapping_add(1);
            }
            c = (c & 0xFF00) | u16::from((c as u8).wrapping_add(1));
        }
        cell = cell.wrapping_add(32);
        if g.sprites.mark_column_step != 0 {
            pattern = pattern.wrapping_add(1);
        }
    }
}

/// Draws the character figure at `at` (`0:C47D`): its 6 × 5 cells XORed into
/// the back buffer from their characters, mirrored by `frame`'s bit 7, then
/// its attributes marked where the cells are not already.
fn draw_figure(g: &mut crate::Game, at: u16, frame: u8, x: u8, y: u8) {
    let mirrored = frame & 0x80 != 0;
    let (columns, bytes, attrs) = if mirrored { (0, 0, 0) } else { (7, 6, 8) };
    g.sprites.figure_columns = columns;
    g.sprites.figure_bytes = bytes;
    g.sprites.figure_attrs = attrs;
    g.sprites.figure_attr_step = if g.read(at) & 0x80 != 0 { 0xAF } else { 0 };
    g.sprites.figure = at.wrapping_add(1);
    let mut column = (x >> 2).wrapping_sub(3);
    if mirrored {
        column = column.wrapping_sub(1);
    }
    let column = column & 0x1F;
    let row = (y >> 3).wrapping_sub(1);
    let cell_at = |n: u8, g: &crate::Game, reversed: bool| {
        if reversed {
            g.read(FIGURE_COLUMNS_REVERSED.wrapping_add(u16::from(n)))
        } else {
            n
        }
    };

    let mut line = BACK_BUFFER.wrapping_add(u16::from(row) << 8 | u16::from(column));
    let mut n: u8 = 0;
    for _ in 0..5 {
        let mut to = line;
        for _ in 0..6 {
            let i = cell_at(n, g, g.sprites.figure_columns == 0);
            let code = g.read(g.sprites.figure.wrapping_add(u16::from(i)));
            if code != 0 {
                let glyph = FIGURE_CHARACTERS.wrapping_add(u16::from(code) * 8);
                let mut p = to;
                for r in 0..8 {
                    let mut b = g.read(glyph.wrapping_add(r));
                    if g.sprites.figure_bytes == 0 {
                        g.sprites.figure_mirror = b;
                        b = g.read(0xFD00 | u16::from(b));
                    }
                    let v = b ^ g.read(p);
                    g.write(p, v);
                    p = (p & 0xFF00) | u16::from((p as u8).wrapping_add(0x20));
                }
            }
            to = (to & 0xFF00) | u16::from((to as u8).wrapping_add(1) & 0x1F);
            n = n.wrapping_add(1);
        }
        line = line.wrapping_add(0x100);
    }

    let offset = (row & 7) << 5 | column;
    let mut cells = CHANGED.wrapping_add(u16::from(row >> 3) << 8 | u16::from(offset));
    let colours = g.sprites.figure.wrapping_add(0x1E);
    let mut n: u8 = 0;
    for _ in 0..5 {
        let mut c = cells;
        for _ in 0..6 {
            let mut i = cell_at(n, g, g.sprites.figure_attrs == 0);
            if g.sprites.figure_attr_step == 0xAF {
                i = 0;
            }
            let attr = g.read(colours.wrapping_add(u16::from(i)));
            if g.read(c) == 0 && attr != 0 {
                g.write(c, attr);
            }
            c = (c & 0xFFE0) | u16::from((c as u8).wrapping_add(1) & 0x1F);
            n = n.wrapping_add(1);
        }
        cells = cells.wrapping_add(32);
    }
}

/// Draws frame `frame` (bit 7: mirrored) of the current table at `x`, `y`
/// (`0:C5CE`): a character figure whole, or a pixel frame's cells marked,
/// the frame mirrored in place if need be, and its rows shifted into place
/// and XORed into the back buffer.
pub fn draw_frame(g: &mut crate::Game, sprites: &Sprites, frame: u8, x: u8, y: u8) {
    let entry = g.sprites.table.wrapping_add(u16::from(frame << 1));
    let at = sprites
        .table(g.sprites.table)
        .and_then(|f| f.get(usize::from((frame << 1) / 2)))
        .map_or_else(|| word(g, entry), |f| f.at);
    if g.read(at) & 0x0F >= 4 {
        draw_figure(g, at, frame, x, y);
        return;
    }
    mark_cells(g, at, x, y);
    mirror_frame(g, at, frame);

    // The shift: 2 pixels for each step of the position's low two bits,
    // through the chain, cut short by a `RET` planted at that stop.
    let k = x & 3;
    let stop = CHAIN + 16 * u16::from(k);
    g.write(stop, 0xC9);
    let mut y = y;
    if x < 8 {
        y = y.wrapping_sub(1);
    }
    let column = (x.wrapping_sub(8) >> 2) & 0x1F;
    let mut to =
        BACK_BUFFER.wrapping_add(u16::from(y >> 3) << 8 | u16::from((y & 7) << 5 | column));
    let mut from = at.wrapping_add(3);
    for _ in 0..times(g.read(at.wrapping_add(1))) {
        let row = u32::from(g.read(from)) << 24
            | u32::from(g.read(from.wrapping_add(1))) << 16
            | u32::from(g.read(from.wrapping_add(2))) << 8;
        from = from.wrapping_add(3);
        for (i, b) in (row >> (2 * u32::from(k)))
            .to_be_bytes()
            .into_iter()
            .enumerate()
        {
            let p = to.wrapping_add(i as u16);
            let v = g.read(p) ^ b;
            g.write(p, v);
        }
        to = to.wrapping_add(0x20);
    }
    g.write(stop, 0xCB);
}

/// A sprite record's fields, as offsets from its start (`docs/re/robin.md`,
/// *A sprite*).
mod record {
    pub const COUNTER: u16 = 0;
    pub const RELOAD: u16 = 1;
    pub const SEQUENCE: u16 = 2;
    pub const FLAGS: u16 = 4;
    pub const DRAWN: u16 = 5;
    pub const NEXT: u16 = 8;
}

/// Flags: active, drawn, to be drawn.
const ACTIVE: u8 = 1;
const DRAWN: u8 = 2;
const TO_DRAW: u8 = 4;

/// Animates the sprite whose record is at `at` and redraws it (`0:C7EF`):
/// when its counter runs out (its top bit set), resets it and takes the next
/// frame of its sequence; erases it where it was drawn; and draws it anew
/// if it is to be drawn.
pub fn animate(g: &mut crate::Game, sprites: &Sprites, at: u16) {
    let field = |f: u16| at.wrapping_add(f);
    if g.read(field(record::FLAGS)) & ACTIVE == 0 {
        return;
    }
    let counter = g.read(field(record::COUNTER)).wrapping_sub(1);
    g.write(field(record::COUNTER), counter);
    if counter & 0x80 != 0 {
        let reload = g.read(field(record::RELOAD));
        g.write(field(record::COUNTER), reload);
        let mut next = word(g, field(record::SEQUENCE));
        let mut frame = g.read(next);
        next = next.wrapping_add(1);
        if frame == 0xFF {
            next = word(g, next);
            frame = g.read(next);
            next = next.wrapping_add(1);
        }
        for (i, b) in next.to_le_bytes().into_iter().enumerate() {
            g.write(field(record::SEQUENCE + i as u16), b);
        }
        g.write(field(record::NEXT), frame);
    }
    let flags = g.read(field(record::FLAGS));
    if flags & DRAWN != 0 {
        g.write(field(record::FLAGS), flags & !DRAWN);
        let (frame, x, y) = (
            g.read(field(record::DRAWN)),
            g.read(field(record::DRAWN + 1)),
            g.read(field(record::DRAWN + 2)),
        );
        draw_frame(g, sprites, frame, x, y);
    }
    let flags = g.read(field(record::FLAGS));
    if flags & TO_DRAW != 0 {
        g.write(field(record::FLAGS), (flags & !TO_DRAW) | DRAWN);
        let next: [u8; 3] = std::array::from_fn(|i| g.read(field(record::NEXT + i as u16)));
        // Copied from the last byte down, as the original's LDDR does.
        for i in (0..3).rev() {
            g.write(field(record::DRAWN + i as u16), next[i]);
        }
        draw_frame(g, sprites, next[0], next[1], next[2]);
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
