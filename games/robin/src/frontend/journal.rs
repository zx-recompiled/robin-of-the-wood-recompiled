//! What the player has found this game, for the aids that show it (#92):
//! the locations Robin has entered, and when (#96). Kept by the window from
//! the session each frame, never by the game.

use robin::map::LOCATIONS;
use robin::session::{Session, State};

#[derive(Clone, Debug)]
pub struct Journal {
    /// Each location Robin has stood in this game.
    visited: Box<[bool; LOCATIONS]>,
    count: usize,
    /// Where he is, while a game is being played.
    pub here: Option<u16>,
    /// Counts every change, so the panel is redrawn only then.
    version: u64,
    playing: bool,
}

impl Default for Journal {
    fn default() -> Journal {
        Journal {
            visited: Box::new([false; LOCATIONS]),
            count: 0,
            here: None,
            version: 0,
            playing: false,
        }
    }
}

impl Journal {
    /// Takes in the frame just played. A new game starts a new journal.
    pub fn note(&mut self, session: &Session) {
        let playing = session.state == State::Playing;
        if playing && !self.playing {
            *self = Journal {
                version: self.version + 1,
                ..Journal::default()
            };
        }
        self.playing = playing;
        let here = playing.then_some(session.game.map.location);
        if here != self.here {
            self.here = here;
            self.version += 1;
        }
        if let Some(at) = here
            && let Some(seen) = self.visited.get_mut(usize::from(at))
            && !*seen
        {
            *seen = true;
            self.count += 1;
            self.version += 1;
        }
    }

    /// Whether Robin has been to `location` this game.
    #[must_use]
    pub fn visited(&self, location: u16) -> bool {
        self.visited
            .get(usize::from(location))
            .copied()
            .unwrap_or(false)
    }

    /// How many locations he's been to.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }
}

#[cfg(test)]
impl Journal {
    /// A journal that has been to `visited`, standing at `here`.
    pub fn of(visited: impl IntoIterator<Item = u16>, here: u16) -> Journal {
        let mut j = Journal::default();
        for at in visited.into_iter().chain([here]) {
            if !j.visited[usize::from(at)] {
                j.visited[usize::from(at)] = true;
                j.count += 1;
            }
        }
        j.here = Some(here);
        j
    }
}
