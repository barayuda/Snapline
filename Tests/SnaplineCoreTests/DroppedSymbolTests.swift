import CoreGraphics
import XCTest
@testable import SnaplineCore

final class DroppedSymbolTests: XCTestCase {
    /// Renders `line`, and returns it with the pixel spans of the words Vision "kept" (all words except `dropped`).
    private func line(_ text: String, dropping dropped: Set<Int>, size: CGFloat = 30) -> (CGImage, [Range<Int>]) {
        let image = SymbolRepair.render(text, size: size, foreground: 0.92, background: 0.11)
        // Group glyph pieces into words using the real character widths of each word.
        let words = text.split(separator: " ").map(String.init)
        let runs = Ink(image).columnRuns()
        var spans: [Range<Int>] = []
        var next = 0
        for (i, word) in words.enumerated() {
            let pieces = Ink(SymbolRepair.render(word, size: size, foreground: 0.92, background: 0.11)).columnRuns().count
            let span = runs[next].lowerBound..<runs[next + pieces - 1].upperBound
            next += pieces
            if !dropped.contains(i) { spans.append(span) }
        }
        return (image, spans)
    }

    func testDroppedCommandSymbolIsFoundBetweenWords() {
        let (image, kept) = line("show ⌘ ⇧ ok", dropping: [1])
        let found = SymbolRepair.droppedSymbols(in: image, keptWords: kept)
        XCTAssertEqual(found.count, 1)
        XCTAssertEqual(found.first?.afterWord, 0)
        XCTAssertEqual(found.first?.symbol, "⌘")
    }

    func testNothingIsReportedWhenNoWordWasDropped() {
        let (image, kept) = line("press ⇧⌘C then paste", dropping: [])
        XCTAssertTrue(SymbolRepair.droppedSymbols(in: image, keptWords: kept).isEmpty)
    }

    func testUnknownDroppedShapeIsReportedWithoutASymbol() {
        // "§" is not one of our symbols: report the gap so the user can check it, but do not guess.
        let (image, kept) = line("see § here", dropping: [1])
        let found = SymbolRepair.droppedSymbols(in: image, keptWords: kept)
        XCTAssertEqual(found.count, 1)
        XCTAssertNil(found.first?.symbol)
    }
}

final class UncertainMarksTests: XCTestCase {
    func testWrapStripAndFindRanges() {
        let marked = "press " + UncertainMarks.wrap("+9C") + ", then " + UncertainMarks.wrap("�")
        XCTAssertEqual(UncertainMarks.strip(marked), "press +9C, then �")
        let (plain, ranges) = UncertainMarks.extract(marked)
        XCTAssertEqual(plain, "press +9C, then �")
        XCTAssertEqual(ranges.map { (plain as NSString).substring(with: $0) }, ["+9C", "�"])
        XCTAssertTrue(UncertainMarks.contains(marked))
        XCTAssertFalse(UncertainMarks.contains(plain))
    }

    func testMarksSurviveReadingOrder() {
        let marked = "and press " + UncertainMarks.wrap("+9C") + ", then paste."
        let text = ReadingOrder.text(from: [(marked, CGRect(x: 0.1, y: 0.5, width: 0.5, height: 0.04))])
        XCTAssertEqual(UncertainMarks.extract(text).ranges.count, 1)
        XCTAssertEqual(UncertainMarks.strip(text), "and press +9C, then paste.")
    }
}

final class ShortcutWordsTests: XCTestCase {
    func testModifierCombosBecomeWords() {
        XCTAssertEqual(ShortcutWords.convert("press ⇧⌘C, then paste."), "press Shift+Cmd+C, then paste.")
        XCTAssertEqual(ShortcutWords.convert("Save As ⌥⌘S and ⌃⌥H"), "Save As Option+Cmd+S and Ctrl+Option+H")
        XCTAssertEqual(ShortcutWords.convert("⌘↩ sends, ⌘← goes home"), "Cmd+Return sends, Cmd+Left goes home")
    }

    func testStandaloneKeySymbolsBecomeWords() {
        XCTAssertEqual(ShortcutWords.convert("Press ⎋ to cancel or ↩ to confirm, ⌫ deletes"),
                       "Press Esc to cancel or Return to confirm, Delete deletes")
    }

    func testArrowsAndOtherSymbolsInProseStay() {
        XCTAssertEqual(ShortcutWords.convert("A → B, 5 ≠ 6, done ✓"), "A → B, 5 ≠ 6, done ✓")
        XCTAssertEqual(ShortcutWords.convert("plain text"), "plain text")
    }
}
