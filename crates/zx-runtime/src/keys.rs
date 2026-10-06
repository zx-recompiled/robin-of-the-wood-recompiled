//! Spectrum keyboard matrix and Kempston joystick.

use crate::machine::Zx;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Keyboard matrix position: half-row (address line A8 + row) and bit.
    Matrix(u8, u8),
    /// Kempston joystick bit.
    Kempston(u8),
}

/// Half-rows in port order, bit 0 first.
const ROWS: [[&str; 5]; 8] = [
    ["caps", "z", "x", "c", "v"],
    ["a", "s", "d", "f", "g"],
    ["q", "w", "e", "r", "t"],
    ["1", "2", "3", "4", "5"],
    ["0", "9", "8", "7", "6"],
    ["p", "o", "i", "u", "y"],
    ["enter", "l", "k", "j", "h"],
    ["space", "symbol", "m", "n", "b"],
];

const KEMPSTON: [&str; 5] = ["joy_right", "joy_left", "joy_down", "joy_up", "joy_fire"];

impl Key {
    /// Looks a key up by name: `"a"`..`"z"`, `"0"`..`"9"`, `"enter"`,
    /// `"space"`, `"caps"`, `"symbol"`, or `"joy_up"`/`down`/`left`/`right`/`fire`.
    pub fn by_name(name: &str) -> Option<Key> {
        let name = name.to_ascii_lowercase();
        for (row, keys) in ROWS.iter().enumerate() {
            if let Some(bit) = keys.iter().position(|k| *k == name) {
                return Some(Key::Matrix(row as u8, bit as u8));
            }
        }
        KEMPSTON
            .iter()
            .position(|k| *k == name)
            .map(|bit| Key::Kempston(bit as u8))
    }
}

impl Zx {
    pub fn release_all_keys(&mut self) {
        self.keys = [0xFF; 8];
        self.kempston = 0;
    }

    pub fn set_key(&mut self, key: Key, pressed: bool) {
        match key {
            // The matrix is 8 half-rows of 5 keys; anything else is not a
            // key on this machine. `by_name` never yields one, but the type
            // is public and a configurable keymap could.
            Key::Matrix(row, bit) if row < 8 && bit < 5 => {
                let mask = 1 << bit;
                if pressed {
                    self.keys[row as usize] &= !mask;
                } else {
                    self.keys[row as usize] |= mask;
                }
            }
            Key::Matrix(..) => {}
            Key::Kempston(bit) => {
                let mask = 1 << bit;
                if pressed {
                    self.kempston |= mask;
                } else {
                    self.kempston &= !mask;
                }
            }
        }
    }
}
