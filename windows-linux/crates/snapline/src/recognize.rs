//! Text recognition and face detection on this PC: Windows OCR on Windows; elsewhere not yet (Linux: Phase 6).

use snapline_overlay::Rect;
use snapline_overlay::scan::{Line, Recognized};
use snapline_overlay::tiny_skia::Pixmap;

/// Shown in the size badge when auto-hide could not run.
pub const UNAVAILABLE: &str = "auto-hide unavailable";
/// Shown in the size badge when text or face detection worked but the other did not.
pub const INCOMPLETE: &str = "auto-hide incomplete";
/// Shown in the size badge when the scan itself broke (a bug in Snapline, not missing OCR).
pub const FAILED: &str = "auto-hide failed";
/// The scan's error when its thread panicked.
pub const CRASHED: &str = "the scan stopped unexpectedly";

/// The badge for a scan that ended with the error `why`.
#[must_use]
pub fn badge_for(why: &str) -> &'static str {
    if why == CRASHED { FAILED } else { UNAVAILABLE }
}

/// What a read found, and why a step is missing when only some of the steps worked.
pub struct Reading {
    pub found: Recognized,
    pub problem: Option<String>,
}

/// Why Windows text recognition cannot run: no OCR language (`no_language`), or OCR itself is missing.
#[must_use]
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "used by the Windows recognizer")
)]
pub fn ocr_unavailable(no_language: bool) -> &'static str {
    if no_language {
        "Windows text recognition is not set up on this PC. Add a language that includes \
         \"Optical character recognition\" in Settings → Time & language → Language & region."
    } else {
        "Windows text recognition is not available on this PC. On Windows N editions, install the \
         Media Feature Pack from Settings → Apps → Optional features."
    }
}

pub trait Recognizer {
    /// Reads the text in `image` when `text` is true, and finds faces when `faces` is true.
    ///
    /// # Errors
    /// When every step asked for failed (no OCR language, not Windows); the error is a sentence for the user.
    fn recognize(&self, image: &Pixmap, text: bool, faces: bool) -> Result<Reading, String>;
}

/// Puts the text and face steps together (`None`: not asked for). A step that failed leaves a `problem` and the
/// other step's results stand, so a missing face detector never costs the text boxes. Fails only when every
/// step asked for failed.
///
/// # Errors
/// The first step's error, when no step worked.
#[cfg_attr(
    not(windows),
    allow(
        dead_code,
        reason = "only the Windows recognizer reads two steps so far"
    )
)]
pub fn merge(
    text: Option<Result<Vec<Line>, String>>,
    faces: Option<Result<Vec<Rect>, String>>,
) -> Result<Reading, String> {
    let asked = usize::from(text.is_some()) + usize::from(faces.is_some());
    let mut found = Recognized::default();
    let mut problems = Vec::new();
    match text {
        Some(Ok(lines)) => found.lines = lines,
        Some(Err(e)) => problems.push(e),
        None => {}
    }
    match faces {
        Some(Ok(rects)) => found.faces = rects,
        Some(Err(e)) => problems.push(e),
        None => {}
    }
    if asked > 0 && problems.len() == asked {
        return Err(problems.swap_remove(0));
    }
    Ok(Reading {
        found,
        problem: (!problems.is_empty()).then(|| problems.join(" ")),
    })
}

#[cfg(not(windows))]
struct NotYet;

#[cfg(not(windows))]
impl Recognizer for NotYet {
    fn recognize(&self, _image: &Pixmap, _text: bool, _faces: bool) -> Result<Reading, String> {
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
    use snapline_overlay::Rect;
    use snapline_overlay::scan::{Line, Word};

    // Phase 5c: a crashed scan said "auto-hide unavailable", as if OCR were missing.
    #[test]
    fn a_crashed_scan_has_its_own_badge() {
        assert_eq!(badge_for(CRASHED), FAILED);
        assert_eq!(badge_for(ocr_unavailable(true)), UNAVAILABLE);
    }

    fn line(text: &str) -> Line {
        Line {
            words: vec![Word {
                text: text.into(),
                rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            }],
        }
    }

    // Review finding: one failed step must not throw away what the other found.
    #[test]
    fn a_failed_step_keeps_the_other_steps_results() {
        let face = Rect::new(1.0, 2.0, 3.0, 4.0);
        let r = merge(Some(Ok(vec![line("4111")])), Some(Err("no faces".into()))).unwrap();
        assert_eq!(r.found.lines, [line("4111")]);
        assert_eq!(r.problem.as_deref(), Some("no faces"));
        let r = merge(Some(Err("no OCR".into())), Some(Ok(vec![face]))).unwrap();
        assert_eq!(r.found.faces, [face]);
        assert_eq!(r.problem.as_deref(), Some("no OCR"));
    }

    #[test]
    fn it_fails_only_when_every_step_asked_for_fails() {
        assert_eq!(
            merge(Some(Err("no OCR".into())), Some(Err("no faces".into()))).err(),
            Some("no OCR".into())
        );
        assert_eq!(
            merge(Some(Err("no OCR".into())), None).err(),
            Some("no OCR".into())
        );
        let only_faces = merge(None, Some(Ok(vec![]))).unwrap();
        assert!(only_faces.problem.is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn other_systems_say_it_is_not_available_yet() {
        let image = Pixmap::new(10, 10).unwrap();
        assert!(
            system()
                .recognize(&image, true, true)
                .err()
                .unwrap()
                .contains("not available")
        );
    }

    /// Runs real Windows OCR. Not on CI: GitHub's Windows runner has the en-US OCR data and reads a valid bitmap,
    /// yet finds no text (31 Oct 2026), so run it on a Windows PC: `cargo test -p snapline -- --ignored`.
    /// A PC without an OCR language prints why and passes.
    #[cfg(windows)]
    #[test]
    #[ignore = "Windows OCR finds no text on the GitHub runner; run on a Windows PC"]
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
        match system().recognize(&image, true, false) {
            Ok(Reading { found, .. }) => {
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

    #[test]
    fn the_ocr_message_says_what_to_do() {
        assert!(ocr_unavailable(true).contains("Language & region"));
        assert!(ocr_unavailable(false).contains("Media Feature Pack"));
    }
}
