//! Execution tracing for the recompiler's analysis pass.
//!
//! While the interpreter runs the game headless, this records which
//! addresses were executed, which addresses control arrived at by a jump,
//! call, return or interrupt (block entry points), and which instructions
//! were executed with different bytes at different times (self-modifying
//! code).
//!
//! Everything is recorded by where the byte is, not by the address it was
//! reached through: a page of memory (the ROM or a RAM bank) and the offset
//! in it, as [`Trace::at`] numbers them. On a 48K machine the four pages are
//! the four quarters of the address space, so the number is just the
//! address. On a 128K, code in different banks at the same address stays
//! apart.

use crate::memory::PAGE;

/// Which page each 16K slot of the address space shows, as
/// [`crate::memory::Memory::slots`] gives it.
pub type Slots = [usize; 4];

#[derive(Clone)]
pub struct Trace {
    /// Bytes of the instruction starting at each place, as first executed.
    pub executed: Vec<Option<(u8, [u8; 4])>>,
    /// Instruction starts whose bytes changed between executions.
    pub self_modified: Vec<bool>,
    /// Places reached other than by falling through from the previous instruction.
    pub entries: Vec<bool>,
    /// Where the next instruction is expected if execution falls through.
    pub fallthrough: Option<usize>,
    /// Places that are part of some executed instruction.
    pub code: Vec<bool>,
    /// Code bytes written after they were executed.
    pub written_code: Vec<bool>,
}

impl Default for Trace {
    /// A trace for a 48K machine.
    fn default() -> Self {
        Trace::new(4)
    }
}

impl Trace {
    /// A trace for a machine with `pages` pages of memory: 4 on a 48K, 10 on
    /// a 128K.
    #[must_use]
    pub fn new(pages: usize) -> Trace {
        let n = pages * PAGE;
        Trace {
            executed: vec![None; n],
            self_modified: vec![false; n],
            entries: vec![false; n],
            fallthrough: None,
            code: vec![false; n],
            written_code: vec![false; n],
        }
    }

    /// Where `addr` is, with `slots` paged: the index every table uses.
    #[must_use]
    pub fn at(slots: &Slots, addr: u16) -> usize {
        slots[usize::from(addr >> 14)] * PAGE + usize::from(addr) % PAGE
    }

    /// Records the instruction at `pc`: its `len` bytes, the first of the four
    /// `bytes` read from there.
    pub fn on_exec(&mut self, slots: &Slots, pc: u16, bytes: [u8; 4], len: u8) {
        let here = Trace::at(slots, pc);
        if self.fallthrough != Some(here) {
            self.entries[here] = true;
        }
        self.fallthrough = Some(Trace::at(slots, pc.wrapping_add(u16::from(len))));

        let mut bytes = bytes;
        bytes[len as usize..].fill(0);
        match &self.executed[here] {
            Some(prev) if *prev != (len, bytes) => self.self_modified[here] = true,
            Some(_) => {}
            None => {
                self.executed[here] = Some((len, bytes));
                // An instruction can run across a slot boundary, into
                // another page.
                for i in 0..u16::from(len) {
                    self.code[Trace::at(slots, pc.wrapping_add(i))] = true;
                }
            }
        }
    }

    /// Called when control is transferred by something other than an
    /// instruction (an interrupt).
    pub fn on_interrupt(&mut self) {
        self.fallthrough = None;
    }

    pub fn on_write(&mut self, slots: &Slots, addr: u16) {
        let at = Trace::at(slots, addr);
        if self.code[at] {
            self.written_code[at] = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_a_48k_a_place_is_its_address() {
        let slots = [0, 1, 2, 3];
        for addr in [0x0000u16, 0x3FFF, 0x4000, 0x8123, 0xFFFF] {
            assert_eq!(Trace::at(&slots, addr), usize::from(addr));
        }
    }

    #[test]
    fn the_same_address_in_two_banks_is_two_places() {
        // A 128K with bank 0 at 0xC000, then bank 6 (pages 2 + bank).
        let (bank0, bank6) = ([1, 7, 4, 2], [1, 7, 4, 8]);
        let mut t = Trace::new(10);
        t.on_exec(&bank0, 0xC000, [0x00, 0, 0, 0], 1);
        t.on_exec(&bank6, 0xC000, [0xC9, 0, 0, 0], 1);
        assert_eq!(
            t.executed[Trace::at(&bank0, 0xC000)],
            Some((1, [0x00, 0, 0, 0]))
        );
        assert_eq!(
            t.executed[Trace::at(&bank6, 0xC000)],
            Some((1, [0xC9, 0, 0, 0]))
        );
        assert!(
            !t.self_modified.iter().any(|&b| b),
            "two places, not one modified"
        );
    }

    #[test]
    fn an_instruction_across_a_slot_boundary_marks_both_pages() {
        let slots = [1, 7, 4, 8];
        let mut t = Trace::new(10);
        t.on_exec(&slots, 0xBFFF, [0x21, 0x34, 0x12, 0], 3);
        assert!(t.code[Trace::at(&slots, 0xBFFF)]);
        assert!(t.code[Trace::at(&slots, 0xC000)]);
        assert!(t.code[Trace::at(&slots, 0xC001)]);
        assert_eq!(Trace::at(&slots, 0xC000), 8 * PAGE);
    }
}
