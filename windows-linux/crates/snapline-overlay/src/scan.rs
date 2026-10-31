//! From recognised text and faces to the boxes auto-hide covers, and to the text Copy Text copies.

use snapline_core::reading_order;
use snapline_core::selection_geometry::{Rect, slice};
use snapline_core::sensitive_text::{SensitiveCategory, matches};
use std::ops::Range;

/// An area auto-hide found. `hidden` is false after the user clicked it to show it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HideBox {
    pub rect: Rect,
    pub category: SensitiveCategory,
    pub hidden: bool,
}

/// A recognised word and its box.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Word {
    pub text: String,
    pub rect: Rect,
}

/// A recognised line: its words, left to right.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub words: Vec<Word>,
}

/// What text recognition and face detection found, in the pixels of the image that was read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recognized {
    pub lines: Vec<Line>,
    pub faces: Vec<Rect>,
}

fn grow(r: Rect, dx: f64, dy: f64) -> Rect {
    Rect::new(r.x - dx, r.y - dy, r.width + 2.0 * dx, r.height + 2.0 * dy)
}

fn scale_rect(r: Rect, f: f64) -> Rect {
    Rect::new(r.x * f, r.y * f, r.width * f, r.height * f)
}

impl Line {
    /// The words joined by single spaces, and each word's byte range in that text.
    fn text(&self) -> (String, Vec<Range<usize>>) {
        let mut text = String::new();
        let mut ranges = Vec::new();
        for (k, w) in self.words.iter().enumerate() {
            if k > 0 {
                text.push(' ');
            }
            ranges.push(text.len()..text.len() + w.text.len());
            text.push_str(&w.text);
        }
        (text, ranges)
    }

    /// The box of the bytes `m` of `text` (from `self.text()`): one word, part of one word, or several words.
    fn match_box(&self, text: &str, ranges: &[Range<usize>], m: &Range<usize>) -> Option<Rect> {
        let touched: Vec<usize> = (0..ranges.len())
            .filter(|&k| ranges[k].start < m.end && m.start < ranges[k].end)
            .collect();
        match touched.as_slice() {
            [] => None,
            &[k] => {
                let (word, range) = (&self.words[k], &ranges[k]);
                let (start, end) = (m.start.max(range.start), m.end.min(range.end));
                // `slice` counts characters, not bytes.
                let chars = text[range.start..start].chars().count()
                    ..text[range.start..end].chars().count();
                let count = word.text.chars().count();
                Some(if chars.start == 0 && chars.end == count {
                    word.rect
                } else {
                    slice(word.rect, chars, count)
                })
            }
            many => many.iter().map(|&k| self.words[k].rect).reduce(Rect::union),
        }
    }
}

impl Recognized {
    /// Every rect multiplied by `factor` (to map OCR on a scaled-down image back to the screen).
    #[must_use]
    pub fn scaled(self, factor: f64) -> Self {
        Self {
            lines: self
                .lines
                .into_iter()
                .map(|l| Line {
                    words: l
                        .words
                        .into_iter()
                        .map(|w| Word {
                            rect: scale_rect(w.rect, factor),
                            ..w
                        })
                        .collect(),
                })
                .collect(),
            faces: self
                .faces
                .into_iter()
                .map(|r| scale_rect(r, factor))
                .collect(),
        }
    }
}

/// How much to scale an image so its longer side fits Windows OCR's limit (1.0 when it already fits).
#[must_use]
pub fn ocr_scale(width: u32, height: u32, max_dimension: u32) -> f64 {
    let longer = width.max(height).max(1);
    (f64::from(max_dimension) / f64::from(longer)).min(1.0)
}

/// The boxes auto-hide covers: sensitive text in `categories`, and faces when `Face` is one of them.
/// `scale` is pixels per point, for the padding.
#[must_use]
pub fn hide_boxes(
    found: &Recognized,
    categories: &[SensitiveCategory],
    scale: f64,
) -> Vec<HideBox> {
    let text_categories: Vec<SensitiveCategory> = categories
        .iter()
        .copied()
        .filter(|c| *c != SensitiveCategory::Face)
        .collect();
    let mut boxes = Vec::new();
    if !text_categories.is_empty() {
        for line in &found.lines {
            let (text, ranges) = line.text();
            for m in matches(&text, &text_categories) {
                if let Some(rect) = line.match_box(&text, &ranges, &m.range) {
                    boxes.push(HideBox {
                        rect: grow(rect, 3.0 * scale, 2.0 * scale),
                        category: m.category,
                        hidden: true,
                    });
                }
            }
        }
    }
    if categories.contains(&SensitiveCategory::Face) {
        for face in &found.faces {
            // Face boxes are tight around the features; grow them to cover hair and chin.
            boxes.push(HideBox {
                rect: grow(*face, face.width * 0.2, face.height * 0.3),
                category: SensitiveCategory::Face,
                hidden: true,
            });
        }
    }
    boxes
}

/// The recognised text in reading order (wrapped lines joined back, as on macOS), or "".
#[must_use]
pub fn copy_text(found: &Recognized, width: f64, height: f64) -> String {
    // `reading_order` takes Vision-style boxes: normalised, origin at the bottom left.
    let pieces: Vec<(String, Rect)> = found
        .lines
        .iter()
        .filter_map(|line| {
            let r = line.words.iter().map(|w| w.rect).reduce(Rect::union)?;
            let normalized = Rect::new(
                r.x / width,
                1.0 - (r.y + r.height) / height,
                r.width / width,
                r.height / height,
            );
            Some((line.text().0, normalized))
        })
        .collect();
    reading_order::text(&pieces)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: [SensitiveCategory; 3] = [
        SensitiveCategory::Contact,
        SensitiveCategory::Payment,
        SensitiveCategory::Secret,
    ];

    /// One line whose words are 10 px per character, 4 px apart, starting at x = 100, y = 50, 20 px tall.
    fn line(text: &str) -> Line {
        let mut x = 100.0;
        let words = text
            .split(' ')
            .map(|w| {
                let width = 10.0 * w.chars().count() as f64;
                let word = Word {
                    text: w.into(),
                    rect: Rect::new(x, 50.0, width, 20.0),
                };
                x += width + 4.0;
                word
            })
            .collect();
        Line { words }
    }

    fn one(text: &str) -> Recognized {
        Recognized {
            lines: vec![line(text)],
            faces: vec![],
        }
    }

    #[test]
    fn a_card_number_spanning_words_gets_one_box_over_all_of_them() {
        let boxes = hide_boxes(&one("Card 4111 1111 1111 1111"), &TEXT, 1.0);
        assert_eq!(boxes.len(), 1);
        let b = boxes[0];
        assert_eq!(b.category, SensitiveCategory::Payment);
        assert!(b.hidden);
        // "Card" is 40 px + 4 gap: the card starts at x = 144; four 40 px groups with 4 px gaps end at 316.
        assert!(
            (b.rect.x - (144.0 - 3.0)).abs() < 1e-9
                && (b.rect.x + b.rect.width - (316.0 + 3.0)).abs() < 1e-9,
            "{b:?}"
        );
        assert!((b.rect.y - 48.0).abs() < 1e-9 && (b.rect.height - 24.0).abs() < 1e-9);
    }

    #[test]
    fn part_of_a_word_is_cut_out_of_the_word_box() {
        let key = format!(
            "API_KEY={}",
            ["sk_live_", "4eC39HqLyjWDarjtT1zdp7dc"].concat()
        );
        let boxes = hide_boxes(&one(&key), &TEXT, 1.0);
        assert_eq!(boxes.len(), 1);
        let word = line(&key).words[0].rect;
        assert!(
            boxes[0].rect.x > word.x + 30.0,
            "the box starts after \"API_KEY=\": {:?}",
            boxes[0].rect
        );
    }

    // Review Focus 1.
    #[test]
    fn boxes_stay_on_the_value_after_multibyte_text() {
        let plain = hide_boxes(&one("Cafe: 4111 1111 1111 1111"), &TEXT, 1.0);
        let accented = hide_boxes(&one("Café: 4111 1111 1111 1111"), &TEXT, 1.0);
        assert_eq!(
            plain[0].rect, accented[0].rect,
            "\"é\" is 2 bytes but 1 character"
        );
        let key = format!("—key={}", ["sk_live_", "4eC39HqLyjWDarjtT1zdp7dc"].concat());
        let word = line(&key).words[0].rect;
        let b = hide_boxes(&one(&key), &TEXT, 1.0)[0].rect;
        assert!(
            b.x > word.x + 30.0 && b.x + b.width <= word.x + word.width + 15.0,
            "{b:?} in {word:?}"
        );
    }

    #[test]
    fn faces_grow_and_only_when_asked() {
        let found = Recognized {
            lines: vec![],
            faces: vec![Rect::new(100.0, 100.0, 50.0, 60.0)],
        };
        assert_eq!(hide_boxes(&found, &TEXT, 1.0), []);
        let boxes = hide_boxes(&found, &[SensitiveCategory::Face], 1.0);
        assert_eq!(boxes[0].rect, Rect::new(90.0, 82.0, 70.0, 96.0));
        assert_eq!(boxes[0].category, SensitiveCategory::Face);
    }

    #[test]
    fn copy_text_reads_lines_in_order() {
        let found = Recognized {
            lines: vec![
                Line {
                    words: vec![Word {
                        text: "second".into(),
                        rect: Rect::new(10.0, 60.0, 60.0, 20.0),
                    }],
                },
                Line {
                    words: vec![Word {
                        text: "first".into(),
                        rect: Rect::new(10.0, 20.0, 50.0, 20.0),
                    }],
                },
            ],
            faces: vec![],
        };
        assert_eq!(copy_text(&found, 200.0, 100.0), "first\nsecond");
        assert_eq!(copy_text(&Recognized::default(), 200.0, 100.0), "");
    }

    // Review Focus 4.
    #[test]
    fn big_images_are_scaled_for_ocr_and_boxes_scaled_back() {
        assert!((ocr_scale(7680, 4320, 10_000) - 1.0).abs() < f64::EPSILON);
        let factor = ocr_scale(15_360, 8640, 10_000);
        assert!((factor - 10_000.0 / 15_360.0).abs() < 1e-12);
        let read = Recognized {
            lines: vec![line("x")],
            faces: vec![Rect::new(10.0, 20.0, 30.0, 40.0)],
        };
        let back = read.scaled(1.0 / factor);
        assert!((back.faces[0].x - 10.0 / factor).abs() < 1e-9);
        assert!((back.lines[0].words[0].rect.width - 10.0 / factor).abs() < 1e-9);
    }
}
