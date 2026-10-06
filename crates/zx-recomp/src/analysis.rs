//! Code discovery and block formation.
//!
//! Entry points come from the trace, the config, the runtime's miss log and
//! the tape's entry point. From those, recursive descent follows every statically
//! known branch, call and return address. Each entry point then becomes a
//! block: a straight run of instructions ending at an unconditional control
//! transfer or where another entry point begins.

use std::collections::BTreeSet;

use crate::config::Config;
use zx_core::{Decoded, Flow, decode};
use zx_runtime::trace::Trace;

pub struct BlockInstr {
    pub addr: u16,
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
    pub start: u16,
    pub instrs: Vec<BlockInstr>,
    pub exit: Exit,
}

pub struct Analysis {
    /// Memory image code was compiled from: the tape's program with the ROM, with
    /// code bytes as they were when the trace executed them.
    pub image: Vec<u8>,
    pub rom_loaded: bool,
    pub blocks: Vec<Block>,
    pub stats: Stats,
    /// Instruction start addresses found by recursive descent.
    pub code_starts: Vec<bool>,
    /// Targets of CALL and RST instructions.
    pub call_targets: BTreeSet<u16>,
    /// Block entry points.
    pub entries: BTreeSet<u16>,
}

#[derive(Default, Debug)]
pub struct Stats {
    pub traced_entries: usize,
    pub traced_instrs: usize,
    pub self_modifying: Vec<u16>,
    pub entries: usize,
    pub instrs: usize,
}

struct Ctx<'a> {
    cfg: &'a Config,
    image: &'a [u8],
    trace: &'a Trace,
    rom_loaded: bool,
}

impl Ctx<'_> {
    fn decode(&self, pc: u16) -> Decoded {
        decode(&|a| self.image[a as usize], pc)
    }

    /// Whether the instruction at `pc` must be left to the interpreter.
    fn interpret(&self, pc: u16) -> bool {
        self.trace.self_modified[pc as usize]
            || self
                .cfg
                .analysis
                .interpret
                .iter()
                .any(|[lo, hi]| (*lo..=*hi).contains(&pc))
    }

    /// Whether code at `pc` can be compiled at all.
    fn compilable(&self, pc: u16) -> bool {
        pc >= 0x4000 || self.rom_loaded
    }
}

pub fn analyze(
    cfg: &Config,
    start_memory: &[u8],
    start_pc: u16,
    rom_loaded: bool,
    trace: &Trace,
    extra_entries: &[u16],
) -> Analysis {
    let mut image = start_memory.to_vec();
    let mut stats = Stats::default();
    for (pc, entry) in trace.executed.iter().enumerate() {
        if let Some((len, bytes)) = entry {
            stats.traced_instrs += 1;
            for (i, &b) in bytes.iter().enumerate().take(*len as usize) {
                image[(pc + i) & 0xFFFF] = b;
            }
        }
    }
    stats.self_modifying = (0..0x10000)
        .filter(|&a| trace.self_modified[a])
        .map(|a| a as u16)
        .collect();

    let ctx = Ctx {
        cfg,
        image: &image,
        trace,
        rom_loaded,
    };

    // --- seeds --------------------------------------------------------------
    let mut entries = BTreeSet::new();
    for a in 0..0x10000usize {
        if trace.entries[a] {
            entries.insert(a as u16);
        }
    }
    stats.traced_entries = entries.len();
    entries.insert(start_pc);
    if rom_loaded {
        entries.insert(0x0038);
    }
    entries.extend(cfg.analysis.entry_points.iter().copied());
    entries.extend(extra_entries.iter().copied());
    entries.retain(|&a| ctx.compilable(a));

    // --- recursive descent --------------------------------------------------
    let mut visited = vec![false; 0x10000];
    let mut call_targets = BTreeSet::new();
    let mut work: Vec<u16> = entries.iter().copied().collect();
    while let Some(start) = work.pop() {
        let mut pc = start;
        loop {
            if visited[pc as usize] || !ctx.compilable(pc) {
                break;
            }
            visited[pc as usize] = true;
            let d = ctx.decode(pc);
            let next = pc.wrapping_add(d.len as u16);
            let mut target = |a: u16, work: &mut Vec<u16>| {
                if ctx.compilable(a) && entries.insert(a) {
                    work.push(a);
                }
            };
            match d.instr.flow() {
                Flow::Next => {}
                Flow::Jump(a) => {
                    target(a, &mut work);
                    break;
                }
                Flow::Branch(a) => target(a, &mut work),
                Flow::Call { target: a, .. } => {
                    call_targets.insert(a);
                    target(a, &mut work);
                    if cfg.analysis.inline_strings.contains(&a) {
                        // Resume after the string's terminator.
                        let mut end = next;
                        while ctx.image[end as usize] != 0xFF && end != 0xFFFF {
                            end += 1;
                        }
                        target(end.wrapping_add(1), &mut work);
                        break;
                    }
                    if cfg.analysis.noreturn.contains(&a) {
                        break;
                    }
                    // The return lands here: a block boundary.
                    target(next, &mut work);
                }
                Flow::Return { conditional } => {
                    if !conditional {
                        break;
                    }
                }
                Flow::Indirect => break,
                Flow::Halt => {
                    target(next, &mut work);
                    break;
                }
            }
            pc = next;
        }
    }

    // --- blocks -------------------------------------------------------------
    // Splitting over-long blocks adds entries, which can only shorten other
    // blocks, so one pass to find split points is enough.
    let max = cfg.analysis.max_block_instrs.max(1);
    let splits: Vec<u16> = entries
        .iter()
        .filter_map(|&e| match form_block(&ctx, e, &entries, max).exit {
            Exit::FallInto(a) if !entries.contains(&a) => Some(a),
            _ => None,
        })
        .collect();
    entries.extend(splits.into_iter().filter(|&a| ctx.compilable(a)));

    let blocks: Vec<Block> = entries
        .iter()
        .map(|&e| form_block(&ctx, e, &entries, max))
        .collect();
    stats.entries = blocks.len();
    stats.instrs = blocks.iter().map(|b| b.instrs.len()).sum();

    Analysis {
        image,
        rom_loaded,
        blocks,
        stats,
        code_starts: visited,
        call_targets,
        entries,
    }
}

fn form_block(ctx: &Ctx, start: u16, entries: &BTreeSet<u16>, max: usize) -> Block {
    let mut pc = start;
    let mut instrs = Vec::new();
    loop {
        let reached_other = pc != start && entries.contains(&pc);
        if reached_other || instrs.len() >= max || !ctx.compilable(pc) {
            return Block {
                start,
                instrs,
                exit: Exit::FallInto(pc),
            };
        }
        let d = ctx.decode(pc);
        let interpret = ctx.interpret(pc);
        instrs.push(BlockInstr {
            addr: pc,
            d,
            interpret,
        });
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
            }
            Flow::Next | Flow::Branch(_) => false,
        };
        let next = pc.wrapping_add(d.len as u16);
        if terminates {
            return Block {
                start,
                instrs,
                exit: Exit::Terminated,
            };
        }
        if next < pc {
            // Wrapped past 0xFFFF; let the dispatcher take it from there.
            return Block {
                start,
                instrs,
                exit: Exit::FallInto(next),
            };
        }
        pc = next;
    }
}
