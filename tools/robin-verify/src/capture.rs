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

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::hash::{Hash, Hasher};

use robin::Game;
use robin::assets::{Assets, BANK};
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
    /// The rewrite: the state, the tape's data, and the registers at entry;
    /// returns the registers with its outputs set.
    pub rewrite: fn(&mut Game, &Assets, Regs) -> Regs,
}

/// What one routine's suite saw.
#[derive(Default)]
pub struct Tally {
    pub calls: u64,
    pub compared: u64,
    pub repeats: u64,
    pub scrambled: u64,
    seen: HashSet<u64>,
    scrambled_by_caller: BTreeMap<u16, u32>,
    pub executed: BTreeSet<u16>,
    pub failures: Vec<String>,
}

/// How many distinct calls from each caller get the scrambling check.
const SCRAMBLE_PER_CALLER: u32 = 20;
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

    fn in_code(&self, z: &Zx, pc: u16) -> bool {
        (self.code.0..=self.code.1).contains(&pc)
            && (pc < 0xC000
                || self
                    .bank
                    .is_none_or(|b| z.memory.slot(3) == Memory::bank(b)))
    }

    /// Takes the call `z` is about to make.
    pub fn capture(&self, z: &Zx, assets: &Assets, t: &mut Tally) {
        t.calls += 1;
        let entry = z.clone();
        let run = match self.run_original(&entry) {
            Ok(run) => run,
            Err(e) => {
                fail(t, e);
                return;
            }
        };
        if !t.seen.insert(run.key) {
            t.repeats += 1;
            return;
        }
        t.compared += 1;
        t.executed.extend(run.executed.iter().copied());
        let at = format!(
            "call from {:04x} (case {})",
            run.ret.wrapping_sub(3),
            t.compared
        );
        let differ = self.compare(&entry, &run, assets);
        if !differ.is_empty() {
            fail(t, format!("{}: {at}: {}", self.name, differ.join("; ")));
            return;
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
                Ok(run2) => {
                    let differ = self.compare(&poisoned, &run2, assets);
                    if !differ.is_empty() {
                        fail(
                            t,
                            format!(
                                "{}: {at}, with the {} byte(s) it only writes set to other values first: {}",
                                self.name,
                                run.blind.len(),
                                differ.join("; ")
                            ),
                        );
                        return;
                    }
                }
                Err(e) => {
                    fail(t, e);
                    return;
                }
            }
        }

        let n = t.scrambled_by_caller.entry(run.ret).or_default();
        if *n < SCRAMBLE_PER_CALLER {
            *n += 1;
            t.scrambled += 1;
            if let Some(e) = self.scramble(&run.after, run.entry_sp) {
                fail(t, format!("{}: {at}: {e}", self.name));
            }
        }
    }

    /// Runs the original routine alone from `entry` to its return, with no
    /// interrupts, recording what the call reads and writes.
    fn run_original(&self, entry: &Zx) -> Result<Run, String> {
        let mut c = entry.clone();
        let entry_sp = c.sp;
        let ret = c.read16(entry_sp);
        let mut min_sp = entry_sp;
        let mut key = DefaultHasher::new();
        Regs::of(entry).hash(&mut key);
        entry_sp.hash(&mut key);
        let mut ports = Vec::new();
        let mut executed = Vec::new();
        let mut read: HashSet<(usize, usize)> = HashSet::new();
        let mut blind: BTreeSet<(usize, usize)> = BTreeSet::new();
        let mut steps = 0u64;
        while !(c.pc == ret && c.sp == entry_sp.wrapping_add(2)) {
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
                    _ => {}
                }
            }
            interp::step(&mut c);
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
            banks: banks(&c),
            after: c,
        })
    }

    /// What differs between the original's `run` from `entry` and the
    /// rewrite from the same state.
    fn compare(&self, entry: &Zx, run: &Run, assets: &Assets) -> Vec<String> {
        let before = Regs::of(entry);
        let mut g = Game::from_memory(&banks(entry));
        let out = (self.rewrite)(&mut g, assets, before);
        let after = Regs::of(&run.after);
        let mut differ = Vec::new();

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
    /// and flag that is neither an output nor preserved scrambled, until the
    /// caller returns or [`FOLLOW`] steps. Any difference in where they go or
    /// what they write means a caller reads one of those registers.
    fn scramble(&self, returned: &Zx, entry_sp: u16) -> Option<String> {
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
        let mut low = same.sp.min(odd.sp);
        for step in 0..FOLLOW {
            if same.pc != odd.pc {
                return Some(format!(
                    "with {scrambled:?} scrambled, the caller went to {:04x} instead of {:04x} after {step} steps: it reads one of them",
                    odd.pc, same.pc
                ));
            }
            if same.sp > entry_sp.wrapping_add(2) {
                break;
            }
            interp::step(&mut same);
            interp::step(&mut odd);
            low = low.min(same.sp).min(odd.sp);
        }
        // What either pushed below where the stack now is, nobody reads.
        let mut dead: HashSet<(usize, usize)> = HashSet::new();
        let mut a = low;
        while a != same.sp {
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
        live.first().map(|&(n, i)| {
            format!(
                "with {scrambled:?} scrambled, the caller wrote memory differently: {} byte(s), first {n}:{i:04x} in {}",
                live.len(),
                Game::part_at(n, i)
            )
        })
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

/// One run of the original routine.
struct Run {
    key: u64,
    ret: u16,
    entry_sp: u16,
    min_sp: u16,
    ports: Vec<(u16, u16)>,
    executed: Vec<u16>,
    /// The places it wrote before reading, other than the dead stack.
    blind: BTreeSet<(usize, usize)>,
    banks: Box<[[u8; BANK]; 8]>,
    after: Zx,
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

fn fail(t: &mut Tally, e: String) {
    if t.failures.len() < KEEP {
        t.failures.push(e);
    } else if t.failures.len() == KEEP {
        t.failures.push("... and more".into());
    }
}
