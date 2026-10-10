//! The picture as the beam draws it, T-state by T-state (#15): the border in
//! groups of 4 T-states and each screen cell read when the ULA reads it, so
//! a program that changes the border, the screen or the shown bank in the
//! middle of a frame looks as it does on the machine. [`crate::screen`]
//! draws a frame all at once, which is enough for everything else.
//!
//! Its timing is from written references, never fitted to the tests it is
//! checked by (#15, Decision 2):
//!
//! - **The fetches**, Ramsoft's *Floating Bus Technical Guide*, measured on
//!   real machines: a line's 32 cells in 16 cycles of 8 T-states, each
//!   reading a bitmap, its attribute, the next bitmap and its attribute,
//!   one a T-state, then idle for 4. The first bitmap is read at 14,368 on
//!   the 128K and 14,347 on the 48K, and each line starts a line later.
//! - **The border**, the World of Spectrum *128K ZX Spectrum Reference*: an
//!   `OUT` that finishes 14,365 to 14,368 T-states after the interrupt
//!   changes the border from the first screen byte's position, in steps of 4
//!   T-states. So a group of 8 pixels shows the last colour written by its
//!   time, the first screen byte's being 14,368, the time its cell is read.
//!
//! **A write counts once its machine cycle has ended**, as that reference
//! counts an `OUT` by when it finishes. The same rule serves memory and the
//! shown bank (bit 3 of `0x7FFD`). The interpreter charges an instruction's
//! cycles before it runs it, so it notes when each write cycle ends
//! ([`Raster::write_ends`]), and each write takes the next of those times.

use zx_core::Model;
use zx_core::screen::{PALETTE, attr_offset, line_offset};

use crate::machine::Zx;
use crate::screen::{BORDER, HEIGHT, WIDTH};

/// When the first screen cell's bitmap is read, on each machine.
const fn first_fetch(model: Model) -> u32 {
    match model {
        Model::Spectrum48 => 14_347,
        Model::Spectrum128 => 14_368,
    }
}

/// The groups of 8 pixels in a line: 32 across the screen and 4 of border
/// either side.
const GROUPS: usize = WIDTH / 8;
const LINES: usize = HEIGHT;
const SIDE: usize = BORDER / 8;

/// A frame drawn as the beam goes. Turned on by setting [`Zx::raster`].
#[derive(Clone)]
pub struct Raster {
    /// The last frame finished, `screen::WIDTH` by `screen::HEIGHT`, 0RGB.
    pub picture: Vec<u32>,
    /// The frame being drawn.
    drawing: Vec<u32>,
    /// The next cell to be read, line by line, 32 a line.
    next: usize,
    /// The border's colour as the frame began, and each change since, with
    /// the T-state its `OUT` finished.
    border_from: u8,
    border: Vec<(u32, u8)>,
    /// When each write cycle of the instruction running ends, in order, and
    /// how many have been taken.
    pub write_ends: Vec<u32>,
    pub taken: usize,
}

impl Raster {
    #[must_use]
    pub fn new(border: u8) -> Raster {
        Raster {
            picture: vec![PALETTE[usize::from(border)]; WIDTH * HEIGHT],
            drawing: vec![0; WIDTH * HEIGHT],
            next: 0,
            border_from: border,
            border: Vec::new(),
            write_ends: Vec::new(),
            taken: 0,
        }
    }

    /// When the write the instruction is making now ends: the next of its
    /// write cycles' ends, or, for a write outside an instruction (an
    /// interrupt's push), `now`.
    pub fn write_end(&mut self, now: u32) -> u32 {
        let end = self.write_ends.get(self.taken).copied().unwrap_or(now);
        self.taken += 1;
        end
    }

    /// Notes a change of border colour by an `OUT` that finished at `end`.
    pub fn border(&mut self, end: u32, colour: u8) {
        self.border.push((end, colour & 7));
    }

    /// When the cell `n` (line by line, 32 a line) is read: its bitmap, with
    /// its attribute one T-state after.
    fn fetch_time(model: Model, line: u32, n: usize) -> u32 {
        let col = (n % 32) as u32;
        first_fetch(model) + (n / 32) as u32 * line + col / 2 * 8 + col % 2 * 2
    }
}

impl Zx {
    /// Reads every cell due before `end`, from memory as it is now: what a
    /// write ending at `end` must not change.
    pub fn raster_catch_up(&mut self, end: u32) {
        let (model, line, flash) = (self.model, self.timing.line, (self.frame / 16) % 2 == 1);
        let page = self.screen_page();
        let Some(r) = self.raster.as_deref_mut() else {
            return;
        };
        let screen = self.memory.page(page);
        while r.next < 32 * 192 && Raster::fetch_time(model, line, r.next) < end {
            let (y, col) = (r.next / 32, r.next % 32);
            let bits = screen[line_offset(y) + col];
            let attr = screen[0x1800 + attr_offset(y) + col];
            let bright = usize::from((attr >> 6) & 1) * 8;
            let mut ink = PALETTE[usize::from(attr & 7) + bright];
            let mut paper = PALETTE[usize::from((attr >> 3) & 7) + bright];
            if attr & 0x80 != 0 && flash {
                std::mem::swap(&mut ink, &mut paper);
            }
            let at = (BORDER + y) * WIDTH + BORDER + col * 8;
            for bit in 0..8 {
                r.drawing[at + bit] = if bits & (0x80 >> bit) != 0 {
                    ink
                } else {
                    paper
                };
            }
            r.next += 1;
        }
    }

    /// Ends the frame drawn so far, `frame` T-states long: the cells left,
    /// then the border, group by group. Changes after its end carry over.
    pub fn raster_finish(&mut self, frame: u32) {
        self.raster_catch_up(frame);
        let (model, line) = (self.model, self.timing.line);
        let Some(r) = self.raster.as_deref_mut() else {
            return;
        };
        let start = first_fetch(model);
        let mut colour = r.border_from;
        let mut changes = r.border.iter().peekable();
        for y in 0..LINES {
            for g in 0..GROUPS {
                let paper = (SIDE..SIDE + 32).contains(&g) && (BORDER..BORDER + 192).contains(&y);
                // The group's time, from the first screen byte's group.
                let at = (i64::from(start)
                    + (y as i64 - BORDER as i64) * i64::from(line)
                    + (g as i64 - SIDE as i64) * 4)
                    .max(0) as u32;
                while let Some(&&(end, c)) = changes.peek() {
                    if end > at {
                        break;
                    }
                    colour = c;
                    changes.next();
                }
                if !paper {
                    let from = y * WIDTH + g * 8;
                    r.drawing[from..from + 8].fill(PALETTE[usize::from(colour)]);
                }
            }
        }
        // The colour left at the frame's end, and anything written after it.
        let last = r
            .border
            .iter()
            .rev()
            .find(|&&(end, _)| end <= frame)
            .map_or(r.border_from, |&(_, c)| c);
        r.border = r
            .border
            .iter()
            .filter(|&&(end, _)| end > frame)
            .map(|&(end, c)| (end - frame, c))
            .collect();
        r.border_from = last;
        std::mem::swap(&mut r.picture, &mut r.drawing);
        r.next = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lines_cells_are_read_in_pairs_every_8_t_states() {
        let at = |n| Raster::fetch_time(Model::Spectrum128, 228, n);
        assert_eq!(
            [at(0), at(1), at(2), at(3)],
            [14_368, 14_370, 14_376, 14_378]
        );
        assert_eq!(at(31), 14_368 + 15 * 8 + 2);
        assert_eq!(at(32), 14_368 + 228, "the next line");
        assert_eq!(Raster::fetch_time(Model::Spectrum48, 224, 0), 14_347);
    }
}
