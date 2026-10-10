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
    play_sample(g, io, pick);
}

/// Plays the `n`th of the four samples (`4:C015` is the fourth's entry).
pub fn play_sample(g: &mut Game, io: &mut Io, n: usize) {
    let (delay, length, amplitudes, sample) = SAMPLES[n];
    play(g, io, delay, length, amplitudes, sample);
}

/// Plays the fourth sample (`4:C015`), as a shot hitting Robin does.
pub fn shot_sample(g: &mut Game, io: &mut Io) {
    play_sample(g, io, 3);
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

/// The bank the music driver is in, and where it keeps whether a tune is
/// playing and the effect it's to start.
pub const MUSIC_BANK: u8 = 6;
const PLAYING: u16 = 0xC190;
const EFFECT: u16 = 0xC15A;

/// Writes `v` to AY register `r` (`6:C127`).
fn ay(io: &mut Io, r: u8, v: u8) {
    io.out(0xFFFD, r);
    io.out(0xBFFD, v);
}

/// Starts the sound for meeting the wanderer, for the interrupt's music
/// player, unless a tune is playing: its flag, and the mixer and the noise
/// period set (`6:C139`).
pub fn meeting(g: &mut Game, io: &mut Io) {
    let bank = usize::from(MUSIC_BANK);
    if g.read_in(bank, PLAYING) != 0 {
        return;
    }
    g.write_in(bank, EFFECT, 0x80);
    g.write_in(bank, PLAYING, 0);
    ay(io, 7, 0x28);
    ay(io, 6, 0x1F);
}

/// Where the music driver keeps the tune it's playing: its start, the
/// next notes' two pointers, and its settings.
const TUNE_START: u16 = 0xCBD9;
const TUNE_SPEED: u16 = 0xC0A1;
const TUNE_COUNT: u16 = 0xC0A3;
const TUNE_NOTES: u16 = 0xC9A2;
const TUNE_OTHER: u16 = 0xC9A0;
/// The values the AY's registers 0 to 13 are reset to.
const AY_RESET: u16 = 0xC230;

/// A tune for the music driver: where it starts again, its speed, and its
/// two parts' notes.
#[derive(Clone, Copy)]
pub struct Tune {
    pub start: u16,
    pub speed: u8,
    pub notes: u16,
    pub other: u16,
}

/// The tune a hit plays (`6:C048`), and the one an arrival plays (`6:C000`).
pub const HIT: Tune = Tune {
    start: 0xC04B,
    speed: 0x08,
    notes: 0xC98C,
    other: 0xC977,
};
pub const ARRIVAL: Tune = Tune {
    start: 0xC04B,
    speed: 0x08,
    notes: 0xC7C4,
    other: 0xC657,
};

/// The tune the game over plays (`6:C006`).
pub const GAME_OVER: Tune = Tune {
    start: 0xC065,
    speed: 0x0C,
    notes: 0xC954,
    other: 0xC92F,
};

/// Starts the game over's tune (`6:C006`).
pub fn game_over_tune(g: &mut Game, io: &mut Io) {
    start_tune(g, io, GAME_OVER);
}

/// Starts the tune a hit plays (`6:C048`).
pub fn hit_tune(g: &mut Game, io: &mut Io) {
    start_tune(g, io, HIT);
}

/// Starts the tune an arrival plays (`6:C000`).
pub fn arrival_tune(g: &mut Game, io: &mut Io) {
    start_tune(g, io, ARRIVAL);
}

/// Starts `tune` for the interrupt's music player (`6:C07D`): its pointers
/// and settings, then the AY's registers reset and the envelope period set.
pub fn start_tune(g: &mut Game, io: &mut Io, tune: Tune) {
    let bank = usize::from(MUSIC_BANK);
    let word = |g: &mut Game, at: u16, v: u16| {
        let [lo, hi] = v.to_le_bytes();
        g.write_in(bank, at, lo);
        g.write_in(bank, at + 1, hi);
    };
    word(g, TUNE_START, tune.start);
    g.write_in(bank, TUNE_SPEED, tune.speed);
    g.write_in(bank, EFFECT, 0);
    g.write_in(bank, PLAYING, 0);
    g.write_in(bank, TUNE_COUNT, 1);
    word(g, TUNE_NOTES, tune.notes);
    word(g, TUNE_OTHER, tune.other);
    reset_ay(g, io);
    ay(io, 12, 0x20);
}

/// The AY's registers 0 to 13 set from the music player's table
/// (`6:C21C`, through `6:C045`).
pub fn reset_ay(g: &mut Game, io: &mut Io) {
    let bank = usize::from(MUSIC_BANK);
    for r in 0..14u8 {
        let v = g.read_in(bank, AY_RESET + u16::from(r));
        ay(io, r, v);
    }
}

/// The zap a hit plays there and then (`0xBC4C`): for each count from
/// `0x14` to `0x27`, that many writes to the beeper of R's bits 3 and 4.
pub fn zap(io: &mut Io) {
    for count in 0x14..0x28 {
        for _ in 0..count {
            let a = io.random.r() & 0x18;
            io.out(u16::from(a) << 8 | 0xFE, a);
        }
    }
}

/// A beeper sound of `b` toggles of EAR and MIC, the delay between them
/// shortening from `b` (`0:D8C6`): 256 for 0.
pub fn beep(io: &mut Io, mut b: u8) {
    let mut a = 0u8;
    loop {
        a ^= 0x18;
        io.out(u16::from(a) << 8 | 0xFE, a);
        b = b.wrapping_sub(1);
        if b == 0 {
            return;
        }
    }
}

/// Where the music driver keeps an effect's two settings.
const EFFECT_A: u16 = 0xC18C;
const EFFECT_B: u16 = 0xC18E;

/// Starts the effect a departure plays, for the interrupt's player
/// (`6:C018`): its settings, a tune's flag cleared, and the mixer set.
pub fn departure(g: &mut Game, io: &mut Io) {
    start_effect(g, io, 0xFF, 0x0000, 0x0002);
}

/// Starts an effect (`6:C1D5`).
fn start_effect(g: &mut Game, io: &mut Io, playing: u8, a: u16, b: u16) {
    let bank = usize::from(MUSIC_BANK);
    for (at, v) in [(EFFECT_A, a), (EFFECT_B, b)] {
        let [lo, hi] = v.to_le_bytes();
        g.write_in(bank, at, lo);
        g.write_in(bank, at + 1, hi);
    }
    g.write_in(bank, PLAYING, playing);
    g.write_in(bank, EFFECT, 0);
    ay(io, 7, 0x38);
}

/// The sample a departure's end plays (`4:C006`).
pub fn departure_sample(g: &mut Game, io: &mut Io) {
    play(g, io, 5, 0x0578, 0xC9A0, 0xC9B8);
}

/// The sample the scripted scene's end plays (`4:C000`).
pub fn scene_sample(g: &mut Game, io: &mut Io) {
    play(g, io, 1, 0x07D0, 0xC1B0, 0xC1D0);
}

/// Where the warble's sample is: its length, then its bits; and its delay
/// between bits, kept in its code.
const WARBLE: u16 = 0x8B79;
const WARBLE_DELAY: u16 = 0x8B56;

/// A beeper warble (`0x8B32`): the sample at `0x8B79` played 11 times, a
/// bit at a time from the highest, EAR and MIC on for a 1 and off for a 0,
/// with the delay between bits growing from 2 to `0x0C`. Each time is
/// followed by a pause, a 16K copy of the ROM onto itself. Its bytes are
/// rotated through and left as they were.
pub fn warble(g: &mut Game, io: &mut Io) {
    let length = u16::from_le_bytes([g.read(WARBLE), g.read(WARBLE + 1)]);
    for delay in 2..=0x0Cu8 {
        g.write(WARBLE_DELAY, delay);
        for n in 0..length {
            let b = g.read(WARBLE + 2 + n);
            for bit in (0..8).rev() {
                let a = if b >> bit & 1 != 0 { 0x18 } else { 0 };
                io.out(u16::from(a) << 8 | 0xFE, a);
            }
        }
    }
}

/// The sample the menu plays (`4:C00F`).
pub fn menu_sample(g: &mut Game, io: &mut Io) {
    play(g, io, 3, 0x1644, 0xDF80, 0xDFE0);
}
