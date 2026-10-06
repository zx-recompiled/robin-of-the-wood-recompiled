//! Differential checks: each rewritten routine of Robin of the Wood beside
//! the original's, from the original's own real calls, byte for byte (#6,
//! #24).
//!
//! Usage: `robin-verify [ASSETS_DIR]`. It needs the supported tape and
//! `128.rom`, and fails without them, since a check that didn't run must not
//! look like one that passed. The original boots from the tape and plays
//! `tools/re/robin.toml`'s script; every call it makes to a rewritten routine
//! is a case (`capture`). Nothing is stored: the states are the original's
//! memory, which is never written to a file.

mod capture;
mod suites;

use std::path::PathBuf;
use std::process::ExitCode;

use robin::Game;
use robin::assets::BANK;
use zx_runtime::{Misses, Zx, bus, interp, loader::boot_128k};

use capture::Tally;

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("robin-verify: {e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<bool, String> {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "assets".into()));
    let tape = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .filter_map(|e| std::fs::read(e.path()).ok())
        .find(|b| robin::is_the_tape(b))
        .ok_or_else(|| format!("no supported tape in {} (assets/README.md)", dir.display()))?;
    let rom = std::fs::read(dir.join("128.rom"))
        .map_err(|e| format!("{}: {e} (assets/README.md)", dir.join("128.rom").display()))?;

    let assets = robin::assets::read_tape(&tape)?;
    let blocks = zx_core::tape::load_tzx(&tape)?;
    let mut z = boot_128k(&rom, blocks, robin::layout::ENTRY_PC, 2000)?;
    let cfg = zx_recomp::Config::parse(include_str!("../../re/robin.toml"))?;
    let mut script = zx_recomp::script::Script::new(&cfg.trace)?;

    let routines = suites::all();
    let mut tallies: Vec<Tally> = routines.iter().map(|_| Tally::default()).collect();
    let mut table_writes: Vec<String> = Vec::new();
    let mut misses = Misses::default();
    let start = std::time::Instant::now();
    for frame in 0..cfg.trace.frames {
        script.press(frame, &mut z);
        z.run_frame(
            |z: &mut Zx| {
                for (r, t) in routines.iter().zip(tallies.iter_mut()) {
                    if r.is_entered(z) {
                        r.capture(z, t);
                    }
                }
                let pc = z.pc;
                if !suites::may_write_tables(&routines, pc) {
                    let d = interp::decode_at(z, pc);
                    for c in bus::cycles(z, &d, pc).iter() {
                        if c.kind != bus::Kind::Write
                            || z.memory.slot(3) != zx_runtime::memory::Memory::bank(0)
                        {
                            continue;
                        }
                        for (name, lo, hi) in suites::TABLES {
                            if (lo..=hi).contains(&c.at) && table_writes.len() < 20 {
                                table_writes.push(format!("{pc:04x} wrote {:04x} in {name}", c.at));
                            }
                        }
                    }
                }
                false
            },
            &mut misses,
        );
    }
    println!(
        "robin-verify: {} frames of play from the tape in {:.1?}",
        cfg.trace.frames,
        start.elapsed()
    );

    let mut ok = true;
    for (r, t) in routines.iter().zip(&tallies) {
        let unreached = r.unreached(&z, t);
        println!(
            "  {}: {} calls, {} compared, {} repeats skipped, {} scrambling checks, {} varied runs ({} did not return); {} instruction(s) never reached{}",
            r.name,
            t.calls,
            t.compared,
            t.repeats,
            t.scrambled,
            t.varied,
            t.varied_hung,
            unreached.len(),
            if unreached.is_empty() {
                String::new()
            } else {
                format!(
                    ": {}",
                    unreached
                        .iter()
                        .map(|a| format!("{a:04x}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            }
        );
        for f in &t.failures {
            println!("    FAIL {f}");
            ok = false;
        }
        if t.compared == 0 {
            println!("    FAIL no call was compared: a suite that saw nothing proves nothing");
            ok = false;
        }
    }

    // The tables: built by the rewrite from the tape alone, they must be what
    // the original holds after playing, and nothing but their builders may
    // have written them.
    let mut from_tape = Box::new([[0u8; BANK]; 8]);
    for (n, b) in from_tape.iter_mut().enumerate() {
        *b = *assets.bank(n);
    }
    let mut g = Game::from_memory(&from_tape);
    robin::screen::build_mirror(&mut g);
    robin::screen::build_rows(&mut g);
    let mut now = Box::new([[0u8; BANK]; 8]);
    for (n, b) in now.iter_mut().enumerate() {
        *b = *z.memory.page(zx_runtime::memory::Memory::bank(n));
    }
    let played = Game::from_memory(&now);
    let tables_match = g.mirror == played.mirror && g.rows == played.rows;
    println!(
        "  the start-up tables, built from the tape alone: {}",
        if tables_match {
            "as the original's"
        } else {
            "DIFFERENT from the original's"
        }
    );
    ok &= tables_match;
    for w in &table_writes {
        println!("    FAIL {w}, after start-up: only their builders may");
        ok = false;
    }
    println!("robin-verify: {}", if ok { "all match" } else { "FAILED" });
    Ok(ok)
}
