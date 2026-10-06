//! The 128K's AY-3-8912 sound chip, as the processor sees it: sixteen
//! registers behind two ports. No sound is made here; the reference machine
//! only needs what the program writes and what it reads back. The sound is
//! the frontend's.
//!
//! From General Instrument's AY-3-8910/8912 data sheet: **read**, not yet
//! checked against a real chip.

/// Bits each register keeps. Tone periods' coarse halves, the noise period,
/// the three volumes and the envelope shape are narrower than a byte, and
/// read back with the unused bits clear.
const MASK: [u8; 16] = [
    0xFF, 0x0F, 0xFF, 0x0F, 0xFF, 0x0F, // tone periods A, B, C: fine, coarse
    0x1F, // noise period
    0xFF, // mixer and I/O direction
    0x1F, 0x1F, 0x1F, // volumes A, B, C (bit 4: envelope)
    0xFF, 0xFF, // envelope period
    0x0F, // envelope shape
    0xFF, 0xFF, // I/O ports A and B
];

/// One write the program made, and when.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AyWrite {
    pub frame: u64,
    /// T-state within the frame, as the `OUT` that made it finished.
    pub t: u32,
    pub reg: u8,
    pub value: u8,
}

#[derive(Clone, Default)]
pub struct Ay {
    regs: [u8; 16],
    /// The register address latch, as last written to `0xFFFD`.
    latch: u8,
    /// Every write with its time, when something wants them (the sound
    /// checks); `None` otherwise, so a long run does not grow it unread.
    pub log: Option<Vec<AyWrite>>,
}

impl Ay {
    /// Selects a register (a write to `0xFFFD`). The latch takes the whole
    /// byte: the top four bits are the chip's address, which is 0, so a value
    /// of 16 or more selects nothing.
    pub fn select(&mut self, v: u8) {
        self.latch = v;
    }

    /// The selected register, or none.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        (self.latch < 16).then_some(usize::from(self.latch))
    }

    /// Writes the selected register (a write to `0xBFFD`), keeping the bits it
    /// has.
    pub fn write(&mut self, v: u8, frame: u64, t: u32) {
        let Some(reg) = self.selected() else {
            return;
        };
        self.regs[reg] = v & MASK[reg];
        if let Some(log) = &mut self.log {
            log.push(AyWrite {
                frame,
                t,
                reg: reg as u8,
                value: v,
            });
        }
    }

    /// Reads the selected register (a read of `0xFFFD`). An I/O port set as
    /// an input (bit 6 or 7 of the mixer clear) reads its lines, which
    /// nothing drives here, so they read high.
    #[must_use]
    pub fn read(&self) -> u8 {
        match self.selected() {
            None => 0xFF,
            Some(14) if self.regs[7] & 0x40 == 0 => 0xFF,
            Some(15) if self.regs[7] & 0x80 == 0 => 0xFF,
            Some(reg) => self.regs[reg],
        }
    }

    /// A register's value, for a check to compare.
    #[must_use]
    pub fn reg(&self, reg: usize) -> u8 {
        self.regs[reg]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_keep_only_their_bits() {
        let mut ay = Ay::default();
        for reg in 0..14 {
            ay.select(reg);
            ay.write(0xFF, 0, 0);
            assert_eq!(ay.read(), MASK[usize::from(reg)], "register {reg}");
        }
    }

    #[test]
    fn a_latch_of_16_or_more_selects_nothing() {
        let mut ay = Ay::default();
        ay.select(3);
        ay.write(0x05, 0, 0);
        ay.select(0x13);
        ay.write(0x0A, 0, 0);
        assert_eq!(ay.read(), 0xFF);
        ay.select(3);
        assert_eq!(ay.read(), 0x05);
    }

    #[test]
    fn io_ports_read_high_as_inputs_and_back_as_outputs() {
        let mut ay = Ay::default();
        ay.select(14);
        ay.write(0x12, 0, 0);
        assert_eq!(ay.read(), 0xFF);
        ay.select(7);
        ay.write(0x40, 0, 0);
        ay.select(14);
        assert_eq!(ay.read(), 0x12);
    }

    #[test]
    fn writes_are_logged_with_their_time_only_when_asked() {
        let mut ay = Ay::default();
        ay.select(8);
        ay.write(0x0F, 3, 100);
        assert!(ay.log.is_none());
        ay.log = Some(Vec::new());
        ay.write(0x3F, 4, 200);
        assert_eq!(
            ay.log.as_deref(),
            Some(
                &[AyWrite {
                    frame: 4,
                    t: 200,
                    reg: 8,
                    value: 0x3F
                }][..]
            )
        );
    }
}
