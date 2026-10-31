//! Twin of `macOS/Sources/SnaplineCore/UncertainMarks.swift`: invisible markers (Unicode private-use
//! characters) around text the user should check. They travel through reading order and are removed
//! before anything reaches the clipboard.

use std::ops::Range;

const OPEN: char = '\u{E000}';
const CLOSE: char = '\u{E001}';

/// `s` between the markers.
#[must_use]
pub fn wrap(s: &str) -> String {
    format!("{OPEN}{s}{CLOSE}")
}

/// Whether `s` has any marked spot.
#[must_use]
pub fn contains(s: &str) -> bool {
    s.contains(OPEN)
}

/// `s` without markers (what goes to the clipboard).
#[must_use]
pub fn strip(s: &str) -> String {
    s.chars().filter(|&c| c != OPEN && c != CLOSE).collect()
}

/// Plain text plus the byte ranges (in the plain text) that were marked.
#[must_use]
pub fn extract(s: &str) -> (String, Vec<Range<usize>>) {
    let mut plain = String::new();
    let mut ranges = Vec::new();
    let mut start = None;
    for ch in s.chars() {
        if ch == OPEN {
            start = Some(plain.len());
        } else if ch == CLOSE {
            if let Some(st) = start.take() {
                ranges.push(st..plain.len());
            }
        } else {
            plain.push(ch);
        }
    }
    (plain, ranges)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked() -> String {
        format!("press {}, then {}", wrap("+9C"), wrap("�"))
    }

    #[test]
    fn wrap_and_strip() {
        assert!(contains(&marked()));
        assert_eq!(strip(&marked()), "press +9C, then �");
        assert!(!contains(&strip(&marked())));
    }

    #[test]
    fn extract_returns_plain_text_and_marked_ranges() {
        let (plain, ranges) = extract(&marked());
        assert_eq!(plain, "press +9C, then �");
        let pieces: Vec<&str> = ranges.iter().map(|r| &plain[r.clone()]).collect();
        assert_eq!(pieces, ["+9C", "�"]);
    }
}
