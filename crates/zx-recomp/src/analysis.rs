//! Code discovery and block formation.
//!
//! Entry points come from the trace, the config, the runtime's miss log and
//! the tape's entry point. From those, recursive descent follows every statically
//! known branch, call and return address. Each entry point then becomes a
//! block: a straight run of instructions ending at an unconditional control
//! transfer or where another entry point begins.
//!
//! Code is found by [`Place`]: where it is, not just where it runs. On a 48K
//! that is just the address. On a 128K, the top 16K shows whichever RAM bank
//! is paged, so the same address in banks 0, 4 and 6 is three different
//! pieces of code. A branch into the top 16K from code there stays in the same
//! bank; from anywhere else the bytes cannot say which bank it lands in, so it
//! is followed into each bank the trace saw running there, and through a
//! configured banked-call routine into the bank its inline byte names.

use std::collections::BTreeSet;
use std::fmt;

use crate::config::Config;
use zx_core::{Decoded, Flow, decode};
use zx_runtime::Zx;
use zx_runtime::memory::PAGE;
use zx_runtime::trace::Trace;

/// Where a piece of code is: the address it runs at, the page of memory its
/// byte is in, and on a 128K, for the paged top 16K, which RAM bank that is.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Place {
    pub page: usize,
    pub addr: u16,
    /// The RAM bank, when the place is in a slot that is paged.
    pub bank: Option<u8>,
}

impl Place {
    /// Its index in the trace's tables and the analysis' image.
    #[must_use]
    pub fn index(self) -> usize {
        self.page * PAGE + usize::from(self.addr) % PAGE
    }
}

/// `0:cd5f` in a paged bank, `5b8a` anywhere fixed.
impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.bank {
            Some(b) => write!(f, "{b}:{:04x}", self.addr),
            None => write!(f, "{:04x}", self.addr),
        }
    }
}

/// Which page each slot of the address space shows, or `None` for a slot that
/// is paged (a 128K's top 16K).
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub fixed: [Option<usize>; 4],
}

/// The page of a 128K's RAM bank `b`: after its two ROMs.
const fn bank_page(b: usize) -> usize {
    2 + b
}

impl Layout {
    /// The layout of `z`: everything fixed on a 48K; the top slot paged on a
    /// 128K.
    #[must_use]
    pub fn of(z: &Zx) -> Layout {
        let s = z.memory.slots();
        match z.model {
            zx_core::Model::Spectrum48 => Layout {
                fixed: [Some(s[0]), Some(s[1]), Some(s[2]), Some(s[3])],
            },
            zx_core::Model::Spectrum128 => Layout {
                fixed: [Some(s[0]), Some(s[1]), Some(s[2]), None],
            },
        }
    }

    /// The place `addr` is in a fixed slot, or `None` if its slot is paged.
    #[must_use]
    pub fn fixed_place(&self, addr: u16) -> Option<Place> {
        self.fixed[usize::from(addr >> 14)].map(|page| Place {
            page,
            addr,
            bank: None,
        })
    }

    /// `addr` in the paged slot with RAM bank `bank` there.
    #[must_use]
    pub fn banked(&self, addr: u16, bank: u8) -> Place {
        Place {
            page: bank_page(usize::from(bank)),
            addr,
            bank: Some(bank),
        }
    }

    /// The place a trace index stands for: in a fixed slot if one shows that
    /// page, otherwise the paged slot.
    #[must_use]
    pub fn place_of(&self, index: usize) -> Place {
        let (page, off) = (index / PAGE, (index % PAGE) as u16);
        match self.fixed.iter().position(|&p| p == Some(page)) {
            Some(slot) => Place {
                page,
                addr: slot as u16 * PAGE as u16 + off,
                bank: None,
            },
            None => self.banked(0xC000 + off, (page - bank_page(0)) as u8),
        }
    }
}

pub struct BlockInstr {
    pub at: Place,
    pub d: Decoded,
    /// Executed by the interpreter at run time (self-modifying code).
    pub interpret: bool,
}

pub enum Exit {
    /// The last instruction always leaves the block.
    Terminated,
    /// Execution runs on into the block at this address.
    FallInto(u16),
}

pub struct Block {
    pub start: Place,
    pub instrs: Vec<BlockInstr>,
    pub exit: Exit,
}

pub struct Analysis {
    /// Every page of memory, as code was found in it: the program the tape
    /// loads with the ROM, and code bytes as the trace executed them.
    pub image: Vec<u8>,
    pub layout: Layout,
    pub rom_loaded: bool,
    pub blocks: Vec<Block>,
    pub stats: Stats,
    /// Instruction starts found by recursive descent, by [`Place::index`].
    pub code_starts: Vec<bool>,
    /// Targets of CALL and RST instructions.
    pub call_targets: BTreeSet<Place>,
    /// Block entry points.
    pub entries: BTreeSet<Place>,
}

impl Analysis {
    /// The byte at `addr` as seen from code at `from`: through the fixed
    /// slots, or in the paged one, the bank `from` is in (bank 0's page when
    /// `from` is in a fixed page, for the rare instruction that runs across
    /// into the paged slot).
    #[must_use]
    pub fn byte(&self, from: Place, addr: u16) -> u8 {
        let page = self.layout.fixed[usize::from(addr >> 14)].unwrap_or(match from.bank {
            Some(_) => from.page,
            None => bank_page(0),
        });
        self.image[page * PAGE + usize::from(addr) % PAGE]
    }

    /// The instruction at `at`.
    #[must_use]
    pub fn decode(&self, at: Place) -> Decoded {
        decode(&|a| self.byte(at, a), at.addr)
    }

    /// Where a jump, call or reference from `from` to `addr` can land: the
    /// fixed page `addr` is in; or, in the paged slot, the bank `from` is in;
    /// or, from a fixed page, each bank `trace` saw executing there.
    #[must_use]
    pub fn places(&self, trace: &Trace, from: Place, addr: u16) -> Vec<Place> {
        if let Some(p) = self.layout.fixed_place(addr) {
            return vec![p];
        }
        if let Some(b) = from.bank {
            return vec![self.layout.banked(addr, b)];
        }
        (0..8)
            .map(|b| self.layout.banked(addr, b))
            .filter(|p| trace.executed.get(p.index()).is_some_and(Option::is_some))
            .collect()
    }
}

#[derive(Default, Debug)]
pub struct Stats {
    pub traced_entries: usize,
    pub traced_instrs: usize,
    pub self_modifying: Vec<Place>,
    pub entries: usize,
    pub instrs: usize,
}

struct Ctx<'a> {
    cfg: &'a Config,
    analysis: &'a Analysis,
    trace: &'a Trace,
}

impl Ctx<'_> {
    /// Whether the instruction at `at` must be left to the interpreter.
    fn interpret(&self, at: Place) -> bool {
        self.trace.self_modified[at.index()]
            || self
                .cfg
                .analysis
                .interpret
                .iter()
                .any(|[lo, hi]| (*lo..=*hi).contains(&at.addr))
    }

    /// Whether code at `at` can be found at all: not the ROM, unless there is
    /// one.
    fn compilable(&self, at: Place) -> bool {
        at.addr >= 0x4000 || self.analysis.rom_loaded
    }

    fn places(&self, from: Place, addr: u16) -> Vec<Place> {
        self.analysis.places(self.trace, from, addr)
    }

    /// The place after the instruction at `at`, if it can be known.
    fn next(&self, at: Place, len: u8) -> Option<Place> {
        let addr = at.addr.wrapping_add(u16::from(len));
        match self.places(at, addr).as_slice() {
            [one] => Some(*one),
            _ => None,
        }
    }
}

pub fn analyze(cfg: &Config, start: &Zx, trace: &Trace, extra_entries: &[u16]) -> Analysis {
    let layout = Layout::of(start);
    let pages = start.memory.pages();
    let mut image: Vec<u8> = (0..pages)
        .flat_map(|p| start.memory.page(p).iter().copied())
        .collect();
    let mut stats = Stats::default();
    for (i, entry) in trace.executed.iter().enumerate() {
        if let Some((len, bytes)) = entry {
            stats.traced_instrs += 1;
            // The bytes of an instruction that crosses into the next page
            // are the next page's; this keeps the ones in its own.
            for (k, &b) in bytes.iter().enumerate().take(*len as usize) {
                if (i % PAGE) + k < PAGE {
                    image[i + k] = b;
                }
            }
        }
    }
    stats.self_modifying = (0..trace.self_modified.len())
        .filter(|&i| trace.self_modified[i])
        .map(|i| layout.place_of(i))
        .collect();

    let mut analysis = Analysis {
        image,
        layout,
        rom_loaded: start.memory.rom_loaded(),
        blocks: Vec::new(),
        stats: Stats::default(),
        code_starts: vec![false; pages * PAGE],
        call_targets: BTreeSet::new(),
        entries: BTreeSet::new(),
    };

    // --- seeds --------------------------------------------------------------
    let mut entries = BTreeSet::new();
    for i in 0..trace.entries.len() {
        if trace.entries[i] {
            entries.insert(layout.place_of(i));
        }
    }
    stats.traced_entries = entries.len();
    let here = |addr: u16| {
        layout
            .fixed_place(addr)
            .unwrap_or_else(|| layout.place_of(Trace::at(&start.memory.slots(), addr)))
    };
    entries.insert(here(start.pc));
    if analysis.rom_loaded {
        entries.insert(here(0x0038));
    }
    entries.extend(cfg.analysis.entry_points.iter().map(|&a| here(a)));
    entries.extend(extra_entries.iter().map(|&a| here(a)));

    // --- recursive descent --------------------------------------------------
    let mut visited = vec![false; pages * PAGE];
    let mut call_targets = BTreeSet::new();
    {
        let ctx = Ctx {
            cfg,
            analysis: &analysis,
            trace,
        };
        entries.retain(|&p| ctx.compilable(p));
        let mut work: Vec<Place> = entries.iter().copied().collect();
        while let Some(first) = work.pop() {
            let mut at = first;
            loop {
                if visited[at.index()] || !ctx.compilable(at) {
                    break;
                }
                visited[at.index()] = true;
                let d = analysis.decode(at);
                let mut target = |p: Place, work: &mut Vec<Place>| {
                    if ctx.compilable(p) && entries.insert(p) {
                        work.push(p);
                    }
                };
                let next = ctx.next(at, d.len);
                let follow =
                    |a: u16,
                     work: &mut Vec<Place>,
                     target: &mut dyn FnMut(Place, &mut Vec<Place>)| {
                        for p in ctx.places(at, a) {
                            target(p, work);
                        }
                    };
                match d.instr.flow() {
                    Flow::Next => {}
                    Flow::Jump(a) => {
                        follow(a, &mut work, &mut target);
                        break;
                    }
                    Flow::Branch(a) => follow(a, &mut work, &mut target),
                    Flow::Call { target: a, .. } => {
                        for p in ctx.places(at, a) {
                            call_targets.insert(p);
                        }
                        follow(a, &mut work, &mut target);
                        let after = at.addr.wrapping_add(u16::from(d.len));
                        if cfg.analysis.banked_calls.contains(&a) {
                            // The call is followed by the target's address
                            // and the paging byte that selects its bank.
                            let lo = analysis.byte(at, after);
                            let hi = analysis.byte(at, after.wrapping_add(1));
                            let paging = analysis.byte(at, after.wrapping_add(2));
                            let to = u16::from(hi) << 8 | u16::from(lo);
                            let p = layout
                                .fixed_place(to)
                                .unwrap_or_else(|| layout.banked(to, paging & 7));
                            call_targets.insert(p);
                            target(p, &mut work);
                            if let Some(resume) = ctx.next(at, d.len + 3) {
                                target(resume, &mut work);
                            }
                            break;
                        }
                        if cfg.analysis.inline_strings.contains(&a) {
                            // Resume after the string's terminator.
                            let mut end = after;
                            while analysis.byte(at, end) != 0xFF && end != 0xFFFF {
                                end += 1;
                            }
                            follow(end.wrapping_add(1), &mut work, &mut target);
                            break;
                        }
                        if cfg.analysis.noreturn.contains(&a) {
                            break;
                        }
                        // The return lands here: a block boundary.
                        if let Some(n) = next {
                            target(n, &mut work);
                        }
                    }
                    Flow::Return { conditional } => {
                        if !conditional {
                            break;
                        }
                    }
                    Flow::Indirect => break,
                    Flow::Halt => {
                        if let Some(n) = next {
                            target(n, &mut work);
                        }
                        break;
                    }
                }
                match next {
                    Some(n) => at = n,
                    None => break,
                }
            }
        }
    }

    // --- blocks -------------------------------------------------------------
    // Splitting over-long blocks adds entries, which can only shorten other
    // blocks, so one pass to find split points is enough.
    let max = cfg.analysis.max_block_instrs.max(1);
    let blocks = {
        let ctx = Ctx {
            cfg,
            analysis: &analysis,
            trace,
        };
        let splits: Vec<Place> = entries
            .iter()
            .filter_map(|&e| match form_block(&ctx, e, &entries, max) {
                (_, Some(p)) if !entries.contains(&p) && ctx.compilable(p) => Some(p),
                _ => None,
            })
            .collect();
        entries.extend(splits);
        entries
            .iter()
            .map(|&e| form_block(&ctx, e, &entries, max).0)
            .collect::<Vec<_>>()
    };
    stats.entries = blocks.len();
    stats.instrs = blocks.iter().map(|b| b.instrs.len()).sum();

    analysis.blocks = blocks;
    analysis.stats = stats;
    analysis.code_starts = visited;
    analysis.call_targets = call_targets;
    analysis.entries = entries;
    analysis
}

/// The block starting at `start`, and the place it falls into, if any.
fn form_block(
    ctx: &Ctx,
    start: Place,
    entries: &BTreeSet<Place>,
    max: usize,
) -> (Block, Option<Place>) {
    let mut at = start;
    let mut instrs = Vec::new();
    loop {
        let reached_other = at != start && entries.contains(&at);
        if reached_other || instrs.len() >= max || !ctx.compilable(at) {
            return (
                Block {
                    start,
                    instrs,
                    exit: Exit::FallInto(at.addr),
                },
                Some(at),
            );
        }
        let d = ctx.analysis.decode(at);
        let interpret = ctx.interpret(at);
        instrs.push(BlockInstr { at, d, interpret });
        let terminates = match d.instr.flow() {
            Flow::Jump(_) | Flow::Indirect | Flow::Halt => true,
            Flow::Return { conditional } => !conditional,
            Flow::Call {
                conditional,
                target,
            } => {
                !conditional
                    || ctx.cfg.analysis.noreturn.contains(&target)
                    || ctx.cfg.analysis.inline_strings.contains(&target)
                    || ctx.cfg.analysis.banked_calls.contains(&target)
            }
            Flow::Next | Flow::Branch(_) => false,
        };
        if terminates {
            return (
                Block {
                    start,
                    instrs,
                    exit: Exit::Terminated,
                },
                None,
            );
        }
        let wrapped = at.addr.wrapping_add(u16::from(d.len)) < at.addr;
        match ctx.next(at, d.len) {
            Some(n) if !wrapped => at = n,
            // Wrapped past 0xFFFF, or into a bank that can't be known.
            other => {
                return (
                    Block {
                        start,
                        instrs,
                        exit: Exit::FallInto(at.addr.wrapping_add(u16::from(d.len))),
                    },
                    other,
                );
            }
        }
    }
}
