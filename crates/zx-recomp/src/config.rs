//! Per-game recompiler configuration (the `<game>.toml` file).
//!
//! The config holds everything game-specific except the game itself: file
//! names and hashes of the user-supplied tape and ROM, where the tape's
//! program starts, analysis hints,
//! and the scripted input used to trace the game.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub game: Game,
    #[serde(default)]
    pub analysis: Analysis,
    #[serde(default)]
    pub trace: Trace,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Game {
    pub name: String,
    /// Tape file name (`.tap`), looked up in the assets directory.
    pub tape: String,
    /// Expected SHA-1 of the tape. Builds fail on a mismatch.
    pub tape_sha1: Option<String>,
    /// Where the ROM's loader returns into the program once the tape has
    /// loaded, and the stack pointer then.
    pub entry_pc: u16,
    pub entry_sp: u16,
    /// 48K ROM file name. Without it, ROM code cannot run.
    pub rom: Option<String>,
    pub rom_sha1: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Analysis {
    /// Extra code entry points (targets of computed jumps and the like).
    #[serde(default)]
    pub entry_points: Vec<u16>,
    /// Call/RST targets that do not return to the instruction after the call
    /// (for example ROM `RST 8` error reports, or routines that read inline
    /// data after the call).
    #[serde(default)]
    pub noreturn: Vec<u16>,
    /// Routines that print the `0xFF`-terminated string following the CALL
    /// and return after it.
    #[serde(default)]
    pub inline_strings: Vec<u16>,
    /// Inclusive address ranges that are always interpreted, never compiled.
    #[serde(default)]
    pub interpret: Vec<[u16; 2]>,
    /// Longest block, in instructions, before it is split.
    #[serde(default = "default_max_block")]
    pub max_block_instrs: usize,
}

fn default_max_block() -> usize {
    256
}

impl Default for Analysis {
    fn default() -> Self {
        Analysis {
            entry_points: Vec::new(),
            noreturn: Vec::new(),
            inline_strings: Vec::new(),
            interpret: Vec::new(),
            max_block_instrs: default_max_block(),
        }
    }
}

/// Headless run used to discover code, computed jump targets and
/// self-modifying code.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    #[serde(default = "default_frames")]
    pub frames: u32,
    /// Scripted key presses.
    #[serde(default)]
    pub input: Vec<InputEvent>,
    /// Random input to explore more of the game.
    pub random: Option<RandomInput>,
}

fn default_frames() -> u32 {
    3000
}

impl Default for Trace {
    fn default() -> Self {
        Trace {
            frames: default_frames(),
            input: Vec::new(),
            random: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputEvent {
    /// Frame at which the keys go down.
    pub at: u32,
    /// Frames to hold them.
    #[serde(default = "default_hold")]
    pub hold: u32,
    /// Key names as accepted by `zx_runtime::keys::Key::by_name`.
    pub keys: Vec<String>,
}

fn default_hold() -> u32 {
    3
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RandomInput {
    /// First frame of random input.
    pub start: u32,
    /// Frames between changes of the held keys.
    #[serde(default = "default_every")]
    pub every: u32,
    /// Keys to choose from. Each change holds up to two of them.
    pub keys: Vec<String>,
    #[serde(default)]
    pub seed: u64,
}

fn default_every() -> u32 {
    8
}

impl Config {
    /// Reads a recompiler configuration from TOML.
    ///
    /// # Errors
    ///
    /// If the text is not valid TOML, or does not match the schema.
    pub fn parse(text: &str) -> Result<Config, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }
}
