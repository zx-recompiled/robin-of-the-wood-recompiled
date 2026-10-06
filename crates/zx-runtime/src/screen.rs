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
