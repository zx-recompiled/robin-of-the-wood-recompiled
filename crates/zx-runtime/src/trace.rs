//! Execution tracing for the recompiler's analysis pass.
//!
//! While the interpreter runs the game headless, this records which
//! addresses were executed, which addresses control arrived at by a jump,
//! call, return or interrupt (block entry points), and which instructions
//! were executed with different bytes at different times (self-modifying
//! code).

#[derive(Clone)]
pub struct Trace {
    /// Bytes of the instruction starting at each address, as first executed.
    pub executed: Vec<Option<(u8, [u8; 4])>>,
    /// Instruction starts whose bytes changed between executions.
    pub self_modified: Vec<bool>,
    /// Addresses reached other than by falling through from the previous instruction.
    pub entries: Vec<bool>,
    /// Address the next instruction is expected at if execution falls through.
    pub fallthrough: Option<u16>,
    /// Addresses that are part of some executed instruction.
    pub code: Vec<bool>,
    /// Code bytes written after they were executed.
    pub written_code: Vec<bool>,
}

impl Default for Trace {
    fn default() -> Self {
        Trace {
            executed: vec![None; 0x10000],
            self_modified: vec![false; 0x10000],
            entries: vec![false; 0x10000],
            fallthrough: None,
            code: vec![false; 0x10000],
            written_code: vec![false; 0x10000],
        }
    }
}

impl Trace {
    pub fn on_exec(&mut self, pc: u16, mem: &[u8; 0x10000], len: u8) {
        if self.fallthrough != Some(pc) {
            self.entries[pc as usize] = true;
        }
        self.fallthrough = Some(pc.wrapping_add(len as u16));

        let mut bytes = [0u8; 4];
        for (i, b) in bytes.iter_mut().enumerate().take(len as usize) {
            *b = mem[pc.wrapping_add(i as u16) as usize];
        }
        match &self.executed[pc as usize] {
            Some(prev) if *prev != (len, bytes) => self.self_modified[pc as usize] = true,
            Some(_) => {}
            None => {
                self.executed[pc as usize] = Some((len, bytes));
                for i in 0..len as u16 {
                    self.code[pc.wrapping_add(i) as usize] = true;
                }
            }
        }
    }

    /// Called when control is transferred by something other than an
    /// instruction (an interrupt).
    pub fn on_interrupt(&mut self) {
        self.fallthrough = None;
    }

    pub fn on_write(&mut self, addr: u16) {
        if self.code[addr as usize] {
            self.written_code[addr as usize] = true;
        }
    }
}
