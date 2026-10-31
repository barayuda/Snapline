//! Text for the text tool and the overlay's labels: measure and draw with one font into a tiny-skia pixmap.

use crate::color::Rgb;
use ab_glyph::{Font as _, FontArc, PxScale, ScaleFont as _};
use snapline_core::selection_geometry::{Point, Size};
use tiny_skia::{Pixmap, PremultipliedColorU8};

/// Outfit Bold from the brand fonts (OFL). The fallback, and the font every test uses.
static BUNDLED: &[u8] = include_bytes!("../../../../scripts/brand/fonts/Outfit-Bold.ttf");

#[derive(Clone)]
pub struct TextFont(FontArc);

impl TextFont {
    /// # Panics
    /// Never: the bundled font is checked by the tests.
    #[must_use]
    pub fn bundled() -> Self {
        Self(FontArc::try_from_slice(BUNDLED).expect("bundled font is valid"))
    }

    /// Segoe UI Semibold on Windows, so text looks native; the bundled font elsewhere or when it is missing.
    #[must_use]
    pub fn system() -> Self {
        #[cfg(windows)]
        {
            if let Some(font) = std::env::var_os("WINDIR")
                .and_then(|dir| {
                    std::fs::read(std::path::Path::new(&dir).join("Fonts").join("seguisb.ttf")).ok()
                })
                .and_then(|bytes| FontArc::try_from_vec(bytes).ok())
            {
                return Self(font);
            }
        }
        Self::bundled()
    }

    /// `size` is the em size in pixels, as font sizes are on macOS. `ab_glyph`'s own scale is the line height,
    /// so convert: the line is (ascent − descent) / units-per-em times the em size.
    fn scale(&self, size: f32) -> PxScale {
        match self.0.units_per_em() {
            Some(upem) => PxScale::from(size * self.0.height_unscaled() / upem),
            None => PxScale::from(size),
        }
    }

    /// Width of `text` and height of one line, in pixels.
    #[must_use]
    pub fn measure(&self, text: &str, size: f32) -> Size {
        let f = self.0.as_scaled(self.scale(size));
        let mut width = 0.0;
        let mut prev = None;
        for c in text.chars() {
            let id = f.glyph_id(c);
            if let Some(p) = prev {
                width += f.kern(p, id);
            }
            width += f.h_advance(id);
            prev = Some(id);
        }
        Size {
            width: f64::from(width),
            height: f64::from(f.ascent() - f.descent()),
        }
    }

    /// Draws `text` with the top-left corner of its line at `origin`, blending into `pixmap`.
    /// Anything outside the pixmap is skipped.
    pub fn draw(&self, pixmap: &mut Pixmap, text: &str, origin: Point, size: f32, color: Rgb) {
        let scale = self.scale(size);
        let font = self.0.as_scaled(scale);
        let baseline = origin.y as f32 + font.ascent();
        let (w, h) = (pixmap.width().cast_signed(), pixmap.height().cast_signed());
        let pixels = pixmap.pixels_mut();
        let mut x = origin.x as f32;
        let mut prev = None;
        for c in text.chars() {
            let id = font.glyph_id(c);
            if let Some(p) = prev {
                x += font.kern(p, id);
            }
            let glyph = id.with_scale_and_position(scale, ab_glyph::point(x, baseline));
            x += font.h_advance(id);
            prev = Some(id);
            let Some(outline) = self.0.outline_glyph(glyph) else {
                continue;
            };
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                let (px, py) = (
                    bounds.min.x as i32 + gx.cast_signed(),
                    bounds.min.y as i32 + gy.cast_signed(),
                );
                if px < 0 || py < 0 || px >= w || py >= h {
                    return;
                }
                let i = (py * w + px) as usize;
                pixels[i] = blend(pixels[i], color, coverage.min(1.0));
            });
        }
    }
}

/// `color` at `alpha` over a premultiplied pixel (source-over).
fn blend(dst: PremultipliedColorU8, c: Rgb, alpha: f32) -> PremultipliedColorU8 {
    let mix = |s: u8, d: u8| (f32::from(s) * alpha + f32::from(d) * (1.0 - alpha)).round() as u8;
    PremultipliedColorU8::from_rgba(
        mix(c.0, dst.red()),
        mix(c.1, dst.green()),
        mix(c.2, dst.blue()),
        mix(255, dst.alpha()),
    )
    .unwrap_or(dst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{PALETTE, WHITE};
    use tiny_skia::Color;

    fn white(w: u32, h: u32) -> Pixmap {
        let mut p = Pixmap::new(w, h).unwrap();
        p.fill(Color::WHITE);
        p
    }

    #[test]
    fn longer_text_is_wider_and_lines_have_height() {
        let f = TextFont::bundled();
        let short = f.measure("Hi", 20.0);
        let long = f.measure("Hi there", 20.0);
        assert!(long.width > short.width && short.width > 0.0);
        assert!(short.height > 20.0 && short.height < 40.0, "{short:?}");
        assert!(f.measure("", 20.0).width.abs() < f64::EPSILON);
    }

    #[test]
    fn draws_inside_its_measured_box_only() {
        let f = TextFont::bundled();
        let mut p = white(200, 60);
        f.draw(&mut p, "Hi", Point { x: 10.0, y: 10.0 }, 30.0, PALETTE[0]);
        let m = f.measure("Hi", 30.0);
        let reddish = |x: u32, y: u32| {
            let c = p.pixel(x, y).unwrap();
            c.red() > 200 && c.green() < 120
        };
        let inside = (10..(10 + m.width as u32))
            .any(|x| (10..(10 + m.height as u32)).any(|y| reddish(x, y)));
        assert!(inside, "no text pixels inside the measured box");
        let right = 10 + m.width as u32 + 3;
        let untouched = |x: u32, y: u32| {
            let c = p.pixel(x, y).unwrap();
            c.red() == 255 && c.green() == 255
        };
        assert!(
            (right..200).all(|x| (0..60).all(|y| untouched(x, y))),
            "text drawn past its width"
        );
    }

    #[test]
    fn text_at_the_edges_does_not_panic() {
        let f = TextFont::bundled();
        let mut p = white(50, 20);
        f.draw(&mut p, "Edge", Point { x: -8.0, y: -8.0 }, 24.0, WHITE);
        f.draw(&mut p, "Edge", Point { x: 45.0, y: 15.0 }, 24.0, WHITE);
    }

    // `size` is the em size in pixels, like a font size in points on macOS: a 100 px line of Outfit is
    // about 126 px tall (ascent + descent), not 168 (the 96/72 point-to-pixel factor applied twice).
    #[test]
    fn size_is_the_em_size_in_pixels() {
        let height = TextFont::bundled().measure("H", 100.0).height;
        assert!((115.0..140.0).contains(&height), "line height {height}");
    }
}
