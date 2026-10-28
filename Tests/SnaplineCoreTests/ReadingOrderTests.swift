import XCTest
@testable import SnaplineCore

final class ReadingOrderTests: XCTestCase {
    // Boxes are Vision-style: normalized, origin bottom-left (higher y = higher on screen).
    private func box(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat = 0.2, _ h: CGFloat = 0.04) -> CGRect {
        CGRect(x: x, y: y, width: w, height: h)
    }

    func testLinesGoTopToBottom() {
        let text = ReadingOrder.text(from: [
            ("second line", box(0.1, 0.50)),
            ("first line", box(0.1, 0.80)),
            ("third line", box(0.1, 0.20)),
        ])
        XCTAssertEqual(text, "first line\nsecond line\nthird line")
    }

    func testPiecesOnTheSameRowJoinLeftToRight() {
        // Slightly different baselines still count as one row.
        let text = ReadingOrder.text(from: [
            ("Total", box(0.1, 0.600)),
            ("IDR 1.250.000", box(0.6, 0.605)),
            ("Guest", box(0.1, 0.700)),
            ("Jane Doe", box(0.6, 0.698)),
        ])
        XCTAssertEqual(text, "Guest Jane Doe\nTotal IDR 1.250.000")
    }

    func testNoSpaceBeforePunctuationAfterACodeSpan() {
        // Vision often splits inline code from the following text: "a21e64e" + ". The commit".
        let text = ReadingOrder.text(from: [
            ("pushed as", box(0.05, 0.5, 0.2)),
            ("a21e64e", box(0.27, 0.5, 0.1)),
            (". The commit", box(0.38, 0.5, 0.2)),
        ])
        XCTAssertEqual(text, "pushed as a21e64e. The commit")
    }

    func testNoSpaceBeforePunctuationInsideOnePiece() {
        // Real Vision output for a line with an inline code span.
        XCTAssertEqual(ReadingOrder.text(from: [("pushed to GitHub as a21e64e . The commit is", box(0.05, 0.5, 0.8))]),
                       "pushed to GitHub as a21e64e. The commit is")
        XCTAssertEqual(ReadingOrder.text(from: [("1 + 2 = 3 and 50 % off", box(0.05, 0.5, 0.8))]),
                       "1 + 2 = 3 and 50 % off")
    }

    func testWrappedLinesJoinIntoOneParagraph() {
        // Two full-width lines that continue in lowercase, then a short last line, then a new paragraph.
        let text = ReadingOrder.text(from: [
            ("The commit is signed with your key and has no", box(0.05, 0.80, 0.85)),
            ("co-author line. Your local main and origin are", box(0.05, 0.75, 0.84)),
            ("in sync.", box(0.05, 0.70, 0.15)),
            ("Copy Text has not been tried yet.", box(0.05, 0.55, 0.6)),
        ])
        XCTAssertEqual(text, """
            The commit is signed with your key and has no co-author line. Your local main and origin are in sync.
            Copy Text has not been tried yet.
            """)
    }

    func testWrappedLineStartingWithKeyboardSymbolJoins() {
        let text = ReadingOrder.text(from: [
            ("To test it, select an area with text and press", box(0.05, 0.80, 0.85)),
            ("⇧⌘C, then paste.", box(0.05, 0.75, 0.25)),
        ])
        XCTAssertEqual(text, "To test it, select an area with text and press ⇧⌘C, then paste.")
    }

    func testListLinesStaySeparate() {
        // Close together and left-aligned, but each item starts with a capital: a list, not a wrapped sentence.
        let text = ReadingOrder.text(from: [
            ("Save As: ⌥⌘S", box(0.05, 0.80, 0.3)),
            ("Undo: ⌘Z", box(0.05, 0.75, 0.3)),
            ("Hide: ⌃⌥H", box(0.05, 0.70, 0.3)),
        ])
        XCTAssertEqual(text, "Save As: ⌥⌘S\nUndo: ⌘Z\nHide: ⌃⌥H")
    }

    func testEmptyInput() {
        XCTAssertEqual(ReadingOrder.text(from: []), "")
    }
}
