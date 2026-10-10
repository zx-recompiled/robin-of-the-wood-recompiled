//! The host keyboard to the Spectrum's half-rows, and the arrows to a
//! Kempston joystick, as starquake-recompiled maps them (`REUSED.md`).

use std::collections::HashSet;

use robin::controls::Controls;
use winit::keyboard::KeyCode;

/// The Spectrum keys (half-row, bit) a host key presses.
#[allow(
    clippy::match_same_arms,
    reason = "the arms are the keyboard's own layout"
)]
fn matrix(key: KeyCode) -> &'static [(usize, u8)] {
    use KeyCode::*;
    match key {
        ShiftLeft | ShiftRight => &[(0, 0)],
        KeyZ => &[(0, 1)],
        KeyX => &[(0, 2)],
        KeyC => &[(0, 3)],
        KeyV => &[(0, 4)],
        KeyA => &[(1, 0)],
        KeyS => &[(1, 1)],
        KeyD => &[(1, 2)],
        KeyF => &[(1, 3)],
        KeyG => &[(1, 4)],
        KeyQ => &[(2, 0)],
        KeyW => &[(2, 1)],
        KeyE => &[(2, 2)],
        KeyR => &[(2, 3)],
        KeyT => &[(2, 4)],
        Digit1 => &[(3, 0)],
        Digit2 => &[(3, 1)],
        Digit3 => &[(3, 2)],
        Digit4 => &[(3, 3)],
        Digit5 => &[(3, 4)],
        Digit0 => &[(4, 0)],
        Digit9 => &[(4, 1)],
        Digit8 => &[(4, 2)],
        Digit7 => &[(4, 3)],
        Digit6 => &[(4, 4)],
        KeyP => &[(5, 0)],
        KeyO => &[(5, 1)],
        KeyI => &[(5, 2)],
        KeyU => &[(5, 3)],
        KeyY => &[(5, 4)],
        Enter => &[(6, 0)],
        KeyL => &[(6, 1)],
        KeyK => &[(6, 2)],
        KeyJ => &[(6, 3)],
        KeyH => &[(6, 4)],
        Space => &[(7, 0)],
        ControlLeft | ControlRight => &[(7, 1)],
        KeyM => &[(7, 2)],
        KeyN => &[(7, 3)],
        KeyB => &[(7, 4)],
        // BREAK, Caps Shift with Space, which starts a new game.
        Escape => &[(0, 0), (7, 0)],
        _ => &[],
    }
}

/// The Kempston joystick's bit a host key moves.
fn kempston(key: KeyCode) -> u8 {
    use KeyCode::*;
    match key {
        ArrowRight => 0x01,
        ArrowLeft => 0x02,
        ArrowDown => 0x04,
        ArrowUp => 0x08,
        AltLeft | AltRight | Period | Comma => 0x10,
        _ => 0,
    }
}

/// The controls with the host keys `held` down, built from the whole set,
/// so two host keys on one Spectrum key let go of it only when both are.
#[must_use]
pub fn build(held: &HashSet<KeyCode>) -> Controls {
    let mut c = Controls::default();
    for &key in held {
        for &(row, bit) in matrix(key) {
            c.keys[row] &= !(1 << bit);
        }
        c.kempston |= kempston(key);
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_is_break_and_the_arrows_a_joystick() {
        let held: HashSet<KeyCode> = [KeyCode::Escape, KeyCode::ArrowLeft].into();
        let c = build(&held);
        assert_eq!(c.keys[0], 0x1E, "Caps Shift");
        assert_eq!(c.keys[7], 0x1E, "Space");
        assert_eq!(c.kempston, 0x02, "left");
    }
}
