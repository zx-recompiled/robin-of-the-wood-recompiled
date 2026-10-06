//! Annotated disassembly listing, for reverse engineering.
//!
//! Code found by recursive descent is disassembled with labels; everything
//! else is dumped as data. Instructions the trace actually executed are
//! marked `*`, so code only found statically (possibly data misread as
//! code) stands out.
//!
//! There is a section for each page of memory that holds code: on a 48K, the
//! RAM; on a 128K, the fixed banks 5 and 2 and each bank seen paged at
//! 0xC000. Places in a paged bank are written `bank:address` (`0:cd5f`),
//! everything else by its address alone.
//!
//! A listing is the original's code, disassembled. It is never committed;
//! what goes into the repository is the notes written from it.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::analysis::{Analysis, Place};
use zx_core::{Addr, Instr, Op8};
use zx_runtime::memory::PAGE;
use zx_runtime::trace::Trace;

fn label(at: Place, analysis: &Analysis) -> String {
    if analysis.call_targets.contains(&at) {
        format!("sub_{at}")
    } else {
        format!("l_{at}")
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

/// What a page is called in a section heading.
fn page_name(analysis: &Analysis, page: usize, first: Place) -> String {
    match (first.bank, analysis.layout.fixed[0] == Some(page)) {
        (Some(b), _) => format!("bank {b}, at c000"),
        (None, true) => "ROM".to_string(),
        (None, false) => {
            let base = first.addr & 0xC000;
            if analysis.layout.fixed[3].is_none() {
                format!("bank {}, at {base:04x}", page - 2)
            } else {
                format!("RAM at {base:04x}")
            }
        }
    }
}

pub fn listing(analysis: &Analysis, trace: &Trace) -> String {
    let pages = analysis.image.len() / PAGE;

    // Cross references: who jumps or calls to, and who reads or writes, each
    // place.
    let mut xrefs: BTreeMap<Place, Vec<Place>> = BTreeMap::new();
    for i in 0..analysis.code_starts.len() {
        if !analysis.code_starts[i] {
            continue;
        }
        let from = analysis.layout.place_of(i);
        let d = analysis.decode(from);
        let targets: Vec<u16> = match d.instr.flow() {
            zx_core::Flow::Jump(t) | zx_core::Flow::Branch(t) => vec![t],
            zx_core::Flow::Call { target, .. } => vec![target],
            _ => data_refs(&d.instr),
        };
        for t in targets {
            for p in analysis.places(trace, from, t) {
                xrefs.entry(p).or_default().push(from);
            }
        }
    }
    let refs = |r: &[Place]| {
        let list: Vec<String> = r.iter().take(8).map(ToString::to_string).collect();
        let more = if r.len() > 8 { " ..." } else { "" };
        format!("{}{more}", list.join(" "))
    };

    // The fixed pages in address order, then the paged banks.
    let fixed: Vec<usize> = analysis.layout.fixed.iter().flatten().copied().collect();
    let order = fixed
        .iter()
        .copied()
        .chain((0..pages).filter(|p| !fixed.contains(p)));

    let mut out = String::new();
    for page in order {
        let base = page * PAGE;
        if !analysis.code_starts[base..base + PAGE].iter().any(|&c| c) {
            continue;
        }
        // From the first code, or the first data something refers to, in
        // the page: on a 48K that skips the screen and BASIC's variables.
        let first = (0..PAGE)
            .find(|&o| {
                analysis.code_starts[base + o]
                    || xrefs.contains_key(&analysis.layout.place_of(base + o))
            })
            .unwrap_or(0)
            & !0xF;
        let _ = writeln!(
            out,
            "\n; ==== {} ====",
            page_name(analysis, page, analysis.layout.place_of(base))
        );
        let mut o = first;
        while o < PAGE {
            let at = analysis.layout.place_of(base + o);
            let byte = |k: usize| analysis.image[base + (o + k) % PAGE];
            if analysis.code_starts[base + o] {
                let d = analysis.decode(at);
                if analysis.entries.contains(&at) || xrefs.contains_key(&at) {
                    let from = xrefs
                        .get(&at)
                        .map_or(String::new(), |r| format!("  ; from {}", refs(r)));
                    let _ = writeln!(out, "\n{}:{from}", label(at, analysis));
                }
                let bytes: Vec<String> = (0..usize::from(d.len))
                    .map(|k| format!("{:02x}", byte(k)))
                    .collect();
                let traced = if trace.executed[base + o].is_some() {
                    '*'
                } else {
                    ' '
                };
                let smc = if trace.self_modified[base + o] {
                    "  ; SELF-MODIFIED"
                } else {
                    ""
                };
                let _ = writeln!(
                    out,
                    "{:<6} {traced} {:<12} {}{smc}",
                    at.to_string(),
                    bytes.join(" "),
                    d.instr
                );
                o += usize::from(d.len);
            } else {
                // Data: up to 16 bytes, stopping at the next code or label.
                let mut n = 0usize;
                while n < 16
                    && o + n < PAGE
                    && !analysis.code_starts[base + o + n]
                    && (n == 0 || !xrefs.contains_key(&analysis.layout.place_of(base + o + n)))
                {
                    n += 1;
                }
                if let Some(r) = xrefs.get(&at) {
                    let _ = writeln!(out, "\nd_{at}:  ; used by {}", refs(r));
                }
                let bytes: Vec<u8> = (0..n).map(byte).collect();
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
                let written = (0..n).any(|k| trace.written_code[base + o + k]);
                let _ = writeln!(
                    out,
                    "{:<6}   db {:<47} ; {ascii}{}",
                    at.to_string(),
                    hex.join(" "),
                    if written { " (written)" } else { "" }
                );
                o += n;
            }
        }
    }
    out
}
