//! The forest's places that matter to a player, as the game keeps them, for
//! the aids that show them (#92, #96). Nothing here changes the game: each
//! is read from its state (`docs/re/robin.md`, *The map*, *Trades and
//! journeys*, *The ending*).

use crate::game::Game;

/// The trade's location, set when a game starts (`0xD28F`, one of four):
/// where three gold buys the sword, the bow, then each piece. The players'
/// Ent.
const TRADE_AT: u16 = 0xD28F;
/// The nine doorways (`0xDAB9`), where what Robin carries decides where he's
/// sent: the players' witch.
const DOORWAYS: u16 = 0xDAB9;

/// Where a doorway sends Robin with three kind-5 items (the players'
/// flowers): the players' castle.
pub const CASTLE: u16 = 0xCC;
/// Where a doorway sends him with none of kind 5 or 2, and where the scene's
/// end takes him: the players' dungeon.
pub const DUNGEON: u16 = 0x9C;
/// The one location entered by its own path, the ending: the players'
/// tournament.
pub const ENDING: u16 = 0x69;

fn word(g: &Game, at: u16) -> u16 {
    u16::from_le_bytes([g.read(at), g.read(at.wrapping_add(1))])
}

/// The trade's location.
#[must_use]
pub fn trade(g: &Game) -> u16 {
    word(g, TRADE_AT)
}

/// The nine doorways' locations.
#[must_use]
pub fn doorways(g: &Game) -> [u16; 9] {
    std::array::from_fn(|n| word(g, DOORWAYS + n as u16 * 2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::BANK;

    #[test]
    fn the_places_are_read_where_the_game_keeps_them() {
        let mut g = Game::from_memory(&Box::new([[0u8; BANK]; 8]));
        g.map.specials[0] = 0x109;
        g.write(DOORWAYS + 2, 0x0B);
        g.write(DOORWAYS + 3, 0x01);
        assert_eq!(trade(&g), 0x109, "where the new game put the trade");
        assert_eq!(doorways(&g)[1], 0x10B);
    }
}
