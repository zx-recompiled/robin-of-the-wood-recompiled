//! `.tap` tape loader: the blocks a Spectrum would load from cassette.
//!
//! A tape is a sequence of blocks, each a two-byte length followed by that
//! many bytes: a flag (0x00 for a header, 0xFF for data), the payload, and a
//! checksum. A header says what the block after it is and, for code, where
//! it loads.

/// Bytes of a Spectrum screen: bitmap then attributes.
pub const SCREEN_LEN: usize = 6912;

const HEADER: u8 = 0x00;
const DATA: u8 = 0xFF;
const CODE: u8 = 3;
const SCREEN_ADDR: u16 = 0x4000;

/// A loaded tape.
pub struct Tape {
    /// Contents of 0x4000..=0xFFFF, as the code blocks left it.
    pub ram: Vec<u8>,
    /// The picture shown while the rest of the tape loaded, if it has one.
    pub loading_screen: Option<Vec<u8>>,
}

impl Tape {
    /// Full 64K address space with the RAM in place and zeros for the ROM.
    pub fn memory(&self) -> Vec<u8> {
        let mut mem = vec![0u8; 0x4000];
        mem.extend_from_slice(&self.ram);
        mem
    }
}

/// Loads every code block on a `.tap`, in the order the Spectrum would.
///
/// # Errors
///
/// If a block runs off the end of the file or fails its checksum, or if the
/// tape places nothing in RAM. A block that would load outside RAM is
/// skipped rather than rejected, so one stray block does not lose the tape.
pub fn load_tap(bytes: &[u8]) -> Result<Tape, String> {
    let mut ram = vec![0u8; 0xC000];
    let mut loading_screen = None;
    // Where the block after the current header will load.
    let mut pending: Option<(u16, usize)> = None;
    let mut loaded = 0usize;
    // Code blocks seen, as against bytes placed: a zero-length one would
    // otherwise be reported as no code blocks at all.
    let mut blocks = 0usize;
    let mut i = 0usize;

    while i + 2 <= bytes.len() {
        let len = bytes[i] as usize | (bytes[i + 1] as usize) << 8;
        i += 2;
        if len < 2 || i + len > bytes.len() {
            return Err(format!("truncated tape block at {i:#x}"));
        }
        let block = &bytes[i..i + len];
        i += len;

        match block[0] {
            HEADER if len >= 19 => {
                pending = (block[1] == CODE).then(|| {
                    let length = block[12] as usize | (block[13] as usize) << 8;
                    let start = block[14] as u16 | (block[15] as u16) << 8;
                    (start, length)
                });
            }
            DATA => {
                let Some((start, length)) = pending.take() else {
                    continue;
                };
                let data = &block[1..len - 1];
                // The last byte is a checksum: the flag and every data byte
                // XORed together. A bit-flipped tape used to load in silence.
                let sum = block[..len - 1].iter().fold(0u8, |a, b| a ^ b);
                if sum != block[len - 1] {
                    return Err(format!(
                        "tape block at {start:#06x} is corrupt (checksum {:#04x}, expected {sum:#04x})",
                        block[len - 1]
                    ));
                }
                blocks += 1;
                let n = length.min(data.len());
                let at = start as usize;
                // A block that loads into the ROM is not something a Spectrum
                // would honour either. Skip it rather than throw away a tape
                // whose game blocks have already loaded.
                if at < 0x4000 || at + n > 0x10000 {
                    continue;
                }
                // The loading picture goes to the screen first, and the game
                // lands on top of it later.
                if at == SCREEN_ADDR as usize && n == SCREEN_LEN && loading_screen.is_none() {
                    loading_screen = Some(data[..n].to_vec());
                }
                ram[at - 0x4000..at - 0x4000 + n].copy_from_slice(&data[..n]);
                loaded += n;
            }
            _ => pending = None,
        }
    }

    if loaded == 0 {
        return Err(if blocks == 0 {
            "no code blocks on the tape".into()
        } else {
            format!("the tape's {blocks} code block(s) placed nothing in RAM")
        });
    }
    Ok(Tape {
        ram,
        loading_screen,
    })
}

/// The blocks of a `.tzx` file, each as it would be on a `.tap`: flag byte,
/// data, checksum.
///
/// Only what a tape loaded at the ROM's own speed holds is read: standard
/// data blocks (`0x10`). The information blocks a dump carries alongside
/// (`0x30` text, `0x32` archive info) are skipped. Anything else, such as a
/// turbo, pure-tone or direct-recording block, means a loader this reader
/// cannot stand in for, and is refused by name.
///
/// # Errors
///
/// If the file is not a TZX file, a block runs off its end, or a block is of
/// a kind this reader does not take.
pub fn load_tzx(bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    const HEADER: &[u8] = b"ZXTape!\x1A";
    if bytes.len() < 10 || &bytes[..8] != HEADER {
        return Err("not a TZX file".into());
    }
    let word = |at: usize| {
        bytes
            .get(at..at + 2)
            .map(|b| usize::from(b[0]) | usize::from(b[1]) << 8)
    };
    let mut blocks = Vec::new();
    let mut i = 10usize;
    while i < bytes.len() {
        let id = bytes[i];
        let short = || format!("TZX block {id:#04x} at {i:#x} runs off the end");
        match id {
            0x10 => {
                let len = word(i + 3).ok_or_else(short)?;
                let data = bytes.get(i + 5..i + 5 + len).ok_or_else(short)?;
                if len < 2 {
                    return Err(format!(
                        "TZX block at {i:#x} is too short to be a tape block"
                    ));
                }
                blocks.push(data.to_vec());
                i += 5 + len;
            }
            0x30 => i += 2 + usize::from(*bytes.get(i + 1).ok_or_else(short)?),
            0x32 => i += 3 + word(i + 1).ok_or_else(short)?,
            _ => {
                return Err(format!(
                    "TZX block {id:#04x} at {i:#x} is not a standard-speed block; this tape needs a loader that is not supported"
                ));
            }
        }
    }
    if i > bytes.len() {
        return Err("the last TZX block runs off the end".into());
    }
    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A TZX file of these blocks, each `(id, body)` as it follows the id.
    fn tzx(blocks: &[(u8, Vec<u8>)]) -> Vec<u8> {
        let mut out = b"ZXTape!\x1A\x01\x14".to_vec();
        for (id, body) in blocks {
            out.push(*id);
            out.extend_from_slice(body);
        }
        out
    }

    /// A standard-speed block's body: a pause, then the tape block.
    fn standard(block: &[u8]) -> Vec<u8> {
        let mut body = vec![0xE8, 0x03];
        body.extend_from_slice(&(block.len() as u16).to_le_bytes());
        body.extend_from_slice(block);
        body
    }

    #[test]
    fn standard_blocks_come_out_as_tap_blocks() {
        let file = tzx(&[
            (0x32, vec![3, 0, 1, 0, 0x41]),
            (0x10, standard(&[0x00, 1, 2, 3])),
            (0x30, vec![2, b'h', b'i']),
            (0x10, standard(&[0xFF, 9, 9])),
        ]);
        assert_eq!(
            load_tzx(&file).expect("reads"),
            vec![vec![0x00, 1, 2, 3], vec![0xFF, 9, 9]]
        );
    }

    #[test]
    fn other_blocks_and_bad_files_are_refused() {
        assert!(load_tzx(b"ZXTape").is_err());
        assert!(
            load_tzx(&tzx(&[(0x11, vec![0; 18])]))
                .unwrap_err()
                .contains("0x11")
        );
        let mut cut = tzx(&[(0x10, standard(&[0xFF, 1, 2, 3]))]);
        cut.pop();
        assert!(load_tzx(&cut).is_err());
    }
}
