//! Command-line driver: recompile a game to a file, or trace it and take
//! screenshots to check the analysis.
//!
//! ```text
//! zx-recomp <config.toml> [--assets DIR] [--out FILE] [--misses FILE]
//!           [--shot FRAME:FILE.png]... [--trace-only]
//! ```

use std::path::PathBuf;

use zx_recomp::{Config, Inputs, analysis, read_misses, report, tracer};
use zx_runtime::{png, screen};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut config = None;
    let mut assets = None;
    let mut misses = None;
    let mut shots: Vec<(u32, PathBuf)> = Vec::new();
    let mut trace_only = false;
    let mut listing = None;
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--assets" => assets = Some(PathBuf::from(value()?)),
            "--listing" => listing = Some(PathBuf::from(value()?)),
            "--misses" => misses = Some(PathBuf::from(value()?)),
            "--shot" => {
                let v = value()?;
                let (frame, path) = v.split_once(':').ok_or("--shot takes FRAME:FILE")?;
                shots.push((frame.parse().map_err(|_| "bad frame")?, path.into()));
            }
            "--trace-only" => trace_only = true,
            _ if a.starts_with("--") => return Err(format!("unknown option {a}")),
            _ if config.is_none() => config = Some(PathBuf::from(a)),
            _ => return Err(format!("unexpected argument {a}")),
        }
    }
    let config = config.ok_or("usage: zx-recomp <config.toml> [options]")?;
    let text =
        std::fs::read_to_string(&config).map_err(|e| format!("{}: {e}", config.display()))?;
    let cfg = Config::parse(&text)?;
    let assets = assets.unwrap_or_else(|| PathBuf::from("assets"));
    let inputs = Inputs::load(&cfg, &assets)?;
    println!("tape {} (sha1 {})", cfg.game.tape, inputs.tape_sha1);

    let start = std::time::Instant::now();
    let mut frame_buf = vec![0u32; screen::WIDTH * screen::HEIGHT];
    for (f, path) in &shots {
        if *f >= cfg.trace.frames {
            return Err(format!(
                "--shot {f}:{} is at or past the {} frames traced",
                path.display(),
                cfg.trace.frames
            ));
        }
    }
    let mut shot_errors = Vec::new();
    let traced = tracer::run(&cfg.trace, &inputs, |frame, z| {
        for (f, path) in &shots {
            if *f == frame {
                screen::render(z, &mut frame_buf);
                let png = png::encode(&frame_buf, screen::WIDTH, screen::HEIGHT);
                if let Err(e) = std::fs::write(path, png) {
                    shot_errors.push(format!("{}: {e}", path.display()));
                }
            }
        }
    })?;
    if let Some(e) = shot_errors.first() {
        return Err(e.clone());
    }
    println!(
        "traced {} frames in {:.2?}",
        cfg.trace.frames,
        start.elapsed()
    );
    let z = &traced.machine;
    println!(
        "final pc={:04x} sp={:04x} af={:04x} bc={:04x} de={:04x} hl={:04x} ix={:04x} iy={:04x} im={} iff1={}",
        z.pc,
        z.sp,
        z.af(),
        z.bc(),
        z.de(),
        z.hl(),
        z.ix,
        z.iy,
        z.im,
        z.iff1
    );
    if trace_only {
        let rom_entries: Vec<String> = (0..0x4000)
            .filter(|&a| traced.trace.entries[a])
            .map(|a| format!("{a:04x}"))
            .collect();
        println!("ROM entry points reached: {}", rom_entries.join(" "));
        return Ok(());
    }

    let extra = misses.map(|p| read_misses(&p)).unwrap_or_default();
    let analysis = analysis::analyze(
        &cfg,
        &inputs.memory(),
        inputs.start.pc,
        inputs.rom.is_some(),
        &traced.trace,
        &extra,
    );
    print!("{}", report(&analysis));
    if let Some(path) = listing {
        let text = zx_recomp::listing::listing(&analysis, &traced.trace, 0x5b00, 0xffff);
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
