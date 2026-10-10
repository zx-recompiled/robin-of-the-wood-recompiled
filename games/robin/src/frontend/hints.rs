//! Rule hints (#92, #99): a note in the panel when one of the game's
//! unexplained rules applies where Robin is. Each is what the game's own
//! code does (`docs/re/robin.md`), not folklore, and each says when it
//! applies. The most specific one wins.

use robin::objective::{Next, Objective};
use robin::places::Character;

/// What the hints are chosen from.
pub struct Facts<'a> {
    pub here: Option<u16>,
    pub trade: u16,
    pub doorways: &'a [u16; 9],
    pub now: [Option<u16>; 3],
    pub hermit_met: bool,
    pub objective: Option<Objective>,
}

/// The hint for now, if one applies.
#[must_use]
pub fn hint(f: &Facts) -> Option<String> {
    let here = f.here?;
    let o = f.objective?;
    let at = |who: Character| f.now[who as usize] == Some(here);
    if f.doorways.contains(&here) {
        // The doorway's end (`0:DA04`), by the kind-5 items he carries.
        return Some(
            "The witch's doorway takes what you carry. Three flowers send you to the castle, \
             two restore your health and one is just taken. With none, she takes a bag of gold, \
             or with no gold, sends you to the dungeon."
                .into(),
        );
    }
    if here == f.trade && o.next() != Next::Done {
        // The trade (`0:DDEF`): three kind-2 items, when R comes up 0x13.
        return Some(if o.gold >= 3 {
            "You have the Ent's three bags: stay a moment, and the trade comes when it comes."
                .into()
        } else {
            format!(
                "The Ent trades three bags of gold, for the sword, then the bow, then each of \
                 three magic arrows. You have {}.",
                o.gold
            )
        });
    }
    if at(Character::Sheriff) {
        // The scene (`0xB723`) starts once Robin is in the screen's middle.
        return Some(
            "The Sheriff comes for you once you reach the middle of the screen, and takes you \
             to the dungeon."
                .into(),
        );
    }
    if at(Character::Bishop) {
        // The robbery: striking the first of the pair drops its gold at the
        // bishop's feet.
        return Some(
            "Strike the bishop's guard: the gold he carries drops where the bishop stands.".into(),
        );
    }
    if at(Character::Hermit) && !f.hermit_met {
        return Some("Meet the hermit: he gives you energy, once a game.".into());
    }
    if o.robbed >= 6 {
        // The pair come on only while the count is below six; each trade
        // lowers it by three.
        return Some(
            "The bishop has stopped coming: he's been robbed six times. He comes again once the \
             Ent has traded your gold."
                .into(),
        );
    }
    if o.full {
        return Some(
            "Your inventory is full: the next thing you pick up pushes the last one out, and \
             gold pushed out is dropped where you stand."
                .into(),
        );
    }
    if o.bow && o.arrows == 0 {
        return Some("Out of arrows: a quiver picked up gives you ten.".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(here: u16, objective: Objective) -> Facts<'static> {
        static DOORS: [u16; 9] = [0x10B, 0, 0, 0, 0, 0, 0, 0, 0];
        Facts {
            here: Some(here),
            trade: 0x109,
            doorways: &DOORS,
            now: [None; 3],
            hermit_met: false,
            objective: Some(objective),
        }
    }

    fn objective() -> Objective {
        Objective {
            gold: 1,
            sword: false,
            bow: false,
            pieces: 0,
            robbed: 0,
            full: false,
            arrows: 0,
        }
    }

    #[test]
    fn each_place_and_state_has_its_hint() {
        assert!(hint(&facts(0x10B, objective())).unwrap().contains("witch"));
        assert!(
            hint(&facts(0x109, objective()))
                .unwrap()
                .contains("You have 1.")
        );
        let rich = Objective {
            gold: 3,
            ..objective()
        };
        assert!(hint(&facts(0x109, rich)).unwrap().contains("stay a moment"));
        let mut f = facts(0x050, objective());
        f.now[Character::Bishop as usize] = Some(0x050);
        assert!(hint(&f).unwrap().contains("guard"));
        assert_eq!(hint(&facts(0x050, objective())), None, "nothing applies");
        let armed = Objective {
            bow: true,
            sword: true,
            ..objective()
        };
        assert!(
            hint(&facts(0x050, armed))
                .unwrap()
                .contains("Out of arrows")
        );
    }

    #[test]
    fn no_hint_outside_a_game() {
        let mut f = facts(0x10B, objective());
        f.here = None;
        assert_eq!(hint(&f), None);
    }
}
