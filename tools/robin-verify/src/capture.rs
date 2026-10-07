//! Catching the original's real calls to a routine and comparing each with
//! the rewrite (#24, Decisions 2 and 3).
//!
//! When the original reaches a routine's entry during play, the machine is
//! copied, and the copy runs the original routine alone, with no
//! interrupts, to its return. The call's key is its registers and every byte
//! it read; a key seen before is a repeat, counted and skipped. A new call
//! runs the rewrite on [`Game::from_memory`] of the same state, and the two
//! are compared at once: all of memory, the routine's outputs, the registers
//! it preserves, and any port it writes. For the first calls from each
//! caller, the registers and flags that are neither outputs nor preserved
//! are scrambled after the return, and the original must go on exactly as
//! before, which shows no caller reads them.
//!
//! The calls are checked on every core, and tallied as if one by one, in
//! the order they were made (`checker`, #45).

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::hash::{Hash, Hasher};

use robin::Game;
use robin::assets::{Assets, BANK};
use robin::controls::Controls;
use robin::inputs::{Inputs, Random};
use zx_recomp::script::Script;
use zx_runtime::memory::Memory;
use zx_runtime::{Zx, bus, interp};

/// A Z80 register a routine's callers can see.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Reg {
    A,
    F,
    B,
    C,
    D,
    E,
    H,
    L,
    A_,
    F_,
    B_,
    C_,
    D_,
    E_,
    H_,
    L_,
    Ixh,
    Ixl,
    Iyh,
    Iyl,
}

pub const ALL: [Reg; 20] = [
    Reg::A,
    Reg::F,
    Reg::B,
    Reg::C,
    Reg::D,
    Reg::E,
    Reg::H,
    Reg::L,
    Reg::A_,
    Reg::F_,
    Reg::B_,
    Reg::C_,
    Reg::D_,
    Reg::E_,
    Reg::H_,
    Reg::L_,
    Reg::Ixh,
    Reg::Ixl,
    Reg::Iyh,
    Reg::Iyl,
];

/// The registers, as a rewritten routine takes and returns them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Regs([u8; 20]);

impl Regs {
    pub fn of(z: &Zx) -> Regs {
        let [ixh, ixl] = z.ix.to_be_bytes();
        let [iyh, iyl] = z.iy.to_be_bytes();
        Regs([
            z.a, z.f, z.b, z.c, z.d, z.e, z.h, z.l, z.a_, z.f_, z.b_, z.c_, z.d_, z.e_, z.h_, z.l_,
            ixh, ixl, iyh, iyl,
        ])
    }

    pub fn get(&self, r: Reg) -> u8 {
        self.0[r as usize]
    }

    pub fn set(&mut self, r: Reg, v: u8) {
        self.0[r as usize] = v;
    }

    pub fn set_pair(&mut self, hi: Reg, lo: Reg, v: u16) {
        let [h, l] = v.to_be_bytes();
        self.set(hi, h);
        self.set(lo, l);
    }

    fn put(&self, z: &mut Zx) {
        let r = &self.0;
        (z.a, z.f, z.b, z.c, z.d, z.e, z.h, z.l) = (r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7]);
        (z.a_, z.f_, z.b_, z.c_, z.d_, z.e_, z.h_, z.l_) =
            (r[8], r[9], r[10], r[11], r[12], r[13], r[14], r[15]);
        z.ix = u16::from_be_bytes([r[16], r[17]]);
        z.iy = u16::from_be_bytes([r[18], r[19]]);
    }
}

/// One routine of the original and its rewrite.
#[derive(Clone)]
pub struct Routine {
    pub name: &'static str,
    /// Where it is: the bank paged at `0xC000` for code there.
    pub bank: Option<usize>,
    pub entry: u16,
    /// Its code, first and last byte, for the coverage report.
    pub code: (u16, u16),
    /// Registers the rewrite computes, compared with the original's.
    pub outputs: &'static [Reg],
    /// Registers the original leaves as they were, which callers rely on.
    pub preserves: &'static [Reg],
    /// Where it ends, for a stretch of code that is not a routine (the main
    /// loop's): it runs from `entry` to the first of these. Empty for a
    /// routine, which runs to its return.
    pub exits: &'static [u16],
    /// The rewrite: the state, the data parsed from the tape, the registers
    /// at entry, and the inputs: the controls as the original saw them, and
    /// what it read from R (#37); returns the registers with its outputs
    /// set.
    pub rewrite: fn(&mut Game, &Assets, Regs, &mut Inputs) -> Regs,
}

/// What one routine's suite saw.
#[derive(Default)]
pub struct Tally {
    pub calls: u64,
    pub compared: u64,
    pub repeats: u64,
    pub scrambled: u64,
    /// Runs with the data it read changed (a supplement: not states the
    /// original had), and those that did not return.
    pub varied: u64,
    pub varied_hung: u64,
    /// Scrambling checks whose stack never came back to the caller's level,
    /// so that stretch of it was left out of the comparison.
    pub stack_left_out: u64,
    /// Calls skipped because the original read the ROM during them, which
    /// the rewrite has none of (#21), and the first such: where, and what.
    pub rom_reads: u64,
    pub first_rom_read: Option<String>,
    pub(crate) seen: HashSet<u64>,
    pub(crate) scrambled_by_caller: BTreeMap<u16, u32>,
    pub executed: BTreeSet<u16>,
    pub failures: Vec<String>,
}

/// The parts of the state whose bytes a varied run changes: data, never
/// code, tables or where to write.
const VARIED: [&str; 4] = [
    "the screen",
    "the back buffer",
    "the attribute buffer",
    "the changed-cell map",
];

/// How many distinct calls from each caller get the scrambling check.
pub(crate) const SCRAMBLE_PER_CALLER: u32 = 20;
/// How far the original runs before a call is taken as hung.
const STEP_LIMIT: u64 = 10_000_000;
/// How far the scrambled copy and the original are followed after a return.
const FOLLOW: u64 = 1_000_000;
/// At most this many failures are kept per routine.
const KEEP: usize = 12;

fn banks(z: &Zx) -> Box<[[u8; BANK]; 8]> {
    let mut b = Box::new([[0u8; BANK]; 8]);
    for (n, bank) in b.iter_mut().enumerate() {
        *bank = *z.memory.page(Memory::bank(n));
    }
    b
}

/// The bank and offset `addr` reaches with `z`'s paging, for RAM.
fn ram_place(z: &Zx, addr: u16) -> Option<(usize, usize)> {
    let page = z.memory.slot(usize::from(addr >> 14));
    page.checked_sub(2)
        .map(|bank| (bank, usize::from(addr) % BANK))
}

impl Routine {
    /// Whether `z` is at this routine's entry.
    pub fn is_entered(&self, z: &Zx) -> bool {
        z.pc == self.entry
            && self
                .bank
                .is_none_or(|b| z.memory.slot(3) == Memory::bank(b))
    }

    /// Whether this routine's bank is paged, if it has one.
    fn in_bank(&self, z: &Zx) -> bool {
        self.bank
            .is_none_or(|b| z.memory.slot(3) == Memory::bank(b))
    }

    fn in_code(&self, z: &Zx, pc: u16) -> bool {
        (self.code.0..=self.code.1).contains(&pc)
            && (pc < 0xC000
                || self
                    .bank
                    .is_none_or(|b| z.memory.slot(3) == Memory::bank(b)))
    }

    /// Where a call is from, for its messages, numbered as the `case`th
    /// compared.
    pub(crate) fn at(&self, run: &Run, case: u64) -> String {
        if self.exits.is_empty() {
            format!("call from {:04x} (case {case})", run.ret.wrapping_sub(3))
        } else {
            format!("run from {:04x} (case {case})", self.entry)
        }
    }

    /// The rewrite against a new call `run` from `entry`, then the same call
    /// with the bytes it only writes poisoned, then with the data it read
    /// varied; the first that differs ends it.
    pub(crate) fn compare_all(&self, entry: &Zx, run: &Run, play: &Play, at: &str) -> Compared {
        let mut c = Compared::default();
        let differ = self.compare(entry, run, play);
        if !differ.is_empty() {
            c.failure = Some(format!("{}: {at}: {}", self.name, differ.join("; ")));
            return c;
        }

        // The same call with every byte it writes before reading set to
        // something else first. The original never sees their old values, so
        // it does just the same; a rewrite that leaves one unwritten, where
        // the old value happened to be right, now shows.
        if !run.blind.is_empty() {
            let mut poisoned = entry.clone();
            for &(n, i) in &run.blind {
                let v = run.after_bank_byte(n, i);
                poke(&mut poisoned, n, i, !v);
            }
            match self.run_original(&poisoned) {
                Ok(run2) if run2.rom_read.is_some() => c.rom_reads += 1,
                Ok(run2) => {
                    let differ = self.compare(&poisoned, &run2, play);
                    if !differ.is_empty() {
                        c.failure = Some(format!(
                            "{}: {at}, with the {} byte(s) it only writes set to other values first: {}",
                            self.name,
                            run.blind.len(),
                            differ.join("; ")
                        ));
                        return c;
                    }
                }
                Err(e) => {
                    c.failure = Some(e);
                    return c;
                }
            }
        }

        // A supplement to the real calls (#6, Decision 8): the same call with
        // every byte of the screen and the play area's buffers that it read
        // changed. Play never shows some values (no attribute with bit 7 set
        // ever reached the flush, for one), so this compares the two on
        // inputs play doesn't give. Only data is changed, never code, the
        // tables or anything that says where to write.
        let vary: Vec<(usize, usize)> = run
            .read
            .iter()
            .copied()
            .filter(|&(n, i)| VARIED.contains(&Game::part_at(n, i)))
            .collect();
        if !vary.is_empty() {
            let mut varied = entry.clone();
            for &(n, i) in &vary {
                let mut h = DefaultHasher::new();
                (run.key, n, i).hash(&mut h);
                let flip = (h.finish() as u8) | 1;
                let v = entry.memory.page(Memory::bank(n))[i];
                poke(&mut varied, n, i, v ^ flip);
            }
            match self.run_original(&varied) {
                Ok(run3) if run3.rom_read.is_some() => c.rom_reads += 1,
                Ok(run3) => {
                    c.varied += 1;
                    let differ = self.compare(&varied, &run3, play);
                    if !differ.is_empty() {
                        c.failure = Some(format!(
                            "{}: {at}, with the {} byte(s) of data it read changed: {}",
                            self.name,
                            vary.len(),
                            differ.join("; ")
                        ));
                        return c;
                    }
                }
                Err(_) => c.varied_hung += 1,
            }
        }
        c
    }

    /// The stack level a call leaves: a stretch leaves the stack as it found
    /// it, not one return address above.
    pub(crate) fn level(&self, run: &Run) -> u16 {
        if self.exits.is_empty() {
            run.entry_sp.wrapping_add(2)
        } else {
            run.entry_sp
        }
    }

    /// Who a call returns to, which the scrambling check is shared out by.
    pub(crate) fn caller(&self, run: &Run) -> u16 {
        if self.exits.is_empty() {
            run.ret
        } else {
            self.entry
        }
    }

    /// Runs the original routine alone from `entry` to its return, with no
    /// interrupts, recording what the call reads and writes.
    pub(crate) fn run_original(&self, entry: &Zx) -> Result<Run, String> {
        let mut c = entry.clone();
        let entry_sp = c.sp;
        let ret = c.read16(entry_sp);
        let mut min_sp = entry_sp;
        let mut key = DefaultHasher::new();
        Regs::of(entry).hash(&mut key);
        entry_sp.hash(&mut key);
        let mut ports = Vec::new();
        let mut executed = Vec::new();
        let mut read: BTreeSet<(usize, usize)> = BTreeSet::new();
        let mut rom_read = None;
        let mut blind: BTreeSet<(usize, usize)> = BTreeSet::new();
        let mut steps = 0u64;
        let done = |c: &Zx| {
            if self.exits.is_empty() {
                c.pc == ret && c.sp == entry_sp.wrapping_add(2)
            } else {
                self.exits.contains(&c.pc) && self.in_bank(c)
            }
        };
        let mut random = Vec::new();
        while !done(&c) {
            steps += 1;
            if steps > STEP_LIMIT {
                return Err(format!(
                    "{}: did not return within {STEP_LIMIT} steps",
                    self.name
                ));
            }
            let pc = c.pc;
            if self.in_code(&c, pc) {
                executed.push(pc);
            }
            let d = interp::decode_at(&c, pc);
            for cy in bus::cycles(&c, &d, pc).iter() {
                match cy.kind {
                    bus::Kind::Read => {
                        (cy.at, c.read(cy.at)).hash(&mut key);
                        if cy.at < 0x4000 && rom_read.is_none() {
                            rom_read = Some((pc, cy.at));
                        }
                        if let Some(p) = ram_place(&c, cy.at) {
                            read.insert(p);
                        }
                    }
                    bus::Kind::Write => {
                        if let Some(p) = ram_place(&c, cy.at)
                            && !read.contains(&p)
                        {
                            blind.insert(p);
                        }
                    }
                    bus::Kind::PortWrite => ports.push((pc, cy.at)),
                    // What a port answers is an input like memory's bytes:
                    // two calls with different keys held are different
                    // calls.
                    bus::Kind::PortRead => (cy.at, c.port_in(cy.at)).hash(&mut key),
                    _ => {}
                }
            }
            // `LD A,R`: what R gave is an input, like a key held (#37).
            let reads_r = c.read(pc) == 0xED && c.read(pc.wrapping_add(1)) == 0x5F;
            interp::step(&mut c);
            if reads_r {
                random.push(c.a);
                c.a.hash(&mut key);
            }
            min_sp = min_sp.min(c.sp);
        }
        // The stack below the caller's is not the routine's output.
        let mut a = min_sp;
        while a != entry_sp {
            if let Some(p) = ram_place(entry, a) {
                blind.remove(&p);
            }
            a = a.wrapping_add(1);
        }
        Ok(Run {
            key: key.finish(),
            ret,
            entry_sp,
            min_sp,
            ports,
            executed,
            blind,
            read,
            rom_read,
            random,
            banks: banks(&c),
            after: c,
        })
    }

    /// What differs between the original's `run` from `entry` and the
    /// rewrite from the same state.
    fn compare(&self, entry: &Zx, run: &Run, play: &Play) -> Vec<String> {
        let before = Regs::of(entry);
        let mut g = Game::from_memory(&banks(entry));
        // A rewrite that panics where the original carries on is a
        // difference like any other: reported, not the end of the run.
        let rewrite = self.rewrite;
        let mut inputs = Inputs {
            controls: Controls {
                keys: entry.keys,
                kempston: entry.kempston,
                ear: entry.ear,
            },
            random: Random::given(run.random.clone()),
        };
        let out = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rewrite(&mut g, play.assets, before, &mut inputs)
        })) {
            Ok(out) => out,
            Err(e) => {
                let why = e
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| e.downcast_ref::<&str>().copied())
                    .unwrap_or("a panic");
                return vec![format!("the rewrite panicked: {why}")];
            }
        };
        let after = Regs::of(&run.after);
        let mut differ = Vec::new();
        if inputs.random.left() > 0 {
            differ.push(format!(
                "the original read R {} time(s) and the rewrite drew {} fewer",
                run.random.len(),
                inputs.random.left()
            ));
        }

        // All of memory, but for the stack below the caller's: the
        // original's pushes and calls leave bytes there that nobody reads.
        let mut dead: HashSet<(usize, usize)> = HashSet::new();
        let mut a = run.min_sp;
        while a != run.entry_sp {
            if let Some(p) = ram_place(entry, a) {
                dead.insert(p);
            }
            a = a.wrapping_add(1);
        }
        let (orig, new) = (&run.banks, g.to_memory());
        let mut bytes = Vec::new();
        for n in 0..8 {
            for i in 0..BANK {
                if orig[n][i] != new[n][i] && !dead.contains(&(n, i)) {
                    bytes.push((n, i));
                }
            }
        }
        if let Some(&(n, i)) = bytes.first() {
            differ.push(format!(
                "{} byte(s) of memory differ, first {}:{:04x} in {} (original {:02x}, rewrite {:02x})",
                bytes.len(),
                n,
                i,
                Game::part_at(n, i),
                orig[n][i],
                new[n][i]
            ));
        }
        for &r in self.outputs {
            if out.get(r) != after.get(r) {
                differ.push(format!(
                    "output {r:?}: original {:02x}, rewrite {:02x}",
                    after.get(r),
                    out.get(r)
                ));
            }
        }
        for &r in self.preserves {
            if after.get(r) != before.get(r) {
                differ.push(format!(
                    "{r:?} not preserved by the original: {:02x} became {:02x} (the notes are wrong)",
                    before.get(r),
                    after.get(r)
                ));
            }
        }
        for (pc, port) in &run.ports {
            differ.push(format!(
                "writes port {port:04x} at {pc:04x}, which the rewrite does not model"
            ));
        }
        differ
    }

    /// Follows the original from its return twice, once with every register
    /// and flag that is neither an output nor preserved scrambled. Both play
    /// on as the original would: interrupts taken, and the input script
    /// carried on from `play`. Where they go is compared at every
    /// instruction until the caller returns, or for [`FOLLOW`] instructions,
    /// then until the stack is back at the caller's level; then all of
    /// memory but the dead stack. Any difference means a caller reads one of
    /// those registers.
    pub(crate) fn scramble(&self, returned: &Zx, level: u16, play: &Play) -> Scrambled {
        let mut same = returned.clone();
        let mut odd = returned.clone();
        let mut regs = Regs::of(&odd);
        let scrambled: Vec<Reg> = ALL
            .iter()
            .copied()
            .filter(|r| !self.outputs.contains(r) && !self.preserves.contains(r))
            .collect();
        for &r in &scrambled {
            regs.set(r, !regs.get(r) ^ 0x5A);
        }
        regs.put(&mut odd);
        let (mut keys_same, mut keys_odd) = (play.script.clone(), play.script.clone());
        let start = same.frame;
        let mut low = same.sp;
        let mut step = |same: &mut Zx, odd: &mut Zx| {
            // A frame is over: the next step begins another, so its keys go
            // down first, as the play loop does it.
            if same.t >= same.timing.frame {
                let f = play.frame + (same.frame + 1 - start) as u32;
                keys_same.press(f, same);
                keys_odd.press(f, odd);
            }
            same.step_in_frame();
            odd.step_in_frame();
        };
        let diverged = |same: &Zx, odd: &Zx| {
            Scrambled::Reads(format!(
                "with {scrambled:?} scrambled, the caller went to {:04x} instead of {:04x}: it reads one of them",
                odd.pc, same.pc
            ))
        };
        // Until the caller returns...
        for _ in 0..FOLLOW {
            if same.pc != odd.pc {
                return diverged(&same, &odd);
            }
            if same.sp > level {
                break;
            }
            step(&mut same, &mut odd);
            low = low.min(same.sp).min(odd.sp);
        }
        // ...then until the stack is back at its level, so that what a later
        // routine saved there, a scrambled register among it, is popped.
        let mut back = same.sp >= level;
        for _ in 0..FOLLOW {
            if same.pc != odd.pc {
                return diverged(&same, &odd);
            }
            if same.sp >= level {
                back = true;
                break;
            }
            step(&mut same, &mut odd);
            low = low.min(same.sp).min(odd.sp);
        }
        // What either pushed below where the stack now is, nobody reads; and
        // if the stack never came back, what is between is left out too, and
        // counted.
        let mut dead: HashSet<(usize, usize)> = HashSet::new();
        let mut a = low;
        let top = if back { same.sp } else { level };
        while a != top {
            if let Some(p) = ram_place(&same, a) {
                dead.insert(p);
            }
            a = a.wrapping_add(1);
        }
        let (a, b) = (banks(&same), banks(&odd));
        let mut live = Vec::new();
        for n in 0..8 {
            for i in 0..BANK {
                if a[n][i] != b[n][i] && !dead.contains(&(n, i)) {
                    live.push((n, i));
                }
            }
        }
        match live.first() {
            Some(&(n, i)) => Scrambled::Reads(format!(
                "with {scrambled:?} scrambled, the caller wrote memory differently: {} byte(s), first {n}:{i:04x} in {}",
                live.len(),
                Game::part_at(n, i)
            )),
            None if back => Scrambled::Unread,
            None => Scrambled::UnreadStackLeftOut,
        }
    }

    /// The instructions of this routine no case executed.
    pub fn unreached(&self, z: &Zx, t: &Tally) -> Vec<u16> {
        let mut out = Vec::new();
        let mut pc = self.code.0;
        let mut m = z.clone();
        if let Some(b) = self.bank {
            m.memory.page_128k(b as u8 | 0x10);
        }
        while pc <= self.code.1 {
            if !t.executed.contains(&pc) {
                out.push(pc);
            }
            let len = u16::from(interp::decode_at(&m, pc).len.max(1));
            pc = pc.wrapping_add(len);
        }
        out
    }
}

/// Where play is when a call is caught: the input script as it stands and
/// the frame it is in, so a copy of the machine can play on as the original.
pub struct Play<'a> {
    pub script: &'a Script,
    pub frame: u32,
    /// The data parsed from the tape, which the rewrites read.
    pub assets: &'a Assets,
}

/// What the scrambling check found.
pub(crate) enum Scrambled {
    /// No difference: the caller does not read those registers.
    Unread,
    /// No difference, but the stack never came back to the caller's level,
    /// so that stretch of it was left out.
    UnreadStackLeftOut,
    /// A difference: the caller reads one of them.
    Reads(String),
}

/// One run of the original routine.
pub(crate) struct Run {
    pub(crate) key: u64,
    pub(crate) ret: u16,
    entry_sp: u16,
    min_sp: u16,
    ports: Vec<(u16, u16)>,
    pub(crate) executed: Vec<u16>,
    /// The places it wrote before reading, other than the dead stack.
    blind: BTreeSet<(usize, usize)>,
    /// Every place it read.
    read: BTreeSet<(usize, usize)>,
    /// The first read of the ROM, if it made one: where, and what.
    pub(crate) rom_read: Option<(u16, u16)>,
    /// What R gave each `LD A,R`, in order.
    random: Vec<u8>,
    banks: Box<[[u8; BANK]; 8]>,
    pub(crate) after: Zx,
}

impl Run {
    fn after_bank_byte(&self, n: usize, i: usize) -> u8 {
        self.banks[n][i]
    }
}

/// Sets byte `i` of bank `n` in `z`, paging it in and back.
fn poke(z: &mut Zx, n: usize, i: usize, v: u8) {
    let i = i as u16;
    match n {
        5 => z.memory.poke(0x4000 + i, v),
        2 => z.memory.poke(0x8000 + i, v),
        _ => {
            let was = z.port_7ffd;
            z.memory.page_128k((was & !7) | n as u8);
            z.memory.poke(0xC000 + i, v);
            z.memory.page_128k(was);
        }
    }
}

/// What comparing a new call found, before the scrambling check.
#[derive(Default)]
pub(crate) struct Compared {
    pub rom_reads: u64,
    pub varied: u64,
    pub varied_hung: u64,
    pub failure: Option<String>,
}

pub(crate) fn fail(t: &mut Tally, e: String) {
    if t.failures.len() < KEEP {
        t.failures.push(e);
    } else if t.failures.len() == KEEP {
        t.failures.push("... and more".into());
    }
}

#[cfg(test)]
mod tests {
    //! The checks checked, on a made-up program, with no tape (#6,
    //! Decision 3): each must fail when what it watches for is wrong.
    use super::*;
    use crate::checker::{Checker, Job};
    use std::sync::Arc;

    /// A 128K with no ROM, about to call a routine at `0x8100` from
    /// `0x8000`. The routine is `routine`; after it returns, the caller runs
    /// `after`, then halts.
    fn machine(routine: &[u8], after: &[u8]) -> Zx {
        let mut z = zx_runtime::loader::power_on_128k(&[0u8; 0x8000]);
        z.memory.page_128k(0x10);
        let caller = [0xCD, 0x00, 0x81].iter().chain(after).chain(&[0x76]);
        for (at, &b) in (0x8000u16..).zip(caller) {
            z.memory.poke(at, b);
        }
        for (i, &b) in routine.iter().enumerate() {
            z.memory.poke(0x8100 + i as u16, b);
        }
        (z.pc, z.sp, z.iff1) = (0x8000, 0x9F00, false);
        interp::step(&mut z);
        assert_eq!(z.pc, 0x8100, "at the routine");
        z
    }

    fn routine(
        outputs: &'static [Reg],
        rewrite: fn(&mut Game, &Assets, Regs, &mut Inputs) -> Regs,
    ) -> Routine {
        Routine {
            name: "made up",
            bank: None,
            entry: 0x8100,
            code: (0x8100, 0x810F),
            outputs,
            preserves: &[],
            exits: &[],
            rewrite,
        }
    }

    /// Assets for a made-up program: nothing reads them.
    fn no_assets() -> Assets {
        Assets::from_banks(Box::new([[0u8; BANK]; 8]))
    }

    /// No keys pressed, ever.
    fn quiet() -> Script {
        let cfg =
            zx_recomp::Config::parse("[game]\nname = \"t\"\ntape = \"t\"\n").expect("a config");
        Script::new(&cfg.trace).expect("a script")
    }

    /// `r`'s tally after checking `calls` in order, as the verifier checks
    /// them.
    fn check_calls(r: &Routine, calls: &[Zx]) -> Tally {
        let mut checker = Checker::new(Arc::from(vec![r.clone()]), &Arc::new(no_assets()));
        for z in calls {
            checker.take(Job {
                routine: 0,
                entry: z.clone(),
                script: quiet(),
                frame: 0,
            });
        }
        checker.finish().remove(0)
    }

    fn failures(r: &Routine, z: &Zx) -> Vec<String> {
        let t = check_calls(r, std::slice::from_ref(z));
        assert_eq!(t.compared, 1);
        t.failures
    }

    /// `LD A,0x42 : RET`, and the caller stores A.
    const RETURNS_A: &[u8] = &[0x3E, 0x42, 0xC9];
    const STORES_A: &[u8] = &[0x32, 0x00, 0x90];

    #[test]
    fn a_caller_reading_an_undeclared_register_is_caught() {
        let z = machine(RETURNS_A, STORES_A);
        let f = failures(&routine(&[], |_, _, r, _| r), &z);
        assert!(f.iter().any(|e| e.contains("scrambled")), "{f:?}");
        let f = failures(
            &routine(&[Reg::A], |_, _, mut r, _| {
                r.set(Reg::A, 0x42);
                r
            }),
            &z,
        );
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn a_wrong_output_is_caught() {
        let z = machine(RETURNS_A, STORES_A);
        let f = failures(
            &routine(&[Reg::A], |_, _, mut r, _| {
                r.set(Reg::A, 0x43);
                r
            }),
            &z,
        );
        assert!(f.iter().any(|e| e.contains("output A")), "{f:?}");
    }

    /// `XOR A : LD (0x9100),A : RET`: writes a byte that is already 0.
    const CLEARS_9100: &[u8] = &[0xAF, 0x32, 0x00, 0x91, 0xC9];

    #[test]
    fn a_write_left_out_is_caught_even_where_the_value_was_already_right() {
        let z = machine(CLEARS_9100, &[]);
        let f = failures(&routine(&[], |_, _, r, _| r), &z);
        assert!(
            f.iter()
                .any(|e| e.contains("only writes set to other values")),
            "{f:?}"
        );
        let f = failures(
            &routine(&[], |g, _, r, _| {
                g.write(0x9100, 0);
                r
            }),
            &z,
        );
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn a_rewrite_that_panics_is_a_failure_not_the_end() {
        let z = machine(RETURNS_A, STORES_A);
        let f = failures(&routine(&[], |_, _, _, _| panic!("planted")), &z);
        assert!(f.iter().any(|e| e.contains("panicked: planted")), "{f:?}");
    }

    #[test]
    fn a_call_that_reads_the_rom_is_skipped_and_counted_not_compared() {
        // `LD A,(0x1000) : RET`: the rewrite has no ROM to match it with.
        let z = machine(&[0x3A, 0x00, 0x10, 0xC9], &[]);
        let r = routine(&[], |_, _, _, _| panic!("never run: the call is skipped"));
        let t = check_calls(&r, &[z]);
        assert_eq!((t.calls, t.compared, t.rom_reads), (1, 0, 1));
        assert!(t.failures.is_empty(), "{:?}", t.failures);
        assert!(
            t.first_rom_read
                .as_deref()
                .is_some_and(|r| r.contains("read 1000"))
        );
    }

    #[test]
    fn a_port_write_is_caught() {
        // `OUT (0xFE),A : RET`.
        let z = machine(&[0xD3, 0xFE, 0xC9], &[]);
        let f = failures(&routine(&[], |_, _, r, _| r), &z);
        assert!(f.iter().any(|e| e.contains("writes port")), "{f:?}");
    }

    #[test]
    fn many_calls_checked_at_once_tally_as_one_by_one_in_order() {
        // `RET`, called with B from 0 to 59, each twice; the rewrite gets A
        // wrong where B is a multiple of 7. The earlier the call, the longer
        // its rewrite takes, so the workers finish them out of order.
        let z = machine(&[0xC9], STORES_A);
        let r = routine(&[Reg::A], |_, _, mut r, _| {
            let b = r.get(Reg::B);
            std::thread::sleep(std::time::Duration::from_millis(u64::from(60 - b)));
            if b % 7 == 0 {
                r.set(Reg::A, !r.get(Reg::A));
            }
            r
        });
        let calls: Vec<Zx> = (0..60u8)
            .flat_map(|b| [b, b / 2])
            .map(|b| {
                let mut z = z.clone();
                z.b = b;
                z
            })
            .collect();
        let t = check_calls(&r, &calls);
        assert_eq!((t.calls, t.compared, t.repeats), (120, 60, 60));
        // B is case B + 1, in the order the calls were made.
        let cases: Vec<String> = (0..60)
            .step_by(7)
            .map(|b| format!("(case {})", b + 1))
            .collect();
        assert_eq!(t.failures.len(), cases.len());
        for (f, case) in t.failures.iter().zip(&cases) {
            assert!(f.contains(case) && f.contains("output A"), "{f}");
        }
        // The first calls from the caller that compared clean.
        assert_eq!(t.scrambled, u64::from(SCRAMBLE_PER_CALLER));
    }

    #[test]
    fn a_repeat_is_skipped_and_counted() {
        let z = machine(RETURNS_A, STORES_A);
        let r = routine(&[Reg::A], |_, _, mut r, _| {
            r.set(Reg::A, 0x42);
            r
        });
        let t = check_calls(&r, &[z.clone(), z]);
        assert_eq!((t.calls, t.compared, t.repeats), (2, 1, 1));
    }

    /// `IN A,(0xFE) : RET` reading the top half-row, and the caller stores A.
    const READS_KEYS: &[u8] = &[0x3E, 0x7F, 0xDB, 0xFE, 0xC9];

    #[test]
    fn calls_with_different_keys_held_are_different_calls() {
        let r = routine(&[Reg::A], |_, _, mut r, i| {
            r.set(Reg::A, i.controls.read(0x7FFE));
            r
        });
        let z = machine(READS_KEYS, STORES_A);
        let mut space = z.clone();
        space.keys[7] = 0x1E;
        let t = check_calls(&r, &[z, space.clone(), space]);
        assert_eq!((t.calls, t.compared, t.repeats), (3, 2, 1));
        assert!(
            t.failures.is_empty(),
            "the controls reach the rewrite: {:?}",
            t.failures
        );
    }

    #[test]
    fn a_stretch_runs_from_its_entry_to_an_exit() {
        // `LD A,0x42 : JP 0x8110`, a stretch that never returns; its exit
        // is 0x8110, where `LD (0x9000),A` follows.
        let mut z = machine(&[0x3E, 0x42, 0xC3, 0x10, 0x81], &[]);
        for (i, &b) in [0x32, 0x00, 0x90, 0x76].iter().enumerate() {
            z.memory.poke(0x8110 + i as u16, b);
        }
        let mut r = routine(&[Reg::A], |_, _, mut r, _| {
            r.set(Reg::A, 0x42);
            r
        });
        r.exits = &[0x8110];
        let f = failures(&r, &z);
        assert!(f.is_empty(), "{f:?}");
        let mut wrong = routine(&[Reg::A], |_, _, mut r, _| {
            r.set(Reg::A, 0x41);
            r
        });
        wrong.exits = &[0x8110];
        assert!(failures(&wrong, &z).iter().any(|e| e.contains("output A")));
    }

    /// `LD A,R : RET`, and the caller stores A.
    const READS_R: &[u8] = &[0xED, 0x5F, 0xC9];

    #[test]
    fn what_r_gave_is_an_input_and_must_be_drawn_exactly() {
        let z = machine(READS_R, STORES_A);
        let draws = routine(&[Reg::A], |_, _, mut r, i| {
            r.set(Reg::A, i.random.r());
            r
        });
        assert!(failures(&draws, &z).is_empty());
        let ignores = routine(&[Reg::A], |_, _, r, _| r);
        assert!(
            failures(&ignores, &z)
                .iter()
                .any(|e| e.contains("drew 1 fewer"))
        );
        let twice = routine(&[Reg::A], |_, _, mut r, i| {
            i.random.r();
            r.set(Reg::A, i.random.r());
            r
        });
        assert!(failures(&twice, &z).iter().any(|e| e.contains("panicked")));
    }
}
