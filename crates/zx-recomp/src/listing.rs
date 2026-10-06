//! Annotated disassembly listing, for reverse engineering.
//!
//! Code found by recursive descent is disassembled with labels; everything
//! else is dumped as data. Instructions the trace actually executed are
//! marked `*`, so code only found statically (possibly data misread as
//! code) stands out.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::analysis::Analysis;
use zx_core::{Addr, Instr, Op8, decode};
use zx_runtime::trace::Trace;

fn label(a: u16, analysis: &Analysis) -> String {
    if analysis.call_targets.contains(&a) {
        format!("sub_{a:04x}")
    } else {
        format!("l_{a:04x}")
    }
}

/// Addresses an instruction refers to as data (for cross references).
fn data_refs(i: &Instr) -> Vec<u16> {
    let op = |o: &Op8| match o {
        Op8::Mem(Addr::Abs(a)) => Some(*a),
        _ => None,
    };
    match i {
        Instr::Ld8(d, s) => op(d).into_iter().chain(op(s)).collect(),
        Instr::Ld16(_, nn) if *nn >= 0x4000 => vec![*nn],
        Instr::Ld16Load(_, a) | Instr::Ld16Store(a, _) => vec![*a],
        _ => Vec::new(),
    }
}

pub fn listing(analysis: &Analysis, trace: &Trace, from: u16, to: u16) -> String {
    let image = &analysis.image;
    let mem = |a: u16| image[a as usize];

    // Cross references: who jumps/calls to, and who reads/writes, each address.
    let mut xrefs: BTreeMap<u16, Vec<u16>> = BTreeMap::new();
    for a in 0..0x10000usize {
        if !analysis.code_starts[a] {
            continue;
        }
        let d = decode(&mem, a as u16);
        let targets: Vec<u16> = match d.instr.flow() {
            zx_core::Flow::Jump(t) | zx_core::Flow::Branch(t) => vec![t],
            zx_core::Flow::Call { target, .. } => vec![target],
            _ => data_refs(&d.instr),
        };
        for t in targets {
            xrefs.entry(t).or_default().push(a as u16);
        }
    }

    let mut out = String::new();
    let mut a = from as u32;
    while a <= to as u32 {
        let pc = a as u16;
        if analysis.code_starts[pc as usize] {
            let d = decode(&mem, pc);
            if analysis.entries.contains(&pc) || xrefs.contains_key(&pc) {
                let refs = xrefs.get(&pc).map_or(String::new(), |r| {
                    let list: Vec<String> = r.iter().take(8).map(|x| format!("{x:04x}")).collect();
                    let more = if r.len() > 8 { " ..." } else { "" };
                    format!("  ; from {}{more}", list.join(" "))
                });
                let _ = writeln!(out, "\n{}:{refs}", label(pc, analysis));
            }
            let bytes: Vec<String> = (0..d.len as u16)
                .map(|i| format!("{:02x}", mem(pc.wrapping_add(i))))
                .collect();
            let traced = if trace.executed[pc as usize].is_some() {
                '*'
            } else {
                ' '
            };
            let smc = if trace.self_modified[pc as usize] {
                "  ; SELF-MODIFIED"
            } else {
                ""
            };
            let _ = writeln!(
                out,
                "{pc:04x} {traced} {:<12} {}{smc}",
                bytes.join(" "),
                d.instr
            );
            a += d.len as u32;
        } else {
            // Data: up to 16 bytes, stopping at the next code or label.
            let start = pc;
            let mut n = 0u32;
            while n < 16
                && a + n <= to as u32
                && !analysis.code_starts[(a + n) as usize]
                && (n == 0 || !xrefs.contains_key(&((a + n) as u16)))
            {
                n += 1;
            }
            if let Some(r) = xrefs.get(&start) {
                let list: Vec<String> = r.iter().take(8).map(|x| format!("{x:04x}")).collect();
                let _ = writeln!(out, "\nd_{start:04x}:  ; used by {}", list.join(" "));
            }
            let bytes: Vec<u8> = (0..n).map(|i| mem(start.wrapping_add(i as u16))).collect();
            let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
            let ascii: String = bytes
                .iter()
                .map(|&b| {
                    if (0x20..0x7f).contains(&b) {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            let written = (0..n).any(|i| trace.written_code[(a + i) as usize]);
            let _ = writeln!(
                out,
                "{start:04x}   db {:<47} ; {ascii}{}",
                hex.join(" "),
                if written { " (written)" } else { "" }
            );
            a += n;
        }
    }
    out
}
