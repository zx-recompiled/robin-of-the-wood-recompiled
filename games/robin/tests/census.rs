//! What the original does with the ROM, over a long run (#3).
//!
//! The rewrite has no ROM, so it must reproduce whatever ROM behaviour the
//! game relies on. #1 saw none in six traced frames, apart from the interrupt
//! reading the ROM's first byte. This runs the original from its hand-over
//! for 20,000 frames under random held keys, through the menu and into play,
//! and records every instruction executed and every byte read below 0x4000.
//! Reads are taken from the bus model's own list of what each instruction
//! puts on the bus (`bus::cycles`, checked against the Fuse corpus), so the
//! interpreter is not slowed by watching. Anything beyond the known uses
//! (`known`) fails, by address and by what reached it.
//!
//! Needs the supported tape and `128.rom` in `assets/`.

mod common;

use std::collections::BTreeMap;

use zx_runtime::{Zx, bus, interp};

const FRAMES: u32 = 20_000;

/// The interrupt's jump, a `JR` at 0xFFFF, reading its displacement from the
/// ROM's first byte (`docs/re/robin.md`, *Interrupts*). (read, by)
const INTERRUPT_READ: (u16, u16) = (0x0000, 0xFFFF);

/// The game's uses of the ROM, and why each is allowed: what a read at `at`
/// by the instruction at `by` is, or `None` if it is not one.
fn known(at: u16, by: u16) -> Option<&'static str> {
    match (at, by) {
        INTERRUPT_READ => Some("the interrupt's jump reading its displacement"),
        // Found by this census; `docs/re/robin.md`, *What it uses from the
        // ROM*. Between the eleven notes of a beeper sound, `LDIR` copies the
        // ROM onto itself, 16K each time: a delay. The bytes go nowhere
        // (writes to ROM are ignored) and the flags they leave are discarded,
        // so only the time it takes matters, never what the ROM holds.
        (0x0000..=0x3FFF, 0x8B46) => Some("a delay copying the ROM onto itself"),
        // Also found by this census, and a real dependency on the ROM's
        // contents (#21): the random-number routine at 0xCD5F reads the byte
        // at R * 0x101 and mixes it into its seed, so when R is below 0x40
        // that byte is the ROM's.
        (at, 0xCD66) if at >> 8 == at & 0xFF => {
            Some("the random-number routine reading the byte at R * 0x101")
        }
        _ => None,
    }
}

#[test]
fn the_game_uses_no_rom_routine() {
    let Some((tape, rom)) = common::tape_and_rom() else {
        return;
    };
    let mut z = common::boot(&tape, &rom);

    // (address, instruction address or ROM page) -> how many times.
    let mut executed: BTreeMap<(u16, usize), u64> = BTreeMap::new();
    let mut reads: BTreeMap<(u16, u16), u64> = BTreeMap::new();
    let mut instructions = 0u64;
    common::play(&mut z, FRAMES, |z: &mut Zx| {
        instructions += 1;
        let pc = z.pc;
        let d = interp::decode_at(z, pc);
        if pc < 0x4000 {
            *executed.entry((pc, z.memory.slot(0))).or_default() += 1;
        }
        // The opcode fetches are the first `m1` cycles; the rest that
        // read are operands and data. An interrupt's own vector read
        // is not an instruction's, so it is not seen here; it is at
        // I * 0x100 + 0xFF, in RAM while I is 0xE2 (*Interrupts*).
        for c in bus::cycles(z, &d, pc).iter().skip(usize::from(d.m1)) {
            if c.kind == bus::Kind::Read && c.at < 0x4000 {
                *reads.entry((c.at, pc)).or_default() += 1;
            }
        }
    });

    println!(
        "census: {FRAMES} frames, {instructions} instructions; {} ROM addresses executed, {} \
         (read, by) pairs below 0x4000",
        executed.len(),
        reads.len()
    );
    let mut by_reason: BTreeMap<&str, (usize, u64)> = BTreeMap::new();
    for (&(at, by), &n) in &reads {
        if let Some(why) = known(at, by) {
            let e = by_reason.entry(why).or_default();
            e.0 += 1;
            e.1 += n;
        }
    }
    for (why, (addresses, n)) in &by_reason {
        println!("  known: {why}: {addresses} address(es), {n} reads");
    }
    let unexpected_reads: Vec<_> = reads
        .keys()
        .filter(|&&(at, by)| known(at, by).is_none())
        .collect();
    assert!(
        executed.is_empty(),
        "the game ran ROM code: {:x?}",
        executed.iter().take(20).collect::<Vec<_>>()
    );
    assert!(
        unexpected_reads.is_empty(),
        "the game read the ROM beyond its known uses, {} (read, by) pairs: {:x?}",
        unexpected_reads.len(),
        unexpected_reads.iter().take(20).collect::<Vec<_>>()
    );
    assert!(
        reads.contains_key(&INTERRUPT_READ),
        "the interrupt's read of 0x0000 was not seen: is the census watching?"
    );
}
