//! A game played, a frame at a time: the original's main loop, interrupt,
//! menu and waits, in the order the original runs them, for a frontend or a
//! test to drive (#66).
//!
//! The original's main loop isn't tied to the frame: a pass takes 0.3 to 2
//! frames, 0.989 on average. A session runs one pass a frame, which keeps
//! the original's average speed but not its unevenness (`README.md`,
//! *Status*). Each frame, the interrupt runs first, as it does at the
//! frame's start, unless the original has interrupts off.

use crate::assets::{Assets, BANK};
use crate::controls::Controls;
use crate::game::Game;
use crate::interrupt;
use crate::io::{Io, Random};
use crate::main_loop::{self, Pass};
use crate::new_game::{self, Choice, Menu, Redefine};

/// The ending's delay between its two waits, in frames: `0x8000` turns of a
/// loop of 26 T-states.
const ENDING_DELAY: u8 = 12;

/// A frame of the 128K, in T-states.
pub const FRAME_T: u32 = 70_908;

/// The frames after this one a sound of `t` T-states holds the game for:
/// the original stands still while it plays (#67).
#[must_use]
pub const fn held_for(t: u32) -> u32 {
    t / FRAME_T
}

/// What the game is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// The menu, waiting for a key.
    Menu,
    /// Redefining the keys.
    Redefining(Redefine),
    /// In play.
    Playing,
    /// The game over's message up, waiting for a key.
    GameOver,
    /// The ending's message up, and its waits.
    Ending(EndingWait),
}

/// Where the ending's waits have got to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndingWait {
    /// For every key to be let go.
    Release,
    /// The delay, frames left.
    Delay(u8),
    /// For a key.
    Press,
}

/// A game played: its state, what it exchanges with the world, and where it
/// is.
#[derive(Clone, Debug)]
pub struct Session {
    pub game: Game,
    pub io: Io,
    pub state: State,
    /// The border's colour, as last written to the ULA's port.
    pub border: u8,
    /// Frames played, which the picture's flashing goes by.
    pub frames: u64,
    /// Frames still to stand still for, while a sound plays, and whether
    /// the interrupt is off meanwhile.
    pub held: u32,
    pub held_quiet: bool,
}

impl Session {
    /// The game as the tape leaves it, started as the original starts: the
    /// first start (`0:CC66`) and the menu. `seed` seeds its random numbers.
    #[must_use]
    pub fn new(assets: &Assets, seed: u64) -> Session {
        let mut banks = Box::new([[0u8; BANK]; 8]);
        for (n, bank) in banks.iter_mut().enumerate() {
            *bank = *assets.bank(n);
        }
        let mut game = Game::from_memory(&banks);
        let mut io = Io {
            random: Random::seeded(seed),
            ..Io::default()
        };
        new_game::first_start(&mut game, &mut io);
        new_game::show_menu(&mut game, &mut io);
        let mut session = Session {
            game,
            io,
            state: State::Menu,
            border: 0,
            frames: 0,
            held: 0,
            held_quiet: false,
        };
        session.note_border();
        session
    }

    /// One frame, with `controls` held: the interrupt, then a step. What it
    /// wrote to the ports is in `io.writes`, from this frame alone.
    ///
    /// While a sound from an earlier frame still plays, the game stands
    /// still, as the original does, and only the interrupt runs, unless the
    /// sound plays with interrupts off.
    pub fn frame(&mut self, assets: &Assets, controls: Controls) {
        self.io.controls = controls;
        self.io.writes.clear();
        self.io.beeps.clear();
        self.io.t = 0;
        self.io.interrupts_off = false;
        self.frames += 1;
        if self.held > 0 {
            self.held -= 1;
            if !self.held_quiet {
                interrupt::frame(&mut self.game, &mut self.io);
            }
            return;
        }
        if !matches!(self.state, State::Ending(_)) {
            interrupt::frame(&mut self.game, &mut self.io);
        }
        self.state = self.step(assets);
        self.note_border();
        self.held = held_for(self.io.t);
        self.held_quiet = self.io.interrupts_off;
    }

    /// The border, from this frame's writes to the ULA's port (bit 0 of
    /// the port clear): bits 0 to 2.
    fn note_border(&mut self) {
        if let Some(&(_, v)) = self.io.writes.iter().rev().find(|(p, _)| p & 1 == 0) {
            self.border = v & 7;
        }
    }

    /// The picture now, `picture::FULL_W` by `picture::FULL_H`, as 0RGB.
    #[must_use]
    pub fn picture(&self) -> Vec<u32> {
        let mut out = vec![0; crate::picture::FULL_W * crate::picture::FULL_H];
        crate::picture::draw(
            &self.game.display.screen[..],
            self.border,
            self.frames,
            &mut out,
        );
        out
    }

    fn step(&mut self, assets: &Assets) -> State {
        let (g, io) = (&mut self.game, &mut self.io);
        match self.state {
            State::Menu => match new_game::menu_keys(io) {
                Menu::Waiting => State::Menu,
                Menu::Start => {
                    new_game::set_up(g, assets, io);
                    State::Playing
                }
                Menu::Pick => match new_game::choose(g, io) {
                    Choice::Redefine => State::Redefining(new_game::redefine(g)),
                    Choice::Set(_) | Choice::Nothing => {
                        new_game::show_menu(g, io);
                        State::Menu
                    }
                },
            },
            State::Redefining(mut r) => {
                if r.step(g, io) {
                    new_game::show_menu(g, io);
                    State::Menu
                } else {
                    State::Redefining(r)
                }
            }
            State::Playing => match main_loop::frame(g, assets, io) {
                Pass::Next => State::Playing,
                Pass::NewGame => self.restart(),
                Pass::GameOver => State::GameOver,
                Pass::Ending => State::Ending(EndingWait::Release),
            },
            State::GameOver => {
                if new_game::any_key(io) {
                    self.restart()
                } else {
                    State::GameOver
                }
            }
            State::Ending(wait) => match wait {
                EndingWait::Release if new_game::any_key(io) => self.state,
                EndingWait::Release => State::Ending(EndingWait::Delay(ENDING_DELAY)),
                EndingWait::Delay(0) => State::Ending(EndingWait::Press),
                EndingWait::Delay(n) => State::Ending(EndingWait::Delay(n - 1)),
                EndingWait::Press if new_game::any_key(io) => self.restart(),
                EndingWait::Press => self.state,
            },
        }
    }

    /// A new game, as BREAK and the ends of the waits start it (`0xBE5A`):
    /// the start, and the menu.
    fn restart(&mut self) -> State {
        new_game::start(&mut self.game, &mut self.io);
        new_game::show_menu(&mut self.game, &mut self.io);
        State::Menu
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sound_holds_the_game_for_the_frames_it_takes() {
        assert_eq!(held_for(0), 0);
        assert_eq!(held_for(FRAME_T - 1), 0, "within the frame");
        assert_eq!(held_for(FRAME_T), 1);
        assert_eq!(held_for(10 * FRAME_T + 5), 10);
    }
}
