//! The Spectrum 48K machine state: Z80 registers, memory and ULA I/O, plus the
//! ALU helpers shared by recompiled code and the fallback interpreter.
//!
//! Recompiled code manipulates this struct directly (`z.a = z.read(z.hl());`),
//! so everything the generated code touches is public and kept flat.

use zx_core::{BlockOp, Cond, MachineState, Reg8, Reg16};

pub const CF: u8 = 0x01;
pub const NF: u8 = 0x02;
pub const PF: u8 = 0x04;
pub const XF: u8 = 0x08;
pub const HF: u8 = 0x10;
pub const YF: u8 = 0x20;
pub const ZF: u8 = 0x40;
pub const SF: u8 = 0x80;

/// T-states per frame on a 48K Spectrum (224 per line * 312 lines).
pub use zx_core::timing::FRAME_T;
/// How long the ULA holds /INT low at the start of each frame.
pub const INT_LEN: u32 = 32;

#[inline]
fn sz53(v: u8) -> u8 {
    (v & (SF | YF | XF)) | if v == 0 { ZF } else { 0 }
}

#[inline]
fn parity(v: u8) -> u8 {
    if v.count_ones().is_multiple_of(2) {
        PF
    } else {
        0
    }
}

#[inline]
fn sz53p(v: u8) -> u8 {
    sz53(v) | parity(v)
}

#[derive(Clone)]
pub struct Zx {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub a_: u8,
    pub f_: u8,
    pub b_: u8,
    pub c_: u8,
    pub d_: u8,
    pub e_: u8,
    pub h_: u8,
    pub l_: u8,
    pub ix: u16,
    pub iy: u16,
    pub sp: u16,
    pub pc: u16,
    pub i: u8,
    pub r: u8,
    pub iff1: bool,
    pub iff2: bool,
    pub im: u8,
    pub halted: bool,
    /// Set when the last executed instruction was EI: interrupts are not
    /// accepted until one more instruction has run.
    pub ei_delay: bool,
    /// Q: the flags the last instruction wrote, or 0 if it wrote none.
    ///
    /// Internal to the processor and seen only through `SCF` and `CCF`, which
    /// on a Zilog Z80 take flag bits 3 and 5 from `(Q ^ F) | A` (found in
    /// 2012; David Banks, "Undocumented Flags", hoglet67/Z80Decoder wiki).
    /// z80test's `z80ccf` checks it. Set by [`crate::interp::step`].
    pub q: u8,
    /// MEMPTR (also called WZ): an address the processor keeps internally.
    ///
    /// Seen only through `BIT n,(HL)`, which takes flag bits 3 and 5 from its
    /// high byte. Which instructions set it, and to what, is from "MEMPTR,
    /// esoteric register of the Zilog Z80" (boo_boo and Vladimir Kladov),
    /// with David Banks's 2018 findings for repeating block instructions.
    /// z80test's `z80memptr` checks only part of it: of eight of these rules
    /// broken in turn, it caught two (README, *What the reference interpreter
    /// rests on*). Set by [`crate::interp::step`].
    pub wz: u16,

    /// T-states since the start of the current frame.
    pub t: u32,
    /// The ROM and RAM, and which page each quarter of the address space
    /// shows.
    pub memory: crate::memory::Memory,

    pub border: u8,
    /// Current level of the beeper (EAR output, bit 4 of port 0xFE).
    pub ear: bool,
    /// Level of the beeper at the start of the frame.

    /// T-states within the frame at which the beeper toggled.

    /// Keyboard half-rows, one byte per address line A8..A15; a 0 bit is a pressed key.
    pub keys: [u8; 8],
    /// Kempston joystick state (bit 0 right, 1 left, 2 down, 3 up, 4 fire).
    pub kempston: u8,
    pub int_pending: bool,
    pub frame: u64,

    /// Execution/write tracing used by the recompiler's analysis pass.
    pub trace: Option<Box<crate::trace::Trace>>,

    /// Replaces the ULA for port reads.
    ///
    /// What a port read returns is the machine's business, not the
    /// processor's, and the Z80 conformance tests exercise the processor on
    /// its own. `None`, the normal case, is the real ULA.
    pub port_in_hook: Option<fn(u16) -> u8>,
}

impl Zx {
    pub fn new(state: &MachineState, rom: Option<&[u8]>) -> Zx {
        Zx {
            a: state.a,
            f: state.f,
            b: state.b,
            c: state.c,
            d: state.d,
            e: state.e,
            h: state.h,
            l: state.l,
            a_: state.a_,
            f_: state.f_,
            b_: state.b_,
            c_: state.c_,
            d_: state.d_,
            e_: state.e_,
            h_: state.h_,
            l_: state.l_,
            ix: state.ix,
            iy: state.iy,
            sp: state.sp,
            pc: state.pc,
            i: state.i,
            r: state.r,
            iff1: state.iff1,
            iff2: state.iff2,
            im: state.im,
            halted: false,
            ei_delay: false,
            q: 0,
            wz: 0,
            t: 0,
            memory: crate::memory::Memory::new_48k(&state.ram, rom),
            border: state.border,
            ear: false,

            keys: [0xFF; 8],
            kempston: 0,
            int_pending: false,
            frame: 0,
            trace: None,
            port_in_hook: None,
        }
    }

    // --- timing -------------------------------------------------------------

    /// Accounts for one instruction: `t` T-states and `m1` opcode fetches.
    #[inline(always)]
    pub fn step(&mut self, t: u8, m1: u8) {
        self.t += t as u32;
        self.r = (self.r & 0x80) | (self.r.wrapping_add(m1) & 0x7F);
    }

    // --- register pairs -----------------------------------------------------

    #[inline(always)]
    pub fn bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }
    #[inline(always)]
    pub fn de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }
    #[inline(always)]
    pub fn hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }
    #[inline(always)]
    pub fn af(&self) -> u16 {
        (self.a as u16) << 8 | self.f as u16
    }
    #[inline(always)]
    pub fn set_bc(&mut self, v: u16) {
        self.b = (v >> 8) as u8;
        self.c = v as u8;
    }
    #[inline(always)]
    pub fn set_de(&mut self, v: u16) {
        self.d = (v >> 8) as u8;
        self.e = v as u8;
    }
    #[inline(always)]
    pub fn set_hl(&mut self, v: u16) {
        self.h = (v >> 8) as u8;
        self.l = v as u8;
    }
    #[inline(always)]
    pub fn set_af(&mut self, v: u16) {
        self.a = (v >> 8) as u8;
        self.f = v as u8;
    }
    #[inline(always)]
    pub fn ixh(&self) -> u8 {
        (self.ix >> 8) as u8
    }
    #[inline(always)]
    pub fn ixl(&self) -> u8 {
        self.ix as u8
    }
    #[inline(always)]
    pub fn iyh(&self) -> u8 {
        (self.iy >> 8) as u8
    }
    #[inline(always)]
    pub fn iyl(&self) -> u8 {
        self.iy as u8
    }
    #[inline(always)]
    pub fn set_ixh(&mut self, v: u8) {
        self.ix = (self.ix & 0x00FF) | (v as u16) << 8;
    }
    #[inline(always)]
    pub fn set_ixl(&mut self, v: u8) {
        self.ix = (self.ix & 0xFF00) | v as u16;
    }
    #[inline(always)]
    pub fn set_iyh(&mut self, v: u8) {
        self.iy = (self.iy & 0x00FF) | (v as u16) << 8;
    }
    #[inline(always)]
    pub fn set_iyl(&mut self, v: u8) {
        self.iy = (self.iy & 0xFF00) | v as u16;
    }

    pub fn ex_de_hl(&mut self) {
        std::mem::swap(&mut self.d, &mut self.h);
        std::mem::swap(&mut self.e, &mut self.l);
    }

    pub fn ex_af(&mut self) {
        std::mem::swap(&mut self.a, &mut self.a_);
        std::mem::swap(&mut self.f, &mut self.f_);
    }

    pub fn exx(&mut self) {
        std::mem::swap(&mut self.b, &mut self.b_);
        std::mem::swap(&mut self.c, &mut self.c_);
        std::mem::swap(&mut self.d, &mut self.d_);
        std::mem::swap(&mut self.e, &mut self.e_);
        std::mem::swap(&mut self.h, &mut self.h_);
        std::mem::swap(&mut self.l, &mut self.l_);
    }

    // --- generic operand access (used by the interpreter) -------------------

    pub fn r8(&self, r: Reg8) -> u8 {
        match r {
            Reg8::B => self.b,
            Reg8::C => self.c,
            Reg8::D => self.d,
            Reg8::E => self.e,
            Reg8::H => self.h,
            Reg8::L => self.l,
            Reg8::A => self.a,
            Reg8::IXH => self.ixh(),
            Reg8::IXL => self.ixl(),
            Reg8::IYH => self.iyh(),
            Reg8::IYL => self.iyl(),
        }
    }

    pub fn set_r8(&mut self, r: Reg8, v: u8) {
        match r {
            Reg8::B => self.b = v,
            Reg8::C => self.c = v,
            Reg8::D => self.d = v,
            Reg8::E => self.e = v,
            Reg8::H => self.h = v,
            Reg8::L => self.l = v,
            Reg8::A => self.a = v,
            Reg8::IXH => self.set_ixh(v),
            Reg8::IXL => self.set_ixl(v),
            Reg8::IYH => self.set_iyh(v),
            Reg8::IYL => self.set_iyl(v),
        }
    }

    pub fn r16(&self, r: Reg16) -> u16 {
        match r {
            Reg16::BC => self.bc(),
            Reg16::DE => self.de(),
            Reg16::HL => self.hl(),
            Reg16::SP => self.sp,
            Reg16::AF => self.af(),
            Reg16::IX => self.ix,
            Reg16::IY => self.iy,
        }
    }

    pub fn set_r16(&mut self, r: Reg16, v: u16) {
        match r {
            Reg16::BC => self.set_bc(v),
            Reg16::DE => self.set_de(v),
            Reg16::HL => self.set_hl(v),
            Reg16::SP => self.sp = v,
            Reg16::AF => self.set_af(v),
            Reg16::IX => self.ix = v,
            Reg16::IY => self.iy = v,
        }
    }

    pub fn cond(&self, c: Cond) -> bool {
        match c {
            Cond::NZ => self.f & ZF == 0,
            Cond::Z => self.f & ZF != 0,
            Cond::NC => self.f & CF == 0,
            Cond::C => self.f & CF != 0,
            Cond::PO => self.f & PF == 0,
            Cond::PE => self.f & PF != 0,
            Cond::P => self.f & SF == 0,
            Cond::M => self.f & SF != 0,
        }
    }

    // --- ULA contention -----------------------------------------------------

    /// Charges one machine cycle, ULA delay included.
    pub fn charge(&mut self, c: crate::bus::Cycle) {
        zx_core::bus::charge(&mut self.t, c);
    }

    // --- memory -------------------------------------------------------------

    #[inline(always)]
    pub fn read(&self, addr: u16) -> u8 {
        self.memory.read(addr)
    }

    /// Writes a byte. ROM ignores writes — but only when a ROM was actually
    /// loaded; with none, the bottom 16K is just memory.
    #[inline(always)]
    pub fn write(&mut self, addr: u16, v: u8) {
        if self.memory.write(addr, v)
            && let Some(trace) = &mut self.trace
        {
            trace.on_write(addr);
        }
    }

    /// The page the ULA displays: on a 48K machine, the one at `0x4000`.
    #[must_use]
    pub fn screen_page(&self) -> usize {
        self.memory.slot(1)
    }

    #[inline(always)]
    pub fn read16(&self, addr: u16) -> u16 {
        self.read(addr) as u16 | (self.read(addr.wrapping_add(1)) as u16) << 8
    }

    #[inline(always)]
    pub fn write16(&mut self, addr: u16, v: u16) {
        self.write(addr, v as u8);
        self.write(addr.wrapping_add(1), (v >> 8) as u8);
    }

    #[inline(always)]
    pub fn push(&mut self, v: u16) {
        self.sp = self.sp.wrapping_sub(2);
        self.write16(self.sp, v);
    }

    #[inline(always)]
    pub fn pop(&mut self) -> u16 {
        let v = self.read16(self.sp);
        self.sp = self.sp.wrapping_add(2);
        v
    }

    /// Whether memory at `addr` still holds the bytes a block was compiled from.
    #[inline(always)]
    pub fn code_ok(&self, addr: u16, bytes: &[u8]) -> bool {
        // Not wrapping round past 0xFFFF: a block cannot run off the top.
        addr as usize + bytes.len() <= 0x10000
            && bytes
                .iter()
                .enumerate()
                .all(|(i, &b)| self.read(addr.wrapping_add(i as u16)) == b)
    }

    // --- I/O ----------------------------------------------------------------

    pub fn port_in(&mut self, port: u16) -> u8 {
        if let Some(read) = self.port_in_hook {
            return read(port);
        }
        if port & 1 == 0 {
            let high = (port >> 8) as u8;
            let mut keys = 0x1F;
            for (row, bits) in self.keys.iter().enumerate() {
                if high & (1 << row) == 0 {
                    keys &= bits;
                }
            }
            // Issue 3 behaviour: bit 6 follows the EAR output.
            let ear = if self.ear { 0x40 } else { 0 };
            0xA0 | ear | keys
        } else if port & 0x20 == 0 {
            self.kempston
        } else {
            0xFF
        }
    }

    pub fn port_out(&mut self, port: u16, v: u8) {
        if port & 1 == 0 {
            self.border = v & 7;
            self.ear = v & 0x10 != 0;
        }
    }

    // --- 8-bit ALU ----------------------------------------------------------

    #[inline]
    fn add8(&mut self, v: u8, carry: u8) {
        let a = self.a;
        let r16 = a as u16 + v as u16 + carry as u16;
        let r = r16 as u8;
        let overflow = if !(a ^ v) & (a ^ r) & 0x80 != 0 {
            PF
        } else {
            0
        };
        self.f = sz53(r) | ((a ^ v ^ r) & HF) | overflow | (r16 >> 8) as u8;
        self.a = r;
    }

    #[inline]
    fn sub8(&mut self, v: u8, carry: u8) -> u8 {
        let a = self.a;
        let r16 = (a as u16).wrapping_sub(v as u16).wrapping_sub(carry as u16);
        let r = r16 as u8;
        let overflow = if (a ^ v) & (a ^ r) & 0x80 != 0 { PF } else { 0 };
        let borrow = if r16 > 0xFF { CF } else { 0 };
        self.f = sz53(r) | NF | ((a ^ v ^ r) & HF) | overflow | borrow;
        r
    }

    #[inline]
    pub fn alu_add(&mut self, v: u8) {
        self.add8(v, 0);
    }
    #[inline]
    pub fn alu_adc(&mut self, v: u8) {
        self.add8(v, self.f & CF);
    }
    #[inline]
    pub fn alu_sub(&mut self, v: u8) {
        self.a = self.sub8(v, 0);
    }
    #[inline]
    pub fn alu_sbc(&mut self, v: u8) {
        self.a = self.sub8(v, self.f & CF);
    }
    #[inline]
    pub fn alu_and(&mut self, v: u8) {
        self.a &= v;
        self.f = sz53p(self.a) | HF;
    }
    #[inline]
    pub fn alu_xor(&mut self, v: u8) {
        self.a ^= v;
        self.f = sz53p(self.a);
    }
    #[inline]
    pub fn alu_or(&mut self, v: u8) {
        self.a |= v;
        self.f = sz53p(self.a);
    }
    #[inline]
    pub fn alu_cp(&mut self, v: u8) {
        self.sub8(v, 0);
        // CP takes the undocumented bits 3 and 5 from the operand.
        self.f = (self.f & !(XF | YF)) | (v & (XF | YF));
    }

    pub fn alu(&mut self, op: zx_core::AluOp, v: u8) {
        use zx_core::AluOp::*;
        match op {
            Add => self.alu_add(v),
            Adc => self.alu_adc(v),
            Sub => self.alu_sub(v),
            Sbc => self.alu_sbc(v),
            And => self.alu_and(v),
            Xor => self.alu_xor(v),
            Or => self.alu_or(v),
            Cp => self.alu_cp(v),
        }
    }

    #[inline]
    pub fn inc8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_add(1);
        let h = if v & 0x0F == 0x0F { HF } else { 0 };
        let o = if v == 0x7F { PF } else { 0 };
        self.f = (self.f & CF) | sz53(r) | h | o;
        r
    }

    #[inline]
    pub fn dec8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_sub(1);
        let h = if v & 0x0F == 0 { HF } else { 0 };
        let o = if v == 0x80 { PF } else { 0 };
        self.f = (self.f & CF) | NF | sz53(r) | h | o;
        r
    }

    pub fn daa(&mut self) {
        let a = self.a;
        let mut diff = 0u8;
        let mut carry = self.f & CF;
        if self.f & HF != 0 || a & 0x0F > 9 {
            diff |= 0x06;
        }
        if carry != 0 || a > 0x99 {
            diff |= 0x60;
            carry = CF;
        }
        let r = if self.f & NF != 0 {
            a.wrapping_sub(diff)
        } else {
            a.wrapping_add(diff)
        };
        self.f = sz53p(r) | (self.f & NF) | carry | ((a ^ r) & HF);
        self.a = r;
    }

    pub fn cpl(&mut self) {
        self.a = !self.a;
        self.f = (self.f & (SF | ZF | PF | CF)) | HF | NF | (self.a & (XF | YF));
    }

    pub fn neg(&mut self) {
        let v = self.a;
        self.a = 0;
        self.a = self.sub8(v, 0);
    }

    /// Flag bits 3 and 5 of `SCF` and `CCF`, which depend on [`Zx::q`].
    #[inline]
    fn scf_ccf_undocumented(&self) -> u8 {
        ((self.q ^ self.f) | self.a) & (XF | YF)
    }

    pub fn scf(&mut self) {
        self.f = (self.f & (SF | ZF | PF)) | self.scf_ccf_undocumented() | CF;
    }

    pub fn ccf(&mut self) {
        let hc = if self.f & CF != 0 { HF } else { CF };
        self.f = (self.f & (SF | ZF | PF)) | self.scf_ccf_undocumented() | hc;
    }

    pub fn rlca(&mut self) {
        self.a = self.a.rotate_left(1);
        self.f = (self.f & (SF | ZF | PF)) | (self.a & (XF | YF | CF));
    }

    pub fn rrca(&mut self) {
        let c = self.a & 1;
        self.a = self.a.rotate_right(1);
        self.f = (self.f & (SF | ZF | PF)) | (self.a & (XF | YF)) | c;
    }

    pub fn rla(&mut self) {
        let c = self.a >> 7;
        self.a = (self.a << 1) | (self.f & CF);
        self.f = (self.f & (SF | ZF | PF)) | (self.a & (XF | YF)) | c;
    }

    pub fn rra(&mut self) {
        let c = self.a & 1;
        self.a = (self.a >> 1) | ((self.f & CF) << 7);
        self.f = (self.f & (SF | ZF | PF)) | (self.a & (XF | YF)) | c;
    }

    pub fn rld(&mut self) {
        let hl = self.hl();
        let v = self.read(hl);
        self.write(hl, (v << 4) | (self.a & 0x0F));
        self.a = (self.a & 0xF0) | (v >> 4);
        self.f = (self.f & CF) | sz53p(self.a);
    }

    pub fn rrd(&mut self) {
        let hl = self.hl();
        let v = self.read(hl);
        self.write(hl, (self.a << 4) | (v >> 4));
        self.a = (self.a & 0xF0) | (v & 0x0F);
        self.f = (self.f & CF) | sz53p(self.a);
    }

    // --- CB rotates, shifts and bit tests ----------------------------------

    #[inline]
    fn shift_result(&mut self, r: u8, carry: u8) -> u8 {
        self.f = sz53p(r) | carry;
        r
    }

    #[inline]
    pub fn rlc(&mut self, v: u8) -> u8 {
        self.shift_result(v.rotate_left(1), v >> 7)
    }
    #[inline]
    pub fn rrc(&mut self, v: u8) -> u8 {
        self.shift_result(v.rotate_right(1), v & 1)
    }
    #[inline]
    pub fn rl(&mut self, v: u8) -> u8 {
        let r = (v << 1) | (self.f & CF);
        self.shift_result(r, v >> 7)
    }
    #[inline]
    pub fn rr(&mut self, v: u8) -> u8 {
        let r = (v >> 1) | ((self.f & CF) << 7);
        self.shift_result(r, v & 1)
    }
    #[inline]
    pub fn sla(&mut self, v: u8) -> u8 {
        self.shift_result(v << 1, v >> 7)
    }
    #[inline]
    pub fn sra(&mut self, v: u8) -> u8 {
        self.shift_result((v >> 1) | (v & 0x80), v & 1)
    }
    #[inline]
    pub fn sll(&mut self, v: u8) -> u8 {
        self.shift_result((v << 1) | 1, v >> 7)
    }
    #[inline]
    pub fn srl(&mut self, v: u8) -> u8 {
        self.shift_result(v >> 1, v & 1)
    }

    pub fn rot(&mut self, op: zx_core::RotOp, v: u8) -> u8 {
        use zx_core::RotOp::*;
        match op {
            Rlc => self.rlc(v),
            Rrc => self.rrc(v),
            Rl => self.rl(v),
            Rr => self.rr(v),
            Sla => self.sla(v),
            Sra => self.sra(v),
            Sll => self.sll(v),
            Srl => self.srl(v),
        }
    }

    #[inline]
    /// Tests bit `n` of `v`.
    ///
    /// Flag bits 3 and 5 are undocumented and do not come from `v` for every
    /// form of the instruction, so the caller says where they come from:
    /// `BIT n,(IX+d)` takes them from the high byte of the address it just
    /// worked out, and every other form from the byte tested.
    pub fn bit(&mut self, n: u8, v: u8, undocumented: u8) {
        let set = v & (1 << n);
        let zp = if set == 0 { ZF | PF } else { 0 };
        let s = if n == 7 && set != 0 { SF } else { 0 };
        self.f = (self.f & CF) | HF | (undocumented & (XF | YF)) | zp | s;
    }

    // --- 16-bit ALU ---------------------------------------------------------

    #[inline]
    pub fn add16(&mut self, a: u16, b: u16) -> u16 {
        let r32 = a as u32 + b as u32;
        let r = r32 as u16;
        let h = (((a ^ b ^ r) >> 8) as u8) & HF;
        self.f = (self.f & (SF | ZF | PF)) | ((r >> 8) as u8 & (XF | YF)) | h | (r32 >> 16) as u8;
        r
    }

    pub fn adc_hl(&mut self, v: u16) {
        let hl = self.hl();
        let r32 = hl as u32 + v as u32 + (self.f & CF) as u32;
        let r = r32 as u16;
        let hi = (r >> 8) as u8;
        let overflow = if !(hl ^ v) & (hl ^ r) & 0x8000 != 0 {
            PF
        } else {
            0
        };
        self.f = (hi & (SF | XF | YF))
            | if r == 0 { ZF } else { 0 }
            | (((hl ^ v ^ r) >> 8) as u8 & HF)
            | overflow
            | (r32 >> 16) as u8;
        self.set_hl(r);
    }

    pub fn sbc_hl(&mut self, v: u16) {
        let hl = self.hl();
        let r32 = (hl as u32)
            .wrapping_sub(v as u32)
            .wrapping_sub((self.f & CF) as u32);
        let r = r32 as u16;
        let hi = (r >> 8) as u8;
        let overflow = if (hl ^ v) & (hl ^ r) & 0x8000 != 0 {
            PF
        } else {
            0
        };
        self.f = (hi & (SF | XF | YF))
            | if r == 0 { ZF } else { 0 }
            | NF
            | (((hl ^ v ^ r) >> 8) as u8 & HF)
            | overflow
            | if r32 > 0xFFFF { CF } else { 0 };
        self.set_hl(r);
    }

    // --- misc ---------------------------------------------------------------

    pub fn ld_a_i(&mut self) {
        self.a = self.i;
        self.f = (self.f & CF) | sz53(self.a) | if self.iff2 { PF } else { 0 };
    }

    pub fn ld_a_r(&mut self) {
        self.a = self.r;
        self.f = (self.f & CF) | sz53(self.a) | if self.iff2 { PF } else { 0 };
    }

    /// Flags for `IN r,(C)`.
    pub fn in_flags(&mut self, v: u8) {
        self.f = (self.f & CF) | sz53p(v);
    }

    pub fn ei(&mut self) {
        self.iff1 = true;
        self.iff2 = true;
    }

    pub fn di(&mut self) {
        self.iff1 = false;
        self.iff2 = false;
    }

    // --- block instructions -------------------------------------------------
    // Each performs one iteration and returns whether the repeating form of
    // the instruction would go round again.

    fn ld_block(&mut self, delta: u16) -> bool {
        let v = self.read(self.hl());
        self.write(self.de(), v);
        self.set_hl(self.hl().wrapping_add(delta));
        self.set_de(self.de().wrapping_add(delta));
        self.set_bc(self.bc().wrapping_sub(1));
        let n = v.wrapping_add(self.a);
        let pv = if self.bc() != 0 { PF } else { 0 };
        self.f = (self.f & (SF | ZF | CF)) | pv | (n & XF) | ((n << 4) & YF);
        self.bc() != 0
    }

    pub fn ldi(&mut self) -> bool {
        self.ld_block(1)
    }

    pub fn ldd(&mut self) -> bool {
        self.ld_block(0xFFFF)
    }

    fn cp_block(&mut self, delta: u16) -> bool {
        let v = self.read(self.hl());
        let a = self.a;
        let r = a.wrapping_sub(v);
        self.set_hl(self.hl().wrapping_add(delta));
        self.set_bc(self.bc().wrapping_sub(1));
        let h = (a ^ v ^ r) & HF;
        let n = r.wrapping_sub(if h != 0 { 1 } else { 0 });
        let pv = if self.bc() != 0 { PF } else { 0 };
        let z = if r == 0 { ZF } else { 0 };
        self.f = (self.f & CF) | NF | (r & SF) | z | h | pv | (n & XF) | ((n << 4) & YF);
        self.bc() != 0 && r != 0
    }

    pub fn cpi(&mut self) -> bool {
        self.cp_block(1)
    }

    pub fn cpd(&mut self) -> bool {
        self.cp_block(0xFFFF)
    }

    fn io_block_flags(&mut self, v: u8, k: u16) {
        let n = if v & 0x80 != 0 { NF } else { 0 };
        let hc = if k > 0xFF { HF | CF } else { 0 };
        self.f = sz53(self.b) | n | hc | parity((k as u8 & 7) ^ self.b);
    }

    fn in_block(&mut self, delta: u16) -> bool {
        let v = self.port_in(self.bc());
        self.write(self.hl(), v);
        self.set_hl(self.hl().wrapping_add(delta));
        self.b = self.b.wrapping_sub(1);
        let k = v as u16 + self.c.wrapping_add(delta as u8) as u16;
        self.io_block_flags(v, k);
        self.b != 0
    }

    pub fn ini(&mut self) -> bool {
        self.in_block(1)
    }

    pub fn ind(&mut self) -> bool {
        self.in_block(0xFFFF)
    }

    fn out_block(&mut self, delta: u16) -> bool {
        let v = self.read(self.hl());
        self.b = self.b.wrapping_sub(1);
        self.port_out(self.bc(), v);
        self.set_hl(self.hl().wrapping_add(delta));
        let k = v as u16 + self.l as u16;
        self.io_block_flags(v, k);
        self.b != 0
    }

    pub fn outi(&mut self) -> bool {
        self.out_block(1)
    }

    pub fn outd(&mut self) -> bool {
        self.out_block(0xFFFF)
    }

    /// What a repeating block instruction does to the flags when it goes
    /// round again, on top of what one iteration did.
    ///
    /// Bits 3 and 5 come from the high byte of `pc`, the instruction's own
    /// address. The I/O instructions also change H and P/V. Found by David
    /// Banks in 2018, as MAME implements it; z80test's `z80full` checks it.
    pub fn block_repeat_flags(&mut self, op: BlockOp, pc: u16) {
        use BlockOp::*;
        self.f = (self.f & !(XF | YF)) | ((pc >> 8) as u8 & (XF | YF));
        if !matches!(op.step_op(), Ini | Ind | Outi | Outd) {
            return;
        }
        // The value of P/V changes by the parity of `x`.
        let toggle = |f: &mut u8, x: u8| *f ^= (parity(x & 7) ^ PF) & PF;
        let b = self.b;
        if self.f & CF != 0 {
            self.f &= !HF;
            // N is bit 7 of the byte transferred.
            if self.f & NF != 0 {
                toggle(&mut self.f, b.wrapping_sub(1));
                if b & 0x0F == 0x00 {
                    self.f |= HF;
                }
            } else {
                toggle(&mut self.f, b.wrapping_add(1));
                if b & 0x0F == 0x0F {
                    self.f |= HF;
                }
            }
        } else {
            toggle(&mut self.f, b);
        }
    }

    pub fn block(&mut self, op: BlockOp) -> bool {
        use BlockOp::*;
        match op.step_op() {
            Ldi => self.ldi(),
            Ldd => self.ldd(),
            Cpi => self.cpi(),
            Cpd => self.cpd(),
            Ini => self.ini(),
            Ind => self.ind(),
            Outi => self.outi(),
            _ => self.outd(),
        }
    }

    /// Calls the subroutine at `addr` in the interpreter and runs it until it
    /// returns, with interrupts ignored. Returns `false` if it had not
    /// returned after `max_instrs` instructions. Used to run pieces of the
    /// original program in isolation (for example to compare a rewritten
    /// routine's output against the original's).
    pub fn call(&mut self, addr: u16, max_instrs: u64) -> bool {
        self.call_until(addr, None, max_instrs)
    }

    /// Like [`Zx::call`], but also stops (returning `true`) when execution
    /// reaches `stop`, for running just the first part of a routine.
    pub fn call_until(&mut self, addr: u16, stop: Option<u16>, max_instrs: u64) -> bool {
        self.call_until_any(addr, stop.as_slice(), max_instrs)
    }

    /// Like [`Zx::call_until`] with several stop addresses.
    pub fn call_until_any(&mut self, addr: u16, stops: &[u16], max_instrs: u64) -> bool {
        self.call_until_any_with(addr, stops, max_instrs, |_| {}).0
    }

    /// Like [`Zx::call_until_any`], and also how long the call took.
    ///
    /// The clock is put back afterwards, so this is the only way to find out.
    /// Timing a routine is what the sound checks do.
    pub fn call_until_any_timed(
        &mut self,
        addr: u16,
        stops: &[u16],
        max_instrs: u64,
    ) -> (bool, u32) {
        self.call_until_any_with(addr, stops, max_instrs, |_| {})
    }

    /// Like [`Zx::call_until_any`], calling `watch` before each instruction,
    /// and reporting how long the call took.
    ///
    /// Some of what a routine does leaves no trace in memory afterwards --
    /// the blocking sound requests are just calls -- so the only way to
    /// compare them is to watch the original as it runs.
    pub fn call_until_any_with(
        &mut self,
        addr: u16,
        stops: &[u16],
        max_instrs: u64,
        mut watch: impl FnMut(&Zx),
    ) -> (bool, u32) {
        const SENTINEL: u16 = 0x0000;
        let sp = self.sp;
        // A call is not a frame. No interrupt arrives during one, so a HALT
        // falls through to the next instruction rather than waiting for ever,
        // and the T-states spent belong to whatever frame the caller is in.
        // All of it is put back afterwards: leaving `t` at twelve frames'
        // worth and `halted` set used to give the next `run_until` an
        // interrupt on each of its first dozen instructions.
        let (t, halted, ei_delay) = (self.t, self.halted, self.ei_delay);
        self.halted = false;
        self.ei_delay = false;
        self.push(SENTINEL);
        self.pc = addr;
        let mut reached = false;
        for _ in 0..max_instrs {
            watch(self);
            crate::interp::step(self);
            if (self.pc == SENTINEL && self.sp == sp) || stops.contains(&self.pc) {
                reached = true;
                break;
            }
        }
        let elapsed = self.t.wrapping_sub(t);
        self.t = t;
        self.halted = halted;
        self.ei_delay = ei_delay;
        (reached, elapsed)
    }

    // --- interrupts ---------------------------------------------------------

    pub fn accept_interrupt(&mut self) {
        // A halted processor sits on the HALT instruction; leaving the halt
        // state is what steps past it, so the return address is the one
        // after it.
        if self.halted {
            self.pc = self.pc.wrapping_add(1);
        }
        self.halted = false;
        self.q = 0;
        self.di();
        self.push(self.pc);
        if self.im == 2 {
            // The Spectrum's data bus floats at 0xFF during the acknowledge.
            let vector = (self.i as u16) << 8 | 0xFF;
            self.pc = self.read16(vector);
            self.step(19, 1);
        } else {
            self.pc = 0x0038;
            self.step(13, 1);
        }
        self.wz = self.pc;
    }
}
