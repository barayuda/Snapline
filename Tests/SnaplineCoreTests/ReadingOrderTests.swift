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

    func testEmptyInput() {
        XCTAssertEqual(ReadingOrder.text(from: []), "")
    }
}
