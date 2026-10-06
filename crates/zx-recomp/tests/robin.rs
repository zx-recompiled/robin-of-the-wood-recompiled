//! `tools/re/robin.toml` traces the original from its hand-over and finds the
//! code the notes already name (#4). Needs the tape and `128.rom` in
//! `assets/`; without them it says it was skipped.

use std::path::PathBuf;

use zx_recomp::analysis::{Place, analyze};
use zx_recomp::{Config, Inputs, listing::listing, tracer};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn the_trace_finds_the_code_the_notes_name() {
    let text = std::fs::read_to_string(root().join("tools/re/robin.toml")).expect("the config");
    let cfg = Config::parse(&text).expect("parses");
    let inputs = match Inputs::load(&cfg, &root().join("assets")) {
        Ok(i) => i,
        Err(e) => {
            println!("skipped: {e}");
            return;
        }
    };
    let traced = tracer::run(&cfg.trace, &inputs, |_, _| {}).expect("traces");
    let a = analyze(&cfg, &inputs.machine, &traced.trace, &[]);
    let fixed = |addr: u16| a.layout.fixed_place(addr).expect("a fixed page");
    let bank0 = |addr: u16| a.layout.banked(addr, 0);
    for (what, at) in [
        ("the program's start", fixed(0xBE4A)),
        ("the banked-call routine", fixed(0x5B8A)),
        ("the beeper delay", fixed(0x8B32)),
        ("the random-number routine", bank0(0xCD5F)),
    ] {
        let found: Place = at;
        assert!(
            a.code_starts[found.index()],
            "{what} at {found} was not found"
        );
        println!("found {what} at {found}");
    }
    assert!(
        !a.code_starts[..0x8000].iter().any(|&c| c),
        "code was found in the ROM, which the game never runs (games/robin/tests/census.rs)"
    );
    let text = listing(&a, &traced.trace);
    for section in [
        "bank 5, at 4000",
        "bank 2, at 8000",
        "bank 0, at c000",
        "bank 6, at c000",
    ] {
        assert!(text.contains(section), "no section for {section}");
    }
    print!("{}", zx_recomp::report(&a));
}
