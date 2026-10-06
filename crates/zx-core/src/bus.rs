//! What a machine cycle costs, once the ULA has had its say.
//!
//! The Z80 puts an address on the bus for a few T-states at a time. While the
//! ULA is drawing it wants the bottom 16K for itself, and holds the processor
//! off rather than share, so what a cycle costs depends on which address it
//! names and when it happens.
//!
//! These are the primitives both sides use. The reference interpreter charges
//! an instruction by walking the cycles the decoder implies; the game's sound
//! models charge the few instructions they reproduce by hand, one cycle at a
//! time, through the same functions. That is the point of them being here:
//! the two cannot disagree about what a push costs, because there is only one
//! answer to ask.

use crate::timing::contention;

/// What the processor is doing with the bus for one machine cycle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// An opcode fetch or a memory read.
    Read,
    Write,
    /// The address is on the bus but nothing is transferred, which the ULA
    /// charges for just the same.
    Idle,
    PortRead,
    PortWrite,
}

/// One machine cycle: an address on the bus for `len` T-states.
#[derive(Clone, Copy, Debug)]
pub struct Cycle {
    pub at: u16,
    pub len: u32,
    pub kind: Kind,
}

impl Cycle {
    #[must_use]
    pub const fn read(at: u16) -> Cycle {
        Cycle {
            at,
            len: 3,
            kind: Kind::Read,
        }
    }

    #[must_use]
    pub const fn write(at: u16) -> Cycle {
        Cycle {
            at,
            len: 3,
            kind: Kind::Write,
        }
    }

    #[must_use]
    pub const fn fetch(at: u16, len: u32) -> Cycle {
        Cycle {
            at,
            len,
            kind: Kind::Read,
        }
    }
}

/// The machine cycles of one instruction.
///
/// Fixed capacity and no allocation, because this is on the interpreter's
/// hottest path: a verification run executes hundreds of millions of
/// instructions, and a heap allocation each would dominate it. The longest
/// instruction is `CPIR` repeating, at 13.
pub struct Cycles {
    buf: [Cycle; Cycles::MAX],
    len: usize,
}

impl Cycles {
    pub const MAX: usize = 20;

    #[must_use]
    pub fn new() -> Cycles {
        Cycles {
            buf: [Cycle {
                at: 0,
                len: 0,
                kind: Kind::Idle,
            }; Cycles::MAX],
            len: 0,
        }
    }

    pub fn push(&mut self, c: Cycle) {
        debug_assert!(
            self.len < Cycles::MAX,
            "an instruction with more than {} cycles",
            Cycles::MAX
        );
        if self.len < Cycles::MAX {
            self.buf[self.len] = c;
            self.len += 1;
        }
    }

    /// `n` one-T-state cycles with `at` on the address bus.
    pub fn idle(&mut self, at: u16, n: u32) {
        for _ in 0..n {
            self.push(Cycle {
                at,
                len: 1,
                kind: Kind::Idle,
            });
        }
    }
}

impl Default for Cycles {
    fn default() -> Cycles {
        Cycles::new()
    }
}

impl std::ops::Deref for Cycles {
    type Target = [Cycle];
    fn deref(&self) -> &[Cycle] {
        &self.buf[..self.len]
    }
}

/// Whether an address is in the quarter of memory the ULA shares.
#[must_use]
pub const fn contended(addr: u16) -> bool {
    0x4000 <= addr && addr < 0x8000
}

/// Charges one machine cycle onto `t`, the ULA's delay included.
pub fn charge(t: &mut u32, c: Cycle) {
    match c.kind {
        Kind::PortRead | Kind::PortWrite => charge_io(t, c.at),
        _ => {
            if contended(c.at) {
                *t += contention(*t);
            }
            *t += c.len;
        }
    }
}

/// Charges an I/O cycle, whose delays follow a different pattern from
/// memory's.
///
/// The address lines are on the bus for the whole four T-states, so a port in
/// the ULA's own range is contended when the cycle starts as well. A port
/// with bit 0 clear is the ULA's own, and it holds the processor for the
/// three T-states it takes to answer.
pub fn charge_io(t: &mut u32, port: u16) {
    let ula_range = (0x40..0x80).contains(&(port >> 8));
    let ula_port = port & 1 == 0;
    let tick = |t: &mut u32, contend: bool, len: u32| {
        if contend {
            *t += contention(*t);
        }
        *t += len;
    };
    match (ula_range, ula_port) {
        (true, true) => {
            tick(t, true, 1);
            tick(t, true, 3);
        }
        (true, false) => {
            for _ in 0..4 {
                tick(t, true, 1);
            }
        }
        (false, true) => {
            tick(t, false, 1);
            tick(t, true, 3);
        }
        (false, false) => tick(t, false, 4),
    }
}
