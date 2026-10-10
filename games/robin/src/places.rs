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

/// The characters the map can show (#97), each named as players know it and
/// matched to the game by its sprite and what it does:
/// - **the bishop**: the fifth character's companion (`0xBA4F`), a figure
///   with a mitre, a crozier and a cross on its robe. He walks a route with
///   his guard, the first of the pair, whose gold drops where he stands
///   (*The fifth character*);
/// - **the Sheriff**: the scripted scene's character (`0xB7BD`), a burly
///   pointing figure, who takes Robin to the dungeon;
/// - **the hermit**: the wanderer (`0xBB1B`), a hooded figure in a robe, who
///   gives Robin energy once a game. Some tips call him the druid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Character {
    Bishop,
    Sheriff,
    Hermit,
}

pub const CHARACTERS: [Character; 3] = [Character::Bishop, Character::Sheriff, Character::Hermit];

/// The bishop's record, its location word at `+0x10`, and who of the pair is
/// out (bit 0, the bishop).
const BISHOP: u16 = 0xBA4F;
const OUT: u16 = 0xBA61;
/// Where the scene is, picked when a game starts and after it plays.
const SHERIFF_AT: u16 = 0xD295;
/// The wanderer's location word.
const HERMIT_AT: u16 = 0xBB26;

/// Where `who` is now, if he's anywhere: the bishop only while he's out.
#[must_use]
pub fn whereabouts(g: &Game, who: Character) -> Option<u16> {
    match who {
        Character::Bishop => (g.read(OUT) & 1 != 0).then(|| word(g, BISHOP + 0x10)),
        Character::Sheriff => Some(word(g, SHERIFF_AT)),
        Character::Hermit => Some(word(g, HERMIT_AT)),
    }
}

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

    #[test]
    fn the_bishop_is_somewhere_only_while_he_s_out() {
        let mut g = Game::from_memory(&Box::new([[0u8; BANK]; 8]));
        g.write(BISHOP + 0x10, 0x23);
        g.write(BISHOP + 0x11, 0x01);
        assert_eq!(whereabouts(&g, Character::Bishop), None);
        g.write(OUT, 1);
        assert_eq!(whereabouts(&g, Character::Bishop), Some(0x123));
    }
}
