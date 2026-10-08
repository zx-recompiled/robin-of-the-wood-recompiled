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

mod armed;
mod capture;
mod checker;
mod collector;
mod methods;
mod suites;
mod tour;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use robin::Game;
use robin::assets::{Assets, BANK};
use zx_runtime::{Misses, Zx, bus, interp, loader::boot_128k};

use capture::Play;
use checker::{Checker, Job};

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
    pub routines: Arc<[capture::Routine]>,
    pub table_writes: Vec<String>,
    /// Checks the calls caught, on every core, while play goes on (#45).
    checker: Checker,
    /// Which routines start at each address, so an instruction is looked up
    /// once, not against every routine.
    entries: Vec<Vec<usize>>,
    /// The code allowed to write the start-up tables.
    builders: Vec<(u16, u16)>,
}

impl Verifier {
    fn new(routines: Vec<capture::Routine>, assets: &Arc<Assets>) -> Verifier {
        let routines: Arc<[capture::Routine]> = routines.into();
        let mut entries = vec![Vec::new(); 0x10000];
        for (n, r) in routines.iter().enumerate() {
            entries[usize::from(r.entry)].push(n);
        }
        Verifier {
            builders: suites::table_builders(&routines),
            checker: Checker::new(Arc::clone(&routines), assets),
            routines,
            table_writes: Vec::new(),
            entries,
        }
    }

    /// Looks at the instruction `z` is about to run: a call to a rewritten
    /// routine is a case, and a write to the start-up tables after their
    /// builders is a failure.
    pub fn observe(&mut self, z: &Zx, play: &Play) {
        for &n in &self.entries[usize::from(z.pc)] {
            if self.routines[n].is_entered(z) {
                self.checker.take(Job {
                    routine: n,
                    entry: z.clone(),
                    script: play.script.clone(),
                    frame: play.frame,
                });
            }
        }
        let pc = z.pc;
        if !self
            .builders
            .iter()
            .any(|&(lo, hi)| (lo..=hi).contains(&pc))
        {
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().cloned().unwrap_or_else(|| "assets".into()));
    let tape = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .filter_map(|e| std::fs::read(e.path()).ok())
        .find(|b| robin::is_the_tape(b))
        .ok_or_else(|| format!("no supported tape in {} (assets/README.md)", dir.display()))?;
    let rom = std::fs::read(dir.join("128.rom"))
        .map_err(|e| format!("{}: {e} (assets/README.md)", dir.join("128.rom").display()))?;

    let assets = Arc::new(robin::assets::read_tape(&tape)?);
    let blocks = zx_core::tape::load_tzx(&tape)?;
    let mut z = boot_128k(&rom, blocks, robin::layout::ENTRY_PC, 2000)?;
    let cfg = zx_recomp::Config::parse(include_str!("../../re/robin.toml"))?;
    let mut script = zx_recomp::script::Script::new(&cfg.trace)?;

    let routines = suites::all();
    let mut v = Verifier::new(routines, &assets);
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
    // Reading the tallies waits for every call caught to be checked.
    let played: Vec<u64> = v.checker.tallies().iter().map(|t| t.compared).collect();
    println!(
        "robin-verify: {} frames of play from the tape in {:.1?}",
        cfg.trace.frames,
        start.elapsed()
    );

    let start = std::time::Instant::now();
    let (z0, frame) = tour_start.ok_or("the game never reached its main loop: no tour")?;
    let quiet = zx_recomp::script::Script::new(&zx_recomp::Config::parse(tour::QUIET)?.trace)?;
    let toured = tour::run(z0, frame, &quiet, &assets, &mut v);
    let toured_counts: Vec<u64> = v.checker.tallies().iter().map(|t| t.compared).collect();
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
    println!(
        "robin-verify: of the tour's moves, {} walked off the edge and {} tried but blocked by a wall or a character",
        toured.walked, toured.fell_back,
    );
    let method_problems = methods::run(&rom, &tape, &assets, &mut v)?;
    let armed = armed::run(&rom, &tape, &assets, &mut v)?;
    // What all the runs reached together is the actions' coverage report.
    println!(
        "robin-verify: played a game with Robin armed (a supplement): his actions ran in states {:?}",
        armed.states
    );
    let picked = collector::run(&rom, &tape, &assets, &mut v)?;
    println!(
        "robin-verify: played a game with things to pick up where Robin starts (a supplement): he took {picked}"
    );
    let Verifier {
        routines,
        table_writes,
        checker,
        ..
    } = &mut v;
    let tallies = checker.tallies();

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
    for p in &method_problems {
        println!("  FAIL the control methods: {p}");
    }
    ok &= method_problems.is_empty();
    for (((r, t), &before), &toured) in routines
        .iter()
        .zip(tallies)
        .zip(&played)
        .zip(&toured_counts)
    {
        let unreached = r.unreached(&z, t);
        println!(
            "  {}: {} calls, {} compared ({} from the tour, {} in the other games), {} repeats skipped, {} scrambling checks ({} with the stack left out), {} varied runs ({} did not return); {} instruction(s) never reached{}",
            r.name,
            t.calls,
            t.compared,
            toured - before,
            t.compared - toured,
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
        if t.rom_reads > 0 {
            println!(
                "    SKIPPED {} run(s) in which the original read the ROM, which the rewrite has none of (#21); first: {}",
                t.rom_reads,
                t.first_rom_read
                    .as_deref()
                    .unwrap_or("in a second or varied run")
            );
        }
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
