//! `.szx` snapshots (ZX-State, Spectaculator's format, also written by
//! SpecEmu), for test programs that are only published as one.
//!
//! The game is never read from a snapshot (see [`crate::state`]); this exists
//! for Richard and Tim Butler's 128K timing tests. Only what a 48K or 128K
//! machine needs is read: the registers, the paging and border, the RAM
//! pages, and the AY. Other blocks are skipped.

use crate::state::{MachineState, Model, RAM_128};

/// What a snapshot holds beyond the [`MachineState`]: the moment in the frame
/// it was taken, and the AY's registers.
pub struct Snapshot {
    pub state: MachineState,
    /// T-states into the frame.
    pub t: u32,
    /// The last instruction was `EI`, so an interrupt waits one more.
    pub after_ei: bool,
    /// The processor is halted.
    pub halted: bool,
    /// The AY's registers, and the one selected.
    pub ay: Option<([u8; 16], u8)>,
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from(b[at]) | u16::from(b[at + 1]) << 8
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Reads a `.szx` file.
///
/// # Errors
///
/// If it isn't one, is for a machine other than the 48K or 128K, is cut
/// short, or a compressed page doesn't inflate to 16K.
pub fn load(bytes: &[u8]) -> Result<Snapshot, String> {
    if bytes.len() < 8 || &bytes[..4] != b"ZXST" {
        return Err("not a ZX-State (.szx) file".into());
    }
    let model = match bytes[6] {
        1 => Model::Spectrum48,
        2 | 3 => Model::Spectrum128,
        id => return Err(format!("unsupported machine (ZX-State id {id})")),
    };
    let mut ram = vec![
        0u8;
        match model {
            Model::Spectrum48 => 0xC000,
            Model::Spectrum128 => RAM_128,
        }
    ];
    let mut regs: Option<&[u8]> = None;
    let (mut border, mut port_7ffd) = (0u8, 0u8);
    let mut ay = None;
    let mut i = 8usize;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let len = u32_at(bytes, i + 4) as usize;
        let body = bytes
            .get(i + 8..i + 8 + len)
            .ok_or_else(|| format!("block {} runs off the end", String::from_utf8_lossy(id)))?;
        i += 8 + len;
        match id {
            b"Z80R" if len >= 37 => regs = Some(body),
            b"SPCR" if len >= 8 => {
                border = body[0];
                port_7ffd = body[1];
            }
            b"AY\0\0" if len >= 18 => {
                ay = Some((std::array::from_fn(|r| body[2 + r]), body[1]));
            }
            b"RAMP" if len >= 3 => {
                let compressed = u16_at(body, 0) & 1 != 0;
                let page = usize::from(body[2]);
                let data = if compressed {
                    miniz_oxide::inflate::decompress_to_vec_zlib(&body[3..])
                        .map_err(|e| format!("RAM page {page} does not inflate: {e:?}"))?
                } else {
                    body[3..].to_vec()
                };
                if data.len() != 0x4000 {
                    return Err(format!("RAM page {page} is {} bytes, not 16K", data.len()));
                }
                // A 48K snapshot numbers its pages as the 128K's banks: 5, 2
                // and 0 are 0x4000, 0x8000 and 0xC000.
                let at = match model {
                    Model::Spectrum128 if page < 8 => page * 0x4000,
                    Model::Spectrum48 => match page {
                        5 => 0,
                        2 => 0x4000,
                        0 => 0x8000,
                        _ => continue,
                    },
                    Model::Spectrum128 => continue,
                };
                ram[at..at + 0x4000].copy_from_slice(&data);
            }
            _ => {}
        }
    }
    let r = regs.ok_or("no Z80R (registers) block")?;
    let w = |n: usize| u16_at(r, 2 * n);
    let hi = |v: u16| (v >> 8) as u8;
    let lo = |v: u16| v as u8;
    let state = MachineState {
        a: hi(w(0)),
        f: lo(w(0)),
        b: hi(w(1)),
        c: lo(w(1)),
        d: hi(w(2)),
        e: lo(w(2)),
        h: hi(w(3)),
        l: lo(w(3)),
        a_: hi(w(4)),
        f_: lo(w(4)),
        b_: hi(w(5)),
        c_: lo(w(5)),
        d_: hi(w(6)),
        e_: lo(w(6)),
        h_: hi(w(7)),
        l_: lo(w(7)),
        ix: w(8),
        iy: w(9),
        sp: w(10),
        pc: w(11),
        i: r[24],
        r: r[25],
        iff1: r[26] != 0,
        iff2: r[27] != 0,
        im: r[28],
        border: border & 7,
        model,
        port_7ffd,
        ram,
    };
    Ok(Snapshot {
        state,
        t: u32_at(r, 29),
        after_ei: r[34] & 1 != 0,
        halted: r[34] & 2 != 0,
        ay,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal 128K snapshot: registers, paging, and one stored page.
    fn snapshot() -> Vec<u8> {
        let mut out = b"ZXST\x01\x04\x02\x00".to_vec();
        let mut block = |id: &[u8], body: &[u8]| {
            out.extend_from_slice(id);
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(body);
        };
        let mut z80r = vec![0u8; 37];
        z80r[22..24].copy_from_slice(&0x8000u16.to_le_bytes()); // PC
        z80r[20..22].copy_from_slice(&0xFF00u16.to_le_bytes()); // SP
        z80r[28] = 1; // IM 1
        z80r[29..33].copy_from_slice(&1234u32.to_le_bytes());
        block(b"Z80R", &z80r);
        block(b"SPCR", &[2, 0x13, 0, 0, 0, 0, 0, 0]);
        let mut ramp = vec![0, 0, 3];
        ramp.extend(vec![0xAB; 0x4000]);
        block(b"RAMP", &ramp);
        out
    }

    #[test]
    fn reads_registers_paging_and_pages() {
        let s = load(&snapshot()).expect("loads");
        assert_eq!(s.state.model, Model::Spectrum128);
        assert_eq!((s.state.pc, s.state.sp, s.state.im), (0x8000, 0xFF00, 1));
        assert_eq!((s.state.border, s.state.port_7ffd), (2, 0x13));
        assert_eq!(s.t, 1234);
        assert_eq!(s.state.ram[3 * 0x4000], 0xAB);
        assert_eq!(s.state.ram[2 * 0x4000], 0);
    }

    #[test]
    fn refuses_what_it_cannot_read() {
        assert!(load(b"nothing").is_err());
        let mut plus3 = snapshot();
        plus3[6] = 5;
        assert!(load(&plus3).is_err());
    }
}
