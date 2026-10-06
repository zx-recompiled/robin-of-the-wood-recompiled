//! Runtime for statically recompiled ZX Spectrum 48K programs.
//!
//! The recompiler turns each block of Z80 code into a Rust function that
//! operates on [`Zx`]. At run time [`Zx::run_frame`] dispatches to those
//! functions by program counter and falls back to the [`interp`]reter for
//! anything that was not compiled (or whose bytes have since changed).

pub mod bus;
pub mod interp;
pub mod keys;
pub use zx_core::png;
pub mod machine;
pub mod screen;
pub mod trace;

pub use machine::*;

use std::collections::BTreeMap;

/// Runs the compiled block at `z.pc`, if there is one, and returns whether it did.
pub type BlockFn = fn(&mut Zx) -> bool;

/// Addresses the recompiled code could not handle, with hit counts. Feed
/// these back to the recompiler as extra entry points.
#[derive(Default)]
pub struct Misses {
    pub counts: BTreeMap<u16, u64>,
}

impl Zx {
    /// Runs one 50 Hz frame.
    pub fn run_frame(&mut self, code: BlockFn, misses: &mut Misses) {
        self.int_pending = true;

        let mut in_fallback = false;
        while self.t < FRAME_T {
            if self.int_pending {
                if self.iff1 && !self.ei_delay {
                    self.int_pending = false;
                    if let Some(trace) = &mut self.trace {
                        trace.on_interrupt();
                    }
                    self.accept_interrupt();
                } else if self.t >= INT_LEN {
                    self.int_pending = false;
                }
            }
            self.ei_delay = false;

            if self.halted {
                // HALT executes NOPs until the next interrupt.
                let until = if self.int_pending {
                    self.t + 4
                } else {
                    FRAME_T
                };
                let nops = (until - self.t).div_ceil(4);
                self.r = (self.r & 0x80) | (self.r.wrapping_add(nops as u8) & 0x7F);
                self.t += nops * 4;
                continue;
            }

            if code(self) {
                in_fallback = false;
                continue;
            }
            // Count each run of interpreted code once, at the address it started.
            if !in_fallback {
                *misses.counts.entry(self.pc).or_default() += 1;
                in_fallback = true;
            }
            interp::step(self);
        }
        self.t -= FRAME_T;
        self.frame += 1;
    }
}

impl Zx {
    /// Runs the interpreter with 50 Hz interrupts, like real hardware, until
    /// execution reaches `target` (after at least one instruction). Returns
    /// `false` if that takes more than `max_frames` frames.
    pub fn run_until(&mut self, target: u16, max_frames: u32) -> bool {
        self.run_until_any(&[target], max_frames)
    }

    /// Like [`Zx::run_until`] with several targets.
    pub fn run_until_any(&mut self, targets: &[u16], max_frames: u32) -> bool {
        self.run_until_any_with(targets, max_frames, |_| {})
    }

    /// Like [`Zx::run_until_any`], calling `watch` before each instruction.
    pub fn run_until_any_with(
        &mut self,
        targets: &[u16],
        max_frames: u32,
        mut watch: impl FnMut(&Zx),
    ) -> bool {
        let mut frames = 0;
        let mut first = true;
        loop {
            if self.t >= FRAME_T {
                self.t -= FRAME_T;
                self.frame += 1;
                frames += 1;
                if frames > max_frames {
                    return false;
                }
                self.int_pending = true;
            }
            if self.int_pending {
                if self.iff1 && !self.ei_delay {
                    self.int_pending = false;
                    self.accept_interrupt();
                } else if self.t >= INT_LEN {
                    self.int_pending = false;
                }
            }
            self.ei_delay = false;
            if self.halted {
                let until = if self.int_pending {
                    self.t + 4
                } else {
                    FRAME_T
                };
                let nops = (until - self.t).div_ceil(4);
                self.r = (self.r & 0x80) | (self.r.wrapping_add(nops as u8) & 0x7F);
                self.t += nops * 4;
                continue;
            }
            if !first && targets.contains(&self.pc) {
                return true;
            }
            first = false;
            watch(self);
            interp::step(self);
        }
    }
}

/// A `BlockFn` with no compiled code: everything runs in the interpreter.
pub fn no_code(_: &mut Zx) -> bool {
    false
}

pub mod prelude {
    pub use crate::machine::{CF, FRAME_T, HF, NF, PF, SF, XF, YF, ZF, Zx};
}
