//! When each instruction touches the bus, and what the ULA charges for it.
//!
//! The interpreter runs an instruction all at once and then charges its
//! T-states. That is fine until the ULA is involved: while it draws the
//! picture it stops the processor from reaching the bottom 16K of RAM, so
//! what an instruction costs depends on where it lives and when it runs.
//!
//! So the timing is kept apart from the execution: [`cycles`] says which
//! addresses an instruction puts on the bus, in what order, and for how long
//! each. What each of those costs is `zx_core::bus`, which the game's sound
//! models charge through as well, so the two cannot disagree.
//!
//! The Fuse test corpus states the bus activity of all 1335 of its cases, so
//! this is checked rather than believed: `tests/fuse.rs` compares every
//! contention point against it, and checks that the cycles add up to what the
//! decoder charges for the instruction.

pub use zx_core::bus::{Cycle, Cycles, Kind};
use zx_core::{Addr, BlockOp, Decoded, Instr, Op8};

use crate::machine::Zx;

const fn read(at: u16) -> Cycle {
    Cycle::read(at)
}

const fn write(at: u16) -> Cycle {
    Cycle::write(at)
}

const fn fetch(at: u16, len: u32) -> Cycle {
    Cycle::fetch(at, len)
}

/// `n` one-T-state cycles with `at` on the address bus.
fn idle(out: &mut Cycles, at: u16, n: u32) {
    out.idle(at, n);
}
impl Zx {
    /// The address the refresh cycle leaves on the bus, which idle cycles sit
    /// on. R has already counted the opcode fetches by then.
    fn ir(&self, m1: u8) -> u16 {
        let r = (self.r & 0x80) | (self.r.wrapping_add(m1) & 0x7F);
        (self.i as u16) << 8 | r as u16
    }

    fn addr_of(&self, a: Addr) -> u16 {
        match a {
            Addr::BC => self.bc(),
            Addr::DE => self.de(),
            Addr::HL => self.hl(),
            Addr::Idx(i, d) => self.r16(i.reg16()).wrapping_add(d as i16 as u16),
            Addr::Abs(nn) => nn,
        }
    }
}

/// Whether an operand is memory, and where.
fn mem_of(z: &Zx, o: Op8) -> Option<u16> {
    match o {
        Op8::Mem(a) => Some(z.addr_of(a)),
        _ => None,
    }
}

/// The machine cycles of the instruction `d` at `pc`, in order.
///
/// Worked out from the state before the instruction runs, which is what the
/// processor has when it puts each address on the bus.
#[must_use]
pub fn cycles(z: &Zx, d: &Decoded, pc: u16) -> Cycles {
    use Instr::*;
    let mut out = Cycles::new();

    // The opcode fetches. All are 4 T-states except DJNZ's, which is 5, and
    // the ED-prefixed ones where the second fetch is 4 as well.
    let first = if matches!(d.instr, Djnz(_)) { 5 } else { 4 };
    for i in 0..d.m1 as u16 {
        out.push(fetch(pc.wrapping_add(i), if i == 0 { first } else { 4 }));
    }

    // The bytes after the fetches: an immediate, an address, or an indexed
    // instruction's displacement.
    let operands = pc.wrapping_add(d.m1 as u16);
    let indexed = |o: Op8| matches!(o, Op8::Mem(Addr::Idx(..)));

    // A DD/FD instruction reads its displacement and then takes five cycles
    // to add it. A CB-prefixed one has already had both fetches, so the
    // displacement *and the opcode* come next as ordinary reads, and the add
    // takes two — the other three are hidden in the opcode's own read.
    let cb_indexed = d.m1 == 2 && d.len == 4;
    let displacement = |out: &mut Cycles| {
        out.push(read(operands));
        if cb_indexed {
            out.push(read(operands.wrapping_add(1)));
            idle(out, operands.wrapping_add(1), 2);
        } else {
            idle(out, operands, 5);
        }
    };

    match d.instr {
        // --- nothing beyond the fetch ---------------------------------------
        Nop | Halt | Di | Ei | ExDeHl | ExAf | Exx | Daa | Cpl | Neg | Ccf | Scf | Rlca | Rrca
        | Rla | Rra | JpInd(_) | Im(_) => {}

        // Two extra cycles on the refresh address, for the register move.
        LdIA | LdRA | LdAI | LdAR => idle(&mut out, z.ir(d.m1), 1),

        // --- 8-bit loads and arithmetic -------------------------------------
        Ld8(dst, src) => {
            // `LD (HL),n` and `LD (IX+d),n` read the byte, and the indexed
            // form takes five cycles to work the address out.
            if indexed(dst) || indexed(src) {
                // `LD (IX+d),n` reads the displacement, then the byte, then
                // takes two cycles rather than five: it has the byte to do
                // the sum behind.
                out.push(read(operands));
                if let Op8::Imm(_) = src {
                    out.push(read(operands.wrapping_add(1)));
                    idle(&mut out, operands.wrapping_add(1), 2);
                } else {
                    idle(&mut out, operands, 5);
                }
            } else if let Op8::Imm(_) = src {
                out.push(read(operands));
            } else if matches!(dst, Op8::Mem(Addr::Abs(_))) || matches!(src, Op8::Mem(Addr::Abs(_)))
            {
                // `LD (nn),A` and `LD A,(nn)` read the address first.
                out.push(read(operands));
                out.push(read(operands.wrapping_add(1)));
            }
            if let Some(a) = mem_of(z, src) {
                out.push(read(a));
            }
            if let Some(a) = mem_of(z, dst) {
                out.push(write(a));
            }
        }
        Alu(_, o) | Bit(_, o) => {
            if indexed(o) {
                displacement(&mut out);
            } else if let Op8::Imm(_) = o {
                out.push(read(operands));
            }
            if let Some(a) = mem_of(z, o) {
                out.push(read(a));
                if matches!(d.instr, Bit(..)) {
                    idle(&mut out, a, 1);
                }
            }
        }
        Inc8(o) | Dec8(o) | Rot(_, o, _) | Res(_, o, _) | Set(_, o, _) => {
            if indexed(o) {
                displacement(&mut out);
            }
            if let Some(a) = mem_of(z, o) {
                out.push(read(a));
                idle(&mut out, a, 1);
                out.push(write(a));
            }
        }
        Rld | Rrd => {
            let hl = z.hl();
            out.push(read(hl));
            idle(&mut out, hl, 4);
            out.push(write(hl));
        }

        // --- 16-bit loads ---------------------------------------------------
        Ld16(_, _) => {
            out.push(read(operands));
            out.push(read(operands.wrapping_add(1)));
        }
        Ld16Load(_, nn) => {
            out.push(read(operands));
            out.push(read(operands.wrapping_add(1)));
            out.push(read(nn));
            out.push(read(nn.wrapping_add(1)));
        }
        Ld16Store(nn, _) => {
            out.push(read(operands));
            out.push(read(operands.wrapping_add(1)));
            out.push(write(nn));
            out.push(write(nn.wrapping_add(1)));
        }
        Push(_) | Rst(_) => {
            idle(&mut out, z.ir(d.m1), 1);
            out.push(write(z.sp.wrapping_sub(1)));
            out.push(write(z.sp.wrapping_sub(2)));
        }
        ExSp(_) => {
            out.push(read(z.sp));
            out.push(read(z.sp.wrapping_add(1)));
            idle(&mut out, z.sp.wrapping_add(1), 1);
            out.push(write(z.sp.wrapping_add(1)));
            out.push(write(z.sp));
            idle(&mut out, z.sp, 2);
        }

        // --- 16-bit arithmetic ----------------------------------------------
        LdSp(_) | Inc16(_) | Dec16(_) => idle(&mut out, z.ir(d.m1), 2),
        Add16(..) | Adc16(_) | Sbc16(_) => idle(&mut out, z.ir(d.m1), 7),

        // --- control flow ---------------------------------------------------
        Jp(_, _) | Call(None, _) => {
            out.push(read(operands));
            out.push(read(operands.wrapping_add(1)));
            if let Call(..) = d.instr {
                idle(&mut out, operands.wrapping_add(1), 1);
                out.push(write(z.sp.wrapping_sub(1)));
                out.push(write(z.sp.wrapping_sub(2)));
            }
        }
        Call(Some(c), _) => {
            out.push(read(operands));
            out.push(read(operands.wrapping_add(1)));
            if z.cond(c) {
                idle(&mut out, operands.wrapping_add(1), 1);
                out.push(write(z.sp.wrapping_sub(1)));
                out.push(write(z.sp.wrapping_sub(2)));
            }
        }
        Jr(c, _) => {
            out.push(read(operands));
            if c.is_none_or(|c| z.cond(c)) {
                idle(&mut out, operands, 5);
            }
        }
        Djnz(_) => {
            out.push(read(operands));
            if z.b.wrapping_sub(1) != 0 {
                idle(&mut out, operands, 5);
            }
        }
        // An unconditional return is a POP into PC, and costs the same.
        Pop(_) | Ret(None) | Reti | Retn => {
            out.push(read(z.sp));
            out.push(read(z.sp.wrapping_add(1)));
        }
        Ret(Some(c)) => {
            idle(&mut out, z.ir(d.m1), 1);
            if z.cond(c) {
                out.push(read(z.sp));
                out.push(read(z.sp.wrapping_add(1)));
            }
        }
        // --- ports ----------------------------------------------------------
        InA(n) => {
            out.push(read(operands));
            out.push(Cycle {
                at: (z.a as u16) << 8 | n as u16,
                len: 4,
                kind: Kind::PortRead,
            });
        }
        OutA(n) => {
            out.push(read(operands));
            out.push(Cycle {
                at: (z.a as u16) << 8 | n as u16,
                len: 4,
                kind: Kind::PortWrite,
            });
        }
        InC(_) => out.push(Cycle {
            at: z.bc(),
            len: 4,
            kind: Kind::PortRead,
        }),
        OutC(_) => out.push(Cycle {
            at: z.bc(),
            len: 4,
            kind: Kind::PortWrite,
        }),

        // --- block instructions ---------------------------------------------
        Block(op) => {
            let (hl, de, bc) = (z.hl(), z.de(), z.bc());
            match op {
                BlockOp::Ldi | BlockOp::Ldd | BlockOp::Ldir | BlockOp::Lddr => {
                    out.push(read(hl));
                    out.push(write(de));
                    idle(&mut out, de, 2);
                    if op.repeats() && bc.wrapping_sub(1) != 0 {
                        idle(&mut out, de, 5);
                    }
                }
                BlockOp::Cpi | BlockOp::Cpd | BlockOp::Cpir | BlockOp::Cpdr => {
                    out.push(read(hl));
                    idle(&mut out, hl, 5);
                    if op.repeats() && bc.wrapping_sub(1) != 0 && z.a != z.read(hl) {
                        idle(&mut out, hl, 5);
                    }
                }
                BlockOp::Ini | BlockOp::Ind | BlockOp::Inir | BlockOp::Indr => {
                    idle(&mut out, z.ir(d.m1), 1);
                    out.push(Cycle {
                        at: bc,
                        len: 4,
                        kind: Kind::PortRead,
                    });
                    out.push(write(hl));
                    if op.repeats() && z.b.wrapping_sub(1) != 0 {
                        idle(&mut out, hl, 5);
                    }
                }
                BlockOp::Outi | BlockOp::Outd | BlockOp::Otir | BlockOp::Otdr => {
                    // B is counted down before the byte goes out, so the port
                    // and the cycles that follow see the new value.
                    let bc = (z.b.wrapping_sub(1) as u16) << 8 | z.c as u16;
                    idle(&mut out, z.ir(d.m1), 1);
                    out.push(read(hl));
                    out.push(Cycle {
                        at: bc,
                        len: 4,
                        kind: Kind::PortWrite,
                    });
                    if op.repeats() && z.b.wrapping_sub(1) != 0 {
                        idle(&mut out, bc, 5);
                    }
                }
            }
        }
    }
    out
}
