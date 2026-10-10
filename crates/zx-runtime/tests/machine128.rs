//! The 128K machine's own behaviour: paging, the lock, the shadow screen and
//! contention by bank.
//!
//! These are written from the 128K's specification (the World of Spectrum
//! 128K reference, and Sinclair's own technical manual), so they check that
//! the machine does what was meant, not that it matches a real one. That is
//! the job of the hardware-measured timing tests (#12).

use zx_core::{MachineState, Model, bus::Cycle, state::RAM_128};
use zx_runtime::{Zx, memory::Memory};

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

#[test]
fn the_ay_is_selected_written_and_read_through_its_ports() {
    let mut z = machine(0);
    z.ay.log = Some(Vec::new());
    z.port_out(0xFFFD, 8);
    z.t = 1234;
    z.port_out(0xBFFD, 0x3F);
    assert_eq!(z.port_in(0xFFFD), 0x1F, "volume keeps five bits");
    assert_eq!(z.ay.reg(8), 0x1F);
    let log = z.ay.log.as_deref().unwrap_or_default();
    assert_eq!(log.len(), 1);
    assert_eq!((log[0].t, log[0].reg, log[0].value), (1234, 8, 0x3F));
    // Neither port pages memory, and the paging port reaches no register.
    assert_eq!(z.read(0xC000), 0);
    z.port_out(0x7FFD, 0x08);
    assert_eq!(z.ay.reg(8), 0x1F);
}

#[test]
fn a_48k_has_no_ay() {
    let mut z = Zx::new(&blank(), None);
    z.port_out(0xFFFD, 8);
    z.port_out(0xBFFD, 0x0F);
    assert_eq!(z.ay.reg(8), 0);
    assert_eq!(z.port_in(0xFFFD), 0xFF);
}

#[test]
fn a_trace_keeps_the_same_address_in_two_banks_apart() {
    use zx_runtime::trace::Trace;
    let mut z = machine(0);
    z.trace = Some(Box::new(Trace::new(z.memory.pages())));
    // NOP in bank 0 at 0xC000, then INC A in bank 6 at the same address.
    z.memory.poke(0xC000, 0x00);
    z.port_out(0x7FFD, 0x16);
    z.memory.poke(0xC000, 0x3C);
    for port in [0x10, 0x16] {
        z.port_out(0x7FFD, port);
        z.pc = 0xC000;
        zx_runtime::interp::step(&mut z);
    }
    let t = z.trace.as_ref().expect("tracing");
    let at = |bank: usize| (Memory::bank(bank)) * 0x4000;
    assert_eq!(t.executed[at(0)].map(|e| e.1[0]), Some(0x00));
    assert_eq!(t.executed[at(6)].map(|e| e.1[0]), Some(0x3C));
    assert!(!t.self_modified.iter().any(|&b| b));
}

// --- the raster (#15) ---------------------------------------------------------

/// A 128K machine drawing its frame as the beam goes, with every bank 0.
fn rastering() -> Zx {
    let state = MachineState {
        model: Model::Spectrum128,
        ram: vec![0; RAM_128],
        ..blank()
    };
    let mut z = Zx::new(&state, None);
    z.raster = Some(Box::new(zx_runtime::raster::Raster::new(0)));
    z
}

/// The frame finished, as drawn.
fn finished(mut z: Zx) -> Vec<u32> {
    let frame = z.timing.frame;
    z.raster_finish(frame);
    z.raster.expect("on").picture
}

/// The first group of 8 pixels on the border line just above the screen,
/// counted from the one above the first cell, that shows `colour`.
fn first_group(p: &[u32], colour: usize) -> Option<i64> {
    let y = zx_runtime::screen::BORDER - 1;
    (0..zx_runtime::screen::WIDTH / 8)
        .find(|&g| p[y * zx_runtime::screen::WIDTH + g * 8] == zx_core::screen::PALETTE[colour])
        .map(|g| g as i64 - (zx_runtime::screen::BORDER / 8) as i64)
}

#[test]
fn an_out_finishing_at_14365_to_14368_colours_the_border_from_the_first_cell() {
    // The WoS 128K reference, a line (228 T-states) earlier, on the border
    // line above the screen: each group of 8 pixels shows the last colour
    // written by its time.
    for (end, group) in [(14_364, -1), (14_365, 0), (14_368, 0), (14_369, 1)] {
        let mut z = rastering();
        z.t = end - 228;
        z.port_out(0x00FE, 2);
        let p = finished(z);
        assert_eq!(first_group(&p, 2), Some(group), "an OUT finishing at {end}");
    }
}

#[test]
fn a_cell_is_read_when_ramsoft_says_and_shows_what_was_written_by_then() {
    // The first cell's bitmap is read at 14,368: a write that has ended by
    // then shows, one ending after it doesn't.
    for (end, shows) in [(14_368, true), (14_369, false)] {
        let mut z = rastering();
        z.write(0x5800, 0x07);
        z.t = end;
        z.write(0x4000, 0xFF);
        let p = finished(z);
        let y = zx_runtime::screen::BORDER;
        let lit = p[y * zx_runtime::screen::WIDTH + zx_runtime::screen::BORDER];
        assert_eq!(
            lit == zx_core::screen::PALETTE[7],
            shows,
            "a write ending at {end}"
        );
    }
}

#[test]
fn the_shown_bank_is_the_one_paged_when_a_cell_is_read() {
    // Bank 7's first cell is lit; bank 5's isn't. Showing bank 7 from
    // 14,369 reaches the second cell (read at 14,370) but not the first.
    let mut z = rastering();
    z.port_out(0x7FFD, 0x07);
    for at in [0xC000u16, 0xC001] {
        z.write(at, 0xFF);
    }
    z.write(0xD800, 0x07);
    z.write(0xD801, 0x07);
    z.port_out(0x7FFD, 0x00);
    z.t = 14_369;
    z.port_out(0x7FFD, 0x08);
    let p = finished(z);
    let y = zx_runtime::screen::BORDER;
    let at = |cell: usize| p[y * zx_runtime::screen::WIDTH + zx_runtime::screen::BORDER + cell * 8];
    assert_eq!(
        at(0),
        zx_core::screen::PALETTE[0],
        "the first cell from bank 5"
    );
    assert_eq!(at(1), zx_core::screen::PALETTE[7], "the second from bank 7");
}
