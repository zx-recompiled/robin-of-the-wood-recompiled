//! ULA display rendering.

use crate::machine::Zx;

pub const BORDER: usize = 32;
pub const WIDTH: usize = zx_core::screen::WIDTH + 2 * BORDER;
pub const HEIGHT: usize = zx_core::screen::HEIGHT + 2 * BORDER;

/// Where the bitmap and attributes sit in the page the ULA displays.
const BITMAP: usize = 0x0000;
const ATTRS: usize = 0x1800;

/// Renders the screen memory and border into a `WIDTH` x `HEIGHT` 0RGB buffer.
pub fn render(z: &Zx, out: &mut [u32]) {
    out.fill(zx_core::screen::PALETTE[z.border as usize]);
    let page = z.memory.page(z.screen_page());
    zx_core::screen::render(
        &page[BITMAP..],
        &page[ATTRS..],
        (z.frame / 16) % 2 == 1,
        out,
        WIDTH,
        BORDER * WIDTH + BORDER,
        |c| c,
    );
}

/// The ROM character set's offset in the BASIC ROM: 96 glyphs of 8 bytes,
/// from space to the copyright sign.
pub const FONT: usize = 0x3D00;

/// The text on the screen the ULA displays, read cell by cell against
/// `font` (glyphs of 8 bytes from space, as at [`FONT`]): 24 lines of 32
/// characters, trailing spaces trimmed. A cell matching no glyph, normal or
/// inverse, reads as `?`. This is how a test program's printed verdict is
/// read: the same pixels a person would read off the screen.
#[must_use]
pub fn text(z: &Zx, font: &[u8]) -> Vec<String> {
    let page = z.memory.page(z.screen_page());
    let glyph = |row: usize, col: usize| -> [u8; 8] {
        std::array::from_fn(|y| {
            // The Spectrum's bitmap interleaves thirds, pixel lines and rows.
            let at = (row & 0x18) << 8 | y << 8 | (row & 7) << 5 | col;
            page[BITMAP + at]
        })
    };
    (0..24)
        .map(|row| {
            let line: String = (0..32)
                .map(|col| {
                    let g = glyph(row, col);
                    let inverse = g.map(|b| !b);
                    font.chunks(8)
                        .position(|f| f == g || f == inverse)
                        .map_or('?', |i| (b' ' + i as u8) as char)
                })
                .collect();
            line.trim_end().to_string()
        })
        .collect()
}
