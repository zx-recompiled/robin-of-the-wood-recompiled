//! A machine's state to start from: the processor's registers and RAM.
//!
//! It comes from the player's tape ([`MachineState::from_tape`]): the program's
//! memory as its code blocks leave it, and the processor where the ROM's
//! tape loader leaves it when it returns into the game. No `.z80` file is
//! read: the one this project used to be checked against held a damaged
//! byte in code, which the rewrite then copied (starquake-recompiled#117).

/// Which Spectrum a machine is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    /// The 48K: one ROM and 48K of RAM, never paged.
    Spectrum48,
    /// The 128K and the grey +2: two ROMs and eight 16K RAM banks, paged
    /// through port `0x7FFD`.
    Spectrum128,
}

impl Model {
    /// The shape of the machine's frame.
    #[must_use]
    pub const fn timing(self) -> crate::timing::Timing {
        match self {
            Model::Spectrum48 => crate::timing::SPECTRUM_48,
            Model::Spectrum128 => crate::timing::SPECTRUM_128,
        }
    }
}

/// Bytes in a 128K machine's RAM: eight banks of 16K.
pub const RAM_128: usize = 8 * 0x4000;

/// The processor's registers and the RAM a machine starts from.
#[derive(Clone, Debug)]
pub struct MachineState {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub a_: u8,
    pub f_: u8,
    pub b_: u8,
    pub c_: u8,
    pub d_: u8,
    pub e_: u8,
    pub h_: u8,
    pub l_: u8,
    pub ix: u16,
    pub iy: u16,
    pub sp: u16,
    pub pc: u16,
    pub i: u8,
    pub r: u8,
    pub iff1: bool,
    pub iff2: bool,
    pub im: u8,
    pub border: u8,
    /// Which machine this is.
    pub model: Model,
    /// The last value written to port `0x7FFD`, which says what is paged
    /// where on a 128K machine. Unused on a 48K.
    pub port_7ffd: u8,
    /// The RAM. On a 48K machine, the contents of `0x4000..=0xFFFF`; on a
    /// 128K, the eight banks in order, bank 0 first ([`RAM_128`] bytes).
    pub ram: Vec<u8>,
}

impl MachineState {
    /// Full 64K address space with the RAM in place and zeros for the ROM.
    /// On a 128K machine, as paged: bank 5, bank 2, and the bank `port_7ffd`
    /// selects.
    #[must_use]
    pub fn memory(&self) -> Vec<u8> {
        let mut mem = vec![0u8; 0x4000];
        match self.model {
            Model::Spectrum48 => mem.extend_from_slice(&self.ram),
            Model::Spectrum128 => {
                for bank in [5, 2, usize::from(self.port_7ffd & 7)] {
                    mem.extend_from_slice(&self.ram[bank * 0x4000..(bank + 1) * 0x4000]);
                }
            }
        }
        mem
    }
}

/// IY while Spectrum BASIC runs: the system variables' base, `ERR_NR`.
pub const BASIC_IY: u16 = 0x5C3A;
/// I while Spectrum BASIC runs, as the ROM sets it at start-up.
pub const BASIC_I: u8 = 0x3F;

impl MachineState {
    /// The machine as a tape's program starts: its RAM as the code blocks
    /// leave it, and the processor as the ROM's loader returns into it at
    /// `pc` with the stack at `sp`. The loader runs with interrupts off in
    /// interrupt mode 1, and BASIC's IY and I are still in place; the other
    /// registers are whatever the program sets before it reads them.
    #[must_use]
    pub fn from_tape(tape: &crate::tape::Tape, pc: u16, sp: u16) -> MachineState {
        MachineState {
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            a_: 0,
            f_: 0,
            b_: 0,
            c_: 0,
            d_: 0,
            e_: 0,
            h_: 0,
            l_: 0,
            ix: 0,
            iy: BASIC_IY,
            sp,
            pc,
            i: BASIC_I,
            r: 0,
            iff1: false,
            iff2: false,
            im: 1,
            border: 0,
            model: Model::Spectrum48,
            port_7ffd: 0,
            ram: tape.ram.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_128k_states_memory_is_seen_as_paged() {
        let mut ram = vec![0u8; RAM_128];
        for (bank, chunk) in ram.chunks_mut(0x4000).enumerate() {
            chunk.fill(bank as u8);
        }
        let tape = crate::tape::Tape {
            ram: vec![0; 0xC000],
            loading_screen: None,
        };
        let state = MachineState {
            model: Model::Spectrum128,
            port_7ffd: 0x16,
            ram,
            ..MachineState::from_tape(&tape, 0, 0)
        };
        let mem = state.memory();
        assert_eq!(mem.len(), 0x10000);
        assert_eq!((mem[0x4000], mem[0x8000], mem[0xC000]), (5, 2, 6));
    }
}
