//! Checks the reference interpreter against an independent description of
//! what a Z80 does.
//!
//! Every other check in this project compares the rewritten game against this
//! interpreter, and the rewrite was built by checking against it. So a wrong
//! opcode here would be copied into the rewrite and every suite would still
//! pass. This test and `tests/z80test.rs` are the ones that do not rest on our
//! own work. This one runs the Z80 test corpus written for the Fuse emulator,
//! which states for 1335 cases what the registers, memory and T-state count
//! should be afterwards. It is Fuse's model rather than measurements, so where
//! z80test, which is measured, shows it wrong, the case is listed in
//! [`KNOWN_WRONG`]; it is kept for timing, which z80test does not check.
//!
//! The corpus is not in this repository, for the same reason the game and the
//! ROM are not. See `assets/README.md` for where to get it; without it this
//! test says so and passes, and the count it prints makes a vacuous run
//! obvious.

use std::path::PathBuf;

use zx_core::MachineState;
use zx_runtime::{Zx, bus, interp};

/// Everything the corpus states about the processor at one moment.
#[derive(Clone, PartialEq, Eq)]
struct State {
    /// AF BC DE HL AF' BC' DE' HL' IX IY SP PC, in that order.
    regs: [u16; 12],
    i: u8,
    r: u8,
    iff1: bool,
    iff2: bool,
    im: u8,
    halted: bool,
    /// T-states: to run for in `tests.in`, reached in `tests.expected`.
    t: u32,
}

const NAMES: [&str; 12] = [
    "AF", "BC", "DE", "HL", "AF'", "BC'", "DE'", "HL'", "IX", "IY", "SP", "PC",
];

struct Case {
    name: String,
    state: State,
    /// Memory to place before the test, and after it in the expected file.
    mem: Vec<(u16, Vec<u8>)>,
    /// When the instruction put an address on the memory bus, and which:
    /// the corpus's `MC` events. Only in the expected file.
    ///
    /// The reads and writes are deliberately left out. What they carry is
    /// already settled by comparing memory and registers, and the corpus
    /// records them as the Fuse interpreter happens to do them — a JR that
    /// is not taken contends for its displacement without ever reading it,
    /// for instance. The contention events are the processor's, and they are
    /// the ones that decide what the ULA charges.
    events: Vec<(u32, String, u16)>,
}

/// Cases where the corpus is wrong about a real Z80, each with the evidence.
///
/// The corpus describes Fuse's model of the processor, not measurements.
/// Where z80test (`tests/z80test.rs`), which is measured on a real Spectrum,
/// shows the model wrong, the case is listed here rather than its expected
/// values edited. The test checks that exactly these fail, so a new failure
/// cannot hide among them.
const KNOWN_WRONG: &[(&str, &str)] = &[
    (
        "37_1",
        "SCF: bits 3 and 5 from A alone. From a fresh processor (Q = 0) a Zilog \
         Z80 takes them from F | A (z80test z80ccf, and z80full test 001)",
    ),
    (
        "3f",
        "CCF: bits 3 and 5 from A alone. From a fresh processor (Q = 0) a Zilog \
         Z80 takes them from F | A (z80test z80ccf, and z80full test 002)",
    ),
    (
        "cb4e",
        "BIT 1,(HL): bits 3 and 5 from the byte tested. A Z80 takes them from \
         MEMPTR's high byte, 0 here (z80test z80memptr, and z80full test 071)",
    ),
    (
        "cb5e",
        "BIT 3,(HL): bits 3 and 5 from the byte tested. A Z80 takes them from \
         MEMPTR's high byte, 0 here (z80test z80memptr, and z80full test 071)",
    ),
    (
        "cb6e",
        "BIT 5,(HL): bits 3 and 5 from the byte tested. A Z80 takes them from \
         MEMPTR's high byte, 0 here (z80test z80memptr, and z80full test 071)",
    ),
    (
        "cb76",
        "BIT 6,(HL): bits 3 and 5 from the byte tested. A Z80 takes them from \
         MEMPTR's high byte, 0 here (z80test z80memptr, and z80full test 071)",
    ),
];

/// What the Fuse test harness returns for a port read: the port's high byte.
/// The real ULA is the machine's, not the processor's.
fn port_in(port: u16) -> u8 {
    (port >> 8) as u8
}

fn hex16(s: &str) -> u16 {
    u16::from_str_radix(s, 16).unwrap_or_else(|_| panic!("not a hex word: {s:?}"))
}

/// Reads the `<address> <byte>... -1` lines that end a test, stopping at the
/// terminator `end` (a lone `-1` in `tests.in`, a blank line in the expected).
fn read_mem<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    blank_ends: bool,
) -> Vec<(u16, Vec<u8>)> {
    let mut mem = Vec::new();
    for line in lines {
        let line = line.trim();
        if line == "-1" || (blank_ends && line.is_empty()) {
            break;
        }
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let at = hex16(parts.next().expect("memory line address"));
        let bytes = parts
            .take_while(|p| *p != "-1")
            .map(|p| u8::from_str_radix(p, 16).expect("memory byte"))
            .collect();
        mem.push((at, bytes));
    }
    mem
}

fn read_state<'a>(lines: &mut impl Iterator<Item = &'a str>, regs_line: &str) -> State {
    let regs: Vec<u16> = regs_line.split_whitespace().map(hex16).collect();
    assert_eq!(regs.len(), 12, "expected 12 registers, got {regs_line:?}");
    let rest = lines.next().expect("state line");
    let f: Vec<&str> = rest.split_whitespace().collect();
    assert_eq!(f.len(), 7, "expected 7 state fields, got {rest:?}");
    State {
        regs: regs.try_into().expect("12 registers"),
        i: u8::from_str_radix(f[0], 16).expect("I"),
        r: u8::from_str_radix(f[1], 16).expect("R"),
        iff1: f[2] != "0",
        iff2: f[3] != "0",
        im: f[4].parse().expect("IM"),
        halted: f[5] != "0",
        t: f[6].parse().expect("tstates"),
    }
}

fn parse_in(text: &str) -> Vec<Case> {
    let mut lines = text.lines().peekable();
    let mut out = Vec::new();
    loop {
        while lines.peek().is_some_and(|l| l.trim().is_empty()) {
            lines.next();
        }
        let Some(name) = lines.next() else { break };
        let regs_line = lines.next().expect("register line").to_string();
        let state = read_state(&mut lines, &regs_line);
        out.push(Case {
            name: name.trim().to_string(),
            state,
            mem: read_mem(&mut lines, false),
            events: Vec::new(),
        });
    }
    out
}

fn parse_expected(text: &str) -> Vec<Case> {
    let mut lines = text.lines().peekable();
    let mut out = Vec::new();
    loop {
        while lines.peek().is_some_and(|l| l.trim().is_empty()) {
            lines.next();
        }
        let Some(name) = lines.next() else { break };
        // Then the bus events, which we do not model: they are the ULA's
        // contention pattern, cycle by cycle, and this interpreter accounts
        // for an instruction all at once. An event line is recognised by its
        // second field naming an event type. It has to be that and not "the
        // field is letters", because a hex word can be all letters too:
        // `0000 ffff ...` is a register line whose second word is not hex to
        // look at.
        let mut events = Vec::new();
        let regs_line = loop {
            let line = lines.next().expect("register line");
            let f: Vec<&str> = line.split_whitespace().collect();
            let is_event = f
                .get(1)
                .is_some_and(|s| matches!(*s, "MR" | "MW" | "MC" | "PR" | "PW" | "PC" | "PB"));
            if !is_event {
                break line.to_string();
            }
            if f[1] == "MC" {
                events.push((
                    f[0].parse().expect("event time"),
                    f[1].to_string(),
                    hex16(f[2]),
                ));
            }
        };
        let state = read_state(&mut lines, &regs_line);
        out.push(Case {
            name: name.trim().to_string(),
            state,
            mem: read_mem(&mut lines, true),
            events,
        });
    }
    out
}

fn blank_machine() -> Zx {
    let state = MachineState {
        a: 0,
        f: 0,
        b: 0,
        c: 0,
        d: 0,
        e: 0,
        h: 0,
        l: 0,
        a_: 0,
        f_: 0,
        b_: 0,
        c_: 0,
        d_: 0,
        e_: 0,
        h_: 0,
        l_: 0,
        ix: 0,
        iy: 0,
        sp: 0,
        pc: 0,
        i: 0,
        r: 0,
        iff1: false,
        iff2: false,
        im: 0,
        border: 0,
        model: zx_core::Model::Spectrum48,
        port_7ffd: 0,
        ram: vec![0; 0xC000],
    };
    let mut z = Zx::new(&state, None);
    z.port_in_hook = Some(port_in);
    z
}

fn load(z: &mut Zx, case: &Case) {
    let s = &case.state;
    let [af, bc, de, hl, af_, bc_, de_, hl_, ix, iy, sp, pc] = s.regs;
    z.set_af(af);
    z.set_bc(bc);
    z.set_de(de);
    z.set_hl(hl);
    z.ex_af();
    z.exx();
    z.set_af(af_);
    z.set_bc(bc_);
    z.set_de(de_);
    z.set_hl(hl_);
    z.ex_af();
    z.exx();
    z.ix = ix;
    z.iy = iy;
    z.sp = sp;
    z.pc = pc;
    z.i = s.i;
    z.r = s.r;
    z.iff1 = s.iff1;
    z.iff2 = s.iff2;
    z.im = s.im;
    z.halted = s.halted;
    z.t = 0;
    for (at, bytes) in &case.mem {
        for (i, &b) in bytes.iter().enumerate() {
            z.memory.poke(at.wrapping_add(i as u16), b);
        }
    }
}

fn actual(z: &Zx) -> State {
    let mut alt = z.clone();
    alt.ex_af();
    alt.exx();
    State {
        regs: [
            z.af(),
            z.bc(),
            z.de(),
            z.hl(),
            alt.af(),
            alt.bc(),
            alt.de(),
            alt.hl(),
            z.ix,
            z.iy,
            z.sp,
            z.pc,
        ],
        i: z.i,
        r: z.r,
        iff1: z.iff1,
        iff2: z.iff2,
        im: z.im,
        halted: z.halted,
        t: z.t,
    }
}

/// The memory bus events the model says an instruction produces, in the
/// corpus's own form: a contention event when the address goes on the bus,
/// and a read or write event when the transfer finishes.
fn model_events(z: &Zx, pc: u16, start: u32) -> Vec<(u32, String, u16)> {
    let d = interp::decode_at(z, pc);
    let mut t = start;
    let mut out = Vec::new();
    for &c in bus::cycles(z, &d, pc).iter() {
        // A port cycle contends on the I/O pattern, not the memory one, and
        // the corpus's PC/PR/PW events are filtered out to match.
        if !matches!(c.kind, bus::Kind::PortRead | bus::Kind::PortWrite) {
            out.push((t, "MC".to_string(), c.at));
        }
        t += c.len;
    }
    out
}

/// What differs between the corpus and us, in words.
fn differences(want: &State, got: &State) -> Vec<String> {
    let mut out = Vec::new();
    for (i, name) in NAMES.iter().enumerate() {
        if want.regs[i] != got.regs[i] {
            out.push(format!(
                "{name} want {:04x} got {:04x}",
                want.regs[i], got.regs[i]
            ));
        }
    }
    let mut byte = |name: &str, w: u8, g: u8| {
        if w != g {
            out.push(format!("{name} want {w:02x} got {g:02x}"));
        }
    };
    byte("I", want.i, got.i);
    byte("R", want.r, got.r);
    byte("IM", want.im, got.im);
    for (name, w, g) in [
        ("IFF1", want.iff1, got.iff1),
        ("IFF2", want.iff2, got.iff2),
        ("halted", want.halted, got.halted),
    ] {
        if w != g {
            out.push(format!("{name} want {w} got {g}"));
        }
    }
    if want.t != got.t {
        out.push(format!("T-states want {} got {}", want.t, got.t));
    }
    out
}

/// Loads the corpus, or says why it cannot and leaves the test to pass.
fn corpus() -> Option<(Vec<Case>, Vec<Case>)> {
    let dir = std::env::var_os("FUSE_TESTS").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets"),
        PathBuf::from,
    );
    let (input, expected) = (dir.join("tests.in"), dir.join("tests.expected"));
    let (Ok(input), Ok(expected)) = (
        std::fs::read_to_string(&input),
        std::fs::read_to_string(&expected),
    ) else {
        println!(
            "skipped: no Z80 test corpus in {}; see assets/README.md for where to get \
             tests.in and tests.expected",
            dir.display()
        );
        return None;
    };
    let cases = parse_in(&input);
    let expect = parse_expected(&expected);
    assert!(!cases.is_empty(), "the corpus parsed to no tests");
    assert_eq!(
        cases.len(),
        expect.len(),
        "the two corpus files disagree on how many tests there are"
    );
    for (a, b) in cases.iter().zip(&expect) {
        assert_eq!(a.name, b.name, "the corpus files are out of step");
    }
    Some((cases, expect))
}

#[test]
fn matches_the_z80_test_corpus() {
    let Some((cases, expect)) = corpus() else {
        return;
    };

    let mut failures: Vec<String> = Vec::new();
    for (case, want) in cases.iter().zip(&expect) {
        let mut z = blank_machine();
        load(&mut z, case);
        // The corpus says how long to run for, and lets the last instruction
        // finish. A halted processor runs NOPs.
        let until = case.state.t;
        let mut before = z.clone();
        while z.t < until {
            if z.halted {
                z.step(4, 1);
            } else {
                interp::step(&mut z);
            }
        }

        let mut diffs = differences(&want.state, &actual(&z));

        // The expected memory is only what changed, so compare all of it:
        // that catches a write we should not have made as well as one we
        // should have.
        for (at, bytes) in &want.mem {
            for (i, &b) in bytes.iter().enumerate() {
                before.memory.poke(at.wrapping_add(i as u16), b);
            }
        }
        let wrong: Vec<usize> = (0..0x10000)
            .filter(|&a| before.read(a as u16) != z.read(a as u16))
            .collect();
        for &a in wrong.iter().take(4) {
            diffs.push(format!(
                "[{a:04x}] want {:02x} got {:02x}",
                before.read(a as u16),
                z.read(a as u16)
            ));
        }
        if wrong.len() > 4 {
            diffs.push(format!("and {} more bytes of memory", wrong.len() - 4));
        }

        if !diffs.is_empty() {
            failures.push(format!("{}: {}", case.name, diffs.join(", ")));
        }
    }

    let passed = cases.len() - failures.len();
    let why = |f: &String| {
        KNOWN_WRONG
            .iter()
            .find(|(name, _)| f.split(':').next() == Some(*name))
            .map(|(_, why)| *why)
    };
    let is_known = |f: &String| why(f).is_some();
    let (known, unexpected): (Vec<String>, Vec<String>) = failures.into_iter().partition(is_known);
    println!(
        "Z80 corpus: {passed}/{} cases match; {} more are known to be wrong in the corpus",
        cases.len(),
        known.len()
    );
    for f in &known {
        println!("  known wrong, {f}");
        println!("    {}", why(f).unwrap_or_default());
    }
    // Listed but now matching: the list is stale, or the interpreter has gone
    // back to the corpus's model.
    let stale: Vec<&str> = KNOWN_WRONG
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !known.iter().any(|f| f.split(':').next() == Some(*name)))
        .collect();
    if !unexpected.is_empty() || !stale.is_empty() {
        for f in unexpected.iter().take(40) {
            println!("  {f}");
        }
        if unexpected.len() > 40 {
            println!("  ... and {} more", unexpected.len() - 40);
        }
        panic!(
            "{} of {} Z80 conformance cases differ; listed as known wrong but matching: {stale:?}",
            unexpected.len(),
            cases.len()
        );
    }
}

/// The same corpus, checked against the bus model rather than the results.
///
/// [`matches_the_z80_test_corpus`] proves the interpreter computes the right
/// answer. This proves it reaches for the right bytes at the right moment,
/// which is what decides how much the ULA charges it — see `bus.rs`.
#[test]
fn bus_activity_matches_the_z80_test_corpus() {
    let Some((cases, expect)) = corpus() else {
        return;
    };

    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for (case, want) in cases.iter().zip(&expect) {
        let mut z = blank_machine();
        load(&mut z, case);
        // One instruction: the corpus's events are of the whole run, and the
        // cases that run more than one are compared as far as the first goes.
        let d = interp::decode_at(&z, z.pc);
        let got = model_events(&z, z.pc, 0);
        let want_events: Vec<_> = want.events.iter().take(got.len()).cloned().collect();
        checked += 1;

        // The cycles have to add up to what the decoder charges, or the model
        // cannot be used as the timing: one is the other, split into pieces.
        let total: u32 = bus::cycles(&z, &d, z.pc).iter().map(|c| c.len).sum();
        let (short, long) = (u32::from(d.t), u32::from(d.t) + u32::from(d.t_extra));
        if total != short && total != long {
            failures.push(format!(
                "{}: cycles add to {total}, decoder says {short} or {long}",
                case.name
            ));
            continue;
        }
        if got != want_events {
            let show = |v: &[(u32, String, u16)]| {
                v.iter()
                    .map(|(t, k, a)| format!("{t} {k} {a:04x}"))
                    .collect::<Vec<_>>()
                    .join(" | ")
            };
            failures.push(format!(
                "{}:\n    want {}\n    got  {}",
                case.name,
                show(&want_events),
                show(&got)
            ));
        }
    }

    println!(
        "Z80 bus model: {}/{checked} cases match",
        checked - failures.len()
    );
    if !failures.is_empty() {
        for f in failures.iter().take(25) {
            println!("  {f}");
        }
        if failures.len() > 25 {
            println!("  ... and {} more", failures.len() - 25);
        }
        panic!("{} of {checked} bus-activity cases differ", failures.len());
    }
}
