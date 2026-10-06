//! Feeding a tape to the ROM's own loader, the way a fast-loading emulator
//! does.
//!
//! Running the ROM's tape routine edge by edge would take minutes of
//! emulated time per program. Instead, when the processor reaches
//! `LD-BYTES` with the BASIC ROM paged in, the next block is put where the
//! routine was asked to put it, and the routine finishes as if it had read
//! it from tape. Everything around the load (BASIC's `LOAD`, the 128's Tape
//! Loader, the program's own loader stub) runs for real.
//!
//! This covers programs loaded through `LD-BYTES`' front door. A loader that
//! jumps into the middle of the routine, as Robin of the Wood's does at
//! `0x0563` (`docs/re/robin.md`), needs its own trap.

use crate::machine::Zx;
use crate::memory::PAGE;

/// The ROM's `LD-BYTES`: load (or verify) one block.
pub const LD_BYTES: u16 = 0x0556;
/// The ROM's `SA/LD-RET`, where `LD-BYTES` finishes: it restores the border,
/// checks for BREAK and enables interrupts, then returns to the caller.
pub const SA_LD_RET: u16 = 0x053F;

/// A tape's blocks, waiting to be loaded in order.
pub struct TapeFeeder {
    /// Each block as it is on tape: flag byte, data, checksum.
    blocks: Vec<Vec<u8>>,
    next: usize,
}

impl TapeFeeder {
    /// The blocks of a `.tap` file.
    ///
    /// # Errors
    ///
    /// If a block runs off the end of the file or is too short to have a
    /// flag and a checksum.
    pub fn from_tap(bytes: &[u8]) -> Result<TapeFeeder, String> {
        let mut blocks = Vec::new();
        let mut i = 0usize;
        while i + 2 <= bytes.len() {
            let len = usize::from(bytes[i]) | usize::from(bytes[i + 1]) << 8;
            i += 2;
            if len < 2 || i + len > bytes.len() {
                return Err(format!("truncated tape block at {i:#x}"));
            }
            blocks.push(bytes[i..i + len].to_vec());
            i += len;
        }
        Ok(TapeFeeder { blocks, next: 0 })
    }

    /// Blocks not yet loaded.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.blocks.len() - self.next
    }

    /// If the processor is at `LD-BYTES` with the BASIC ROM paged in, loads
    /// the next block as the routine was asked and sends the processor on to
    /// `SA/LD-RET`, returning true. Otherwise does nothing.
    ///
    /// The routine is entered with the flag byte it wants in A, carry set to
    /// load or clear to verify, the length in DE and the address in IX. It
    /// finishes with carry set if the block matched: the right flag, the
    /// right length and a good checksum. A block with the wrong flag is
    /// consumed and fails, as the ROM skips it, so BASIC's search for a
    /// header goes on to the next block.
    pub fn on_step(&mut self, z: &mut Zx) -> bool {
        if z.pc != LD_BYTES || !basic_rom_paged(z) || self.next >= self.blocks.len() {
            return false;
        }
        let block = &self.blocks[self.next];
        self.next += 1;
        let load = z.f & crate::machine::CF != 0;
        let want = usize::from(z.d) << 8 | usize::from(z.e);
        let mut ok = block[0] == z.a;
        if ok {
            let data = &block[1..block.len() - 1];
            let sum = block[..block.len() - 1].iter().fold(0u8, |a, b| a ^ b);
            let n = data.len().min(want);
            for (i, &b) in data[..n].iter().enumerate() {
                let at = z.ix.wrapping_add(i as u16);
                if load {
                    z.write(at, b);
                } else if z.read(at) != b {
                    ok = false;
                }
            }
            z.ix = z.ix.wrapping_add(n as u16);
            z.set_de((want - n) as u16);
            ok = ok && data.len() == want && sum == block[block.len() - 1];
        }
        z.f = if ok {
            z.f | crate::machine::CF
        } else {
            z.f & !crate::machine::CF
        };
        z.pc = SA_LD_RET;
        true
    }
}

/// Whether the ROM in the bottom slot is the one holding Spectrum BASIC: the
/// only ROM on a 48K, ROM 1 on a 128K.
#[must_use]
pub fn basic_rom_paged(z: &Zx) -> bool {
    match z.model {
        zx_core::Model::Spectrum48 => true,
        zx_core::Model::Spectrum128 => z.memory.slot(0) == 1,
    }
}

/// The BASIC ROM's 16K, wherever the machine keeps it.
#[must_use]
pub fn basic_rom(z: &Zx) -> &[u8; PAGE] {
    z.memory.page(match z.model {
        zx_core::Model::Spectrum48 => 0,
        zx_core::Model::Spectrum128 => 1,
    })
}
