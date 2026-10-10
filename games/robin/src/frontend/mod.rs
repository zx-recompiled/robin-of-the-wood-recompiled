//! The window, the keyboard and the headless runner around a
//! [`robin::session::Session`] (#66), adapted from starquake-recompiled's
//! frontend (`REUSED.md`).

pub mod headless;
pub mod input;
pub mod tape;
pub mod video;

/// The 128K's frame rate: 70,908 T-states at 3.5469 MHz.
pub const FRAMES_PER_SECOND: f64 = 3_546_900.0 / 70_908.0;
