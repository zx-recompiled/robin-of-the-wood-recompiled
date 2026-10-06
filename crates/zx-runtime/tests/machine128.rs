//! The 128K machine's own behaviour: paging, the lock, the shadow screen and
//! contention by bank.
//!
//! These are written from the 128K's specification (the World of Spectrum
//! 128K reference, and Sinclair's own technical manual), so they check that
//! the machine does what was meant, not that it matches a real one. That is
//! `#12`'s hardware-measured timing tests' job.

use zx_core::{MachineState, Model, bus::Cycle, state::RAM_128};
use zx_runtime::Zx;

/// A 128K machine whose every bank is filled with its own number, and whose
/// ROMs are `0xA0` and `0xA1` throughout.
fn machine(port_7ffd: u8) -> Zx {
    let mut ram = vec![0u8; RAM_128];
    for (bank, chunk) in ram.chunks_mut(0x4000).enumerate() {
        chunk.fill(bank as u8);
    }
    let mut rom = vec![0xA0u8; 0x4000];
    rom.extend(vec![0xA1u8; 0x4000]);
    let state = MachineState {
        model: Model::Spectrum128,
        port_7ffd,
        ram,
        ..blank()
    };
    Zx::new(&state, Some(&rom))
}

fn blank() -> MachineState {
    MachineState {
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
        model: Model::Spectrum48,
        port_7ffd: 0,
        ram: vec![0; 0xC000],
    }
}

#[test]
fn bank_5_and_2_never_move_and_the_top_slot_follows_the_port() {
    let mut z = machine(0);
    assert_eq!(z.read(0x4000), 5);
    assert_eq!(z.read(0x8000), 2);
    for bank in 0..8 {
        z.port_out(0x7FFD, bank);
        assert_eq!(z.read(0xC000), bank, "bank {bank} at 0xC000");
        assert_eq!(z.read(0x4000), 5);
        assert_eq!(z.read(0x8000), 2);
    }
}

#[test]
fn bit_4_chooses_the_rom_and_rom_ignores_writes() {
    let mut z = machine(0x00);
    assert_eq!(z.read(0x0000), 0xA0);
    z.port_out(0x7FFD, 0x10);
    assert_eq!(z.read(0x0000), 0xA1);
    z.write(0x0000, 0x55);
    assert_eq!(z.read(0x0000), 0xA1);
    z.port_out(0x7FFD, 0x00);
    assert_eq!(z.read(0x0000), 0xA0);
}

#[test]
fn a_bank_is_the_same_memory_wherever_it_appears() {
    let mut z = machine(5);
    z.write(0xC123, 0x42);
    assert_eq!(z.read(0x4123), 0x42, "bank 5 at 0xC000 is bank 5 at 0x4000");
    z.port_out(0x7FFD, 2);
    z.write(0xC456, 0x24);
    assert_eq!(z.read(0x8456), 0x24, "bank 2 at 0xC000 is bank 2 at 0x8000");
    // Paged out and back in, a bank keeps what was written to it.
    z.port_out(0x7FFD, 3);
    z.write(0xC000, 0x99);
    z.port_out(0x7FFD, 4);
    assert_eq!(z.read(0xC000), 4);
    z.port_out(0x7FFD, 3);
    assert_eq!(z.read(0xC000), 0x99);
}

#[test]
fn bit_5_locks_paging_until_reset() {
    let mut z = machine(0);
    z.port_out(0x7FFD, 0x20 | 3);
    assert_eq!(z.read(0xC000), 3);
    z.port_out(0x7FFD, 6);
    assert_eq!(z.read(0xC000), 3, "locked");
    assert_eq!(z.port_7ffd, 0x23);
}

#[test]
fn the_port_is_decoded_from_a15_and_a1_only() {
    let mut z = machine(0);
    // A15 and A1 low: pages, whatever the other lines.
    z.port_out(0x3FFD, 1);
    assert_eq!(z.read(0xC000), 1);
    z.port_out(0x0001, 2);
    assert_eq!(z.read(0xC000), 2);
    // A15 high (the AY's ports) or A1 high: does not.
    z.port_out(0xFFFD, 3);
    z.port_out(0xBFFD, 4);
    z.port_out(0x7FFF, 5);
    assert_eq!(z.read(0xC000), 2);
}

#[test]
fn a_48k_machine_ignores_the_paging_port() {
    let mut z = Zx::new(&blank(), None);
    z.write(0xC000, 0x77);
    z.port_out(0x7FFD, 0x07);
    assert_eq!(z.read(0xC000), 0x77);
    assert_eq!(z.port_7ffd, 0);
}

#[test]
fn bit_3_shows_bank_7_as_the_screen() {
    let mut z = machine(0);
    assert_eq!(z.memory.page(z.screen_page())[0], 5);
    z.port_out(0x7FFD, 0x08);
    assert_eq!(z.memory.page(z.screen_page())[0], 7);
    // Which bank is shown has nothing to do with which is paged in.
    assert_eq!(z.read(0xC000), 0);
}

#[test]
fn the_odd_banks_are_contended_wherever_they_are() {
    let mut z = machine(0);
    assert!(!z.contended(0x0000));
    assert!(z.contended(0x4000), "bank 5");
    assert!(z.contended(0x7FFF));
    assert!(!z.contended(0x8000), "bank 2");
    for bank in 0..8u8 {
        z.port_out(0x7FFD, bank);
        assert_eq!(z.contended(0xC000), bank % 2 == 1, "bank {bank} at 0xC000");
    }
}

#[test]
fn a_contended_bank_at_0xc000_costs_what_the_ula_charges() {
    let mut z = machine(1);
    let first = z.timing.first_contended;
    // At the first contended T-state the ULA holds the processor for 6.
    z.t = first;
    z.charge(Cycle::read(0xC000));
    assert_eq!(z.t, first + 6 + 3);
    // The same read from an even bank is never held.
    z.port_out(0x7FFD, 0);
    z.t = first;
    z.charge(Cycle::read(0xC000));
    assert_eq!(z.t, first + 3);
}

#[test]
fn the_machine_has_the_128ks_timing() {
    let z = machine(0);
    assert_eq!(z.timing, zx_core::timing::SPECTRUM_128);
    assert_eq!(Zx::new(&blank(), None).timing, zx_core::timing::SPECTRUM_48);
}
