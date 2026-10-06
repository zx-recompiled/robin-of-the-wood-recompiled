//! Checks the reference interpreter against measurements of a real Z80.
//!
//! `tests/fuse.rs` checks it against the Fuse corpus, which is one emulator's
//! description of the processor. Where that description is wrong, matching it
//! certifies the mistake. Patrik Rak's z80test (raxoft/z80test, MIT) runs
//! every instruction over a large set of inputs and compares a checksum of
//! the results with one taken on a real 48K Spectrum with a Zilog Z80. So this
//! is the authority on flags and registers, and Fuse is kept for timing.
//!
//! Three variants are run: `z80full` (every flag and register), `z80ccf`
//! (`CCF` after every instruction, which shows the hidden Q register) and
//! `z80memptr` (`BIT n,(HL)` after every instruction, which shows MEMPTR). The
//! others are subsets of `z80full`, or, for `z80ccfscr`, a picture with no
//! verdict.
//!
//! The tapes are not in this repository: they would trip the guard against
//! committed tapes, and they are fetched in CI. See `assets/README.md`.
//! Without them this test says so and passes, and the count it prints makes a
//! vacuous run obvious.

use std::path::PathBuf;

use zx_core::{MachineState, tape};
use zx_runtime::{Zx, interp};

/// Where z80test is loaded and started: `LOAD "" CODE` puts it here.
const START: u16 = 0x8000;
/// Below the program, as `CLEAR 32767` leaves it.
const STACK: u16 = 0x7FF0;
/// The ROM entry points z80test uses, and the interrupt routine its printing
/// enables interrupts around.
const RST_10: u16 = 0x0010;
const CHAN_OPEN: u16 = 0x1601;
const IM1: u16 = 0x0038;
/// Where the program returns to when it is done.
const SENTINEL: u16 = 0x0000;

/// What `IN A,(0xFE)` returns on the machine the expected results were taken
/// on, with no key pressed: everything high but bit 6. z80test checks for it
/// before its `IN` tests.
fn port_in(_: u16) -> u8 {
    0xBF
}

/// A stand-in for the ROM, so the test needs no `48.rom`.
///
/// z80test opens the screen channel through `CHAN-OPEN` and prints through
/// `RST 0x10`. Both just return here: the characters are read from A as the
/// call arrives. Interrupts are never raised, but the routine they would run
/// returns too.
fn stub_rom(z: &mut Zx) {
    const RET: u8 = 0xC9;
    const EI: u8 = 0xFB;
    z.mem[RST_10 as usize] = RET;
    z.mem[CHAN_OPEN as usize] = RET;
    z.mem[IM1 as usize] = EI;
    z.mem[IM1 as usize + 1] = RET;
}

/// The text z80test prints, as lines.
///
/// It positions its verdicts with the ROM's `TAB` control (23, then a column
/// and a byte), which becomes one space here; 13 ends a line, and the
/// copyright sign (127) is dropped.
#[derive(Default)]
struct Screen {
    lines: Vec<String>,
    line: String,
    tab_bytes: u8,
}

impl Screen {
    fn print(&mut self, c: u8) {
        if self.tab_bytes > 0 {
            self.tab_bytes -= 1;
            return;
        }
        match c {
            13 => self.lines.push(std::mem::take(&mut self.line)),
            23 => {
                self.line.push(' ');
                self.tab_bytes = 2;
            }
            32..=126 => self.line.push(c as char),
            _ => {}
        }
    }
}

/// How a variant went.
struct Outcome {
    /// Every line printed.
    lines: Vec<String>,
    passed: usize,
    /// Tests z80test runs only to explain an earlier failure (the `NEC` and
    /// `ST` variants of `SCF` and `CCF`), skipped when there was none.
    skipped: usize,
    /// Each failed test's line and the line after it (the checksums).
    failed: Vec<String>,
    /// The program's own verdict, its last line.
    verdict: String,
}

fn tapes() -> PathBuf {
    std::env::var_os("Z80TEST").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets"),
        PathBuf::from,
    )
}

/// Runs one variant to the end, or says why it cannot and returns `None`.
fn run(variant: &str) -> Option<Outcome> {
    let path = tapes().join(format!("{variant}.tap"));
    let Ok(bytes) = std::fs::read(&path) else {
        println!(
            "skipped: no {}; see assets/README.md for where to get z80test",
            path.display()
        );
        return None;
    };
    let tape = tape::load_tap(&bytes).expect("z80test tape loads");
    let mut z = Zx::new(&MachineState::from_tape(&tape, START, STACK), None);
    z.port_in_hook = Some(port_in);
    stub_rom(&mut z);

    // Called like `RANDOMIZE USR 32768`, returning to a sentinel.
    z.push(SENTINEL);
    let mut screen = Screen::default();
    // z80full, the longest, runs about 190 million instructions. The limit
    // only stops a run that has lost its way.
    let mut left: u64 = 1_000_000_000;
    loop {
        if z.pc == RST_10 {
            screen.print(z.a);
        }
        interp::step(&mut z);
        if z.pc == SENTINEL && z.sp == STACK {
            break;
        }
        // No frames here, so keep the frame clock from overflowing; nothing
        // in the test reads it.
        if z.t > 1 << 30 {
            z.t = 0;
        }
        left -= 1;
        assert!(left > 0, "{variant} did not return");
    }
    if !screen.line.is_empty() {
        screen.lines.push(std::mem::take(&mut screen.line));
    }

    let lines = screen.lines;
    let passed = lines.iter().filter(|l| l.ends_with(" OK")).count();
    let skipped = lines.iter().filter(|l| l.ends_with(" Skipped")).count();
    let failed = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.ends_with(" FAILED"))
        .map(|(i, l)| format!("{l} {}", lines.get(i + 1).map_or("", |s| s.trim())))
        .collect();
    let verdict = lines
        .iter()
        .rev()
        .find(|l| l.starts_with("Result:"))
        .cloned()
        .unwrap_or_default();
    Some(Outcome {
        lines,
        passed,
        skipped,
        failed,
        verdict,
    })
}

fn check(variant: &str) {
    let Some(out) = run(variant) else {
        return;
    };
    let total = out.passed + out.skipped + out.failed.len();
    println!(
        "z80test {variant}: {}/{total} tests pass, {} skipped as z80test intends",
        out.passed, out.skipped
    );
    for f in &out.failed {
        println!("  {f}");
    }
    assert!(total > 0, "{variant} printed no results: {:?}", out.lines);
    assert!(
        out.failed.is_empty() && out.verdict == "Result: all tests passed.",
        "{variant}: {} of {total} tests failed ({:?})",
        out.failed.len(),
        out.verdict
    );
}

#[test]
fn z80full() {
    check("z80full");
}

#[test]
fn z80ccf() {
    check("z80ccf");
}

#[test]
fn z80memptr() {
    check("z80memptr");
}
