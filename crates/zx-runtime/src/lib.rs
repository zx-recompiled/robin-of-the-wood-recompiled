//! Runtime for statically recompiled ZX Spectrum 48K programs.
//!
//! The recompiler turns each block of Z80 code into a Rust function that
//! operates on [`Zx`]. At run time [`Zx::run_frame`] dispatches to those
//! functions by program counter and falls back to the [`interp`]reter for
//! anything that was not compiled (or whose bytes have since changed).

pub mod ay;
pub mod bus;
pub mod interp;
pub mod keys;
pub mod loader;
pub use zx_core::png;
pub mod machine;
pub mod memory;
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
    ///
    /// `code` is called before each instruction: if it returns true it has
    /// dealt with that point itself (run compiled code, or stood in for a
    /// ROM routine, as [`crate::loader::TapeFeeder`] does), and the loop
    /// looks again at wherever it left the processor.
    pub fn run_frame(&mut self, mut code: impl FnMut(&mut Zx) -> bool, misses: &mut Misses) {
        self.int_pending = true;

        let mut in_fallback = false;
        while self.t < self.timing.frame {
            if self.interrupt_or_halt() {
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
        self.t -= self.timing.frame;
        self.frame += 1;
    }

    /// One step of what [`Zx::run_frame`] does, for a caller that has to stop
    /// between any two instructions: starts the next frame if this one is
    /// over, takes an interrupt if one is due, and runs one instruction, or
    /// the NOPs of a HALT. A frame run this way is the same as one run by
    /// [`Zx::run_frame`] with no `code`.
    pub fn step_in_frame(&mut self) {
        if self.t >= self.timing.frame {
            self.t -= self.timing.frame;
            self.frame += 1;
            self.int_pending = true;
        }
        if !self.interrupt_or_halt() {
            interp::step(self);
        }
    }

    /// Before each instruction of a frame: ends the interrupt pulse when it
    /// has run its length, takes an interrupt if one is due, and runs a
    /// halted processor's NOPs. Returns whether it did the latter, in which
    /// case there is no instruction to run.
    fn interrupt_or_halt(&mut self) -> bool {
        // /INT is held for the first `int_len` T-states of the frame, and
        // only then: once that has passed it is gone, whether or not
        // interrupts were enabled to see it. Checking that first matters:
        // a pulse that lasted until the next enabled instruction instead
        // made Patrik Rak's `minfo` measure it 8 T-states too long.
        if self.int_pending && self.t >= self.timing.int_len {
            self.int_pending = false;
        }
        if self.int_pending && self.iff1 && !self.ei_delay {
            self.int_pending = false;
            if self.trace.is_some() {
                // A halted processor returns past its HALT.
                let back = self.pc.wrapping_add(u16::from(self.halted));
                let (slots, sp) = (self.memory.slots(), self.sp);
                if let Some(trace) = &mut self.trace {
                    trace.on_interrupt(&slots, back, sp);
                }
            }
            self.accept_interrupt();
        }
        self.ei_delay = false;

        if self.halted {
            // HALT executes NOPs until the next interrupt.
            let until = if self.int_pending {
                self.t + 4
            } else {
                self.timing.frame
            };
            let nops = (until - self.t).div_ceil(4);
            self.r = (self.r & 0x80) | (self.r.wrapping_add(nops as u8) & 0x7F);
            self.t += nops * 4;
            return true;
        }
        false
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
        self.run_until_any_with(targets, max_frames, |_| false)
    }

    /// Like [`Zx::run_until_any`], calling `hook` before each instruction. As
    /// in [`Zx::run_frame`], a hook that returns true has dealt with that
    /// point itself (stood in for a ROM routine, say), and the loop looks
    /// again at wherever it left the processor.
    pub fn run_until_any_with(
        &mut self,
        targets: &[u16],
        max_frames: u32,
        mut hook: impl FnMut(&mut Zx) -> bool,
    ) -> bool {
        let mut frames = 0;
        let mut first = true;
        loop {
            if self.t >= self.timing.frame {
                self.t -= self.timing.frame;
                self.frame += 1;
                frames += 1;
                if frames > max_frames {
                    return false;
                }
                self.int_pending = true;
            }
            // As in `run_frame`: the pulse ends at `int_len`, enabled or not.
            if self.int_pending && self.t >= self.timing.int_len {
                self.int_pending = false;
            }
            if self.int_pending && self.iff1 && !self.ei_delay {
                self.int_pending = false;
                self.accept_interrupt();
            }
            self.ei_delay = false;
            if self.halted {
                let until = if self.int_pending {
                    self.t + 4
                } else {
                    self.timing.frame
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
            if hook(self) {
                continue;
            }
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
