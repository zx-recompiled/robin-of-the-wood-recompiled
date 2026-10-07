//! The screen and the play area: building the tables drawing uses
//! (`docs/re/robin.md`, *The screen*).

use crate::game::Game;

/// Builds the mirror table: each byte with its bits in reverse order, so
/// looking a glyph or sprite byte up flips it left to right.
pub fn build_mirror(g: &mut Game) {
    for (n, m) in g.display.mirror.iter_mut().enumerate() {
        *m = (n as u8).reverse_bits();
    }
}

/// Builds the row table: the screen address of each pixel row, in the
/// Spectrum's interleaved layout.
pub fn build_rows(g: &mut Game) {
    for (y, r) in g.display.rows.iter_mut().enumerate() {
        *r = row_address(y as u8);
    }
}

/// The screen address of pixel row `y`: the third of the screen, then the
/// pixel row within the character, then the character row within the third.
#[must_use]
pub const fn row_address(y: u8) -> u16 {
    let y = y as u16;
    0x4000 | (y & 0xC0) << 5 | (y & 0x07) << 8 | (y & 0x38) << 2
}

/// Clears the screen's pixels.
pub fn clear_screen(g: &mut Game) {
    g.display.screen[..6144].fill(0);
}

/// Sets every attribute on the screen to `attr`.
pub fn fill_attrs(g: &mut Game, attr: u8) {
    g.display.screen[6144..].fill(attr);
}

/// Clears the play area: every cell of the attribute buffer to `attr`, and
/// the back buffer to 0.
pub fn clear_play_area(g: &mut Game, attr: u8) {
    g.play.attrs.fill(attr);
    g.play.pixels.fill(0);
}

/// Marks no cell as changed.
pub fn clear_changed(g: &mut Game) {
    g.play.changed.fill(0);
}

/// Clears the bottom 32 pixel rows of the screen, found through the row
/// table.
pub fn clear_lower_panel(g: &mut Game) {
    for y in 160..192 {
        let at = usize::from(g.display.rows[y].wrapping_sub(0x4000));
        for b in &mut g.display.screen[at..at + 32] {
            *b = 0;
        }
    }
}

/// The play area's size, in character cells, and where it starts on the
/// screen.
pub const PLAY_ROWS: usize = 18;
pub const PLAY_COLUMNS: std::ops::Range<usize> = 2..30;

/// The screen offset of pixel row `p` of the character cell at `row` and
/// `column`.
const fn cell_pixel(row: usize, p: usize, column: usize) -> usize {
    (row & 0x18) << 8 | p << 8 | (row & 7) << 5 | column
}

/// Copies every changed cell of the play area to the screen, and marks it
/// unchanged: its attribute (the attribute buffer's, or the mark itself where
/// that is zero, with bit 7 masked off), then its eight pixel rows from the
/// back buffer.
pub fn flush(g: &mut Game) {
    for row in 0..PLAY_ROWS {
        for column in PLAY_COLUMNS {
            let cell = row * 32 + column;
            let mark = g.play.changed[cell];
            if mark == 0 {
                continue;
            }
            g.play.changed[cell] = 0;
            let attr = match g.play.attrs[cell] {
                0 => mark,
                a => a,
            };
            g.display.screen[6144 + cell] = attr & 0x7F;
            for p in 0..8 {
                g.display.screen[cell_pixel(row, p, column)] =
                    g.play.pixels[row * 256 + p * 32 + column];
            }
        }
    }
}

/// Copies the whole play area's pixels from the back buffer to the screen,
/// row by row through the row table.
pub fn copy_pixels(g: &mut Game) {
    for y in 0..PLAY_ROWS * 8 {
        let to = usize::from(g.display.rows[y].wrapping_sub(0x4000)) + PLAY_COLUMNS.start;
        let from = y * 32 + PLAY_COLUMNS.start;
        let n = PLAY_COLUMNS.len();
        g.display.screen[to..to + n].copy_from_slice(&g.play.pixels[from..from + n]);
    }
}

/// Copies the whole play area's attributes from the attribute buffer to the
/// screen, bit 7 masked off.
pub fn copy_attrs(g: &mut Game) {
    for row in 0..PLAY_ROWS {
        for column in PLAY_COLUMNS {
            let cell = row * 32 + column;
            g.display.screen[6144 + cell] = g.play.attrs[cell] & 0x7F;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cells_pixel_rows_are_where_the_screen_keeps_them() {
        for (row, p, column) in [(0, 0, 2), (7, 7, 29), (8, 0, 2), (17, 3, 15)] {
            let y = (row * 8 + p) as u8;
            assert_eq!(
                0x4000 + cell_pixel(row, p, column),
                usize::from(row_address(y)) + column
            );
        }
    }

    #[test]
    fn rows_are_the_spectrums_layout() {
        assert_eq!(row_address(0), 0x4000);
        assert_eq!(row_address(1), 0x4100);
        assert_eq!(row_address(8), 0x4020);
        assert_eq!(row_address(63), 0x47E0);
        assert_eq!(row_address(64), 0x4800);
        assert_eq!(row_address(191), 0x57E0);
    }
}
