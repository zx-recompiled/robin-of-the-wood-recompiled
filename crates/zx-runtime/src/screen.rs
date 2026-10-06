//! ULA display rendering.

use crate::machine::Zx;

pub const BORDER: usize = 32;
pub const WIDTH: usize = zx_core::screen::WIDTH + 2 * BORDER;
pub const HEIGHT: usize = zx_core::screen::HEIGHT + 2 * BORDER;

/// Where the screen sits in a 48K machine's memory.
const BITMAP: usize = 0x4000;
const ATTRS: usize = 0x5800;

/// Renders the screen memory and border into a `WIDTH` x `HEIGHT` 0RGB buffer.
pub fn render(z: &Zx, out: &mut [u32]) {
    out.fill(zx_core::screen::PALETTE[z.border as usize]);
    zx_core::screen::render(
        &z.mem[BITMAP..],
        &z.mem[ATTRS..],
        (z.frame / 16) % 2 == 1,
        out,
        WIDTH,
        BORDER * WIDTH + BORDER,
        |c| c,
    );
}
