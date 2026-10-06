//! Runs the game headless in the interpreter with scripted input and records
//! what executes. Static analysis alone cannot see the targets of computed
//! jumps or which code rewrites itself; a trace can.

use crate::Inputs;
use crate::config;
use crate::script::Script;
use zx_runtime::trace::Trace;
use zx_runtime::{Misses, Zx, no_code};

pub struct TraceResult {
    pub trace: Trace,
    /// Final machine state, for inspection (screenshots in the CLI).
    pub machine: Zx,
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
    let mut script = Script::new(cfg)?;
    let mut z = inputs.machine.clone();
    z.trace = Some(Box::new(Trace::new(z.memory.pages())));
    let mut misses = Misses::default();

    for frame in 0..cfg.frames {
        script.press(frame, &mut z);
        z.run_frame(no_code, &mut misses);
        on_frame(frame, &z);
    }

    let trace = *z.trace.take().expect("trace enabled");
    Ok(TraceResult { trace, machine: z })
}
