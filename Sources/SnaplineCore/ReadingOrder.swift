import CoreGraphics
import Foundation

/// Turns recognised text pieces into plain text in reading order.
public enum ReadingOrder {
    private struct Row {
        var text: String
        var box: CGRect
    }

    /// `pieces` use Vision boxes: normalized, origin bottom-left. Rows go top to bottom; pieces whose
    /// vertical centres are within half a line height share a row and are joined left to right.
    /// Lines that the screen wrapped in the middle of a sentence are joined back into one line.
    public static func text(from pieces: [(String, CGRect)]) -> String {
        var groups: [[(String, CGRect)]] = []
        for piece in pieces.map({ (tidy($0.0), $0.1) }).sorted(by: { $0.1.midY > $1.1.midY }) {
            if let first = groups.last?.first,
               abs(first.1.midY - piece.1.midY) < min(first.1.height, piece.1.height) / 2 {
                groups[groups.count - 1].append(piece)
            } else {
                groups.append([piece])
            }
        }

        let rows = groups.map { group -> Row in
            let sorted = group.sorted { $0.1.minX < $1.1.minX }
            return Row(text: sorted.map(\.0).reduce("") { join($0, $1, separator: " ") },
                       box: sorted.map(\.1).reduce(sorted[0].1) { $0.union($1) })
        }

        let widest = rows.map(\.box.width).max() ?? 0
        var lines: [String] = []
        for (i, row) in rows.enumerated() {
            if i > 0, continues(rows[i - 1], row, widest: widest) {
                lines[lines.count - 1] = join(lines[lines.count - 1], row.text, separator: " ")
            } else {
                lines.append(row.text)
            }
        }
        return lines.joined(separator: "\n")
    }

    /// Vision puts a space between inline code and the punctuation after it ("a21e64e . The").
    /// Drop a single space before closing punctuation that ends a word.
    private static let spaceBeforePunctuation = try! NSRegularExpression(pattern: #"(?<=\S) (?=[.,;:!?)\]}…”’»](?:\s|$))"#)

    static func tidy(_ s: String) -> String {
        spaceBeforePunctuation.stringByReplacingMatches(in: s, range: NSRange(s.startIndex..., in: s), withTemplate: "")
    }

    /// No space before closing punctuation (Vision often splits inline code from the "." after it).
    private static func join(_ a: String, _ b: String, separator: String) -> String {
        if a.isEmpty { return b }
        if let first = b.first, ".,;:!?)]}%…”’»".contains(first) { return a + b }
        return a + separator + b
    }

    /// A row continues the one above when it looks like a wrapped sentence: directly below it, same left
    /// edge, the row above is nearly full width and does not end a sentence, and this row starts in lowercase
    /// or with a keyboard symbol ("…and press" / "⇧⌘C, then paste.").
    /// Lists (short lines, or lines starting with a capital) stay one item per line.
    private static func continues(_ above: Row, _ row: Row, widest: CGFloat) -> Bool {
        let h = min(above.box.height, row.box.height)
        guard abs(above.box.minX - row.box.minX) < h * 2,
              above.box.minY - row.box.maxY < h * 0.8,
              above.box.width >= widest * 0.6,
              let first = row.text.first, first.isLowercase || SymbolRepair.symbols.contains(first),
              let last = above.text.last, !".!?:".contains(last) else { return false }
        return true
    }
}
