//! The playable program: the rewritten game in a window, with the keyboard
//! (#66). Sound is #67's.
//!
//! Usage: `robin [TAPE]`, or `robin [TAPE] --headless FRAMES [DIR]` to play a
//! scripted run without a window and write screenshots to `DIR`.

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
    let path = match args
        .first()
        .map(PathBuf::from)
        .or_else(frontend::tape::find)
    {
        Some(path) => path,
        None => {
            eprintln!("{}", frontend::tape::NOT_FOUND);
            std::process::exit(1);
        }
    };
    let result = match headless {
        Some((frames, dir)) => frontend::headless::run(&path, frames, &dir),
        None => frontend::video::run(&path),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
