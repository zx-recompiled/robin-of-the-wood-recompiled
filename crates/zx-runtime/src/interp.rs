//! Fallback interpreter.
//!
//! Runs whatever the recompiler could not see statically: code reached only
//! through computed jumps that were never traced, self-modifying code, and
//! the recompiler's own trace pass. It uses the same decoder and ALU helpers
//! as the generated code, so the two agree instruction for instruction.

use crate::machine::Zx;
use zx_core::{Addr, BlockOp, Decoded, Instr, Op8, Reg8, decode};

pub fn decode_at(z: &Zx, pc: u16) -> Decoded {
    decode(&|a| z.read(a), pc)
}

fn addr(z: &Zx, a: Addr) -> u16 {
    match a {
        Addr::BC => z.bc(),
        Addr::DE => z.de(),
        Addr::HL => z.hl(),
        Addr::Idx(i, d) => z.r16(i.reg16()).wrapping_add(d as i16 as u16),
        Addr::Abs(nn) => nn,
    }
}

fn get8(z: &Zx, o: Op8) -> u8 {
    match o {
        Op8::Reg(r) => z.r8(r),
        Op8::Mem(a) => z.read(addr(z, a)),
        Op8::Imm(n) => n,
    }
}

fn set8(z: &mut Zx, o: Op8, v: u8) {
    match o {
        Op8::Reg(r) => z.set_r8(r, v),
        Op8::Mem(a) => z.write(addr(z, a), v),
        Op8::Imm(_) => unreachable!("store to immediate"),
    }
}

/// The address an indexed operand names, which is what MEMPTR becomes.
fn indexed(z: &Zx, o: Op8) -> Option<u16> {
    match o {
        Op8::Mem(a @ Addr::Idx(..)) => Some(addr(z, a)),
        _ => None,
    }
}

/// MEMPTR after an 8-bit load, which sets it only when going through an
/// index register or when A goes to or from memory by a register pair or an
/// absolute address. A store from A leaves A in the high byte.
fn ld8_memptr(z: &Zx, dst: Op8, src: Op8) -> Option<u16> {
    let a = Op8::Reg(Reg8::A);
    if let Some(at) = indexed(z, dst).or_else(|| indexed(z, src)) {
        return Some(at);
    }
    let after = |at: u16| at.wrapping_add(1);
    let stored = |at: u16| (z.a as u16) << 8 | (after(at) & 0xFF);
    match (dst, src) {
        (r, Op8::Mem(m @ (Addr::BC | Addr::DE | Addr::Abs(_)))) if r == a => {
            Some(after(addr(z, m)))
        }
        (Op8::Mem(m @ (Addr::BC | Addr::DE | Addr::Abs(_))), r) if r == a => {
            Some(stored(addr(z, m)))
        }
        _ => None,
    }
}

/// Executes one instruction at `z.pc`.
pub fn step(z: &mut Zx) {
    let pc = z.pc;
    let d = decode_at(z, pc);
    let next = pc.wrapping_add(d.len as u16);
    if z.trace.is_some() {
        let bytes = [0u16, 1, 2, 3].map(|i| z.read(pc.wrapping_add(i)));
        if let Some(trace) = &mut z.trace {
            trace.on_exec(pc, bytes, d.len);
        }
    }
    // The cycles are worked out before anything moves, because each address
    // they name is the one the processor has at the moment it puts it out.
    let cycles = crate::bus::cycles(z, &d, pc);
    z.pc = next;
    z.step(0, d.m1);
    for &c in cycles.iter() {
        z.charge(c);
    }
    // A `DD` or `FD` prefix followed by another prefix runs as a 4 T-state
    // NOP, but the processor does not take an interrupt after a prefix, so a
    // chain of them holds interrupts off for its whole length, as `EI` does
    // for one instruction. Patrik Rak's `minfo` times the interrupt with such
    // a chain.
    let lone_prefix = d.len == 1 && matches!(z.read(pc), 0xDD | 0xFD);
    execute(z, &d, pc, next);
    if lone_prefix {
        z.ei_delay = true;
    }
    z.q = if writes_flags(&d.instr) { z.f } else { 0 };
}

/// Whether an instruction computes new flags, which is what sets Q.
///
/// Loading F wholesale (`POP AF`, `EX AF,AF'`) does not count: z80test's
/// `z80ccf` shows Q is 0 after those.
fn writes_flags(i: &Instr) -> bool {
    use Instr::*;
    matches!(
        i,
        Alu(..)
            | Inc8(_)
            | Dec8(_)
            | Add16(..)
            | Adc16(_)
            | Sbc16(_)
            | Daa
            | Cpl
            | Neg
            | Ccf
            | Scf
            | Rlca
            | Rrca
            | Rla
            | Rra
            | Rld
            | Rrd
            | Rot(..)
            | Bit(..)
            | InC(_)
            | Block(_)
            | LdAI
            | LdAR
    )
}

/// Carries out the instruction. The timing has already been charged, cycle by
/// cycle, including the extra ones a taken branch or a repeat costs — `cycles`
/// tests the same conditions this does, so `t_extra` is not added again here.
fn execute(z: &mut Zx, d: &Decoded, pc: u16, next: u16) {
    use Instr::*;
    let cond = |z: &Zx, c: Option<zx_core::Cond>| c.is_none_or(|c| z.cond(c));
    match d.instr {
        Nop => {}
        // The processor holds PC on the HALT and keeps re-fetching it;
        // accepting an interrupt is what steps past it.
        Halt => {
            z.halted = true;
            z.pc = pc;
        }
        Di => z.di(),
        Ei => {
            z.ei();
            z.ei_delay = true;
        }
        Ld8(dst, src) => {
            let v = get8(z, src);
            set8(z, dst, v);
            if let Some(wz) = ld8_memptr(z, dst, src) {
                z.wz = wz;
            }
        }
        Ld16(r, nn) => z.set_r16(r, nn),
        Ld16Load(r, a) => {
            let v = z.read16(a);
            z.set_r16(r, v);
            z.wz = a.wrapping_add(1);
        }
        Ld16Store(a, r) => {
            z.write16(a, z.r16(r));
            z.wz = a.wrapping_add(1);
        }
        LdSp(r) => z.sp = z.r16(r),
        Push(r) => z.push(z.r16(r)),
        Pop(r) => {
            let v = z.pop();
            z.set_r16(r, v);
        }
        ExDeHl => z.ex_de_hl(),
        ExAf => z.ex_af(),
        Exx => z.exx(),
        ExSp(r) => {
            let v = z.read16(z.sp);
            z.write16(z.sp, z.r16(r));
            z.set_r16(r, v);
            z.wz = v;
        }
        Alu(op, src) => {
            let v = get8(z, src);
            z.alu(op, v);
            z.wz = indexed(z, src).unwrap_or(z.wz);
        }
        Inc8(o) => {
            let v = z.inc8(get8(z, o));
            set8(z, o, v);
            z.wz = indexed(z, o).unwrap_or(z.wz);
        }
        Dec8(o) => {
            let v = z.dec8(get8(z, o));
            set8(z, o, v);
            z.wz = indexed(z, o).unwrap_or(z.wz);
        }
        Inc16(r) => z.set_r16(r, z.r16(r).wrapping_add(1)),
        Dec16(r) => z.set_r16(r, z.r16(r).wrapping_sub(1)),
        Add16(dst, src) => {
            let before = z.r16(dst);
            let v = z.add16(before, z.r16(src));
            z.set_r16(dst, v);
            z.wz = before.wrapping_add(1);
        }
        Adc16(r) => {
            z.wz = z.hl().wrapping_add(1);
            z.adc_hl(z.r16(r));
        }
        Sbc16(r) => {
            z.wz = z.hl().wrapping_add(1);
            z.sbc_hl(z.r16(r));
        }
        Daa => z.daa(),
        Cpl => z.cpl(),
        Neg => z.neg(),
        Ccf => z.ccf(),
        Scf => z.scf(),
        Rlca => z.rlca(),
        Rrca => z.rrca(),
        Rla => z.rla(),
        Rra => z.rra(),
        Rld => {
            z.wz = z.hl().wrapping_add(1);
            z.rld();
        }
        Rrd => {
            z.wz = z.hl().wrapping_add(1);
            z.rrd();
        }
        Rot(op, o, copy) => {
            let v = z.rot(op, get8(z, o));
            set8(z, o, v);
            if let Some(r) = copy {
                z.set_r8(r, v);
            }
            z.wz = indexed(z, o).unwrap_or(z.wz);
        }
        Bit(n, o) => {
            let v = get8(z, o);
            z.wz = indexed(z, o).unwrap_or(z.wz);
            // Undocumented: testing a register takes flag bits 3 and 5 from
            // the byte tested, and testing memory from MEMPTR's high byte,
            // which `(IX+d)` has just set to its address.
            let undocumented = match o {
                Op8::Mem(_) => (z.wz >> 8) as u8,
                _ => v,
            };
            z.bit(n, v, undocumented);
        }
        Res(n, o, copy) | Set(n, o, copy) => {
            let v = get8(z, o);
            let v = if matches!(d.instr, Res(..)) {
                v & !(1 << n)
            } else {
                v | (1 << n)
            };
            set8(z, o, v);
            if let Some(r) = copy {
                z.set_r8(r, v);
            }
            z.wz = indexed(z, o).unwrap_or(z.wz);
        }
        // An absolute jump or call sets MEMPTR to its target whether it is
        // taken or not; a relative one only when taken.
        Jp(c, a) => {
            z.wz = a;
            if cond(z, c) {
                z.pc = a;
            }
        }
        Jr(c, a) => {
            if cond(z, c) {
                z.pc = a;
                z.wz = a;
            }
        }
        JpInd(r) => z.pc = z.r16(r),
        Djnz(a) => {
            z.b = z.b.wrapping_sub(1);
            if z.b != 0 {
                z.pc = a;
                z.wz = a;
            }
        }
        Call(c, a) => {
            z.wz = a;
            if cond(z, c) {
                z.push(next);
                z.pc = a;
            }
        }
        Ret(c) => {
            if cond(z, c) {
                z.pc = z.pop();
                z.wz = z.pc;
            }
        }
        Reti | Retn => {
            z.iff1 = z.iff2;
            z.pc = z.pop();
            z.wz = z.pc;
        }
        Rst(n) => {
            z.push(next);
            z.pc = n as u16;
            z.wz = z.pc;
        }
        InA(n) => {
            let port = (z.a as u16) << 8 | n as u16;
            z.a = z.port_in(port);
            z.wz = port.wrapping_add(1);
        }
        InC(r) => {
            let v = z.port_in(z.bc());
            z.in_flags(v);
            if let Some(r) = r {
                z.set_r8(r, v);
            }
            z.wz = z.bc().wrapping_add(1);
        }
        OutA(n) => {
            z.port_out((z.a as u16) << 8 | n as u16, z.a);
            z.wz = (z.a as u16) << 8 | (n.wrapping_add(1) as u16);
        }
        OutC(r) => {
            let v = r.map_or(0, |r| z.r8(r));
            z.port_out(z.bc(), v);
            z.wz = z.bc().wrapping_add(1);
        }
        Block(op) => {
            use BlockOp::*;
            let bc = z.bc();
            let again = z.block(op) && op.repeats();
            match op.step_op() {
                Cpi => z.wz = z.wz.wrapping_add(1),
                Cpd => z.wz = z.wz.wrapping_sub(1),
                Ini => z.wz = bc.wrapping_add(1),
                Ind => z.wz = bc.wrapping_sub(1),
                Outi => z.wz = z.bc().wrapping_add(1),
                Outd => z.wz = z.bc().wrapping_sub(1),
                _ => {}
            }
            if again {
                z.pc = pc;
                z.wz = pc.wrapping_add(1);
                z.block_repeat_flags(op, pc);
            }
        }
        Im(m) => z.im = m,
        LdIA => z.i = z.a,
        LdRA => z.r = z.a,
        LdAI => z.ld_a_i(),
        LdAR => z.ld_a_r(),
    }
}
