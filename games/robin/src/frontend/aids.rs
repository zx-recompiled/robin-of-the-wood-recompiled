//! The aids the player has asked for (#92): which are on, the rules that
//! tie the map's own switches to it, and the picker that sets them, a
//! window that pauses the game. Nothing here reaches the game: the window
//! reads it to draw the panel, and to know when to stand the game still.

/// One of the aids, each its own switch (#92, Decision 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aid {
    Map,
    Seen,
    Now,
    Objective,
    Hints,
    Energy,
    Lives,
    NoWitch,
    Saves,
}

/// Every aid, in the picker's order.
pub const ALL: [Aid; 9] = [
    Aid::Map,
    Aid::Seen,
    Aid::Now,
    Aid::Objective,
    Aid::Hints,
    Aid::Energy,
    Aid::Lives,
    Aid::NoWitch,
    Aid::Saves,
];

impl Aid {
    fn index(self) -> usize {
        ALL.iter().position(|&a| a == self).unwrap_or(0)
    }

    /// The switch it depends on (#92, Decision 8): who you've seen is drawn
    /// on the map, and where they are now adds to who you've seen.
    #[must_use]
    pub const fn parent(self) -> Option<Aid> {
        match self {
            Aid::Seen => Some(Aid::Map),
            Aid::Now => Some(Aid::Seen),
            _ => None,
        }
    }

    /// How deep it sits under the map: 0 for its own switch.
    #[must_use]
    pub fn depth(self) -> usize {
        self.parent().map_or(0, |p| p.depth() + 1)
    }

    /// An assist changes the game; the rest only show it (#92, Decision 2).
    #[must_use]
    pub const fn is_assist(self) -> bool {
        matches!(self, Aid::Energy | Aid::Lives | Aid::NoWitch | Aid::Saves)
    }

    /// Its name in the picker, and the line under it.
    #[must_use]
    pub const fn label(self) -> (&'static str, &'static str) {
        match self {
            Aid::Map => (
                "Map as you explore",
                "fills in as you go; ` shows it full size",
            ),
            Aid::Seen => (
                "Who you've seen, on the map",
                "the characters, where you last saw them",
            ),
            Aid::Now => ("…and where they are now", "the ones who wander, live"),
            Aid::Objective => ("Objective", "gold, and what to buy next"),
            Aid::Hints => ("Rule hints", "a note when a rule applies"),
            Aid::Energy => ("Infinite energy", ""),
            Aid::Lives => ("Infinite lives", ""),
            Aid::NoWitch => ("No witch", "she never appears"),
            Aid::Saves => ("Saves", "F5 save, F9 restore, saved on quit"),
        }
    }

    /// Its command-line flag (#92, Decision 7).
    #[must_use]
    pub const fn flag(self) -> &'static str {
        match self {
            Aid::Map => "--map",
            Aid::Seen => "--map-seen",
            Aid::Now => "--map-now",
            Aid::Objective => "--objective",
            Aid::Hints => "--hints",
            Aid::Energy => "--infinite-energy",
            Aid::Lives => "--infinite-lives",
            Aid::NoWitch => "--no-witch",
            Aid::Saves => "--saves",
        }
    }
}

/// Which aids are on, and the picker.
#[derive(Clone, Debug, Default)]
pub struct Aids {
    on: [bool; ALL.len()],
    /// The picker, while it's open: which row has the focus.
    picker: Option<usize>,
    /// Whether the whole map is open, large (#96).
    full_map: bool,
    /// Counts every change, so the window redraws the panel only then.
    version: u64,
}

impl Aids {
    /// The aids named by the flags among `args`, which are taken out. A flag
    /// turns on what it depends on too.
    #[must_use]
    pub fn from_args(args: &mut Vec<String>) -> Aids {
        let mut aids = Aids::default();
        args.retain(|arg| match ALL.iter().find(|a| a.flag() == arg) {
            Some(&aid) => {
                aids.switch_on(aid);
                false
            }
            None => true,
        });
        aids
    }

    /// Whether `aid` is on: its own switch, and everything it depends on.
    #[must_use]
    pub fn is_on(&self, aid: Aid) -> bool {
        self.on[aid.index()] && aid.parent().is_none_or(|p| self.is_on(p))
    }

    /// Whether any aid is on, which decides what the panel shows.
    #[must_use]
    pub fn any(&self) -> bool {
        ALL.iter().any(|&a| self.is_on(a))
    }

    /// Whether `aid` can be switched: what it depends on is on.
    #[must_use]
    pub fn available(&self, aid: Aid) -> bool {
        aid.parent().is_none_or(|p| self.is_on(p))
    }

    fn switch_on(&mut self, aid: Aid) {
        if let Some(p) = aid.parent() {
            self.switch_on(p);
        }
        self.on[aid.index()] = true;
    }

    /// Switches `aid`, if it can be. Switching one off switches off what
    /// depends on it (#92, Decision 8).
    pub fn toggle(&mut self, aid: Aid) {
        if !self.available(aid) {
            return;
        }
        let on = !self.on[aid.index()];
        self.on[aid.index()] = on;
        if !on {
            for a in ALL {
                if !self.is_on(a) {
                    self.on[a.index()] = false;
                }
            }
        }
        self.full_map &= self.is_on(Aid::Map);
        self.version += 1;
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    // --- the picker ---------------------------------------------------------

    /// Whether the picker is open.
    #[must_use]
    pub const fn picker_open(&self) -> bool {
        self.picker.is_some()
    }

    /// Whether the whole map is open (#92, Decision 9).
    #[must_use]
    pub const fn full_map_open(&self) -> bool {
        self.full_map
    }

    /// Whether the game stands still: the picker or the whole map is open.
    #[must_use]
    pub const fn paused(&self) -> bool {
        self.picker.is_some() || self.full_map
    }

    /// Opens the whole map, or closes it, when the map is on.
    pub fn open_or_close_map(&mut self) {
        self.full_map = !self.full_map && self.is_on(Aid::Map);
        self.version += 1;
    }

    /// The aid with the picker's focus.
    #[must_use]
    pub fn focus(&self) -> Option<Aid> {
        self.picker.map(|n| ALL[n])
    }

    /// Opens the picker, or closes it. It closes the whole map.
    pub fn open_or_close(&mut self) {
        self.picker = match self.picker {
            Some(_) => None,
            None => Some(0),
        };
        self.full_map = false;
        self.version += 1;
    }

    /// Moves the focus up or down, past the switches that can't be switched
    /// now.
    pub fn move_focus(&mut self, down: bool) {
        let Some(mut n) = self.picker else {
            return;
        };
        loop {
            n = if down {
                (n + 1) % ALL.len()
            } else {
                (n + ALL.len() - 1) % ALL.len()
            };
            if self.available(ALL[n]) {
                break;
            }
        }
        self.picker = Some(n);
        self.version += 1;
    }

    /// Switches the focused aid.
    pub fn toggle_focus(&mut self) {
        if let Some(aid) = self.focus() {
            self.toggle(aid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_are_off_at_first() {
        let aids = Aids::default();
        assert!(!aids.any());
    }

    #[test]
    fn the_map_s_own_switches_need_the_map() {
        let mut aids = Aids::default();
        aids.toggle(Aid::Seen);
        assert!(!aids.is_on(Aid::Seen), "not without the map");
        aids.toggle(Aid::Map);
        aids.toggle(Aid::Seen);
        aids.toggle(Aid::Now);
        assert!(aids.is_on(Aid::Now));
        aids.toggle(Aid::Map);
        assert!(
            !aids.is_on(Aid::Seen) && !aids.is_on(Aid::Now),
            "off with it"
        );
        aids.toggle(Aid::Map);
        assert!(!aids.is_on(Aid::Seen), "and stay off when it's back");
    }

    #[test]
    fn a_flag_turns_on_what_it_needs_and_is_taken_out() {
        let mut args = vec!["tape.tzx".to_string(), "--map-now".into(), "--saves".into()];
        let aids = Aids::from_args(&mut args);
        assert_eq!(args, ["tape.tzx"]);
        assert!(aids.is_on(Aid::Map) && aids.is_on(Aid::Seen) && aids.is_on(Aid::Now));
        assert!(aids.is_on(Aid::Saves) && !aids.is_on(Aid::Objective));
    }

    #[test]
    fn the_whole_map_opens_only_with_the_map_on() {
        let mut aids = Aids::default();
        aids.open_or_close_map();
        assert!(!aids.full_map_open());
        aids.toggle(Aid::Map);
        aids.open_or_close_map();
        assert!(aids.full_map_open() && aids.paused());
        aids.open_or_close_map();
        assert!(!aids.paused());
    }

    #[test]
    fn the_focus_skips_what_can_t_be_switched() {
        let mut aids = Aids::default();
        aids.open_or_close();
        assert_eq!(aids.focus(), Some(Aid::Map));
        aids.move_focus(true);
        assert_eq!(aids.focus(), Some(Aid::Objective), "past the map's own");
        aids.move_focus(false);
        aids.toggle_focus();
        aids.move_focus(true);
        assert_eq!(aids.focus(), Some(Aid::Seen), "now that the map is on");
    }
}
