//! The aids panel beside the game, and the picker over it (#92, #95), drawn
//! to the approved mockups, as starquake-recompiled draws its guidance
//! panel (`REUSED.md`).
//!
//! Everything is laid out in logical pixels of the whole window, the
//! Spectrum's own: the picture takes the left [`PICTURE_W`], the panel the
//! rest. The overlay's canvas scales them to the window.

use super::aids::{ALL, Aid, Aids};
use super::text::{Canvas, Fonts, Rgb, Span, Weight};

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

pub struct Panel {
    fonts: Fonts,
}

impl Panel {
    pub fn new() -> Panel {
        Panel {
            fonts: Fonts::load(),
        }
    }

    /// Draws the panel for `aids`, and the picker over everything if it's
    /// open.
    pub fn draw(&mut self, canvas: &mut Canvas, aids: &Aids) {
        canvas.clear_transparent();
        canvas.round_rect(PICTURE_W, 0.0, PANEL_W, WINDOW_H, 0.0, BACKGROUND);
        canvas.round_rect(PICTURE_W, 0.0, 0.4, WINDOW_H, 0.0, EDGE);
        let left = PICTURE_W + PAD;
        let width = PANEL_W - 2.0 * PAD;
        self.spaced(canvas, left, 7.0, "AIDS", DIM);
        self.key_cap(canvas, PICTURE_W + PANEL_W - PAD, 6.5, "F1");
        if aids.any() {
            self.sections(canvas, aids, left, width);
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
        }
    }

    /// The sections for the aids that are on, top to bottom. Each aid's own
    /// contents come with its sub-issue of #92.
    fn sections(&mut self, canvas: &mut Canvas, aids: &Aids, left: f32, width: f32) {
        let mut y = 18.0;
        let coming = [
            (Aid::Map, "MAP", "The map comes with #96."),
            (Aid::Objective, "OBJECTIVE", "The objective comes with #98."),
            (Aid::Hints, "HINTS", "Rule hints come with #99."),
        ];
        for (aid, title, note) in coming {
            if !aids.is_on(aid) {
                continue;
            }
            self.spaced(canvas, left, y, title, DIM);
            let (_, h) = self.fonts.text(
                Some(canvas),
                left,
                y + 5.0,
                Some(width),
                1.3,
                &[span(note, 4.2, Weight::Regular, FAINT)],
            );
            y += 9.0 + h;
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
        Panel::new().draw(&mut canvas, aids);
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
        Panel::new().draw(&mut canvas, aids);
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
        aids.open_or_close();
        aids.move_focus(true);
        png(&aids, "picker");
    }
}
