//! The analysis and listing on a 128K, where the same address in two banks
//! is two pieces of code (#4). A small program is laid out by hand; nothing
//! here needs a ROM or a tape.

use zx_core::{MachineState, Model, state::RAM_128};
use zx_recomp::Config;
use zx_recomp::analysis::{Place, analyze};
use zx_recomp::listing::listing;
use zx_runtime::Zx;
use zx_runtime::memory::Memory;
use zx_runtime::trace::Trace;

/// A 128K machine with bank 0 paged, and in its memory:
/// - bank 2, 0x8000: a banked call to 0xC010 in bank 6, then `JP 0xC000`;
/// - bank 2, 0x8100: the banked-call routine (just `RET` here);
/// - bank 0, 0xC000: `NOP`, `JP 0x8000`;
/// - bank 6, 0xC000: `LD A,0x55; RET`, which nothing reaches;
/// - bank 6, 0xC010: `INC A; RET`.
fn machine() -> Zx {
    let mut ram = vec![0u8; RAM_128];
    let mut put = |bank: usize, at: u16, bytes: &[u8]| {
        let off = bank * 0x4000 + usize::from(at) % 0x4000;
        ram[off..off + bytes.len()].copy_from_slice(bytes);
    };
    put(
        2,
        0x8000,
        &[0xCD, 0x00, 0x81, 0x10, 0xC0, 0x16, 0xC3, 0x00, 0xC0],
    );
    put(2, 0x8100, &[0xC9]);
    put(0, 0xC000, &[0x00, 0xC3, 0x00, 0x80]);
    put(6, 0xC000, &[0x3E, 0x55, 0xC9]);
    put(6, 0xC010, &[0x3C, 0xC9]);
    let tape = zx_core::tape::Tape {
        ram: vec![0; 0xC000],
        loading_screen: None,
    };
    let state = MachineState {
        model: Model::Spectrum128,
        port_7ffd: 0x10,
        ram,
        ..MachineState::from_tape(&tape, 0x8000, 0x7F00)
    };
    Zx::new(&state, None)
}

/// A trace that saw bank 0's 0xC000 run, and nothing else.
fn trace(z: &Zx) -> Trace {
    let mut t = Trace::new(z.memory.pages());
    t.executed[Memory::bank(0) * 0x4000] = Some((1, [0, 0, 0, 0]));
    t
}

fn config() -> Config {
    Config::parse(
        "[game]\nname = \"t\"\nmachine = \"128k\"\ntape = \"t.tzx\"\nboot_until = 0x8000\n\
         [analysis]\nbanked_calls = [0x8100]\n",
    )
    .expect("parses")
}

fn banked(bank: u8, addr: u16) -> Place {
    Place {
        page: 2 + usize::from(bank),
        addr,
        bank: Some(bank),
    }
}

#[test]
fn code_is_found_in_the_right_bank() {
    let z = machine();
    let a = analyze(&config(), &z, &trace(&z), &[]);
    // Through the banked-call hint, into bank 6.
    assert!(
        a.entries.contains(&banked(6, 0xC010)),
        "the banked call's target"
    );
    assert!(a.call_targets.contains(&banked(6, 0xC010)));
    // A jump from bank 2 into 0xC000: where the trace saw it run, bank 0.
    assert!(a.entries.contains(&banked(0, 0xC000)), "the traced bank");
    // The same address in bank 6 is other code that nothing reaches.
    assert!(!a.entries.contains(&banked(6, 0xC000)));
    assert!(!a.code_starts[banked(6, 0xC000).index()]);
    // And from bank 0 back to bank 2, a fixed page.
    let routine = Place {
        page: Memory::bank(2),
        addr: 0x8100,
        bank: None,
    };
    assert!(a.call_targets.contains(&routine));
    assert_eq!(banked(6, 0xC010).to_string(), "6:c010");
    assert_eq!(routine.to_string(), "8100");
}

#[test]
fn the_listing_has_a_section_per_bank_and_names_places_by_bank() {
    let z = machine();
    let t = trace(&z);
    let a = analyze(&config(), &z, &t, &[]);
    let text = listing(&a, &t);
    for want in [
        "; ==== bank 2, at 8000 ====",
        "; ==== bank 0, at c000 ====",
        "; ==== bank 6, at c000 ====",
        "sub_6:c010:",
        "l_0:c000:",
        "0:c000 * 00",
    ] {
        assert!(text.contains(want), "wanted {want:?} in\n{text}");
    }
    // Bank 6's 0xC000 is never taken for the code that runs in bank 0.
    assert!(!text.contains("ld a,$55"), "in\n{text}");
    // The fixed pages come first, then the paged banks.
    let at = |s: &str| text.find(s).expect(s);
    assert!(at("bank 2, at 8000") < at("bank 0, at c000"));
    assert!(at("bank 0, at c000") < at("bank 6, at c000"));
}
