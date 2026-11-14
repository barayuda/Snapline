import CoreGraphics
import Foundation
import Testing
@testable import SnaplineCore

/// Serialized for the same CoreText font-fallback reason as `SymbolRepairTests`.
@Suite(.serialized)
struct DroppedSymbolTests {
    /// Renders `text`, and returns it with the pixel spans of the words Vision "kept" (all words except `dropped`).
    private func line(_ text: String, dropping dropped: Set<Int>, size: CGFloat = 30) throws -> (CGImage, [Range<Int>]) {
        let image = SymbolRepair.render(text, size: size, foreground: 0.92, background: 0.11)
        // Group glyph pieces into words, using how many pieces each word has on its own.
        let runs = Ink(image).columnRuns()
        var spans: [Range<Int>] = []
        var next = 0
        for (i, word) in text.split(separator: " ").enumerated() {
            let pieces = Ink(SymbolRepair.render(String(word), size: size, foreground: 0.92, background: 0.11)).columnRuns().count
            try #require(next + pieces <= runs.count, "word \(word) does not line up with the image")
            let span = runs[next].lowerBound..<runs[next + pieces - 1].upperBound
            next += pieces
            if !dropped.contains(i) { spans.append(span) }
        }
        return (image, spans)
    }

    @Test func droppedCommandSymbolIsFoundBetweenWords() throws {
        let (image, kept) = try line("show ⌘ ⇧ ok", dropping: [1])
        #expect(SymbolRepair.droppedSymbols(in: image, keptWords: kept) == [SymbolRepair.Dropped(afterWord: 0, symbol: "⌘")])
    }

    @Test func nothingIsReportedWhenNoWordWasDropped() throws {
        let (image, kept) = try line("press ⇧⌘C then paste", dropping: [])
        #expect(SymbolRepair.droppedSymbols(in: image, keptWords: kept).isEmpty)
    }

    @Test func unknownDroppedShapeIsReportedWithoutASymbol() throws {
        // "§" is not one of our symbols: report the gap so the user can check it, but do not guess.
        let (image, kept) = try line("see § here", dropping: [1])
        #expect(SymbolRepair.droppedSymbols(in: image, keptWords: kept) == [SymbolRepair.Dropped(afterWord: 0, symbol: nil)])
    }
}

struct UncertainMarksTests {
    @Test func wrapAndStrip() {
        let marked = "press " + UncertainMarks.wrap("+9C") + ", then " + UncertainMarks.wrap("�")
        #expect(UncertainMarks.contains(marked))
        #expect(UncertainMarks.strip(marked) == "press +9C, then �")
        #expect(!UncertainMarks.contains(UncertainMarks.strip(marked)))
    }

    @Test func extractReturnsPlainTextAndMarkedRanges() {
        let marked = "press " + UncertainMarks.wrap("+9C") + ", then " + UncertainMarks.wrap("�")
        let (plain, ranges) = UncertainMarks.extract(marked)
        #expect(plain == "press +9C, then �")
        #expect(ranges.map { (plain as NSString).substring(with: $0) } == ["+9C", "�"])
    }

    @Test func marksSurviveReadingOrder() {
        let marked = "and press " + UncertainMarks.wrap("+9C") + ", then paste."
        let text = ReadingOrder.text(from: [(marked, CGRect(x: 0.1, y: 0.5, width: 0.5, height: 0.04))])
        #expect(UncertainMarks.extract(text).ranges.count == 1)
        #expect(UncertainMarks.strip(text) == "and press +9C, then paste.")
    }
}

struct ShortcutWordsTests {
    @Test(arguments: [
        ("press ⇧⌘C, then paste.", "press Shift+Cmd+C, then paste."),
        ("Save As ⌥⌘S and ⌃⌥H", "Save As Option+Cmd+S and Ctrl+Option+H"),
        ("⌘↩ sends, ⌘← goes home", "Cmd+Return sends, Cmd+Left goes home"),
        ("Press ⎋ to cancel or ↩ to confirm, ⌫ deletes", "Press Esc to cancel or Return to confirm, Delete deletes"),
        ("A → B, 5 ≠ 6, done ✓", "A → B, 5 ≠ 6, done ✓"),   // arrows and symbols in prose stay
        ("plain text", "plain text"),
    ])
    func convertsKeyboardSymbolsToWords(input: String, expected: String) {
        #expect(ShortcutWords.convert(input) == expected)
    }
}
