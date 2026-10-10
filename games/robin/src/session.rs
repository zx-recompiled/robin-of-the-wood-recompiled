//! A game played, a frame at a time: the original's main loop, interrupt,
//! menu and waits, in the order the original runs them, for a frontend or a
//! test to drive (#66).
//!
//! The original's main loop isn't tied to the frame: a pass takes 0.3 to 2
//! frames, 0.989 on average. A session runs one pass a frame, which keeps
//! the original's average speed but not its unevenness (`README.md`,
//! *Status*). Each frame, the interrupt runs first, as it does at the
//! frame's start, unless the original has interrupts off.
//!
//! An animation inside one call (the reveal, the meeting's flash, a
//! journey's wipe) is shown at the original's pace, a picture at a time,
//! while the game stands still (#82).

use crate::assets::{Assets, BANK};
use crate::controls::Controls;
use crate::game::{Game, Picture};
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

/// Whether the interrupt runs at the start of the `since`th frame after a
/// step, whose spans with interrupts off were `quiet`.
fn interrupts_at(quiet: &[std::ops::Range<u32>], since: u32) -> bool {
    let at = since.saturating_mul(FRAME_T);
    !quiet.iter().any(|q| q.contains(&at))
}

/// An animation's pictures, shown at the original's pace (#82).
#[derive(Clone, Debug, Default)]
struct Animation {
    /// The pictures still to come, each with the clock at it: the
    /// T-states from the step's start, the sounds and animations before it
    /// included.
    due: Vec<(u32, Box<[u8; 6912]>)>,
    /// The T-states shown so far.
    shown: u32,
    /// The screen on show: the last picture due, or, before the first, the
    /// screen as it was before the step. None once the last is due, when the
    /// screen the step left is shown.
    showing: Option<Box<[u8; 6912]>>,
}

impl Animation {
    /// `pictures`, from a step that started with `before` on the screen.
    /// The step's clock runs past the last, so it holds the game for as
    /// long as they take.
    fn start(&mut self, before: Box<[u8; 6912]>, pictures: Vec<Picture>) {
        self.due = pictures.into_iter().map(|p| (p.at, p.screen)).collect();
        self.shown = 0;
        self.showing = Some(before);
    }

    /// A frame shown: the screen is the last picture due by its end.
    fn frame(&mut self) {
        let Some(&(last, _)) = self.due.last() else {
            self.showing = None;
            return;
        };
        self.shown = self.shown.saturating_add(FRAME_T);
        if self.shown >= last {
            self.due.clear();
            self.showing = None;
            return;
        }
        let shown = self.shown;
        if let Some(n) = self.due.iter().rposition(|&(at, _)| at <= shown) {
            let later = self.due.split_off(n + 1);
            self.showing = self.due.pop().map(|(_, screen)| screen);
            self.due = later;
        }
    }
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
    /// Frames still to stand still for, while a sound or an animation
    /// plays.
    pub held: u32,
    /// Meanwhile, the frames since the step, and the step's spans with
    /// interrupts off, in which no frame's interrupt runs.
    since: u32,
    quiet: Vec<std::ops::Range<u32>>,
    /// An animation being shown.
    animation: Animation,
    /// Whether the first frame is still to come, which shows the menu.
    fresh: bool,
}

impl Session {
    /// The game as the tape leaves it, started as the original starts: the
    /// first start (`0:CC66`), then, in the first frame, the menu, so its
    /// reveal and its sample play as every later menu's do. `seed` seeds its
    /// random numbers.
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
        let mut session = Session {
            game,
            io,
            state: State::Menu,
            border: 0,
            frames: 0,
            held: 0,
            since: 0,
            quiet: Vec::new(),
            animation: Animation::default(),
            fresh: true,
        };
        session.note_border();
        session
    }

    /// One frame, with `controls` held: the interrupt, then a step. What it
    /// wrote to the ports is in `io.writes`, from this frame alone.
    ///
    /// While a sound from an earlier frame still plays, the game stands
    /// still, as the original does, and only the interrupt runs, unless it
    /// falls while interrupts are off, as they are while a sample plays. So
    /// it does while an animation's pictures are shown (#82).
    pub fn frame(&mut self, assets: &Assets, controls: Controls) {
        self.io.controls = controls;
        self.io.writes.clear();
        self.io.beeps.clear();
        self.io.ay_writes.clear();
        self.io.t = 0;
        self.io.quiet.clear();
        self.frames += 1;
        if self.held > 0 {
            self.held -= 1;
            self.since += 1;
            if interrupts_at(&self.quiet, self.since) {
                interrupt::frame(&mut self.game, &mut self.io);
            }
            self.animation.frame();
            return;
        }
        if !matches!(self.state, State::Ending(_)) {
            interrupt::frame(&mut self.game, &mut self.io);
        }
        let before = self.game.display.screen.clone();
        self.game.pictures.clear();
        if self.fresh {
            self.fresh = false;
            new_game::show_menu(&mut self.game, &mut self.io);
        } else {
            self.state = self.step(assets);
        }
        self.note_border();
        self.held = held_for(self.io.t);
        self.since = 0;
        self.quiet = std::mem::take(&mut self.io.quiet);
        let pictures = std::mem::take(&mut self.game.pictures);
        if !pictures.is_empty() {
            self.animation.start(before, pictures);
        }
        self.animation.frame();
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
        let screen = self
            .animation
            .showing
            .as_deref()
            .unwrap_or(&self.game.display.screen);
        crate::picture::draw(&screen[..], self.border, self.frames, &mut out);
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
                    Choice::Redefine => State::Redefining(new_game::redefine(g, io)),
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

    #[test]
    fn only_a_samples_span_keeps_the_interrupt_off() {
        // Two samples, each just past a frame's start, then an animation or
        // another sound with interrupts on.
        let quiet = [100..FRAME_T + 5, 2 * FRAME_T - 10..2 * FRAME_T + 5];
        let runs: Vec<bool> = (1..=4).map(|n| interrupts_at(&quiet, n)).collect();
        assert_eq!(runs, [false, false, true, true]);
    }

    fn screen(v: u8) -> Box<[u8; 6912]> {
        Box::new([v; 6912])
    }

    #[test]
    fn an_animation_shows_each_picture_once_it_is_due() {
        let mut a = Animation::default();
        // Due a little after 1, 1.25, 1.5 and 2.5 frames.
        let pictures = [4, 5, 6, 10].map(|q| Picture {
            at: q * FRAME_T / 4 + 10,
            screen: screen(q as u8),
        });
        a.start(screen(0), pictures.to_vec());
        let mut shown = Vec::new();
        for _ in 0..4 {
            a.frame();
            shown.push(a.showing.as_ref().map(|s| s[0]));
        }
        assert_eq!(
            shown,
            [Some(0), Some(6), None, None],
            "the screen before until the first is due; the last due by each \
             frame's end; then the screen the step left"
        );
    }
}
