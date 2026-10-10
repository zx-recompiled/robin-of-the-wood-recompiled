//! The playable program: the rewritten game in a window, with the keyboard
//! (#66) and its sound (#67), or the screen that asks for the tape
//! when none is found (#70).
//!
//! Usage: `robin [TAPE]`, or `robin [TAPE] --headless FRAMES [DIR]` to play a
//! scripted run without a window and write screenshots and its sound to
//! `DIR`.

mod frontend;

use std::path::PathBuf;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let headless = args.iter().position(|a| a == "--headless").map(|i| {
        let rest: Vec<String> = args.drain(i..).skip(1).collect();
        let frames = rest.first().and_then(|f| f.parse().ok()).unwrap_or(3000);
        let dir = rest
            .get(1)
            .map_or_else(|| PathBuf::from("screenshots"), PathBuf::from);
        (frames, dir)
    });
    // A tape named on the command line is used as it is; otherwise the
    // usual places are searched.
    let tape = match args.first() {
        Some(path) => match frontend::tape::read(&PathBuf::from(path)) {
            Ok(bytes) => Some(bytes),
            Err(e) => fail(&e),
        },
        None => {
            let folders = frontend::tape::folders();
            frontend::tape::find(&folders, robin::is_the_tape).map(|t| t.bytes)
        }
    };
    let result = match headless {
        // Without a window there's nobody to ask, so no tape is the end.
        Some((frames, dir)) => match &tape {
            Some(bytes) => frontend::headless::run(bytes, frames, &dir),
            None => fail(&frontend::tape::not_found_message(
                &frontend::tape::folders(),
            )),
        },
        // In a window, no tape means asking for one.
        None => frontend::video::run(tape),
    };
    if let Err(e) = result {
        fail(&format!("error: {e}"));
    }
}

/// Says what went wrong, and gives up.
fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}
