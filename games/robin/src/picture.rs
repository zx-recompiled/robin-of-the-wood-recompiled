//! The picture the Spectrum shows: the screen with its border, as 0RGB
//! pixels, for the window and for screenshots (#66).

use zx_core::screen::{BITMAP_LEN, HEIGHT, PALETTE, WIDTH, render};

/// The border's width on each side, as starquake-recompiled shows it.
pub const BORDER: usize = 32;
pub const FULL_W: usize = WIDTH + 2 * BORDER;
pub const FULL_H: usize = HEIGHT + 2 * BORDER;

/// Draws `screen` (6,912 bytes: bitmap, then attributes) inside a `border`
/// in `out`, `FULL_W` by `FULL_H`. Flashing cells swap ink and paper every
/// 16 frames, as the ULA does, by `frame`.
///
/// # Panics
///
/// If `screen` is short of the 6,912 bytes, or `out` of the picture's size.
pub fn draw(screen: &[u8], border: u8, frame: u64, out: &mut [u32]) {
    out[..FULL_W * FULL_H].fill(PALETTE[usize::from(border & 7)]);
    render(
        &screen[..BITMAP_LEN],
        &screen[BITMAP_LEN..],
        (frame / 16) % 2 == 1,
        out,
        FULL_W,
        BORDER * FULL_W + BORDER,
        |c| c,
    );
}
