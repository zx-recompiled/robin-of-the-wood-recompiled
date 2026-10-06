//! What a ZX Spectrum frame is made of.
//!
//! Both the reference interpreter and the game work in T-states, the
//! processor's clock ticks, because that is what the original's sound and
//! timing are built from. Keeping the numbers here means the two cannot
//! drift apart.

/// The Z80's clock on a 48K Spectrum, in Hz.
pub const CPU_HZ: u32 = 3_500_000;

/// T-states in one frame. The ULA gives the processor this many between
/// interrupts, and the game's whole sense of time comes from it.
pub const FRAME_T: u32 = 69888;

/// Frames in a second, near enough for pacing. A frame is really 69888 /
/// 3500000 of a second, so the true rate is 50.08 Hz: use [`FRAME_T`] and
/// [`CPU_HZ`] where the difference matters.
pub const FRAMES_PER_SECOND: u32 = 50;

/// How long a frame lasts, to the nanosecond. Not quite 20ms, and the
/// difference is a game running 0.16% slow or fast.
pub const FRAME_NANOS: u64 = FRAME_T as u64 * 1_000_000_000 / CPU_HZ as u64;

/// T-states the ULA adds to an access at T-state `t` in the frame.
///
/// While it is drawing a line the ULA is reading the screen itself, and it
/// holds the processor off the bus rather than share. It needs two bytes (a
/// bitmap byte and its attribute) out of every eight T-states, so the delay
/// counts down 6,5,4,3,2,1,0,0 and starts again. Only the 128 T-states of
/// each line where it is fetching are contended; the rest of the line, the
/// border and the retrace are free.
///
/// What this applies to is the caller's business: memory in 0x4000..0x8000,
/// and I/O on its own pattern.
#[must_use]
pub const fn contention(t: u32) -> u32 {
    /// The T-state of the first contended access of the first drawn line.
    const FIRST: u32 = 14335;
    /// T-states in a line, and how many of them the ULA is fetching for.
    const LINE: u32 = 224;
    const FETCHING: u32 = 128;
    const LINES: u32 = 192;
    const PATTERN: [u32; 8] = [6, 5, 4, 3, 2, 1, 0, 0];

    // The pattern repeats every frame, and `t` is not always kept inside one:
    // a routine can accumulate millions of T-states without a frame boundary.
    // Taking it modulo the frame is what the ULA does anyway.
    let into_frame = t % FRAME_T;
    if into_frame < FIRST {
        return 0;
    }
    let since = into_frame - FIRST;
    if since >= LINES * LINE {
        return 0;
    }
    let into_line = since % LINE;
    if into_line >= FETCHING {
        return 0;
    }
    PATTERN[(into_line % 8) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_just_under_20ms() {
        assert_eq!(FRAME_NANOS, 19_968_000);
        assert!((FRAME_NANOS as f64 / 1e9 * 50.0 - 0.9984).abs() < 1e-9);
    }
}
