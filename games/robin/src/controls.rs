//! What the player is pressing, as the original reads it: the keyboard's
//! eight half-rows and the Kempston joystick's port. The frontend sets these
//! from the player's keys or joystick; the game reads them through ports,
//! as the original does (`docs/re/robin.md`, *The controls*).

/// The keyboard and joystick as the ports show them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Controls {
    /// One byte a half-row, for address lines A8 to A15: bits 0–4 the keys,
    /// 0 when pressed.
    pub keys: [u8; 8],
    /// The Kempston port: bit 0 right, 1 left, 2 down, 3 up, 4 fire, 1 when
    /// pressed.
    pub kempston: u8,
    /// The level the ULA's port shows in bit 6.
    pub ear: bool,
}

impl Default for Controls {
    /// Nothing pressed.
    fn default() -> Controls {
        Controls {
            keys: [0x1F; 8],
            kempston: 0,
            ear: false,
        }
    }
}

impl Controls {
    /// The byte a read of `port` gives: the ULA's port (bit 0 clear), with
    /// the half-rows whose address lines are low ANDed together, or the
    /// Kempston port, or nothing.
    #[must_use]
    pub fn read(&self, port: u16) -> u8 {
        if port & 1 == 0 {
            let high = (port >> 8) as u8;
            let keys = (0..8)
                .filter(|row| high & (1 << row) == 0)
                .fold(0x1F, |k, row| k & self.keys[row]);
            0xA0 | if self.ear { 0x40 } else { 0 } | keys
        } else if port & 0x20 == 0 {
            self.kempston
        } else {
            0xFF
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_read_as_the_machine_answers() {
        let mut c = Controls::default();
        assert_eq!(c.read(0xFEFE), 0xBF);
        c.keys[1] = 0x1E; // A, in the half-row at A9.
        assert_eq!(c.read(0xFDFE), 0xBE);
        assert_eq!(c.read(0x00FE), 0xBE, "every half-row at once");
        assert_eq!(c.read(0xFEFE), 0xBF, "another half-row");
        c.kempston = 0x11;
        assert_eq!(c.read(0x001F), 0x11);
        c.ear = true;
        assert_eq!(c.read(0xFEFE), 0xFF);
    }
}
