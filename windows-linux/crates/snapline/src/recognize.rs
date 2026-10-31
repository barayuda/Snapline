//! Text recognition and face detection on this PC: Windows OCR on Windows; elsewhere not yet (Linux: Phase 6).

use snapline_overlay::scan::Recognized;
use snapline_overlay::tiny_skia::Pixmap;

/// Shown in the size badge when auto-hide could not run.
pub const UNAVAILABLE: &str = "auto-hide unavailable";

pub trait Recognizer {
    /// Reads `image`; finds faces too when `faces` is true. The error is a sentence for the user.
    ///
    /// # Errors
    /// When this PC cannot recognise text (no OCR language, or not Windows).
    fn recognize(&self, image: &Pixmap, faces: bool) -> Result<Recognized, String>;
}

#[cfg(not(windows))]
struct NotYet;

#[cfg(not(windows))]
impl Recognizer for NotYet {
    fn recognize(&self, _image: &Pixmap, _faces: bool) -> Result<Recognized, String> {
        Err("Text recognition is not available on this system yet.".into())
    }
}

/// The recognizer for this system.
#[must_use]
pub fn system() -> Box<dyn Recognizer> {
    #[cfg(windows)]
    {
        Box::new(crate::windows_ocr::WindowsOcr)
    }
    #[cfg(not(windows))]
    {
        Box::new(NotYet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapline_overlay::tiny_skia::Pixmap;

    #[cfg(not(windows))]
    #[test]
    fn other_systems_say_it_is_not_available_yet() {
        let image = Pixmap::new(10, 10).unwrap();
        assert!(
            system()
                .recognize(&image, true)
                .unwrap_err()
                .contains("not available")
        );
    }

    /// Runs real Windows OCR on CI. A runner without an OCR language prints why and passes, so CI shows it.
    #[cfg(windows)]
    #[test]
    fn windows_ocr_reads_a_card_number() {
        use snapline_overlay::Point;
        use snapline_overlay::color::BLACK;
        use snapline_overlay::text::TextFont;
        let mut image = Pixmap::new(900, 120).unwrap();
        image.fill(snapline_overlay::tiny_skia::Color::WHITE);
        TextFont::bundled().draw(
            &mut image,
            "Card 4111 1111 1111 1111",
            Point { x: 20.0, y: 30.0 },
            40.0,
            BLACK,
        );
        match system().recognize(&image, false) {
            Ok(found) => {
                let text: Vec<String> = found
                    .lines
                    .iter()
                    .flat_map(|l| l.words.iter().map(|w| w.text.clone()))
                    .collect();
                assert!(text.join(" ").contains("4111"), "{text:?}");
                assert!(
                    found.lines[0].words[0].rect.x < 100.0,
                    "boxes in image pixels"
                );
            }
            Err(e) => eprintln!("skipped: Windows OCR is not available on this runner: {e}"),
        }
    }
}
