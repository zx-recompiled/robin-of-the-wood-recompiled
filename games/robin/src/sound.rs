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

/// The menu's tune (`6:C003`).
pub const MENU: Tune = Tune {
    start: 0xC058,
    speed: 0x06,
    notes: 0xC4EC,
    other: 0xC37F,
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

/// Where the player keeps whether the music is off (`0xFF`) or on (0), and
/// the count before ENTER can toggle it again.
const MUSIC_OFF: u16 = 0xC2CD;
const DEBOUNCE: u16 = 0xC2CE;
/// The tone periods, a word for each note.
const PERIODS: u16 = 0xC308;

/// ENTER and the tune, each frame, from the interrupt (`6:C2CF`, through
/// `6:C03C`). ENTER held, once the count since the last toggle has run out,
/// toggles the music: back on, the tune starts again; off, two volumes go to
/// 0. While the music is on, the tune plays on.
///
/// # Panics
///
/// If the tune to start again is none of the four the game starts.
pub fn music(g: &mut Game, io: &mut Io) {
    let bank = usize::from(MUSIC_BANK);
    let count = g.read_in(bank, DEBOUNCE);
    if count != 0 {
        g.write_in(bank, DEBOUNCE, count - 1);
    }
    let enter = io.input(0xBFFE) & 1 == 0;
    if enter && g.read_in(bank, DEBOUNCE) == 0 {
        g.write_in(bank, DEBOUNCE, 0x32);
        let off = !g.read_in(bank, MUSIC_OFF);
        g.write_in(bank, MUSIC_OFF, off);
        if off == 0 {
            let start =
                u16::from_le_bytes([g.read_in(bank, TUNE_START), g.read_in(bank, TUNE_START + 1)]);
            let tune = match start {
                0xC04B => ARRIVAL,
                0xC058 => MENU,
                0xC065 => GAME_OVER,
                s => panic!("the tune to start again, at 0xCBD9, is {s:#06x}, none of the game's"),
            };
            start_tune(g, io, tune);
        } else {
            ay(io, 8, 0);
            ay(io, 10, 0);
        }
        return;
    }
    if g.read_in(bank, MUSIC_OFF) == 0 {
        step_tune(g, io);
    }
}

/// The tune, on (`6:C0A2`): every so many frames, by its speed, each of its
/// two parts plays its next note.
pub fn step_tune(g: &mut Game, io: &mut Io) {
    let bank = usize::from(MUSIC_BANK);
    let mut count = g.read_in(bank, TUNE_COUNT).wrapping_sub(1);
    let play = count == 0;
    if play {
        count = g.read_in(bank, TUNE_SPEED);
    }
    g.write_in(bank, TUNE_COUNT, count);
    if play {
        first_part(g, io);
        second_part(g, io);
    }
}

fn read_word(g: &Game, at: u16) -> u16 {
    let bank = usize::from(MUSIC_BANK);
    u16::from_le_bytes([g.read_in(bank, at), g.read_in(bank, at + 1)])
}

fn write_word(g: &mut Game, at: u16, v: u16) {
    let bank = usize::from(MUSIC_BANK);
    let [lo, hi] = v.to_le_bytes();
    g.write_in(bank, at, lo);
    g.write_in(bank, at + 1, hi);
}

/// The next note of a part from its pointer at `pointer`: `0xFF` loops it
/// to the address `skip` bytes after; 0 is a rest.
fn next_note(g: &mut Game, pointer: u16, skip: u16) -> Option<u8> {
    let bank = usize::from(MUSIC_BANK);
    let at = read_word(g, pointer).wrapping_add(1);
    write_word(g, pointer, at);
    let mut note = g.read_in(bank, at);
    if note == 0xFF {
        let to = read_word(g, at.wrapping_add(skip));
        write_word(g, pointer, to);
        note = g.read_in(bank, to);
    }
    (note != 0).then_some(note)
}

/// The note's period, from the table.
fn period(g: &Game, note: u8) -> (u8, u8) {
    let bank = usize::from(MUSIC_BANK);
    let at = PERIODS.wrapping_add(u16::from(note) * 2);
    (g.read_in(bank, at), g.read_in(bank, at + 1))
}

/// The tune's first part (`6:C0B5`), on tone A with a fixed volume: its
/// loop's address 3 bytes after the `0xFF`, and its notes' bit 7 ignored.
fn first_part(g: &mut Game, io: &mut Io) {
    let Some(note) = next_note(g, TUNE_OTHER, 3) else {
        return;
    };
    let (lo, hi) = period(g, note & 0x7F);
    ay(io, 0, lo);
    ay(io, 1, hi);
    ay(io, 8, 0x0C);
}

/// The tune's second part (`6:C0EC`), on tone C with the envelope: its
/// loop's address straight after the `0xFF`.
fn second_part(g: &mut Game, io: &mut Io) {
    let Some(note) = next_note(g, TUNE_NOTES, 1) else {
        return;
    };
    let (lo, hi) = period(g, note);
    ay(io, 4, lo);
    ay(io, 5, hi);
    ay(io, 10, 0x1F);
    ay(io, 13, 0);
}

/// The wobble's step and its counter, kept in its code.
const WOBBLE_STEP: u16 = 0xC27E;
const WOBBLE_COUNT: u16 = 0xC293;

/// The wobble, each frame (`6:C26F`, through `6:C030`): tone C's period,
/// read back from registers 4 and 5, moved on by a step and written back.
/// Every fourth frame the step changes sign, as the code works it out: the
/// low byte negated, the high byte inverted.
pub fn wobble(g: &mut Game, io: &mut Io) {
    io.out(0xFFFD, 4);
    let lo = io.input(0xFFFD);
    io.out(0xFFFD, 5);
    let hi = io.input(0xFFFD);
    let step = read_word(g, WOBBLE_STEP);
    let [lo, hi] = u16::from_le_bytes([lo, hi])
        .wrapping_add(step)
        .to_le_bytes();
    io.out(0xBFFD, hi);
    io.out(0xFFFD, 4);
    io.out(0xBFFD, lo);
    let bank = usize::from(MUSIC_BANK);
    let count = g.read_in(bank, WOBBLE_COUNT).wrapping_sub(1);
    g.write_in(bank, WOBBLE_COUNT, count);
    if count & 3 != 0 {
        return;
    }
    let [l, h] = step.to_le_bytes();
    write_word(g, WOBBLE_STEP, u16::from_le_bytes([l.wrapping_neg(), !h]));
}

/// The effect's sweep: tone B's period, and what's added each frame.
const SWEEP: u16 = 0xC18C;
const SWEEP_STEP: u16 = 0xC18E;

/// The effect, each frame (`6:C1EB`, through `6:C015`): while its count
/// runs, tone B at full volume, its period swept, the low byte's top bit
/// flipped each time. Once it's run out, the other sound (`6:C15B`).
pub fn effect(g: &mut Game, io: &mut Io) {
    let bank = usize::from(MUSIC_BANK);
    let count = g.read_in(bank, PLAYING);
    if count == 0 {
        quiet(g, io);
        return;
    }
    g.write_in(bank, PLAYING, count - 1);
    ay(io, 9, 0x0F);
    let [lo, hi] = read_word(g, SWEEP).to_le_bytes();
    let at = u16::from_le_bytes([lo ^ 0x80, hi]).wrapping_add(read_word(g, SWEEP_STEP));
    write_word(g, SWEEP, at);
    let [lo, hi] = at.to_le_bytes();
    ay(io, 2, lo);
    ay(io, 3, hi);
}

/// With no effect running (`6:C15B`): with no meeting's sound either, the
/// mixer set and tone B quiet. With it, it counts down, and every eighth
/// frame tone B's volume is set from the count.
pub fn quiet(g: &mut Game, io: &mut Io) {
    let bank = usize::from(MUSIC_BANK);
    let n = g.read_in(bank, EFFECT);
    if n == 0 {
        ay(io, 7, 0x38);
        ay(io, 9, 0);
        ay(io, 2, 0);
        ay(io, 3, 0);
        return;
    }
    let n = n - 1;
    g.write_in(bank, EFFECT, n);
    if n & 7 != 0 {
        return;
    }
    ay(io, 9, n >> 3 & 0x0F);
}
