//! Runs the game headless in the interpreter with scripted input and records
//! what executes. Static analysis alone cannot see the targets of computed
//! jumps or which code rewrites itself; a trace can.

use crate::Inputs;
use crate::config;
use zx_runtime::keys::Key;
use zx_runtime::trace::Trace;
use zx_runtime::{Misses, Zx, no_code};

pub struct TraceResult {
    pub trace: Trace,
    /// Final machine state, for inspection (screenshots in the CLI).
    pub machine: Zx,
}

fn keys(names: &[String]) -> Result<Vec<Key>, String> {
    names
        .iter()
        .map(|n| Key::by_name(n).ok_or_else(|| format!("unknown key name {n:?} in trace config")))
        .collect()
}

/// xorshift64*; deterministic so builds are reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

/// Runs the interpreter for the configured number of frames, calling
/// `on_frame` with the machine at each frame boundary.
///
/// # Errors
///
/// If the configuration names a key the Spectrum does not have.
///
/// # Panics
///
/// If the machine was built without tracing enabled, which this function
/// does itself, so only a change here can cause it.
pub fn run(
    cfg: &config::Trace,
    inputs: &Inputs,
    mut on_frame: impl FnMut(u32, &Zx),
) -> Result<TraceResult, String> {
    let scripted: Vec<(u32, u32, Vec<Key>)> = cfg
        .input
        .iter()
        .map(|e| Ok((e.at, e.hold, keys(&e.keys)?)))
        .collect::<Result<_, String>>()?;
    let random = match &cfg.random {
        Some(r) => Some((r, keys(&r.keys)?)),
        None => None,
    };
    let mut rng = Rng(random.as_ref().map_or(1, |(r, _)| r.seed) | 1);
    let mut held_random: Vec<Key> = Vec::new();

    let mut z = Zx::new(&inputs.start, inputs.rom.as_deref());
    z.trace = Some(Box::default());
    let mut misses = Misses::default();

    for frame in 0..cfg.frames {
        z.release_all_keys();
        for (at, hold, keys) in &scripted {
            if frame >= *at && frame < at.saturating_add(*hold) {
                for &k in keys {
                    z.set_key(k, true);
                }
            }
        }
        if let Some((r, choices)) = &random
            && frame >= r.start
            && !choices.is_empty()
        {
            if (frame - r.start) % r.every.max(1) == 0 {
                held_random.clear();
                for _ in 0..(rng.next() % 3) {
                    held_random.push(choices[(rng.next() % choices.len() as u64) as usize]);
                }
            }
            for &k in &held_random {
                z.set_key(k, true);
            }
        }
        z.run_frame(no_code, &mut misses);
        on_frame(frame, &z);
    }

    let trace = *z.trace.take().expect("trace enabled");
    Ok(TraceResult { trace, machine: z })
}
