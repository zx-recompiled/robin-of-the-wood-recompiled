//! The screen and the play area: building the tables drawing uses
//! (`docs/re/robin.md`, *The screen*).

use crate::game::Game;

/// Builds the mirror table: each byte with its bits in reverse order, so
/// looking a glyph or sprite byte up flips it left to right.
pub fn build_mirror(g: &mut Game) {
    for (n, m) in g.mirror.iter_mut().enumerate() {
        *m = (n as u8).reverse_bits();
    }
}

/// Builds the row table: the screen address of each pixel row, in the
/// Spectrum's interleaved layout.
pub fn build_rows(g: &mut Game) {
    for (y, r) in g.rows.iter_mut().enumerate() {
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

#[cfg(test)]
mod tests {
    use super::*;

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
