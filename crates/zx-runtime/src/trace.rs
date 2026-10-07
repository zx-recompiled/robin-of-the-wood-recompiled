//! Execution tracing for the recompiler's analysis pass.
//!
//! While the interpreter runs the game headless, this records which
//! addresses were executed, which addresses control arrived at by a jump,
//! call, return or interrupt (block entry points), and which instructions
//! were executed with different bytes at different times (self-modifying
//! code).
//!
//! An interrupt's return to where it interrupted is not an entry: the code
//! there was only paused. Nor is a repeating block instruction (`LDIR` and
//! the like) running again at its own address. An unconditional jump's
//! destination always is, even when it is the next instruction.
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
    /// Where the next instruction is expected if execution falls through;
    /// `None` after an unconditional jump or an interrupt.
    pub fallthrough: Option<usize>,
    /// Places that are part of some executed instruction.
    pub code: Vec<bool>,
    /// Code bytes written after they were executed.
    pub written_code: Vec<bool>,
    /// The interrupts not yet returned from, innermost last.
    interrupted: Vec<Interrupted>,
    /// The stack pointer of a return the last instruction made, if it was one.
    returning: Option<u16>,
    /// The place of the last instruction, if it was a repeating block
    /// instruction, which runs again there until it is done.
    repeating: Option<usize>,
}

/// An interrupt the handler has not yet returned from.
#[derive(Clone, Copy)]
struct Interrupted {
    /// Where the return address was pushed.
    sp: u16,
    /// The place the handler returns to, if arriving there would have
    /// continued the interrupted code; `None` if it was an entry anyway (the
    /// interrupt came right after a jump).
    resume: Option<usize>,
}

/// How many unreturned interrupts are remembered. A handler that never
/// returns, on a stack of its own, would otherwise grow the list forever; a
/// forgotten one only costs a label where it returns.
const MAX_INTERRUPTED: usize = 16;

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
            interrupted: Vec::new(),
            returning: None,
            repeating: None,
        }
    }

    /// Where `addr` is, with `slots` paged: the index every table uses.
    #[must_use]
    pub fn at(slots: &Slots, addr: u16) -> usize {
        slots[usize::from(addr >> 14)] * PAGE + usize::from(addr) % PAGE
    }

    /// Records the instruction at `pc`: its `len` bytes, the first of the four
    /// `bytes` read from there, run with the stack pointer at `sp`.
    pub fn on_exec(&mut self, slots: &Slots, pc: u16, bytes: [u8; 4], len: u8, sp: u16) {
        let here = Trace::at(slots, pc);
        if !self.continues(here, sp) {
            self.entries[here] = true;
        }
        // An unconditional jump arrives at a branch target even when that is
        // the next instruction: code that sets a `JR`'s offset to 0 to skip
        // nothing.
        self.fallthrough =
            (!is_jump(bytes)).then(|| Trace::at(slots, pc.wrapping_add(u16::from(len))));
        self.returning = is_return(bytes).then_some(sp);
        self.repeating = is_block_repeat(bytes).then_some(here);

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

    /// Called when an interrupt is accepted, before it pushes `pc` (the
    /// address it returns to) with the stack pointer at `sp`.
    pub fn on_interrupt(&mut self, slots: &Slots, pc: u16, sp: u16) {
        let back = Trace::at(slots, pc);
        let resume = self.continues(back, sp).then_some(back);
        if self.interrupted.len() == MAX_INTERRUPTED {
            self.interrupted.remove(0);
        }
        self.interrupted.push(Interrupted {
            sp: sp.wrapping_sub(2),
            resume,
        });
        self.fallthrough = None;
    }

    /// Whether arriving at `here`, with the stack pointer at `sp`, carries on
    /// from the last instruction rather than entering somewhere new: it falls
    /// through to it, repeats a block instruction, or is the return from an
    /// interrupt to where it interrupted.
    fn continues(&mut self, here: usize, sp: u16) -> bool {
        let mut expected = self.fallthrough;
        // A return that popped the innermost interrupt's return address goes
        // back to where it interrupted, and to nowhere else.
        if let Some(ret_sp) = self.returning.take() {
            let top = self.interrupted.last();
            if sp == ret_sp.wrapping_add(2) && top.is_some_and(|i| i.sp == ret_sp) {
                expected = top.and_then(|i| i.resume);
            }
        }
        // An interrupt whose return address is above the stack has been
        // returned from or abandoned.
        self.interrupted.retain(|i| i.sp >= sp);
        let repeated = self.repeating.take() == Some(here);
        repeated || expected == Some(here)
    }

    pub fn on_write(&mut self, slots: &Slots, addr: u16) {
        let at = Trace::at(slots, addr);
        if self.code[at] {
            self.written_code[at] = true;
        }
    }
}

/// Whether the instruction is a return: `RET`, `RET cc`, `RETI` or `RETN`.
fn is_return(bytes: [u8; 4]) -> bool {
    match bytes {
        [0xC9, ..] => true,
        [op, ..] if op & 0xC7 == 0xC0 => true,
        [0xED, op, ..] => op & 0xC7 == 0x45,
        _ => false,
    }
}

/// Whether the instruction is an unconditional jump: `JR`, `JP` or `JP (HL)`,
/// `(IX)` or `(IY)`.
fn is_jump(bytes: [u8; 4]) -> bool {
    matches!(bytes, [0x18 | 0xC3 | 0xE9, ..] | [0xDD | 0xFD, 0xE9, ..])
}

/// Whether the instruction is a block instruction that repeats: `LDIR`,
/// `CPIR`, `INIR`, `OTIR` and their decrementing twins.
fn is_block_repeat(bytes: [u8; 4]) -> bool {
    matches!(bytes, [0xED, 0xB0..=0xB3 | 0xB8..=0xBB, ..])
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
        t.on_exec(&bank0, 0xC000, [0x00, 0, 0, 0], 1, 0xFF00);
        t.on_exec(&bank6, 0xC000, [0xC9, 0, 0, 0], 1, 0xFF00);
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
        t.on_exec(&slots, 0xBFFF, [0x21, 0x34, 0x12, 0], 3, 0xFF00);
        assert!(t.code[Trace::at(&slots, 0xBFFF)]);
        assert!(t.code[Trace::at(&slots, 0xC000)]);
        assert!(t.code[Trace::at(&slots, 0xC001)]);
        assert_eq!(Trace::at(&slots, 0xC000), 8 * PAGE);
    }

    const SLOTS: Slots = [0, 1, 2, 3];
    // The stack before an interrupt, and inside its handler.
    const SP: u16 = 0xFF00;
    const IN: u16 = SP - 2;

    /// Runs `op` (its bytes, which give its length) at `pc` on a 48K.
    fn run(t: &mut Trace, pc: u16, op: &[u8], sp: u16) {
        let mut bytes = [0; 4];
        bytes[..op.len()].copy_from_slice(op);
        t.on_exec(&SLOTS, pc, bytes, op.len() as u8, sp);
    }

    fn interrupt(t: &mut Trace, back: u16, sp: u16) {
        t.on_interrupt(&SLOTS, back, sp);
    }

    const NOP: &[u8] = &[0x00];
    const RET: &[u8] = &[0xC9];
    const EI: &[u8] = &[0xFB];

    #[test]
    fn returning_from_an_interrupt_is_not_an_entry() {
        let mut t = Trace::default();
        run(&mut t, 0x8000, NOP, SP);
        run(&mut t, 0x8001, NOP, SP);
        interrupt(&mut t, 0x8002, SP);
        run(&mut t, 0x0038, &[0xF5], IN); // push af
        run(&mut t, 0x0039, &[0xF1], IN - 2); // pop af
        run(&mut t, 0x003A, EI, IN);
        run(&mut t, 0x003B, RET, IN);
        run(&mut t, 0x8002, NOP, SP);
        assert!(t.entries[0x0038], "the handler's start is an entry");
        assert!(!t.entries[0x8002], "where it returns to is not");
        assert!(!t.entries[0x0039] && !t.entries[0x003B]);
    }

    #[test]
    fn a_return_by_reti_or_after_a_ret_not_taken_is_not_an_entry() {
        let mut t = Trace::default();
        run(&mut t, 0x8000, NOP, SP);
        interrupt(&mut t, 0x8001, SP);
        run(&mut t, 0x0038, &[0xC0], IN); // ret nz, not taken
        run(&mut t, 0x0039, &[0xED, 0x4D], IN); // reti
        run(&mut t, 0x8001, NOP, SP);
        assert!(!t.entries[0x0039]);
        assert!(!t.entries[0x8001]);
    }

    #[test]
    fn a_return_from_a_nested_interrupt_is_not_an_entry() {
        let mut t = Trace::default();
        run(&mut t, 0x8000, NOP, SP);
        interrupt(&mut t, 0x8001, SP);
        run(&mut t, 0x9000, EI, IN);
        interrupt(&mut t, 0x9001, IN);
        run(&mut t, 0xA000, RET, IN - 2);
        run(&mut t, 0x9001, NOP, IN);
        run(&mut t, 0x9002, RET, IN);
        run(&mut t, 0x8001, NOP, SP);
        assert!(t.entries[0x9000] && t.entries[0xA000]);
        assert!(!t.entries[0x9001], "the inner return");
        assert!(!t.entries[0x8001], "the outer return");
    }

    #[test]
    fn a_return_from_a_subroutine_in_the_handler_is_still_an_entry() {
        let mut t = Trace::default();
        run(&mut t, 0x8000, NOP, SP);
        interrupt(&mut t, 0x8001, SP);
        run(&mut t, 0x0038, &[0xCD, 0x00, 0x90], IN); // call 9000
        run(&mut t, 0x9000, RET, IN - 2);
        run(&mut t, 0x003B, RET, IN);
        run(&mut t, 0x8001, NOP, SP);
        assert!(t.entries[0x003B], "a subroutine's return point");
        assert!(!t.entries[0x8001]);
    }

    #[test]
    fn an_interrupt_after_a_jump_keeps_its_target_an_entry() {
        let mut t = Trace::default();
        run(&mut t, 0x8000, &[0xC3, 0x00, 0x81], SP); // jp 8100
        interrupt(&mut t, 0x8100, SP);
        run(&mut t, 0x0038, RET, IN);
        run(&mut t, 0x8100, NOP, SP);
        assert!(t.entries[0x8100]);
    }

    #[test]
    fn an_interrupt_after_a_call_keeps_the_call_and_its_return_entries() {
        let mut t = Trace::default();
        run(&mut t, 0x8000, &[0xCD, 0x00, 0x81], SP); // call 8100
        interrupt(&mut t, 0x8100, IN);
        run(&mut t, 0x0038, RET, IN - 2);
        run(&mut t, 0x8100, RET, IN);
        run(&mut t, 0x8003, NOP, SP);
        assert!(t.entries[0x8100], "the call's target");
        assert!(t.entries[0x8003], "the call's return point");
    }

    #[test]
    fn a_handler_that_does_not_return_leaves_an_entry() {
        let mut t = Trace::default();
        run(&mut t, 0x8000, NOP, SP);
        interrupt(&mut t, 0x8001, SP);
        run(&mut t, 0x0038, &[0xE1], IN); // pop hl
        run(&mut t, 0x0039, &[0xC3, 0x01, 0x80], SP); // jp 8001
        run(&mut t, 0x8001, NOP, SP);
        assert!(t.entries[0x8001]);
        assert!(t.interrupted.is_empty(), "the interrupt is forgotten");
    }

    #[test]
    fn a_handler_that_never_returns_is_forgotten_in_the_end() {
        // Each time, it carries on with a new stack below the old one.
        let mut t = Trace::default();
        let mut sp = SP;
        for _ in 0..2 * MAX_INTERRUPTED {
            run(&mut t, 0x8000, NOP, sp);
            interrupt(&mut t, 0x8001, sp);
            run(&mut t, 0x0038, &[0x31, 0x00, 0x00], sp - 2); // ld sp,nn
            sp -= 0x10;
        }
        assert_eq!(t.interrupted.len(), MAX_INTERRUPTED);
    }

    #[test]
    fn a_halt_is_returned_past() {
        let mut t = Trace::default();
        run(&mut t, 0x8000, &[0x76], SP); // halt
        interrupt(&mut t, 0x8001, SP);
        run(&mut t, 0x0038, RET, IN);
        run(&mut t, 0x8001, NOP, SP);
        assert!(!t.entries[0x8001]);
    }

    #[test]
    fn a_jump_to_the_next_instruction_is_an_entry() {
        let mut t = Trace::default();
        run(&mut t, 0x8000, &[0x18, 0x00], SP); // jr $+2
        run(&mut t, 0x8002, &[0xC3, 0x05, 0x80], SP); // jp $+3
        run(&mut t, 0x8005, &[0xC2, 0x08, 0x80], SP); // jp nz,$+3
        run(&mut t, 0x8008, NOP, SP);
        assert!(t.entries[0x8002] && t.entries[0x8005]);
        assert!(!t.entries[0x8008], "a conditional jump may not be taken");
    }

    #[test]
    fn a_repeating_block_instruction_is_not_an_entry_each_time() {
        let ldir = &[0xED, 0xB0];
        let mut t = Trace::default();
        run(&mut t, 0x7FFF, NOP, SP);
        run(&mut t, 0x8000, ldir, SP);
        run(&mut t, 0x8000, ldir, SP);
        interrupt(&mut t, 0x8000, SP);
        run(&mut t, 0x0038, RET, IN);
        run(&mut t, 0x8000, ldir, SP);
        run(&mut t, 0x8002, NOP, SP);
        assert!(!t.entries[0x8000] && !t.entries[0x8002]);
    }

    #[test]
    fn returns_and_block_repeats_are_recognised() {
        for op in [0xC9, 0xC0, 0xC8, 0xD0, 0xD8, 0xE0, 0xE8, 0xF0, 0xF8] {
            assert!(is_return([op, 0, 0, 0]), "{op:02x}");
        }
        for op in [0x45, 0x4D, 0x55, 0x5D, 0x65, 0x6D, 0x75, 0x7D] {
            assert!(is_return([0xED, op, 0, 0]), "ed {op:02x}");
        }
        for op in [0xC3, 0xCD, 0xE9, 0xC1, 0x00] {
            assert!(!is_return([op, 0, 0, 0]), "{op:02x}");
        }
        assert!(!is_return([0xED, 0x44, 0, 0]), "neg");
        for op in [[0x18, 0], [0xC3, 0], [0xE9, 0], [0xDD, 0xE9], [0xFD, 0xE9]] {
            assert!(is_jump([op[0], op[1], 0, 0]), "{op:02x?}");
        }
        for op in [[0x20, 0], [0x10, 0], [0xC2, 0], [0xCD, 0], [0xDD, 0x21]] {
            assert!(!is_jump([op[0], op[1], 0, 0]), "{op:02x?}");
        }
        for op in [0xB0, 0xB1, 0xB2, 0xB3, 0xB8, 0xB9, 0xBA, 0xBB] {
            assert!(is_block_repeat([0xED, op, 0, 0]), "ed {op:02x}");
        }
        for op in [0xA0, 0xA8, 0xB4, 0xBC] {
            assert!(!is_block_repeat([0xED, op, 0, 0]), "ed {op:02x}");
        }
    }
}
