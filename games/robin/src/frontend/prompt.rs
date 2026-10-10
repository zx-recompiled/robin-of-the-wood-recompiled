//! The screen that asks for the tape, when none is found at startup.
//!
//! Drawn in the game's window, as starquake-recompiled's is, to its approved
//! mockups (starquake-recompiled#61; here #70): locate the tape with the
//! system's own file dialog, drop it on the window, or visit World of
//! Spectrum's page, which opens only when the player asks. A tape that
//! passes the check is kept in the data folder so the question is asked
//! once. Adapted from starquake-recompiled's (`REUSED.md`).

use std::path::{Path, PathBuf};

use winit::keyboard::KeyCode;

use super::tape::{self, Refused, Tape};
use super::text::{Canvas, Fonts, Rgb, Span, Weight};

/// The screen's size in logical pixels: the game window's.
pub const WIDTH: f32 = 960.0;
pub const HEIGHT: f32 = 768.0;

/// Where the game can be found, for a player who has not got it.
const PAGE: &str = "https://worldofspectrum.net/item/0004177/";
const PAGE_SHOWN: &str = "worldofspectrum.net/item/0004177";

pub const BACKGROUND: Rgb = [0x0b, 0x0c, 0x10];
const LABEL: Rgb = [0x6d, 0x73, 0x85];
const TITLE: Rgb = [0xf2, 0xf3, 0xf7];
const BODY: Rgb = [0xa9, 0xaf, 0xbe];
const BRIGHT: Rgb = [0xe6, 0xe8, 0xee];
const MUTED: Rgb = [0x8b, 0x90, 0xa0];
const ZONE: Rgb = [0x10, 0x13, 0x1a];
const ZONE_LINE: Rgb = [0x34, 0x39, 0x48];
const ACCENT: Rgb = [0x8f, 0xb4, 0xff];
const ACCENT_HOVER: Rgb = [0xa9, 0xc5, 0xff];
const BUTTON: Rgb = [0x16, 0x19, 0x20];
const BUTTON_HOVER: Rgb = [0x20, 0x24, 0x2e];
const BUTTON_LINE: Rgb = [0x3a, 0x3f, 0x4c];
const KEY_TEXT: Rgb = [0xc9, 0xcd, 0xd8];
const ERROR: Rgb = [0x2a, 0x16, 0x18];
const ERROR_LINE: Rgb = [0x5a, 0x2a, 0x2f];
const ERROR_DOT: Rgb = [0xe0, 0x67, 0x6f];
const ERROR_TITLE: Rgb = [0xf3, 0xc6, 0xca];
const ERROR_TEXT: Rgb = [0xc7, 0xa3, 0xa7];

/// What the window should do after the prompt has handled an event.
pub enum Outcome {
    Nothing,
    Redraw,
    Quit,
    /// A tape passed the check: start the game with it.
    Start(Tape),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Button {
    Locate,
    Website,
}

/// A line shown above the drop zone: a title and what to do about it.
struct Message {
    title: String,
    detail: String,
}

pub struct Prompt {
    fonts: Fonts,
    message: Option<Message>,
    /// A tape that passed the check but could not be kept: the main button
    /// starts the game with it instead.
    unkept: Option<Tape>,
    hover: Option<Button>,
}

impl Prompt {
    pub fn new() -> Prompt {
        Prompt {
            fonts: Fonts::load(),
            message: None,
            unkept: None,
            hover: None,
        }
    }

    pub fn draw(&mut self, canvas: &mut Canvas) {
        canvas.clear(BACKGROUND);
        let left = 120.0;

        self.spaced(canvas, left, 96.0, "ROBIN OF THE WOOD");
        self.fonts.text(
            Some(canvas),
            left,
            118.0,
            None,
            1.0,
            &[span(
                "This needs your copy of the game",
                30.0,
                Weight::SemiBold,
                TITLE,
            )],
        );
        self.fonts.text(
            Some(canvas),
            left,
            168.0,
            Some(680.0),
            1.6,
            &[span(
                "Nothing from the original game is included. The graphics, maps and sound are read \
                 from your own Robin of the Wood tape each time the game starts.",
                15.0,
                Weight::Regular,
                BODY,
            )],
        );

        let top = self.zone_top();
        if let Some(m) = &self.message {
            canvas.round_rect(left, 236.0, 720.0, 90.0, 10.0, ERROR_LINE);
            canvas.round_rect(left + 1.0, 237.0, 718.0, 88.0, 9.0, ERROR);
            canvas.round_rect(left + 18.0, 253.0, 18.0, 18.0, 9.0, ERROR_DOT);
            self.fonts.text(
                Some(canvas),
                left + 24.5,
                253.0,
                None,
                1.0,
                &[span("!", 13.0, Weight::SemiBold, ERROR)],
            );
            self.fonts.text(
                Some(canvas),
                left + 46.0,
                250.0,
                Some(650.0),
                1.0,
                &[span(&m.title, 15.0, Weight::SemiBold, ERROR_TITLE)],
            );
            self.fonts.text(
                Some(canvas),
                left + 46.0,
                274.0,
                Some(650.0),
                1.5,
                &[span(&m.detail, 13.0, Weight::Regular, ERROR_TEXT)],
            );
        }

        canvas.round_rect(left, top, 720.0, 164.0, 14.0, ZONE);
        canvas.outline(left, top, 720.0, 164.0, 14.0, 2.0, Some(6.0), ZONE_LINE);
        let (x, y, w, h) = self.button(Button::Locate);
        let fill = if self.hover == Some(Button::Locate) {
            ACCENT_HOVER
        } else {
            ACCENT
        };
        canvas.round_rect(x, y, w, h, 9.0, fill);
        let label = self.locate_label();
        self.centred(
            canvas,
            y + 12.0,
            &[span(label, 16.0, Weight::SemiBold, BACKGROUND)],
        );
        self.centred(
            canvas,
            top + 98.0,
            &[span(
                "or drop it onto this window",
                14.0,
                Weight::Regular,
                MUTED,
            )],
        );
        self.centred(
            canvas,
            top + 126.0,
            &[
                span("The 128K tape, as a ", 12.0, Weight::Regular, LABEL),
                span(".tzx", 12.0, Weight::Regular, BODY),
                span(", or the ", 12.0, Weight::Regular, LABEL),
                span(".zip", 12.0, Weight::Regular, BODY),
                span(" it was downloaded in", 12.0, Weight::Regular, LABEL),
            ],
        );

        let section = top + 212.0;
        self.spaced(canvas, left, section, "DON'T HAVE IT?");
        self.fonts.text(
            Some(canvas),
            left,
            section + 24.0,
            Some(480.0),
            1.55,
            &[
                span(
                    "World of Spectrum keeps Spectrum software available, and removes titles whose rights \
                     holders object. Download ",
                    14.0,
                    Weight::Regular,
                    BODY,
                ),
                span("the 128K tape", 14.0, Weight::Regular, BRIGHT),
                span(" there, then locate it here.", 14.0, Weight::Regular, BODY),
            ],
        );
        let (x, y, w, h) = self.button(Button::Website);
        let fill = if self.hover == Some(Button::Website) {
            BUTTON_HOVER
        } else {
            BUTTON
        };
        canvas.round_rect(x, y, w, h, 8.0, BUTTON_LINE);
        canvas.round_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, 7.0, fill);
        self.fonts.text(
            Some(canvas),
            x + 16.0,
            y + 10.0,
            None,
            1.0,
            &[website_label()],
        );
        self.fonts.text(
            Some(canvas),
            left,
            section + 96.0,
            None,
            1.0,
            &[span(PAGE_SHOWN, 12.0, Weight::Regular, LABEL)],
        );

        self.fonts.text(
            Some(canvas),
            left,
            710.0,
            None,
            1.0,
            &[span(
                "Robin of the Wood \u{a9} 1985 Odin Computer Graphics. Not affiliated.",
                12.0,
                Weight::Regular,
                LABEL,
            )],
        );
        let mut right = 840.0;
        for (key, what) in [
            ("Esc", "quit"),
            ("W", "website"),
            ("Enter", self.enter_does()),
        ] {
            let what_w = self
                .fonts
                .measure(&[span(what, 12.0, Weight::Regular, LABEL)]);
            let key_w = self
                .fonts
                .measure(&[span(key, 12.0, Weight::SemiBold, KEY_TEXT)])
                .max(8.0)
                + 12.0;
            let x = right - what_w;
            self.fonts.text(
                Some(canvas),
                x,
                710.0,
                None,
                1.0,
                &[span(what, 12.0, Weight::Regular, LABEL)],
            );
            let kx = x - 5.0 - key_w;
            canvas.round_rect(kx, 707.0, key_w, 20.0, 5.0, BUTTON_LINE);
            canvas.round_rect(kx + 1.0, 708.0, key_w - 2.0, 17.0, 4.0, BUTTON);
            self.fonts.text(
                Some(canvas),
                kx + 6.0,
                710.0,
                None,
                1.0,
                &[span(key, 12.0, Weight::SemiBold, KEY_TEXT)],
            );
            right = kx - 16.0;
        }
    }

    pub fn cursor(&mut self, x: f32, y: f32) -> Outcome {
        let over = [Button::Locate, Button::Website].into_iter().find(|&b| {
            let (bx, by, w, h) = self.button(b);
            x >= bx && x < bx + w && y >= by && y < by + h
        });
        if over == self.hover {
            return Outcome::Nothing;
        }
        self.hover = over;
        Outcome::Redraw
    }

    pub fn clicked(&mut self) -> Outcome {
        match self.hover {
            Some(Button::Locate) => self.locate(),
            Some(Button::Website) => self.website(),
            None => Outcome::Nothing,
        }
    }

    pub fn key(&mut self, code: KeyCode) -> Outcome {
        match code {
            KeyCode::Enter | KeyCode::NumpadEnter => self.locate(),
            KeyCode::KeyW => self.website(),
            KeyCode::Escape => Outcome::Quit,
            _ => Outcome::Nothing,
        }
    }

    pub fn dropped(&mut self, path: &Path) -> Outcome {
        self.take(path)
    }

    fn locate(&mut self) -> Outcome {
        if let Some(tape) = self.unkept.take() {
            return Outcome::Start(tape);
        }
        let mut dialog = rfd::FileDialog::new()
            .set_title("Locate the Robin of the Wood tape")
            // Some Linux dialogs match extensions case by case, hence both.
            .add_filter(
                "Robin of the Wood tape (.tzx, .zip)",
                &["tzx", "TZX", "zip", "ZIP"],
            );
        #[cfg(not(target_os = "macos"))]
        {
            dialog = dialog.add_filter("All files", &["*"]);
        }
        if let Some(dir) = downloads() {
            dialog = dialog.set_directory(dir);
        }
        match dialog.pick_file() {
            Some(path) => self.take(&path),
            None => Outcome::Redraw,
        }
    }

    fn take(&mut self, path: &Path) -> Outcome {
        let file = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        match tape::load(path, robin::is_the_tape) {
            Ok(found) => match tape::keep(&found.bytes) {
                Ok(_) => Outcome::Start(found),
                Err(why) => {
                    self.message = Some(Message {
                        title: "The tape could not be kept for next time".into(),
                        detail: format!(
                            "{why}. The game will start from the file you picked, and ask for it again \
                             next time."
                        ),
                    });
                    self.unkept = Some(found);
                    Outcome::Redraw
                }
            },
            Err(why) => {
                self.unkept = None;
                self.message = Some(refusal(&file, &why));
                Outcome::Redraw
            }
        }
    }

    fn website(&mut self) -> Outcome {
        if let Err(e) = open_page() {
            self.message = Some(Message {
                title: "No browser could be opened".into(),
                detail: format!("{e}. The page is at {PAGE}"),
            });
        }
        Outcome::Redraw
    }

    fn zone_top(&self) -> f32 {
        if self.message.is_some() { 336.0 } else { 250.0 }
    }

    fn locate_label(&self) -> &'static str {
        if self.unkept.is_some() {
            "Start the game"
        } else {
            "Locate the tape\u{2026}"
        }
    }

    fn enter_does(&self) -> &'static str {
        if self.unkept.is_some() {
            "start"
        } else {
            "locate"
        }
    }

    /// Where a button is, in logical pixels: x, y, width, height.
    fn button(&mut self, which: Button) -> (f32, f32, f32, f32) {
        let top = self.zone_top();
        match which {
            Button::Locate => {
                let w = self.fonts.measure(&[span(
                    self.locate_label(),
                    16.0,
                    Weight::SemiBold,
                    BACKGROUND,
                )]) + 52.0;
                ((WIDTH - w) / 2.0, top + 36.0, w, 44.0)
            }
            Button::Website => {
                let w = self.fonts.measure(&[website_label()]) + 32.0;
                (840.0 - w, top + 250.0, w, 38.0)
            }
        }
    }

    fn centred(&mut self, canvas: &mut Canvas, y: f32, spans: &[Span]) {
        let w = self.fonts.measure(spans);
        self.fonts
            .text(Some(canvas), (WIDTH - w) / 2.0, y, None, 1.0, spans);
    }

    /// A small label with its letters spread out, which the layout cannot do
    /// itself.
    fn spaced(&mut self, canvas: &mut Canvas, mut x: f32, y: f32, text: &str) {
        let mut buf = [0u8; 4];
        for c in text.chars() {
            let s = span(c.encode_utf8(&mut buf), 12.0, Weight::SemiBold, LABEL);
            self.fonts
                .text(Some(canvas), x, y, None, 1.0, std::slice::from_ref(&s));
            x += self.fonts.advance(c, 12.0, Weight::SemiBold) + 1.8;
        }
    }
}

fn span(text: &str, size: f32, weight: Weight, colour: Rgb) -> Span<'_> {
    Span {
        text,
        size,
        weight,
        colour,
    }
}

fn website_label() -> Span<'static> {
    span(
        "Open World of Spectrum \u{2197}",
        14.0,
        Weight::SemiBold,
        BRIGHT,
    )
}

fn refusal(file: &str, why: &Refused) -> Message {
    match why {
        Refused::NotTheTape => Message {
            title: "That isn't the Robin of the Wood tape this version needs".into(),
            detail: format!(
                "The tape in {file} is a different dump. This version works only with the 128K \
                 release (SHA-1 {}\u{2026}).",
                &robin::assets::TAPE_SHA1[..8]
            ),
        },
        Refused::NoTapeInZip => Message {
            title: "There is no tape in that zip".into(),
            detail: format!("{file} has no .tzx file in it."),
        },
        Refused::Unreadable(e) => Message {
            title: "That file could not be read".into(),
            detail: e.clone(),
        },
    }
}

/// The folder a browser saves downloads to, if there is one.
fn downloads() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.map(|h| PathBuf::from(h).join("Downloads"))
        .filter(|d| d.is_dir())
}

/// Opens the archive's page in the player's browser, with the system's own
/// opener rather than a crate.
fn open_page() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(windows)]
    let opener = "explorer";
    #[cfg(all(unix, not(target_os = "macos")))]
    let opener = "xdg-open";
    std::process::Command::new(opener)
        .arg(PAGE)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{opener} could not be run: {e}"))
}

#[cfg(test)]
mod render_check {
    use super::*;

    /// Draws the prompt, as it first appears and after a wrong file, to PNGs
    /// in the folder `ROBIN_PROMPT_PNG` names, for the mockup, without
    /// opening a window. Does nothing when it is not set.
    #[test]
    fn render_to_png() {
        let Some(out) = std::env::var_os("ROBIN_PROMPT_PNG") else {
            return;
        };
        let out = PathBuf::from(out);
        for (name, message) in [("idle", false), ("error", true)] {
            let mut p = Prompt::new();
            if message {
                p.message = Some(refusal("robin-48k.zip", &Refused::NotTheTape));
            }
            let scale = 2.0;
            let (w, h) = ((WIDTH * scale) as usize, (HEIGHT * scale) as usize);
            let mut pixels = vec![0u8; w * h * 4];
            let mut canvas = Canvas {
                pixels: &mut pixels,
                width: w,
                height: h,
                scale,
            };
            p.draw(&mut canvas);
            let rgb: Vec<u32> = pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]))
                .collect();
            std::fs::write(
                out.join(format!("prompt-{name}.png")),
                zx_core::png::encode(&rgb, w, h),
            )
            .unwrap();
        }
    }
}
