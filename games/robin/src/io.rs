//! What a routine exchanges with the world outside the game's memory: the
//! controls and random numbers it reads, the AY's registers, which it can
//! read back, and the ports it writes.
//!
//! The original takes its random numbers from the refresh register R, which
//! counts the instructions the processor has fetched, so a rewrite cannot
//! reproduce them (`README.md`, *Status*; #37). In the checks, a routine is
//! given the very values the original read, in order. The game itself will
//! draw its own.

use crate::controls::Controls;

/// Random numbers, drawn in order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Random {
    values: Vec<u8>,
    drawn: usize,
}

impl Random {
    /// The values a routine is to draw, in order: in the checks, those the
    /// original read from R.
    #[must_use]
    pub fn given(values: Vec<u8>) -> Random {
        Random { values, drawn: 0 }
    }

    /// The next value, as the original's `LD A,R` gives it.
    ///
    /// # Panics
    ///
    /// If more are drawn than were given: the rewrite read R where the
    /// original did not.
    pub fn r(&mut self) -> u8 {
        let v = *self.values.get(self.drawn).unwrap_or_else(|| {
            panic!(
                "drew random value {} where the original read {}",
                self.drawn + 1,
                self.values.len()
            )
        });
        self.drawn += 1;
        v
    }

    /// How many were given and not drawn.
    #[must_use]
    pub fn left(&self) -> usize {
        self.values.len() - self.drawn
    }
}

/// The AY sound chip's registers, as the game has set them: what a read of
/// them gives back. The sound itself is the frontend's (#7).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ay {
    pub regs: [u8; 16],
    /// The register selected through `0xFFFD`; 16 or more selects none.
    pub latch: u8,
}

/// The bits each register keeps.
const AY_BITS: [u8; 16] = [
    0xFF, 0x0F, 0xFF, 0x0F, 0xFF, 0x0F, 0x1F, 0xFF, 0x1F, 0x1F, 0x1F, 0xFF, 0xFF, 0x0F, 0xFF, 0xFF,
];

impl Default for Ay {
    fn default() -> Ay {
        Ay {
            regs: [0; 16],
            latch: 0xFF,
        }
    }
}

impl Ay {
    /// Whether `port` is the AY's: `0xFFFD` selects a register and reads it,
    /// `0xBFFD` writes it, decoded from A15, A14 and A1.
    fn decode(port: u16) -> Option<bool> {
        match port & 0xC002 {
            0xC000 => Some(true),
            0x8000 => Some(false),
            _ => None,
        }
    }

    /// The selected register. An I/O port set as an input reads its lines,
    /// which nothing drives, so they read high.
    #[must_use]
    pub fn read(&self) -> u8 {
        match self.latch {
            14 if self.regs[7] & 0x40 == 0 => 0xFF,
            15 if self.regs[7] & 0x80 == 0 => 0xFF,
            r @ 0..16 => self.regs[usize::from(r)],
            _ => 0xFF,
        }
    }

    fn write(&mut self, v: u8) {
        if let Some(r) = self.regs.get_mut(usize::from(self.latch)) {
            *r = v & AY_BITS[usize::from(self.latch)];
        }
    }
}

/// Everything a routine exchanges with outside the game's memory.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Io {
    pub controls: Controls,
    pub random: Random,
    pub ay: Ay,
    /// Every port written, and the value, in order.
    pub writes: Vec<(u16, u8)>,
}

impl Io {
    /// What a read of `port` gives: the AY's selected register, or the
    /// controls.
    #[must_use]
    pub fn input(&self, port: u16) -> u8 {
        if port & 1 != 0 && Ay::decode(port) == Some(true) {
            self.ay.read()
        } else {
            self.controls.read(port)
        }
    }

    /// Writes `v` to `port`: kept, in order, and passed to the AY if it's
    /// one of its ports.
    pub fn out(&mut self, port: u16, v: u8) {
        self.writes.push((port, v));
        match Ay::decode(port) {
            Some(true) => self.ay.latch = v,
            Some(false) => self.ay.write(v),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ay_reads_back_what_was_written_and_writes_are_kept() {
        let mut io = Io::default();
        io.out(0xFFFD, 7);
        io.out(0xBFFD, 0xBF);
        io.out(0xFFFD, 1);
        io.out(0xBFFD, 0xFF);
        assert_eq!(io.input(0xFFFD), 0x0F, "a coarse period keeps four bits");
        io.out(0xFFFD, 14);
        assert_eq!(
            io.input(0xFFFD),
            0xFF,
            "port A is an input: nothing drives it"
        );
        io.out(0xFFFD, 7);
        assert_eq!(io.input(0xFFFD), 0xBF);
        assert_eq!(io.writes.len(), 6);
        assert_eq!(io.writes[1], (0xBFFD, 0xBF));
    }

    #[test]
    fn values_are_drawn_in_order_and_no_more() {
        let mut r = Random::given(vec![3, 1]);
        assert_eq!((r.r(), r.left()), (3, 1));
        assert_eq!((r.r(), r.left()), (1, 0));
        assert!(std::panic::catch_unwind(move || r.r()).is_err());
    }
}
