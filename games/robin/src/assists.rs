//! The assists (#92, #100), applied around the game after each of its
//! frames, never inside its routines, so no differential suite can see them.
//! The players' words differ from the game's:
//!
//! - **Infinite energy** is the game's health (`0xBE47`), which the lower
//!   panel's colour shows: a hit takes 1, and at 2 he's knocked down
//!   (*Fighting*, *Robin hit*). It's kept full.
//! - **Infinite lives** is the game's energy (`0xD481`), the digit on the
//!   panel: a knock-down takes 1 in all, and below 0 the game is over
//!   (*Robin's actions*, *Knocked down*). It's kept from going below 0.
//! - **No witch**: the witch is the nine doorways (`0xDAB9`). While it's on,
//!   their table is set aside and none of them matches a location; when it's
//!   off again, the table is put back.

use crate::game::Game;

/// His health when full, as a doorway restores it.
const FULL_HEALTH: u8 = 0x0F;
/// The doorways' table: nine location words.
const DOORWAYS: u16 = 0xDAB9;
/// A location no doorway can be at.
const NOWHERE: u16 = 0xFFFF;

/// Which assists are on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Assists {
    pub energy: bool,
    pub lives: bool,
    pub no_witch: bool,
}

/// What the assists have set aside, to put back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SetAside {
    doorways: Option<[u16; 9]>,
}

impl SetAside {
    /// The doorways' table as the game had it, if it's set aside now.
    #[must_use]
    pub const fn doorways(&self) -> Option<[u16; 9]> {
        self.doorways
    }
}

fn word(g: &Game, at: u16) -> u16 {
    u16::from_le_bytes([g.read(at), g.read(at.wrapping_add(1))])
}

fn set_word(g: &mut Game, at: u16, v: u16) {
    let [lo, hi] = v.to_le_bytes();
    g.write(at, lo);
    g.write(at.wrapping_add(1), hi);
}

/// Applies `on` to the game after a frame of play. `kept` holds what's
/// been set aside, so switching an assist off puts it back.
pub fn apply(g: &mut Game, on: Assists, kept: &mut SetAside) {
    if on.energy && g.fighting.health < FULL_HEALTH {
        g.fighting.health = FULL_HEALTH;
        crate::actions::panel_colours(g);
    }
    if on.lives && g.robin.energy & 0x80 != 0 {
        g.robin.energy = 0;
        crate::actions::show_energy(g);
    }
    match (on.no_witch, kept.doorways) {
        (true, None) => {
            let table = std::array::from_fn(|n| word(g, DOORWAYS + n as u16 * 2));
            for n in 0..9 {
                set_word(g, DOORWAYS + n * 2, NOWHERE);
            }
            kept.doorways = Some(table);
        }
        (false, Some(table)) => {
            for (n, at) in table.into_iter().enumerate() {
                set_word(g, DOORWAYS + n as u16 * 2, at);
            }
            kept.doorways = None;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::BANK;

    fn game() -> Game {
        Game::from_memory(&Box::new([[0u8; BANK]; 8]))
    }

    #[test]
    fn energy_is_kept_full_and_lives_from_running_out() {
        let mut g = game();
        g.fighting.health = 5;
        g.robin.energy = 0xFF;
        let mut kept = SetAside::default();
        apply(&mut g, Assists::default(), &mut kept);
        assert_eq!(
            (g.fighting.health, g.robin.energy),
            (5, 0xFF),
            "nothing when off"
        );
        let on = Assists {
            energy: true,
            lives: true,
            no_witch: false,
        };
        apply(&mut g, on, &mut kept);
        assert_eq!((g.fighting.health, g.robin.energy), (FULL_HEALTH, 0));
    }

    #[test]
    fn the_witch_s_doorways_are_set_aside_and_put_back() {
        let mut g = game();
        set_word(&mut g, DOORWAYS + 2, 0x10B);
        let mut kept = SetAside::default();
        let no_witch = Assists {
            no_witch: true,
            ..Assists::default()
        };
        apply(&mut g, no_witch, &mut kept);
        assert_eq!(word(&g, DOORWAYS + 2), NOWHERE);
        assert_eq!(kept.doorways().map(|t| t[1]), Some(0x10B));
        apply(&mut g, no_witch, &mut kept);
        assert_eq!(kept.doorways().map(|t| t[1]), Some(0x10B), "set aside once");
        apply(&mut g, Assists::default(), &mut kept);
        assert_eq!(word(&g, DOORWAYS + 2), 0x10B);
        assert_eq!(kept.doorways(), None);
    }
}
