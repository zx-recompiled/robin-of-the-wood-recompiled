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

pub use config::Config;

/// The user-supplied files a build works from.
pub struct Inputs {
    /// The machine as the tape's program starts.
    pub start: MachineState,
    pub tape_sha1: String,
    pub rom: Option<Vec<u8>>,
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
        let tape_path = assets.join(&cfg.game.tape);
        let tape_bytes = read(&tape_path, "tape")?;
        let tape_sha1 = sha1_hex(&tape_bytes);
        check_hash("tape", &tape_sha1, cfg.game.tape_sha1.as_deref())?;
        let tape = zx_core::tape::load_tap(&tape_bytes)
            .map_err(|e| format!("{}: {e}", tape_path.display()))?;
        let start = MachineState::from_tape(&tape, cfg.game.entry_pc, cfg.game.entry_sp);

        let (rom, rom_sha1) = match &cfg.game.rom {
            Some(name) => {
                let bytes = read(&assets.join(name), "ROM")?;
                if bytes.len() != 0x4000 {
                    return Err(format!(
                        "ROM {name} is {} bytes, expected 16384",
                        bytes.len()
                    ));
                }
                let hash = sha1_hex(&bytes);
                check_hash("ROM", &hash, cfg.game.rom_sha1.as_deref())?;
                (Some(bytes), Some(hash))
            }
            None => (None, None),
        };
        Ok(Inputs {
            start,
            tape_sha1,
            rom,
            rom_sha1,
        })
    }

    /// Memory image the analysis starts from: RAM from the tape, plus the ROM.
    pub fn memory(&self) -> Vec<u8> {
        let mut mem = self.start.memory();
        if let Some(rom) = &self.rom {
            mem[..0x4000].copy_from_slice(rom);
        }
        mem
    }
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
