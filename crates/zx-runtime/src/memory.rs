//! The machine's memory: its ROM and RAM pages, and which of them the
//! processor sees in each quarter of the address space.
//!
//! A Spectrum's address space is four 16K slots. On a 48K machine they never
//! change: the ROM, then three RAM pages. On a 128K machine the ROM slot and
//! the top slot can be repointed at other pages by a write to port `0x7FFD`,
//! and the same RAM bank can appear in two slots at once. So the memory is
//! the pages themselves plus four slot numbers, and paging is repointing a
//! slot: whatever slot an address is read through, it is the same byte.

/// Bytes in a page, and in a slot of the address space.
pub const PAGE: usize = 0x4000;

#[derive(Clone)]
pub struct Memory {
    pages: Box<[[u8; PAGE]]>,
    /// The page each slot shows.
    slots: [usize; 4],
    /// Pages that are ROM, which the processor cannot write. A machine built
    /// without a ROM file has none: its bottom 16K is ordinary memory.
    rom: Box<[bool]>,
}

impl Memory {
    /// A 48K machine's memory: `rom` (if any) in slot 0, and `ram`, the 48K
    /// from `0x4000` up, in the three slots above it.
    ///
    /// The ROM is a user-supplied file, so a short or long one is not a
    /// panic: whatever is there is used, and the rest stays zero.
    #[must_use]
    pub fn new_48k(ram: &[u8], rom: Option<&[u8]>) -> Memory {
        let mut pages = vec![[0u8; PAGE]; 4].into_boxed_slice();
        if let Some(rom) = rom {
            let n = rom.len().min(PAGE);
            pages[0][..n].copy_from_slice(&rom[..n]);
        }
        for (page, chunk) in pages[1..].iter_mut().zip(ram.chunks(PAGE)) {
            page[..chunk.len()].copy_from_slice(chunk);
        }
        Memory {
            pages,
            slots: [0, 1, 2, 3],
            rom: vec![rom.is_some(), false, false, false].into_boxed_slice(),
        }
    }

    /// The byte at `addr`, through whichever page its slot shows.
    #[inline(always)]
    #[must_use]
    pub fn read(&self, addr: u16) -> u8 {
        self.pages[self.slots[(addr >> 14) as usize]][addr as usize & (PAGE - 1)]
    }

    /// Writes a byte unless its slot shows a ROM, and says whether it did.
    #[inline(always)]
    pub fn write(&mut self, addr: u16, v: u8) -> bool {
        let page = self.slots[(addr >> 14) as usize];
        if self.rom[page] {
            return false;
        }
        self.pages[page][addr as usize & (PAGE - 1)] = v;
        true
    }

    /// Writes a byte even into ROM, for setting up a test.
    pub fn poke(&mut self, addr: u16, v: u8) {
        let page = self.slots[(addr >> 14) as usize];
        self.pages[page][addr as usize & (PAGE - 1)] = v;
    }

    /// The page slot `slot` shows.
    #[must_use]
    pub fn slot(&self, slot: usize) -> usize {
        self.slots[slot]
    }

    /// One page, whatever slot it is in, or none.
    #[must_use]
    pub fn page(&self, page: usize) -> &[u8; PAGE] {
        &self.pages[page]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_48k_layout_is_rom_then_ram() {
        let ram: Vec<u8> = (0..0xC000).map(|i| (i / PAGE) as u8 + 1).collect();
        let m = Memory::new_48k(&ram, Some(&[0xF3; PAGE]));
        assert_eq!(m.read(0x0000), 0xF3);
        assert_eq!(m.read(0x4000), 1);
        assert_eq!(m.read(0x8000), 2);
        assert_eq!(m.read(0xFFFF), 3);
    }

    #[test]
    fn rom_ignores_writes_and_ram_takes_them() {
        let mut m = Memory::new_48k(&[0; 0xC000], Some(&[0xF3; PAGE]));
        assert!(!m.write(0x0000, 0));
        assert_eq!(m.read(0x0000), 0xF3);
        assert!(m.write(0x4000, 7));
        assert_eq!(m.read(0x4000), 7);
        m.poke(0x0000, 0xC9);
        assert_eq!(m.read(0x0000), 0xC9);
    }

    #[test]
    fn without_a_rom_the_bottom_16k_is_memory() {
        let mut m = Memory::new_48k(&[0; 0xC000], None);
        assert!(m.write(0x0010, 0xC9));
        assert_eq!(m.read(0x0010), 0xC9);
    }

    #[test]
    fn a_short_rom_or_ram_is_not_a_panic() {
        let m = Memory::new_48k(&[5; 10], Some(&[1, 2, 3]));
        assert_eq!(m.read(0x0002), 3);
        assert_eq!(m.read(0x0003), 0);
        assert_eq!(m.read(0x4009), 5);
        assert_eq!(m.read(0x400A), 0);
    }
}
