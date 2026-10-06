//! The routines of the original that have been rewritten, and how each
//! rewrite takes the original's registers (`docs/re/robin.md`).

use robin::screen;

use crate::capture::{Reg, Routine};

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
        Routine {
            name: "clear the screen (0:CEFE)",
            bank: Some(0),
            entry: 0xCEFE,
            code: (0xCEFE, 0xCF0B),
            outputs: &[],
            preserves: &[],
            rewrite: |g, _, r| {
                screen::clear_screen(g);
                r
            },
        },
        Routine {
            name: "fill the attributes (0:CF0C)",
            bank: Some(0),
            entry: 0xCF0C,
            code: (0xCF0C, 0xCF18),
            outputs: &[],
            preserves: &[],
            rewrite: |g, _, r| {
                screen::fill_attrs(g, r.get(Reg::A));
                r
            },
        },
        Routine {
            name: "clear the play area (0:CF19)",
            bank: Some(0),
            entry: 0xCF19,
            code: (0xCF19, 0xCF32),
            outputs: &[],
            preserves: &[],
            rewrite: |g, _, r| {
                screen::clear_play_area(g, r.get(Reg::A));
                r
            },
        },
        Routine {
            name: "clear the changed-cell map (0:CF33)",
            bank: Some(0),
            entry: 0xCF33,
            code: (0xCF33, 0xCF40),
            outputs: &[],
            preserves: &[],
            rewrite: |g, _, r| {
                screen::clear_changed(g);
                r
            },
        },
        Routine {
            name: "clear the lower panel (0:CF41)",
            bank: Some(0),
            entry: 0xCF41,
            code: (0xCF41, 0xCF5E),
            outputs: &[],
            preserves: &[],
            rewrite: |g, _, r| {
                screen::clear_lower_panel(g);
                r
            },
        },
        Routine {
            name: "flush the changed cells (0:C754)",
            bank: Some(0),
            entry: 0xC754,
            code: (0xC754, 0xC7AE),
            outputs: &[],
            preserves: &[],
            rewrite: |g, _, r| {
                screen::flush(g);
                r
            },
        },
        Routine {
            name: "copy the play area's pixels (0:C6FE)",
            bank: Some(0),
            entry: 0xC6FE,
            code: (0xC6FE, 0xC753),
            outputs: &[],
            preserves: &[],
            rewrite: |g, _, r| {
                screen::copy_pixels(g);
                r
            },
        },
        Routine {
            name: "copy the play area's attributes (0:C086)",
            bank: Some(0),
            entry: 0xC086,
            code: (0xC086, 0xC0A3),
            outputs: &[],
            preserves: &[],
            rewrite: |g, _, r| {
                screen::copy_attrs(g);
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
