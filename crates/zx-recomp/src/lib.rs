//! zx-recomp: the reverse-engineering tool this project was built with.
//!
//! It runs the original headless and works out what its code does, so the
//! rewrite can be written from a readable listing rather than from raw bytes:
//!
//! 1. Load the user's tape (and ROM) and check them against the hashes
//!    in the game config.
//! 2. Trace: run the game headless in the interpreter with scripted input,
//!    recording executed code, jump targets and self-modifying code.
//! 3. Analyse: recursive descent from every known entry point.
//! 4. Print an annotated disassembly, marking what the trace actually ran.
//!
//! It once generated Rust from the analysis as well. That approach was
//! abandoned in favour of the hand-written rewrite in starquake-recompiled's
//! `games/starquake`, which needs no runtime, and the generator has been
//! removed.

pub mod analysis;
pub mod config;
pub mod listing;
pub mod tracer;

use std::fmt::Write as _;
use std::path::Path;

use zx_core::MachineState;
use zx_core::sha1::sha1_hex;
use zx_runtime::Zx;

pub use config::Config;

/// The user-supplied files a build works from, and the machine they make.
pub struct Inputs {
    /// The machine as the tape's program starts.
    pub machine: Zx,
    pub tape_sha1: String,
    /// Whether a ROM was loaded (without one, ROM code cannot run).
    pub rom_loaded: bool,
    pub rom_sha1: Option<String>,
}

fn read(path: &Path, what: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| {
        format!(
            "cannot read {what} {}: {e}\n\
             Copy your own copy of the file into the assets directory (see assets/README.md).",
            path.display()
        )
    })
}

fn check_hash(what: &str, actual: &str, expected: Option<&str>) -> Result<(), String> {
    match expected {
        Some(e) if !e.eq_ignore_ascii_case(actual) => Err(format!(
            "{what} has SHA-1 {actual}, but this config was written for {e}.\n\
             Use the dump the config expects, or update the hash in the config \
             (other dumps may need different analysis hints)."
        )),
        _ => Ok(()),
    }
}

impl Inputs {
    /// Reads the tape and ROM the configuration names, from `assets`.
    ///
    /// # Errors
    ///
    /// If either file is missing or unreadable, its SHA-1 does not match the
    /// one the configuration pins, or the tape will not parse.
    pub fn load(cfg: &Config, assets: &Path) -> Result<Inputs, String> {
        use config::Machine;
        let g = &cfg.game;
        let tape_path = assets.join(&g.tape);
        let tape_bytes = read(&tape_path, "tape")?;
        let tape_sha1 = sha1_hex(&tape_bytes);
        check_hash("tape", &tape_sha1, g.tape_sha1.as_deref())?;
        let in_tape = |e: String| format!("{}: {e}", tape_path.display());

        let rom_len = match g.machine {
            Machine::Spectrum48 => 0x4000,
            Machine::Spectrum128 => 0x8000,
        };
        let (rom, rom_sha1) = match &g.rom {
            Some(name) => {
                let bytes = read(&assets.join(name), "ROM")?;
                if bytes.len() != rom_len {
                    return Err(format!(
                        "ROM {name} is {} bytes, expected {rom_len}",
                        bytes.len()
                    ));
                }
                let hash = sha1_hex(&bytes);
                check_hash("ROM", &hash, g.rom_sha1.as_deref())?;
                (Some(bytes), Some(hash))
            }
            None => (None, None),
        };

        let machine = match g.machine {
            Machine::Spectrum48 => {
                let (Some(pc), Some(sp)) = (g.entry_pc, g.entry_sp) else {
                    return Err("a 48K config needs entry_pc and entry_sp".into());
                };
                let tape = zx_core::tape::load_tap(&tape_bytes).map_err(in_tape)?;
                Zx::new(&MachineState::from_tape(&tape, pc, sp), rom.as_deref())
            }
            Machine::Spectrum128 => {
                let until = g.boot_until.ok_or("a 128K config needs boot_until")?;
                let rom = rom
                    .as_deref()
                    .ok_or("a 128K boots from its ROM: name it in rom")?;
                let blocks = if tape_bytes.starts_with(b"ZXTape!") {
                    zx_core::tape::load_tzx(&tape_bytes).map_err(in_tape)?
                } else {
                    tape_blocks(&tape_bytes).map_err(in_tape)?
                };
                zx_runtime::loader::boot_128k(rom, blocks, until, 3000)?
            }
        };
        Ok(Inputs {
            machine,
            tape_sha1,
            rom_loaded: rom.is_some(),
            rom_sha1,
        })
    }

    /// Memory image the analysis starts from: the 64K the processor sees as
    /// the program starts, the ROM included.
    pub fn memory(&self) -> Vec<u8> {
        (0..=0xFFFFu16).map(|a| self.machine.read(a)).collect()
    }
}

/// A `.tap` file's blocks, as they are on tape.
fn tape_blocks(bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut blocks = Vec::new();
    let mut i = 0usize;
    while i + 2 <= bytes.len() {
        let len = usize::from(bytes[i]) | usize::from(bytes[i + 1]) << 8;
        let block = bytes
            .get(i + 2..i + 2 + len)
            .ok_or_else(|| format!("truncated tape block at {i:#x}"))?;
        blocks.push(block.to_vec());
        i += 2 + len;
    }
    Ok(blocks)
}

/// Reads a miss log written by the runtime: one hex address per line.
pub fn read_misses(path: &Path) -> Vec<u16> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| l.split_whitespace().next())
        .filter_map(|w| u16::from_str_radix(w.trim_start_matches("0x"), 16).ok())
        .collect()
}

pub fn report(a: &analysis::Analysis) -> String {
    let s = &a.stats;
    let mut r = String::new();
    let _ = writeln!(r, "traced instructions:   {}", s.traced_instrs);
    let _ = writeln!(r, "traced entry points:   {}", s.traced_entries);
    let _ = writeln!(r, "blocks generated:      {}", s.entries);
    let _ = writeln!(r, "instructions compiled: {}", s.instrs);
    let _ = writeln!(r, "self-modifying instrs: {}", s.self_modifying.len());
    for a in &s.self_modifying {
        let _ = writeln!(r, "  {a:04x}");
    }
    r
}
