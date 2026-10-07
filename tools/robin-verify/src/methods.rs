//! The control methods play never uses (#34, Decision 3). For each, the
//! original boots again from the tape, the method is chosen at the menu as
//! a player would choose it, and a short game is played with it: random
//! joystick moves, or random presses of the keys it reads. Choosing a
//! method plays the title sequence again, which reads no keys, so 0 is
//! pressed several times to start the game. Every call to a rewritten
//! routine on the way is a case, as in play.

use robin::assets::Assets;
use robin::movement::{KEMPSTON, REDEFINED_KEYS, SINCLAIR};
use zx_recomp::script::Script;
use zx_runtime::{Misses, Zx, loader::boot_128k};

use crate::Verifier;
use crate::capture::Play;

/// How many frames each method is played for, from the hand-over.
const FRAMES: u32 = 2000;

/// A method: its name, the script that chooses it and plays with it, and
/// what the game must hold once it has been chosen.
struct Method {
    name: &'static str,
    script: &'static str,
    address: u16,
    /// Whether it sets new keys through the menu's screen.
    redefines: bool,
}

/// The tape's own keys: fire, up, down, left, right.
const TAPE_KEYS: [u8; 5] = [0x24, 0x25, 0x26, 0x08, 0x10];

const METHODS: [Method; 3] = [
    Method {
        name: "the Kempston joystick (menu key 2)",
        script: r#"[game]
name = "kempston"
tape = "-"
[trace]
frames = 2000
[[trace.input]]
at = 300
hold = 5
keys = ["2"]
[[trace.input]]
at = 400
hold = 5
keys = ["0"]
[[trace.input]]
at = 600
hold = 5
keys = ["0"]
[[trace.input]]
at = 800
hold = 5
keys = ["0"]
[[trace.input]]
at = 1000
hold = 5
keys = ["0"]
[trace.random]
start = 450
every = 20
keys = ["joy_right", "joy_left", "joy_down", "joy_up", "joy_fire"]
seed = 21
"#,
        address: KEMPSTON,
        redefines: false,
    },
    Method {
        name: "the Sinclair joystick (menu key 3)",
        script: r#"[game]
name = "sinclair"
tape = "-"
[trace]
frames = 2000
[[trace.input]]
at = 300
hold = 5
keys = ["3"]
[[trace.input]]
at = 400
hold = 5
keys = ["0"]
[[trace.input]]
at = 600
hold = 5
keys = ["0"]
[[trace.input]]
at = 800
hold = 5
keys = ["0"]
[[trace.input]]
at = 1000
hold = 5
keys = ["0"]
[trace.random]
start = 450
every = 20
keys = ["6", "7", "8", "9", "0"]
seed = 22
"#,
        address: SINCLAIR,
        redefines: false,
    },
    Method {
        name: "redefined keys, set through the menu (menu key 1)",
        // Fire, up, down, left and right become P, O, K, L and M, typed on
        // the menu's own screen with the keys let go in between.
        script: r#"[game]
name = "redefined"
tape = "-"
[trace]
frames = 2000
[[trace.input]]
at = 300
hold = 5
keys = ["1"]
[[trace.input]]
at = 360
hold = 5
keys = ["p"]
[[trace.input]]
at = 400
hold = 5
keys = ["o"]
[[trace.input]]
at = 440
hold = 5
keys = ["k"]
[[trace.input]]
at = 480
hold = 5
keys = ["l"]
[[trace.input]]
at = 520
hold = 5
keys = ["m"]
[[trace.input]]
at = 600
hold = 5
keys = ["0"]
[[trace.input]]
at = 800
hold = 5
keys = ["0"]
[[trace.input]]
at = 1000
hold = 5
keys = ["0"]
[trace.random]
start = 650
every = 20
keys = ["p", "o", "k", "l", "m"]
seed = 23
"#,
        address: REDEFINED_KEYS,
        redefines: true,
    },
];

/// Plays each method in turn. Returns what went wrong, if anything: a
/// method the game did not take up.
pub fn run(
    rom: &[u8],
    tape: &[u8],
    assets: &Assets,
    v: &mut Verifier,
) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    for m in &METHODS {
        let cfg = zx_recomp::Config::parse(m.script)?;
        let mut script = Script::new(&cfg.trace)?;
        let blocks = zx_core::tape::load_tzx(tape)?;
        let mut z = boot_128k(rom, blocks, robin::layout::ENTRY_PC, 2000)?;
        let mut misses = Misses::default();
        for frame in 0..FRAMES {
            script.press(frame, &mut z);
            z.run_frame(
                |z: &mut Zx| {
                    v.observe(
                        z,
                        &Play {
                            script: &script,
                            frame,
                            assets,
                        },
                    );
                    false
                },
                &mut misses,
            );
        }
        let chosen = z.read16(0xD152);
        let keys: [u8; 5] = std::array::from_fn(|i| z.read(0xD154 + i as u16));
        println!("robin-verify: played {FRAMES} frames with {}", m.name);
        if chosen != m.address {
            problems.push(format!(
                "{}: the game was left using the method at {chosen:#06x}, not {:#06x}",
                m.name, m.address
            ));
        }
        let distinct = (0..5).all(|i| (0..i).all(|j| keys[i] != keys[j]));
        if m.redefines && (keys == TAPE_KEYS || !distinct) {
            problems.push(format!(
                "{}: the keys are {keys:02x?}, not five new ones set through the menu",
                m.name
            ));
        }
    }
    Ok(problems)
}
