//! Twin of `macOS/Sources/SnaplineCore/ReadingOrder.swift`: turns recognised text pieces into plain text
//! in reading order.

use crate::selection_geometry::Rect;
use crate::uncertain_marks;

/// Keyboard symbols plus a few text symbols, the same list as `SymbolRepair.symbols` on macOS.
/// A wrapped line may start with one ("⇧⌘C, then paste."). The `SymbolRepair` port should use this list.
pub const KEYBOARD_SYMBOLS: [char; 24] = [
    '⌘', '⇧', '⌥', '⌃', '↩', '⏎', '⎋', '⇥', '⇤', '⌫', '⌦', '⇪', '⇞', '⇟', '↖', '↘', '⏏', '↑', '↓',
    '←', '→', '✓', '≠', '±',
];

/// `pieces` use OCR boxes: normalized, origin bottom-left. Rows go top to bottom; pieces whose
/// vertical centres are within half a line height share a row and are joined left to right.
/// Lines that the screen wrapped in the middle of a sentence are joined back into one line.
#[must_use]
pub fn text(pieces: &[(String, Rect)]) -> String {
    let mut sorted: Vec<(String, Rect)> = pieces.iter().map(|(t, b)| (tidy(t), *b)).collect();
    sorted.sort_by(|a, b| b.1.mid_y().total_cmp(&a.1.mid_y()));

    let mut groups: Vec<Vec<(String, Rect)>> = Vec::new();
    for piece in sorted {
        match groups.last_mut() {
            Some(group)
                if (group[0].1.mid_y() - piece.1.mid_y()).abs()
                    < group[0].1.height.min(piece.1.height) / 2.0 =>
            {
                group.push(piece);
            }
            _ => groups.push(vec![piece]),
        }
    }

    let rows: Vec<Row> = groups
        .into_iter()
        .map(|mut group| {
            group.sort_by(|a, b| a.1.min_x().total_cmp(&b.1.min_x()));
            let first = group[0].1;
            Row {
                text: group
                    .iter()
                    .fold(String::new(), |acc, (t, _)| join(&acc, t)),
                bbox: group.iter().fold(first, |acc, (_, b)| acc.union(*b)),
            }
        })
        .collect();

    let widest = rows.iter().map(|r| r.bbox.width).fold(0.0, f64::max);
    let mut lines: Vec<String> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        match lines.last_mut() {
            Some(last) if i > 0 && continues(&rows[i - 1], row, widest) => {
                *last = join(last, &row.text);
            }
            _ => lines.push(row.text.clone()),
        }
    }
    lines.join("\n")
}

struct Row {
    text: String,
    bbox: Rect,
}

/// OCR puts a space between inline code and the punctuation after it ("a21e64e . The").
/// Drop a single space before closing punctuation that ends a word.
static SPACE_BEFORE_PUNCTUATION: std::sync::LazyLock<fancy_regex::Regex> =
    std::sync::LazyLock::new(|| {
        fancy_regex::Regex::new(r"(?<=\S) (?=[.,;:!?)\]}…”’»](?:\s|$))").expect("valid pattern")
    });

fn tidy(s: &str) -> String {
    SPACE_BEFORE_PUNCTUATION.replace_all(s, "").into_owned()
}

/// Joins with a space, but no space before closing punctuation (OCR often splits inline code from the "." after it).
fn join(a: &str, b: &str) -> String {
    if a.is_empty() {
        return b.to_owned();
    }
    if b.starts_with(|c| ".,;:!?)]}%…”’»".contains(c)) {
        return format!("{a}{b}");
    }
    format!("{a} {b}")
}

/// A row continues the one above when it looks like a wrapped sentence: directly below it, same left
/// edge, the row above is nearly full width and does not end a sentence, and this row starts in lowercase
/// or with a keyboard symbol ("…and press" / "⇧⌘C, then paste.").
/// Lists (short lines, or lines starting with a capital) stay one item per line.
fn continues(above: &Row, row: &Row, widest: f64) -> bool {
    let h = above.bbox.height.min(row.bbox.height);
    let starts_like_a_continuation = uncertain_marks::strip(&row.text)
        .chars()
        .next()
        .is_some_and(|c| c.is_lowercase() || KEYBOARD_SYMBOLS.contains(&c));
    let above_ends_open = uncertain_marks::strip(&above.text)
        .chars()
        .next_back()
        .is_some_and(|c| !".!?:".contains(c));
    (above.bbox.min_x() - row.bbox.min_x()).abs() < h * 2.0
        && above.bbox.min_y() - row.bbox.max_y() < h * 0.8
        && above.bbox.width >= widest * 0.6
        && starts_like_a_continuation
        && above_ends_open
}

#[cfg(test)]
mod tests {
    use super::*;

    // Boxes are OCR-style: normalized, origin bottom-left (higher y = higher on screen).
    fn piece(text: &str, x: f64, y: f64, w: f64) -> (String, Rect) {
        (text.to_owned(), Rect::new(x, y, w, 0.04))
    }

    #[test]
    fn lines_go_top_to_bottom() {
        let text = text(&[
            piece("second line", 0.1, 0.50, 0.2),
            piece("first line", 0.1, 0.80, 0.2),
            piece("third line", 0.1, 0.20, 0.2),
        ]);
        assert_eq!(text, "first line\nsecond line\nthird line");
    }

    #[test]
    fn pieces_on_the_same_row_join_left_to_right() {
        // Slightly different baselines still count as one row.
        let text = text(&[
            piece("Total", 0.1, 0.600, 0.2),
            piece("IDR 1.250.000", 0.6, 0.605, 0.2),
            piece("Guest", 0.1, 0.700, 0.2),
            piece("Jane Doe", 0.6, 0.698, 0.2),
        ]);
        assert_eq!(text, "Guest Jane Doe\nTotal IDR 1.250.000");
    }

    #[test]
    fn no_space_before_punctuation_after_a_code_span() {
        // OCR often splits inline code from the following text: "a21e64e" + ". The commit".
        let text = text(&[
            piece("pushed as", 0.05, 0.5, 0.2),
            piece("a21e64e", 0.27, 0.5, 0.1),
            piece(". The commit", 0.38, 0.5, 0.2),
        ]);
        assert_eq!(text, "pushed as a21e64e. The commit");
    }

    #[test]
    fn no_space_before_punctuation_inside_one_piece() {
        // Real Vision output for a line with an inline code span.
        assert_eq!(
            text(&[piece(
                "pushed to GitHub as a21e64e . The commit is",
                0.05,
                0.5,
                0.8
            )]),
            "pushed to GitHub as a21e64e. The commit is"
        );
        assert_eq!(
            text(&[piece("1 + 2 = 3 and 50 % off", 0.05, 0.5, 0.8)]),
            "1 + 2 = 3 and 50 % off"
        );
    }

    #[test]
    fn wrapped_lines_join_into_one_paragraph() {
        // Two full-width lines that continue in lowercase, then a short last line, then a new paragraph.
        let text = text(&[
            piece(
                "The commit is signed with your key and has no",
                0.05,
                0.80,
                0.85,
            ),
            piece(
                "co-author line. Your local main and origin are",
                0.05,
                0.75,
                0.84,
            ),
            piece("in sync.", 0.05, 0.70, 0.15),
            piece("Copy Text has not been tried yet.", 0.05, 0.55, 0.6),
        ]);
        assert_eq!(
            text,
            "The commit is signed with your key and has no co-author line. Your local main and origin are in sync.\n\
             Copy Text has not been tried yet."
        );
    }

    #[test]
    fn wrapped_line_starting_with_keyboard_symbol_joins() {
        let text = text(&[
            piece(
                "To test it, select an area with text and press",
                0.05,
                0.80,
                0.85,
            ),
            piece("⇧⌘C, then paste.", 0.05, 0.75, 0.25),
        ]);
        assert_eq!(
            text,
            "To test it, select an area with text and press ⇧⌘C, then paste."
        );
    }

    #[test]
    fn list_lines_stay_separate() {
        // Close together and left-aligned, but each item starts with a capital: a list, not a wrapped sentence.
        let text = text(&[
            piece("Save As: ⌥⌘S", 0.05, 0.80, 0.3),
            piece("Undo: ⌘Z", 0.05, 0.75, 0.3),
            piece("Hide: ⌃⌥H", 0.05, 0.70, 0.3),
        ]);
        assert_eq!(text, "Save As: ⌥⌘S\nUndo: ⌘Z\nHide: ⌃⌥H");
    }

    #[test]
    fn empty_input() {
        assert_eq!(text(&[]), "");
    }

    #[test]
    fn marks_survive_reading_order() {
        let marked = format!("and press {}, then paste.", uncertain_marks::wrap("+9C"));
        let text = text(&[piece(&marked, 0.1, 0.5, 0.5)]);
        assert_eq!(uncertain_marks::extract(&text).1.len(), 1);
        assert_eq!(uncertain_marks::strip(&text), "and press +9C, then paste.");
    }
}
