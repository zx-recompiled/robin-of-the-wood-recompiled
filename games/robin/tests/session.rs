//! The rewrite played on its own, a frame at a time (#66): the menu, a game,
//! BREAK, the controls picked and the keys redefined, with presses held for
//! several frames, as hands hold them (starquake-recompiled#80), and a long
//! run under random held keys.
//!
//! Needs the supported tape in `assets/`.

mod common;

use robin::assets::Assets;
use robin::controls::Controls;
use robin::session::{Session, State};

fn tape() -> Option<Assets> {
    let tape = std::fs::read_dir(common::assets())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::read(e.path()).ok())
        .find(|b| robin::is_the_tape(b));
    let Some(tape) = tape else {
        println!("skipped: needs the supported tape in assets/");
        return None;
    };
    Some(robin::assets::read_tape(&tape).expect("the tape reads"))
}

/// The half-rows with `keys` held, each a half-row and a bit.
fn held(keys: &[(usize, u8)]) -> [u8; 8] {
    let mut rows = [0x1F; 8];
    for &(row, bit) in keys {
        rows[row] &= !(1 << bit);
    }
    rows
}

const NONE: [u8; 8] = [0x1F; 8];
const ZERO: (usize, u8) = (4, 0);
const ONE: (usize, u8) = (3, 0);
const TWO: (usize, u8) = (3, 1);
const CAPS: (usize, u8) = (0, 0);
const SPACE: (usize, u8) = (7, 0);

/// `n` frames with `keys` held.
fn hold(s: &mut Session, a: &Assets, keys: [u8; 8], n: usize) {
    for _ in 0..n {
        s.frame(a, controls(keys));
    }
}

fn controls(keys: [u8; 8]) -> Controls {
    Controls {
        keys,
        ..Controls::default()
    }
}

#[test]
fn zero_at_the_menu_starts_a_game_and_break_ends_it() {
    let Some(a) = tape() else { return };
    let mut s = Session::new(&a, 1);
    assert_eq!(s.state, State::Menu);
    hold(&mut s, &a, NONE, 50);
    assert_eq!(s.state, State::Menu, "nothing pressed, still the menu");
    hold(&mut s, &a, held(&[ZERO]), 5);
    assert_eq!(s.state, State::Playing);
    hold(&mut s, &a, NONE, 200);
    assert_eq!(s.state, State::Playing);
    hold(&mut s, &a, held(&[CAPS, SPACE]), 5);
    assert_eq!(s.state, State::Menu, "BREAK goes back to the menu");
}

#[test]
fn two_at_the_menu_picks_kempston() {
    let Some(a) = tape() else { return };
    let mut s = Session::new(&a, 2);
    hold(&mut s, &a, held(&[TWO]), 5);
    assert_eq!(s.state, State::Menu);
    assert_eq!(s.game.read(0xABEE), 1, "the method kept is Kempston");
}

#[test]
fn one_at_the_menu_redefines_five_keys_held_and_let_go() {
    let Some(a) = tape() else { return };
    let mut s = Session::new(&a, 3);
    hold(&mut s, &a, held(&[ONE]), 5);
    assert!(matches!(s.state, State::Redefining(_)));
    // Q, A, O, P, M: each held a few frames, then let go.
    let picks = [(2, 0), (1, 0), (5, 1), (5, 0), (7, 2)];
    for (n, &key) in picks.iter().enumerate() {
        hold(&mut s, &a, NONE, 5);
        hold(&mut s, &a, held(&[key]), 5);
        if n < 4 {
            assert!(matches!(s.state, State::Redefining(_)), "after key {n}");
        }
    }
    assert_eq!(s.state, State::Menu, "all five recorded");
    assert_eq!(s.game.read(0xABEE), 0, "the method kept is the keys");
    let codes: Vec<u8> = (0..5).map(|n| s.game.read(0xD154 + n)).collect();
    assert_eq!(codes, [0x25, 0x26, 0x1A, 0x22, 0x10], "Q, A, O, P, M");
}

#[test]
fn a_long_game_under_random_keys_plays_on() {
    let Some(a) = tape() else { return };
    let mut s = Session::new(&a, 4);
    hold(&mut s, &a, held(&[ZERO]), 5);
    let mut rng = common::XorShift(0x2545_F491);
    let ways = [(2, 0), (1, 0), (7, 3), (7, 2), (3, 0)];
    let mut keys = NONE;
    let (mut playing, mut seen) = (0, std::collections::BTreeSet::new());
    for frame in 0..20_000 {
        if frame % 15 == 0 {
            keys = NONE;
            for &w in &ways {
                if rng.next(3) == 0 {
                    keys = held(&[w]);
                }
            }
        }
        if s.state != State::Playing && frame % 200 == 0 {
            keys = held(&[ZERO]);
        }
        s.frame(&a, controls(keys));
        if s.state == State::Playing {
            playing += 1;
            seen.insert(s.game.map.location);
        }
    }
    println!("{playing} frames in play, {} locations", seen.len());
    assert!(playing > 10_000, "mostly in play: {playing}");
    assert!(seen.len() > 3, "Robin got about: {} locations", seen.len());
}

#[test]
fn the_menu_shows_on_a_black_border() {
    let Some(a) = tape() else { return };
    let mut s = Session::new(&a, 5);
    hold(&mut s, &a, NONE, 10);
    let picture = s.picture();
    let corner = picture[0];
    assert_eq!(corner, zx_core::screen::PALETTE[0], "the border is black");
    let lit = picture.iter().filter(|&&p| p != corner).count();
    assert!(lit > 1000, "the menu is drawn: {lit} pixels not black");
}
