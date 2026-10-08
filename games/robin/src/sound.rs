//! The game's sound effects: what they write to the ports (`docs/re/robin.md`,
//! *Sound*). How they sound, with the time between the writes, is the
//! frontend's (#7).

use crate::game::Game;
use crate::io::Io;

/// The bank the sample player is in.
pub const PLAYER_BANK: usize = 4;
/// Where the player keeps its delay between bits, and the AY's registers
/// while it plays.
const DELAY: u16 = 0xC141;
const SAVED: u16 = 0xC194;
/// The byte every register is set to before playing.
const SET_TO: u16 = 0xC1A2;

/// The four samples `4:C012` picks from: the delay between bits, the length
/// less one, the amplitudes (one a 64 bytes), and the sample.
const SAMPLES: [(u8, u16, u16, u16); 4] = [
    (1, 0x0834, 0xCF30, 0xCF58),
    (1, 0x07D0, 0xD790, 0xD7B0),
    (5, 0x044C, 0xFA20, 0xFA38),
    (1, 0x03E8, 0xFA10, 0xF628),
];

/// A beeper twang: EAR and MIC toggled once for each count of `b` up to
/// `0x80`, with a delay that grows between them (`0xBE2C`).
pub fn twang(io: &mut Io, mut b: u8) {
    let mut a = 0u8;
    loop {
        a ^= 0x18;
        io.out(u16::from(a) << 8 | 0xFE, a);
        b = b.wrapping_add(1);
        if b & 0x80 != 0 {
            return;
        }
    }
}

/// Plays one of the four samples, picked by R, through the beeper and the
/// AY (`4:C012`), with bank 4 paged in.
pub fn sample(g: &mut Game, io: &mut Io) {
    let pick = usize::from(io.random.r() & 6) / 2;
    let (delay, length, amplitudes, sample) = SAMPLES[pick];
    play(g, io, delay, length, amplitudes, sample);
}

/// The AY's registers the player saves and sets, from 14 down to 1.
fn registers() -> impl Iterator<Item = (u16, u8)> {
    (1..=14u8).rev().zip(0..).map(|(r, n)| (n, r))
}

/// Plays `length + 1` bytes of `sample`, a bit at a time from the highest,
/// each masked with the amplitude at `amplitudes` (the next one every 64
/// bytes), to the beeper's EAR and MIC and to the AY register left selected
/// (`4:C090`). The AY's registers are saved first, all set to one value,
/// and restored after.
pub fn play(g: &mut Game, io: &mut Io, delay: u8, length: u16, amplitudes: u16, sample: u16) {
    let bank = PLAYER_BANK;
    g.write_in(bank, DELAY, delay);
    for (n, r) in registers() {
        io.out(0xFFFD, r);
        let v = io.input(0xFFFD);
        g.write_in(bank, SAVED + n, v);
    }
    let set_to = g.read_in(bank, SET_TO);
    for (_, r) in registers() {
        io.out(0xFFFD, r);
        io.out(0xBFFD, set_to);
    }
    let (mut left, mut amplitude, mut at) = (length, amplitudes, sample);
    'bytes: loop {
        for _ in 0..0x40 {
            for _ in 0..8 {
                // Rotated in place: after eight, the byte is as it was.
                let byte = g.read_in(bank, at);
                g.write_in(bank, at, byte.rotate_left(1));
                let a = if byte & 0x80 != 0 {
                    g.read_in(bank, amplitude)
                } else {
                    0
                };
                let ear = (a >> 1) & 0x18;
                io.out(u16::from(ear) << 8 | 0xFE, ear);
                io.out(0xBFFD, a & 0x0F);
            }
            at = at.wrapping_add(1);
            left = left.wrapping_sub(1);
            if left & 0x8000 != 0 {
                break 'bytes;
            }
        }
        amplitude = amplitude.wrapping_add(1);
    }
    for (n, r) in registers() {
        io.out(0xFFFD, r);
        io.out(0xBFFD, g.read_in(bank, SAVED + n));
    }
}
