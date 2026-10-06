//! Whether the game reads memory the tape did not load (#5).
//!
//! The rewrite starts from the 128K's banks as the game's reader builds them
//! from the tape alone (`robin::assets`). The original starts from a machine
//! that also holds whatever the ROM, BASIC and `r1` left behind: system
//! variables, the cleared screen, `r1` itself, and banks 1 and 3 as they
//! powered on. If the game reads any of that before writing it, the two
//! would start differently.
//!
//! So this runs the original from its hand-over for 20,000 frames under
//! random held keys, as `tests/census.rs` does, and keeps a map of RAM: a
//! byte is *set* if a game block loaded it or the game has written it since.
//! Every read of a byte not yet set fails, by place and by what read it,
//! unless it is one of the known reads (`known`). Reads and writes are taken
//! from the bus model's list of what each instruction puts on the bus
//! (`bus::cycles`), opcode fetches included, since running memory nobody
//! loaded counts too. An interrupt is not an instruction, so its own bus
//! activity is added here: the return address it pushes, and in IM 2 the
//! vector it reads.
//!
//! Needs the supported tape and `128.rom` in `assets/`.

mod common;

use std::collections::BTreeMap;

use robin::layout::{GAME_BLOCKS, LOADER_BLOCKS};
use zx_runtime::memory::{Memory, PAGE};
use zx_runtime::trace::Trace;
use zx_runtime::{Zx, bus, interp};

const FRAMES: u32 = 20_000;

/// The 128K's pages: the two ROMs, then banks 0 to 7.
const PAGES: usize = 10;

/// A place in memory, as a page and an offset, written as the listings name
/// it: `bank:address` at `0xC000`, the plain address in banks 5 and 2.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Place(usize);

impl std::fmt::Debug for Place {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (page, off) = (self.0 / PAGE, self.0 % PAGE);
        match page.checked_sub(2) {
            Some(5) => write!(f, "{:04x}", 0x4000 + off),
            Some(2) => write!(f, "{:04x}", 0x8000 + off),
            Some(bank) => write!(f, "{bank}:{:04x}", 0xC000 + off),
            None => write!(f, "rom{page}:{off:04x}"),
        }
    }
}

/// One past the end of the buffer the routine at `0:CE27` copies from.
const PAST_THE_BUFFER: Place = Place(Memory::bank(0) * PAGE + 0x2A40);

/// The reads of memory the tape did not load that the game is known to make:
/// what a read of `at` by the instruction at `by` is, or `None` if it is not
/// one. Each is harmless only because the original, booted, and the reader's
/// banks hold the same byte there, and the test checks that they do.
fn known(at: Place, by: u16) -> Option<&'static str> {
    match (at, by) {
        // Found by this census; `docs/re/robin.md`, *What it reads that the
        // tape did not load*. The routine copies the non-zero bytes of a
        // 0x240-byte buffer at 0xE800 to 0xE500, and its loop stops only
        // when the count goes below zero, so it reads one byte past the end.
        // Nothing loads or writes that byte; a non-zero one would land at
        // 0xE740.
        (PAST_THE_BUFFER, 0xCE30) => Some("a copy loop reading one byte past its buffer"),
        _ => None,
    }
}

/// Every byte a game block loads, as the facts place it.
fn loaded() -> Vec<bool> {
    let mut set = vec![false; PAGES * PAGE];
    for b in &GAME_BLOCKS {
        for k in 0..b.len {
            let addr = b.at.wrapping_add(k as u16);
            let bank = match addr >> 14 {
                1 => 5,
                2 => 2,
                _ => usize::from(b.port_7ffd & 7),
            };
            set[Memory::bank(bank) * PAGE + usize::from(addr) % PAGE] = true;
        }
    }
    set
}

#[test]
fn the_game_reads_only_what_the_tape_loaded_or_it_wrote() {
    let Some((tape, rom)) = common::tape_and_rom() else {
        return;
    };
    let mut z = common::boot(&tape, &rom);
    // The byte at each place as the original holds it at the hand-over, and
    // as the game's own reader builds it from the tape.
    let booted: Vec<u8> = (0..PAGES).flat_map(|p| z.memory.page(p).to_vec()).collect();
    let read = robin::assets::read_tape(&tape).expect("the game reads its tape");
    let as_read = |at: Place| read.bank(at.0 / PAGE - 2)[at.0 % PAGE];
    let mut set = loaded();
    let loaded_bytes = set.iter().filter(|&&s| s).count();
    assert_eq!(
        loaded_bytes,
        GAME_BLOCKS.iter().map(|b| b.len).sum::<usize>(),
        "the blocks overlap"
    );
    assert_eq!(LOADER_BLOCKS, 4);

    // (place, instruction address) -> how many times.
    let mut unset: BTreeMap<(Place, u16), u64> = BTreeMap::new();
    let mut interrupts = 0u64;
    // What the previous instruction was, and whether interrupts were enabled
    // as it began: if they are disabled now and it was not a DI, an interrupt
    // came in between.
    let mut before: Option<(u8, bool)> = None;

    common::play(&mut z, FRAMES, |z: &mut Zx| {
        let slots = z.memory.slots();
        let place = |addr: u16| Trace::at(&slots, addr);
        if let Some((opcode, iff1)) = before
            && iff1
            && !z.iff1
            && opcode != 0xF3
        {
            interrupts += 1;
            // The return address, pushed where SP now points.
            set[place(z.sp)] = true;
            set[place(z.sp.wrapping_add(1))] = true;
            if z.im == 2 {
                let vector = u16::from(z.i) << 8 | 0xFF;
                for at in [vector, vector.wrapping_add(1)] {
                    if !set[place(at)] {
                        *unset.entry((Place(place(at)), z.pc)).or_default() += 1;
                    }
                }
            }
        }
        let pc = z.pc;
        let d = interp::decode_at(z, pc);
        for c in bus::cycles(z, &d, pc).iter() {
            let at = place(c.at);
            if at < 2 * PAGE {
                continue; // The ROM: `tests/census.rs`'s business.
            }
            match c.kind {
                bus::Kind::Read if !set[at] => {
                    *unset.entry((Place(at), pc)).or_default() += 1;
                }
                bus::Kind::Write => set[at] = true,
                _ => {}
            }
        }
        before = Some((z.read(pc), z.iff1));
    });

    println!(
        "{FRAMES} frames, {interrupts} interrupts; {loaded_bytes} bytes loaded, {} set by the \
         end; {} (place, by) reads of bytes not yet set",
        set[2 * PAGE..].iter().filter(|&&s| s).count(),
        unset.len()
    );
    let mut by_reason: BTreeMap<&str, (usize, u64)> = BTreeMap::new();
    for (&(at, by), &n) in &unset {
        if let Some(why) = known(at, by) {
            let e = by_reason.entry(why).or_default();
            e.0 += 1;
            e.1 += n;
        }
    }
    for (why, (places, n)) in &by_reason {
        println!("  known: {why}: {places} (place, by) pair(s), {n} reads");
    }
    for &(at, _) in unset.keys().filter(|&&(at, by)| known(at, by).is_some()) {
        assert_eq!(
            booted[at.0],
            as_read(at),
            "{at:?}: the booted original and the reader's banks differ, so the rewrite would read \
             something else there"
        );
    }
    let unexpected: Vec<_> = unset
        .iter()
        .filter(|&(&(at, by), _)| known(at, by).is_none())
        .map(|(&(at, by), &n)| format!("{at:?} by {by:04x} ({n}x)"))
        .collect();
    assert!(
        unset.contains_key(&(PAST_THE_BUFFER, 0xCE30)),
        "the known read past the buffer was not seen: is the census watching?"
    );
    assert!(
        interrupts > u64::from(FRAMES) / 2,
        "only {interrupts} interrupts seen: is the census watching?"
    );
    assert!(
        unexpected.is_empty(),
        "the game read {} byte(s) the tape did not load and it had not written: {}",
        unexpected.len(),
        unexpected
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    );
}
