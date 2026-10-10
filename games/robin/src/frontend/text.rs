//! Drawing for the screens the host shows itself, outside the game: text in
//! Inter, rasterised by `fontdue`, and plain shapes, into an RGBA frame.
//!
//! The game's own font is on the tape, so it cannot be used for the screen
//! that asks for the tape; and #1 chose a modern, legible face for
//! everything the host draws. Inter is under the SIL Open Font License and is
//! shipped unmodified, with its licence beside it in `fonts/`.

use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle, WrapStyle};
use fontdue::{Font, FontSettings};

/// An opaque colour.
pub type Rgb = [u8; 3];

#[derive(Clone, Copy)]
pub enum Weight {
    Regular = 0,
    SemiBold = 1,
}

/// One run of text: its words, size in logical pixels, weight and colour.
pub struct Span<'a> {
    pub text: &'a str,
    pub size: f32,
    pub weight: Weight,
    pub colour: Rgb,
}

/// An RGBA frame drawn at `scale` device pixels per logical pixel, so what
/// is laid out in logical pixels comes out sharp on a high-density screen.
pub struct Canvas<'a> {
    pub pixels: &'a mut [u8],
    pub width: usize,
    pub height: usize,
    pub scale: f32,
}

pub struct Fonts {
    fonts: [Font; 2],
    layout: Layout<Rgb>,
}

impl Fonts {
    /// # Panics
    ///
    /// If the bundled font files do not parse, which would be a broken build.
    pub fn load() -> Fonts {
        let load =
            |bytes: &[u8]| Font::from_bytes(bytes, FontSettings::default()).expect("bundled font");
        Fonts {
            fonts: [
                load(include_bytes!("../../fonts/Inter-Regular.ttf")),
                load(include_bytes!("../../fonts/Inter-SemiBold.ttf")),
            ],
            layout: Layout::new(CoordinateSystem::PositiveYDown),
        }
    }

    /// Lays out `spans` from (`x`, `y`), the top left in logical pixels,
    /// wrapping at `max_width` if given, `line_height` times the size apart,
    /// and draws them if `canvas` is.
    /// Returns the size the text took, in logical pixels.
    pub fn text(
        &mut self,
        canvas: Option<&mut Canvas>,
        x: f32,
        y: f32,
        max_width: Option<f32>,
        line_height: f32,
        spans: &[Span],
    ) -> (f32, f32) {
        let scale = canvas.as_ref().map_or(1.0, |c| c.scale);
        self.layout.reset(&LayoutSettings {
            x: x * scale,
            y: y * scale,
            max_width: max_width.map(|w| w * scale),
            wrap_style: WrapStyle::Word,
            // Given as a multiple of the size, as CSS has it; the layout
            // wants a multiple of the font's own line spacing, which for
            // Inter is 1.21 times the size.
            line_height: line_height / 1.21,
            ..LayoutSettings::default()
        });
        for span in spans {
            self.layout.append(
                &self.fonts,
                &TextStyle::with_user_data(
                    span.text,
                    span.size * scale,
                    span.weight as usize,
                    span.colour,
                ),
            );
        }
        let glyphs = self.layout.glyphs();
        let width = glyphs
            .iter()
            .map(|g| g.x + g.width as f32)
            .fold(x * scale, f32::max)
            - x * scale;
        let height = self.layout.height();
        if let Some(canvas) = canvas {
            for g in glyphs {
                if g.width == 0 || g.height == 0 {
                    continue;
                }
                let (metrics, coverage) = self.fonts[g.font_index].rasterize_config(g.key);
                for row in 0..metrics.height {
                    for col in 0..metrics.width {
                        let a = coverage[row * metrics.width + col];
                        if a > 0 {
                            canvas.blend(
                                g.x as isize + col as isize,
                                g.y as isize + row as isize,
                                g.user_data,
                                a,
                            );
                        }
                    }
                }
            }
        }
        (width / scale, height / scale)
    }

    /// How far one character moves the pen, in logical pixels, spaces
    /// included (a space draws nothing, so it cannot be measured by what it
    /// draws).
    pub fn advance(&self, c: char, size: f32, weight: Weight) -> f32 {
        self.fonts[weight as usize].metrics(c, size).advance_width
    }

    /// The width `spans` take on one line, in logical pixels.
    pub fn measure(&mut self, spans: &[Span]) -> f32 {
        self.text(None, 0.0, 0.0, None, 1.0, spans).0
    }
}

impl Canvas<'_> {
    fn blend(&mut self, x: isize, y: isize, colour: Rgb, alpha: u8) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        // Over, with the frame's colours premultiplied by its alpha, so a
        // transparent frame can be laid over something else afterwards.
        let i = (y as usize * self.width + x as usize) * 4;
        let a = u16::from(alpha);
        for (k, &c) in colour.iter().enumerate() {
            let under = u16::from(self.pixels[i + k]);
            self.pixels[i + k] = ((u16::from(c) * a + under * (255 - a)) / 255) as u8;
        }
        let under = u16::from(self.pixels[i + 3]);
        self.pixels[i + 3] = (a + under * (255 - a) / 255) as u8;
    }

    pub fn clear(&mut self, colour: Rgb) {
        self.pixels
            .as_chunks_mut::<4>()
            .0
            .fill([colour[0], colour[1], colour[2], 0xFF]);
    }

    /// A filled rectangle with rounded corners, in logical pixels.
    pub fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, colour: Rgb) {
        self.shape(x, y, w, h, radius, colour, 255, |_, _| true);
    }

    /// The outline of a rounded rectangle, `thickness` logical pixels wide,
    /// dashed when `dash` is given (the length of a dash and of a gap).
    #[allow(
        clippy::too_many_arguments,
        reason = "a rectangle, its corners, its line and its dash"
    )]
    pub fn outline(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        thickness: f32,
        dash: Option<f32>,
        colour: Rgb,
    ) {
        let s = self.scale;
        let t = thickness * s;
        let (x0, y0, x1, y1) = (x * s, y * s, (x + w) * s, (y + h) * s);
        let r = radius * s;
        self.shape(x, y, w, h, radius, colour, 255, move |px, py| {
            // Inside the shape; on the line if the same shape shrunk by the
            // thickness does not cover it.
            let inner = inside(px, py, x0 + t, y0 + t, x1 - t, y1 - t, (r - t).max(0.0));
            if inner {
                return false;
            }
            match dash {
                None => true,
                Some(d) => {
                    // Dash along whichever edge the point is nearest.
                    let along = if px - x0 < t || x1 - px < t { py } else { px };
                    (along / (d * s)).floor() as i64 % 2 == 0
                }
            }
        });
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "a rectangle, its corners and its colour"
    )]
    fn shape(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        colour: Rgb,
        alpha: u8,
        keep: impl Fn(f32, f32) -> bool,
    ) {
        let s = self.scale;
        let (x0, y0, x1, y1) = (x * s, y * s, (x + w) * s, (y + h) * s);
        let r = radius * s;
        let rows = (y0.floor().max(0.0) as usize)..(y1.ceil() as usize).min(self.height);
        for py in rows {
            for px in (x0.floor().max(0.0) as usize)..(x1.ceil() as usize).min(self.width) {
                // Four samples per pixel, for edges that are not jagged.
                let mut hits = 0u8;
                for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let (sx, sy) = (px as f32 + dx, py as f32 + dy);
                    if inside(sx, sy, x0, y0, x1, y1, r) && keep(sx, sy) {
                        hits += 1;
                    }
                }
                if hits > 0 {
                    self.blend(
                        px as isize,
                        py as isize,
                        colour,
                        ([0u16, 64, 128, 191, 255][hits as usize] * u16::from(alpha) / 255) as u8,
                    );
                }
            }
        }
    }
}

/// `v` brought into the range between `a` and `b`, whichever way round the
/// two are. A shape no wider than its corners are round (the round badge a
/// pad button is drawn as) leaves the two ends of its straight middle equal
/// in arithmetic and, at a scale that is not a whole number, a hair apart in
/// floating point. `f32::clamp` panics outright on a range the wrong way
/// round, which took the window down with it (as ZX Sidekick found,
/// zx-sidekick/zx-sidekick#38).
fn between(v: f32, a: f32, b: f32) -> f32 {
    v.clamp(a.min(b), a.max(b))
}

#[allow(clippy::too_many_arguments, reason = "a point and a rounded rectangle")]
fn inside(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    if px < x0 || px >= x1 || py < y0 || py >= y1 {
        return false;
    }
    let cx = between(px, x0 + r, x1 - r);
    let cy = between(py, y0 + r, y1 - r);
    (px - cx).powi(2) + (py - cy).powi(2) <= r * r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_drawn_and_measured() {
        let mut fonts = Fonts::load();
        let (w, h) = (200, 40);
        let mut pixels = vec![0u8; w * h * 4];
        let mut canvas = Canvas {
            pixels: &mut pixels,
            width: w,
            height: h,
            scale: 1.0,
        };
        canvas.clear([0, 0, 0]);
        let span = |text| Span {
            text,
            size: 20.0,
            weight: Weight::Regular,
            colour: [255, 255, 255],
        };
        fonts.text(Some(&mut canvas), 4.0, 4.0, None, 1.0, &[span("Robin")]);
        assert!(
            pixels.as_chunks::<4>().0.iter().any(|p| p[0] > 200),
            "nothing was drawn"
        );
        assert!(fonts.measure(&[span("WWW")]) > fonts.measure(&[span("iii")]));
    }

    #[test]
    fn a_rounded_rectangle_has_round_corners() {
        let (w, h) = (20, 20);
        let mut pixels = vec![0u8; w * h * 4];
        let mut canvas = Canvas {
            pixels: &mut pixels,
            width: w,
            height: h,
            scale: 1.0,
        };
        canvas.round_rect(0.0, 0.0, 20.0, 20.0, 8.0, [255, 255, 255]);
        let at = |x: usize, y: usize| pixels[(y * w + x) * 4];
        assert_eq!(at(0, 0), 0, "the corner is cut");
        assert_eq!(at(10, 10), 255, "the middle is filled");
    }

    /// A circle is exactly as wide as its corners are round. At a scale
    /// that is not a whole number, which fullscreen gives, the two edges of
    /// such a shape's middle land a hair apart, which used to reach
    /// `f32::clamp` the wrong way round and bring the window down. The
    /// failure hangs on the exact product of position and scale, so this
    /// sweeps both, on a canvas big enough to hold every step.
    #[test]
    fn a_circle_is_drawn_at_any_scale() {
        let (w, h) = (256, 256);
        let mut pixels = vec![0u8; w * h * 4];
        for step in 0..500 {
            let scale = 1.0 + step as f32 * 0.013;
            let at = 3.0 + step as f32 * 0.017;
            pixels.fill(0);
            let mut canvas = Canvas {
                pixels: &mut pixels,
                width: w,
                height: h,
                scale,
            };
            canvas.round_rect(at, at, 22.0, 22.0, 11.0, [0xFF, 0xFF, 0xFF]);
            canvas.outline(at, at, 22.0, 22.0, 11.0, 1.5, None, [0xFF, 0xFF, 0xFF]);
            let alpha =
                |x: f32, y: f32| pixels[((y * scale) as usize * w + (x * scale) as usize) * 4 + 3];
            assert_eq!(
                alpha(at + 11.0, at + 11.0),
                255,
                "the middle at scale {scale}"
            );
            assert_eq!(alpha(at + 1.0, at + 1.0), 0, "the corner at scale {scale}");
        }
    }
}
