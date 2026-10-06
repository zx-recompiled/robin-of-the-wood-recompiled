//! The routines of the original that have been rewritten, and how each
//! rewrite takes the original's registers (`docs/re/robin.md`).

use robin::screen;

use crate::capture::Routine;

/// The tables the start-up code builds, and must never be rewritten after.
pub const TABLES: [(&str, u16, u16); 2] = [
    ("the mirror table", 0xFD00, 0xFDFF),
    ("the row table", 0xFE00, 0xFF7F),
];

pub fn all() -> Vec<Routine> {
    vec![
        Routine {
            name: "build the mirror table (0:CEC8)",
            bank: Some(0),
            entry: 0xCEC8,
            code: (0xCEC8, 0xCEDD),
            outputs: &[],
            preserves: &[],
            rewrite: |g, _, r| {
                screen::build_mirror(g);
                r
            },
        },
        Routine {
            name: "build the row table (0:CEDE)",
            bank: Some(0),
            entry: 0xCEDE,
            code: (0xCEDE, 0xCEFD),
            outputs: &[],
            preserves: &[],
            rewrite: |g, _, r| {
                screen::build_rows(g);
                r
            },
        },
    ]
}

/// The routines allowed to write the start-up tables: their builders.
pub fn may_write_tables(routines: &[Routine], pc: u16) -> bool {
    routines
        .iter()
        .take(2)
        .any(|r| (r.code.0..=r.code.1).contains(&pc))
}
