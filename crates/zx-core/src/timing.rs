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

/// The shape of a machine's frame: what the ULA's timing is made of.
///
/// A 48K Spectrum and a 128K one differ in their clock, how long a line and a
/// frame are, when the ULA starts fetching the screen, and how long it holds
/// the interrupt. Each value names its source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    /// The processor's clock, in Hz.
    pub cpu_hz: u32,
    /// T-states between interrupts.
    pub frame: u32,
    /// T-states in a line of the picture.
    pub line: u32,
    /// The T-state of the first contended access of the first drawn line.
    pub first_contended: u32,
    /// How long the ULA holds /INT low at the start of each frame.
    pub int_len: u32,
}

impl Timing {
    /// T-states the ULA adds to an access at T-state `t` in the frame.
    ///
    /// While it is drawing a line the ULA is reading the screen itself, and it
    /// holds the processor off the bus rather than share. It needs two bytes
    /// (a bitmap byte and its attribute) out of every eight T-states, so the
    /// delay counts down 6,5,4,3,2,1,0,0 and starts again. Only the 128
    /// T-states of each line where it is fetching are contended; the rest of
    /// the line, the border and the retrace are free. The same on both
    /// machines; only where the lines start and how long they are differ.
    ///
    /// What this applies to is the caller's business: the contended memory,
    /// and I/O on its own pattern.
    #[must_use]
    pub const fn contention(&self, t: u32) -> u32 {
        const FETCHING: u32 = 128;
        const LINES: u32 = 192;
        const PATTERN: [u32; 8] = [6, 5, 4, 3, 2, 1, 0, 0];

        // The pattern repeats every frame, and `t` is not always kept inside
        // one: a routine can accumulate millions of T-states without a frame
        // boundary. Taking it modulo the frame is what the ULA does anyway.
        let into_frame = t % self.frame;
        if into_frame < self.first_contended {
            return 0;
        }
        let since = into_frame - self.first_contended;
        if since >= LINES * self.line {
            return 0;
        }
        let into_line = since % self.line;
        if into_line >= FETCHING {
            return 0;
        }
        PATTERN[(into_line % 8) as usize]
    }
}

/// A 48K Spectrum, as this project has always modelled it. Checked against
/// the Fuse corpus's record of every contended access, and the interrupt's
/// length (32) matches Patrik Rak's `minfo` on a real 48K.
pub const SPECTRUM_48: Timing = Timing {
    cpu_hz: CPU_HZ,
    frame: FRAME_T,
    line: 224,
    first_contended: 14335,
    int_len: 32,
};

/// The 128K Spectrum and grey +2 with a Zilog processor, the machine Robin of
/// the Wood is checked on.
///
/// - `cpu_hz`: 3,546,900 Hz, from Fuse's machine definitions (libspectrum
///   `timings.c`). A written value.
/// - `frame`: 70,908, measured by Brendan Alford with Patrik Rak's `minfo` on
///   a toastrack 128K and a grey +2 (World of Spectrum forums, comment
///   757503). It agrees with the written references.
/// - `line`: 228, from the World of Spectrum 128K reference and Fuse. Written;
///   311 lines of it make the measured frame.
/// - `first_contended`: 14,362, measured: Rak's Timing Test on a real grey +2
///   (Víctor Iborra's +2 ES in 128K mode, photographed 2023-07-05, redcode
///   ZXSpectrum wiki, *Timing Test*) shows the contended-NOP pattern one
///   T-state later than the written references' 14,361, and Brendan Alford's
///   `btime` on a 128K agrees (14,362).
/// - `int_len`: 35, measured with `minfo` on a Zilog toastrack 128K. A NEC
///   grey +2 measured 34, and Fuse's written figure is 36.
pub const SPECTRUM_128: Timing = Timing {
    cpu_hz: 3_546_900,
    frame: 70908,
    line: 228,
    first_contended: 14362,
    int_len: 35,
};

/// T-states the ULA adds to an access at T-state `t` in a 48K frame: see
/// [`Timing::contention`].
#[must_use]
pub const fn contention(t: u32) -> u32 {
    SPECTRUM_48.contention(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_128k_frame_is_311_lines() {
        assert_eq!(SPECTRUM_128.frame, 311 * SPECTRUM_128.line);
        assert_eq!(SPECTRUM_48.frame, 312 * SPECTRUM_48.line);
    }

    #[test]
    fn contention_starts_and_ends_where_each_machine_says() {
        for m in [SPECTRUM_48, SPECTRUM_128] {
            let first = m.first_contended;
            assert_eq!(m.contention(first - 1), 0);
            assert_eq!(m.contention(first), 6);
            assert_eq!(m.contention(first + 7), 0);
            assert_eq!(m.contention(first + 8), 6);
            // The last fetch of the first line, and the start of the second.
            assert_eq!(m.contention(first + 127), 0);
            assert_eq!(m.contention(first + 128), 0);
            assert_eq!(m.contention(first + m.line), 6);
            // After the 192nd line, nothing.
            assert_eq!(m.contention(first + 191 * m.line), 6);
            assert_eq!(m.contention(first + 192 * m.line), 0);
            // A frame later, the same again.
            assert_eq!(m.contention(first + m.frame), 6);
        }
    }

    #[test]
    fn a_frame_is_just_under_20ms() {
        assert_eq!(FRAME_NANOS, 19_968_000);
        assert!((FRAME_NANOS as f64 / 1e9 * 50.0 - 0.9984).abs() < 1e-9);
    }
}
