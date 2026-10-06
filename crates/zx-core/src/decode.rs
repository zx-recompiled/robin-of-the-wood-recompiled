//! Z80 instruction decoder.
//!
//! Decoding follows the x/y/z/p/q opcode decomposition described in
//! "Decoding Z80 Opcodes" (Cristian Dinu). Undocumented instructions (IXH/IXL
//! halves, SLL, DDCB register copies, ED "NONI" NOPs) are decoded because
//! Spectrum games do use them.

use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reg8 {
    B,
    C,
    D,
    E,
    H,
    L,
    A,
    IXH,
    IXL,
    IYH,
    IYL,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reg16 {
    BC,
    DE,
    HL,
    SP,
    AF,
    IX,
    IY,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Idx {
    IX,
    IY,
}

impl Idx {
    pub fn reg16(self) -> Reg16 {
        match self {
            Idx::IX => Reg16::IX,
            Idx::IY => Reg16::IY,
        }
    }
}

/// A memory operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Addr {
    BC,
    DE,
    HL,
    Idx(Idx, i8),
    Abs(u16),
}

/// An 8-bit operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op8 {
    Reg(Reg8),
    Mem(Addr),
    Imm(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cond {
    NZ,
    Z,
    NC,
    C,
    PO,
    PE,
    P,
    M,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AluOp {
    Add,
    Adc,
    Sub,
    Sbc,
    And,
    Xor,
    Or,
    Cp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RotOp {
    Rlc,
    Rrc,
    Rl,
    Rr,
    Sla,
    Sra,
    Sll,
    Srl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockOp {
    Ldi,
    Ldd,
    Ldir,
    Lddr,
    Cpi,
    Cpd,
    Cpir,
    Cpdr,
    Ini,
    Ind,
    Inir,
    Indr,
    Outi,
    Outd,
    Otir,
    Otdr,
}

impl BlockOp {
    pub fn repeats(self) -> bool {
        use BlockOp::*;
        matches!(self, Ldir | Lddr | Cpir | Cpdr | Inir | Indr | Otir | Otdr)
    }

    /// The single-step operation a repeating instruction performs each iteration.
    pub fn step_op(self) -> BlockOp {
        use BlockOp::*;
        match self {
            Ldir => Ldi,
            Lddr => Ldd,
            Cpir => Cpi,
            Cpdr => Cpd,
            Inir => Ini,
            Indr => Ind,
            Otir => Outi,
            Otdr => Outd,
            op => op,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Instr {
    Nop,
    Halt,
    Di,
    Ei,
    /// `LD dst,src` for every 8-bit form.
    Ld8(Op8, Op8),
    /// `LD rr,nn`
    Ld16(Reg16, u16),
    /// `LD rr,(nn)`
    Ld16Load(Reg16, u16),
    /// `LD (nn),rr`
    Ld16Store(u16, Reg16),
    /// `LD SP,HL/IX/IY`
    LdSp(Reg16),
    Push(Reg16),
    Pop(Reg16),
    ExDeHl,
    ExAf,
    Exx,
    /// `EX (SP),HL/IX/IY`
    ExSp(Reg16),
    Alu(AluOp, Op8),
    Inc8(Op8),
    Dec8(Op8),
    Inc16(Reg16),
    Dec16(Reg16),
    Add16(Reg16, Reg16),
    Adc16(Reg16),
    Sbc16(Reg16),
    Daa,
    Cpl,
    Neg,
    Ccf,
    Scf,
    Rlca,
    Rrca,
    Rla,
    Rra,
    Rld,
    Rrd,
    /// CB rotates/shifts. The register is the undocumented DDCB copy target.
    Rot(RotOp, Op8, Option<Reg8>),
    Bit(u8, Op8),
    Res(u8, Op8, Option<Reg8>),
    Set(u8, Op8, Option<Reg8>),
    Jp(Option<Cond>, u16),
    /// `JP (HL/IX/IY)`
    JpInd(Reg16),
    Jr(Option<Cond>, u16),
    Djnz(u16),
    Call(Option<Cond>, u16),
    Ret(Option<Cond>),
    Reti,
    Retn,
    Rst(u8),
    /// `IN A,(n)`
    InA(u8),
    /// `IN r,(C)`; `None` is the flags-only `IN (C)`.
    InC(Option<Reg8>),
    /// `OUT (n),A`
    OutA(u8),
    /// `OUT (C),r`; `None` is `OUT (C),0`.
    OutC(Option<Reg8>),
    Block(BlockOp),
    Im(u8),
    LdIA,
    LdRA,
    LdAI,
    LdAR,
}

/// How an instruction affects control flow, as far as can be known statically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Always continues with the next instruction.
    Next,
    /// Always transfers to the given address.
    Jump(u16),
    /// Either the given address or the next instruction.
    Branch(u16),
    /// Pushes the next address and jumps (the call may or may not be taken).
    Call { target: u16, conditional: bool },
    /// Returns (the return may or may not be taken).
    Return { conditional: bool },
    /// Jumps to an address only known at run time.
    Indirect,
    /// Stops until the next interrupt, then continues with the next instruction.
    Halt,
}

impl Instr {
    pub fn flow(&self) -> Flow {
        use Instr::*;
        match *self {
            Jp(None, a) | Jr(None, a) => Flow::Jump(a),
            Jp(Some(_), a) | Jr(Some(_), a) | Djnz(a) => Flow::Branch(a),
            Call(c, a) => Flow::Call {
                target: a,
                conditional: c.is_some(),
            },
            Rst(n) => Flow::Call {
                target: n as u16,
                conditional: false,
            },
            Ret(c) => Flow::Return {
                conditional: c.is_some(),
            },
            Reti | Retn => Flow::Return { conditional: false },
            JpInd(_) => Flow::Indirect,
            Halt => Flow::Halt,
            Block(op) if op.repeats() => Flow::Next,
            _ => Flow::Next,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded {
    pub instr: Instr,
    /// Length in bytes.
    pub len: u8,
    /// T-states. For conditional instructions, the not-taken time; for
    /// repeating block instructions, the time of the final iteration.
    pub t: u8,
    /// Extra T-states when a conditional branch is taken or a block
    /// instruction repeats.
    pub t_extra: u8,
    /// Opcode fetches (how far the R register advances).
    pub m1: u8,
}

struct Cursor<'a, F> {
    mem: &'a F,
    pc: u16,
    len: u8,
}

impl<F: Fn(u16) -> u8> Cursor<'_, F> {
    fn byte(&mut self) -> u8 {
        let v = (self.mem)(self.pc.wrapping_add(self.len as u16));
        self.len += 1;
        v
    }

    fn word(&mut self) -> u16 {
        let lo = self.byte() as u16;
        lo | (self.byte() as u16) << 8
    }

    /// Reads a relative displacement and returns the absolute target.
    fn rel(&mut self) -> u16 {
        let d = self.byte() as i8;
        self.pc
            .wrapping_add(self.len as u16)
            .wrapping_add(d as i16 as u16)
    }
}

const CC: [Cond; 8] = [
    Cond::NZ,
    Cond::Z,
    Cond::NC,
    Cond::C,
    Cond::PO,
    Cond::PE,
    Cond::P,
    Cond::M,
];

const ALU: [AluOp; 8] = [
    AluOp::Add,
    AluOp::Adc,
    AluOp::Sub,
    AluOp::Sbc,
    AluOp::And,
    AluOp::Xor,
    AluOp::Or,
    AluOp::Cp,
];

const ROT: [RotOp; 8] = [
    RotOp::Rlc,
    RotOp::Rrc,
    RotOp::Rl,
    RotOp::Rr,
    RotOp::Sla,
    RotOp::Sra,
    RotOp::Sll,
    RotOp::Srl,
];

fn reg_plain(i: u8) -> Reg8 {
    match i {
        0 => Reg8::B,
        1 => Reg8::C,
        2 => Reg8::D,
        3 => Reg8::E,
        4 => Reg8::H,
        5 => Reg8::L,
        7 => Reg8::A,
        _ => unreachable!("register index 6 is (HL)"),
    }
}

fn reg_idx(i: u8, idx: Option<Idx>) -> Reg8 {
    match (i, idx) {
        (4, Some(Idx::IX)) => Reg8::IXH,
        (5, Some(Idx::IX)) => Reg8::IXL,
        (4, Some(Idx::IY)) => Reg8::IYH,
        (5, Some(Idx::IY)) => Reg8::IYL,
        _ => reg_plain(i),
    }
}

fn rp(p: u8, hl: Reg16) -> Reg16 {
    [Reg16::BC, Reg16::DE, hl, Reg16::SP][p as usize]
}

fn rp2(p: u8, hl: Reg16) -> Reg16 {
    [Reg16::BC, Reg16::DE, hl, Reg16::AF][p as usize]
}

/// Decodes the instruction at `pc`, reading bytes through `mem`.
pub fn decode<F: Fn(u16) -> u8>(mem: &F, pc: u16) -> Decoded {
    let mut c = Cursor { mem, pc, len: 0 };
    match c.byte() {
        0xCB => decode_cb(&mut c),
        0xED => decode_ed(&mut c),
        op @ (0xDD | 0xFD) => {
            let idx = if op == 0xDD { Idx::IX } else { Idx::IY };
            match mem(pc.wrapping_add(1)) {
                // A prefix followed by another prefix acts as a 4 T-state NOP.
                0xDD | 0xFD | 0xED => Decoded {
                    instr: Instr::Nop,
                    len: 1,
                    t: 4,
                    t_extra: 0,
                    m1: 1,
                },
                0xCB => decode_idx_cb(&mut c, idx),
                _ => decode_main(&mut c, Some(idx)),
            }
        }
        _ => {
            c.len = 0;
            decode_main(&mut c, None)
        }
    }
}

fn decode_main<F: Fn(u16) -> u8>(c: &mut Cursor<F>, idx: Option<Idx>) -> Decoded {
    use Instr::*;
    let op = c.byte();
    let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
    let (p, q) = (y >> 1, y & 1);
    let hl = idx.map_or(Reg16::HL, Idx::reg16);

    // Set when the instruction uses (IX+d)/(IY+d); timing then differs.
    let mut idx_mem = false;
    // Extra T-states of the (IX+d) form over the (HL) form.
    let mut idx_mem_extra = 12;
    let mut op8 = |c: &mut Cursor<F>, i: u8| -> Op8 {
        if i == 6 {
            match idx {
                None => Op8::Mem(Addr::HL),
                Some(ix) => {
                    idx_mem = true;
                    Op8::Mem(Addr::Idx(ix, c.byte() as i8))
                }
            }
        } else {
            Op8::Reg(reg_idx(i, idx))
        }
    };

    let (instr, t, t_extra): (Instr, u8, u8) = match x {
        0 => match z {
            0 => match y {
                0 => (Nop, 4, 0),
                1 => (ExAf, 4, 0),
                2 => (Djnz(c.rel()), 8, 5),
                3 => (Jr(None, c.rel()), 12, 0),
                _ => (Jr(Some(CC[(y - 4) as usize]), c.rel()), 7, 5),
            },
            1 if q == 0 => (Ld16(rp(p, hl), c.word()), 10, 0),
            1 => (Add16(hl, rp(p, hl)), 11, 0),
            2 => {
                let a = Op8::Reg(Reg8::A);
                match (q, p) {
                    (0, 0) => (Ld8(Op8::Mem(Addr::BC), a), 7, 0),
                    (0, 1) => (Ld8(Op8::Mem(Addr::DE), a), 7, 0),
                    (0, 2) => (Ld16Store(c.word(), hl), 16, 0),
                    (0, _) => (Ld8(Op8::Mem(Addr::Abs(c.word())), a), 13, 0),
                    (_, 0) => (Ld8(a, Op8::Mem(Addr::BC)), 7, 0),
                    (_, 1) => (Ld8(a, Op8::Mem(Addr::DE)), 7, 0),
                    (_, 2) => (Ld16Load(hl, c.word()), 16, 0),
                    (_, _) => (Ld8(a, Op8::Mem(Addr::Abs(c.word()))), 13, 0),
                }
            }
            3 if q == 0 => (Inc16(rp(p, hl)), 6, 0),
            3 => (Dec16(rp(p, hl)), 6, 0),
            4 | 5 => {
                let o = op8(c, y);
                let t = if matches!(o, Op8::Mem(_)) { 11 } else { 4 };
                (if z == 4 { Inc8(o) } else { Dec8(o) }, t, 0)
            }
            6 => {
                let o = op8(c, y);
                let n = c.byte();
                idx_mem_extra = 9;
                (
                    Ld8(o, Op8::Imm(n)),
                    if matches!(o, Op8::Mem(_)) { 10 } else { 7 },
                    0,
                )
            }
            _ => {
                let i = [Rlca, Rrca, Rla, Rra, Daa, Cpl, Scf, Ccf][y as usize];
                (i, 4, 0)
            }
        },
        1 => {
            if y == 6 && z == 6 {
                (Halt, 4, 0)
            } else if z == 6 {
                // With (IX+d), the other operand stays H/L rather than IXH/IXL.
                let src = op8(c, 6);
                (Ld8(Op8::Reg(reg_plain(y)), src), 7, 0)
            } else if y == 6 {
                let dst = op8(c, 6);
                (Ld8(dst, Op8::Reg(reg_plain(z))), 7, 0)
            } else {
                (
                    Ld8(Op8::Reg(reg_idx(y, idx)), Op8::Reg(reg_idx(z, idx))),
                    4,
                    0,
                )
            }
        }
        2 => {
            let o = op8(c, z);
            let t = if matches!(o, Op8::Mem(_)) { 7 } else { 4 };
            (Alu(ALU[y as usize], o), t, 0)
        }
        _ => match z {
            0 => (Ret(Some(CC[y as usize])), 5, 6),
            1 if q == 0 => (Pop(rp2(p, hl)), 10, 0),
            1 => match p {
                0 => (Ret(None), 10, 0),
                1 => (Exx, 4, 0),
                2 => (JpInd(hl), 4, 0),
                _ => (LdSp(hl), 6, 0),
            },
            2 => (Jp(Some(CC[y as usize]), c.word()), 10, 0),
            3 => match y {
                0 => (Jp(None, c.word()), 10, 0),
                2 => (OutA(c.byte()), 11, 0),
                3 => (InA(c.byte()), 11, 0),
                4 => (ExSp(hl), 19, 0),
                5 => (ExDeHl, 4, 0),
                6 => (Di, 4, 0),
                7 => (Ei, 4, 0),
                _ => unreachable!("CB prefix is decoded separately"),
            },
            4 => (Call(Some(CC[y as usize]), c.word()), 10, 7),
            5 if q == 0 => (Push(rp2(p, hl)), 11, 0),
            5 if p == 0 => (Call(None, c.word()), 17, 0),
            5 => unreachable!("prefix bytes are decoded separately"),
            6 => (Alu(ALU[y as usize], Op8::Imm(c.byte())), 7, 0),
            _ => (Rst(y * 8), 11, 0),
        },
    };

    let t = match idx {
        Some(_) if idx_mem => t + idx_mem_extra,
        Some(_) => t + 4,
        None => t,
    };
    Decoded {
        instr,
        len: c.len,
        t,
        t_extra,
        m1: if idx.is_some() { 2 } else { 1 },
    }
}

fn decode_cb<F: Fn(u16) -> u8>(c: &mut Cursor<F>) -> Decoded {
    let op = c.byte();
    let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
    let target = if z == 6 {
        Op8::Mem(Addr::HL)
    } else {
        Op8::Reg(reg_plain(z))
    };
    let instr = match x {
        0 => Instr::Rot(ROT[y as usize], target, None),
        1 => Instr::Bit(y, target),
        2 => Instr::Res(y, target, None),
        _ => Instr::Set(y, target, None),
    };
    let t = match (z == 6, x == 1) {
        (false, _) => 8,
        (true, true) => 12,
        (true, false) => 15,
    };
    Decoded {
        instr,
        len: 2,
        t,
        t_extra: 0,
        m1: 2,
    }
}

fn decode_idx_cb<F: Fn(u16) -> u8>(c: &mut Cursor<F>, idx: Idx) -> Decoded {
    c.byte(); // CB
    let d = c.byte() as i8;
    let op = c.byte();
    let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
    let target = Op8::Mem(Addr::Idx(idx, d));
    let copy = if z == 6 { None } else { Some(reg_plain(z)) };
    let instr = match x {
        0 => Instr::Rot(ROT[y as usize], target, copy),
        1 => Instr::Bit(y, target),
        2 => Instr::Res(y, target, copy),
        _ => Instr::Set(y, target, copy),
    };
    Decoded {
        instr,
        len: 4,
        t: if x == 1 { 20 } else { 23 },
        t_extra: 0,
        m1: 2,
    }
}

fn decode_ed<F: Fn(u16) -> u8>(c: &mut Cursor<F>) -> Decoded {
    use Instr::*;
    let op = c.byte();
    let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
    let (p, q) = (y >> 1, y & 1);
    let r = |y: u8| if y == 6 { None } else { Some(reg_plain(y)) };
    let (instr, t, t_extra) = match (x, z) {
        (1, 0) => (InC(r(y)), 12, 0),
        (1, 1) => (OutC(r(y)), 12, 0),
        (1, 2) if q == 0 => (Sbc16(rp(p, Reg16::HL)), 15, 0),
        (1, 2) => (Adc16(rp(p, Reg16::HL)), 15, 0),
        (1, 3) => {
            let nn = c.word();
            if q == 0 {
                (Ld16Store(nn, rp(p, Reg16::HL)), 20, 0)
            } else {
                (Ld16Load(rp(p, Reg16::HL), nn), 20, 0)
            }
        }
        (1, 4) => (Neg, 8, 0),
        (1, 5) => (if y == 1 { Reti } else { Retn }, 14, 0),
        (1, 6) => (Im([0, 0, 1, 2, 0, 0, 1, 2][y as usize]), 8, 0),
        (1, 7) => match y {
            0 => (LdIA, 9, 0),
            1 => (LdRA, 9, 0),
            2 => (LdAI, 9, 0),
            3 => (LdAR, 9, 0),
            4 => (Rrd, 18, 0),
            5 => (Rld, 18, 0),
            _ => (Nop, 8, 0),
        },
        (2, 0..=3) if y >= 4 => {
            use BlockOp::*;
            let ops = [
                [Ldi, Cpi, Ini, Outi],
                [Ldd, Cpd, Ind, Outd],
                [Ldir, Cpir, Inir, Otir],
                [Lddr, Cpdr, Indr, Otdr],
            ];
            let op = ops[(y - 4) as usize][z as usize];
            (Block(op), 16, if op.repeats() { 5 } else { 0 })
        }
        _ => (Nop, 8, 0),
    };
    Decoded {
        instr,
        len: c.len,
        t,
        t_extra,
        m1: 2,
    }
}

// ---------------------------------------------------------------------------
// Disassembly

impl fmt::Display for Reg8 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Reg8::B => "b",
            Reg8::C => "c",
            Reg8::D => "d",
            Reg8::E => "e",
            Reg8::H => "h",
            Reg8::L => "l",
            Reg8::A => "a",
            Reg8::IXH => "ixh",
            Reg8::IXL => "ixl",
            Reg8::IYH => "iyh",
            Reg8::IYL => "iyl",
        };
        f.write_str(s)
    }
}

impl fmt::Display for Reg16 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Reg16::BC => "bc",
            Reg16::DE => "de",
            Reg16::HL => "hl",
            Reg16::SP => "sp",
            Reg16::AF => "af",
            Reg16::IX => "ix",
            Reg16::IY => "iy",
        };
        f.write_str(s)
    }
}

impl fmt::Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Addr::BC => f.write_str("(bc)"),
            Addr::DE => f.write_str("(de)"),
            Addr::HL => f.write_str("(hl)"),
            Addr::Idx(i, d) => {
                let r = i.reg16();
                if d < 0 {
                    write!(f, "({r}-${:02x})", -(d as i16))
                } else {
                    write!(f, "({r}+${d:02x})")
                }
            }
            Addr::Abs(nn) => write!(f, "(${nn:04x})"),
        }
    }
}

impl fmt::Display for Op8 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Op8::Reg(r) => write!(f, "{r}"),
            Op8::Mem(a) => write!(f, "{a}"),
            Op8::Imm(n) => write!(f, "${n:02x}"),
        }
    }
}

impl fmt::Display for Cond {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Cond::NZ => "nz",
            Cond::Z => "z",
            Cond::NC => "nc",
            Cond::C => "c",
            Cond::PO => "po",
            Cond::PE => "pe",
            Cond::P => "p",
            Cond::M => "m",
        };
        f.write_str(s)
    }
}

impl fmt::Display for Instr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Instr::*;
        let cc = |c: Option<Cond>| c.map_or(String::new(), |c| format!("{c},"));
        let copy = |r: Option<Reg8>| r.map_or(String::new(), |r| format!(",{r}"));
        match *self {
            Nop => write!(f, "nop"),
            Halt => write!(f, "halt"),
            Di => write!(f, "di"),
            Ei => write!(f, "ei"),
            Ld8(d, s) => write!(f, "ld {d},{s}"),
            Ld16(r, nn) => write!(f, "ld {r},${nn:04x}"),
            Ld16Load(r, nn) => write!(f, "ld {r},(${nn:04x})"),
            Ld16Store(nn, r) => write!(f, "ld (${nn:04x}),{r}"),
            LdSp(r) => write!(f, "ld sp,{r}"),
            Push(r) => write!(f, "push {r}"),
            Pop(r) => write!(f, "pop {r}"),
            ExDeHl => write!(f, "ex de,hl"),
            ExAf => write!(f, "ex af,af'"),
            Exx => write!(f, "exx"),
            ExSp(r) => write!(f, "ex (sp),{r}"),
            Alu(op, s) => {
                let m = match op {
                    AluOp::Add => "add a,",
                    AluOp::Adc => "adc a,",
                    AluOp::Sub => "sub ",
                    AluOp::Sbc => "sbc a,",
                    AluOp::And => "and ",
                    AluOp::Xor => "xor ",
                    AluOp::Or => "or ",
                    AluOp::Cp => "cp ",
                };
                write!(f, "{m}{s}")
            }
            Inc8(o) => write!(f, "inc {o}"),
            Dec8(o) => write!(f, "dec {o}"),
            Inc16(r) => write!(f, "inc {r}"),
            Dec16(r) => write!(f, "dec {r}"),
            Add16(d, s) => write!(f, "add {d},{s}"),
            Adc16(s) => write!(f, "adc hl,{s}"),
            Sbc16(s) => write!(f, "sbc hl,{s}"),
            Daa => write!(f, "daa"),
            Cpl => write!(f, "cpl"),
            Neg => write!(f, "neg"),
            Ccf => write!(f, "ccf"),
            Scf => write!(f, "scf"),
            Rlca => write!(f, "rlca"),
            Rrca => write!(f, "rrca"),
            Rla => write!(f, "rla"),
            Rra => write!(f, "rra"),
            Rld => write!(f, "rld"),
            Rrd => write!(f, "rrd"),
            Rot(op, o, c) => {
                let m = format!("{op:?}").to_lowercase();
                write!(f, "{m} {o}{}", copy(c))
            }
            Bit(n, o) => write!(f, "bit {n},{o}"),
            Res(n, o, c) => write!(f, "res {n},{o}{}", copy(c)),
            Set(n, o, c) => write!(f, "set {n},{o}{}", copy(c)),
            Jp(c, a) => write!(f, "jp {}${a:04x}", cc(c)),
            JpInd(r) => write!(f, "jp ({r})"),
            Jr(c, a) => write!(f, "jr {}${a:04x}", cc(c)),
            Djnz(a) => write!(f, "djnz ${a:04x}"),
            Call(c, a) => write!(f, "call {}${a:04x}", cc(c)),
            Ret(Some(c)) => write!(f, "ret {c}"),
            Ret(None) => write!(f, "ret"),
            Reti => write!(f, "reti"),
            Retn => write!(f, "retn"),
            Rst(n) => write!(f, "rst ${n:02x}"),
            InA(n) => write!(f, "in a,(${n:02x})"),
            InC(Some(r)) => write!(f, "in {r},(c)"),
            InC(None) => write!(f, "in (c)"),
            OutA(n) => write!(f, "out (${n:02x}),a"),
            OutC(Some(r)) => write!(f, "out (c),{r}"),
            OutC(None) => write!(f, "out (c),0"),
            Block(op) => write!(f, "{}", format!("{op:?}").to_lowercase()),
            Im(m) => write!(f, "im {m}"),
            LdIA => write!(f, "ld i,a"),
            LdRA => write!(f, "ld r,a"),
            LdAI => write!(f, "ld a,i"),
            LdAR => write!(f, "ld a,r"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(bytes: &[u8]) -> Decoded {
        let mut mem = [0u8; 8];
        mem[..bytes.len()].copy_from_slice(bytes);
        decode(&|a| mem[a as usize & 7], 0)
    }

    fn dis(bytes: &[u8]) -> (String, u8, u8) {
        let d = dec(bytes);
        (d.instr.to_string(), d.len, d.t)
    }

    #[test]
    fn main_table() {
        assert_eq!(dis(&[0x00]), ("nop".into(), 1, 4));
        assert_eq!(dis(&[0x7e]), ("ld a,(hl)".into(), 1, 7));
        assert_eq!(dis(&[0x36, 0x12]), ("ld (hl),$12".into(), 2, 10));
        assert_eq!(dis(&[0x21, 0x34, 0x12]), ("ld hl,$1234".into(), 3, 10));
        assert_eq!(dis(&[0x2a, 0x34, 0x12]), ("ld hl,($1234)".into(), 3, 16));
        assert_eq!(dis(&[0x18, 0xfe]), ("jr $0000".into(), 2, 12));
        assert_eq!(dis(&[0x10, 0x02]), ("djnz $0004".into(), 2, 8));
        assert_eq!(dis(&[0xcd, 0xa9, 0x30]), ("call $30a9".into(), 3, 17));
        assert_eq!(dis(&[0xc9]), ("ret".into(), 1, 10));
        assert_eq!(dis(&[0xe6, 0x0f]), ("and $0f".into(), 2, 7));
        assert_eq!(dis(&[0xff]), ("rst $38".into(), 1, 11));
    }

    #[test]
    fn index_prefixes() {
        assert_eq!(dis(&[0xdd, 0x7e, 0x05]), ("ld a,(ix+$05)".into(), 3, 19));
        assert_eq!(dis(&[0xfd, 0x66, 0xfe]), ("ld h,(iy-$02)".into(), 3, 19));
        assert_eq!(
            dis(&[0xdd, 0x36, 0x01, 0x99]),
            ("ld (ix+$01),$99".into(), 4, 19)
        );
        assert_eq!(dis(&[0xdd, 0x34, 0x01]), ("inc (ix+$01)".into(), 3, 23));
        assert_eq!(dis(&[0xdd, 0x7c]), ("ld a,ixh".into(), 2, 8));
        assert_eq!(dis(&[0xdd, 0x21, 0, 0x60]), ("ld ix,$6000".into(), 4, 14));
        assert_eq!(dis(&[0xdd, 0xe9]), ("jp (ix)".into(), 2, 8));
        assert_eq!(dis(&[0xdd, 0xeb]), ("ex de,hl".into(), 2, 8));
        assert_eq!(dis(&[0xdd, 0xdd]), ("nop".into(), 1, 4));
    }

    #[test]
    fn cb_ed() {
        assert_eq!(dis(&[0xcb, 0x46]), ("bit 0,(hl)".into(), 2, 12));
        assert_eq!(dis(&[0xcb, 0x11]), ("rl c".into(), 2, 8));
        assert_eq!(
            dis(&[0xdd, 0xcb, 0x03, 0xc6]),
            ("set 0,(ix+$03)".into(), 4, 23)
        );
        assert_eq!(
            dis(&[0xfd, 0xcb, 0x03, 0x00]),
            ("rlc (iy+$03),b".into(), 4, 23)
        );
        assert_eq!(dis(&[0xed, 0xb0]), ("ldir".into(), 2, 16));
        assert_eq!(dis(&[0xed, 0x52]), ("sbc hl,de".into(), 2, 15));
        assert_eq!(dis(&[0xed, 0x5f]), ("ld a,r".into(), 2, 9));
        assert_eq!(
            dis(&[0xed, 0x73, 0x00, 0x80]),
            ("ld ($8000),sp".into(), 4, 20)
        );
        assert_eq!(dis(&[0xed, 0x00]), ("nop".into(), 2, 8));
    }
}
