//! The tape feeder's two ways into `LD-BYTES`, on a machine set up as each
//! caller leaves it. No ROM is needed: the feeder stands in for the routine.

use zx_core::{MachineState, Model, state::RAM_128};
use zx_runtime::{
    CF, Zx,
    loader::{LD_BYTES, LD_BYTES_PAST_PREAMBLE, SA_LD_RET, TapeFeeder},
    memory::Memory,
};

/// A tape block: the flag, the data, and the checksum the ROM checks.
fn block(flag: u8, data: &[u8]) -> Vec<u8> {
    let mut b = vec![flag];
    b.extend_from_slice(data);
    b.push(b.iter().fold(0, |a, x| a ^ x));
    b
}

/// A 128K machine with ROM 1 (BASIC) paged and bank `bank` at 0xC000.
fn machine(bank: u8) -> Zx {
    let tape = zx_core::tape::Tape {
        ram: vec![0; 0xC000],
        loading_screen: None,
    };
    let state = MachineState {
        model: Model::Spectrum128,
        port_7ffd: 0x10 | bank,
        ram: vec![0; RAM_128],
        sp: 0x8000,
        ..MachineState::from_tape(&tape, 0, 0x8000)
    };
    Zx::new(&state, None)
}

/// Sets the processor up as a caller leaves it at a door: `flag` and
/// carry (load) where that door expects them, the length and address, and
/// `ret` as the address to come back to.
fn at_door(z: &mut Zx, door: u16, flag: u8, at: u16, len: u16, ret: u16) {
    z.push(ret);
    z.pc = door;
    z.ix = at;
    z.set_de(len);
    if door == LD_BYTES {
        z.a = flag;
        z.f = CF;
    } else {
        // The caller did `EX AF,AF'`: what it set is in the other pair.
        z.a_ = flag;
        z.f_ = CF;
        z.a = 0x55;
        z.f = 0;
    }
}

#[test]
fn the_front_door_loads_and_goes_on_to_sa_ld_ret() {
    let mut z = machine(0);
    let mut feeder = TapeFeeder::from_blocks(vec![block(0xFF, &[1, 2, 3])]);
    at_door(&mut z, LD_BYTES, 0xFF, 0x8100, 3, 0x1234);
    assert!(feeder.on_step(&mut z));
    assert_eq!([z.read(0x8100), z.read(0x8101), z.read(0x8102)], [1, 2, 3]);
    assert_eq!((z.pc, z.ix, z.de()), (SA_LD_RET, 0x8103, 0));
    assert!(z.f & CF != 0, "loaded");
    // SA/LD-RET returns to the caller itself: its address is still stacked.
    assert_eq!(z.read16(z.sp), 0x1234);
    assert_eq!(feeder.remaining(), 0);
}

#[test]
fn the_side_door_takes_its_flag_from_af_dash_and_returns_to_the_caller() {
    let mut z = machine(6);
    let mut feeder = TapeFeeder::from_blocks(vec![block(0xFF, &[7; 16])]);
    at_door(&mut z, LD_BYTES_PAST_PREAMBLE, 0xFF, 0xC000, 16, 0x5040);
    let sp = z.sp;
    assert!(feeder.on_step(&mut z));
    assert_eq!(z.pc, 0x5040, "straight back to the caller");
    assert_eq!(z.sp, sp + 2);
    assert!(z.f & CF != 0, "loaded");
    // Into the bank paged at 0xC000, which is bank 6.
    assert_eq!(z.memory.page(Memory::bank(6))[..16], [7; 16]);
    assert_eq!(z.memory.page(Memory::bank(0))[0], 0);
}

#[test]
fn a_wrong_flag_or_a_bad_block_fails_and_is_used_up() {
    let mut bad = block(0xFF, &[1, 2]);
    *bad.last_mut().expect("a checksum") ^= 1;
    let mut feeder = TapeFeeder::from_blocks(vec![block(0x00, &[1, 2]), bad, block(0xFF, &[1])]);
    let mut z = machine(0);
    for (len, why) in [
        (2, "the wrong flag"),
        (2, "a bad checksum"),
        (2, "too short"),
    ] {
        at_door(&mut z, LD_BYTES, 0xFF, 0x8000, len, 0);
        assert!(feeder.on_step(&mut z));
        assert!(z.f & CF == 0, "{why} must fail");
    }
    assert_eq!(feeder.remaining(), 0);
}

#[test]
fn nothing_happens_away_from_the_doors_or_with_the_editor_rom_paged() {
    let mut feeder = TapeFeeder::from_blocks(vec![block(0xFF, &[1])]);
    let mut z = machine(0);
    at_door(&mut z, LD_BYTES, 0xFF, 0x8000, 1, 0);
    z.pc = 0x0557;
    assert!(!feeder.on_step(&mut z));
    // ROM 0, the 128's editor, has other code at the same addresses.
    z.port_out(0x7FFD, 0x00);
    z.pc = LD_BYTES;
    assert!(!feeder.on_step(&mut z));
    assert_eq!(feeder.remaining(), 1);
}
