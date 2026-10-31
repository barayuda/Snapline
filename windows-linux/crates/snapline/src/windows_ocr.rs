//! Windows.Media.Ocr and Windows.Media.FaceAnalysis, run on the calling (worker) thread.

use crate::recognize::Recognizer;
use snapline_overlay::Rect;
use snapline_overlay::scan::{Line, Recognized, Word, ocr_scale};
use snapline_overlay::tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::FaceAnalysis::FaceDetector;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::HSTRING;

pub struct WindowsOcr;

const NO_OCR: &str = "Windows text recognition is not set up on this PC. Add a language that includes \
                      \"Optical character recognition\" in Settings → Time & language → Language & region.";

/// The user's languages first; English when none of them has OCR (Windows has no Indonesian OCR).
fn engine() -> Result<OcrEngine, String> {
    OcrEngine::TryCreateFromUserProfileLanguages()
        .or_else(|_| {
            OcrEngine::TryCreateFromLanguage(&Language::CreateLanguage(&HSTRING::from("en-US"))?)
        })
        .map_err(|_| NO_OCR.to_owned())
}

/// A Windows bitmap of `image` (BGRA; the screenshot is opaque, so premultiplied and straight are the same).
fn bitmap(image: &Pixmap) -> windows::core::Result<SoftwareBitmap> {
    let mut bgra = image.data().to_vec();
    for px in bgra.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    let writer = DataWriter::new()?;
    writer.WriteBytes(&bgra)?;
    SoftwareBitmap::CreateCopyFromBuffer(
        &writer.DetachBuffer()?,
        BitmapPixelFormat::Bgra8,
        image.width().cast_signed(),
        image.height().cast_signed(),
    )
}

/// `image` scaled by `factor` (< 1) to fit OCR's size limit.
fn shrink(image: &Pixmap, factor: f64) -> Option<Pixmap> {
    let w = (f64::from(image.width()) * factor) as u32;
    let h = (f64::from(image.height()) * factor) as u32;
    let mut out = Pixmap::new(w.max(1), h.max(1))?;
    let paint = PixmapPaint {
        quality: FilterQuality::Bilinear,
        ..PixmapPaint::default()
    };
    let k = factor as f32;
    out.draw_pixmap(
        0,
        0,
        image.as_ref(),
        &paint,
        Transform::from_scale(k, k),
        None,
    );
    Some(out)
}

fn failed(e: &windows::core::Error) -> String {
    format!("Windows text recognition failed: {}", e.message())
}

fn faces(bitmap: &SoftwareBitmap) -> windows::core::Result<Vec<Rect>> {
    if !FaceDetector::IsSupported()? {
        return Ok(Vec::new());
    }
    let gray = SoftwareBitmap::Convert(bitmap, BitmapPixelFormat::Gray8)?;
    let detector = FaceDetector::CreateAsync()?.join()?;
    let mut found = Vec::new();
    for face in detector.DetectFacesAsync(&gray)?.join()? {
        let b = face.FaceBox()?;
        found.push(Rect::new(
            f64::from(b.X),
            f64::from(b.Y),
            f64::from(b.Width),
            f64::from(b.Height),
        ));
    }
    Ok(found)
}

impl Recognizer for WindowsOcr {
    fn recognize(&self, image: &Pixmap, find_faces: bool) -> Result<Recognized, String> {
        // SAFETY: plain call on this thread; "already initialised" is fine, so the result is ignored.
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let engine = engine()?;
        let max = OcrEngine::MaxImageDimension().map_err(|e| failed(&e))?;
        let factor = ocr_scale(image.width(), image.height(), max);
        let small;
        let source = if factor < 1.0 {
            small = shrink(image, factor).ok_or("The screen is too large to read.")?;
            &small
        } else {
            image
        };
        let bitmap = bitmap(source).map_err(|e| failed(&e))?;
        let result = engine
            .RecognizeAsync(&bitmap)
            .and_then(|op| op.join())
            .map_err(|e| failed(&e))?;
        let mut found = Recognized::default();
        for line in result.Lines().map_err(|e| failed(&e))? {
            let mut words = Vec::new();
            for word in line.Words().map_err(|e| failed(&e))? {
                let r = word.BoundingRect().map_err(|e| failed(&e))?;
                words.push(Word {
                    text: word.Text().map_err(|e| failed(&e))?.to_string_lossy(),
                    rect: Rect::new(
                        f64::from(r.X),
                        f64::from(r.Y),
                        f64::from(r.Width),
                        f64::from(r.Height),
                    ),
                });
            }
            found.lines.push(Line { words });
        }
        if find_faces {
            found.faces = faces(&bitmap).map_err(|e| failed(&e))?;
        }
        Ok(if factor < 1.0 {
            found.scaled(1.0 / factor)
        } else {
            found
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapline_overlay::Point;
    use snapline_overlay::color::BLACK;
    use snapline_overlay::text::TextFont;

    /// Temporary: fails on purpose and shows what Windows OCR sees for a few bitmap variants, to tell an empty
    /// result from a bad bitmap. Remove once the cause is known.
    #[test]
    fn ocr_diagnostics() {
        let mut out: Vec<String> = Vec::new();
        // SAFETY: plain call on the test thread.
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let mut image = Pixmap::new(900, 120).unwrap();
        image.fill(snapline_overlay::tiny_skia::Color::WHITE);
        TextFont::bundled().draw(
            &mut image,
            "Card 4111 1111 1111 1111",
            Point { x: 20.0, y: 30.0 },
            40.0,
            BLACK,
        );
        let dark = image.pixels().iter().filter(|p| p.red() < 128).count();
        out.push(format!("dark pixels {dark} of {}", image.pixels().len()));
        match engine() {
            Err(e) => out.push(format!("no engine: {e}")),
            Ok(engine) => {
                let lang = engine
                    .RecognizerLanguage()
                    .and_then(|l| l.LanguageTag())
                    .map(|t| t.to_string_lossy());
                out.push(format!(
                    "language {lang:?}, max {:?}",
                    OcrEngine::MaxImageDimension()
                ));
                let bgra = bitmap(&image).unwrap();
                out.push(format!(
                    "bitmap {:?}x{:?} {:?} {:?}",
                    bgra.PixelWidth(),
                    bgra.PixelHeight(),
                    bgra.BitmapPixelFormat(),
                    bgra.BitmapAlphaMode()
                ));
                let mut read = |b: &SoftwareBitmap, name: &str| {
                    out.push(match engine.RecognizeAsync(b).and_then(|op| op.join()) {
                        Ok(r) => format!(
                            "{name}: lines {:?}, text {:?}",
                            r.Lines().and_then(|l| l.Size()),
                            r.Text().map(|t| t.to_string_lossy())
                        ),
                        Err(e) => format!("{name}: error {e:?}"),
                    });
                };
                read(&bgra, "bgra premultiplied");
                if let Ok(gray) = SoftwareBitmap::Convert(&bgra, BitmapPixelFormat::Gray8) {
                    read(&gray, "gray8");
                }
                if let Ok(ignore) = SoftwareBitmap::ConvertWithAlpha(
                    &bgra,
                    BitmapPixelFormat::Bgra8,
                    windows::Graphics::Imaging::BitmapAlphaMode::Ignore,
                ) {
                    read(&ignore, "bgra ignore");
                }
            }
        }
        panic!("OCR diagnostics:\n{}", out.join("\n"));
    }
}
