//! The menu and a new game (`docs/re/robin.md`, *A new game*). The menu's
//! waits for keys, which the original makes in busy loops, are steps the
//! caller takes once a frame.

use crate::actions;
use crate::game::Game;
use crate::io::Io;
use crate::print;
use crate::screen;
use crate::sound;

/// The control method chosen, 0 keys, 1 Kempston, 2 Interface II, and the
/// table of ports and bits it reads.
const METHOD: u16 = 0xABEE;
const CONTROLS: u16 = 0xD152;
/// Each method's table, in its order.
const TABLES: [u16; 3] = [0xD06E, 0xD07F, 0xD088];

/// The menu's tune (`6:C003`).
const MENU_TUNE: sound::Tune = sound::Tune {
    start: 0xC058,
    speed: 0x06,
    notes: 0xC4EC,
    other: 0xC37F,
};

/// The menu (`0xAB28`): the play area cleared to white on black, a
/// highlight at the chosen method's line, its five lines, and revealed.
pub fn menu(g: &mut Game) {
    screen::clear_play_area(g, 0x47);
    let line = (g.read(METHOD) + 1).wrapping_mul(0x20);
    print::print_message(g, 0x40, 0x10, line);
    for (message, x, y) in [
        (0x48, 0x24, 0x04),
        (0x49, 0x24, 0x1C),
        (0x4A, 0x24, 0x3C),
        (0x4B, 0x24, 0x5C),
        (0x4C, 0x10, 0x7C),
    ] {
        print::print_message(g, message, x, y);
    }
    screen::reveal(g);
}

/// The menu shown, with its sample and its tune (`0:CCAB`).
pub fn show_menu(g: &mut Game, io: &mut Io) {
    menu(g);
    actions::banked_call(g, io, sound::PLAYER_BANK as u8, sound::menu_sample);
    actions::banked_call(g, io, sound::MUSIC_BANK, |g, io| {
        sound::start_tune(g, io, MENU_TUNE);
    });
}

/// What a poll of the menu's keys found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    /// Nothing yet: poll again.
    Waiting,
    /// 0: start the game.
    Start,
    /// One of 1 to 4: pick from them ([`choose`]), then show the menu again.
    Pick,
}

/// One poll of the menu's keys (`0:CCBB`): 0 starts; any of 1 to 4 is a
/// pick.
#[must_use]
pub fn menu_keys(io: &Io) -> Menu {
    if io.input(0xEFFE) & 1 == 0 {
        Menu::Start
    } else if !io.input(0xF7FE) & 0x0F == 0 {
        Menu::Waiting
    } else {
        Menu::Pick
    }
}

/// What a pick did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// 1: the keys are to be redefined, then the method set to keys.
    Redefine,
    /// 2 or 3, that method set; or 4, nothing.
    Chosen,
}

/// The method the menu's keys pick (`0:CFB2`): 1 keys, after they're
/// redefined; 2 Kempston; 3 Interface II; else nothing.
pub fn choose(g: &mut Game, io: &Io) -> Choice {
    let row = io.input(0xF7FE);
    if row & 1 == 0 {
        return Choice::Redefine;
    }
    let method = if row & 2 == 0 {
        1
    } else if row & 4 == 0 {
        2
    } else {
        return Choice::Chosen;
    };
    set_method(g, method);
    Choice::Chosen
}

/// The method chosen: its number, and its table (`0:CFD5`).
pub fn set_method(g: &mut Game, method: u8) {
    g.write(METHOD, method);
    let [lo, hi] = TABLES[usize::from(method)].to_le_bytes();
    g.write(CONTROLS, lo);
    g.write(CONTROLS + 1, hi);
}

/// The redefinition's strings, the keys it records, and their names.
const HEADING: u16 = 0xD0E8;
const PROMPTS: u16 = 0xD10C;
const PROMPT_LENGTH: u16 = 0x0E;
const NAME_IN_PROMPT: u16 = 0x0B;
const KEYS: u16 = 0xD154;
const NAMES: u16 = 0xD159;
/// How many keys it asks for: the four ways and fire.
const ASKED: u8 = 5;

/// Redefining the keys (`0:CFDC`): for each of five, every key let go, then
/// one pressed, recorded and named. Its waits are steps, once a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Redefine {
    /// How many have been recorded.
    pub done: u8,
    /// Whether every key has been let go since the last.
    pub released: bool,
}

/// The redefinition's screen and its first prompt (`0:CFDC`).
pub fn redefine(g: &mut Game) -> Redefine {
    screen::clear_play_area(g, 0x45);
    screen::reveal(g);
    print::print_replacing(g, HEADING);
    print::print_replacing(g, PROMPTS);
    Redefine {
        done: 0,
        released: false,
    }
}

impl Redefine {
    /// One frame's step: waiting for every key to be let go (`0:D028`), then
    /// for one (`0:D01D`). Once it's pressed, it's recorded, and the next
    /// prompt printed. True once all five are, with the keys set as the
    /// method (`0:CFBD`).
    pub fn step(&mut self, g: &mut Game, io: &Io) -> bool {
        if !self.released {
            if any_key(io) {
                return false;
            }
            self.released = true;
        }
        let Scan::One(code) = scan(io) else {
            return false;
        };
        record(g, self.done, code);
        self.done += 1;
        self.released = false;
        if self.done == ASKED {
            set_method(g, 0);
            return true;
        }
        print::print_replacing(g, PROMPTS + u16::from(self.done) * PROMPT_LENGTH);
        false
    }
}

/// Whether any key is held, read from every half-row at once (`0:D028`).
#[must_use]
pub fn any_key(io: &Io) -> bool {
    !io.input(0x00FE) & 0x1F != 0
}

/// What a scan of the keyboard found (`0:D033`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scan {
    None,
    /// One key, by its code: `0x27` for the first of the first half-row
    /// read, down by one a key, a half-row of five at a time.
    One(u8),
    Several,
}

/// The half-rows the scan reads, in its order.
pub const SCAN_ROWS: [u16; 8] = [
    0xFEFE, 0xFDFE, 0xFBFE, 0xF7FE, 0xEFFE, 0xDFFE, 0xBFFE, 0x7FFE,
];

/// A scan of the keyboard (`0:D033`): the one key held, if only one is.
#[must_use]
pub fn scan(io: &Io) -> Scan {
    let mut found = None;
    for (row, port) in SCAN_ROWS.into_iter().enumerate() {
        let keys = !io.input(port) & 0x1F;
        if keys == 0 {
            continue;
        }
        if found.is_some() || keys & (keys - 1) != 0 {
            return Scan::Several;
        }
        let bit = keys.trailing_zeros() as u8;
        found = Some(0x2F - row as u8 - 8 * (bit + 1));
    }
    found.map_or(Scan::None, Scan::One)
}

/// The `n`th key recorded as `code`, and its name put into its prompt,
/// printed again (`0:D000`).
pub fn record(g: &mut Game, n: u8, code: u8) {
    g.write(KEYS + u16::from(n), code);
    let name = g.read(NAMES + u16::from(code));
    let prompt = PROMPTS + u16::from(n) * PROMPT_LENGTH;
    g.write(prompt + NAME_IN_PROMPT, name);
    print::print_replacing(g, prompt);
}
