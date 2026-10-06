//! The original, loaded from its tape in the 128K reference machine through
//! the real ROM and its own loader, at the moment it hands over to itself:
//! the state every check starts from (#3).
//!
//! They need the supported tape and `128.rom` in `assets/`, so they run
//! locally; without them each says it was skipped.

use std::path::PathBuf;

use robin::layout::{
    ENTRY_7FFD, ENTRY_IM, ENTRY_PC, ENTRY_SP, GAME_BLOCKS, LOADER_BLOCKS, START, banks_from_tape,
};
use zx_runtime::{Zx, loader::boot_128k, memory::Memory};

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// The tape's blocks and the machine booted from them to the hand-over, or
/// `None` (and a note) without the tape or the ROM.
fn booted() -> Option<(Vec<Vec<u8>>, Zx)> {
    let tape = std::fs::read_dir(assets())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::read(e.path()).ok())
        .find(|b| robin::is_the_tape(b));
    let rom = std::fs::read(assets().join("128.rom")).ok();
    let (Some(tape), Some(rom)) = (tape, rom) else {
        println!("skipped: needs the supported tape and 128.rom in assets/");
        return None;
    };
    let blocks = zx_core::tape::load_tzx(&tape).expect("the tape reads");
    let z = boot_128k(&rom, blocks.clone(), ENTRY_PC, 2000).expect("the original boots");
    Some((blocks, z))
}

#[test]
fn the_hand_over_is_as_the_facts_say() {
    let Some((_, z)) = booted() else {
        return;
    };
    assert_eq!(z.pc, ENTRY_PC);
    assert_eq!(z.sp, ENTRY_SP, "r1's own stack");
    assert_eq!(z.port_7ffd, ENTRY_7FFD);
    assert_eq!(z.im, ENTRY_IM);
    assert!(!z.iff1 && !z.iff2, "interrupts disabled");
}

#[test]
fn the_stub_at_the_hand_over_goes_to_the_programs_start() {
    let Some((_, mut z)) = booted() else {
        return;
    };
    assert!(
        z.run_until(START, 1),
        "from {ENTRY_PC:#06x} to {START:#06x}"
    );
    assert_eq!(z.port_7ffd, ENTRY_7FFD);
}

/// Each of the game's blocks is where the facts say, byte for byte, and the
/// banks built from the tape and the facts alone, as the game will build
/// them, match the booted machine there.
#[test]
fn the_blocks_are_where_the_facts_say() {
    let Some((blocks, z)) = booted() else {
        return;
    };
    let facts = banks_from_tape(&blocks).expect("the tape fits the facts");
    let mut checked = 0usize;
    for (i, b) in GAME_BLOCKS.iter().enumerate() {
        let data = &blocks[LOADER_BLOCKS + i];
        let data = &data[1..data.len() - 1];
        for (k, &v) in data.iter().enumerate() {
            let addr = b.at.wrapping_add(k as u16);
            let bank = match addr >> 14 {
                1 => 5,
                2 => 2,
                _ => usize::from(b.port_7ffd & 7),
            };
            let off = usize::from(addr & 0x3FFF);
            let booted = z.memory.page(Memory::bank(bank))[off];
            assert_eq!(
                booted,
                v,
                "block {}, byte {k}: bank {bank} {off:#06x}",
                LOADER_BLOCKS + i
            );
            assert_eq!(
                facts[bank * 0x4000 + off],
                booted,
                "facts-built bank {bank} {off:#06x}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, GAME_BLOCKS.iter().map(|b| b.len).sum::<usize>());
    println!("{checked} bytes of the game's four blocks where the facts say");
}
