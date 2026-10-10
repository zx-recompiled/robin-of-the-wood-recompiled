//! Where Robin stands with the game's goal, for the aids that show it (#92,
//! #98): his gold, and what the trade has given him. Read from the game's
//! state, never changed (`docs/re/robin.md`, *Items*, *The trade*).

use crate::game::Game;

/// His inventory: eight slots, `0xFF` where empty.
const INVENTORY: u16 = 0xD472;
/// What he has: the sword, the bow, and how many of the three pieces.
const SWORD: u16 = 0xD47A;
const BOW: u16 = 0xD47B;
const PIECES: u16 = 0xD47E;
/// An item of kind 2 is a bag of gold, as the trade takes three.
const GOLD: u8 = 2;
/// What the trade takes each time, and how many trades there are: the
/// sword, the bow, and the three pieces.
pub const PER_TRADE: u8 = 3;
pub const TRADES: u8 = 5;

/// What the trade gives next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Sword,
    Bow,
    /// The piece with this number, from 1.
    Piece(u8),
    /// Nothing: he has everything.
    Done,
}

/// His gold and what he's been given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Objective {
    pub gold: u8,
    pub sword: bool,
    pub bow: bool,
    pub pieces: u8,
}

impl Objective {
    #[must_use]
    pub fn of(g: &Game) -> Objective {
        Objective {
            gold: (0..8).filter(|&n| g.read(INVENTORY + n) == GOLD).count() as u8,
            sword: g.read(SWORD) != 0,
            bow: g.read(BOW) != 0,
            pieces: g.read(PIECES).min(3),
        }
    }

    /// What the next trade gives, in the order the trade gives them.
    #[must_use]
    pub const fn next(&self) -> Next {
        if !self.sword {
            Next::Sword
        } else if !self.bow {
            Next::Bow
        } else if self.pieces < 3 {
            Next::Piece(self.pieces + 1)
        } else {
            Next::Done
        }
    }

    /// The trades made.
    #[must_use]
    pub const fn traded(&self) -> u8 {
        self.sword as u8 + self.bow as u8 + self.pieces
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::BANK;

    #[test]
    fn gold_is_counted_in_the_inventory_and_the_trades_come_in_order() {
        let mut g = Game::from_memory(&Box::new([[0u8; BANK]; 8]));
        for (n, kind) in [2, 5, 2, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
            .into_iter()
            .enumerate()
        {
            g.write(INVENTORY + n as u16, kind);
        }
        let o = Objective::of(&g);
        assert_eq!((o.gold, o.next()), (2, Next::Sword));
        g.write(SWORD, 1);
        g.write(BOW, 1);
        g.write(PIECES, 1);
        let o = Objective::of(&g);
        assert_eq!((o.next(), o.traded()), (Next::Piece(2), 3));
    }
}
