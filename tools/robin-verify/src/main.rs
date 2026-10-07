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
mod tour;

use std::path::PathBuf;
use std::process::ExitCode;

use robin::Game;
use robin::assets::BANK;
use zx_runtime::{Misses, Zx, bus, interp, loader::boot_128k};

use capture::{Play, Tally};

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

/// The suites, and what they have seen.
pub struct Verifier {
    pub routines: Vec<capture::Routine>,
    pub tallies: Vec<Tally>,
    pub table_writes: Vec<String>,
}

impl Verifier {
    /// Looks at the instruction `z` is about to run: a call to a rewritten
    /// routine is a case, and a write to the start-up tables after their
    /// builders is a failure.
    pub fn observe(&mut self, z: &Zx, play: &Play) {
        for (r, t) in self.routines.iter().zip(self.tallies.iter_mut()) {
            if r.is_entered(z) {
                r.capture(z, play, t);
            }
        }
        let pc = z.pc;
        if !suites::may_write_tables(&self.routines, pc) {
            let d = interp::decode_at(z, pc);
            for c in bus::cycles(z, &d, pc).iter() {
                if c.kind != bus::Kind::Write
                    || z.memory.slot(3) != zx_runtime::memory::Memory::bank(0)
                {
                    continue;
                }
                for (name, lo, hi) in suites::TABLES {
                    if (lo..=hi).contains(&c.at) && self.table_writes.len() < 20 {
                        self.table_writes
                            .push(format!("{pc:04x} wrote {:04x} in {name}", c.at));
                    }
                }
            }
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
    let mut v = Verifier {
        tallies: routines.iter().map(|_| Tally::default()).collect(),
        routines,
        table_writes: Vec::new(),
    };
    let mut misses = Misses::default();
    let start = std::time::Instant::now();
    // Where the tour starts: the first time the main loop begins once the
    // game is under way.
    let mut tour_start: Option<(Zx, u32)> = None;
    for frame in 0..cfg.trace.frames {
        script.press(frame, &mut z);
        z.run_frame(
            |z: &mut Zx| {
                if tour_start.is_none() && frame > tour::AFTER && tour::at_main_loop(z) {
                    tour_start = Some((z.clone(), frame));
                }
                v.observe(
                    z,
                    &Play {
                        script: &script,
                        frame,
                        assets: &assets,
                    },
                );
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
    let played: Vec<u64> = v.tallies.iter().map(|t| t.compared).collect();

    let start = std::time::Instant::now();
    let (z0, frame) = tour_start.ok_or("the game never reached its main loop: no tour")?;
    let quiet = zx_recomp::script::Script::new(&zx_recomp::Config::parse(tour::QUIET)?.trace)?;
    let toured = tour::run(z0, frame, &quiet, &assets, &mut v);
    println!(
        "robin-verify: the tour drew {} of {} locations in {:.1?}{}",
        toured.drawn.len(),
        robin::map::LOCATIONS,
        start.elapsed(),
        if toured.drawn.contains(&tour::THROUGH_ITS_DRAWING) {
            ", 0x69 through its drawing alone"
        } else {
            ""
        }
    );
    let (tallies, routines, table_writes) = (&v.tallies, &v.routines, &v.table_writes);

    let mut ok = toured.problems.is_empty() && toured.drawn.len() == robin::map::LOCATIONS;
    for p in &toured.problems {
        println!("  FAIL the tour: {p}");
    }
    if toured.drawn.len() != robin::map::LOCATIONS {
        println!(
            "  FAIL the tour drew {} locations, not all {}",
            toured.drawn.len(),
            robin::map::LOCATIONS
        );
    }
    for ((r, t), &before) in routines.iter().zip(tallies).zip(&played) {
        let unreached = r.unreached(&z, t);
        println!(
            "  {}: {} calls, {} compared ({} from the tour), {} repeats skipped, {} scrambling checks ({} with the stack left out), {} varied runs ({} did not return); {} instruction(s) never reached{}",
            r.name,
            t.calls,
            t.compared,
            t.compared - before,
            t.repeats,
            t.scrambled,
            t.stack_left_out,
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
    let tables_match =
        g.display.mirror == played.display.mirror && g.display.rows == played.display.rows;
    println!(
        "  the start-up tables, built from the tape alone: {}",
        if tables_match {
            "as the original's"
        } else {
            "DIFFERENT from the original's"
        }
    );
    ok &= tables_match;
    for w in table_writes {
        println!("    FAIL {w}, after start-up: only their builders may");
        ok = false;
    }
    println!("robin-verify: {}", if ok { "all match" } else { "FAILED" });
    Ok(ok)
}
