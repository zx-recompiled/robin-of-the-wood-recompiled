//! The aids panel beside the game, and the picker over it (#92, #95), drawn
//! to the approved mockups, as starquake-recompiled draws its guidance
//! panel (`REUSED.md`).
//!
//! Everything is laid out in logical pixels of the whole window, the
//! Spectrum's own: the picture takes the left [`PICTURE_W`], the panel the
//! rest. The overlay's canvas scales them to the window.

use super::aids::{ALL, Aid, Aids};
use super::journal::Journal;
use super::text::{Canvas, Fonts, Rgb, Span, Weight};
use robin::map::{COLUMNS, ROWS};
use robin::objective::{self, Next, Objective};
use robin::places::{self, CHARACTERS, Character};

/// The picture's width with its border, and the panel's beside it. The
/// window is a whole multiple of both, so the picture scales exactly.
pub const PICTURE_W: f32 = robin::picture::FULL_W as f32;
pub const PANEL_W: f32 = 136.0;
pub const WINDOW_W: f32 = PICTURE_W + PANEL_W;
pub const WINDOW_H: f32 = robin::picture::FULL_H as f32;

const BACKGROUND: Rgb = [0x11, 0x13, 0x18];
const EDGE: Rgb = [0x23, 0x26, 0x2E];
const TEXT: Rgb = [0xD7, 0xDB, 0xE3];
const DIM: Rgb = [0x8A, 0x90, 0xA0];
const FAINT: Rgb = [0x5D, 0x63, 0x72];
const WHITE: Rgb = [0xFF, 0xFF, 0xFF];
const BLUE: Rgb = [0x6B, 0xA5, 0xF0];
const CARD: Rgb = [0x13, 0x15, 0x1B];
const CARD_EDGE: Rgb = [0x2B, 0x2F, 0x3A];
const TRACK_OFF: Rgb = [0x2A, 0x2E, 0x38];
const TRACK_ON: Rgb = [0x3A, 0x6F, 0xD1];
const KNOB_OFF: Rgb = [0x7D, 0x83, 0x94];
const NEST: Rgb = [0x2B, 0x3B, 0x5C];

/// The panel's inside, from its left edge.
const PAD: f32 = 8.0;

fn span(text: &str, size: f32, weight: Weight, colour: Rgb) -> Span<'_> {
    Span {
        text,
        size,
        weight,
        colour,
    }
}

/// What the panel shows of the game: the journal, and where its places are
/// this game (#96).
#[derive(Default)]
pub struct View<'a> {
    pub journal: Option<&'a Journal>,
    /// His gold and what the trade has given him, in a game (#98).
    pub objective: Option<Objective>,
    /// Whether he's met the hermit this game (#99).
    pub hermit_met: bool,
    pub trade: u16,
    pub doorways: [u16; 9],
}

/// A place the map marks once Robin has been there: its colour and name.
const PLACES: [(Rgb, &str); 5] = [
    ([0x5F, 0xD6, 0x8A], "the Ent"),
    ([0xB5, 0x8C, 0xFF], "witch's doorway"),
    ([0x6B, 0xA5, 0xF0], "castle"),
    ([0x9A, 0xA4, 0xB2], "dungeon"),
    ([0xE3, 0xC0, 0x6B], "tournament"),
];

impl View<'_> {
    /// The place at `location`, if it's one the map marks.
    fn place(&self, location: u16) -> Option<usize> {
        if location == self.trade {
            Some(0)
        } else if self.doorways.contains(&location) {
            Some(1)
        } else if location == places::CASTLE {
            Some(2)
        } else if location == places::DUNGEON {
            Some(3)
        } else if location == places::ENDING {
            Some(4)
        } else {
            None
        }
    }
}

/// Each character's colour and name on the map (#97).
fn character(who: Character) -> (Rgb, &'static str) {
    match who {
        Character::Bishop => ([0xF0, 0x6B, 0xA8], "bishop"),
        Character::Sheriff => ([0xF0, 0x86, 0x4B], "Sheriff"),
        Character::Hermit => ([0x5F, 0xD6, 0xC8], "hermit"),
    }
}

/// Which characters' marks the map shows: where last seen, and where now.
#[derive(Clone, Copy, Default)]
struct Marks {
    seen: bool,
    now: bool,
}

/// The characters' marks `aids` asks for.
fn marks(aids: &Aids) -> Marks {
    Marks {
        seen: aids.is_on(Aid::Seen),
        now: aids.is_on(Aid::Now),
    }
}

/// How far `to` is from `from`, the shortest way round the forest, in
/// words: "here", or "3 east, 2 north".
fn direction(from: u16, to: u16) -> String {
    let short = |d: i32, n: i32| {
        let d = d.rem_euclid(n);
        if d > n / 2 { d - n } else { d }
    };
    let (cols, rows) = (i32::from(COLUMNS), i32::from(ROWS));
    let dx = short(i32::from(to % COLUMNS) - i32::from(from % COLUMNS), cols);
    let dy = short(i32::from(to / COLUMNS) - i32::from(from / COLUMNS), rows);
    let mut parts = Vec::new();
    if dx != 0 {
        parts.push(format!(
            "{} {}",
            dx.abs(),
            if dx > 0 { "east" } else { "west" }
        ));
    }
    if dy != 0 {
        parts.push(format!(
            "{} {}",
            dy.abs(),
            if dy > 0 { "south" } else { "north" }
        ));
    }
    if parts.is_empty() {
        "here".to_string()
    } else {
        parts.join(", ")
    }
}

const UNSEEN: Rgb = [0x1A, 0x1D, 0x24];
const SEEN: Rgb = [0x2B, 0x3B, 0x5C];

pub struct Panel {
    fonts: Fonts,
}

impl Panel {
    pub fn new() -> Panel {
        Panel {
            fonts: Fonts::load(),
        }
    }

    /// Draws the panel for `aids` and what's in `view`, and over everything
    /// the picker or the whole map, if one is open.
    pub fn draw(&mut self, canvas: &mut Canvas, aids: &Aids, view: &View) {
        canvas.clear_transparent();
        canvas.round_rect(PICTURE_W, 0.0, PANEL_W, WINDOW_H, 0.0, BACKGROUND);
        canvas.round_rect(PICTURE_W, 0.0, 0.4, WINDOW_H, 0.0, EDGE);
        let left = PICTURE_W + PAD;
        let width = PANEL_W - 2.0 * PAD;
        self.spaced(canvas, left, 7.0, "AIDS", DIM);
        self.key_cap(canvas, PICTURE_W + PANEL_W - PAD, 6.5, "F1");
        if aids.any() {
            self.sections(canvas, aids, view, left, width);
        } else {
            // Every aid off: the panel stays and says so (#92, Decision 10).
            self.fonts.text(
                Some(canvas),
                left,
                12.5,
                None,
                1.0,
                &[span("Off", 6.5, Weight::SemiBold, WHITE)],
            );
            let lines = [
                "No aids: the 1985 game as it was.",
                "Press F1 or Tab to choose some.",
            ];
            for (n, line) in lines.iter().enumerate() {
                let s = [span(line, 4.4, Weight::Regular, DIM)];
                let w = self.fonts.measure(&s);
                let x = PICTURE_W + (PANEL_W - w) / 2.0;
                self.fonts.text(
                    Some(canvas),
                    x,
                    WINDOW_H / 2.0 - 6.0 + n as f32 * 7.0,
                    None,
                    1.0,
                    &s,
                );
            }
        }
        if aids.picker_open() {
            self.picker(canvas, aids);
        } else if aids.full_map_open() {
            self.full_map(canvas, view, marks(aids));
        }
    }

    /// The forest's 16 × 20 locations at (`x`, `y`), each `cell` × `tall` with
    /// `gap` between: unseen, seen, and where Robin is, with the places he's
    /// been to marked.
    #[allow(clippy::too_many_arguments, reason = "where, how big, and from what")]
    fn grid(
        &mut self,
        canvas: &mut Canvas,
        view: &View,
        marks: Marks,
        x: f32,
        y: f32,
        cell: f32,
        tall: f32,
        gap: f32,
    ) {
        let Some(journal) = view.journal else {
            return;
        };
        let dot = tall.min(cell) * 0.55;
        for row in 0..ROWS {
            for col in 0..COLUMNS {
                let at = row * COLUMNS + col;
                let (cx, cy) = (
                    x + f32::from(col) * (cell + gap),
                    y + f32::from(row) * (tall + gap),
                );
                let seen = journal.visited(at);
                let colour = if journal.here == Some(at) {
                    WHITE
                } else if seen {
                    SEEN
                } else {
                    UNSEEN
                };
                canvas.round_rect(cx, cy, cell, tall, 0.6, colour);
                if seen
                    && journal.here != Some(at)
                    && let Some(p) = view.place(at)
                {
                    canvas.round_rect(
                        cx + (cell - dot) / 2.0,
                        cy + (tall - dot) / 2.0,
                        dot,
                        dot,
                        dot / 2.0,
                        PLACES[p].0,
                    );
                }
            }
        }
        // The characters, in a corner of their cell: a ring where last
        // seen, filled where they are now.
        let d = tall.min(cell) * 0.5;
        for (n, who) in CHARACTERS.into_iter().enumerate() {
            let colour = character(who).0;
            let corner = |at: u16| {
                (
                    x + f32::from(at % COLUMNS) * (cell + gap) + cell
                        - d
                        - 0.3
                        - n as f32 * (d * 0.6),
                    y + f32::from(at / COLUMNS) * (tall + gap) + 0.3,
                )
            };
            if marks.seen
                && let Some(at) = journal.last_seen(who)
            {
                let (cx, cy) = corner(at);
                canvas.outline(cx, cy, d, d, d / 2.0, d * 0.25, None, colour);
            }
            if marks.now
                && let Some(at) = journal.now(who)
            {
                let (cx, cy) = corner(at);
                canvas.round_rect(cx, cy, d, d, d / 2.0, colour);
            }
        }
    }

    /// Who Robin has seen, and where they are now, in words (#97).
    fn seen_list(
        &mut self,
        canvas: &mut Canvas,
        view: &View,
        marks: Marks,
        left: f32,
        y: f32,
    ) -> f32 {
        let Some(journal) = view.journal else {
            return y;
        };
        self.spaced(canvas, left, y, "SEEN ON THE MAP", DIM);
        let mut row = y + 5.5;
        for who in CHARACTERS {
            let (colour, name) = character(who);
            let d = 2.2;
            canvas.round_rect(left, row + 1.1, d, d, d / 2.0, colour);
            let name = format!("{}{}", name[..1].to_uppercase(), &name[1..]);
            self.fonts.text(
                Some(canvas),
                left + 3.6,
                row,
                None,
                1.0,
                &[span(&name, 4.0, Weight::Regular, TEXT)],
            );
            let seen = match (journal.last_seen(who), journal.here) {
                (Some(at), Some(here)) => direction(here, at),
                (Some(_), None) => "seen".to_string(),
                (None, _) => "not seen this game".to_string(),
            };
            let mut spans = vec![span(&seen, 4.0, Weight::Regular, DIM)];
            let now = match (marks.now, journal.now(who), journal.here) {
                (true, Some(at), Some(here)) => Some(format!(" · now {}", direction(here, at))),
                (true, None, _) => Some(" · not about".to_string()),
                _ => None,
            };
            if let Some(now) = &now {
                spans.push(span(now, 4.0, Weight::Regular, colour));
            }
            self.fonts.text(
                Some(canvas),
                left + 22.0,
                row,
                Some(PANEL_W - 2.0 * PAD - 22.0),
                1.2,
                &spans,
            );
            row += 6.0;
        }
        row + 2.0
    }

    /// The map's key, from `x` at `y`, wrapping at `width`. Where it ends.
    #[allow(clippy::too_many_arguments, reason = "where, how big, and what's on")]
    fn legend(
        &mut self,
        canvas: &mut Canvas,
        marks: Marks,
        x: f32,
        y: f32,
        width: f32,
        size: f32,
    ) -> f32 {
        let mut entries = vec![(WHITE, "you")];
        entries.extend(PLACES);
        if marks.seen {
            entries.extend(CHARACTERS.map(character));
        }
        let (mut cx, mut cy) = (x, y);
        for (colour, name) in entries {
            let s = [span(name, size, Weight::Regular, [0xA8, 0xAE, 0xBB])];
            let w = self.fonts.measure(&s) + size * 1.4;
            if cx + w > x + width {
                cx = x;
                cy += size * 1.6;
            }
            let d = size * 0.6;
            canvas.round_rect(cx, cy + size * 0.25, d, d, d / 2.0, colour);
            self.fonts
                .text(Some(canvas), cx + d + size * 0.3, cy, None, 1.0, &s);
            cx += w + size * 0.8;
        }
        cy + size * 1.6
    }

    /// The whole map, large, with the game paused (#92, Decision 9).
    fn full_map(&mut self, canvas: &mut Canvas, view: &View, marks: Marks) {
        canvas.shade(0.0, 0.0, WINDOW_W, WINDOW_H, [0x05, 0x06, 0x09], 160);
        let (cell, tall, gap) = (12.0, 8.6, 1.0);
        let grid_w = f32::from(COLUMNS) * (cell + gap) - gap;
        let grid_h = f32::from(ROWS) * (tall + gap) - gap;
        let w = grid_w + 18.0;
        let h = grid_h + 44.0;
        let (x, y) = ((WINDOW_W - w) / 2.0, (WINDOW_H - h) / 2.0);
        canvas.round_rect(x, y, w, h, 4.0, CARD_EDGE);
        canvas.round_rect(x + 0.4, y + 0.4, w - 0.8, h - 0.8, 3.7, CARD);
        let count = view.journal.map_or(0, Journal::count);
        let title = format!("Sherwood · {count} of 320 seen");
        self.fonts.text(
            Some(canvas),
            x + 9.0,
            y + 6.0,
            None,
            1.0,
            &[span(&title, 6.0, Weight::SemiBold, WHITE)],
        );
        let paused = [span("The game is paused", 4.0, Weight::Regular, DIM)];
        let pw = self.fonts.measure(&paused);
        self.fonts
            .text(Some(canvas), x + w - 9.0 - pw, y + 8.0, None, 1.0, &paused);
        self.grid(canvas, view, marks, x + 9.0, y + 16.0, cell, tall, gap);
        self.legend(canvas, marks, x + 9.0, y + 19.0 + grid_h, grid_w, 3.9);
        let foot = y + h - 9.0;
        self.fonts.text(
            Some(canvas),
            x + 9.0,
            foot + 0.8,
            None,
            1.0,
            &[span(
                "The forest wraps round at its edges.",
                3.9,
                Weight::Regular,
                DIM,
            )],
        );
        let back = [span("back to the game", 3.9, Weight::Regular, DIM)];
        let bw = self.fonts.measure(&back);
        self.fonts
            .text(Some(canvas), x + w - 9.0 - bw, foot + 0.8, None, 1.0, &back);
        self.key_cap(canvas, x + w - 11.0 - bw, foot, "`");
    }

    /// The sections for the aids that are on, top to bottom. Each aid's own
    /// contents come with its sub-issue of #92.
    fn sections(&mut self, canvas: &mut Canvas, aids: &Aids, view: &View, left: f32, width: f32) {
        let mut y = 18.0;
        if aids.is_on(Aid::Map) {
            let count = view.journal.map_or(0, Journal::count);
            self.fonts.text(
                Some(canvas),
                left,
                y - 1.0,
                None,
                1.0,
                &[span(
                    &format!("Map · {count} of 320 seen"),
                    5.6,
                    Weight::SemiBold,
                    WHITE,
                )],
            );
            self.fonts.text(
                Some(canvas),
                left,
                y + 6.0,
                None,
                1.0,
                &[span("` for the whole map", 3.7, Weight::Regular, DIM)],
            );
            let (cell, tall, gap) = (6.4, 4.0, 0.6);
            let grid_w = f32::from(COLUMNS) * (cell + gap) - gap;
            let gx = left + (width - grid_w) / 2.0;
            let m = marks(aids);
            self.grid(canvas, view, m, gx, y + 12.0, cell, tall, gap);
            y = self.legend(
                canvas,
                m,
                left,
                y + 15.0 + f32::from(ROWS) * (tall + gap),
                width,
                3.5,
            ) + 3.0;
            if m.seen {
                y = self.seen_list(canvas, view, m, left, y + 1.0) + 2.0;
            }
        }
        if aids.is_on(Aid::Objective) {
            y = self.objective(canvas, view, left, width, y);
        }
        if aids.is_on(Aid::Hints) {
            y = self.hint(canvas, view, left, width, y);
        }
        let assists: Vec<Aid> = ALL
            .into_iter()
            .filter(|a| a.is_assist() && aids.is_on(*a))
            .collect();
        if !assists.is_empty() {
            self.spaced(canvas, left, y, "ASSISTS", DIM);
            let mut x = left;
            let mut row = y + 5.0;
            for aid in assists {
                let label = aid.label().0;
                let s = [span(label, 3.8, Weight::Regular, [0xC9, 0xB2, 0xFF])];
                let w = self.fonts.measure(&s) + 5.0;
                if x + w > left + width {
                    x = left;
                    row += 7.0;
                }
                canvas.round_rect(x, row, w, 6.0, 3.0, [0x2A, 0x21, 0x40]);
                self.fonts
                    .text(Some(canvas), x + 2.5, row + 0.9, None, 1.0, &s);
                x += w + 2.0;
            }
            self.fonts.text(
                Some(canvas),
                left,
                row + 9.0,
                Some(width),
                1.3,
                &[span(
                    "The assists come with #100 and #101.",
                    3.6,
                    Weight::Regular,
                    FAINT,
                )],
            );
        }
    }

    /// The rule that applies here, if one does (#99), in a box. Where it
    /// ends.
    fn hint(&mut self, canvas: &mut Canvas, view: &View, left: f32, width: f32, y: f32) -> f32 {
        let journal = view.journal;
        let facts = super::hints::Facts {
            here: journal.and_then(|j| j.here),
            trade: view.trade,
            doorways: &view.doorways,
            now: CHARACTERS.map(|who| journal.and_then(|j| j.now(who))),
            hermit_met: view.hermit_met,
            objective: view.objective,
        };
        let Some(text) = super::hints::hint(&facts) else {
            self.spaced(canvas, left, y, "HINT", DIM);
            self.fonts.text(
                Some(canvas),
                left,
                y + 5.0,
                Some(width),
                1.3,
                &[span("None here.", 4.0, Weight::Regular, FAINT)],
            );
            return y + 14.0;
        };
        let s = [span(&text, 4.0, Weight::Regular, TEXT)];
        let h = self.fonts.height_at(canvas.scale, width - 7.0, 1.35, &s);
        canvas.round_rect(left, y, width, h + 6.0, 1.3, [0x1A, 0x1F, 0x2B]);
        canvas.round_rect(left, y, 1.0, h + 6.0, 0.0, BLUE);
        self.fonts.text(
            Some(canvas),
            left + 4.0,
            y + 3.0,
            Some(width - 7.0),
            1.35,
            &s,
        );
        y + h + 11.0
    }

    /// The objective (#98): his gold, how much more the next trade needs and
    /// what it gives, and what he has. Where it ends.
    fn objective(
        &mut self,
        canvas: &mut Canvas,
        view: &View,
        left: f32,
        width: f32,
        y: f32,
    ) -> f32 {
        self.spaced(canvas, left, y, "OBJECTIVE", DIM);
        let Some(o) = view.objective else {
            self.fonts.text(
                Some(canvas),
                left,
                y + 5.0,
                None,
                1.0,
                &[span("When a game starts.", 4.0, Weight::Regular, FAINT)],
            );
            return y + 14.0;
        };
        let next = match o.next() {
            Next::Sword => "the sword".to_string(),
            Next::Bow => "the bow".to_string(),
            Next::Piece(n) => format!("magic arrow {n}"),
            Next::Done => String::new(),
        };
        let bags = |n: u8| {
            if n == 1 {
                "1 bag".to_string()
            } else {
                format!("{n} bags")
            }
        };
        let line = match o.next() {
            Next::Done => "Every trade made: on to the tournament.".to_string(),
            _ if o.gold >= objective::PER_TRADE => {
                format!("{}: enough for {next}, at the Ent", bags(o.gold))
            }
            _ => format!(
                "{} · {} more for {next}",
                bags(o.gold),
                objective::PER_TRADE - o.gold
            ),
        };
        let (_, h) = self.fonts.text(
            Some(canvas),
            left,
            y + 5.0,
            Some(width),
            1.2,
            &[span(&line, 4.2, Weight::SemiBold, WHITE)],
        );
        let mut row = y + 6.0 + h;
        // All the gold the trades want, and how much of it he's brought.
        let want = f32::from(objective::PER_TRADE * objective::TRADES);
        let had = f32::from(o.traded() * objective::PER_TRADE + o.gold).min(want);
        canvas.round_rect(left, row, width, 2.0, 1.0, [0x1F, 0x23, 0x2B]);
        canvas.round_rect(left, row, width * had / want, 2.0, 1.0, [0xE3, 0xC0, 0x6B]);
        row += 3.2;
        self.fonts.text(
            Some(canvas),
            left,
            row,
            None,
            1.0,
            &[span(
                &format!(
                    "{} of the {} bags the Ent wants in all",
                    had as u8, want as u8
                ),
                3.5,
                Weight::Regular,
                FAINT,
            )],
        );
        row += 6.0;
        let got = [
            ("Sword", o.sword),
            ("Bow", o.bow),
            ("Arrow 1", o.pieces >= 1),
            ("Arrow 2", o.pieces >= 2),
            ("Arrow 3", o.pieces >= 3),
        ];
        let mut x = left;
        for (name, has) in got {
            let text = if has {
                format!("✓ {name}")
            } else {
                name.to_string()
            };
            let colour = if has { [0x7F, 0xE0, 0xA6] } else { DIM };
            let s = [span(&text, 3.6, Weight::Regular, colour)];
            let w = self.fonts.measure(&s) + 3.4;
            if x + w > left + width {
                x = left;
                row += 6.6;
            }
            let back = if has {
                [0x17, 0x35, 0x28]
            } else {
                [0x1C, 0x20, 0x28]
            };
            canvas.round_rect(x, row, w, 5.6, 1.3, back);
            self.fonts
                .text(Some(canvas), x + 1.7, row + 0.8, None, 1.0, &s);
            x += w + 1.6;
        }
        row + 9.0
    }

    /// The picker: a window in the middle over the dimmed game, which stands
    /// still while it's open (#92, Decision 7).
    fn picker(&mut self, canvas: &mut Canvas, aids: &Aids) {
        canvas.shade(0.0, 0.0, WINDOW_W, WINDOW_H, [0x05, 0x06, 0x09], 160);
        let w = 188.0;
        // Laid out once to measure it, then drawn the height it takes.
        let h = self.switches(None, aids, 0.0, 0.0, w) + 18.0;
        let (x, y) = ((WINDOW_W - w) / 2.0, (WINDOW_H - h) / 2.0);
        canvas.round_rect(x, y, w, h, 4.0, CARD_EDGE);
        canvas.round_rect(x + 0.4, y + 0.4, w - 0.8, h - 0.8, 3.7, CARD);
        let inner = x + 9.0;
        let right = x + w - 9.0;
        self.fonts.text(
            Some(canvas),
            inner,
            y + 7.0,
            None,
            1.0,
            &[span("Aids", 6.7, Weight::SemiBold, WHITE)],
        );
        let paused = [span("The game is paused", 4.2, Weight::Regular, DIM)];
        let pw = self.fonts.measure(&paused);
        self.fonts
            .text(Some(canvas), right - pw, y + 9.0, None, 1.0, &paused);
        self.switches(Some(canvas), aids, x, y, w);
        canvas.round_rect(x + 0.4, y + h - 14.0, w - 0.8, 0.4, 0.0, [0x23, 0x26, 0x2E]);
        let foot = y + h - 10.0;
        let mut fx = inner;
        for (keys, what) in [(&["↑", "↓"][..], "choose"), (&["Enter"][..], "switch")] {
            for k in keys {
                fx = self.key_cap_at(canvas, fx, foot, k) + 1.5;
            }
            let s = [span(what, 4.0, Weight::Regular, DIM)];
            self.fonts
                .text(Some(canvas), fx + 0.5, foot + 0.8, None, 1.0, &s);
            fx += self.fonts.measure(&s) + 6.0;
        }
        let done = [span("done", 4.0, Weight::Regular, DIM)];
        let dw = self.fonts.measure(&done);
        self.fonts
            .text(Some(canvas), right - dw, foot + 0.8, None, 1.0, &done);
        self.key_cap(canvas, right - dw - 2.0, foot, "F1");
    }

    /// The picker's switches, in the window at (`x`, `y`), `w` wide, drawn
    /// if `canvas` is given. Where they end, from `y`.
    fn switches(
        &mut self,
        mut canvas: Option<&mut Canvas>,
        aids: &Aids,
        x: f32,
        y: f32,
        w: f32,
    ) -> f32 {
        let inner = x + 9.0;
        let right = x + w - 9.0;
        let mut row = y + 19.0;
        for (n, aid) in ALL.into_iter().enumerate() {
            if n == 0 {
                if let Some(c) = canvas.as_deref_mut() {
                    self.spaced(c, inner, row, "GUIDANCE", DIM);
                }
                row += 6.0;
            }
            if aid == Aid::Energy {
                if let Some(c) = canvas.as_deref_mut() {
                    self.spaced(c, inner, row + 1.0, "ASSISTS", DIM);
                }
                row += 7.0;
            }
            let indent = aid.depth() as f32 * 9.0;
            let available = aids.available(aid);
            let (label, note) = aid.label();
            let (colour, small) = if available {
                (TEXT, [0x7D, 0x83, 0x94])
            } else {
                (FAINT, [0x45, 0x4A, 0x57])
            };
            let tall = if note.is_empty() { 8.0 } else { 11.0 };
            if let Some(c) = canvas.as_deref_mut() {
                if aid.depth() > 0 {
                    c.round_rect(inner + indent - 4.0, row, 0.7, tall - 1.0, 0.0, NEST);
                }
                self.fonts.text(
                    Some(&mut *c),
                    inner + indent,
                    row + 1.0,
                    None,
                    1.0,
                    &[span(label, 4.7, Weight::Regular, colour)],
                );
                if !note.is_empty() {
                    self.fonts.text(
                        Some(&mut *c),
                        inner + indent,
                        row + 6.2,
                        None,
                        1.0,
                        &[span(note, 3.8, Weight::Regular, small)],
                    );
                }
                self.toggle(
                    c,
                    right - 11.0,
                    row + (tall - 6.7) / 2.0,
                    aids.is_on(aid),
                    available,
                );
                if aids.focus() == Some(aid) {
                    c.outline(
                        inner + indent - 2.0,
                        row - 1.0,
                        right - inner - indent + 4.0,
                        tall + 1.0,
                        1.7,
                        0.7,
                        None,
                        BLUE,
                    );
                }
            }
            row += tall + 1.5;
            if aid == Aid::Now {
                let (_, h) = self.fonts.text(
                    canvas.as_deref_mut(),
                    inner + 9.0,
                    row,
                    Some(right - inner - 9.0),
                    1.25,
                    &[span(
                        "These need the map, and \u{201c}now\u{201d} needs \u{201c}who you've seen\u{201d}.",
                        3.7,
                        Weight::Regular,
                        BLUE,
                    )],
                );
                row += h + 2.0;
            }
        }
        row - y
    }

    /// A switch, on or off, greyed when it can't be switched.
    fn toggle(&mut self, canvas: &mut Canvas, x: f32, y: f32, on: bool, available: bool) {
        let (track, knob, at) = if on {
            (TRACK_ON, WHITE, x + 5.2)
        } else {
            (TRACK_OFF, KNOB_OFF, x + 1.0)
        };
        let (track, knob) = if available {
            (track, knob)
        } else {
            ([0x1C, 0x1F, 0x26], [0x3A, 0x3E, 0x49])
        };
        canvas.round_rect(x, y, 11.0, 6.7, 3.35, track);
        canvas.round_rect(at, y + 1.0, 4.7, 4.7, 2.35, knob);
    }

    /// Letters spaced out, for a section's small capitals.
    fn spaced(&mut self, canvas: &mut Canvas, x: f32, y: f32, text: &str, colour: Rgb) {
        let mut x = x;
        for c in text.chars() {
            let s = c.to_string();
            self.fonts.text(
                Some(canvas),
                x,
                y,
                None,
                1.0,
                &[span(&s, 3.6, Weight::SemiBold, colour)],
            );
            x += self.fonts.advance(c, 3.6, Weight::SemiBold) + 0.5;
        }
    }

    /// A key's name in a box, ending at `right`.
    fn key_cap(&mut self, canvas: &mut Canvas, right: f32, y: f32, key: &str) {
        let s = [span(key, 3.7, Weight::Regular, DIM)];
        let w = self.fonts.measure(&s) + 3.4;
        self.key_cap_at(canvas, right - w, y, key);
    }

    /// A key's name in a box from `x`. Where it ends.
    fn key_cap_at(&mut self, canvas: &mut Canvas, x: f32, y: f32, key: &str) -> f32 {
        let s = [span(key, 3.7, Weight::Regular, [0xC8, 0xCC, 0xD6])];
        let w = self.fonts.measure(&s) + 3.4;
        canvas.outline(x, y, w, 5.6, 1.2, 0.35, None, [0x3A, 0x3F, 0x4B]);
        self.fonts
            .text(Some(canvas), x + 1.7, y + 0.8, None, 1.0, &s);
        x + w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draws the panel for `aids` at three device pixels a logical one.
    fn drawn(aids: &Aids) -> Vec<u8> {
        let (w, h) = ((WINDOW_W * 3.0) as usize, (WINDOW_H * 3.0) as usize);
        let mut pixels = vec![0; w * h * 4];
        let mut canvas = Canvas {
            pixels: &mut pixels,
            width: w,
            height: h,
            scale: 3.0,
        };
        Panel::new().draw(&mut canvas, aids, &View::default());
        pixels
    }

    #[test]
    fn the_picture_is_left_clear_and_the_panel_drawn() {
        let p = drawn(&Aids::default());
        let w = (WINDOW_W * 3.0) as usize;
        let at = |x: usize, y: usize| p[(y * w + x) * 4 + 3];
        assert_eq!(at(100, 100), 0, "the picture shows through");
        assert_eq!(at(w - 5, 100), 0xFF, "the panel is opaque");
    }

    #[test]
    fn directions_go_the_short_way_round() {
        assert_eq!(direction(0x105, 0x105), "here");
        assert_eq!(direction(0x105, 0x108), "3 east");
        assert_eq!(direction(0x100, 0x10F), "1 west", "round the edge");
        assert_eq!(direction(0x005, 0x135), "1 north", "round the top");
        assert_eq!(direction(0x105, 0x0E7), "2 east, 2 north");
    }

    #[test]
    fn the_picker_dims_the_picture() {
        let mut aids = Aids::default();
        aids.open_or_close();
        let p = drawn(&aids);
        let w = (WINDOW_W * 3.0) as usize;
        assert!(p[(100 * w + 20) * 4 + 3] > 0, "the game is shaded over");
    }
}

/// Draws the panel's states to PNGs in `$PANEL_PNG`, for looking at them
/// against the mockups: `cargo test -p robin --bin robin -- --ignored`.
#[cfg(test)]
mod pictures {
    use super::*;

    fn png(aids: &Aids, name: &str) {
        png_with(aids, &View::default(), name);
    }

    fn png_with(aids: &Aids, view: &View, name: &str) {
        let Ok(dir) = std::env::var("PANEL_PNG") else {
            return;
        };
        let (w, h) = ((WINDOW_W * 3.0) as usize, (WINDOW_H * 3.0) as usize);
        let mut pixels = vec![0; w * h * 4];
        let mut canvas = Canvas {
            pixels: &mut pixels,
            width: w,
            height: h,
            scale: 3.0,
        };
        Panel::new().draw(&mut canvas, aids, view);
        // Over a dark grey picture, so what's see-through shows.
        let out: Vec<u32> = pixels
            .chunks(4)
            .map(|p| {
                let under = 0x30u32;
                let a = u32::from(p[3]);
                let c = |v: u8| u32::from(v) + under * (255 - a) / 255;
                c(p[0]) << 16 | c(p[1]) << 8 | c(p[2])
            })
            .collect();
        std::fs::write(
            format!("{dir}/{name}.png"),
            zx_core::png::encode(&out, w, h),
        )
        .expect("written");
    }

    #[test]
    #[ignore = "draws pictures to look at"]
    fn render_to_png() {
        png(&Aids::default(), "off");
        let mut args: Vec<String> = [
            "--map",
            "--objective",
            "--hints",
            "--infinite-energy",
            "--saves",
        ]
        .map(String::from)
        .to_vec();
        let mut aids = Aids::from_args(&mut args);
        png(&aids, "on");
        // A walk round rows 15 to 18, past the trade and two doorways.
        let walk: Vec<u16> = (15..19)
            .flat_map(|r| (2..12).map(move |c| r * 16 + c))
            .chain([0x9C, 0x8C, 0x7C])
            .collect();
        let journal = Journal::of(walk, 0x105).with(
            [Some(0x10B), None, Some(0x0F2)],
            [Some(0x0EC), Some(0x043), Some(0x0F4)],
        );
        let mut seen_args: Vec<String> = ["--map-now", "--objective"].map(String::from).to_vec();
        let seen = Aids::from_args(&mut seen_args);
        let view = View {
            journal: Some(&journal),
            objective: Some(Objective {
                gold: 2,
                sword: true,
                bow: false,
                pieces: 0,
                robbed: 2,
                full: false,
                arrows: 0,
            }),
            hermit_met: false,
            trade: 0x109,
            doorways: [0x0F4, 0x10B, 0, 0, 0, 0, 0, 0, 0],
        };
        png_with(&aids, &view, "map");
        png_with(&seen, &view, "seen");
        let mut full = seen.clone();
        full.open_or_close_map();
        png_with(&full, &view, "seen-full");
        let at_ent = Journal::of([0x108], 0x109);
        let mut hint_args: Vec<String> = ["--map", "--objective", "--hints"]
            .map(String::from)
            .to_vec();
        let hinting = Aids::from_args(&mut hint_args);
        let hint_view = View {
            journal: Some(&at_ent),
            ..view
        };
        png_with(&hinting, &hint_view, "hint");
        aids.open_or_close_map();
        png_with(&aids, &view, "full-map");
        aids.open_or_close_map();
        aids.open_or_close();
        aids.move_focus(true);
        png(&aids, "picker");
    }
}
